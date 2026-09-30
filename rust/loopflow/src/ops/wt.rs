//! Checkout and branch deletion, shared by Task and PR abandonment.
use std::path::{Path, PathBuf};
use std::process::Command;

use crate::engine::git::{
    acquire_worktree_lease, delete_local_branch, get_default_branch, is_clean, ref_exists,
    worktree_remove_owned,
};
use crate::engine::worktrees::{list_worktrees, main_repo_root, sibling_worktree_name};
use crate::ops::{OpsError, OpsResult, Progress};

#[derive(Debug)]
pub(crate) struct BranchDeletion {
    pub repo: PathBuf,
    pub branch: String,
    worktree: Option<PathBuf>,
    remote_head: Option<String>,
    force: bool,
}

fn git(repo: &Path, args: &[&str]) -> OpsResult<String> {
    let output = Command::new("git").args(args).current_dir(repo).output()?;
    if !output.status.success() {
        return Err(OpsError::Message(format!(
            "git {}: {}",
            args.join(" "),
            String::from_utf8_lossy(&output.stderr).trim()
        )));
    }
    Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
}

pub(crate) fn prepare_delete(
    repo: &Path,
    selector: &str,
    force: bool,
) -> OpsResult<BranchDeletion> {
    let repo = main_repo_root(repo)?;
    let matches = list_worktrees(&repo)?
        .into_iter()
        .filter(|wt| {
            wt.branch.as_deref() == Some(selector)
                || wt.path.to_string_lossy() == selector
                || sibling_worktree_name(&wt.path).as_deref() == Some(selector)
                || wt.path.file_name().is_some_and(|name| name == selector)
        })
        .collect::<Vec<_>>();
    if matches.len() > 1 {
        return Err(OpsError::Message(format!(
            "multiple worktrees match {selector:?}; use the full branch"
        )));
    }
    let worktree = matches.into_iter().next();
    let branch = match &worktree {
        Some(wt) => wt
            .branch
            .clone()
            .ok_or_else(|| OpsError::Message("detached checkout has no branch to delete".into()))?,
        None => selector.to_string(),
    };
    git(
        &repo,
        &["check-ref-format", &format!("refs/heads/{branch}")],
    )?;
    if branch == get_default_branch(&repo)? || worktree.as_ref().is_some_and(|wt| wt.path == repo) {
        return Err(OpsError::Message(
            "cannot delete the primary checkout or default branch".into(),
        ));
    }
    if let Some(wt) = &worktree {
        if !force && !is_clean(&wt.path)? {
            return Err(OpsError::Message(
                "worktree has uncommitted changes; use --force to discard them".into(),
            ));
        }
    }
    let remotes = git(&repo, &["remote"])?;
    let remote_head = if remotes.lines().any(|remote| remote == "origin") {
        let refs = git(
            &repo,
            &[
                "ls-remote",
                "--heads",
                "origin",
                &format!("refs/heads/{branch}"),
            ],
        )?;
        refs.split_whitespace().next().map(str::to_string)
    } else {
        None
    };
    Ok(BranchDeletion {
        repo,
        branch,
        worktree: worktree.map(|wt| wt.path),
        remote_head,
        force,
    })
}

pub(crate) fn apply_delete(deletion: BranchDeletion, progress: &impl Progress) -> OpsResult<()> {
    let BranchDeletion {
        repo,
        branch,
        worktree,
        remote_head,
        force,
    } = deletion;
    let lease = worktree
        .as_ref()
        .map(|path| acquire_worktree_lease(&repo, path, "branch deletion"))
        .transpose()?;
    if let Some(path) = &worktree {
        if !force && !is_clean(path)? {
            return Err(OpsError::Message(
                "worktree changed during deletion; retained its branches".into(),
            ));
        }
    }
    if let Some(head) = remote_head {
        progress.status("Deleting remote branch...");
        // A changed remote is new work, not permission to delete it on this retry.
        git(
            &repo,
            &[
                "push",
                &format!("--force-with-lease=refs/heads/{branch}:{head}"),
                "origin",
                &format!(":refs/heads/{branch}"),
            ],
        )?;
    }
    if let Some(path) = worktree {
        progress.status("Removing worktree...");
        worktree_remove_owned(
            &repo,
            &path,
            lease.as_ref().expect("worktree has a deletion lease"),
        )?;
    }
    if ref_exists(&repo, &format!("refs/heads/{branch}"))? {
        delete_local_branch(&repo, &branch)?;
    }
    Ok(())
}

pub fn delete_worktree(
    repo: &Path,
    selector: &str,
    force: bool,
    progress: &impl Progress,
) -> OpsResult<()> {
    let deletion = prepare_delete(repo, selector, force)?;
    super::task::notice_retained_task(&deletion.repo, &deletion.branch, progress)?;
    progress.status(
        "PR and Task outcomes are unchanged; use pr abandon or task abandon to close them.",
    );
    apply_delete(deletion, progress)
}

#[cfg(test)]
mod tests {
    use super::{apply_delete, git, prepare_delete};
    use crate::ops::NullProgress;
    use loopflow_test_support::TestRepo;

    #[test]
    fn deletion_removes_checkout_and_both_branches_and_retries_without_a_checkout() {
        let fixture = TestRepo::new();
        let directory = tempfile::tempdir().unwrap();
        let checkout = directory.path().join("work");
        git(
            fixture.path(),
            &[
                "worktree",
                "add",
                "-b",
                "cancel-me",
                checkout.to_str().unwrap(),
            ],
        )
        .unwrap();
        git(fixture.path(), &["push", "origin", "cancel-me"]).unwrap();
        let deletion = prepare_delete(&checkout, "cancel-me", false).unwrap();
        apply_delete(deletion, &NullProgress).unwrap();
        assert!(!checkout.exists());
        assert!(git(fixture.path(), &["branch", "--list", "cancel-me"])
            .unwrap()
            .is_empty());
        assert!(git(
            fixture.path(),
            &["ls-remote", "--heads", "origin", "refs/heads/cancel-me"]
        )
        .unwrap()
        .is_empty());
        apply_delete(
            prepare_delete(fixture.path(), "cancel-me", false).unwrap(),
            &NullProgress,
        )
        .unwrap();
    }

    #[test]
    fn deletion_preserves_dirty_primary_and_changed_remote_work() {
        let fixture = TestRepo::new();
        let directory = tempfile::tempdir().unwrap();
        let checkout = directory.path().join("work");
        git(
            fixture.path(),
            &[
                "worktree",
                "add",
                "-b",
                "cancel-me",
                checkout.to_str().unwrap(),
            ],
        )
        .unwrap();
        git(fixture.path(), &["push", "origin", "cancel-me"]).unwrap();
        std::fs::write(checkout.join("draft"), "keep me").unwrap();
        assert!(prepare_delete(fixture.path(), "cancel-me", false).is_err());
        assert!(prepare_delete(fixture.path(), "main", true).is_err());
        assert_eq!(
            std::fs::read_to_string(checkout.join("draft")).unwrap(),
            "keep me"
        );
        let deletion = prepare_delete(fixture.path(), "cancel-me", true).unwrap();
        git(&checkout, &["add", "draft"]).unwrap();
        git(&checkout, &["commit", "-m", "new work"]).unwrap();
        git(&checkout, &["push", "origin", "cancel-me"]).unwrap();
        assert!(apply_delete(deletion, &NullProgress).is_err());
        assert!(checkout.join("draft").exists());
        assert!(!git(
            fixture.path(),
            &["ls-remote", "--heads", "origin", "refs/heads/cancel-me"]
        )
        .unwrap()
        .is_empty());
    }
}
