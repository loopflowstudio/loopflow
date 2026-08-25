//! Agent invocation for spawning coding agent runners (Claude, Codex, Gemini, OpenCode).
//!
//! This module handles building commands and spawning subprocesses for each
//! supported coding agent. Output can be captured or streamed.

use std::collections::BTreeMap;
use std::env;
use std::fs;
use std::io::{BufRead, BufReader, Read, Write};
use std::path::{Path, PathBuf};
use std::process::{Child, Command, ExitStatus, Stdio};
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::{mpsc, Mutex, OnceLock};
use std::thread;
use std::time::{Duration, Instant};

use crate::engine::config::{default_agent, parse_agent};
use crate::engine::error::CoreError;
use crate::engine::platform::kill_process;
use crate::engine::stream::{format_event, ParseResult, StreamFormat, StreamParser};
use crate::engine::structured_reply::{render_structured_reply_guidance, StructuredReply};
use crate::provider_account::{
    resolve_provider_account_exact_blocking, resolve_recorded_provider_account_blocking,
    ProviderAccountRoute, RateLimitSignal,
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
/// must be torn down on interrupt (e.g. the wave server's discovery pointer,
/// the loop's pass process group) registers a hook here.
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

/// Run all registered interrupt cleanups. Called from the signal handler.
pub fn run_interrupt_cleanups() {
    if let Some(hooks) = INTERRUPT_HOOKS.get() {
        if let Ok(hooks) = hooks.lock() {
            for hook in hooks.iter() {
                hook();
            }
        }
    }
}

/// Result from launching a runner.
#[derive(Debug, Clone, Default)]
pub struct LaunchResult {
    pub exit_code: i32,
    pub stdout: String,
    pub stderr: String,
    /// Opaque provider continuation token observed during this launch.
    pub provider_session_id: Option<String>,
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

/// Roots that a managed delivery launch must prove writable before it starts.
///
/// Presence marks a trusted Loopflow Task boundary.
#[derive(Debug, Clone, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct AgentExecutionBoundary {
    pub writable_roots: Vec<PathBuf>,
}

pub(crate) const EXECUTION_IDENTITY_ENV: [&str; 5] = [
    crate::journal::LF_TRACE_ID_ENV,
    crate::journal::LF_PROCESS_ID_ENV,
    crate::durable::RUN_ID_ENV,
    crate::run_record::RUN_DIR_ENV,
    crate::run_record::PARENT_RUN_ID_ENV,
];

#[derive(Clone, Default)]
pub struct AgentConfig {
    /// System/context prompt content.
    pub system_prompt: String,
    /// Task prompt content sent as the turn input.
    pub task_prompt: String,
    /// Agent string (for example: "claude:opus" or "codex").
    pub agent: Option<String>,
    /// Max turn budget when supported by the harness.
    pub max_turns: Option<u32>,
    /// Opaque provider continuation token for this launch.
    pub resume_token: Option<String>,
    /// Stable managed account identity selected before durable capture.
    pub provider_account_id: Option<ProviderAccountId>,
    /// Home whose deterministic account directory resolves a recorded account.
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
    /// Engine-injected structured replies (rendered via harness prompt guidance).
    pub structured_replies: Vec<StructuredReply>,
    /// Temp file for relaying shell directives back to the invoking shell.
    /// When set, the agent subprocess gets `LOOPFLOW_DIRECTIVE_FILE` pointing
    /// to this path. The caller reads it after the agent exits and forwards
    /// safe directives (e.g. `cd`) to the real directive file.
    pub directive_relay: Option<std::path::PathBuf>,
    /// Environment scoped to this provider process and its descendants.
    pub env: BTreeMap<String, String>,
}

impl AgentConfig {
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

/// Select and pin a managed Claude/Codex account before publishing a Run.
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
    match (
        launch.provider_account_id.clone(),
        launch.provider_account_authority_home.clone(),
    ) {
        (Some(account_id), Some(home)) => resolve_recorded_provider_account_blocking(
            provider,
            launch.resume_token.clone(),
            account_id,
            home,
        ),
        (account_id, None) => resolve_provider_account_exact_blocking(
            provider,
            launch.resume_token.clone(),
            account_id,
        ),
        (None, Some(_)) => Err(crate::provider_account::ProviderAccountError::Runtime(
            "recorded account authority requires an account ID".to_string(),
        )),
    }
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
    /// Run in auto/batch mode (non-interactive).
    pub auto: bool,
    /// Stream output in real-time.
    pub stream: bool,
    /// Path to context file for system prompt loading (model-agnostic).
    /// For Claude, this uses --append-system-prompt-file.
    pub context_file: Option<std::path::PathBuf>,
    /// How to display streaming output (Raw = dump JSON, Human = formatted).
    pub stream_format: StreamFormat,
    /// Optional timeout for the subprocess.
    pub timeout: Option<Duration>,
    /// Durable local capture for this harness launch.
    pub capture: Option<AgentCapture>,
}

#[derive(Debug, Clone)]
pub struct AgentCapture(crate::run_record::CaptureHandle);

impl AgentCapture {
    fn record_raw(&self, stream: &str, line: &str) {
        self.0.record_raw(stream, line);
    }

    fn record_stream_event(&self, event: &crate::engine::stream::StreamEvent) {
        self.0.record_stream_event(event);
    }

    fn record_conversation(&self, event: crate::chat::types::ConversationEvent) {
        self.0.record_conversation(event);
    }

    fn fail_and_begin_attempt(
        &self,
        provider: String,
        model: Option<String>,
        account_id: Option<ProviderAccountId>,
    ) {
        self.0.fail_and_begin_attempt(provider, model, account_id);
    }

    fn set_provider_session_id(&self, session_id: Option<String>) {
        self.0.set_provider_session_id(session_id);
    }

    fn finish(&self, outcome: &str) -> crate::store::StoreResult<()> {
        self.0.finish(outcome)
    }
}

impl From<crate::run_record::CaptureHandle> for AgentCapture {
    fn from(capture: crate::run_record::CaptureHandle) -> Self {
        Self(capture)
    }
}

/// Agent capability flags.
#[derive(Debug, Clone, Default)]
pub struct AgentCapabilities {
    /// Enable Chrome integration (Claude only).
    pub chrome: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
enum CodexSandboxMode {
    ReadOnly,
    WorkspaceWrite,
    DangerFullAccess,
}

impl CodexSandboxMode {
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

pub fn codex_permission_args(
    cwd: Option<&Path>,
    auto: bool,
    skip_permissions: bool,
) -> Vec<String> {
    codex_permission_args_for_scope(cwd, auto, skip_permissions, AgentWriteScope::Configured)
}

fn codex_permission_args_for_scope(
    cwd: Option<&Path>,
    auto: bool,
    skip_permissions: bool,
    write_scope: AgentWriteScope,
) -> Vec<String> {
    if write_scope == AgentWriteScope::Worktree {
        if skip_permissions {
            return vec!["--dangerously-bypass-approvals-and-sandbox".to_string()];
        }
        let mut args = vec!["--sandbox".to_string(), "workspace-write".to_string()];
        if auto {
            args.push("--ask-for-approval".to_string());
            args.push("never".to_string());
        }
        return args;
    }
    if skip_permissions {
        return vec!["--dangerously-bypass-approvals-and-sandbox".to_string()];
    }

    let config = read_codex_permission_config(cwd);
    codex_permission_args_for_config(config, auto, skip_permissions)
}

fn codex_permission_args_for_config(
    config: CodexPermissionConfig,
    auto: bool,
    skip_permissions: bool,
) -> Vec<String> {
    if skip_permissions {
        return vec!["--dangerously-bypass-approvals-and-sandbox".to_string()];
    }

    let mut args = Vec::new();
    if config.sandbox < Some(CodexSandboxMode::WorkspaceWrite) {
        if config.sandbox.is_some() {
            eprintln!(
                "warning: Codex config sandbox_mode is less permissive than Loopflow's workspace-write default; launching with --sandbox workspace-write"
            );
        }
        args.push("--sandbox".to_string());
        args.push("workspace-write".to_string());
    }

    if auto && config.approval < Some(CodexApprovalPolicy::Never) {
        if config.approval.is_some() {
            eprintln!(
                "warning: Codex config approval_policy is less permissive than Loopflow's non-interactive default; launching with --ask-for-approval never"
            );
        }
        args.push("--ask-for-approval".to_string());
        args.push("never".to_string());
    }

    args
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

/// Common Claude CLI arguments shared across engine and session paths.
///
/// Both `build_claude_command` (engine one-shot) and the session harness
/// `build_args` construct a `ClaudeArgs` and call `to_args()`, then add
/// their mode-specific flags on top (`--print` for engine, `-p` for harness).
#[derive(Debug, Clone, Default, PartialEq)]
pub struct ClaudeArgs {
    /// Model variant (already resolved, no "claude:" prefix).
    pub model: Option<String>,
    /// System prompt text for `--append-system-prompt`.
    pub system_prompt: Option<String>,
    /// System prompt file for `--append-system-prompt-file` (takes precedence over text).
    pub system_prompt_file: Option<std::path::PathBuf>,
    /// Additional directories the harness may access.
    pub add_dirs: Vec<PathBuf>,
    /// Skip permission prompts.
    pub skip_permissions: bool,
    /// Enforce the managed Task worktree boundary.
    pub worktree_isolation: bool,
    /// Max turn budget.
    pub max_turns: Option<u32>,
    /// Enable streaming output (`--output-format stream-json --verbose`).
    pub stream: bool,
    /// Enable Chrome integration.
    pub chrome: bool,
    /// Resume an existing Claude Code session.
    pub resume_id: Option<String>,
}

impl ClaudeArgs {
    /// Resolve a model string to a Claude `--model` variant.
    ///
    /// Strips the `claude:` prefix if present, applies defaults (bare `"claude"` → `"opus"`),
    /// and passes through non-claude model strings unchanged.
    pub fn resolve_model(model: &str) -> Option<String> {
        let (harness, model_variant) = parse_agent(model);
        if harness == "claude" {
            model_variant
        } else {
            Some(model.to_string())
        }
    }

    /// Build `Vec<String>` of Claude CLI args (without program name or prompt content).
    pub fn to_args(&self) -> Vec<String> {
        let mut args = Vec::new();

        if self.chrome {
            args.push("--chrome".to_string());
        }

        if let Some(ref model) = self.model {
            args.push("--model".to_string());
            args.push(model.clone());
        }

        if let Some(ref file) = self.system_prompt_file {
            args.push("--append-system-prompt-file".to_string());
            args.push(file.to_string_lossy().to_string());
        } else if let Some(ref text) = self.system_prompt {
            if !text.trim().is_empty() {
                args.push("--append-system-prompt".to_string());
                args.push(text.clone());
            }
        }

        if !self.add_dirs.is_empty() {
            args.push("--add-dir".to_string());
            args.extend(
                self.add_dirs
                    .iter()
                    .map(|dir| dir.to_string_lossy().to_string()),
            );
        }

        if self.skip_permissions {
            args.push("--dangerously-skip-permissions".to_string());
        }

        if self.worktree_isolation {
            args.push("--permission-mode".to_string());
            args.push("acceptEdits".to_string());
            args.push("--setting-sources".to_string());
            args.push(String::new());
            args.push("--settings".to_string());
            args.push(
                serde_json::json!({
                    "sandbox": {
                        "enabled": true,
                        "failIfUnavailable": true,
                        "allowUnsandboxedCommands": false
                    }
                })
                .to_string(),
            );
        }

        if let Some(max_turns) = self.max_turns {
            args.push("--max-turns".to_string());
            args.push(max_turns.to_string());
        }

        if self.stream {
            args.push("--output-format".to_string());
            args.push("stream-json".to_string());
            args.push("--verbose".to_string());
        }

        if let Some(ref id) = self.resume_id {
            args.push("--resume".to_string());
            args.push(id.clone());
        }

        args
    }
}

/// Build CLI args for a Claude session turn (`claude -p ...`).
///
/// This is shared by session harnesses so session turn invocation stays aligned
/// with engine-owned Claude argument conventions.
pub fn build_claude_session_turn_args(
    content: &str,
    config: &AgentConfig,
    resume_id: Option<&str>,
) -> Vec<String> {
    let mut args = vec!["-p".to_string(), content.to_string()];
    let claude_args = ClaudeArgs {
        model: config.agent.as_deref().and_then(ClaudeArgs::resolve_model),
        system_prompt: Some(system_prompt_with_structured_replies(config)),
        system_prompt_file: None,
        add_dirs: provider_writable_roots(config),
        skip_permissions: config.execution_boundary.is_some()
            || (config.write_scope == AgentWriteScope::Configured
                && claude_skip_permissions(config.cwd.as_deref(), true, config.skip_permissions)),
        worktree_isolation: config.write_scope == AgentWriteScope::Worktree
            && config.execution_boundary.is_none(),
        max_turns: config.max_turns,
        stream: true,
        chrome: false,
        resume_id: resume_id.map(str::to_string),
    };
    args.extend(claude_args.to_args());
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
    /// The selected managed account exhausted a subscription window.
    #[error("account subscription limit")]
    AccountSubscriptionLimit { resets_at: Option<i64> },
    /// The selected managed account's credential was explicitly revoked.
    #[error("account credential invalidated")]
    AccountCredentialInvalidated,
}

/// Build `thread/start` params for Codex app-server sessions.
///
/// Mirrors model/cwd mapping used by one-shot Codex launches so session and
/// non-session paths stay in sync.
pub fn build_codex_thread_start_params(
    launch: &AgentConfig,
) -> serde_json::Map<String, serde_json::Value> {
    let mut params = serde_json::Map::new();

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

    if launch.write_scope == AgentWriteScope::Worktree {
        params.insert(
            "approvalPolicy".to_string(),
            serde_json::Value::String("never".to_string()),
        );
        params.insert(
            "sandbox".to_string(),
            serde_json::Value::String(
                if launch.execution_boundary.is_some() {
                    "danger-full-access"
                } else {
                    "workspace-write"
                }
                .to_string(),
            ),
        );
    } else if launch.skip_permissions {
        params.insert(
            "approvalPolicy".to_string(),
            serde_json::Value::String("never".to_string()),
        );
        params.insert(
            "sandbox".to_string(),
            serde_json::Value::String("danger-full-access".to_string()),
        );
    } else {
        let config = read_codex_permission_config(launch.cwd.as_deref());
        if config.sandbox < Some(CodexSandboxMode::WorkspaceWrite) {
            params.insert(
                "sandbox".to_string(),
                serde_json::Value::String("workspace-write".to_string()),
            );
        }
        if config.approval < Some(CodexApprovalPolicy::Never) {
            params.insert(
                "approvalPolicy".to_string(),
                serde_json::Value::String("never".to_string()),
            );
        }
    }

    params
}

/// Extra workspace roots agents need for Git worktree metadata.
pub fn workspace_add_dirs(cwd: &Path) -> Vec<PathBuf> {
    let Ok(main_repo) = crate::engine::worktrees::main_repo_root(cwd) else {
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

/// Build Claude CLI command.
pub fn build_claude_command(
    launch: &AgentConfig,
    process: &ProcessConfig,
    capabilities: &AgentCapabilities,
    model_variant: Option<&str>,
) -> Vec<String> {
    let mut cmd = vec!["claude".to_string()];

    let claude_args = ClaudeArgs {
        model: model_variant.map(str::to_string),
        system_prompt: Some(system_prompt_with_structured_replies(launch)),
        system_prompt_file: process.context_file.clone(),
        add_dirs: provider_writable_roots(launch),
        skip_permissions: launch.execution_boundary.is_some()
            || (launch.write_scope == AgentWriteScope::Configured
                && claude_skip_permissions(
                    launch.cwd.as_deref(),
                    process.auto,
                    launch.skip_permissions,
                )),
        worktree_isolation: launch.write_scope == AgentWriteScope::Worktree
            && launch.execution_boundary.is_none(),
        max_turns: launch.max_turns,
        stream: process.auto && process.stream,
        chrome: capabilities.chrome,
        resume_id: launch.resume_token.clone(),
    };
    cmd.extend(claude_args.to_args());

    if process.auto {
        cmd.push("--print".to_string());
    }

    cmd
}

/// Build Codex CLI command.
pub fn build_codex_command(
    launch: &AgentConfig,
    process: &ProcessConfig,
    model_variant: Option<&str>,
) -> Vec<String> {
    // `codex exec` for batch/auto mode, `codex` for interactive
    let mut cmd = if process.auto {
        let mut cmd = vec!["codex".to_string(), "exec".to_string()];
        if launch.resume_token.is_some() {
            cmd.push("resume".to_string());
        }
        cmd
    } else {
        vec!["codex".to_string()]
    };

    cmd.push("-c".to_string());
    cmd.push(format!("service_tier=\"{CODEX_DEFAULT_SERVICE_TIER}\""));

    // Load context via model_instructions_file (replaces AGENTS.md)
    if let Some(ref context_file) = process.context_file {
        cmd.push("-c".to_string());
        cmd.push(format!(
            "model_instructions_file=\"{}\"",
            context_file.display()
        ));
    }

    if let Some(variant) = model_variant {
        cmd.push("-c".to_string());
        cmd.push(format!("model=\"{variant}\""));
    }

    if let Some(ref cwd) = launch.cwd {
        cmd.push("-C".to_string());
        cmd.push(cwd.to_string_lossy().to_string());
    }

    if (launch.write_scope == AgentWriteScope::Worktree && launch.execution_boundary.is_none())
        || !launch.skip_permissions
    {
        for dir in provider_writable_roots(launch) {
            cmd.push("--add-dir".to_string());
            cmd.push(dir.to_string_lossy().to_string());
        }
    }

    if process.stream {
        cmd.push("--json".to_string());
    }

    cmd.extend(codex_permission_args_for_scope(
        launch.cwd.as_deref(),
        process.auto,
        launch.skip_permissions,
        launch.write_scope,
    ));

    if process.auto {
        if let Some(resume_token) = &launch.resume_token {
            cmd.push(resume_token.clone());
        }
    }

    cmd
}

/// Build Gemini CLI command.
pub fn build_gemini_command(
    launch: &AgentConfig,
    process: &ProcessConfig,
    model_variant: Option<&str>,
) -> Vec<String> {
    let mut cmd = vec!["gemini".to_string()];

    if let Some(variant) = model_variant {
        cmd.push("-m".to_string());
        cmd.push(variant.to_string());
    }

    if process.stream {
        cmd.push("--output-format".to_string());
        cmd.push("stream-json".to_string());
    }

    if launch.skip_permissions {
        cmd.push("--yolo".to_string());
    }

    cmd
}

/// Build OpenCode CLI command.
pub fn build_opencode_command(process: &ProcessConfig, model_variant: Option<&str>) -> Vec<String> {
    // `opencode run` for batch/auto mode, `opencode` for interactive TUI
    let mut cmd = if process.auto {
        vec!["opencode".to_string(), "run".to_string()]
    } else {
        vec!["opencode".to_string()]
    };

    if let Some(variant) = model_variant {
        cmd.push("--model".to_string());
        cmd.push(variant.to_string());
    }

    if process.auto && process.stream {
        cmd.push("--format".to_string());
        cmd.push("json".to_string());
    }

    cmd
}

/// Build the `OPENCODE_CONFIG_CONTENT` env var JSON string.
///
/// Returns `None` when no config overrides are needed (interactive mode, no context).
pub fn build_opencode_env(process: &ProcessConfig) -> Option<String> {
    build_opencode_env_for_scope(process, AgentWriteScope::Configured)
}

fn build_opencode_env_for_scope(
    process: &ProcessConfig,
    write_scope: AgentWriteScope,
) -> Option<String> {
    let mut oc_config = serde_json::Map::new();
    if write_scope == AgentWriteScope::Worktree {
        oc_config.insert(
            "permission".into(),
            serde_json::json!({"*": "allow", "external_directory": "deny"}),
        );
    } else if process.auto {
        oc_config.insert(
            "permission".into(),
            serde_json::Value::String("allow".into()),
        );
    }
    if let Some(ref context_file) = process.context_file {
        oc_config.insert(
            "instructions".into(),
            serde_json::json!([context_file.to_string_lossy()]),
        );
    }
    if oc_config.is_empty() {
        None
    } else {
        Some(serde_json::Value::Object(oc_config).to_string())
    }
}

/// Highest-priority OpenCode configuration for a managed Task provider.
pub fn opencode_worktree_config() -> String {
    build_opencode_env_for_scope(&ProcessConfig::default(), AgentWriteScope::Worktree)
        .expect("worktree scope always produces OpenCode configuration")
}

pub fn build_agent_env(launch: &AgentConfig, process: &ProcessConfig) -> BTreeMap<String, String> {
    let mut env = launch.env.clone();
    let agent = launch.agent();
    let (harness, _) = parse_agent(agent);
    match harness.as_str() {
        "gemini" => {
            if let Some(ref context_file) = process.context_file {
                env.insert(
                    "GEMINI_SYSTEM_MD".to_string(),
                    context_file.to_string_lossy().to_string(),
                );
            }
        }
        "opencode" => {
            if let Some(env_val) = build_opencode_env_for_scope(process, launch.write_scope) {
                env.insert("OPENCODE_CONFIG_CONTENT".to_string(), env_val);
            }
        }
        _ => {}
    }

    env
}

/// Apply harness-specific environment variables to a command.
fn apply_harness_env(
    harness: &str,
    cmd: &mut Command,
    launch: &AgentConfig,
    process: &ProcessConfig,
) {
    match harness {
        "gemini" => {
            if let Some(ref context_file) = process.context_file {
                cmd.env(
                    "GEMINI_SYSTEM_MD",
                    context_file.to_string_lossy().to_string(),
                );
            }
        }
        "opencode" => {
            if let Some(env_val) = build_opencode_env_for_scope(process, launch.write_scope) {
                cmd.env("OPENCODE_CONFIG_CONTENT", env_val);
            }
        }
        _ => {}
    }
}

/// Build command for any model.
pub fn build_model_command(
    launch: &AgentConfig,
    process: &ProcessConfig,
    capabilities: &AgentCapabilities,
) -> Vec<String> {
    let agent = launch.agent();
    let (harness, model_variant) = parse_agent(agent);
    let model_variant = model_variant.as_deref();
    match harness.as_str() {
        "codex" => build_codex_command(launch, process, model_variant),
        "gemini" => build_gemini_command(launch, process, model_variant),
        "opencode" => build_opencode_command(process, model_variant),
        "claude" => build_claude_command(launch, process, capabilities, model_variant),
        // Unknown harness: fall back to Claude with the full model string as variant.
        _ => build_claude_command(launch, process, capabilities, Some(agent)),
    }
}

/// Build a full CLI command (including prompt) for a model.
pub fn build_agent_command(
    launch: &AgentConfig,
    process: &ProcessConfig,
    capabilities: &AgentCapabilities,
) -> Vec<String> {
    let mut cmd = build_model_command(launch, process, capabilities);
    if !launch.task_prompt.is_empty() {
        cmd.push(launch.task_prompt.clone());
    }
    cmd
}

/// Launch an agent subprocess and wait for it to exit.
pub fn launch_agent(
    launch: &AgentConfig,
    process: &ProcessConfig,
    capabilities: &AgentCapabilities,
) -> Result<LaunchResult, CoreError> {
    let mut launch = launch.clone();
    pin_provider_account_id_blocking(&mut launch)?;
    let (harness, model) = parse_agent(launch.agent());
    let implicit_capture = if process.capture.is_none() {
        Some(_begin_implicit_capture(
            &launch,
            process,
            capabilities,
            &harness,
            model,
        )?)
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
        launch.env.extend(capture.0.environment());
        capture.0.mark_spawn_requested();
    }
    let result = _launch_with_transient_retries(
        &launch,
        &process,
        &TRANSIENT_RETRY_DELAYS,
        |attempt, retry| _launch_agent_once(attempt, &process, capabilities, retry),
        thread::sleep,
    );
    if let Some(capture) = implicit_capture {
        let outcome = match &result {
            Ok(result) if result.exit_code == 0 => "completed",
            Ok(_) | Err(_) => "failed",
        };
        capture
            .finish(outcome)
            .map_err(|error| CoreError::ExecutionFailed(error.to_string()))?;
    }
    result
}

#[derive(Debug)]
enum AgentAttempt {
    Finished {
        result: LaunchResult,
        can_failover: bool,
    },
    AccountUnavailable(CoreError),
}

fn _launch_with_transient_retries(
    launch: &AgentConfig,
    process: &ProcessConfig,
    retry_delays: &[Duration],
    mut run: impl FnMut(&AgentConfig, bool) -> Result<AgentAttempt, CoreError>,
    mut wait: impl FnMut(Duration),
) -> Result<LaunchResult, CoreError> {
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
        let (harness, _) = parse_agent(attempt_config.agent());
        let failure = if process.auto {
            result
                .failure
                .or_else(|| _classify_agent_failure(&harness, &result))
        } else {
            None
        };
        result.failure = failure;
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
                attempt_config.task_prompt = format!(
                    "{LIMIT_FAILOVER_PROMPT}\n\nOriginal task:\n\n{}",
                    launch.task_prompt
                );
                (Duration::ZERO, true)
            }
            AgentFailure::AccountCredentialInvalidated => {
                account_failure = Some(result.clone());
                attempt_config.provider_account_id = None;
                attempt_config.task_prompt = format!(
                    "{CREDENTIAL_FAILOVER_PROMPT}\n\nOriginal task:\n\n{}",
                    launch.task_prompt
                );
                (Duration::ZERO, true)
            }
            _ => {
                account_failure = None;
                let resume_token = _provider_resume_token(&result).or(launch.resume_token.clone());
                if matches!(harness.as_str(), "claude" | "codex") {
                    if let Some(resume_token) = resume_token {
                        attempt_config.resume_token = Some(resume_token);
                        attempt_config.task_prompt = RETRY_PROMPT.to_string();
                    }
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

fn _classify_agent_failure(harness: &str, result: &LaunchResult) -> Option<AgentFailure> {
    if result.exit_code == 0 {
        return None;
    }
    if _find_provider_error(result, credential_invalidated_failure).is_some() {
        return Some(AgentFailure::AccountCredentialInvalidated);
    }
    if let Some(signal) = _account_limit_signal(harness, result).filter(|signal| signal.limited) {
        return Some(AgentFailure::AccountSubscriptionLimit {
            resets_at: signal.resets_at,
        });
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

fn _find_provider_error<T>(result: &LaunchResult, classify: fn(&str) -> Option<T>) -> Option<T> {
    for line in result.stdout.lines() {
        let Ok(value) = serde_json::from_str::<serde_json::Value>(line) else {
            continue;
        };
        if !_is_provider_error(&value) {
            continue;
        }
        if let Some(failure) = ["message", "error", "result"]
            .into_iter()
            .filter_map(|field| value.get(field))
            .find_map(|value| _classify_provider_error_value(value, classify))
        {
            return Some(failure);
        }
    }
    result.stderr.lines().find_map(classify)
}

fn _is_provider_error(value: &serde_json::Value) -> bool {
    match value
        .get("type")
        .and_then(serde_json::Value::as_str)
        .unwrap_or_default()
    {
        "turn.failed" | "error" => true,
        "result" => {
            value
                .get("is_error")
                .and_then(serde_json::Value::as_bool)
                .unwrap_or(false)
                || value
                    .get("subtype")
                    .and_then(serde_json::Value::as_str)
                    .is_some_and(|subtype| subtype == "failed" || subtype.contains("error"))
        }
        _ => false,
    }
}

fn _classify_provider_error_value<T>(
    value: &serde_json::Value,
    classify: fn(&str) -> Option<T>,
) -> Option<T> {
    match value {
        serde_json::Value::String(text) => classify(text),
        serde_json::Value::Object(fields) => fields
            .values()
            .find_map(|value| _classify_provider_error_value(value, classify)),
        serde_json::Value::Array(values) => values
            .iter()
            .find_map(|value| _classify_provider_error_value(value, classify)),
        _ => None,
    }
}

pub(crate) fn classify_retryable_agent_failure(text: &str) -> Option<AgentFailure> {
    let text = text.to_ascii_lowercase();
    if text.contains("at capacity") || text.contains("capacity temporarily unavailable") {
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

fn _account_limit_signal(harness: &str, result: &LaunchResult) -> Option<RateLimitSignal> {
    let mut signal = result
        .stdout
        .lines()
        .filter_map(|line| match harness {
            "claude" => crate::harness::claude_rate_limit_signal(line),
            "codex" => serde_json::from_str::<serde_json::Value>(line)
                .ok()
                .and_then(|value| crate::harness::codex_rate_limit_signal(&value)),
            _ => None,
        })
        .max_by_key(|signal| (signal.limited, signal.utilization_percent.unwrap_or(0)));

    if result.exit_code != 0 && _find_provider_error(result, _classify_subscription_limit).is_some()
    {
        signal = Some(RateLimitSignal {
            utilization_percent: Some(100),
            resets_at: signal.as_ref().and_then(|signal| signal.resets_at),
            limited: true,
            reason: "subscription usage limit".to_string(),
            windows: signal.map(|signal| signal.windows).unwrap_or_default(),
        });
    }
    signal
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

fn _provider_resume_token(result: &LaunchResult) -> Option<String> {
    result.provider_session_id.clone().or_else(|| {
        result.stdout.lines().find_map(|line| {
            let value: serde_json::Value = serde_json::from_str(line).ok()?;
            let session_id = [
                value.get("session_id"),
                value.get("sessionId"),
                value.get("thread_id"),
                value.pointer("/stream_event/event/session_id"),
                value.pointer("/params/thread/id"),
            ]
            .into_iter()
            .flatten()
            .find_map(|value| value.as_str().filter(|value| !value.is_empty()))
            .map(str::to_string);
            session_id
        })
    })
}

fn _begin_implicit_capture(
    launch: &AgentConfig,
    process: &ProcessConfig,
    capabilities: &AgentCapabilities,
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
    let spec = crate::run_record::RunSpec {
        harness: harness.to_string(),
        model,
        surface: if process.auto { "headless" } else { "tui" }.to_string(),
        cwd: cwd.clone(),
        repo: Some(cwd.clone()),
        worktree: Some(cwd),
        skill: None,
        subjects: Vec::new(),
    };
    let capture = if process.auto {
        crate::run_record::CaptureHandle::begin_with_launch(
            spec,
            crate::run_record::RunLaunchRequest::from_prepared(launch, capabilities),
        )
    } else {
        crate::run_record::CaptureHandle::begin(spec)
    };
    capture
        .map(|capture| {
            capture.record_input("initial", &launch.task_prompt);
            capture.into()
        })
        .map_err(|error| {
            CoreError::ExecutionFailed(format!(
                "failed to publish Run manifest before agent launch: {error}"
            ))
        })
}

fn _launch_codex_harness_once(
    launch: &AgentConfig,
    process: &ProcessConfig,
    model: Option<String>,
    retry: bool,
) -> Result<AgentAttempt, CoreError> {
    if tokio::runtime::Handle::try_current().is_ok() {
        let launch = launch.clone();
        let process = process.clone();
        return std::thread::Builder::new()
            .name("lf-codex-harness".to_string())
            .spawn(move || _launch_codex_harness_once(&launch, &process, model, retry))
            .map_err(|error| CoreError::ExecutionFailed(error.to_string()))?
            .join()
            .map_err(|_| {
                CoreError::ExecutionFailed("Codex harness thread panicked".to_string())
            })?;
    }

    use crate::chat::types::{ConversationEvent, Lifecycle};
    use crate::harness::ApprovalPolicy;

    let capture = process.capture.as_ref();
    let account_route = match resolve_account_route_blocking(Provider::Codex, launch) {
        Ok(route) => route,
        Err(error) => {
            return Ok(AgentAttempt::AccountUnavailable(
                CoreError::ExecutionFailed(format!("failed to select provider account: {error}")),
            ));
        }
    };
    if retry {
        if let Some(capture) = capture {
            capture.fail_and_begin_attempt(
                "codex".to_string(),
                model,
                account_route
                    .as_ref()
                    .map(|route| route.account_id().clone()),
            );
        }
    }

    let mut config = launch.clone();
    let prompt = std::mem::take(&mut config.task_prompt);
    let writer_worktree = config.cwd.clone().or_else(|| std::env::current_dir().ok());
    let writer_guard = writer_worktree
        .as_deref()
        .map(|cwd| crate::ops::git_operation::prepare_agent_writer(cwd, &config.env))
        .transpose()
        .map_err(|error| CoreError::ExecutionFailed(error.to_string()))?
        .flatten();
    if let Some(guard) = writer_guard.as_ref() {
        config
            .env
            .entry(crate::ops::git_operation::LF_WORKTREE_WRITER_ID_ENV.to_string())
            .or_insert_with(|| guard.writer_id().to_string());
    }

    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .map_err(|error| CoreError::ExecutionFailed(error.to_string()))?;
    let result = runtime.block_on(async {
        let (event_tx, mut event_rx) = tokio::sync::mpsc::unbounded_channel();
        let (raw_tx, mut raw_rx) = tokio::sync::mpsc::unbounded_channel();
        let mut harness = crate::harness::default_create_harness(
            "codex",
            ApprovalPolicy::AutoApprove,
            event_tx,
        )
        .map_err(|error| CoreError::ExecutionFailed(error.to_string()))?;
        harness.set_provider_account_id(
            account_route
                .as_ref()
                .map(|route| route.account_id().clone()),
        );
        harness.set_provider_session_id(launch.resume_token.clone());
        if capture.is_some() {
            harness.set_raw_provider_sender(Some(raw_tx));
        }
        harness
            .start(&config)
            .await
            .map_err(|error| CoreError::ExecutionFailed(error.to_string()))?;
        let provider_session_id = harness.provider_session_id();
        if let Some(capture) = capture {
            capture.set_provider_session_id(provider_session_id.clone());
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
            while exit_code.is_none() {
                tokio::select! {
                    raw = raw_rx.recv(), if capture.is_some() => {
                        if let Some(raw) = raw {
                            if let Some(capture) = capture {
                                capture.record_raw(raw.stream, &raw.line);
                            }
                            if process.stream && process.stream_format == StreamFormat::Raw {
                                println!("{}", raw.line);
                            }
                        }
                    }
                    event = event_rx.recv() => {
                        let Some(event) = event else {
                            stderr.push_str("codex event stream closed\n");
                            exit_code = Some(1);
                            continue;
                        };
                        if let Some(capture) = capture {
                            capture.record_conversation(event.clone());
                        }
                        match event {
                            ConversationEvent::TextDelta { content, .. } => {
                                stdout.push_str(&content);
                                if process.stream && process.stream_format != StreamFormat::Raw {
                                    print!("{content}");
                                    let _ = std::io::stdout().flush();
                                }
                            }
                            ConversationEvent::Error { code, message, .. } => {
                                stderr.push_str(&format!("{code}: {message}\n"));
                                if matches!(code.as_str(), "codex_disconnected" | "provider_rate_limited") {
                                    exit_code = Some(1);
                                }
                            }
                            ConversationEvent::TurnCompleted { status, .. } => {
                                exit_code = Some(if status == Lifecycle::Completed { 0 } else { 1 });
                            }
                            _ => {}
                        }
                    }
                }
            }
            while let Ok(raw) = raw_rx.try_recv() {
                if let Some(capture) = capture {
                    capture.record_raw(raw.stream, &raw.line);
                }
                if process.stream && process.stream_format == StreamFormat::Raw {
                    println!("{}", raw.line);
                }
            }
            if process.stream
                && process.stream_format != StreamFormat::Raw
                && !stdout.ends_with('\n')
            {
                println!();
            }
            Ok(LaunchResult {
                exit_code: exit_code.expect("event loop stops with an exit code"),
                stdout,
                stderr,
                provider_session_id,
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
        let _ = harness.stop().await;
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
                .record_launch_blocking(result.provider_session_id.clone(), None)
                .map_err(|error| CoreError::ExecutionFailed(error.to_string()))?;
        }
    }

    result
}

fn _launch_agent_once(
    launch: &AgentConfig,
    process: &ProcessConfig,
    capabilities: &AgentCapabilities,
    retry: bool,
) -> Result<AgentAttempt, CoreError> {
    let start = Instant::now();
    let (harness, model) = parse_agent(launch.agent());
    if harness == "codex" && process.auto {
        return _launch_codex_harness_once(launch, process, model, retry);
    }
    let cmd_args = build_model_command(launch, process, capabilities);
    if cmd_args.is_empty() {
        return Err(CoreError::ExecutionFailed("Empty command".to_string()));
    }

    let program = &cmd_args[0];
    let args = &cmd_args[1..];
    tracing::debug!(
        elapsed_ms = start.elapsed().as_millis(),
        "launch_agent prepared command"
    );
    tracing::debug!(program, args = ?args, "spawning agent command");

    let mut cmd = Command::new(program);
    cmd.args(args);
    if !launch.task_prompt.is_empty() {
        cmd.arg(&launch.task_prompt);
    }

    if let Some(ref cwd) = launch.cwd {
        cmd.current_dir(cwd);
    }

    let mut scoped_env = launch.env.clone();
    let writer_worktree = launch.cwd.clone().or_else(|| std::env::current_dir().ok());
    let writer_guard = writer_worktree
        .as_deref()
        .map(|cwd| crate::ops::git_operation::prepare_agent_writer(cwd, &scoped_env))
        .transpose()
        .map_err(|error| CoreError::ExecutionFailed(error.to_string()))?
        .flatten();
    if let Some(guard) = writer_guard.as_ref() {
        scoped_env
            .entry(crate::ops::git_operation::LF_WORKTREE_WRITER_ID_ENV.to_string())
            .or_insert_with(|| guard.writer_id().to_string());
    }
    for name in EXECUTION_IDENTITY_ENV {
        cmd.env_remove(name);
    }
    cmd.envs(&scoped_env);

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
    let managed_provider = match harness.as_str() {
        "claude" => Some(Provider::Claude),
        "codex" => Some(Provider::Codex),
        _ => None,
    };
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
    if account_route
        .as_ref()
        .is_some_and(crate::provider_account::ProviderAccountRoute::uses_native_home)
        && managed_provider == Some(Provider::Codex)
    {
        cmd.args(["-c", "cli_auth_credentials_store=\"file\""]);
    }
    if let Some(route) = &account_route {
        tracing::info!(
            provider = %harness,
            account_id = %route.account_id(),
            "selected provider account"
        );
        route.apply(&mut cmd);
    }
    if retry {
        if let Some(capture) = &process.capture {
            capture.fail_and_begin_attempt(
                harness.clone(),
                model.clone(),
                account_route
                    .as_ref()
                    .map(|route| route.account_id().clone()),
            );
        }
    }
    apply_harness_env(&harness, &mut cmd, launch, process);

    let capture = process.capture.as_ref();

    let result = if process.auto && process.stream {
        // Stream mode: capture stdout line by line
        launch_streaming(&mut cmd, process.stream_format, process.timeout, capture)
    } else if process.auto {
        // Batch mode: capture all output
        launch_batch(&mut cmd, process.timeout, capture)
    } else {
        // Interactive mode: inherit stdio
        launch_interactive(&mut cmd, process.timeout)
    };
    let mut can_failover =
        account_route.is_some() && launch.provider_account_authority_home.is_none();
    if let (Some(route), Ok(result)) = (&account_route, &result) {
        let credential_invalidated =
            _find_provider_error(result, credential_invalidated_failure).is_some();
        if credential_invalidated {
            if let Err(error) = route.record_credential_invalidated_blocking("token_invalidated") {
                tracing::warn!(%error, "failed to record invalidated provider credential");
                can_failover = false;
            }
        } else {
            let resume_token = _provider_resume_token(result);
            let signal = _account_limit_signal(&harness, result);
            let limited = signal.as_ref().is_some_and(|signal| signal.limited);
            if let Err(error) = route.record_launch_blocking(resume_token, signal) {
                tracing::warn!(%error, "failed to record provider account launch");
                if limited {
                    can_failover = false;
                }
            }
        }
    }
    result.map(|result| AgentAttempt::Finished {
        result,
        can_failover,
    })
}

fn launch_batch(
    cmd: &mut Command,
    timeout: Option<Duration>,
    capture: Option<&AgentCapture>,
) -> Result<LaunchResult, CoreError> {
    let start = Instant::now();
    cmd.stdin(Stdio::null());
    cmd.stdout(Stdio::piped());
    cmd.stderr(Stdio::piped());
    let mut child = cmd.spawn()?;
    let _pid_guard = ChildPidGuard::new(child.id());

    let stdout = child
        .stdout
        .take()
        .ok_or_else(|| CoreError::ExecutionFailed("Failed to capture stdout".to_string()))?;
    let stderr = child
        .stderr
        .take()
        .ok_or_else(|| CoreError::ExecutionFailed("Failed to capture stderr".to_string()))?;

    let stdout_handle = thread::spawn(move || -> std::io::Result<Vec<u8>> {
        let mut reader = BufReader::new(stdout);
        let mut bytes = Vec::new();
        reader.read_to_end(&mut bytes)?;
        Ok(bytes)
    });
    let stderr_handle = thread::spawn(move || -> std::io::Result<Vec<u8>> {
        let mut reader = BufReader::new(stderr);
        let mut bytes = Vec::new();
        reader.read_to_end(&mut bytes)?;
        Ok(bytes)
    });

    let (status, timed_out) = wait_for_exit(&mut child, timeout)?;
    tracing::debug!(
        elapsed_ms = start.elapsed().as_millis(),
        "agent batch completed"
    );

    let stdout_bytes = stdout_handle
        .join()
        .map_err(|_| CoreError::ExecutionFailed("stdout reader thread panicked".to_string()))?
        .map_err(|err| CoreError::ExecutionFailed(err.to_string()))?;
    let stderr_bytes = stderr_handle
        .join()
        .map_err(|_| CoreError::ExecutionFailed("stderr reader thread panicked".to_string()))?
        .map_err(|err| CoreError::ExecutionFailed(err.to_string()))?;

    if let Some(capture) = capture {
        let mut parser = StreamParser::new();
        for line in String::from_utf8_lossy(&stdout_bytes).lines() {
            capture.record_raw("stdout", line);
            if let ParseResult::Events(events) = parser.feed_line(line) {
                for event in &events {
                    capture.record_stream_event(event);
                }
            }
        }
        for line in String::from_utf8_lossy(&stderr_bytes).lines() {
            capture.record_raw("stderr", line);
        }
    }

    if timed_out {
        return Err(CoreError::ExecutionFailed(format!(
            "agent timed out after {}",
            format_timeout(timeout)
        )));
    }

    Ok(LaunchResult {
        exit_code: status.code().unwrap_or(1),
        stdout: String::from_utf8_lossy(&stdout_bytes).to_string(),
        stderr: String::from_utf8_lossy(&stderr_bytes).to_string(),
        provider_session_id: None,
        failure: None,
    })
}

fn launch_interactive(
    cmd: &mut Command,
    timeout: Option<Duration>,
) -> Result<LaunchResult, CoreError> {
    let start = Instant::now();
    let mut child = cmd.spawn()?;
    let _pid_guard = ChildPidGuard::new(child.id());
    tracing::debug!(
        elapsed_ms = start.elapsed().as_millis(),
        "agent spawned (interactive)"
    );
    let (status, timed_out) = wait_for_exit(&mut child, timeout)?;
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
    Ok(LaunchResult {
        exit_code: status.code().unwrap_or(1),
        stdout: String::new(),
        stderr: String::new(),
        provider_session_id: None,
        failure: None,
    })
}

fn launch_streaming(
    cmd: &mut Command,
    stream_format: StreamFormat,
    timeout: Option<Duration>,
    capture: Option<&AgentCapture>,
) -> Result<LaunchResult, CoreError> {
    cmd.stdin(Stdio::null());
    cmd.stdout(Stdio::piped());
    cmd.stderr(Stdio::piped());

    let start = Instant::now();
    let mut child = cmd.spawn()?;
    let _pid_guard = ChildPidGuard::new(child.id());
    tracing::debug!(elapsed_ms = start.elapsed().as_millis(), "agent spawned");

    let stdout = child
        .stdout
        .take()
        .ok_or_else(|| CoreError::ExecutionFailed("Failed to capture stdout".to_string()))?;
    let stderr = child
        .stderr
        .take()
        .ok_or_else(|| CoreError::ExecutionFailed("Failed to capture stderr".to_string()))?;

    enum StreamKind {
        Stdout,
        Stderr,
    }

    let (tx, rx) = mpsc::channel::<(StreamKind, String)>();

    let tx_out = tx.clone();
    let stdout_handle = thread::spawn(move || {
        let reader = BufReader::new(stdout);
        for line in reader.lines().map_while(Result::ok) {
            let _ = tx_out.send((StreamKind::Stdout, line));
        }
    });

    let tx_err = tx.clone();
    let stderr_handle = thread::spawn(move || {
        let reader = BufReader::new(stderr);
        for line in reader.lines().map_while(Result::ok) {
            let _ = tx_err.send((StreamKind::Stderr, line));
        }
    });

    drop(tx);

    let mut stdout_content = String::new();
    let mut stderr_content = String::new();
    let mut logged_first_output = false;
    let mut parser = StreamParser::new();

    let use_color = match stream_format {
        StreamFormat::Human(c) => Some(c),
        StreamFormat::Raw => None,
    };

    let timeout_at = timeout.map(|value| Instant::now() + value);
    let mut timed_out = false;
    loop {
        match rx.recv_timeout(Duration::from_millis(100)) {
            Ok((stream, line)) => {
                if !logged_first_output {
                    tracing::info!(
                        elapsed_ms = start.elapsed().as_millis(),
                        "agent produced first output"
                    );
                    logged_first_output = true;
                }
                match stream {
                    StreamKind::Stdout => {
                        if let Some(capture) = capture {
                            capture.record_raw("stdout", &line);
                        }
                        if let Some(color) = use_color {
                            match parser.feed_line(&line) {
                                ParseResult::Events(events) => {
                                    for event in &events {
                                        if let Some(capture) = capture {
                                            capture.record_stream_event(event);
                                        }
                                        format_event(event, color);
                                    }
                                }
                                ParseResult::Skipped => {}
                                ParseResult::Passthrough => println!("{line}"),
                            }
                        } else {
                            if let ParseResult::Events(events) = parser.feed_line(&line) {
                                for event in &events {
                                    if let Some(capture) = capture {
                                        capture.record_stream_event(event);
                                    }
                                }
                            }
                            println!("{line}");
                        }
                        stdout_content.push_str(&line);
                        stdout_content.push('\n');
                    }
                    StreamKind::Stderr => {
                        if let Some(capture) = capture {
                            capture.record_raw("stderr", &line);
                        }
                        if use_color.is_some() {
                            // In Human mode, Claude --verbose duplicates stream-json
                            // on stderr. Parse it and skip recognized events to avoid
                            // printing raw JSON alongside formatted output.
                            match parser.feed_line(&line) {
                                ParseResult::Events(_) | ParseResult::Skipped => {}
                                ParseResult::Passthrough => eprintln!("{line}"),
                            }
                        } else {
                            eprintln!("{line}");
                        }
                        stderr_content.push_str(&line);
                        stderr_content.push('\n');
                    }
                }
            }
            Err(mpsc::RecvTimeoutError::Timeout) => {}
            Err(mpsc::RecvTimeoutError::Disconnected) => break,
        }

        if !timed_out && timeout_at.is_some_and(|value| Instant::now() >= value) {
            let _ = child.kill();
            timed_out = true;
        }
    }

    let status = child.wait()?;
    tracing::debug!(
        elapsed_ms = start.elapsed().as_millis(),
        "agent streaming completed"
    );

    let _ = stdout_handle.join();
    let _ = stderr_handle.join();

    if timed_out {
        return Err(CoreError::ExecutionFailed(format!(
            "agent timed out after {}",
            format_timeout(timeout)
        )));
    }

    Ok(LaunchResult {
        exit_code: status.code().unwrap_or(1),
        stdout: stdout_content,
        stderr: stderr_content,
        provider_session_id: None,
        failure: None,
    })
}

fn wait_for_exit(
    child: &mut Child,
    timeout: Option<Duration>,
) -> Result<(ExitStatus, bool), CoreError> {
    let timeout_at = timeout.map(|value| Instant::now() + value);
    loop {
        if let Some(status) = child.try_wait()? {
            return Ok((status, false));
        }

        if timeout_at.is_some_and(|value| Instant::now() >= value) {
            let _ = child.kill();
            let status = child.wait()?;
            return Ok((status, true));
        }

        thread::sleep(Duration::from_millis(50));
    }
}

fn format_timeout(timeout: Option<Duration>) -> String {
    timeout
        .map(|value| format!("{}ms", value.as_millis()))
        .unwrap_or_else(|| "unknown duration".to_string())
}

/// Check if a CLI is available.
pub fn check_cli_available(cli: &str) -> bool {
    static CACHE: OnceLock<Mutex<std::collections::HashMap<String, bool>>> = OnceLock::new();
    let cache = CACHE.get_or_init(|| Mutex::new(std::collections::HashMap::new()));
    if let Ok(guard) = cache.lock() {
        if let Some(value) = guard.get(cli) {
            return *value;
        }
    }

    let available = Command::new(cli)
        .arg("--version")
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .map(|s| s.success())
        .unwrap_or(false);

    if let Ok(mut guard) = cache.lock() {
        guard.insert(cli.to_string(), available);
    }

    available
}

/// Agent runner trait for dependency injection in tests.
pub trait Runner: Send + Sync {
    fn launch(
        &self,
        launch: &AgentConfig,
        process: &ProcessConfig,
        capabilities: &AgentCapabilities,
    ) -> Result<LaunchResult, CoreError>;
}

/// Default agent runner that spawns actual processes.
#[derive(Debug, Clone, Copy, Default)]
pub struct DefaultRunner;

impl Runner for DefaultRunner {
    fn launch(
        &self,
        launch: &AgentConfig,
        process: &ProcessConfig,
        capabilities: &AgentCapabilities,
    ) -> Result<LaunchResult, CoreError> {
        launch_agent(launch, process, capabilities)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

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
    fn codex_permission_args_supply_loopflow_floor_when_unset() {
        let args = codex_permission_args_for_config(CodexPermissionConfig::default(), true, false);

        assert_arg_pair(&args, "--sandbox", "workspace-write");
        assert_arg_pair(&args, "--ask-for-approval", "never");
    }

    #[test]
    fn codex_permission_args_do_not_downgrade_danger_full_access() {
        let args = codex_permission_args_for_config(
            CodexPermissionConfig {
                sandbox: Some(CodexSandboxMode::DangerFullAccess),
                approval: Some(CodexApprovalPolicy::Never),
            },
            true,
            false,
        );

        assert!(!args.contains(&"--sandbox".to_string()));
        assert!(!args.contains(&"--ask-for-approval".to_string()));
    }

    #[test]
    fn codex_permission_args_raise_less_permissive_config_to_floor() {
        let args = codex_permission_args_for_config(
            CodexPermissionConfig {
                sandbox: Some(CodexSandboxMode::ReadOnly),
                approval: Some(CodexApprovalPolicy::OnRequest),
            },
            true,
            false,
        );

        assert_arg_pair(&args, "--sandbox", "workspace-write");
        assert_arg_pair(&args, "--ask-for-approval", "never");
    }

    #[test]
    fn parse_claude_permission_mode_reads_default_mode() {
        assert_eq!(
            parse_claude_permission_mode(r#"{"permissions":{"defaultMode":"bypassPermissions"}}"#),
            Some(ClaudePermissionMode::BypassPermissions)
        );
    }

    #[test]
    fn build_claude_command_auto() {
        let launch = AgentConfig {
            skip_permissions: false,
            ..default_launch()
        };
        let process = ProcessConfig {
            auto: true,
            stream: false,
            ..Default::default()
        };
        let cmd = build_claude_command(&launch, &process, &AgentCapabilities::default(), None);
        assert!(cmd.contains(&"--print".to_string()));
        assert_eq!(
            cmd.contains(&"--dangerously-skip-permissions".to_string()),
            claude_skip_permissions(None, true, false)
        );
    }

    #[test]
    fn build_claude_command_yolo() {
        let launch = AgentConfig {
            skip_permissions: true,
            ..default_launch()
        };
        let process = ProcessConfig {
            auto: true,
            stream: false,
            ..Default::default()
        };
        let cmd = build_claude_command(&launch, &process, &AgentCapabilities::default(), None);
        assert!(cmd.contains(&"--dangerously-skip-permissions".to_string()));
    }

    #[test]
    fn build_claude_command_stream() {
        let launch = default_launch();
        let process = ProcessConfig {
            auto: true,
            stream: true,
            ..Default::default()
        };
        let cmd = build_claude_command(&launch, &process, &AgentCapabilities::default(), None);
        assert!(cmd.contains(&"stream-json".to_string()));
        assert!(cmd.contains(&"--verbose".to_string()));
    }

    #[test]
    fn build_claude_command_with_model_variant() {
        let launch = default_launch();
        let process = auto_process();
        let cmd = build_claude_command(
            &launch,
            &process,
            &AgentCapabilities::default(),
            Some("opus"),
        );
        assert!(cmd.contains(&"--model".to_string()));
        assert!(cmd.contains(&"opus".to_string()));
    }

    #[test]
    fn build_claude_command_with_chrome_flag() {
        let launch = default_launch();
        let process = auto_process();
        let cmd =
            build_claude_command(&launch, &process, &AgentCapabilities { chrome: true }, None);
        assert!(cmd.contains(&"--chrome".to_string()));
    }

    #[test]
    fn build_claude_command_adds_main_repo_for_worktree_metadata() {
        let (_tmp, main, worktree) = git_worktree_fixture();
        let launch = AgentConfig {
            cwd: Some(worktree),
            ..default_launch()
        };
        let process = auto_process();

        let cmd = build_claude_command(&launch, &process, &AgentCapabilities::default(), None);
        let idx = cmd
            .iter()
            .position(|arg| arg == "--add-dir")
            .expect("add-dir flag");
        assert_eq!(
            PathBuf::from(&cmd[idx + 1]).canonicalize().unwrap(),
            main.canonicalize().unwrap()
        );
    }

    #[test]
    fn build_claude_command_omits_add_dir_for_main_repo() {
        let (_tmp, main, _worktree) = git_worktree_fixture();
        let launch = AgentConfig {
            cwd: Some(main),
            ..default_launch()
        };
        let process = auto_process();

        let cmd = build_claude_command(&launch, &process, &AgentCapabilities::default(), None);
        assert!(!cmd.contains(&"--add-dir".to_string()));
    }

    #[test]
    fn build_codex_command_auto() {
        let launch = AgentConfig {
            skip_permissions: false,
            ..default_launch()
        };
        let process = ProcessConfig {
            auto: true,
            stream: false,
            ..Default::default()
        };
        let cmd = build_codex_command(&launch, &process, None);
        let policy_args = codex_permission_args(None, true, false);
        assert!(cmd.contains(&"exec".to_string()));
        assert_arg_pair(&cmd, "-c", "service_tier=\"default\"");
        assert!(!cmd.contains(&"--full-auto".to_string()));
        assert_eq!(
            cmd.contains(&"--sandbox".to_string()),
            policy_args.contains(&"--sandbox".to_string())
        );
        assert_eq!(
            cmd.contains(&"--ask-for-approval".to_string()),
            policy_args.contains(&"--ask-for-approval".to_string())
        );
        assert!(!cmd.contains(&"--dangerously-bypass-approvals-and-sandbox".to_string()));
    }

    #[test]
    fn build_codex_command_interactive() {
        let launch = AgentConfig {
            skip_permissions: false,
            ..default_launch()
        };
        let process = ProcessConfig {
            auto: false,
            stream: false,
            ..Default::default()
        };
        let cmd = build_codex_command(&launch, &process, None);
        assert_arg_pair(&cmd, "-c", "service_tier=\"default\"");
        assert!(
            !cmd.contains(&"exec".to_string()),
            "interactive mode should not use 'exec'"
        );
        assert!(!cmd.contains(&"--full-auto".to_string()));
        assert_eq!(
            cmd.contains(&"--sandbox".to_string()),
            codex_permission_args(None, false, false).contains(&"--sandbox".to_string())
        );
    }

    #[test]
    fn build_codex_command_adds_main_repo_for_worktree_metadata() {
        let (_tmp, main, worktree) = git_worktree_fixture();
        let launch = AgentConfig {
            cwd: Some(worktree),
            skip_permissions: false,
            ..default_launch()
        };
        let process = auto_process();

        let cmd = build_codex_command(&launch, &process, None);
        let idx = cmd
            .iter()
            .position(|arg| arg == "--add-dir")
            .expect("add-dir flag");
        assert_eq!(
            PathBuf::from(&cmd[idx + 1]).canonicalize().unwrap(),
            main.canonicalize().unwrap()
        );
    }

    #[test]
    fn build_codex_command_omits_add_dir_for_main_repo() {
        let (_tmp, main, _worktree) = git_worktree_fixture();
        let launch = AgentConfig {
            cwd: Some(main),
            skip_permissions: false,
            ..default_launch()
        };
        let process = auto_process();

        let cmd = build_codex_command(&launch, &process, None);
        assert!(!cmd.contains(&"--add-dir".to_string()));
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
        let process = auto_process();

        let codex = build_codex_command(&launch, &process, None);
        assert!(codex.contains(&"--dangerously-bypass-approvals-and-sandbox".to_string()));
        assert!(!codex.contains(&"--add-dir".to_string()));
        let thread = build_codex_thread_start_params(&launch);
        assert_eq!(thread["sandbox"], "danger-full-access");
        assert_eq!(thread["approvalPolicy"], "never");
        assert!(thread.get("config").is_none());

        let claude = build_claude_command(&launch, &process, &AgentCapabilities::default(), None);
        assert!(claude.contains(&"--dangerously-skip-permissions".to_string()));
        assert!(!claude.contains(&"--add-dir".to_string()));
    }

    #[test]
    fn build_codex_command_with_model() {
        let launch = default_launch();
        let process = auto_process();
        let cmd = build_codex_command(&launch, &process, Some("o3"));
        assert!(cmd.contains(&"model=\"o3\"".to_string()));
    }

    #[test]
    fn build_codex_command_yolo() {
        let launch = AgentConfig {
            skip_permissions: true,
            ..default_launch()
        };
        let process = auto_process();
        let cmd = build_codex_command(&launch, &process, None);
        assert!(cmd.contains(&"--dangerously-bypass-approvals-and-sandbox".to_string()));
        assert!(!cmd.contains(&"--sandbox".to_string()));
        assert!(!cmd.contains(&"--ask-for-approval".to_string()));
        assert!(!cmd.contains(&"--full-auto".to_string()));
    }

    #[test]
    fn build_gemini_command_yolo() {
        let launch = AgentConfig {
            skip_permissions: true,
            ..default_launch()
        };
        let process = auto_process();
        let cmd = build_gemini_command(&launch, &process, None);
        assert!(cmd.contains(&"--yolo".to_string()));
    }

    #[test]
    fn build_gemini_command_with_model() {
        let launch = default_launch();
        let process = auto_process();
        let cmd = build_gemini_command(&launch, &process, Some("gemini-1.5"));
        assert!(cmd.contains(&"-m".to_string()));
        assert!(cmd.contains(&"gemini-1.5".to_string()));
    }

    #[test]
    fn build_claude_command_with_context_file() {
        let launch = default_launch();
        let process = ProcessConfig {
            context_file: Some(std::path::PathBuf::from("/tmp/context.md")),
            ..Default::default()
        };
        let cmd = build_claude_command(&launch, &process, &AgentCapabilities::default(), None);
        assert!(cmd.contains(&"--append-system-prompt-file".to_string()));
        assert!(cmd.contains(&"/tmp/context.md".to_string()));
    }

    #[test]
    fn build_codex_command_with_context_file() {
        let launch = default_launch();
        let process = ProcessConfig {
            context_file: Some(std::path::PathBuf::from("/tmp/context.md")),
            ..Default::default()
        };
        let cmd = build_codex_command(&launch, &process, None);
        assert!(cmd.contains(&"-c".to_string()));
        assert!(cmd.contains(&"model_instructions_file=\"/tmp/context.md\"".to_string()));
    }

    #[test]
    fn build_codex_command_without_context_file() {
        // Skill-launched skills clear the system prompt, so no context file is
        // written. Codex must not receive an empty `model_instructions_file`.
        let launch = default_launch();
        let process = ProcessConfig {
            context_file: None,
            ..Default::default()
        };
        let cmd = build_codex_command(&launch, &process, None);
        assert!(!cmd.iter().any(|a| a.contains("model_instructions_file")));
    }

    #[test]
    fn build_opencode_command_auto() {
        let process = ProcessConfig {
            auto: true,
            ..Default::default()
        };
        let cmd = build_opencode_command(&process, None);
        assert_eq!(cmd[0], "opencode");
        assert_eq!(cmd[1], "run");
    }

    #[test]
    fn build_opencode_command_interactive() {
        let process = ProcessConfig::default();
        let cmd = build_opencode_command(&process, None);
        assert_eq!(cmd, vec!["opencode"]);
    }

    #[test]
    fn build_opencode_command_with_model() {
        let process = ProcessConfig::default();
        let cmd = build_opencode_command(&process, Some("anthropic/claude-sonnet-4-5"));
        assert!(cmd.contains(&"--model".to_string()));
        assert!(cmd.contains(&"anthropic/claude-sonnet-4-5".to_string()));
    }

    #[test]
    fn build_opencode_command_streaming() {
        let process = ProcessConfig {
            auto: true,
            stream: true,
            ..Default::default()
        };
        let cmd = build_opencode_command(&process, None);
        assert!(cmd.contains(&"run".to_string()));
        assert!(cmd.contains(&"--format".to_string()));
        assert!(cmd.contains(&"json".to_string()));
    }

    // ── build_opencode_env ──────────────────────────────────────

    #[test]
    fn build_opencode_env_auto_and_context() {
        let process = ProcessConfig {
            auto: true,
            context_file: Some(std::path::PathBuf::from("/tmp/lf-context.md")),
            ..Default::default()
        };
        let env = build_opencode_env(&process).unwrap();
        let v: serde_json::Value = serde_json::from_str(&env).unwrap();
        assert_eq!(v["permission"], "allow");
        assert_eq!(v["instructions"][0], "/tmp/lf-context.md");
    }

    #[test]
    fn build_opencode_env_auto_only() {
        let process = ProcessConfig {
            auto: true,
            ..Default::default()
        };
        let env = build_opencode_env(&process).unwrap();
        let v: serde_json::Value = serde_json::from_str(&env).unwrap();
        assert_eq!(v["permission"], "allow");
        assert!(v.get("instructions").is_none());
    }

    #[test]
    fn build_opencode_env_context_only() {
        let process = ProcessConfig {
            context_file: Some(std::path::PathBuf::from("/tmp/lf-context.md")),
            ..Default::default()
        };
        let env = build_opencode_env(&process).unwrap();
        let v: serde_json::Value = serde_json::from_str(&env).unwrap();
        assert!(v.get("permission").is_none());
        assert_eq!(v["instructions"][0], "/tmp/lf-context.md");
    }

    #[test]
    fn build_opencode_env_neither() {
        assert!(build_opencode_env(&ProcessConfig::default()).is_none());
    }

    // ── build_agent_command: opencode integration ───────────────

    #[test]
    fn build_agent_command_opencode_default() {
        let launch = AgentConfig {
            agent: Some("opencode".to_string()),
            task_prompt: "fix the bug".to_string(),
            ..Default::default()
        };
        let process = ProcessConfig {
            auto: true,
            ..Default::default()
        };
        let cmd = build_agent_command(&launch, &process, &AgentCapabilities::default());
        assert_eq!(cmd[0], "opencode");
        assert_eq!(cmd[1], "run");
        assert_eq!(*cmd.last().unwrap(), "fix the bug");
    }

    #[test]
    fn build_agent_command_opencode_with_variant() {
        let launch = AgentConfig {
            agent: Some("opencode:anthropic/claude-sonnet".to_string()),
            task_prompt: "fix the bug".to_string(),
            ..Default::default()
        };
        let process = ProcessConfig {
            auto: true,
            ..Default::default()
        };
        let cmd = build_agent_command(&launch, &process, &AgentCapabilities::default());
        assert!(cmd.contains(&"--model".to_string()));
        assert!(cmd.contains(&"anthropic/claude-sonnet".to_string()));
        assert_eq!(*cmd.last().unwrap(), "fix the bug");
    }

    // ── ClaudeArgs ────────────────────────────────────────────────

    #[test]
    fn claude_args_empty() {
        let args = ClaudeArgs::default().to_args();
        assert!(args.is_empty());
    }

    #[test]
    fn claude_args_model() {
        let args = ClaudeArgs {
            model: Some("opus".to_string()),
            ..Default::default()
        }
        .to_args();
        assert_eq!(args, vec!["--model", "opus"]);
    }

    #[test]
    fn claude_args_system_prompt_file_takes_precedence() {
        let args = ClaudeArgs {
            system_prompt: Some("inline text".to_string()),
            system_prompt_file: Some("/tmp/context.md".into()),
            ..Default::default()
        }
        .to_args();
        assert!(args.contains(&"--append-system-prompt-file".to_string()));
        assert!(args.contains(&"/tmp/context.md".to_string()));
        assert!(!args.contains(&"--append-system-prompt".to_string()));
    }

    #[test]
    fn claude_args_system_prompt_text() {
        let args = ClaudeArgs {
            system_prompt: Some("Be concise".to_string()),
            ..Default::default()
        }
        .to_args();
        assert_eq!(args, vec!["--append-system-prompt", "Be concise"]);
    }

    #[test]
    fn claude_args_empty_system_prompt_skipped() {
        let args = ClaudeArgs {
            system_prompt: Some("  ".to_string()),
            ..Default::default()
        }
        .to_args();
        assert!(args.is_empty());
    }

    #[test]
    fn claude_args_all_flags() {
        let args = ClaudeArgs {
            model: Some("sonnet".to_string()),
            system_prompt: Some("Be brief".to_string()),
            system_prompt_file: None,
            add_dirs: vec!["/tmp/repo".into()],
            skip_permissions: true,
            worktree_isolation: false,
            max_turns: Some(10),
            stream: true,
            chrome: true,
            resume_id: Some("sess_abc".to_string()),
        }
        .to_args();
        assert!(args.contains(&"--chrome".to_string()));
        assert!(args.contains(&"--model".to_string()));
        assert!(args.contains(&"sonnet".to_string()));
        assert!(args.contains(&"--append-system-prompt".to_string()));
        assert!(args.contains(&"--add-dir".to_string()));
        assert!(args.contains(&"/tmp/repo".to_string()));
        assert!(args.contains(&"--dangerously-skip-permissions".to_string()));
        assert!(args.contains(&"--max-turns".to_string()));
        assert!(args.contains(&"10".to_string()));
        assert!(args.contains(&"--output-format".to_string()));
        assert!(args.contains(&"stream-json".to_string()));
        assert!(args.contains(&"--verbose".to_string()));
        assert!(args.contains(&"--resume".to_string()));
        assert!(args.contains(&"sess_abc".to_string()));
    }

    #[test]
    fn claude_args_resolve_model_bare() {
        // "claude" → default variant "opus"
        assert_eq!(
            ClaudeArgs::resolve_model("claude"),
            Some("opus".to_string())
        );
    }

    #[test]
    fn claude_args_resolve_model_with_variant() {
        assert_eq!(
            ClaudeArgs::resolve_model("claude:sonnet"),
            Some("sonnet".to_string())
        );
    }

    #[test]
    fn claude_args_resolve_model_full_string() {
        // Non-claude backend passes through unchanged
        assert_eq!(
            ClaudeArgs::resolve_model("claude-sonnet-4-5-20250514"),
            Some("claude-sonnet-4-5-20250514".to_string())
        );
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
    fn build_claude_session_turn_args_minimal() {
        let config = AgentConfig {
            system_prompt: String::new(),
            task_prompt: "task".to_string(),
            agent: None,
            cwd: Some("/tmp".into()),
            max_turns: None,
            resume_token: None,
            provider_account_id: None,
            provider_account_authority_home: None,
            write_scope: AgentWriteScope::Configured,
            execution_boundary: None,
            skip_permissions: false,
            structured_replies: Vec::new(),
            directive_relay: None,
            env: BTreeMap::new(),
        };
        let args = build_claude_session_turn_args("hello", &config, None);
        assert_eq!(args[0], "-p");
        assert_eq!(args[1], "hello");
        assert!(args.contains(&"--output-format".to_string()));
        assert!(args.contains(&"stream-json".to_string()));
        assert!(args.contains(&"--verbose".to_string()));
        assert_eq!(
            args.contains(&"--dangerously-skip-permissions".to_string()),
            claude_skip_permissions(Some(Path::new("/tmp")), true, false)
        );
    }

    #[test]
    fn build_claude_session_turn_args_full() {
        let config = AgentConfig {
            system_prompt: "Be concise".to_string(),
            task_prompt: "task".to_string(),
            agent: Some("claude-sonnet-4-5-20250514".to_string()),
            cwd: Some("/tmp".into()),
            max_turns: Some(5),
            resume_token: None,
            provider_account_id: None,
            provider_account_authority_home: None,
            write_scope: AgentWriteScope::Configured,
            execution_boundary: None,
            skip_permissions: true,
            structured_replies: Vec::new(),
            directive_relay: None,
            env: BTreeMap::new(),
        };
        let args = build_claude_session_turn_args("fix tests", &config, Some("sess_abc"));
        assert!(args.contains(&"--resume".to_string()));
        assert!(args.contains(&"sess_abc".to_string()));
        assert!(args.contains(&"--model".to_string()));
        assert!(args.contains(&"claude-sonnet-4-5-20250514".to_string()));
        assert!(args.contains(&"--dangerously-skip-permissions".to_string()));
        assert!(args.contains(&"--max-turns".to_string()));
        assert!(args.contains(&"5".to_string()));
        assert!(args.contains(&"--append-system-prompt".to_string()));
        assert!(args.contains(&"Be concise".to_string()));
    }

    #[test]
    fn build_claude_session_turn_args_appends_loopflow_guidance() {
        let config = AgentConfig {
            system_prompt: "Base prompt".to_string(),
            task_prompt: "task".to_string(),
            agent: None,
            cwd: Some("/tmp".into()),
            max_turns: None,
            resume_token: None,
            provider_account_id: None,
            provider_account_authority_home: None,
            write_scope: AgentWriteScope::Configured,
            execution_boundary: None,
            skip_permissions: false,
            structured_replies: vec![StructuredReply {
                name: "suggest_actions".to_string(),
                description: "Suggest actions".to_string(),
                guidance: "Emit <lf:suggest_actions> JSON.".to_string(),
            }],
            directive_relay: None,
            env: BTreeMap::new(),
        };

        let args = build_claude_session_turn_args("hello", &config, None);
        let prompt_idx = args
            .iter()
            .position(|arg| arg == "--append-system-prompt")
            .expect("expected --append-system-prompt");
        let prompt = args
            .get(prompt_idx + 1)
            .expect("system prompt text should follow flag");
        assert!(prompt.contains("Base prompt"));
        assert!(prompt.contains("<lf:structured_replies>"));
        assert!(prompt.contains("<lf:suggest_actions>"));
    }

    #[test]
    fn build_model_command_uses_codex_default_for_bare_codex_model() {
        let launch = AgentConfig {
            agent: Some("codex".to_string()),
            ..default_launch()
        };
        let process = auto_process();
        let cmd = build_model_command(&launch, &process, &AgentCapabilities::default());
        assert!(!cmd.iter().any(|arg| arg.contains("model=\"codex\"")));
    }

    #[test]
    fn build_model_command_uses_loopflow_default_for_bare_opencode() {
        let launch = AgentConfig {
            agent: Some("opencode".to_string()),
            ..default_launch()
        };
        let process = auto_process();
        let cmd = build_model_command(&launch, &process, &AgentCapabilities::default());
        assert_eq!(cmd.first(), Some(&"opencode".to_string()));
        assert!(cmd.contains(&"--model".to_string()));
        assert!(cmd.contains(&"opencode/glm-5.2".to_string()));
    }

    #[test]
    fn build_model_command_falls_back_to_claude_for_unknown_model() {
        let unknown_model = "gpt-5.1-codex-high";
        let launch = AgentConfig {
            agent: Some(unknown_model.to_string()),
            ..default_launch()
        };
        let process = auto_process();
        let cmd = build_model_command(&launch, &process, &AgentCapabilities::default());
        assert_eq!(cmd.first(), Some(&"claude".to_string()));
        assert!(cmd.contains(&"--model".to_string()));
        assert!(cmd.contains(&unknown_model.to_string()));
    }

    fn managed_attempt(result: LaunchResult) -> AgentAttempt {
        AgentAttempt::Finished {
            result,
            can_failover: true,
        }
    }

    fn failed_result(message: &str) -> LaunchResult {
        LaunchResult {
            exit_code: 1,
            stdout: format!(r#"{{"type":"turn.failed","error":{{"message":"{message}"}}}}"#),
            ..Default::default()
        }
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
            LaunchResult {
                exit_code: 1,
                stdout: concat!(
                    "{\"type\":\"thread.started\",\"thread_id\":\"thread-123\"}\n",
                    "{\"type\":\"turn.failed\",\"error\":{\"message\":\"Selected model is at capacity. Please try a different model.\"}}\n"
                )
                .to_string(),
                stderr: String::new(),
                provider_session_id: None,
                failure: None,
            },
            LaunchResult {
                exit_code: 0,
                ..Default::default()
            },
        ]
        .into_iter();
        let mut attempts = Vec::new();
        let mut waits = Vec::new();

        let result = _launch_with_transient_retries(
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
        assert_eq!(attempts[1].resume_token.as_deref(), Some("thread-123"));
        assert_eq!(waits, vec![Duration::ZERO]);
    }

    #[test]
    fn subscription_limit_fails_over_without_resuming_the_account_session() {
        let launch = AgentConfig {
            agent: Some("claude:opus".to_string()),
            task_prompt: "compress the branch".to_string(),
            ..Default::default()
        };
        let process = auto_process();
        let failure = LaunchResult {
            exit_code: 1,
            stdout: concat!(
                "{\"type\":\"system\",\"session_id\":\"session-123\"}\n",
                "{\"type\":\"rate_limit_event\",\"rate_limit_info\":{\"status\":\"rejected\",\"rate_limit_type\":\"five_hour\",\"utilization\":1.0,\"resets_at\":1900000000}}\n",
                "{\"type\":\"result\",\"subtype\":\"error_during_execution\",\"is_error\":true,\"result\":\"You've hit your usage limit\"}\n"
            )
            .to_string(),
            stderr: String::new(),
            provider_session_id: None,
            failure: None,
        };
        assert!(matches!(
            _classify_agent_failure("claude", &failure),
            Some(AgentFailure::AccountSubscriptionLimit {
                resets_at: Some(1_900_000_000)
            })
        ));
        let mut results = vec![
            failure,
            LaunchResult {
                exit_code: 0,
                ..Default::default()
            },
        ]
        .into_iter();
        let mut attempts = Vec::new();
        let mut waits = Vec::new();

        let result = _launch_with_transient_retries(
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
            _classify_agent_failure("codex", &failure),
            Some(AgentFailure::AccountCredentialInvalidated)
        );
        let mut results = vec![
            failure,
            LaunchResult {
                exit_code: 0,
                ..Default::default()
            },
        ]
        .into_iter();
        let mut attempts = Vec::new();

        let result = _launch_with_transient_retries(
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

        let result = _launch_with_transient_retries(
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
        let result = _launch_with_transient_retries(
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
    fn permanent_agent_failure_is_not_retried() {
        let launch = AgentConfig {
            agent: Some("codex".to_string()),
            ..default_launch()
        };
        let process = auto_process();
        let mut attempts = 0;

        let result = _launch_with_transient_retries(
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

        let result = _launch_with_transient_retries(
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
    fn resumed_commands_preserve_provider_session() {
        let launch = AgentConfig {
            agent: Some("codex".to_string()),
            resume_token: Some("thread-123".to_string()),
            ..default_launch()
        };
        let codex = build_codex_command(&launch, &auto_process(), None);
        assert_eq!(&codex[..3], ["codex", "exec", "resume"]);
        assert_eq!(codex.last().map(String::as_str), Some("thread-123"));

        let launch = AgentConfig {
            agent: Some("claude:opus".to_string()),
            resume_token: Some("session-123".to_string()),
            ..default_launch()
        };
        let claude = build_claude_command(
            &launch,
            &auto_process(),
            &AgentCapabilities::default(),
            Some("opus"),
        );
        assert_arg_pair(&claude, "--resume", "session-123");
    }
}
