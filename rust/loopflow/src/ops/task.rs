pub(crate) mod remote;
mod workspace;
pub use workspace::{task_shell, watch_task_files, TaskFilesFrame};
mod planning_explain;
pub use planning_explain::{
    explain_task_planning, TaskPlanningAction, TaskPlanningExplanation, TaskPlanningRequest,
};
mod checkout_explain;
pub use checkout_explain::{
    explain_task_checkout, TaskCheckoutAction, TaskCheckoutBehavior, TaskCheckoutExplanation,
};
mod explain;
pub use explain::{explain_task_run, TaskRunAction, TaskRunExplanation};
mod directory;
mod handoff;
mod lifecycle;
#[cfg(test)]
mod lifecycle_tests;
pub(crate) use lifecycle::{cleanup_completed_task, notice_retained_task, record_abandoned_pr};
pub use lifecycle::{task_abandon, task_delete, task_repository, task_sweep};
mod file_save;
pub mod ship;
pub use directory::{task_files, TaskDirectory, TaskFileEntry, TaskFileKind};
pub use file_save::{task_save, TaskFileRecovery, TaskFileSave};

use crate::work::task::comment::TaskComments;
use std::collections::BTreeMap;
use std::fs::{File, OpenOptions};
use std::io::Read;
use std::path::{Component, Path, PathBuf};
use std::process::Command;
use std::sync::Arc;
use std::time::{Duration, Instant};

use crate::agent::checkout_execution_boundary;
use crate::child::ChildRef;
use crate::config::{load_config_or_default, parse_agent};
use crate::durable::{TaskState, WorkStatus};
use crate::git::worktree_name::WorktreeName;
use crate::git::worktrees::{
    plan_branch_placement, PlacementPlan, PlacementStrategy, WorktreeSegment,
};
use crate::git::{
    current_branch, fetch, get_default_branch, is_clean, merge_base, ref_exists, rev_parse,
};
use crate::naming::git_user;
use crate::ops::error::{OpsError, OpsResult};
use crate::ops::task_actions::{derive_task_actions, TaskActionEvidence, TaskActionModel};
use crate::store::sqlite::{SqliteStore, TaskCheckout};
use crate::store::{
    open_existing_store, open_registry_for_authority, RegistryUnavailable, SharedStore, Store,
};
use crate::work::task::{
    CiCheck, CiObservation, CiState, GithubObservation, GithubObservationResult, GithubPr,
    Observation, PrMergeMode, PrMergeRequest, PrPhase, PrPresentation, PrPublication, Task,
    TaskEventKind, TaskPr, TaskPrId,
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
pub struct TaskProcessOptions {
    pub wave: Option<String>,
    pub reason: Option<String>,
    pub agent: Option<String>,
    pub name: Option<String>,
    pub graph: Option<String>,
    /// The step `lf task run --from` starts the Task's Graph at.
    pub from: Option<String>,
    pub stack_on: Option<String>,
    pub directive: Option<String>,
    /// An existing branch the Task takes as its own, with the checkout that has it.
    pub branch: Option<String>,
}

impl TaskProcessOptions {
    fn validate_directive(&self, existing: Option<&Task>) -> OpsResult<()> {
        if let Some(text) = &self.directive {
            if text.trim().is_empty() {
                return Err(task_error("directive cannot be empty"));
            }
            if let Some(task) = existing {
                return Err(task_error(format!(
                    "Task {} already exists; use `lf task comment {} <new-direction>`",
                    task.identifier, task.identifier
                )));
            }
        }
        Ok(())
    }

    fn validate_workspace_name(&self, task: &Task) -> OpsResult<()> {
        if let Some(name) = &self.name {
            let requested = WorktreeSegment::parse(name).map_err(task_error)?;
            if task.worktree.is_some() && requested.as_str() != task.workspace_slug {
                return Err(task_error(format!(
                    "Task {} already uses workspace name {:?}",
                    task.identifier, task.workspace_slug
                )));
            }
        }
        Ok(())
    }
}

#[derive(Debug, Clone, Default)]
pub struct TaskCheckoutOptions {
    pub wave: Option<String>,
    pub name: Option<String>,
    pub stack_on: Option<String>,
    pub directive: Option<String>,
    pub design: Option<PathBuf>,
    pub recover_from: Option<String>,
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
    pub machine_id: Option<crate::durable::MachineId>,
    pub issue_id: Option<String>,
    pub issue_identifier: String,
    pub task_id: String,
    pub external_project_id: Option<String>,
    pub project: Option<String>,
    pub pm_snapshot_synced_at: Option<i64>,
    pub pm_writeback: crate::work::task::LinearWritebackState,
    pub wave: Option<String>,
    pub project_id: Option<String>,
    pub repository_id: String,
    /// Completion and abandonment are independent of the Task's Graph position.
    pub status: TaskState,
    /// A durable completion request awaiting settlement.
    pub completion_pending: Option<String>,
    pub execution: crate::ops::task_execution::TaskExecutionSnapshot,
    pub work: crate::task_work::TaskWork,
    pub worktree: Option<String>,
    pub workspace_slug: String,
    pub branch: Option<String>,
    pub base_commit: Option<String>,
    pub agent: Option<String>,
    pub provider: String,
    pub pr: Option<TaskPr>,
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

impl<'a> From<&'a TaskCheckout> for TaskWorkspace<'a> {
    fn from(checkout: &'a TaskCheckout) -> Self {
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
    fn new(task: &'a Task) -> OpsResult<Self> {
        Ok(Self {
            checkout: TaskWorkspace {
                issue_identifier: &task.identifier,
                task_id: &task.id,
                worktree: task.worktree()?,
            },
            base_commit: &task.base_commit,
        })
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

// File access reads recorded checkout location without reconciling PR or execution state.
fn file_store() -> OpsResult<SqliteStore> {
    #[cfg(test)]
    let test_path = super::linear::LINEAR_TEST_CONTEXT
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
    SqliteStore::open_read_only(&path)
        .map_err(|error| task_error(format!("cannot read Task registry: {error}")))
}

fn file_context(store: &SqliteStore, issue: &str) -> OpsResult<TaskCheckout> {
    let task_id = store
        .resolve_task_id(issue, None)
        .map_err(task_error)?
        .ok_or_else(|| task_error(format!("no Task exists for {issue:?}")))?;
    let checkout = store
        .task_checkout(&task_id)
        .map_err(|error| task_error(format!("cannot read Task checkout: {error}")))?
        .ok_or_else(|| task_error(format!("Task {issue:?} has no checkout")))?;
    let machine = checkout.machine_id.as_ref().ok_or_else(|| {
        task_error("Task checkout Machine is unknown; its files cannot be located")
    })?;
    if *machine != store.local_machine().map_err(task_error)?.id {
        return Err(task_error(format!(
            "Task checkout belongs to Machine {machine}; read its files on that Machine"
        )));
    }
    Ok(checkout)
}

fn comparison_context(issue: &str, base: &str) -> OpsResult<(TaskCheckout, String)> {
    let store = file_store()?;
    let checkout = file_context(&store, issue)?;
    let task = store
        .task(&checkout.task_id)
        .map_err(|error| task_error(format!("failed to read Task comparison base: {error}")))?
        .ok_or_else(|| task_error("Task no longer exists"))?;
    let base = resolve_file_base(&checkout.worktree, &task.base_commit, base)?;
    Ok((checkout, base))
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
    if let Ok(store) = super::linear::LINEAR_TEST_CONTEXT.try_with(|context| context.store.clone())
    {
        return Ok(store);
    }
    open_existing_store()
        .await
        .map(Arc::new)
        .ok_or_else(|| task_error("no Loopflow registry on this machine; run `lf doctor`"))
}

/// Durable placement for a Task checkout forked from another Task's PR.
#[derive(Debug, Clone)]
pub struct StackedSync {
    pub fork_base: String,
    pub child: Task,
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
        let Some(parent_id) = task.parent_pr_id.clone() else {
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
            fork_base: task.base_commit.clone(),
            child: *task,
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
    let task_id = stacked.child.id.clone();
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
            .sync_task_placement(
                &task_id,
                &new_base,
                clear_parent,
                time::OffsetDateTime::now_utc(),
            )
            .await
            .map_err(task_error)?;
        Ok(())
    })
}

async fn owning_wave(store: &SharedStore, task: &Task) -> OpsResult<Option<Wave>> {
    let Some(id) = &task.wave_id else {
        return Ok(None);
    };
    store
        .get_wave(id)
        .await
        .map_err(task_error)?
        .map(Some)
        .ok_or_else(|| task_error(format!("owning Wave {id} is not registered")))
}

pub(crate) fn repository_for_task(store: &Store, task: &Task) -> OpsResult<PathBuf> {
    store
        .sqlite
        .repository_path(&task.repository_id)
        .map_err(task_error)?
        .map(PathBuf::from)
        .ok_or_else(|| task_error(format!("Task {} repository is unavailable", task.id)))
}

pub(crate) async fn task_work_status(store: &Store, task: &Task) -> OpsResult<WorkStatus> {
    let work = store
        .work_for_child(&ChildRef::Task(task.id.clone()))
        .await
        .map_err(task_error)?;
    store.work_status(&work).await.map_err(task_error)
}

/// What `lf task run` set out to do once the Task was placed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TaskRunStart {
    /// A run of the Task's Graph from this step; the caller starts it as
    /// `lf --task ISSUE graph run`.
    Run(String),
    /// The Task now waits at this interactive step; nothing runs.
    Waiting(String),
    /// Nothing remained: the Task reached `end`.
    Ended,
}

/// Place the Task and fill what a run leaves unsaid: `agent` as the Task's
/// agent, `reason` as a steer, and where the run begins. A Task keeps the
/// graph it took up; naming another takes that one up, and a Task with none
/// takes up its Wave's, its repository's, or `code`.
pub fn task_place(
    repo: &Path,
    issue: &str,
    options: TaskProcessOptions,
    mode: TaskRunMode,
) -> OpsResult<(Task, TaskRunStart)> {
    let TaskProcessOptions {
        graph,
        from,
        agent,
        reason,
        ..
    } = options.clone();
    let source = remote::source_for_task(issue)?;
    let mut task = prepare_task(repo, issue, options, None, source)?;
    block_on_task(async {
        let store = task_store().await?;
        let reason = reason
            .as_deref()
            .map(str::trim)
            .filter(|reason| !reason.is_empty());
        store
            .sqlite
            .require_task_launch(&task.id)
            .map_err(task_error)?;
        let start =
            start_task_run(&store, &mut task, graph.as_deref(), from.as_deref(), mode).await?;
        select_task_agent(&store, &mut task, agent.as_deref()).await?;
        if let Some(reason) = reason {
            append_task_comment(&store, &task, reason, true)?;
        }
        Ok((task, start))
    })
}

/// How a Task run meets its interactive steps.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TaskRunMode {
    /// Stop before the next one; the Task waits there in its conversation.
    Stop,
    /// Run each with a person in this terminal (`-i`).
    InPlace,
    /// Run each headless, standing in for the person (`-b`, `lf ship`).
    Headless,
}

impl TaskRunMode {
    pub fn runs_through(self) -> bool {
        self != Self::Stop
    }
}

/// Where `lf task run` begins, chosen the same way for launch and
/// explanation without writing anything.
#[derive(Debug)]
pub(crate) struct TaskRunSelection {
    pub graph: crate::task_graph::CapturedGraph,
    /// Whether the Task takes `graph` up first: it has none, or named another.
    pub take_up: bool,
    /// Where the run begins; `None` when nothing remains before `end`.
    pub start: Option<crate::task_graph::Place>,
}

impl TaskRunSelection {
    /// Whether the run reaches `end`: it runs through interactive steps, or
    /// none lies ahead. Such a run hands the Task off, granting merge authority.
    pub fn reaches_end(&self, mode: TaskRunMode) -> bool {
        self.start
            .as_ref()
            .is_none_or(|place| mode.runs_through() || self.graph.next_interactive(place).is_none())
    }
}

/// The graph a Task with none takes up: its Wave's, its repository's, else
/// `code`. A Task need not exist yet; the scope it would be filed in decides.
pub(crate) fn task_graph_default(
    store: &Store,
    wave: Option<&crate::id::WaveId>,
    repo: &Path,
) -> OpsResult<(String, &'static str)> {
    if let Some(wave) = wave {
        if let Some(name) = store.sqlite.selected_wave_graph(wave).map_err(task_error)? {
            crate::task_graph::validate_graph_name(&name).map_err(task_error)?;
            return Ok((name, "wave"));
        }
    }
    let name = crate::config::load_repo_config(repo)
        .map_err(task_error)?
        .and_then(|c| c.graph);
    if let Some(name) = name {
        crate::task_graph::validate_graph_name(&name).map_err(task_error)?;
        return Ok((name, "repository"));
    }
    Ok((DEFAULT_TASK_GRAPH.into(), "builtin"))
}

/// The graph a Task takes up when nothing else selects one.
pub const DEFAULT_TASK_GRAPH: &str = "code";

pub(crate) fn select_task_run(
    store: &SharedStore,
    task: &Task,
    checkout: &Path,
    requested: Option<&str>,
    from: Option<&str>,
) -> OpsResult<TaskRunSelection> {
    use crate::task_graph::{CapturedGraph, END, START};
    let issue = &task.identifier;
    if store.sqlite.task_state(&task.id).map_err(task_error)? == crate::durable::TaskState::Deferred
    {
        return Err(task_error(format!(
            "Task is deferred; use `lf task resume {issue}` before running a Graph"
        )));
    }
    let current = store.sqlite.task_graph_row(&task.id).map_err(task_error)?;
    let load = |name: &str, source: &str, repo: &Path| {
        CapturedGraph::load(name, repo)
            .map_err(task_error)?
            .ok_or_else(|| {
                let available = crate::graph::available_graph_names(repo)
                    .map(|names| names.join(", "))
                    .unwrap_or_default();
                task_error(format!(
                    "{source}{}. Available: {available}",
                    crate::task_graph::missing_graph(name)
                ))
            })
    };
    let (graph, row) = match (current, requested) {
        (Some(row), None) => (row.graph.clone(), Some(row)),
        (Some(row), Some(name)) if name == row.graph.name => (row.graph.clone(), Some(row)),
        (_, Some(name)) => (load(name, "", checkout)?, None),
        (None, None) => {
            let repo = repository_for_task(store, task)?;
            let (name, source) = task_graph_default(store, task.wave_id.as_ref(), &repo)?;
            (load(&name, &format!("{source} default "), &repo)?, None)
        }
    };
    let take_up = row.is_none();
    if let Some((_, process)) = row.as_ref().and_then(|row| row.run.as_ref()) {
        if process.completed_at.is_none() && store.sqlite.process_may_run(process) {
            return Err(task_error(format!(
                "Task {issue} is running graph {} from {}; read `lf task status {issue}`, or `lf task interrupt {issue}` to stop it",
                graph.name, row.as_ref().expect("a run belongs to a row").step
            )));
        }
    }
    let step = row.as_ref().map_or(START, |row| row.step.as_str());
    let start = match (from, row.as_ref().and_then(|row| row.run.as_ref())) {
        (Some(name), _) => Some(graph.find(name).ok_or_else(|| {
            task_error(format!(
                "graph {} has no step {name:?}. Steps: {}",
                graph.name,
                graph.describe_steps()
            ))
        })?),
        // A run that stopped starts again from where it began.
        (None, Some((cursor, _))) => graph.place_of(cursor),
        (None, None) if step == START => graph.find(START),
        (None, None) if step == END => {
            return Err(task_error(format!(
                "Task {issue} is at the end of graph {}; `lf task run {issue} --from <step>` runs part of it again, `lf task run {issue} <graph>` takes up another. Steps: {}",
                graph.name,
                graph.describe_steps()
            )))
        }
        (None, None) => {
            let place = graph.interactive_step(step).ok_or_else(|| {
                task_error(format!(
                    "Task {issue} is at {step:?}, which graph {} no longer names; `lf task run {issue} --from <step>` chooses where to go on. Steps: {}",
                    graph.name,
                    graph.describe_steps()
                ))
            })?;
            graph.after(&place)
        }
    };
    Ok(TaskRunSelection {
        graph,
        take_up,
        start,
    })
}

/// Put the Task where `lf task run` sets out: on a run carried by this
/// process, at the interactive step `--from` names, or at `end` when nothing
/// remains.
async fn start_task_run(
    store: &SharedStore,
    task: &mut Task,
    requested: Option<&str>,
    from: Option<&str>,
    mode: TaskRunMode,
) -> OpsResult<TaskRunStart> {
    let through = mode.runs_through();
    let selection = select_task_run(store, task, task.worktree()?, requested, from)?;
    let ships = selection.reaches_end(mode);
    let TaskRunSelection {
        graph,
        take_up,
        start,
    } = selection;
    let issue = task.identifier.clone();
    let process = crate::journal::current_lf_process_id()
        .ok_or_else(|| task_error("a Task run requires a registered Process"))?;
    if take_up {
        store
            .sqlite
            .take_up_task_graph(&task.id, &graph)
            .map_err(task_error)?;
    }
    let row = store
        .sqlite
        .task_graph_row(&task.id)
        .map_err(task_error)?
        .ok_or_else(|| task_error(format!("Task {issue} lost its graph while starting")))?;
    let Some(start) = start else {
        if store
            .sqlite
            .end_task_graph(&task.id, &process)
            .map_err(task_error)?
        {
            settle_pending_completion(store, task).await?;
        }
        return Ok(TaskRunStart::Ended);
    };
    // Starting at an interactive step without running past it puts the Task there.
    if let Some(step) = graph.interactive_at(&start).filter(|_| !through) {
        store
            .sqlite
            .set_task_graph_step(&task.id, step)
            .map_err(task_error)?;
        return Ok(TaskRunStart::Waiting(step.to_owned()));
    }
    let label = if start == crate::task_graph::Place::default() {
        format!("{} from start", graph.name)
    } else {
        format!("{} from {}", graph.name, graph.step_name(&start))
    };
    if !store
        .sqlite
        .start_task_graph_run(&task.id, &row, &start.cursor(), &process, ships)
        .map_err(task_error)?
    {
        return Err(task_error(format!(
            "Task {issue} moved on graph {} while this run was starting; read `lf task status {issue}` and run again",
            graph.name
        )));
    }
    Ok(TaskRunStart::Run(label))
}

/// The run this process carried ended; settle a completion its arrival at
/// `end` requested. A failed request is retried alone with `lf task complete`.
pub fn task_run_finished(task: &Task) -> OpsResult<()> {
    block_on_task(async {
        let store = task_store().await?;
        settle_pending_completion(&store, &mut task.clone()).await
    })
}

async fn settle_pending_completion(store: &SharedStore, task: &mut Task) -> OpsResult<()> {
    if let Some((request, _)) = store
        .sqlite
        .task_completion_pending(&task.id)
        .map_err(task_error)?
    {
        settle_completion(store, task, request).await?;
    }
    Ok(())
}

pub fn task_checkout(repo: &Path, issue: &str, options: TaskCheckoutOptions) -> OpsResult<Task> {
    let design = options
        .design
        .as_deref()
        .map(|path| handoff::read_design(repo, path))
        .transpose()?;
    let task = prepare_task(
        repo,
        issue,
        TaskProcessOptions {
            wave: options.wave,
            name: options.name,
            stack_on: options.stack_on,
            directive: options.directive,
            ..Default::default()
        },
        options.recover_from.as_deref(),
        remote::source_for_task(issue)?,
    )?;
    if let Some(design) = design {
        handoff::write_design(&task, &design)?;
    }
    Ok(task)
}

/// Check out a Task that moved here on its carried branch and commit.
pub(crate) fn check_out_moved_task(repo: &Path, source: remote::TaskSource) -> OpsResult<Task> {
    let issue = source.task_id.to_string();
    prepare_task(
        repo,
        &issue,
        TaskProcessOptions::default(),
        None,
        Some(source),
    )
}

/// `source` is the branch and commit another Machine pushed for this Task.
fn prepare_task(
    repo: &Path,
    issue: &str,
    options: TaskProcessOptions,
    recover_from: Option<&str>,
    source: Option<remote::TaskSource>,
) -> OpsResult<Task> {
    options.validate_directive(None)?;
    block_on_task(async {
        let store = task_store().await?;
        if let Some(source) = &source {
            source.require_pushed(repo)?;
        }
        let main = crate::git::worktrees::main_repo_root(repo).map_err(task_error)?;
        let saved = super::planning_sync::find_task(&store, &main.to_string_lossy(), issue).await?;
        let acquired = saved.is_none();
        let task = match saved {
            Some(task) => task,
            None => match crate::config::load_config(Some(&main))
                .map_err(task_error)?
                .unwrap_or_default()
                .planning_transport()
            {
                crate::config::PlanningConfig::Linear { .. } => {
                    super::linear::acquire_task(&main, issue, super::linear::LinearRefresh::Auto)
                        .await?
                }
                crate::config::PlanningConfig::Git { .. } => {
                    return Err(task_error(format!(
                        "Task {issue:?} not found in selected Git planning"
                    )));
                }
                remote => {
                    return Err(task_error(format!(
                        "Task {issue:?} not found in Loopflow planning; {} tasks are not acquired yet",
                        remote.remote_name().unwrap_or("provider")
                    )));
                }
            },
        };
        let destination = store
            .task_execution_route(&task.id)
            .await
            .map_err(task_error)?;
        if destination.machine_id != store.local_machine().await.map_err(task_error)?.id {
            return Err(task_error(format!(
                "Task {} is owned on Machine {}; launch it through `lf --task {}` so the command reaches its owner",
                task.identifier, destination.machine_id, task.id,
            )));
        }
        let wave = owning_wave(&store, &task).await?;
        let canonical = crate::repository::CanonicalRepo::discover(&main).map_err(task_error)?;
        if crate::repository::CanonicalRepo::discover(&repository_for_task(&store, &task)?)
            .map_err(task_error)?
            != canonical
        {
            return Err(task_error("Task belongs to another repository"));
        }
        if options
            .wave
            .as_deref()
            .is_some_and(|expected| Some(expected) != wave.as_ref().map(Wave::slug))
        {
            return Err(task_error(format!(
                "requested Wave does not own Task {}",
                task.identifier
            )));
        }
        validate_task_preparation(&store, &task).await?;
        options.validate_directive((!acquired).then_some(&task))?;
        options.validate_workspace_name(&task)?;
        if recover_from.is_some() && task.worktree.is_none() {
            return Err(task_error(
                "Recovery needs the Task's recorded checkout and branch",
            ));
        }
        let task = if task.worktree.is_none() {
            if let Some(graph) = options.graph.as_deref() {
                if crate::task_graph::CapturedGraph::load(graph, repo)
                    .map_err(task_error)?
                    .is_none()
                {
                    return Err(task_error(format!("graph {graph:?} is not defined")));
                }
            }
            // A retained provider branch may already own Git work. It is metadata,
            // not a reason to reacquire planning or allocate another Task.
            let branch = options
                .branch
                .clone()
                .or(source.as_ref().map(|source| source.branch.clone()))
                .or(store
                    .sqlite
                    .task_issue_branch(&task.id)
                    .map_err(task_error)?);
            let placement =
                prepare_task_placement(&store, &main, &task, branch.as_deref(), &options).await?;
            let task = place_prepared_task(&store, &task.id, placement).await?;
            // The checkout claimed the Task; Linear learns who took it.
            let repository = repository_for_task(&store, &task)?;
            super::task_ownership::assert_assignee(&store.sqlite, &repository, &task.id).await;
            task
        } else {
            if let Some(parent) = options.stack_on.as_deref() {
                stack_existing_task(&store, &task, parent).await?;
            }
            restore_task_checkout(&store, &task, recover_from).await?;
            task
        };
        if let Some(source) = &source {
            source.require_checkout(&task)?;
        }
        Ok(task)
    })
}

async fn validate_task_preparation(store: &SharedStore, task: &Task) -> OpsResult<()> {
    validate_preparation_status(task, task_work_status(store, task).await?)?;
    store
        .sqlite
        .validate_task_planning(task)
        .map_err(task_error)?;
    Ok(())
}

fn validate_preparation_status(task: &Task, status: WorkStatus) -> OpsResult<()> {
    match status {
        WorkStatus::Done => Err(task_error(format!(
            "Task {} is done; `lf task reopen {}` reopens planning without moving its Graph",
            task.identifier, task.identifier
        ))),
        WorkStatus::Abandoned => Err(task_error(format!(
            "Task {} is abandoned; inspect its retained history with `lf task status {}`",
            task.identifier, task.identifier
        ))),
        WorkStatus::Ready => Ok(()),
    }
}

async fn restore_task_checkout(
    store: &SharedStore,
    task: &Task,
    recover_from: Option<&str>,
) -> OpsResult<()> {
    let repo = crate::git::worktrees::main_repo_root(&repository_for_task(store, task)?)?;
    let worktree = task.worktree()?;
    let _lease = crate::git::acquire_worktree_lease(&repo, worktree, "Task checkout")?;
    let recovered = recover_from
        .map(|commit| {
            let output = git_output_bytes(
                &repo,
                &[
                    "rev-parse",
                    "--verify",
                    "--end-of-options",
                    &format!("{commit}^{{commit}}"),
                ],
            )?;
            let commit = String::from_utf8_lossy(&output).trim().to_string();
            if crate::git::worktrees::branch_exists(&repo, &task.branch)?
                && rev_parse(&repo, &task.branch)? != commit
            {
                return Err(task_error(
                    "Recovery never replaces an existing Task branch",
                ));
            }
            if worktree.join(".git").try_exists()? && rev_parse(worktree, "HEAD")? != commit {
                return Err(task_error(
                    "Recovery never replaces an existing Task checkout",
                ));
            }
            Ok(commit)
        })
        .transpose()?;
    if worktree.join(".git").try_exists()? {
        return finish_task_checkout(store, task).await;
    }
    validate_checkout_restoration(task, &repo)?;
    let mut args = vec!["worktree".to_string(), "add".into(), "--force".into()];
    // --force replaces only the stale registration at this absent exact path.
    // A different registered path was rejected above; no branch is reset.
    if !crate::git::worktrees::branch_exists(&repo, &task.branch)? {
        let remote = format!("refs/remotes/origin/{}", task.branch);
        if recovered.is_none()
            && !ref_exists(&repo, &remote)?
            && store
                .active_task_pr(&task.id)
                .await
                .map_err(task_error)?
                .is_some_and(|pr| pr.github().is_some())
        {
            fetch(&repo, "origin", &task.branch)?;
        }
        let base = if let Some(commit) = recovered {
            commit
        } else if ref_exists(&repo, &remote)? {
            remote
        } else {
            let started = store
                .task_events_after(&task.id, 0)
                .await
                .map_err(task_error)?
                .iter()
                .any(|event| {
                    matches!(
                        &event.kind,
                        TaskEventKind::CheckoutReady { .. } | TaskEventKind::PrStarted { .. }
                    )
                });
            if started {
                return Err(task_error(format!("Task branch {} is missing locally and remotely; its committed work cannot be recovered from the base alone; use --recover-from COMMIT only after locating its retained history", task.branch)));
            }
            task.base_commit.clone()
        };
        args.extend([
            "--no-track".into(),
            "-b".into(),
            task.branch.clone(),
            worktree.display().to_string(),
            base,
        ]);
    } else {
        args.extend([worktree.display().to_string(), task.branch.clone()]);
    }
    git_output_bytes(&repo, &args.iter().map(String::as_str).collect::<Vec<_>>())?;
    finish_task_checkout(store, task).await
}

fn validate_checkout_restoration(task: &Task, repo: &Path) -> OpsResult<()> {
    let worktree = task.worktree()?;
    if match worktree.symlink_metadata() {
        Ok(_) => true,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => false,
        Err(error) => return Err(error.into()),
    } {
        return Err(task_error(format!(
            "Task checkout path {} is occupied; its contents were preserved",
            worktree.display()
        )));
    }
    let checkouts = crate::git::worktrees::list_worktrees(repo)?;
    let destination = crate::store::canonicalize_with_missing_tail(worktree)?;
    for other in checkouts
        .iter()
        .filter(|entry| entry.branch.as_deref() == Some(&task.branch))
    {
        if crate::store::canonicalize_with_missing_tail(&other.path)? != destination {
            return Err(task_error(format!(
                "Task branch {} is registered at {}; preserve that checkout before restoring {}",
                task.branch,
                other.path.display(),
                worktree.display()
            )));
        }
    }
    Ok(())
}

async fn finish_task_checkout(store: &SharedStore, task: &Task) -> OpsResult<()> {
    let events = store
        .task_events_after(&task.id, 0)
        .await
        .map_err(task_error)?;
    if !events.iter().any(|event| {
        matches!(
            &event.kind,
            TaskEventKind::CheckoutReady { .. } | TaskEventKind::PrStarted { .. }
        )
    }) {
        store
            .append_task_event(
                &task.id,
                &TaskEventKind::CheckoutReady {
                    branch: task.branch.clone(),
                    base_commit: task.base_commit.clone(),
                },
            )
            .await
            .map_err(task_error)?;
    }
    Ok(())
}

async fn stack_existing_task(store: &SharedStore, task: &Task, requested: &str) -> OpsResult<()> {
    let parent = select_existing_stack_parent(store, task, requested).await?;
    let _mutation = lock_task_pr_mutation(task.worktree()?)?;
    store
        .stack_task_placement(task, &parent.id)
        .await
        .map_err(|error| task_error(error.to_string()))?;
    eprintln!(
        "Task {} selects parent PR {}. Checkout and GitHub are unchanged; run `lf sync` in {} to integrate it.",
        task.identifier, parent.id, task.worktree()?.display()
    );
    Ok(())
}

async fn select_existing_stack_parent(
    store: &SharedStore,
    task: &Task,
    requested: &str,
) -> OpsResult<TaskPr> {
    let parent_task = store
        .get_task_by_issue(requested)
        .await
        .map_err(|error| task_error(error.to_string()))?
        .ok_or_else(|| task_error(format!("stack parent {requested:?} has no Task")))?;
    // Retrying the same parent retains its historical PR identity. An explicit
    // different Task selects that Task's active PR; the store checks the graph.
    if let Some(id) = &task.parent_pr_id {
        if let Some(parent) = store.get_task_pr(id).await.map_err(task_error)? {
            if parent.task_id == parent_task.id {
                return Ok(parent);
            }
        }
    }
    store
        .active_task_pr(&parent_task.id)
        .await
        .map_err(task_error)?
        .ok_or_else(|| task_error("stack parent PR is missing"))
}

#[derive(Debug)]
struct TaskPlacement {
    plan: PlacementPlan,
    workspace_slug: String,
    stack_parent: Option<TaskPr>,
    github: Option<GithubPr>,
    requested_agent: Option<String>,
    directive: Option<String>,
}

fn plan_task_placement(
    main_repo: &Path,
    task: &Task,
    recorded_branch: Option<&str>,
    options: &TaskProcessOptions,
) -> OpsResult<(PlacementPlan, String)> {
    let explicit_segment = options
        .name
        .as_deref()
        .map(WorktreeSegment::parse)
        .transpose()
        .map_err(task_error)?;
    let title = match &explicit_segment {
        Some(segment) => segment.as_str().to_string(),
        None => task
            .title
            .split(|ch: char| !ch.is_ascii_alphanumeric())
            .filter(|word| !word.is_empty())
            .take(3)
            .collect::<Vec<_>>()
            .join("-")
            .to_ascii_lowercase()
            .chars()
            .take(32)
            .collect::<String>()
            .trim_end_matches('-')
            .to_string(),
    };
    let segment = WorktreeSegment::parse(
        format!("{}-{title}", task.identifier.to_ascii_lowercase()).trim_end_matches('-'),
    )
    .map_err(task_error)?;
    let recorded_branch = recorded_branch.filter(|branch| !branch.is_empty());
    let branch = match recorded_branch {
        Some(branch) => branch.to_string(),
        None => WorktreeName::new(git_user(main_repo)?, segment.clone())
            .expect("git_user returns a nonempty branch-safe author")
            .branch(),
    };
    let segment = explicit_segment.unwrap_or(segment);
    let workspace_slug = segment.as_str().to_string();
    let plan = plan_branch_placement(main_repo, segment, Some(&branch))?;
    if recorded_branch.is_none() && plan.strategy != PlacementStrategy::Create {
        return Err(task_error(format!(
            "branch {} already exists; choose another workspace name with --name",
            plan.branch
        )));
    }
    // Plan without touching an occupied checkout; restoration consumes the saved placement.
    if plan.strategy != PlacementStrategy::UseExistingWorktree && plan.worktree_path.try_exists()? {
        return Err(task_error(format!(
            "worktree path already exists: {}",
            plan.worktree_path.display()
        )));
    }
    Ok((plan, workspace_slug))
}

async fn select_new_stack_parent(
    store: &SharedStore,
    task: &Task,
    requested: Option<&str>,
) -> OpsResult<Option<TaskPr>> {
    if let Some(parent_issue) = requested {
        let parent_task = store
            .get_task_by_issue(parent_issue)
            .await
            .map_err(|error| task_error(format!("failed to read parent Task: {error}")))?
            .ok_or_else(|| {
                task_error(format!(
                    "stack parent {parent_issue:?} has no Task; run it first"
                ))
            })?;
        if parent_task.id == task.id {
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
                parent_task.worktree()?.display()
            )));
        }
        Ok(Some(parent))
    } else {
        Ok(None)
    }
}

