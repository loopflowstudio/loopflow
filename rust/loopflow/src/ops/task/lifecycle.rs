//! Cancellation composes the existing provider, Task, PR and checkout owners.
use std::path::Path;
use std::sync::Arc;

use crate::durable::{WorkRef, WorkStatus};
use crate::engine::git::current_branch;
use crate::engine::worktrees::main_repo_root;
use crate::ops::pm::PmResolvedTask;
use crate::ops::wt::BranchDeletion;
use crate::ops::{NullProgress, OpsResult, Progress};
use crate::store::{open_registry_for_authority, RegistryUnavailable, SharedStore};
use crate::work::task::{PrPhase, Task, TaskPr};

use super::{block_on_task, task_error, task_store};

/// Completion is durable before cleanup; failure never reverses the outcome.
pub(crate) async fn cleanup_completed_task(store: &SharedStore, task: &Task) -> OpsResult<()> {
    if super::task_work_status(store, task).await? != WorkStatus::Done {
        return Ok(());
    }
    if let Some(position) = store.flow_position(&task.id).await.map_err(task_error)? {
        if let Some(claim) = position.claim {
            if crate::journal::task_worker_owner_evidence(&claim.owner)
                != crate::journal::ProcessIdentityEvidence::Dead
            {
                eprintln!(
                    "Task {} is complete; checkout cleanup waits for its worker to settle.",
                    task.plan.identifier
                );
                return Ok(());
            }
            if claim.worker_run_id.is_some() {
                store
                    .finish_task_flow(task, &claim, None)
                    .await
                    .map_err(task_error)?;
            } else {
                store
                    .release_task_worker(&task.id, &claim)
                    .await
                    .map_err(task_error)?;
            }
        }
        store.complete_task(task, None).await.map_err(task_error)?;
    }
    let result = async {
        let wave = super::owning_wave(store, task).await?;
        let repo = main_repo_root(Path::new(wave.repo()))?;
        if task.worktree.exists()
            && std::fs::canonicalize(&task.worktree)? == std::fs::canonicalize(&repo)?
        {
            eprintln!(
                "Task {} is complete; retained the primary checkout and branch.",
                task.plan.identifier
            );
            return Ok(());
        }
        let _mutation = task
            .worktree
            .exists()
            .then(|| super::lock_task_pr_mutation(&task.worktree))
            .transpose()?;
        let mut deletions = Vec::new();
        for pr in store.task_prs(&task.id).await.map_err(task_error)? {
            let deletion = match pr.phase() {
                PrPhase::Merged => crate::ops::wt::prepare_landed_delete(
                    &repo,
                    &pr.branch,
                    pr.head_sha()
                        .ok_or_else(|| task_error("merged PR has no recorded head"))?,
                )?,
                PrPhase::Abandoned if pr.publication.is_none() => {
                    crate::ops::wt::prepare_landed_delete(&repo, &pr.branch, &pr.base_commit)?
                }
                PrPhase::Abandoned => crate::ops::wt::prepare_delete(&repo, &pr.branch, false)?,
                _ => {
                    return Err(task_error(
                        "Task still has an unsettled PR; retained checkout",
                    ))
                }
            };
            deletions.push(deletion);
        }
        for deletion in deletions {
            crate::ops::wt::apply_delete(deletion, &NullProgress)?;
        }
        if task.worktree.exists() {
            return Err(task_error(
                "checkout is on a different branch; retained it for explicit wt delete",
            ));
        }
        Ok(())
    }
    .await;
    result.map_err(|error| task_error(format!(
        "Task {} is complete, but cleanup is incomplete: {error}. Retry `lf task complete {} --summary 'Retry cleanup'`.",
        task.plan.identifier, task.plan.identifier,
    )))
}

async fn branch_task(repo: &Path, branch: &str) -> OpsResult<Option<(SharedStore, Task)>> {
    let store = match open_registry_for_authority().await {
        Ok(store) => Arc::new(store),
        Err(RegistryUnavailable::MissingFile { .. })
            if std::env::var_os(crate::durable::RUN_ID_ENV).is_none() =>
        {
            return Ok(None)
        }
        Err(error) => return Err(super::task_registry_error(error)),
    };
    let Some(task) = historical_branch_task(&store, branch).await? else {
        return Ok(None);
    };
    // Branch names are only unique inside their repository.
    let wave = store
        .get_wave(&task.wave_id)
        .await
        .map_err(task_error)?
        .ok_or_else(|| task_error("Task Wave is unavailable"))?;
    if main_repo_root(repo)? != main_repo_root(Path::new(wave.repo()))? {
        return Err(task_error("branch belongs to a Task in another repository"));
    }
    Ok(Some((store, task)))
}

