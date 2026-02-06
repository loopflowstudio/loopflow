use std::path::Path;
use std::process::Command;
use std::time::Duration;

use loopflow_engine::config::load_config_or_default;
use loopflow_engine::git::{create_branch, current_branch, get_default_branch, push_with_upstream};
use loopflow_engine::naming::format_branch_name;
use loopflow_engine::worktrees::{main_repo_root, worktree_short_name};

use crate::commit::{commit_workflow, CommitOptions};
use crate::error::{OpsError, OpsResult};
use crate::messages::generate_pr_message;
use crate::progress::Progress;

#[derive(Debug, Clone, Default)]
pub struct NextOptions {
    pub block: bool,
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

    let commit_options = CommitOptions {
        add: true,
        lint: false,
        push: true,
        create_draft_pr: true,
        task: "commit".to_string(),
        flow_parents: Vec::new(),
        message: None,
    };
    let _ = commit_workflow(repo, &commit_options, progress)?;

    if options.rebase {
        crate::rebase::rebase_with_recovery(
            repo,
            &crate::rebase::RebaseOptions {
                onto: format!("origin/{base_branch}"),
                push: true,
            },
            progress,
        )?;
    }

    if let Some(pr_number) = current_pr_number(repo)? {
        enable_auto_merge(repo, pr_number, progress)?;
        if options.block {
            wait_for_merge(repo, pr_number, progress)?;
        }
    } else if options.create_pr {
        let _ = crate::pr::create_or_update_pr(
            repo,
            &crate::pr::PrOptions {
                refresh: true,
                lint: false,
            },
            progress,
        )?;
        if let Some(pr_number) = current_pr_number(repo)? {
            enable_auto_merge(repo, pr_number, progress)?;
        }
    }

    // Infer wave name: explicit > worktree directory > current branch
    let wave_name = options
        .wave_name
        .clone()
        .or_else(|| worktree_short_name(repo))
        .unwrap_or(current.clone());

    // Generate new branch using schema
    let config = load_config_or_default(Some(repo));
    let branch_config = config.branch_names.as_ref();
    let new_branch = format_branch_name(&wave_name, branch_config, repo)
        .map_err(|e| OpsError::Message(format!("failed to generate branch name: {e}")))?;

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

fn enable_auto_merge(repo: &Path, number: u64, progress: &impl Progress) -> OpsResult<()> {
    progress.status("Refreshing PR...");
    let message = generate_pr_message(repo)?;

    let _ = Command::new("gh")
        .arg("pr")
        .arg("edit")
        .arg(number.to_string())
        .arg("--title")
        .arg(&message.title)
        .arg("--body")
        .arg(&message.body)
        .current_dir(repo)
        .status();

    let mut cmd = Command::new("gh");
    cmd.arg("pr")
        .arg("merge")
        .arg(number.to_string())
        .arg("--squash")
        .arg("--auto")
        .arg("--subject")
        .arg(&message.title);
    if !message.body.trim().is_empty() {
        cmd.arg("--body").arg(&message.body);
    }
    let status = cmd.current_dir(repo).status()?;
    if !status.success() {
        return Err(OpsError::Message("failed to enable auto-merge".to_string()));
    }
    Ok(())
}

fn wait_for_merge(repo: &Path, number: u64, progress: &impl Progress) -> OpsResult<()> {
    progress.status("Waiting for PR to merge...");
    let start = std::time::Instant::now();
    let timeout = Duration::from_secs(600);
    while start.elapsed() < timeout {
        if let Some(state) = pr_state(repo, number)? {
            let state = state.to_uppercase();
            if state == "MERGED" {
                progress.status("PR merged");
                return Ok(());
            }
            if state == "CLOSED" {
                return Err(OpsError::Message("PR closed without merge".to_string()));
            }
        }
        std::thread::sleep(Duration::from_secs(5));
    }
    Err(OpsError::Message("timeout waiting for merge".to_string()))
}
