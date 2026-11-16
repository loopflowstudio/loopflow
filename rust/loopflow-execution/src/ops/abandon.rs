use std::path::Path;
use std::process::Command;

use crate::git::current_branch;
use crate::ops::error::{OpsError, OpsResult};
use crate::ops::progress::Progress;

#[derive(Debug, Clone)]
pub struct AbandonOptions {
    pub branch: Option<String>,
    pub force: bool,
}

/// Close one PR and delete its checkout and branches; retain its owning Task.
pub fn abandon_branch(
    repo: &Path,
    options: &AbandonOptions,
    progress: &impl Progress,
) -> OpsResult<()> {
    let branch = options.branch.clone().map(Ok).unwrap_or_else(|| {
        current_branch(repo)?.ok_or_else(|| OpsError::Message("not on a branch".into()))
    })?;
    let deletion = super::wt::prepare_delete(repo, &branch, options.force)?;
    super::task::notice_retained_task(&deletion.repo, &deletion.branch, progress)?;
    if !options.force
        && !progress.confirm(&format!(
            "Close PR and delete checkout and branches for {branch:?}?"
        ))
    {
        return Err(OpsError::Message("aborted".into()));
    }
    super::task::block_on_task(abandon_prepared(deletion, progress))
}

pub(crate) async fn abandon_prepared(
    deletion: super::wt::BranchDeletion,
    progress: &impl Progress,
) -> OpsResult<()> {
    close_pr(&deletion.repo, &deletion.branch)?;
    super::task::record_abandoned_pr(&deletion.repo, &deletion.branch).await?;
    super::wt::apply_delete(deletion, progress)
}

pub(crate) fn branch_prs(repo: &Path, branch: &str) -> OpsResult<Vec<(u64, String)>> {
    let output = Command::new("gh")
        .args([
            "pr",
            "list",
            "--head",
            branch,
            "--state",
            "all",
            "--json",
            "number,state,headRepository",
            "--limit",
            "1000",
        ])
        .current_dir(repo)
        .output()?;
    if !output.status.success() {
        return Err(OpsError::Message(format!(
            "cannot read PRs for {branch}: {}",
            String::from_utf8_lossy(&output.stderr).trim()
        )));
    }
    #[derive(serde::Deserialize)]
    struct PullRequest {
        number: u64,
        state: String,
        #[serde(rename = "headRepository")]
        head_repository: Option<Repository>,
    }
    #[derive(serde::Deserialize)]
    struct Repository {
        #[serde(rename = "nameWithOwner")]
        name_with_owner: String,
    }
    let owner = crate::repository::RepoId::discover(repo)
        .map_err(|error| OpsError::Message(error.to_string()))?;
    let prs: Vec<PullRequest> = serde_json::from_slice(&output.stdout)
        .map_err(|error| OpsError::Message(format!("invalid GitHub PR response: {error}")))?;
    if prs.len() == 1000 {
        return Err(OpsError::Message(
            "PR history is incomplete; deletion was not attempted".into(),
        ));
    }
    if prs
        .iter()
        .any(|pr| pr.state == "OPEN" && pr.head_repository.is_none())
    {
        return Err(OpsError::Message(
            "open PR has unavailable head repository; deletion was not attempted".into(),
        ));
    }
    Ok(prs
        .into_iter()
        .filter(|pr| {
            pr.head_repository
                .as_ref()
                .is_some_and(|head| head.name_with_owner.eq_ignore_ascii_case(owner.as_str()))
        })
        .map(|pr| (pr.number, pr.state))
        .collect())
}

pub(crate) fn close_pr(repo: &Path, branch: &str) -> OpsResult<()> {
    let prs = branch_prs(repo, branch)?;
    if !prs.iter().any(|(_, state)| state == "OPEN")
        && prs.iter().any(|(_, state)| state == "MERGED")
    {
        return Err(OpsError::Message(format!("{branch} has a merged PR; use wt delete to remove its checkout while preserving the merge outcome")));
    }
    for (number, state) in prs {
        match state.as_str() {
            "OPEN" => {
                let output = Command::new("gh")
                    .args(["pr", "close", &number.to_string()])
                    .current_dir(repo)
                    .output()?;
                if !output.status.success() {
                    return Err(OpsError::Message(format!(
                        "failed to close PR #{number}: {}",
                        String::from_utf8_lossy(&output.stderr).trim()
                    )));
                }
            }
            "CLOSED" | "MERGED" => {}
            _ => {
                return Err(OpsError::Message(format!(
                    "unknown PR #{number} state {state:?}"
                )))
            }
        }
    }
    Ok(())
}
