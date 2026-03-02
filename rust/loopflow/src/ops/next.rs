use std::path::Path;
use std::process::Command;

use crate::engine::config::load_config_or_default;
use crate::engine::git::{
    create_branch, current_branch, get_default_branch, push_with_upstream, sync_main,
};
use crate::engine::naming::{format_branch_name, generate_word_pair};
use crate::engine::worktrees::{branch_exists, main_repo_root, wave_name_from_worktree};

use crate::ops::commit::{commit_workflow, CommitOptions};
use crate::ops::error::{OpsError, OpsResult};
use crate::ops::progress::Progress;

#[derive(Debug, Clone, Default)]
pub struct NextOptions {
    pub create_pr: bool,
    pub rebase: bool,
    /// Wave name override (used when lfd orchestrates). If None, inferred from worktree or branch.
    pub wave_name: Option<String>,
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
        message: Some("lf ops next: checkpoint".to_string()),
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
                refresh: false,
                title: Some(draft_title),
                body: Some("*Draft — title and body will be updated.*".to_string()),
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

    // Generate new branch using schema
    let config = load_config_or_default(Some(repo));
    let branch_config = config.branch_names.as_ref();
    let mut new_branch = format_branch_name(&wave_name, branch_config, repo)
        .map_err(|e| OpsError::Message(format!("failed to generate branch name: {e}")))?;

    // If the generated name already exists (e.g. same-minute timestamp across next runs),
    // append a word pair to ensure uniqueness and easier identification.
    while branch_exists(repo, &new_branch)? {
        new_branch = format!("{new_branch}.{}", generate_word_pair());
    }

    progress.status(&format!("Creating branch: {}", new_branch));
    create_branch(repo, &new_branch)?;
    push_with_upstream(repo, "origin", &new_branch)?;

    Ok(NextResult { new_branch })
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
