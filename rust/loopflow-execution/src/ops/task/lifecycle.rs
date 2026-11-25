//! Task decisions commit locally; PR and checkout cleanup retain separate authority.
use std::path::Path;

use crate::git::current_branch;
use crate::git::worktrees::main_repo_root;
use crate::ops::wt::BranchDeletion;
use crate::ops::{NullProgress, OpsResult, Progress};
use crate::store::SharedStore;
use loopflow_model::durable::{WorkRef, WorkStatus};
use loopflow_storage::sqlite::tracker_actions::ActionOwner;
use loopflow_storage::work::task::{Task, TaskPr};

use super::{block_on_task, repository_for_task, task_error, task_store};

async fn branch_task(repo: &Path, branch: &str) -> OpsResult<Option<(SharedStore, Task)>> {
    let store = match super::authority_store().await? {
        Some(store) => store,
        None if crate::journal::agent_caller().is_none() => return Ok(None),
        None => return Err(super::missing_task_registry()),
    };
    let Some(task) = historical_branch_task(&store, branch).await? else {
        return Ok(None);
    };
    // Branch names are only unique inside their repository.
    if main_repo_root(repo)? != main_repo_root(&repository_for_task(&store, &task)?)? {
        return Err(task_error("branch belongs to a Task in another repository"));
    }
    Ok(Some((store, task)))
}

/// Abandon and `wt delete` remove the checkout; running work keeps it in place.
fn require_idle(store: &SharedStore, task: &Task) -> OpsResult<()> {
    let blockers = crate::ops::task_execution::task_execution_blockers(&store.sqlite, &task.id)?;
    if !blockers.is_empty() {
        return Err(task_error(blockers.join("; ")));
    }
    Ok(())
}

pub fn notice_retained_task(repo: &Path, branch: &str, progress: &impl Progress) -> OpsResult<()> {
    block_on_task(async {
        if let Some((store, task)) = branch_task(repo, branch).await? {
            require_idle(&store, &task)?;
            progress.status(&format!("Task {} and its Linear outcome remain unchanged; use `lf task abandon {}` to cancel the Task.", task.identifier, task.identifier));
        }
        Ok(())
    })
}

pub async fn record_abandoned_pr(repo: &Path, branch: &str) -> OpsResult<()> {
    let Some((store, task)) = branch_task(repo, branch).await? else {
        return Ok(());
    };
    if let Some(mut pr) = store.active_task_pr(&task.id).await.map_err(task_error)? {
        if pr.branch == branch && pr.is_active() {
            let now = time::OffsetDateTime::now_utc();
            pr.abandoned_at = Some(now);
            pr.updated_at = now;
            store.settle_task_pr(&pr).await.map_err(task_error)?;
        }
    }
    Ok(())
}

async fn resolve_task(
    store: &SharedStore,
    repo: Option<&str>,
    selector: &str,
) -> OpsResult<Option<Task>> {
    if let Some(id) = crate::store::children::resolve_task_id(&store.sqlite, selector, repo)
        .map_err(task_error)?
    {
        return store.get_task(&id).await.map_err(task_error);
    }
    historical_branch_task(store, selector).await
}

async fn historical_branch_task(store: &SharedStore, branch: &str) -> OpsResult<Option<Task>> {
    store
        .sqlite
        .historical_branch_task(branch)
        .map_err(task_error)
}

pub fn task_abandon(repo: &Path, selector: Option<&str>, force: bool) -> OpsResult<String> {
    let selector = match selector {
        Some(selector) => selector.to_string(),
        None => {
            current_branch(repo)?.ok_or_else(|| task_error("no Task selector or current branch"))?
        }
    };
    let repo = task_repository(repo, Some(&selector))?;
    block_on_task(abandon(&repo, &selector, force)).map_err(|error| {
        task_error(format!(
            "{error}. Abandonment is incomplete; retry `lf task abandon {selector}`."
        ))
    })
}

async fn abandon(repo: &Path, selector: &str, force: bool) -> OpsResult<String> {
    let store = task_store().await?;
    let task = match resolve_task(&store, Some(&repo.to_string_lossy()), selector).await? {
        Some(task) => task,
        None => {
            crate::ops::task::resolve_saved_task(repo, None, selector)
                .await?
                .1
        }
    };
    save_abandon(&store, &task).await?;
    // Failed cleanup cannot undo the decision or grant control over live work.
    let cleanup = cleanup_abandoned(repo, &store, &task, force).await;
    if let Err(error) = cleanup {
        crate::journal::request::diagnostic(format_args!(
            "{} is canceled; retained checkout/PR: {error}",
            task.identifier
        ));
    }
    Ok(task.identifier)
}

pub(super) async fn cleanup_abandoned(
    repo: &Path,
    store: &SharedStore,
    task: &Task,
    force: bool,
) -> OpsResult<bool> {
    if let Some(deletion) = prepare_abandon(repo, store, task, force).await? {
        crate::ops::abandon::abandon_prepared(deletion, &NullProgress).await?;
        return Ok(true);
    }
    Ok(false)
}

async fn save_abandon(store: &SharedStore, task: &Task) -> OpsResult<()> {
    store
        .abandon(&WorkRef::Task(task.id.clone()), "explicit Task abandonment")
        .await
        .map_err(task_error)?;
    super::retirement::close_conversations(&store.sqlite, &task.id)?;
    crate::ops::planning_sync::sync_after_save(
        store,
        &repository_for_task(store, task)?.to_string_lossy(),
        Some(&ActionOwner::Task(task.id.clone())),
    )
    .await;
    Ok(())
}

