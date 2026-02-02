//! Agent invocation for spawning coding agent runners (Claude, Codex, Gemini).
//!
//! This module handles building commands and spawning subprocesses for each
//! supported coding agent. Output can be captured or streamed.

use std::io::{BufRead, BufReader};
use std::process::{Command, Stdio};

use crate::config::parse_model;
use crate::error::CoreError;

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
    /// Run in auto/batch mode (non-interactive)
    pub auto: bool,
    /// Stream output in real-time
    pub stream: bool,
    /// Skip permission prompts
    pub skip_permissions: bool,
    /// Model variant (e.g., "opus", "sonnet")
    pub model_variant: Option<String>,
    /// Enable Chrome integration (Claude only)
    pub chrome: bool,
    /// Working directory
    pub cwd: Option<std::path::PathBuf>,
}

/// Build Claude CLI command.
pub fn build_claude_command(config: &LaunchConfig) -> Vec<String> {
    let mut cmd = vec!["claude".to_string()];

    if config.chrome {
        cmd.push("--chrome".to_string());
    }

    if let Some(ref variant) = config.model_variant {
        cmd.push("--model".to_string());
        cmd.push(variant.clone());
    }

    if config.auto {
        cmd.push("--print".to_string());
        cmd.push("--dangerously-skip-permissions".to_string());
        if config.stream {
            cmd.push("--output-format".to_string());
            cmd.push("stream-json".to_string());
            cmd.push("--verbose".to_string());
        }
    } else if config.skip_permissions {
        cmd.push("--dangerously-skip-permissions".to_string());
    }

    cmd
}

/// Build Codex CLI command.
pub fn build_codex_command(config: &LaunchConfig) -> Vec<String> {
    let mut cmd = vec!["codex".to_string(), "exec".to_string()];

    if let Some(ref variant) = config.model_variant {
        cmd.push("-c".to_string());
        cmd.push(format!("model=\"{}\"", variant));
    }

    if let Some(ref cwd) = config.cwd {
        cmd.push("-C".to_string());
        cmd.push(cwd.to_string_lossy().to_string());
    }

    if config.stream {
        cmd.push("--json".to_string());
    }

    if config.skip_permissions {
        cmd.push("--sandbox".to_string());
        cmd.push("danger-full-access".to_string());
        cmd.push("-c".to_string());
        cmd.push("approval_policy=\"never\"".to_string());
    } else {
        cmd.push("--sandbox".to_string());
        cmd.push("workspace-write".to_string());

        if config.auto {
            cmd.push("--full-auto".to_string());
        }
    }

    cmd
}

/// Build Gemini CLI command.
pub fn build_gemini_command(config: &LaunchConfig) -> Vec<String> {
    let mut cmd = vec!["gemini".to_string()];

    if let Some(ref variant) = config.model_variant {
        cmd.push("-m".to_string());
        cmd.push(variant.clone());
    }

    if config.stream {
        cmd.push("--output-format".to_string());
        cmd.push("stream-json".to_string());
    }

    if config.skip_permissions {
        cmd.push("--yolo".to_string());
    }

    cmd
}

/// Build command for any model.
pub fn build_model_command(model: &str, config: &LaunchConfig) -> Vec<String> {
    match model {
        "claude" => build_claude_command(config),
        "codex" => build_codex_command(config),
        "gemini" => build_gemini_command(config),
        _ => build_claude_command(config), // default to claude
    }
}

/// Launch an agent with the given prompt.
///
/// # Arguments
/// * `model` - Model string like "claude:opus" or "codex"
/// * `prompt` - The prompt to send to the agent
/// * `config` - Launch configuration
///
/// # Returns
/// LaunchResult with exit code and captured output (if any)
pub fn launch_agent(
    model: &str,
    prompt: &str,
    config: &LaunchConfig,
) -> Result<LaunchResult, CoreError> {
    let (backend, variant) = parse_model(model);

    let mut launch_config = config.clone();
    if launch_config.model_variant.is_none() {
        launch_config.model_variant = variant;
    }

    let cmd_args = build_model_command(&backend, &launch_config);
    if cmd_args.is_empty() {
        return Err(CoreError::ExecutionFailed("Empty command".to_string()));
    }

    let program = &cmd_args[0];
    let args = &cmd_args[1..];

    let mut cmd = Command::new(program);
    cmd.args(args);
    cmd.arg(prompt);

    if let Some(ref cwd) = config.cwd {
        cmd.current_dir(cwd);
    }

    if config.auto && config.stream {
        // Stream mode: capture stdout line by line
        launch_streaming(&mut cmd)
    } else if config.auto {
        // Batch mode: capture all output
        launch_batch(&mut cmd)
    } else {
        // Interactive mode: inherit stdio
        launch_interactive(&mut cmd)
    }
}

