use std::path::Path;
use std::process::Command;

use loopflow_engine::git::{
    current_branch, delete_local_branch, delete_remote_branch, is_clean, worktree_remove,
};
use loopflow_engine::worktrees::{list_worktrees, main_repo_root};

use crate::error::{OpsError, OpsResult};
use crate::progress::Progress;

#[derive(Debug, Clone)]
pub struct AbandonOptions {
    pub branch: Option<String>,
    pub force: bool,
}

pub fn abandon_branch(
    repo: &Path,
    options: &AbandonOptions,
    progress: &impl Progress,
) -> OpsResult<()> {
    let main_repo = main_repo_root(repo).unwrap_or_else(|_| repo.to_path_buf());
    let branch = match options.branch.as_deref() {
        Some(branch) => branch.to_string(),
        None => {
            current_branch(repo)?.ok_or_else(|| OpsError::Message("not on a branch".to_string()))?
        }
    };

    let worktree_path = find_worktree_path(&main_repo, &branch)?
        .ok_or_else(|| OpsError::Message(format!("no worktree found for '{}'", branch)))?;

    if !is_clean(&worktree_path)? {
        if !options.force {
            return Err(OpsError::Message(
                "uncommitted changes; use --force".to_string(),
            ));
        }
        progress.error("Abandoning worktree with uncommitted changes");
    }

    if !options.force {
        let confirmed = progress.confirm(&format!(
            "Abandon branch '{}' (close PR, delete remote, remove worktree)?",
            branch
        ));
        if !confirmed {
            return Err(OpsError::Message("aborted".to_string()));
        }
    }

    progress.status("Closing PR...");
    let _ = Command::new("gh")
        .arg("pr")
        .arg("close")
        .arg(&branch)
        .current_dir(&main_repo)
        .status();

    progress.status("Deleting remote branch...");
    let _ = delete_remote_branch(&main_repo, "origin", &branch);

    progress.status("Removing worktree...");
    let _ = worktree_remove(&main_repo, &worktree_path);

    progress.status("Deleting local branch...");
    let _ = delete_local_branch(&main_repo, &branch);

    Ok(())
}

fn find_worktree_path(main_repo: &Path, branch: &str) -> OpsResult<Option<std::path::PathBuf>> {
    let worktrees = list_worktrees(main_repo)?;
    Ok(worktrees
        .into_iter()
        .find(|wt| wt.branch.as_deref() == Some(branch))
        .map(|wt| wt.path))
}
