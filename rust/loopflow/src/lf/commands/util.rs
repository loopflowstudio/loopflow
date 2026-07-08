use anyhow::{anyhow, Result};
use std::path::{Path, PathBuf};
use std::process::Command;

use crate::engine::{check_cli_available, codex_permission_args, workspace_add_dirs, LaunchTarget};

pub fn find_repo_root() -> Result<PathBuf> {
    crate::engine::repo::find_repo_root()
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
                Err(err) => eprintln!("Could not open vendor app ({err}); falling back to TUI."),
            }
        } else if harness == "opencode" {
            eprintln!("OpenCode has no standalone app; opening the TUI.");
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
            for dir in workspace_add_dirs(worktree) {
                args.push("--add-dir".to_string());
                args.push(dir.to_string_lossy().to_string());
            }
            args.extend(codex_permission_args(Some(worktree), false, false));
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
            for dir in workspace_add_dirs(worktree) {
                args.push("--add-dir".to_string());
                args.push(dir.to_string_lossy().to_string());
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

    #[test]
    fn session_launch_tui_codex_sets_worktree_model_and_prompt() {
        let launch =
            build_session_launch(LaunchTarget::Tui, "codex", Some("o3"), &path(), "fix it")
                .expect("build launch");

        assert_eq!(launch.command.program, "codex");
        assert_eq!(launch.command.cwd, path());
        assert!(launch.command.args.starts_with(&args(&[
            "-C",
            "/tmp/loop flow",
            "-c",
            "model=\"o3\""
        ])));
        assert_eq!(
            launch.command.args.last().map(String::as_str),
            Some("fix it")
        );
        assert_eq!(
            launch.command.args.contains(&"--sandbox".to_string()),
            crate::engine::codex_permission_args(Some(&path()), false, false)
                .contains(&"--sandbox".to_string())
        );
        assert_eq!(launch.ide_url, None);
    }

    #[test]
    fn session_launch_tui_codex_adds_main_repo_for_worktree_metadata() {
        let (_tmp, main, worktree) = git_worktree_fixture();

        let launch = build_session_launch(LaunchTarget::Tui, "codex", None, &worktree, "fix it")
            .expect("build launch");

        let idx = launch
            .command
            .args
            .iter()
            .position(|arg| arg == "--add-dir")
            .expect("add-dir flag");
        assert_eq!(
            PathBuf::from(&launch.command.args[idx + 1])
                .canonicalize()
                .unwrap(),
            main.canonicalize().unwrap()
        );
    }

    #[test]
    fn session_launch_tui_claude_runs_in_worktree_with_model_and_prompt() {
        let launch = build_session_launch(
            LaunchTarget::Tui,
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
    fn session_launch_tui_claude_adds_main_repo_for_worktree_metadata() {
        let (_tmp, main, worktree) = git_worktree_fixture();

        let launch = build_session_launch(
            LaunchTarget::Tui,
            "claude",
            Some("sonnet"),
            &worktree,
            "fix it",
        )
        .expect("build launch");

        let idx = launch
            .command
            .args
            .iter()
            .position(|arg| arg == "--add-dir")
            .expect("add-dir flag");
        assert_eq!(
            PathBuf::from(&launch.command.args[idx + 1])
                .canonicalize()
                .unwrap(),
            main.canonicalize().unwrap()
        );
    }

    #[test]
    fn session_launch_tui_opencode_sets_worktree_prompt_and_model() {
        let launch = build_session_launch(
            LaunchTarget::Tui,
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
