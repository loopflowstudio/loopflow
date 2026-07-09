use std::path::Path;
use std::process::Command;

use crate::engine::git::{
    create_branch, current_branch, get_default_branch, push_with_upstream, sync_main,
};
use crate::engine::worktrees::{fresh_stamped_branch, main_repo_root, wave_name_from_worktree};

use crate::ops::commit::{commit_workflow, CommitOptions};
use crate::ops::error::{OpsError, OpsResult};
use crate::ops::progress::Progress;

#[derive(Debug, Clone, Default)]
pub struct NextOptions {
    pub create_pr: bool,
    pub rebase: bool,
    /// Wave name override (used when lfd orchestrates). If None, inferred from worktree or branch.
    pub wave_name: Option<String>,
    pub agent: Option<String>,
}

#[derive(Debug, Clone)]
pub struct NextResult {
    pub new_branch: String,
}

pub fn next_branch(
    repo: &Path,
    options: &NextOptions,
    progress: &impl Progress,
) -> OpsResult<NextResult> {
    let main_repo = main_repo_root(repo).unwrap_or_else(|_| repo.to_path_buf());
    let base_branch = get_default_branch(&main_repo)?;
    let current =
        current_branch(repo)?.ok_or_else(|| OpsError::Message("not on a branch".to_string()))?;

    if current == base_branch {
        return Err(OpsError::Message(format!(
            "cannot run next from {}",
            base_branch
        )));
    }

    if let Some(pr_number) = current_pr_number(repo)? {
        if let Some(state) = pr_state(repo, pr_number)? {
            if state.to_uppercase() == "MERGED" {
                progress.status("PR already merged, starting fresh from main...");
                reset_to_main(repo, &base_branch)?;
            }
        }
    }

    let commit_options = CommitOptions {
        add: true,
        push: true,
        create_draft_pr: true,
        message: Some("lf wt: checkpoint".to_string()),
        agent: options.agent.clone(),
        ..CommitOptions::for_task("commit")
    };
    let _ = commit_workflow(repo, &commit_options, progress)?;

    if options.rebase {
        crate::ops::rebase::rebase_with_recovery(
            repo,
            &crate::ops::rebase::RebaseOptions {
                onto: format!("origin/{base_branch}"),
                push: true,
            },
            progress,
        )?;
    }

    if current_pr_number(repo)?.is_none() && options.create_pr {
        let wave = options
            .wave_name
            .clone()
            .or_else(|| wave_name_from_worktree(repo));
        let draft_title = wave
            .map(|name| format!("{name}: draft"))
            .unwrap_or_else(|| current.clone());
        let _ = crate::ops::pr::create_or_update_pr(
            repo,
            &crate::ops::pr::PrOptions {
                title: Some(draft_title),
                body: Some("*Draft — title and body will be updated.*".to_string()),
                agent: options.agent.clone(),
            },
            progress,
        )?;
    }

    // Infer wave name: explicit > worktree directory > current branch
    let wave_name = options
        .wave_name
        .clone()
        .or_else(|| wave_name_from_worktree(repo))
        .unwrap_or(current.clone());

    let new_branch = fresh_stamped_branch(repo, &wave_name)?;

    progress.status(&format!("Creating branch: {}", new_branch));
    create_branch(repo, &new_branch)?;
    push_with_upstream(repo, "origin", &new_branch)?;

    Ok(NextResult { new_branch })
}

/// Rotate a recurring wave onto a fresh schema-named branch: generate the
/// next branch name (de-colliding with word pairs), create it in the
/// worktree, push it with upstream. Returns the new branch name.
pub fn advance_branch(worktree: &Path, wave_name: &str) -> OpsResult<String> {
    let new_branch = fresh_stamped_branch(worktree, wave_name)?;

    create_branch(worktree, &new_branch)?;
    push_with_upstream(worktree, "origin", &new_branch)?;
    Ok(new_branch)
}

