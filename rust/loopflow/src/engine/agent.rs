//! Agent invocation for spawning coding agent runners (Claude, Codex, Gemini, OpenCode).
//!
//! This module handles building commands and spawning subprocesses for each
//! supported coding agent. Output can be captured or streamed.

use std::io::{BufRead, BufReader};
use std::process::{Command, Stdio};
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::{mpsc, Mutex, OnceLock};
use std::thread;
use std::time::Instant;

use crate::engine::config::parse_model;
use crate::engine::error::CoreError;
use crate::engine::platform::kill_process;
use crate::engine::stream::{format_event, ParseResult, StreamFormat, StreamParser};
use crate::engine::structured_reply::{render_structured_reply_guidance, StructuredReply};

/// PID of the current child agent process. The Ctrl+C handler sends SIGTERM
/// to this process before exiting so the agent doesn't survive as an orphan.
static CHILD_PID: AtomicU32 = AtomicU32::new(0);
const RLM_PASSTHROUGH_VARS: &[&str] = &["RLM_MAX_DEPTH", "RLM_MAX_PARALLEL", "RLM_MODEL"];

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

/// Result from launching a runner.
#[derive(Debug, Clone, Default)]
pub struct LaunchResult {
    pub exit_code: i32,
    pub stdout: String,
    pub stderr: String,
}

/// Configuration for launching an agent.
#[derive(Debug, Clone, Default)]
pub struct AgentConfig {
    /// System/context prompt content.
    pub system_prompt: String,
    /// Task prompt content sent as the turn input.
    pub task_prompt: String,
    /// Model string (for example: "claude:opus" or "codex").
    pub model: Option<String>,
    /// Max turn budget when supported by the harness.
    pub max_turns: Option<u32>,
    /// Working directory.
    pub cwd: Option<std::path::PathBuf>,
    /// Skip permission prompts
    pub skip_permissions: bool,
    /// Engine-injected structured replies (rendered via harness prompt guidance).
    pub structured_replies: Vec<StructuredReply>,
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
}

