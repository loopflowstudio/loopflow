use std::path::{Path, PathBuf};
use std::process::{Command, Output};

use serde::Serialize;

use crate::error::{CoreError, GitError};

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct RebaseResult {
    pub success: bool,
    pub conflicts: Option<Vec<PathBuf>>,
    pub new_head: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct BranchInfo {
    pub old_branch: String,
    pub old_head: String,
    pub new_branch: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, clap::ValueEnum)]
#[serde(rename_all = "snake_case")]
pub enum LandStrategy {
    SquashMerge,
    LocalMerge,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct LandResult {
    pub merged_commit: String,
    pub branch_deleted: bool,
}

fn run_git(repo: &Path, args: &[&str]) -> Result<Output, GitError> {
    Ok(Command::new("git")
        .arg("-C")
        .arg(repo)
        .args(args)
        .output()?)
}

fn git_stdout(repo: &Path, args: &[&str]) -> Result<String, GitError> {
    let output = run_git(repo, args)?;
    if !output.status.success() {
        return Err(GitError::CommandFailed {
            command: format!("git {}", args.join(" ")),
            stderr: String::from_utf8_lossy(&output.stderr).to_string(),
        });
    }
    Ok(String::from_utf8_lossy(&output.stdout).to_string())
}

fn list_conflicts(repo: &Path) -> Result<Vec<PathBuf>, GitError> {
    let output = git_stdout(repo, &["diff", "--name-only", "--diff-filter=U"])?;
    let conflicts = output
        .lines()
        .filter(|line| !line.trim().is_empty())
        .map(PathBuf::from)
        .collect::<Vec<_>>();
    Ok(conflicts)
}

pub fn get_status(repo: &Path) -> Result<String, CoreError> {
    let output = run_git(repo, &["status", "--porcelain"])?;
    if !output.status.success() {
        return Err(CoreError::ExecutionFailed("git status failed".to_string()));
    }
    Ok(String::from_utf8_lossy(&output.stdout).to_string())
}

pub fn get_diff(repo: &Path) -> Result<String, CoreError> {
    let output = run_git(repo, &["diff"])?;
    if !output.status.success() {
        return Err(CoreError::ExecutionFailed("git diff failed".to_string()));
    }
    Ok(String::from_utf8_lossy(&output.stdout).to_string())
}

/// Fetch a remote ref (e.g., "origin/main").
pub fn fetch(repo: &Path, remote: &str, refspec: &str) -> Result<(), GitError> {
    let output = run_git(repo, &["fetch", remote, refspec])?;
    if !output.status.success() {
        return Err(GitError::CommandFailed {
            command: format!("git fetch {} {}", remote, refspec),
            stderr: String::from_utf8_lossy(&output.stderr).to_string(),
        });
    }
    Ok(())
}

/// Check if `commit` is an ancestor of `descendant`.
/// Returns true if commit is fully merged into descendant.
pub fn is_ancestor(repo: &Path, commit: &str, descendant: &str) -> Result<bool, GitError> {
    let output = run_git(repo, &["merge-base", "--is-ancestor", commit, descendant])?;
    Ok(output.status.success())
}

/// Checkout a ref (branch, tag, or commit).
pub fn checkout(repo: &Path, ref_name: &str) -> Result<(), GitError> {
    git_stdout(repo, &["checkout", ref_name])?;
    Ok(())
}

/// Create and checkout a new branch from current HEAD.
pub fn checkout_new_branch(repo: &Path, branch: &str) -> Result<(), GitError> {
    git_stdout(repo, &["checkout", "-b", branch])?;
    Ok(())
}

/// Push and set upstream tracking.
pub fn push_with_upstream(repo: &Path, remote: &str, branch: &str) -> Result<(), GitError> {
    let output = run_git(repo, &["push", "-u", remote, branch])?;
    if !output.status.success() {
        return Err(GitError::CommandFailed {
            command: format!("git push -u {} {}", remote, branch),
            stderr: String::from_utf8_lossy(&output.stderr).to_string(),
        });
    }
    Ok(())
}

/// Get current branch name. Returns None if in detached HEAD state.
pub fn current_branch(repo: &Path) -> Result<Option<String>, GitError> {
    let output = run_git(repo, &["symbolic-ref", "--short", "HEAD"])?;
    if output.status.success() {
        Ok(Some(
            String::from_utf8_lossy(&output.stdout).trim().to_string(),
        ))
    } else {
        // Detached HEAD
        Ok(None)
    }
}

pub fn rebase(
    worktree: &Path,
    onto: &str,
    base_commit: Option<&str>,
) -> Result<RebaseResult, GitError> {
    let mut args = vec!["rebase"];
    if let Some(base) = base_commit {
        args.extend(["--onto", onto, base]);
    } else {
        args.push(onto);
    }
    let output = run_git(worktree, &args)?;
    if output.status.success() {
        let new_head = git_stdout(worktree, &["rev-parse", "HEAD"])?
            .trim()
            .to_string();
        return Ok(RebaseResult {
            success: true,
            conflicts: None,
            new_head: Some(new_head),
        });
    }

    let conflicts = list_conflicts(worktree)?;
    let _ = run_git(worktree, &["rebase", "--abort"])?;
    Ok(RebaseResult {
        success: false,
        conflicts: if conflicts.is_empty() {
            None
        } else {
            Some(conflicts)
        },
        new_head: None,
    })
}

pub fn create_branch(worktree: &Path, name: &str) -> Result<BranchInfo, GitError> {
    let old_branch = git_stdout(worktree, &["rev-parse", "--abbrev-ref", "HEAD"])?
        .trim()
        .to_string();
    let old_head = git_stdout(worktree, &["rev-parse", "HEAD"])?
        .trim()
        .to_string();
    git_stdout(worktree, &["checkout", "-b", name])?;

    Ok(BranchInfo {
        old_branch,
        old_head,
        new_branch: name.to_string(),
    })
}

pub fn push(worktree: &Path, force_with_lease: bool) -> Result<(), GitError> {
    let mut args = vec!["push"];
    if force_with_lease {
        args.push("--force-with-lease");
    }
    git_stdout(worktree, &args)?;
    Ok(())
}

pub fn land(
    worktree: &Path,
    strategy: LandStrategy,
    main_branch: &str,
) -> Result<LandResult, GitError> {
    let feature_branch = git_stdout(worktree, &["rev-parse", "--abbrev-ref", "HEAD"])?
        .trim()
        .to_string();
    git_stdout(worktree, &["checkout", main_branch])?;
    match strategy {
        LandStrategy::SquashMerge => {
            git_stdout(worktree, &["merge", "--squash", &feature_branch])?;
            git_stdout(
                worktree,
                &["commit", "-m", &format!("Squash merge {}", feature_branch)],
            )?;
        }
        LandStrategy::LocalMerge => {
            git_stdout(
                worktree,
                &[
                    "merge",
                    "--no-ff",
                    &feature_branch,
                    "-m",
                    &format!("Merge {}", feature_branch),
                ],
            )?;
        }
    }
    let merged_commit = git_stdout(worktree, &["rev-parse", "HEAD"])?
        .trim()
        .to_string();
    let delete_output = run_git(worktree, &["branch", "-d", &feature_branch])?;
    let branch_deleted = delete_output.status.success();
    Ok(LandResult {
        merged_commit,
        branch_deleted,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    fn init_repo() -> TempDir {
        let dir = tempfile::tempdir().expect("create temp dir");
        git_stdout(dir.path(), &["init", "-b", "main"]).expect("git init");
        git_stdout(
            dir.path(),
            &["config", "user.email", "loopflow@example.com"],
        )
        .expect("set user.email");
        git_stdout(dir.path(), &["config", "user.name", "Loopflow"]).expect("set user.name");
        dir
    }

    fn commit_file(repo: &Path, name: &str, content: &str) {
        let path = repo.join(name);
        fs::write(&path, content).expect("write file");
        git_stdout(repo, &["add", name]).expect("git add");
        git_stdout(repo, &["commit", "-m", &format!("add {}", name)]).expect("git commit");
    }

    #[test]
    fn git_create_branch_records_old_state() {
        let repo = init_repo();
        commit_file(repo.path(), "README.md", "hello");

        let info = create_branch(repo.path(), "feature").expect("create branch");
        assert_eq!(info.old_branch, "main");
        assert_eq!(info.new_branch, "feature");
        assert!(!info.old_head.is_empty());
    }

    #[test]
    fn git_rebase_succeeds_on_linear_history() {
        let repo = init_repo();
        commit_file(repo.path(), "README.md", "hello");
        create_branch(repo.path(), "feature").expect("create branch");
        commit_file(repo.path(), "feature.txt", "feature");

        git_stdout(repo.path(), &["checkout", "main"]).expect("checkout main");
        commit_file(repo.path(), "main.txt", "main");

        git_stdout(repo.path(), &["checkout", "feature"]).expect("checkout feature");
        let result = rebase(repo.path(), "main", None).expect("rebase");
        assert!(result.success);
        assert!(result.new_head.is_some());
    }

    #[test]
    fn git_push_force_with_lease() {
        let repo = init_repo();
        commit_file(repo.path(), "README.md", "hello");

        let remote_dir = tempfile::tempdir().expect("remote dir");
        git_stdout(remote_dir.path(), &["init", "--bare"]).expect("init bare");

        git_stdout(
            repo.path(),
            &[
                "remote",
                "add",
                "origin",
                remote_dir.path().to_str().expect("remote path"),
            ],
        )
        .expect("add remote");

        push(repo.path(), true).expect("push");
    }

    #[test]
    fn git_is_ancestor_returns_true_when_merged() {
        let repo = init_repo();
        commit_file(repo.path(), "README.md", "hello");
        let base_commit = git_stdout(repo.path(), &["rev-parse", "HEAD"])
            .unwrap()
            .trim()
            .to_string();

        create_branch(repo.path(), "feature").expect("create branch");
        commit_file(repo.path(), "feature.txt", "feature");

        // feature is ahead of base_commit, so base_commit is ancestor of feature
        assert!(is_ancestor(repo.path(), &base_commit, "feature").unwrap());

        // feature is not an ancestor of base_commit
        assert!(!is_ancestor(repo.path(), "feature", &base_commit).unwrap());
    }

    #[test]
    fn git_is_ancestor_after_merge() {
        let repo = init_repo();
        commit_file(repo.path(), "README.md", "hello");

        create_branch(repo.path(), "feature").expect("create branch");
        commit_file(repo.path(), "feature.txt", "feature");
        let feature_head = git_stdout(repo.path(), &["rev-parse", "HEAD"])
            .unwrap()
            .trim()
            .to_string();

        // Merge feature into main
        git_stdout(repo.path(), &["checkout", "main"]).expect("checkout main");
        git_stdout(repo.path(), &["merge", "feature", "--no-ff", "-m", "merge"]).expect("merge");

        // Now feature is an ancestor of main
        assert!(is_ancestor(repo.path(), &feature_head, "main").unwrap());
    }

    #[test]
    fn git_checkout_and_checkout_new_branch() {
        let repo = init_repo();
        commit_file(repo.path(), "README.md", "hello");

        checkout_new_branch(repo.path(), "feature").expect("create feature");
        assert_eq!(
            current_branch(repo.path()).unwrap(),
            Some("feature".to_string())
        );

        checkout(repo.path(), "main").expect("checkout main");
        assert_eq!(
            current_branch(repo.path()).unwrap(),
            Some("main".to_string())
        );
    }

    #[test]
    fn git_current_branch_detached_head() {
        let repo = init_repo();
        commit_file(repo.path(), "README.md", "hello");
        let commit = git_stdout(repo.path(), &["rev-parse", "HEAD"])
            .unwrap()
            .trim()
            .to_string();

        // Detach HEAD
        git_stdout(repo.path(), &["checkout", &commit]).expect("detach");
        assert_eq!(current_branch(repo.path()).unwrap(), None);
    }

    #[test]
    fn git_push_with_upstream() {
        let repo = init_repo();
        commit_file(repo.path(), "README.md", "hello");

        let remote_dir = tempfile::tempdir().expect("remote dir");
        git_stdout(remote_dir.path(), &["init", "--bare"]).expect("init bare");
        git_stdout(
            repo.path(),
            &[
                "remote",
                "add",
                "origin",
                remote_dir.path().to_str().expect("remote path"),
            ],
        )
        .expect("add remote");

        checkout_new_branch(repo.path(), "feature").expect("create feature");
        commit_file(repo.path(), "feature.txt", "feature");
        push_with_upstream(repo.path(), "origin", "feature").expect("push with upstream");

        // Verify upstream is set
        let tracking = git_stdout(
            repo.path(),
            &["rev-parse", "--abbrev-ref", "feature@{upstream}"],
        )
        .expect("get upstream");
        assert_eq!(tracking.trim(), "origin/feature");
    }
}