async fn prepare_task_placement(
    store: &SharedStore,
    main_repo: &Path,
    task: &Task,
    recorded_branch: Option<&str>,
    options: &TaskProcessOptions,
) -> OpsResult<TaskPlacement> {
    options.validate_directive(None)?;
    let directive = options
        .directive
        .as_deref()
        .map(str::trim)
        .map(str::to_owned);
    let recorded_branch = recorded_branch.filter(|branch| !branch.is_empty());
    fetch_task_refs(main_repo)?;
    let (mut plan, workspace_slug) =
        plan_task_placement(main_repo, task, recorded_branch, options)?;
    let stack_parent = select_new_stack_parent(store, task, options.stack_on.as_deref()).await?;
    let mut base_commit = match &stack_parent {
        Some(parent) => {
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
    let github = if recorded_branch.is_some() || plan.strategy != PlacementStrategy::Create {
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
    if plan.strategy != PlacementStrategy::Create {
        let branch_ref = if ref_exists(main_repo, &format!("refs/heads/{}", plan.branch))? {
            format!("refs/heads/{}", plan.branch)
        } else {
            format!("refs/remotes/origin/{}", plan.branch)
        };
        base_commit = merge_base(main_repo, &base_commit, &branch_ref).map_err(task_error)?;
    }
    // Provider creation can yield while another fetch advances the branch.
    // Place the checkout on the same commit recorded by its placement.
    plan.base_ref = base_commit;
    Ok(TaskPlacement {
        plan,
        workspace_slug,
        stack_parent,
        github,
        requested_agent: options.agent.clone(),
        directive,
    })
}

async fn place_prepared_task(
    store: &SharedStore,
    task_id: &crate::durable::TaskId,
    prepared: TaskPlacement,
) -> OpsResult<Task> {
    let TaskPlacement {
        plan,
        workspace_slug,
        stack_parent,
        github,
        requested_agent,
        directive,
    } = prepared;
    let (mut task, acquisition) = loop {
        let task = store
            .get_task(task_id)
            .await
            .map_err(task_error)?
            .ok_or_else(|| task_error("Task is unavailable"))?;
        let wave = owning_wave(store, &task).await?;
        let acquisition = match wave {
            Some(wave) => Some(super::planning_sync::lock_wave_planning(&wave).await?),
            None => None,
        };
        let current = store
            .get_task(task_id)
            .await
            .map_err(task_error)?
            .ok_or_else(|| task_error("Task is unavailable"))?;
        // Refiling may have finished before this guard was acquired. Continue
        // under the current owner rather than carrying the previous Wave's lock.
        if current.wave_id == task.wave_id {
            break (current, acquisition);
        }
    };
    if matches!(
        task_work_status(store, &task).await?,
        WorkStatus::Done | WorkStatus::Abandoned
    ) {
        return Err(task_error(format!(
            "Task {} is terminal; inspect its retained history",
            task.identifier
        )));
    }
    if task.worktree.is_some() {
        restore_task_checkout(store, &task, None).await?;
        select_task_agent(store, &mut task, requested_agent.as_deref()).await?;
        return Ok(task);
    }
    let now = time::OffsetDateTime::now_utc();
    let pr = TaskPr {
        id: TaskPrId::new(),
        task_id: task.id.clone(),
        sequence: 1,
        slug: workspace_slug.clone(),
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

    task = store
        .place_task(
            &task.id,
            &plan.worktree_path,
            &workspace_slug,
            &pr,
            acquisition,
        )
        .await
        .map_err(task_error)?;
    if let Some(direction) = directive.as_deref() {
        append_task_comment(store, &task, direction, true)?;
    }
    // A competing reservation may have won. Restore its persisted placement,
    // never the losing caller's plan or PR.
    if let Err(error) = restore_task_checkout(store, &task, None).await {
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
            tracing::warn!(task = %task.id, %event_error, "checkout failed after placement committed; failure event did not persist");
        }
        return Err(error);
    }
    select_task_agent(store, &mut task, requested_agent.as_deref()).await?;
    Ok(task)
}

pub(crate) async fn select_task_creation_wave(
    store: &Store,
    repo: &Path,
    wave: Option<&str>,
) -> OpsResult<Option<Wave>> {
    let Some(wave) = wave else {
        return Ok(None);
    };
    crate::work::wave::context::resolve_managed_wave(Some(store), Some(repo), Some(wave), None)
        .await
        .map(Some)
        .map_err(task_error)
}

pub fn task_create(
    repo: &Path,
    wave: Option<&str>,
    title: Option<String>,
    report: Option<String>,
) -> OpsResult<Task> {
    let input = resolve_task_create_input(title.as_deref(), report.as_deref())?;
    let main = crate::git::worktrees::main_repo_root(repo).map_err(task_error)?;
    block_on_task(async {
        let store = super::planning_sync::planning_store().await?;
        if let Err(error) =
            super::planning_sync::acquire_repository(&store, &main.to_string_lossy()).await
        {
            tracing::debug!(%error,"planning acquisition pending; creating from retained planning");
        }

        if let Some(name) = wave {
            let locator =
                crate::work::wave::WaveLocator::discover(&main, name).map_err(task_error)?;
            if store
                .get_wave_at(&locator)
                .await
                .map_err(task_error)?
                .is_none()
            {
                super::project::ensure(repo, name).await?;
            }
        }
        let selected = select_task_creation_wave(&store, &main, wave).await?;
        let (creation_project, acquisition) = match selected {
            Some(wave) => {
                let acquisition = super::planning_sync::lock_wave_planning(&wave).await?;
                let project = super::project::current_project(&store, &wave)?.id;
                (Some(project), Some(acquisition))
            }
            None => (None, None),
        };
        let repository_id = store
            .sqlite
            .ensure_repository(&main.to_string_lossy())
            .map_err(task_error)?;
        let task = store
            .create_task(
                &crate::work::task::NewTask {
                    due_date: None,
                    id: crate::durable::TaskId::new(),
                    repository_id,
                    project_id: creation_project,
                    title: input.title,
                    description: input.report,
                },
                acquisition,
            )
            .await
            .map_err(task_error)?;
        super::planning_sync::sync_after_save(&store, &main.to_string_lossy()).await;
        store
            .get_task(&task.id)
            .await
            .map_err(task_error)?
            .ok_or_else(|| task_error("Created Task is missing"))
    })
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
    skill: Option<&crate::graph::Skill>,
) -> String {
    crate::prompt::process::resolve_agent(agent, skill, &load_config_or_default(Some(worktree)))
}

async fn select_task_agent(
    store: &SharedStore,
    task: &mut Task,
    agent: Option<&str>,
) -> OpsResult<()> {
    if let Some(agent) = agent {
        checkout_execution_boundary(task.worktree()?, agent)
            .map_err(|error| task_error(error.to_string()))?;
        store
            .set_task_agent(&task.id, agent)
            .await
            .map_err(|error| task_error(format!("failed to save Task agent: {error}")))?;
        task.agent = Some(agent.to_string());
    }
    Ok(())
}

fn task_configuration_refusal(task: &Task, skill: Option<&crate::graph::Skill>) -> Option<String> {
    let worktree = task.worktree.as_deref()?;
    checkout_execution_boundary(
        worktree,
        &resolve_task_agent(worktree, task.agent.as_deref(), skill),
    )
    .err()
    .map(|error| error.to_string())
}

pub(crate) async fn task_process_refusal(
    store: &SharedStore,
    task: &Task,
) -> crate::store::StoreResult<Option<String>> {
    if let Some(refusal) = task_configuration_refusal(task, None) {
        return Ok(Some(refusal));
    }
    persisted_task_process_refusal(store, task).await
}

async fn persisted_task_process_refusal(
    store: &SharedStore,
    task: &Task,
) -> crate::store::StoreResult<Option<String>> {
    let event = store.latest_task_event(&task.id).await?;
    Ok(task_event_process_refusal(event.as_ref()).map(str::to_string))
}

pub(crate) fn task_event_process_refusal(
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
    if task.worktree.is_none() {
        return Ok(None);
    }
    let event = store
        .latest_task_event(&task.id)
        .await
        .map_err(|error| task_error(format!("failed to read Task worktree state: {error}")))?;
    if let Some(event) = event {
        if let TaskEventKind::WorktreeInitializing { branch, path, .. }
        | TaskEventKind::CheckoutInitializing { branch, path, .. } = &event.kind
        {
            let initializing = event.created_at + TASK_WORKTREE_INITIALIZATION_GRACE
                > time::OffsetDateTime::now_utc();
            let reason = if initializing {
                format!(
                    "Task {} is initializing worktree {path} on branch {branch:?}; no body is expected until placement completes",
                    task.identifier
                )
            } else {
                format!(
                    "Task {} worktree initialization did not complete at {path} on branch {branch:?}; finish or restore that exact path before `lf task run {}`; Task identity and PR history are unchanged",
                    task.identifier, task.identifier
                )
            };
            return Ok(Some(TaskWorktreeBlocker {
                initializing,
                reason,
            }));
        }
    }
    if task.worktree()?.exists() {
        return Ok(None);
    }
    let branch = format!(" on branch {:?}", task.branch);
    Ok(Some(TaskWorktreeBlocker {
        initializing: false,
        reason: format!(
            "Task {} worktree {} is missing; restore that exact path{branch} before `lf task run {}`; Task identity and PR history are unchanged",
            task.identifier,
            task.worktree()?.display(),
            task.identifier,
        ),
    }))
}

pub(crate) async fn task_for_checkout(store: &SharedStore, repo: &Path) -> OpsResult<Option<Task>> {
    // Branch names are not checkout identity: another clone can use the same
    // branch. Never probe a remote Machine's recorded path on this Machine.
    let Ok(root) = crate::git::worktree_root(repo) else {
        return Ok(None);
    };
    let local = store.local_machine().await.map_err(task_error)?.id;
    let mut id = None;
    for checkout in store.task_checkouts().await.map_err(task_error)? {
        if checkout.machine_id.as_ref() != Some(&local) {
            if checkout.machine_id.is_none() && checkout.worktree == root {
                return Err(task_error("Task checkout Machine is unknown"));
            }
            continue;
        }
        if crate::git::worktree_root(&checkout.worktree).ok().as_ref() == Some(&root) {
            if id.is_some() {
                return Err(task_error("Multiple Tasks claim this checkout"));
            }
            id = Some(checkout.task_id);
        }
    }
    match id {
        Some(id) => store.get_task(&id).await.map_err(task_error),
        None => Ok(None),
    }
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
    /// This checkout has no Task. Task-specific bookkeeping is an
    /// explicit no-op; the ordinary PR flow continues unchanged.
    Unmanaged,
    /// The registry is healthy and the checkout belongs to a Task.
    /// Boxed so the `Unmanaged` no-op variant stays small.
    Managed { store: SharedStore, task: Box<Task> },
}

/// Turn a [`RegistryUnavailable`] into an actionable authority error. The
/// message always names the recovery action so the operator can move.
fn task_registry_error(err: RegistryUnavailable) -> OpsError {
    task_error(match err {
        RegistryUnavailable::MissingFile { path } => format!(
            "Task PR authority refused: the shared Loopflow registry {} is missing. \
             Run `lf doctor` to inspect the registry.",
            path.display()
        ),
        RegistryUnavailable::Unresolved { error } => format!(
            "Task PR authority refused: the shared Loopflow registry path is not usable: {error}. \
             Fix LF_HOME or run `lf doctor`."
        ),
        RegistryUnavailable::Incompatible { path, error } => format!(
            "Task PR authority refused: the shared Loopflow registry {} is present but \
             inaccessible or schema-incompatible: {error}. Run `lf doctor`.",
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

fn task_pr_for_publication(task: &Task) -> TaskPr {
    TaskPr {
        id: TaskPrId::new(),
        task_id: task.id.clone(),
        sequence: 1,
        slug: task.workspace_slug.clone(),
        branch: task.branch.clone(),
        base_commit: task.base_commit.clone(),
        parent_pr_id: task.parent_pr_id.clone(),
        publication: None,
        merge_commit: None,
        abandoned_at: None,
        ci_observation: None,
        github_observation: None,
        linear_attachment_id: None,
        linear_comment_id: None,
        linear_link_error: None,
        created_at: time::OffsetDateTime::now_utc(),
        updated_at: time::OffsetDateTime::now_utc(),
    }
}

pub(crate) fn request_task_pr_publication(repo: &Path, title: &str, body: &str) -> OpsResult<bool> {
    let title = title.trim();
    let body = body.trim();
    if title.is_empty() || body.is_empty() {
        return Err(task_error(
            "Task PR settlement requires a non-empty reviewer-facing title and body; supply both or let Loopflow generate them",
        ));
    }
    block_on_task(async move {
        let ManagedTask::Managed { store, task } = resolve_managed_task(repo).await? else {
            return Ok(false);
        };
        let context = _task_pr_context_from_store(&store, &task).await?;
        _validate_task_pr_copy(&context, body)?;
        let existing = store.active_task_pr(&task.id).await.map_err(task_error)?;
        let existing = existing.filter(|pr| !pr.is_settled());
        let creating = existing.is_none();
        let mut pr = existing.unwrap_or_else(|| task_pr_for_publication(&task));
        if creating {
            pr.sequence = store
                .task_prs(&task.id)
                .await
                .map_err(task_error)?
                .iter()
                .map(|pr| pr.sequence)
                .max()
                .unwrap_or(0)
                + 1;
        }
        let branch = crate::git::current_branch(repo)?
            .ok_or_else(|| task_error("Task worktree is not on a branch"))?;
        if pr.branch != branch {
            return Err(task_error(format!(
                "Task {} active PR expects branch {:?}, but the worktree is on another branch",
                task.identifier, pr.branch
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
            presentation: pr
                .publication
                .as_ref()
                .and_then(|publication| publication.presentation.clone()),
            github,
            merge,
        });
        pr.updated_at = now;
        if creating {
            store.insert_task_pr(&pr).await
        } else {
            store.update_task_pr(&pr).await
        }
        .map_err(|error| task_error(format!("failed to request PR publication: {error}")))?;
        Ok(true)
    })
}

pub(crate) fn record_task_pr_presentation(repo: &Path, title: &str, body: &str) -> OpsResult<()> {
    let head_sha = rev_parse(repo, "HEAD")?;
    block_on_task(async {
        let ManagedTask::Managed { store, task } = resolve_managed_task(repo).await? else {
            return Ok(());
        };
        let mut pr = store
            .active_task_pr(&task.id)
            .await
            .map_err(task_error)?
            .ok_or_else(|| task_error("publication intent disappeared"))?;
        let publication = pr
            .publication
            .as_mut()
            .ok_or_else(|| task_error("publication was not requested"))?;
        publication.presentation = Some(PrPresentation {
            title: title.into(),
            body: body.into(),
            head_sha,
        });
        store.update_task_pr(&pr).await.map_err(task_error)
    })
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct TaskPrContext {
    pub(crate) title: String,
    pub(crate) identifier: String,
    pub(crate) url: Option<String>,
    pub(crate) merge_request: Option<PrMergeRequest>,
}

impl TaskPrContext {
    pub(crate) fn task_link(&self) -> String {
        let label = format!(
            "{} · {}",
            _markdown_link_text(self.title.trim()),
            _markdown_link_text(self.identifier.trim())
        );
        match &self.url {
            Some(url) => format!("[{label}]({url})"),
            None => label,
        }
    }
}

fn _markdown_link_text(value: &str) -> String {
    value
        .replace('\\', "\\\\")
        .replace('[', "\\[")
        .replace(']', "\\]")
}

/// Publication observes a previous PR's settlement itself; callers never need
/// to run reconciliation before continuing work in a checkout.
pub(crate) fn refresh_task_pr_for_publication(repo: &Path) -> OpsResult<()> {
    block_on_task(async {
        let ManagedTask::Managed { store, mut task } = resolve_managed_task(repo).await? else {
            return Ok(());
        };
        let Some(mut pr) = store.active_task_pr(&task.id).await.map_err(task_error)? else {
            return Ok(());
        };
        if let Some(known) = pr.github().filter(|_| pr.phase() != PrPhase::Merged) {
            let open = crate::ops::pr::current_pr(repo)?;
            if pr.phase() == PrPhase::Abandoned
                || open.is_none_or(|open| open.number != u64::from(known.number))
            {
                pr = reconcile_task_pr_observation(
                    &store,
                    &mut task,
                    crate::ops::pr::PrReadFreshness::Fresh,
                )
                .await?
                .ok_or_else(|| task_error("publication PR disappeared"))?;
                if let Observation::Degraded { reason, .. } = task.observation {
                    return Err(task_error(reason));
                }
            }
        }
        if pr.phase() == PrPhase::Merged && task.parent_pr_id.is_some() {
            // Its own merge delivered the dependency too. The old PR retains
            // that stack; the next delivery compares with the default branch.
            store
                .sync_task_placement(
                    &task.id,
                    &task.base_commit,
                    true,
                    time::OffsetDateTime::now_utc(),
                )
                .await
                .map_err(task_error)?;
        }
        Ok(())
    })
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
    let url = if task.linear_id.is_some() {
        Some(
            store
                .sqlite
                .task_issue_url(&task.id)
                .map_err(task_error)?
                .filter(|url| _valid_task_url(url))
                .ok_or_else(|| _missing_task_pr_url(task))?,
        )
    } else {
        None
    };
    let pr = store.active_task_pr(&task.id).await.map_err(task_error)?;
    Ok(TaskPrContext {
        title: task.title.clone(),
        identifier: task.identifier.clone(),
        url,
        merge_request: pr.as_ref().filter(|pr| !pr.is_settled()).and_then(|pr| {
            pr.merge_request()
                .filter(|request| {
                    pr.github().and_then(|github| github.head_sha.as_deref())
                        == Some(request.head_sha.as_str())
                })
                .cloned()
        }),
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

fn _missing_task_pr_url(task: &Task) -> OpsError {
    task_error(format!(
        "Task {} has no valid provider URL in the cached PM snapshot. Run `lf wave status --sync` before publishing this Task PR",
        task.identifier,
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
            "Task PR body must include its owning Task: {anchor}"
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
) -> OpsResult<Option<(u32, String)>> {
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
            .ok_or_else(|| task_error(format!("Task {} has no active PR", task.identifier)))?;
        if pr.is_settled() || branch.as_deref() != Some(pr.branch.as_str()) {
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
    let path = crate::git::absolute_git_dir(repo)?.join("lf-pr-mutation.lock");
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
    let Some(mut pr) = store
        .active_task_pr(&task.id)
        .await
        .map_err(|error| task_error(format!("failed to read active PR: {error}")))?
    else {
        return Ok(false);
    };
    if pr.is_settled() {
        return Ok(false);
    }
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
    inherit_pr: &impl Fn(&mut Command),
) -> OpsResult<bool> {
    let head_sha = head_sha.map(str::to_string);
    block_on_task(async move {
        let ManagedTask::Managed { store, task } = resolve_managed_task(repo).await? else {
            return Ok(false);
        };
        let head_sha = head_sha
            .filter(|head| !head.trim().is_empty())
            .ok_or_else(|| {
                task_error(format!(
                    "GitHub did not report the current head for Task {}; refusing to request a merge without an exact commit",
                    task.identifier
                ))
            })?;
        let mut pr = store
            .active_task_pr(&task.id)
            .await
            .map_err(|error| task_error(format!("failed to read active PR: {error}")))?
            .ok_or_else(|| task_error(format!("Task {} has no active PR", task.identifier)))?;
        let publication = pr.publication.as_mut().ok_or_else(|| {
            task_error(format!(
                "Task {} has no durable PR publication request",
                task.identifier
            ))
        })?;
        let github_head = publication
            .github
            .as_ref()
            .and_then(|github| github.head_sha.as_deref());
        if github_head != Some(head_sha.as_str()) {
            return Err(task_error(format!(
                "Task {} stored GitHub head {:?}, not requested merge head {}; refusing an unpinned settlement",
                task.identifier, github_head, head_sha
            )));
        }
        if publication
            .presentation
            .as_ref()
            .is_none_or(|presentation| presentation.head_sha != head_sha)
        {
            return Err(task_error(format!(
                "Task {} has no non-empty reviewer-facing title and body for head {}; refresh PR copy before requesting settlement",
                task.identifier, head_sha
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
            .filter(|request| request.mode == mode && request.head_sha == head_sha)
            .map_or(now, |request| request.requested_at);
        publication.merge = Some(PrMergeRequest {
            mode,
            requested_at,
            head_sha: head_sha.clone(),
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

pub(crate) fn fetch_task_refs(repo: &Path) -> OpsResult<()> {
    if crate::git::has_origin(repo)? {
        git_output(
            repo,
            &[
                "fetch",
                "--prune",
                "origin",
                "+refs/heads/*:refs/remotes/origin/*",
            ],
        )
        .map_err(|error| task_error(format!("failed to fetch task base and branches: {error}")))?;
    }
    Ok(())
}

/// Resolve a new Task's base through the default branch's existing Git tracking.
fn resolve_upstream_base(repo: &Path, default_branch: &str) -> OpsResult<(String, String)> {
    let base = if default_branch == get_default_branch(repo)? {
        crate::git::integration_base(repo)?
    } else if has_remote(repo)? {
        format!("origin/{default_branch}")
    } else {
        default_branch.to_string()
    };
    crate::git::fetch_tracking_ref(repo, &base)
        .map_err(|error| task_error(format!("failed to fetch task base: {error}")))?;
    let base_ref = if base == default_branch {
        format!("refs/heads/{default_branch}")
    } else {
        base
    };
    let base_commit = rev_parse(repo, &base_ref)
        .map_err(|error| task_error(format!("failed to resolve task base {base_ref}: {error}")))?;
    Ok((base_ref, base_commit))
}

/// Reuse the Task's recorded diff base, including a stacked parent.
pub(crate) fn task_diff_base(repo: &Path) -> OpsResult<Option<String>> {
    block_on_task(async move {
        match resolve_managed_task(repo).await? {
            ManagedTask::Managed { task, .. } => Ok(Some(task.base_commit.clone())),
            ManagedTask::Unmanaged => Ok(None),
        }
    })
}

/// Refresh the Task's comparison base from current Git ancestry before push.
/// Saved placement never classifies branch commits as foreign: publication
/// preserves HEAD and uses the same fork point as GitHub. Managed Task and
/// branch ownership still apply; emptiness is checked after committing.
pub(crate) fn refresh_task_pr_base(repo: &Path) -> OpsResult<()> {
    let repo = repo.to_path_buf();
    block_on_task(async move {
        let ManagedTask::Managed { store, task } = resolve_managed_task(&repo).await? else {
            return Ok(());
        };
        resolve_task_pr_base(&store, &task, &repo, BaseUpdate::Record, None)
            .await
            .map(|_| ())
    })
}

/// Prove the post-sync Task range against the operation's immutable target
/// without advancing durable metadata before a requested push is verified.
pub(crate) fn validate_task_pr_range_for_integration(
    repo: &Path,
    target_ref: &str,
    target_sha: &str,
) -> OpsResult<()> {
    verify_task_pr_range_for_integration(repo, target_ref, target_sha, BaseUpdate::Preserve)
}

/// Record the immutable base only after every requested Git postcondition,
/// including remote-head equality, has passed.
pub(crate) fn record_task_pr_range_after_integration(
    repo: &Path,
    target_ref: &str,
    target_sha: &str,
) -> OpsResult<()> {
    verify_task_pr_range_for_integration(repo, target_ref, target_sha, BaseUpdate::Record)
}

fn verify_task_pr_range_for_integration(
    repo: &Path,
    target_ref: &str,
    target_sha: &str,
    base_update: BaseUpdate,
) -> OpsResult<()> {
    let repo = repo.to_path_buf();
    let target_ref = target_ref.to_string();
    let target_sha = target_sha.to_string();
    block_on_task(async move {
        let ManagedTask::Managed { store, task } = resolve_managed_task(&repo).await? else {
            return Ok(());
        };
        resolve_task_pr_base(
            &store,
            &task,
            &repo,
            base_update,
            Some((target_ref, target_sha)),
        )
        .await
        .map(|_| ())
    })
}

/// Refuse empty Task PRs before GitHub mutations, including existing PR updates.
/// Non-Task worktrees are unaffected; an unreadable managed registry is an error.
pub(crate) fn require_task_pr_range_nonempty(repo: &Path) -> OpsResult<()> {
    let repo = repo.to_path_buf();
    block_on_task(async move {
        let ManagedTask::Managed { store, task } = resolve_managed_task(&repo).await? else {
            return Ok(());
        };
        require_task_pr_range_nonempty_in(&store, &task, &repo).await
    })
}

/// Compare with the live stack parent, or the default branch after it settles.
/// Publication retains the stack fork separately for explicit integration.
async fn resolve_task_pr_upstream(
    store: &SharedStore,
    task: &Task,
    repo: &Path,
    default_branch: &str,
) -> OpsResult<(String, String)> {
    if let Some(parent_id) = task.parent_pr_id.as_ref() {
        let parent = store
            .get_task_pr(parent_id)
            .await
            .map_err(|error| task_error(format!("failed to read stack parent: {error}")))?
            .ok_or_else(|| task_error(format!("stack parent {parent_id} is missing")))?;
        let parent_live = parent.merge_commit.is_none() && parent.abandoned_at.is_none();
        if parent_live {
            return resolve_upstream_base(repo, &parent.branch);
        }
    }
    resolve_upstream_base(repo, default_branch)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum BaseUpdate {
    Preserve,
    Record,
}

async fn resolve_task_pr_base(
    store: &SharedStore,
    task: &Task,
    repo: &Path,
    base_update: BaseUpdate,
    upstream_override: Option<(String, String)>,
) -> OpsResult<String> {
    let branch =
        current_branch(repo)?.ok_or_else(|| task_error("Task worktree is not on a branch"))?;
    if task.branch != branch {
        return Err(task_error(format!(
            "Task {} expects branch {:?}, but the worktree is on {:?}",
            task.identifier, task.branch, branch
        )));
    }

    let default_branch = get_default_branch(repo)?;
    // A sync can integrate any ref, including this PR's own remote branch. Only
    // a pinned default-branch target is the PR's upstream; every other target
    // is measured against the real one, so integration never moves the base.
    let pinned = upstream_override.filter(|(target_ref, _)| {
        let name = target_ref
            .strip_prefix("origin/")
            .or_else(|| target_ref.strip_prefix("refs/heads/"))
            .unwrap_or(target_ref);
        name == default_branch
    });
    let (base_ref, upstream) = match pinned {
        Some(target) => target,
        None => resolve_task_pr_upstream(store, task, repo, &default_branch).await?,
    };
    let head = rev_parse(repo, "HEAD")
        .map_err(|error| task_error(format!("failed to resolve Task HEAD: {error}")))?;
    let identifier = &task.identifier;

    let merge_base = crate::git::merge_base(repo, &upstream, &head).map_err(|error| {
        task_error(format!(
            "Task {identifier} cannot resolve a PR comparison between {branch:?} and {base_ref}: {error}. \
             Keep this branch; inspect the upstream and its shared history before publishing"
        ))
    })?;

    // A stored fork may predate a reset, rebase, upstream rewrite or expired
    // reflog. A stack still needs its saved fork for the squash-parent comparison
    // merge; record_stack_sync owns that write after integration. Root Tasks
    // need only the current PR comparison.
    if base_update == BaseUpdate::Record
        && task.parent_pr_id.is_none()
        && merge_base != task.base_commit
    {
        store
            .sync_task_placement(
                &task.id,
                &merge_base,
                false,
                time::OffsetDateTime::now_utc(),
            )
            .await
            .map_err(|error| task_error(format!("failed to record Task PR base: {error}")))?;
    }
    Ok(merge_base)
}

/// Judge emptiness against the current PR range, independently of a stack's
/// retained integration fork.
async fn require_task_pr_range_nonempty_in(
    store: &SharedStore,
    task: &Task,
    repo: &Path,
) -> OpsResult<()> {
    let base = resolve_task_pr_base(store, task, repo, BaseUpdate::Record, None).await?;
    let identifier = &task.identifier;
    let short = base.chars().take(12).collect::<String>();
    let base_tree = rev_parse(repo, &format!("{base}^{{tree}}"))?;
    let head_tree = rev_parse(repo, "HEAD^{tree}")?;
    if head_tree == base_tree {
        return Err(task_error(format!(
            "Task {identifier} PR range is empty: the tree at HEAD matches the comparison base \
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
            .ok_or_else(|| task_error(format!("Task {} has no active PR", task.identifier)))?;
        let github_pr = github_pr.ok_or_else(|| {
            task_error(format!(
                "GitHub PR for Task {} could not be read after creation or update",
                task.identifier
            ))
        })?;
        if github_pr.branch != pr.branch {
            return Err(task_error(format!(
                "Task {} active PR expects branch {:?}, but GitHub reported {:?}",
                task.identifier, pr.branch, github_pr.branch
            )));
        }
        let number = u32::try_from(github_pr.number).map_err(|_| {
            task_error(format!(
                "pull request #{} exceeds supported range",
                github_pr.number
            ))
        })?;
        let url = github_pr.url.clone();
        if pr
            .github()
            .is_some_and(|known| known.number != number || known.url != url)
        {
            return Err(task_error(
                "Task already owns a different pull request; new delivery requires a new Task",
            ));
        }
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
        link_pr_to_planning(&store, &task, &mut pr).await;
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

pub(crate) fn reconcile_checkout_pr(repo: &Path) -> OpsResult<Option<(Task, TaskPr, TaskState)>> {
    block_on_task(async {
        let ManagedTask::Managed { store, mut task } = resolve_managed_task(repo).await? else {
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
        let state = store.sqlite.task_state(&task.id).map_err(task_error)?;
        Ok(pr.map(|pr| (*task, pr, state)))
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
    // Old landing rows remain history after migration; they cannot settle a
    // different current delivery or a PR-less Task.
    let Some(current) = store.active_task_pr(task_id).await.map_err(task_error)? else {
        return Ok(());
    };
    if current.github().map(|github| github.number) != Some(landing.pr_number) {
        return Ok(());
    }
    let pr = if current.phase() == PrPhase::Merged {
        current
    } else {
        reconcile_task_pr_observation(store, &mut task, crate::ops::pr::PrReadFreshness::Fresh)
            .await?
            .ok_or_else(|| task_error("landing Task PR disappeared during merge settlement"))?
    };
    apply_merged_task_landing(store, &mut task, &pr, landing).await?;
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
    {
        return Err(task_error(format!(
            "GitHub did not confirm landing pull request #{} merged for Task {}",
            landing.pr_number, task.identifier
        )));
    }
    reconcile_task_completion(store, task).await
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

// Local control commands often arrive in a burst (`status`, then `complete`,
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

/// `abandoned_at` on a published PR caches GitHub's closed state rather than
/// deciding it — `lf pr abandon` runs `gh pr close` before stamping it — so a
/// reopen must be able to clear it. A merge is terminal: GitHub cannot unmerge.
async fn reconcile_task_pr_observation(
    store: &SharedStore,
    task: &mut Task,
    freshness: crate::ops::pr::PrReadFreshness,
) -> OpsResult<Option<TaskPr>> {
    // Reconciliation updates the same projection as publication/finalization.
    // Refuse overlap so a remote read begun before a push cannot overwrite the
    // request or head recorded by the command that completed after it. A
    // removed checkout (a landed stack parent) hosts no mutation to overlap.
    let checkout = task.worktree.clone().filter(|path| path.exists());
    let _mutation = checkout.as_deref().map(lock_task_pr_mutation).transpose()?;
    let Some(mut pr) = store.active_task_pr(&task.id).await.map_err(task_error)? else {
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
    let repo = match checkout {
        Some(path) => path,
        None => repository_for_task(store, task)?,
    };
    let observation = match number {
        Some(number) => crate::ops::pr::observe_pr_by_number(&repo, number, &pr.branch, freshness),
        None => crate::ops::pr::observe_pr_by_branch(&repo, &pr.branch),
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
    invalidate_stale_merge_request(&repo, publication, &github_pr, &|_| {})?;
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
            if let Some(ci_observation) =
                observe_required_checks(&repo, github_pr.number, github_pr.head_sha.as_deref(), now)
            {
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
            store.settle_task_pr(&pr).await.map_err(task_error)?;
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

#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct TaskStatus {
    pub sync: Option<crate::store::planning_sync::PlanningSyncStatus>,
    pub task: Option<Task>,
    pub project: Option<crate::work::project::Project>,
    pub observed_at: Option<i64>,
    pub planning_error: Option<String>,
    pub planning_stale: bool,
    pub planning_state: crate::store::PlanningState,
    pub execution: Option<TaskSnapshot>,
    pub ownership: Option<crate::durable::TaskOwnership>,
}

pub fn task_status(repo: &Path, issue: Option<&str>) -> OpsResult<TaskStatus> {
    block_on_task(async {
        let store = task_store().await?;
        super::linear_observe::sync_repository(&store, &repo.to_string_lossy()).await;
        let mut task = match issue {
            Some(issue) => {
                super::planning_sync::find_task(&store, &repo.to_string_lossy(), issue).await?
            }
            None => Some(
                task_for_checkout(&store, repo)
                    .await?
                    .ok_or_else(|| task_error("this checkout has no Task"))?,
            ),
        };
        if issue.is_none() {
            if let Some(task) = &task {
                if store.sqlite.task_deleted(task).map_err(task_error)? {
                    return Err(task_error("this checkout's Task was deleted; use an explicit Task identifier to read its history"));
                }
            }
        }
        let inspection =
            if task.is_none() && super::planning_client::is_linear(&repo.to_string_lossy()) {
                let selector = issue.ok_or_else(|| task_error("this checkout has no Task"))?;
                let read = super::linear::inspect_task_planning_async(
                    repo,
                    selector,
                    super::linear::LinearRefresh::Auto,
                )
                .await?;
                task = super::planning_sync::find_task(&store, &repo.to_string_lossy(), selector)
                    .await?;
                Some(read)
            } else {
                None
            };
        let mut result = TaskStatus {
            sync: None,
            task: None,
            project: None,
            observed_at: None,
            planning_error: inspection
                .as_ref()
                .and_then(|read| read.refresh_error.clone()),
            planning_stale: false,
            planning_state: crate::store::PlanningState::Unavailable,
            execution: None,
            ownership: None,
        };
        if let Some(task) = task {
            let acquisition = store
                .sqlite
                .task_acquisition(&task.id)
                .map_err(task_error)?;
            result.project = match &task.project_id {
                Some(id) => store.get_project(id).await.map_err(task_error)?,
                None => None,
            };
            result.sync = Some(
                store
                    .sqlite
                    .task_planning_sync(&task.id)
                    .map_err(task_error)?,
            );
            result.execution = Some(read_task_snapshot(&store, task.clone()).await?);
            result.ownership = Some(store.sqlite.task_ownership(&task.id).map_err(task_error)?);
            result.planning_state = acquisition.state;
            result.observed_at = acquisition.observed_at;
            result.task = Some(task);
        } else if let Some(read) = inspection {
            // Foreign observations remain acquisition evidence, not owned Work.
            result.planning_state = read.observation.state;
            result.observed_at = read.observation.observed_at;
        }
        if result.planning_error.is_some()
            && result.planning_state == crate::store::PlanningState::Available
        {
            result.planning_state = crate::store::PlanningState::Unavailable;
        }
        result.planning_stale = result.planning_state != crate::store::PlanningState::Available
            || result.observed_at.is_some_and(|at| {
                time::OffsetDateTime::now_utc().unix_timestamp() - at
                    >= super::linear::LINEAR_SOFT_STALE_SECS
            });
        Ok(result)
    })
}

/// Sessions copied here from rented Machines this Task ran on.
pub fn task_copied_sessions(
    task: &crate::durable::TaskId,
) -> OpsResult<Vec<crate::machine_records::CopiedSession>> {
    block_on_task(async {
        let store = task_store().await?;
        store
            .sqlite
            .copied_sessions(Some(task), None)
            .map_err(task_error)
    })
}

/// Complete the Task without moving its Graph or changing its Processes.
pub fn task_complete(repo: &Path, issue: &str, note: Option<&str>) -> OpsResult<Task> {
    block_on_task(async {
        let (store, mut task) = resolve_saved_task(repo, None, issue).await?;
        if let Some(request) = store
            .sqlite
            .request_task_completion(&task.id, note)
            .map_err(task_error)?
        {
            settle_completion(&store, &mut task, request).await?;
        } else {
            reconcile_task_completion(&store, &mut task).await?;
        }
        Ok(task)
    })
}

pub fn task_defer(issue: &str, deferred: bool) -> OpsResult<()> {
    block_on_task(async {
        let store = task_store().await?;
        let task = store
            .get_task_by_issue(issue)
            .await
            .map_err(task_error)?
            .ok_or_else(|| task_error(format!("Task {issue} is unavailable")))?;
        store
            .sqlite
            .set_task_deferred(&task.id, deferred)
            .map_err(task_error)?;
        let repo = repository_for_task(&store, &task)?;
        super::planning_sync::sync_after_save(&store, &repo.to_string_lossy()).await;
        Ok(())
    })
}

pub fn task_reopen(issue: &str, note: Option<&str>) -> OpsResult<Task> {
    block_on_task(async {
        let store = task_store().await?;
        let task = store
            .get_task_by_issue(issue)
            .await
            .map_err(task_error)?
            .ok_or_else(|| task_error(format!("Task {issue} is unavailable")))?;
        let task = store
            .sqlite
            .reopen_task(&task.id, note)
            .map_err(task_error)?;
        let repo = repository_for_task(&store, &task)?;
        super::planning_sync::sync_after_save(&store, &repo.to_string_lossy()).await;
        Ok(task)
    })
}

pub fn task_sync(issue: &str, retry_creation: bool, retry_updates: bool) -> OpsResult<String> {
    block_on_task(async {
        let store = task_store().await?;
        let task = store
            .get_task_by_issue(issue)
            .await
            .map_err(task_error)?
            .ok_or_else(|| task_error(format!("Task {issue} is unavailable")))?;
        let repository = repository_for_task(&store, &task)?;
        let repo = repository.as_path();
        let Some(provider) = super::planning_client::remote_name(repo) else {
            return Err(task_error(
                "Task repository has no remote planning provider",
            ));
        };
        if task.linear_id.is_none() {
            if let Some(project) = &task.project_id {
                super::planning_export::sync_export(
                    &store,
                    repo,
                    &crate::durable::WorkRef::Project(project.clone()),
                )
                .await?;
            }
            if retry_creation {
                super::planning_export::retry_task_export(&store, repo, &task.id).await?;
            } else {
                super::planning_export::sync_export(
                    &store,
                    repo,
                    &crate::durable::WorkRef::Task(task.id.clone()),
                )
                .await?;
            }
        }
        let task = store
            .get_task(&task.id)
            .await
            .map_err(task_error)?
            .ok_or_else(|| task_error("Task is missing"))?;
        if task.linear_id.is_none() {
            return Err(task_error(format!(
                "Task publication to {provider} remains pending"
            )));
        }
        // State and field writes share Linear's issue revision. Finish the
        // state attempt first so our own field acknowledgement cannot make its
        // ownership read stale or look like an intervening reopening. Retain
        // each result: a failed state attempt still permits the other effects.
        let work = crate::durable::WorkRef::Task(task.id.clone());
        let state = super::linear_observe::deliver_task_state(&store, &task, retry_updates).await;
        let (comments, fields) = tokio::join!(
            super::linear_observe::sync_task_comments(&store, &task),
            super::planning_delivery::deliver_fields(&store, repo, &work, retry_updates),
        );
        state?;
        comments?;
        fields?;
        let saved = store
            .get_task(&task.id)
            .await
            .map_err(task_error)?
            .ok_or_else(|| task_error("Task is missing"))?;
        let sync = store
            .sqlite
            .task_planning_sync(&saved.id)
            .map_err(task_error)?;
        let pending = sync.lines();
        if !pending.is_empty() {
            return Ok(format!("{}: {}", saved.identifier, pending.join("\n")));
        }
        Ok(match saved.pm_writeback {
            crate::work::task::LinearWritebackState::Current => {
                format!("{}: synchronized", saved.identifier)
            }
            crate::work::task::LinearWritebackState::Pending { error, .. } => {
                format!("{}: {error}", saved.identifier)
            }
        })
    })
}

pub(crate) fn completion_pending(store: &SharedStore, task: &Task) -> OpsResult<Option<String>> {
    Ok(store
        .sqlite
        .task_completion_pending(&task.id)
        .map_err(task_error)?
        .map(|(_, reason)| {
            format!(
                "Completion pending: {reason}. Retry: lf task complete {}",
                task.identifier
            )
        }))
}

async fn settle_completion(store: &SharedStore, task: &mut Task, request: i64) -> OpsResult<()> {
    let result = async {
        if task_work_status(store, task).await? == WorkStatus::Abandoned {
            return Err(task_error("An abandoned Task cannot be completed"));
        }
        reconcile_task_pr_observation(store, task, crate::ops::pr::PrReadFreshness::Cached).await?;
        if let Some(refusal) = task_completion_refusal(store, task).await? {
            return Err(task_error(refusal));
        }
        // Recheck after observation: a newer authored status may have superseded the request.
        if store
            .sqlite
            .task_completion_pending(&task.id)
            .map_err(task_error)?
            .map(|(id, _)| id)
            != Some(request)
        {
            return if task_work_status(store, task).await? == WorkStatus::Done {
                Ok(())
            } else {
                Err(task_error(
                    "Completion request was superseded by newer Task status",
                ))
            };
        }
        if !store
            .complete_task(task, request)
            .await
            .map_err(task_error)?
        {
            return Err(task_error(
                "Completion request was superseded by newer Task status",
            ));
        }
        *task = store
            .get_task(&task.id)
            .await
            .map_err(task_error)?
            .ok_or_else(|| task_error("completed Task is missing"))?;
        let repo = repository_for_task(store, task)?;
        super::planning_sync::sync_after_save(store, &repo.to_string_lossy()).await;
        // Cleanup retains live work and unpublished artifacts independently of status.
        cleanup_completed_task(store, task).await?;
        Ok(())
    }
    .await;
    if let Err(error) = &result {
        store
            .sqlite
            .fail_task_completion(&task.id, request, &error.to_string())
            .map_err(task_error)?;
    }
    result
}

/// Publication-state label carried by the PR's Linear linkage.
fn pr_link_state_label(pr: &TaskPr) -> String {
    match pr.phase() {
        PrPhase::Merged => "Merged".to_string(),
        PrPhase::Abandoned => "Abandoned".to_string(),
        _ => "Open".to_string(),
    }
}

/// Idempotently show the PR on the Task's planning item and record the outcome
/// on the PR: a Linear attachment and managed comment, or for Asana and Notion one
/// comment per PR state. Best-effort: a degraded writeback lands in
/// `linear_link_error` and leaves the GitHub result intact; the next publication
/// command retries. Does nothing for a PR with no GitHub URL yet.
pub(super) async fn link_pr_to_planning(store: &SharedStore, task: &Task, pr: &mut TaskPr) {
    let Some(issue_id) = task.linear_id.as_ref() else {
        return;
    };
    let Some(github) = pr.github().cloned() else {
        return;
    };
    let state = pr_link_state_label(pr);
    let title = format!("GitHub PR #{}", github.number);
    let body = format!("[GitHub PR #{}]({}) — {}", github.number, github.url, state);
    let repo = match repository_for_task(store, task) {
        Ok(repo) => repo,
        Err(error) => {
            pr.linear_link_error = Some(error.to_string());
            return;
        }
    };
    if !super::planning_client::is_linear(&repo.to_string_lossy()) {
        let posted = async {
            super::planning_client::PlanningClient::for_items(&repo)
                .await?
                .sync_comment("", issue_id.as_str(), &body)
                .await
                .map_err(|error| task_error(error.to_string()))
        }
        .await;
        if let Err(error) = &posted {
            tracing::warn!(
                issue = task.identifier,
                pr = github.number,
                "planning link degraded; the GitHub PR is published and the next publish retries: {error}"
            );
        }
        pr.linear_link_error = posted.err().map(|error| error.to_string());
        return;
    }
    let prior = crate::ops::linear::PrLinkageIds {
        attachment_id: pr.linear_attachment_id.clone(),
        comment_id: pr.linear_comment_id.clone(),
    };
    let request = crate::ops::linear::PrLinkRequest {
        issue_id: issue_id.as_str().to_string(),
        url: github.url.clone(),
        title,
        subtitle: state,
        body,
    };
    let outcome = crate::ops::linear::link_pr_async(&repo, &request, &prior).await;
    // Say so at publish time. The PR line in `lf task status` carries the durable
    // reading, but an operator running `lf pr open` should not have to go looking.
    if let Some(error) = &outcome.error {
        tracing::warn!(
            issue = task.identifier,
            pr = github.number,
            "Linear link degraded; the GitHub PR is published and the next publish retries: {error}"
        );
    }
    pr.linear_attachment_id = outcome.ids.attachment_id;
    pr.linear_comment_id = outcome.ids.comment_id;
    pr.linear_link_error = outcome.error;
}

// ---------------------------------------------------------------------------
// Completion requires the Task's recorded PR to be merged. Follow-up comments
// are ordinary Work and do not supply completion or cleanup authority.
// ---------------------------------------------------------------------------

pub(crate) fn completion_refusal(pr: Option<&TaskPr>, identifier: &str) -> Option<String> {
    pr.filter(|pr| pr.phase() != PrPhase::Merged).map(|_| {
        format!("Task {identifier} cannot complete: its pull request has not merged; merge it or abandon the Task")
    })
}

pub(crate) async fn task_completion_refusal(
    store: &SharedStore,
    task: &Task,
) -> OpsResult<Option<String>> {
    let pr = store.active_task_pr(&task.id).await.map_err(task_error)?;
    Ok(completion_refusal(pr.as_ref(), &task.identifier))
}

/// Reconcile retained delivery on the repository's existing periodic check.
/// Merge observation does not request Task completion.
pub(crate) async fn reconcile_delivered_task(
    store: &SharedStore,
    task: &mut Task,
) -> OpsResult<()> {
    // Local checkout cleanup is not evidence of delivery or Task completion.
    if let Some(worktree) = &task.worktree {
        if !worktree.try_exists()? {
            return Ok(());
        }
    }
    match task_work_status(store, task).await? {
        WorkStatus::Abandoned => return Ok(()),
        WorkStatus::Done => return reconcile_task_completion(store, task).await,
        WorkStatus::Ready => {}
    }
    let Some(pr) = store.active_task_pr(&task.id).await.map_err(task_error)? else {
        return Ok(());
    };
    if !matches!(
        pr.phase(),
        PrPhase::Open | PrPhase::Publishing | PrPhase::Merged
    ) {
        return Ok(());
    }
    reconcile_task_pr_observation(store, task, crate::ops::pr::PrReadFreshness::Fresh).await?;
    if let Observation::Degraded { reason, .. } = &task.observation {
        return Err(task_error(reason));
    }
    // Observing a merge succeeds independently of the Task completion request.
    reconcile_task_completion(store, task).await
}

pub(crate) async fn reconcile_task_completion(
    store: &SharedStore,
    task: &mut Task,
) -> OpsResult<()> {
    if store.sqlite.task_state(&task.id).map_err(task_error)? == crate::durable::TaskState::Deferred
    {
        return Ok(());
    }
    match task_work_status(store, task).await? {
        WorkStatus::Done => {
            return Ok(());
        }
        WorkStatus::Abandoned => return Ok(()),
        WorkStatus::Ready => {}
    }
    if let Some((request, _)) = store
        .sqlite
        .task_completion_pending(&task.id)
        .map_err(task_error)?
    {
        settle_completion(store, task, request).await?;
    }

    Ok(())
}

pub fn task_snapshot(task: &Task) -> OpsResult<TaskSnapshot> {
    block_on_task(async {
        let store = task_store().await?;
        read_task_snapshot(&store, task.clone()).await
    })
}

async fn read_task_snapshot(store: &SharedStore, task: Task) -> OpsResult<TaskSnapshot> {
    let wave = owning_wave(store, &task).await?;
    let repo = repository_for_task(store, &task)?;
    let project = match &task.project_id {
        Some(id) => Some(
            store
                .get_project(id)
                .await
                .map_err(task_error)?
                .ok_or_else(|| task_error(format!("owning Project {id} is missing")))?,
        ),
        None => None,
    };
    let work = store
        .work_for_child(&ChildRef::Task(task.id.clone()))
        .await
        .map_err(|error| task_error(format!("failed to resolve Task Work: {error}")))?;
    let latest_event = store
        .latest_task_event(&task.id)
        .await
        .map_err(|error| task_error(format!("failed to read latest task event: {error}")))?;
    let pr = store.active_task_pr(&task.id).await.map_err(task_error)?;
    let active = pr.as_ref().filter(|pr| pr.is_active());
    let work_status = store
        .work_status(&work)
        .await
        .map_err(|error| task_error(format!("failed to derive Task Work status: {error}")))?;
    let execution = crate::ops::task_execution::task_execution(store, &task.id)
        .await
        .map_err(task_error)?;
    let work_set = store.sqlite.task_work(&task.id).map_err(task_error)?;
    let status = store.task_state(&task.id).await.map_err(task_error)?;
    let actions = if store.sqlite.task_deleted(&task).map_err(task_error)? {
        TaskActionModel {
            recommended: Some(crate::ops::task_actions::TaskAction::NoAction),
            reason: "Task was deleted; retained history is read-only".into(),
        }
    } else if status == crate::durable::TaskState::Deferred {
        TaskActionModel {
            recommended: Some(crate::ops::task_actions::TaskAction::NoAction),
            reason: format!(
                "Task is deferred; `lf task resume {}` clears the hold without starting execution",
                task.identifier
            ),
        }
    } else {
        let predecessor_phase = match task.parent_pr_id.as_ref() {
            Some(parent_id) => store
                .get_task_pr(parent_id)
                .await
                .map_err(|error| task_error(format!("failed to read parent PR: {error}")))?
                .map(|pr| pr.phase()),
            None => None,
        };
        let completion_refusal = completion_refusal(pr.as_ref(), &task.identifier);
        let worktree_blocker = task_worktree_blocker(store, &task).await?;
        let resume_refusal = worktree_blocker
            .as_ref()
            .map(|blocker| blocker.reason.clone());
        let launch_refusal = if worktree_blocker.is_some() {
            None
        } else {
            task_configuration_refusal(&task, None)
                .or_else(|| task_event_process_refusal(latest_event.as_ref()).map(str::to_string))
        };
        let action_evidence = TaskActionEvidence {
            status: work_status.clone(),
            execution: Some(&execution),
            latest_pr_phase: pr.as_ref().map(TaskPr::phase),
            latest_pr_merge_request: pr.as_ref().and_then(TaskPr::merge_request),
            latest_pr_presentation_current: pr
                .as_ref()
                .filter(|pr| pr.phase() == PrPhase::Open)
                .map(|pr| pr.presentation().is_some()),
            completion_refusal: completion_refusal.as_deref(),
            resume_refusal: resume_refusal.as_deref(),
            ci: active.and_then(|pr| pr.fresh_ci()),
            predecessor_phase,
            launch_refusal: launch_refusal.as_deref(),
        };
        derive_task_actions(&action_evidence)
    };
    let agent = resolve_task_agent(
        task.worktree.as_deref().unwrap_or(&repo),
        task.agent.as_deref(),
        None,
    );
    let (provider, _) = parse_agent(&agent);
    let machine_id = store
        .task_checkout(&task.id)
        .await
        .map_err(task_error)?
        .and_then(|row| row.machine_id);
    let local_machine = store.local_machine().await.map_err(task_error)?;
    let worktree = task.worktree.as_ref().map(|path| {
        if machine_id.as_ref() == Some(&local_machine.id) {
            crate::git::worktree_root(path)
                .ok()
                .and_then(|root| root.canonicalize().ok())
                .unwrap_or_else(|| path.clone())
        } else {
            path.clone()
        }
    });
    let completion_pending = completion_pending(store, &task)?;
    Ok(TaskSnapshot {
        machine_id,
        issue_id: task.linear_id.as_ref().map(|id| id.as_str().to_string()),
        issue_identifier: task.identifier,
        task_id: task.id.to_string(),
        external_project_id: project
            .as_ref()
            .and_then(|p| p.linear_id.as_ref())
            .map(|id| id.as_str().to_string()),
        project: project.map(|p| p.slug),
        pm_snapshot_synced_at: task.pm_snapshot_synced_at,
        pm_writeback: task.pm_writeback,
        wave: wave.as_ref().map(|w| w.slug().to_string()),
        project_id: task.project_id.as_ref().map(ToString::to_string),
        repository_id: task.repository_id.to_string(),
        status,
        completion_pending,
        execution,
        work: work_set,
        worktree: worktree.map(|path| path.display().to_string()),
        workspace_slug: task.workspace_slug,
        branch: task.worktree.as_ref().map(|_| task.branch.clone()),
        base_commit: task.worktree.as_ref().map(|_| task.base_commit.clone()),
        agent: task.agent,
        provider,
        pr,
        latest_event,
        created_at: task.created_at,
        updated_at: task.updated_at,
        observation: task.observation,
        actions,
    })
}

/// Maximum byte length for Task file content, drafts and returned patches.
pub const MAX_FILE_BYTES: usize = 1_000_000;

pub fn task_changes(issue: &str, base: &str) -> OpsResult<TaskChangesSnapshot> {
    let (checkout, base) = comparison_context(issue, base)?;
    changes_snapshot(TaskComparison {
        checkout: TaskWorkspace::from(&checkout),
        base_commit: &base,
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
    let (checkout, base) = comparison_context(issue, base)?;
    let workspace = TaskComparison {
        checkout: TaskWorkspace::from(&checkout),
        base_commit: &base,
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

pub(crate) fn task_workspace_context(task: &Task) -> OpsResult<String> {
    const MAX_PATCH_TOKENS: usize = 15_000;

    let workspace = TaskComparison::new(task)?;
    let changes = changes_snapshot(workspace)?;
    let diff = diff_snapshot(workspace, None)?;
    // This inventory locates changes; it is not a content receipt. Avoid reading
    // and hashing every file again, especially when the patch is too large.
    let files = changes
        .files
        .iter()
        .map(|file| {
            let states = [
                (file.committed, "committed"),
                (file.staged, "staged"),
                (file.unstaged, "unstaged"),
                (file.untracked, "untracked"),
            ]
            .into_iter()
            .filter_map(|(present, state)| present.then_some(state))
            .collect::<Vec<_>>()
            .join(", ");
            let path = serde_json::to_string(&file.path).expect("a file path is a JSON string");
            format!("{path}: {states}")
        })
        .collect::<Vec<_>>()
        .join("\n");
    let include_patch = !diff.binary
        && !diff.truncated
        && crate::prompt::count_tokens(&diff.patch) < MAX_PATCH_TOKENS;
    let patch = if include_patch {
        diff.patch.as_str()
    } else {
        "Patch omitted from prompt because it is binary, truncated, or exceeds 15,000 tokens. Read the named worktree paths for exact bytes."
    };
    Ok(format!(
        "<lf:task-workspace>\nCheckout base: {}\nCurrent HEAD: {}\nChanges across the checkout base, index, worktree, and untracked files:\n{files}\n\nPatch (included={include_patch}, binary={}, truncated={}):\n{patch}\n</lf:task-workspace>",
        changes.base_commit, changes.head_commit, diff.binary, diff.truncated
    ))
}

pub fn task_file(issue: &str, path: &str, inspect_recovery: bool) -> OpsResult<TaskFileSnapshot> {
    let checkout = file_context(&file_store()?, issue)?;
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

fn resolve_file_base(worktree: &Path, parent: &str, selection: &str) -> OpsResult<String> {
    let reference = match selection {
        "parent" => parent,
        "head" => "HEAD",
        sha if sha.len() == 40 && sha.bytes().all(|byte| byte.is_ascii_hexdigit()) => sha,
        _ => {
            return Err(task_error(
                "Comparison must be parent, head, or a resolved commit SHA",
            ))
        }
    };
    Ok(git_output(
        worktree,
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

#[derive(Debug)]
pub struct TaskEdit {
    pub id: String,
    pub sync: crate::store::planning_sync::PlanningSyncStatus,
}

pub(crate) fn validate_task_edit_input(update: &crate::work::task::TaskUpdate) -> OpsResult<()> {
    if *update == crate::work::task::TaskUpdate::default() {
        return Err(task_error(
            "task edit requires --title, --notes, --rank, --assignee or --unassign",
        ));
    }
    if update
        .title
        .as_deref()
        .is_some_and(|title| title.trim().is_empty())
    {
        return Err(task_error("Task title cannot be empty"));
    }
    Ok(())
}

pub fn task_edit(
    repo: &Path,
    issue: &str,
    wave: Option<&str>,
    update: crate::work::task::TaskUpdate,
) -> OpsResult<TaskEdit> {
    validate_task_edit_input(&update)?;
    block_on_task(async {
        let (store, task) = resolve_saved_task(repo, wave, issue).await?;
        let edited = store
            .edit_task(&task.id, task.revision, &update)
            .await
            .map_err(task_error)?;
        super::planning_sync::sync_after_save(
            &store,
            &repository_for_task(&store, &task)?.to_string_lossy(),
        )
        .await;
        Ok(TaskEdit {
            id: edited.id.to_string(),
            sync: store
                .sqlite
                .task_planning_sync(&edited.id)
                .map_err(task_error)?,
        })
    })
}

pub fn task_move_membership(repo: &Path, issue: &str, wave: Option<&str>) -> OpsResult<TaskEdit> {
    block_on_task(async {
        let (store, task) = resolve_saved_task(repo, None, issue).await?;
        let destination = match wave {
            Some(name) => Some(
                crate::work::wave::context::resolve_managed_wave(
                    Some(&store),
                    Some(repo),
                    Some(name),
                    None,
                )
                .await
                .map_err(task_error)?,
            ),
            None => None,
        };
        let mut waves: Vec<_> = owning_wave(&store, &task)
            .await?
            .into_iter()
            .chain(destination.clone())
            .collect();
        waves.sort_by(|a, b| a.id().as_str().cmp(b.id().as_str()));
        waves.dedup_by(|a, b| a.id() == b.id());
        let mut guards = Vec::new();
        for wave in &waves {
            guards.push(super::planning_sync::lock_wave_planning(wave).await?);
        }
        let project = destination
            .as_ref()
            .map(|wave| super::project::current_project(&store, wave))
            .transpose()?;
        store
            .sqlite
            .move_task(
                &task.id,
                task.project_id.as_ref(),
                project.as_ref().map(|p| &p.id),
            )
            .map_err(task_error)?;
        drop(guards);
        super::planning_sync::sync_after_save(
            &store,
            &repository_for_task(&store, &task)?.to_string_lossy(),
        )
        .await;
        Ok(TaskEdit {
            id: task.id.to_string(),
            sync: store
                .sqlite
                .task_planning_sync(&task.id)
                .map_err(task_error)?,
        })
    })
}

pub fn task_comment(
    repo: &Path,
    issue: &str,
    wave: Option<&str>,
    message: Option<&str>,
    steer: bool,
) -> OpsResult<crate::work::task::comment::TaskComments> {
    block_on_task(task_comment_async(repo, wave, issue, message, steer))
}

pub(crate) fn validate_task_comment_input(message: &str) -> OpsResult<()> {
    if message.trim().is_empty() {
        return Err(task_error("Task comment cannot be empty"));
    }
    Ok(())
}

pub(crate) fn append_task_comment(
    store: &Store,
    task: &Task,
    message: &str,
    steer: bool,
) -> OpsResult<String> {
    validate_task_comment_input(message)?;
    let id = uuid::Uuid::new_v4().to_string();
    let capture = crate::session_record::inherited_capture_key().map_err(task_error)?;
    let provenance =
        capture.or_else(|| crate::journal::agent_caller().map(|caller| caller.session_id.into()));
    let (body, author) = if let Some(provenance) = provenance.filter(|_| !steer) {
        (
            format!(
                "{}\n\n<!-- loopflow-progress:{provenance}:{id} -->",
                message.trim()
            ),
            crate::work::task::comment::TaskCommentAuthor::Integration,
        )
    } else {
        let name = crate::config::participant_name().map_err(task_error)?;
        let requester = name
            .as_ref()
            .map(|name| {
                format!(
                    "\n\n<!-- loopflow-requester:{} -->",
                    serde_json::to_string(name)
                        .expect("name is serializable")
                        .replace('<', "\\u003c")
                        .replace('>', "\\u003e")
                )
            })
            .unwrap_or_default();
        (
            format!(
                "{}{requester}\n\n<!-- loopflow-steer:{id} -->",
                message.trim()
            ),
            crate::work::task::comment::TaskCommentAuthor::Person { name },
        )
    };
    store
        .sqlite
        .append_task_comment(
            &task.id,
            &crate::work::task::comment::TaskComment {
                id: id.clone(),
                body,
                author,
                created_at: Some(
                    time::OffsetDateTime::now_utc()
                        .format(&Rfc3339)
                        .map_err(task_error)?,
                ),
            },
        )
        .map_err(task_error)?;
    Ok(id)
}

/// Stop the Task's live execution and its scheduled automation. The request is
/// a durable interrupt on the Work, recorded only when something live can act
/// on it: a running Graph stops before its next step and ends an agent turn at
/// once, and its Task run starts nothing again.
pub fn task_interrupt(issue: &str) -> OpsResult<TaskControlResult> {
    let issue = issue.to_string();
    block_on_task(async move {
        let store = task_store().await?;
        let task = store
            .get_task_by_issue(&issue)
            .await
            .map_err(|error| task_error(format!("failed to resolve task: {error}")))?
            .ok_or_else(|| task_error(format!("no Task exists for {issue:?}")))?;
        let work = crate::durable::WorkRef::Task(task.id.clone());
        let receipt = match live_execution(&store, &task).await? {
            Some(target) => {
                store.append_interrupt(&work).await.map_err(task_error)?;
                super::child::WorkControlReceipt::Interrupt { work, target }
            }
            None => super::child::WorkControlReceipt::NoLiveRun { work },
        };
        Ok(TaskControlResult {
            issue_id: task.identifier.clone(),
            task_id: task.id.to_string(),
            receipt,
            observation: task.observation.clone(),
        })
    })
}

/// What an interrupt would reach now, as the receipt's text: the Task's latest
/// Graph while its own process lives, otherwise another open Process in its
/// work, such as a Task run between Graph attempts or a Task Session. Unknown
/// liveness counts as live.
async fn live_execution(store: &SharedStore, task: &Task) -> OpsResult<Option<String>> {
    use crate::journal::ProcessIdentityEvidence;
    use crate::ops::task_execution::TaskExecutionState;
    let graph = store
        .sqlite
        .task_graphs(&task.id)
        .map_err(task_error)?
        .pop()
        .filter(|graph| {
            crate::journal::process_evidence(&store.sqlite, graph.id())
                != ProcessIdentityEvidence::Dead
        });
    if let Some(graph) = graph {
        let name = graph.name.clone();
        let (execution, _) =
            crate::ops::task_execution::execution_of_latest_graph(store, Some(graph))
                .await
                .map_err(task_error)?;
        let step = execution.step.unwrap_or_default();
        let position = match execution.state {
            TaskExecutionState::Running | TaskExecutionState::Stalled => Some(format!("at {step}")),
            TaskExecutionState::Starting => Some(format!("between steps after {step}")),
            TaskExecutionState::Unknown => Some(format!("at {step} (liveness unknown)")),
            TaskExecutionState::Idle | TaskExecutionState::Blocked => None,
        };
        if let Some(position) = position {
            return Ok(Some(format!("stopped Graph {name} {position}")));
        }
    }
    let processes = store
        .sqlite
        .task_open_processes(&task.id)
        .map_err(task_error)?;
    let open = crate::ops::task_execution::unresolved_processes(&store.sqlite, &processes)?;
    Ok(open.first().map(|(process, evidence)| {
        let command = process
            .command
            .as_deref()
            .and_then(|command| serde_json::from_str::<Vec<String>>(command).ok())
            .map(|argv| format!(" running `{}`", argv.join(" ")))
            .unwrap_or_default();
        let unknown = match evidence {
            ProcessIdentityEvidence::Unknown => " (liveness unknown)",
            _ => "",
        };
        format!(
            "interrupt recorded for Process {}{command}{unknown}; a Task run or a `--task` Session stops on it",
            process.id
        )
    }))
}

pub fn task_wait(issue: &str, until: TaskWaitUntil, timeout: Option<Duration>) -> OpsResult<Task> {
    let started = Instant::now();
    loop {
        let (task, status) = block_on_task(async {
            let store = task_store().await?;
            let task = store
                .get_task_by_issue(issue)
                .await
                .map_err(task_error)?
                .ok_or_else(|| task_error(format!("no Task exists for {issue:?}")))?;
            let status = task_work_status(&store, &task).await?;
            Ok((task, status))
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

/// Read the saved Task thread or save a comment without allocating execution state.
pub(crate) async fn task_comment_async(
    repo: &Path,
    wave: Option<&str>,
    issue: &str,
    message: Option<&str>,
    steer: bool,
) -> OpsResult<TaskComments> {
    let (store, task) = resolve_saved_task(repo, wave, issue).await?;
    if let Some(message) = message {
        append_task_comment(&store, &task, message, steer)?;
        super::planning_sync::sync_after_save(
            &store,
            &repository_for_task(&store, &task)?.to_string_lossy(),
        )
        .await;
    }
    let refresh_error = if message.is_none() && task.linear_id.is_some() {
        match tokio::time::timeout(
            std::time::Duration::from_secs(5),
            super::linear_observe::refresh_task_comments(&store, &task),
        )
        .await
        {
            Ok(Ok(())) => None,
            Ok(Err(error)) => Some(error.to_string()),
            Err(_) => Some("Comment refresh timed out; showing saved comments".into()),
        }
    } else {
        None
    };
    let mut thread = store
        .sqlite
        .task_comments(&task.id)
        .map_err(|error| OpsError::Message(error.to_string()))?;
    thread.refresh_error = refresh_error;
    Ok(thread)
}

pub(crate) async fn resolve_saved_task(
    repo: &Path,
    wave: Option<&str>,
    issue: &str,
) -> OpsResult<(SharedStore, Task)> {
    let store = Arc::new(super::planning_sync::planning_store().await?);
    let canonical = crate::repository::CanonicalRepo::discover(repo)
        .map_err(|error| OpsError::Message(error.to_string()))?;
    let task = super::planning_sync::find_task(&store, &canonical.to_string(), issue).await?;
    let task = match task {
        Some(task) => task,
        None => {
            // Cold acquisition imports the remote record once. The stored Task owns
            // every subsequent read and write, including while Linear is unavailable.
            match crate::config::load_config(Some(repo))
                .map_err(task_error)?
                .unwrap_or_default()
                .planning_transport()
            {
                crate::config::PlanningConfig::Linear { .. } => {
                    super::linear::acquire_task(repo, issue, super::linear::LinearRefresh::Force)
                        .await?
                }
                crate::config::PlanningConfig::Git { .. } => {
                    return Err(task_error(format!(
                        "Task {issue:?} not found in selected Git planning"
                    )))
                }
                remote => {
                    return Err(task_error(format!(
                    "Task {issue:?} not found in Loopflow planning; {} tasks are not acquired yet",
                    remote.remote_name().unwrap_or("provider")
                )))
                }
            }
        }
    };
    validate_saved_task_scope(&store, repo, wave, &task).await?;
    Ok((store, task))
}

pub(crate) async fn validate_saved_task_scope(
    store: &Store,
    repo: &Path,
    wave: Option<&str>,
    task: &Task,
) -> OpsResult<()> {
    let canonical = crate::repository::CanonicalRepo::discover(repo).map_err(task_error)?;
    let task_repo = repository_for_task(store, task)?;
    if crate::repository::CanonicalRepo::discover(&task_repo).map_err(task_error)? != canonical {
        return Err(task_error("Task belongs to another repository"));
    }
    let expected_wave = match wave {
        Some(selector) => Some(
            crate::work::wave::context::resolve_managed_wave(
                Some(store),
                Some(repo),
                Some(selector),
                None,
            )
            .await
            .map_err(|error| OpsError::Message(error.to_string()))?,
        ),
        None => None,
    };
    if expected_wave
        .as_ref()
        .is_some_and(|expected| Some(expected.id()) != task.wave_id.as_ref())
    {
        return Err(task_error(format!(
            "requested Wave does not own Task {}",
            task.identifier
        )));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::{
        apply_merged_task_landing, checkout_execution_boundary, lock_task_pr_mutation,
        resolve_task_create_input, task_event_process_refusal,
    };
    use crate::child::ChildRef;
    use crate::durable::{TaskState, WorkRef, WorkStatus};
    use crate::linear::{LinearIssueId, LinearProjectId};
    use crate::ops::pr_landing::{
        reconcile_pr_landing, wait_with_io, LandingIo, LandingObservation,
    };
    use crate::pr_landing::{NewPrLanding, PrLanding, PrLandingState};
    use crate::store::{SharedStore, StorageConfig};
    use crate::test_linear_input::SeedLinear;
    use crate::work::project::{Project, ProjectId};
    use crate::work::task::{
        GithubPr, Observation, PrMergeMode, PrMergeRequest, PrPresentation, PrPublication, Task,
        TaskEventKind, TaskId, TaskPr, TaskPrId,
    };
    use crate::work::wave::Wave;
    use std::ffi::OsString;
    use std::os::unix::fs::PermissionsExt;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::sync::{Arc, Mutex};
    use std::time::Duration;

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
    fn paused_snapshot_never_recommends_running_work() {
        let _ledger = crate::journal::TestLedgerGuard::new();
        let runtime = tokio::runtime::Runtime::new().unwrap();
        let repo = loopflow_test_support::TestRepo::new();
        let fixture = runtime.block_on(task_fixture_at("DEFER-1", repo.path().into()));
        fixture
            .store
            .sqlite
            .set_task_deferred(&fixture.task.id, true)
            .unwrap();
        let snapshot = runtime
            .block_on(super::read_task_snapshot(&fixture.store, fixture.task))
            .unwrap();
        assert_eq!(snapshot.status, crate::durable::TaskState::Deferred);
        assert_eq!(
            snapshot.actions.recommended,
            Some(crate::ops::task_actions::TaskAction::NoAction)
        );
        assert!(snapshot.actions.reason.contains("lf task resume DEFER-1"));
    }

    #[test]
    fn task_decision_preserves_unknown_history_and_live_process_protection() {
        let ledger = crate::journal::TestLedgerGuard::new();
        let runtime = tokio::runtime::Runtime::new().unwrap();
        let repo = loopflow_test_support::TestRepo::new();
        let fixture = runtime.block_on(task_fixture_at("ACCEPT-HISTORY", repo.path().into()));
        let mut pr = runtime
            .block_on(fixture.store.task_prs(&fixture.task.id))
            .unwrap()
            .remove(0);
        repo.create_branch(&pr.branch);
        pr.base_commit = repo.head_sha();
        runtime
            .block_on(fixture.store.sync_task_placement(
                &pr.task_id,
                &pr.base_commit,
                false,
                pr.updated_at,
            ))
            .unwrap();

        rusqlite::Connection::open(&fixture.database_path)
            .unwrap()
            .execute(
                "DELETE FROM task_prs WHERE task_id=?1",
                [fixture.task.id.as_str()],
            )
            .unwrap();
        let process = crate::process::LfProcess {
            kind: crate::process::ProcessKind::Lf,
            agent_session_id: None,
            os_started_at: None,
            id: crate::id::LfProcessId::new(),
            pid: None,
            trace_id: crate::id::TraceId::new(),
            parent_lf_process_id: None,
            via_agent: None,
            caller_session_id: None,
            caller_agent_process_id: None,
            command: Some("historical diagnostic".into()),
            repo: None,
            cwd: Some(repo.path().to_string_lossy().into_owned()),
            started_at: time::OffsetDateTime::now_utc().unix_timestamp(),
            completed_at: None,
            outcome: None,
            exit_code: None,
            signal: None,
            error: None,
        };
        fixture.store.sqlite.record_process(&process).unwrap();
        let session = fixture.store.sqlite.test_session(
            "reserved-session",
            &crate::session_record::new_artifact_key(),
        );
        let conn = rusqlite::Connection::open(&fixture.database_path).unwrap();
        conn.execute("UPDATE agent_sessions SET cwd=?1,task_id=?2,wave_id=?3,input_published=0,interactive=0 WHERE id=?4",
            rusqlite::params![repo.path().to_str().unwrap(), fixture.task.id.as_str(), fixture.task.wave_id.as_ref().unwrap().as_str(), session.id]).unwrap();
        conn.execute("INSERT INTO session_events(session_id,kind,receipt_key,observed_at,payload) VALUES(?1,'captured',?2,1,'{}')",
            rusqlite::params![session.id, crate::session_record::new_artifact_key()]).unwrap();
        conn.execute(
            "UPDATE agent_sessions SET current_capture=?1 WHERE id=?2",
            rusqlite::params![conn.last_insert_rowid(), session.id],
        )
        .unwrap();
        fixture
            .store
            .sqlite
            .record_session_event(
                &session.id,
                &"thread".into(),
                "turn",
                crate::session::SessionEventKind::Started,
                &serde_json::json!({}),
            )
            .unwrap();
        let before = fixture.store.sqlite.session(&session.id).unwrap();
        // A reserved input and an unanswered turn are history; only the
        // Process with unknown evidence blocks.
        let blockers = crate::ops::task_execution::task_execution_blockers(
            &fixture.store.sqlite,
            &fixture.task.id,
        )
        .unwrap();
        assert_eq!(blockers.len(), 1, "{blockers:?}");
        assert!(blockers[0].contains(&format!("lf history show {}", process.id)));
        let gate = runtime
            .block_on(super::task_completion_refusal(
                &fixture.store,
                &fixture.task,
            ))
            .unwrap();
        assert!(gate.is_none(), "{:?}", gate);

        // Exact live identity does not change the decision or confer stop authority.
        let root = ledger.home().join(crate::journal::PROCESS_RECEIPT_ROOT);
        std::fs::create_dir_all(&root).unwrap();
        let pid = std::process::id();
        let receipt = crate::journal::ProcessReceipt {
            schema_version: 1,
            trace_id: process.trace_id.to_string(),
            lf_process_id: process.id.to_string(),
            pid,
            started_at: crate::journal::process_started_at(pid).unwrap().unwrap(),
        };
        let receipt_path = root.join(format!("{}.json", process.id));
        let receipt_bytes = serde_json::to_vec(&receipt).unwrap();
        std::fs::write(&receipt_path, &receipt_bytes).unwrap();
        assert!(runtime
            .block_on(super::task_completion_refusal(
                &fixture.store,
                &fixture.task
            ))
            .unwrap()
            .is_none());
        for _ in 0..2 {
            runtime
                .block_on(
                    fixture.store.complete_task(
                        &fixture.task,
                        fixture
                            .store
                            .sqlite
                            .request_task_completion(&fixture.task.id, None)
                            .unwrap()
                            .unwrap_or(0),
                    ),
                )
                .unwrap();
            runtime
                .block_on(super::cleanup_completed_task(&fixture.store, &fixture.task))
                .unwrap();
        }
        assert!(repo.path().exists());
        assert_eq!(
            runtime
                .block_on(super::task_work_status(&fixture.store, &fixture.task))
                .unwrap(),
            WorkStatus::Done
        );
        assert_eq!(
            fixture.store.sqlite.process(&process.id).unwrap(),
            Some(process)
        );
        assert_eq!(fixture.store.sqlite.session(&session.id).unwrap(), before);
        assert_eq!(std::fs::read(receipt_path).unwrap(), receipt_bytes);
        assert!(crate::ops::task_execution::task_execution_blockers(
            &fixture.store.sqlite,
            &fixture.task.id,
        )
        .unwrap()
        .iter()
        .any(|reason| reason.contains("live or unresolved")));
    }

    #[test]
    fn prepared_placement_consumes_accepted_facts_before_checkout_or_execution() {
        let _ledger = crate::journal::TestLedgerGuard::new();
        let runtime = tokio::runtime::Runtime::new().unwrap();
        {
            let repo = loopflow_test_support::TestRepo::new();
            let fixture = runtime.block_on(task_fixture_at("EXISTING", repo.path().to_path_buf()));
            std::env::set_var("LF_HOME", fixture._database.path());
            std::fs::create_dir_all(repo.path().join(".lf/wave/task-recovery")).unwrap();
            std::fs::write(
                repo.path().join(".lf/wave/task-recovery/GOAL.md"),
                "Recover work.",
            )
            .unwrap();
            let mut snapshot: crate::test_linear_input::Inventory = serde_json::from_str(
                include_str!("../../../../tests/fixtures/dto/task_history_planning.json"),
            )
            .unwrap();
            snapshot.projects.truncate(1);
            snapshot.items.truncate(1);
            snapshot.projects[0].id = "999bdbdd-c045-41a6-8ffc-a97c4a40b0b3".into();
            snapshot.projects[0].status = crate::work::project::ProjectStatus::Started;
            snapshot.items[0].id = "new-issue".into();
            snapshot.items[0].completed = false;
            snapshot.items[0].completed_at = None;
            snapshot.items[0].state = Some("unstarted".into());
            snapshot.items[0].project_id = Some("999bdbdd-c045-41a6-8ffc-a97c4a40b0b3".into());
            snapshot.items[0].revision = Some("2026-10-05T12:00:00Z".into());
            snapshot.items[0].name = "Accepted after resolution".into();
            snapshot.items[0].description = "Preserve this direction".into();
            snapshot.items[0].revision = Some("2026-10-05T13:00:00Z".into());
            runtime
                .block_on(fixture.store.seed_inventory(
                    crate::test_linear_input::InventoryFixture {
                        wave_id: fixture.task.wave_id.clone().unwrap(),
                        provider: "linear".into(),
                        initiative: snapshot.projects[0].initiative_ids[0].clone(),
                        synced_at: 7,
                        snapshot,
                    },
                    None,
                ))
                .unwrap();
            let checkout = repo.path().join("reserved-checkout");
            let prepared = |path| super::TaskPlacement {
                plan: crate::git::worktrees::PlacementPlan {
                    base_ref: repo.head_sha(),
                    branch: "retained-branch".into(),
                    worktree_path: path,
                    strategy: crate::git::worktrees::PlacementStrategy::UseExistingWorktree,
                },
                workspace_slug: "reserved-checkout".into(),
                stack_parent: None,
                github: None,
                requested_agent: None,
                directive: None,
            };
            let saved = runtime
                .block_on(fixture.store.get_task_by_issue("new-issue"))
                .unwrap()
                .unwrap();
            let task = runtime
                .block_on(super::place_prepared_task(
                    &fixture.store,
                    &saved.id,
                    prepared(checkout.clone()),
                ))
                .unwrap();
            {
                assert_eq!(task.title, "Accepted after resolution");
                assert_eq!(task.description, "Preserve this direction");
                assert_eq!(task.pm_snapshot_synced_at, Some(7));
                assert_eq!(task.worktree.as_ref(), Some(&checkout));
                assert_eq!(
                    runtime.block_on(fixture.store.get_task(&task.id)).unwrap(),
                    Some(task.clone())
                );
                assert!(runtime
                    .block_on(fixture.store.active_task_pr(&task.id))
                    .unwrap()
                    .is_none());
                assert_eq!(task.branch, "retained-branch");
                assert_eq!(task.base_commit, repo.head_sha());
            }
            assert!(runtime
                .block_on(fixture.store.get_task(&fixture.task.id))
                .unwrap()
                .is_some());
        }
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

    #[test]
    fn a_dead_graph_is_history_while_a_live_graph_process_retains_the_checkout() {
        let _ledger = crate::journal::TestLedgerGuard::new();
        let runtime = tokio::runtime::Runtime::new().unwrap();
        let fixture = runtime.block_on(task_fixture("WORK-1"));
        let worktree = fixture
            .task
            .worktree
            .as_ref()
            .unwrap()
            .to_string_lossy()
            .into_owned();
        let blockers = || {
            crate::ops::task_execution::task_execution_blockers(
                &fixture.store.sqlite,
                &fixture.task.id,
            )
            .unwrap()
        };
        fixture.store.sqlite.test_graph(
            "code",
            &worktree,
            &[("implement", Some("failed"))],
            Some("failed"),
        );
        assert!(
            blockers().is_empty(),
            "a Graph whose process exited is history"
        );
        let live = fixture
            .store
            .sqlite
            .test_graph("code", &worktree, &[("implement", None)], None);
        assert!(blockers()
            .iter()
            .any(|reason| reason.contains(live.as_str())));
        let conn = rusqlite::Connection::open(&fixture.database_path).unwrap();
        conn.execute(
            "UPDATE processes SET completed_at=started_at,outcome='interrupted' WHERE completed_at IS NULL",
            [],
        )
        .unwrap();
        assert!(blockers().is_empty());

        // A reboot proves stale execution exited without settling the Graph.
        let now = time::OffsetDateTime::now_utc().unix_timestamp();
        let process = crate::process::LfProcess {
            kind: crate::process::ProcessKind::Lf,
            agent_session_id: None,
            os_started_at: None,
            id: crate::id::LfProcessId::new(),
            pid: None,
            trace_id: crate::id::TraceId::new(),
            parent_lf_process_id: None,
            via_agent: None,
            caller_session_id: None,
            caller_agent_process_id: None,
            command: Some("implement".into()),
            repo: None,
            cwd: Some(
                fixture
                    .task
                    .worktree
                    .as_ref()
                    .unwrap()
                    .to_string_lossy()
                    .into_owned(),
            ),
            started_at: now - 100,
            completed_at: None,
            outcome: None,
            exit_code: None,
            signal: None,
            error: None,
        };
        fixture.store.sqlite.record_process(&process).unwrap();
        let session = fixture
            .store
            .sqlite
            .test_session("stalled", &crate::session_record::new_artifact_key());
        conn.execute(
            "UPDATE agent_sessions SET cwd=?1 WHERE id=?2",
            rusqlite::params![
                fixture.task.worktree.as_ref().unwrap().to_str().unwrap(),
                session.id
            ],
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
        // The stalled Session's unanswered turn is history, not execution.
        assert_eq!(unresolved.len(), 1, "{unresolved:?}");
        assert!(unresolved[0].contains(process.id.as_str()));
        assert!(after_boot.is_empty(), "{after_boot:?}");
        assert_eq!(
            fixture.store.sqlite.process(&process.id).unwrap(),
            Some(process)
        );
        assert_eq!(
            fixture
                .store
                .sqlite
                .task_graphs(&fixture.task.id)
                .unwrap()
                .len(),
            2
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

            summary: String::new(),
            status: crate::work::project::ProjectStatus::Started,
            linear_id: Some(LinearProjectId::new("999bdbdd-c045-41a6-8ffc-a97c4a40b0b3").unwrap()),
            slug: "task-recovery".to_string(),
            name: "Task recovery".to_string(),
            description: "Keep automatic Task recovery bounded.".to_string(),
            krs: Vec::new(),
            metric_targets: Vec::new(),
            pm_snapshot_synced_at: Some(now.unix_timestamp()),

            wave_id: wave.id().clone(),
            created_at: now,
            updated_at: now,
        };
        let task = Task {
            repository_id: store.sqlite.ensure_repository(wave.repo()).unwrap(),
            id: TaskId::new(),

            revision: 0,
            linear_id: Some(LinearIssueId::new(format!("{identifier}-issue")).unwrap()),
            identifier: identifier.to_string(),
            title: "Task recovery fixture".to_string(),
            description: String::new(),
            pm_snapshot_synced_at: Some(now.unix_timestamp()),

            pm_writeback: crate::work::task::LinearWritebackState::Current,
            wave_id: Some(wave.id().clone()),
            project_id: Some(project.id.clone()),
            worktree: Some(repository),
            workspace_slug: "task-recovery-fixture".to_string(),
            branch: "test/task-recovery-fixture".to_string(),
            base_commit: "deadbeef".to_string(),
            parent_pr_id: None,
            agent: None,
            created_at: now,
            updated_at: now,
            rank: 0,
            assignee: None,
            due_date: None,
            state: None,
            completed_at: None,
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
        crate::store::sqlite::project_selection::write_project_binding(
            &store.sqlite,
            wave.id(),
            None,
            project.linear_id.as_ref().unwrap().as_str(),
            &crate::store::PlanningLocks::new(tempfile::tempfile().unwrap()),
        )
        .unwrap();
        store.seed_task(&task, &pr).await.unwrap();
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
    fn task_checkout_machine_survives_without_lifecycle_reconciliation() {
        let repo = loopflow_test_support::TestRepo::new();
        let runtime = tokio::runtime::Runtime::new().unwrap();
        let fixture = runtime.block_on(task_fixture_at("FILES-1", repo.path().to_path_buf()));
        let local = fixture.store.sqlite.local_machine().unwrap().id;
        let remote = crate::durable::MachineId::new();
        fixture
            .store
            .sqlite
            .add_machine(&remote, "worker", "worker")
            .unwrap();
        let mut pr = runtime
            .block_on(fixture.store.active_task_pr(&fixture.task.id))
            .unwrap()
            .unwrap();
        repo.create_branch(&pr.branch);
        pr.base_commit = super::git_output(repo.path(), &["rev-parse", "HEAD"])
            .unwrap()
            .trim()
            .into();
        rusqlite::Connection::open(&fixture.database_path)
            .unwrap()
            .execute(
                "UPDATE tasks SET base_commit=?2 WHERE id=?1",
                rusqlite::params![fixture.task.id.as_str(), pr.base_commit],
            )
            .unwrap();
        std::fs::create_dir_all(repo.path().join("scratch")).unwrap();
        std::fs::write(repo.path().join("scratch/note.md"), "local note\n").unwrap();
        let original_task = runtime
            .block_on(fixture.store.get_task(&fixture.task.id))
            .unwrap()
            .unwrap();
        let connection = rusqlite::Connection::open(&fixture.database_path).unwrap();
        connection
            .execute(
                "INSERT INTO agent_sessions(id,title,title_source,created_at,input_published,cwd)
             VALUES('checkout-session','Retained','human',1,0,?1)",
                [repo.path().to_str().unwrap()],
            )
            .unwrap();
        assert_eq!(
            fixture
                .store
                .sqlite
                .task_work(&fixture.task.id)
                .unwrap()
                .sessions
                .len(),
            1
        );
        connection.execute_batch(
            "CREATE TRIGGER no_file_task_update BEFORE UPDATE ON tasks BEGIN SELECT RAISE(FAIL,'file access cannot update Task'); END;
             CREATE TRIGGER no_file_pr_update BEFORE UPDATE ON task_prs BEGIN SELECT RAISE(FAIL,'file access cannot reconcile PR'); END;"
        ).unwrap();
        crate::ops::linear::LINEAR_TEST_CONTEXT.sync_scope(
            crate::ops::linear::LinearTestContext {
                path: fixture.database_path.clone(),
                store: fixture.store.clone(),
                graphql_url: "http://127.0.0.1:1".into(),
            },
            || {
                let snapshot = super::task_snapshot(&fixture.task).unwrap();
                assert_eq!(snapshot.machine_id, Some(local.clone()));
                assert_eq!(snapshot.task_id, fixture.task.id.as_str());
                assert_eq!(snapshot.pr.as_ref().map(|pr| &pr.id), Some(&pr.id));
                for issue in [
                    "FILES-1",
                    fixture.task.id.as_str(),
                    &fixture.task.id.as_str()[5..9],
                    &fixture.task.id.as_str()[..9],
                    fixture.task.linear_id.as_ref().unwrap().as_str(),
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
        crate::ops::linear::LINEAR_TEST_CONTEXT.sync_scope(
            crate::ops::linear::LinearTestContext {
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
                let changes = super::task_changes("FILES-1", "parent").unwrap();
                assert_eq!(changes.base_commit, pr.base_commit);
                assert_eq!(changes.scratch, ["scratch/note.md"]);
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
    fn task_checkout_machine_unknown_or_remote_never_reads_a_local_path() {
        let repo = loopflow_test_support::TestRepo::new();
        let runtime = tokio::runtime::Runtime::new().unwrap();
        let fixture = runtime.block_on(task_fixture_at("LOCATION-1", repo.path().into()));
        std::fs::write(repo.path().join("note.md"), "local file").unwrap();
        let remote = crate::durable::MachineId::new();
        fixture
            .store
            .sqlite
            .add_machine(&remote, "remote", "remote")
            .unwrap();
        let connection = rusqlite::Connection::open(&fixture.database_path).unwrap();
        connection
            .execute(
                "INSERT INTO agent_sessions(id,title,title_source,created_at,input_published,cwd)
             VALUES('unbound','Local conversation','human',1,0,?1),
                   ('bound','Explicit conversation','human',1,0,?1)",
                [repo.path().to_str().unwrap()],
            )
            .unwrap();
        connection
            .execute(
                "UPDATE agent_sessions SET task_id=?1,wave_id=?2 WHERE id='bound'",
                rusqlite::params![
                    fixture.task.id.as_str(),
                    fixture.task.wave_id.as_ref().unwrap().as_str()
                ],
            )
            .unwrap();
        for machine in [None, Some(remote.as_str())] {
            connection
                .execute(
                    "UPDATE tasks SET checkout_machine_id=?1 WHERE id=?2",
                    rusqlite::params![machine, fixture.task.id.as_str()],
                )
                .unwrap();
            crate::ops::linear::LINEAR_TEST_CONTEXT.sync_scope(
                crate::ops::linear::LinearTestContext {
                    path: fixture.database_path.clone(),
                    store: fixture.store.clone(),
                    graphql_url: "http://127.0.0.1:1".into(),
                },
                || {
                    let expected = if machine.is_none() {
                        "Machine is unknown"
                    } else {
                        "belongs to Machine"
                    };
                    assert!(super::task_file("LOCATION-1", "note.md", false)
                        .unwrap_err()
                        .to_string()
                        .contains(expected));
                    assert!(super::task_changes("LOCATION-1", "head")
                        .unwrap_err()
                        .to_string()
                        .contains(expected));
                    assert!(super::task_files("LOCATION-1", "", None, false)
                        .unwrap_err()
                        .to_string()
                        .contains(expected));
                    let work = fixture.store.sqlite.task_work(&fixture.task.id).unwrap();
                    assert_eq!(
                        work.sessions
                            .iter()
                            .map(|s| s.id.as_str())
                            .collect::<Vec<_>>(),
                        ["bound"]
                    );
                },
            );
        }
        assert_eq!(
            std::fs::read_to_string(repo.path().join("note.md")).unwrap(),
            "local file"
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
        let head = super::resolve_file_base(repo.path(), &parent, "head").unwrap();
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
        let TaskFixture { mut task, .. } = task_fixture_at(
            "TEST-WORKSPACE",
            std::fs::canonicalize(repository.path()).unwrap(),
        )
        .await;
        task.worktree = Some(std::fs::canonicalize(repository.path()).unwrap());
        task.base_commit = base;

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

        let context = super::task_workspace_context(&task).unwrap();

        for expected in [
            "\"tracked.txt\": committed, unstaged",
            "\"staged.txt\": staged",
            "\"untracked.txt\": untracked",
            "unstaged bytes",
            "staged bytes",
            "untracked bytes",
        ] {
            assert!(context.contains(expected), "missing {expected:?}");
        }

        // A large branch keeps every path and state without filling the goal
        // budget with per-file hashes, nulls and repeated JSON field names.
        for index in 0..300 {
            std::fs::write(repository.path().join(format!("file-{index:03}.bin")), [0]).unwrap();
        }
        let unusual = "quoted\"\npath.bin";
        std::fs::write(repository.path().join(unusual), [0]).unwrap();
        let context = super::task_workspace_context(&task).unwrap();
        for index in 0..300 {
            assert!(context.contains(&format!("\"file-{index:03}.bin\": untracked")));
        }
        assert!(context.contains(&format!(
            "{}: untracked",
            serde_json::to_string(unusual).unwrap()
        )));
        assert!(context.contains("Patch omitted"));
        assert!(!context.contains("content_sha256"));
        assert!(crate::prompt::count_tokens(&context) < 5_000);
    }

    #[tokio::test]
    #[allow(clippy::await_holding_lock)] // the guard serializes process-wide capture environment
    async fn parent_run_cannot_override_task_worktree_resolution() {
        let _lock = crate::journal::test_env_lock();
        let _environment =
            EnvRestore::capture(&[crate::session_record::CAPTURE_KEY_ENV, "LF_RUN_DIR"]);
        let repository = loopflow_test_support::TestRepo::new();
        repository.create_branch("test/task-recovery-fixture");
        repository.push_new_branch("test/task-recovery-fixture");
        let TaskFixture { store, task, .. } =
            task_fixture_at("TEST-PARENT", repository.path().to_path_buf()).await;
        let parent_run_id = crate::session_record::new_artifact_key();
        std::env::set_var(
            crate::session_record::CAPTURE_KEY_ENV,
            parent_run_id.as_str(),
        );
        std::env::remove_var("LF_RUN_DIR");

        let resolved = super::task_for_checkout(&store, task.worktree.as_ref().unwrap())
            .await
            .unwrap()
            .unwrap();

        assert_eq!(resolved.id, task.id);
    }

    #[tokio::test]
    async fn checkout_resolution_respects_machine_identity() {
        let repo = loopflow_test_support::TestRepo::new();
        let fixture = task_fixture_at("LOCATION-1", repo.path().canonicalize().unwrap()).await;
        let peer = crate::durable::MachineId::new();
        fixture
            .store
            .add_machine(&peer, "peer", "peer")
            .await
            .unwrap();
        let conn = rusqlite::Connection::open(&fixture.database_path).unwrap();
        conn.execute(
            "UPDATE tasks SET checkout_machine_id=?2 WHERE id=?1",
            rusqlite::params![fixture.task.id.as_str(), peer.as_str()],
        )
        .unwrap();
        assert!(super::task_for_checkout(&fixture.store, repo.path())
            .await
            .unwrap()
            .is_none());
        conn.execute(
            "UPDATE tasks SET checkout_machine_id=NULL WHERE id=?1",
            [fixture.task.id.as_str()],
        )
        .unwrap();
        assert!(super::task_for_checkout(&fixture.store, repo.path())
            .await
            .unwrap_err()
            .to_string()
            .contains("Machine is unknown"));
    }

    #[tokio::test(start_paused = true)]
    async fn waited_landing_survives_pending_and_repair_then_completes_requested_task() {
        let ledger = crate::journal::TestLedgerGuard::new();
        let restore = EnvRestore::capture(&["PATH"]);
        let repository = loopflow_test_support::TestRepo::new();
        let fixture = task_fixture_at("LOO-503", repository.path().canonicalize().unwrap()).await;
        ledger.set_db_path(fixture.database_path.clone());
        let store = &fixture.store;
        let task = &fixture.task;
        repository.create_branch(&task.branch);
        assert!(std::process::Command::new("git")
            .args([
                "config",
                &format!("url.{}.insteadOf", repository.bare_path().display()),
                "https://github.com/loopflowstudio/loopflow.git",
            ])
            .current_dir(repository.path())
            .status()
            .unwrap()
            .success());
        assert!(std::process::Command::new("git")
            .args([
                "remote",
                "set-url",
                "origin",
                "https://github.com/loopflowstudio/loopflow.git"
            ])
            .current_dir(repository.path())
            .status()
            .unwrap()
            .success());

        // Task settlement independently reads GitHub. No test writes a merged
        // PR or completed Task into the database.
        let github = ledger.home().join("github.json");
        std::fs::write(&github, "{}").unwrap();
        let gh = ledger.home().join("gh");
        std::fs::write(
            &gh,
            format!(
                "#!/bin/sh\nif [ \"$1\" = --version ]; then exit 0; fi\ncat '{}'\n",
                github.display()
            ),
        )
        .unwrap();
        std::fs::set_permissions(&gh, std::fs::Permissions::from_mode(0o755)).unwrap();
        std::env::set_var(
            "PATH",
            format!(
                "{}:{}",
                ledger.home().display(),
                std::env::var("PATH").unwrap()
            ),
        );

        let now = time::OffsetDateTime::now_utc();
        let mut pr = store.active_task_pr(&task.id).await.unwrap().unwrap();
        pr.publication = Some(PrPublication {
            requested_at: now,
            presentation: Some(PrPresentation {
                title: "Wait through repair".into(),
                body: "Complete only after verified delivery.".into(),
                head_sha: "original".into(),
            }),
            github: Some(GithubPr {
                number: 503,
                url: "https://github.com/loopflowstudio/loopflow/pull/503".into(),
                head_sha: Some("original".into()),
            }),
            merge: Some(PrMergeRequest {
                mode: PrMergeMode::Auto,
                requested_at: now,
                head_sha: "original".into(),
            }),
        });
        store.update_task_pr(&pr).await.unwrap();
        store
            .sqlite
            .request_task_completion(&task.id, Some("Accepted delivery"))
            .unwrap();
        let landing = PrLanding::new(
            NewPrLanding {
                repo: "loopflowstudio/loopflow".into(),
                pr_number: 503,
                worktree: task.worktree.clone().unwrap(),
                branch: task.branch.clone(),
                task_id: Some(task.id.clone()),
                requested_head_sha: "original".into(),
            },
            now,
        )
        .unwrap();
        let landing = store.start_or_join_pr_landing(&landing).await.unwrap();

        let phase = Arc::new(AtomicUsize::new(0));
        let (observed, mut observations) = tokio::sync::mpsc::unbounded_channel();
        let observe: crate::ops::pr_landing::ObservePr = {
            let phase = phase.clone();
            Arc::new(move |_| {
                let phase = phase.load(Ordering::SeqCst);
                observed.send(phase).unwrap();
                Ok(match phase {
                    0 => LandingObservation::Pending {
                        head_sha: "original".into(),
                    },
                    1 => LandingObservation::Failing {
                        head_sha: "original".into(),
                        failing_checks: vec![crate::work::task::CiCheck {
                            name: "rust".into(),
                            url: None,
                        }],
                    },
                    2 => LandingObservation::Pending {
                        head_sha: "repaired".into(),
                    },
                    _ => LandingObservation::Merged {
                        head_sha: "repaired".into(),
                        merge_commit: "merge".into(),
                    },
                })
            })
        };
        let repairs = Arc::new(AtomicUsize::new(0));
        let (started, running) = tokio::sync::oneshot::channel();
        let (release, held) = std::sync::mpsc::channel();
        let repair: crate::ops::pr_landing::RepairCi = {
            let started = Mutex::new(Some(started));
            let held = Mutex::new(held);
            let repairs = repairs.clone();
            let store = store.clone();
            let phase = phase.clone();
            Arc::new(move |landing, _, _| {
                repairs.fetch_add(1, Ordering::SeqCst);
                started.lock().unwrap().take().unwrap().send(()).unwrap();
                held.lock().unwrap().recv().map_err(super::task_error)?;
                // Model the repair provider's publication through the same
                // persisted head/request boundary used by arm.
                let mut pr = store
                    .sqlite
                    .active_task_pr(landing.task_id.as_ref().unwrap())
                    .unwrap()
                    .unwrap();
                let publication = pr.publication.as_mut().unwrap();
                publication.presentation.as_mut().unwrap().head_sha = "repaired".into();
                publication.github.as_mut().unwrap().head_sha = Some("repaired".into());
                publication.merge.as_mut().unwrap().head_sha = "repaired".into();
                store.sqlite.update_task_pr(&pr).unwrap();
                let mut requested = landing.clone();
                requested.requested_head_sha = "repaired".into();
                store.sqlite.start_or_join_pr_landing(&requested).unwrap();
                phase.store(2, Ordering::SeqCst);
                Ok(())
            })
        };
        let mut waiter = tokio::spawn({
            let store = store.clone();
            let id = landing.id.clone();
            let observe = observe.clone();
            let repair = repair.clone();
            async move { wait_with_io(&store, &id, LandingIo::scripted(observe, Some(repair))).await }
        });

        assert_eq!(
            tokio::select! {
                observation = observations.recv() => observation,
                result = &mut waiter => panic!("waiter stopped before pending observation: {result:?}"),
            },
            Some(0)
        );
        tokio::time::advance(Duration::from_secs(24 * 60 * 60)).await;
        assert_eq!(
            tokio::select! {
                observation = observations.recv() => observation,
                result = &mut waiter => panic!("waiter stopped during pending CI: {result:?}"),
            },
            Some(0)
        );
        assert!(!waiter.is_finished());
        assert_eq!(repairs.load(Ordering::SeqCst), 0);
        assert!(store
            .ci_incidents_since(time::OffsetDateTime::UNIX_EPOCH, None, None)
            .await
            .unwrap()
            .is_empty());
        assert_ne!(store.task_state(&task.id).await.unwrap(), TaskState::Done);

        phase.store(1, Ordering::SeqCst);
        tokio::select! {
            result = running => result.unwrap(),
            result = &mut waiter => panic!("waiter stopped before repair: {result:?}"),
        }
        tokio::time::advance(Duration::from_secs(24 * 60 * 60)).await;
        assert!(!waiter.is_finished());
        let competing = reconcile_pr_landing(
            store.clone(),
            landing.clone(),
            LandingIo::scripted(observe, Some(repair)),
        )
        .await
        .unwrap();
        assert_eq!(
            competing, landing,
            "the competing observer cannot claim the landing"
        );
        assert_eq!(
            store
                .get_pr_landing(&landing.id)
                .await
                .unwrap()
                .unwrap()
                .state,
            PrLandingState::Repairing
        );
        assert_eq!(repairs.load(Ordering::SeqCst), 1);
        release.send(()).unwrap();
        loop {
            let observation = tokio::select! {
                observation = observations.recv() => observation,
                result = &mut waiter => panic!("waiter stopped before observing publication: {result:?}"),
            };
            match observation {
                Some(2) => break,
                Some(_) => {}
                None => panic!("waiter stopped before observing the repaired head"),
            }
        }
        assert!(!waiter.is_finished());
        assert_ne!(store.task_state(&task.id).await.unwrap(), TaskState::Done);
        let pr = store.active_task_pr(&task.id).await.unwrap().unwrap();
        assert_eq!(pr.head_sha(), Some("repaired"));
        assert_eq!(pr.merge_request().unwrap().head_sha, "repaired");

        std::fs::write(&github, r#"{"merged":true,"state":"closed","draft":false,"merge_commit_sha":"merge","merged_at":"2026-10-17T00:00:00Z","number":503,"html_url":"https://github.com/loopflowstudio/loopflow/pull/503","head":{"sha":"repaired"}}"#).unwrap();
        phase.store(3, Ordering::SeqCst);
        waiter.await.unwrap().unwrap();
        assert_eq!(store.task_state(&task.id).await.unwrap(), TaskState::Done);
        let saved = store.get_pr_landing(&landing.id).await.unwrap().unwrap();
        assert_eq!(saved.state, PrLandingState::Merged);
        assert_eq!(saved.requested_head_sha, "repaired");
        assert_eq!(saved.observed_head_sha, "repaired");
        assert_eq!(saved.merge_commit.as_deref(), Some("merge"));
        let incidents = store
            .ci_incidents_since(time::OffsetDateTime::UNIX_EPOCH, None, None)
            .await
            .unwrap();
        assert_eq!(incidents.len(), 1);
        assert_eq!(incidents[0].incident.failure_set, ["rust"]);
        assert!(incidents[0].incident.merged_at.is_some());
        assert_eq!(repairs.load(Ordering::SeqCst), 1);
        drop(restore);
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
        } = task_fixture_at("LOO-248", repository.path().canonicalize().unwrap()).await;
        let now = time::OffsetDateTime::now_utc();
        let mut pr = store.active_task_pr(&task.id).await.unwrap().unwrap();
        pr.branch = "HEAD".to_string();
        rusqlite::Connection::open(database_path)
            .unwrap()
            .execute(
                "UPDATE tasks SET branch=?2 WHERE id=?1",
                rusqlite::params![task.id.as_str(), pr.branch],
            )
            .unwrap();
        let landed_head = crate::git::rev_parse(task.worktree.as_ref().unwrap(), "HEAD").unwrap();
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
            }),
        });
        store.update_task_pr(&pr).await.unwrap();
        let landing = crate::pr_landing::PrLanding::new(
            crate::pr_landing::NewPrLanding {
                repo: "loopflowstudio/loopflow".to_string(),
                pr_number: 248,
                worktree: task.worktree.as_ref().unwrap().clone(),
                branch: pr.branch.clone(),
                task_id: Some(task.id.clone()),
                requested_head_sha: landed_head.clone(),
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
        store.settle_task_pr(&pr).await.unwrap();
        let follow_up = crate::work::task::TaskFollowUp {
            outcome: "Installed latency meets budget".into(),
            evidence: "20 samples below 1s p95".into(),
            check_at: now.unix_timestamp() - 1,
        };
        store
            .append_task_event(
                &task.id,
                &TaskEventKind::FollowUp {
                    remaining: Some(follow_up.clone()),
                    reason: "Accepted production check".into(),
                },
            )
            .await
            .unwrap();
        for _ in 0..2 {
            apply_merged_task_landing(&store, &mut task, &pr, &landing)
                .await
                .unwrap();
            assert_eq!(store.work_status(&work).await.unwrap(), WorkStatus::Ready);
            assert!(super::task_completion_refusal(&store, &task)
                .await
                .unwrap()
                .is_none());
            assert!(store.task_events_after(&task.id, 0).await.unwrap().iter().any(|event|
                matches!(&event.kind, TaskEventKind::FollowUp { remaining: Some(saved), .. } if saved == &follow_up)
            ));
        }
        apply_merged_task_landing(&store, &mut task, &pr, &landing)
            .await
            .unwrap();
        assert_eq!(store.work_status(&work).await.unwrap(), WorkStatus::Ready);
        store
            .sqlite
            .request_task_completion(&task.id, Some("Accepted outcome complete"))
            .unwrap();
        apply_merged_task_landing(&store, &mut task, &pr, &landing)
            .await
            .unwrap();
        assert_eq!(store.work_status(&work).await.unwrap(), WorkStatus::Done);
        apply_merged_task_landing(&store, &mut task, &pr, &landing)
            .await
            .unwrap();
        assert_eq!(store.work_status(&work).await.unwrap(), WorkStatus::Done);

        let current = store.active_task_pr(&task.id).await.unwrap().unwrap();
        let task = store.get_task(&task.id).await.unwrap().unwrap();
        let mut next = super::task_pr_for_publication(&task);
        next.sequence = current.sequence + 1;
        next.branch = "wrong-placement".into();
        assert!(store.insert_task_pr(&next).await.is_err());
        assert_eq!(store.active_task_pr(&task.id).await.unwrap(), Some(current));
        assert_eq!(store.work_status(&work).await.unwrap(), WorkStatus::Done);

        next.branch = task.branch.clone();
        store.insert_task_pr(&next).await.unwrap();
        // A new PR respects completion; only an explicit reopen uncompletes.
        assert_eq!(store.work_status(&work).await.unwrap(), WorkStatus::Done);
        store
            .sqlite
            .reopen_task(&task.id, Some("New delivery"))
            .unwrap();
        store
            .sqlite
            .request_task_completion(&task.id, Some("New delivery"))
            .unwrap();
        super::settle_task_landing(&store, &landing).await.unwrap();
        assert_eq!(store.work_status(&work).await.unwrap(), WorkStatus::Ready);
        assert_eq!(
            store.active_task_pr(&task.id).await.unwrap().unwrap().id,
            next.id
        );
        assert!(store
            .sqlite
            .task_completion_pending(&task.id)
            .unwrap()
            .is_some());
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
            crate::ops::linear::LINEAR_TEST_CONTEXT.sync_scope(
                crate::ops::linear::LinearTestContext {
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
                        super::task_status(&repo, Some(&snapshot.task_id)).unwrap(),
                        status
                    );
                    assert_eq!(snapshot.status, TaskState::Done);
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
                "UPDATE tasks SET planning_completed=1,planning_state='completed' WHERE id=?1",
                [fixture.task.id.as_str()],
            )
            .unwrap();
        connection.execute(
            "INSERT INTO task_deletions(wave_id,issue_id,identifier,confirmed_at) VALUES (?1,?2,?3,456)",
            rusqlite::params![fixture.task.wave_id.as_ref().unwrap().as_str(), fixture.task.linear_id.as_ref().unwrap().as_str(), fixture.task.identifier],
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
                    "SELECT planning_completed FROM tasks WHERE id=?1",
                    [fixture.task.id.as_str()],
                    |row| row.get::<_, i64>(0)
                )
                .unwrap(),
            1
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
            let id = super::append_task_comment(&fixture.store, &fixture.task, "Keep going", true)
                .unwrap();
            let comments = fixture
                .store
                .sqlite
                .task_comments(&fixture.task.id)
                .unwrap()
                .comments;
            let body = &comments
                .iter()
                .find(|comment| comment.id == id)
                .unwrap()
                .body;
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
        parent_task.linear_id = Some(LinearIssueId::new("stack-parent-issue").unwrap());
        parent_task.identifier = "STACK-2".into();
        parent_task.worktree = Some(repo.path().join("parent"));
        parent_task.workspace_slug = "stack-parent".into();
        let mut parent = child.clone();
        parent.id = TaskPrId::new();
        parent.task_id = parent_task.id.clone();
        parent_task.branch = "test/stack-parent".into();
        parent.branch = parent_task.branch.clone();
        parent.slug = "stack-parent".into();
        store.seed_task(&parent_task, &parent).await.unwrap();
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
        store
            .stack_task_placement(&fixture.task, &parent.id)
            .await
            .unwrap();
        store
            .stack_task_placement(&fixture.task, &parent.id)
            .await
            .unwrap();
        assert_eq!(
            store.task_events_after(&fixture.task.id, 0).await.unwrap(),
            events
        );
        // Both self-parenting and a reverse edge preserve the accepted dependency.
        assert!(store
            .stack_task_placement(&parent_task, &parent.id)
            .await
            .is_err());
        let mut published_child = store
            .active_task_pr(&fixture.task.id)
            .await
            .unwrap()
            .unwrap();
        published_child.publication = parent.publication.clone();
        store.update_task_pr(&published_child).await.unwrap();
        assert!(store
            .stack_task_placement(&parent_task, &child.id)
            .await
            .unwrap_err()
            .to_string()
            .contains("cycle"));
        assert_eq!(
            store.get_task_pr(&parent.id).await.unwrap(),
            Some(parent.clone())
        );
        assert_eq!(
            store.get_task_pr(&child.id).await.unwrap(),
            Some(published_child)
        );

        let current = store.get_task(&fixture.task.id).await.unwrap().unwrap();
        let mut next_task = parent_task.clone();
        next_task.id = TaskId::new();
        next_task.identifier = "STACK-3".into();
        next_task.linear_id = None;
        next_task.branch = "test/next-parent".into();
        next_task.workspace_slug = "next-parent".into();
        next_task.worktree = Some(repo.path().join("next-parent"));
        let mut next = store.get_task_pr(&parent.id).await.unwrap().unwrap();
        next.id = TaskPrId::new();
        next.task_id = next_task.id.clone();
        next.branch = next_task.branch.clone();
        next.slug = next_task.workspace_slug.clone();
        store.seed_task(&next_task, &next).await.unwrap();
        let selected = super::select_existing_stack_parent(store, &current, "STACK-3")
            .await
            .unwrap();
        store
            .stack_task_placement(&current, &selected.id)
            .await
            .unwrap();
        let moved = store.get_task_pr(&child.id).await.unwrap().unwrap();
        assert_eq!(moved.parent_pr_id, Some(next.id));
        assert_eq!(moved.base_commit, child.base_commit);
        assert_eq!(moved.id, child.id);
        assert_eq!(
            store.task_events_after(&fixture.task.id, 0).await.unwrap(),
            events
        );
        assert!(store
            .stack_task_placement(&current, &parent.id)
            .await
            .is_err());
    }

    #[tokio::test]
    async fn task_placement_accepts_shared_worktree_names() {
        let repo = loopflow_test_support::TestRepo::new();
        let fixture = task_fixture_at("FIX-2", repo.path().to_path_buf()).await;
        for (name, expected) in [
            ("fix", "fix"),
            ("Fix Workspace", "fix-workspace"),
            ("one-two-three-four-five-six", "one-two-three-four-five-six"),
        ] {
            let (plan, slug) = super::plan_task_placement(
                repo.path(),
                &fixture.task,
                None,
                &super::TaskProcessOptions {
                    name: Some(name.into()),
                    ..Default::default()
                },
            )
            .unwrap();
            assert_eq!(slug, expected);
            assert!(plan.worktree_path.ends_with(format!(
                "{}.{}",
                repo.path().file_name().unwrap().to_str().unwrap(),
                expected
            )));
            assert!(!plan.worktree_path.exists());
        }
    }

    #[tokio::test]
    async fn task_preparation_rejects_invalid_inputs_before_allocating_placement() {
        let repo = loopflow_test_support::TestRepo::new();
        let fixture = task_fixture_at("FIX-2", repo.path().to_path_buf()).await;
        for (options, message) in [
            (
                super::TaskProcessOptions {
                    name: Some("bad.name".into()),
                    ..Default::default()
                },
                "flat worktree name",
            ),
            (
                super::TaskProcessOptions {
                    directive: Some("  ".into()),
                    ..Default::default()
                },
                "directive cannot be empty",
            ),
        ] {
            let error = super::prepare_task_placement(
                &fixture.store,
                repo.path(),
                &fixture.task,
                None,
                &options,
            )
            .await
            .unwrap_err();
            assert!(error.to_string().contains(message), "{error}");
        }
        assert_eq!(
            crate::git::worktrees::list_worktrees(repo.path())
                .unwrap()
                .len(),
            1
        );
    }

    #[tokio::test]
    async fn task_preparation_preserves_occupied_placement_and_resolves_base_without_creating() {
        let repo = loopflow_test_support::TestRepo::new();
        let fixture = task_fixture_at("FIX-2", repo.path().to_path_buf()).await;
        let options = super::TaskProcessOptions {
            name: Some("existing-task".into()),
            ..Default::default()
        };
        let planned = super::prepare_task_placement(
            &fixture.store,
            repo.path(),
            &fixture.task,
            None,
            &options,
        )
        .await
        .unwrap();
        assert_eq!(planned.plan.base_ref, repo.head_sha());
        assert!(!planned.plan.worktree_path.exists());
        std::fs::create_dir_all(&planned.plan.worktree_path).unwrap();
        let authored = planned.plan.worktree_path.join("authored.txt");
        std::fs::write(&authored, "retain these bytes").unwrap();
        let result = super::prepare_task_placement(
            &fixture.store,
            repo.path(),
            &fixture.task,
            None,
            &options,
        )
        .await;
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
        let fixture = task_fixture_at("FIX-2", repo.path().to_path_buf()).await;
        let upstream = repo.head_sha();
        repo.create_file("local.txt", "unpublished main work");
        repo.stage_all();
        repo.commit("Local main work");
        repo.create_branch("local-feature");
        repo.create_file("dirty.txt", "uncommitted work");
        let prepared = super::prepare_task_placement(
            &fixture.store,
            repo.path(),
            &fixture.task,
            None,
            &super::TaskProcessOptions::default(),
        )
        .await
        .unwrap();
        assert_eq!(prepared.plan.base_ref, upstream);
        assert_eq!(
            crate::git::current_branch(repo.path()).unwrap().as_deref(),
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
        let fixture = task_fixture_at("FIX-2", repo.path().to_path_buf()).await;
        let prepared = super::prepare_task_placement(
            &fixture.store,
            repo.path(),
            &fixture.task,
            None,
            &super::TaskProcessOptions::default(),
        )
        .await
        .unwrap();
        let original = repo.head_sha();
        repo.create_file("later.txt", "arrived before placement committed");
        repo.stage_all();
        repo.commit("Advance the remote after preparation");
        repo.push();
        assert_ne!(
            crate::git::rev_parse(repo.path(), "origin/main").unwrap(),
            original
        );

        let placement =
            crate::git::worktrees::create_from_placement_plan(repo.path(), &prepared.plan).unwrap();
        let placed_head = crate::git::rev_parse(&placement.path, "HEAD").unwrap();
        let has_later_file = placement.path.join("later.txt").exists();
        std::fs::remove_dir_all(&placement.path).unwrap();

        assert_eq!(placed_head, original);
        assert_eq!(placed_head, prepared.plan.base_ref);
        assert!(!has_later_file);
    }

    #[tokio::test]
    async fn task_preparation_refuses_an_unresolvable_base_before_allocating_placement() {
        let repo = loopflow_test_support::TestRepo::new();
        let fixture = task_fixture_at("FIX-2", repo.path().to_path_buf()).await;
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
        let error = super::prepare_task_placement(
            &fixture.store,
            repo.path(),
            &fixture.task,
            None,
            &super::TaskProcessOptions {
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
            crate::git::worktrees::list_worktrees(repo.path())
                .unwrap()
                .len(),
            1
        );
    }

    #[tokio::test]
    async fn task_preparation_rejects_unpublished_parent_before_allocating_child() {
        let repo = loopflow_test_support::TestRepo::new();
        let parent = task_fixture_at("FIX-1", repo.path().canonicalize().unwrap()).await;
        for selector in ["FIX-1", parent.task.id.as_str(), "FIX-1-issue"] {
            let error = super::prepare_task_placement(
                &parent.store,
                repo.path(),
                &parent.task,
                None,
                &super::TaskProcessOptions {
                    stack_on: Some(selector.into()),
                    ..Default::default()
                },
            )
            .await
            .unwrap_err();
            assert!(
                error.to_string().contains("cannot stack on itself"),
                "{error}"
            );
        }
        let child = Task {
            id: TaskId::new(),
            ..parent.task.clone()
        };
        let result = super::prepare_task_placement(
            &parent.store,
            repo.path(),
            &child,
            None,
            &super::TaskProcessOptions {
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
        let retained = parent
            .store
            .get_task(&parent.task.id)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(retained.worktree, parent.task.worktree);
        assert_eq!(parent.store.list_tasks(None).await.unwrap().len(), 1);
        assert_eq!(
            parent.store.task_prs(&parent.task.id).await.unwrap().len(),
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
        assert_eq!(
            checkout_execution_boundary(&worktree, "pi:fixture/model").unwrap(),
            boundary
        );
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
        assert_eq!(task_event_process_refusal(Some(&event)), Some(blocker));
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