async fn require_idle(store: &SharedStore, task: &Task, settle_dead: bool) -> OpsResult<()> {
    if let Some(position) = store.flow_position(&task.id).await.map_err(task_error)? {
        if let Some(claim) = position.claim {
            if !settle_dead {
                return Err(task_error("worker claim requires explicit settlement"));
            }
            match crate::journal::task_worker_owner_evidence(&claim.owner) {
                crate::journal::ProcessIdentityEvidence::Dead => {
                    store.release_task_worker(&task.id, &claim).await.map_err(task_error)?;
                }
                _ => return Err(task_error(format!("{} has live or unresolved execution; interrupt it and wait for exit before abandoning", task.plan.identifier))),
            }
        }
    }
    Ok(())
}

pub(crate) fn notice_retained_task(
    repo: &Path,
    branch: &str,
    progress: &impl Progress,
) -> OpsResult<()> {
    block_on_task(async {
        if let Some((store, task)) = branch_task(repo, branch).await? {
            require_idle(&store, &task, true).await?;
            progress.status(&format!("Task {} and its Linear outcome remain unchanged; use `lf task abandon {}` to cancel the Task.", task.plan.identifier, task.plan.identifier));
        }
        Ok(())
    })
}

pub(crate) async fn record_abandoned_pr(repo: &Path, branch: &str) -> OpsResult<()> {
    if let Some((store, task)) = branch_task(repo, branch).await? {
        settle_pr(&store, &task, branch).await?;
    }
    Ok(())
}

async fn settle_pr(store: &SharedStore, task: &Task, branch: &str) -> OpsResult<()> {
    for mut pr in store.task_prs(&task.id).await.map_err(task_error)? {
        if pr.branch == branch && pr.is_active() {
            let now = time::OffsetDateTime::now_utc();
            pr.abandoned_at = Some(now);
            pr.updated_at = now;
            store.settle_task_pr(&pr, None).await.map_err(task_error)?;
        }
    }
    Ok(())
}

async fn resolve_task(store: &SharedStore, selector: &str) -> OpsResult<Option<Task>> {
    if let Some(task) = store
        .get_task_by_issue(selector)
        .await
        .map_err(task_error)?
    {
        return Ok(Some(task));
    }
    historical_branch_task(store, selector).await
}

async fn historical_branch_task(store: &SharedStore, branch: &str) -> OpsResult<Option<Task>> {
    let mut found = None;
    for task in store.list_tasks(None).await.map_err(task_error)? {
        if store
            .task_prs(&task.id)
            .await
            .map_err(task_error)?
            .iter()
            .any(|pr| pr.branch == branch)
        {
            if found.is_some() {
                return Err(task_error(format!(
                    "branch {branch:?} belongs to multiple Tasks; use an issue ID"
                )));
            }
            found = Some(task);
        }
    }
    Ok(found)
}

pub fn task_abandon(repo: &Path, selector: Option<&str>, force: bool) -> OpsResult<String> {
    let selector = match selector {
        Some(selector) => selector.to_string(),
        None => {
            current_branch(repo)?.ok_or_else(|| task_error("no Task selector or current branch"))?
        }
    };
    if let Some(destination) = crate::ops::task_destination::destination().map_err(task_error)? {
        crate::ops::task_destination::check_task(&destination, &selector).map_err(task_error)?;
        let mut args = vec!["task".into(), "abandon".into(), selector];
        if force {
            args.push("--force".into());
        }
        return crate::ops::task_destination::json(&destination, repo, args, None)
            .map_err(task_error);
    }
    let repo = task_repository(repo, Some(&selector))?;
    block_on_task(abandon(&repo, &selector, force)).map_err(|error| {
        task_error(format!(
            "{error}. Abandonment is incomplete; retry `lf task abandon {selector}`."
        ))
    })
}

async fn abandon(repo: &Path, selector: &str, force: bool) -> OpsResult<String> {
    let store = task_store().await?;
    let task = resolve_task(&store, selector).await?;
    let issue = task.as_ref().map_or(selector, |task| task.plan.id.as_str());
    // Fresh ownership and outcome before any effects, even for historical Tasks.
    let resolved = crate::ops::pm::pm_resolve_task_async(repo, issue).await?;
    if resolved.item.state.as_deref() == Some("completed")
        || resolved.item.state.as_deref() == Some("duplicate")
    {
        return Err(task_error(format!(
            "{} is already terminal; preserving its outcome",
            resolved.item.identifier
        )));
    }
    let deletions =
        prepare_abandon(repo, &store, task.as_ref(), &resolved.item.id, force, false).await?;
    apply_abandon(repo, &store, task.as_ref(), &resolved, deletions).await
}

