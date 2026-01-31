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
        Err(CoreError::WorktreeError(
            "git worktree add failed".to_string(),
        ))
    }
}

/// Remove a worktree and optionally force-delete its branch.
///
/// When `force_delete_branch` is true, the branch is deleted even if unmerged.
/// Use this for temporary worktrees (forks) that we created and know are safe to remove.
pub fn remove_worktree(worktree: &Path, force_delete_branch: bool) -> Result<(), CoreError> {
    // Get the branch name before removing the worktree
    let branch = Command::new("git")
        .arg("-C")
        .arg(worktree)
        .arg("rev-parse")
        .arg("--abbrev-ref")
        .arg("HEAD")
        .output()
        .ok()
        .filter(|o| o.status.success())
        .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string());

    // Get the main repo path for branch deletion
    let main_repo = Command::new("git")
        .arg("-C")
        .arg(worktree)
        .arg("rev-parse")
        .arg("--path-format=absolute")
        .arg("--git-common-dir")
        .output()
        .ok()
        .filter(|o| o.status.success())
        .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string());

    // Remove the worktree
    let status = Command::new("git")
        .arg("worktree")
        .arg("remove")
        .arg("--force")
        .arg(worktree)
        .status()?;
    if !status.success() {
        return Err(CoreError::WorktreeError(
            "git worktree remove failed".to_string(),
        ));
    }

    // Delete the branch if requested
    if force_delete_branch {
        if let (Some(branch), Some(repo)) = (branch, main_repo) {
            // Don't delete main/master
            if branch != "main" && branch != "master" && branch != "HEAD" {
                let _ = Command::new("git")
                    .arg("-C")
                    .arg(&repo)
                    .arg("branch")
                    .arg("-D")
                    .arg(&branch)
                    .status();
            }
        }
    }

    Ok(())
}

pub fn find_worktree_root(path: &Path) -> Result<String, CoreError> {
    let output = Command::new("git")
        .arg("-C")
        .arg(path)
        .arg("rev-parse")
        .arg("--show-toplevel")
        .output()?;
    if !output.status.success() {
        return Err(CoreError::WorktreeError("git rev-parse failed".to_string()));
    }
    Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
}
