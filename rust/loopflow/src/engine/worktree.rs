use std::path::Path;
use std::process::Command;

use crate::engine::error::CoreError;

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
    let git_value = |args: &[&str]| {
        Command::new("git")
            .arg("-C")
            .arg(worktree)
            .args(args)
            .output()
            .ok()
            .filter(|o| o.status.success())
            .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string())
    };

    let main_repo_root = || {
        let git_common_dir =
            git_value(&["rev-parse", "--path-format=absolute", "--git-common-dir"])?;
        let common_dir = std::path::PathBuf::from(git_common_dir);
        if common_dir
            .file_name()
            .is_some_and(|name| name == std::ffi::OsStr::new(".git"))
        {
            return common_dir.parent().map(std::path::Path::to_path_buf);
        }
        None
    };

    // Get the branch name before removing the worktree
    let branch = git_value(&["rev-parse", "--abbrev-ref", "HEAD"]);

    // Get the main repo path for worktree removal and branch deletion
    let main_repo = main_repo_root()
        .or_else(|| git_value(&["rev-parse", "--show-toplevel"]).map(std::path::PathBuf::from));

    // Remove the worktree
    let remove_base = main_repo.as_deref().unwrap_or(worktree);
    let status = Command::new("git")
        .arg("-C")
        .arg(remove_base)
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

#[cfg(test)]
mod tests {
    use super::remove_worktree;
    use tempfile::tempdir;

    #[test]
    fn remove_worktree_deletes_branch_from_main_repo() {
        let tmp = tempdir().expect("tempdir");
        let repo = tmp.path().join("repo");
        std::fs::create_dir_all(&repo).expect("repo dir");
        let worktree = tmp.path().join("repo-feature");

        std::process::Command::new("git")
            .arg("-C")
            .arg(&repo)
            .arg("init")
            .status()
            .expect("git init");
        std::process::Command::new("git")
            .arg("-C")
            .arg(&repo)
            .args(["config", "user.email", "test@example.com"])
            .status()
            .expect("git config email");
        std::process::Command::new("git")
            .arg("-C")
            .arg(&repo)
            .args(["config", "user.name", "Test User"])
            .status()
            .expect("git config name");
        std::fs::write(repo.join("README.md"), "# test\n").expect("seed file");
        std::process::Command::new("git")
            .arg("-C")
            .arg(&repo)
            .args(["add", "."])
            .status()
            .expect("git add");
        std::process::Command::new("git")
            .arg("-C")
            .arg(&repo)
            .args(["commit", "-m", "initial"])
            .status()
            .expect("git commit");

        std::process::Command::new("git")
            .arg("-C")
            .arg(&repo)
            .arg("worktree")
            .arg("add")
            .arg("-b")
            .arg("feature-branch")
            .arg(&worktree)
            .arg("HEAD")
            .status()
            .expect("git worktree add");

        remove_worktree(&worktree, true).expect("remove worktree");
        assert!(!worktree.exists(), "worktree directory should be removed");

        let branches = std::process::Command::new("git")
            .arg("-C")
            .arg(&repo)
            .args(["branch", "--list", "feature-branch"])
            .output()
            .expect("git branch list");
        assert!(
            String::from_utf8_lossy(&branches.stdout).trim().is_empty(),
            "feature branch should be deleted"
        );
    }
}