fn launch_batch(cmd: &mut Command) -> Result<LaunchResult, CoreError> {
    let output = cmd.output()?;
    Ok(LaunchResult {
        exit_code: output.status.code().unwrap_or(1),
        stdout: String::from_utf8_lossy(&output.stdout).to_string(),
        stderr: String::from_utf8_lossy(&output.stderr).to_string(),
    })
}

fn launch_interactive(cmd: &mut Command) -> Result<LaunchResult, CoreError> {
    let status = cmd.status()?;
    Ok(LaunchResult {
        exit_code: status.code().unwrap_or(1),
        stdout: String::new(),
        stderr: String::new(),
    })
}

fn launch_streaming(cmd: &mut Command) -> Result<LaunchResult, CoreError> {
    cmd.stdout(Stdio::piped());
    cmd.stderr(Stdio::piped());

    let mut child = cmd.spawn()?;
    let mut stdout_content = String::new();

    if let Some(stdout) = child.stdout.take() {
        let reader = BufReader::new(stdout);
        for line in reader.lines().map_while(Result::ok) {
            stdout_content.push_str(&line);
            stdout_content.push('\n');
        }
    }

    let status = child.wait()?;
    let stderr_content = child
        .stderr
        .take()
        .map(|mut s| {
            let mut buf = String::new();
            std::io::Read::read_to_string(&mut s, &mut buf).ok();
            buf
        })
        .unwrap_or_default();

    Ok(LaunchResult {
        exit_code: status.code().unwrap_or(1),
        stdout: stdout_content,
        stderr: stderr_content,
    })
}

/// Check if a CLI is available.
pub fn check_cli_available(cli: &str) -> bool {
    Command::new(cli)
        .arg("--version")
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .map(|s| s.success())
        .unwrap_or(false)
}

/// Agent runner trait for dependency injection in tests.
pub trait Runner: Send + Sync {
    fn launch(
        &self,
        model: &str,
        prompt: &str,
        config: &LaunchConfig,
    ) -> Result<LaunchResult, CoreError>;
}

/// Default agent runner that spawns actual processes.
#[derive(Debug, Clone, Copy, Default)]
pub struct DefaultRunner;

impl Runner for DefaultRunner {
    fn launch(
        &self,
        model: &str,
        prompt: &str,
        config: &LaunchConfig,
    ) -> Result<LaunchResult, CoreError> {
        launch_agent(model, prompt, config)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn build_claude_command_auto() {
        let config = LaunchConfig {
            auto: true,
            stream: false,
            skip_permissions: false,
            ..Default::default()
        };
        let cmd = build_claude_command(&config);
        assert!(cmd.contains(&"--print".to_string()));
        assert!(cmd.contains(&"--dangerously-skip-permissions".to_string()));
    }

    #[test]
    fn build_claude_command_stream() {
        let config = LaunchConfig {
            auto: true,
            stream: true,
            ..Default::default()
        };
        let cmd = build_claude_command(&config);
        assert!(cmd.contains(&"stream-json".to_string()));
        assert!(cmd.contains(&"--verbose".to_string()));
    }

    #[test]
    fn build_claude_command_with_model_variant() {
        let config = LaunchConfig {
            model_variant: Some("opus".to_string()),
            ..Default::default()
        };
        let cmd = build_claude_command(&config);
        assert!(cmd.contains(&"--model".to_string()));
        assert!(cmd.contains(&"opus".to_string()));
    }

    #[test]
    fn build_claude_command_with_chrome_flag() {
        let config = LaunchConfig {
            chrome: true,
            ..Default::default()
        };
        let cmd = build_claude_command(&config);
        assert!(cmd.contains(&"--chrome".to_string()));
    }

    #[test]
    fn build_codex_command_auto() {
        let config = LaunchConfig {
            auto: true,
            stream: false,
            skip_permissions: false,
            ..Default::default()
        };
        let cmd = build_codex_command(&config);
        assert!(cmd.contains(&"exec".to_string()));
        assert!(cmd.contains(&"--full-auto".to_string()));
    }

    #[test]
    fn build_codex_command_with_model() {
        let config = LaunchConfig {
            model_variant: Some("o3".to_string()),
            ..Default::default()
        };
        let cmd = build_codex_command(&config);
        assert!(cmd.contains(&"model=\"o3\"".to_string()));
    }

    #[test]
    fn build_gemini_command_yolo() {
        let config = LaunchConfig {
            skip_permissions: true,
            ..Default::default()
        };
        let cmd = build_gemini_command(&config);
        assert!(cmd.contains(&"--yolo".to_string()));
    }

    #[test]
    fn build_gemini_command_with_model() {
        let config = LaunchConfig {
            model_variant: Some("gemini-1.5".to_string()),
            ..Default::default()
        };
        let cmd = build_gemini_command(&config);
        assert!(cmd.contains(&"-m".to_string()));
        assert!(cmd.contains(&"gemini-1.5".to_string()));
    }
}
