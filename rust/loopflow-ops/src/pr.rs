use std::path::Path;
use std::process::Command;

use serde::Deserialize;

use loopflow_engine::git::{current_branch, get_default_branch, sync_main};
use loopflow_engine::worktrees::{list_worktrees, main_repo_root};

use crate::commit::{commit_workflow, CommitOptions};
use crate::error::{OpsError, OpsResult};
use crate::messages::{generate_pr_message, generate_pr_message_from_diff};
use crate::progress::Progress;
use crate::util::{command_exists, stderr_from_output};

#[derive(Debug, Clone)]
pub struct PrOptions {
    pub refresh: bool,
    pub lint: bool,
}

#[derive(Debug, Clone)]
pub struct PrResult {
    pub url: String,
    pub created: bool,
    pub updated: bool,
}

#[derive(Debug, Deserialize)]
struct GhPr {
    url: String,
    state: String,
    #[serde(default)]
    is_draft: bool,
    number: u64,
}

pub fn create_or_update_pr(
    repo: &Path,
    options: &PrOptions,
    progress: &impl Progress,
) -> OpsResult<PrResult> {
    if !gh_available() {
        return Err(OpsError::Message("gh CLI not found".to_string()));
    }

    if options.lint {
        crate::lint::ensure_lint_passes(repo, progress)?;
    }

    sync_main_repo(repo, progress)?;

    let commit_options = CommitOptions {
        add: true,
        lint: false,
        push: true,
        create_draft_pr: false,
        task: "commit".to_string(),
        flow_parents: Vec::new(),
        message: None,
    };
    let _ = commit_workflow(repo, &commit_options, progress)?;

    if commits_behind_main(repo)? > 0 {
        progress.status("Branch behind base, rebasing...");
        let main_repo = main_repo_root(repo).unwrap_or_else(|_| repo.to_path_buf());
        let base_branch = get_default_branch(&main_repo)?;
        crate::rebase::rebase_with_recovery(
            repo,
            &crate::rebase::RebaseOptions {
                onto: format!("origin/{base_branch}"),
                push: true,
            },
            progress,
        )?;
    }

    if let Some(pr) = find_open_pr(repo)? {
        if !options.refresh && !has_unpushed_commits(repo)? && !pr.is_draft {
            open_url(&pr.url);
            return Ok(PrResult {
                url: pr.url,
                created: false,
                updated: false,
            });
        }

        progress.status("Updating PR...");
        let message = if let Some(diff) = get_pr_diff(repo)? {
            generate_pr_message_from_diff(repo, &diff)?
        } else {
            generate_pr_message(repo)?
        };
        update_pr(repo, pr.number, &message.title, &message.body)?;
        if pr.is_draft {
            mark_pr_ready(repo, pr.number)?;
        }
        open_url(&pr.url);
        return Ok(PrResult {
            url: pr.url,
            created: false,
            updated: true,
        });
    }

    progress.status("Creating PR...");
    let message = generate_pr_message(repo)?;
    let base = pr_target(repo)?;
    let url = create_pr(repo, &message.title, &message.body, &base)?;
    if let Some(pr) = find_open_pr(repo)? {
        if pr.is_draft {
            mark_pr_ready(repo, pr.number)?;
        }
    }
    open_url(&url);
    Ok(PrResult {
        url,
        created: true,
        updated: false,
    })
}

pub fn gh_available() -> bool {
    command_exists("gh")
}

fn get_pr_diff(repo: &Path) -> OpsResult<Option<String>> {
    let output = Command::new("gh")
        .arg("pr")
        .arg("diff")
        .current_dir(repo)
        .output()?;
    if !output.status.success() {
        return Ok(None);
    }
    let diff = String::from_utf8_lossy(&output.stdout).to_string();
    if diff.trim().is_empty() {
        Ok(None)
    } else {
        Ok(Some(diff))
    }
}

pub fn pr_exists_for_current_branch(repo: &Path) -> OpsResult<bool> {
    Ok(find_open_pr(repo)?.is_some())
}

fn find_open_pr(repo: &Path) -> OpsResult<Option<GhPr>> {
    let branch =
        current_branch(repo)?.ok_or_else(|| OpsError::Message("not on a branch".to_string()))?;
    let output = Command::new("gh")
        .arg("pr")
        .arg("list")
        .arg("--head")
        .arg(&branch)
        .arg("--json")
        .arg("url,state,isDraft,number")
        .current_dir(repo)
        .output()?;

    if !output.status.success() {
        return Ok(None);
    }

    let stdout = String::from_utf8_lossy(&output.stdout).to_string();
    let mut list: Vec<GhPr> = serde_json::from_str(&stdout).unwrap_or_default();
    list.retain(|pr| pr.state.to_uppercase() == "OPEN");
    Ok(list.into_iter().next())
}

