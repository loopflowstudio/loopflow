//! Task lifecycle composes the provider, PR and checkout owners.
use std::collections::HashSet;
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

use super::{block_on_task, owning_wave, task_error, task_store};

/// Completion is durable before cleanup; failure never reverses the outcome.
pub(crate) async fn cleanup_completed_task(store: &SharedStore, task: &Task) -> OpsResult<()> {
    if super::task_work_status(store, task).await? != WorkStatus::Done {
        return Ok(());
    }
    let blockers = associated_execution_blockers(store, task)?;
    if !blockers.is_empty() {
        eprintln!(
            "Task {} is complete; retained checkout: {}",
            task.plan.identifier,
            blockers.join("; ")
        );
        return Ok(());
    }
    let result = async {
        let wave = owning_wave(store, task).await?;
        let repo = main_repo_root(Path::new(wave.repo()))?;
        let _mutation = if let Some(worktree) = task.worktree.as_ref().filter(|path| path.exists())
        {
            if std::fs::canonicalize(worktree)? == std::fs::canonicalize(&repo)? {
                eprintln!(
                    "Task {} is complete; retained the primary checkout and branch.",
                    task.plan.identifier
                );
                return Ok(());
            }
            Some(super::lock_task_pr_mutation(worktree)?)
        } else {
            None
        };
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
        if task.worktree.as_ref().is_some_and(|path| path.exists()) {
            return Err(task_error(
                "checkout is on a different branch; retained it for explicit wt delete",
            ));
        }
        Ok(())
    }
    .await;
    result.map_err(|error| {
        task_error(format!(
            "Task {} is complete, but cleanup is incomplete: {error}. Retry `lf task move {} end`.",
            task.plan.identifier, task.plan.identifier,
        ))
    })
}

async fn branch_task(repo: &Path, branch: &str) -> OpsResult<Option<(SharedStore, Task)>> {
    let store = match open_registry_for_authority().await {
        Ok(store) => Arc::new(store),
        Err(RegistryUnavailable::MissingFile { .. })
            if crate::journal::agent_caller().is_none() =>
        {
            return Ok(None)
        }
        Err(error) => return Err(super::task_registry_error(error)),
    };
    let Some(task) = historical_branch_task(&store, branch).await? else {
        return Ok(None);
    };
    // Branch names are only unique inside their repository.
    let wave = owning_wave(&store, &task).await?;
    if main_repo_root(repo)? != main_repo_root(Path::new(wave.repo()))? {
        return Err(task_error("branch belongs to a Task in another repository"));
    }
    Ok(Some((store, task)))
}

fn require_idle(store: &SharedStore, task: &Task) -> OpsResult<()> {
    let blockers = associated_execution_blockers(store, task)?;
    if !blockers.is_empty() {
        return Err(task_error(blockers.join("; ")));
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
            require_idle(&store, &task)?;
            progress.status(&format!("Task {} and its Linear outcome remain unchanged; use `lf task abandon {}` to cancel the Task.", task.plan.identifier, task.plan.identifier));
        }
        Ok(())
    })
}

pub(crate) async fn record_abandoned_pr(repo: &Path, branch: &str) -> OpsResult<()> {
    let Some((store, task)) = branch_task(repo, branch).await? else {
        return Ok(());
    };
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
    if let Some(task) = &task {
        if store
            .sqlite
            .project_planning_authority(&task.project_id)
            .map_err(task_error)?
            == crate::planning::PlanningAuthority::Local
        {
            if super::task_work_status(&store, task).await? != WorkStatus::Abandoned {
                store
                    .abandon(&WorkRef::Task(task.id.clone()), "explicit Task abandonment")
                    .await
                    .map_err(task_error)?;
            }
            let cleanup = async {
                for deletion in
                    prepare_abandon(repo, &store, Some(task), selector, force, false).await?
                {
                    crate::ops::abandon::abandon_prepared(deletion, &NullProgress).await?;
                }
                Ok::<(), crate::ops::OpsError>(())
            }
            .await;
            if let Err(error) = cleanup {
                eprintln!(
                    "{} is canceled; retained checkout/PR: {error}",
                    task.plan.identifier
                );
            }
            return Ok(task.plan.identifier.clone());
        }
    }
    let issue = task
        .as_ref()
        .and_then(|task| task.plan.linear_id.as_ref())
        .map_or(selector, |id| id.as_str());
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
    if let Some(task) = &task {
        if super::task_work_status(&store, task).await? == WorkStatus::Done {
            return Err(task_error("completed Tasks cannot be abandoned"));
        }
    }
    let outcome = apply_abandon(repo, &store, task.as_ref(), &resolved, Vec::new()).await?;
    // The decision is durable before cleanup. A retained checkout or PR never
    // turns cancellation into a failed decision or authorizes process control.
    let cleanup = async {
        for deletion in
            prepare_abandon(repo, &store, task.as_ref(), &resolved.item.id, force, false).await?
        {
            crate::ops::abandon::abandon_prepared(deletion, &NullProgress).await?;
        }
        Ok::<(), crate::ops::OpsError>(())
    }
    .await;
    if let Err(error) = cleanup {
        eprintln!(
            "{} is canceled; retained checkout/PR: {error}",
            resolved.item.identifier
        );
    }
    Ok(outcome)
}

