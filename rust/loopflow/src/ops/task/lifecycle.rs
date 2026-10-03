//! Task lifecycle composes the provider, PR, worker and checkout owners.
use std::collections::HashSet;
use std::path::Path;
use std::sync::Arc;

use crate::durable::{FlowSession, WorkRef, WorkStatus};
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
    let blockers = associated_work_blockers(store, task)?;
    if !blockers.is_empty() {
        eprintln!(
            "Task {} is complete; retained checkout: {}",
            task.plan.identifier,
            blockers.join("; ")
        );
        return Ok(());
    }
    if let Some(position) = store.task_flow(&task.id).await.map_err(task_error)? {
        if !position.finished {
            eprintln!(
                "Task {} is complete; retained checkout for unfinished Flow {}.",
                task.plan.identifier,
                position.id(),
            );
            return Ok(());
        }
    }
    let result = async {
        let wave = owning_wave(store, task).await?;
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
    let wave = owning_wave(&store, &task).await?;
    if main_repo_root(repo)? != main_repo_root(Path::new(wave.repo()))? {
        return Err(task_error("branch belongs to a Task in another repository"));
    }
    Ok(Some((store, task)))
}

fn execution_unsettled(store: &SharedStore, flow: &FlowSession) -> OpsResult<bool> {
    let step = store
        .sqlite
        .pending_flow_step_exec(flow.id())
        .map_err(task_error)?;
    let provider_pending = store
        .sqlite
        .pending_flow_conversation(flow.id())
        .map_err(task_error)?
        .is_some();
    Ok(provider_pending
        || step.as_ref().is_some_and(|exec| {
            crate::journal::exec_process_evidence(&store.sqlite, exec)
                != crate::journal::ProcessIdentityEvidence::Dead
        })
        || flow.claim.as_ref().is_some_and(|claim| {
            crate::journal::task_worker_owner_evidence(&claim.owner)
                != crate::journal::ProcessIdentityEvidence::Dead
        }))
}