fn update_pr(repo: &Path, number: u64, title: &str, body: &str) -> OpsResult<()> {
    let output = Command::new("gh")
        .arg("pr")
        .arg("edit")
        .arg(number.to_string())
        .arg("--title")
        .arg(title)
        .arg("--body")
        .arg(body)
        .current_dir(repo)
        .output()?;
    if !output.status.success() {
        return Err(OpsError::CommandFailed {
            command: "gh pr edit".to_string(),
            stderr: stderr_from_output(&output),
        });
    }
    Ok(())
}

fn mark_pr_ready(repo: &Path, number: u64) -> OpsResult<()> {
    let output = Command::new("gh")
        .arg("pr")
        .arg("ready")
        .arg(number.to_string())
        .current_dir(repo)
        .output()?;
    if !output.status.success() {
        return Err(OpsError::CommandFailed {
            command: "gh pr ready".to_string(),
            stderr: stderr_from_output(&output),
        });
    }
    Ok(())
}

fn create_pr(repo: &Path, title: &str, body: &str, base: &str) -> OpsResult<String> {
    let mut cmd = Command::new("gh");
    cmd.arg("pr")
        .arg("create")
        .arg("--title")
        .arg(title)
        .arg("--body")
        .arg(body)
        .arg("--base")
        .arg(base);
    let output = cmd.current_dir(repo).output()?;
    if !output.status.success() {
        return Err(OpsError::CommandFailed {
            command: "gh pr create".to_string(),
            stderr: stderr_from_output(&output),
        });
    }
    Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
}

fn sync_main_repo(repo: &Path, _progress: &impl Progress) -> OpsResult<()> {
    let main_repo = main_repo_root(repo).unwrap_or_else(|_| repo.to_path_buf());
    let base_branch = get_default_branch(&main_repo)?;
    if !sync_main(&main_repo, &base_branch)? {
        return Err(OpsError::Message(
            "working tree dirty; sync aborted".to_string(),
        ));
    }
    Ok(())
}

fn commits_behind_main(repo: &Path) -> OpsResult<i32> {
    let main_repo = main_repo_root(repo).unwrap_or_else(|_| repo.to_path_buf());
    let base_branch = get_default_branch(&main_repo)?;
    let output = Command::new("git")
        .arg("rev-list")
        .arg("--count")
        .arg(format!("HEAD..origin/{}", base_branch))
        .current_dir(repo)
        .output()?;
    if !output.status.success() {
        return Ok(0);
    }
    let count = String::from_utf8_lossy(&output.stdout)
        .trim()
        .parse::<i32>()
        .unwrap_or(0);
    Ok(count)
}

fn has_unpushed_commits(repo: &Path) -> OpsResult<bool> {
    let output = Command::new("git")
        .arg("rev-list")
        .arg("--count")
        .arg("@{u}..HEAD")
        .current_dir(repo)
        .output()?;
    if !output.status.success() {
        return Ok(true);
    }
    let count = String::from_utf8_lossy(&output.stdout)
        .trim()
        .parse::<i32>()
        .unwrap_or(0);
    Ok(count > 0)
}

fn pr_target(repo: &Path) -> OpsResult<String> {
    let main_repo = main_repo_root(repo).unwrap_or_else(|_| repo.to_path_buf());
    let current_branch =
        current_branch(repo)?.ok_or_else(|| OpsError::Message("not on a branch".to_string()))?;

    if let Ok(worktrees) = list_worktrees(&main_repo) {
        if let Some(state) = worktrees
            .into_iter()
            .find(|wt| wt.branch.as_deref() == Some(&current_branch))
        {
            if let Some(base_branch) = state.base_branch {
                if base_branch != current_branch {
                    return resolve_pr_target(repo, &base_branch);
                }
            }
        }
    }

    let default = get_default_branch(&main_repo)?;
    Ok(default)
}

fn resolve_pr_target(repo: &Path, base_branch: &str) -> OpsResult<String> {
    if base_branch == "main" {
        return Ok("main".to_string());
    }

    let output = Command::new("gh")
        .arg("pr")
        .arg("view")
        .arg(base_branch)
        .arg("--json")
        .arg("state")
        .arg("-q")
        .arg(".state")
        .current_dir(repo)
        .output()?;
    if output.status.success() {
        let state = String::from_utf8_lossy(&output.stdout)
            .trim()
            .to_uppercase();
        if state == "MERGED" {
            return Ok("main".to_string());
        }
    }
    Ok(base_branch.to_string())
}

fn open_url(url: &str) {
    loopflow_engine::platform::open_url(url);
}
