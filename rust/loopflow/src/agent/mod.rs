//! Agent invocation for spawning coding agent runners (Claude, Codex, OpenCode).
//!
//! This module handles building commands and spawning subprocesses for each
//! supported coding agent. Output can be captured or streamed.

mod session;
pub(crate) use session::{
    build_session_command, native_connection_command, resolve_session_config, session_environment,
    NativeConnection, ProviderInvocation, SessionCommand,
};

use crate::id::AgentSessionId;
use std::collections::BTreeMap;
use std::env;
use std::fs;
use std::io::{Seek, Write};
use std::path::{Path, PathBuf};
use std::process::{Child, Command};
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::{Mutex, OnceLock};
use std::thread;
use std::time::{Duration, Instant};

use crate::config::{default_agent, parse_agent};
use crate::error::CoreError;
use crate::os_process::wait_for_exit;
use crate::platform::kill_process;
use crate::prompt::structured_reply::{render_structured_reply_guidance, StructuredReply};
use crate::provider_account::{
    resolve_provider_account_exact_blocking, resolve_recorded_provider_account_blocking,
    ProviderAccountRoute,
};
use crate::provider_auth::Provider;
use crate::session_record::CaptureHandle;
use crate::store::ProviderAccountId;

/// PID of the current child agent process. The Ctrl+C handler sends SIGTERM
/// to this process before exiting so the agent doesn't survive as an orphan.
static CHILD_PID: AtomicU32 = AtomicU32::new(0);

/// Tracks the active agent PID and clears it even on early returns.
struct ChildPidGuard;

impl ChildPidGuard {
    fn new(pid: u32) -> Self {
        CHILD_PID.store(pid, Ordering::Release);
        Self
    }
}

impl Drop for ChildPidGuard {
    fn drop(&mut self) {
        CHILD_PID.store(0, Ordering::Release);
    }
}

/// Kill the child agent process if one is running.
pub fn kill_child_if_running() {
    let pid = CHILD_PID.load(Ordering::Acquire);
    if pid != 0 {
        kill_process(pid);
    }
}

/// Cleanups to run when the process is interrupted before it exits. The
/// handler covers SIGINT (Ctrl+C), SIGTERM, and SIGHUP (`tmux kill-session`
/// delivers SIGHUP — see the ctrlc `termination` feature in Cargo.toml) and
/// calls `std::process::exit`, which skips Rust destructors, so anything that
/// must be torn down on interrupt registers a hook here.
#[allow(clippy::type_complexity)]
static INTERRUPT_HOOKS: OnceLock<Mutex<Vec<Box<dyn Fn() + Send>>>> = OnceLock::new();

fn interrupt_hooks() -> &'static Mutex<Vec<Box<dyn Fn() + Send>>> {
    INTERRUPT_HOOKS.get_or_init(|| Mutex::new(Vec::new()))
}

/// Register a cleanup to run on interrupt (SIGINT/SIGTERM/SIGHUP), before
/// the process exits.
pub fn register_interrupt_cleanup(f: impl Fn() + Send + 'static) {
    interrupt_hooks()
        .lock()
        .expect("interrupt hooks lock poisoned")
        .push(Box::new(f));
}

/// Finish interruption while excluding the command's ordinary return path.
/// Killing a child can wake that path before the remaining hooks finish.
pub fn exit_on_interrupt() -> ! {
    let hooks = INTERRUPT_HOOKS.get().and_then(|hooks| hooks.lock().ok());
    if let Some(hooks) = &hooks {
        for hook in hooks.iter() {
            hook();
        }
    }
    kill_child_if_running();
    // Keep the hook lock until process exit: a child failure must not replace
    // the interrupted exit while cleanup is still running on this thread.
    std::process::exit(130);
}

/// Do not let ordinary command return overtake an interruption already cleaning up.
pub fn wait_for_interrupt_cleanup() {
    if let Some(hooks) = INTERRUPT_HOOKS.get() {
        drop(hooks.lock());
    }
}

/// Provider result observed by the driving Process.
#[derive(Debug, Clone, Default)]
pub struct AgentProcessResult {
    pub exit_code: i32,
    /// Harness answer prose, never raw provider protocol.
    pub stdout: String,
    /// Harness error messages; native terminals leave both output fields empty.
    pub stderr: String,
    /// Opaque provider continuation token observed during this Process.
    pub agent_session: Option<AgentSessionId>,
    /// Typed provider failure when the process output identifies one.
    pub failure: Option<AgentFailure>,
}

/// Filesystem write boundary for a provider launch.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AgentWriteScope {
    /// Respect the provider's configured permissions and Loopflow's ordinary floor.
    #[default]
    Configured,
    /// Permit writes only inside the assigned working directory.
    Worktree,
}

/// Additional writable roots required by a confined agent command.
#[derive(Debug, Clone, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct AgentExecutionBoundary {
    pub writable_roots: Vec<PathBuf>,
}

pub(crate) fn checkout_execution_boundary(
    repo: &Path,
    agent: &str,
) -> anyhow::Result<AgentExecutionBoundary> {
    let (harness, _) = parse_agent(agent);
    if !matches!(harness.as_str(), "codex" | "claude" | "pi") {
        return Err(anyhow::anyhow!(format!(
            "Agent execution unavailable: agent {agent:?} uses harness {harness:?}, which does not support a managed execution boundary; select codex, claude, or pi"
        )));
    }
    let common_dir = crate::git::worktrees::git_common_dir(repo).map_err(|error| {
        anyhow::anyhow!(format!(
            "Agent execution unavailable: failed to resolve linked Git metadata from {}: {error}",
            repo.display()
        ))
    })?;
    let control = crate::os_process::execution_context().map_err(|error| {
        anyhow::anyhow!(format!(
            "Agent execution unavailable: Loopflow control-plane authority is unavailable: {error}"
        ))
    })?;
    let mut writable_roots = vec![common_dir, control.lf_home];
    writable_roots.sort();
    writable_roots.dedup();
    Ok(AgentExecutionBoundary { writable_roots })
}

pub(crate) const EXECUTION_IDENTITY_ENV: [&str; 9] = [
    crate::steer::PROVIDER_CONTEXT_ENV,
    crate::steer::STEER_SEEN_ENV,
    crate::context_block::CONTEXT_DELIVERY_ENV,
    crate::process::AGENT_CALLER_ENV,
    crate::journal::LF_TRACE_ID_ENV,
    crate::journal::LF_PROCESS_ID_ENV,
    crate::session_record::CAPTURE_KEY_ENV,
    "LF_RUN_ID",
    "LF_RUN_DIR",
];

#[derive(Clone, Default)]
pub struct AgentConfig {
    /// Chrome integration for Claude terminal and stream sessions.
    pub chrome: bool,
    /// System/context prompt content.
    pub system_prompt: String,
    /// Saved sources for native startup and compaction hooks.
    pub conversation_context: Option<crate::context_block::ContextDelivery>,
    /// Task prompt content sent as the turn input.
    pub task_prompt: String,
    /// Native skill selection, kept separate from gathered reference context.
    pub skill_invocation: Option<crate::skills::invocation::SkillInvocation>,
    /// Native final-answer schema for this turn.
    pub output_schema: Option<serde_json::Value>,
    /// Agent string (for example: "claude:opus" or "codex").
    pub agent: Option<String>,
    /// Max turn budget when supported by the harness.
    pub max_turns: Option<u32>,
    /// Opaque provider continuation token for this launch.
    pub resume_token: Option<AgentSessionId>,
    /// Stable managed account identity selected before durable capture.
    pub provider_account_id: Option<ProviderAccountId>,
    /// Machine whose deterministic account directory resolves a recorded account.
    /// This is launch authority, not a replay input, and is never serialized.
    pub provider_account_authority_home: Option<std::path::PathBuf>,
    /// Working directory.
    pub cwd: Option<std::path::PathBuf>,
    /// Maximum filesystem write scope granted to the provider.
    pub write_scope: AgentWriteScope,
    /// Exact roots and network access needed beyond `cwd`.
    pub execution_boundary: Option<AgentExecutionBoundary>,
    /// Skip permission prompts
    pub skip_permissions: bool,
    /// Graph-runner-injected structured replies (rendered via harness prompt guidance).
    pub structured_replies: Vec<StructuredReply>,
    /// Temp file for relaying shell directives back to the invoking shell.
    /// When set, the agent subprocess gets `LOOPFLOW_DIRECTIVE_FILE` pointing
    /// to this path. The caller reads it after the agent exits and forwards
    /// safe directives (e.g. `cd`) to the real directive file.
    pub directive_relay: Option<std::path::PathBuf>,
    /// Environment scoped to this provider process and its descendants.
    pub env: BTreeMap<String, String>,
    /// Exact attachment selected before provider launch. Never
    /// inherited by provider tools or serialized into replay input.
    pub session_attachment: Option<(String, crate::process::SessionAttachment)>,
}

impl AgentConfig {
    pub(crate) fn apply_agent_override(&mut self) {
        if let Some(agent) = crate::config::agent_override() {
            self.agent = Some(agent.clone());
            self.env
                .insert(crate::config::AGENT_OVERRIDE_ENV.into(), agent);
        }
    }

    /// Return the selected agent or Loopflow's compiled default.
    pub fn agent(&self) -> &str {
        match self.agent.as_deref() {
            Some(agent) => agent,
            None => default_agent(),
        }
    }
}

impl std::fmt::Debug for AgentConfig {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("AgentConfig")
            .field(
                "system_prompt",
                &format_args!("({} bytes)", self.system_prompt.len()),
            )
            .field(
                "task_prompt",
                &format_args!("({} bytes)", self.task_prompt.len()),
            )
            .field("agent", &self.agent)
            .field("max_turns", &self.max_turns)
            .field("resume_token", &self.resume_token)
            .field("provider_account_id", &self.provider_account_id)
            .field(
                "provider_account_authority_home",
                &self.provider_account_authority_home,
            )
            .field("write_scope", &self.write_scope)
            .field("execution_boundary", &self.execution_boundary)
            .field("cwd", &self.cwd)
            .field("skip_permissions", &self.skip_permissions)
            .field("structured_replies", &self.structured_replies)
            .field("directive_relay", &self.directive_relay)
            .field("env_keys", &self.env.keys().collect::<Vec<_>>())
            .finish()
    }
}