/// Remove planning immediately, retaining execution and pending tracker
/// delivery. Returns the identifier and whether the tracker still has
/// something to hear about this Task.
pub fn task_delete(repo: &Path, issue: &str) -> OpsResult<(String, bool)> {
    let repo = task_repository(repo, Some(issue))?;
    block_on_task(async {
        let store = task_store().await?;
        let task = match resolve_task(&store, Some(&repo.to_string_lossy()), issue).await? {
            Some(task) => task,
            None => {
                crate::ops::task::resolve_saved_task(&repo, None, issue)
                    .await?
                    .1
            }
        };
        store.sqlite.delete_task(&task.id).map_err(task_error)?;
        crate::ops::planning_sync::sync_after_save(
            &store,
            &repository_for_task(&store, &task)?.to_string_lossy(),
            Some(&ActionOwner::Task(task.id.clone())),
        )
        .await;
        let pending = store
            .sqlite
            .tracker_actions_for(&ActionOwner::Task(task.id.clone()))
            .map_err(task_error)?
            .iter()
            .any(|action| {
                matches!(
                    action.phase,
                    loopflow_storage::sqlite::tracker_actions::ActionPhase::Queued
                        | loopflow_storage::sqlite::tracker_actions::ActionPhase::Sent
                )
            });
        Ok((task.identifier, pending))
    })
}

async fn prepare_abandon(
    repo: &Path,
    store: &SharedStore,
    task: &Task,
    force: bool,
) -> OpsResult<Option<BranchDeletion>> {
    let pr = store.active_task_pr(&task.id).await.map_err(task_error)?;
    if store
        .work_status(&WorkRef::Task(task.id.clone()))
        .await
        .map_err(task_error)?
        == WorkStatus::Done
    {
        return Err(task_error("completed Tasks cannot be abandoned"));
    }
    require_idle(store, task)?;
    if pr.is_some() {
        if let Some(issue) = &task.tracker_id {
            require_known_prs(repo, pr.as_ref(), issue.as_str()).await?;
        }
    }

    // Merged history is never recast as abandonment.
    if let Some(pr) = pr.filter(|pr| pr.merge_commit.is_none()) {
        let deletion = crate::ops::wt::prepare_delete(repo, &pr.branch, force)?;
        let prs = crate::ops::abandon::branch_prs(repo, &pr.branch)?;
        if prs.iter().any(|(_, state)| state == "MERGED") && pr.is_active() {
            return Err(task_error(format!(
                "{} merged outside Loopflow; reconcile its Task before abandoning",
                pr.branch
            )));
        }
        return Ok(Some(deletion));
    }
    Ok(None)
}

async fn require_known_prs(repo: &Path, pr: Option<&TaskPr>, issue: &str) -> OpsResult<()> {
    for attachment in crate::ops::linear::issue_client()
        .await?
        .item_attachments(issue)
        .await
        .map_err(task_error)?
    {
        let url = attachment.url;
        if !url.contains("/pull/")
            || pr
                .and_then(TaskPr::github)
                .is_some_and(|github| github.url == url)
        {
            continue;
        }
        let parsed = reqwest::Url::parse(&url).map_err(task_error)?;
        let parts = parsed
            .path()
            .trim_matches('/')
            .split('/')
            .collect::<Vec<_>>();
        let [owner, repository, "pull", number] = parts.as_slice() else {
            return Err(task_error(
                "linked PR URL does not identify an exact pull request",
            ));
        };
        let number = number.parse::<u64>().map_err(task_error)?.to_string();
        let repository = format!("{owner}/{repository}");
        // Use the configured GitHub service, never an attachment-supplied host.
        let output = std::process::Command::new("gh")
            .args([
                "pr",
                "view",
                &number,
                "--repo",
                &repository,
                "--json",
                "state",
            ])
            .current_dir(repo)
            .output()?;
        if !output.status.success() {
            return Err(task_error(format!(
                "linked PR {url} is untracked and unreadable; retained checkout and PR"
            )));
        }
        #[derive(serde::Deserialize)]
        struct PullRequest {
            state: String,
        }
        let pr: PullRequest = serde_json::from_slice(&output.stdout).map_err(task_error)?;
        if !matches!(pr.state.as_str(), "CLOSED" | "MERGED") {
            return Err(task_error(format!("linked PR {url} is untracked and {}; close it explicitly before removing this Task's checkout", pr.state)));
        }
    }
    Ok(())
}

/// Resolve placed Task operations through retained Wave ownership, even after
/// their checkout is gone. Unplaced planning work uses the caller's repository.
pub fn task_repository(directory: &Path, selector: Option<&str>) -> OpsResult<std::path::PathBuf> {
    let current = crate::repo::discover_repo_root(directory).map_err(task_error)?;
    let scope = current
        .as_ref()
        .map(|repo| crate::repository::discover(repo).map(|repo| repo.to_string()))
        .transpose()
        .map_err(task_error)?;
    if let Some(selector) = selector {
        let retained = block_on_task(async {
            let store = task_store().await?;
            let Some(task) = resolve_task(&store, scope.as_deref(), selector).await? else {
                return Ok(None);
            };
            Ok(Some(repository_for_task(&store, &task)?))
        })?;
        if let Some(repo) = retained {
            return main_repo_root(&repo).map_err(Into::into);
        }
    }
    current.ok_or_else(|| task_error("unplaced Task needs a repository; run from its repository"))
}
