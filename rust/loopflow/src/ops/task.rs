mod directory;
mod lifecycle;
pub(crate) use lifecycle::{cleanup_completed_task, notice_retained_task, record_abandoned_pr};
pub use lifecycle::{task_abandon, task_delete, task_repository, task_sweep};
mod file_save;
pub use directory::{task_files, TaskDirectory, TaskFileEntry, TaskFileKind};
pub use file_save::{task_save, TaskFileRecovery, TaskFileSave};

use std::collections::BTreeMap;
use std::fs::{File, OpenOptions};
use std::io::Read;
use std::path::{Component, Path, PathBuf};
use std::process::Command;
use std::sync::Arc;
use std::time::{Duration, Instant};

use crate::child::ChildRef;
use crate::durable::WorkStatus;
use crate::engine::agent::checkout_execution_boundary;
use crate::engine::config::{load_config_or_default, parse_agent};
use crate::engine::git::{
    checkout, checkout_new_branch_from, cherry_pick_range, current_branch, delete_local_branch,
    fetch, get_default_branch, is_ancestor, is_clean, is_materially_clean, merge_base,
    push_with_upstream, ref_exists, rev_parse, stash_including_untracked, stash_pop,
};
use crate::engine::naming::sanitize_for_branch;
use crate::engine::worktrees::{
    create_from_placement_plan, plan_branch_placement, PlacementPlan, PlacementStrategy,
    WorktreeSegment,
};
use crate::engine::{compile_flow, load_flow, ConcreteStep};
use crate::ops::error::{OpsError, OpsResult};
use crate::ops::task_actions::{derive_task_actions, TaskActionEvidence, TaskActionModel};
use crate::planning::{LinearIssueId, TaskPlan};
use crate::store::{
    open_existing_store, open_registry_for_authority, RegistryUnavailable, SharedStore, Store,
    StoreError,
};
use crate::work::task::{
    AfterMerge, CiCheck, CiObservation, CiState, GithubObservation, GithubObservationResult,
    GithubPr, Observation, PmWritebackOperation, PmWritebackState, PrMergeMode, PrMergeRequest,
    PrPhase, PrPresentation, PrPublication, Task, TaskEventKind, TaskPr, TaskPrId,
};
use crate::work::wave::Wave;
use fs2::FileExt;
use sha2::{Digest, Sha256};
use time::format_description::well_known::Rfc3339;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TaskWaitUntil {
    Open,
    Terminal,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct TaskExecOptions {
    pub wave: Option<String>,
    pub reason: Option<String>,
    pub agent: Option<String>,
    pub name: Option<String>,
    pub flow: Option<String>,
    pub stack_on: Option<String>,
    pub directive: Option<String>,
}

#[derive(Debug, Clone, Default)]
pub struct TaskCheckoutOptions {
    pub name: Option<String>,
    pub stack_on: Option<String>,
    pub directive: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TaskCreateInput {
    pub title: String,
    pub report: String,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct TaskControlResult {
    pub issue_id: String,
    pub task_id: String,
    pub receipt: super::child::WorkControlReceipt,
    pub observation: Observation,
}

#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct TaskSnapshot {
    pub home_id: Option<crate::durable::HomeId>,
    pub issue_id: String,
    pub issue_identifier: String,
    pub task_id: String,
    pub external_project_id: String,
    pub project: String,
    pub pm_snapshot_synced_at: i64,
    pub pm_writeback: crate::work::task::PmWritebackState,
    pub wave: String,
    pub project_id: String,
    pub status: WorkStatus,
    pub execution: crate::ops::task_execution::TaskExecutionSnapshot,
    pub work: crate::task_work::TaskWork,
    pub worktree: String,
    pub workspace_slug: String,
    pub agent: Option<String>,
    pub provider: String,
    pub prs: Vec<TaskPr>,
    pub active_pr: Option<TaskPrId>,
    pub latest_event: Option<crate::work::task::TaskEvent>,
    pub created_at: time::OffsetDateTime,
    pub updated_at: time::OffsetDateTime,
    /// Freshness of the PR state against GitHub as of this read. `Degraded`
    /// means a bounded remote read failed and the PR fields are cached, not
    /// freshly confirmed.
    pub observation: Observation,
    pub actions: TaskActionModel,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct TaskChangedFile {
    pub path: String,
    pub old_path: Option<String>,
    pub committed: bool,
    pub staged: bool,
    pub unstaged: bool,
    pub untracked: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct TaskChangesSnapshot {
    pub recovery_directory: Option<String>,
    pub issue_identifier: String,
    pub task_id: String,
    pub base_commit: String,
    pub head_commit: String,
    pub files: Vec<TaskChangedFile>,
    pub scratch: Vec<String>,
    pub scratch_truncated: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct TaskDiffSnapshot {
    pub issue_identifier: String,
    pub task_id: String,
    pub base_commit: String,
    pub path: Option<String>,
    pub patch: String,
    pub binary: bool,
    pub truncated: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TaskFileState {
    Text,
    Binary,
    UnsupportedEncoding,
    Truncated,
    Missing,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct TaskFileSnapshot {
    pub recoveries: Vec<TaskFileRecovery>,
    pub issue_identifier: String,
    pub task_id: String,
    pub path: String,
    pub content: Option<String>,
    pub state: TaskFileState,
    pub revision: Option<String>,
    pub read_only_reason: Option<String>,
    pub size_bytes: u64,
}

#[derive(Debug, Clone, Copy)]
struct TaskWorkspace<'a> {
    issue_identifier: &'a str,
    task_id: &'a crate::work::task::TaskId,
    worktree: &'a Path,
}

impl<'a> From<&'a crate::store::sqlite::TaskCheckout> for TaskWorkspace<'a> {
    fn from(checkout: &'a crate::store::sqlite::TaskCheckout) -> Self {
        Self {
            issue_identifier: &checkout.issue_identifier,
            task_id: &checkout.task_id,
            worktree: &checkout.worktree,
        }
    }
}

#[derive(Debug, Clone, Copy)]
struct TaskComparison<'a> {
    checkout: TaskWorkspace<'a>,
    base_commit: &'a str,
}

impl<'a> TaskComparison<'a> {
    fn new(task: &'a Task, pr: &'a TaskPr) -> Self {
        Self {
            checkout: TaskWorkspace {
                issue_identifier: &task.plan.identifier,
                task_id: &task.id,
                worktree: &task.worktree,
            },
            base_commit: &pr.base_commit,
        }
    }
}

fn active_pr(task: &Task) -> OpsResult<TaskPr> {
    let task_id = task.id.clone();
    block_on_task(async move {
        task_store()
            .await?
            .active_task_pr(&task_id)
            .await
            .map_err(|error| task_error(format!("failed to read active PR: {error}")))?
            .ok_or_else(|| task_error("Task has no active PR"))
    })
}

// File access reads recorded placement without reconciling PR or execution state.
fn file_store() -> OpsResult<crate::store::sqlite::SqliteStore> {
    #[cfg(test)]
    let test_path = super::pm::PM_TEST_CONTEXT
        .try_with(|context| context.path.clone())
        .ok();
    #[cfg(not(test))]
    let test_path: Option<std::path::PathBuf> = None;
    let path = match test_path {
        Some(path) => path,
        None => {
            let crate::store::StorageConfig::Sqlite { path } =
                crate::store::storage_config_from_env().map_err(|error| {
                    task_error(format!("cannot resolve Task registry: {error}"))
                })?;
            path
        }
    };
    crate::store::sqlite::SqliteStore::open_read_only(&path)
        .map_err(|error| task_error(format!("cannot read Task registry: {error}")))
}

fn file_context(issue: &str) -> OpsResult<crate::store::sqlite::TaskCheckout> {
    let store = file_store()?;
    let mut checkouts = store
        .task_checkouts()
        .map_err(|error| task_error(format!("cannot read Task checkouts: {error}")))?
        .into_iter()
        .filter(|row| {
            row.task_id.as_str() == issue || row.issue_id == issue || row.issue_identifier == issue
        });
    let checkout = checkouts
        .next()
        .ok_or_else(|| task_error(format!("no Task exists for {issue:?}")))?;
    if checkouts.next().is_some() {
        return Err(task_error(format!(
            "multiple stable Tasks resolve to {issue:?}"
        )));
    }
    if let Some(home) = &checkout.home_id {
        if *home != store.local_home().map_err(task_error)?.id {
            return Err(task_error(format!(
                "Task checkout belongs to Home {home}; read its files on that Home"
            )));
        }
    }
    Ok(checkout)
}

fn comparison_context(issue: &str) -> OpsResult<(Task, TaskPr)> {
    let store = file_store()?;
    let task = store
        .task_by_issue(issue)
        .map_err(|error| task_error(format!("failed to read Task: {error}")))?
        .ok_or_else(|| task_error(format!("no Task exists for {issue:?}")))?;
    let pr = store
        .active_task_pr(&task.id)
        .map_err(|error| task_error(format!("failed to read active PR: {error}")))?
        .ok_or_else(|| task_error("Task has no active PR"))?;
    Ok((task, pr))
}

fn task_error(message: impl std::fmt::Display) -> OpsError {
    OpsError::Message(message.to_string())
}

pub(super) fn block_on_task<T>(
    future: impl std::future::Future<Output = OpsResult<T>>,
) -> OpsResult<T> {
    tokio::runtime::Runtime::new()
        .map_err(|error| task_error(format!("failed to build task runtime: {error}")))?
        .block_on(future)
}

async fn task_store() -> OpsResult<SharedStore> {
    #[cfg(test)]
    if let Ok(store) = super::pm::PM_TEST_CONTEXT.try_with(|context| context.store.clone()) {
        return Ok(store);
    }
    open_existing_store().await.map(Arc::new).ok_or_else(|| {
        task_error("no Loopflow registry on this machine; start the owning Wave first")
    })
}

/// Durable placement for a Task PR forked from another Task's active PR.
#[derive(Debug, Clone)]
pub struct StackedSync {
    pub fork_base: String,
    pub child: TaskPr,
    /// The live parent branch, or `None` once the parent has merged.
    pub parent_branch: Option<String>,
}

/// Resolve a Task's current cross-Task stack from durable ids, consulting
/// GitHub because an out-of-band parent merge can precede registry reconcile.
/// A worktree that is not a Task worktree yields `None`; a Task worktree whose
/// registry is missing/inaccessible/incompatible is refused with an actionable
/// authority error, so a stacked sync never silently degrades to generic.
pub fn task_stack(worktree: &Path) -> OpsResult<Option<StackedSync>> {
    block_on_task(async move {
        let ManagedTask::Managed { store, task } = resolve_managed_task(worktree).await? else {
            return Ok(None);
        };
        let Some(active) = store.active_task_pr(&task.id).await.map_err(task_error)? else {
            return Ok(None);
        };
        let Some(parent_id) = active.parent_pr_id.clone() else {
            return Ok(None);
        };
        let parent = store
            .get_task_pr(&parent_id)
            .await
            .map_err(task_error)?
            .ok_or_else(|| task_error(format!("stack parent {parent_id} is missing")))?;
        let mut parent_task = store
            .get_task(&parent.task_id)
            .await
            .map_err(task_error)?
            .ok_or_else(|| task_error("stack parent Task is missing"))?;
        // Reuse the parent's persisted PR number and observation cache. Stack
        // resolution used to enumerate every PR on the branch independently,
        // bypassing both the Task cache and outage-tolerant reconcile.
        reconcile_task_pr(&store, &mut parent_task).await?;
        let parent = store
            .get_task_pr(&parent_id)
            .await
            .map_err(task_error)?
            .ok_or_else(|| task_error(format!("stack parent {parent_id} disappeared")))?;
        let merged = parent.merge_commit.is_some();
        let closed = parent.abandoned_at.is_some();
        if closed && !merged {
            return Err(task_error(format!(
                "stack parent {} closed without merging; re-place the child deliberately",
                parent.branch
            )));
        }
        Ok(Some(StackedSync {
            fork_base: active.base_commit.clone(),
            child: active,
            parent_branch: (!merged).then_some(parent.branch),
        }))
    })
}

/// Return a stacked child only after its parent has merged. Landing before that
/// would silently drop a dependency that is not present on the default branch.
pub fn stack_for_landing(worktree: &Path) -> OpsResult<Option<StackedSync>> {
    let stacked = task_stack(worktree)?;
    if let Some(stacked) = &stacked {
        if let Some(parent) = &stacked.parent_branch {
            return Err(task_error(format!(
                "Task PR is stacked on {parent}, which has not merged; land the parent first"
            )));
        }
    }
    Ok(stacked)
}

/// Persist the exact base reached by a successful deterministic sync. Clear
/// the parent link only when its work is now present on the default branch.
/// Refuses with an actionable authority error if the registry is not usable, so
/// a post-sync base is never silently dropped — the sync already pushed, so
/// the operator must know the durable record did not advance with it.
pub fn record_stack_sync(
    stacked: &StackedSync,
    new_base: &str,
    clear_parent: bool,
) -> OpsResult<()> {
    let pr_id = stacked.child.id.clone();
    let new_base = new_base.to_string();
    block_on_task(async move {
        let store = Arc::new(
            open_registry_for_authority()
                .await
                .map_err(task_registry_error)?,
        );
        // The immutable event log preserves the audit trail — the child's
        // `PrStarted` (parent base) and the parent's `PrMerged` remain — so the
        // sync only repoints the mutable row to the post-merge truth.
        store
            .sync_task_pr(
                &pr_id,
                &new_base,
                clear_parent,
                time::OffsetDateTime::now_utc(),
            )
            .await
            .map_err(task_error)?;
        Ok(())
    })
}

async fn owning_wave(store: &SharedStore, task: &Task) -> OpsResult<Wave> {
    store
        .get_wave(&task.wave_id)
        .await
        .map_err(|error| task_error(format!("failed to read owning Wave: {error}")))?
        .ok_or_else(|| task_error(format!("owning Wave {} is not registered", task.wave_id)))
}

pub(crate) async fn task_work_status(store: &Store, task: &Task) -> OpsResult<WorkStatus> {
    let work = store
        .work_for_child(&ChildRef::Task(task.id.clone()))
        .await
        .map_err(task_error)?;
    store.work_status(&work).await.map_err(task_error)
}

/// Place the Task and fill what a run leaves unsaid: the Project's Flow when
/// none is named, `agent` as the Task's agent, `reason` as a steer. The caller
/// then runs the returned Flow like any `lf --task ISSUE run FLOW`.
pub fn task_place(repo: &Path, issue: &str, options: TaskExecOptions) -> OpsResult<(Task, String)> {
    let TaskExecOptions {
        flow,
        agent,
        reason,
        ..
    } = options.clone();
    // An unknown Flow is refused before any worktree is placed for it.
    if let Some(flow) = flow.as_deref() {
        load_task_flow(repo, flow)?;
    }
    let mut task = prepare_task(repo, issue, options)?;
    block_on_task(async {
        let store = task_store().await?;
        let project = store
            .get_project(&task.project_id)
            .await
            .map_err(task_error)?
            .ok_or_else(|| task_error("Task Project is missing"))?;
        let (flow, _) = load_task_flow(
            &task.worktree,
            flow.as_deref().unwrap_or(&project.plan.flow),
        )?;
        select_task_agent(&store, &mut task, agent.as_deref()).await?;
        if let Some(reason) = reason
            .as_deref()
            .map(str::trim)
            .filter(|reason| !reason.is_empty())
        {
            super::linear_observe::publish_task_steer(&store, &task, reason).await?;
        }
        Ok((task, flow))
    })
}

pub fn task_checkout(repo: &Path, issue: &str, options: TaskCheckoutOptions) -> OpsResult<Task> {
    prepare_task(
        repo,
        issue,
        TaskExecOptions {
            name: options.name,
            stack_on: options.stack_on,
            directive: options.directive,
            ..Default::default()
        },
    )
}

fn prepare_task(repo: &Path, issue: &str, options: TaskExecOptions) -> OpsResult<Task> {
    let TaskExecOptions {
        wave: expected_wave,
        name,
        stack_on,
        directive,
        flow: requested_flow,
        agent: requested_agent,
        reason,
    } = options;
    let directive = directive
        .map(|directive| {
            let directive = directive.trim().to_string();
            if directive.is_empty() {
                Err(task_error("directive cannot be empty"))
            } else {
                Ok(directive)
            }
        })
        .transpose()?;
    let existing = block_on_task(async {
        let store = task_store().await?;
        let mut existing = store
            .get_task_by_issue(issue)
            .await
            .map_err(|error| task_error(format!("failed to read task registry: {error}")))?;
        if let Some(task) = &mut existing {
            if let Some(expected) = &expected_wave {
                let wave = owning_wave(&store, task).await?;
                if wave.slug() != expected {
                    return Err(task_error(format!(
                        "--wave {expected} does not own Task {} (Wave {})",
                        task.plan.identifier,
                        wave.slug()
                    )));
                }
            }
            let status = task_work_status(&store, task).await?;
            match status {
                WorkStatus::Done => {
                    return Err(task_error(format!(
                        "Task {} is completed; start a new Linear task",
                        task.plan.identifier
                    )))
                }
                WorkStatus::Abandoned => {
                    return Err(task_error(format!(
                    "Task {} is abandoned; inspect its retained history with `lf task status {}`",
                    task.plan.identifier, task.plan.identifier
                )))
                }
                WorkStatus::Ready => {}
            }
            if let Some(requested) = name.as_deref() {
                let requested = parse_workspace_slug(requested)?;
                if requested.as_str() != task.workspace_slug {
                    return Err(task_error(format!(
                        "Task {} already uses workspace name {:?}",
                        task.plan.identifier, task.workspace_slug
                    )));
                }
            }
            if directive.is_some() {
                return Err(task_error(format!(
                    "Task {} already exists; use `lf task comment {} <new-direction>`",
                    task.plan.identifier, task.plan.identifier,
                )));
            }
        }
        Ok(existing)
    })?;
    if let Some(existing) = existing {
        if let Some(parent) = stack_on.as_deref() {
            block_on_task(async {
                stack_existing_task(&task_store().await?, &existing, parent).await
            })?;
        }
        block_on_task(async {
            let store = task_store().await?;
            restore_task_checkout(&store, &existing).await
        })?;
        return Ok(existing);
    }
    let main_repo = crate::engine::worktrees::main_repo_root(repo).map_err(task_error)?;
    let resolved =
        crate::ops::task_pm::resolve_task(&main_repo, issue, crate::ops::pm::PmRefresh::Auto)?;
    if let Some(expected) = &expected_wave {
        if &resolved.wave != expected {
            return Err(task_error(format!(
                "--wave {expected} does not own Task {} (Wave {})",
                resolved.item.identifier, resolved.wave
            )));
        }
    }
    require_startable_issue(&resolved.item)?;
    let prepared = block_on_task(prepare_new_task(
        &main_repo,
        &resolved.item.name,
        Some(&resolved.item),
        &TaskExecOptions {
            wave: expected_wave,
            reason,
            name,
            stack_on,
            directive,
            flow: requested_flow,
            agent: requested_agent,
        },
    ))?;
    create_prepared_task(main_repo, resolved, prepared)
}

/// Recover checkout files from retained Task placement without replacing history.
async fn restore_task_checkout(store: &SharedStore, task: &Task) -> OpsResult<()> {
    let pr = store
        .active_task_pr(&task.id)
        .await
        .map_err(task_error)?
        .ok_or_else(|| task_error("Task has no active PR from which to restore its checkout"))?;
    let wave = owning_wave(store, task).await?;
    let repo = crate::engine::worktrees::main_repo_root(Path::new(wave.repo()))?;
    let _lease =
        crate::engine::git::acquire_worktree_lease(&repo, &task.worktree, "Task checkout")?;
    if task.worktree.join(".git").exists() {
        return finish_task_checkout(store, task, &pr).await;
    }
    if task.worktree.symlink_metadata().is_ok() {
        return Err(task_error(format!(
            "Task checkout path {} is occupied; its contents were preserved",
            task.worktree.display()
        )));
    }
    let checkouts = crate::engine::worktrees::list_worktrees(&repo)?;
    let destination = crate::store::canonicalize_with_missing_tail(&task.worktree)?;
    for other in checkouts
        .iter()
        .filter(|entry| entry.branch.as_deref() == Some(&pr.branch))
    {
        if crate::store::canonicalize_with_missing_tail(&other.path)? != destination {
            return Err(task_error(format!(
                "Task branch {} is registered at {}; preserve that checkout before restoring {}",
                pr.branch,
                other.path.display(),
                task.worktree.display()
            )));
        }
    }
    let blockers = lifecycle::associated_execution_blockers(store, task)?;
    if !blockers.is_empty() {
        return Err(task_error(blockers.join("; ")));
    }
    let mut args = vec!["worktree".to_string(), "add".into(), "--force".into()];
    // --force replaces only the stale registration at this absent exact path.
    // A different registered path was rejected above; no branch is reset.
    if !crate::engine::worktrees::branch_exists(&repo, &pr.branch)? {
        let remote = format!("refs/remotes/origin/{}", pr.branch);
        if !ref_exists(&repo, &remote)? && pr.github().is_some() {
            fetch(&repo, "origin", &pr.branch)?;
        }
        let base = if ref_exists(&repo, &remote)? {
            remote
        } else {
            let started = store.task_events_after(&task.id, 0).await.map_err(task_error)?
                .iter().any(|event| matches!(&event.kind, TaskEventKind::PrStarted { pr_id, .. } if pr_id == &pr.id));
            if started {
                return Err(task_error(format!("Task branch {} is missing locally and remotely; its committed work cannot be recovered from the base alone", pr.branch)));
            }
            pr.base_commit.clone()
        };
        args.extend([
            "--no-track".into(),
            "-b".into(),
            pr.branch.clone(),
            task.worktree.display().to_string(),
            base,
        ]);
    } else {
        args.extend([task.worktree.display().to_string(), pr.branch.clone()]);
    }
    git_output_bytes(&repo, &args.iter().map(String::as_str).collect::<Vec<_>>())?;
    finish_task_checkout(store, task, &pr).await
}

async fn finish_task_checkout(store: &SharedStore, task: &Task, pr: &TaskPr) -> OpsResult<()> {
    let events = store
        .task_events_after(&task.id, 0)
        .await
        .map_err(task_error)?;
    if !events.iter().any(
        |event| matches!(&event.kind, TaskEventKind::PrStarted { pr_id, .. } if pr_id == &pr.id),
    ) {
        // The first child commit owns the deletion. A retry after that commit
        // but before PrStarted must preserve any notes the child has since made.
        if pr.parent_pr_id.is_some() && rev_parse(&task.worktree, "HEAD")? == pr.base_commit {
            git_output_bytes(
                &task.worktree,
                &["rm", "-r", "-f", "--ignore-unmatch", "--", "scratch"],
            )?;
            git_output_bytes(
                &task.worktree,
                &["commit", "--allow-empty", "-m", "Clear inherited scratch"],
            )?;
        }
        store
            .append_task_event(
                &task.id,
                &TaskEventKind::PrStarted {
                    pr_id: pr.id.clone(),
                    sequence: pr.sequence,
                    branch: pr.branch.clone(),
                    base_commit: pr.base_commit.clone(),
                },
            )
            .await
            .map_err(task_error)?;
    }
    Ok(())
}

async fn stack_existing_task(store: &SharedStore, task: &Task, requested: &str) -> OpsResult<()> {
    let active = store
        .active_task_pr(&task.id)
        .await
        .map_err(|error| task_error(error.to_string()))?
        .ok_or_else(|| task_error("existing Task has no active PR"))?;
    let parent_task = store
        .get_task_by_issue(requested)
        .await
        .map_err(|error| task_error(error.to_string()))?
        .ok_or_else(|| task_error(format!("stack parent {requested:?} has no Task")))?;
    // A retry names the retained PR, even if its Task has since opened another PR.
    let parent = match &active.parent_pr_id {
        Some(id) => store.get_task_pr(id).await,
        None => store.active_task_pr(&parent_task.id).await,
    }
    .map_err(|error| task_error(error.to_string()))?
    .ok_or_else(|| task_error("stack parent PR is missing"))?;
    if parent.task_id != parent_task.id {
        return Err(task_error(format!(
            "Task {} already selects parent PR {}; changing an existing dependency requires explicit reparenting",
            task.plan.identifier, parent.id
        )));
    }
    let _mutation = lock_task_pr_mutation(&task.worktree)?;
    store
        .stack_task_pr(&active, &parent.id)
        .await
        .map_err(|error| task_error(error.to_string()))?;
    eprintln!(
        "Task {} selects parent PR {}. Checkout and GitHub are unchanged; run `lf sync` in {} to integrate it.",
        task.plan.identifier, parent.id, task.worktree.display()
    );
    Ok(())
}

#[derive(Debug)]
struct PreparedTask {
    plan: PlacementPlan,
    workspace_slug: String,
    stack_parent: Option<TaskPr>,
    github: Option<GithubPr>,
    requested_agent: Option<String>,
    directive: Option<String>,
}

async fn prepare_new_task(
    main_repo: &Path,
    title: &str,
    item: Option<&crate::pm::PmItem>,
    options: &TaskExecOptions,
) -> OpsResult<PreparedTask> {
    let directive = options
        .directive
        .as_deref()
        .map(str::trim)
        .map(str::to_string);
    if directive.as_deref() == Some("") {
        return Err(task_error("directive cannot be empty"));
    }
    let segment = match options.name.as_deref() {
        Some(name) => parse_workspace_slug(name)?,
        None => derive_workspace_slug(title)?,
    };
    let workspace_slug = segment.as_str().to_string();
    let branch = item
        .and_then(|item| item.branch_name.as_deref())
        .filter(|branch| !branch.is_empty());
    let mut plan = plan_branch_placement(main_repo, segment, branch)
        .map_err(|error| task_error(format!("failed to plan task worktree: {error}")))?;
    // Fail before filing an issue; execution checks again after provider work.
    if plan.strategy != PlacementStrategy::UseExistingWorktree && plan.worktree_path.exists() {
        return Err(task_error(format!(
            "worktree path already exists: {}",
            plan.worktree_path.display()
        )));
    }
    let stack_parent = if let Some(parent_issue) = options.stack_on.as_deref() {
        let store = task_store().await?;
        let parent_task = store
            .get_task_by_issue(parent_issue)
            .await
            .map_err(|error| task_error(format!("failed to read parent Task: {error}")))?
            .ok_or_else(|| {
                task_error(format!(
                    "stack parent {parent_issue:?} has no Task; run it first"
                ))
            })?;
        if Some(parent_task.plan.id.as_str()) == item.map(|item| item.id.as_str()) {
            return Err(task_error("a Task cannot stack on itself"));
        }
        let parent = store
            .active_task_pr(&parent_task.id)
            .await
            .map_err(|error| task_error(format!("failed to read parent PR: {error}")))?
            .ok_or_else(|| task_error("stack parent has no active PR"))?;
        if parent.github().is_none() {
            return Err(task_error(format!(
                "open the parent PR from {} before stacking work on it",
                parent_task.worktree.display()
            )));
        }
        Some(parent)
    } else {
        None
    };
    let mut base_commit = match &stack_parent {
        Some(parent) => {
            fetch(main_repo, "origin", &parent.branch).map_err(|error| {
                task_error(format!(
                    "failed to fetch parent branch {}: {error}",
                    parent.branch
                ))
            })?;
            let base_ref = format!("origin/{}", parent.branch);
            rev_parse(main_repo, &base_ref).map_err(|error| {
                task_error(format!("failed to resolve task base {base_ref}: {error}"))
            })?
        }
        None => {
            let (_, base_commit) = resolve_upstream_base(main_repo, &plan.base_ref)?;
            base_commit
        }
    };
    let github = if branch.is_some() || plan.strategy != PlacementStrategy::Create {
        super::pr::branch_pr(main_repo, &plan.branch)?
            .map(|pr| {
                Ok::<_, OpsError>(GithubPr {
                    number: u32::try_from(pr.number).map_err(task_error)?,
                    url: pr.url,
                    head_sha: pr.head_sha,
                })
            })
            .transpose()?
    } else {
        None
    };
    if plan.strategy == PlacementStrategy::Create && github.is_some() {
        // A fresh clone may know the PR before fetching its branch.
        fetch(
            main_repo,
            "origin",
            &format!(
                "refs/heads/{}:refs/remotes/origin/{}",
                plan.branch, plan.branch
            ),
        )
        .map_err(task_error)?;
        plan.strategy = PlacementStrategy::CheckoutExisting;
    }
    if plan.strategy != PlacementStrategy::Create {
        let branch_ref = if ref_exists(main_repo, &format!("refs/heads/{}", plan.branch))? {
            format!("refs/heads/{}", plan.branch)
        } else {
            format!("refs/remotes/origin/{}", plan.branch)
        };
        base_commit = merge_base(main_repo, &base_commit, &branch_ref).map_err(task_error)?;
    }
    // Provider creation can yield while another fetch advances the branch.
    // Place the checkout on the same commit recorded by its first PR.
    plan.base_ref = base_commit;
    Ok(PreparedTask {
        plan,
        workspace_slug,
        stack_parent,
        github,
        requested_agent: options.agent.clone(),
        directive,
    })
}

fn create_prepared_task(
    main_repo: PathBuf,
    resolved: crate::ops::task_pm::ResolvedTask,
    prepared: PreparedTask,
) -> OpsResult<Task> {
    let PreparedTask {
        plan,
        workspace_slug,
        stack_parent,
        github,
        requested_agent,
        directive,
    } = prepared;
    let project = block_on_task(crate::ops::project::resolve_project_for_task(
        &main_repo,
        &resolved.wave,
        &resolved.project.id,
    ))?;
    let project_id = project.id.clone();
    let wave_id = project.wave_id.clone();

    block_on_task(async move {
        let store = task_store().await?;
        // Re-resolve after worktree planning: a concurrent run may have created
        // the Task in the gap. Non-terminal Work wins. Terminal Work remains
        // authoritative and requires an explicit recovery transition.
        if let Some(mut existing) = store
            .get_task_by_issue(&resolved.item.id)
            .await
            .map_err(|error| task_error(format!("failed to read task registry: {error}")))?
        {
            match task_work_status(&store, &existing).await? {
                WorkStatus::Done => {
                    return Err(task_error(format!(
                        "Task {} is completed; start a new Linear task",
                        existing.plan.identifier
                    )))
                }
                WorkStatus::Abandoned => {
                    return Err(task_error(format!(
                    "Task {} is abandoned; inspect its retained history with `lf task status {}`",
                    existing.plan.identifier, existing.plan.identifier
                )))
                }
                WorkStatus::Ready => {
                    select_task_agent(&store, &mut existing, requested_agent.as_deref()).await?;
                    return Ok(existing);
                }
            }
        }
        let now = time::OffsetDateTime::now_utc();
        let task = Task {
            id: crate::work::task::TaskId::new(),
            plan: TaskPlan {
                id: LinearIssueId::new(resolved.item.id.clone()).map_err(task_error)?,
                identifier: resolved.item.identifier.clone(),
                title: resolved.item.name.clone(),
                description: resolved.item.description.clone(),
                pm_snapshot_synced_at: resolved.observed_at,
            },
            wave_id,
            project_id,
            pm_writeback: PmWritebackState::Current,
            worktree: plan.worktree_path.clone(),
            workspace_slug: workspace_slug.clone(),
            agent: requested_agent,
            abandon_intent: None,
            created_at: now,
            updated_at: now,
            observation: crate::work::task::Observation::NotRequired,
        };
        let pr = TaskPr {
            id: TaskPrId::new(),
            task_id: task.id.clone(),
            sequence: 1,
            slug: workspace_slug,
            branch: plan.branch.clone(),
            base_commit: plan.base_ref.clone(),
            parent_pr_id: stack_parent.as_ref().map(|parent| parent.id.clone()),
            publication: github.map(|github| PrPublication {
                requested_at: now,
                presentation: None,
                github: Some(github),
                merge: None,
            }),
            merge_commit: None,
            abandoned_at: None,
            ci_observation: None,
            github_observation: None,
            linear_attachment_id: None,
            linear_comment_id: None,
            linear_link_error: None,
            created_at: now,
            updated_at: now,
        };

        let _checkout_lease = crate::engine::git::acquire_worktree_lease(
            &main_repo,
            &task.worktree,
            "Task checkout",
        )?;
        match store.create_task_with_worktree(&task, &pr).await {
            Ok(()) => {
                if let Some(direction) = directive.as_deref() {
                    let mut publication_task = task.clone();
                    publication_task.worktree = main_repo.clone();
                    super::linear_observe::publish_task_steer(&store, &publication_task, direction)
                        .await?;
                }
            }
            Err(StoreError::Sqlite(_)) => {
                if let Some(mut existing) = store
                    .get_task_by_issue(&resolved.item.id)
                    .await
                    .map_err(|error| {
                        task_error(format!("failed to recover task reservation: {error}"))
                    })?
                {
                    if !matches!(
                        task_work_status(&store, &existing).await?,
                        WorkStatus::Done | WorkStatus::Abandoned
                    ) {
                        select_task_agent(&store, &mut existing, task.agent.as_deref()).await?;
                        return Ok(existing);
                    }
                }
                return Err(task_error(
                    "task reservation collided with another task placement",
                ));
            }
            Err(error) => {
                return Err(task_error(format!(
                    "failed to create Task planning state: {error}"
                )))
            }
        }
        if let Err(error) = create_from_placement_plan(&main_repo, &plan) {
            if let Err(event_error) = store
                .append_task_event(
                    &task.id,
                    &TaskEventKind::Failed {
                        error: error.to_string(),
                        resumable: true,
                    },
                )
                .await
            {
                tracing::warn!(task = %task.id, %event_error, "worktree creation failed after Task planning state committed; failure event did not persist");
            }
            return Err(task_error(format!("failed to create task wt: {error}")));
        }

        finish_task_checkout(&store, &task, &pr).await?;
        Ok(task)
    })
}

pub(crate) fn project_context(project: &crate::pm::PmProject) -> String {
    let mut context = format!(
        "Chapter metric targets:\n{}",
        serde_json::to_string(&project.metric_targets).expect("metric targets serialize")
    );
    context.push_str(&format!("\n\nChapter Task flow: {}", project.flow));
    if !project.krs.is_empty() {
        context.push_str("\n\nKRs:");
        for kr in &project.krs {
            let mark = if kr.holds { "x" } else { " " };
            context.push_str(&format!("\n- [{mark}] {}", kr.text));
        }
    }
    context
}

pub fn task_create(
    repo: &Path,
    wave: Option<&str>,
    title: Option<String>,
    report: Option<String>,
) -> OpsResult<crate::pm::PmItem> {
    let input = resolve_task_create_input(title.as_deref(), report.as_deref())?;
    let main = crate::engine::worktrees::main_repo_root(repo).map_err(task_error)?;
    let project =
        crate::ops::task_pm::resolve_current_project(&main, wave, crate::ops::pm::PmRefresh::Auto)?;
    let marker = format!(
        "<!-- loopflow-task-start:{} -->",
        hex::encode(Sha256::digest(
            format!(
                "{}\0{}\0{}",
                project.snapshot.wave, input.title, input.report
            )
            .as_bytes()
        ))
    );
    let (created, ()) = block_on_task(crate::ops::task_pm::create_and_load_task(
        &main,
        &project.snapshot.wave,
        &input.title,
        &input.report,
        &marker,
        |_, _| async { Ok(()) },
    ))?;
    Ok(created.item)
}

pub fn resolve_task_create_input(
    explicit_title: Option<&str>,
    piped_report: Option<&str>,
) -> OpsResult<TaskCreateInput> {
    let report = piped_report
        .map(str::trim)
        .filter(|value| !value.is_empty());
    let title = explicit_title
        .map(str::trim)
        .filter(|value| !value.is_empty());
    let title =
        match (title, report) {
            (Some(title), _) => title.to_string(),
            (None, Some(report)) => {
                let first_line = report
                    .lines()
                    .map(str::trim)
                    .find(|line| !line.is_empty())
                    .expect("non-empty report has a meaningful line");
                truncate_task_title(first_line, 100)
            }
            (None, None) => return Err(task_error(
                "Task title or piped report is required: `pbpaste | lf task create --wave <wave>`",
            )),
        };
    let report = report.unwrap_or(&title).to_string();
    Ok(TaskCreateInput { title, report })
}

fn truncate_task_title(value: &str, max_chars: usize) -> String {
    if value.chars().count() <= max_chars {
        return value.to_string();
    }
    let mut title = value.chars().take(max_chars - 1).collect::<String>();
    title.push('…');
    title
}

pub(crate) fn resolve_task_agent(
    worktree: &Path,
    agent: Option<&str>,
    skill: Option<&crate::engine::Skill>,
) -> String {
    crate::engine::exec::resolve_agent(agent, skill, &load_config_or_default(Some(worktree)))
}

async fn select_task_agent(
    store: &SharedStore,
    task: &mut Task,
    agent: Option<&str>,
) -> OpsResult<()> {
    if let Some(agent) = agent {
        checkout_execution_boundary(&task.worktree, agent)
            .map_err(|error| task_error(error.to_string()))?;
        store
            .set_task_agent(&task.id, agent)
            .await
            .map_err(|error| task_error(format!("failed to save Task agent: {error}")))?;
        task.agent = Some(agent.to_string());
    }
    Ok(())
}

fn task_configuration_refusal(task: &Task, skill: Option<&crate::engine::Skill>) -> Option<String> {
    checkout_execution_boundary(
        &task.worktree,
        &resolve_task_agent(&task.worktree, task.agent.as_deref(), skill),
    )
    .err()
    .map(|error| error.to_string())
}

pub(crate) async fn task_exec_refusal(
    store: &SharedStore,
    task: &Task,
) -> crate::store::StoreResult<Option<String>> {
    if let Some(refusal) = task_configuration_refusal(task, None) {
        return Ok(Some(refusal));
    }
    persisted_task_exec_refusal(store, task).await
}

async fn persisted_task_exec_refusal(
    store: &SharedStore,
    task: &Task,
) -> crate::store::StoreResult<Option<String>> {
    let event = store.latest_task_event(&task.id).await?;
    Ok(task_event_exec_refusal(event.as_ref()).map(str::to_string))
}

pub(crate) fn task_event_exec_refusal(
    event: Option<&crate::work::task::TaskEvent>,
) -> Option<&str> {
    match event.map(|event| &event.kind) {
        Some(TaskEventKind::Failed {
            error,
            resumable: false,
        }) => Some(error),
        _ => None,
    }
}

fn require_startable_issue(item: &crate::pm::PmItem) -> OpsResult<()> {
    if item.terminal_reason().is_some() {
        return Err(task_error(format!(
            "Task {} is terminal and cannot start execution",
            item.identifier
        )));
    }
    Ok(())
}

/// Existing work consumes validated local facts; refresh and provider writes own acquisition.
/// What a Flow launch for a Task must hold, however the Task was named: ready
/// Work in its Wave's current chapter whose planning still matches.
pub(crate) async fn require_task_flow_launch(
    store: &SharedStore,
    task_id: &crate::work::task::TaskId,
) -> OpsResult<()> {
    let task = store
        .get_task(task_id)
        .await
        .map_err(task_error)?
        .ok_or_else(|| task_error(format!("Task {task_id} is not registered")))?;
    if task_work_status(store, &task).await? != WorkStatus::Ready {
        return Err(task_error(format!(
            "Task {} is terminal and cannot launch a Flow",
            task.plan.identifier
        )));
    }
    let resolved = crate::ops::task_pm::resolve_task_async(
        &task.worktree,
        task.plan.id.as_str(),
        crate::ops::pm::PmRefresh::Never,
    )
    .await?;
    require_startable_issue(&resolved.item)?;
    let project = store
        .get_project(&task.project_id)
        .await
        .map_err(task_error)?
        .ok_or_else(|| task_error("Task Project is missing"))?;
    let wave = owning_wave(store, &task).await?;
    if resolved.item.id != task.plan.id.as_str()
        || resolved.project.id != project.plan.id.as_str()
        || resolved.wave != wave.slug()
    {
        return Err(task_error(format!(
            "Task {} planning no longer matches its registered Work; its Flow history is preserved",
            task.plan.identifier
        )));
    }
    store
        .sqlite
        .require_task_launch(&task.id)
        .map_err(task_error)
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct TaskWorktreeBlocker {
    pub initializing: bool,
    pub reason: String,
}

const TASK_WORKTREE_INITIALIZATION_GRACE: time::Duration = time::Duration::minutes(5);

pub(crate) async fn task_worktree_blocker(
    store: &SharedStore,
    task: &Task,
) -> OpsResult<Option<TaskWorktreeBlocker>> {
    let event = store
        .latest_task_event(&task.id)
        .await
        .map_err(|error| task_error(format!("failed to read Task worktree state: {error}")))?;
    if let Some(event) = event {
        if let TaskEventKind::WorktreeInitializing { branch, path, .. } = &event.kind {
            let initializing = event.created_at + TASK_WORKTREE_INITIALIZATION_GRACE
                > time::OffsetDateTime::now_utc();
            let reason = if initializing {
                format!(
                    "Task {} is initializing worktree {path} on branch {branch:?}; no body is expected until placement completes",
                    task.plan.identifier
                )
            } else {
                format!(
                    "Task {} worktree initialization did not complete at {path} on branch {branch:?}; finish or restore that exact path before `lf task run {}`; Task identity and PR history are unchanged",
                    task.plan.identifier, task.plan.identifier
                )
            };
            return Ok(Some(TaskWorktreeBlocker {
                initializing,
                reason,
            }));
        }
    }
    if task.worktree.exists() {
        return Ok(None);
    }
    let active = store
        .active_task_pr(&task.id)
        .await
        .map_err(|error| task_error(format!("failed to read active Task PR: {error}")))?;
    let branch = active
        .as_ref()
        .map(|pr| format!(" on branch {:?}", pr.branch))
        .unwrap_or_default();
    Ok(Some(TaskWorktreeBlocker {
        initializing: false,
        reason: format!(
            "Task {} worktree {} is missing; restore that exact path{branch} before `lf task run {}`; Task identity and PR history are unchanged",
            task.plan.identifier,
            task.worktree.display(),
            task.plan.identifier,
        ),
    }))
}

fn load_task_flow(repo: &Path, requested: &str) -> OpsResult<(String, Vec<ConcreteStep>)> {
    let definition = load_flow(requested, repo)
        .map_err(|error| task_error(format!("failed to load Task flow {requested:?}: {error}")))?;
    let steps = compile_flow(&definition, repo).map_err(|error| {
        task_error(format!("failed to expand Task flow {requested:?}: {error}"))
    })?;
    if steps.is_empty() {
        return Err(task_error(format!("Task flow {requested:?} has no steps")));
    }

    Ok((definition.name, steps))
}

fn parse_workspace_slug(value: &str) -> OpsResult<WorktreeSegment> {
    let value = value.trim();
    let words = value.split('-').filter(|word| !word.is_empty()).count();
    if sanitize_for_branch(value) != value
        || value.contains(['.', '_', '/'])
        || !(2..=5).contains(&words)
    {
        return Err(task_error(
            "workspace name must be 2-5 lowercase kebab-case words",
        ));
    }
    WorktreeSegment::parse(value).map_err(task_error)
}

fn derive_workspace_slug(title: &str) -> OpsResult<WorktreeSegment> {
    derive_workspace_slug_with_cap(title, 5)
}

/// Derive a workspace slug, keeping the kebab-word count at or below `max_words`
/// so a caller that appends a suffix word still fits the 2-5 word limit.
fn derive_workspace_slug_with_cap(title: &str, max_words: usize) -> OpsResult<WorktreeSegment> {
    let sanitized = sanitize_for_branch(title);
    let mut words = sanitized
        .split('-')
        .filter(|word| !word.is_empty())
        .take(max_words)
        .collect::<Vec<_>>();
    if words.len() == 1 {
        words.push("task");
    }
    parse_workspace_slug(&words.join("-"))
}

fn parse_pr_slug(value: &str) -> OpsResult<String> {
    let value = value.trim();
    let words = value.split('-').filter(|word| !word.is_empty()).count();
    if sanitize_for_branch(value) != value
        || value.contains(['.', '_', '/'])
        || !(1..=5).contains(&words)
    {
        return Err(task_error(
            "next PR name must be 1-5 lowercase kebab-case words",
        ));
    }
    Ok(value.to_string())
}

pub(crate) async fn task_for_checkout(store: &SharedStore, repo: &Path) -> OpsResult<Option<Task>> {
    let Some(branch) = current_branch(repo).map_err(OpsError::from)? else {
        return Ok(None);
    };
    store
        .get_task_by_branch(&branch)
        .await
        .map_err(|error| task_error(format!("failed to resolve Task branch {branch:?}: {error}")))
}

/// A managed Task worktree, or an explicit decision
/// that this worktree is not a Task worktree.
///
/// The PR publication, stacking, submit, and land entry points share this one
/// resolver so they cannot disagree about Task ownership. An absent registry
/// without an explicit declaration permits ordinary PR work. An unreadable
/// registry or unresolved declaration reports the missing authority instead.
#[derive(Debug)]
enum ManagedTask {
    /// This checkout is not on a Task branch. Task-specific bookkeeping is an
    /// explicit no-op; the ordinary PR flow continues unchanged.
    Unmanaged,
    /// The registry is healthy and the checkout is on a Task's current branch.
    /// Boxed so the `Unmanaged` no-op variant stays small.
    Managed { store: SharedStore, task: Box<Task> },
}

/// Turn a [`RegistryUnavailable`] into an actionable authority error. The
/// message always names the recovery action so the operator can move.
fn task_registry_error(err: RegistryUnavailable) -> OpsError {
    task_error(match err {
        RegistryUnavailable::MissingFile { path } => format!(
            "Task PR authority refused: the shared Loopflow registry {} is missing. \
             Start the owning Wave (it creates the registry) or run `lf home doctor`.",
            path.display()
        ),
        RegistryUnavailable::Unresolved { error } => format!(
            "Task PR authority refused: the shared Loopflow registry path is not usable: {error}. \
             Fix LF_HOME or run `lf home doctor`."
        ),
        RegistryUnavailable::Incompatible { path, error } => format!(
            "Task PR authority refused: the shared Loopflow registry {} is present but \
             inaccessible or schema-incompatible: {error}. Run `lf home doctor`.",
            path.display()
        ),
    })
}

/// Resolve the managed Task for a PR entry point at `repo`.
///
/// - Checkout ownership, then an explicit declaration, resolves a Task → [`ManagedTask::Managed`].
/// - Neither resolves a Task → [`ManagedTask::Unmanaged`].
/// - Registry file missing without a declaration → [`ManagedTask::Unmanaged`].
///   Missing records cannot recover checkout ownership or authorize inherited identity.
/// - Registry present but unopenable → refuse.
async fn resolve_managed_task(repo: &Path) -> OpsResult<ManagedTask> {
    let store = match open_registry_for_authority().await {
        Ok(store) => Arc::new(store),
        Err(RegistryUnavailable::MissingFile { .. })
            if std::env::var_os(crate::lf::WORK_DECLARATION_ENV).is_none() =>
        {
            return Ok(ManagedTask::Unmanaged);
        }
        Err(err) => return Err(task_registry_error(err)),
    };
    let binding = crate::ops::resolve_execution_binding(&store, repo).await?;
    let task = match binding.map(|binding| binding.work) {
        Some(crate::durable::WorkRef::Task(id)) => store.get_task(&id).await.map_err(task_error)?,
        _ => None,
    };
    match task {
        Some(task) => Ok(ManagedTask::Managed {
            store,
            task: Box::new(task),
        }),
        None => Ok(ManagedTask::Unmanaged),
    }
}

pub(crate) fn guard_task_mutation(repo: &Path) -> OpsResult<()> {
    block_on_task(async move {
        let _ = resolve_managed_task(repo).await?;
        Ok(())
    })
}

pub(crate) fn record_task_pr_repair(
    repo: &Path,
    kind: crate::work::task::TaskPrRepairKind,
) -> OpsResult<bool> {
    block_on_task(async move {
        let ManagedTask::Managed { store, task } = resolve_managed_task(repo).await? else {
            return Ok(false);
        };
        let Some(pr) = store
            .active_task_pr(&task.id)
            .await
            .map_err(|error| task_error(format!("failed to read active PR: {error}")))?
        else {
            return Ok(false);
        };
        let occurred_at = time::OffsetDateTime::now_utc();
        store
            .record_task_pr_repair_incident(&pr.id, kind, occurred_at)
            .await
            .map_err(|error| task_error(format!("failed to record Task PR repair: {error}")))
    })
}

pub(crate) fn request_task_pr_publication(repo: &Path, title: &str, body: &str) -> OpsResult<bool> {
    let title = title.trim();
    let body = body.trim();
    if title.is_empty() || body.is_empty() {
        return Err(task_error(
            "Task PR settlement requires a non-empty reviewer-facing title and body; supply both or let Loopflow generate them",
        ));
    }
    let head_sha = rev_parse(repo, "HEAD")?;
    block_on_task(async move {
        let ManagedTask::Managed { store, task } = resolve_managed_task(repo).await? else {
            return Ok(false);
        };
        let context = _task_pr_context_from_store(&store, &task).await?;
        _validate_task_pr_copy(&context, body)?;
        let mut pr = store
            .active_task_pr(&task.id)
            .await
            .map_err(|error| task_error(format!("failed to read active PR: {error}")))?
            .ok_or_else(|| task_error(format!("Task {} has no active PR", task.plan.identifier)))?;
        let branch = crate::engine::git::current_branch(repo)?
            .ok_or_else(|| task_error("Task worktree is not on a branch"))?;
        if pr.branch != branch {
            return Err(task_error(format!(
                "Task {} active PR expects branch {:?}, but the worktree is on another branch",
                task.plan.identifier, pr.branch
            )));
        }
        let now = time::OffsetDateTime::now_utc();
        let github = pr.github().cloned();
        let merge = pr
            .publication
            .as_ref()
            .and_then(|publication| publication.merge.as_ref())
            .filter(|request| {
                github
                    .as_ref()
                    .and_then(|github| github.head_sha.as_deref())
                    == Some(request.head_sha.as_str())
            })
            .cloned();
        pr.publication = Some(PrPublication {
            requested_at: pr
                .publication
                .as_ref()
                .map_or(now, |publication| publication.requested_at),
            presentation: Some(PrPresentation {
                title: title.to_string(),
                body: body.to_string(),
                head_sha,
            }),
            github,
            merge,
        });
        pr.updated_at = now;
        store
            .update_task_pr(&pr)
            .await
            .map_err(|error| task_error(format!("failed to request PR publication: {error}")))?;
        Ok(true)
    })
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct TaskPrContext {
    pub(crate) title: String,
    pub(crate) identifier: String,
    pub(crate) url: String,
    pub(crate) sequence: u32,
    pub(crate) merge_request: Option<PrMergeRequest>,
}

impl TaskPrContext {
    pub(crate) fn task_link(&self) -> String {
        format!(
            "[{} · {}]({})",
            _markdown_link_text(self.title.trim()),
            _markdown_link_text(self.identifier.trim()),
            self.url
        )
    }
}

fn _markdown_link_text(value: &str) -> String {
    value
        .replace('\\', "\\\\")
        .replace('[', "\\[")
        .replace(']', "\\]")
}

pub(crate) fn task_pr_context(repo: &Path) -> OpsResult<Option<TaskPrContext>> {
    block_on_task(async move {
        let ManagedTask::Managed { store, task } = resolve_managed_task(repo).await? else {
            return Ok(None);
        };
        _task_pr_context_from_store(&store, &task).await.map(Some)
    })
}

async fn _task_pr_context_from_store(store: &SharedStore, task: &Task) -> OpsResult<TaskPrContext> {
    let wave = owning_wave(store, task).await?;
    let snapshot = store
        .pm_snapshot(&task.wave_id)
        .await
        .map_err(|error| task_error(format!("failed to read cached PM snapshot: {error}")))?
        .ok_or_else(|| _missing_task_pr_url(task, wave.slug()))?;
    let snapshot = snapshot.snapshot;
    let item = snapshot
        .items
        .iter()
        .find(|item| item.id == task.plan.id.as_str())
        .ok_or_else(|| _missing_task_pr_url(task, wave.slug()))?;
    let url = item
        .url
        .as_deref()
        .filter(|url| _valid_task_url(url))
        .ok_or_else(|| _missing_task_pr_url(task, wave.slug()))?;
    let prs = store
        .task_prs(&task.id)
        .await
        .map_err(|error| task_error(format!("failed to read Task PRs: {error}")))?;
    let Some(pr) = prs.iter().find(|pr| pr.is_active()) else {
        // Work committed after the PR settled has nowhere to publish yet.
        let refusal = no_active_pr_resume_refusal(&task.plan.identifier, None, prs.last())
            .expect("no active PR yields a refusal");
        return Err(task_error(format!(
            "{refusal}; `lf pr next [slug]` carries follow-up work into the next PR"
        )));
    };
    Ok(TaskPrContext {
        title: task.plan.title.clone(),
        identifier: task.plan.identifier.clone(),
        url: url.to_string(),
        sequence: pr.sequence,
        merge_request: pr
            .merge_request()
            .filter(|request| {
                pr.github().and_then(|github| github.head_sha.as_deref())
                    == Some(request.head_sha.as_str())
            })
            .cloned(),
    })
}

fn _valid_task_url(value: &str) -> bool {
    let Ok(url) = reqwest::Url::parse(value) else {
        return false;
    };
    matches!(url.scheme(), "http" | "https")
        && url.host_str().is_some()
        && !value.chars().any(char::is_control)
}

fn _missing_task_pr_url(task: &Task, wave: &str) -> OpsError {
    task_error(format!(
        "Task {} has no valid provider URL in the cached PM snapshot. Run `lf repo refresh {wave}` before publishing this Task PR",
        task.plan.identifier,
    ))
}

fn _validate_task_pr_copy(context: &TaskPrContext, body: &str) -> OpsResult<()> {
    let anchor = format!("**Task:** {}", context.task_link());
    if !body.lines().any(|line| {
        line.trim()
            .strip_prefix('>')
            .map(str::trim)
            .is_some_and(|line| line == anchor)
    }) {
        return Err(task_error(format!(
            "Task PR body must include the owning Linear Task link: {anchor}"
        )));
    }
    Ok(())
}

/// The exact clean Task settlement already represented by local HEAD and the
/// stored GitHub head. `land` uses this read before any head mutation so a
/// replay can observe an already-armed request instead of clearing it.
pub(crate) fn matching_task_pr_merge_request(
    repo: &Path,
    mode: PrMergeMode,
    after_merge: AfterMerge,
    next_slug: Option<&str>,
) -> OpsResult<Option<(u32, String)>> {
    let next_slug = next_slug.map(parse_pr_slug).transpose()?;
    if after_merge == AfterMerge::CompleteTask && next_slug.is_some() {
        return Err(task_error("--complete and --next cannot be used together"));
    }
    block_on_task(async move {
        let ManagedTask::Managed { store, task } = resolve_managed_task(repo).await? else {
            return Ok(None);
        };
        if !is_clean(repo)? {
            return Ok(None);
        }
        let branch = current_branch(repo)?;
        let head = rev_parse(repo, "HEAD")?;
        let pr = store
            .active_task_pr(&task.id)
            .await
            .map_err(|error| task_error(format!("failed to read active PR: {error}")))?
            .ok_or_else(|| task_error(format!("Task {} has no active PR", task.plan.identifier)))?;
        if branch.as_deref() != Some(pr.branch.as_str()) {
            return Ok(None);
        }
        let Some(github) = pr.github() else {
            return Ok(None);
        };
        let Some(request) = pr.merge_request() else {
            return Ok(None);
        };
        if pr.presentation().is_none()
            || github.head_sha.as_deref() != Some(head.as_str())
            || request.mode != mode
            || request.after_merge != after_merge
            || request.next_slug != next_slug
        {
            return Ok(None);
        }
        Ok(Some((github.number, head)))
    })
}

/// Clear settlement intent before a Loopflow-owned operation can move the PR
/// head. Auto-merge is revoked remotely first; a crash between the two steps is
/// replay-safe because the next attempt observes it already disabled.
pub(crate) fn clear_task_pr_merge_before_head_mutation(
    repo: &Path,
    mutation_is_unconditional: bool,
    inherit_pr: &impl Fn(&mut Command),
) -> OpsResult<bool> {
    block_on_task(async move {
        let ManagedTask::Managed { store, task } = resolve_managed_task(repo).await? else {
            return Ok(false);
        };
        clear_task_pr_merge(&store, &task, repo, mutation_is_unconditional, inherit_pr).await
    })
}

/// Serialize the local operations that may change a Task PR head or its merge
/// request. The file descriptor owns the advisory lock until this guard drops.
#[derive(Debug)]
pub(crate) struct TaskPrMutationGuard {
    _file: File,
}

pub(crate) fn lock_task_pr_mutation(repo: &Path) -> OpsResult<TaskPrMutationGuard> {
    let path = crate::engine::git::absolute_git_dir(repo)?.join("lf-pr-mutation.lock");
    let file = OpenOptions::new()
        .create(true)
        .read(true)
        .write(true)
        .truncate(false)
        .open(path)?;
    match FileExt::try_lock_exclusive(&file) {
        Ok(()) => Ok(TaskPrMutationGuard { _file: file }),
        Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => Err(OpsError::Message(
            "another PR or branch-head mutation is already running for this worktree".to_string(),
        )),
        Err(error) => Err(error.into()),
    }
}

async fn clear_task_pr_merge(
    store: &SharedStore,
    task: &Task,
    repo: &Path,
    mutation_is_unconditional: bool,
    inherit_pr: &impl Fn(&mut Command),
) -> OpsResult<bool> {
    let mut pr = store
        .active_task_pr(&task.id)
        .await
        .map_err(|error| task_error(format!("failed to read active PR: {error}")))?
        .ok_or_else(|| task_error(format!("Task {} has no active PR", task.plan.identifier)))?;
    let Some(request) = pr
        .publication
        .as_ref()
        .and_then(|publication| publication.merge.as_ref())
        .cloned()
    else {
        return Ok(false);
    };
    if !mutation_is_unconditional {
        let head = rev_parse(repo, "HEAD")?;
        if is_clean(repo)? && head == request.head_sha {
            return Ok(false);
        }
    }
    if request.mode == PrMergeMode::Auto {
        let number = pr
            .github()
            .expect("merge request validation requires GitHub PR")
            .number;
        crate::ops::pr::disable_auto_merge(repo, number, inherit_pr)?;
    }
    pr.publication
        .as_mut()
        .expect("merge request requires publication")
        .merge = None;
    pr.updated_at = time::OffsetDateTime::now_utc();
    store
        .update_task_pr(&pr)
        .await
        .map_err(|error| task_error(format!("failed to clear stale PR merge request: {error}")))?;
    Ok(true)
}

/// Persist the explicit merge request before `submit` assigns or `land` arms
/// GitHub. Repeating the same mode/head request preserves its first timestamp.
pub(crate) fn request_task_pr_merge(
    repo: &Path,
    mode: PrMergeMode,
    head_sha: Option<&str>,
    after_merge: AfterMerge,
    next_slug: Option<&str>,
    inherit_pr: &impl Fn(&mut Command),
) -> OpsResult<bool> {
    let head_sha = head_sha.map(str::to_string);
    let next_slug = next_slug.map(parse_pr_slug).transpose()?;
    if after_merge == AfterMerge::CompleteTask && next_slug.is_some() {
        return Err(task_error("--complete and --next cannot be used together"));
    }
    block_on_task(async move {
        let ManagedTask::Managed { store, task } = resolve_managed_task(repo).await? else {
            return Ok(false);
        };
        let head_sha = head_sha
            .filter(|head| !head.trim().is_empty())
            .ok_or_else(|| {
                task_error(format!(
                    "GitHub did not report the current head for Task {}; refusing to request a merge without an exact commit",
                    task.plan.identifier
                ))
            })?;
        let mut pr = store
            .active_task_pr(&task.id)
            .await
            .map_err(|error| task_error(format!("failed to read active PR: {error}")))?
            .ok_or_else(|| task_error(format!("Task {} has no active PR", task.plan.identifier)))?;
        let publication = pr.publication.as_mut().ok_or_else(|| {
            task_error(format!(
                "Task {} has no durable PR publication request",
                task.plan.identifier
            ))
        })?;
        let github_head = publication
            .github
            .as_ref()
            .and_then(|github| github.head_sha.as_deref());
        if github_head != Some(head_sha.as_str()) {
            return Err(task_error(format!(
                "Task {} stored GitHub head {:?}, not requested merge head {}; refusing an unpinned settlement",
                task.plan.identifier, github_head, head_sha
            )));
        }
        if publication
            .presentation
            .as_ref()
            .is_none_or(|presentation| presentation.head_sha != head_sha)
        {
            return Err(task_error(format!(
                "Task {} has no non-empty reviewer-facing title and body for head {}; refresh PR copy before requesting settlement",
                task.plan.identifier, head_sha
            )));
        }
        if publication
            .merge
            .as_ref()
            .is_some_and(|request| request.mode == PrMergeMode::Auto)
            && mode == PrMergeMode::User
        {
            let number = publication
                .github
                .as_ref()
                .expect("merge request validation requires GitHub PR")
                .number;
            crate::ops::pr::disable_auto_merge(repo, number, inherit_pr)?;
        }
        let now = time::OffsetDateTime::now_utc();
        let requested_at = publication
            .merge
            .as_ref()
            .filter(|request| {
                request.mode == mode
                    && request.head_sha == head_sha
                    && request.after_merge == after_merge
                    && request.next_slug == next_slug
            })
            .map_or(now, |request| request.requested_at);
        publication.merge = Some(PrMergeRequest {
            mode,
            requested_at,
            head_sha: head_sha.clone(),
            after_merge,
            next_slug,
        });
        pr.updated_at = now;
        store
            .update_task_pr(&pr)
            .await
            .map_err(|error| task_error(format!("failed to request PR merge: {error}")))?;
        Ok(true)
    })
}

/// Whether the repository has at least one configured git remote.
fn has_remote(repo: &Path) -> OpsResult<bool> {
    Ok(!git_output(repo, &["remote"])?.trim().is_empty())
}

/// Resolve `(base_ref, base_commit)` for a new Task PR. With a remote, fetch and
/// anchor on `origin/<default>`; without one, fall back explicitly to local
/// `<default>`. The `base_ref` prefix (`origin/` vs `refs/heads/`) tells callers
/// which case applied.
fn resolve_upstream_base(repo: &Path, default_branch: &str) -> OpsResult<(String, String)> {
    let base_ref = if has_remote(repo)? {
        fetch(repo, "origin", default_branch)
            .map_err(|error| task_error(format!("failed to fetch task base: {error}")))?;
        format!("origin/{default_branch}")
    } else {
        format!("refs/heads/{default_branch}")
    };
    let base_commit = rev_parse(repo, &base_ref)
        .map_err(|error| task_error(format!("failed to resolve task base {base_ref}: {error}")))?;
    Ok((base_ref, base_commit))
}

/// Prove the active Task PR's ancestry is uncontaminated before the first push.
/// A worktree that is provably not a Task worktree is an explicit no-op — plain,
/// non-Task PRs are unaffected. A Task worktree whose registry is missing,
/// inaccessible, or schema-incompatible is refused with an actionable authority
/// error before any push, so it never degrades to generic PR behavior.
///
/// This is the **ancestry-only** gate: it runs before
/// `commit_workflow`/`prepare_land` push, where work may still be uncommitted,
/// so it cannot judge emptiness. Use [`require_task_pr_range_nonempty`] after
/// the publication path commits, before any `gh pr` side effect.
///
/// Let `B` = recorded `base_commit`, `O` = upstream tip (`origin/<default>` for
/// a root PR, or the live parent's branch tip for a stacked child), `H` = HEAD,
/// and `M = merge-base(O, H)`. The parity invariant is `M == B`, which
/// guarantees GitHub's range (`M..H`) equals the recorded range (`B..H`) equals
/// `lf diff --files`:
/// - `M == B` — parity holds; publish.
/// - `M` ancestor of `B` — the recorded base itself carries commits absent from
///   `O` (inherited foreign ancestry, the #877/#882 shape). Refuse before any
///   push, naming the foreign commits/files and the safe sync.
/// - `B` ancestor of `M` — `O` advanced past a stale or squash-merged base. Safe:
///   heal `base_commit → M` so the durable evidence and `lf diff --files` stay
///   truthful, then publish the minimal `M..H` range.
/// - divergent — ambiguous ancestry; refuse, naming the commits and files on
///   both sides (`M..B` and `B..M`) plus the safe sync.
pub(crate) fn verify_task_pr_range(repo: &Path) -> OpsResult<()> {
    let repo = repo.to_path_buf();
    block_on_task(async move {
        let ManagedTask::Managed { store, task } = resolve_managed_task(&repo).await? else {
            return Ok(());
        };
        verify_task_pr_range_in(&store, &task, &repo).await
    })
}

/// Prove the post-sync Task range against the operation's immutable target
/// without advancing durable metadata before a requested push is verified.
pub(crate) fn validate_task_pr_range_for_integration(
    repo: &Path,
    target_ref: &str,
    target_sha: &str,
) -> OpsResult<()> {
    verify_task_pr_range_for_integration(repo, target_ref, target_sha, StaleBaseAction::Accept)
}

/// Record the immutable base only after every requested Git postcondition,
/// including remote-head equality, has passed.
pub(crate) fn record_task_pr_range_after_integration(
    repo: &Path,
    target_ref: &str,
    target_sha: &str,
) -> OpsResult<()> {
    verify_task_pr_range_for_integration(repo, target_ref, target_sha, StaleBaseAction::Heal)
}

fn verify_task_pr_range_for_integration(
    repo: &Path,
    target_ref: &str,
    target_sha: &str,
    stale_base: StaleBaseAction,
) -> OpsResult<()> {
    let repo = repo.to_path_buf();
    let target_ref = target_ref.to_string();
    let target_sha = target_sha.to_string();
    block_on_task(async move {
        let ManagedTask::Managed { store, task } = resolve_managed_task(&repo).await? else {
            return Ok(());
        };
        verify_task_pr_range_mode(
            &store,
            &task,
            &repo,
            stale_base,
            Some((target_ref, target_sha)),
        )
        .await
    })
}

/// Prove the active Task PR's range is **authoritative and non-empty** before
/// any `gh pr create/edit/ready/merge` side effect. Runs the ancestry parity
/// proof (healing a stale base), then refuses when the tree at HEAD matches the
/// recorded base — an empty PR that must not reach GitHub. Unconditional: an
/// already-open PR reset or synced empty is refused just like a first
/// publication. A worktree that is provably not a Task worktree is an explicit
/// no-op; a Task worktree whose registry is unusable is refused with an
/// actionable authority error rather than degrading to generic PR behavior.
pub(crate) fn require_task_pr_range_nonempty(repo: &Path) -> OpsResult<()> {
    let repo = repo.to_path_buf();
    block_on_task(async move {
        let ManagedTask::Managed { store, task } = resolve_managed_task(&repo).await? else {
            return Ok(());
        };
        require_task_pr_range_nonempty_in(&store, &task, &repo).await
    })
}

/// Resolve the upstream a Task PR's ancestry should be measured against. A root
/// PR measures against `origin/<default>` (or local `<default>` without a
/// remote). A stacked child with a live parent measures against the parent's
/// branch tip — so the parent's own commits are expected ancestry, not foreign
/// contamination, and the child's range is `fork_point..HEAD` against the
/// durable parent boundary. A child whose parent merged (or was abandoned) has
/// been synced onto `<default>` by [`record_stack_sync`]; it measures
/// against `origin/<default>` like a root PR.
async fn resolve_verifier_upstream(
    store: &SharedStore,
    pr: &TaskPr,
    repo: &Path,
    default_branch: &str,
) -> OpsResult<(String, String)> {
    if let Some(parent_id) = pr.parent_pr_id.as_ref() {
        let parent = store
            .get_task_pr(parent_id)
            .await
            .map_err(|error| task_error(format!("failed to read stack parent: {error}")))?
            .ok_or_else(|| task_error(format!("stack parent {parent_id} is missing")))?;
        let parent_live = parent.merge_commit.is_none() && parent.abandoned_at.is_none();
        if parent_live {
            let base_ref = if has_remote(repo)? {
                fetch(repo, "origin", &parent.branch).map_err(|error| {
                    task_error(format!("failed to fetch parent branch: {error}"))
                })?;
                format!("origin/{}", parent.branch)
            } else {
                format!("refs/heads/{}", parent.branch)
            };
            let tip = rev_parse(repo, &base_ref).map_err(|error| {
                task_error(format!(
                    "failed to resolve parent branch {base_ref}: {error}"
                ))
            })?;
            return Ok((base_ref, tip));
        }
    }
    resolve_upstream_base(repo, default_branch)
}

/// Core parity proof. Takes the store + task explicitly so it can be
/// exercised in tests without a live LF_HOME (mirrors `ensure_working_pr`).
pub(crate) async fn verify_task_pr_range_in(
    store: &SharedStore,
    task: &Task,
    repo: &Path,
) -> OpsResult<()> {
    verify_task_pr_range_mode(store, task, repo, StaleBaseAction::Heal, None).await
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum StaleBaseAction {
    Accept,
    Heal,
}

async fn verify_task_pr_range_mode(
    store: &SharedStore,
    task: &Task,
    repo: &Path,
    stale_base: StaleBaseAction,
    upstream_override: Option<(String, String)>,
) -> OpsResult<()> {
    let mut pr = store
        .active_task_pr(&task.id)
        .await
        .map_err(|error| task_error(format!("failed to read active PR: {error}")))?
        .ok_or_else(|| task_error(format!("Task {} has no active PR", task.plan.identifier)))?;
    let branch =
        current_branch(repo)?.ok_or_else(|| task_error("Task worktree is not on a branch"))?;
    if pr.branch != branch {
        return Err(task_error(format!(
            "Task {} active PR expects branch {:?}, but the worktree is on {:?}",
            task.plan.identifier, pr.branch, branch
        )));
    }

    let (base_ref, upstream) = match upstream_override {
        Some(target) => target,
        None => {
            let default_branch = get_default_branch(repo)?;
            resolve_verifier_upstream(store, &pr, repo, &default_branch).await?
        }
    };
    let head = rev_parse(repo, "HEAD")
        .map_err(|error| task_error(format!("failed to resolve Task HEAD: {error}")))?;
    let base = pr.base_commit.clone();
    let identifier = &task.plan.identifier;
    let short = |sha: &str| sha.chars().take(12).collect::<String>();

    let merge_base = crate::engine::git::merge_base(repo, &upstream, &head).map_err(|_| {
        task_error(format!(
            "Task {identifier} branch {branch:?} shares no history with {base_ref}; \
             re-cut the branch from {base_ref} before publishing"
        ))
    })?;

    if merge_base == base {
        // Parity holds: the GitHub range is exactly base_commit..HEAD.
        return Ok(());
    }

    if crate::engine::git::is_ancestor(repo, &merge_base, &base)? {
        // M < B: the recorded base carries commits not on the upstream — the
        // foreign ancestry that contaminated #877/#882. Refuse before push.
        let range = format!("{merge_base}..{base}");
        let commits = git_output(repo, &["log", "--oneline", "--no-decorate", &range])?;
        let files = git_output(repo, &["diff", "--name-only", &range])?;
        let commits = commits.trim();
        let files = files.trim();
        return Err(task_error(format!(
            "Task {identifier} PR range is contaminated: recorded base {} carries commit(s) \
             not on {base_ref}, which would leak into the PR:\n{commits}\naffecting files:\n{files}\n\
             Refused before any push. Recover with:\n  git rebase --onto {base_ref} {} {branch}",
            short(&base),
            short(&base),
        )));
    }

    if crate::engine::git::is_ancestor(repo, &base, &merge_base)? {
        // B < M: the upstream advanced past a stale or squash-merged base.
        // Heal the recorded base to the true fork point so lf diff --files and
        // the durable evidence report the minimal M..HEAD range.
        if stale_base == StaleBaseAction::Accept {
            return Ok(());
        }
        pr.base_commit = merge_base.clone();
        pr.updated_at = time::OffsetDateTime::now_utc();
        store
            .heal_task_pr_base(&pr)
            .await
            .map_err(|error| task_error(format!("failed to heal Task PR base: {error}")))?;
        return Ok(());
    }

    // Neither is an ancestor of the other: genuinely ambiguous ancestry. Name
    // the commits and files on both sides so the user can identify exactly which
    // work is foreign without opening raw internals.
    let base_side = format!("{merge_base}..{base}");
    let upstream_side = format!("{base}..{merge_base}");
    let base_commits = git_output(repo, &["log", "--oneline", "--no-decorate", &base_side])?;
    let base_files = git_output(repo, &["diff", "--name-only", &base_side])?;
    let upstream_commits =
        git_output(repo, &["log", "--oneline", "--no-decorate", &upstream_side])?;
    let upstream_files = git_output(repo, &["diff", "--name-only", &upstream_side])?;
    Err(task_error(format!(
        "Task {identifier} PR base {} and {base_ref} have diverged with no common lineage at \
         the recorded base. Refused before any push.\n\
         Commits on the recorded base not on {base_ref}:\n{base_commits}\
         affecting files:\n{base_files}\n\
         Commits on {base_ref} not reachable from the recorded base:\n{upstream_commits}\
         affecting files:\n{upstream_files}\n\
         Recover with:\n  git rebase --onto {base_ref} {} {branch}",
        short(&base),
        short(&base),
    )))
}

/// Core authoritative non-empty proof. Runs the ancestry parity check (which
/// heals a stale base in place), then re-reads the PR and refuses when the tree
/// at HEAD matches the healed recorded base — an empty range that must not
/// reach `gh pr create/edit/ready/merge`. The emptiness check uses the
/// **recorded** `base_commit`, not a recomputed merge-base, so it stays
/// authoritative even when the upstream has advanced.
async fn require_task_pr_range_nonempty_in(
    store: &SharedStore,
    task: &Task,
    repo: &Path,
) -> OpsResult<()> {
    verify_task_pr_range_in(store, task, repo).await?;
    let pr = store
        .active_task_pr(&task.id)
        .await
        .map_err(|error| task_error(format!("failed to read active PR: {error}")))?
        .ok_or_else(|| task_error(format!("Task {} has no active PR", task.plan.identifier)))?;
    let base = &pr.base_commit;
    let identifier = &task.plan.identifier;
    let short = base.chars().take(12).collect::<String>();
    let head = rev_parse(repo, "HEAD")
        .map_err(|error| task_error(format!("failed to resolve Task HEAD: {error}")))?;
    if head == *base {
        return Err(task_error(format!(
            "Task {identifier} PR range is empty: HEAD is the recorded base {short}, so the PR has \
             no commits to publish. Commit the Task's work, or complete the Task directly if the \
             work is done. Refused before any GitHub side effect."
        )));
    }
    let range = format!("{base}..HEAD");
    let status = Command::new("git")
        .args(["diff", "--quiet", &range])
        .current_dir(repo)
        .status()?;
    if status.success() {
        return Err(task_error(format!(
            "Task {identifier} PR range is empty: the tree at HEAD matches the recorded base \
             {short}, so the PR has no changes to publish. Commit the Task's work, or complete the \
             Task directly if the work is done. Refused before any GitHub side effect."
        )));
    }
    Ok(())
}

pub(crate) fn attach_task_github_pr(
    repo: &Path,
    github_pr: Option<&crate::ops::pr::PrInfo>,
    inherit_pr: &impl Fn(&mut Command),
) -> OpsResult<bool> {
    block_on_task(async move {
        let ManagedTask::Managed { store, task } = resolve_managed_task(repo).await? else {
            return Ok(false);
        };
        let mut pr = store
            .active_task_pr(&task.id)
            .await
            .map_err(|error| task_error(format!("failed to read active PR: {error}")))?
            .ok_or_else(|| task_error(format!("Task {} has no active PR", task.plan.identifier)))?;
        let github_pr = github_pr.ok_or_else(|| {
            task_error(format!(
                "GitHub PR for Task {} could not be read after creation or update",
                task.plan.identifier
            ))
        })?;
        if github_pr.branch != pr.branch {
            return Err(task_error(format!(
                "Task {} active PR expects branch {:?}, but GitHub reported {:?}",
                task.plan.identifier, pr.branch, github_pr.branch
            )));
        }
        let number = u32::try_from(github_pr.number).map_err(|_| {
            task_error(format!(
                "pull request #{} exceeds supported range",
                github_pr.number
            ))
        })?;
        let url = github_pr.url.clone();
        let opened = pr
            .github()
            .is_none_or(|github| github.number != number || github.url != url);
        // An existing GitHub PR may predate local publication. Retain its
        // identity now; reviewer copy is recorded only after promotion succeeds.
        let publication = pr.publication.get_or_insert_with(|| PrPublication {
            requested_at: time::OffsetDateTime::now_utc(),
            presentation: None,
            github: None,
            merge: None,
        });
        // A known identity can carry a head-pinned merge request. Revoke it
        // before replacing its head; the previously acknowledged identity
        // remains stored if that reconciliation fails. First attachment has
        // no merge request and can be saved immediately.
        invalidate_stale_merge_request(repo, publication, github_pr, inherit_pr)?;
        publication.github = Some(GithubPr {
            number,
            url: url.clone(),
            head_sha: github_pr.head_sha.clone(),
        });
        // Persist acknowledged identity before any further provider operation.
        pr.updated_at = time::OffsetDateTime::now_utc();
        store
            .update_task_pr(&pr)
            .await
            .map_err(|error| task_error(format!("failed to attach GitHub PR: {error}")))?;
        if opened {
            let event = TaskEventKind::PrOpened {
                pr_id: pr.id.clone(),
                sequence: pr.sequence,
                number,
                url,
            };
            store
                .append_task_event(&task.id, &event)
                .await
                .map_err(task_error)?;
        }
        // Linear linkage is idempotent. A failed or interrupted writeback cannot
        // erase the GitHub identity already committed above.
        link_pr_to_linear(&store, &task, &mut pr).await;
        pr.updated_at = time::OffsetDateTime::now_utc();
        store
            .update_task_pr(&pr)
            .await
            .map_err(|error| task_error(format!("failed to record PR linkage: {error}")))?;
        Ok(true)
    })
}

/// A merge request belongs to one exact head. Revoke an armed auto-merge before
/// forgetting a stale request so a later push cannot inherit settlement intent.
fn invalidate_stale_merge_request(
    repo: &Path,
    publication: &mut PrPublication,
    github_pr: &crate::ops::pr::PrInfo,
    inherit_pr: &impl Fn(&mut Command),
) -> OpsResult<()> {
    let Some(request) = publication.merge.as_ref() else {
        return Ok(());
    };
    let observed_head = github_pr.head_sha.as_deref().ok_or_else(|| {
        task_error(format!(
            "GitHub did not report the current head for pull request #{}; refusing to change its head-pinned merge request",
            github_pr.number
        ))
    })?;
    if observed_head == request.head_sha {
        return Ok(());
    }
    if request.mode == PrMergeMode::Auto && matches!(github_pr.state.as_str(), "open" | "draft") {
        let number = u32::try_from(github_pr.number).map_err(|_| {
            task_error(format!(
                "pull request #{} exceeds supported range",
                github_pr.number
            ))
        })?;
        crate::ops::pr::disable_auto_merge(repo, number, inherit_pr)?;
    }
    publication.merge = None;
    Ok(())
}

pub(crate) async fn reconcile_task_pr(
    store: &SharedStore,
    task: &mut Task,
) -> OpsResult<Option<TaskPr>> {
    reconcile_task_pr_observation(store, task, crate::ops::pr::PrReadFreshness::Cached).await
}

pub(crate) fn reconcile_checkout_pr(repo: &Path) -> OpsResult<Option<TaskPr>> {
    block_on_task(async {
        let store = task_store().await?;
        let Some(mut task) = task_for_checkout(&store, repo).await? else {
            return Ok(None);
        };
        let pr = reconcile_task_pr_observation(
            &store,
            &mut task,
            crate::ops::pr::PrReadFreshness::Fresh,
        )
        .await?;
        if let Observation::Degraded { reason, .. } = task.observation {
            return Err(task_error(reason));
        }
        Ok(pr)
    })
}

/// Apply a watched landing's authoritative merged observation to its Task.
/// The landing owns Auto settlement; Task and Project runners do not poll or
/// infer it from process liveness.
pub(crate) async fn settle_task_landing(
    store: &SharedStore,
    landing: &crate::pr_landing::PrLanding,
) -> OpsResult<()> {
    let task_id = landing
        .task_id
        .as_ref()
        .ok_or_else(|| task_error("direct landing has no Task to settle"))?;
    let mut task = store
        .get_task(task_id)
        .await
        .map_err(|error| task_error(format!("failed to read landing Task: {error}")))?
        .ok_or_else(|| task_error(format!("landing Task {task_id} disappeared")))?;
    // A previous check may have rotated to the next PR before it exited.
    // Reconcile the delivery's PR, not whichever successor is active now.
    let settled = store
        .task_prs(task_id)
        .await
        .map_err(|error| task_error(error.to_string()))?
        .into_iter()
        .find(|pr| {
            pr.phase() == PrPhase::Merged
                && pr.github().map(|github| github.number) == Some(landing.pr_number)
        });
    let pr = match settled {
        Some(pr) => pr,
        None => {
            reconcile_task_pr_observation(store, &mut task, crate::ops::pr::PrReadFreshness::Fresh)
                .await?
                .ok_or_else(|| task_error("landing Task PR disappeared during merge settlement"))?
        }
    };
    apply_merged_task_landing(store, &mut task, &pr, landing).await?;
    if landing.after_merge == Some(AfterMerge::CompleteTask) {
        if let PmWritebackState::Pending { error, .. } = &task.pm_writeback {
            return Err(task_error(format!("Linear completion pending: {error}")));
        }
        if task_work_status(store, &task).await? != WorkStatus::Done {
            return Err(task_error("Task completion gate is not yet satisfied"));
        }
    }
    Ok(())
}

async fn apply_merged_task_landing(
    store: &SharedStore,
    task: &mut Task,
    pr: &TaskPr,
    landing: &crate::pr_landing::PrLanding,
) -> OpsResult<()> {
    if pr.phase() != PrPhase::Merged
        || pr.github().map(|github| github.number) != Some(landing.pr_number)
        || pr.head_sha() != Some(landing.observed_head_sha.as_str())
        || pr.merge_request().map(|request| request.after_merge) != landing.after_merge
        || pr
            .merge_request()
            .and_then(|request| request.next_slug.as_ref())
            != landing.next_slug.as_ref()
    {
        return Err(task_error(format!(
            "GitHub did not confirm landing pull request #{} merged for Task {}",
            landing.pr_number, task.plan.identifier
        )));
    }
    match landing.after_merge {
        Some(AfterMerge::CompleteTask) => reconcile_task_completion(store, task).await,
        Some(AfterMerge::ContinueTask) if landing.next_slug.is_some() => {
            ensure_working_pr(store, task).await.map(|_| ())
        }
        Some(AfterMerge::ContinueTask) | None => Ok(()),
    }
}

/// Read the open PR's required checks and classify them for `head_sha`. Returns
/// `None` — no current-head CI owner can be derived — when GitHub reports no
/// head, there are no required checks, or gh is unavailable.
/// Failure dominates: any failing required check makes the head `Failing` even
/// while others are still pending.
fn observe_required_checks(
    worktree: &Path,
    pr_number: u64,
    head_sha: Option<&str>,
    now: time::OffsetDateTime,
) -> Option<CiObservation> {
    let head_sha = head_sha?.to_string();
    let checks = crate::ops::pr::merge_gate_state(worktree, pr_number, &head_sha)
        .ok()
        .flatten()?;
    let state = if checks.failing {
        CiState::Failing
    } else if checks.pending {
        CiState::Pending
    } else {
        CiState::Passing
    };
    Some(CiObservation {
        head_sha,
        state,
        // Keep actionable leaf failures, never the required aggregate, so the
        // landing supervisor can report the broken jobs precisely.
        failing_checks: checks
            .failing_leaves
            .into_iter()
            .map(|check| CiCheck {
                name: check.name,
                url: check.url,
            })
            .collect(),
        observed_at: now,
    })
}

// Local control commands often arrive in a burst (`status`, then `follow-up`,
// then another `status`). One minute keeps merge/CI state responsive while
// bounding those bursts to one GitHub read. A failed read opens a longer circuit:
// a quota or outage should not be hammered by every short-lived `lf` process.
const PR_OBSERVATION_TTL: time::Duration = time::Duration::seconds(60);
const PR_OBSERVATION_DEGRADED_BACKOFF: time::Duration = time::Duration::minutes(5);

fn cached_github_observation(pr: &TaskPr, now: time::OffsetDateTime) -> Option<Observation> {
    let observation = pr.github_observation.as_ref()?;
    let retry_at = observation.checked_at
        + match observation.result {
            GithubObservationResult::Fresh | GithubObservationResult::Partial { .. } => {
                PR_OBSERVATION_TTL
            }
            GithubObservationResult::Degraded { .. } => PR_OBSERVATION_DEGRADED_BACKOFF,
        };
    if retry_at <= now {
        return None;
    }
    Some(match &observation.result {
        GithubObservationResult::Fresh | GithubObservationResult::Partial { .. } => {
            Observation::Cached {
                observed_at: observation.checked_at,
            }
        }
        GithubObservationResult::Degraded { reason } => Observation::Degraded {
            reason: reason.clone(),
            cached_as_of: pr.updated_at,
            retry_at,
        },
    })
}

/// The PR this reconcile answers for: the active row, else the newest published
/// settlement. Merged evidence remains available to completion retries.
///
/// `abandoned_at` on a published PR caches GitHub's closed state rather than
/// deciding it — `lf pr abandon` runs `gh pr close` before stamping it — so a
/// reopen must be able to clear it. A merge is terminal: GitHub cannot unmerge.
async fn reconcile_subject(store: &SharedStore, task: &Task) -> OpsResult<Option<TaskPr>> {
    if let Some(active) = store
        .active_task_pr(&task.id)
        .await
        .map_err(|error| task_error(format!("failed to read active PR: {error}")))?
    {
        return Ok(Some(active));
    }
    let prs = store
        .task_prs(&task.id)
        .await
        .map_err(|error| task_error(format!("failed to read Task PRs: {error}")))?;
    Ok(prs.into_iter().next_back().filter(|pr| {
        matches!(pr.phase(), PrPhase::Abandoned | PrPhase::Merged) && pr.github().is_some()
    }))
}

async fn reconcile_task_pr_observation(
    store: &SharedStore,
    task: &mut Task,
    freshness: crate::ops::pr::PrReadFreshness,
) -> OpsResult<Option<TaskPr>> {
    // Reconciliation updates the same projection as publication/finalization.
    // Refuse overlap so a remote read begun before a push cannot overwrite the
    // request or head recorded by the command that completed after it.
    let _mutation = lock_task_pr_mutation(&task.worktree)?;
    let Some(mut pr) = reconcile_subject(store, task).await? else {
        return Ok(None);
    };
    if pr.phase() == PrPhase::Merged {
        task.observation = Observation::NotRequired;
        return Ok(Some(pr));
    }
    // Ordinary status does not enumerate unpublished branches. Explicit fresh
    // reconciliation can recover a publication whose GitHub identity was lost.
    let number = pr.github().map(|github| github.number);
    if number.is_none() && matches!(freshness, crate::ops::pr::PrReadFreshness::Cached) {
        task.observation = Observation::NotRequired;
        return Ok(Some(pr));
    }
    let now = time::OffsetDateTime::now_utc();
    // A fresh caller must never receive the store's warm head observation.
    if matches!(freshness, crate::ops::pr::PrReadFreshness::Cached) {
        if let Some(observation) = cached_github_observation(&pr, now) {
            task.observation = observation;
            return Ok(Some(pr));
        }
    }
    let previous = pr.clone();
    let observation = match number {
        Some(number) => {
            crate::ops::pr::observe_pr_by_number(&task.worktree, number, &pr.branch, freshness)
        }
        None => crate::ops::pr::observe_pr_by_branch(&task.worktree, &pr.branch),
    };
    let github_pr = match observation {
        crate::ops::pr::PrObservation::Fresh(info) => {
            pr.github_observation = Some(GithubObservation {
                checked_at: now,
                result: GithubObservationResult::Fresh,
            });
            task.observation = Observation::Fresh { observed_at: now };
            info
        }
        crate::ops::pr::PrObservation::NotFound => {
            // The PR ref was deleted remotely; a merge (if any) is already
            // persisted. Cache the successful absence briefly and keep the
            // settled/working state.
            pr.github_observation = Some(GithubObservation {
                checked_at: now,
                result: GithubObservationResult::Fresh,
            });
            pr.updated_at = now;
            store.update_task_pr(&pr).await.map_err(task_error)?;
            task.observation = Observation::Fresh { observed_at: now };
            return Ok(Some(pr));
        }
        crate::ops::pr::PrObservation::Degraded { reason } => {
            let retry_at = now + PR_OBSERVATION_DEGRADED_BACKOFF;
            pr.github_observation = Some(GithubObservation {
                checked_at: now,
                result: GithubObservationResult::Degraded {
                    reason: reason.clone(),
                },
            });
            // `updated_at` remains the time of the cached PR data, not the
            // failed attempt. Only the observation metadata changes.
            store.update_task_pr(&pr).await.map_err(task_error)?;
            task.observation = Observation::Degraded {
                reason,
                cached_as_of: pr.updated_at,
                retry_at,
            };
            return Ok(Some(pr));
        }
    };
    let number = u32::try_from(github_pr.number).map_err(|_| {
        task_error(format!(
            "pull request #{} exceeds supported range",
            github_pr.number
        ))
    })?;
    let url = github_pr.url.clone();
    let previous_phase = previous.phase();
    let previous_github = previous.github().cloned();
    let publication = pr.publication.get_or_insert(PrPublication {
        requested_at: now,
        presentation: None,
        github: None,
        merge: None,
    });
    invalidate_stale_merge_request(&task.worktree, publication, &github_pr, &|_| {})?;
    publication.github = Some(GithubPr {
        number,
        url: url.clone(),
        head_sha: github_pr.head_sha.clone(),
    });

    let mut authoritative_merged_at = None;
    let pr_event = match github_pr.state.as_str() {
        "merged" => {
            let merge_commit = github_pr.merge_commit.clone().ok_or_else(|| {
                task_error(format!(
                    "GitHub reports pull request #{} merged without a merge commit",
                    github_pr.number
                ))
            })?;
            pr.merge_commit = Some(merge_commit.clone());
            pr.ci_observation = None;
            match github_pr.merged_at.as_deref() {
                Some(value) => match time::OffsetDateTime::parse(value, &Rfc3339) {
                    Ok(value) => authoritative_merged_at = Some(value),
                    Err(error) => {
                        let reason = format!(
                            "GitHub returned malformed merged_at for pull request #{}: {error}",
                            github_pr.number
                        );
                        pr.github_observation = Some(GithubObservation {
                            checked_at: now,
                            result: GithubObservationResult::Partial { reason },
                        });
                    }
                },
                None => {
                    let reason = format!(
                        "GitHub returned no merged_at for merged pull request #{}",
                        github_pr.number
                    );
                    pr.github_observation = Some(GithubObservation {
                        checked_at: now,
                        result: GithubObservationResult::Partial { reason },
                    });
                }
            }
            Some(TaskEventKind::PrMerged {
                pr_id: pr.id.clone(),
                sequence: pr.sequence,
                number,
                url: url.clone(),
                merge_commit,
            })
        }
        "closed" => {
            // A PR the flow already abandoned settles again when GitHub's
            // `closed` is observed. Re-stamping the time makes the second
            // settle differ from the first and wedges the task on
            // "already settled differently" — the first abandonment is the
            // fact; observation only confirms it.
            pr.abandoned_at = pr.abandoned_at.or(Some(now));
            pr.ci_observation = None;
            None
        }
        _ => {
            // GitHub has it open, so any `abandoned_at` here is a stale claim that
            // it was closed. Clearing it returns the same row to `Open`.
            pr.abandoned_at = None;
            if let Some(ci_observation) = observe_required_checks(
                &task.worktree,
                github_pr.number,
                github_pr.head_sha.as_deref(),
                now,
            ) {
                pr.ci_observation = Some(ci_observation);
            }
            Some(TaskEventKind::PrOpened {
                pr_id: pr.id.clone(),
                sequence: pr.sequence,
                number,
                url: url.clone(),
            })
        }
    };

    let pr_changed = pr != previous;
    if pr_changed {
        pr.updated_at = now;
        if pr.phase() == PrPhase::Merged {
            let outcome = store
                .settle_task_pr_merged(&pr, authoritative_merged_at)
                .await
                .map_err(task_error)?;
            if let crate::store::TaskPrMergeEvidenceOutcome::Conflict { accepted_at } = outcome {
                let reason =
                    format!("GitHub merged_at conflicts with first accepted value {accepted_at}");
                pr.github_observation = Some(GithubObservation {
                    checked_at: now,
                    result: GithubObservationResult::Partial { reason },
                });
            }
        } else if pr.is_settled() {
            store.settle_task_pr(&pr, None).await.map_err(task_error)?;
        } else {
            store.update_task_pr(&pr).await.map_err(task_error)?;
        }
    }
    if pr_changed {
        if let Some(event) = pr_event {
            let should_append = match &event {
                TaskEventKind::PrOpened { .. } => previous_github.as_ref() != pr.github(),
                TaskEventKind::PrMerged { .. } => previous_phase != PrPhase::Merged,
                _ => true,
            };
            if should_append {
                store
                    .append_task_event(&task.id, &event)
                    .await
                    .map_err(task_error)?;
            }
        }
    }
    Ok(Some(pr))
}

/// The slug for the next serial PR: the operator's `--next` override, else the
/// settled PR's recorded `next_slug`, else the sequence number. One computation
/// shared by the recovery gate and the rotation.
fn next_pr_slug(settled: &TaskPr, slug_override: Option<&str>) -> String {
    slug_override
        .map(str::to_string)
        .or_else(|| settled.next_slug().map(str::to_string))
        .unwrap_or_else(|| (settled.sequence + 1).to_string())
}

/// The deterministic next serial branch for a settled Task PR — the same branch
/// `ensure_working_pr_with_options` would cut. The recovery gate reads this so
/// a partial rotation (worktree already on the next branch) is adopted, not
/// refused as an unrelated branch.
fn deterministic_next_branch(
    task: &Task,
    settled: &TaskPr,
    slug_override: Option<&str>,
) -> OpsResult<String> {
    let slug = next_pr_slug(settled, slug_override);
    let author = settled
        .branch
        .split_once('/')
        .map(|(author, _)| author)
        .ok_or_else(|| {
            task_error(format!(
                "Task PR branch {:?} has no author prefix",
                settled.branch
            ))
        })?;
    Ok(format!("{author}/{}-{slug}", task.workspace_slug))
}

pub(crate) async fn ensure_working_pr(
    store: &SharedStore,
    task: &mut Task,
) -> OpsResult<Option<TaskPr>> {
    ensure_working_pr_with_options(store, task, RotateOptions::runner()).await
}

/// How a serial-PR rotation treats the worktree. Automated settlement rotates
/// only a clean tree (`carry_dirty = false`); the operator's `lf pr next` carries the
/// preserved follow-up edits forward onto the next serial branch
/// (`carry_dirty = true`) and may name that branch via `slug_override`.
#[derive(Debug, Clone, Default)]
pub(crate) struct RotateOptions {
    carry_dirty: bool,
    slug_override: Option<String>,
}

impl RotateOptions {
    fn runner() -> Self {
        Self::default()
    }
}

enum CommittedFollowUp {
    ProvenEmpty,
    Range { from: String, to: String },
    Unprovable { reason: &'static str },
}

/// Classify the commits reachable from `branch` but not from `cut`. The cut is
/// the boundary past which commits are work this classification is asked about;
/// each caller picks it. A cut that cannot be placed on the branch is
/// `Unprovable` rather than empty: `is_ancestor` maps every nonzero exit to
/// false, so a rewritten branch and a missing object arrive here identically and
/// neither proves there is nothing there.
fn commits_past(
    worktree: &Path,
    branch: &str,
    cut: &str,
    not_ancestor: &'static str,
) -> OpsResult<CommittedFollowUp> {
    let tip = rev_parse(worktree, branch)
        .map_err(|error| task_error(format!("failed to resolve settled branch tip: {error}")))?;
    if tip == cut {
        return Ok(CommittedFollowUp::ProvenEmpty);
    }
    let ancestor = is_ancestor(worktree, cut, branch)
        .map_err(|error| task_error(format!("failed to check follow-up ancestry: {error}")))?;
    if !ancestor {
        return Ok(CommittedFollowUp::Unprovable {
            reason: not_ancestor,
        });
    }
    Ok(CommittedFollowUp::Range {
        from: cut.to_string(),
        to: branch.to_string(),
    })
}

/// Classify follow-up work committed on the settled branch *after* its PR
/// merged. The merged branch tip is `head_sha` — recorded by reconcile from
/// GitHub's `headRefOid`; commits reachable from the branch but not from
/// `head_sha` are the post-merge follow-up. A missing or unrelated recorded tip
/// cannot prove the range empty: rotation still skips an unsafe carry, while
/// completion fails closed until the boundary becomes provable.
fn committed_follow_up_range(worktree: &Path, settled: &TaskPr) -> OpsResult<CommittedFollowUp> {
    let Some(head_sha) = settled.github().and_then(|github| github.head_sha.clone()) else {
        return Ok(CommittedFollowUp::Unprovable {
            reason: "the published pull request head is missing",
        });
    };
    commits_past(
        worktree,
        &settled.branch,
        &head_sha,
        "the published pull request head is not an ancestor of the settled branch",
    )
}

/// Classify the authored work an unpublished PR holds. The cut is the fork point
/// recorded when the PR was minted, so commits past it are this PR's own work and
/// `ProvenEmpty` means the branch never moved off its base. Same tri-state, same
/// ancestry rule as the merged cut above — only the boundary differs.
fn unpublished_work(worktree: &Path, pr: &TaskPr) -> OpsResult<CommittedFollowUp> {
    commits_past(
        worktree,
        &pr.branch,
        &pr.base_commit,
        "the recorded base is not an ancestor of the unpublished branch",
    )
}

/// The commit `branch` forks from — the one authority for `base_commit`, and the
/// same expression `verify_task_pr_range_in` asserts before every
/// publish. A merge-base is always an ancestor of both inputs, so a base recorded
/// here can never read `Unprovable` for incoherence.
fn fork_point(worktree: &Path, base_ref: &str, branch: &str) -> OpsResult<String> {
    merge_base(worktree, base_ref, branch).map_err(|error| {
        task_error(format!(
            "{branch:?} shares no history with {base_ref}: {error}"
        ))
    })
}

/// Re-derive a `base_commit` an older mint left incoherent with its branch, which
/// wedged completion on `Unprovable` forever (W2-300). The mint can no longer
/// write such a row; this frees the ones it already did. Fail-soft throughout: any
/// failure leaves the row for the gate to refuse, so this heals the data the gate
/// reads and never relaxes the gate.
///
/// Scoped to the legacy mint's exact signature, `M <= B <= upstream`: it sourced
/// `B` from the upstream line, so a base it wrote is always a commit the upstream
/// carries. Merely "the fork point is an ancestor of `B`" is too weak — a sibling
/// or foreign base satisfies that too, and is contamination rather than a stale
/// mint. Those stay `Unprovable`, which is the fail-closed answer.
async fn heal_incoherent_base(store: &SharedStore, task: &Task, pr: TaskPr) -> OpsResult<TaskPr> {
    if pr.phase() != PrPhase::Working || !task.worktree.exists() {
        return Ok(pr);
    }
    // Local, so a coherent row costs no fetch.
    if is_ancestor(&task.worktree, &pr.base_commit, &pr.branch).unwrap_or(false) {
        return Ok(pr);
    }
    let Ok(default_branch) = get_default_branch(&task.worktree) else {
        return Ok(pr);
    };
    let Ok((base_ref, _)) = resolve_upstream_base(&task.worktree, &default_branch) else {
        return Ok(pr);
    };
    let Ok(fork) = fork_point(&task.worktree, &base_ref, &pr.branch) else {
        tracing::warn!(
            task = %task.plan.identifier,
            branch = %pr.branch,
            base = %pr.base_commit,
            "Task PR base is incoherent and shares no history with the upstream; \
             leaving the row for the completion gate to refuse"
        );
        return Ok(pr);
    };
    let ancestry = |commit: &str, descendant: &str| {
        is_ancestor(&task.worktree, commit, descendant).unwrap_or(false)
    };
    if !(ancestry(&fork, &pr.base_commit) && ancestry(&pr.base_commit, &base_ref)) {
        tracing::warn!(
            task = %task.plan.identifier,
            branch = %pr.branch,
            base = %pr.base_commit,
            "Task PR base is incoherent but is not on the upstream line, so no past mint \
             wrote it; leaving the row for the completion gate to refuse"
        );
        return Ok(pr);
    }
    let mut healed = pr;
    tracing::info!(
        task = %task.plan.identifier,
        branch = %healed.branch,
        from = %healed.base_commit,
        to = %fork,
        "healing a Task PR base that is not an ancestor of its branch"
    );
    healed.base_commit = fork;
    healed.updated_at = time::OffsetDateTime::now_utc();
    store
        .heal_task_pr_base(&healed)
        .await
        .map_err(|error| task_error(format!("failed to heal Task PR base: {error}")))?;
    Ok(healed)
}

pub(crate) fn no_active_pr_resume_refusal(
    identifier: &str,
    active: Option<&TaskPr>,
    latest: Option<&TaskPr>,
) -> Option<String> {
    if active.is_some() {
        return None;
    }
    let suffix = match latest {
        Some(pr) => {
            let which = pr
                .github()
                .map(|github| format!("pull request #{}", github.number))
                .unwrap_or_else(|| format!("PR sequence {}", pr.sequence));
            format!("{which} {}", pr.phase().as_str())
        }
        None => "no PR history recorded".to_string(),
    };
    Some(format!("Task {identifier} has no active PR; {suffix}"))
}

fn roll_back_failed_rotation(
    worktree: &Path,
    settled_branch: &str,
    recovery_branch: &str,
    stashed: bool,
) -> OpsResult<()> {
    checkout(worktree, settled_branch)
        .map_err(|error| task_error(format!("failed to restore settled branch: {error}")))?;
    delete_local_branch(worktree, recovery_branch)
        .map_err(|error| task_error(format!("failed to remove recovery branch: {error}")))?;
    if stashed {
        stash_pop(worktree)
            .map_err(|error| task_error(format!("failed to restore follow-up edits: {error}")))?;
    }
    Ok(())
}

async fn ensure_working_pr_with_options(
    store: &SharedStore,
    task: &mut Task,
    rotate: RotateOptions,
) -> OpsResult<Option<TaskPr>> {
    reconcile_task_pr_observation(store, task, crate::ops::pr::PrReadFreshness::Cached).await?;
    if matches!(
        task_work_status(store, task).await?,
        WorkStatus::Done | WorkStatus::Abandoned
    ) {
        return Ok(None);
    }
    if let Some(active) = store
        .active_task_pr(&task.id)
        .await
        .map_err(|error| task_error(format!("failed to read active PR: {error}")))?
    {
        return Ok(Some(heal_incoherent_base(store, task, active).await?));
    }

    let prs = store
        .task_prs(&task.id)
        .await
        .map_err(|error| task_error(format!("failed to read Task PRs: {error}")))?;
    let settled = prs
        .last()
        .cloned()
        .ok_or_else(|| task_error("Task has no PR history"))?;
    if !settled.is_settled() {
        return Err(task_error(format!(
            "Task PR {} is neither active nor settled",
            settled.id
        )));
    }
    // Rotating past an abandoned predecessor needs GitHub to have confirmed it
    // closed; the reconcile above read this row, so its verdict is already in
    // `task.observation`. A degraded read leaves the claim unverified, and a
    // successor minted on it strands an empty branch under a still-open PR.
    if let (PrPhase::Abandoned, Some(github)) = (settled.phase(), settled.github()) {
        if let Observation::Degraded { reason, .. } = &task.observation {
            return Err(task_error(format!(
                "cannot confirm pull request #{} is closed before starting the next PR: {reason}. \
                 Retry once GitHub is readable; if the PR was reopened, it continues as-is.",
                github.number
            )));
        }
    }
    let committed_carry = committed_follow_up_range(&task.worktree, &settled)?;
    // A settled completing PR normally never rotates. Two things independently
    // authorize one more serial PR: follow-up committed past the merged tip,
    // which the completion gate refuses to settle over, and a pending
    // directive, which the successor exists to incorporate.
    if settled.after_merge() == AfterMerge::CompleteTask
        && !matches!(&committed_carry, CommittedFollowUp::Range { .. })
    {
        return Ok(None);
    }
    let sequence = settled.sequence + 1;
    let slug = next_pr_slug(&settled, rotate.slug_override.as_deref());
    let branch = deterministic_next_branch(task, &settled, rotate.slug_override.as_deref())?;
    let default_branch = get_default_branch(&task.worktree)
        .map_err(|error| task_error(format!("failed to resolve default branch: {error}")))?;
    // `base_ref` positions the branch below; the recorded `base_commit` is read
    // from the branch itself once it is positioned, never from a parallel read of
    // the upstream — see `fork_point`.
    let (base_ref, _) = resolve_upstream_base(&task.worktree, &default_branch)?;
    if !rotate.carry_dirty
        && !is_clean(&task.worktree)
            .map_err(|error| task_error(format!("failed to inspect Task worktree: {error}")))?
    {
        return Err(task_error(format!(
            "Task {} cannot rotate PRs while {} has uncommitted changes",
            task.plan.identifier,
            task.worktree.display()
        )));
    }
    // The merged branch tip GitHub recorded (`head_sha`) is the cut between
    // already-merged work and the follow-up the worker committed on top after the
    // merge. Rotation carries that committed range forward — plus any dirty edits
    // — so no work is dropped when moving onto the next serial branch.
    let current = current_branch(&task.worktree)
        .map_err(|error| task_error(format!("failed to inspect Task branch: {error}")))?
        .ok_or_else(|| task_error("Task worktree is detached"))?;
    if current != branch {
        if current != settled.branch {
            return Err(task_error(format!(
                "Task {} expected settled branch {:?} or recovery branch {:?}, but {} is on {:?}",
                task.plan.identifier,
                settled.branch,
                branch,
                task.worktree.display(),
                current
            )));
        }
        let local_ref = format!("refs/heads/{branch}");
        let remote_ref = format!("refs/remotes/origin/{branch}");
        let collision = ref_exists(&task.worktree, &local_ref)
            .map_err(|error| task_error(format!("failed to inspect branch collision: {error}")))?
            || ref_exists(&task.worktree, &remote_ref).map_err(|error| {
                task_error(format!("failed to inspect branch collision: {error}"))
            })?;
        if collision {
            return Err(task_error(format!(
                "next PR branch {branch:?} already exists; retry the settling command with a clearer --next name"
            )));
        }
        // Stash dirty edits so the new branch starts clean: `checkout -b` then
        // carries nothing, the committed range cherry-picks onto a clean index,
        // and the stash pop reapplies the dirty edits on top.
        let stashed = stash_including_untracked(&task.worktree)
            .map_err(|error| task_error(format!("failed to stash follow-up edits: {error}")))?;
        if let Err(error) = checkout_new_branch_from(&task.worktree, &branch, &base_ref) {
            let recovered = current_branch(&task.worktree)
                .map_err(|read_error| {
                    task_error(format!("failed to inspect recovery branch: {read_error}"))
                })?
                .as_deref()
                == Some(branch.as_str());
            if !recovered {
                if stashed {
                    stash_pop(&task.worktree).map_err(|recovery_error| {
                        task_error(format!(
                            "failed to rotate Task worktree: {error}; restoring follow-up edits \
                             also failed: {recovery_error}"
                        ))
                    })?;
                }
                return Err(task_error(format!(
                    "failed to rotate Task worktree: {error}; follow-up edits were restored"
                )));
            }
        }
        if let CommittedFollowUp::Range { from, to } = &committed_carry {
            if let Err(error) = cherry_pick_range(&task.worktree, from, to) {
                roll_back_failed_rotation(&task.worktree, &settled.branch, &branch, stashed)
                    .map_err(|recovery_error| {
                        task_error(format!(
                        "failed to carry committed follow-up from {:?} onto {branch}: {error}; \
                         automatic recovery also failed: {recovery_error}",
                        settled.branch
                    ))
                    })?;
                return Err(task_error(format!(
                    "failed to carry committed follow-up from {:?} onto {branch}: {error}; \
                     restored {:?} with its follow-up edits so the rotation can be retried",
                    settled.branch, settled.branch
                )));
            }
        }
        if stashed {
            stash_pop(&task.worktree).map_err(|error| {
                task_error(format!(
                    "carried the committed follow-up but could not reapply dirty edits: {error}; \
                     the recovery branch and retained stash are in {} for conflict resolution",
                    task.worktree.display()
                ))
            })?;
        }
    }
    // The branch is now positioned — freshly cut at `base_ref`, or reused where a
    // partial rotation already left it. Record the base it actually forks from, so
    // the pair agrees by construction whichever of those two it was. Reading the
    // upstream tip here instead is what paired a fresh base with a stale branch
    // and left completion unable to prove the successor empty (W2-300).
    let base_commit = fork_point(&task.worktree, &base_ref, &branch)?;

    let _mutation = lock_task_pr_mutation(&task.worktree)?;
    push_with_upstream(&task.worktree, "origin", &branch, &|_| {})
        .map_err(|error| task_error(format!("failed to push next PR branch: {error}")))?;

    let now = time::OffsetDateTime::now_utc();
    let next = TaskPr {
        id: TaskPrId::new(),
        task_id: task.id.clone(),
        sequence,
        slug,
        branch,
        base_commit,
        parent_pr_id: None,
        publication: None,
        merge_commit: None,
        abandoned_at: None,
        ci_observation: None,
        github_observation: None,
        linear_attachment_id: None,
        linear_comment_id: None,
        linear_link_error: None,
        created_at: now,
        updated_at: now,
    };
    match store.settle_task_pr(&settled, Some(&next)).await {
        Ok(()) => {
            store
                .append_task_event(
                    &task.id,
                    &TaskEventKind::PrStarted {
                        pr_id: next.id.clone(),
                        sequence: next.sequence,
                        branch: next.branch.clone(),
                        base_commit: next.base_commit.clone(),
                    },
                )
                .await
                .map_err(task_error)?;
            Ok(Some(next))
        }
        Err(error) => {
            let recovered = store
                .task_prs(&task.id)
                .await
                .map_err(task_error)?
                .into_iter()
                .find(|pr| pr.sequence == sequence);
            match recovered {
                Some(pr)
                    if pr.branch == next.branch
                        && pr.base_commit == next.base_commit
                        && pr.phase() == PrPhase::Working =>
                {
                    Ok(Some(pr))
                }
                _ => Err(task_error(format!(
                    "failed to record next Task PR after branch rotation: {error}"
                ))),
            }
        }
    }
}

/// Advance a Task to its next serial PR after an out-of-band merge. Reconciles
/// the merge into the settled PR, then rotates the worktree to sequence N+1 —
/// carrying preserved follow-up edits forward — so a stopped worker (or an
/// operator) can push the next PR without manual git surgery. `slug` names the
/// next branch; otherwise the settled PR's `next_slug`, otherwise the sequence.
pub fn pr_next(repo: &Path, slug: Option<&str>) -> OpsResult<TaskPr> {
    let slug_override = slug.map(parse_pr_slug).transpose()?;
    let repo = repo.to_path_buf();
    block_on_task(async move {
        let store = task_store().await?;
        let mut task = task_for_checkout(&store, &repo)
            .await?
            .ok_or_else(|| task_error("no Task owns this worktree"))?;
        // Observe an out-of-band merge before deciding whether to rotate.
        reconcile_task_pr_observation(&store, &mut task, crate::ops::pr::PrReadFreshness::Cached)
            .await?;
        if let Some(active) = store
            .active_task_pr(&task.id)
            .await
            .map_err(|error| task_error(format!("failed to read active PR: {error}")))?
        {
            let which = active
                .github()
                .map(|github| format!("#{}", github.number))
                .unwrap_or_else(|| format!("sequence {}", active.sequence));
            return Err(task_error(format!(
                "current PR {which} is not merged yet; land it or wait for the merge before `lf pr next`"
            )));
        }
        if matches!(
            task_work_status(&store, &task).await?,
            WorkStatus::Done | WorkStatus::Abandoned
        ) {
            return Err(task_error(format!(
                "Task {} is terminal; nothing to rotate",
                task.plan.identifier
            )));
        }
        let rotate = RotateOptions {
            carry_dirty: true,
            slug_override,
        };
        ensure_working_pr_with_options(&store, &mut task, rotate)
            .await?
            .ok_or_else(|| task_error("Task has no settled PR to rotate from"))
    })
}

#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct TaskStatus {
    pub planning: Option<crate::store::PmTaskRecord>,
    pub planning_error: Option<String>,
    pub planning_stale: bool,
    pub planning_state: crate::store::PlanningState,
    pub execution: Option<TaskSnapshot>,
}

pub fn task_status(repo: &Path, issue: Option<&str>) -> OpsResult<TaskStatus> {
    let task = task_execution_status(repo, issue)?;
    let selector = task
        .as_ref()
        .map(|task| task.plan.id.as_str())
        .or(issue)
        .ok_or_else(|| task_error("this checkout has no Task"))?;
    let read = match crate::ops::pm::inspect_task_planning(
        repo,
        selector,
        crate::ops::pm::PmRefresh::Auto,
    ) {
        Ok(read) => read,
        Err(error) if task.is_some() => crate::ops::pm::TaskPlanningInspection {
            observation: crate::store::PmTaskObservation {
                record: None,
                state: crate::store::PlanningState::Unavailable,
            },
            refresh_error: Some(error.to_string()),
        },
        Err(error) => return Err(error),
    };
    let planning_stale = read.is_stale();
    let planning_state = read.observation.state;
    let planning = read.observation.record;
    let planning_error = read.refresh_error;
    let execution = task.as_ref().map(task_snapshot).transpose()?;
    Ok(TaskStatus {
        planning,
        planning_error,
        planning_stale,
        planning_state,
        execution,
    })
}

fn task_execution_status(repo: &Path, issue: Option<&str>) -> OpsResult<Option<Task>> {
    block_on_task(async move {
        let store = task_store().await?;
        let task = match issue {
            Some(issue) => store
                .get_task_by_issue(issue)
                .await
                .map_err(|error| task_error(format!("failed to read task status: {error}")))?,
            None => task_for_checkout(&store, repo).await?,
        };
        let Some(mut task) = task else {
            return Ok(None);
        };
        if store
            .task_deletion(&task.wave_id, task.plan.id.as_str())
            .await
            .map_err(task_error)?
            .is_some()
        {
            return match issue {
                Some(_) => Ok(Some(task)),
                None => Err(task_error("this checkout's Task was deleted; use an explicit Task identifier to read its history")),
            };
        }
        let launch_refusal = task_exec_refusal(&store, &task)
            .await
            .map_err(|error| task_error(format!("failed to read Task blocker: {error}")))?;
        if launch_refusal.is_none() && task_worktree_blocker(&store, &task).await?.is_none() {
            reconcile_task_pr(&store, &mut task).await?;
        }
        Ok(Some(task))
    })
}

/// Find a Task whose only active PR is the empty artifact of rotating past
/// already-merged work.
pub(crate) fn find_discardable_task_successor(repo: &Path) -> OpsResult<Option<String>> {
    let repo = repo.to_path_buf();
    block_on_task(async move {
        let ManagedTask::Managed { store, task } = resolve_managed_task(&repo).await? else {
            return Ok(None);
        };
        let materially_clean = is_materially_clean(&task.worktree)
            .map_err(|error| task_error(format!("failed to inspect Task worktree: {error}")))?;
        if !materially_clean {
            return Ok(None);
        }
        let gate = task_completion_gate(&store, &task).await?;
        if !gate.satisfied || gate.discardable_successor.is_none() {
            return Ok(None);
        }
        Ok(Some(task.plan.identifier.clone()))
    })
}

/// Complete planning-only work without allocating execution placement.
pub fn task_complete(repo: &Path, issue: &str, summary: String) -> OpsResult<Option<Task>> {
    let summary = summary.trim().to_string();
    if summary.is_empty() {
        return Err(task_error("completion summary cannot be empty"));
    }
    let registered = block_on_task(async {
        task_store()
            .await?
            .get_task_by_issue(issue)
            .await
            .map_err(task_error)
    })?;
    if registered.is_some() {
        return complete_task(issue, summary).map(Some);
    }
    block_on_task(super::pm::complete_planning_task(repo, issue, &summary))?;
    Ok(None)
}

fn complete_task(issue: &str, summary: String) -> OpsResult<Task> {
    block_on_task(async move {
        let store = task_store().await?;
        let mut task = store
            .get_task_by_issue(issue)
            .await
            .map_err(|error| task_error(format!("failed to read Task: {error}")))?
            .ok_or_else(|| task_error(format!("no Task exists for {issue:?}")))?;
        let work = store
            .work_for_child(&ChildRef::Task(task.id.clone()))
            .await
            .map_err(task_error)?;
        match store.work_status(&work).await.map_err(task_error)? {
            WorkStatus::Done => {
                reconcile_task_completion(&store, &mut task).await?;
                cleanup_completed_task(&store, &task).await?;
                return Ok(task);
            }
            WorkStatus::Abandoned => {
                return Err(task_error(format!(
                    "Task {} is abandoned and cannot be completed",
                    task.plan.identifier
                )))
            }
            WorkStatus::Ready => {}
        }
        reconcile_task_pr_observation(&store, &mut task, crate::ops::pr::PrReadFreshness::Cached)
            .await?;
        if !is_clean(&task.worktree)
            .map_err(|error| task_error(format!("failed to inspect Task worktree: {error}")))?
        {
            return Err(task_error(
                "Task worktree has uncommitted changes; publish or explicitly abandon them first",
            ));
        }
        // The completion gate requires every active PR to be settled. Do not
        // bypass that fact or infer merge from a green head.
        let gate = task_completion_gate(&store, &task).await?;
        if let Some(refusal) = gate.refusal(&task.plan.identifier) {
            // Nothing has been written. A refusal leaves a discardable
            // successor active, so the Task keeps its PR and no rotation is
            // provoked.
            return Err(task_error(refusal));
        }
        reconcile_pm_writeback(&store, &mut task, None).await?;
        store
            .append_task_event(&task.id, &TaskEventKind::Progress { summary })
            .await
            .map_err(task_error)?;
        // Every other condition is now proven, so the rotation's empty artifact
        // is retired in the completion transaction. Retaining its branch identity
        // lets cleanup retry after a crash without guessing what it may delete.
        store
            .complete_task(&task, gate.discardable_successor.as_ref())
            .await
            .map_err(|error| task_error(format!("failed to complete Task: {error}")))?;
        cleanup_completed_task(&store, &task).await?;
        Ok(task)
    })
}

/// The concise publication-state label carried by a PR's Linear linkage. Derived
/// purely from the PR model — its phase and after-merge disposition — so the label
/// is a projection of the source of truth, not a second state.
fn pr_link_state_label(pr: &TaskPr) -> String {
    match pr.phase() {
        PrPhase::Merged => "Merged".to_string(),
        PrPhase::Abandoned => "Abandoned".to_string(),
        _ => {
            let completes = pr.after_merge() == AfterMerge::CompleteTask;
            if completes {
                "Open · completes task on merge".to_string()
            } else if let Some(request) = pr.merge_request() {
                match request.mode {
                    PrMergeMode::User => "Open · user merge requested".to_string(),
                    PrMergeMode::Auto => "Open · auto-merge requested".to_string(),
                }
            } else {
                "Open · published".to_string()
            }
        }
    }
}

/// Idempotently refresh the PR's Linear linkage (attachment + managed comment) and
/// record the outcome on the PR. Best-effort: a degraded writeback lands in
/// `linear_link_error` and leaves the GitHub result intact; the next publication
/// command retries. Does nothing for a PR with no GitHub URL yet.
async fn link_pr_to_linear(store: &SharedStore, task: &Task, pr: &mut TaskPr) {
    let Some(github) = pr.github().cloned() else {
        return;
    };
    let state = pr_link_state_label(pr);
    let title = format!("GitHub PR #{}", github.number);
    let body = format!("[GitHub PR #{}]({}) — {}", github.number, github.url, state);
    let wave = match owning_wave(store, task).await {
        Ok(wave) => wave,
        Err(error) => {
            pr.linear_link_error = Some(error.to_string());
            return;
        }
    };
    let prior = crate::ops::pm::PrLinkageIds {
        attachment_id: pr.linear_attachment_id.clone(),
        comment_id: pr.linear_comment_id.clone(),
    };
    let request = crate::ops::pm::PrLinkRequest {
        issue_id: task.plan.id.as_str().to_string(),
        url: github.url.clone(),
        title,
        subtitle: state,
        body,
    };
    let outcome =
        crate::ops::pm::pm_link_pr_async(&task.worktree, wave.slug(), &request, &prior).await;
    // Say so at publish time. The PR line in `lf task status` carries the durable
    // reading, but an operator running `lf pr open` should not have to go looking.
    if let Some(error) = &outcome.error {
        tracing::warn!(
            issue = task.plan.identifier,
            pr = github.number,
            "Linear link degraded; the GitHub PR is published and the next publish retries: {error}"
        );
    }
    pr.linear_attachment_id = outcome.ids.attachment_id;
    pr.linear_comment_id = outcome.ids.comment_id;
    pr.linear_link_error = outcome.error;
}

fn writeback_state(result: OpsResult<()>) -> PmWritebackState {
    match result {
        Ok(()) => PmWritebackState::Current,
        Err(error) => PmWritebackState::Pending {
            operation: PmWritebackOperation::CompleteTask,
            error: error.to_string(),
        },
    }
}

async fn reconcile_pm_writeback(
    store: &SharedStore,
    task: &mut Task,
    pr_url: Option<&str>,
) -> OpsResult<()> {
    let result = async {
        let wave = owning_wave(store, task).await?;
        crate::ops::task_pm::complete_task(
            Path::new(wave.repo()),
            wave.slug(),
            task.plan.id.as_str(),
            pr_url,
        )
        .await
    }
    .await;
    // A known conflicting outcome refuses new success. Transport and refresh
    // failures retain the existing pending writeback contract.
    if let Err(error @ OpsError::TaskCompletionConflict { .. }) = result {
        task.pm_writeback = PmWritebackState::Pending {
            operation: PmWritebackOperation::CompleteTask,
            error: error.to_string(),
        };
        return Err(error);
    }
    task.pm_writeback = writeback_state(result);
    if let Some(refreshed) = store.get_task(&task.id).await.map_err(task_error)? {
        task.plan = refreshed.plan;
    }
    Ok(())
}

async fn retry_pm_writeback(store: &SharedStore, task: &mut Task) -> OpsResult<()> {
    let prs = store.task_prs(&task.id).await.map_err(task_error)?;
    let pr_url = prs
        .iter()
        .rev()
        .find_map(|pr| pr.github().map(|github| github.url.as_str()));
    // Already-Done history remains Done even if Linear later conflicts.
    let result = reconcile_pm_writeback(store, task, pr_url).await;
    task.updated_at = time::OffsetDateTime::now_utc();
    match result {
        Err(OpsError::TaskCompletionConflict { .. }) => Ok(()),
        result => result,
    }
}

// ---------------------------------------------------------------------------
// Completion gate: the single source of truth for "may this Task be completed
// in the PM yet?" A Task is completable only when every active PR is settled
// (merged or explicitly abandoned). Every path that sets a Task to `Completed` and
// fires the `CompleteTask` PM writeback consults this gate, so the PM row, the
// durable Task, PR state, and Work flow converge monotonically.
// ---------------------------------------------------------------------------

/// The outcome of evaluating the completion gate.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct CompletionGate {
    pub satisfied: bool,
    pub blockers: Vec<String>,
    /// A successor the lifecycle rotated after the Task's work merged that
    /// provably holds nothing — never published, branch never moved off its
    /// recorded base. It is the rotation's artifact, not work, so it does not
    /// block completion; it must not outlive one either.
    ///
    /// Classification only, and exactly one thing acts on it: [`complete_task`]
    /// passes it as the completion transaction's `skipped_pr`, which retires the
    /// row and writes the terminal status together. Discarding it any earlier
    /// would leave a non-terminal Task with no active PR — the state
    /// [`ensure_working_pr_with_options`] rotates another empty PR from.
    pub discardable_successor: Option<TaskPr>,
}

impl CompletionGate {
    /// One actionable, human-readable sentence. Empty when the gate is
    /// satisfied.
    pub fn reason(&self) -> String {
        if self.blockers.is_empty() {
            String::new()
        } else {
            self.blockers.join("; ")
        }
    }

    pub(crate) fn refusal(&self, identifier: &str) -> Option<String> {
        (!self.satisfied).then(|| {
            format!(
                "Task {identifier} cannot complete until its gates close: {}",
                self.reason()
            )
        })
    }
}

/// Evaluate the completion gate against the Task's durable PR state. Pure over
/// store state: running it twice changes nothing.
pub(crate) async fn task_completion_gate(
    store: &SharedStore,
    task: &Task,
) -> OpsResult<CompletionGate> {
    let mut gate = CompletionGate {
        satisfied: true,
        blockers: Vec::new(),
        discardable_successor: None,
    };
    let work_done = task_work_status(store, task).await? == WorkStatus::Done;
    if work_done && !task.worktree.exists() {
        return Ok(gate);
    }
    if let Some(blocker) = task_worktree_blocker(store, task).await? {
        gate.satisfied = false;
        gate.blockers.push(blocker.reason);
        return Ok(gate);
    }

    gate.blockers
        .extend(lifecycle::associated_work_blockers(store, task)?);

    // Work committed past the tip GitHub merged is owned by no PR; completing
    // would strand it outside the Task. Only the newest PR can still hold it: a
    // rotation carries the range onto its successor but leaves the settled
    // branch's commits in place, so scanning every merged PR would never clear.
    let prs = store
        .task_prs(&task.id)
        .await
        .map_err(|error| task_error(format!("failed to read Task PRs: {error}")))?;
    // Discarding a successor is only ever settling *over* landed work. Without a
    // merged predecessor there is nothing to settle, and an empty unpublished PR
    // keeps today's refusal.
    let has_merged_predecessor = prs.iter().any(|pr| pr.phase() == PrPhase::Merged);
    if let Some(newest) = prs.last() {
        if newest.phase() == PrPhase::Merged && newest.after_merge() == AfterMerge::CompleteTask {
            let number = newest
                .github()
                .map(|github| github.number)
                .unwrap_or_default();
            match committed_follow_up_range(&task.worktree, newest)? {
                CommittedFollowUp::ProvenEmpty => {}
                CommittedFollowUp::Range { .. } => gate.blockers.push(format!(
                    "follow-up work is committed past merged pull request #{number}"
                )),
                // Missing later evidence blocks entry into completion, but it
                // cannot reverse a terminal fact. Repair still reopens on a
                // proven range or any other concrete gate blocker.
                CommittedFollowUp::Unprovable { .. } if work_done => {}
                CommittedFollowUp::Unprovable { reason } => gate.blockers.push(format!(
                    "cannot prove merged pull request #{number} has no committed follow-up: {reason}"
                )),
            }
        }
    }

    // Every active PR must be settled (merged or explicitly abandoned).
    if let Some(pr) = store
        .active_task_pr(&task.id)
        .await
        .map_err(|error| task_error(format!("failed to read active PR: {error}")))?
    {
        let which = pr
            .github()
            .map(|github| format!("#{}", github.number))
            .unwrap_or_else(|| format!("sequence {}", pr.sequence));
        match pr.phase() {
            PrPhase::Open => gate.blockers.push(format!(
                "pull request {which} is open; merge it or run `lf pr abandon`"
            )),
            PrPhase::Publishing => gate.blockers.push(format!(
                "pull request {which} is still publishing; wait for it to land or run `lf pr abandon`"
            )),
            // An unpublished PR means three different things; say which. The
            // classification is inert: a gate that goes on to refuse leaves the
            // row exactly as it found it.
            PrPhase::Working => match unpublished_work(&task.worktree, &pr)? {
                CommittedFollowUp::ProvenEmpty if has_merged_predecessor => {
                    gate.discardable_successor = Some(pr.clone());
                }
                CommittedFollowUp::ProvenEmpty => gate.blockers.push(format!(
                    "pull request {which} is unpublished; publish and merge it or run `lf pr abandon`"
                )),
                CommittedFollowUp::Range { .. } => gate.blockers.push(format!(
                    "follow-up work is committed on unpublished pull request {which}; \
                     publish and merge it or run `lf pr abandon`"
                )),
                CommittedFollowUp::Unprovable { reason } => gate.blockers.push(format!(
                    "cannot prove unpublished pull request {which} is empty: {reason}"
                )),
            },
            PrPhase::Merged | PrPhase::Abandoned => {}
        }
    }

    gate.satisfied = gate.blockers.is_empty();
    Ok(gate)
}

/// True when the Task has a settled merged PR whose `after_merge` is
/// `CompleteTask` — i.e. completion is pending on the gate, not on a future PR.
async fn merged_completing_pr(store: &SharedStore, task: &Task) -> OpsResult<Option<TaskPr>> {
    let prs = store
        .task_prs(&task.id)
        .await
        .map_err(|error| task_error(format!("failed to read Task PRs: {error}")))?;
    Ok(prs
        .into_iter()
        .find(|pr| pr.phase() == PrPhase::Merged && pr.after_merge() == AfterMerge::CompleteTask))
}

pub(crate) async fn reconcile_task_completion(
    store: &SharedStore,
    task: &mut Task,
) -> OpsResult<()> {
    match task_work_status(store, task).await? {
        WorkStatus::Done => {
            if matches!(task.pm_writeback, PmWritebackState::Pending { .. }) {
                retry_pm_writeback(store, task).await?;
                store
                    .update_task_pm_writeback(&task.id, &task.pm_writeback, task.updated_at)
                    .await
                    .map_err(task_error)?;
            }
            return Ok(());
        }
        WorkStatus::Abandoned => return Ok(()),
        WorkStatus::Ready => {}
    }
    let Some(pr) = merged_completing_pr(store, task).await? else {
        return Ok(());
    };
    let gate = task_completion_gate(store, task).await?;
    // Automatic completion leaves legacy empty successors for explicit
    // `lf task complete`; current CompleteTask merges do not rotate.
    if !gate.satisfied || gate.discardable_successor.is_some() {
        return Ok(());
    }
    let url = pr.github().map(|github| github.url.as_str());
    reconcile_pm_writeback(store, task, url).await?;
    // PR reconciliation already persisted the merge. Completion writes only
    // Task outcome and writeback facts, never a stale copy of the settled PR.
    store.complete_task(task, None).await.map_err(task_error)
}

pub fn task_snapshot(task: &Task) -> OpsResult<TaskSnapshot> {
    let task = task.clone();
    block_on_task(async move {
        let store = task_store().await?;
        let wave = owning_wave(&store, &task).await?;
        let project = store
            .get_project(&task.project_id)
            .await
            .map_err(|error| task_error(format!("failed to read owning Project: {error}")))?
            .ok_or_else(|| task_error(format!("owning Project {} is missing", task.project_id)))?;
        let work = store
            .work_for_child(&ChildRef::Task(task.id.clone()))
            .await
            .map_err(|error| task_error(format!("failed to resolve Task Work: {error}")))?;
        let latest_event = store
            .task_events_after(&task.id, 0)
            .await
            .map_err(|error| task_error(format!("failed to read task events: {error}")))?
            .into_iter()
            .last();
        let prs = store
            .task_prs(&task.id)
            .await
            .map_err(|error| task_error(format!("failed to read Task PRs: {error}")))?;
        let latest = prs.last();
        let active = prs.iter().find(|pr| pr.is_active());
        let active_pr = active.map(|pr| pr.id.clone());
        let work_status = store
            .work_status(&work)
            .await
            .map_err(|error| task_error(format!("failed to derive Task Work status: {error}")))?;
        let execution = crate::ops::task_execution::task_execution(&store, &task.id)
            .await
            .map_err(task_error)?;
        let work_set = store.sqlite.task_work(&task.id).map_err(task_error)?;
        let actions = if store
            .task_deletion(&task.wave_id, task.plan.id.as_str())
            .await
            .map_err(task_error)?
            .is_some()
        {
            TaskActionModel {
                recommended: Some(crate::ops::task_actions::TaskAction::NoAction),
                reason: "Task was deleted; retained history is read-only".into(),
            }
        } else {
            let predecessor_phase = match active.and_then(|pr| pr.parent_pr_id.as_ref()) {
                Some(parent_id) => store
                    .get_task_pr(parent_id)
                    .await
                    .map_err(|error| task_error(format!("failed to read parent PR: {error}")))?
                    .map(|pr| pr.phase()),
                None => None,
            };
            let completion_gate = task_completion_gate(&store, &task).await?;
            let completion_refusal = completion_gate.refusal(&task.plan.identifier);
            let worktree_blocker = task_worktree_blocker(&store, &task).await?;
            let resume_refusal = worktree_blocker
                .as_ref()
                .map(|blocker| blocker.reason.clone())
                .or_else(|| no_active_pr_resume_refusal(&task.plan.identifier, active, latest));
            let launch_refusal = if worktree_blocker.is_some() {
                None
            } else {
                task_configuration_refusal(&task, None)
                    .or_else(|| task_event_exec_refusal(latest_event.as_ref()).map(str::to_string))
            };
            let action_evidence = TaskActionEvidence {
                status: work_status.clone(),
                execution: Some(&execution),
                latest_pr_phase: latest.map(|pr| pr.phase()),
                latest_pr_after_merge: latest
                    .filter(|pr| pr.phase() == PrPhase::Merged)
                    .map(TaskPr::after_merge),
                latest_pr_merge_request: latest.and_then(TaskPr::merge_request),
                latest_pr_presentation_current: latest
                    .filter(|pr| pr.phase() == PrPhase::Open)
                    .map(|pr| pr.presentation().is_some()),
                completion_refusal: completion_refusal.as_deref(),
                resume_refusal: resume_refusal.as_deref(),
                ci: active.and_then(|pr| pr.fresh_ci()),
                predecessor_phase,
                abandon_intent: task.abandon_intent.is_some(),
                launch_refusal: launch_refusal.as_deref(),
            };
            derive_task_actions(&action_evidence)
        };
        let agent = resolve_task_agent(&task.worktree, task.agent.as_deref(), None);
        let (provider, _) = parse_agent(&agent);
        let home_id = store
            .task_checkouts()
            .await
            .map_err(task_error)?
            .into_iter()
            .find(|row| row.task_id == task.id)
            .and_then(|row| row.home_id);
        let local_home = store.local_home().await.map_err(task_error)?;
        let worktree = if home_id.as_ref() == Some(&local_home.id) {
            crate::engine::git::worktree_root(&task.worktree)
                .ok()
                .and_then(|root| root.canonicalize().ok())
                .unwrap_or_else(|| task.worktree.clone())
        } else {
            task.worktree.clone()
        };
        Ok(TaskSnapshot {
            home_id,
            issue_id: task.plan.id.as_str().to_string(),
            issue_identifier: task.plan.identifier,
            task_id: task.id.to_string(),
            external_project_id: project.plan.id.as_str().to_string(),
            project: project.plan.slug,
            pm_snapshot_synced_at: task.plan.pm_snapshot_synced_at,
            pm_writeback: task.pm_writeback,
            wave: wave.slug().to_string(),
            project_id: task.project_id.to_string(),
            status: work_status,
            execution,
            work: work_set,
            worktree: worktree.display().to_string(),
            workspace_slug: task.workspace_slug,
            agent: task.agent,
            provider,
            prs,
            active_pr,
            latest_event,
            created_at: task.created_at,
            updated_at: task.updated_at,
            observation: task.observation,
            actions,
        })
    })
}

/// Maximum byte length for Task file content, drafts and returned patches.
pub const MAX_FILE_BYTES: usize = 1_000_000;

pub fn task_changes(issue: &str, base: &str) -> OpsResult<TaskChangesSnapshot> {
    let (task, pr) = comparison_context(issue)?;
    let workspace = TaskComparison::new(&task, &pr);
    let base = resolve_file_base(workspace, base)?;
    changes_snapshot(TaskComparison {
        base_commit: &base,
        ..workspace
    })
}

fn changes_snapshot(workspace: TaskComparison<'_>) -> OpsResult<TaskChangesSnapshot> {
    let mut files = BTreeMap::<String, TaskChangedFile>::new();
    record_changed_paths(
        workspace.checkout.worktree,
        &[
            "diff",
            "--name-only",
            "-z",
            &format!("{}..HEAD", workspace.base_commit),
        ],
        &mut files,
        |file| file.committed = true,
    )?;
    record_changed_paths(
        workspace.checkout.worktree,
        &["diff", "--cached", "--name-only", "-z"],
        &mut files,
        |file| file.staged = true,
    )?;
    record_changed_paths(
        workspace.checkout.worktree,
        &["diff", "--name-only", "-z"],
        &mut files,
        |file| file.unstaged = true,
    )?;
    record_changed_paths(
        workspace.checkout.worktree,
        &["ls-files", "--others", "--exclude-standard", "-z"],
        &mut files,
        |file| file.untracked = true,
    )?;
    let head_commit = git_output(workspace.checkout.worktree, &["rev-parse", "HEAD"])?
        .trim()
        .to_string();
    let net = net_changed_paths(workspace)?;
    files.retain(|path, file| {
        file.old_path = net.get(path).cloned().flatten();
        net.contains_key(path) || file.untracked
    });
    let (scratch, scratch_truncated) = scratch_paths(workspace.checkout.worktree)?;
    let recovery = file_save::recovery_directory(workspace.checkout)?;
    Ok(TaskChangesSnapshot {
        recovery_directory: recovery
            .exists()
            .then(|| recovery.to_string_lossy().into_owned()),
        issue_identifier: workspace.checkout.issue_identifier.to_string(),
        task_id: workspace.checkout.task_id.to_string(),
        base_commit: workspace.base_commit.to_string(),
        head_commit,
        files: files.into_values().collect(),
        scratch,
        scratch_truncated,
    })
}

pub fn task_diff(
    issue: &str,
    path: Option<&str>,
    base: &str,
    draft: Option<&str>,
) -> OpsResult<TaskDiffSnapshot> {
    let (task, pr) = comparison_context(issue)?;
    let workspace = TaskComparison::new(&task, &pr);
    let base = resolve_file_base(workspace, base)?;
    let workspace = TaskComparison {
        base_commit: &base,
        ..workspace
    };
    match draft {
        Some(draft) => draft_snapshot(
            workspace,
            path.ok_or_else(|| task_error("Draft diff requires a path"))?,
            draft,
        ),
        None => diff_snapshot(workspace, path),
    }
}

fn diff_snapshot(workspace: TaskComparison<'_>, path: Option<&str>) -> OpsResult<TaskDiffSnapshot> {
    let relative = path.map(validate_task_relative_path).transpose()?;
    let old_path = match &relative {
        Some(path) => net_changed_paths(workspace)?.remove(path).flatten(),
        None => None,
    };
    let mut args = vec![
        "diff",
        "--no-ext-diff",
        "--no-color",
        workspace.base_commit,
        "--",
    ];
    // Both sides of a rename are needed for Git to preserve its identity.
    if let Some(old) = &old_path {
        args.push(old);
    }
    if let Some(path) = &relative {
        args.push(path);
    }
    let mut patch = git_output_bytes(workspace.checkout.worktree, &args)?;
    let untracked = untracked_paths(workspace.checkout.worktree)?;
    let include_untracked = untracked
        .into_iter()
        .filter(|candidate| relative.as_ref().is_none_or(|path| path == candidate));
    for path in include_untracked {
        let output = Command::new("git")
            .current_dir(workspace.checkout.worktree)
            .args(["diff", "--no-index", "--no-color", "--", "/dev/null", &path])
            .output()
            .map_err(|error| {
                task_error(format!("failed to diff untracked file {path}: {error}"))
            })?;
        if !output.status.success() && output.status.code() != Some(1) {
            return Err(task_error(format!(
                "failed to diff untracked file {path}: {}",
                String::from_utf8_lossy(&output.stderr).trim()
            )));
        }
        patch.extend_from_slice(&output.stdout);
    }
    Ok(patch_snapshot(workspace, relative, &patch))
}

pub(crate) fn task_workspace_context(task: &Task, pr: &TaskPr) -> OpsResult<String> {
    const MAX_PATCH_TOKENS: usize = 15_000;

    #[derive(serde::Serialize)]
    struct ContextFile<'a> {
        path: &'a str,
        committed: bool,
        staged: bool,
        unstaged: bool,
        untracked: bool,
        size_bytes: Option<u64>,
        content_sha256: Option<String>,
    }

    let workspace = TaskComparison::new(task, pr);
    let changes = changes_snapshot(workspace)?;
    let diff = diff_snapshot(workspace, None)?;
    let files = changes
        .files
        .iter()
        .map(|file| {
            let bytes = std::fs::read(task.worktree.join(&file.path)).ok();
            ContextFile {
                path: &file.path,
                committed: file.committed,
                staged: file.staged,
                unstaged: file.unstaged,
                untracked: file.untracked,
                size_bytes: bytes.as_ref().map(|bytes| bytes.len() as u64),
                content_sha256: bytes
                    .as_ref()
                    .map(|bytes| hex::encode(Sha256::digest(bytes))),
            }
        })
        .collect::<Vec<_>>();
    let files = serde_json::to_string_pretty(&files)
        .map_err(|error| task_error(format!("failed to encode Task changes: {error}")))?;
    let include_patch = !diff.binary
        && !diff.truncated
        && crate::engine::prompt::count_tokens(&diff.patch) < MAX_PATCH_TOKENS;
    let patch = if include_patch {
        diff.patch.as_str()
    } else {
        "Patch omitted from prompt because it is binary, truncated, or exceeds 15,000 tokens. Read the named worktree paths for exact bytes."
    };
    Ok(format!(
        "<lf:task-workspace>\nActive PR base: {}\nCurrent HEAD: {}\nChanges across the active PR base, index, worktree, and untracked files:\n{files}\n\nPatch (included={include_patch}, binary={}, truncated={}):\n{patch}\n</lf:task-workspace>",
        changes.base_commit, changes.head_commit, diff.binary, diff.truncated
    ))
}

pub fn task_file(issue: &str, path: &str, inspect_recovery: bool) -> OpsResult<TaskFileSnapshot> {
    let checkout = file_context(issue)?;
    let workspace = TaskWorkspace::from(&checkout);
    let mut file = file_snapshot(workspace, path)?;
    if inspect_recovery {
        file.recoveries = file_save::recoveries(workspace, &file.path)?;
    }
    Ok(file)
}

fn file_snapshot(workspace: TaskWorkspace<'_>, path: &str) -> OpsResult<TaskFileSnapshot> {
    let relative = validate_task_relative_path(path)?;
    let root = workspace
        .worktree
        .canonicalize()
        .map_err(|error| task_error(format!("cannot resolve Task worktree: {error}")))?;
    let mut snapshot = TaskFileSnapshot {
        recoveries: Vec::new(),
        issue_identifier: workspace.issue_identifier.to_string(),
        task_id: workspace.task_id.to_string(),
        path: relative.clone(),
        content: None,
        state: TaskFileState::Missing,
        revision: None,
        read_only_reason: None,
        size_bytes: 0,
    };
    let absolute = match root.join(&relative).canonicalize() {
        Ok(path) => path,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(snapshot),
        Err(error) => {
            return Err(task_error(format!(
                "cannot open Task file {relative:?}: {error}"
            )))
        }
    };
    if !absolute.starts_with(&root) || !absolute.is_file() {
        return Err(task_error(format!(
            "Task file {relative:?} does not resolve to a file inside the Task worktree"
        )));
    }
    let mut component_path = root.clone();
    for component in Path::new(&relative).components() {
        component_path.push(component);
        if std::fs::symlink_metadata(&component_path)?
            .file_type()
            .is_symlink()
        {
            snapshot.read_only_reason =
                Some("Files reached through a symlink are read-only.".into());
            break;
        }
    }
    if Path::new(&relative)
        .components()
        .any(|part| part.as_os_str() == ".git")
    {
        snapshot.read_only_reason = Some("Git metadata is read-only.".into());
    }
    let file = File::open(&absolute)
        .map_err(|error| task_error(format!("cannot read Task file {relative:?}: {error}")))?;
    snapshot.size_bytes = file.metadata()?.len();
    let mut bytes = Vec::new();
    file.take(MAX_FILE_BYTES as u64 + 1)
        .read_to_end(&mut bytes)?;
    snapshot.state = if bytes.len() > MAX_FILE_BYTES {
        TaskFileState::Truncated
    } else if bytes.contains(&0) {
        TaskFileState::Binary
    } else if let Ok(content) = String::from_utf8(bytes) {
        snapshot.revision = Some(hex::encode(Sha256::digest(content.as_bytes())));
        snapshot.content = Some(content);
        TaskFileState::Text
    } else {
        TaskFileState::UnsupportedEncoding
    };
    Ok(snapshot)
}

fn resolve_file_base(workspace: TaskComparison<'_>, selection: &str) -> OpsResult<String> {
    let reference = match selection {
        "parent" => workspace.base_commit,
        "head" => "HEAD",
        sha if sha.len() == 40 && sha.bytes().all(|byte| byte.is_ascii_hexdigit()) => sha,
        _ => {
            return Err(task_error(
                "Comparison must be parent, head, or a resolved commit SHA",
            ))
        }
    };
    Ok(git_output(
        workspace.checkout.worktree,
        &["rev-parse", "--verify", &format!("{reference}^{{commit}}")],
    )?
    .trim()
    .to_string())
}

fn net_changed_paths(workspace: TaskComparison<'_>) -> OpsResult<BTreeMap<String, Option<String>>> {
    let output = git_output_bytes(
        workspace.checkout.worktree,
        &[
            "diff",
            "--name-status",
            "--no-ext-diff",
            "--find-renames",
            "-z",
            workspace.base_commit,
            "--",
        ],
    )?;
    let entries = nul_paths(&output);
    let mut entries = entries.into_iter();
    let mut paths = BTreeMap::new();
    while let Some(status) = entries.next() {
        let source = entries
            .next()
            .ok_or_else(|| task_error("Missing Git path"))?;
        if status.starts_with('R') || status.starts_with('C') {
            let destination = entries
                .next()
                .ok_or_else(|| task_error("Missing Git rename destination"))?;
            paths.insert(destination, Some(source));
        } else {
            paths.insert(source, None);
        }
    }
    Ok(paths)
}

fn scratch_paths(root: &Path) -> OpsResult<(Vec<String>, bool)> {
    // Bound traversal as well as returned files. Never descend into symlinks.
    let mut pending = vec![root.join("scratch")];
    let mut paths = Vec::new();
    let mut visited = 0;
    while let Some(directory) = pending.pop() {
        match std::fs::symlink_metadata(&directory) {
            Ok(metadata) if metadata.is_dir() => {}
            Ok(_) => continue,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => continue,
            Err(error) => return Err(error.into()),
        }
        for entry in std::fs::read_dir(directory)? {
            let entry = entry?;
            visited += 1;
            if visited > 2_000 {
                paths.sort();
                return Ok((paths, true));
            }
            let kind = entry.file_type()?;
            if kind.is_dir() {
                pending.push(entry.path());
            } else if kind.is_file() {
                paths.push(
                    entry
                        .path()
                        .strip_prefix(root)
                        .expect("scratch is under worktree")
                        .to_string_lossy()
                        .into_owned(),
                );
            }
        }
    }
    paths.sort();
    Ok((paths, false))
}

fn patch_snapshot(
    workspace: TaskComparison<'_>,
    path: Option<String>,
    bytes: &[u8],
) -> TaskDiffSnapshot {
    let truncated = bytes.len() > MAX_FILE_BYTES;
    let patch = String::from_utf8_lossy(&bytes[..bytes.len().min(MAX_FILE_BYTES)]).into_owned();
    // Git's binary markers start a line; patch content is always prefixed.
    let binary = patch
        .lines()
        .any(|line| line.starts_with("Binary files ") || line == "GIT binary patch");
    TaskDiffSnapshot {
        issue_identifier: workspace.checkout.issue_identifier.to_string(),
        task_id: workspace.checkout.task_id.to_string(),
        base_commit: workspace.base_commit.to_string(),
        path,
        patch,
        binary,
        truncated,
    }
}

fn draft_snapshot(
    workspace: TaskComparison<'_>,
    path: &str,
    draft: &str,
) -> OpsResult<TaskDiffSnapshot> {
    if draft.len() > MAX_FILE_BYTES {
        return Err(task_error("Draft exceeds 1 MB"));
    }
    let path = validate_task_relative_path(path)?;
    let changes = net_changed_paths(workspace)?;
    let old_path = changes
        .get(&path)
        .and_then(Option::as_deref)
        .unwrap_or(&path);
    let spec = format!("{}:{old_path}", workspace.base_commit);
    let exists = !git_output_bytes(
        workspace.checkout.worktree,
        &["ls-tree", "-z", workspace.base_commit, "--", old_path],
    )?
    .is_empty();
    let original = if exists {
        git_output_bytes(workspace.checkout.worktree, &["show", &spec])?
    } else {
        Vec::new()
    };
    // Private temporary files only: draft comparisons never touch the worktree or index.
    // The a/ and b/ directories stand in for Git's prefixes so headers name the real paths.
    let directory = tempfile::tempdir()?;
    let before = format!("a/{old_path}");
    let after = format!("b/{path}");
    for (relative, bytes) in [(&before, original.as_slice()), (&after, draft.as_bytes())] {
        let file = directory.path().join(relative);
        std::fs::create_dir_all(file.parent().expect("prefixed path has a parent"))?;
        std::fs::write(file, bytes)?;
    }
    let output = Command::new("git")
        .current_dir(directory.path())
        .args([
            "diff",
            "--no-index",
            "--no-ext-diff",
            "--no-color",
            "--no-prefix",
            "--",
            &before,
            &after,
        ])
        .output()?;
    if !output.status.success() && output.status.code() != Some(1) {
        return Err(task_error(String::from_utf8_lossy(&output.stderr)));
    }
    Ok(patch_snapshot(workspace, Some(path), &output.stdout))
}

fn validate_task_relative_path(path: &str) -> OpsResult<String> {
    let path = Path::new(path);
    if path.as_os_str().is_empty() || path.is_absolute() {
        return Err(task_error(
            "Task paths must stay relative to the Task worktree",
        ));
    }
    let mut normalized = PathBuf::new();
    for component in path.components() {
        match component {
            Component::Normal(value) => normalized.push(value),
            Component::CurDir => {}
            _ => {
                return Err(task_error(
                    "Task paths must stay relative to the Task worktree",
                ))
            }
        }
    }
    if normalized.as_os_str().is_empty() {
        return Err(task_error("Task paths must name a file"));
    }
    Ok(normalized.to_string_lossy().to_string())
}

fn record_changed_paths(
    worktree: &Path,
    args: &[&str],
    files: &mut BTreeMap<String, TaskChangedFile>,
    mark: impl Fn(&mut TaskChangedFile),
) -> OpsResult<()> {
    for path in nul_paths(&git_output_bytes(worktree, args)?) {
        let file = files
            .entry(path)
            .or_insert_with_key(|path| TaskChangedFile {
                path: path.clone(),
                old_path: None,
                committed: false,
                staged: false,
                unstaged: false,
                untracked: false,
            });
        mark(file);
    }
    Ok(())
}

fn untracked_paths(worktree: &Path) -> OpsResult<Vec<String>> {
    Ok(nul_paths(&git_output_bytes(
        worktree,
        &["ls-files", "--others", "--exclude-standard", "-z"],
    )?))
}

fn nul_paths(output: &[u8]) -> Vec<String> {
    output
        .split(|byte| *byte == 0)
        .filter(|path| !path.is_empty())
        .map(|path| String::from_utf8_lossy(path).into_owned())
        .collect()
}

fn git_output(worktree: &Path, args: &[&str]) -> OpsResult<String> {
    Ok(String::from_utf8_lossy(&git_output_bytes(worktree, args)?).into_owned())
}

fn git_output_bytes(worktree: &Path, args: &[&str]) -> OpsResult<Vec<u8>> {
    let output = Command::new("git")
        .current_dir(worktree)
        .args(args)
        .output()
        .map_err(|error| task_error(format!("failed to run git {}: {error}", args.join(" "))))?;
    if !output.status.success() {
        return Err(task_error(format!(
            "git {} failed: {}",
            args.join(" "),
            String::from_utf8_lossy(&output.stderr).trim()
        )));
    }
    Ok(output.stdout)
}

pub fn task_edit(
    repo: &Path,
    issue: &str,
    wave: Option<&str>,
    title: Option<String>,
    notes: Option<String>,
) -> OpsResult<super::pm::PmUpdateResult> {
    if title.is_none() && notes.is_none() {
        return Err(task_error("task edit requires --title or --notes"));
    }
    if title
        .as_deref()
        .is_some_and(|title| title.trim().is_empty())
    {
        return Err(task_error("Task title cannot be empty"));
    }
    super::pm::pm_update(
        repo,
        &super::pm::PmUpdateOptions {
            wave: wave.map(str::to_string),
            id: issue.to_string(),
            update: super::pm::PmTaskUpdate::Edit(crate::pm::PmItemUpdate {
                name: title,
                description: notes,
            }),
        },
        &super::NullProgress,
    )
}

pub fn task_comment(
    repo: &Path,
    issue: &str,
    wave: Option<&str>,
    message: Option<&str>,
    steer: bool,
) -> OpsResult<super::pm::TaskComments> {
    block_on_task(super::pm::task_comment_async(
        repo, wave, issue, message, steer,
    ))
}

/// Request that the Task end its current turn so the next re-reads its
/// direction. The request is a durable comment on the Work; a live run observes
/// it and interrupts. With no live run it is inert — there is no turn to end.
pub fn task_interrupt(issue: &str) -> OpsResult<TaskControlResult> {
    let issue = issue.to_string();
    block_on_task(async move {
        let store = task_store().await?;
        let task = store
            .get_task_by_issue(&issue)
            .await
            .map_err(|error| task_error(format!("failed to resolve task: {error}")))?
            .ok_or_else(|| task_error(format!("no Task exists for {issue:?}")))?;
        store
            .sqlite
            .set_task_automation(&task.id, false)
            .map_err(task_error)?;
        let work = crate::durable::WorkRef::Task(task.id.clone());
        store.append_interrupt(&work).await.map_err(task_error)?;
        Ok(TaskControlResult {
            issue_id: task.plan.identifier.clone(),
            task_id: task.id.to_string(),
            receipt: super::child::WorkControlReceipt::Interrupt { work },
            observation: task.observation.clone(),
        })
    })
}

pub fn task_wait(issue: &str, until: TaskWaitUntil, timeout: Option<Duration>) -> OpsResult<Task> {
    let started = Instant::now();
    loop {
        let task = task_execution_status(Path::new("."), Some(issue))?
            .ok_or_else(|| task_error(format!("no Task exists for {issue:?}")))?;
        let status = block_on_task(async {
            let store = task_store().await?;
            task_work_status(&store, &task).await
        })?;
        let reached = match until {
            TaskWaitUntil::Open => {
                matches!(status, WorkStatus::Done | WorkStatus::Abandoned)
                    || active_pr(&task).is_ok_and(|pr| pr.phase() == PrPhase::Open)
            }
            TaskWaitUntil::Terminal => matches!(status, WorkStatus::Done | WorkStatus::Abandoned),
        };
        if reached || timeout.is_some_and(|limit| started.elapsed() >= limit) {
            return Ok(task);
        }
        std::thread::sleep(Duration::from_secs(1));
    }
}

#[cfg(test)]
mod tests {
    use super::{
        apply_merged_task_landing, checkout_execution_boundary, lock_task_pr_mutation,
        resolve_task_create_input, task_event_exec_refusal,
    };
    use crate::child::ChildRef;
    use crate::durable::{WorkRef, WorkStatus};
    use crate::planning::{LinearIssueId, LinearProjectId, ProjectPlan, TaskPlan};
    use crate::store::{SharedStore, StorageConfig};
    use crate::work::project::{Project, ProjectId};
    use crate::work::task::{
        AfterMerge, GithubPr, Observation, PmWritebackState, PrMergeMode, PrMergeRequest,
        PrPresentation, PrPublication, Task, TaskEventKind, TaskId, TaskPr, TaskPrId,
    };
    use crate::work::wave::Wave;
    use std::ffi::OsString;
    use std::os::unix::fs::PermissionsExt;

    struct TaskFixture {
        _database: tempfile::TempDir,
        database_path: std::path::PathBuf,
        store: SharedStore,
        task: Task,
        work: WorkRef,
    }

    struct EnvRestore(Vec<(&'static str, Option<OsString>)>);

    impl EnvRestore {
        fn capture(names: &[&'static str]) -> Self {
            Self(
                names
                    .iter()
                    .map(|name| (*name, std::env::var_os(name)))
                    .collect(),
            )
        }
    }

    impl Drop for EnvRestore {
        fn drop(&mut self) {
            for (name, value) in &self.0 {
                match value {
                    Some(value) => std::env::set_var(name, value),
                    None => std::env::remove_var(name),
                }
            }
        }
    }

    async fn task_fixture(identifier: &str) -> TaskFixture {
        let repository =
            std::fs::canonicalize(std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../.."))
                .unwrap();
        task_fixture_at(identifier, repository).await
    }

    #[test]
    fn explicit_reconciliation_recovers_merged_delivery_without_completing_work() {
        let _ledger = crate::journal::TestLedgerGuard::new();
        let _environment = EnvRestore::capture(&["PATH"]);
        let repo = loopflow_test_support::TestRepo::new();
        assert!(std::process::Command::new("git")
            .current_dir(repo.path())
            .args([
                "remote",
                "set-url",
                "origin",
                "https://github.com/example/repo.git"
            ])
            .output()
            .unwrap()
            .status
            .success());
        let bin = tempfile::tempdir().unwrap();
        let gh = bin.path().join("gh");
        std::fs::write(
            &gh,
            "#!/bin/sh\nfixture=$(dirname \"$0\")\ncase \"$1\" in\npr) cat \"$fixture/discovery.json\" ;;\napi) cat \"$fixture/pr.json\" ;;\nesac\n",
        )
        .unwrap();
        std::fs::set_permissions(&gh, std::fs::Permissions::from_mode(0o755)).unwrap();
        let prior_path = std::env::var_os("PATH").unwrap_or_default();
        std::env::set_var(
            "PATH",
            std::env::join_paths(
                std::iter::once(bin.path().to_path_buf()).chain(std::env::split_paths(&prior_path)),
            )
            .unwrap(),
        );
        std::fs::write(
            bin.path().join("pr.json"),
            serde_json::json!({
                "number":1283,"html_url":"https://github.com/example/repo/pull/1283",
                "state":"closed","merged":true,"mergeable_state":null,
                "merge_commit_sha":"merged-commit","merged_at":"2026-09-25T21:56:42Z",
                "head":{"sha":"authored-head"}
            })
            .to_string(),
        )
        .unwrap();
        let viewer = repo.path().join("unrelated-viewer.html");
        std::fs::write(&viewer, "keep this authored artifact").unwrap();
        let runtime = tokio::runtime::Runtime::new().unwrap();
        for (discovery, merged) in [
            ("[]", false),
            ("[{\"number\":1283},{\"number\":1284}]", false),
            ("invalid response", false),
            ("[{\"number\":1283}]", true),
        ] {
            std::fs::write(bin.path().join("discovery.json"), discovery).unwrap();
            let mut fixture = runtime.block_on(task_fixture_at("RECOVER-PR", repo.path().into()));
            let before = runtime
                .block_on(fixture.store.task_prs(&fixture.task.id))
                .unwrap();
            let cached = runtime
                .block_on(super::reconcile_task_pr(&fixture.store, &mut fixture.task))
                .unwrap()
                .unwrap();
            assert!(cached.publication.is_none());
            let pr = runtime
                .block_on(super::reconcile_task_pr_observation(
                    &fixture.store,
                    &mut fixture.task,
                    crate::ops::pr::PrReadFreshness::Fresh,
                ))
                .unwrap()
                .unwrap();
            assert_eq!(pr.id, before[0].id);
            assert_eq!(
                pr.merge_commit.as_deref(),
                merged.then_some("merged-commit")
            );
            assert_eq!(
                pr.github().map(|github| github.number),
                merged.then_some(1283)
            );
            assert_eq!(
                runtime
                    .block_on(fixture.store.task_prs(&fixture.task.id))
                    .unwrap()
                    .len(),
                1
            );
            assert_ne!(
                runtime
                    .block_on(super::task_work_status(&fixture.store, &fixture.task))
                    .unwrap(),
                WorkStatus::Done
            );
            assert_eq!(
                std::fs::read_to_string(&viewer).unwrap(),
                "keep this authored artifact"
            );
            if merged {
                let saved = runtime
                    .block_on(fixture.store.task_prs(&fixture.task.id))
                    .unwrap()
                    .remove(0);
                let repeated = runtime
                    .block_on(super::reconcile_task_pr_observation(
                        &fixture.store,
                        &mut fixture.task,
                        crate::ops::pr::PrReadFreshness::Fresh,
                    ))
                    .unwrap()
                    .unwrap();
                assert_eq!(repeated, saved);
            } else {
                assert!(pr.publication.is_none());
            }
        }
    }

    #[tokio::test]
    async fn completed_session_with_exited_provider_does_not_block_task_work() {
        let mut child = std::process::Command::new("true").spawn().unwrap();
        let exited_pid = child.id();
        child.wait().unwrap();
        let live_pid = std::process::id();
        let live_start = crate::journal::process_started_at(live_pid)
            .unwrap()
            .unwrap();
        for (completed, provider, blocked) in [
            (false, Some((exited_pid, 1_i64)), true),
            (true, Some((exited_pid, 1_i64)), false),
            (true, Some((live_pid, live_start)), true),
            (true, None, true),
        ] {
            let fixture = task_fixture("CLOSED-SESSION").await;
            let conn = rusqlite::Connection::open(&fixture.database_path).unwrap();
            conn.execute(
                "INSERT INTO agent_sessions(id,title,title_source,created_at,input_published,
                    interactive,task_id,completed_at,provider_pid,provider_started_at,wave_id,cwd)
                 VALUES('old-session','old implement','generated',1,1,0,?1,?2,?3,?4,?5,?6)",
                rusqlite::params![
                    fixture.task.id.as_str(),
                    completed.then_some(2_i64),
                    provider.map(|value| value.0),
                    provider.map(|value| value.1),
                    fixture.task.wave_id.as_str(),
                    fixture.task.worktree.to_str().unwrap(),
                ],
            )
            .unwrap();
            conn.execute(
                "INSERT INTO session_events(session_id,kind,receipt_key,observed_at,payload)
                 VALUES('old-session','captured',?1,1,'{}')",
                [crate::session_record::new_artifact_key()],
            )
            .unwrap();
            conn.execute(
                "UPDATE agent_sessions SET current_capture=?1 WHERE id='old-session'",
                [conn.last_insert_rowid()],
            )
            .unwrap();
            fixture
                .store
                .sqlite
                .record_session_event(
                    "old-session",
                    "thread",
                    "turn",
                    crate::session::SessionEventKind::Started,
                    &serde_json::json!({}),
                )
                .unwrap();
            assert_eq!(
                crate::ops::task_automation::admission_blocker(
                    &fixture.store.sqlite,
                    &fixture.task.id,
                    None,
                )
                .unwrap()
                .is_some(),
                blocked,
            );
            assert_eq!(
                !super::lifecycle::associated_work_blockers(&fixture.store, &fixture.task)
                    .unwrap()
                    .is_empty(),
                blocked,
            );
            // Administrative closure never invents a native completion receipt.
            assert!(fixture
                .store
                .sqlite
                .session_has_pending_turn("old-session")
                .unwrap());
        }
    }

    #[test]
    fn a_dead_flow_is_history_while_a_live_driver_holds_completion() {
        let _ledger = crate::journal::TestLedgerGuard::new();
        let runtime = tokio::runtime::Runtime::new().unwrap();
        let fixture = runtime.block_on(task_fixture("WORK-1"));
        let flow = runtime
            .block_on(fixture.store.create_flow(crate::durable::FlowSession {
                invocation: crate::durable::test_flow_invocation(
                    "code",
                    0,
                    "implement",
                    None,
                    false,
                ),
                cursor: Default::default(),
                version: 0,
                task_id: Some(fixture.task.id.clone()),
                wave_id: Some(fixture.task.wave_id.clone()),
                cwd: fixture.task.worktree.clone(),
                message: None,
                model: None,
                current_attempt: None,
                pending_session_id: None,
                failure: None,
                finished: false,
                updated_at: time::OffsetDateTime::now_utc(),
            }))
            .unwrap();
        let blockers =
            || super::lifecycle::associated_work_blockers(&fixture.store, &fixture.task).unwrap();
        assert!(blockers().is_empty(), "an undriven Flow is history");
        let driver = crate::ops::flow_run::driver_lock(flow.id()).unwrap();
        assert!(blockers().iter().any(|reason| reason.contains(flow.id())));
        drop(driver);
        assert!(blockers().is_empty());

        // A reboot proves stale execution exited without settling the Flow.
        let now = time::OffsetDateTime::now_utc().unix_timestamp();
        let exec = crate::exec::Exec {
            id: crate::id::ExecId::new(),
            trace_id: crate::id::TraceId::new(),
            parent_exec_id: None,
            via_agent: None,
            caller_session_id: None,
            caller_provider_generation: None,
            command: Some("implement".into()),
            repo: None,
            cwd: Some(fixture.task.worktree.to_string_lossy().into_owned()),
            started_at: now - 100,
            completed_at: None,
            outcome: None,
            exit_code: None,
            signal: None,
            error: None,
        };
        fixture.store.sqlite.record_exec(&exec).unwrap();
        let session = fixture
            .store
            .sqlite
            .test_session("stalled", &crate::session_record::new_artifact_key());
        let conn = rusqlite::Connection::open(&fixture.database_path).unwrap();
        conn.execute(
            "UPDATE agent_sessions SET cwd=?1 WHERE id=?2",
            rusqlite::params![fixture.task.worktree.to_str().unwrap(), session.id],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO session_events(session_id,provider_thread,provider_turn,kind,receipt_key,observed_at,payload)
             VALUES(?1,'thread','turn','started','turn',?2,'{}')",
            rusqlite::params![session.id, now - 100],
        )
        .unwrap();
        crate::journal::set_test_machine_booted_at(Some(now - 200));
        let unresolved = blockers();
        crate::journal::set_test_machine_booted_at(Some(now - 50));
        let after_boot = blockers();
        crate::journal::set_test_machine_booted_at(None);
        assert_eq!(unresolved.len(), 2, "{unresolved:?}");
        assert!(unresolved
            .iter()
            .any(|reason| reason.contains(exec.id.as_str())));
        assert!(unresolved.iter().any(|reason| reason.contains("stalled")));
        assert!(after_boot.is_empty(), "{after_boot:?}");
        assert_eq!(fixture.store.sqlite.exec(&exec.id).unwrap(), Some(exec));
        assert_eq!(
            runtime
                .block_on(fixture.store.latest_task_flow(&fixture.task.id))
                .unwrap(),
            Some(flow)
        );
    }

    async fn task_fixture_at(identifier: &str, repository: std::path::PathBuf) -> TaskFixture {
        let database = tempfile::tempdir().unwrap();
        let database_path = database.path().join("loopflow.db");
        let store = std::sync::Arc::new(
            crate::store::open_ephemeral_store(&StorageConfig::sqlite(database_path.clone()))
                .await
                .unwrap(),
        );
        let now = time::OffsetDateTime::now_utc();
        let wave = Wave::new(
            crate::id::WaveId::new(),
            "task-recovery".to_string(),
            repository.display().to_string(),
        );
        let project = Project {
            id: ProjectId::new(),
            plan: ProjectPlan {
                flow: "feature".into(),
                status: crate::pm::ProjectStatus::Started,
                id: LinearProjectId::new("task-recovery-project").unwrap(),
                slug: "task-recovery".to_string(),
                name: "Task recovery".to_string(),
                prompt_context: "Keep automatic Task recovery bounded.".to_string(),
                pm_snapshot_synced_at: now.unix_timestamp(),
            },
            wave_id: wave.id().clone(),
            iteration: 0,
            abandon_intent: None,
            created_at: now,
            updated_at: now,
        };
        let task = Task {
            id: TaskId::new(),
            plan: TaskPlan {
                id: LinearIssueId::new(format!("{identifier}-issue")).unwrap(),
                identifier: identifier.to_string(),
                title: "Task recovery fixture".to_string(),
                description: String::new(),
                pm_snapshot_synced_at: now.unix_timestamp(),
            },
            pm_writeback: PmWritebackState::Current,
            wave_id: wave.id().clone(),
            project_id: project.id.clone(),
            worktree: repository,
            workspace_slug: "task-recovery-fixture".to_string(),
            agent: None,
            abandon_intent: None,
            created_at: now,
            updated_at: now,
            observation: Observation::NotRequired,
        };
        let pr = TaskPr {
            id: TaskPrId::new(),
            task_id: task.id.clone(),
            sequence: 1,
            slug: task.workspace_slug.clone(),
            branch: format!("test/{}", task.workspace_slug),
            base_commit: "deadbeef".to_string(),
            parent_pr_id: None,
            publication: None,
            merge_commit: None,
            abandoned_at: None,
            ci_observation: None,
            github_observation: None,
            linear_attachment_id: None,
            linear_comment_id: None,
            linear_link_error: None,
            created_at: now,
            updated_at: now,
        };
        store.create_wave(&wave).await.unwrap();
        store.create_project(&project).await.unwrap();
        store.create_task(&task, &pr).await.unwrap();
        let work = store
            .work_for_child(&ChildRef::Task(task.id.clone()))
            .await
            .unwrap();
        TaskFixture {
            _database: database,
            database_path,
            store,
            task,
            work,
        }
    }

    #[test]
    fn task_files_use_recorded_placement_without_lifecycle_reconciliation() {
        let repo = loopflow_test_support::TestRepo::new();
        let runtime = tokio::runtime::Runtime::new().unwrap();
        let fixture = runtime.block_on(task_fixture_at("FILES-1", repo.path().to_path_buf()));
        let mut pr = runtime
            .block_on(fixture.store.active_task_pr(&fixture.task.id))
            .unwrap()
            .unwrap();
        pr.base_commit = super::git_output(repo.path(), &["rev-parse", "HEAD"])
            .unwrap()
            .trim()
            .into();
        rusqlite::Connection::open(&fixture.database_path)
            .unwrap()
            .execute(
                "UPDATE task_prs SET base_commit=?2 WHERE id=?1",
                rusqlite::params![pr.id.as_str(), pr.base_commit],
            )
            .unwrap();
        std::fs::create_dir_all(repo.path().join("scratch")).unwrap();
        std::fs::write(repo.path().join("scratch/note.md"), "local note\n").unwrap();
        let original_task = runtime
            .block_on(fixture.store.get_task(&fixture.task.id))
            .unwrap()
            .unwrap();
        let connection = rusqlite::Connection::open(&fixture.database_path).unwrap();
        connection.execute_batch(
            "CREATE TRIGGER no_file_task_update BEFORE UPDATE ON tasks BEGIN SELECT RAISE(FAIL,'file access cannot update Task'); END;
             CREATE TRIGGER no_file_pr_update BEFORE UPDATE ON task_prs BEGIN SELECT RAISE(FAIL,'file access cannot reconcile PR'); END;"
        ).unwrap();
        crate::ops::pm::PM_TEST_CONTEXT.sync_scope(
            crate::ops::pm::PmTestContext {
                path: fixture.database_path.clone(),
                store: fixture.store.clone(),
                graphql_url: "http://127.0.0.1:1".into(),
            },
            || {
                for issue in [
                    "FILES-1",
                    fixture.task.id.as_str(),
                    fixture.task.plan.id.as_str(),
                ] {
                    let file = super::task_file(issue, "scratch/note.md", false).unwrap();
                    assert_eq!(file.content.as_deref(), Some("local note\n"));
                    let changes = super::task_changes(issue, "parent").unwrap();
                    assert_eq!(changes.base_commit, pr.base_commit);
                    assert_eq!(changes.scratch, ["scratch/note.md"]);
                    let diff =
                        super::task_diff(issue, Some("scratch/note.md"), "head", None).unwrap();
                    assert!(diff.patch.contains("+local note"));
                }
                let file = super::task_file("FILES-1", "scratch/note.md", false).unwrap();
                let saved = super::file_save::task_save(
                    "FILES-1",
                    "scratch/note.md",
                    file.revision.as_deref().unwrap(),
                    "saved\n",
                )
                .unwrap();
                assert!(saved.published);
                assert_eq!(saved.file.content.as_deref(), Some("saved\n"));
            },
        );
        assert_eq!(
            runtime
                .block_on(fixture.store.get_task(&fixture.task.id))
                .unwrap()
                .unwrap(),
            original_task
        );
        assert_eq!(
            runtime
                .block_on(fixture.store.active_task_pr(&fixture.task.id))
                .unwrap()
                .unwrap(),
            pr
        );
        connection
            .execute_batch("PRAGMA foreign_keys=OFF; DELETE FROM task_prs; DELETE FROM projects;")
            .unwrap();
        crate::ops::pm::PM_TEST_CONTEXT.sync_scope(
            crate::ops::pm::PmTestContext {
                path: fixture.database_path.clone(),
                store: fixture.store.clone(),
                graphql_url: "http://127.0.0.1:1".into(),
            },
            || {
                let file = super::task_file("FILES-1", "scratch/note.md", false).unwrap();
                let saved = super::task_save(
                    "FILES-1",
                    "scratch/note.md",
                    file.revision.as_deref().unwrap(),
                    "no PR needed\n",
                )
                .unwrap();
                assert!(saved.published);
                assert_eq!(saved.file.content.as_deref(), Some("no PR needed\n"));
                let directory = super::task_files("FILES-1", "scratch", None, false).unwrap();
                assert_eq!(directory.entries[0].path, "scratch/note.md");
                assert!(super::task_changes("FILES-1", "parent").is_err());
                let conflict = super::task_save(
                    "FILES-1",
                    "scratch/note.md",
                    file.revision.as_deref().unwrap(),
                    "stale",
                )
                .unwrap();
                assert!(!conflict.published);
                assert_eq!(conflict.file.content, saved.file.content);
            },
        );
    }

    #[test]
    fn task_files_compare_net_parent_head_and_revisioned_drafts() {
        let repo = tempfile::tempdir().unwrap();
        let git = |args: &[&str]| {
            super::git_output(repo.path(), args)
                .unwrap()
                .trim()
                .to_string()
        };
        git(&["init", "-q", "-b", "main"]);
        let commit = || {
            git(&[
                "-c",
                "user.name=Test",
                "-c",
                "user.email=test@example.com",
                "commit",
                "-qm",
                "fixture",
            ]);
        };
        std::fs::write(repo.path().join("cancelled.txt"), "base\n").unwrap();
        std::fs::write(repo.path().join("old.txt"), "rename me\n").unwrap();
        git(&["add", "."]);
        commit();
        let parent = git(&["rev-parse", "HEAD"]);
        std::fs::write(repo.path().join("cancelled.txt"), "committed\n").unwrap();
        git(&["mv", "old.txt", "new.txt"]);
        git(&["add", "."]);
        commit();
        std::fs::write(repo.path().join("cancelled.txt"), "base\n").unwrap();
        std::fs::create_dir_all(repo.path().join("scratch/nested")).unwrap();
        std::fs::write(
            repo.path().join("scratch/nested/notes.md"),
            "\u{feff}notes\r\n",
        )
        .unwrap();
        let id = TaskId::new();
        let workspace = super::TaskComparison {
            checkout: super::TaskWorkspace {
                issue_identifier: "TEST-1",
                task_id: &id,
                worktree: repo.path(),
            },
            base_commit: &parent,
        };
        let changes = super::changes_snapshot(workspace).unwrap();
        assert!(!changes
            .files
            .iter()
            .any(|file| file.path == "cancelled.txt"));
        let renamed = changes
            .files
            .iter()
            .find(|file| file.path == "new.txt")
            .unwrap();
        assert_eq!(renamed.old_path.as_deref(), Some("old.txt"));
        assert_eq!(changes.scratch, ["scratch/nested/notes.md"]);
        let patch = super::diff_snapshot(workspace, Some("new.txt")).unwrap();
        assert!(patch.patch.contains("rename from old.txt"));
        let head = super::resolve_file_base(workspace, "head").unwrap();
        let head_changes = super::changes_snapshot(super::TaskComparison {
            base_commit: &head,
            ..workspace
        })
        .unwrap();
        assert!(head_changes
            .files
            .iter()
            .any(|file| file.path == "cancelled.txt"));
        assert!(!head_changes.files.iter().any(|file| file.path == "new.txt"));
        let file = super::file_snapshot(workspace.checkout, "scratch/nested/notes.md").unwrap();
        assert_eq!(file.content.as_deref(), Some("\u{feff}notes\r\n"));
        assert_eq!(file.revision.as_ref().unwrap().len(), 64);
        let draft = super::draft_snapshot(workspace, "new.txt", "local draft\n").unwrap();
        assert!(draft.patch.contains("--- a/old.txt\n+++ b/new.txt\n"));
        assert!(draft.patch.contains("-rename me"));
        assert!(draft.patch.contains("+local draft"));
        let quoted = super::draft_snapshot(workspace, "new.txt", "Binary files differ\n").unwrap();
        assert!(!quoted.binary);
        std::fs::write(repo.path().join("new.txt"), "Binary files differ\n").unwrap();
        assert!(
            !super::diff_snapshot(workspace, Some("new.txt"))
                .unwrap()
                .binary
        );
        std::fs::write(repo.path().join("new.txt"), "rename me\n").unwrap();
        assert_eq!(
            std::fs::read_to_string(repo.path().join("new.txt")).unwrap(),
            "rename me\n"
        );
        assert_eq!(
            super::file_snapshot(workspace.checkout, "gone")
                .unwrap()
                .state,
            super::TaskFileState::Missing
        );
        for (bytes, state) in [
            (vec![0], super::TaskFileState::Binary),
            (vec![255], super::TaskFileState::UnsupportedEncoding),
            (vec![b'a'; 1_000_001], super::TaskFileState::Truncated),
        ] {
            std::fs::write(repo.path().join("edge"), bytes).unwrap();
            let file = super::file_snapshot(workspace.checkout, "edge").unwrap();
            assert_eq!(file.state, state);
            assert!(file.content.is_none());
            assert!(file.revision.is_none());
        }
        assert!(super::file_snapshot(workspace.checkout, "../outside").is_err());
        #[cfg(unix)]
        {
            let before = super::file_snapshot(workspace.checkout, "new.txt").unwrap();
            assert!(before.read_only_reason.is_none());
            std::fs::rename(repo.path().join("new.txt"), repo.path().join("target.txt")).unwrap();
            std::os::unix::fs::symlink("target.txt", repo.path().join("new.txt")).unwrap();
            let after = super::file_snapshot(workspace.checkout, "new.txt").unwrap();
            assert_eq!(before.revision, after.revision);
            assert_eq!(before.content, after.content);
            assert!(after.read_only_reason.is_some());
            std::os::unix::fs::symlink("nested", repo.path().join("scratch/alias")).unwrap();
            let nested =
                super::file_snapshot(workspace.checkout, "scratch/alias/notes.md").unwrap();
            assert_eq!(nested.content, file.content);
            assert!(nested.read_only_reason.is_some());
            let outside = tempfile::tempdir().unwrap();
            std::fs::write(outside.path().join("private"), "outside").unwrap();
            std::os::unix::fs::symlink(outside.path(), repo.path().join("scratch/link")).unwrap();
            assert_eq!(
                super::scratch_paths(repo.path()).unwrap().0,
                ["scratch/nested/notes.md"]
            );
            assert!(super::file_snapshot(workspace.checkout, "scratch/link/private").is_err());
        }
    }

    #[tokio::test]
    async fn task_workspace_context_covers_committed_staged_unstaged_and_untracked_bytes() {
        let repository = tempfile::tempdir().unwrap();
        let git = |args: &[&str]| {
            let output = std::process::Command::new("git")
                .current_dir(repository.path())
                .args(args)
                .output()
                .unwrap();
            assert!(
                output.status.success(),
                "git {} failed: {}",
                args.join(" "),
                String::from_utf8_lossy(&output.stderr)
            );
            String::from_utf8(output.stdout).unwrap().trim().to_string()
        };
        git(&["init", "-q", "-b", "main"]);
        std::fs::write(repository.path().join("tracked.txt"), "base\n").unwrap();
        git(&["add", "tracked.txt"]);
        git(&[
            "-c",
            "user.email=test@loopflow.dev",
            "-c",
            "user.name=Loopflow Test",
            "commit",
            "-q",
            "-m",
            "base",
        ]);
        let base = git(&["rev-parse", "HEAD"]);
        let TaskFixture {
            store, mut task, ..
        } = task_fixture_at(
            "TEST-WORKSPACE",
            std::fs::canonicalize(repository.path()).unwrap(),
        )
        .await;
        let mut pr = store.active_task_pr(&task.id).await.unwrap().unwrap();
        task.worktree = std::fs::canonicalize(repository.path()).unwrap();
        pr.base_commit = base;

        std::fs::write(repository.path().join("tracked.txt"), "committed\n").unwrap();
        git(&["add", "tracked.txt"]);
        git(&[
            "-c",
            "user.email=test@loopflow.dev",
            "-c",
            "user.name=Loopflow Test",
            "commit",
            "-q",
            "-m",
            "committed",
        ]);
        std::fs::write(repository.path().join("staged.txt"), "staged bytes\n").unwrap();
        git(&["add", "staged.txt"]);
        std::fs::write(repository.path().join("tracked.txt"), "unstaged bytes\n").unwrap();
        std::fs::write(repository.path().join("untracked.txt"), "untracked bytes\n").unwrap();

        let context = super::task_workspace_context(&task, &pr).unwrap();

        for expected in [
            "\"committed\": true",
            "\"staged\": true",
            "\"unstaged\": true",
            "\"untracked\": true",
            "\"content_sha256\"",
            "unstaged bytes",
            "staged bytes",
            "untracked bytes",
        ] {
            assert!(context.contains(expected), "missing {expected:?}");
        }
    }

    #[tokio::test]
    #[allow(clippy::await_holding_lock)] // the guard serializes process-wide Run env
    async fn parent_run_cannot_override_task_worktree_resolution() {
        let _lock = crate::journal::test_env_lock();
        let _environment = EnvRestore::capture(&[
            crate::durable::RUN_ID_ENV,
            crate::session_record::RUN_DIR_ENV,
        ]);
        let repository = loopflow_test_support::TestRepo::new();
        repository.create_branch("test/task-recovery-fixture");
        repository.push_new_branch("test/task-recovery-fixture");
        let TaskFixture { store, task, .. } =
            task_fixture_at("TEST-PARENT", repository.path().to_path_buf()).await;
        let parent_run_id = crate::session_record::new_artifact_key();
        std::env::set_var(crate::durable::RUN_ID_ENV, parent_run_id.as_str());
        std::env::remove_var(crate::session_record::RUN_DIR_ENV);

        let resolved = super::task_for_checkout(&store, &task.worktree)
            .await
            .unwrap()
            .unwrap();

        assert_eq!(resolved.id, task.id);
    }

    #[tokio::test]
    async fn watched_landing_completes_task_only_from_merged_pr_evidence() {
        let repository = loopflow_test_support::TestRepo::new();
        let TaskFixture {
            _database,
            store,
            mut task,
            work,
            database_path,
            ..
        } = task_fixture_at("LOO-248", repository.path().to_path_buf()).await;
        let now = time::OffsetDateTime::now_utc();
        let mut pr = store.active_task_pr(&task.id).await.unwrap().unwrap();
        pr.branch = "HEAD".to_string();
        rusqlite::Connection::open(database_path)
            .unwrap()
            .execute(
                "UPDATE task_prs SET branch=?2 WHERE id=?1",
                rusqlite::params![pr.id.as_str(), pr.branch],
            )
            .unwrap();
        let landed_head = crate::engine::git::rev_parse(&task.worktree, "HEAD").unwrap();
        pr.publication = Some(PrPublication {
            requested_at: now,
            presentation: Some(PrPresentation {
                title: "Watch the landing".to_string(),
                body: "Finish only after GitHub confirms merge.".to_string(),
                head_sha: landed_head.clone(),
            }),
            github: Some(GithubPr {
                number: 248,
                url: "https://github.com/loopflowstudio/loopflow/pull/248".to_string(),
                head_sha: Some(landed_head.clone()),
            }),
            merge: Some(PrMergeRequest {
                mode: PrMergeMode::Auto,
                requested_at: now,
                head_sha: landed_head.clone(),
                after_merge: AfterMerge::CompleteTask,
                next_slug: None,
            }),
        });
        store.update_task_pr(&pr).await.unwrap();
        let landing = crate::pr_landing::PrLanding::new(
            crate::pr_landing::NewPrLanding {
                repo: "loopflowstudio/loopflow".to_string(),
                pr_number: 248,
                worktree: task.worktree.clone(),
                branch: pr.branch.clone(),
                task_id: Some(task.id.clone()),
                requested_head_sha: landed_head.clone(),
                after_merge: Some(AfterMerge::CompleteTask),
                next_slug: None,
            },
            now,
        )
        .unwrap();

        assert_eq!(store.work_status(&work).await.unwrap(), WorkStatus::Ready);
        let error = apply_merged_task_landing(&store, &mut task, &pr, &landing)
            .await
            .expect_err("an armed but unmerged PR cannot complete the Task");
        assert!(error.to_string().contains("did not confirm"));
        assert_eq!(store.work_status(&work).await.unwrap(), WorkStatus::Ready);

        pr.merge_commit = Some(landed_head);
        store.settle_task_pr(&pr, None).await.unwrap();
        apply_merged_task_landing(&store, &mut task, &pr, &landing)
            .await
            .unwrap();
        assert_eq!(store.work_status(&work).await.unwrap(), WorkStatus::Done);
    }

    #[test]
    fn deleted_task_status_requires_explicit_history_lookup() {
        const CHILD: &str = "LOOPFLOW_DELETED_STATUS_CHILD";
        if std::env::var_os(CHILD).is_some() {
            let path =
                std::path::PathBuf::from(std::env::var_os("LF_HOME").unwrap()).join("loopflow.db");
            let runtime = tokio::runtime::Runtime::new().unwrap();
            let store = std::sync::Arc::new(
                runtime
                    .block_on(crate::store::open_ephemeral_store(&StorageConfig::sqlite(
                        path.clone(),
                    )))
                    .unwrap(),
            );
            // Install fault injection after schema validation opens the fixture.
            let connection = rusqlite::Connection::open(&path).unwrap();
            connection.execute_batch(
            "CREATE TRIGGER no_history_task_update BEFORE UPDATE ON tasks BEGIN SELECT RAISE(FAIL,'historical Task must remain read-only'); END;
             CREATE TRIGGER no_history_pr_update BEFORE UPDATE ON task_prs BEGIN SELECT RAISE(FAIL,'historical PR must remain read-only'); END;
             CREATE TRIGGER no_history_event BEFORE INSERT ON task_events BEGIN SELECT RAISE(FAIL,'historical Task must not gain events'); END;"
        ).unwrap();
            crate::ops::pm::PM_TEST_CONTEXT.sync_scope(
                crate::ops::pm::PmTestContext {
                    path,
                    store,
                    graphql_url: "http://127.0.0.1:1".into(),
                },
                || {
                    let repo = std::env::current_dir().unwrap();
                    let error = super::task_status(&repo, None).unwrap_err();
                    assert!(error.to_string().contains("Task was deleted"), "{error}");
                    let status = super::task_status(&repo, Some("HISTORY-1")).unwrap();
                    let snapshot = status.execution.as_ref().unwrap();
                    assert_eq!(
                        super::task_status(&repo, Some(&snapshot.task_id)).unwrap(),
                        status
                    );
                    assert_eq!(
                        super::task_status(&repo, Some(&snapshot.issue_id)).unwrap(),
                        status
                    );
                    assert_eq!(snapshot.status, WorkStatus::Done);
                    assert_eq!(
                        snapshot.actions.recommended,
                        Some(crate::ops::task_actions::TaskAction::NoAction)
                    );
                },
            );
            return;
        }

        let repo = loopflow_test_support::TestRepo::new();
        repo.create_branch("test/task-recovery-fixture");
        let runtime = tokio::runtime::Runtime::new().unwrap();
        let fixture = runtime.block_on(task_fixture_at("HISTORY-1", repo.path().to_path_buf()));
        let connection = rusqlite::Connection::open(&fixture.database_path).unwrap();
        // Seed a completed historical outcome and positive removal evidence.
        // This is a reader proof, not a provider-deletion fixture.
        connection
            .execute(
                "UPDATE tasks SET work_state='done',work_terminal_at=123 WHERE id=?1",
                [fixture.task.id.as_str()],
            )
            .unwrap();
        connection.execute(
            "INSERT INTO task_deletions(wave_id,issue_id,identifier,confirmed_at) VALUES (?1,?2,?3,456)",
            rusqlite::params![fixture.task.wave_id.as_str(), fixture.task.plan.id.as_str(), fixture.task.plan.identifier],
        ).unwrap();
        let pr = runtime
            .block_on(fixture.store.active_task_pr(&fixture.task.id))
            .unwrap();
        let events = runtime
            .block_on(fixture.store.task_events_after(&fixture.task.id, 0))
            .unwrap();
        let output = std::process::Command::new(std::env::current_exe().unwrap())
            .args([
                "--exact",
                "ops::task::tests::deleted_task_status_requires_explicit_history_lookup",
                "--test-threads=1",
            ])
            .env_clear()
            .envs(std::env::vars().filter(|(name, _)| !name.starts_with("LF_")))
            .env(CHILD, "1")
            .env("LF_HOME", fixture._database.path())
            .current_dir(repo.path())
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "{}\n{}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
        assert_eq!(
            runtime
                .block_on(fixture.store.active_task_pr(&fixture.task.id))
                .unwrap(),
            pr
        );
        assert_eq!(
            runtime
                .block_on(fixture.store.task_events_after(&fixture.task.id, 0))
                .unwrap(),
            events
        );
        assert_eq!(
            connection
                .query_row(
                    "SELECT work_terminal_at FROM tasks WHERE id=?1",
                    [fixture.task.id.as_str()],
                    |row| row.get::<_, i64>(0)
                )
                .unwrap(),
            123
        );
    }

    #[test]
    fn piped_report_supplies_title_and_preserves_full_description() {
        let report =
            "\n  lf wave status rejects stored timestamp  \n\nstack trace\nmore evidence\n";
        let input = resolve_task_create_input(None, Some(report)).expect("resolve piped report");

        assert_eq!(input.title, "lf wave status rejects stored timestamp");
        assert_eq!(
            input.report,
            "lf wave status rejects stored timestamp  \n\nstack trace\nmore evidence"
        );
    }

    #[tokio::test]
    #[allow(clippy::await_holding_lock)] // Serializes identity configuration used during publication.
    async fn steering_publication_resolves_empty_participant_from_config_or_git() {
        let _lock = crate::journal::test_env_lock();
        let _restore = EnvRestore::capture(&[
            "LF_HOME",
            "LF_USER_NAME",
            "GIT_CONFIG_COUNT",
            "GIT_CONFIG_KEY_0",
            "GIT_CONFIG_VALUE_0",
        ]);
        let repo = loopflow_test_support::TestRepo::new();
        let fixture = task_fixture_at("FIX-STEER", repo.path().canonicalize().unwrap()).await;
        std::env::set_var("LF_HOME", fixture._database.path());
        std::env::set_var("GIT_CONFIG_COUNT", "1");
        std::env::set_var("GIT_CONFIG_KEY_0", "user.name");
        std::env::set_var("GIT_CONFIG_VALUE_0", "Git Person");
        fixture
            .store
            .upsert_provider_token(&crate::store::ProviderToken {
                provider: "linear".into(),
                access_token: "fixture".into(),
                refresh_token: None,
                oauth_client_id: None,
                expires_at: None,
                login: None,
                updated_at: 1,
                credential_type: crate::store::CredentialType::ApiKey,
            })
            .await
            .unwrap();
        for (configured, participant, expected) in [
            (Some("Configured Person"), "", "Configured Person"),
            (None, "  ", "Git Person"),
            (Some("Configured Person"), "Caller", "Caller"),
        ] {
            std::env::set_var("LF_USER_NAME", participant);
            let config = configured
                .map(|name| format!("user:\n  name: {name}\n"))
                .unwrap_or_default();
            std::fs::write(fixture._database.path().join("config.yaml"), config).unwrap();
            let (url, requests) = crate::pm::test_server::spawn(vec![
                crate::pm::test_server::json_response(
                    axum::http::StatusCode::OK,
                    serde_json::json!({"data":{"commentCreate":{"comment":{"id":"posted"}}}}),
                ),
                crate::pm::test_server::json_response(
                    axum::http::StatusCode::OK,
                    serde_json::json!({"data":{"issue":{
                        "updatedAt":"2026-09-26T00:00:00Z", "title":fixture.task.plan.title,
                        "description":fixture.task.plan.description,
                        "comments":{"nodes":[],"pageInfo":{"hasNextPage":false,"endCursor":null}}
                    }}}),
                ),
            ])
            .await;
            let posted = crate::ops::pm::PM_TEST_CONTEXT
                .scope(
                    crate::ops::pm::PmTestContext {
                        path: fixture.database_path.clone(),
                        store: fixture.store.clone(),
                        graphql_url: url,
                    },
                    crate::ops::linear_observe::publish_task_steer(
                        &fixture.store,
                        &fixture.task,
                        "Keep going",
                    ),
                )
                .await
                .unwrap();
            assert_eq!(posted, "posted");
            let requests = requests.lock().await;
            let publication: serde_json::Value = serde_json::from_str(&requests[0].body).unwrap();
            let body = publication["variables"]["body"].as_str().unwrap();
            assert!(body.contains("Keep going"));
            assert!(body.contains("<!-- loopflow-steer:"));
            assert!(
                body.contains(&format!("<!-- loopflow-requester:\"{expected}\" -->")),
                "{body}"
            );
            assert_eq!(
                crate::ops::linear_observe::comment_requester(body, Some("Publisher")),
                Some(expected.to_string())
            );
        }
    }

    #[tokio::test]
    async fn stacking_is_idempotent_and_preserves_history() {
        let _ledger = crate::journal::TestLedgerGuard::new();
        let repo = loopflow_test_support::TestRepo::new();
        let fixture = task_fixture_at("STACK-1", repo.path().canonicalize().unwrap()).await;
        let store = &fixture.store;
        let child = store
            .active_task_pr(&fixture.task.id)
            .await
            .unwrap()
            .unwrap();
        let mut parent_task = fixture.task.clone();
        parent_task.id = TaskId::new();
        parent_task.plan.id = LinearIssueId::new("stack-parent-issue").unwrap();
        parent_task.plan.identifier = "STACK-2".into();
        parent_task.worktree = repo.path().join("parent");
        parent_task.workspace_slug = "stack-parent".into();
        let mut parent = child.clone();
        parent.id = TaskPrId::new();
        parent.task_id = parent_task.id.clone();
        parent.branch = "test/stack-parent".into();
        parent.slug = "stack-parent".into();
        store.create_task(&parent_task, &parent).await.unwrap();
        parent.publication = Some(PrPublication {
            requested_at: parent.created_at,
            presentation: None,
            github: Some(GithubPr {
                number: 42,
                url: "https://github.com/fixture/repo/pull/42".into(),
                head_sha: None,
            }),
            merge: None,
        });
        store.update_task_pr(&parent).await.unwrap();
        let events = store.task_events_after(&fixture.task.id, 0).await.unwrap();
        store.stack_task_pr(&child, &parent.id).await.unwrap();
        store.stack_task_pr(&child, &parent.id).await.unwrap();
        assert_eq!(
            store.task_events_after(&fixture.task.id, 0).await.unwrap(),
            events
        );
        // Both self-parenting and a reverse edge preserve the accepted dependency.
        assert!(store.stack_task_pr(&parent, &parent.id).await.is_err());
        let mut published_child = store
            .active_task_pr(&fixture.task.id)
            .await
            .unwrap()
            .unwrap();
        published_child.publication = parent.publication.clone();
        store.update_task_pr(&published_child).await.unwrap();
        assert!(store
            .stack_task_pr(&parent, &child.id)
            .await
            .unwrap_err()
            .to_string()
            .contains("cycle"));
        assert_eq!(store.get_task_pr(&parent.id).await.unwrap(), Some(parent));
        assert_eq!(
            store.get_task_pr(&child.id).await.unwrap(),
            Some(published_child)
        );
    }

    #[tokio::test]
    async fn task_preparation_rejects_invalid_inputs_before_allocating_placement() {
        let repo = loopflow_test_support::TestRepo::new();
        for (options, message) in [
            (
                super::TaskExecOptions {
                    name: Some("bad.name".into()),
                    ..Default::default()
                },
                "kebab-case",
            ),
            (
                super::TaskExecOptions {
                    directive: Some("  ".into()),
                    ..Default::default()
                },
                "directive cannot be empty",
            ),
        ] {
            let error = super::prepare_new_task(repo.path(), "New task", None, &options)
                .await
                .unwrap_err();
            assert!(error.to_string().contains(message), "{error}");
        }
        assert_eq!(
            crate::engine::worktrees::list_worktrees(repo.path())
                .unwrap()
                .len(),
            1
        );
    }

    #[tokio::test]
    async fn task_preparation_preserves_occupied_placement_and_resolves_base_without_creating() {
        let repo = loopflow_test_support::TestRepo::new();
        let options = super::TaskExecOptions {
            name: Some("existing-task".into()),
            ..Default::default()
        };
        let planned = super::prepare_new_task(repo.path(), "New task", None, &options)
            .await
            .unwrap();
        assert_eq!(planned.plan.base_ref, repo.head_sha());
        assert!(!planned.plan.worktree_path.exists());
        std::fs::create_dir_all(&planned.plan.worktree_path).unwrap();
        let authored = planned.plan.worktree_path.join("authored.txt");
        std::fs::write(&authored, "retain these bytes").unwrap();
        let result = super::prepare_new_task(repo.path(), "New task", None, &options).await;
        assert!(result.unwrap_err().to_string().contains("already exists"));
        assert_eq!(
            std::fs::read_to_string(&authored).unwrap(),
            "retain these bytes"
        );
        std::fs::remove_dir_all(planned.plan.worktree_path).unwrap();
    }

    #[tokio::test]
    async fn task_checkout_pins_upstream_without_requiring_clean_canonical_main() {
        let repo = loopflow_test_support::TestRepo::new();
        let upstream = repo.head_sha();
        repo.create_file("local.txt", "unpublished main work");
        repo.stage_all();
        repo.commit("Local main work");
        repo.create_branch("local-feature");
        repo.create_file("dirty.txt", "uncommitted work");
        let prepared = super::prepare_new_task(
            repo.path(),
            "Clean task",
            None,
            &super::TaskExecOptions::default(),
        )
        .await
        .unwrap();
        assert_eq!(prepared.plan.base_ref, upstream);
        assert_eq!(
            crate::engine::git::current_branch(repo.path())
                .unwrap()
                .as_deref(),
            Some("local-feature")
        );
        assert_eq!(
            std::fs::read_to_string(repo.path().join("dirty.txt")).unwrap(),
            "uncommitted work"
        );
        assert!(!prepared.plan.worktree_path.exists());
    }

    #[tokio::test]
    async fn task_preparation_keeps_the_resolved_base_when_the_remote_ref_advances() {
        let repo = loopflow_test_support::TestRepo::new();
        let prepared = super::prepare_new_task(
            repo.path(),
            "Pinned base",
            None,
            &super::TaskExecOptions::default(),
        )
        .await
        .unwrap();
        let original = repo.head_sha();
        repo.create_file("later.txt", "arrived while Linear was creating the issue");
        repo.stage_all();
        repo.commit("Advance the remote after preparation");
        repo.push();
        assert_ne!(
            crate::engine::git::rev_parse(repo.path(), "origin/main").unwrap(),
            original
        );

        let placement =
            crate::engine::worktrees::create_from_placement_plan(repo.path(), &prepared.plan)
                .unwrap();
        let placed_head = crate::engine::git::rev_parse(&placement.path, "HEAD").unwrap();
        let has_later_file = placement.path.join("later.txt").exists();
        std::fs::remove_dir_all(&placement.path).unwrap();

        assert_eq!(placed_head, original);
        assert_eq!(placed_head, prepared.plan.base_ref);
        assert!(!has_later_file);
    }

    #[tokio::test]
    async fn task_preparation_refuses_an_unresolvable_base_before_allocating_placement() {
        let repo = loopflow_test_support::TestRepo::new();
        let output = std::process::Command::new("git")
            .current_dir(repo.path())
            .args([
                "remote",
                "set-url",
                "origin",
                "/nonexistent/loopflow-fixture-remote",
            ])
            .output()
            .unwrap();
        assert!(output.status.success());
        let error = super::prepare_new_task(
            repo.path(),
            "New task",
            None,
            &super::TaskExecOptions {
                name: Some("missing-base".into()),
                ..Default::default()
            },
        )
        .await
        .unwrap_err();
        assert!(
            error.to_string().contains("failed to fetch task base"),
            "{error}"
        );
        assert_eq!(
            crate::engine::worktrees::list_worktrees(repo.path())
                .unwrap()
                .len(),
            1
        );
    }

    #[tokio::test]
    #[allow(clippy::await_holding_lock)] // Serializes the isolated registry environment.
    async fn task_preparation_rejects_unpublished_parent_before_allocating_child() {
        let _lock = crate::journal::test_env_lock();
        let names = ["LF_HOME", "LF_ACCOUNT_LEASE"];
        let _restore = EnvRestore::capture(&names);
        for name in names {
            std::env::remove_var(name);
        }
        let repo = loopflow_test_support::TestRepo::new();
        let fixture = task_fixture_at("FIX-1", repo.path().canonicalize().unwrap()).await;
        std::env::set_var("LF_HOME", fixture._database.path());
        let result = super::prepare_new_task(
            repo.path(),
            "Child",
            None,
            &super::TaskExecOptions {
                name: Some("child-task".into()),
                stack_on: Some("FIX-1".into()),
                ..Default::default()
            },
        )
        .await;
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("open the parent PR"));
        let retained = fixture
            .store
            .get_task(&fixture.task.id)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(retained.worktree, fixture.task.worktree);
        assert_eq!(fixture.store.list_tasks(None).await.unwrap().len(), 1);
        assert_eq!(
            fixture
                .store
                .task_prs(&fixture.task.id)
                .await
                .unwrap()
                .len(),
            1
        );
    }

    #[test]
    fn execution_boundary_resolves_linked_git_and_control_roots() {
        let _env_lock = crate::journal::test_env_lock();
        let _restore = EnvRestore::capture(&["LF_BIN", "LF_HOME"]);
        let directory = tempfile::tempdir().unwrap();
        let main = directory.path().join("repo");
        let worktree = directory.path().join("repo.task");
        std::fs::create_dir(&main).unwrap();
        for args in [
            vec!["init", "-b", "main"],
            vec!["config", "user.email", "test@example.com"],
            vec!["config", "user.name", "Loopflow Test"],
        ] {
            assert!(std::process::Command::new("git")
                .current_dir(&main)
                .args(args)
                .status()
                .unwrap()
                .success());
        }
        std::fs::write(main.join("README.md"), "proof\n").unwrap();
        for args in [vec!["add", "."], vec!["commit", "-m", "proof"]] {
            assert!(std::process::Command::new("git")
                .current_dir(&main)
                .args(args)
                .status()
                .unwrap()
                .success());
        }
        assert!(std::process::Command::new("git")
            .current_dir(&main)
            .args(["worktree", "add", "-b", "task", worktree.to_str().unwrap()])
            .status()
            .unwrap()
            .success());
        let control = directory.path().join("control");
        std::fs::create_dir(&control).unwrap();
        std::env::set_var("LF_BIN", std::env::current_exe().unwrap());
        std::env::set_var("LF_HOME", &control);

        let boundary = checkout_execution_boundary(&worktree, "codex").unwrap();

        assert_eq!(boundary.writable_roots.len(), 2);
        assert!(boundary
            .writable_roots
            .contains(&main.join(".git").canonicalize().unwrap()));
        assert!(boundary.writable_roots.contains(&control));
    }

    #[tokio::test]
    async fn nonresumable_execution_blocker_remains_a_launch_refusal() {
        let TaskFixture { store, task, .. } = task_fixture("TEST-BOUNDARY").await;
        let blocker = "Task execution boundary is blocked: linked Git index.lock is not writable";
        store
            .append_task_event(
                &task.id,
                &TaskEventKind::Failed {
                    error: blocker.to_string(),
                    resumable: false,
                },
            )
            .await
            .unwrap();
        let settled = store.latest_task_event(&task.id).await.unwrap().unwrap();

        let event = store.latest_task_event(&task.id).await.unwrap().unwrap();
        assert_eq!(event, settled);
        assert_eq!(task_event_exec_refusal(Some(&event)), Some(blocker));
        assert!(matches!(
            event.kind,
            TaskEventKind::Failed {
                resumable: false,
                ..
            }
        ));
    }

    #[test]
    fn pr_mutation_lock_refuses_a_concurrent_writer() {
        let repo = tempfile::tempdir().expect("temporary repository");
        let status = std::process::Command::new("git")
            .args(["init", "--quiet"])
            .current_dir(repo.path())
            .status()
            .expect("initialize repository");
        assert!(status.success());

        let _first = lock_task_pr_mutation(repo.path()).expect("first mutation lock");
        let error = lock_task_pr_mutation(repo.path()).expect_err("second writer must be refused");
        assert!(error.to_string().contains("already running"));
    }
}
