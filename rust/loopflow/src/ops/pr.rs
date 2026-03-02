use std::path::{Path, PathBuf};
use std::process::Command;

use serde::Deserialize;

use crate::engine::git::{current_branch, get_default_branch, rev_parse, sync_main};
use crate::engine::worktrees::{list_worktrees, main_repo_root};

use crate::ops::commit::{commit_workflow, CommitOptions};
use crate::ops::error::{OpsError, OpsResult};
use crate::ops::progress::Progress;
use crate::ops::util::{command_exists, stderr_from_output};

#[derive(Debug, Clone)]
pub struct PrOptions {
    pub refresh: bool,
    pub title: Option<String>,
    pub body: Option<String>,
}

#[derive(Debug, Clone)]
pub struct PrResult {
    pub url: String,
    pub created: bool,
    pub updated: bool,
}

#[derive(Debug, Clone)]
pub struct PrInfo {
    pub url: String,
    pub number: u64,
    pub state: String,
    pub branch: String,
}

#[derive(Debug, Deserialize)]
struct GhPr {
    url: String,
    state: String,
    #[serde(default, rename = "isDraft")]
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

    // Snapshot origin's view of our branch before any work
    let branch =
        current_branch(repo)?.ok_or_else(|| OpsError::Message("not on a branch".to_string()))?;
    let origin_before = rev_parse(repo, &format!("origin/{branch}")).ok();

    let main_repo = resolve_main_repo(repo);
    let base_branch = get_default_branch(&main_repo)?;

    // Commit before sync — sync_main checks is_clean() and fails on dirty trees
    let commit_options = CommitOptions {
        add: true,
        push: true,
        message: Some("lf ops pr: prepare branch".to_string()),
        ..CommitOptions::for_task("commit")
    };
    commit_workflow(repo, &commit_options, progress)?;

    let _ = sync_main(&main_repo, &base_branch);

    if commits_behind(repo, &base_branch)? > 0 {
        progress.status("Branch behind base, rebasing...");
        crate::ops::rebase::rebase_with_recovery(
            repo,
            &crate::ops::rebase::RebaseOptions {
                onto: format!("origin/{base_branch}"),
                push: true,
            },
            progress,
        )?;
    }

    // Refresh PR if HEAD moved from where origin was before we started
    let head_now = rev_parse(repo, "HEAD").ok();
    let has_changes = origin_before.is_none() || origin_before != head_now;
    let title = options
        .title
        .as_deref()
        .map(str::trim)
        .filter(|v| !v.is_empty());
    let body = options
        .body
        .as_deref()
        .map(str::trim)
        .filter(|v| !v.is_empty());

    match (title, body) {
        (Some(title), Some(body)) => {
            if let Some(pr) = find_open_pr(repo)? {
                progress.status("Updating PR...");
                update_pr(repo, pr.number, title, body)?;
                if pr.is_draft {
                    mark_pr_ready(repo, pr.number)?;
                }
                open_url(&pr.url);
                Ok(PrResult {
                    url: pr.url,
                    created: false,
                    updated: true,
                })
            } else {
                progress.status("Creating PR...");
                let base = pr_target(repo, &main_repo, &base_branch)?;
                let url = create_pr(repo, title, body, &base)?;
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
        }
        (None, None) if options.refresh => match find_open_pr(repo)? {
            Some(pr) => {
                open_url(&pr.url);
                Ok(PrResult {
                    url: pr.url,
                    created: false,
                    updated: has_changes,
                })
            }
            None => Err(OpsError::Message("no open PR to refresh".to_string())),
        },
        _ => Err(OpsError::Message(
            "title and body required — use `lf pr` to generate them.".to_string(),
        )),
    }
}

pub fn gh_available() -> bool {
    command_exists("gh")
}

pub fn pr_exists_for_current_branch(repo: &Path) -> OpsResult<bool> {
    Ok(find_open_pr(repo)?.is_some())
}

pub fn current_pr(repo: &Path) -> OpsResult<Option<PrInfo>> {
    if !gh_available() {
        return Ok(None);
    }

    let branch =
        current_branch(repo)?.ok_or_else(|| OpsError::Message("not on a branch".to_string()))?;

    if let Some(pr) = find_open_pr(repo)? {
        let state = if pr.is_draft { "draft" } else { "open" }.to_string();
        return Ok(Some(PrInfo {
            url: pr.url,
            number: pr.number,
            state,
            branch,
        }));
    }

    Ok(None)
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
        return Err(OpsError::CommandFailed {
            command: format!("gh pr list --head {branch}"),
            stderr: stderr_from_output(&output),
        });
    }

    let stdout = String::from_utf8_lossy(&output.stdout).to_string();
    let list: Vec<GhPr> = serde_json::from_str(&stdout)
        .map_err(|e| OpsError::Message(format!("failed to parse gh pr list output: {e}")))?;
    let open = list
        .into_iter()
        .find(|pr| pr.state.to_uppercase() == "OPEN");
    Ok(open)
}

pub fn update_pr(repo: &Path, number: u64, title: &str, body: &str) -> OpsResult<()> {
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

fn resolve_main_repo(repo: &Path) -> PathBuf {
    main_repo_root(repo).unwrap_or_else(|_| repo.to_path_buf())
}

fn commits_behind(repo: &Path, base_branch: &str) -> OpsResult<i32> {
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

fn pr_target(repo: &Path, main_repo: &Path, default_branch: &str) -> OpsResult<String> {
    let current_branch =
        current_branch(repo)?.ok_or_else(|| OpsError::Message("not on a branch".to_string()))?;

    if let Ok(worktrees) = list_worktrees(main_repo) {
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

    Ok(default_branch.to_string())
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
    crate::engine::platform::open_url(url);
}