/// Select and pin a managed Claude/Codex account before publishing a capture.
pub(crate) fn pin_provider_account_id_blocking(launch: &mut AgentConfig) -> Result<(), CoreError> {
    let (harness, _) = parse_agent(launch.agent());
    let provider = match harness.as_str() {
        "claude" => Provider::Claude,
        "codex" => Provider::Codex,
        _ if launch.provider_account_id.is_some() => {
            return Err(CoreError::ExecutionFailed(
                "managed provider account IDs apply only to Claude and Codex".to_string(),
            ));
        }
        _ => return Ok(()),
    };
    let route = resolve_account_route_blocking(provider, launch).map_err(|error| {
        CoreError::ExecutionFailed(format!("failed to select provider account: {error}"))
    })?;
    if let Some(route) = route {
        launch.provider_account_id = Some(route.account_id().clone());
    }
    Ok(())
}

fn resolve_account_route_blocking(
    provider: Provider,
    launch: &AgentConfig,
) -> Result<Option<ProviderAccountRoute>, crate::provider_account::ProviderAccountError> {
    let route = match (
        launch.provider_account_id.clone(),
        launch.provider_account_authority_home.clone(),
    ) {
        (Some(account_id), Some(home)) => {
            resolve_recorded_provider_account_blocking(provider, account_id, home)
        }
        (account_id, None) => resolve_provider_account_exact_blocking(
            provider,
            launch.resume_token.clone(),
            account_id,
            launch
                .env
                .get(crate::work::wave::context::WAVE_ID_ENV)
                .and_then(|id| crate::id::WaveId::parse(id).ok())
                .or_else(crate::work::wave::context::ambient_wave_id),
        ),
        (None, Some(_)) => Err(crate::provider_account::ProviderAccountError::Runtime(
            "recorded account authority requires an account ID".to_string(),
        )),
    }?;
    if route.is_none() && launch.execution_boundary.is_some() {
        return Err(crate::provider_account::ProviderAccountError::NoEligibleAccount {
            provider,
            accounts: "a connected managed account is required by this execution boundary; ambient credentials cannot satisfy it".to_string(),
        });
    }
    Ok(route)
}

/// Retain provider instructions on disk so native resume can read the same context.
pub(crate) fn write_system_prompt_file(
    config: &AgentConfig,
    name: &str,
) -> Result<Option<PathBuf>, CoreError> {
    let prompt = system_prompt_with_structured_replies(config);
    if prompt.trim().is_empty() {
        return Ok(None);
    }
    let cwd = config
        .cwd
        .clone()
        .map(Ok)
        .unwrap_or_else(std::env::current_dir)?;
    Ok(Some(crate::prompt::write_prompt_log(
        &cwd,
        &prompt,
        &format!("{name}.context"),
        None,
    )?))
}

/// Build the effective system prompt including structured reply guidance.
pub fn system_prompt_with_structured_replies(config: &AgentConfig) -> String {
    let guidance = render_structured_reply_guidance(&config.structured_replies);
    if guidance.is_empty() {
        return config.system_prompt.clone();
    }
    if config.system_prompt.trim().is_empty() {
        return guidance;
    }

    format!("{}\n\n{guidance}", config.system_prompt.trim_end())
}

/// Process/runtime options for launching an agent subprocess.
#[derive(Debug, Clone, Default)]
pub struct ProcessConfig {
    /// Task policy input; the provider loop remains the ordinary command loop.
    pub task_input: Option<crate::ops::task_input::TaskInput>,
    /// Run in auto/batch mode (non-interactive).
    pub auto: bool,
    /// Stream output in real-time.
    pub stream: bool,
    /// Path to context file for system prompt loading (model-agnostic).
    /// For Claude, this uses --append-system-prompt-file.
    pub context_file: Option<std::path::PathBuf>,
    /// Optional timeout for the subprocess.
    pub timeout: Option<Duration>,
    /// Durable local capture for this harness launch.
    pub capture: Option<AgentCapture>,
}

#[derive(Debug, Clone)]
pub struct AgentCapture(CaptureHandle);

impl From<CaptureHandle> for AgentCapture {
    fn from(capture: CaptureHandle) -> Self {
        Self(capture)
    }
}

