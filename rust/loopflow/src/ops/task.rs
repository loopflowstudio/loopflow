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
mod delivery;
pub(crate) use delivery::{
    attach_task_github_pr, clear_task_pr_merge_before_head_mutation, fetch_task_refs,
    lock_task_pr_mutation, matching_task_pr_merge_request, reconcile_checkout_pr,
    reconcile_task_pr, record_task_pr_presentation, record_task_pr_range_after_integration,
    record_task_pr_repair, refresh_task_pr_base, refresh_task_pr_for_publication,
    request_task_pr_merge, request_task_pr_publication, require_task_pr_range_nonempty,
    settle_task_landing, task_diff_base, task_pr_context, validate_task_pr_range_for_integration,
    TaskPrContext,
};
use delivery::{
    reconcile_task_pr_observation, resolve_managed_task, resolve_upstream_base,
    task_registry_error, ManagedTask,
};
mod snapshot;
pub(crate) use snapshot::task_workspace_context;
use snapshot::{
    file_snapshot, git_output, git_output_bytes, read_task_snapshot, resolve_file_base,
    validate_task_relative_path,
};
pub use snapshot::{task_changes, task_diff, task_file, task_snapshot, MAX_FILE_BYTES};
mod file_save;
pub mod ship;
pub use directory::{task_files, TaskDirectory, TaskFileEntry, TaskFileKind};
pub use file_save::{task_save, TaskFileRecovery, TaskFileSave};

use crate::work::task::comment::TaskComments;
use crate::work::wave::setup::SetupSource;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::{Duration, Instant};

use crate::agent::checkout_execution_boundary;
use crate::child::ChildRef;
use crate::durable::{TaskState, WorkStatus};
use crate::git::worktree_name::WorktreeName;
use crate::git::worktrees::{
    plan_branch_placement, PlacementPlan, PlacementStrategy, WorktreeSegment,
};
use crate::git::{fetch, merge_base, ref_exists, rev_parse};
use crate::naming::git_user;
use crate::ops::error::{OpsError, OpsResult};
use crate::ops::task_actions::TaskActionModel;
use crate::store::sqlite::{SqliteStore, TaskCheckout};
use crate::store::{open_existing_store, open_registry_for_authority, SharedStore, Store};
use crate::work::task::{
    GithubPr, Observation, PrPhase, PrPublication, Task, TaskEventKind, TaskPr, TaskPrId,
};
use crate::work::wave::Wave;
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
    #[serde(with = "time::serde::rfc3339")]
    pub created_at: time::OffsetDateTime,
    #[serde(with = "time::serde::rfc3339")]
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