/// Trash the issue after its placed work has been canceled or completed.
pub fn task_delete(repo: &Path, issue: &str) -> OpsResult<String> {
    let repo = task_repository(repo, Some(issue))?;
    block_on_task(async {
        let store = task_store().await?;
        if let Some(task) = store.get_task_by_issue(issue).await.map_err(task_error)? {
            if store
                .sqlite
                .project_planning_authority(&task.project_id)
                .map_err(task_error)?
                == crate::planning::PlanningAuthority::Local
            {
                if !store.sqlite.task_deleted(&task).map_err(task_error)? {
                    if super::task_work_status(&store, &task).await? != WorkStatus::Done {
                        abandon(&repo, issue, false).await?;
                    }
                    store
                        .sqlite
                        .delete_local_task(&task.id)
                        .map_err(task_error)?;
                }
                return Ok(task.plan.identifier);
            }
            let deleted = store
                .task_deletion(&task.wave_id, task.plan.linear_id()?.as_str())
                .await
                .map_err(task_error)?
                .is_some();
            if !deleted && task.worktree.is_some() {
                if super::task_work_status(&store, &task).await? == WorkStatus::Done {
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

// Cleanup preparation is separate from explicit cancellation. A sweep uses
// it before choosing work, so uncertain or live work is never swept.
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
    let local = task
        .map(|task| store.sqlite.project_planning_authority(&task.project_id))
        .transpose()
        .map_err(task_error)?
        == Some(crate::planning::PlanningAuthority::Local);
    if !local {
        require_known_prs(repo, &prs, issue).await?;
    }
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
        require_idle(store, task)?;
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

#[derive(Debug, serde::Serialize, serde::Deserialize)]
pub struct SweepEntry {
    pub wave: String,
    pub project: String,
    pub issue: Option<String>,
    pub outcome: String,
}

pub fn task_sweep(repo: &Path, apply: bool) -> OpsResult<Vec<SweepEntry>> {
    block_on_task(async {
        let store = task_store().await?;
        let sweep = crate::ops::pm::chapter_sweep_candidates(repo).await?;
        let mut entries: Vec<_> = sweep
            .skipped_projects
            .into_iter()
            .map(|(wave, project)| SweepEntry {
                wave,
                project: project.name,
                issue: None,
                outcome: format!(
                    "skipped: foreign-Team Project {} belongs to Teams [{}]",
                    project.id,
                    project.team_ids.join(", ")
                ),
            })
            .collect();
        for (wave, project, item) in sweep.candidates {
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
                issue: Some(item.identifier),
                outcome,
            });
        }
        Ok(entries)
    })
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
            let wave = owning_wave(&store, &task).await?;
            Ok(Some(std::path::PathBuf::from(wave.repo())))
        })?;
        if let Some(repo) = retained {
            return main_repo_root(&repo).map_err(Into::into);
        }
    }
    crate::repo::discover_repo_root(directory)
        .map_err(task_error)?
        .ok_or_else(|| task_error("unplaced Task needs a repository; run from its repository"))
}

/// Checkout restoration and cleanup wait for live or unresolved execution.
pub(super) fn associated_execution_blockers(
    store: &SharedStore,
    task: &Task,
) -> OpsResult<Vec<String>> {
    let open = store.sqlite.open_processes().map_err(task_error)?;
    let work = store
        .sqlite
        .task_open_work(&task.id, &open)
        .map_err(task_error)?;
    let mut blockers = Vec::new();
    // The caller cannot outlive the processes that launched it, nor wait on
    // the Flow whose step it is. Lineage exempts waiting, not authority.
    let mut lineage = HashSet::new();
    let mut next = crate::journal::current_process_lfid();
    while let Some(id) = next.filter(|id| lineage.insert(id.clone())) {
        next = store
            .sqlite
            .process(&id)
            .map_err(task_error)?
            .and_then(|process| process.parent_process_lfid);
    }
    // A Flow is its driver and step Processes; the loop over Processes below judges
    // them. Sessions are judged here on their own evidence.
    for session in &work.sessions {
        if let Some(input) = store.sqlite.session(&session.id).map_err(task_error)? {
            if input.completed_at.is_none() && !input.interactive && !input.input_published {
                blockers.push(format!("Session {} has a reserved input", session.id));
            }
        }
        if store
            .sqlite
            .session_has_pending_turn(&session.id)
            .map_err(task_error)?
            && (session.completed_at.is_none()
                || crate::ops::task_automation::session_engine_unresolved(
                    &store.sqlite,
                    &session.id,
                )?)
        {
            blockers.push(format!(
                "Session {} has an unresolved provider turn",
                session.id
            ));
        }
    }
    for process in work
        .processes
        .iter()
        .filter(|process| process.completed_at.is_none())
    {
        if lineage.contains(&process.lfid) {
            continue;
        }
        if crate::journal::process_evidence(&store.sqlite, &process.lfid)
            != crate::journal::ProcessIdentityEvidence::Dead
        {
            blockers.push(format!(
                "Process {} has live or unresolved execution; inspect `lf monitor show {}`",
                process.lfid, process.lfid
            ));
        }
    }
    Ok(blockers)
}
