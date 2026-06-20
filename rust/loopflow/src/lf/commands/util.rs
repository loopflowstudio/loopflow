use anyhow::{anyhow, Result};
use std::path::{Path, PathBuf};
use std::process::Command;

use crate::engine::{check_cli_available, LaunchTarget};

pub fn find_repo_root() -> Result<PathBuf> {
    let output = std::process::Command::new("git")
        .args(["rev-parse", "--show-toplevel"])
        .output()?;

    if !output.status.success() {
        return Ok(std::env::current_dir()?);
    }

    Ok(PathBuf::from(
        String::from_utf8_lossy(&output.stdout).trim(),
    ))
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct SessionCommand {
    program: String,
    args: Vec<String>,
    cwd: PathBuf,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct SessionLaunch {
    command: SessionCommand,
    ide_url: Option<String>,
}

pub fn launch_session(
    target: LaunchTarget,
    harness: &str,
    model: Option<&str>,
    worktree: &Path,
    prompt: &str,
) -> Result<()> {
    let launch = build_session_launch(target, harness, model, worktree, prompt)?;

    if target == LaunchTarget::Ide {
        if let Some(url) = launch.ide_url.as_deref() {
            match crate::engine::platform::open_url_checked(url) {
                Ok(()) => return Ok(()),
                Err(err) => eprintln!("Could not open vendor app ({err}); falling back to CLI."),
            }
        } else if harness == "opencode" {
            eprintln!("OpenCode has no standalone app; falling back to CLI.");
        }
    }

    spawn_session_command(&launch.command)
}

fn build_session_launch(
    target: LaunchTarget,
    harness: &str,
    model: Option<&str>,
    worktree: &Path,
    prompt: &str,
) -> Result<SessionLaunch> {
    let worktree = absolute_path(worktree);
    let command = build_session_command(harness, model, &worktree, prompt)?;
    let ide_url = if target == LaunchTarget::Ide {
        build_ide_url(harness, &worktree, prompt)
    } else {
        None
    };

    Ok(SessionLaunch { command, ide_url })
}

fn build_ide_url(harness: &str, worktree: &Path, prompt: &str) -> Option<String> {
    let worktree = percent_encode(&worktree.to_string_lossy());
    let prompt = percent_encode(prompt);
    match harness {
        "codex" => Some(format!(
            "codex://threads/new?path={worktree}&prompt={prompt}"
        )),
        "claude" => Some(format!("claude://code/new?folder={worktree}&q={prompt}")),
        "opencode" => None,
        _ => None,
    }
}

fn build_session_command(
    harness: &str,
    model: Option<&str>,
    worktree: &Path,
    prompt: &str,
) -> Result<SessionCommand> {
    let cwd = worktree.to_path_buf();
    let worktree_arg = worktree.to_string_lossy().to_string();

    match harness {
        "codex" => {
            let mut args = vec!["-C".to_string(), worktree_arg];
            if let Some(model) = model {
                args.push("-c".to_string());
                args.push(format!("model=\"{model}\""));
            }
            args.push("--sandbox".to_string());
            args.push("workspace-write".to_string());
            args.push(prompt.to_string());
            Ok(SessionCommand {
                program: "codex".to_string(),
                args,
                cwd,
            })
        }
        "claude" => {
            let mut args = Vec::new();
            if let Some(model) = model {
                args.push("--model".to_string());
                args.push(model.to_string());
            }
            args.push(prompt.to_string());
            Ok(SessionCommand {
                program: "claude".to_string(),
                args,
                cwd,
            })
        }
        "opencode" => {
            let mut args = vec![worktree_arg, "--prompt".to_string(), prompt.to_string()];
            if let Some(model) = model {
                args.push("--model".to_string());
                args.push(model.to_string());
            }
            Ok(SessionCommand {
                program: "opencode".to_string(),
                args,
                cwd,
            })
        }
        _ => Err(anyhow!(
            "unsupported session launcher harness '{}'. Use claude, codex, or opencode.",
            harness
        )),
    }
}

fn spawn_session_command(command: &SessionCommand) -> Result<()> {
    if !check_cli_available(&command.program) {
        return Err(anyhow!(
            "'{}' CLI not found. Run `lf op doctor` to check dependencies.",
            command.program
        ));
    }

    let status = Command::new(&command.program)
        .args(&command.args)
        .current_dir(&command.cwd)
        .env_remove("LOOPFLOW_DIRECTIVE_FILE")
        .env_remove("ANTHROPIC_API_KEY")
        .env_remove("OPENAI_API_KEY")
        .env_remove("GEMINI_API_KEY")
        .status()?;

    if status.success() {
        Ok(())
    } else {
        Err(anyhow!("session launcher exited with status {status}"))
    }
}

fn absolute_path(path: &Path) -> PathBuf {
    if let Ok(canonical) = path.canonicalize() {
        return canonical;
    }
    if path.is_absolute() {
        return path.to_path_buf();
    }
    std::env::current_dir()
        .map(|cwd| cwd.join(path))
        .unwrap_or_else(|_| path.to_path_buf())
}

fn percent_encode(value: &str) -> String {
    let mut encoded = String::new();
    for byte in value.bytes() {
        match byte {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'.' | b'_' | b'~' => {
                encoded.push(byte as char);
            }
            _ => encoded.push_str(&format!("%{byte:02X}")),
        }
    }
    encoded
}

#[cfg(test)]
mod tests {
    use super::*;

    fn path() -> PathBuf {
        PathBuf::from("/tmp/loop flow")
    }

    fn args(values: &[&str]) -> Vec<String> {
        values.iter().map(|value| value.to_string()).collect()
    }

    #[test]
    fn session_launch_cli_codex_sets_worktree_model_sandbox_and_prompt() {
        let launch =
            build_session_launch(LaunchTarget::Cli, "codex", Some("o3"), &path(), "fix it")
                .expect("build launch");

        assert_eq!(
            launch.command,
            SessionCommand {
                program: "codex".to_string(),
                args: args(&[
                    "-C",
                    "/tmp/loop flow",
                    "-c",
                    "model=\"o3\"",
                    "--sandbox",
                    "workspace-write",
                    "fix it",
                ]),
                cwd: path(),
            }
        );
        assert_eq!(launch.ide_url, None);
    }

    #[test]
    fn session_launch_cli_claude_runs_in_worktree_with_model_and_prompt() {
        let launch = build_session_launch(
            LaunchTarget::Cli,
            "claude",
            Some("sonnet"),
            &path(),
            "fix it",
        )
        .expect("build launch");

        assert_eq!(
            launch.command,
            SessionCommand {
                program: "claude".to_string(),
                args: args(&["--model", "sonnet", "fix it"]),
                cwd: path(),
            }
        );
        assert_eq!(launch.ide_url, None);
    }

    #[test]
    fn session_launch_cli_opencode_sets_worktree_prompt_and_model() {
        let launch = build_session_launch(
            LaunchTarget::Cli,
            "opencode",
            Some("moonshotai/kimi-k2"),
            &path(),
            "fix it",
        )
        .expect("build launch");

        assert_eq!(
            launch.command,
            SessionCommand {
                program: "opencode".to_string(),
                args: args(&[
                    "/tmp/loop flow",
                    "--prompt",
                    "fix it",
                    "--model",
                    "moonshotai/kimi-k2",
                ]),
                cwd: path(),
            }
        );
        assert_eq!(launch.ide_url, None);
    }

    #[test]
    fn session_launch_ide_codex_builds_scheme_with_encoded_path_and_prompt() {
        let launch =
            build_session_launch(LaunchTarget::Ide, "codex", None, &path(), "fix & test\nnow")
                .expect("build launch");

        assert_eq!(
            launch.ide_url.as_deref(),
            Some("codex://threads/new?path=%2Ftmp%2Floop%20flow&prompt=fix%20%26%20test%0Anow")
        );
        assert_eq!(launch.command.program, "codex");
    }

    #[test]
    fn session_launch_ide_claude_builds_code_scheme_with_encoded_folder_and_prompt() {
        let launch = build_session_launch(
            LaunchTarget::Ide,
            "claude",
            None,
            &path(),
            "fix & test\nnow",
        )
        .expect("build launch");

        assert_eq!(
            launch.ide_url.as_deref(),
            Some("claude://code/new?folder=%2Ftmp%2Floop%20flow&q=fix%20%26%20test%0Anow")
        );
        assert_eq!(launch.command.program, "claude");
    }

    #[test]
    fn session_launch_ide_opencode_falls_back_to_cli_shape() {
        let launch = build_session_launch(LaunchTarget::Ide, "opencode", None, &path(), "fix it")
            .expect("build launch");

        assert_eq!(launch.command.program, "opencode");
        assert_eq!(launch.ide_url, None);
    }
}
