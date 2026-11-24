//! Agent invocation for spawning coding agent runners (Claude, Codex, OpenCode).
//!
//! This module handles building commands and spawning subprocesses for each
//! supported coding agent. Output can be captured or streamed.

mod availability;
pub use availability::MachineAgents;

mod session;
pub use session::resolve_session_config;

use loopflow_model::id::AgentSessionId;
use std::collections::BTreeMap;
use std::io::{Seek, Write};
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::{Mutex, OnceLock};
use std::thread;
use std::time::{Duration, SystemTime};

#[cfg(test)]
use crate::config::parse_agent;
use crate::config::Agent;
use crate::error::CoreError;
use crate::harness::Mode;
use crate::process::LfProcessRuntime;
use crate::prompt::structured_reply::{render_structured_reply_guidance, StructuredReply};

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

static INTERRUPTED: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);

/// Whether a signal is ending this process. A provider that exits from here on
/// was stopped with it: its turn was interrupted, it did not fail.
pub fn interrupted() -> bool {
    INTERRUPTED.load(std::sync::atomic::Ordering::SeqCst)
}

/// Finish interruption while excluding the command's ordinary return path.
/// Killing a child can wake that path before the remaining hooks finish.
pub fn exit_on_interrupt() -> ! {
    INTERRUPTED.store(true, std::sync::atomic::Ordering::SeqCst);
    // Before taking the hooks lock: the command's own return waits on it.
    crate::os_process::foreground::end_leader();
    let hooks = INTERRUPT_HOOKS.get().and_then(|hooks| hooks.lock().ok());
    if let Some(hooks) = &hooks {
        for hook in hooks.iter().rev() {
            hook();
        }
    }
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
    /// Driver answer prose, never raw provider protocol.
    pub stdout: String,
    /// Driver error messages; native terminals leave both output fields empty.
    pub stderr: String,
    /// Opaque provider continuation token observed during this Process.
    pub agent_session: Option<AgentSessionId>,
    /// Typed provider failure when the process output identifies one.
    pub failure: Option<AgentFailure>,
}

/// What a conversation sets in its own shells, lf's markers and its
/// provider's. A conversation lf starts sets its own; it never inherits the
/// caller's. An inherited `CLAUDE_CODE_CHILD_SESSION` turns off Claude's
/// transcript; an inherited `CODEX_SANDBOX` hands a nested Codex its parent's
/// sandbox. Settings a person chooses (`CLAUDE_CONFIG_DIR`, `CODEX_HOME`,
/// credentials) are not markers and pass through.
pub(crate) const CALLER_SESSION_ENV: &[&str] = &[
    crate::steer::PROVIDER_CONTEXT_ENV,
    crate::steer::BARE_STEER_ENV,
    crate::steer::STEER_SEEN_ENV,
    crate::steer::STEER_SESSION_ENV,
    crate::steer::STEER_TASK_ENV,
    crate::context_block::CONTEXT_DELIVERY_ENV,
    crate::process::AGENT_CALLER_ENV,
    crate::journal::LF_TRACE_ID_ENV,
    crate::journal::LF_PROCESS_ID_ENV,
    crate::journal::LF_CALLER_SESSION_ENV,
    crate::session_record::CAPTURE_KEY_ENV,
    "LF_EXECUTION_ID",
    "LF_RUN_DIR",
    // Claude Code
    "CLAUDECODE",
    "CLAUDE_CODE_CHILD_SESSION",
    "CLAUDE_CODE_SESSION_ID",
    "CLAUDE_CODE_SESSION_ATTENDED",
    "CLAUDE_CODE_ENTRYPOINT",
    "CLAUDE_CODE_EXECPATH",
    "CLAUDE_CODE_MESSAGING_SOCKET",
    "CLAUDE_CODE_MESSAGING_TOKEN",
    "CLAUDE_CODE_CHILD_NOTES_DIR",
    "CLAUDE_PID",
    "CLAUDE_EFFORT",
    "AI_AGENT",
    // Codex
    "CODEX_CI",
    "CODEX_SANDBOX",
    "CODEX_SANDBOX_NETWORK_DISABLED",
    "CODEX_SESSION_ID",
    "CODEX_THREAD_ID",
    "CODEX_TOOL_CALL_ID",
    "CODEX_VERSION",
    "CODEX_APPLY_PATCH_PRESERVE_LINE_ENDINGS",
    // OpenCode
    "OPENCODE",
    "OPENCODE_SESSION_ID",
    "OPENCODE_TERMINAL",
    "OPENCODE_CONFIG_CONTENT",
];

/// Start `command` as a new conversation rather than the caller's child. Run
/// it before applying the new conversation's environment, which sets lf's
/// markers afresh.
pub(crate) fn remove_caller_session_env(command: &mut std::process::Command) {
    for name in CALLER_SESSION_ENV {
        command.env_remove(name);
    }
}

#[derive(Clone)]
pub struct AgentConfig {
    /// Claude Chrome integration. `None` sends no flag so Claude's own
    /// configuration decides; an explicit choice sends `--chrome`/`--no-chrome`.
    pub chrome: Option<bool>,
    /// Initial Codex tier; omission leaves provider configuration in charge.
    pub codex_service_tier: Option<crate::config::CodexServiceTier>,
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
    /// Resolved harness and provider model.
    pub agent: Agent,
    /// Agents a headless launch moves to when a usage limit, an unavailable
    /// model or a revoked login refuses `agent`, as `Config::resolve_agents`
    /// gives them. Empty when the agent was an explicit `harness:model`.
    pub fallback_agents: Vec<Agent>,
    /// Max turn budget when supported by the harness.
    pub max_turns: Option<u32>,
    /// Retry continuation state, converted to `harness::Start` at the launch boundary.
    pub resume_token: Option<AgentSessionId>,
    /// Working directory.
    pub cwd: Option<std::path::PathBuf>,
    /// The resolved `yolo` setting. Ask `skips_permissions` what a launch sends.
    pub yolo: bool,
    /// Graph-runner-injected structured replies (rendered via harness prompt guidance).
    pub structured_replies: Vec<StructuredReply>,
    /// Temp file for relaying shell directives back to the invoking shell.
    /// When set, the agent subprocess gets `LOOPFLOW_DIRECTIVE_FILE` pointing
    /// to this path. The caller reads it after the agent exits and forwards
    /// safe directives (e.g. `cd`) to the real directive file.
    pub directive_relay: Option<std::path::PathBuf>,
    /// The Session launch lock of the `session resume` running this launch,
    /// released once the provider is recorded.
    pub launch_lock: Option<crate::ops::human_session::LaunchLock>,
    /// Environment scoped to this provider process and its descendants.
    pub env: BTreeMap<String, String>,
}

impl AgentConfig {
    pub fn new(agent: Agent) -> Self {
        Self {
            agent,
            chrome: None,
            codex_service_tier: None,
            system_prompt: String::new(),
            conversation_context: None,
            task_prompt: String::new(),
            skill_invocation: None,
            output_schema: None,
            fallback_agents: Vec::new(),
            max_turns: None,
            resume_token: None,
            cwd: None,
            yolo: false,
            structured_replies: Vec::new(),
            directive_relay: None,
            launch_lock: None,
            env: BTreeMap::new(),
        }
    }

    /// Whether the provider runs without asking permission: `yolo`, or nobody
    /// is attending to answer a prompt.
    pub fn skips_permissions(&self, mode: Mode) -> bool {
        self.yolo || mode == Mode::Headless
    }