/// Trash the issue after its placed work has been canceled or completed.
pub fn task_delete(repo: &Path, issue: &str) -> OpsResult<String> {
    let repo = task_repository(repo, Some(issue))?;
    block_on_task(async {
        let store = task_store().await?;
        if let Some(task) = store.get_task_by_issue(issue).await.map_err(task_error)? {
            let deleted = store
                .task_deletion(&task.wave_id, task.plan.id.as_str())
                .await
                .map_err(task_error)?
                .is_some();
            if !deleted {
                if super::task_work_status(&store, &task).await? == WorkStatus::Done {
                    require_idle(&store, &task, true).await?;
                    cleanup_completed_task(&store, &task).await?;
                } else {
                    abandon(&repo, issue, false).await?;
                }
            }
        }
        crate::ops::pm::delete_task(&repo, issue).await
    })
    .map_err(|error| {
        task_error(format!(
            "{error}. Removal is incomplete; retry `lf task delete {issue}`."
        ))
    })
}

// Preview and apply share every exclusion. Preparation never retires a sweep's
// worker claim; explicit abandonment may settle a worker proven dead.
async fn prepare_abandon(
    repo: &Path,
    store: &SharedStore,
    task: Option<&Task>,
    issue: &str,
    force: bool,
    sweep: bool,
) -> OpsResult<Vec<BranchDeletion>> {
    let prs = match task {
        Some(task) => store.task_prs(&task.id).await.map_err(task_error)?,
        None => Vec::new(),
    };
    require_known_prs(repo, &prs, issue).await?;
    let mut deletions = Vec::new();
    if let Some(task) = task {
        if store
            .work_status(&WorkRef::Task(task.id.clone()))
            .await
            .map_err(task_error)?
            == WorkStatus::Done
        {
            return Err(task_error("completed Tasks cannot be abandoned"));
        }
        require_idle(store, task, !sweep).await?;
        for pr in &prs {
            // Merged history is never recast as abandonment.
            if pr.merge_commit.is_none() {
                let deletion = crate::ops::wt::prepare_delete(repo, &pr.branch, force)?;
                let prs = crate::ops::abandon::branch_prs(repo, &pr.branch)?;
                if sweep && prs.iter().any(|(_, state)| state == "OPEN") {
                    return Err(task_error(format!(
                        "{} has an open PR; excluded from chapter sweep",
                        pr.branch
                    )));
                }
                if prs.iter().any(|(_, state)| state == "MERGED") && pr.is_active() {
                    return Err(task_error(format!(
                        "{} merged outside Loopflow; reconcile its Task before abandoning",
                        pr.branch
                    )));
                }
                deletions.push(deletion);
            }
        }
    }
    Ok(deletions)
}

async fn apply_abandon(
    repo: &Path,
    store: &SharedStore,
    task: Option<&Task>,
    resolved: &PmResolvedTask,
    deletions: Vec<BranchDeletion>,
) -> OpsResult<String> {
    if let Some(task) = task {
        store
            .begin_task_abandon(&task.id)
            .await
            .map_err(task_error)?;
    }
    crate::ops::pm::issue_client(repo)
        .await?
        .cancel_item(&resolved.item.id)
        .await
        .map_err(task_error)?;
    if let Some(task) = task {
        let work = WorkRef::Task(task.id.clone());
        if store.work_status(&work).await.map_err(task_error)? != WorkStatus::Abandoned {
            // This transaction refuses a concurrently claimed worker.
            store
                .abandon(&work, "explicit Task abandonment")
                .await
                .map_err(task_error)?;
        }
        for deletion in deletions {
            crate::ops::abandon::abandon_prepared(deletion, &NullProgress).await?;
        }
    }
    let context = crate::ops::pm::resolve_context(repo, &resolved.wave).await?;
    crate::ops::pm::refresh_pm_snapshot(repo, &resolved.wave, &context).await?;
    Ok(resolved.item.identifier.clone())
}

async fn require_known_prs(repo: &Path, prs: &[TaskPr], issue: &str) -> OpsResult<()> {
    for url in crate::ops::pm::issue_client(repo)
        .await?
        .item_attachment_urls(issue)
        .await
        .map_err(task_error)?
    {
        if !url.contains("/pull/")
            || prs
                .iter()
                .filter_map(TaskPr::github)
                .any(|github| github.url == url)
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
                "linked PR {url} is untracked and unreadable; cancellation was not attempted"
            )));
        }
        #[derive(serde::Deserialize)]
        struct PullRequest {
            state: String,
        }
        let pr: PullRequest = serde_json::from_slice(&output.stdout).map_err(task_error)?;
        if !matches!(pr.state.as_str(), "CLOSED" | "MERGED") {
            return Err(task_error(format!("linked PR {url} is untracked and {}; close it explicitly before canceling this Task", pr.state)));
        }
    }
    Ok(())
}

