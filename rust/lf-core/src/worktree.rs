use std::path::Path;
use std::process::Command;

use crate::error::CoreError;

pub fn create_worktree(repo: &Path, worktree: &Path, branch: &str) -> Result<(), CoreError> {
    let status = Command::new("git")
        .arg("-C")
        .arg(repo)
        .arg("worktree")
        .arg("add")
        .arg(worktree)
        .arg("-b")
        .arg(branch)
        .status()?;
    if status.success() {
        Ok(())
    } else {
        Err(CoreError::WorktreeError("git worktree add failed".to_string()))
    }
}

pub fn remove_worktree(worktree: &Path) -> Result<(), CoreError> {
    let status = Command::new("git")
        .arg("worktree")
        .arg("remove")
        .arg(worktree)
        .status()?;
    if status.success() {
        Ok(())
    } else {
        Err(CoreError::WorktreeError(
            "git worktree remove failed".to_string(),
        ))
    }
}

pub fn find_worktree_root(path: &Path) -> Result<String, CoreError> {
    let output = Command::new("git")
        .arg("-C")
        .arg(path)
        .arg("rev-parse")
        .arg("--show-toplevel")
        .output()?;
    if !output.status.success() {
        return Err(CoreError::WorktreeError(
            "git rev-parse failed".to_string(),
        ));
    }
    Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
}