    pub fn apply_agent_override(&mut self) -> Result<(), crate::error::LoadError> {
        if let Some(agent) = crate::config::agent_override() {
            self.agent = Agent::parse(&agent)?;
        }
        Ok(())
    }
}

/// Unit tests set the fields they exercise; Codex stands in for the rest.
#[cfg(test)]
impl Default for AgentConfig {
    fn default() -> Self {
        Self::new(Agent {
            harness: loopflow_harness::HarnessKind::Codex,
            model: None,
        })
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
            .field("fallback_agents", &self.fallback_agents)
            .field("max_turns", &self.max_turns)
            .field("resume_token", &self.resume_token)
            .field("cwd", &self.cwd)
            .field("yolo", &self.yolo)
            .field("structured_replies", &self.structured_replies)
            .field("directive_relay", &self.directive_relay)
            .field("launch_lock", &self.launch_lock.is_some())
            .field("env_keys", &self.env.keys().collect::<Vec<_>>())
            .finish()
    }
}

/// Status only: a store that cannot say so never stops the recovery.
fn _note_limit_recovery(
    note: impl FnOnce(
        &loopflow_storage::sqlite::SqliteStore,
        &loopflow_model::id::LfProcessId,
    ) -> crate::store::StoreResult<()>,
) {
    let Some(process) = crate::journal::current_lf_process_id() else {
        return;
    };
    let noted = || -> anyhow::Result<()> {
        let path = crate::store::database_path_from_env()?;
        Ok(note(&crate::store::open_sqlite(&path)?, &process)?)
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

/// Every agent the step may use refused it on a usage limit: it waits `delay`
/// before starting its order again. The terminal and Task status say until when.
fn _record_limit_wait(provider: &str, reason: &str, delay: Duration) {
    let retry_at = loopflow_storage::rows::now_unix()
        + i64::try_from(delay.as_secs()).expect("usage-limit wait fits in i64 seconds");
    eprintln!(
        "{provider}: {reason}\nWaiting until {} before trying again",
        crate::output::format_time(retry_at)
    );
    _record_limit_recovery(provider, reason, Some(retry_at));
}

/// The step left `agent` for the next agent in its order. The terminal gets
/// one line; the recorded recovery keeps the provider's whole `reason`.
fn _record_agent_move(agent: &Agent, failure: impl std::fmt::Display, reason: &str, next: &Agent) {
    eprintln!("{agent}: {failure}; continuing on {next}");
    let provider = agent.harness.as_str();
    _record_limit_recovery(
        provider,
        &format!("{reason}{}{next}", crate::ops::task_execution::MOVED_TO),
        None,
    );
}

/// A provider refused the step and no agent it may use is left: the stop
/// and what would let it move, where Task status and `lf task run` read it.
/// The caller puts the advice in the run's final error, so it prints once.
fn _record_stop(provider: &str, reason: &str, advice: &str) {
    _record_limit_recovery(
        provider,
        &format!("{reason}{}{advice}", crate::ops::task_execution::STOPPED),
        None,
    );
}

/// The provider revoked the login: no model of it can go on until someone
/// signs in again.
fn _sign_in_advice(kind: loopflow_harness::HarnessKind) -> String {
    let provider = kind.as_str();
    let sign_in = kind.harness().sign_in_advice();
    format!("The {provider} login was revoked; sign in again on this Machine: {sign_in}")
}

/// `rungs` is the whole order the step could walk; it stops after the last.
fn _limit_stop_advice(provider: &str, launch: &AgentConfig, rungs: &[Agent]) -> String {
    if !launch.fallback_agents.is_empty() {
        return format!(
            "Nothing is left in its order after {}; add another agent to `agents`",
            rungs
                .iter()
                .map(ToString::to_string)
                .collect::<Vec<_>>()
                .join(", ")
        );
    }
    let another = loopflow_harness::HarnessKind::ALL
        .into_iter()
        .find(|kind| kind.as_str() != provider)
        .expect("more than one harness")
        .as_str();
    format!(
        "No fallback remains. Change an `agent` or `-a` pin to select another model or provider, or remove it to allow fallbacks. If `agents` is missing or has only one entry, configure fallbacks with `lf set --global agents {provider},{another}`"
    )
}

/// Retain provider instructions on disk so native resume can read the same context.
pub fn write_system_prompt_file(
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
    /// Stream output in real-time.
    pub stream: bool,
    /// Path to context file for system prompt loading (model-agnostic).
    /// For Claude, this uses --append-system-prompt-file.
    pub context_file: Option<std::path::PathBuf>,
    /// Optional timeout for the subprocess.
    pub timeout: Option<Duration>,
    /// Live resources for this lf invocation, retained across provider retries.
    pub runtime: Option<LfProcessRuntime>,
    /// A Graph step waits for capacity once a usage limit has refused every
    /// agent in its order. Everything else headless (commit messages, PR
    /// text) has someone waiting on it and fails then.
    pub wait_for_capacity: bool,
}

/// Common Claude options shared by terminal and stream-input launches.
pub(crate) fn claude_options(
    config: &AgentConfig,
    model: Option<&str>,
    context_file: Option<&Path>,
    mode: Mode,
) -> Vec<String> {
    // Driver preferences (Chrome, model) pass through only when a Loopflow
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
    if config.skips_permissions(mode) {
        args.push("--dangerously-skip-permissions".into());
    }
    if mode == Mode::Headless {
        if let Some(max_turns) = config.max_turns {
            args.extend(["--max-turns".into(), max_turns.to_string()]);
        }
        if let Some(schema) = &config.output_schema {
            args.extend(["--json-schema".into(), schema.to_string()]);
        }
    }
    args
}

const TRANSIENT_RETRY_DELAYS: [Duration; 4] = [
    Duration::from_secs(2),
    Duration::from_secs(5),
    Duration::from_secs(15),
    Duration::from_secs(30),
];
/// The last resort for a headless run every configured agent refused on a usage
/// limit: wait, then start the order again; after the last wait, stop.
const USAGE_LIMIT_WAITS: [Duration; 4] = [
    Duration::from_secs(5 * 60),
    Duration::from_secs(15 * 60),
    Duration::from_secs(30 * 60),
    Duration::from_secs(60 * 60),
];
const RETRY_PROMPT: &str = "Continue the original task after the transient provider failure. Re-read the current workspace state, finish any interrupted work, and return the result.";
const LIMIT_FAILOVER_PROMPT: &str = "Continue this task after the previous provider reached its usage limit. Re-read the current workspace state and finish any interrupted work.";
const MODEL_FAILOVER_PROMPT: &str = "Continue this task after the previous model was unavailable. Re-read the current workspace state and finish any interrupted work.";
const AGENT_FAILOVER_PROMPT: &str = "Continue this task on this provider. Re-read the current workspace state and finish any interrupted work.";
const CREDENTIAL_FAILOVER_PROMPT: &str = "Continue this task after the previous provider's credential was revoked. Re-read the current workspace state and finish any interrupted work.";

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
    /// The selected model is unavailable; another configured entry may work.
    #[error("model unavailable")]
    ModelUnavailable,
    /// The provider process reported a retryable connection failure.
    #[error("provider transport")]
    Transport,
    /// The native provider refused the turn on its usage limit.
    #[error("provider usage limit")]
    UsageLimit,
    /// The native provider's credential was explicitly revoked.
    #[error("provider credential invalidated")]
    CredentialInvalidated,
}

impl AgentFailure {
    /// The failure a harness decoder's error code states. `None` for a code
    /// that names no cause (`codex_error`); the provider's words decide then.
    pub(crate) fn from_code(code: &str) -> Option<Self> {
        match code {
            "provider_rate_limited" => Some(Self::UsageLimit),
            "provider_model_unavailable" => Some(Self::ModelUnavailable),
            "provider_credential_invalidated" => Some(Self::CredentialInvalidated),
            "provider_capacity" => Some(Self::Capacity),
            _ => None,
        }
    }

    /// Of two failures one turn reported, the one that decides what happens
    /// next: a revoked login, then a spent quota, then an unserved model, then
    /// a transient failure. The earlier wins a tie.
    pub(crate) fn graver(self, other: Self) -> Self {
        let rank = |failure| match failure {
            Self::CredentialInvalidated => 0,
            Self::UsageLimit => 1,
            Self::ModelUnavailable => 2,
            _ => 3,
        };
        if rank(other) < rank(self) {
            other
        } else {
            self
        }
    }
}

/// Build `thread/start` params for Codex app-server sessions. A launch that
/// skips permissions gets full access and no approvals; otherwise Codex's own
/// configuration decides.
pub fn build_codex_thread_start_params(
    launch: &AgentConfig,
    mode: Mode,
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

    if let Some(model) = launch.agent.model.clone() {
        params.insert("model".to_string(), serde_json::Value::String(model));
    }

    if let Some(cwd) = launch.cwd.as_ref() {
        params.insert(
            "cwd".to_string(),
            serde_json::Value::String(cwd.to_string_lossy().to_string()),
        );
    }

    if launch.skips_permissions(mode) {
        params.insert("sandbox".into(), "danger-full-access".into());
        params.insert("approvalPolicy".into(), "never".into());
    }

    params
}

/// A bare Codex launch's options, as plain `codex` takes them: the thread
/// settings an app-server launch would send, the writable roots and the two
/// context hooks. Nothing else of lf's configuration rides along.
pub(crate) fn codex_terminal_options(
    launch: &AgentConfig,
    kind: crate::harness::LaunchKind,
) -> anyhow::Result<Vec<String>> {
    let params = build_codex_thread_start_params(launch, kind.mode());
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
            crate::harness::context::codex_hooks(context, None, kind)?,
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

/// Drive one headless provider turn, retrying according to process policy.
pub fn run_agent(
    launch: &AgentConfig,
    process: &ProcessConfig,
    kind: crate::harness::LaunchKind,
) -> Result<AgentProcessResult, CoreError> {
    let cwd = match &launch.cwd {
        Some(cwd) => cwd.clone(),
        None => std::env::current_dir()?,
    };
    let command = std::env::args().collect::<Vec<_>>();
    crate::journal::with_runtime(&cwd, &command, || {
        run_admitted_agent(launch, process, kind).map_err(Into::into)
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
    kind: crate::harness::LaunchKind,
) -> Result<AgentProcessResult, CoreError> {
    let mut launch = launch.clone();
    let capabilities = crate::harness::launcher(launch.agent.harness).capabilities();
    if launch.max_turns.is_some() && !capabilities.turn_limit {
        eprintln!("warning: Loopflow turn limits are supported only by Claude; this provider will run without a turn limit");
    }
    if launch.output_schema.is_some() && !capabilities.output_schema {
        eprintln!("warning: Loopflow native response-schema enforcement is Claude-specific; using ordinary provider output (Graph output validation still applies)");
    }
    if launch.resume_token.is_none() {
        if let Some(runtime) = &process.runtime {
            launch.resume_token = runtime.conversation_resume_token().map_err(|error| {
                CoreError::ExecutionFailed(format!("conversation recovery failed: {error}"))
            })?;
        }
    }
    let harness = launch.agent.harness.as_str();
    let model = launch.agent.model.clone();
    let implicit_runtime = if process.runtime.is_none() {
        Some(_begin_process_runtime(&launch, harness, model)?)
    } else {
        None
    };
    let mut process = process.clone();
    for name in CALLER_SESSION_ENV {
        launch.env.remove(*name);
    }
    if let Some(runtime) = &implicit_runtime {
        process.runtime = Some(runtime.clone());
    }
    if let Some(runtime) = &process.runtime {
        runtime.claim_session_attachment().map_err(|error| {
            CoreError::ExecutionFailed(format!("conversation admission failed: {error}"))
        })?;
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
    let result = _run_with_retries(
        &launch,
        &process,
        &TRANSIENT_RETRY_DELAYS,
        |attempt, retry| _run_harness_once(attempt, &process, kind, retry),
        _sleep_wall_clock,
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

fn _run_with_retries(
    launch: &AgentConfig,
    process: &ProcessConfig,
    retry_delays: &[Duration],
    mut run: impl FnMut(&AgentConfig, bool) -> Result<AgentProcessResult, CoreError>,
    mut wait: impl FnMut(Duration),
) -> Result<AgentProcessResult, CoreError> {
    let rungs = _agent_order(launch);
    let mut rung = 0;
    let mut attempt = launch.clone();
    let mut launched = false;
    let mut transient_failures = 0;
    let mut usage_waits = 0;
    loop {
        let provider = attempt.agent.harness.as_str();
        let mut result = run(&attempt, launched)?;
        launched = true;
        let Some(failure) = result.failure else {
            return Ok(result);
        };
        let prompt = match failure {
            AgentFailure::UsageLimit => LIMIT_FAILOVER_PROMPT,
            AgentFailure::CredentialInvalidated => CREDENTIAL_FAILOVER_PROMPT,
            AgentFailure::ModelUnavailable => MODEL_FAILOVER_PROMPT,
            _ => RETRY_PROMPT,
        };
        if let Some(thread) = result
            .agent_session
            .clone()
            .or(attempt.resume_token.clone())
        {
            attempt.resume_token = Some(thread);
            attempt.task_prompt = prompt.to_owned();
            attempt.skill_invocation = None;
        }
        // Without a native thread, retry the same input.
        let delay = match failure {
            AgentFailure::UsageLimit
            | AgentFailure::CredentialInvalidated
            | AgentFailure::ModelUnavailable => {
                let reason = _failure_reason(&result, failure);
                // A revoked login is every model of its provider; only
                // another provider in the order can go on.
                let revoked = failure == AgentFailure::CredentialInvalidated;
                let next = rungs
                    .iter()
                    .enumerate()
                    .skip(rung + 1)
                    .find(|(_, next)| !revoked || next.harness.as_str() != provider);
                if let Some((index, next)) = next {
                    rung = index;
                    _record_agent_move(&attempt.agent, failure, &reason, next);
                    attempt = _climb(&attempt, next, process, launch);
                    Duration::ZERO
                } else if let (AgentFailure::UsageLimit, true, Some(&delay)) = (
                    failure,
                    process.wait_for_capacity,
                    USAGE_LIMIT_WAITS.get(usage_waits),
                ) {
                    // Each try after the wait starts the order again and reads
                    // the native login afresh.
                    usage_waits += 1;
                    _record_limit_wait(provider, &reason, delay);
                    if rung > 0 {
                        rung = 0;
                        attempt = _climb(&attempt, &rungs[0], process, launch);
                    }
                    delay
                } else {
                    let advice = if revoked {
                        _sign_in_advice(attempt.agent.harness)
                    } else {
                        _limit_stop_advice(provider, launch, &rungs)
                    };
                    _record_stop(provider, &reason, &advice);
                    result.stderr = format!("{reason}\n{advice}");
                    return Ok(result);
                }
            }
            _ => {
                let Some(delay) = retry_delays.get(transient_failures).copied() else {
                    return Ok(result);
                };
                transient_failures += 1;
                delay
            }
        };
        tracing::warn!(%failure, delay_ms = delay.as_millis(), resumed = attempt.resume_token.is_some(), "recoverable provider failure; retrying");
        wait(delay);
    }
}

/// Sleep `delay` of wall-clock time. `Instant` stops while a Mac sleeps, so a
/// usage-limit wait checks `SystemTime` between short naps.
fn _sleep_wall_clock(delay: Duration) {
    let deadline = SystemTime::now() + delay;
    loop {
        let left = deadline
            .duration_since(SystemTime::now())
            .unwrap_or_default();
        if left.is_zero() {
            return;
        }
        thread::sleep(left.min(Duration::from_secs(30)));
    }
}

fn _with_original_task(prompt: &str, launch: &AgentConfig) -> String {
    format!("{prompt}\n\nOriginal task:\n\n{}", launch.task_prompt)
}

/// The order to move through when a provider refuses: the launch's agent,
/// then its fallbacks.
fn _agent_order(launch: &AgentConfig) -> Vec<Agent> {
    let mut order: Vec<Agent> = Vec::new();
    for agent in std::iter::once(&launch.agent).chain(&launch.fallback_agents) {
        if !order.contains(agent) {
            order.push(agent.clone());
        }
    }
    order
}

/// Put the current attempt on `agent`. The thread moves within its provider;
/// another provider reads the conversation so far and begins its own.
fn _climb(
    origin: &AgentConfig,
    agent: &Agent,
    process: &ProcessConfig,
    launch: &AgentConfig,
) -> AgentConfig {
    let mut next = origin.clone();
    next.agent = agent.clone();
    if origin.agent.harness == agent.harness {
        return next;
    }
    next.resume_token = None;
    (next.skill_invocation, next.task_prompt) = match _history_import(process) {
        Ok(Some(turn)) => (None, turn),
        // The provider refused before the agent said anything: the launch's
        // own first turn, whole.
        Ok(None) => (launch.skill_invocation.clone(), launch.task_prompt.clone()),
        Err(error) => {
            tracing::warn!(%error, "conversation could not move providers; the task starts over");
            (
                launch.skill_invocation.clone(),
                _with_original_task(AGENT_FAILOVER_PROMPT, launch),
            )
        }
    };
    next
}

/// The conversation so far as the next provider's first turn, through the
/// import `lf session new -a` uses. None when the agent has not replied or
/// used a tool; an error when the run has no Session or its transcript cannot
/// be read yet.
fn _history_import(process: &ProcessConfig) -> anyhow::Result<Option<String>> {
    let (session_id, _) = process
        .runtime
        .as_ref()
        .and_then(|runtime| runtime.session_attachment())
        .ok_or_else(|| anyhow::anyhow!("the run has no Session"))?;
    let store = crate::store::open_sqlite(&crate::store::database_path_from_env()?)?;
    let session = store
        .session(&session_id)?
        .ok_or_else(|| anyhow::anyhow!("Session {session_id} is not recorded"))?;
    crate::ops::human_session::conversation_log::continuation_of_work(&store, &session)
}

/// The provider's own words for a failure, when it gave any.
fn _failure_reason(result: &AgentProcessResult, failure: AgentFailure) -> String {
    let reason = result.stderr.trim();
    if reason.is_empty() {
        failure.to_string()
    } else {
        reason.to_owned()
    }
}

/// A provider error that sends the run to another agent: a revoked login, a
/// spent quota or a model the provider does not serve. The decoder's code
/// decides when it names a cause; otherwise the provider's words do.
pub(crate) fn routing_failure(code: &str, message: &str) -> Option<AgentFailure> {
    _failure(code, message, _routing_words)
}

/// [`routing_failure`], then the transient failures lf retries for a provider
/// that does not retry them itself.
pub(crate) fn provider_failure(code: &str, message: &str) -> Option<AgentFailure> {
    _failure(code, message, |line| {
        _routing_words(line).or_else(|| classify_retryable_agent_failure(line))
    })
}

/// What `code` names, else the gravest failure any line of the message
/// states. A phrase list reads one line: "model" and "is not available" on
/// different lines name nothing.
fn _failure(
    code: &str,
    message: &str,
    words: impl Fn(&str) -> Option<AgentFailure>,
) -> Option<AgentFailure> {
    AgentFailure::from_code(code).or_else(|| {
        message
            .lines()
            .filter_map(words)
            .reduce(AgentFailure::graver)
    })
}

fn _routing_words(line: &str) -> Option<AgentFailure> {
    if credential_invalidated_failure(line).is_some() {
        Some(AgentFailure::CredentialInvalidated)
    } else if subscription_limit_text(line).is_some() {
        Some(AgentFailure::UsageLimit)
    } else if _model_unavailable(line).is_some() {
        Some(AgentFailure::ModelUnavailable)
    } else {
        None
    }
}

/// A provider's line as the phrase lists read it: lowercase, with typographic
/// apostrophes (Codex writes "You\u{2019}ve") as ASCII.
fn _provider_words(line: &str) -> String {
    line.to_ascii_lowercase()
        .replace(['\u{2018}', '\u{2019}'], "'")
}

fn _model_unavailable(text: &str) -> Option<()> {
    let text = _provider_words(text);
    let names_model = text.contains("model")
        || crate::config::CLAUDE_MODEL_FAMILIES
            .iter()
            .any(|model| text.contains(model));
    (text.contains("model_not_found")
        || (names_model
            && [
                "is unavailable",
                "is currently unavailable",
                "is not available",
                "is not supported",
            ]
            .iter()
            .any(|phrase| text.contains(phrase))))
    .then_some(())
}

pub(crate) fn credential_invalidated_failure(text: &str) -> Option<()> {
    let text = _provider_words(text);
    (text.contains("token_invalidated")
        || text.contains("refresh token was revoked")
        || text.contains("access token could not be refreshed")
        || text.contains("your session has ended. please log in again"))
    .then_some(())
}

pub(crate) fn classify_retryable_agent_failure(text: &str) -> Option<AgentFailure> {
    let text = _provider_words(text);
    if text.contains("at capacity")
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

/// A provider's line saying the account's quota is spent, for a failure the
/// provider gave no type. Claude's openers are its own list of usage-limit
/// messages; its capacity, per-model and seat-type lines name no spent window.
pub(crate) fn subscription_limit_text(line: &str) -> Option<()> {
    let text = _provider_words(line);
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
    .then_some(())
}

fn _begin_process_runtime(
    launch: &AgentConfig,
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
        surface: "headless".to_string(),
        cwd: cwd.clone(),
        repo: Some(cwd.clone()),
        worktree: Some(cwd),
        skill: None,
        subjects: Vec::new(),
        graph: crate::session_record::SessionGraphMembership::Independent,
        work: None,
    };
    let runtime = crate::session_record::SessionRecorder::begin_with_request(
        spec,
        crate::session_record::AgentProcessRequest::from_prepared(launch)?,
    )
    .map(crate::process::LfProcessRuntime::new);
    runtime.map_err(|error| {
        CoreError::ExecutionFailed(format!(
            "failed to publish Session capture manifest before agent launch: {error}"
        ))
    })
}

fn _run_harness_once(
    launch: &AgentConfig,
    process: &ProcessConfig,
    kind: crate::harness::LaunchKind,
    retry: bool,
) -> Result<AgentProcessResult, CoreError> {
    if tokio::runtime::Handle::try_current().is_ok() {
        let launch = launch.clone();
        let process = process.clone();
        return std::thread::Builder::new()
            .name("lf-native-harness".to_string())
            .spawn(move || _run_harness_once(&launch, &process, kind, retry))
            .map_err(|error| CoreError::ExecutionFailed(error.to_string()))?
            .join()
            .map_err(|_| {
                CoreError::ExecutionFailed("Native harness thread panicked".to_string())
            })?;
    }

    let runtime = process.runtime.as_ref();
    let provider = launch.agent.harness.as_str();
    let model = launch.agent.model.clone();
    _note_limit_recovery(|store, process| store.end_agent_limit_wait(process));
    if retry {
        if let Some(runtime) = runtime {
            runtime
                .retry_agent_process(provider.to_owned(), model, launch.resume_token.as_ref())
                .map_err(|error| CoreError::ExecutionFailed(error.to_string()))?;
        }
    }

    let mut config = launch.clone();
    if let Some(runtime) = runtime {
        // A new harness gets the invocation owner's current snapshot. Existing
        // dispatch/history operations never refresh theirs from the runtime.
        config.env.extend(runtime.environment());
    }
    let recording = crate::harness::Recording::from_runtime(
        runtime.expect("run_admitted_agent supplies a runtime before retrying"),
    )
    .map_err(|error| CoreError::ExecutionFailed(error.to_string()))?;
    let value = crate::harness::Launch {
        config: &config,
        harness: crate::harness::launcher(launch.agent.harness),
        kind,
        recording: &recording,
        start: crate::harness::Start::from_thread(launch.resume_token.as_ref()),
        context_file: None,
    };
    crate::harness::drive::run(&value, process)
}

/// What to tell someone whose agent CLI is missing.
pub fn missing_agent_message(kind: loopflow_harness::HarnessKind) -> String {
    let hint = crate::harness::launcher(kind).capabilities().install_hint;
    format!("{}. {hint}", _not_installed(kind))
}

fn _not_installed(kind: loopflow_harness::HarnessKind) -> String {
    format!("'{}' is not installed", kind.as_str())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn default_launch() -> AgentConfig {
        AgentConfig {
            task_prompt: "task".to_string(),
            ..crate::agent::AgentConfig::default()
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
        let _lock = crate::journal::test_env_lock();
        let _env = crate::test_ambient::EnvGuard::clear(&["LF_BIN"]);
        // OpenCode's and Pi's context extensions call back into this checkout's lf.
        std::env::set_var("LF_BIN", std::env::current_exe().unwrap());
        let home = tempfile::tempdir().unwrap();
        let admitted = crate::harness::admit_for_test(home.path());
        let launch = crate::harness::launch_for_test(config.agent.harness, config, &admitted);
        launch
            .harness
            .open(&launch)
            .unwrap()
            .process
            .get_args()
            .map(|arg| arg.to_string_lossy().into_owned())
            .collect()
    }

    /// A Graph step: it moves through its agents and waits for capacity.
    fn graph_process() -> ProcessConfig {
        ProcessConfig {
            wait_for_capacity: true,
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
    fn permissions_are_skipped_unless_yolo_is_off_and_someone_attends() {
        for (yolo, mode, skips) in [
            (true, Mode::Interactive, true),
            (true, Mode::Headless, true),
            (false, Mode::Interactive, false),
            (false, Mode::Headless, true),
        ] {
            let launch = AgentConfig {
                yolo,
                ..default_launch()
            };
            let claude = claude_options(&launch, None, None, mode);
            assert_eq!(
                claude.contains(&"--dangerously-skip-permissions".into()),
                skips,
                "claude, yolo {yolo}, {mode:?}"
            );
            let codex = build_codex_thread_start_params(&launch, mode);
            assert_eq!(
                (
                    codex.get("sandbox").and_then(serde_json::Value::as_str),
                    codex
                        .get("approvalPolicy")
                        .and_then(serde_json::Value::as_str),
                ),
                if skips {
                    (Some("danger-full-access"), Some("never"))
                } else {
                    (None, None)
                },
                "codex, yolo {yolo}, {mode:?}"
            );
        }
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
    fn terminal_commands_keep_native_context_and_main_repo_scope() {
        let (_tmp, main, _) = git_worktree_fixture();
        for provider in ["claude", "codex"] {
            let launch = AgentConfig {
                agent: crate::config::Agent::parse(provider).unwrap(),
                cwd: Some(main.clone()),
                system_prompt: "Fixed additions.".into(),
                ..default_launch()
            };
            let args = terminal_args(&launch);
            assert!(!args.contains(&"--add-dir".into()));
            match provider {
                "claude" => assert_arg_pair(&args, "--append-system-prompt", "Fixed additions."),
                "codex" => {
                    let params = build_codex_thread_start_params(&launch, Mode::Headless);
                    assert_eq!(params["developerInstructions"], "Fixed additions.");
                }
                _ => {}
            }
        }
    }

    #[test]
    fn build_codex_thread_start_params_with_variant_and_cwd() {
        let launch = AgentConfig {
            agent: crate::config::Agent::parse("codex:o3").unwrap(),
            cwd: Some("/tmp/repo".into()),
            ..default_launch()
        };

        let params = build_codex_thread_start_params(&launch, Mode::Headless);
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
            agent: crate::config::Agent::parse("codex").unwrap(),
            cwd: Some("/tmp/repo".into()),
            ..default_launch()
        };

        let params = build_codex_thread_start_params(&launch, Mode::Headless);
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
            agent: crate::config::Agent::parse("claude:sonnet").unwrap(),
            yolo: true,
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
        let args =
            crate::harness::claude::stream_args(&config, Some(&"sess_abc".into()), Some(&path))
                .unwrap();
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
    fn bare_claude_command_does_not_select_a_model() {
        let launch = AgentConfig {
            agent: crate::config::Agent::parse("claude").unwrap(),
            ..default_launch()
        };
        let cmd = terminal_args(&launch);
        assert!(
            !cmd.iter()
                .any(|arg| arg == "--model" || arg.starts_with("model=")),
            "bare claude selected a model: {cmd:?}"
        );
    }

    #[test]
    fn bare_headless_uses_the_configured_fallback_order() {
        let launch = AgentConfig {
            agent: crate::config::Agent::parse("claude:fable").unwrap(),
            fallback_agents: vec![crate::config::Agent::parse("codex").unwrap()],
            task_prompt: "finish the operation".into(),
            ..crate::agent::AgentConfig::default()
        };
        let mut attempts = Vec::new();
        let result = _run_with_retries(
            &launch,
            &graph_process(),
            &[],
            |config, _| {
                attempts.push(config.agent.to_string());
                Ok(if config.agent.to_string() == "codex" {
                    AgentProcessResult::default()
                } else {
                    failed_as(
                        "claude_result: You've hit your Fable limit",
                        AgentFailure::UsageLimit,
                    )
                })
            },
            |_| {},
        )
        .unwrap();
        assert_eq!(result.exit_code, 0);
        assert_eq!(attempts, ["claude:fable", "codex"]);
    }

    fn failed_result(message: &str) -> AgentProcessResult {
        AgentProcessResult {
            exit_code: 1,
            stderr: message.to_string(),
            ..Default::default()
        }
    }

    /// A failed run whose harness classified the provider's error.
    fn failed_as(message: &str, failure: AgentFailure) -> AgentProcessResult {
        AgentProcessResult {
            failure: Some(failure),
            ..failed_result(message)
        }
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
    fn model_unavailable_is_a_provider_refusal_not_spent_quota() {
        for (code, message) in [
            (
                "claude_result",
                "Fable 5 is currently unavailable. Switch to another model.",
            ),
            ("claude_result", "Selected model is not available."),
            ("codex_error", "model_not_found"),
            (
                "codex_error",
                "The 'gpt-6.2-sol' model is not supported when using Codex with a ChatGPT account.",
            ),
        ] {
            assert_eq!(
                provider_failure(code, message),
                Some(AgentFailure::ModelUnavailable)
            );
            assert!(subscription_limit_text(message).is_none());
        }
        for message in [
            "service unavailable",
            "model service temporarily unavailable",
        ] {
            assert_eq!(
                provider_failure("claude_result", message),
                Some(AgentFailure::Unavailable)
            );
            // A provider that retries its own transient failures leaves lf none.
            assert_eq!(routing_failure("pi_error", message), None);
        }
    }

    #[test]
    fn a_code_that_names_the_cause_decides_and_the_gravest_line_decides_otherwise() {
        assert_eq!(
            provider_failure("provider_rate_limited", "service unavailable"),
            Some(AgentFailure::UsageLimit)
        );
        assert_eq!(
            provider_failure(
                "codex_error",
                "service unavailable\nYour refresh token was revoked\nquota exceeded"
            ),
            Some(AgentFailure::CredentialInvalidated)
        );
        // "model" and "is not available" on different lines name nothing.
        assert_eq!(
            provider_failure("codex_error", "Selected model:\nthe tool is not available"),
            None
        );
        assert_eq!(
            AgentFailure::Unavailable.graver(AgentFailure::ModelUnavailable),
            AgentFailure::ModelUnavailable
        );
        assert_eq!(
            AgentFailure::Capacity.graver(AgentFailure::Transport),
            AgentFailure::Capacity
        );
    }

    #[test]
    fn model_unavailable_skips_logins_and_keeps_the_configured_order() {
        let launch = AgentConfig {
            agent: crate::config::Agent::parse("claude:fable").unwrap(),
            fallback_agents: vec![
                crate::config::Agent::parse("claude:opus").unwrap(),
                crate::config::Agent::parse("codex").unwrap(),
            ],
            task_prompt: "finish the operation".into(),
            ..crate::agent::AgentConfig::default()
        };
        let mut attempts = Vec::new();
        let result = _run_with_retries(
            &launch,
            &graph_process(),
            &[],
            |config, _| {
                attempts.push(config.clone());
                Ok(if config.agent.to_string() == "codex" {
                    done()
                } else {
                    AgentProcessResult {
                        agent_session: Some("thread".into()),
                        ..failed_as(
                            "claude_result: Selected model is unavailable",
                            AgentFailure::ModelUnavailable,
                        )
                    }
                })
            },
            |delay| assert_eq!(delay, Duration::ZERO),
        )
        .unwrap();
        assert_eq!(result.exit_code, 0);
        assert_eq!(
            attempts
                .iter()
                .map(|c| c.agent.to_string())
                .collect::<Vec<_>>(),
            ["claude:fable", "claude:opus", "codex"]
        );
        assert_eq!(attempts[1].resume_token, Some("thread".into()));
        assert_eq!(attempts[1].task_prompt, MODEL_FAILOVER_PROMPT);
        assert_eq!(attempts[2].resume_token, None);
        assert!(attempts[2].task_prompt.ends_with("finish the operation"));
        assert!(!attempts[2].task_prompt.contains("usage limit"));
    }

    #[test]
    fn transient_capacity_failure_resumes_and_finishes() {
        let launch = AgentConfig {
            agent: crate::config::Agent::parse("codex").unwrap(),
            task_prompt: "compress the branch".to_string(),
            ..crate::agent::AgentConfig::default()
        };
        let process = graph_process();
        let mut results = vec![
            AgentProcessResult {
                exit_code: 1,
                stdout: String::new(),
                stderr: "Selected model is at capacity. Please try a different model.".into(),
                agent_session: Some("thread-123".into()),
                failure: Some(AgentFailure::Capacity),
            },
            AgentProcessResult {
                exit_code: 0,
                ..Default::default()
            },
        ]
        .into_iter();
        let mut attempts = Vec::new();
        let mut waits = Vec::new();

        let result = _run_with_retries(
            &launch,
            &process,
            &[Duration::ZERO],
            |config, retry| {
                assert_eq!(retry, !attempts.is_empty());
                attempts.push(config.clone());
                Ok(results.next().expect("scripted launch result"))
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
    fn a_retry_with_no_native_thread_repeats_the_input_whatever_the_answer_printed() {
        let launch = default_launch();
        let result = AgentProcessResult {
            stdout: "{\"type\":\"system\",\"session_id\":\"quoted-session\"}\n".into(),
            ..failed_as("service unavailable", AgentFailure::Unavailable)
        };
        let mut attempts = 0;
        _run_with_retries(
            &launch,
            &graph_process(),
            &[Duration::ZERO],
            |config, _| {
                attempts += 1;
                assert_eq!(config.resume_token, None);
                assert_eq!(config.task_prompt, launch.task_prompt);
                Ok(result.clone())
            },
            |_| {},
        )
        .unwrap();
        assert_eq!(attempts, 2);
    }

    fn limited_result() -> AgentProcessResult {
        AgentProcessResult {
            exit_code: 1,
            agent_session: Some("thread".into()),
            failure: Some(AgentFailure::UsageLimit),
            ..failed_result("provider_rate_limited: rejected: seven_day_overage_included")
        }
    }

    fn done() -> AgentProcessResult {
        AgentProcessResult {
            exit_code: 0,
            ..Default::default()
        }
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

    /// LOO-974's `implement` step, 2026-11-18, as Codex reported it.
    #[test]
    fn codex_usage_limit_with_a_typographic_apostrophe_is_a_usage_limit() {
        let message = "You\u{2019}ve hit your usage limit. Upgrade to Pro (https://chatgpt.com/explore/pro), visit https://chatgpt.com/settings/usage to purchase more credits or try again at 3:36 PM.";
        assert_eq!(
            provider_failure("codex_error", message),
            Some(AgentFailure::UsageLimit)
        );
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
    fn permanent_agent_failure_is_not_retried() {
        let launch = AgentConfig {
            agent: crate::config::Agent::parse("codex").unwrap(),
            ..default_launch()
        };
        let process = graph_process();
        let mut attempts = 0;

        let result = _run_with_retries(
            &launch,
            &process,
            &[Duration::ZERO],
            |_, _| {
                attempts += 1;
                Ok(failed_result("invalid request"))
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
            agent: crate::config::Agent::parse("codex").unwrap(),
            ..default_launch()
        };
        let process = graph_process();
        let mut attempts = 0;
        let mut waits = 0;

        let result = _run_with_retries(
            &launch,
            &process,
            &[Duration::ZERO, Duration::ZERO],
            |_, _| {
                attempts += 1;
                let mut result = failed_as("service unavailable", AgentFailure::Unavailable);
                result.exit_code = attempts;
                Ok(result)
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
            agent: crate::config::Agent::parse("claude:opus").unwrap(),
            resume_token: Some("session-123".into()),
            ..default_launch()
        };
        let admitted = crate::harness::admit_for_test(tempfile::tempdir().unwrap().path());
        let value = crate::harness::Launch {
            start: crate::harness::Start::Resume {
                thread: launch.resume_token.as_ref().unwrap(),
            },
            ..crate::harness::launch_for_test(
                loopflow_harness::HarnessKind::Claude,
                &launch,
                &admitted,
            )
        };
        let command = value.harness.open(&value).unwrap();
        let claude = command
            .process
            .get_args()
            .map(|arg| arg.to_string_lossy().into_owned())
            .collect::<Vec<_>>();
        assert_arg_pair(&claude, "--resume", "session-123");
    }

    #[test]
    fn saved_claude_model_reaches_terminal_stream_and_recording() {
        for (agent, saved, expected) in [
            ("claude", None, None),
            ("claude", Some("opus"), Some("opus")),
            ("claude:fable", Some("opus"), Some("fable")),
        ] {
            let mut config = crate::config::Config::default();
            config.claude.model = saved.map(str::to_owned);
            let launch = AgentConfig {
                agent: crate::config::Agent::parse(agent)
                    .unwrap()
                    .with_configured_model(&config),
                yolo: true,
                ..default_launch()
            };
            for args in [
                terminal_args(&launch),
                crate::harness::claude::stream_args(&launch, None, None).unwrap(),
            ] {
                let model = args
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
            agent: crate::config::Agent::parse("codex").unwrap(),
            ..default_launch()
        };
        assert_eq!(codex.agent.to_string(), "codex");
    }

    #[test]
    fn a_launch_refused_by_a_usage_limit_moves_to_the_next_agent() {
        let launch = AgentConfig {
            agent: crate::config::Agent::parse("codex").unwrap(),
            fallback_agents: vec![
                crate::config::Agent::parse("claude:fable").unwrap(),
                crate::config::Agent::parse("claude:opus").unwrap(),
            ],
            task_prompt: "compress the diff".into(),
            ..crate::agent::AgentConfig::default()
        };
        let refused = || {
            failed_as(
                "codex_error: You’ve hit your usage limit",
                AgentFailure::UsageLimit,
            )
        };
        let mut scripted = vec![refused(), limited_result(), done()].into_iter();
        let mut attempts = Vec::new();
        let result = _run_with_retries(
            &launch,
            &graph_process(),
            &[],
            |config, _| {
                attempts.push(config.clone());
                Ok(scripted.next().expect("scripted attempt"))
            },
            |_| {},
        )
        .unwrap();
        assert_eq!(result.exit_code, 0);
        let agents: Vec<_> = attempts.iter().map(|c| c.agent.to_string()).collect();
        assert_eq!(agents, ["codex", "claude:fable", "claude:opus"]);
        // Another provider starts on the whole task; another model of the
        // same provider keeps the thread.
        assert!(attempts[1].task_prompt.ends_with("compress the diff"));
        assert_eq!(attempts[1].resume_token, None);
        assert_eq!(attempts[2].resume_token, Some("thread".into()));
        assert_eq!(attempts[2].task_prompt, LIMIT_FAILOVER_PROMPT);
    }

    #[test]
    fn a_pinned_agent_refused_on_every_try_stops_after_the_last_wait() {
        let launch = AgentConfig {
            agent: crate::config::Agent::parse("codex").unwrap(),
            ..crate::agent::AgentConfig::default()
        };
        let mut attempts = 0;
        let mut waits = Vec::new();
        let result = _run_with_retries(
            &launch,
            &graph_process(),
            &[],
            |_, _| {
                attempts += 1;
                Ok(failed_as(
                    "codex_error: You’ve hit your usage limit",
                    AgentFailure::UsageLimit,
                ))
            },
            |delay| waits.push(delay),
        )
        .unwrap();
        assert_eq!(attempts, 5);
        assert_eq!(waits, USAGE_LIMIT_WAITS);
        assert_eq!(result.failure, Some(AgentFailure::UsageLimit));
        assert!(result
            .stderr
            .starts_with("codex_error: You’ve hit your usage limit\nNo fallback remains."));
        // What it records names the way out; with an order, what was tried.
        let advice = _limit_stop_advice("codex", &launch, &[Agent::parse("codex").unwrap()]);
        assert_eq!(
            advice,
            "No fallback remains. Change an `agent` or `-a` pin to select another model or provider, or remove it to allow fallbacks. If `agents` is missing or has only one entry, configure fallbacks with `lf set --global agents codex,claude`"
        );
        let ordered = AgentConfig {
            fallback_agents: vec![crate::config::Agent::parse("claude").unwrap()],
            ..launch
        };
        assert_eq!(
            _limit_stop_advice(
                "claude",
                &ordered,
                &[
                    Agent::parse("codex").unwrap(),
                    Agent::parse("claude").unwrap()
                ]
            ),
            "Nothing is left in its order after codex, claude; add another agent to `agents`"
        );
        // A stopped headless launch explains the configured order it exhausted.
        assert_eq!(
            _limit_stop_advice("codex", &ordered, &[Agent::parse("codex").unwrap()]),
            "Nothing is left in its order after codex; add another agent to `agents`"
        );
    }
    #[test]
    fn a_usage_limit_that_clears_during_a_wait_continues_the_thread() {
        let launch = AgentConfig {
            agent: crate::config::Agent::parse("claude:opus").unwrap(),
            task_prompt: "compress the diff".into(),
            ..crate::agent::AgentConfig::default()
        };
        let mut scripted = vec![limited_result(), done()].into_iter();
        let mut attempts = Vec::new();
        let mut waits = Vec::new();
        let result = _run_with_retries(
            &launch,
            &graph_process(),
            &[],
            |config, _| {
                attempts.push(config.clone());
                Ok(scripted.next().expect("scripted attempt"))
            },
            |delay| waits.push(delay),
        )
        .unwrap();
        assert_eq!(result.exit_code, 0);
        assert_eq!(waits, [Duration::from_secs(5 * 60)]);
        assert_eq!(attempts.len(), 2);
        assert_eq!(attempts[1].agent.to_string(), "claude:opus");
        assert_eq!(attempts[1].resume_token, Some("thread".into()));
        assert_eq!(attempts[1].task_prompt, LIMIT_FAILOVER_PROMPT);
    }

    #[test]
    fn headless_work_outside_a_graph_moves_through_its_order_and_never_waits() {
        // `lf commit` and `lf pr submit` message generation: someone is waiting.
        let launch = AgentConfig {
            agent: crate::config::Agent::parse("codex").unwrap(),
            fallback_agents: vec![crate::config::Agent::parse("claude").unwrap()],
            task_prompt: "describe the diff".into(),
            skill_invocation: Some(crate::skills::invocation::SkillInvocation {
                skill: crate::graph::Skill::named("describe"),
                arguments: "this diff".into(),
            }),
            ..crate::agent::AgentConfig::default()
        };
        let refused = || {
            failed_as(
                "codex_error: You’ve hit your usage limit",
                AgentFailure::UsageLimit,
            )
        };
        for (scripted, agents, failure) in [
            (vec![refused(), done()], vec!["codex", "claude"], None),
            (
                vec![refused(), limited_result()],
                vec!["codex", "claude"],
                Some(AgentFailure::UsageLimit),
            ),
        ] {
            let mut scripted = scripted.into_iter();
            let mut attempts = Vec::new();
            let result = _run_with_retries(
                &launch,
                &ProcessConfig::default(),
                &[],
                |config, _| {
                    assert_eq!(config.skill_invocation, launch.skill_invocation);
                    assert!(config.task_prompt.contains(&launch.task_prompt));
                    attempts.push(config.agent.to_string());
                    Ok(scripted.next().expect("scripted attempt"))
                },
                |delay| assert_eq!(delay, Duration::ZERO, "a command never waits"),
            )
            .unwrap();
            assert_eq!(attempts, agents);
            assert_eq!(result.failure, failure);
            if failure.is_some() {
                assert!(
                    result.stderr.ends_with(
                        "Nothing is left in its order after codex, claude; add another agent to `agents`"
                    ),
                    "{}",
                    result.stderr
                );
            }
        }
    }

    #[test]
    fn a_revoked_login_skips_its_providers_models_and_says_to_sign_in() {
        let revoked = || AgentProcessResult {
            failure: Some(AgentFailure::CredentialInvalidated),
            ..failed_result("claude: Your session has ended. Please log in again.")
        };
        let pinned = AgentConfig {
            agent: crate::config::Agent::parse("claude").unwrap(),
            ..crate::agent::AgentConfig::default()
        };
        let mut attempts = 0;
        let mut waits = Vec::new();
        let result = _run_with_retries(
            &pinned,
            &graph_process(),
            &[],
            |_, _| {
                attempts += 1;
                Ok(revoked())
            },
            |delay| waits.push(delay),
        )
        .unwrap();
        // Claude's other models share the dead login: one try, no wait.
        assert_eq!(attempts, 1);
        assert!(waits.is_empty());
        assert!(
            result
                .stderr
                .ends_with("The claude login was revoked; sign in again on this Machine: run `claude`, then /login"),
            "{}",
            result.stderr
        );

        // Another provider in the order goes on.
        let ordered = AgentConfig {
            fallback_agents: vec![
                crate::config::Agent::parse("claude:fable").unwrap(),
                crate::config::Agent::parse("codex").unwrap(),
            ],
            ..pinned
        };
        let mut scripted = vec![revoked(), done()].into_iter();
        let mut agents = Vec::new();
        let result = _run_with_retries(
            &ordered,
            &graph_process(),
            &[],
            |config, _| {
                agents.push(config.agent.to_string());
                Ok(scripted.next().expect("scripted attempt"))
            },
            |_| {},
        )
        .unwrap();
        assert_eq!(result.exit_code, 0);
        assert_eq!(agents, ["claude", "codex"]);
    }

    #[test]
    fn native_retry_policy_matrix() {
        let directory = tempfile::tempdir().unwrap();
        let runtime = tokio::runtime::Runtime::new().unwrap();
        let store = std::sync::Arc::new(
            runtime
                .block_on(crate::store::open_ephemeral_store(
                    &crate::store::StorageConfig::sqlite(directory.path().join("store.db")),
                ))
                .unwrap(),
        );
        let now = time::OffsetDateTime::now_utc();
        let task = crate::work::task::Task {
            parent_task_id: None,
            id: crate::durable::TaskId::new(),
            revision: 0,
            identifier: "LOO-1".into(),
            title: "Continue work".into(),
            description: String::new(),
            rank: 0,
            assignee: None,
            due_date: None,
            state: None,
            completed_at: None,
            tracker_id: None,
            tracker_synced_at: None,
            tracker_writeback: crate::work::task::LinearWritebackState::Current,
            wave_id: None,
            repository_id: crate::durable::RepositoryId::new(),
            project_id: None,
            worktree: Some(directory.path().into()),
            workspace_slug: "continue".into(),
            branch: "continue".into(),
            base_commit: "fixture".into(),
            parent_pr_id: None,
            created_at: now,
            updated_at: now,
            observation: crate::work::task::Observation::NotRequired,
        };
        for task_bound in [false, true] {
            for pinned in [false, true] {
                let launch = AgentConfig {
                    agent: crate::config::Agent::parse("claude:fable").unwrap(),
                    fallback_agents: if pinned {
                        vec![]
                    } else {
                        vec![
                            Agent::parse("claude:opus").unwrap(),
                            Agent::parse("codex").unwrap(),
                        ]
                    },
                    task_prompt: "finish the operation".into(),
                    resume_token: Some("native-thread".into()),
                    ..crate::agent::AgentConfig::default()
                };
                // Headless here is a Graph step; other headless work
                // moves the same way and never waits (its own test).
                let process = ProcessConfig {
                    wait_for_capacity: true,
                    task_input: task_bound.then(|| {
                        crate::ops::task_input::TaskInput::new(
                            store.clone(),
                            crate::ops::task_input::TaskSeed {
                                task: task.clone(),
                                message: String::new(),
                                interrupt: 0,
                            },
                        )
                    }),
                    ..Default::default()
                };
                for failure in [
                    AgentFailure::UsageLimit,
                    AgentFailure::CredentialInvalidated,
                    AgentFailure::ModelUnavailable,
                ] {
                    let mut attempts = Vec::new();
                    let mut waits = Vec::new();
                    let result = _run_with_retries(
                        &launch,
                        &process,
                        &[],
                        |config, _| {
                            attempts.push(config.clone());
                            Ok(AgentProcessResult {
                                exit_code: 1,
                                failure: Some(failure),
                                agent_session: Some("native-thread".into()),
                                stderr: "provider refused the current turn".into(),
                                ..Default::default()
                            })
                        },
                        |delay| waits.extend(Some(delay).filter(|delay| !delay.is_zero())),
                    )
                    .unwrap();
                    assert_eq!(result.failure, Some(failure));
                    // Only a headless usage limit waits: 5, 15, 30 and 60
                    // minutes, each followed by the whole order again.
                    let backoff = failure == AgentFailure::UsageLimit;
                    assert_eq!(
                        waits,
                        if backoff {
                            USAGE_LIMIT_WAITS.to_vec()
                        } else {
                            vec![]
                        }
                    );
                    // A revoked login skips its provider's other models.
                    let order = match (!pinned, failure) {
                        (false, _) => 1,
                        (true, AgentFailure::CredentialInvalidated) => 2,
                        (true, _) => 3,
                    };
                    let tries = if backoff {
                        1 + USAGE_LIMIT_WAITS.len()
                    } else {
                        1
                    };
                    assert_eq!(attempts.len(), order * tries);
                    if backoff && !pinned {
                        // After a wait the order starts again at its first agent.
                        assert_eq!(attempts[3].agent.to_string(), "claude:fable");
                        assert!(attempts[3].resume_token.is_none());
                        assert!(attempts[3].task_prompt.contains("finish the operation"));
                    }
                    assert_eq!(
                        attempts[0]
                            .resume_token
                            .as_ref()
                            .map(AgentSessionId::as_str),
                        Some("native-thread")
                    );
                    if attempts.len() == 3 {
                        assert_eq!(
                            attempts[1]
                                .resume_token
                                .as_ref()
                                .map(AgentSessionId::as_str),
                            Some("native-thread")
                        );
                        assert_eq!(attempts[2].agent.to_string(), "codex");
                        assert!(attempts[2].resume_token.is_none());
                        assert!(attempts[2].task_prompt.contains("finish the operation"));
                    }
                    if order == 2 {
                        assert_eq!(attempts[1].agent.to_string(), "codex");
                    }
                }
                let mut dispatches = 0;
                let success = _run_with_retries(
                    &launch,
                    &process,
                    &[],
                    |_, _| {
                        dispatches += 1;
                        Ok(AgentProcessResult {
                            stdout:
                                "You've hit your usage limit is an example error, not this answer."
                                    .into(),
                            ..Default::default()
                        })
                    },
                    |_| panic!("a successful answer never retries"),
                )
                .unwrap();
                assert_eq!((dispatches, success.exit_code), (1, 0));
            }
        }
    }
}