#[derive(Debug, serde::Serialize, serde::Deserialize)]
pub struct SweepEntry {
    pub wave: String,
    pub project: String,
    pub issue: String,
    pub outcome: String,
}

pub fn task_sweep(repo: &Path, apply: bool) -> OpsResult<Vec<SweepEntry>> {
    if let Some(destination) = crate::ops::task_destination::destination().map_err(task_error)? {
        let mut args = vec!["task".into(), "sweep".into()];
        if apply {
            args.push("--apply".into());
        }
        return crate::ops::task_destination::json(&destination, repo, args, None)
            .map_err(task_error);
    }
    block_on_task(async {
        let store = task_store().await?;
        let candidates = crate::ops::pm::chapter_sweep_candidates(repo).await?;
        let mut entries = Vec::new();
        for (wave, project, item) in candidates {
            let task = resolve_task(&store, &item.id).await?;
            let outcome = match prepare_abandon(repo, &store, task.as_ref(), &item.id, false, true)
                .await
            {
                Err(error) => format!("skipped: {error}"),
                Ok(_) if !apply => "would cancel Task and remove any retained branches".into(),
                Ok(deletions) => {
                    // Read membership after preparation, immediately before effects.
                    match crate::ops::pm::require_outside_current_chapter(repo, &item.id).await {
                        Err(error) => format!("skipped: {error}"),
                        Ok(resolved) => {
                            match apply_abandon(repo, &store, task.as_ref(), &resolved, deletions)
                                .await
                            {
                                Ok(_) => "canceled".into(),
                                Err(error) => format!(
                                    "incomplete: {error}; retry task abandon {}",
                                    item.identifier
                                ),
                            }
                        }
                    }
                }
            };
            entries.push(SweepEntry {
                wave,
                project,
                issue: item.identifier,
                outcome,
            });
        }
        Ok(entries)
    })
}

/// Address the existing PR/sync operation through its owning Task.
pub fn task_operation(
    repo: &Path,
    issue: &str,
    operation: &str,
    args: &[String],
    agent: Option<&str>,
) -> OpsResult<()> {
    if !matches!(operation, "pr" | "sync") {
        return Err(task_error("expected a PR or sync operation"));
    }
    if let Some(destination) = crate::ops::task_destination::destination().map_err(task_error)? {
        crate::ops::task_destination::check_task(&destination, issue).map_err(task_error)?;
        let mut forwarded = vec!["task".into(), operation.into(), issue.into()];
        forwarded.extend_from_slice(args);
        let output = crate::ops::task_destination::execute(&destination, repo, &forwarded, None)
            .map_err(task_error)?;
        print!("{}", String::from_utf8_lossy(&output));
        return Ok(());
    }
    let task = block_on_task(async {
        task_store()
            .await?
            .get_task_by_issue(issue)
            .await
            .map_err(task_error)?
            .ok_or_else(|| {
                task_error(format!(
                    "no placed Task for {issue}; use task checkout first"
                ))
            })
    })?;
    if !task.worktree.exists() {
        return Err(task_error(format!(
            "Task checkout is absent; recover it with `lf task checkout {issue}`"
        )));
    }
    let mut command = std::process::Command::new(std::env::current_exe()?);
    command.current_dir(&task.worktree);
    if let Some(agent) = agent {
        command.args(["--model", agent]);
    }
    command.arg(operation).args(args);
    let status = command.status()?;
    if !status.success() {
        return Err(task_error(format!(
            "Task {issue} {operation} failed ({status})"
        )));
    }
    Ok(())
}

/// Resolve placed Task operations through retained Wave ownership, even after
/// their checkout is gone. Unplaced planning work uses the caller's repository.
pub fn task_repository(directory: &Path, selector: Option<&str>) -> OpsResult<std::path::PathBuf> {
    if let Some(selector) = selector {
        let retained = block_on_task(async {
            let store = task_store().await?;
            let Some(task) = resolve_task(&store, selector).await? else {
                return Ok(None);
            };
            let wave = store
                .get_wave(&task.wave_id)
                .await
                .map_err(task_error)?
                .ok_or_else(|| task_error("Task Wave is unavailable"))?;
            Ok(Some(std::path::PathBuf::from(wave.repo())))
        })?;
        if let Some(repo) = retained {
            return main_repo_root(&repo).map_err(Into::into);
        }
    }
    let root = crate::repo::discover_repo_root(directory)
        .map_err(task_error)?
        .ok_or_else(|| task_error("unplaced Task needs a repository; run from its repository"))?;
    Ok(root)
}
