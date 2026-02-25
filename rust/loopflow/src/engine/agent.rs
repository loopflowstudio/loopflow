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
pub struct LaunchConfig {
    /// System/context prompt content.
    pub system_prompt: String,
    /// Task prompt content sent as the turn input.
    pub task_prompt: String,
    /// Model string (for example: "claude:opus" or "codex").
    pub model: Option<String>,
    /// Provider-specific max turn budget when supported.
    pub max_turns: Option<u32>,
    /// Working directory.
    pub cwd: Option<std::path::PathBuf>,
    /// Skip permission prompts
    pub skip_permissions: bool,
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

/// Provider capability flags.
#[derive(Debug, Clone, Default)]
pub struct AgentCapabilities {
    /// Enable Chrome integration (Claude only).
    pub chrome: bool,
}

/// Build Claude CLI command.
pub fn build_claude_command(
    launch: &LaunchConfig,
    process: &ProcessConfig,
    capabilities: &AgentCapabilities,
    model_variant: Option<&str>,
) -> Vec<String> {
    let mut cmd = vec!["claude".to_string()];

    if capabilities.chrome {
        cmd.push("--chrome".to_string());
    }

    if let Some(variant) = model_variant {
        cmd.push("--model".to_string());
        cmd.push(variant.to_string());
    }

    // Load context via system prompt file for cleaner input history
    if let Some(ref context_file) = process.context_file {
        cmd.push("--append-system-prompt-file".to_string());
        cmd.push(context_file.to_string_lossy().to_string());
    } else if !launch.system_prompt.trim().is_empty() {
        cmd.push("--append-system-prompt".to_string());
        cmd.push(launch.system_prompt.clone());
    }

    if process.auto {
        cmd.push("--print".to_string());
        cmd.push("--dangerously-skip-permissions".to_string());
        if process.stream {
            cmd.push("--output-format".to_string());
            cmd.push("stream-json".to_string());
            cmd.push("--verbose".to_string());
        }
    } else if launch.skip_permissions {
        cmd.push("--dangerously-skip-permissions".to_string());
    }

    if let Some(max_turns) = launch.max_turns {
        cmd.push("--max-turns".to_string());
        cmd.push(max_turns.to_string());
    }

    cmd
}

/// Build Codex CLI command.
pub fn build_codex_command(
    launch: &LaunchConfig,
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
    launch: &LaunchConfig,
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

/// Build command for any model.
pub fn build_model_command(
    launch: &LaunchConfig,
    process: &ProcessConfig,
    capabilities: &AgentCapabilities,
) -> Vec<String> {
    let model = launch.model.as_deref().unwrap_or("claude");
    let (backend, variant) = parse_model(model);
    let model_variant = variant.as_deref();

    match backend.as_str() {
        "claude" => build_claude_command(launch, process, capabilities, model_variant),
        "codex" => build_codex_command(launch, process, model_variant),
        "gemini" => build_gemini_command(launch, process, model_variant),
        "opencode" => build_opencode_command(process, model_variant),
        _ => build_claude_command(launch, process, capabilities, Some(model)),
    }
}

/// Build a full CLI command (including prompt) for a model.
pub fn build_agent_command(
    launch: &LaunchConfig,
    process: &ProcessConfig,
    capabilities: &AgentCapabilities,
) -> Vec<String> {
    let mut cmd = build_model_command(launch, process, capabilities);
    if !launch.task_prompt.is_empty() {
        cmd.push(launch.task_prompt.clone());
    }
    cmd
}

/// Launch an agent with the given prompt.
///
/// # Arguments
/// * `launch` - Canonical launch config including prompt/model/cwd fields
/// * `process` - Process execution options
/// * `capabilities` - Provider capability flags
///
/// # Returns
/// LaunchResult with exit code and captured output (if any)
pub fn launch_agent(
    launch: &LaunchConfig,
    process: &ProcessConfig,
    capabilities: &AgentCapabilities,
) -> Result<LaunchResult, CoreError> {
    let start = Instant::now();
    let model = launch.model.as_deref().unwrap_or("claude");
    let (backend, _variant) = parse_model(model);

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

    // For Gemini, set GEMINI_SYSTEM_MD env var to load context
    if backend == "gemini" {
        if let Some(ref context_file) = process.context_file {
            cmd.env(
                "GEMINI_SYSTEM_MD",
                context_file.to_string_lossy().to_string(),
            );
        }
    }

    // For OpenCode, set OPENCODE_CONFIG_CONTENT env var for permissions and context
    if backend == "opencode" {
        if let Some(env_val) = build_opencode_env(process) {
            cmd.env("OPENCODE_CONFIG_CONTENT", env_val);
        }
    }

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
        launch: &LaunchConfig,
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
        launch: &LaunchConfig,
        process: &ProcessConfig,
        capabilities: &AgentCapabilities,
    ) -> Result<LaunchResult, CoreError> {
        launch_agent(launch, process, capabilities)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn default_launch() -> LaunchConfig {
        LaunchConfig {
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
        let launch = LaunchConfig {
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
        let launch = LaunchConfig {
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
        let launch = LaunchConfig {
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
        let launch = LaunchConfig {
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
        let launch = LaunchConfig {
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
        let launch = LaunchConfig {
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
        let launch = LaunchConfig {
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
}
