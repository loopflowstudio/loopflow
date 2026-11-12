//! Agent invocation for spawning coding agent runners (Claude, Codex, OpenCode).
//!
//! This module handles building commands and spawning subprocesses for each
//! supported coding agent. Output can be captured or streamed.

mod session;
pub(crate) use session::{
    build_session_command, native_connection_command, resolve_session_config, session_environment,
    ProviderInvocation, SessionCommand,
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

use crate::config::parse_agent;
use crate::error::CoreError;
use crate::os_process::wait_for_exit;
use crate::platform::kill_process;
use crate::process::LfProcessRuntime;
use crate::prompt::structured_reply::{render_structured_reply_guidance, StructuredReply};
use crate::provider_account::{
    resolve_provider_account_exact_blocking, resolve_recorded_provider_account_blocking,
    ProviderAccountRoute,
};
use crate::provider_auth::Provider;
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
/// the process exits. Cleanups run in reverse registration order, as
/// destructors would: the Process record registered at startup completes
/// only after later work (a shell's jobs, a Session attachment) has settled,
/// so whoever waits for the Process to end reads settled state.
pub fn register_interrupt_cleanup(f: impl Fn() + Send + 'static) {
    interrupt_hooks()
        .lock()
        .expect("interrupt hooks lock poisoned")
        .push(Box::new(f));
}

/// Finish interruption while excluding the command's ordinary return path.
/// Killing a child can wake that path before the remaining hooks finish.
pub fn exit_on_interrupt() -> ! {
    // Before taking the hooks lock: the command's own return waits on it.
    crate::os_process::foreground::end_leader();
    let hooks = INTERRUPT_HOOKS.get().and_then(|hooks| hooks.lock().ok());
    if let Some(hooks) = &hooks {
        for hook in hooks.iter().rev() {
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
    /// Claude Chrome integration. `None` sends no flag so Claude's own
    /// configuration decides; an explicit choice sends `--chrome`/`--no-chrome`.
    pub chrome: Option<bool>,
    /// Initial Codex tier; omission leaves provider configuration in charge.
    pub codex_service_tier: Option<crate::config::CodexServiceTier>,
    /// Starting model for bare Claude selection; the authored agent stays bare for fallthrough.
    pub claude_model: Option<String>,
    /// System/context prompt content.
    pub system_prompt: String,
    /// Saved sources for native startup and compaction hooks.
    pub conversation_context: Option<crate::context_block::ContextDelivery>,
    /// Who holds the provider process, which decides its hooks, its server
    /// check and its steers.
    pub holder: crate::session::Holder,
    /// Task prompt content sent as the turn input.
    pub task_prompt: String,
    /// Native skill selection, kept separate from gathered reference context.
    pub skill_invocation: Option<crate::skills::invocation::SkillInvocation>,
    /// Native final-answer schema for this turn.
    pub output_schema: Option<serde_json::Value>,
    /// Agent string (for example: "claude:opus" or "codex").
    pub agent: Option<String>,
    /// Agents a Graph step moves to, in order, when a usage limit refuses
    /// `agent`: the rest of the configured order (`default_agent`, then
    /// `agents`). Empty when the agent was an explicit choice.
    pub fallback_agents: Vec<String>,
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
        }
    }

    /// Resolve the selected agent, or the sole installed harness.
    ///
    /// # Errors
    /// Returns `AgentSelection` when no preference resolves and installation is ambiguous or absent.
    pub fn agent(&self) -> Result<&str, crate::error::LoadError> {
        crate::config::select_agent(self.agent.as_deref())
    }

    pub(crate) fn resolved_agent(&self) -> Result<String, crate::error::LoadError> {
        let agent = self.agent()?;
        let (harness, model) = parse_agent(agent);
        match (harness.as_str(), model, self.claude_model.as_deref()) {
            ("claude", None, Some(model)) => Ok(format!("claude:{model}")),
            _ => Ok(agent.to_owned()),
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
            .field("codex_service_tier", &self.codex_service_tier)
            .field("claude_model", &self.claude_model)
            .field("fallback_agents", &self.fallback_agents)
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
    _pin_provider_account_id(launch).map_err(|attempt| attempt.into_error())
}

/// A Graph step that finds every account cooling launches unpinned: the
/// retry loop falls through to another model or provider, or waits.
fn _pin_provider_account_waiting(
    launch: &mut AgentConfig,
    process: &ProcessConfig,
) -> Result<(), CoreError> {
    match _pin_provider_account_id(launch) {
        Ok(()) => Ok(()),
        Err(AccountUnavailable {
            retry_at: Some(_), ..
        }) if process.wait_for_capacity => Ok(()),
        Err(unavailable) => Err(unavailable.into_error()),
    }
}

fn _pin_provider_account_id(launch: &mut AgentConfig) -> Result<(), AccountUnavailable> {
    let agent = launch.agent().map_err(AccountUnavailable::terminal)?;
    let (harness, _) = parse_agent(agent);
    let provider = match harness.as_str() {
        "claude" => Provider::Claude,
        "codex" => Provider::Codex,
        _ if launch.provider_account_id.is_some() => {
            return Err(AccountUnavailable::terminal(
                "managed provider account IDs apply only to Claude and Codex",
            ));
        }
        _ => return Ok(()),
    };
    let route = resolve_account_route_blocking(provider, launch)
        .map_err(|error| AccountUnavailable::selecting("select", error))?;
    if let Some(route) = route {
        launch.provider_account_id = Some(route.account_id().clone());
    }
    Ok(())
}

/// No account can take this launch, and when one can, if that is known.
#[derive(Debug)]
struct AccountUnavailable {
    error: String,
    retry_at: Option<i64>,
}

impl AccountUnavailable {
    fn terminal(error: impl ToString) -> Self {
        Self {
            error: error.to_string(),
            retry_at: None,
        }
    }

    fn selecting(verb: &str, error: crate::provider_account::ProviderAccountError) -> Self {
        let retry_at = match &error {
            crate::provider_account::ProviderAccountError::NoEligibleAccount {
                retry_at, ..
            } => *retry_at,
            _ => None,
        };
        Self {
            error: format!("failed to {verb} provider account: {error}"),
            retry_at,
        }
    }

    fn into_error(self) -> CoreError {
        CoreError::ExecutionFailed(self.error)
    }
}

/// One step's wait for capacity. Says so once per reset time, in the log and
/// where Task status reads it.
#[derive(Debug, Default)]
struct CapacityWait {
    announced: Option<i64>,
}

impl CapacityWait {
    fn pause(&mut self, provider: &str, reason: &str, retry_at: i64) -> Duration {
        if self.announced != Some(retry_at) {
            self.announced = Some(retry_at);
            eprintln!(
                "Waiting for {provider} capacity until {}: {reason}",
                crate::lf::output::format_time(retry_at)
            );
            _record_limit_recovery(provider, reason, Some(retry_at));
        }
        let left = retry_at - crate::store::rows::now_unix();
        Duration::from_secs(left.clamp(1, CAPACITY_POLL_SECS) as u64)
    }
}

/// Status only: a store that cannot say so never stops the recovery.
fn _note_limit_recovery(
    note: impl FnOnce(
        &crate::store::sqlite::SqliteStore,
        &crate::id::LfProcessId,
    ) -> crate::store::StoreResult<()>,
) {
    let Some(process) = crate::journal::current_lf_process_id() else {
        return;
    };
    let noted = || -> anyhow::Result<()> {
        let path = crate::store::database_path_from_env()?;
        Ok(note(
            &crate::store::sqlite::SqliteStore::new(&path)?,
            &process,
        )?)
    };
    if let Err(error) = noted() {
        tracing::warn!(%error, "could not note limit recovery");
    }
}

fn _record_limit_recovery(provider: &str, reason: &str, retry_at: Option<i64>) {
    _note_limit_recovery(|store, process| {
        store.record_agent_limit_recovery(
            process,
            provider,
            reason,
            retry_at,
            crate::config::agent_override().as_deref(),
        )
    });
}

/// The step left `provider` for the next agent in its order.
fn _record_agent_move(provider: &str, reason: &str, next: &str) {
    _record_limit_recovery(
        provider,
        &format!("{reason}{}{next}", crate::ops::task_execution::MOVED_TO),
        None,
    );
}

/// A usage limit refused the step and no agent it may use is left: the stop
/// and what would let it move, where Task status and `lf task run` read it.
fn _record_limit_stop(provider: &str, reason: &str, launch: &AgentConfig, tried: &[String]) {
    let advice = _limit_stop_advice(provider, launch, tried);
    _record_limit_recovery(
        provider,
        &format!("{reason}{}{advice}", crate::ops::task_execution::STOPPED),
        None,
    );
}

fn _limit_stop_advice(provider: &str, launch: &AgentConfig, tried: &[String]) -> String {
    if !launch.fallback_agents.is_empty() {
        return format!(
            "Nothing is left in its order after {}; add another agent to `agents`",
            tried.join(", ")
        );
    }
    let another = if provider == "claude" {
        "codex"
    } else {
        "claude"
    };
    format!(
        "Nothing else may take the step: an `agent` choice or `-a` pins it to {provider}, or there is no `agents` list. `lf set --global agents {provider},{another}` lets a step move on"
    )
}

fn resolve_account_route_blocking(
    provider: Provider,
    launch: &AgentConfig,
) -> Result<Option<ProviderAccountRoute>, crate::provider_account::ProviderAccountError> {
    let agent = launch.resolved_agent().map_err(|error| {
        crate::provider_account::ProviderAccountError::Runtime(error.to_string())
    })?;
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
            parse_agent(&agent).1,
            launch.cwd.clone(),
        ),
        (None, Some(_)) => Err(crate::provider_account::ProviderAccountError::Runtime(
            "recorded account authority requires an account ID".to_string(),
        )),
    }?;
    if route.is_none() && launch.execution_boundary.is_some() {
        return Err(crate::provider_account::ProviderAccountError::NoEligibleAccount {
            provider,
            accounts: "a connected managed account is required by this execution boundary; ambient credentials cannot satisfy it".to_string(),
            retry_at: None,
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
    /// Live resources for this lf invocation, retained across provider retries.
    pub runtime: Option<LfProcessRuntime>,
    /// A Graph step goes on after a limit resets. Everything else headless
    /// (commit messages, PR text) has someone waiting on it and fails now.
    pub wait_for_capacity: bool,
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

/// App-server overrides. `None` preserves vendor policy. A headless run must
/// write to the workspace and has nobody to answer an approval, so it gets at
/// least `workspace-write` and `never`; an attended Session keeps Codex's own
/// sandbox and approval configuration unless `yolo` or a managed boundary applies.
fn codex_permission_policy(
    config: CodexPermissionConfig,
    launch: &AgentConfig,
    headless: bool,
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
    if !headless {
        return CodexPermissionConfig::default();
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

/// Explicit `yolo` always bypasses. Otherwise an attended Session keeps
/// Claude's own permission mode, and only an unattended run bypasses: nobody
/// is present to answer a prompt, so a prompt would stall the step.
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
    // Harness preferences (Chrome, model) pass through only when a Loopflow
    // setting names them; Claude's own configuration decides the rest.
    let mut args: Vec<String> = Vec::new();
    if let Some(chrome) = config.chrome {
        args.push(if chrome { "--chrome" } else { "--no-chrome" }.into());
    }
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

const TRANSIENT_RETRY_DELAYS: [Duration; 4] = [
    Duration::from_secs(2),
    Duration::from_secs(5),
    Duration::from_secs(15),
    Duration::from_secs(30),
];
const RETRY_PROMPT: &str = "Continue the original task after the transient provider failure. Re-read the current workspace state, finish any interrupted work, and return the result.";
const LIMIT_FAILOVER_PROMPT: &str = "Continue this task after the previous provider account reached its subscription limit. Re-read the current workspace state and finish any interrupted work.";
/// How long a wait for capacity sleeps before looking again. Sleep stops the
/// monotonic clock on a Mac; each look reads the wall clock and the accounts,
/// so a cleared cooldown or a new login ends the wait early.
const CAPACITY_POLL_SECS: i64 = 60;
/// Accounts one step tries back to back before giving up: limits that land on
/// a named account, or on none a cooldown can hold back, never spin.
const MAX_ACCOUNT_FAILOVERS: usize = 4;
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
    AccountSubscriptionLimit,
    /// The selected managed account's credential was explicitly revoked.
    #[error("account credential invalidated")]
    AccountCredentialInvalidated,
}

/// Build `thread/start` params for Codex app-server sessions. `headless` runs
/// get Loopflow's permission floor; attended Sessions keep Codex's own.
pub fn build_codex_thread_start_params(
    launch: &AgentConfig,
    headless: bool,
) -> serde_json::Map<String, serde_json::Value> {
    let mut params = serde_json::Map::new();
    if let Some(tier) = launch.codex_service_tier {
        params.insert("serviceTier".into(), tier.as_str().into());
    }
    let instructions = system_prompt_with_structured_replies(launch);
    if !instructions.is_empty() {
        params.insert(
            "developerInstructions".into(),
            serde_json::Value::String(instructions),
        );
    }

    if let Some(model) = launch.agent.as_deref().and_then(resolve_codex_model) {
        params.insert("model".to_string(), serde_json::Value::String(model));
    }

    if let Some(cwd) = launch.cwd.as_ref() {
        params.insert(
            "cwd".to_string(),
            serde_json::Value::String(cwd.to_string_lossy().to_string()),
        );
    }

    let policy = codex_permission_policy(
        read_codex_permission_config(launch.cwd.as_deref()),
        launch,
        headless,
    );
    if let Some(sandbox) = policy.sandbox {
        params.insert("sandbox".into(), sandbox.as_str().into());
    }
    if policy.approval == Some(CodexApprovalPolicy::Never) {
        params.insert("approvalPolicy".into(), "never".into());
    }

    params
}

/// A bare Codex launch's options, as plain `codex` takes them: the thread
/// settings an app-server launch would send, the writable roots and the two
/// context hooks. Nothing else of lf's configuration rides along.
pub(crate) fn codex_terminal_options(launch: &AgentConfig) -> anyhow::Result<Vec<String>> {
    let params = build_codex_thread_start_params(launch, false);
    let mut args = Vec::new();
    if let Some(model) = params.get("model").and_then(serde_json::Value::as_str) {
        args.extend(["--model".into(), model.into()]);
    }
    if let Some(sandbox) = params.get("sandbox").and_then(serde_json::Value::as_str) {
        args.extend(["--sandbox".into(), sandbox.into()]);
    }
    if let Some(approval) = params
        .get("approvalPolicy")
        .and_then(serde_json::Value::as_str)
    {
        args.extend(["--ask-for-approval".into(), approval.into()]);
    }
    for dir in provider_writable_roots(launch) {
        args.extend(["--add-dir".into(), dir.display().to_string()]);
    }
    let mut config = serde_json::Map::new();
    for (param, key) in [
        ("serviceTier", "service_tier"),
        ("developerInstructions", "developer_instructions"),
    ] {
        if let Some(value) = params.get(param) {
            config.insert(key.into(), value.clone());
        }
    }
    if let Some(context) = &launch.conversation_context {
        config.insert(
            "hooks".into(),
            crate::harness::context::codex_hooks(context, None, launch.holder)?,
        );
    }
    args.extend(crate::harness::codex::config_overrides(&config.into())?);
    Ok(args)
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
    if process.auto && parse_agent(launch.agent()?).0 != "claude" {
        if launch.max_turns.is_some() {
            eprintln!("warning: Loopflow turn limits are supported only by Claude; this provider will run without a turn limit");
        }
        if launch.output_schema.is_some() {
            eprintln!("warning: Loopflow native response-schema enforcement is Claude-specific; using ordinary provider output (Graph output validation still applies)");
        }
    }
    if launch.resume_token.is_none() {
        if let Some(runtime) = &process.runtime {
            launch.resume_token = runtime.conversation_resume_token().map_err(|error| {
                CoreError::ExecutionFailed(format!("conversation recovery failed: {error}"))
            })?;
        }
    }
    _pin_provider_account_waiting(&mut launch, process)?;
    let (harness, model) = parse_agent(&launch.resolved_agent()?);
    let implicit_runtime = if process.runtime.is_none() {
        Some(_begin_process_runtime(&launch, process, &harness, model)?)
    } else {
        None
    };
    let mut process = process.clone();
    for name in EXECUTION_IDENTITY_ENV {
        launch.env.remove(name);
    }
    if let Some(runtime) = &implicit_runtime {
        process.runtime = Some(runtime.clone());
    }
    if let Some(runtime) = &process.runtime {
        runtime.claim_session_attachment().map_err(|error| {
            CoreError::ExecutionFailed(format!("conversation admission failed: {error}"))
        })?;
        launch.session_attachment = runtime.session_attachment();
        if launch.resume_token.is_none() {
            launch.resume_token = runtime.conversation_resume_token().map_err(|error| {
                CoreError::ExecutionFailed(format!("conversation recovery failed: {error}"))
            })?;
        }
        launch.env.extend(runtime.environment());
        runtime.mark_spawn_requested();
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
    if let Some(runtime) = implicit_runtime {
        let outcome = match &result {
            Ok(result) if result.exit_code == 0 => "completed",
            Ok(_) | Err(_) => "failed",
        };
        runtime
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
    AccountUnavailable(AccountUnavailable),
}

fn _run_with_transient_retries(
    launch: &AgentConfig,
    process: &ProcessConfig,
    retry_delays: &[Duration],
    mut run: impl FnMut(&AgentConfig, bool) -> Result<AgentAttempt, CoreError>,
    mut wait: impl FnMut(Duration),
) -> Result<AgentProcessResult, CoreError> {
    let rungs = _fallthrough_rungs(launch, process)?;
    let mut rung = 0;
    // How the step stood when it began falling through; every rung starts
    // from here, and the wait at the top of the ladder returns to it.
    let mut origin: Option<AgentConfig> = None;
    let mut earliest_reset: Option<i64> = None;
    let mut attempt_config = launch.clone();
    let mut launched = false;
    let mut transient_failures = 0;
    let mut failovers = 0;
    let mut account_failure: Option<AgentProcessResult> = None;
    let mut capacity = CapacityWait::default();

    loop {
        let provider = parse_agent(attempt_config.agent()?).0;
        let retry = launched || attempt_config.agent != launch.agent;
        let (mut result, can_failover) = match run(&attempt_config, retry)? {
            AgentAttempt::Finished {
                result,
                can_failover,
            } => (result, can_failover),
            AgentAttempt::AccountUnavailable(AccountUnavailable {
                error,
                retry_at: Some(retry_at),
            }) if process.wait_for_capacity => {
                failovers = 0;
                let reset = earliest_reset.map_or(retry_at, |at| at.min(retry_at));
                let origin = origin.get_or_insert_with(|| attempt_config.clone());
                rung += 1;
                if rung == rungs.len() {
                    rung = 0;
                    earliest_reset = None;
                    wait(capacity.pause(&provider, &error, reset));
                    attempt_config = origin.clone();
                } else {
                    earliest_reset = Some(reset);
                    let next = &rungs[rung];
                    tracing::warn!(error, agent = %next, "every account held back; falling through");
                    _record_agent_move(&provider, &error, next);
                    attempt_config = _climb(origin, next, process, launch);
                }
                continue;
            }
            AgentAttempt::AccountUnavailable(unavailable) => {
                let Some(result) = account_failure.as_ref() else {
                    return Err(unavailable.into_error());
                };
                // A thread that left no transcript cannot move. The step starts
                // over on the checkout rather than ending on a limit.
                if attempt_config.resume_token.take().is_some() {
                    tracing::warn!(error = unavailable.error, "conversation could not move");
                    attempt_config.task_prompt =
                        _with_original_task(&attempt_config.task_prompt, launch);
                    attempt_config.skill_invocation = launch.skill_invocation.clone();
                    continue;
                }
                tracing::warn!(
                    error = unavailable.error,
                    "alternate provider account unavailable"
                );
                if result.failure == Some(AgentFailure::AccountSubscriptionLimit) {
                    let reason = _failure_reason(result, AgentFailure::AccountSubscriptionLimit);
                    _record_limit_stop(&provider, &reason, launch, &rungs[..=rung]);
                }
                return Ok(result.clone());
            }
        };
        launched = true;
        origin = None;
        let failure = if process.auto {
            result.failure.or_else(|| _classify_agent_failure(&result))
        } else {
            None
        };
        result.failure = failure;
        // Pi and OpenCode cannot be resumed without adding a new
        // model-visible input. Preserve failure; only explicit input starts more work.
        if matches!(provider.as_str(), "opencode" | "pi") {
            return Ok(result);
        }
        let Some(failure) = failure else {
            return Ok(result);
        };
        let thread = result.agent_session.clone().or(launch.resume_token.clone());
        attempt_config = launch.clone();
        attempt_config.agent = Some(rungs[rung].clone());
        // A route chooses again: an exact pin would hold the step on an
        // account that has since run out. The thread keeps its account when it can.
        if can_failover {
            attempt_config.provider_account_id = None;
        }
        let delay = match failure {
            AgentFailure::AccountSubscriptionLimit | AgentFailure::AccountCredentialInvalidated => {
                let prompt = if failure == AgentFailure::AccountSubscriptionLimit {
                    LIMIT_FAILOVER_PROMPT
                } else {
                    CREDENTIAL_FAILOVER_PROMPT
                };
                let stuck = !can_failover || failovers == MAX_ACCOUNT_FAILOVERS;
                // No other login can take the step: a launch without a route,
                // or one that refused on every login. The next agent in its
                // order can, whether or not a turn ever ran.
                let next = rungs
                    .get(rung + 1)
                    .filter(|_| stuck && failure == AgentFailure::AccountSubscriptionLimit);
                let reason = _failure_reason(&result, failure);
                if stuck && next.is_none() {
                    if failure == AgentFailure::AccountSubscriptionLimit {
                        _record_limit_stop(&provider, &reason, launch, &rungs[..=rung]);
                    }
                    return Ok(result);
                }
                // The conversation moves with the step; a turn that never
                // reached a thread still has its whole task to do.
                match thread {
                    Some(thread) => {
                        attempt_config.resume_token = Some(thread);
                        attempt_config.task_prompt = prompt.to_string();
                        attempt_config.skill_invocation = None;
                    }
                    None => {
                        attempt_config.task_prompt = _with_original_task(prompt, launch);
                    }
                }
                match next {
                    None => {
                        failovers += 1;
                        account_failure = Some(result.clone());
                        _record_limit_recovery(&provider, &reason, None);
                    }
                    Some(next) => {
                        rung += 1;
                        failovers = 0;
                        account_failure = None;
                        _record_agent_move(&provider, &reason, next);
                        attempt_config = _climb(&attempt_config, next, process, launch);
                    }
                }
                Duration::ZERO
            }
            _ => {
                let Some(delay) = retry_delays.get(transient_failures).copied() else {
                    return Ok(result);
                };
                transient_failures += 1;
                account_failure = None;
                if let Some(thread) = thread {
                    attempt_config.resume_token = Some(thread);
                    attempt_config.task_prompt = RETRY_PROMPT.to_string();
                    attempt_config.skill_invocation = None;
                }
                delay
            }
        };

        tracing::warn!(
            failure = %failure,
            transient_failures,
            failovers,
            delay_ms = delay.as_millis(),
            resumed = attempt_config.resume_token.is_some(),
            "recoverable agent failure; retrying"
        );
        wait(delay);
    }
}

fn _with_original_task(prompt: &str, launch: &AgentConfig) -> String {
    format!("{prompt}\n\nOriginal task:\n\n{}", launch.task_prompt)
}

/// The agents a Graph step tries in turn when a usage limit refuses one. With
/// an order to move through (`default_agent`, then `agents`) it is that order
/// as written, a bare harness on its configured model. With none, a bare
/// harness tries its other models and an explicit `harness:model` stays put.
/// A replay of a recorded account and anything outside a Graph step never moves.
fn _fallthrough_rungs(
    launch: &AgentConfig,
    process: &ProcessConfig,
) -> Result<Vec<String>, CoreError> {
    let first = launch.agent()?.to_owned();
    if !process.wait_for_capacity || launch.provider_account_authority_home.is_some() {
        return Ok(vec![first]);
    }
    let configured = |agent: &str| match (agent, &launch.claude_model) {
        ("claude", Some(model)) => format!("claude:{model}"),
        _ => agent.to_owned(),
    };
    let mut order: Vec<String> = std::iter::once(first.as_str())
        .chain(launch.fallback_agents.iter().map(String::as_str))
        .map(configured)
        .collect();
    // With no order to move through, a bare Claude tries its other models.
    if launch.fallback_agents.is_empty() && first == "claude" {
        order.extend(crate::config::CLAUDE_MODEL_RUNGS.map(|model| format!("claude:{model}")));
    }
    let mut rungs: Vec<String> = Vec::new();
    for rung in order {
        if !rungs.contains(&rung) {
            rungs.push(rung);
        }
    }
    Ok(rungs)
}

/// Put the step on `agent`, starting from how it stood before it began falling
/// through. The thread moves within its provider; another provider reads the
/// conversation so far and begins its own.
fn _climb(
    origin: &AgentConfig,
    agent: &str,
    process: &ProcessConfig,
    launch: &AgentConfig,
) -> AgentConfig {
    let mut next = origin.clone();
    next.agent = Some(agent.to_owned());
    next.provider_account_id = None;
    let same_provider = origin
        .agent
        .as_deref()
        .is_some_and(|from| parse_agent(from).0 == parse_agent(agent).0);
    if !same_provider {
        next.resume_token = None;
        next.skill_invocation = None;
        next.task_prompt = _history_import(process)
            .unwrap_or_else(|| _with_original_task(LIMIT_FAILOVER_PROMPT, launch));
    }
    next
}

/// The conversation so far as the next provider's first turn, through the same
/// import `lf session new -a` uses. None when the step has no Session or its
/// transcript cannot be read yet; the task then starts over on the checkout.
fn _history_import(process: &ProcessConfig) -> Option<String> {
    let (session_id, _) = process.runtime.as_ref()?.session_attachment()?;
    let imported = (|| -> anyhow::Result<String> {
        let store =
            crate::store::sqlite::SqliteStore::new(&crate::store::database_path_from_env()?)?;
        let session = store
            .session(&session_id)?
            .ok_or_else(|| anyhow::anyhow!("Session {session_id} is not recorded"))?;
        crate::ops::human_session::conversation_log::continuation(&store, &session)
    })();
    match imported {
        Ok(turn) => Some(turn),
        Err(error) => {
            tracing::warn!(%error, "conversation could not move providers; the task starts over");
            None
        }
    }
}

/// The provider's own words for a limit, when it gave any.
fn _failure_reason(result: &AgentProcessResult, failure: AgentFailure) -> String {
    result
        .stderr
        .lines()
        .next()
        .map_or_else(|| failure.to_string(), str::to_owned)
}

fn _classify_agent_failure(result: &AgentProcessResult) -> Option<AgentFailure> {
    if result.exit_code == 0 {
        return None;
    }
    if _find_provider_error(result, credential_invalidated_failure).is_some() {
        return Some(AgentFailure::AccountCredentialInvalidated);
    }
    if _find_provider_error(result, subscription_limit_text).is_some() {
        return Some(AgentFailure::AccountSubscriptionLimit);
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
    } else if text.contains("at capacity")
        || text.contains("capacity temporarily unavailable")
        || text.contains("capacity issue")
    {
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

/// A provider's line saying the account's quota is spent, from a step's stderr
/// or the line a native Claude displayed. Claude's openers are its own list of
/// usage-limit messages; its capacity, per-model and seat-type lines name no
/// spent window.
pub(crate) fn subscription_limit_text(line: &str) -> Option<SpentLimit> {
    let text = line.to_ascii_lowercase();
    (text.contains("you've hit your")
        || text.contains("you've reached your")
        || text.contains("you're out of usage credits")
        || text.contains("you have hit your usage limit")
        || text.contains("usage limit reached")
        || text.contains("usage limit has been reached")
        || text.contains("subscription limit")
        || text.contains("subscription quota")
        || text.contains("quota exceeded")
        || text.contains("insufficient_quota")
        || text.contains("rate_limit_reached"))
    .then(|| SpentLimit {
        reason: line.to_string(),
        model: _limited_model(&text),
        credits: text.contains("out of usage credits"),
    })
}

/// Spent quota a provider reported in words.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct SpentLimit {
    pub reason: String,
    /// The model family the line names ("You've reached your Fable limit"),
    /// as limit scopes spell it; `None` when the whole account is spent,
    /// unless `credits`.
    pub model: Option<String>,
    /// "You're out of usage credits": the credits pay for the model the
    /// launch ran, so only that model is spent.
    pub credits: bool,
}

/// The model family named between "your" and "limit" in a lowercased line.
fn _limited_model(text: &str) -> Option<String> {
    let named = text.split_once("your ")?.1.split_once("limit")?.0;
    named
        .split(|character: char| !character.is_ascii_alphanumeric())
        .find(|word| crate::config::CLAUDE_MODEL_FAMILIES.contains(word))
        .map(str::to_string)
}

fn _begin_process_runtime(
    launch: &AgentConfig,
    process: &ProcessConfig,
    harness: &str,
    model: Option<String>,
) -> Result<LfProcessRuntime, CoreError> {
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
    let runtime = if process.auto {
        crate::session_record::SessionRecorder::begin_with_request(
            spec,
            crate::session_record::AgentProcessRequest::from_prepared(launch)?,
        )
        .map(crate::process::LfProcessRuntime::new)
    } else {
        let context = crate::trace::PreparedTurnContext::from_prompts(
            &system_prompt_with_structured_replies(launch),
            &launch.task_prompt,
        );
        crate::session_record::SessionRecorder::begin_with_context(
            spec,
            &context,
            Some(crate::session_record::AgentProcessRequest::from_prepared(
                launch,
            )?),
        )
        .map(crate::process::LfProcessRuntime::new)
    };
    runtime.map_err(|error| {
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

    let runtime = process.runtime.as_ref();
    let (provider, _) = parse_agent(launch.agent()?);
    let account_route = match match provider.as_str() {
        "codex" => resolve_account_route_blocking(Provider::Codex, launch),
        "claude" => resolve_account_route_blocking(Provider::Claude, launch),
        _ => Ok(None),
    } {
        Ok(route) => route,
        Err(error) => {
            return Ok(AgentAttempt::AccountUnavailable(
                AccountUnavailable::selecting("select", error),
            ));
        }
    };
    _note_limit_recovery(|store, process| store.end_agent_limit_wait(process));
    if retry {
        if let Some(runtime) = runtime {
            runtime
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
    if let Some(runtime) = runtime {
        // A new harness gets the invocation owner's current snapshot. Existing
        // dispatch/history operations never refresh theirs from the runtime.
        config.session_attachment = runtime.session_attachment();
        config.env.extend(runtime.environment());
    }
    let prompt = std::mem::take(&mut config.task_prompt);
    let launch_worktree = config.cwd.clone().or_else(|| std::env::current_dir().ok());
    if let Some(cwd) = launch_worktree.as_deref() {
        crate::ops::git_operation::prepare_agent_process(cwd)
            .map_err(|error| CoreError::ExecutionFailed(error.to_string()))?;
    }
    let executor = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .map_err(|error| CoreError::ExecutionFailed(error.to_string()))?;
    let result = executor.block_on(async {
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
        harness.set_runtime(process.runtime.clone());
        harness
            .start(&config)
            .await
            .map_err(|error| CoreError::ExecutionFailed(error.to_string()))?;
        let agent_session = harness.agent_session();
        if let Some(runtime) = runtime {
            runtime.observe_provider(agent_session.clone(), harness.provider_account_id());
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
            let mut limited = false;
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
                    _ = activity_tick.tick(), if runtime.is_some() => {
                        if let Some(runtime) = runtime { runtime.observe_activity(harness.pid()).await; }
                    }
                    event = event_rx.recv() => {
                        let Some(event) = event else {
                            stderr.push_str(&format!("{provider} event stream closed\n"));
                            exit_code = Some(1);
                            continue;
                        };
                        if let Some(runtime) = runtime {
                            runtime.observe_provider(harness.agent_session(), harness.provider_account_id());
                            runtime.record_conversation(event.clone());
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
                                limited |= code == "provider_rate_limited";
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
                if let Some(runtime) = runtime { runtime.record_conversation(event.clone()); }
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
                failure: limited.then_some(AgentFailure::AccountSubscriptionLimit),
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
            // A limit only the provider's words reported has no stream
            // observation: cool the account so the next choice can differ.
            if result.exit_code != 0 && result.failure.is_none() {
                if let Some(limit) = _find_provider_error(result, subscription_limit_text) {
                    route
                        .record_limit_blocking(limit)
                        .map_err(|error| CoreError::ExecutionFailed(error.to_string()))?;
                }
            }
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
    let (harness, model) = parse_agent(&launch.resolved_agent()?);
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
    let hooks = crate::harness::context::native_args(
        &harness,
        launch.conversation_context.as_ref(),
        launch.holder,
    )
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
                AccountUnavailable::selecting("select", error),
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
                    AccountUnavailable::selecting("activate", error),
                ));
            }
        };
    }
    if harness == "claude" {
        crate::harness::claude_trust::accept_created_worktree(&cmd);
    }
    cmd.args(hooks).args(&command.args);
    let runtime = process.runtime.as_ref();
    if retry {
        if let Some(runtime) = runtime {
            runtime
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

    if let Some(runtime) = runtime {
        runtime.observe_provider(
            None,
            account_route
                .as_ref()
                .map(|route| route.account_id().clone()),
        );
    }

    let admitted_runtime = runtime.ok_or_else(|| {
        CoreError::ExecutionFailed("AgentProcess launch has no admitted runtime".into())
    })?;
    let result = run_interactive(
        cmd,
        process.timeout,
        admitted_runtime,
        activation,
        title,
        launch.holder,
    );
    result.map(|result| AgentAttempt::Finished {
        result,
        can_failover: false,
    })
}

/// `activation` is the native credential lock a shared launch took; it is
/// released once the provider process exists.
fn spawn_agent_child(
    cmd: Command,
    runtime: &LfProcessRuntime,
    activation: Option<std::fs::File>,
    holder: crate::session::Holder,
) -> Result<(Child, crate::process::SessionAttachment), CoreError> {
    let child = runtime
        .spawn_native_agent(cmd, holder)
        .map_err(|error| CoreError::ExecutionFailed(error.to_string()))?;
    drop(activation);
    Ok(child)
}

fn run_interactive(
    mut cmd: Command,
    timeout: Option<Duration>,
    runtime: &LfProcessRuntime,
    activation: Option<std::fs::File>,
    mut title: Option<crate::terminal_title::TerminalTitle>,
    holder: crate::session::Holder,
) -> Result<AgentProcessResult, CoreError> {
    let start = Instant::now();
    let program = cmd.get_program().to_string_lossy().into_owned();
    let foreground = crate::os_process::foreground::hand_to(&mut cmd);
    let (mut child, attachment) = spawn_agent_child(cmd, runtime, activation, holder)?;
    let pid = child.id();
    let _pid_guard = ChildPidGuard::new(pid);
    if let Some(foreground) = &foreground {
        foreground.lead(pid);
    }
    tracing::debug!(
        elapsed_ms = start.elapsed().as_millis(),
        "agent spawned (interactive)"
    );
    let (status, timed_out) = wait_for_exit(&mut child, timeout, || {
        if let Some(foreground) = &foreground {
            foreground.follow_stop(pid);
        }
        if let Some(title) = title.as_mut() {
            title.refresh();
        }
    })?;
    runtime
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
    let exit_code = if holder == crate::session::Holder::Caller {
        crate::lf::commands::util::SessionCommandOutcome {
            status,
            interrupted: foreground
                .as_ref()
                .is_some_and(|foreground| foreground.interrupted(&status)),
            moved: None,
        }
        .bare_exit_code(&program)
        .into()
    } else {
        status.code().unwrap_or(1)
    };
    Ok(AgentProcessResult {
        exit_code,
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
                true,
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
            true,
        );
        assert_eq!(policy, CodexPermissionConfig::default());
    }

    #[test]
    fn attended_codex_keeps_its_own_policy_unless_yolo_or_a_boundary_applies() {
        let restrictive = CodexPermissionConfig {
            sandbox: Some(CodexSandboxMode::ReadOnly),
            approval: Some(CodexApprovalPolicy::OnRequest),
        };
        assert_eq!(
            codex_permission_policy(restrictive, &AgentConfig::default(), false),
            CodexPermissionConfig::default()
        );
        let yolo = AgentConfig {
            skip_permissions: true,
            ..AgentConfig::default()
        };
        assert_eq!(
            codex_permission_policy(restrictive, &yolo, false),
            CodexPermissionConfig {
                sandbox: Some(CodexSandboxMode::DangerFullAccess),
                approval: Some(CodexApprovalPolicy::Never),
            }
        );
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
        let thread = build_codex_thread_start_params(&launch, false);
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
                    let params = build_codex_thread_start_params(&launch, true);
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

        let params = build_codex_thread_start_params(&launch, true);
        assert_eq!(
            params.get("model"),
            Some(&serde_json::Value::String("o3".to_string()))
        );
        assert_eq!(
            params.get("cwd"),
            Some(&serde_json::Value::String("/tmp/repo".to_string()))
        );
        assert!(!params.contains_key("serviceTier"));
    }

    #[test]
    fn build_codex_thread_start_params_omits_model_for_bare_codex() {
        let launch = AgentConfig {
            agent: Some("codex".to_string()),
            cwd: Some("/tmp/repo".into()),
            ..default_launch()
        };

        let params = build_codex_thread_start_params(&launch, true);
        assert!(!params.contains_key("model"));
        assert_eq!(
            params.get("cwd"),
            Some(&serde_json::Value::String("/tmp/repo".to_string()))
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
    fn claude_temporary_capacity_rejection_is_capacity_not_spent_quota() {
        assert_eq!(
            classify_retryable_agent_failure(
                "Request rejected (429) · this may be a temporary capacity issue"
            ),
            Some(AgentFailure::Capacity)
        );
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
            Some(AgentFailure::AccountSubscriptionLimit)
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
        assert_eq!(attempts[1].resume_token, Some("session-123".into()));
        assert_eq!(attempts[1].task_prompt, LIMIT_FAILOVER_PROMPT);
        assert_eq!(waits, vec![Duration::ZERO]);
    }

    fn limited_result() -> AgentProcessResult {
        AgentProcessResult {
            exit_code: 1,
            agent_session: Some("thread".into()),
            failure: Some(AgentFailure::AccountSubscriptionLimit),
            ..failed_result("provider_rate_limited: rejected: seven_day_overage_included")
        }
    }

    fn done() -> AgentAttempt {
        managed_attempt(AgentProcessResult {
            exit_code: 0,
            ..Default::default()
        })
    }

    fn cooling(retry_at: Option<i64>) -> AgentAttempt {
        AgentAttempt::AccountUnavailable(AccountUnavailable {
            error: "no eligible managed claude account".to_string(),
            retry_at,
        })
    }

    /// November 2: this event read as a generic rate limit, kept the thread on
    /// the old terms and died in retry_agent_process.
    #[test]
    fn a_limit_event_moves_the_step_and_its_thread_without_spending_transient_retries() {
        let launch = AgentConfig {
            agent: Some("claude".to_string()),
            provider_account_id: Some(ProviderAccountId::parse("primary").unwrap()),
            task_prompt: "implement the plan".to_string(),
            ..Default::default()
        };
        let mut attempts = Vec::new();
        let result = _run_with_transient_retries(
            &launch,
            &auto_process(),
            &[],
            |config, _| {
                attempts.push(config.clone());
                Ok(if attempts.len() == 1 {
                    managed_attempt(limited_result())
                } else {
                    done()
                })
            },
            |_| {},
        )
        .unwrap();
        assert_eq!(result.exit_code, 0);
        assert_eq!(attempts[1].provider_account_id, None);
        assert_eq!(attempts[1].resume_token, Some("thread".into()));
        assert_eq!(attempts[1].task_prompt, LIMIT_FAILOVER_PROMPT);
        assert!(attempts[1].skill_invocation.is_none());
    }

    /// November 7, as it ran: bare `claude` (Fable) ran out of usage credits;
    /// the account still had room for Opus. The step goes on there, on the
    /// same account, in the same thread.
    #[test]
    fn a_credits_limit_goes_on_with_another_model_in_the_same_thread() {
        let launch = AgentConfig {
            agent: Some("claude".to_string()),
            task_prompt: "compress the branch".to_string(),
            ..Default::default()
        };
        let retry_at = crate::store::rows::now_unix() + 3 * 24 * 3600;
        let mut scripted = vec![
            managed_attempt(AgentProcessResult {
                exit_code: 1,
                agent_session: Some("thread".into()),
                failure: Some(AgentFailure::AccountSubscriptionLimit),
                ..failed_result("You're out of usage credits. Switch to another model to continue.")
            }),
            // Bare `claude` is Fable, held on every account until its reset.
            cooling(Some(retry_at)),
            done(),
        ]
        .into_iter();
        let mut attempts = Vec::new();
        let mut waits = Vec::new();
        let result = _run_with_transient_retries(
            &launch,
            &ProcessConfig {
                wait_for_capacity: true,
                ..auto_process()
            },
            &[],
            |config, retry| {
                assert_eq!(retry, !attempts.is_empty());
                attempts.push(config.clone());
                Ok(scripted.next().expect("scripted attempt"))
            },
            |delay| waits.push(delay),
        )
        .unwrap();
        assert_eq!(result.exit_code, 0);
        let opus = &attempts[2];
        assert_eq!(opus.agent.as_deref(), Some("claude:opus"));
        assert_eq!(opus.resume_token, Some("thread".into()));
        assert_eq!(opus.task_prompt, LIMIT_FAILOVER_PROMPT);
        assert_eq!(waits, vec![Duration::ZERO]);
    }

    #[test]
    fn with_every_account_cooling_the_step_waits_and_goes_on_in_the_same_thread() {
        // An explicit model has nowhere to fall through to.
        let launch = AgentConfig {
            agent: Some("claude:opus".to_string()),
            task_prompt: "implement the plan".to_string(),
            ..Default::default()
        };
        let retry_at = crate::store::rows::now_unix() + 3_600;
        let mut scripted = vec![
            managed_attempt(limited_result()),
            cooling(Some(retry_at)),
            cooling(Some(retry_at)),
            done(),
        ]
        .into_iter();
        let mut threads = Vec::new();
        let mut waits = Vec::new();
        let result = _run_with_transient_retries(
            &launch,
            &ProcessConfig {
                wait_for_capacity: true,
                ..auto_process()
            },
            &[],
            |config, _| {
                threads.push(config.resume_token.clone());
                Ok(scripted.next().expect("scripted attempt"))
            },
            |delay| waits.push(delay),
        )
        .unwrap();
        assert_eq!(result.exit_code, 0);
        assert_eq!(threads[3], Some("thread".into()));
        let poll = Duration::from_secs(CAPACITY_POLL_SECS as u64);
        assert_eq!(waits, vec![Duration::ZERO, poll, poll]);
    }

    #[test]
    fn a_bare_harness_tries_its_other_models_before_waiting() {
        let launch = AgentConfig {
            agent: Some("claude".to_string()),
            ..Default::default()
        };
        let retry_at = crate::store::rows::now_unix() + 3_600;
        let mut scripted = vec![
            cooling(Some(retry_at)),
            cooling(Some(retry_at + 60)),
            cooling(Some(retry_at)),
            cooling(Some(retry_at)),
            cooling(Some(retry_at)),
            done(),
        ]
        .into_iter();
        let mut agents = Vec::new();
        let mut waits = Vec::new();
        let result = _run_with_transient_retries(
            &launch,
            &ProcessConfig {
                wait_for_capacity: true,
                ..auto_process()
            },
            &[],
            |config, _| {
                agents.push(config.agent.clone().unwrap());
                Ok(scripted.next().expect("scripted attempt"))
            },
            |delay| waits.push(delay),
        )
        .unwrap();
        assert_eq!(result.exit_code, 0);
        assert_eq!(
            agents,
            [
                "claude",
                "claude:opus",
                "claude:sonnet",
                "claude:haiku",
                "claude",
                "claude:opus"
            ]
        );
        // One wait for the earliest reset once every rung is held back.
        assert_eq!(waits, vec![Duration::from_secs(CAPACITY_POLL_SECS as u64)]);
    }

    #[test]
    fn a_preference_list_moves_the_step_to_the_next_provider_with_its_task() {
        let launch = AgentConfig {
            agent: Some("claude:opus".to_string()),
            fallback_agents: vec!["codex".to_string()],
            task_prompt: "implement the plan".to_string(),
            ..Default::default()
        };
        let retry_at = crate::store::rows::now_unix() + 3_600;
        let mut scripted = vec![
            managed_attempt(limited_result()),
            cooling(Some(retry_at)),
            done(),
        ]
        .into_iter();
        let mut attempts = Vec::new();
        _run_with_transient_retries(
            &launch,
            &ProcessConfig {
                wait_for_capacity: true,
                ..auto_process()
            },
            &[],
            |config, retry| {
                attempts.push((config.clone(), retry));
                Ok(scripted.next().expect("scripted attempt"))
            },
            |_| {},
        )
        .unwrap();
        let agents: Vec<_> = attempts
            .iter()
            .map(|(c, _)| c.agent.clone().unwrap())
            .collect();
        assert_eq!(agents, ["claude:opus", "claude:opus", "codex"]);
        let (codex, retry) = &attempts[2];
        assert!(retry);
        // Claude's thread stays behind; without a Session to read, Codex gets
        // the whole task.
        assert_eq!(codex.resume_token, None);
        assert!(codex.task_prompt.starts_with(LIMIT_FAILOVER_PROMPT));
        assert!(codex.task_prompt.ends_with("implement the plan"));
    }

    #[test]
    fn nothing_that_will_free_up_ends_the_step_with_its_limit_failure() {
        let launch = AgentConfig {
            agent: Some("claude".to_string()),
            ..Default::default()
        };
        // The thread is given up first in case only it stood in the way.
        let mut scripted = vec![
            managed_attempt(limited_result()),
            cooling(None),
            cooling(None),
        ]
        .into_iter();
        let result = _run_with_transient_retries(
            &launch,
            &auto_process(),
            &[],
            |_, _| Ok(scripted.next().expect("scripted attempt")),
            |_| {},
        )
        .unwrap();
        assert_eq!(result.failure, Some(AgentFailure::AccountSubscriptionLimit));
    }

    #[test]
    fn limits_that_no_account_choice_escapes_stop_rather_than_spin() {
        let launch = AgentConfig {
            agent: Some("claude".to_string()),
            ..Default::default()
        };
        let mut attempts = 0;
        let result = _run_with_transient_retries(
            &launch,
            &auto_process(),
            &[],
            |_, _| {
                attempts += 1;
                Ok(managed_attempt(limited_result()))
            },
            |_| {},
        )
        .unwrap();
        assert_eq!(attempts, MAX_ACCOUNT_FAILOVERS + 1);
        assert_eq!(result.exit_code, 1);
    }

    #[test]
    fn only_a_graph_step_waits_for_capacity() {
        let launch = AgentConfig {
            agent: Some("claude".to_string()),
            ..Default::default()
        };
        let retry_at = crate::store::rows::now_unix() + 3_600;
        let error = _run_with_transient_retries(
            &launch,
            &auto_process(),
            &[],
            |_, _| Ok(cooling(Some(retry_at))),
            |_| panic!("only a Graph step sits waiting"),
        )
        .unwrap_err();
        assert!(error.to_string().contains("no eligible managed"));
    }

    #[test]
    fn revoked_credential_with_no_thread_fails_over_with_its_whole_task() {
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
                        AccountUnavailable::terminal("no eligible account"),
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
            Some(AgentFailure::AccountSubscriptionLimit)
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
            Some(AgentFailure::AccountSubscriptionLimit)
        ));
    }

    #[test]
    fn claude_spent_lines_read_as_subscription_limits() {
        for line in [
            "You've hit your session limit \u{b7} resets 3pm",
            "You've hit your weekly limit \u{b7} resets Wed 12:00",
            "You've reached your Fable limit. Switch to another model to continue.",
            "You're out of usage credits. /model to switch models.",
        ] {
            assert!(subscription_limit_text(line).is_some(), "{line}");
        }
    }

    #[test]
    fn a_spent_line_naming_a_model_scopes_its_limit() {
        let model = |line: &str| subscription_limit_text(line).unwrap().model;
        assert_eq!(
            model("You've reached your Fable limit. Switch to another model to continue."),
            Some("fable".to_string())
        );
        assert_eq!(
            model("You've hit your weekly Opus limit \u{b7} resets Wed 12:00"),
            Some("opus".to_string())
        );
        assert_eq!(
            model("You've hit your weekly limit \u{b7} resets Wed 12:00"),
            None
        );
        assert_eq!(
            model("You're out of usage credits. /model to switch models."),
            None
        );
        let credits = |line: &str| subscription_limit_text(line).unwrap().credits;
        assert!(credits(
            "You're out of usage credits. /model to switch models."
        ));
        assert!(!credits(
            "You've hit your weekly limit \u{b7} resets Wed 12:00"
        ));
    }

    #[test]
    fn claude_lines_naming_no_spent_window_are_not_subscription_limits() {
        for line in [
            "API Error: Request rejected (429) \u{b7} this may be a temporary capacity issue.",
            "API Error: Usage credits required for 1M context \u{b7} run /model to switch to standard context",
            "Fable 5 is currently unavailable. Learn more: https://www.anthropic.com/news/",
            "Fable 5 requires usage credits. Switch to another model to continue.",
            "Your seat type doesn't include usage credits",
        ] {
            assert_eq!(subscription_limit_text(line), None, "{line}");
        }
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

    #[test]
    fn saved_claude_model_reaches_terminal_stream_and_recording() {
        for (agent, saved, expected) in [
            ("claude", None, None),
            ("claude", Some("opus"), Some("opus")),
            ("claude:fable", Some("opus"), Some("fable")),
        ] {
            let launch = AgentConfig {
                agent: Some(agent.into()),
                claude_model: saved.map(str::to_owned),
                skip_permissions: true,
                ..default_launch()
            };
            for invocation in [
                ProviderInvocation::Terminal { session_id: None },
                ProviderInvocation::ClaudeStream { session_id: None },
            ] {
                let command = build_session_command(&launch, invocation, None).unwrap();
                let model = command
                    .args
                    .windows(2)
                    .find(|pair| pair[0] == "--model")
                    .map(|pair| pair[1].as_str());
                assert_eq!(model, expected);
            }
            let request =
                crate::session_record::AgentProcessRequest::from_prepared(&launch).unwrap();
            assert_eq!(parse_agent(&request.agent).1.as_deref(), expected);
        }
        let codex = AgentConfig {
            agent: Some("codex".into()),
            claude_model: Some("opus".into()),
            ..default_launch()
        };
        assert_eq!(codex.resolved_agent().unwrap(), "codex");
    }

    #[test]
    fn an_order_is_tried_as_written_with_a_bare_entry_on_its_saved_model() {
        let launch = AgentConfig {
            agent: Some("claude".into()),
            claude_model: Some("opus".into()),
            fallback_agents: vec!["codex".into()],
            ..default_launch()
        };
        let mut attempted = Vec::new();
        let result = _run_with_transient_retries(
            &launch,
            &ProcessConfig {
                wait_for_capacity: true,
                ..auto_process()
            },
            &[],
            |config, _| {
                let agent = config.resolved_agent().unwrap();
                attempted.push(agent.clone());
                Ok(if agent == "codex" {
                    done()
                } else {
                    cooling(Some(crate::store::rows::now_unix() + 3600))
                })
            },
            |_| panic!("Codex still has capacity"),
        )
        .unwrap();
        assert_eq!(result.exit_code, 0);
        // The list carries the model order; nothing is tried that it lacks.
        assert_eq!(attempted, ["claude:opus", "codex"]);
    }

    #[test]
    fn a_lone_bare_claude_tries_its_other_models() {
        // `agents: [claude]` leaves nothing behind the first entry.
        let launch = AgentConfig {
            agent: Some("claude".into()),
            claude_model: Some("fable".into()),
            ..default_launch()
        };
        let mut attempted = Vec::new();
        let result = _run_with_transient_retries(
            &launch,
            &ProcessConfig {
                wait_for_capacity: true,
                ..auto_process()
            },
            &[],
            |config, _| {
                let agent = config.resolved_agent().unwrap();
                attempted.push(agent.clone());
                Ok(if agent == "claude:haiku" {
                    done()
                } else {
                    cooling(Some(crate::store::rows::now_unix() + 3600))
                })
            },
            |_| panic!("Haiku still has capacity"),
        )
        .unwrap();
        assert_eq!(result.exit_code, 0);
        assert_eq!(
            attempted,
            [
                "claude:fable",
                "claude:opus",
                "claude:sonnet",
                "claude:haiku"
            ]
        );
    }

    /// November 12: compress on Codex was refused at launch with no routed
    /// account, and three Graph retries each relaunched the same agent.
    #[test]
    fn a_launch_refused_by_a_usage_limit_moves_to_the_next_agent() {
        let launch = AgentConfig {
            agent: Some("codex".into()),
            fallback_agents: vec!["claude:fable".into(), "claude:opus".into()],
            task_prompt: "compress the diff".into(),
            ..Default::default()
        };
        let refused = || AgentAttempt::Finished {
            result: failed_result("codex_error: You've hit your usage limit"),
            can_failover: false,
        };
        let mut scripted = vec![
            refused(),
            AgentAttempt::Finished {
                result: limited_result(),
                can_failover: false,
            },
            done(),
        ]
        .into_iter();
        let mut attempts = Vec::new();
        let result = _run_with_transient_retries(
            &launch,
            &ProcessConfig {
                wait_for_capacity: true,
                ..auto_process()
            },
            &[],
            |config, _| {
                attempts.push(config.clone());
                Ok(scripted.next().expect("scripted attempt"))
            },
            |_| {},
        )
        .unwrap();
        assert_eq!(result.exit_code, 0);
        let agents: Vec<_> = attempts.iter().map(|c| c.agent.clone().unwrap()).collect();
        assert_eq!(agents, ["codex", "claude:fable", "claude:opus"]);
        // Another provider starts on the whole task; another model of the
        // same provider keeps the thread.
        assert!(attempts[1].task_prompt.ends_with("compress the diff"));
        assert_eq!(attempts[1].resume_token, None);
        assert_eq!(attempts[2].resume_token, Some("thread".into()));
        assert_eq!(attempts[2].task_prompt, LIMIT_FAILOVER_PROMPT);
    }

    #[test]
    fn a_pinned_agent_refused_at_launch_fails_with_its_limit() {
        let launch = AgentConfig {
            agent: Some("codex".into()),
            ..Default::default()
        };
        let mut attempts = 0;
        let result = _run_with_transient_retries(
            &launch,
            &ProcessConfig {
                wait_for_capacity: true,
                ..auto_process()
            },
            &[],
            |_, _| {
                attempts += 1;
                Ok(AgentAttempt::Finished {
                    result: failed_result("codex_error: You've hit your usage limit"),
                    can_failover: false,
                })
            },
            |_| {},
        )
        .unwrap();
        assert_eq!(attempts, 1);
        assert_eq!(result.failure, Some(AgentFailure::AccountSubscriptionLimit));
        // What it records names the way out; with an order, what was tried.
        let advice = _limit_stop_advice("codex", &launch, &["codex".into()]);
        assert!(
            advice.contains("`lf set --global agents codex,claude`"),
            "{advice}"
        );
        let ordered = AgentConfig {
            fallback_agents: vec!["claude".into()],
            ..launch
        };
        assert_eq!(
            _limit_stop_advice("claude", &ordered, &["codex".into(), "claude".into()]),
            "Nothing is left in its order after codex, claude; add another agent to `agents`"
        );
    }
}
