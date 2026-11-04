//! Native host tools run on the Task's recorded Machine; ordinary shells have no
//! LF transport owner. Git remains responsible for editor, pager and difftool policy.
use std::io::{IsTerminal, Write};
use std::os::unix::process::CommandExt;
use std::path::Path;
use std::process::{Command, Stdio};

use anyhow::{Context, Result};

#[derive(Debug, Clone, Copy, clap::ValueEnum)]
pub enum Tool {
    Files,
    Changes,
    Shell,
}

pub fn run(
    issue: &str,
    tool: Tool,
    checkout: &Path,
    path: Option<&Path>,
    base: &str,
    head: Option<&str>,
    difftool: bool,
) -> Result<()> {
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
            file_editor(&full)
        }
        Tool::Changes => {
            if !difftool {
                let path = path
                    .map(|path| path.to_str().context("Changes path is not UTF-8"))
                    .transpose()?;
                let diff = crate::ops::task::task_diff(issue, path, base, head, None)?;
                if diff.truncated {
                    eprintln!("Patch truncated at 1 MB, as in Desktop");
                }
                return page(&checkout.worktree, &diff.patch);
            }
            let changes = crate::ops::task::task_changes(issue, base, head)?;
            let mut command = Command::new("git");
            command.arg("difftool");
            command.arg(&changes.base_commit);
            if head.is_some() {
                command.arg(&changes.head_commit);
            }
            command.arg("--");
            if let Some(path) = path {
                command.arg(path);
            }
            command
        }
    };
    command.current_dir(&checkout.worktree);
    Err(command.exec()).context("start the selected workspace tool")
}

fn page(checkout: &Path, patch: &str) -> Result<()> {
    if !std::io::stdout().is_terminal() {
        std::io::stdout().write_all(patch.as_bytes())?;
        return Ok(());
    }
    let pager = Command::new("git")
        .current_dir(checkout)
        .args(["var", "GIT_PAGER"])
        .output()?;
    anyhow::ensure!(pager.status.success(), "Git could not select its pager");
    let pager = String::from_utf8(pager.stdout)?;
    let mut command = Command::new("/bin/sh");
    command
        .current_dir(checkout)
        .args(["-c", pager.trim()])
        .stdin(Stdio::piped());
    if std::env::var_os("LESS").is_none() {
        command.env("LESS", "FRX");
    }
    let mut child = command.spawn().context("start Git's selected pager")?;
    let written = child
        .stdin
        .take()
        .context("Pager input is unavailable")?
        .write_all(patch.as_bytes());
    let status = child.wait()?;
    if let Err(error) = written {
        if error.kind() != std::io::ErrorKind::BrokenPipe {
            return Err(error.into());
        }
    }
    anyhow::ensure!(
        status.success(),
        "Git's selected pager exited with {status}"
    );
    Ok(())
}

pub(crate) fn file_editor(path: &Path) -> Command {
    let editor = path
        .is_dir()
        .then(|| std::env::var("LF_FILE_BROWSER").ok())
        .flatten()
        .or_else(|| std::env::var("VISUAL").ok())
        .or_else(|| std::env::var("EDITOR").ok())
        .filter(|value| !value.trim().is_empty())
        .unwrap_or_else(|| "vi".into());
    editor_command(&editor, path)
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