fn current_pr_number(repo: &Path) -> OpsResult<Option<u64>> {
    let output = Command::new("gh")
        .arg("pr")
        .arg("view")
        .arg("--json")
        .arg("number")
        .arg("-q")
        .arg(".number")
        .current_dir(repo)
        .output()?;
    if !output.status.success() {
        return Ok(None);
    }
    let raw = String::from_utf8_lossy(&output.stdout).trim().to_string();
    if raw.is_empty() {
        Ok(None)
    } else {
        Ok(raw.parse::<u64>().ok())
    }
}

fn pr_state(repo: &Path, number: u64) -> OpsResult<Option<String>> {
    let output = Command::new("gh")
        .arg("pr")
        .arg("view")
        .arg(number.to_string())
        .arg("--json")
        .arg("state")
        .arg("-q")
        .arg(".state")
        .current_dir(repo)
        .output()?;
    if !output.status.success() {
        return Ok(None);
    }
    let state = String::from_utf8_lossy(&output.stdout).trim().to_string();
    if state.is_empty() {
        Ok(None)
    } else {
        Ok(Some(state))
    }
}

fn reset_to_main(repo: &Path, base_branch: &str) -> OpsResult<()> {
    let status = Command::new("git")
        .args(["checkout", base_branch])
        .current_dir(repo)
        .status()?;
    if !status.success() {
        return Err(OpsError::Message(format!(
            "failed to checkout {}",
            base_branch
        )));
    }
    if !sync_main(repo, base_branch)? {
        return Err(OpsError::Message(
            "working tree dirty; sync aborted".to_string(),
        ));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use std::path::{Path, PathBuf};
    use std::process::Command;

    use tempfile::TempDir;

    use super::advance_branch;
    use crate::engine::git::current_branch;

    fn run_git(dir: &Path, args: &[&str]) {
        let output = Command::new("git")
            .args(args)
            .current_dir(dir)
            .output()
            .expect("git should run");
        assert!(
            output.status.success(),
            "git {:?} failed: {}",
            args,
            String::from_utf8_lossy(&output.stderr)
        );
    }

    fn repo_with_origin() -> (TempDir, PathBuf) {
        let temp = TempDir::new().expect("temp dir");
        let origin = temp.path().join("origin.git");
        let repo = temp.path().join("repo");
        std::fs::create_dir_all(&repo).expect("create repo dir");
        run_git(temp.path(), &["init", "--bare", "-b", "main", "origin.git"]);
        run_git(&repo, &["init", "-b", "main"]);
        run_git(&repo, &["config", "user.email", "test@example.com"]);
        run_git(&repo, &["config", "user.name", "Test User"]);
        std::fs::write(repo.join("README.md"), "seed\n").expect("seed file");
        run_git(&repo, &["add", "."]);
        run_git(&repo, &["commit", "-m", "init"]);
        run_git(
            &repo,
            &["remote", "add", "origin", origin.to_str().expect("origin")],
        );
        run_git(&repo, &["push", "-u", "origin", "main"]);
        (temp, repo)
    }

    #[test]
    fn advance_branch_rotates_onto_fresh_pushed_branch() {
        let (_temp, repo) = repo_with_origin();

        let first = advance_branch(&repo, "goals").expect("first rotation");
        assert!(
            first.contains("goals"),
            "schema name carries the wave: {first}"
        );
        assert_eq!(current_branch(&repo).expect("branch"), Some(first.clone()));

        // The rotation pushed with upstream: the branch exists on origin.
        let remote = Command::new("git")
            .args(["ls-remote", "--heads", "origin", &first])
            .current_dir(&repo)
            .output()
            .expect("ls-remote");
        assert!(
            String::from_utf8_lossy(&remote.stdout).contains(&first),
            "rotated branch is pushed to origin"
        );

        // A second rotation de-collides instead of failing.
        let second = advance_branch(&repo, "goals").expect("second rotation");
        assert_ne!(first, second);
    }
}