impl AgentCapture {
    pub(crate) fn prepare_agent_process(
        &self,
        session: &str,
        expected: &crate::process::SessionAttachment,
    ) -> crate::store::StoreResult<crate::process::SessionAttachment> {
        self.0.prepare_agent_process(session, expected)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
enum CodexSandboxMode {
    ReadOnly,
    WorkspaceWrite,
    DangerFullAccess,
}

impl CodexSandboxMode {
    fn as_str(self) -> &'static str {
        match self {
            Self::ReadOnly => "read-only",
            Self::WorkspaceWrite => "workspace-write",
            Self::DangerFullAccess => "danger-full-access",
        }
    }

    fn parse(value: &str) -> Option<Self> {
        match value {
            "read-only" => Some(Self::ReadOnly),
            "workspace-write" => Some(Self::WorkspaceWrite),
            "danger-full-access" => Some(Self::DangerFullAccess),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
enum CodexApprovalPolicy {
    Untrusted,
    OnRequest,
    OnFailure,
    Never,
}

impl CodexApprovalPolicy {
    fn parse(value: &str) -> Option<Self> {
        match value {
            "untrusted" => Some(Self::Untrusted),
            "on-request" => Some(Self::OnRequest),
            "on-failure" => Some(Self::OnFailure),
            "never" => Some(Self::Never),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
struct CodexPermissionConfig {
    sandbox: Option<CodexSandboxMode>,
    approval: Option<CodexApprovalPolicy>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ClaudePermissionMode {
    AcceptEdits,
    Auto,
    BypassPermissions,
    Manual,
    DontAsk,
    Plan,
}

impl ClaudePermissionMode {
    fn parse(value: &str) -> Option<Self> {
        match value {
            "acceptEdits" => Some(Self::AcceptEdits),
            "auto" => Some(Self::Auto),
            "bypassPermissions" => Some(Self::BypassPermissions),
            "manual" => Some(Self::Manual),
            "dontAsk" => Some(Self::DontAsk),
            "plan" => Some(Self::Plan),
            _ => None,
        }
    }
}

fn read_codex_permission_config(cwd: Option<&Path>) -> CodexPermissionConfig {
    let mut config = CodexPermissionConfig::default();
    for path in codex_config_paths(cwd) {
        let Ok(contents) = fs::read_to_string(&path) else {
            continue;
        };
        config = merge_codex_permission_config(config, parse_codex_permission_config(&contents));
    }
    config
}

fn codex_config_paths(cwd: Option<&Path>) -> Vec<PathBuf> {
    let mut paths = Vec::new();

    if let Some(home) = env::var_os("CODEX_HOME")
        .map(PathBuf::from)
        .or_else(|| dirs::home_dir().map(|home| home.join(".codex")))
    {
        paths.push(home.join("config.toml"));
    }

    if let Some(cwd) = cwd {
        let mut ancestors: Vec<PathBuf> = cwd.ancestors().map(Path::to_path_buf).collect();
        ancestors.reverse();
        for ancestor in ancestors {
            paths.push(ancestor.join(".codex").join("config.toml"));
        }
    }

    paths
}

fn parse_codex_permission_config(contents: &str) -> CodexPermissionConfig {
    let mut config = CodexPermissionConfig::default();
    for line in contents.lines() {
        let line = line.trim();
        if line.starts_with('[') {
            break;
        }
        if let Some(value) = parse_toml_string_assignment(line, "sandbox_mode") {
            config.sandbox = CodexSandboxMode::parse(&value);
        } else if let Some(value) = parse_toml_string_assignment(line, "approval_policy") {
            config.approval = CodexApprovalPolicy::parse(&value);
        }
    }
    config
}

fn parse_toml_string_assignment(line: &str, key: &str) -> Option<String> {
    let (left, right) = line.split_once('=')?;
    if left.trim() != key {
        return None;
    }
    let value = right.split('#').next()?.trim();
    value
        .strip_prefix('"')
        .and_then(|value| value.strip_suffix('"'))
        .map(str::to_string)
}

fn merge_codex_permission_config(
    base: CodexPermissionConfig,
    overlay: CodexPermissionConfig,
) -> CodexPermissionConfig {
    CodexPermissionConfig {
        sandbox: overlay.sandbox.or(base.sandbox),
        approval: overlay.approval.or(base.approval),
    }
}

/// App-server overrides for native and headless startup. None preserves vendor policy.
fn codex_permission_policy(
    config: CodexPermissionConfig,
    launch: &AgentConfig,
) -> CodexPermissionConfig {
    if launch.write_scope == AgentWriteScope::Worktree && launch.execution_boundary.is_none() {
        return CodexPermissionConfig {
            sandbox: Some(CodexSandboxMode::WorkspaceWrite),
            approval: Some(CodexApprovalPolicy::Never),
        };
    }
    if launch.skip_permissions || launch.execution_boundary.is_some() {
        return CodexPermissionConfig {
            sandbox: Some(CodexSandboxMode::DangerFullAccess),
            approval: Some(CodexApprovalPolicy::Never),
        };
    }
    CodexPermissionConfig {
        sandbox: (config.sandbox < Some(CodexSandboxMode::WorkspaceWrite))
            .then_some(CodexSandboxMode::WorkspaceWrite),
        approval: (config.approval < Some(CodexApprovalPolicy::Never))
            .then_some(CodexApprovalPolicy::Never),
    }
}

fn read_claude_permission_mode(cwd: Option<&Path>) -> Option<ClaudePermissionMode> {
    let mut mode = None;
    for path in claude_settings_paths(cwd) {
        let Ok(contents) = fs::read_to_string(&path) else {
            continue;
        };
        if let Some(next) = parse_claude_permission_mode(&contents) {
            mode = Some(next);
        }
    }
    mode
}

fn claude_settings_paths(cwd: Option<&Path>) -> Vec<PathBuf> {
    let mut paths = Vec::new();

    if let Some(home) = dirs::home_dir() {
        paths.push(home.join(".claude").join("settings.json"));
    }

    if let Some(cwd) = cwd {
        let mut ancestors: Vec<PathBuf> = cwd.ancestors().map(Path::to_path_buf).collect();
        ancestors.reverse();
        for ancestor in ancestors {
            paths.push(ancestor.join(".claude").join("settings.json"));
            paths.push(ancestor.join(".claude").join("settings.local.json"));
        }
    }

    paths
}

fn parse_claude_permission_mode(contents: &str) -> Option<ClaudePermissionMode> {
    let json: serde_json::Value = serde_json::from_str(contents).ok()?;
    json.get("permissions")
        .and_then(|permissions| permissions.get("defaultMode"))
        .and_then(serde_json::Value::as_str)
        .and_then(ClaudePermissionMode::parse)
}

fn claude_skip_permissions(cwd: Option<&Path>, auto: bool, skip_permissions: bool) -> bool {
    if skip_permissions {
        return true;
    }
    if !auto {
        return false;
    }

    let mode = read_claude_permission_mode(cwd);
    if mode == Some(ClaudePermissionMode::BypassPermissions) {
        return false;
    }
    if mode.is_some() {
        eprintln!(
            "warning: Claude permissions.defaultMode is less permissive than Loopflow's non-interactive default; launching with --dangerously-skip-permissions"
        );
    }
    true
}

/// Common Claude options shared by terminal and stream-input launches.
fn claude_options(
    config: &AgentConfig,
    model: Option<&str>,
    context_file: Option<&Path>,
    stream: bool,
) -> Vec<String> {
    let mut args = vec![if config.chrome {
        "--chrome"
    } else {
        "--no-chrome"
    }
    .into()];
    if let Some(model) = model {
        args.extend(["--model".into(), model.into()]);
    }
    if let Some(file) = context_file {
        args.extend([
            "--append-system-prompt-file".into(),
            file.display().to_string(),
        ]);
    } else {
        let instructions = system_prompt_with_structured_replies(config);
        if !instructions.trim().is_empty() {
            args.extend(["--append-system-prompt".into(), instructions]);
        }
    }
    let add_dirs = provider_writable_roots(config);
    if !add_dirs.is_empty() {
        args.push("--add-dir".into());
        args.extend(add_dirs.iter().map(|dir| dir.display().to_string()));
    }
    if config.execution_boundary.is_some()
        || (config.write_scope == AgentWriteScope::Configured
            && claude_skip_permissions(config.cwd.as_deref(), stream, config.skip_permissions))
    {
        args.push("--dangerously-skip-permissions".into());
    }
    if config.write_scope == AgentWriteScope::Worktree && config.execution_boundary.is_none() {
        args.extend([
            "--permission-mode".into(),
            "acceptEdits".into(),
            "--setting-sources".into(),
            String::new(),
            "--settings".into(),
            serde_json::json!({
                "sandbox": {
                    "enabled": true,
                    "failIfUnavailable": true,
                    "allowUnsandboxedCommands": false
                }
            })
            .to_string(),
        ]);
    }
    if stream {
        if let Some(max_turns) = config.max_turns {
            args.extend(["--max-turns".into(), max_turns.to_string()]);
        }
        if let Some(schema) = &config.output_schema {
            args.extend(["--json-schema".into(), schema.to_string()]);
        }
    }
    args
}

/// Resolve a model string to a Codex model override.
///
/// For `codex:<variant>`, returns `<variant>`. For bare `codex`, returns `None`
/// so Codex can pick its own default. Non-codex model strings pass through.
pub fn resolve_codex_model(model: &str) -> Option<String> {
    let (harness, model_variant) = parse_agent(model);
    if harness == "codex" {
        model_variant
    } else {
        Some(model.to_string())
    }
}

const CODEX_DEFAULT_SERVICE_TIER: &str = "default";
const TRANSIENT_RETRY_DELAYS: [Duration; 4] = [
    Duration::from_secs(2),
    Duration::from_secs(5),
    Duration::from_secs(15),
    Duration::from_secs(30),
];
const RETRY_PROMPT: &str = "Continue the original task after the transient provider failure. Re-read the current workspace state, finish any interrupted work, and return the result.";
const LIMIT_FAILOVER_PROMPT: &str = "Continue this task after the previous provider account reached its subscription limit. Re-read the current workspace state and finish any interrupted work.";
const CREDENTIAL_FAILOVER_PROMPT: &str = "Continue this task after the previous provider account credential was revoked. Re-read the current workspace state and finish any interrupted work.";

#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
#[non_exhaustive]
pub enum AgentFailure {
    /// The selected model or provider has no current serving capacity.
    #[error("provider capacity")]
    Capacity,
    /// A request-level rate limit may clear after backoff.
    #[error("provider rate limit")]
    RateLimit,
    /// The provider reported temporary service unavailability.
    #[error("provider unavailable")]
    Unavailable,
    /// The provider process reported a retryable connection failure.
    #[error("provider transport")]
    Transport,
    /// The provider home's account changed under a running Codex, which
    /// fails one turn and then works again.
    #[error("provider account switched")]
    AccountSwitched,
    /// The selected managed account exhausted a subscription window.
    #[error("account subscription limit")]
    AccountSubscriptionLimit { resets_at: Option<i64> },
    /// The selected managed account's credential was explicitly revoked.
    #[error("account credential invalidated")]
    AccountCredentialInvalidated,
}

/// Build `thread/start` params for Codex app-server sessions.
pub fn build_codex_thread_start_params(
    launch: &AgentConfig,
) -> serde_json::Map<String, serde_json::Value> {
    let mut params = serde_json::Map::new();
    let instructions = system_prompt_with_structured_replies(launch);
    if !instructions.is_empty() {
        params.insert(
            "developerInstructions".into(),
            serde_json::Value::String(instructions),
        );
    }

    params.insert(
        "serviceTier".to_string(),
        serde_json::Value::String(CODEX_DEFAULT_SERVICE_TIER.to_string()),
    );

    if let Some(model) = launch.agent.as_deref().and_then(resolve_codex_model) {
        params.insert("model".to_string(), serde_json::Value::String(model));
    }

    if let Some(cwd) = launch.cwd.as_ref() {
        params.insert(
            "cwd".to_string(),
            serde_json::Value::String(cwd.to_string_lossy().to_string()),
        );
    }

    let policy =
        codex_permission_policy(read_codex_permission_config(launch.cwd.as_deref()), launch);
    if let Some(sandbox) = policy.sandbox {
        params.insert("sandbox".into(), sandbox.as_str().into());
    }
    if policy.approval == Some(CodexApprovalPolicy::Never) {
        params.insert("approvalPolicy".into(), "never".into());
    }

    params
}

/// Extra workspace roots agents need for Git worktree metadata.
pub fn workspace_add_dirs(cwd: &Path) -> Vec<PathBuf> {
    let Ok(main_repo) = crate::git::worktrees::main_repo_root(cwd) else {
        return Vec::new();
    };
    if paths_equal(cwd, &main_repo) {
        Vec::new()
    } else {
        vec![main_repo]
    }
}

fn provider_writable_roots(launch: &AgentConfig) -> Vec<PathBuf> {
    if launch.execution_boundary.is_some() || launch.write_scope != AgentWriteScope::Configured {
        return Vec::new();
    }
    launch
        .cwd
        .as_deref()
        .map(workspace_add_dirs)
        .unwrap_or_default()
}

fn paths_equal(left: &Path, right: &Path) -> bool {
    match (left.canonicalize(), right.canonicalize()) {
        (Ok(left), Ok(right)) => left == right,
        _ => left == right,
    }
}

/// Claude consumes initial stdin and reopens the controlling terminal for keys.
/// A file avoids blocking on pipe capacity before the native process starts.
pub(crate) fn configure_native_input(command: &mut Command, prompt: &str) -> std::io::Result<()> {
    let mut input = tempfile::tempfile()?;
    input.write_all(prompt.as_bytes())?;
    input.rewind()?;
    command.stdin(input);
    Ok(())
}

/// Native terminal launch arguments must fit cmux's per-argument transport limit.
pub(crate) fn validate_terminal_turn(prompt: &str) -> Result<(), CoreError> {
    const CAP: usize = 122_880;
    if prompt.len() >= CAP {
        return Err(CoreError::ExecutionFailed(format!(
            "first turn is {} bytes; terminal argument cap is {CAP} bytes (including the terminating NUL)",
            prompt.len()
        )));
    }
    Ok(())
}

/// Launch an agent subprocess and wait for it to exit.
pub fn run_agent(
    launch: &AgentConfig,
    process: &ProcessConfig,
) -> Result<AgentProcessResult, CoreError> {
    let cwd = match &launch.cwd {
        Some(cwd) => cwd.clone(),
        None => std::env::current_dir()?,
    };
    let command = std::env::args().collect::<Vec<_>>();
    crate::journal::with_runtime(&cwd, &command, || {
        run_admitted_agent(launch, process).map_err(Into::into)
    })
    .map_err(|error| {
        error
            .downcast::<CoreError>()
            .unwrap_or_else(|error| CoreError::ExecutionFailed(error.to_string()))
    })
}

fn run_admitted_agent(
    launch: &AgentConfig,
    process: &ProcessConfig,
) -> Result<AgentProcessResult, CoreError> {
    let mut launch = launch.clone();
    launch.apply_agent_override();
    if process.auto && parse_agent(launch.agent()).0 != "claude" {
        if launch.max_turns.is_some() {
            eprintln!("warning: Loopflow turn limits are supported only by Claude; this provider will run without a turn limit");
        }
        if launch.output_schema.is_some() {
            eprintln!("warning: Loopflow native response-schema enforcement is Claude-specific; using ordinary provider output (Graph output validation still applies)");
        }
    }
    if launch.resume_token.is_none() {
        if let Some(capture) = &process.capture {
            launch.resume_token = capture.0.conversation_resume_token().map_err(|error| {
                CoreError::ExecutionFailed(format!("conversation recovery failed: {error}"))
            })?;
        }
    }
    pin_provider_account_id_blocking(&mut launch)?;
    let (harness, model) = parse_agent(launch.agent());
    let implicit_capture = if process.capture.is_none() {
        Some(_begin_implicit_capture(&launch, process, &harness, model)?)
    } else {
        None
    };
    let mut process = process.clone();
    for name in EXECUTION_IDENTITY_ENV {
        launch.env.remove(name);
    }
    if let Some(capture) = &implicit_capture {
        process.capture = Some(capture.clone());
    }
    if let Some(capture) = &process.capture {
        capture.0.claim_session_attachment().map_err(|error| {
            CoreError::ExecutionFailed(format!("conversation admission failed: {error}"))
        })?;
        launch.session_attachment = capture.0.session_attachment();
        if launch.resume_token.is_none() {
            launch.resume_token = capture.0.conversation_resume_token().map_err(|error| {
                CoreError::ExecutionFailed(format!("conversation recovery failed: {error}"))
            })?;
        }
        launch.env.extend(capture.0.environment());
        capture.0.mark_spawn_requested();
    }
    let directory = match &launch.cwd {
        Some(cwd) => cwd.clone(),
        None => std::env::current_dir()?,
    };
    let _planning_sync = crate::ops::planning_sync::PlanningSync::start_for_directory(&directory)
        .map_err(|error| {
        CoreError::ExecutionFailed(format!("cannot start planning sync: {error}"))
    })?;
    let result = _run_with_transient_retries(
        &launch,
        &process,
        &TRANSIENT_RETRY_DELAYS,
        |attempt, retry| _run_agent_once(attempt, &process, retry),
        thread::sleep,
    );
    if let Err(error) = crate::ops::planning_sync::check_directory(&directory) {
        tracing::debug!(%error, "end-of-turn planning check pending");
    }
    if let Some(capture) = implicit_capture {
        let outcome = match &result {
            Ok(result) if result.exit_code == 0 => "completed",
            Ok(_) | Err(_) => "failed",
        };
        capture
            .0
            .finish(outcome)
            .map_err(|error| CoreError::ExecutionFailed(error.to_string()))?;
    }
    result
}

#[derive(Debug)]
enum AgentAttempt {
    Finished {
        result: AgentProcessResult,
        can_failover: bool,
    },
    AccountUnavailable(CoreError),
}

fn _run_with_transient_retries(
    launch: &AgentConfig,
    process: &ProcessConfig,
    retry_delays: &[Duration],
    mut run: impl FnMut(&AgentConfig, bool) -> Result<AgentAttempt, CoreError>,
    mut wait: impl FnMut(Duration),
) -> Result<AgentProcessResult, CoreError> {
    let mut attempt_config = launch.clone();
    let mut attempt = 1;
    let mut account_failure = None;

    loop {
        let (mut result, can_failover) = match run(&attempt_config, attempt > 1)? {
            AgentAttempt::Finished {
                result,
                can_failover,
            } => (result, can_failover),
            AgentAttempt::AccountUnavailable(error) => {
                let Some(result) = account_failure else {
                    return Err(error);
                };
                tracing::warn!(%error, "alternate provider account unavailable");
                return Ok(result);
            }
        };
        let failure = if process.auto {
            result.failure.or_else(|| _classify_agent_failure(&result))
        } else {
            None
        };
        result.failure = failure;
        // Pi and OpenCode cannot be resumed without adding a new
        // model-visible input. Preserve failure; only explicit input starts more work.
        if matches!(parse_agent(launch.agent()).0.as_str(), "opencode" | "pi") {
            return Ok(result);
        }
        let Some(failure) = failure else {
            return Ok(result);
        };
        if matches!(
            failure,
            AgentFailure::AccountSubscriptionLimit { .. }
                | AgentFailure::AccountCredentialInvalidated
        ) && !can_failover
        {
            return Ok(result);
        }
        let Some(transient_delay) = retry_delays.get(attempt - 1).copied() else {
            return Ok(result);
        };
        let limit_resets_at = match &failure {
            AgentFailure::AccountSubscriptionLimit { resets_at } => *resets_at,
            _ => None,
        };

        attempt_config = launch.clone();
        let (delay, failover) = match &failure {
            AgentFailure::AccountSubscriptionLimit { .. } => {
                account_failure = Some(result.clone());
                attempt_config.provider_account_id = None;
                attempt_config.resume_token = None;
                attempt_config.task_prompt = format!(
                    "{LIMIT_FAILOVER_PROMPT}\n\nOriginal task:\n\n{}",
                    launch.task_prompt
                );
                (Duration::ZERO, true)
            }
            AgentFailure::AccountCredentialInvalidated => {
                account_failure = Some(result.clone());
                attempt_config.provider_account_id = None;
                attempt_config.resume_token = None;
                attempt_config.task_prompt = format!(
                    "{CREDENTIAL_FAILOVER_PROMPT}\n\nOriginal task:\n\n{}",
                    launch.task_prompt
                );
                (Duration::ZERO, true)
            }
            _ => {
                account_failure = None;
                if let Some(resume_token) =
                    result.agent_session.clone().or(launch.resume_token.clone())
                {
                    attempt_config.resume_token = Some(resume_token);
                    attempt_config.task_prompt = RETRY_PROMPT.to_string();
                    attempt_config.skill_invocation = None;
                }
                (transient_delay, false)
            }
        };

        tracing::warn!(
            failure = %failure,
            attempt,
            next_attempt = attempt + 1,
            max_attempts = retry_delays.len() + 1,
            delay_ms = delay.as_millis(),
            resumed = attempt_config.resume_token.is_some(),
            account_failover = failover,
            limit_resets_at,
            "recoverable agent failure; retrying"
        );
        wait(delay);
        attempt += 1;
    }
}

fn _classify_agent_failure(result: &AgentProcessResult) -> Option<AgentFailure> {
    if result.exit_code == 0 {
        return None;
    }
    if _find_provider_error(result, credential_invalidated_failure).is_some() {
        return Some(AgentFailure::AccountCredentialInvalidated);
    }
    if _find_provider_error(result, _classify_subscription_limit).is_some() {
        return Some(AgentFailure::AccountSubscriptionLimit { resets_at: None });
    }
    _find_provider_error(result, classify_retryable_agent_failure)
}

pub(crate) fn credential_invalidated_failure(text: &str) -> Option<()> {
    let text = text.to_ascii_lowercase();
    (text.contains("token_invalidated")
        || text.contains("refresh_token_invalidated")
        || text.contains("refresh token was revoked")
        || text.contains("access token could not be refreshed")
        || text.contains("your session has ended. please log in again"))
    .then_some(())
}

fn _find_provider_error<T>(
    result: &AgentProcessResult,
    classify: fn(&str) -> Option<T>,
) -> Option<T> {
    // Harnesses separate provider errors from answer prose. Never interpret
    // JSON printed by the model as provider control messages.
    result.stderr.lines().find_map(classify)
}

pub(crate) fn classify_retryable_agent_failure(text: &str) -> Option<AgentFailure> {
    let text = text.to_ascii_lowercase();
    if text.contains("application network permission was revoked")
        || text.contains("application network policy is unavailable")
    {
        Some(AgentFailure::AccountSwitched)
    } else if text.contains("at capacity") || text.contains("capacity temporarily unavailable") {
        Some(AgentFailure::Capacity)
    } else if text.contains("rate limit")
        || text.contains("rate_limit")
        || text.contains("too many requests")
        || text.contains("status 429")
        || text.contains("status code 429")
    {
        Some(AgentFailure::RateLimit)
    } else if text.contains("temporarily unavailable")
        || text.contains("service unavailable")
        || text.contains("server is busy")
        || text.contains("server overloaded")
        || text.contains("try again later")
        || text.contains("internal server error")
        || text.contains("status 502")
        || text.contains("status 503")
        || text.contains("status 504")
    {
        Some(AgentFailure::Unavailable)
    } else if text.contains("connection reset")
        || text.contains("connection closed")
        || text.contains("connection refused")
        || text.contains("network error")
        || text.contains("request timed out")
        || text.contains("request timeout")
    {
        Some(AgentFailure::Transport)
    } else {
        None
    }
}

fn _classify_subscription_limit(text: &str) -> Option<()> {
    let text = text.to_ascii_lowercase();
    (text.contains("you've hit your usage limit")
        || text.contains("you have hit your usage limit")
        || text.contains("usage limit reached")
        || text.contains("usage limit has been reached")
        || text.contains("subscription limit")
        || text.contains("subscription quota")
        || text.contains("quota exceeded")
        || text.contains("insufficient_quota")
        || text.contains("rate_limit_reached"))
    .then_some(())
}

fn _begin_implicit_capture(
    launch: &AgentConfig,
    process: &ProcessConfig,
    harness: &str,
    model: Option<String>,
) -> Result<AgentCapture, CoreError> {
    let cwd = launch
        .cwd
        .clone()
        .or_else(|| std::env::current_dir().ok())
        .ok_or_else(|| {
            CoreError::ExecutionFailed("agent launch has no working directory".into())
        })?;
    let spec = crate::session_record::SessionCaptureSpec {
        harness: harness.to_string(),
        model,
        surface: if process.auto { "headless" } else { "tui" }.to_string(),
        cwd: cwd.clone(),
        repo: Some(cwd.clone()),
        worktree: Some(cwd),
        skill: None,
        subjects: Vec::new(),
        graph: crate::session_record::SessionGraphMembership::Independent,
        work: None,
    };
    let capture = if process.auto {
        CaptureHandle::begin_with_request(
            spec,
            crate::session_record::AgentProcessRequest::from_prepared(launch),
        )
    } else {
        let context = crate::trace::PreparedTurnContext::from_prompts(
            &system_prompt_with_structured_replies(launch),
            &launch.task_prompt,
        );
        CaptureHandle::begin_with_context(
            spec,
            &context,
            Some(crate::session_record::AgentProcessRequest::from_prepared(
                launch,
            )),
        )
    };
    capture.map(Into::into).map_err(|error| {
        CoreError::ExecutionFailed(format!(
            "failed to publish Session capture manifest before agent launch: {error}"
        ))
    })
}

fn _run_harness_once(
    launch: &AgentConfig,
    process: &ProcessConfig,
    model: Option<String>,
    retry: bool,
) -> Result<AgentAttempt, CoreError> {
    if tokio::runtime::Handle::try_current().is_ok() {
        let launch = launch.clone();
        let process = process.clone();
        return std::thread::Builder::new()
            .name("lf-native-harness".to_string())
            .spawn(move || _run_harness_once(&launch, &process, model, retry))
            .map_err(|error| CoreError::ExecutionFailed(error.to_string()))?
            .join()
            .map_err(|_| {
                CoreError::ExecutionFailed("Native harness thread panicked".to_string())
            })?;
    }

    use crate::chat::types::{ConversationEvent, Lifecycle};
    use crate::harness::ApprovalPolicy;

    let capture = process.capture.as_ref().map(|capture| &capture.0);
    let (provider, _) = parse_agent(launch.agent());
    let account_route = match match provider.as_str() {
        "codex" => resolve_account_route_blocking(Provider::Codex, launch),
        "claude" => resolve_account_route_blocking(Provider::Claude, launch),
        _ => Ok(None),
    } {
        Ok(route) => route,
        Err(error) => {
            return Ok(AgentAttempt::AccountUnavailable(
                CoreError::ExecutionFailed(format!("failed to select provider account: {error}")),
            ));
        }
    };
    if retry {
        if let Some(capture) = capture {
            capture
                .retry_agent_process(
                    provider.clone(),
                    model,
                    account_route
                        .as_ref()
                        .map(|route| route.account_id().clone()),
                    launch.resume_token.as_ref(),
                )
                .map_err(|error| CoreError::ExecutionFailed(error.to_string()))?;
        }
    }

    let mut config = launch.clone();
    if let Some(capture) = capture {
        // A new harness gets the invocation owner's current snapshot. Existing
        // dispatch/history operations never refresh theirs from the capture.
        config.session_attachment = capture.session_attachment();
        config.env.extend(capture.environment());
    }
    let prompt = std::mem::take(&mut config.task_prompt);
    let launch_worktree = config.cwd.clone().or_else(|| std::env::current_dir().ok());
    if let Some(cwd) = launch_worktree.as_deref() {
        crate::ops::git_operation::prepare_agent_process(cwd)
            .map_err(|error| CoreError::ExecutionFailed(error.to_string()))?;
    }
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .map_err(|error| CoreError::ExecutionFailed(error.to_string()))?;
    let result = runtime.block_on(async {
        let (event_tx, mut event_rx) = tokio::sync::mpsc::unbounded_channel();
        let mut harness = crate::harness::default_create_harness(
            &provider,
            ApprovalPolicy::AutoApprove,
            event_tx,
        )
        .map_err(|error| CoreError::ExecutionFailed(error.to_string()))?;
        harness.set_provider_account_id(
            account_route
                .as_ref()
                .map(|route| route.account_id().clone()),
        );
        harness.set_agent_session(launch.resume_token.clone());
        harness.set_capture(process.capture.clone());
        harness
            .start(&config)
            .await
            .map_err(|error| CoreError::ExecutionFailed(error.to_string()))?;
        let agent_session = harness.agent_session();
        if let Some(capture) = capture {
            capture.observe_provider(agent_session.clone(), harness.provider_account_id());
        }
        let can_failover = account_route.is_some()
            && launch.provider_account_authority_home.is_none();

        let drive = async {
            harness
                .send_input(&prompt)
                .await
                .map_err(|error| CoreError::ExecutionFailed(error.to_string()))?;
            let mut stdout = String::new();
            let mut stderr = String::new();
            let mut exit_code = None;
            let mut input_tick = tokio::time::interval(Duration::from_millis(250));
            input_tick.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);
            let mut activity_tick = tokio::time::interval(crate::session_record::activity::SAMPLE_INTERVAL);
            activity_tick.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);
            while exit_code.is_none() {
                tokio::select! {
                    _ = input_tick.tick(), if process.task_input.is_some() => {
                        process.task_input.as_ref().expect("Task input is enabled")
                            .poll(harness.as_mut()).await
                            .map_err(|error| CoreError::ExecutionFailed(error.to_string()))?;
                    }
                    _ = activity_tick.tick(), if capture.is_some() => {
                        if let Some(capture) = capture { capture.observe_activity(harness.pid()).await; }
                    }
                    event = event_rx.recv() => {
                        let Some(event) = event else {
                            stderr.push_str(&format!("{provider} event stream closed\n"));
                            exit_code = Some(1);
                            continue;
                        };
                        if let Some(capture) = capture {
                            capture.observe_provider(harness.agent_session(), harness.provider_account_id());
                            capture.record_conversation(event.clone());
                        }
                        match event {
                            ConversationEvent::TextDelta { content, .. } => {
                                stdout.push_str(&content);
                                if process.stream {
                                    print!("{content}");
                                    let _ = std::io::stdout().flush();
                                }
                            }
                            ConversationEvent::Error { code, message, .. } => {
                                stderr.push_str(&format!("{code}: {message}\n"));
                                if crate::harness::is_terminal_harness_error(&code)
                                    || matches!(code.as_str(), "codex_dispatch_rejected" | "provider_rate_limited")
                                {
                                    exit_code = Some(1);
                                }
                            }
                            ConversationEvent::TurnCompleted { status, .. } => {
                                exit_code = Some(match status {
                                    Lifecycle::Completed => 0,
                                    Lifecycle::Interrupted => 130,
                                    _ => 1,
                                });
                            }
                            _ => {}
                        }
                    }
                }
            }
            while let Ok(event) = event_rx.try_recv() {
                if let Some(capture) = capture { capture.record_conversation(event.clone()); }
                if let ConversationEvent::Error { code, message, .. } = event {
                    stderr.push_str(&format!("{code}: {message}\n"));
                }
            }
            if process.stream && !stdout.ends_with('\n') {
                println!();
            }
            Ok(AgentProcessResult {
                exit_code: exit_code.expect("event loop stops with an exit code"),
                stdout,
                stderr,
                agent_session: harness.agent_session(),
                failure: None,
            })
        };
        let result = match process.timeout {
            Some(timeout) => match tokio::time::timeout(timeout, drive).await {
                Ok(result) => result,
                Err(_) => Err(CoreError::ExecutionFailed(format!(
                        "agent timed out after {}",
                        format_timeout(Some(timeout))
                    ))),
            },
            None => drive.await,
        };
        harness.stop().await
            .map_err(|error| CoreError::ExecutionFailed(error.to_string()))?;
        result.map(|result| AgentAttempt::Finished {
            result,
            can_failover,
        })
    });

    if let (Some(route), Ok(AgentAttempt::Finished { result, .. })) = (&account_route, &result) {
        if _find_provider_error(result, credential_invalidated_failure).is_some() {
            route
                .record_credential_invalidated_blocking("token_invalidated")
                .map_err(|error| CoreError::ExecutionFailed(error.to_string()))?;
        } else {
            route
                .pin_session_blocking(result.agent_session.clone())
                .map_err(|error| CoreError::ExecutionFailed(error.to_string()))?;
        }
    }

    result
}

fn _run_agent_once(
    launch: &AgentConfig,
    process: &ProcessConfig,
    retry: bool,
) -> Result<AgentAttempt, CoreError> {
    let start = Instant::now();
    let (harness, model) = parse_agent(launch.agent());
    if process.auto {
        return _run_harness_once(launch, process, model, retry);
    }
    if matches!(harness.as_str(), "codex" | "opencode") {
        let result = crate::lf::commands::util::run_native_agent(launch)
            .map_err(|error| CoreError::ExecutionFailed(error.to_string()))?;
        return Ok(AgentAttempt::Finished {
            result,
            can_failover: false,
        });
    }
    let mut progress = crate::startup::Startup::new(format!("Preparing {harness}"));
    let invocation = match launch.resume_token.as_ref() {
        Some(session_id) => ProviderInvocation::Resume { session_id },
        None => ProviderInvocation::Terminal { session_id: None },
    };
    let command = build_session_command(launch, invocation, process.context_file.as_deref())
        .map_err(|error| CoreError::ExecutionFailed(error.to_string()))?;
    let hooks =
        crate::harness::context::native_args(&harness, launch.conversation_context.as_ref())
            .map_err(|error| CoreError::ExecutionFailed(error.to_string()))?;
    let program = &command.program;
    tracing::debug!(
        elapsed_ms = start.elapsed().as_millis(),
        "run_agent prepared command"
    );
    tracing::debug!(program, "spawning agent command");

    let mut cmd = Command::new(program);
    for name in EXECUTION_IDENTITY_ENV {
        cmd.env_remove(name);
    }
    cmd.envs(
        session_environment(launch, process.context_file.as_deref())
            .map_err(|error| CoreError::ExecutionFailed(error.to_string()))?,
    );
    if let Some(context) = launch
        .conversation_context
        .as_ref()
        .filter(|_| matches!(harness.as_str(), "claude" | "codex" | "pi"))
    {
        context
            .configure_hooks(&mut cmd)
            .map_err(|error| CoreError::ExecutionFailed(error.to_string()))?;
    }
    progress.finish();
    let title = crate::terminal_title::TerminalTitle::prepare(&launch.env, &harness, &mut cmd);
    if let Some(input) = &command.input {
        configure_native_input(&mut cmd, input)
            .map_err(|error| CoreError::ExecutionFailed(error.to_string()))?;
    }

    cmd.current_dir(&command.cwd);
    crate::ops::git_operation::prepare_agent_process(&command.cwd)
        .map_err(|error| CoreError::ExecutionFailed(error.to_string()))?;
    cmd.env_remove(crate::os_process::DISCORD_TOKEN_ENV);

    // Shell integration sets LOOPFLOW_DIRECTIVE_FILE so top-level `lf` commands
    // can request parent-shell actions (for example auto-cd after `lf wt switch`).
    // Agent sessions run arbitrary nested commands; those must not mutate the
    // invoking shell state via the top-level directive file.
    cmd.env_remove("LOOPFLOW_DIRECTIVE_FILE");

    // If the caller set up a directive relay, give the agent a scoped directive
    // file. The caller will filter and forward safe directives after the agent exits.
    if let Some(ref relay_path) = launch.directive_relay {
        cmd.env("LOOPFLOW_DIRECTIVE_FILE", relay_path);
    }

    // Ambient API keys are filtered by executable. Explicitly stored provider
    // credentials are then restored only for the executable that owns them.
    crate::provider_auth::apply_provider_env_to_command(program, &mut cmd);
    crate::harness::configure_vendor_std_env(&mut cmd)
        .map_err(|error| CoreError::ExecutionFailed(error.to_string()))?;

    // Harness-specific environment setup.
    let managed_provider = (harness == "claude").then_some(Provider::Claude);
    let account_route = match managed_provider
        .map(|provider| resolve_account_route_blocking(provider, launch))
        .transpose()
    {
        Ok(route) => route.flatten(),
        Err(error) => {
            return Ok(AgentAttempt::AccountUnavailable(
                CoreError::ExecutionFailed(format!("failed to select provider account: {error}")),
            ));
        }
    };
    let mut activation = None;
    if let Some(route) = &account_route {
        tracing::info!(
            provider = %harness,
            account_id = %route.account_id(),
            "selected provider account"
        );
        cmd.args(route.provider_args());
        activation = match route.launch_as_blocking(&mut cmd) {
            Ok(activation) => activation,
            Err(error) => {
                return Ok(AgentAttempt::AccountUnavailable(
                    CoreError::ExecutionFailed(format!(
                        "failed to activate provider account: {error}"
                    )),
                ));
            }
        };
    }
    if harness == "claude" {
        crate::harness::claude_trust::accept_created_worktree(&cmd);
    }
    cmd.args(hooks).args(&command.args);
    let capture = process.capture.as_ref().map(|capture| &capture.0);
    if retry {
        if let Some(capture) = capture {
            capture
                .retry_agent_process(
                    harness.clone(),
                    model.clone(),
                    account_route
                        .as_ref()
                        .map(|route| route.account_id().clone()),
                    launch.resume_token.as_ref(),
                )
                .map_err(|error| CoreError::ExecutionFailed(error.to_string()))?;
        }
    }

    if let Some(capture) = capture {
        capture.observe_provider(
            None,
            account_route
                .as_ref()
                .map(|route| route.account_id().clone()),
        );
    }

    let admitted_capture = capture.ok_or_else(|| {
        CoreError::ExecutionFailed("AgentProcess launch has no admitted capture".into())
    })?;
    let result = run_interactive(cmd, process.timeout, admitted_capture, activation, title);
    result.map(|result| AgentAttempt::Finished {
        result,
        can_failover: false,
    })
}

/// `activation` is the native credential lock a shared launch took; it is
/// released once the provider process exists.
fn spawn_agent_child(
    cmd: Command,
    capture: &CaptureHandle,
    activation: Option<std::fs::File>,
) -> Result<(Child, crate::process::SessionAttachment), CoreError> {
    let child = capture
        .spawn_native_agent(cmd)
        .map_err(|error| CoreError::ExecutionFailed(error.to_string()))?;
    drop(activation);
    Ok(child)
}

fn run_interactive(
    cmd: Command,
    timeout: Option<Duration>,
    capture: &CaptureHandle,
    activation: Option<std::fs::File>,
    mut title: Option<crate::terminal_title::TerminalTitle>,
) -> Result<AgentProcessResult, CoreError> {
    let start = Instant::now();
    let (mut child, attachment) = spawn_agent_child(cmd, capture, activation)?;
    let _pid_guard = ChildPidGuard::new(child.id());
    tracing::debug!(
        elapsed_ms = start.elapsed().as_millis(),
        "agent spawned (interactive)"
    );
    let (status, timed_out) = wait_for_exit(&mut child, timeout, || {
        if let Some(title) = title.as_mut() {
            title.refresh();
        }
    })?;
    capture
        .record_native_agent_exit(&attachment)
        .map_err(|error| CoreError::ExecutionFailed(error.to_string()))?;
    tracing::debug!(
        elapsed_ms = start.elapsed().as_millis(),
        "agent interactive completed"
    );
    if timed_out {
        return Err(CoreError::ExecutionFailed(format!(
            "agent timed out after {}",
            format_timeout(timeout)
        )));
    }
    Ok(AgentProcessResult {
        exit_code: status.code().unwrap_or(1),
        stdout: String::new(),
        stderr: String::new(),
        agent_session: None,
        failure: None,
    })
}

fn format_timeout(timeout: Option<Duration>) -> String {
    timeout
        .map(|value| format!("{}ms", value.as_millis()))
        .unwrap_or_else(|| "unknown duration".to_string())
}

/// What to tell someone whose agent CLI is missing.
pub fn missing_agent_message(cli: &str) -> String {
    let hint = match cli {
        "claude" => {
            "Install it with `npm install -g @anthropic-ai/claude-code`, then sign in with `lf account connect claude`."
        }
        "codex" => {
            "Install it with `npm install -g @openai/codex`, then sign in with `lf account connect codex`."
        }
        "pi" => "Install Pi 1.3 or later with `npm install -g --ignore-scripts @earendil-works/pi-coding-agent`. Configure its model and credentials in Pi.",
        "opencode" => "Install OpenCode 2 or later with `npm install -g @opencode/cli`; the `opencode-ai` package stops at 1.x.",
        _ => "Install it, or choose another agent with `-a claude` or `-a codex`.",
    };
    format!("'{cli}' is not installed. {hint}")
}

/// Check if a CLI is available.
pub fn check_cli_available(cli: &str) -> bool {
    crate::os_process::which_on_path(Path::new(cli)).is_some()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    #[cfg(unix)]
    fn cli_discovery_needs_an_executable_not_a_version_command() {
        use std::os::unix::fs::{symlink, PermissionsExt};

        let root = tempfile::tempdir().unwrap();
        let provider = root.path().join("provider");
        fs::write(&provider, "#!/bin/sh\nexit 1\n").unwrap();
        fs::set_permissions(&provider, fs::Permissions::from_mode(0o644)).unwrap();
        assert!(!check_cli_available(provider.to_str().unwrap()));
        fs::set_permissions(&provider, fs::Permissions::from_mode(0o755)).unwrap();
        assert!(check_cli_available(provider.to_str().unwrap()));
        let linked = root.path().join("linked-provider");
        symlink(&provider, &linked).unwrap();
        assert!(check_cli_available(linked.to_str().unwrap()));
        fs::remove_file(provider).unwrap();
        assert!(!check_cli_available(linked.to_str().unwrap()));
        assert!(!check_cli_available(root.path().to_str().unwrap()));
    }

    fn default_launch() -> AgentConfig {
        AgentConfig {
            task_prompt: "task".to_string(),
            ..Default::default()
        }
    }

    fn git_worktree_fixture() -> (tempfile::TempDir, PathBuf, PathBuf) {
        let tmp = tempfile::tempdir().expect("tempdir");
        let main = tmp.path().join("repo");
        let worktree = tmp.path().join("repo.feature");
        std::fs::create_dir(&main).expect("create repo dir");
        std::fs::write(main.join("README.md"), "hello\n").expect("write file");

        git(&main, &["init", "-b", "main"]);
        git(&main, &["config", "user.email", "test@example.com"]);
        git(&main, &["config", "user.name", "Test User"]);
        git(&main, &["add", "."]);
        git(&main, &["commit", "-m", "init"]);
        git(
            &main,
            &[
                "worktree",
                "add",
                "-b",
                "feature",
                worktree.to_str().expect("utf8 worktree"),
            ],
        );

        (tmp, main, worktree)
    }

    fn git(repo: &Path, args: &[&str]) {
        let output = std::process::Command::new("git")
            .arg("-C")
            .arg(repo)
            .args(args)
            .output()
            .expect("run git");
        assert!(
            output.status.success(),
            "git -C {} {} failed:\n{}",
            repo.display(),
            args.join(" "),
            String::from_utf8_lossy(&output.stderr)
        );
    }

    fn terminal_args(config: &AgentConfig) -> Vec<String> {
        build_session_command(
            config,
            ProviderInvocation::Terminal { session_id: None },
            None,
        )
        .unwrap()
        .args
    }

    fn auto_process() -> ProcessConfig {
        ProcessConfig {
            auto: true,
            ..Default::default()
        }
    }

    fn assert_arg_pair(args: &[String], flag: &str, value: &str) {
        let idx = args
            .iter()
            .position(|arg| arg == flag)
            .unwrap_or_else(|| panic!("{flag} flag"));
        assert_eq!(args[idx + 1], value);
    }

    #[test]
    fn parse_codex_permission_config_reads_top_level_policy() {
        let config = parse_codex_permission_config(
            r#"
model = "gpt-5.5"
sandbox_mode = "danger-full-access"
approval_policy = "never"

[projects."/tmp/repo"]
trust_level = "trusted"
"#,
        );

        assert_eq!(config.sandbox, Some(CodexSandboxMode::DangerFullAccess));
        assert_eq!(config.approval, Some(CodexApprovalPolicy::Never));
    }

    #[test]
    fn codex_policy_preserves_vendor_grants_and_raises_restrictive_defaults() {
        for sandbox in [
            None,
            Some(CodexSandboxMode::ReadOnly),
            Some(CodexSandboxMode::DangerFullAccess),
        ] {
            let policy = codex_permission_policy(
                CodexPermissionConfig {
                    sandbox,
                    approval: None,
                },
                &AgentConfig::default(),
            );
            assert_eq!(
                policy.sandbox,
                if sandbox == Some(CodexSandboxMode::DangerFullAccess) {
                    None
                } else {
                    Some(CodexSandboxMode::WorkspaceWrite)
                }
            );
            assert_eq!(policy.approval, Some(CodexApprovalPolicy::Never));
        }
        let policy = codex_permission_policy(
            CodexPermissionConfig {
                sandbox: Some(CodexSandboxMode::DangerFullAccess),
                approval: Some(CodexApprovalPolicy::Never),
            },
            &AgentConfig::default(),
        );
        assert_eq!(policy, CodexPermissionConfig::default());
    }

    #[test]
    fn parse_claude_permission_mode_reads_default_mode() {
        assert_eq!(
            parse_claude_permission_mode(r#"{"permissions":{"defaultMode":"bypassPermissions"}}"#),
            Some(ClaudePermissionMode::BypassPermissions)
        );
    }

    #[test]
    fn terminal_turn_cap_counts_utf8_bytes_without_changing_the_request() {
        let at_limit = "😀".repeat(30_720);
        let error = validate_terminal_turn(&at_limit).unwrap_err().to_string();
        assert!(error.contains("122880 bytes"));
        assert!(validate_terminal_turn(&"x".repeat(122_879)).is_ok());
        assert!(validate_terminal_turn(&"x".repeat(122_880)).is_err());
        assert!(validate_terminal_turn("").is_ok());
    }

    #[test]
    fn managed_task_scope_carries_exact_delivery_capabilities() {
        let (_tmp, main, worktree) = git_worktree_fixture();
        let git_control = main.join(".git");
        let control_store = worktree.parent().unwrap().join("control-store");
        let launch = AgentConfig {
            agent: Some("codex".to_string()),
            cwd: Some(worktree),
            write_scope: AgentWriteScope::Worktree,
            execution_boundary: Some(AgentExecutionBoundary {
                writable_roots: vec![git_control.clone(), control_store.clone()],
            }),
            skip_permissions: true,
            ..default_launch()
        };
        let thread = build_codex_thread_start_params(&launch);
        assert_eq!(thread["sandbox"], "danger-full-access");
        assert_eq!(thread["approvalPolicy"], "never");
        assert!(thread.get("config").is_none());

        let claude = terminal_args(&AgentConfig {
            agent: Some("claude".into()),
            ..launch
        });
        assert!(claude.contains(&"--dangerously-skip-permissions".to_string()));
        assert!(!claude.contains(&"--add-dir".to_string()));
    }

    #[test]
    fn terminal_commands_keep_native_context_and_main_repo_scope() {
        let (_tmp, main, _) = git_worktree_fixture();
        for provider in ["claude", "codex", "opencode"] {
            let launch = AgentConfig {
                agent: Some(provider.into()),
                cwd: Some(main.clone()),
                system_prompt: "Fixed additions.".into(),
                ..default_launch()
            };
            let args = terminal_args(&launch);
            assert!(!args.contains(&"--add-dir".into()));
            match provider {
                "claude" => assert_arg_pair(&args, "--append-system-prompt", "Fixed additions."),
                "codex" => {
                    let params = build_codex_thread_start_params(&launch);
                    assert_eq!(params["serviceTier"], "default");
                    assert_eq!(params["developerInstructions"], "Fixed additions.");
                }
                _ => {}
            }
        }
    }

    #[test]
    fn resolve_codex_model_bare() {
        assert_eq!(resolve_codex_model("codex"), None);
    }

    #[test]
    fn resolve_codex_model_with_variant() {
        assert_eq!(resolve_codex_model("codex:o3"), Some("o3".to_string()));
    }

    #[test]
    fn resolve_codex_model_passthrough_non_codex() {
        assert_eq!(
            resolve_codex_model("gpt-5.1-codex-high"),
            Some("gpt-5.1-codex-high".to_string())
        );
    }

    #[test]
    fn build_codex_thread_start_params_with_variant_and_cwd() {
        let launch = AgentConfig {
            agent: Some("codex:o3".to_string()),
            cwd: Some("/tmp/repo".into()),
            ..default_launch()
        };

        let params = build_codex_thread_start_params(&launch);
        assert_eq!(
            params.get("model"),
            Some(&serde_json::Value::String("o3".to_string()))
        );
        assert_eq!(
            params.get("cwd"),
            Some(&serde_json::Value::String("/tmp/repo".to_string()))
        );
        assert_eq!(
            params.get("serviceTier"),
            Some(&serde_json::Value::String("default".to_string()))
        );
    }

    #[test]
    fn build_codex_thread_start_params_omits_model_for_bare_codex() {
        let launch = AgentConfig {
            agent: Some("codex".to_string()),
            cwd: Some("/tmp/repo".into()),
            ..default_launch()
        };

        let params = build_codex_thread_start_params(&launch);
        assert!(!params.contains_key("model"));
        assert_eq!(
            params.get("cwd"),
            Some(&serde_json::Value::String("/tmp/repo".to_string()))
        );
        assert_eq!(
            params.get("serviceTier"),
            Some(&serde_json::Value::String("default".to_string()))
        );
    }

    #[test]
    fn claude_stream_context_keeps_large_instructions_and_reply_guidance_off_argv() {
        let home = tempfile::tempdir().unwrap();
        let config = AgentConfig {
            system_prompt: "Keep the complete context.\n".repeat(6_000),
            cwd: Some(home.path().to_path_buf()),
            agent: Some("claude:sonnet".into()),
            skip_permissions: true,
            max_turns: Some(5),
            structured_replies: vec![StructuredReply {
                name: "suggest_actions".into(),
                description: "Suggest actions".into(),
                guidance: "Emit <lf:suggest_actions> JSON.".into(),
            }],
            ..default_launch()
        };
        let path = write_system_prompt_file(&config, "session")
            .unwrap()
            .unwrap();
        let args = build_session_command(
            &config,
            ProviderInvocation::ClaudeStream {
                session_id: Some(&"sess_abc".into()),
            },
            Some(&path),
        )
        .unwrap()
        .args;
        assert!(args.iter().all(|arg| arg.len() < 122_880));
        for (flag, value) in [
            ("--append-system-prompt-file", path.to_str().unwrap()),
            ("--input-format", "stream-json"),
            ("--output-format", "stream-json"),
            ("--model", "sonnet"),
            ("--max-turns", "5"),
            ("--resume", "sess_abc"),
        ] {
            assert!(args.windows(2).any(|pair| pair == [flag, value]));
        }
        let context = std::fs::read_to_string(path).unwrap();
        assert!(context.contains(&config.system_prompt));
        assert!(context.contains("<lf:structured_replies>"));
        assert!(context.contains("<lf:suggest_actions>"));
    }

    #[test]
    fn bare_harness_commands_do_not_select_a_model() {
        for harness in ["claude", "opencode"] {
            let launch = AgentConfig {
                agent: Some(harness.to_string()),
                ..default_launch()
            };
            let cmd = terminal_args(&launch);
            assert!(
                !cmd.iter()
                    .any(|arg| { arg == "--model" || arg == "-m" || arg.starts_with("model=") }),
                "bare {harness} selected a model: {cmd:?}"
            );
        }
    }

    #[test]
    fn unknown_provider_is_rejected_in_both_launch_modes() {
        let cwd = tempfile::tempdir().unwrap();
        let launch = AgentConfig {
            agent: Some("not-a-provider:model".to_string()),
            cwd: Some(cwd.path().to_path_buf()),
            ..default_launch()
        };
        for auto in [false, true] {
            let process = ProcessConfig {
                auto,
                ..Default::default()
            };
            let error = _run_agent_once(&launch, &process, false).unwrap_err();
            assert!(error.to_string().contains("not-a-provider"), "{error}");
        }
    }

    fn managed_attempt(result: AgentProcessResult) -> AgentAttempt {
        AgentAttempt::Finished {
            result,
            can_failover: true,
        }
    }

    fn failed_result(message: &str) -> AgentProcessResult {
        AgentProcessResult {
            exit_code: 1,
            stderr: message.to_string(),
            ..Default::default()
        }
    }

    #[test]
    fn pi_failure_keeps_exact_history_without_synthetic_retry_input() {
        let launch = AgentConfig {
            agent: Some("pi:fixture/model".into()),
            ..Default::default()
        };
        let mut attempts = 0;
        let result = _run_with_transient_retries(
            &launch,
            &auto_process(),
            &[Duration::ZERO],
            |_, _| {
                attempts += 1;
                Ok(managed_attempt(AgentProcessResult {
                    agent_session: Some("/fixture/pi.jsonl".into()),
                    ..failed_result("connection reset")
                }))
            },
            |_| panic!("Pi owns retries"),
        )
        .unwrap();
        assert_eq!(attempts, 1);
        assert_ne!(result.exit_code, 0);
        assert_eq!(result.agent_session, Some("/fixture/pi.jsonl".into()));
    }

    #[test]
    fn transient_capacity_failure_resumes_and_finishes() {
        let launch = AgentConfig {
            agent: Some("codex".to_string()),
            task_prompt: "compress the branch".to_string(),
            ..Default::default()
        };
        let process = auto_process();
        let mut results = vec![
            AgentProcessResult {
                exit_code: 1,
                stdout: String::new(),
                stderr: "Selected model is at capacity. Please try a different model.".into(),
                agent_session: Some("thread-123".into()),
                failure: None,
            },
            AgentProcessResult {
                exit_code: 0,
                ..Default::default()
            },
        ]
        .into_iter();
        let mut attempts = Vec::new();
        let mut waits = Vec::new();

        let result = _run_with_transient_retries(
            &launch,
            &process,
            &[Duration::ZERO],
            |config, retry| {
                assert_eq!(retry, !attempts.is_empty());
                attempts.push(config.clone());
                Ok(managed_attempt(
                    results.next().expect("scripted launch result"),
                ))
            },
            |delay| waits.push(delay),
        )
        .expect("retry succeeds");

        assert_eq!(result.exit_code, 0);
        assert_eq!(attempts.len(), 2);
        assert_eq!(attempts[0].task_prompt, "compress the branch");
        assert_eq!(attempts[0].resume_token, None);
        assert_eq!(attempts[1].task_prompt, RETRY_PROMPT);
        assert_eq!(
            attempts[1].resume_token.as_ref(),
            Some(&"thread-123".into())
        );
        assert_eq!(waits, vec![Duration::ZERO]);
    }

    #[test]
    fn answer_prose_cannot_supply_provider_errors_or_resume_identity() {
        let launch = default_launch();
        let mut result = AgentProcessResult {
            exit_code: 1,
            stdout: concat!(
                "{\"type\":\"system\",\"session_id\":\"quoted-session\"}\n",
                "{\"type\":\"error\",\"message\":\"You've hit your usage limit\"}\n"
            )
            .into(),
            ..Default::default()
        };
        assert_eq!(_classify_agent_failure(&result), None);
        result.stderr = "service unavailable".into();
        let mut attempts = 0;
        _run_with_transient_retries(
            &launch,
            &auto_process(),
            &[Duration::ZERO],
            |config, _| {
                attempts += 1;
                assert_eq!(config.resume_token, None);
                assert_eq!(config.task_prompt, launch.task_prompt);
                Ok(managed_attempt(result.clone()))
            },
            |_| {},
        )
        .unwrap();
        assert_eq!(attempts, 2);
    }

    #[test]
    fn subscription_limit_fails_over_without_resuming_the_account_session() {
        let launch = AgentConfig {
            agent: Some("claude:opus".to_string()),
            task_prompt: "compress the branch".to_string(),
            resume_token: Some("session-123".into()),
            ..Default::default()
        };
        let process = auto_process();
        let failure = AgentProcessResult {
            exit_code: 1,
            agent_session: Some("session-123".into()),
            ..failed_result("You've hit your usage limit")
        };
        assert!(matches!(
            _classify_agent_failure(&failure),
            Some(AgentFailure::AccountSubscriptionLimit { resets_at: None })
        ));
        let mut results = vec![
            failure,
            AgentProcessResult {
                exit_code: 0,
                ..Default::default()
            },
        ]
        .into_iter();
        let mut attempts = Vec::new();
        let mut waits = Vec::new();

        let result = _run_with_transient_retries(
            &launch,
            &process,
            &[Duration::from_secs(30)],
            |config, retry| {
                assert_eq!(retry, !attempts.is_empty());
                attempts.push(config.clone());
                Ok(managed_attempt(
                    results.next().expect("scripted launch result"),
                ))
            },
            |delay| waits.push(delay),
        )
        .expect("account failover succeeds");

        assert_eq!(result.exit_code, 0);
        assert_eq!(attempts.len(), 2);
        assert_eq!(attempts[1].resume_token, None);
        assert!(attempts[1].task_prompt.starts_with(LIMIT_FAILOVER_PROMPT));
        assert!(attempts[1].task_prompt.contains("compress the branch"));
        assert_eq!(waits, vec![Duration::ZERO]);
    }

    #[test]
    fn revoked_credential_fails_over_without_resuming_the_account_session() {
        let launch = AgentConfig {
            agent: Some("codex".to_string()),
            task_prompt: "open the pull request".to_string(),
            ..Default::default()
        };
        let failure = failed_result(
            "Your authentication token has been invalidated (token_invalidated). Please sign in again.",
        );
        assert_eq!(
            _classify_agent_failure(&failure),
            Some(AgentFailure::AccountCredentialInvalidated)
        );
        let mut results = vec![
            failure,
            AgentProcessResult {
                exit_code: 0,
                ..Default::default()
            },
        ]
        .into_iter();
        let mut attempts = Vec::new();

        let result = _run_with_transient_retries(
            &launch,
            &auto_process(),
            &[Duration::from_secs(30)],
            |config, retry| {
                assert_eq!(retry, !attempts.is_empty());
                attempts.push(config.clone());
                Ok(managed_attempt(
                    results.next().expect("scripted launch result"),
                ))
            },
            |delay| assert_eq!(delay, Duration::ZERO),
        )
        .expect("credential failover succeeds");

        assert_eq!(result.exit_code, 0);
        assert_eq!(attempts.len(), 2);
        assert_eq!(attempts[1].resume_token, None);
        assert!(attempts[1]
            .task_prompt
            .starts_with(CREDENTIAL_FAILOVER_PROMPT));
        assert!(attempts[1].task_prompt.contains("open the pull request"));
    }

    #[test]
    fn unavailable_account_failover_returns_the_typed_failure() {
        let launch = AgentConfig {
            agent: Some("codex".to_string()),
            ..default_launch()
        };
        let process = auto_process();
        let mut attempts = 0;

        let result = _run_with_transient_retries(
            &launch,
            &process,
            &[Duration::ZERO],
            |_, _| {
                attempts += 1;
                if attempts > 1 {
                    return Ok(AgentAttempt::AccountUnavailable(
                        CoreError::ExecutionFailed("no eligible account".to_string()),
                    ));
                }
                Ok(managed_attempt(failed_result(
                    "You've hit your usage limit",
                )))
            },
            |_| {},
        )
        .expect("typed launch failure is returned");

        assert!(matches!(
            result.failure,
            Some(AgentFailure::AccountSubscriptionLimit { .. })
        ));
        assert_eq!(attempts, 2);
    }

    #[test]
    fn unrecorded_subscription_limit_is_not_retried() {
        let launch = AgentConfig {
            agent: Some("codex".to_string()),
            ..default_launch()
        };
        let result = _run_with_transient_retries(
            &launch,
            &auto_process(),
            &[Duration::ZERO],
            |_, _| {
                Ok(AgentAttempt::Finished {
                    result: failed_result("You've hit your usage limit"),
                    can_failover: false,
                })
            },
            |_| panic!("unsafe failover must not retry"),
        )
        .expect("typed launch failure is returned");

        assert!(matches!(
            result.failure,
            Some(AgentFailure::AccountSubscriptionLimit { .. })
        ));
    }

    #[test]
    fn a_turn_cut_off_by_an_account_switch_is_retried() {
        let launch = AgentConfig {
            agent: Some("codex".to_string()),
            ..default_launch()
        };
        let mut attempts = 0;

        let result = _run_with_transient_retries(
            &launch,
            &auto_process(),
            &[Duration::ZERO],
            |_, _| {
                attempts += 1;
                Ok(managed_attempt(if attempts == 1 {
                    let mut failed = AgentProcessResult {
                        exit_code: 1,
                        ..Default::default()
                    };
                    failed.stderr = "codex_error: Fatal error: application network permission \
                                     was revoked\n"
                        .into();
                    failed
                } else {
                    AgentProcessResult::default()
                }))
            },
            |_| {},
        )
        .unwrap();

        assert_eq!((result.exit_code, result.failure, attempts), (0, None, 2));
    }

    #[test]
    fn permanent_agent_failure_is_not_retried() {
        let launch = AgentConfig {
            agent: Some("codex".to_string()),
            ..default_launch()
        };
        let process = auto_process();
        let mut attempts = 0;

        let result = _run_with_transient_retries(
            &launch,
            &process,
            &[Duration::ZERO],
            |_, _| {
                attempts += 1;
                Ok(managed_attempt(failed_result("invalid request")))
            },
            |_| panic!("permanent failure must not wait"),
        )
        .expect("nonzero launch result is returned");

        assert_eq!(result.exit_code, 1);
        assert_eq!(attempts, 1);
    }

    #[test]
    fn transient_retry_exhaustion_returns_last_failure() {
        let launch = AgentConfig {
            agent: Some("codex".to_string()),
            ..default_launch()
        };
        let process = auto_process();
        let mut attempts = 0;
        let mut waits = 0;

        let result = _run_with_transient_retries(
            &launch,
            &process,
            &[Duration::ZERO, Duration::ZERO],
            |_, _| {
                attempts += 1;
                let mut result = failed_result("service unavailable");
                result.exit_code = attempts;
                Ok(managed_attempt(result))
            },
            |_| waits += 1,
        )
        .expect("last launch result is returned");

        assert_eq!(result.exit_code, 3);
        assert_eq!(result.failure, Some(AgentFailure::Unavailable));
        assert_eq!(attempts, 3);
        assert_eq!(waits, 2);
    }

    #[test]
    fn claude_terminal_command_preserves_provider_session() {
        let launch = AgentConfig {
            agent: Some("claude:opus".to_string()),
            resume_token: Some("session-123".into()),
            ..default_launch()
        };
        let claude = build_session_command(
            &launch,
            ProviderInvocation::Resume {
                session_id: launch.resume_token.as_ref().unwrap(),
            },
            None,
        )
        .unwrap()
        .args;
        assert_arg_pair(&claude, "--resume", "session-123");
    }
}
