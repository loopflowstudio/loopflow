//! Display the Task comparison through Git's pager or configured difftool.
use std::io::{IsTerminal, Write};
use std::os::unix::process::CommandExt;
use std::path::Path;
use std::process::{Command, Stdio};

use anyhow::{Context, Result};

pub fn run(
    issue: &str,
    path: Option<&str>,
    base: &str,
    head: Option<&str>,
    draft: Option<&str>,
    json: bool,
    difftool: bool,
) -> Result<()> {
    if difftool {
        let checkout = crate::ops::task::file_context(&crate::ops::task::file_store()?, issue)?;
        let changes = crate::ops::task::task_changes(issue, base, head)?;
        let mut command = Command::new("git");
        command
            .current_dir(&checkout.worktree)
            .arg("difftool")
            .arg(&changes.base_commit);
        if head.is_some() {
            command.arg(&changes.head_commit);
        }
        command.arg("--").args(path);
        return Err(command.exec()).context("start Git's configured difftool");
    }
    let snapshot = crate::ops::task::task_diff(issue, path, base, head, draft)?;
    if json {
        println!("{}", serde_json::to_string_pretty(&snapshot)?);
    } else {
        if std::io::stdout().is_terminal() {
            let checkout = crate::ops::task::file_context(&crate::ops::task::file_store()?, issue)?;
            page(&checkout.worktree, &snapshot.patch)?;
        } else {
            std::io::stdout().write_all(snapshot.patch.as_bytes())?;
        }
        if snapshot.truncated {
            eprintln!("\n[diff truncated at 1 MB]");
        }
    }
    Ok(())
}

fn page(checkout: &Path, patch: &str) -> Result<()> {
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
