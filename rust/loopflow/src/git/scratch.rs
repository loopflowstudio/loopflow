//! Scratch follows the branch diff: existing files are repository content.
use std::path::Path;

use crate::error::GitError;
use crate::git::{get_default_branch, git_stdout, merge_base, rev_parse};

pub(crate) fn base(repo: &Path) -> Result<String, GitError> {
    let branch = get_default_branch(repo)?;
    let remote = format!("refs/remotes/origin/{branch}");
    let target = if rev_parse(repo, &remote).is_ok() {
        remote
    } else {
        format!("refs/heads/{branch}")
    };
    merge_base(repo, "HEAD", &target)
}

/// Renames retain existing content; .gitkeep is never a disposable note.
pub(crate) fn added(repo: &Path, base: &str, tree: &str) -> Result<Vec<String>, GitError> {
    let paths = git_stdout(
        repo,
        &[
            "diff",
            "--name-only",
            "--diff-filter=A",
            "--find-renames",
            "-z",
            base,
            tree,
        ],
    )?;
    Ok(paths
        .split('\0')
        // Detect renames across the whole tree before selecting scratch paths.
        .filter(|path| path.starts_with("scratch/") && *path != "scratch/.gitkeep")
        .map(str::to_owned)
        .collect())
}
