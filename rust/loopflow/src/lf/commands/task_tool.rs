//! Native host tools run on the Task's recorded Machine; ordinary shells have no
//! LF transport owner. Files use the configured browser or editor.
use std::os::unix::process::CommandExt;
use std::path::Path;
use std::process::Command;

use anyhow::{Context, Result};

#[derive(Debug, Clone, Copy, clap::ValueEnum)]
pub enum Tool {
    Files,
    Shell,
}

pub fn run(issue: &str, tool: Tool, checkout: &Path, path: Option<&Path>) -> Result<()> {
    let checkout = crate::ops::task::workspace::pinned_checkout(issue, checkout)?;
    let mut command = match tool {
        Tool::Shell => {
            let mut command =
                Command::new(std::env::var_os("SHELL").unwrap_or_else(|| "/bin/sh".into()));
            command.arg("-l");
            command
        }
        Tool::Files => {
            let path = path.unwrap_or(Path::new("."));
            anyhow::ensure!(
                !path.is_absolute()
                    && !path
                        .components()
                        .any(|part| matches!(part, std::path::Component::ParentDir)),
                "Files path must stay inside the recorded checkout"
            );
            let full = checkout
                .worktree
                .join(path)
                .canonicalize()
                .context("Files target is unavailable")?;
            anyhow::ensure!(
                full.starts_with(checkout.worktree.canonicalize()?),
                "Files target is outside the recorded checkout"
            );
            let editor = if full.is_dir() {
                std::env::var("LF_FILE_BROWSER").ok()
            } else {
                None
            }
            .or_else(|| std::env::var("VISUAL").ok())
            .or_else(|| std::env::var("EDITOR").ok())
            .filter(|value| !value.trim().is_empty())
            .unwrap_or_else(|| "vi".into());
            editor_command(&editor, &full)
        }
    };
    command.current_dir(&checkout.worktree);
    Err(command.exec()).context("start the selected workspace tool")
}

fn editor_command(editor: &str, path: &Path) -> Command {
    // Editor settings are user-authored shell commands. Resolved paths remain
    // positional arguments, never part of the evaluated setting.
    let mut command = Command::new("/bin/sh");
    command
        .args(["-c", &format!("{editor} \"$@\""), "lf-files"])
        .arg(path);
    command
}

#[cfg(test)]
mod tests {
    use super::editor_command;
    use std::path::Path;

    #[test]
    fn editor_setting_can_include_environment_assignments() {
        let output = editor_command(
            r#"EDITOR_TEST_FLAG=present sh -c 'printf "%s\n" "$EDITOR_TEST_FLAG" "$1"' editor"#,
            Path::new("/tmp/selected file"),
        )
        .output()
        .unwrap();
        assert!(output.status.success());
        assert_eq!(
            String::from_utf8(output.stdout).unwrap(),
            "present\n/tmp/selected file\n"
        );
    }

    #[test]
    fn editor_receives_literal_paths_after_its_options() {
        let output = editor_command(
            "printf '%s\\n' --",
            Path::new("/tmp/space ' $(touch never);x"),
        )
        .output()
        .unwrap();
        assert!(output.status.success());
        assert_eq!(
            String::from_utf8(output.stdout).unwrap(),
            "--\n/tmp/space ' $(touch never);x\n"
        );
    }
}
