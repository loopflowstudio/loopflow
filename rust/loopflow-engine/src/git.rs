use std::path::{Path, PathBuf};
use std::process::{Command, Output};

use serde::Serialize;

use crate::error::GitError;

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

fn run_gh(repo: &Path, args: &[&str]) -> Result<Output, GitError> {
    Ok(Command::new("gh").args(args).current_dir(repo).output()?)
}

fn gh_stdout(repo: &Path, args: &[&str]) -> Result<String, GitError> {
    let output = run_gh(repo, args)?;
    if !output.status.success() {
        return Err(GitError::CommandFailed {
            command: format!("gh {}", args.join(" ")),
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

pub fn delete_remote_branch(repo: &Path, remote: &str, branch: &str) -> Result<(), GitError> {
    let output = run_git(repo, &["push", remote, "--delete", branch])?;
    if !output.status.success() {
        return Err(GitError::CommandFailed {
            command: format!("git push {} --delete {}", remote, branch),
            stderr: String::from_utf8_lossy(&output.stderr).to_string(),
        });
    }
    Ok(())
}

pub fn delete_local_branch(repo: &Path, branch: &str) -> Result<(), GitError> {
    let output = run_git(repo, &["branch", "-D", branch])?;
    if !output.status.success() {
        return Err(GitError::CommandFailed {
            command: format!("git branch -D {}", branch),
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

/// Return default branch name from origin/HEAD. Falls back to "main".
pub fn get_default_branch(repo: &Path) -> Result<String, GitError> {
    let output = run_git(
        repo,
        &[
            "symbolic-ref",
            "--quiet",
            "--short",
            "refs/remotes/origin/HEAD",
        ],
    )?;
    if !output.status.success() {
        return Ok("main".to_string());
    }
    let raw = String::from_utf8_lossy(&output.stdout).trim().to_string();
    if raw.is_empty() {
        return Ok("main".to_string());
    }
    if let Some((_, branch)) = raw.split_once('/') {
        if !branch.trim().is_empty() {
            return Ok(branch.trim().to_string());
        }
    }
    Ok(raw)
}

/// Return true if working tree is clean.
pub fn is_clean(repo: &Path) -> Result<bool, GitError> {
    let output = run_git(repo, &["status", "--porcelain"])?;
    if !output.status.success() {
        return Err(GitError::CommandFailed {
            command: "git status --porcelain".to_string(),
            stderr: String::from_utf8_lossy(&output.stderr).to_string(),
        });
    }
    Ok(String::from_utf8_lossy(&output.stdout).trim().is_empty())
}

/// Stage all changes.
pub fn stage_all(repo: &Path) -> Result<(), GitError> {
    git_stdout(repo, &["add", "-A"])?;
    Ok(())
}

/// Commit with message.
pub fn commit(repo: &Path, message: &str) -> Result<(), GitError> {
    git_stdout(repo, &["commit", "-m", message])?;
    Ok(())
}

pub fn pr_exists(repo: &Path) -> Result<bool, GitError> {
    let output = run_gh(repo, &["pr", "view", "--json", "state", "-q", ".state"])?;
    if !output.status.success() {
        return Ok(false);
    }
    Ok(!String::from_utf8_lossy(&output.stdout).trim().is_empty())
}

pub fn pr_create_draft(repo: &Path) -> Result<String, GitError> {
    let url = gh_stdout(repo, &["pr", "create", "--draft", "--fill"])?;
    Ok(url.trim().to_string())
}

pub fn pr_merge_squash_auto(repo: &Path) -> Result<(), GitError> {
    gh_stdout(repo, &["pr", "merge", "--squash", "--auto"])?;
    Ok(())
}

/// Fetch origin/main_branch and reset if main_branch is checked out.
/// Returns true if up-to-date or updated, false if local branch can't be reset.
pub fn sync_main(repo: &Path, main_branch: &str) -> Result<bool, GitError> {
    let output = run_git(repo, &["fetch", "origin", main_branch])?;
    if !output.status.success() {
        return Err(GitError::CommandFailed {
            command: format!("git fetch origin {}", main_branch),
            stderr: String::from_utf8_lossy(&output.stderr).to_string(),
        });
    }

    if current_branch(repo)? != Some(main_branch.to_string()) {
        return Ok(true);
    }

    if !is_clean(repo)? {
        return Ok(false);
    }

    let output = run_git(
        repo,
        &["reset", "--hard", &format!("origin/{}", main_branch)],
    )?;
    Ok(output.status.success())
}

pub fn worktree_remove(repo: &Path, path: &Path) -> Result<(), GitError> {
    let path_str = path.to_string_lossy();
    let output = run_git(repo, &["worktree", "remove", "--force", path_str.as_ref()])?;
    if !output.status.success() {
        return Err(GitError::CommandFailed {
            command: format!("git worktree remove --force {}", path.to_string_lossy()),
            stderr: String::from_utf8_lossy(&output.stderr).to_string(),
        });
    }
    Ok(())
}

/// Move a worktree to a new path.
pub fn worktree_move(repo: &Path, old_path: &Path, new_path: &Path) -> Result<(), GitError> {
    let old_str = old_path.to_string_lossy();
    let new_str = new_path.to_string_lossy();
    let output = run_git(
        repo,
        &["worktree", "move", old_str.as_ref(), new_str.as_ref()],
    )?;
    if !output.status.success() {
        return Err(GitError::CommandFailed {
            command: format!("git worktree move {} {}", old_str, new_str),
            stderr: String::from_utf8_lossy(&output.stderr).to_string(),
        });
    }
    Ok(())
}

/// Create a new worktree with a new branch.
pub fn worktree_add(
    repo: &Path,
    path: &Path,
    branch: &str,
    start_point: &str,
) -> Result<(), GitError> {
    let path_str = path.to_string_lossy();
    let output = run_git(
        repo,
        &[
            "worktree",
            "add",
            "-b",
            branch,
            path_str.as_ref(),
            start_point,
        ],
    )?;
    if !output.status.success() {
        // Apple Git 2.50.1 returns exit code 1 even on successful creation.
        // Verify the worktree was actually created before reporting failure.
        if path.join(".git").exists() {
            return Ok(());
        }
        return Err(GitError::CommandFailed {
            command: format!(
                "git worktree add -b {} {} {}",
                branch, path_str, start_point
            ),
            stderr: String::from_utf8_lossy(&output.stderr).to_string(),
        });
    }
    Ok(())
}

/// Get the SHA for a ref (branch, tag, HEAD, etc.).
pub fn rev_parse(repo: &Path, refspec: &str) -> Result<String, GitError> {
    let sha = git_stdout(repo, &["rev-parse", refspec])?
        .trim()
        .to_string();
    Ok(sha)
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

        // Initial push to set upstream
        push_with_upstream(repo.path(), "origin", "main").expect("initial push");

        // Now test force-with-lease push
        commit_file(repo.path(), "second.txt", "second commit");
        push(repo.path(), true).expect("push force-with-lease");
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
    fn git_get_default_branch_falls_back_to_main() {
        let repo = init_repo();
        let branch = get_default_branch(repo.path()).expect("default branch");
        assert_eq!(branch, "main");
    }

    #[test]
    fn git_is_clean_tracks_changes() {
        let repo = init_repo();
        assert!(is_clean(repo.path()).expect("clean repo"));

        let path = repo.path().join("dirty.txt");
        fs::write(&path, "dirty").expect("write file");
        assert!(!is_clean(repo.path()).expect("dirty repo"));
    }

    #[test]
    fn git_stage_all_and_commit() {
        let repo = init_repo();
        let path = repo.path().join("stage.txt");
        fs::write(&path, "staged").expect("write file");

        stage_all(repo.path()).expect("stage all");
        commit(repo.path(), "add staged").expect("commit");
        assert!(is_clean(repo.path()).expect("clean after commit"));
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

    #[test]
    fn git_delete_local_branch() {
        let repo = init_repo();
        commit_file(repo.path(), "README.md", "hello");

        checkout_new_branch(repo.path(), "feature").expect("create feature");
        commit_file(repo.path(), "feature.txt", "feature");
        checkout(repo.path(), "main").expect("checkout main");

        delete_local_branch(repo.path(), "feature").expect("delete branch");
    }

    #[test]
    fn git_rev_parse_returns_sha() {
        let repo = init_repo();
        commit_file(repo.path(), "README.md", "hello");

        let sha = rev_parse(repo.path(), "HEAD").expect("rev-parse HEAD");
        assert!(!sha.is_empty());
        assert!(sha.chars().all(|c| c.is_ascii_hexdigit()));
    }

    #[test]
    fn git_worktree_move_and_add() {
        let repo = init_repo();
        commit_file(repo.path(), "README.md", "hello");

        // Create a worktree
        let wt_path = repo.path().parent().unwrap().join("test-worktree");
        checkout_new_branch(repo.path(), "feature").expect("create feature");
        checkout(repo.path(), "main").expect("back to main");

        git_stdout(
            repo.path(),
            &["worktree", "add", wt_path.to_str().unwrap(), "feature"],
        )
        .expect("create worktree");

        // Move the worktree
        let new_path = repo.path().parent().unwrap().join("moved-worktree");
        worktree_move(repo.path(), &wt_path, &new_path).expect("move worktree");

        // Verify old path doesn't exist, new path does
        assert!(!wt_path.exists());
        assert!(new_path.exists());

        // Clean up
        worktree_remove(repo.path(), &new_path).expect("remove worktree");
    }

    #[test]
    fn git_worktree_add_creates_branch() {
        let repo = init_repo();
        commit_file(repo.path(), "README.md", "hello");

        let wt_path = repo.path().parent().unwrap().join("new-worktree");
        worktree_add(repo.path(), &wt_path, "new-feature", "HEAD").expect("worktree add");

        // Verify worktree exists
        assert!(wt_path.exists());

        // Verify branch was created
        let branch = current_branch(&wt_path).expect("get branch");
        assert_eq!(branch, Some("new-feature".to_string()));

        // Clean up
        worktree_remove(repo.path(), &wt_path).expect("remove worktree");
    }
}