async fn require_idle(store: &SharedStore, task: &Task, settle_dead: bool) -> OpsResult<()> {
    let blockers = associated_work_blockers(store, task)?;
    if !blockers.is_empty() {
        return Err(task_error(blockers.join("; ")));
    }
    if let Some(position) = store.task_flow(&task.id).await.map_err(task_error)? {
        if execution_unsettled(store, &position)? {
            return Err(task_error(format!("{} has live or unresolved execution; interrupt it and wait for exit before abandoning", task.plan.identifier)));
        }
        if let Some(claim) = &position.claim {
            if !settle_dead {
                return Err(task_error("worker claim requires explicit settlement"));
            }
            store
                .release_flow(position.id(), position.version, Some(claim))
                .await
                .map_err(task_error)?;
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

/// Completion cannot implicitly settle another Flow's work.
pub(super) fn associated_work_blockers(store: &SharedStore, task: &Task) -> OpsResult<Vec<String>> {
    let work = store.sqlite.task_work(&task.id).map_err(task_error)?;
    let mut blockers = execution_blockers(store, &work, ExecutionCheck::RetainWork)?;
    for flow in work.flows.iter().filter(|flow| !flow.managed) {
        if flow.summary.state == crate::session::FlowSummaryState::Current {
            blockers.push(format!(
                "Flow {} is unfinished; inspect `lf flow show {} --sessions`",
                flow.summary.id, flow.summary.id
            ));
        }
    }
    Ok(blockers)
}

/// Restoring a checkout preserves idle Flows; only unresolved execution waits.
pub(super) fn associated_execution_blockers(
    store: &SharedStore,
    task: &Task,
) -> OpsResult<Vec<String>> {
    execution_blockers(
        store,
        &store.sqlite.task_work(&task.id).map_err(task_error)?,
        ExecutionCheck::RetainWork,
    )
}

/// Resumption acquires exact Flow authority; unrelated history is not a claim.
pub(super) fn recovery_execution_blockers(
    store: &SharedStore,
    task: &Task,
) -> OpsResult<Vec<String>> {
    execution_blockers(
        store,
        &store.sqlite.task_work(&task.id).map_err(task_error)?,
        ExecutionCheck::ResumeFlow,
    )
}

enum ExecutionCheck {
    RetainWork,
    ResumeFlow,
}

fn execution_blockers(
    store: &SharedStore,
    work: &crate::task_work::TaskWork,
    check: ExecutionCheck,
) -> OpsResult<Vec<String>> {
    let mut blockers = Vec::new();
    let mut managed_execs = HashSet::new();
    let mut session_execs = HashSet::new();
    // A missing historical receipt differs from an observed process whose
    // liveness query failed. Only the former can be unrelated history.
    let observed_execs: HashSet<String> = match check {
        ExecutionCheck::RetainWork => HashSet::new(),
        ExecutionCheck::ResumeFlow => {
            crate::journal::read_exec_process_receipts_at(&crate::store::lf_home_dir())
                .map_err(task_error)?
                .into_iter()
                .map(|receipt| receipt.exec_id)
                .collect()
        }
    };
    for flow in work
        .flows
        .iter()
        .filter(|flow| flow.summary.state == crate::session::FlowSummaryState::Current)
    {
        if let Some(position) = store.sqlite.flow(&flow.summary.id).map_err(task_error)? {
            if flow.managed {
                managed_execs.extend(
                    store
                        .sqlite
                        .flow_exec_ids(&flow.summary.id)
                        .map_err(task_error)?,
                );
                if let Some(claim) = position.claim {
                    managed_execs.insert(claim.owner.exec_id);
                }
            } else if execution_unsettled(store, &position)? {
                blockers.push(format!(
                    "Flow {} has live or unresolved execution",
                    flow.summary.id
                ));
            }
        }
    }
    for session in work.sessions.iter().filter(|session| !session.managed) {
        if matches!(check, ExecutionCheck::ResumeFlow) && session.completed_at.is_none() {
            if let Some(driver) = store
                .sqlite
                .session_driver(&session.id)
                .map_err(task_error)?
            {
                session_execs.extend(driver.exec_id);
                session_execs.insert(driver.provider_exec_id);
            }
        }
        if let Some(input) = store.sqlite.session(&session.id).map_err(task_error)? {
            if !input.interactive && !input.input_published {
                blockers.push(format!("Session {} has a reserved input", session.id));
            }
        }
        if session.completed_at.is_none()
            && session.kind != crate::session::SessionKind::Conversation
        {
            blockers.push(format!("Session {} awaits completion", session.id));
        }
        if store
            .sqlite
            .session_has_pending_turn(&session.id)
            .map_err(task_error)?
        {
            blockers.push(format!(
                "Session {} has an unresolved provider turn",
                session.id
            ));
        }
    }
    let caller = crate::journal::current_exec_id();
    for exec in work.execs.iter().filter(|exec| exec.completed_at.is_none()) {
        if caller.as_ref() == Some(&exec.id) || managed_execs.contains(&exec.id) {
            continue;
        }
        let blocks = match crate::journal::exec_process_evidence(&store.sqlite, &exec.id) {
            crate::journal::ProcessIdentityEvidence::Dead => false,
            crate::journal::ProcessIdentityEvidence::Live => true,
            crate::journal::ProcessIdentityEvidence::Unknown => {
                matches!(check, ExecutionCheck::RetainWork)
                    || observed_execs.contains(exec.id.as_str())
                    || session_execs.contains(&exec.id)
                    || exec.caller_session_id.as_ref().is_some_and(|id| {
                        work.sessions
                            .iter()
                            .any(|session| &session.id == id && session.completed_at.is_none())
                    })
            }
        };
        if blocks {
            blockers.push(format!(
                "Exec {} has live or unresolved execution; inspect `lf mon show {}`",
                exec.id, exec.id
            ));
        }
    }
    Ok(blockers)
}