/// The graph a Task takes up when it names none: your `lf set graph`, then its
/// Wave's and the repository's `graph:`, then the builtin `code`.
pub(crate) fn task_graph_default(store: &Store, task: &Task) -> OpsResult<(String, SetupSource)> {
    let repo = repository_for_task(store, task)?;
    let wave = match &task.wave_id {
        Some(id) => store.sqlite.get_wave(id).map_err(task_error)?,
        None => None,
    };
    let layers = crate::config::config_layers(Some(&repo), wave.as_ref().map(|w| w.slug()))
        .map_err(task_error)?;
    let (name, layer) = crate::config::graph_setting(&layers).map_err(task_error)?;
    let source = layer.map_or(SetupSource::Default, SetupSource::from);
    Ok((name, source))
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
            let (name, source) = task_graph_default(store, task)?;
            let repo = repository_for_task(store, task)?;
            (
                load(&name, &format!("{} default ", source.as_str()), &repo)?,
                None,
            )
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

/// The Task's agent, else its Wave's and repository's configuration.
pub(crate) fn resolve_task_agent(
    worktree: &Path,
    wave: Option<&str>,
    personal: &crate::config::PersonalSettings,
    agent: Option<&str>,
    skill: Option<&crate::graph::Skill>,
) -> String {
    let config = crate::config::load_wave_config_with(Some(worktree), wave, personal)
        .ok()
        .flatten()
        .unwrap_or_default();
    crate::prompt::process::resolve_agent(agent, skill, &config)
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

fn task_configuration_refusal(
    task: &Task,
    wave: Option<&str>,
    personal: &crate::config::PersonalSettings,
    skill: Option<&crate::graph::Skill>,
) -> Option<String> {
    let worktree = task.worktree.as_deref()?;
    checkout_execution_boundary(
        worktree,
        &resolve_task_agent(worktree, wave, personal, task.agent.as_deref(), skill),
    )
    .err()
    .map(|error| error.to_string())
}

/// Why the Task cannot start a process: its configuration, else a
/// non-resumable failure as its latest event.
pub(crate) fn task_process_refusal_after(
    task: &Task,
    wave: Option<&str>,
    personal: &crate::config::PersonalSettings,
    latest: Option<&crate::work::task::TaskEvent>,
) -> Option<String> {
    task_configuration_refusal(task, wave, personal, None)
        .or_else(|| task_event_process_refusal(latest).map(str::to_string))
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
    task_worktree_blocker_after(task, event.as_ref())
}

/// `task_worktree_blocker` for a Task whose latest event is already read.
pub(crate) fn task_worktree_blocker_after(
    task: &Task,
    latest: Option<&crate::work::task::TaskEvent>,
) -> OpsResult<Option<TaskWorktreeBlocker>> {
    if task.worktree.is_none() {
        return Ok(None);
    }
    if let Some(event) = latest {
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
    /// Its Sessions, Graphs and Workflow; `null` for an unregistered Task.
    pub overview: Option<crate::task_work::TaskOverview>,
    /// `lf graph show --processes` for each of `overview`'s Graphs, in order.
    pub graph_processes: Vec<crate::durable::GraphProcessDetail>,
    pub comments: Option<crate::work::task::comment::TaskComments>,
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
        let planning_error = inspection
            .as_ref()
            .and_then(|read| read.refresh_error.clone());
        let mut result = match (task, inspection) {
            (Some(task), _) => registered_task_status(&store, task).await?,
            (None, inspection) => {
                let mut result = TaskStatus {
                    sync: None,
                    task: None,
                    project: None,
                    observed_at: None,
                    planning_error: None,
                    planning_stale: false,
                    planning_state: crate::store::PlanningState::Unavailable,
                    execution: None,
                    ownership: None,
                    overview: None,
                    graph_processes: Vec::new(),
                    comments: None,
                };
                if let Some(read) = inspection {
                    // Foreign observations remain acquisition evidence, not owned Work.
                    result.planning_state = read.observation.state;
                    result.observed_at = read.observation.observed_at;
                }
                result
            }
        };
        if planning_error.is_some() {
            result.planning_error = planning_error;
            if result.planning_state == crate::store::PlanningState::Available {
                result.planning_state = crate::store::PlanningState::Unavailable;
            }
        }
        result.planning_stale = planning_stale(&result);
        Ok(result)
    })
}

/// `lf task status <id> --watch`: what the store records for `selector` now,
/// without the planning exchange the one-shot read starts first.
pub async fn read_task_status(
    store: &SharedStore,
    repo: &Path,
    selector: &str,
) -> OpsResult<TaskStatus> {
    let task = store
        .sqlite
        .resolve_task_id(selector, Some(&repo.to_string_lossy()))
        .and_then(|id| id.map(|id| store.sqlite.task(&id)).transpose())
        .map_err(task_error)?
        .flatten()
        .ok_or_else(|| task_error(format!("Task {selector} is not registered")))?;
    registered_task_status(store, task).await
}

async fn registered_task_status(store: &SharedStore, task: Task) -> OpsResult<TaskStatus> {
    let acquisition = store
        .sqlite
        .task_acquisition(&task.id)
        .map_err(task_error)?;
    let (overview, graph_processes) = store.sqlite.task_overview(&task.id).map_err(task_error)?;
    let mut result = TaskStatus {
        sync: Some(
            store
                .sqlite
                .task_planning_sync(&task.id)
                .map_err(task_error)?,
        ),
        project: match &task.project_id {
            Some(id) => store.get_project(id).await.map_err(task_error)?,
            None => None,
        },
        observed_at: acquisition.observed_at,
        planning_error: None,
        planning_stale: false,
        planning_state: acquisition.state,
        execution: Some(read_task_snapshot(store, task.clone()).await?),
        ownership: Some(store.sqlite.task_ownership(&task.id).map_err(task_error)?),
        overview: Some(overview),
        graph_processes,
        comments: Some(store.sqlite.task_comments(&task.id).map_err(task_error)?),
        task: Some(task),
    };
    result.planning_stale = planning_stale(&result);
    Ok(result)
}

fn planning_stale(status: &TaskStatus) -> bool {
    status.planning_state != crate::store::PlanningState::Available
        || status.observed_at.is_some_and(|at| {
            time::OffsetDateTime::now_utc().unix_timestamp() - at
                >= super::linear::LINEAR_SOFT_STALE_SECS
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
        .map(|(_, reason)| completion_pending_text(task, &reason)))
}

/// How a pending completion request reads, with its retry.
pub(crate) fn completion_pending_text(task: &Task, reason: &str) -> String {
    format!(
        "Completion pending: {reason}. Retry: lf task complete {}",
        task.identifier
    )
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
        let facts = store
            .sqlite
            .graph_execution_facts([&graph])
            .map_err(task_error)?;
        let (execution, _) = crate::ops::task_execution::execution_of_latest_graph(
            &store.sqlite,
            task,
            Some(&graph),
            &facts,
        )
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
mod tests;