/// Agent capability flags.
#[derive(Debug, Clone, Default)]
pub struct AgentCapabilities {
    /// Enable Chrome integration (Claude only).
    pub chrome: bool,
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
    /// Skip permission prompts.
    pub skip_permissions: bool,
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
        let (backend, variant) = parse_model(model);
        if backend == "claude" {
            variant
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

        if self.skip_permissions {
            args.push("--dangerously-skip-permissions".to_string());
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
        model: config.model.as_deref().and_then(ClaudeArgs::resolve_model),
        system_prompt: Some(system_prompt_with_structured_replies(config)),
        system_prompt_file: None,
        skip_permissions: config.skip_permissions,
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
    let (backend, variant) = parse_model(model);
    if backend == "codex" {
        variant
    } else {
        Some(model.to_string())
    }
}

/// Build `thread/start` params for Codex app-server sessions.
///
/// Mirrors model/cwd mapping used by one-shot Codex launches so session and
/// non-session paths stay in sync.
pub fn build_codex_thread_start_params(
    launch: &AgentConfig,
) -> serde_json::Map<String, serde_json::Value> {
    let mut params = serde_json::Map::new();

    if let Some(model) = launch.model.as_deref().and_then(resolve_codex_model) {
        params.insert("model".to_string(), serde_json::Value::String(model));
    }

    if let Some(cwd) = launch.cwd.as_ref() {
        params.insert(
            "cwd".to_string(),
            serde_json::Value::String(cwd.to_string_lossy().to_string()),
        );
    }

    params
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
        skip_permissions: process.auto || launch.skip_permissions,
        max_turns: launch.max_turns,
        stream: process.auto && process.stream,
        chrome: capabilities.chrome,
        resume_id: None,
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
        vec!["codex".to_string(), "exec".to_string()]
    } else {
        vec!["codex".to_string()]
    };

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

    if process.stream && matches!(process.stream_format, StreamFormat::Raw) {
        cmd.push("--json".to_string());
    }

    if launch.skip_permissions {
        cmd.push("--dangerously-bypass-approvals-and-sandbox".to_string());
    } else {
        cmd.push("--sandbox".to_string());
        cmd.push("workspace-write".to_string());

        // Non-interactive runs should never block on approval prompts.
        if process.auto {
            cmd.push("--ask-for-approval".to_string());
            cmd.push("never".to_string());
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
    let mut oc_config = serde_json::Map::new();
    if process.auto {
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

/// Apply harness-specific environment variables to a command.
fn apply_harness_env(harness: &str, cmd: &mut Command, process: &ProcessConfig) {
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
            if let Some(env_val) = build_opencode_env(process) {
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
    let model = launch.model.as_deref().unwrap_or("claude");
    let (harness, variant) = parse_model(model);
    let variant = variant.as_deref();
    match harness.as_str() {
        "codex" => build_codex_command(launch, process, variant),
        "gemini" => build_gemini_command(launch, process, variant),
        "opencode" => build_opencode_command(process, variant),
        "claude" => build_claude_command(launch, process, capabilities, variant),
        // Unknown harness: fall back to Claude with the full model string as variant.
        _ => build_claude_command(launch, process, capabilities, Some(model)),
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
    let start = Instant::now();
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

    // Shell integration sets LOOPFLOW_DIRECTIVE_FILE so top-level `lf` commands
    // can request parent-shell actions (for example auto-cd after `lf ops wt create`).
    // Agent sessions run arbitrary nested commands; those must not mutate the
    // invoking shell state via the top-level directive file.
    cmd.env_remove("LOOPFLOW_DIRECTIVE_FILE");

    // Never forward API keys to agent subprocesses. Claude Code, Codex, etc.
    // should use their own auth (subscription/OAuth). A stray ANTHROPIC_API_KEY
    // in the shell causes Claude Code to silently bill the API instead.
    cmd.env_remove("ANTHROPIC_API_KEY");
    cmd.env_remove("OPENAI_API_KEY");
    cmd.env_remove("GEMINI_API_KEY");

    // Harness-specific environment setup.
    let model = launch.model.as_deref().unwrap_or("claude");
    let (harness, _) = parse_model(model);
    apply_harness_env(&harness, &mut cmd, process);

    propagate_rlm_env(&mut cmd);

    if process.auto && process.stream {
        // Stream mode: capture stdout line by line
        launch_streaming(&mut cmd, process.stream_format)
    } else if process.auto {
        // Batch mode: capture all output
        launch_batch(&mut cmd)
    } else {
        // Interactive mode: inherit stdio
        launch_interactive(&mut cmd)
    }
}

/// Seed RLM environment variables from config.
///
/// Sets process-level env vars so `propagate_rlm_env` can forward them
/// to child agents. Only sets vars that aren't already present (a parent
/// `lf` process may have already set them).
pub fn seed_rlm_env(config: &crate::engine::config::Config) {
    if std::env::var("RLM_MAX_DEPTH").is_err() {
        std::env::set_var("RLM_MAX_DEPTH", config.rlm_max_depth.to_string());
    }
    if std::env::var("RLM_MAX_PARALLEL").is_err() {
        std::env::set_var("RLM_MAX_PARALLEL", config.rlm_max_parallel.to_string());
    }
    if std::env::var("RLM_MODEL").is_err() {
        if let Some(ref model) = config.rlm_model {
            std::env::set_var("RLM_MODEL", model);
        }
    }
}

fn propagate_rlm_env(cmd: &mut Command) {
    // RLM_DEPTH auto-increments so sub-agents don't need to manage it.
    let next_depth = std::env::var("RLM_DEPTH")
        .ok()
        .and_then(|depth| depth.parse::<usize>().ok())
        .unwrap_or(0)
        + 1;
    cmd.env("RLM_DEPTH", next_depth.to_string());

    for var in RLM_PASSTHROUGH_VARS {
        if let Ok(val) = std::env::var(var) {
            cmd.env(var, val);
        }
    }
}

fn launch_batch(cmd: &mut Command) -> Result<LaunchResult, CoreError> {
    let start = Instant::now();
    cmd.stdin(Stdio::null());
    cmd.stdout(Stdio::piped());
    cmd.stderr(Stdio::piped());
    let child = cmd.spawn()?;
    let _pid_guard = ChildPidGuard::new(child.id());
    let output = child.wait_with_output()?;
    tracing::debug!(
        elapsed_ms = start.elapsed().as_millis(),
        "agent batch completed"
    );
    Ok(LaunchResult {
        exit_code: output.status.code().unwrap_or(1),
        stdout: String::from_utf8_lossy(&output.stdout).to_string(),
        stderr: String::from_utf8_lossy(&output.stderr).to_string(),
    })
}

fn launch_interactive(cmd: &mut Command) -> Result<LaunchResult, CoreError> {
    let start = Instant::now();
    let mut child = cmd.spawn()?;
    let _pid_guard = ChildPidGuard::new(child.id());
    tracing::debug!(
        elapsed_ms = start.elapsed().as_millis(),
        "agent spawned (interactive)"
    );
    let status = child.wait()?;
    tracing::debug!(
        elapsed_ms = start.elapsed().as_millis(),
        "agent interactive completed"
    );
    Ok(LaunchResult {
        exit_code: status.code().unwrap_or(1),
        stdout: String::new(),
        stderr: String::new(),
    })
}

fn launch_streaming(
    cmd: &mut Command,
    stream_format: StreamFormat,
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

    for (stream, line) in rx {
        if !logged_first_output {
            tracing::info!(
                elapsed_ms = start.elapsed().as_millis(),
                "agent produced first output"
            );
            logged_first_output = true;
        }
        match stream {
            StreamKind::Stdout => {
                if let Some(color) = use_color {
                    match parser.feed_line(&line) {
                        ParseResult::Events(events) => {
                            for event in &events {
                                format_event(event, color);
                            }
                        }
                        ParseResult::Skipped => {}
                        ParseResult::Passthrough => println!("{line}"),
                    }
                } else {
                    println!("{line}");
                }
                stdout_content.push_str(&line);
                stdout_content.push('\n');
            }
            StreamKind::Stderr => {
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

    let status = child.wait()?;
    tracing::debug!(
        elapsed_ms = start.elapsed().as_millis(),
        "agent streaming completed"
    );

    let _ = stdout_handle.join();
    let _ = stderr_handle.join();

    Ok(LaunchResult {
        exit_code: status.code().unwrap_or(1),
        stdout: stdout_content,
        stderr: stderr_content,
    })
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

    fn auto_process() -> ProcessConfig {
        ProcessConfig {
            auto: true,
            ..Default::default()
        }
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
        assert!(cmd.contains(&"exec".to_string()));
        assert!(cmd.contains(&"--sandbox".to_string()));
        assert!(cmd.contains(&"workspace-write".to_string()));
        assert!(cmd.contains(&"--ask-for-approval".to_string()));
        assert!(cmd.contains(&"never".to_string()));
        assert!(!cmd.contains(&"--dangerously-bypass-approvals-and-sandbox".to_string()));
        assert!(!cmd.contains(&"--full-auto".to_string()));
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
        assert!(
            !cmd.contains(&"exec".to_string()),
            "interactive mode should not use 'exec'"
        );
        assert!(!cmd.contains(&"--full-auto".to_string()));
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
            model: Some("opencode".to_string()),
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
            model: Some("opencode:anthropic/claude-sonnet".to_string()),
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
            skip_permissions: true,
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
            model: Some("codex:o3".to_string()),
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
    }

    #[test]
    fn build_codex_thread_start_params_omits_model_for_bare_codex() {
        let launch = AgentConfig {
            model: Some("codex".to_string()),
            cwd: Some("/tmp/repo".into()),
            ..default_launch()
        };

        let params = build_codex_thread_start_params(&launch);
        assert!(!params.contains_key("model"));
        assert_eq!(
            params.get("cwd"),
            Some(&serde_json::Value::String("/tmp/repo".to_string()))
        );
    }

    #[test]
    fn build_claude_session_turn_args_minimal() {
        let config = AgentConfig {
            system_prompt: String::new(),
            task_prompt: "task".to_string(),
            model: None,
            cwd: Some("/tmp".into()),
            max_turns: None,
            skip_permissions: false,
            structured_replies: Vec::new(),
        };
        let args = build_claude_session_turn_args("hello", &config, None);
        assert_eq!(
            args,
            vec!["-p", "hello", "--output-format", "stream-json", "--verbose"]
        );
    }

    #[test]
    fn build_claude_session_turn_args_full() {
        let config = AgentConfig {
            system_prompt: "Be concise".to_string(),
            task_prompt: "task".to_string(),
            model: Some("claude-sonnet-4-5-20250514".to_string()),
            cwd: Some("/tmp".into()),
            max_turns: Some(5),
            skip_permissions: true,
            structured_replies: Vec::new(),
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
            model: None,
            cwd: Some("/tmp".into()),
            max_turns: None,
            skip_permissions: false,
            structured_replies: vec![StructuredReply {
                name: "suggest_actions".to_string(),
                description: "Suggest actions".to_string(),
                guidance: "Emit <lf:suggest_actions> JSON.".to_string(),
            }],
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
            model: Some("codex".to_string()),
            ..default_launch()
        };
        let process = auto_process();
        let cmd = build_model_command(&launch, &process, &AgentCapabilities::default());
        assert!(!cmd.iter().any(|arg| arg.contains("model=\"codex\"")));
    }

    #[test]
    fn build_model_command_falls_back_to_claude_for_unknown_model() {
        let unknown_model = "gpt-5.1-codex-high";
        let launch = AgentConfig {
            model: Some(unknown_model.to_string()),
            ..default_launch()
        };
        let process = auto_process();
        let cmd = build_model_command(&launch, &process, &AgentCapabilities::default());
        assert_eq!(cmd.first(), Some(&"claude".to_string()));
        assert!(cmd.contains(&"--model".to_string()));
        assert!(cmd.contains(&unknown_model.to_string()));
    }
}
