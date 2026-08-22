use std::collections::{BTreeMap, BTreeSet, HashSet};
use std::fs::{File, OpenOptions};
use std::path::{Component, Path, PathBuf};
use std::process::Command;
use std::sync::Arc;
use std::time::{Duration, Instant};

use crate::child::ChildRef;
use crate::durable::{
    AgentInvocation, ContainmentObservation, Run, RunContext, RunState, RunTrigger, WaitOn,
    WorkRef, WorkStatus,
};
use crate::engine::config::{load_config_or_default, parse_agent};
use crate::engine::git::{
    checkout, checkout_new_branch_from, cherry_pick_range, current_branch, delete_local_branch,
    fetch, get_default_branch, is_ancestor, is_clean, is_materially_clean, merge_base,
    push_with_upstream, ref_exists, rev_parse, stash_including_untracked, stash_pop,
};
use crate::engine::naming::sanitize_for_branch;
use crate::engine::process::tmux_session_slug;
use crate::engine::worktrees::{
    create_from_placement_plan, git_common_dir, main_repo_root, plan_placement, PlacementStrategy,
    WorktreeSegment,
};
use crate::engine::{expand_flow, load_flow, AgentExecutionBoundary, ConcreteStep};
use crate::ops::error::{OpsError, OpsResult};
use crate::planning::{LinearIssueId, TaskPlan};
use crate::pm::PmSnapshot;
use crate::provider_auth::Provider;
use crate::store::{
    open_existing_store, open_registry_for_authority, ProviderAccountId, RegistryUnavailable,
    SharedStore, Store, StoreError,
};
use crate::task::actions::{derive_task_actions, TaskActionEvidence, TaskActionModel};
use crate::task::{
    AfterMerge, CiCheck, CiObservation, CiState, GithubObservation, GithubObservationResult,
    GithubPr, Observation, PmWritebackOperation, PmWritebackState, PrMergeMode, PrMergeRequest,
    PrPhase, PrPresentation, PrPublication, Task, TaskEventKind, TaskPr, TaskPrId,
};
use crate::wave::Wave;
use fs2::FileExt;
use sha2::{Digest, Sha256};
use time::format_description::well_known::Rfc3339;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TaskWaitUntil {
    Open,
    Terminal,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct TaskFlowOverrides {
    pub first: Option<String>,
    pub loop_: Option<String>,
    pub finally: Option<String>,
}

/// Named cycle presets: where the human gate sits, in one word.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TaskCycle {
    /// Behavior is wrong. Opens with the incident flow (restore → 5whys)
    /// and the human gates at the demo, not a design doc.
    Fix,
    /// Behavior should change; the human shapes the design before code.
    Feature,
}

impl TaskCycle {
    pub fn parse(name: &str) -> Option<Self> {
        match name {
            "fix" => Some(Self::Fix),
            "feature" | "feat" => Some(Self::Feature),
            _ => None,
        }
    }

    /// The (first, finally) flows this cycle stands for.
    pub fn flows(self) -> (&'static str, &'static str) {
        match self {
            Self::Fix => ("incident", "ship-demo"),
            Self::Feature => ("task-design", "ship-demo"),
        }
    }

    pub fn for_lifecycle(lifecycle: &crate::task::TaskLifecyclePlan) -> Self {
        [Self::Fix, Self::Feature]
            .into_iter()
            .find(|cycle| lifecycle.first.flow == cycle.flows().0)
            .unwrap_or(Self::Feature)
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Self::Fix => "fix",
            Self::Feature => "feature",
        }
    }
}

impl TaskFlowOverrides {
    /// Expand a cycle preset into flow overrides. Explicit flow flags win
    /// over the preset; both win over the Project's pinned flows.
    pub fn for_cycle(
        cycle: Option<TaskCycle>,
        first: Option<String>,
        loop_: Option<String>,
        finally: Option<String>,
    ) -> Self {
        let (cycle_first, cycle_finally) = match cycle.map(TaskCycle::flows) {
            Some((first, finally)) => (Some(first), Some(finally)),
            None => (None, None),
        };
        Self {
            first: first.or_else(|| cycle_first.map(str::to_string)),
            loop_,
            finally: finally.or_else(|| cycle_finally.map(str::to_string)),
        }
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct TaskLaunchOptions {
    pub name: Option<String>,
    pub flows: TaskFlowOverrides,
    pub stack_on: Option<String>,
    pub directive: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TaskStartInput {
    pub title: String,
    pub report: String,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct TaskControlResult {
    pub issue_id: String,
    pub task_id: String,
    pub receipt: super::child::WorkControlReceipt,
    pub observation: Observation,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct TaskSnapshot {
    pub issue_id: String,
    pub issue_identifier: String,
    pub task_id: String,
    pub external_project_id: String,
    pub project: String,
    pub pm_snapshot_synced_at: i64,
    pub pm_writeback: crate::task::PmWritebackState,
    pub wave: String,
    pub project_id: String,
    pub status: WorkStatus,
    pub current: crate::child::CurrentWorkObservation,
    pub worktree: String,
    pub workspace_slug: String,
    pub lifecycle: crate::task::TaskLifecyclePlan,
    pub lifecycle_phase: crate::task::TaskLifecyclePhase,
    pub phase_epoch: u32,
    pub phase_cursor: u32,
    pub phase_iteration: u32,
    pub gate_cycle: u32,
    pub gate_proposal: Option<crate::task::TaskGateProposal>,
    pub prs: Vec<TaskPr>,
    pub active_pr: Option<TaskPrId>,
    pub agent: String,
    pub provider: String,
    pub provider_session_id: Option<String>,
    pub invocation: Option<AgentInvocation>,
    pub latest_event: Option<crate::task::TaskEvent>,
    pub created_at: time::OffsetDateTime,
    pub updated_at: time::OffsetDateTime,
    /// Freshness of the PR state against GitHub as of this read. `Degraded`
    /// means a bounded remote read failed and the PR fields are cached, not
    /// freshly confirmed.
    pub observation: Observation,
    pub actions: TaskActionModel,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct TaskChangedFile {
    pub path: String,
    pub committed: bool,
    pub staged: bool,
    pub unstaged: bool,
    pub untracked: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct TaskChangesSnapshot {
    pub issue_identifier: String,
    pub task_id: String,
    pub base_commit: String,
    pub head_commit: String,
    pub files: Vec<TaskChangedFile>,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct TaskDiffSnapshot {
    pub issue_identifier: String,
    pub task_id: String,
    pub path: Option<String>,
    pub patch: String,
    pub binary: bool,
    pub truncated: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct TaskFileSnapshot {
    pub issue_identifier: String,
    pub task_id: String,
    pub path: String,
    pub content: Option<String>,
    pub binary: bool,
    pub size_bytes: u64,
    pub truncated: bool,
}

#[derive(Debug, Clone, Copy)]
struct TaskWorkspace<'a> {
    issue_identifier: &'a str,
    task_id: &'a crate::task::TaskId,
    worktree: &'a Path,
    base_commit: &'a str,
}

impl<'a> TaskWorkspace<'a> {
    fn new(task: &'a Task, pr: &'a TaskPr) -> Self {
        Self {
            issue_identifier: &task.plan.identifier,
            task_id: &task.id,
            worktree: &task.worktree,
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

fn task_error(message: impl Into<String>) -> OpsError {
    OpsError::Message(message.into())
}

fn block_on_task<T>(future: impl std::future::Future<Output = OpsResult<T>>) -> OpsResult<T> {
    tokio::runtime::Runtime::new()
        .map_err(|error| task_error(format!("failed to build task runtime: {error}")))?
        .block_on(future)
}

async fn task_store() -> OpsResult<SharedStore> {
    open_existing_store().await.map(Arc::new).ok_or_else(|| {
        task_error("no Loopflow registry on this machine; start the owning Wave first")
    })
}

/// Durable placement for a Task PR forked from another Task's active PR.
#[derive(Debug, Clone)]
pub struct StackedRebase {
    pub fork_base: String,
    pub child: TaskPr,
    /// The live parent branch, or `None` once the parent has merged.
    pub parent_branch: Option<String>,
}

/// Resolve a Task's current cross-Task stack from durable ids, consulting
/// GitHub because an out-of-band parent merge can precede registry reconcile.
/// A worktree that is not a Task worktree yields `None`; a Task worktree whose
/// registry is missing/inaccessible/incompatible is refused with an actionable
/// authority error, so a stacked rebase never silently degrades to generic.
pub fn task_stack(worktree: &Path) -> OpsResult<Option<StackedRebase>> {
    block_on_task(async move {
        let TaskAuthority::Authority { store, task, .. } = resolve_task_authority(worktree).await?
        else {
            return Ok(None);
        };
        let Some(active) = store
            .active_task_pr(&task.id)
            .await
            .map_err(|error| task_error(error.to_string()))?
        else {
            return Ok(None);
        };
        let Some(parent_id) = active.parent_pr_id.clone() else {
            return Ok(None);
        };
        let parent = store
            .get_task_pr(&parent_id)
            .await
            .map_err(|error| task_error(error.to_string()))?
            .ok_or_else(|| task_error(format!("stack parent {parent_id} is missing")))?;
        let mut parent_task = store
            .get_task(&parent.task_id)
            .await
            .map_err(|error| task_error(error.to_string()))?
            .ok_or_else(|| task_error("stack parent Task is missing"))?;
        // Reuse the parent's persisted PR number and observation cache. Stack
        // resolution used to enumerate every PR on the branch independently,
        // bypassing both the Task cache and outage-tolerant reconcile.
        reconcile_task_pr(&store, &mut parent_task).await?;
        let parent = store
            .get_task_pr(&parent_id)
            .await
            .map_err(|error| task_error(error.to_string()))?
            .ok_or_else(|| task_error(format!("stack parent {parent_id} disappeared")))?;
        let merged = parent.merge_commit.is_some();
        let closed = parent.abandoned_at.is_some();
        if closed && !merged {
            return Err(task_error(format!(
                "stack parent {} closed without merging; re-place the child deliberately",
                parent.branch
            )));
        }
        Ok(Some(StackedRebase {
            fork_base: active.base_commit.clone(),
            child: active,
            parent_branch: (!merged).then_some(parent.branch),
        }))
    })
}

/// Return a stacked child only after its parent has merged. Landing before that
/// would silently drop a dependency that is not present on the default branch.
pub fn stacked_collapse(worktree: &Path) -> OpsResult<Option<StackedRebase>> {
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

/// Persist the exact base reached by a successful deterministic rebase. Clear
/// the parent link only when its work is now present on the default branch.
/// Refuses with an actionable authority error if the registry is not usable, so
/// a post-rebase base is never silently dropped — the rebase already pushed, so
/// the operator must know the durable record did not advance with it.
pub fn record_stack_rebase(
    stacked: &StackedRebase,
    new_base: &str,
    clear_parent: bool,
) -> OpsResult<()> {
    let pr_id = stacked.child.id.clone();
    let new_base = new_base.to_string();
    block_on_task(async move {
        let store = Arc::new(
            open_registry_for_authority()
                .await
                .map_err(registry_authority_error)?,
        );
        // The immutable event log preserves the audit trail — the child's
        // `PrStarted` (parent base) and the parent's `PrMerged` remain — so the
        // collapse only repoints the mutable row to the post-merge truth.
        store
            .rebase_task_pr(
                &pr_id,
                &new_base,
                clear_parent,
                time::OffsetDateTime::now_utc(),
            )
            .await
            .map_err(|error| task_error(error.to_string()))?;
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

async fn task_work_status(store: &Store, task: &Task) -> OpsResult<WorkStatus> {
    let work = store
        .work_for_child(&ChildRef::Task(task.id.clone()))
        .await
        .map_err(|error| task_error(error.to_string()))?;
    store
        .work_status(&work)
        .await
        .map_err(|error| task_error(error.to_string()))
}

pub fn task_run(repo: &Path, issue: &str, options: TaskLaunchOptions) -> OpsResult<Task> {
    let TaskLaunchOptions {
        name,
        flows,
        stack_on,
        directive,
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
                        "Task {} is abandoned; recover it with `lf task recover {}`",
                        task.plan.identifier, task.plan.identifier
                    )))
                }
                WorkStatus::Ready | WorkStatus::Running { .. } | WorkStatus::Waiting { .. } => {}
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
            ensure_task_flow_override(
                &task.worktree,
                &task.plan.identifier,
                crate::task::TaskLifecyclePhase::First,
                flows.first.as_deref(),
                &task.lifecycle.first.flow,
            )?;
            ensure_task_flow_override(
                &task.worktree,
                &task.plan.identifier,
                crate::task::TaskLifecyclePhase::Loop,
                flows.loop_.as_deref(),
                &task.lifecycle.loop_.flow,
            )?;
            ensure_task_flow_override(
                &task.worktree,
                &task.plan.identifier,
                crate::task::TaskLifecyclePhase::Finally,
                flows.finally.as_deref(),
                &task.lifecycle.finally.flow,
            )?;
            if let Some(requested) = stack_on.as_deref() {
                let active = store
                    .active_task_pr(&task.id)
                    .await
                    .map_err(|error| task_error(error.to_string()))?
                    .ok_or_else(|| task_error("existing Task has no active PR"))?;
                let parent_id = active.parent_pr_id.as_ref().ok_or_else(|| {
                    task_error(format!(
                        "Task {} is rooted on main, not stacked on {requested}",
                        task.plan.identifier
                    ))
                })?;
                let parent = store
                    .get_task_pr(parent_id)
                    .await
                    .map_err(|error| task_error(error.to_string()))?
                    .ok_or_else(|| task_error(format!("stack parent {parent_id} is missing")))?;
                let parent_task = store
                    .get_task(&parent.task_id)
                    .await
                    .map_err(|error| task_error(error.to_string()))?
                    .ok_or_else(|| task_error("stack parent Task is missing"))?;
                if requested != parent_task.plan.identifier
                    && requested != parent_task.plan.id.as_str()
                {
                    return Err(task_error(format!(
                        "Task {} is stacked on {}, not {requested}",
                        task.plan.identifier, parent_task.plan.identifier
                    )));
                }
            }
            if directive.is_some() {
                return Err(task_error(format!(
                    "Task {} already exists; use `lf task steer {} <new-direction>`",
                    task.plan.identifier, task.plan.identifier,
                )));
            }
        }
        Ok(existing)
    })?;
    if let Some(existing) = existing {
        validate_task_launch(&existing)?;
        return task_status(existing.plan.id.as_str());
    }
    let main_repo = crate::ops::project::ensure_clean_main(repo, "Task start")
        .map_err(|error| task_error(error.to_string()))?;
    let resolved =
        crate::ops::task_pm::resolve_task(&main_repo, issue, crate::ops::pm::PmRefresh::Auto)?;
    let project_flows = resolved
        .project
        .flows
        .clone()
        .unwrap_or_else(crate::pm::ProjectFlowPlan::empty);
    let lifecycle = resolve_task_lifecycle(&main_repo, &project_flows, &flows)?;
    let segment = match name.as_deref() {
        Some(name) => parse_workspace_slug(name)?,
        None => derive_workspace_slug(&resolved.item.name)?,
    };
    let workspace_slug = segment.as_str().to_string();
    let mut plan = plan_placement(&main_repo, segment)
        .map_err(|error| task_error(format!("failed to plan task worktree: {error}")))?;
    if plan.strategy != PlacementStrategy::Create {
        return Err(task_error(format!(
            "task worktree or branch already exists without a Task: {} ({})",
            plan.worktree_path.display(),
            plan.branch
        )));
    }
    let default_branch =
        get_default_branch(&main_repo).map_err(|error| task_error(error.to_string()))?;
    let stack_parent = stack_on
        .as_deref()
        .map(|parent_issue| {
            block_on_task(async {
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
                if parent_task.plan.id.as_str() == resolved.item.id {
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
                Ok(parent)
            })
        })
        .transpose()?;
    let (base_ref, base_commit) = match &stack_parent {
        Some(parent) => {
            fetch(&main_repo, "origin", &parent.branch).map_err(|error| {
                task_error(format!(
                    "failed to fetch parent branch {}: {error}",
                    parent.branch
                ))
            })?;
            let base_ref = format!("origin/{}", parent.branch);
            let base_commit = rev_parse(&main_repo, &base_ref).map_err(|error| {
                task_error(format!("failed to resolve task base {base_ref}: {error}"))
            })?;
            (base_ref, base_commit)
        }
        None => {
            let (base_ref, base_commit) = resolve_upstream_base(&main_repo, &default_branch)?;
            if base_ref.starts_with("origin/") {
                // Placement anchors on fetched origin; stop before contaminating
                // a new worktree with an ahead-of-upstream canonical main.
                refuse_if_canonical_ahead(&main_repo, &default_branch)?;
            }
            (base_ref, base_commit)
        }
    };
    plan.base_ref = base_ref.clone();
    let project = crate::ops::project::ensure_project_for_task(
        &main_repo,
        crate::ops::task_pm::ResolvedProject {
            snapshot: resolved.snapshot.clone(),
            project: resolved.project.clone(),
        },
    )?;
    let project_id = project.id.clone();
    let wave_id = project.wave_id.clone();
    let config = load_config_or_default(Some(&main_repo));
    let agent = config.agent();
    let (provider, _) = parse_agent(agent);
    let agent = agent.to_string();
    let directive = directive.unwrap_or_else(|| {
        format!(
            "Complete {}: {}\n\n{}",
            resolved.item.identifier, resolved.item.name, resolved.item.description
        )
    });

    block_on_task(async move {
        let store = task_store().await?;
        // Re-resolve after worktree planning: a concurrent run may have created
        // the Task in the gap. Non-terminal Work wins. Terminal Work remains
        // authoritative and requires an explicit recovery transition.
        if let Some(existing) = store
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
                        "Task {} is abandoned; recover it with `lf task recover {}`",
                        existing.plan.identifier, existing.plan.identifier
                    )))
                }
                WorkStatus::Ready | WorkStatus::Running { .. } | WorkStatus::Waiting { .. } => {
                    return Ok(existing)
                }
            }
        }
        let account_id = preflight_task_execution(&main_repo, &agent).await?;
        let now = time::OffsetDateTime::now_utc();
        let mut task = Task {
            id: crate::task::TaskId::new(),
            plan: TaskPlan {
                id: LinearIssueId::new(resolved.item.id.clone())
                    .map_err(|error| task_error(error.to_string()))?,
                identifier: resolved.item.identifier.clone(),
                title: resolved.item.name.clone(),
                description: resolved.item.description.clone(),
                pm_snapshot_synced_at: resolved.snapshot.synced_at,
            },
            wave_id,
            project_id,
            pm_writeback: PmWritebackState::Current,
            worktree: plan.worktree_path.clone(),
            workspace_slug: workspace_slug.clone(),
            lifecycle,
            lifecycle_phase: crate::task::TaskLifecyclePhase::First,
            phase_epoch: 1,
            phase_cursor: 0,
            phase_iteration: 0,
            gate_cycle: 0,
            gate_proposal: None,
            agent,
            provider,
            provider_session_id: None,
            abandon_intent: None,
            created_at: now,
            updated_at: now,
            observation: crate::task::Observation::NotRequired,
        };
        let pr = TaskPr {
            id: TaskPrId::new(),
            task_id: task.id.clone(),
            sequence: 1,
            slug: workspace_slug,
            branch: plan.branch.clone(),
            base_commit,
            parent_pr_id: stack_parent.as_ref().map(|parent| parent.id.clone()),
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

        let caller = crate::ops::ambient_run_context(&store).await?;
        let reservation = match caller.as_ref() {
            Some(caller) => {
                store
                    .create_task_run(Some(caller), &task, &pr, &directive)
                    .await
            }
            None => store.create_task_run(None, &task, &pr, &directive).await,
        };
        let (run, lease) = match reservation {
            Ok(reservation) => reservation,
            Err(StoreError::Sqlite(_)) => {
                if let Some(existing) =
                    store
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
                        return Ok(existing);
                    }
                }
                return Err(task_error(
                    "task reservation collided with another task placement",
                ));
            }
            Err(error) => return Err(task_error(format!("failed to reserve task: {error}"))),
        };

        if let Err(error) = create_from_placement_plan(&main_repo, &plan) {
            store
                .fail_task_run(&task.id, &lease, &error.to_string(), true)
                .await
                .map_err(|settle_error| {
                    task_error(format!(
                        "worktree creation failed ({error}); failed to settle reserved Run: {settle_error}"
                    ))
                })?;
            return Err(task_error(format!(
                "failed to create task worktree: {error}"
            )));
        }

        if let Err(error) = store
            .append_task_event(
                &task.id,
                &TaskEventKind::PrStarted {
                    pr_id: pr.id,
                    sequence: pr.sequence,
                    branch: pr.branch,
                    base_commit: pr.base_commit,
                },
            )
            .await
        {
            store
                .fail_task_run(&task.id, &lease, &error.to_string(), true)
                .await
                .map_err(|settle_error| {
                    task_error(format!(
                        "failed to record initial Task PR ({error}); failed to settle reserved Run: {settle_error}"
                    ))
                })?;
            return Err(task_error(error.to_string()));
        }

        launch_reserved_task_process(&store, &mut task, &run, &lease, account_id).await?;
        wait_until_running(&store, &task.id).await
    })
}

pub(crate) fn project_context(project: &crate::pm::PmProject) -> String {
    let mut context = format!("Definition:\n{}", project.definition.trim());
    if let Some(flows) = project
        .flows
        .as_ref()
        .filter(|flows| **flows != crate::pm::ProjectFlowPlan::empty())
    {
        context.push_str("\n\nProject Task flows:");
        if let Some(first) = &flows.first {
            context.push_str(&format!("\n- first: {first}"));
        }
        if let Some(loop_flow) = &flows.loop_ {
            context.push_str(&format!("\n- loop: {loop_flow}"));
        }
        if let Some(finally) = &flows.finally {
            context.push_str(&format!("\n- finally: {finally}"));
        }
    }
    if !project.krs.is_empty() {
        context.push_str("\n\nKRs:");
        for kr in &project.krs {
            let mark = if kr.holds { "x" } else { " " };
            context.push_str(&format!("\n- [{mark}] {}", kr.text));
        }
    }
    context
}

pub fn task_start(
    repo: &Path,
    project_id: &str,
    title: Option<String>,
    report: Option<String>,
    options: TaskLaunchOptions,
) -> OpsResult<Task> {
    let input = resolve_task_start_input(title.as_deref(), report.as_deref())?;
    let main = crate::ops::project::ensure_clean_main(repo, "Task start")
        .map_err(|error| task_error(error.to_string()))?;
    let project =
        crate::ops::task_pm::resolve_project(&main, project_id, crate::ops::pm::PmRefresh::Auto)?;
    crate::ops::project::require_registered_wave(&main, &project.snapshot.wave)
        .map_err(|error| task_error(error.to_string()))?;
    let project_flows = project
        .project
        .flows
        .clone()
        .unwrap_or_else(crate::pm::ProjectFlowPlan::empty);
    resolve_task_lifecycle(&main, &project_flows, &options.flows)?;
    let config = load_config_or_default(Some(&main));
    block_on_task(preflight_task_execution(&main, config.agent()))?;
    let marker = format!(
        "<!-- loopflow-task-start:{} -->",
        hex::encode(Sha256::digest(
            format!("{}\0{}\0{}", project.project.id, input.title, input.report).as_bytes()
        ))
    );
    let created = crate::ops::task_pm::create_and_load_task(
        &main,
        &project.snapshot.wave,
        &project.project.slug,
        &input.title,
        &input.report,
        &marker,
    )?;
    task_run(&main, &created.item.id, options)
}

pub fn resolve_task_start_input(
    explicit_title: Option<&str>,
    piped_report: Option<&str>,
) -> OpsResult<TaskStartInput> {
    let report = piped_report
        .map(str::trim)
        .filter(|value| !value.is_empty());
    let title = explicit_title
        .map(str::trim)
        .filter(|value| !value.is_empty());
    let title = match (title, report) {
        (Some(title), _) => title.to_string(),
        (None, Some(report)) => {
            let first_line = report
                .lines()
                .map(str::trim)
                .find(|line| !line.is_empty())
                .expect("non-empty report has a meaningful line");
            truncate_task_title(first_line, 100)
        }
        (None, None) => {
            return Err(task_error(
                "Task title or piped report is required: `pbpaste | lf task start <project>`",
            ))
        }
    };
    let report = report.unwrap_or(&title).to_string();
    Ok(TaskStartInput { title, report })
}

fn truncate_task_title(value: &str, max_chars: usize) -> String {
    if value.chars().count() <= max_chars {
        return value.to_string();
    }
    let mut title = value.chars().take(max_chars - 1).collect::<String>();
    title.push('…');
    title
}

fn ensure_task_flow_override(
    repo: &Path,
    issue_identifier: &str,
    phase: crate::task::TaskLifecyclePhase,
    requested: Option<&str>,
    pinned: &str,
) -> OpsResult<()> {
    let Some(requested) = requested else {
        return Ok(());
    };
    let (requested, _) = load_task_flow(repo, requested, phase)?;
    if requested != pinned {
        return Err(task_error(format!(
            "Task {} already pins {} flow {:?}",
            issue_identifier,
            phase.as_str(),
            pinned
        )));
    }
    Ok(())
}

fn resolve_task_lifecycle(
    repo: &Path,
    project: &crate::pm::ProjectFlowPlan,
    overrides: &TaskFlowOverrides,
) -> OpsResult<crate::task::TaskLifecyclePlan> {
    let (first, first_source) = select_task_flow(
        overrides.first.as_deref(),
        project.first.as_deref(),
        "task-design",
        "first",
    );
    let (loop_flow, loop_source) = select_task_flow(
        overrides.loop_.as_deref(),
        project.loop_.as_deref(),
        "slice",
        "loop",
    );
    let (finally, finally_source) = select_task_flow(
        overrides.finally.as_deref(),
        project.finally.as_deref(),
        "ship-demo",
        "finally",
    );
    let (first, first_steps) = load_task_flow(repo, first, crate::task::TaskLifecyclePhase::First)?;
    let (loop_flow, loop_steps) =
        load_task_flow(repo, loop_flow, crate::task::TaskLifecyclePhase::Loop)?;
    let (finally, finally_steps) =
        load_task_flow(repo, finally, crate::task::TaskLifecyclePhase::Finally)?;
    validate_task_lifecycle_facts([
        (&first, &first_source, &first_steps),
        (&loop_flow, &loop_source, &loop_steps),
        (&finally, &finally_source, &finally_steps),
    ])?;
    Ok(crate::task::TaskLifecyclePlan::standard(
        first, loop_flow, finally,
    ))
}

fn validate_task_lifecycle(task: &Task) -> OpsResult<()> {
    let resolve = |phase: crate::task::TaskLifecyclePhase, flow: &str| {
        load_task_flow(&task.worktree, flow, phase).map_err(|error| {
            task_error(format!(
                "Task {} cannot launch: pinned {} flow {flow:?} is invalid: {error}",
                task.plan.identifier,
                phase.as_str(),
            ))
        })
    };
    let (first, first_steps) = resolve(
        crate::task::TaskLifecyclePhase::First,
        &task.lifecycle.first.flow,
    )?;
    let (loop_flow, loop_steps) = resolve(
        crate::task::TaskLifecyclePhase::Loop,
        &task.lifecycle.loop_.flow,
    )?;
    let (finally, finally_steps) = resolve(
        crate::task::TaskLifecyclePhase::Finally,
        &task.lifecycle.finally.flow,
    )?;
    validate_task_lifecycle_facts(
        [
            (
                &first,
                "persisted Task lifecycle `first` configuration",
                &first_steps,
            ),
            (
                &loop_flow,
                "persisted Task lifecycle `loop` configuration",
                &loop_steps,
            ),
            (
                &finally,
                "persisted Task lifecycle `finally` configuration",
                &finally_steps,
            ),
        ],
    )
    .map_err(|error| {
        task_error(format!(
            "Task {} cannot launch: {error}\nPersisted Task lifecycle selection is immutable; abandon {} and start a replacement Task with the corrected flows",
            task.plan.identifier, task.plan.identifier
        ))
    })
}

fn validate_task_launch(task: &Task) -> OpsResult<()> {
    validate_task_lifecycle(task)?;
    task_execution_boundary(&task.worktree, &task.agent)?;
    Ok(())
}

fn task_configuration_refusal(task: &Task) -> Option<String> {
    validate_task_launch(task)
        .err()
        .map(|error| error.to_string())
}

fn task_execution_refusal(task: &Task) -> Option<String> {
    task_execution_boundary(&task.worktree, &task.agent)
        .err()
        .map(|error| error.to_string())
}

pub(crate) async fn task_launch_refusal(
    store: &SharedStore,
    task: &Task,
) -> crate::store::StoreResult<Option<String>> {
    if let Some(refusal) = task_configuration_refusal(task) {
        return Ok(Some(refusal));
    }
    persisted_task_launch_refusal(store, task).await
}

async fn task_launch_refusal_after_lifecycle_validation(
    store: &SharedStore,
    task: &Task,
) -> crate::store::StoreResult<Option<String>> {
    if let Some(refusal) = task_execution_refusal(task) {
        return Ok(Some(refusal));
    }
    persisted_task_launch_refusal(store, task).await
}

async fn persisted_task_launch_refusal(
    store: &SharedStore,
    task: &Task,
) -> crate::store::StoreResult<Option<String>> {
    let event = store.latest_task_event(&task.id).await?;
    Ok(task_event_launch_refusal(event.as_ref()).map(str::to_string))
}

pub(crate) fn task_event_launch_refusal(event: Option<&crate::task::TaskEvent>) -> Option<&str> {
    match event.map(|event| &event.kind) {
        Some(TaskEventKind::Failed {
            error,
            resumable: false,
        }) => Some(error),
        _ => None,
    }
}

pub(crate) fn task_execution_boundary(
    repo: &Path,
    agent: &str,
) -> OpsResult<AgentExecutionBoundary> {
    let (harness, _) = parse_agent(agent);
    if !matches!(harness.as_str(), "codex" | "claude") {
        return Err(task_error(format!(
            "Task execution cannot converge: agent {agent:?} uses harness {harness:?}, which has no managed account route for the required linked Git, Loopflow control-store, provider credential, and network capabilities; select codex or claude"
        )));
    }
    let common_dir = git_common_dir(repo).map_err(|error| {
        task_error(format!(
            "Task execution cannot converge: failed to resolve linked Git metadata from {}: {error}",
            repo.display()
        ))
    })?;
    let control = crate::engine::process::pinned_execution_context().map_err(|error| {
        task_error(format!(
            "Task execution cannot converge: Loopflow control-plane authority is unavailable: {error}"
        ))
    })?;
    let control_store = control.db_path.parent().ok_or_else(|| {
        task_error(format!(
            "Task execution cannot converge: Loopflow control database {} has no writable parent",
            control.db_path.display()
        ))
    })?;
    let mut writable_roots = vec![common_dir, control_store.to_path_buf()];
    writable_roots.sort();
    writable_roots.dedup();
    Ok(AgentExecutionBoundary { writable_roots })
}

fn probe_task_execution_boundary(boundary: &AgentExecutionBoundary) -> OpsResult<()> {
    for root in &boundary.writable_roots {
        let probe = root.join(format!(
            ".loopflow-task-capability-{}",
            uuid::Uuid::new_v4().simple()
        ));
        let result = OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&probe)
            .and_then(|file| {
                file.sync_data()?;
                std::fs::remove_file(&probe)
            });
        if let Err(error) = result {
            let _ = std::fs::remove_file(&probe);
            return Err(task_error(format!(
                "Task execution cannot converge: required writable authority for {} is unavailable: {error}. Run the Task from a Loopflow host whose managed execution profile includes linked Git metadata and the Loopflow control store",
                root.display()
            )));
        }
    }
    Ok(())
}

async fn preflight_task_execution(repo: &Path, agent: &str) -> OpsResult<ProviderAccountId> {
    let boundary = task_execution_boundary(repo, agent)?;
    probe_task_execution_boundary(&boundary)?;
    let (harness, _) = parse_agent(agent);
    let provider = harness.parse::<Provider>().map_err(|_| {
        task_error(format!(
            "Task execution cannot converge: agent {agent:?} has no managed provider-account route; select codex or claude"
        ))
    })?;
    let route = crate::provider_account::resolve_provider_account_exact(provider, None, None)
        .await
        .map_err(|error| {
            task_error(format!(
                "Task execution cannot converge: provider account capability for {harness} is unavailable: {error}. Connect an eligible managed account and retry"
            ))
        })?
        .ok_or_else(|| {
            task_error(format!(
                "Task execution cannot converge: provider account capability for {harness} resolved account_id=null. Configure and connect an eligible managed account before retrying"
            ))
        })?;
    route.verify_ready().await.map_err(|error| {
        task_error(format!(
            "Task execution cannot converge: provider credential capability for {}/{} is unavailable: {error}. Reconnect that managed account before retrying",
            harness,
            route.account_id()
        ))
    })?;
    Ok(route.account_id().clone())
}

fn select_task_flow<'a>(
    task: Option<&'a str>,
    project: Option<&'a str>,
    default: &'a str,
    phase: &str,
) -> (&'a str, String) {
    if let Some(flow) = task {
        return (flow, format!("Task launch `--{phase}`"));
    }
    if let Some(flow) = project {
        return (
            flow,
            format!("Linear Project `## Flows` `{phase}` configuration"),
        );
    }
    (default, format!("built-in Task `{phase}` default"))
}

fn validate_task_lifecycle_facts(phases: [(&str, &str, &[ConcreteStep]); 3]) -> OpsResult<()> {
    let [(_, _, _), (loop_flow, loop_source, loop_steps), (finally_flow, finally_source, finally_steps)] =
        phases;
    let mut violations = Vec::new();
    if loop_steps
        .iter()
        .all(|step| matches!(step, ConcreteStep::Skill(skill) if skill.policy.human))
    {
        violations.push(format!(
            "loop phase flow {:?} from {} is human-only and has no autonomous skill step. Move the human review to `first` or `finally`, or select an autonomous loop such as `slice`",
            loop_flow, loop_source,
        ));
    }

    let settles = finally_steps.last().is_some_and(|step| {
        matches!(
            step,
            ConcreteStep::Op(op)
                if op.item.command == "pr"
                    && op.item.args.first().is_some_and(|arg| arg == "land")
                    && op
                        .item
                        .args
                        .iter()
                        .any(|arg| arg == "-c" || arg == "--complete")
        )
    });
    if !settles {
        violations.push(format!(
            "finally phase flow {:?} from {} does not end with `op: pr land -c`; add terminal Task settlement or select `ship-demo`",
            finally_flow, finally_source,
        ));
    }

    if violations.is_empty() {
        return Ok(());
    }
    Err(task_error(format!(
        "Task lifecycle cannot converge:\n- {}",
        violations.join("\n- ")
    )))
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
                    "Task {} worktree initialization did not complete at {path} on branch {branch:?}; finish or restore that exact path before `lf task resume {}`; Task identity and PR history are unchanged",
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
            "Task {} worktree {} is missing; restore that exact path{branch} before `lf task resume {}`; Task identity and PR history are unchanged",
            task.plan.identifier,
            task.worktree.display(),
            task.plan.identifier,
        ),
    }))
}

fn load_task_flow(
    repo: &Path,
    requested: &str,
    phase: crate::task::TaskLifecyclePhase,
) -> OpsResult<(String, Vec<ConcreteStep>)> {
    let definition = load_flow(requested, repo)
        .map_err(|error| task_error(format!("failed to load Task flow {requested:?}: {error}")))?;
    let steps = expand_flow(&definition, repo).map_err(|error| {
        task_error(format!("failed to expand Task flow {requested:?}: {error}"))
    })?;
    if steps.is_empty() {
        return Err(task_error(format!("Task flow {requested:?} has no steps")));
    }
    let allow_ops = phase == crate::task::TaskLifecyclePhase::Finally;
    if allow_ops {
        let first_op = steps
            .iter()
            .position(|step| matches!(step, ConcreteStep::Op(_)));
        if matches!(first_op, Some(0))
            || first_op.is_some_and(|index| {
                steps[index..]
                    .iter()
                    .any(|step| !matches!(step, ConcreteStep::Op(_)))
            })
        {
            return Err(task_error(format!(
                "Task finally flow {requested:?} must run one or more skills followed by optional ops"
            )));
        }
    }
    if let Some(step) = steps.iter().find(|step| {
        !(matches!(step, ConcreteStep::Skill(_))
            || allow_ops && matches!(step, ConcreteStep::Op(_)))
    }) {
        return Err(task_error(format!(
            "Task flow {requested:?} contains {step:?}; first/loop require skills and finally permits skills or ops"
        )));
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
    WorktreeSegment::parse(value).map_err(|error| task_error(error.to_string()))
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

async fn update_task_pr_with_authority(
    store: &SharedStore,
    pr: &TaskPr,
    lease: Option<&RunContext>,
) -> Result<(), StoreError> {
    match lease {
        Some(lease) => store.update_task_pr_for_run(pr, lease).await,
        None => store.update_task_pr(pr).await,
    }
}

async fn settle_task_pr_with_authority(
    store: &SharedStore,
    settled: &TaskPr,
    next: Option<&TaskPr>,
    lease: Option<&RunContext>,
) -> Result<(), StoreError> {
    match lease {
        Some(lease) => store.settle_task_pr_for_run(settled, next, lease).await,
        None => store.settle_task_pr(settled, next).await,
    }
}

async fn settle_task_pr_merged_with_authority(
    store: &SharedStore,
    settled: &TaskPr,
    merged_at: Option<time::OffsetDateTime>,
    lease: Option<&RunContext>,
) -> Result<crate::store::TaskPrMergeEvidenceOutcome, StoreError> {
    match lease {
        Some(lease) => {
            store
                .settle_task_pr_merged_for_run(settled, merged_at, lease)
                .await
        }
        None => store.settle_task_pr_merged(settled, merged_at).await,
    }
}

async fn append_task_event_with_authority(
    store: &SharedStore,
    task_id: &crate::task::TaskId,
    event: &TaskEventKind,
    lease: Option<&RunContext>,
) -> Result<(), StoreError> {
    match lease {
        Some(lease) => {
            store
                .append_task_event_for_run(task_id, lease, event)
                .await?;
        }
        None => {
            store.append_task_event(task_id, event).await?;
        }
    }
    Ok(())
}

async fn update_task_with_authority(
    store: &SharedStore,
    task: &Task,
    lease: Option<&RunContext>,
) -> Result<(), StoreError> {
    match lease {
        Some(lease) => store.update_task_for_run(task, lease).await,
        None => store.update_task(task).await,
    }
}

async fn complete_task_after_pr_with_authority(
    store: &SharedStore,
    task: &Task,
    pr: &TaskPr,
    lease: Option<&RunContext>,
) -> Result<(), StoreError> {
    match lease {
        Some(lease) => store.complete_task_after_pr_for_run(task, pr, lease).await,
        None => store.complete_task_after_pr(task, pr).await,
    }
}

async fn complete_task_with_authority(
    store: &SharedStore,
    task: &Task,
    skipped_pr: Option<&TaskPr>,
    lease: Option<&RunContext>,
) -> Result<(), StoreError> {
    match lease {
        Some(lease) => store.complete_task_for_run(task, skipped_pr, lease).await,
        None => store.complete_task(task, skipped_pr).await,
    }
}

async fn ambient_task_run_context(
    store: &SharedStore,
    task: &Task,
) -> OpsResult<Option<RunContext>> {
    let Some(lease) = crate::ops::ambient_run_context(store).await? else {
        return Ok(None);
    };
    let work = store
        .work_for_child(&ChildRef::Task(task.id.clone()))
        .await
        .map_err(|error| task_error(format!("failed to resolve Task Work: {error}")))?;
    if lease.work != work {
        return Err(task_error(format!(
            "ambient Run {} cannot mutate Task {}",
            lease.run_id, task.id
        )));
    }
    Ok(Some(lease))
}

async fn task_for_worktree(
    store: &SharedStore,
    repo: &Path,
) -> OpsResult<Option<(Task, Option<RunContext>)>> {
    let checkout = repo.canonicalize().unwrap_or_else(|_| repo.to_path_buf());
    if let Some(lease) = crate::ops::ambient_run_context(store).await? {
        let WorkRef::Task(id) = &lease.work else {
            return Err(task_error(format!(
                "ambient Run {} owns {}, not Task Work",
                lease.run_id,
                lease.work.kind()
            )));
        };
        let task = store
            .get_task(id)
            .await
            .map_err(|error| task_error(format!("failed to read ambient Task: {error}")))?
            .ok_or_else(|| task_error(format!("ambient Task {id} is not registered")))?;
        let worktree = task
            .worktree
            .canonicalize()
            .unwrap_or_else(|_| task.worktree.clone());
        if checkout != worktree {
            return Err(task_error(format!(
                "ambient Task {id} owns {}, not {}",
                task.worktree.display(),
                repo.display()
            )));
        }
        return Ok(Some((task, Some(lease))));
    }

    let worktree_keys: BTreeSet<String> = store
        .list_tasks(None)
        .await
        .map_err(|error| task_error(format!("failed to inspect Tasks: {error}")))?
        .into_iter()
        .filter(|task| {
            task.worktree
                .canonicalize()
                .unwrap_or_else(|_| task.worktree.clone())
                == checkout
        })
        .map(|task| task.worktree.display().to_string())
        .collect();
    let mut current = Vec::new();
    for worktree in worktree_keys {
        if let Some(task) = store
            .get_task_by_worktree(&worktree)
            .await
            .map_err(|error| task_error(format!("failed to resolve Task worktree: {error}")))?
        {
            current.push(task);
        }
    }
    if current.len() > 1 {
        return Err(task_error(format!(
            "multiple Tasks claim worktree {}",
            repo.display()
        )));
    }
    Ok(current.pop().map(|task| (task, None)))
}

/// Proven authority to run a Task-owned PR operation, or an explicit decision
/// that this worktree is not a Task worktree.
///
/// The PR publication, stacking, submit, and land entry points share this one
/// resolver so they cannot disagree about what counts as authority. A missing
/// or incompatible registry never collapses to [`TaskAuthority::NotATaskWorktree`]
/// silently: only a registry file that provably does not exist (no tasks have
/// ever been registered on this machine) and no ambient Task id together prove
/// "not a Task," which preserves ordinary non-Task PR flows. Everything else
/// that cannot be opened is a refusal with an actionable authority error, so a
/// Task entry point never degrades to generic PR behavior.
#[derive(Debug)]
enum TaskAuthority {
    /// This worktree is not a Task worktree. Task-specific bookkeeping is an
    /// explicit no-op; the ordinary PR flow continues unchanged.
    NotATaskWorktree,
    /// Proven authority: the registry is healthy and a Task owns this
    /// worktree. `lease` is present only when an ambient Task body proved it.
    /// Boxed so the `NotATaskWorktree` no-op variant stays small.
    Authority {
        store: SharedStore,
        task: Box<Task>,
        lease: Option<RunContext>,
    },
}

/// Turn a [`RegistryUnavailable`] into an actionable authority error. The
/// message always names the recovery action so the operator can move.
fn registry_authority_error(err: RegistryUnavailable) -> OpsError {
    task_error(match err {
        RegistryUnavailable::MissingFile { path } => format!(
            "Task PR authority refused: the shared Loopflow registry {} is missing. \
             Start the owning Wave (it creates the registry) or run `lf doctor`.",
            path.display()
        ),
        RegistryUnavailable::Unresolved { error } => format!(
            "Task PR authority refused: the shared Loopflow registry path is not usable: {error}. \
             Fix LF_DB_PATH/LF_HOME or run `lf doctor`."
        ),
        RegistryUnavailable::Incompatible { path, error } => format!(
            "Task PR authority refused: the shared Loopflow registry {} is present but \
             inaccessible or schema-incompatible: {error}. Run `lf doctor`.",
            path.display()
        ),
    })
}

/// Resolve Task authority for a PR entry point at `repo`.
///
/// - Registry opens and a task claims this worktree → [`TaskAuthority::Authority`].
/// - Registry opens and no task claims it → [`TaskAuthority::NotATaskWorktree`].
/// - Registry file missing and no ambient Task id → [`TaskAuthority::NotATaskWorktree`]
///   (no registry means no tasks exist, so this is provably an ordinary PR).
/// - Registry missing with an ambient Task id, or present but unopenable → refuse.
async fn resolve_task_authority(repo: &Path) -> OpsResult<TaskAuthority> {
    let ambient = std::env::var_os(crate::durable::RUN_CONTEXT_ENV).is_some();
    let store = match open_registry_for_authority().await {
        Ok(store) => Arc::new(store),
        Err(RegistryUnavailable::MissingFile { .. }) if !ambient => {
            return Ok(TaskAuthority::NotATaskWorktree);
        }
        Err(err) => return Err(registry_authority_error(err)),
    };
    match task_for_worktree(&store, repo).await? {
        Some((task, lease)) => {
            if let Some(lease) = lease.as_ref() {
                validate_automated_task_authority(repo, &store, &task, lease).await?;
            }
            Ok(TaskAuthority::Authority {
                store,
                task: Box::new(task),
                lease,
            })
        }
        None => Ok(TaskAuthority::NotATaskWorktree),
    }
}

pub(crate) fn guard_task_mutation_authority(repo: &Path) -> OpsResult<()> {
    block_on_task(async move {
        let _ = resolve_task_authority(repo).await?;
        Ok(())
    })
}

async fn validate_automated_task_authority(
    repo: &Path,
    store: &SharedStore,
    task: &Task,
    lease: &RunContext,
) -> OpsResult<()> {
    let main_repo = main_repo_root(repo)?;
    let current = Box::pin(crate::ops::task_pm::resolve_task_async(
        &main_repo,
        task.plan.id.as_str(),
        crate::ops::pm::PmRefresh::Never,
    ))
    .await
    .map_err(|error| {
        task_error(format!(
            "Task {} current PM authority refused: {error}",
            task.plan.identifier
        ))
    })?;
    store
        .validate_task_run_route(task, lease, &current.project.id)
        .await
        .map_err(|error| task_error(format!("Task body lost Project authority: {error}")))
}

pub(crate) fn record_task_pr_repair(
    repo: &Path,
    kind: crate::task::TaskPrRepairKind,
) -> OpsResult<bool> {
    block_on_task(async move {
        let TaskAuthority::Authority { store, task, lease } = resolve_task_authority(repo).await?
        else {
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
        match lease.as_ref() {
            Some(lease) => {
                store
                    .record_task_pr_repair_incident_for_run(&pr.id, kind, occurred_at, lease)
                    .await
            }
            None => {
                store
                    .record_task_pr_repair_incident(&pr.id, kind, occurred_at)
                    .await
            }
        }
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
        let TaskAuthority::Authority { store, task, lease } = resolve_task_authority(repo).await?
        else {
            return Ok(false);
        };
        let context = _task_pr_context_from_store(&store, &task).await?;
        _validate_task_pr_copy(&context, title, body)?;
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
        match lease.as_ref() {
            Some(lease) => store.update_task_pr_for_run(&pr, lease).await,
            None => store.update_task_pr(&pr).await,
        }
        .map_err(|error| task_error(format!("failed to request PR publication: {error}")))?;
        Ok(true)
    })
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct TaskPrContext {
    pub(crate) title: String,
    pub(crate) identifier: String,
    pub(crate) url: String,
    pub(crate) lifecycle: crate::task::TaskLifecyclePlan,
    pub(crate) sequence: u32,
}

impl TaskPrContext {
    pub(crate) fn pr_title(&self) -> String {
        format!("{}: {}", self.identifier.trim(), self.title.trim())
    }

    pub(crate) fn task_link(&self) -> String {
        format!(
            "[{} — {}]({})",
            _markdown_link_text(self.identifier.trim()),
            _markdown_link_text(self.title.trim()),
            self.url
        )
    }

    pub(crate) fn cycle(&self) -> TaskCycle {
        TaskCycle::for_lifecycle(&self.lifecycle)
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
        let TaskAuthority::Authority { store, task, .. } = resolve_task_authority(repo).await?
        else {
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
        .ok_or_else(|| _missing_task_pr_url(task, wave.name()))?;
    let snapshot: PmSnapshot = serde_json::from_str(&snapshot.payload).map_err(|error| {
        task_error(format!(
            "cached PM snapshot for Wave {:?} is invalid: {error}. Run `lf pm sync --wave {}` before publishing this Task PR",
            wave.name(),
            wave.name(),
        ))
    })?;
    let item = snapshot
        .items
        .iter()
        .find(|item| item.id == task.plan.id.as_str())
        .ok_or_else(|| _missing_task_pr_url(task, wave.name()))?;
    let url = item
        .url
        .as_deref()
        .filter(|url| _valid_task_url(url))
        .ok_or_else(|| _missing_task_pr_url(task, wave.name()))?;
    let pr = store
        .active_task_pr(&task.id)
        .await
        .map_err(|error| task_error(format!("failed to read active PR: {error}")))?
        .ok_or_else(|| task_error(format!("Task {} has no active PR", task.plan.identifier)))?;
    Ok(TaskPrContext {
        title: task.plan.title.clone(),
        identifier: task.plan.identifier.clone(),
        url: url.to_string(),
        lifecycle: task.lifecycle.clone(),
        sequence: pr.sequence,
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
        "Task {} has no valid provider URL in the cached PM snapshot. Run `lf pm sync --wave {wave}` before publishing this Task PR",
        task.plan.identifier,
    ))
}

fn _validate_task_pr_copy(context: &TaskPrContext, title: &str, body: &str) -> OpsResult<()> {
    let expected_title = context.pr_title();
    if title != expected_title {
        return Err(task_error(format!(
            "Task PR title must be {expected_title:?}"
        )));
    }
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
        let TaskAuthority::Authority { store, task, .. } = resolve_task_authority(repo).await?
        else {
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
) -> OpsResult<bool> {
    block_on_task(async move {
        let TaskAuthority::Authority { store, task, lease } = resolve_task_authority(repo).await?
        else {
            return Ok(false);
        };
        clear_task_pr_merge(
            &store,
            &task,
            lease.as_ref(),
            repo,
            mutation_is_unconditional,
        )
        .await
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
    lease: Option<&RunContext>,
    repo: &Path,
    mutation_is_unconditional: bool,
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
        crate::ops::pr::disable_auto_merge(repo, number)?;
    }
    pr.publication
        .as_mut()
        .expect("merge request requires publication")
        .merge = None;
    pr.updated_at = time::OffsetDateTime::now_utc();
    match lease {
        Some(lease) => store.update_task_pr_for_run(&pr, lease).await,
        None => store.update_task_pr(&pr).await,
    }
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
) -> OpsResult<bool> {
    let head_sha = head_sha.map(str::to_string);
    let next_slug = next_slug.map(parse_pr_slug).transpose()?;
    if after_merge == AfterMerge::CompleteTask && next_slug.is_some() {
        return Err(task_error("--complete and --next cannot be used together"));
    }
    block_on_task(async move {
        let TaskAuthority::Authority { store, task, lease } = resolve_task_authority(repo).await?
        else {
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
            crate::ops::pr::disable_auto_merge(repo, number)?;
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
        match lease.as_ref() {
            Some(lease) => store.update_task_pr_for_run(&pr, lease).await,
            None => store.update_task_pr(&pr).await,
        }
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

/// Refuse placement when the canonical `<default>` checkout carries commits its
/// upstream lacks. A new Task worktree cut from an ahead-of-origin main inherits
/// the unpushed commit — the control-plane violation behind W2-132/#877 and
/// W2-130/#882. Requires a prior fetch so `origin/<default>` is current.
fn refuse_if_canonical_ahead(repo: &Path, default_branch: &str) -> OpsResult<()> {
    if rev_parse(repo, &format!("refs/heads/{default_branch}")).is_err() {
        // No local default branch checked out (fresh clone / detached) — nothing
        // can be ahead.
        return Ok(());
    }
    let range = format!("origin/{default_branch}..{default_branch}");
    let ahead = git_output(repo, &["log", "--oneline", "--no-decorate", &range])?;
    let ahead = ahead.trim();
    if !ahead.is_empty() {
        return Err(task_error(format!(
            "canonical {default_branch} is ahead of origin/{default_branch}; new Task worktrees \
             would inherit these unpushed commit(s):\n{ahead}\nThis is a control-plane violation. \
             Push or reset {default_branch} to origin/{default_branch} before placing Task worktrees."
        )));
    }
    Ok(())
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
/// `lf task changes`:
/// - `M == B` — parity holds; publish.
/// - `M` ancestor of `B` — the recorded base itself carries commits absent from
///   `O` (inherited foreign ancestry, the #877/#882 shape). Refuse before any
///   push, naming the foreign commits/files and the safe rebase.
/// - `B` ancestor of `M` — `O` advanced past a stale or squash-merged base. Safe:
///   heal `base_commit → M` so the durable evidence and `lf task changes` stay
///   truthful, then publish the minimal `M..H` range.
/// - divergent — ambiguous ancestry; refuse, naming the commits and files on
///   both sides (`M..B` and `B..M`) plus the safe rebase.
pub(crate) fn verify_task_pr_range(repo: &Path) -> OpsResult<()> {
    let repo = repo.to_path_buf();
    block_on_task(async move {
        let TaskAuthority::Authority { store, task, lease } = resolve_task_authority(&repo).await?
        else {
            return Ok(());
        };
        verify_task_pr_range_with_authority(&store, &task, lease.as_ref(), &repo).await
    })
}

/// Publication proof: require ancestry parity without changing the recorded
/// fork. Only an explicit integration boundary may advance Task stack/base
/// metadata.
pub(crate) fn verify_task_pr_range_without_healing(repo: &Path) -> OpsResult<()> {
    let repo = repo.to_path_buf();
    block_on_task(async move {
        let TaskAuthority::Authority { store, task, lease } = resolve_task_authority(&repo).await?
        else {
            return Ok(());
        };
        verify_task_pr_range_with_authority_mode(
            &store,
            &task,
            lease.as_ref(),
            &repo,
            StaleBaseAction::Refuse,
            None,
        )
        .await
    })
}

/// Prove the post-rebase Task range against the operation's immutable target
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
        let TaskAuthority::Authority { store, task, lease } = resolve_task_authority(&repo).await?
        else {
            return Ok(());
        };
        verify_task_pr_range_with_authority_mode(
            &store,
            &task,
            lease.as_ref(),
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
/// already-open PR reset or rebased empty is refused just like a first
/// publication. A worktree that is provably not a Task worktree is an explicit
/// no-op; a Task worktree whose registry is unusable is refused with an
/// actionable authority error rather than degrading to generic PR behavior.
pub(crate) fn require_task_pr_range_nonempty(repo: &Path) -> OpsResult<()> {
    let repo = repo.to_path_buf();
    block_on_task(async move {
        let TaskAuthority::Authority { store, task, lease } = resolve_task_authority(&repo).await?
        else {
            return Ok(());
        };
        require_task_pr_range_nonempty_with_authority(&store, &task, lease.as_ref(), &repo).await
    })
}

/// Publication's post-commit proof: require a non-empty authoritative range
/// while leaving integration metadata untouched.
pub(crate) fn require_task_pr_range_nonempty_without_healing(repo: &Path) -> OpsResult<()> {
    let repo = repo.to_path_buf();
    block_on_task(async move {
        let TaskAuthority::Authority { store, task, lease } = resolve_task_authority(&repo).await?
        else {
            return Ok(());
        };
        require_task_pr_range_nonempty_with_authority_mode(
            &store,
            &task,
            lease.as_ref(),
            &repo,
            StaleBaseAction::Refuse,
        )
        .await
    })
}

/// Resolve the upstream a Task PR's ancestry should be measured against. A root
/// PR measures against `origin/<default>` (or local `<default>` without a
/// remote). A stacked child with a live parent measures against the parent's
/// branch tip — so the parent's own commits are expected ancestry, not foreign
/// contamination, and the child's range is `fork_point..HEAD` against the
/// durable parent boundary. A child whose parent merged (or was abandoned) has
/// been collapsed onto `<default>` by [`record_stack_rebase`]; it measures
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
pub(crate) async fn verify_task_pr_range_with_authority(
    store: &SharedStore,
    task: &Task,
    lease: Option<&RunContext>,
    repo: &Path,
) -> OpsResult<()> {
    verify_task_pr_range_with_authority_mode(store, task, lease, repo, StaleBaseAction::Heal, None)
        .await
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum StaleBaseAction {
    Accept,
    Refuse,
    Heal,
}

async fn verify_task_pr_range_with_authority_mode(
    store: &SharedStore,
    task: &Task,
    lease: Option<&RunContext>,
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
        // Heal the recorded base to the true fork point so lf task changes and
        // the durable evidence report the minimal M..HEAD range.
        match stale_base {
            StaleBaseAction::Accept => return Ok(()),
            StaleBaseAction::Refuse => {
                return Err(task_error(format!(
                    "Task {identifier} PR base {} is stale behind the branch fork {}. Publication does not update integration metadata; run `lf rebase` before publishing.",
                    short(&base),
                    short(&merge_base),
                )));
            }
            StaleBaseAction::Heal => {}
        }
        pr.base_commit = merge_base.clone();
        pr.updated_at = time::OffsetDateTime::now_utc();
        match lease {
            Some(lease) => store.heal_task_pr_base_for_run(&pr, lease).await,
            None => store.heal_task_pr_base(&pr).await,
        }
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
async fn require_task_pr_range_nonempty_with_authority(
    store: &SharedStore,
    task: &Task,
    lease: Option<&RunContext>,
    repo: &Path,
) -> OpsResult<()> {
    require_task_pr_range_nonempty_with_authority_mode(
        store,
        task,
        lease,
        repo,
        StaleBaseAction::Heal,
    )
    .await
}

async fn require_task_pr_range_nonempty_with_authority_mode(
    store: &SharedStore,
    task: &Task,
    lease: Option<&RunContext>,
    repo: &Path,
    stale_base: StaleBaseAction,
) -> OpsResult<()> {
    verify_task_pr_range_with_authority_mode(store, task, lease, repo, stale_base, None).await?;
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
) -> OpsResult<bool> {
    block_on_task(async move {
        let TaskAuthority::Authority { store, task, lease } = resolve_task_authority(repo).await?
        else {
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
        let publication = pr.publication.as_mut().ok_or_else(|| {
            task_error(format!(
                "Task {} has no durable PR publication request",
                task.plan.identifier
            ))
        })?;
        invalidate_stale_merge_request(repo, publication, github_pr)?;
        publication.github = Some(GithubPr {
            number,
            url: url.clone(),
            head_sha: github_pr.head_sha.clone(),
        });
        // Idempotently link the PR on its owning Linear issue. This never fails
        // the attach: a degraded writeback is recorded on the PR and retried by
        // the next publication command.
        link_pr_to_linear(&store, &task, &mut pr).await;
        pr.updated_at = time::OffsetDateTime::now_utc();
        match lease.as_ref() {
            Some(lease) => store.update_task_pr_for_run(&pr, lease).await,
            None => store.update_task_pr(&pr).await,
        }
        .map_err(|error| task_error(format!("failed to attach GitHub PR: {error}")))?;
        if opened {
            let event = TaskEventKind::PrOpened {
                pr_id: pr.id,
                sequence: pr.sequence,
                number,
                url,
            };
            match lease.as_ref() {
                Some(lease) => {
                    store
                        .append_task_event_for_run(&task.id, lease, &event)
                        .await
                }
                None => store.append_task_event(&task.id, &event).await,
            }
            .map_err(|error| task_error(error.to_string()))?;
        }
        Ok(true)
    })
}

/// A merge request belongs to one exact head. Revoke an armed auto-merge before
/// forgetting a stale request so a later push cannot inherit settlement intent.
fn invalidate_stale_merge_request(
    repo: &Path,
    publication: &mut PrPublication,
    github_pr: &crate::ops::pr::PrInfo,
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
        crate::ops::pr::disable_auto_merge(repo, number)?;
    }
    publication.merge = None;
    Ok(())
}

pub(crate) fn abandon_task_pr(
    repo: &Path,
    force: bool,
    progress: &impl crate::ops::progress::Progress,
) -> OpsResult<bool> {
    block_on_task(async move {
        let TaskAuthority::Authority { store, task, lease } = resolve_task_authority(repo).await?
        else {
            return Ok(false);
        };
        let _mutation = lock_task_pr_mutation(repo)?;
        let mut pr = store
            .active_task_pr(&task.id)
            .await
            .map_err(|error| task_error(format!("failed to read active PR: {error}")))?
            .ok_or_else(|| {
                task_error(format!(
                    "Task {} has no active PR to abandon",
                    task.plan.identifier
                ))
            })?;
        let branch =
            current_branch(repo)?.ok_or_else(|| task_error("Task worktree is not on a branch"))?;
        if branch != pr.branch {
            return Err(task_error(format!(
                "Task {} active PR expects branch {:?}, but the worktree is on {:?}",
                task.plan.identifier, pr.branch, branch
            )));
        }
        let dirty = !is_clean(repo)?;
        if dirty && !force {
            return Err(task_error("uncommitted changes; use --force"));
        }
        if let Some(lease) = lease.as_ref() {
            validate_automated_task_authority(&task.worktree, &store, &task, lease).await?;
        }
        if dirty {
            progress.status("Discarding uncommitted Task PR changes...");
            for args in [
                ["reset", "--hard", "HEAD"].as_slice(),
                ["clean", "-fd"].as_slice(),
            ] {
                let output = Command::new("git").args(args).current_dir(repo).output()?;
                if !output.status.success() {
                    return Err(task_error(format!(
                        "failed to discard Task PR changes with `git {}`: {}",
                        args.join(" "),
                        String::from_utf8_lossy(&output.stderr).trim()
                    )));
                }
            }
        }
        progress.status("Closing Task PR...");
        let _ = Command::new("gh")
            .args(["pr", "close", &branch])
            .current_dir(repo)
            .status();
        let now = time::OffsetDateTime::now_utc();
        pr.abandoned_at = Some(now);
        pr.updated_at = now;
        match lease.as_ref() {
            Some(lease) => store.settle_task_pr_for_run(&pr, None, lease).await,
            None => store.settle_task_pr(&pr, None).await,
        }
        .map_err(|error| task_error(format!("failed to settle Task PR: {error}")))?;
        Ok(true)
    })
}

/// Record a Task failure without inventing a second Work lifecycle.
async fn record_task_failure(
    store: &SharedStore,
    task: &mut Task,
    _reason: impl Into<String>,
    error: String,
) -> OpsResult<()> {
    store
        .update_task(task)
        .await
        .map_err(|store_error| task_error(store_error.to_string()))?;
    store
        .append_task_event(
            &task.id,
            &TaskEventKind::Failed {
                error,
                resumable: true,
            },
        )
        .await
        .map_err(|store_error| task_error(store_error.to_string()))?;
    Ok(())
}

/// Start a fresh Run for inactive Task Work.
pub(crate) async fn relaunch_inactive_process(
    store: &SharedStore,
    task: &mut Task,
) -> OpsResult<()> {
    relaunch_inactive_process_with_trigger(store, task, None).await
}

pub(crate) async fn resume_inactive_process(store: &SharedStore, task: &mut Task) -> OpsResult<()> {
    relaunch_inactive_process_with_trigger(store, task, Some(RunTrigger::User)).await
}

pub(crate) async fn relaunch_inactive_process_with_trigger(
    store: &SharedStore,
    task: &mut Task,
    trigger: Option<RunTrigger>,
) -> OpsResult<()> {
    let Some(_) = ensure_working_pr(store, task).await? else {
        return Err(task_error(format!(
            "Task {} is terminal and cannot start a Run",
            task.plan.identifier
        )));
    };
    launch_task_process(store, task, trigger).await
}

const MAX_AUTOMATIC_TASK_RECOVERY_RUNS: usize = 3;

#[derive(Debug, Clone, PartialEq, Eq)]
enum AutomaticTaskRelaunch {
    Idle,
    Launch(RunTrigger),
    Exhausted {
        basis: crate::durable::Basis,
        recoveries: usize,
        error: String,
    },
}

async fn task_recovery_chain(store: &SharedStore, latest: Run) -> OpsResult<(usize, Run)> {
    let mut seen = HashSet::new();
    let mut recoveries = 0;
    let mut run = latest;
    loop {
        if !seen.insert(run.id.clone()) {
            return Err(task_error(format!(
                "Task recovery chain contains a cycle at Run {}",
                run.id
            )));
        }
        let Some(prior_run_id) = run.retry_of.clone() else {
            return Ok((recoveries, run));
        };
        recoveries += 1;
        run = store
            .run_by_id(&prior_run_id)
            .await
            .map_err(|error| task_error(format!("failed to read Task recovery chain: {error}")))?;
    }
}

fn failed_run_made_durable_progress(events: &[crate::task::TaskEvent]) -> bool {
    events
        .iter()
        .skip(1)
        .take_while(|event| !matches!(event.kind, TaskEventKind::Started))
        .any(|event| !matches!(event.kind, TaskEventKind::Failed { .. }))
}

async fn plan_automatic_task_relaunch(
    store: &SharedStore,
    task: &Task,
) -> OpsResult<AutomaticTaskRelaunch> {
    let work = store
        .work_for_child(&ChildRef::Task(task.id.clone()))
        .await
        .map_err(|error| task_error(error.to_string()))?;
    if !matches!(
        store
            .work_status(&work)
            .await
            .map_err(|error| task_error(error.to_string()))?,
        WorkStatus::Ready
    ) {
        return Ok(AutomaticTaskRelaunch::Idle);
    }
    let basis = store
        .current_epoch(&work)
        .await
        .map_err(|error| task_error(error.to_string()))?
        .current_basis;
    let Some(crate::task::TaskEvent {
        kind: TaskEventKind::Failed { error, resumable },
        ..
    }) = store
        .latest_task_event(&task.id)
        .await
        .map_err(|error| task_error(error.to_string()))?
    else {
        return Ok(AutomaticTaskRelaunch::Launch(RunTrigger::Input { basis }));
    };
    if !resumable {
        return Ok(AutomaticTaskRelaunch::Idle);
    }
    let Some(latest_run) = store
        .latest_run(&work)
        .await
        .map_err(|error| task_error(error.to_string()))?
    else {
        return Ok(AutomaticTaskRelaunch::Launch(RunTrigger::Input { basis }));
    };
    let (recoveries, root) = task_recovery_chain(store, latest_run.clone()).await?;
    let recent_events = store
        .recent_task_events(&task.id, 16)
        .await
        .map_err(|store_error| task_error(store_error.to_string()))?;
    let durable_progress = failed_run_made_durable_progress(&recent_events);
    let input_advanced = match &root.trigger {
        RunTrigger::Input { basis: prior } => {
            prior.epoch_id != basis.epoch_id || prior.revision < basis.revision
        }
        _ => false,
    };
    if durable_progress || input_advanced {
        return Ok(AutomaticTaskRelaunch::Launch(RunTrigger::Input { basis }));
    }
    if resumable && recoveries < MAX_AUTOMATIC_TASK_RECOVERY_RUNS {
        return Ok(AutomaticTaskRelaunch::Launch(RunTrigger::Recovery {
            prior_run_id: latest_run.id,
        }));
    }
    Ok(AutomaticTaskRelaunch::Exhausted {
        basis,
        recoveries,
        error,
    })
}

async fn prepare_automatic_task_relaunch(
    store: &SharedStore,
    task: &Task,
) -> OpsResult<Option<RunTrigger>> {
    match plan_automatic_task_relaunch(store, task).await? {
        AutomaticTaskRelaunch::Idle => Ok(None),
        AutomaticTaskRelaunch::Launch(trigger) => Ok(Some(trigger)),
        AutomaticTaskRelaunch::Exhausted {
            basis,
            recoveries,
            error,
        } => {
            let work = store
                .work_for_child(&ChildRef::Task(task.id.clone()))
                .await
                .map_err(|store_error| task_error(store_error.to_string()))?;
            let (_, lease) = store
                .reserve_run(
                    &work,
                    RunTrigger::Input {
                        basis: basis.clone(),
                    },
                )
                .await
                .map_err(|store_error| {
                    task_error(format!(
                        "failed to reserve exhausted Task recovery Run: {store_error}"
                    ))
                })?;
            store
                .advance_run(
                    &lease,
                    crate::durable::RunAdvance::Wait {
                        on: WaitOn::Input {
                            after: basis.clone(),
                        },
                    },
                )
                .await
                .map_err(|store_error| {
                    task_error(format!(
                        "failed to park exhausted Task recovery: {store_error}"
                    ))
                })?;
            tracing::warn!(
                task = %task.plan.identifier,
                recoveries,
                limit = MAX_AUTOMATIC_TASK_RECOVERY_RUNS,
                %error,
                "automatic Task recovery exhausted; waiting for durable input"
            );
            Ok(None)
        }
    }
}

async fn relaunch_inactive_process_automatically(
    store: &SharedStore,
    task: &mut Task,
) -> OpsResult<()> {
    let Some(_) = ensure_working_pr(store, task).await? else {
        return Err(task_error(format!(
            "Task {} is terminal and cannot start a Run",
            task.plan.identifier
        )));
    };
    validate_task_launch(task)?;
    let Some(trigger) = prepare_automatic_task_relaunch(store, task).await? else {
        return Ok(());
    };
    launch_task_process(store, task, Some(trigger)).await
}

async fn launch_task_process(
    store: &SharedStore,
    task: &mut Task,
    trigger: Option<RunTrigger>,
) -> OpsResult<()> {
    let work = store
        .work_for_child(&ChildRef::Task(task.id.clone()))
        .await
        .map_err(|error| task_error(error.to_string()))?;
    if store
        .current_run(&work)
        .await
        .map_err(|error| task_error(error.to_string()))?
        .is_some()
    {
        return Ok(());
    }
    validate_task_launch(task)?;
    let account_id = match preflight_task_execution(&task.worktree, &task.agent).await {
        Ok(account_id) => account_id,
        Err(error) => {
            let error = error.to_string();
            if store
                .latest_task_event(&task.id)
                .await
                .map_err(|store_error| task_error(store_error.to_string()))?
                .as_ref()
                .is_some_and(|event| {
                    matches!(
                        &event.kind,
                        TaskEventKind::Failed {
                            error: previous,
                            resumable: false,
                        } if previous == &error
                    )
                })
            {
                return Err(task_error(error));
            }
            store
                .append_task_event(
                    &task.id,
                    &TaskEventKind::Failed {
                        error: error.clone(),
                        resumable: false,
                    },
                )
                .await
                .map_err(|store_error| task_error(store_error.to_string()))?;
            return Err(task_error(error));
        }
    };
    let trigger = match trigger {
        Some(trigger) => trigger,
        None => RunTrigger::Input {
            basis: store
                .current_epoch(&work)
                .await
                .map_err(|error| task_error(error.to_string()))?
                .current_basis,
        },
    };
    let reservation = store.reserve_run(&work, trigger).await;
    let (run, lease) =
        reservation.map_err(|error| task_error(format!("failed to reserve Task Run: {error}")))?;
    launch_reserved_task_process(store, task, &run, &lease, account_id).await
}

async fn launch_reserved_task_process(
    store: &SharedStore,
    task: &mut Task,
    run: &Run,
    lease: &RunContext,
    account_id: ProviderAccountId,
) -> OpsResult<()> {
    let tmux_name = format!(
        "lf-task-{}-{}-{}",
        tmux_session_slug(&task.plan.identifier),
        &task.id.as_str()[3..11],
        &run.id.as_str()[4..12]
    );
    if let Err(error) = store.update_task_for_run(task, lease).await {
        store
            .fail_task_run(&task.id, lease, &error.to_string(), true)
            .await
            .map_err(|settle_error| {
                task_error(format!(
                    "failed to prepare reserved Task Run ({error}); failed to settle it: {settle_error}"
                ))
            })?;
        return Err(task_error(error.to_string()));
    }
    crate::ops::launch_in_run(
        store,
        lease,
        crate::ops::RunLaunch {
            work: WorkRef::Task(task.id.clone()),
            wave_id: task.wave_id.clone(),
            cwd: task.worktree.clone(),
            tmux_name,
            agent: task.agent.clone(),
            account_id: Some(account_id),
            resume_token: task.provider_session_id.clone(),
        },
    )
    .await
    .map(|_| ())
    .map_err(|error| task_error(error.to_string()))
}

async fn wait_until_running(store: &SharedStore, task_id: &crate::task::TaskId) -> OpsResult<Task> {
    let deadline = tokio::time::Instant::now() + super::child::CHILD_STARTUP_GRACE;
    loop {
        let task = store
            .get_task(task_id)
            .await
            .map_err(|error| task_error(format!("failed to observe task startup: {error}")))?
            .ok_or_else(|| task_error("task task disappeared during startup"))?;
        match task_work_status(store, &task).await? {
            WorkStatus::Running { .. } => return Ok(task),
            WorkStatus::Done | WorkStatus::Abandoned => {
                return Err(task_error(format!(
                    "task {} ended during startup",
                    task.plan.identifier
                )))
            }
            WorkStatus::Ready | WorkStatus::Waiting { .. } => {}
        }
        if tokio::time::Instant::now() >= deadline {
            return Err(task_error(format!(
                "task {} process did not report running within 10 seconds",
                task.plan.identifier
            )));
        }
        tokio::time::sleep(Duration::from_millis(100)).await;
    }
}

pub(crate) async fn reconcile_process_liveness(
    store: &SharedStore,
    task: &mut Task,
) -> OpsResult<()> {
    let work = store
        .work_for_child(&ChildRef::Task(task.id.clone()))
        .await
        .map_err(|error| task_error(error.to_string()))?;
    let Some(run) = store
        .current_run(&work)
        .await
        .map_err(|error| task_error(error.to_string()))?
    else {
        return Ok(());
    };
    store
        .ensure_local_run(&run)
        .await
        .map_err(|error| task_error(error.to_string()))?;
    if run.state == RunState::Reserved {
        let still_starting =
            run.created_at + time::Duration::seconds(10) > time::OffsetDateTime::now_utc();
        if still_starting {
            return Ok(());
        }
    }
    let observation = match run.containment.as_ref() {
        Some(containment) => crate::engine::process::containment_observation(containment).await,
        None => ContainmentObservation::Unprovable,
    };
    if observation != ContainmentObservation::Absent {
        store
            .record_run_liveness(&run.id, observation)
            .await
            .map_err(|error| task_error(error.to_string()))?;
        return Ok(());
    }
    if run.containment.is_some()
        && run.started_at.is_some_and(|started_at| {
            started_at + time::Duration::seconds(10) > time::OffsetDateTime::now_utc()
        })
    {
        return Ok(());
    }
    store
        .recover_run(&run.id, observation)
        .await
        .map_err(|error| task_error(error.to_string()))?;
    mark_task_body_lost(store, task).await
}

async fn mark_task_body_lost(store: &SharedStore, task: &mut Task) -> OpsResult<()> {
    if park_invalid_lifecycle(store, task).await? {
        return Ok(());
    }
    if task_launch_refusal_after_lifecycle_validation(store, task)
        .await
        .map_err(|error| task_error(format!("failed to read Task blocker: {error}")))?
        .is_some()
    {
        return Ok(());
    }
    let active = store
        .active_task_pr(&task.id)
        .await
        .map_err(|error| task_error(format!("failed to read active PR: {error}")))?;
    if active
        .as_ref()
        .is_none_or(|pr| pr.phase() == PrPhase::Open && pr.merge_request().is_some())
    {
        return Ok(());
    }
    // Do not write a human instruction into a durable field and stop. This line
    // was the strand: it told a person to type `lf task resume` and nothing ever
    // read it, so 13 Tasks sat frozen until someone swept them by hand. The
    // Run slot is now released above; redispatch this same durable Work after
    // the ordinary adoption safety check.
    let reason = "task process is missing; Loopflow will recover this Task";
    record_task_failure(store, task, reason, reason.to_string()).await?;
    if let Err(error) = task_recovery_adoption(store, task).await {
        tracing::info!(
            task = %task.plan.identifier,
            "not recovering missing Task body: {error}"
        );
        return Ok(());
    }
    relaunch_inactive_process_automatically(store, task).await
}

async fn park_invalid_lifecycle(store: &SharedStore, task: &Task) -> OpsResult<bool> {
    let Some(error) = validate_task_lifecycle(task)
        .err()
        .map(|error| error.to_string())
    else {
        return Ok(false);
    };
    let already_recorded = store
        .latest_task_event(&task.id)
        .await
        .map_err(|store_error| task_error(store_error.to_string()))?
        .is_some_and(|event| {
            matches!(
                event.kind,
                TaskEventKind::Failed {
                    error: recorded,
                    resumable: true,
                } if recorded == error
            )
        });
    if !already_recorded {
        store
            .append_task_event(
                &task.id,
                &TaskEventKind::Failed {
                    error: error.clone(),
                    resumable: true,
                },
            )
            .await
            .map_err(|store_error| task_error(store_error.to_string()))?;
        tracing::warn!(
            task = %task.plan.identifier,
            %error,
            "Task lifecycle is invalid; resident recovery is parked"
        );
    }
    Ok(true)
}
pub(crate) async fn reconcile_task_pr(
    store: &SharedStore,
    task: &mut Task,
) -> OpsResult<Option<TaskPr>> {
    reconcile_task_pr_with_authority(store, task, None, crate::ops::pr::PrReadFreshness::Cached)
        .await
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
    reconcile_process_liveness(store, &mut task).await?;
    let pr = reconcile_task_pr_with_authority(
        store,
        &mut task,
        None,
        crate::ops::pr::PrReadFreshness::Fresh,
    )
    .await?
    .ok_or_else(|| task_error("landing Task PR disappeared during merge settlement"))?;
    apply_merged_task_landing(store, &mut task, &pr, landing).await
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
        Some(AfterMerge::CompleteTask) => reconcile_task_completion(store, task, None).await,
        Some(AfterMerge::ContinueTask) if landing.next_slug.is_some() => {
            ensure_working_pr(store, task).await.map(|_| ())
        }
        Some(AfterMerge::ContinueTask) | None => Ok(()),
    }
}

pub(crate) fn open_pr_wait_reason(pr: &TaskPr) -> String {
    let number = pr
        .github()
        .expect("open Task PR requires a GitHub PR record")
        .number;
    match pr.merge_request() {
        Some(request) if request.mode == PrMergeMode::User => {
            let short = request.head_sha.chars().take(12).collect::<String>();
            format!("pull request #{number} awaits the user's explicit merge of head {short}")
        }
        Some(request) => {
            let short = request.head_sha.chars().take(12).collect::<String>();
            format!("pull request #{number} awaits GitHub auto-merge of head {short}")
        }
        None => format!("pull request #{number} is published; no merge was requested"),
    }
}

pub(crate) async fn reconcile_task_pr_for_run(
    store: &SharedStore,
    task: &mut Task,
    lease: &RunContext,
) -> OpsResult<Option<TaskPr>> {
    reconcile_task_pr_with_authority(
        store,
        task,
        Some(lease),
        crate::ops::pr::PrReadFreshness::Cached,
    )
    .await
}

/// Read the open PR's required checks and classify them for `head_sha`. Returns
/// `None` — no current-head CI owner can be derived — when GitHub reports no
/// head, there are no required checks, or gh is unavailable.
/// Failure dominates: any failing required check makes the head `Failing` even
/// while others are still pending.
fn observe_required_checks(
    worktree: &Path,
    branch: &str,
    head_sha: Option<&str>,
    now: time::OffsetDateTime,
) -> Option<CiObservation> {
    let head_sha = head_sha?.to_string();
    let checks = crate::ops::pr::merge_gate_state(worktree, branch)
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
/// row GitHub could still contradict.
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
    Ok(prs
        .into_iter()
        .next_back()
        .filter(|pr| pr.phase() == PrPhase::Abandoned && pr.github().is_some()))
}

async fn reconcile_task_pr_with_authority(
    store: &SharedStore,
    task: &mut Task,
    lease: Option<&RunContext>,
    freshness: crate::ops::pr::PrReadFreshness,
) -> OpsResult<Option<TaskPr>> {
    if let Some(lease) = lease {
        validate_automated_task_authority(&task.worktree, store, task, lease).await?;
    }
    // Reconciliation updates the same projection as publication/finalization.
    // Refuse overlap so a remote read begun before a push cannot overwrite the
    // request or head recorded by the command that completed after it.
    let _mutation = lock_task_pr_mutation(&task.worktree)?;
    let Some(mut pr) = reconcile_subject(store, task).await? else {
        return Ok(None);
    };
    // GitHub is a reconciliation input, not the Task's store of record. Read the
    // one persisted PR by number (a single bounded REST call, never `gh pr
    // list`); an unpublished working PR has no number and is not read remotely.
    // Recent attempts are reused across processes. A quota/network/GitHub failure
    // opens a durable circuit and keeps the cached row rather than erroring the
    // control command that triggered reconcile.
    let Some(number) = pr.github().map(|github| github.number) else {
        task.observation = Observation::NotRequired;
        return Ok(Some(pr));
    };
    let now = time::OffsetDateTime::now_utc();
    // A fresh caller must never receive the store's warm head observation.
    if matches!(freshness, crate::ops::pr::PrReadFreshness::Cached) {
        if let Some(observation) = cached_github_observation(&pr, now) {
            task.observation = observation;
            return Ok(Some(pr));
        }
    }
    let previous = pr.clone();
    let github_pr =
        match crate::ops::pr::observe_pr_by_number(&task.worktree, number, &pr.branch, freshness) {
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
                update_task_pr_with_authority(store, &pr, lease)
                    .await
                    .map_err(|error| task_error(error.to_string()))?;
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
                update_task_pr_with_authority(store, &pr, lease)
                    .await
                    .map_err(|error| task_error(error.to_string()))?;
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
    let previous_gate_proposal = task.gate_proposal.clone();
    let previous_pm_writeback = task.pm_writeback.clone();
    let publication = pr.publication.get_or_insert(PrPublication {
        requested_at: now,
        presentation: None,
        github: None,
        merge: None,
    });
    invalidate_stale_merge_request(&task.worktree, publication, &github_pr)?;
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
            // Record the merge, but withhold completion while an accepted
            // directive is unincorporated — an auto-merge armed by `lf pr land`
            // must not silently erase direction accepted after it was armed — or
            // while the branch holds follow-up committed past the merged tip,
            // which another serial PR still owes. The PR is settling in flight and
            // is not on disk yet, so its range is read from it directly.
            let completes = pr.after_merge() == AfterMerge::CompleteTask
                && matches!(
                    committed_follow_up_range(&task.worktree, &pr)?,
                    CommittedFollowUp::ProvenEmpty
                );
            if completes {
                let proposal = crate::task::TaskGateProposal {
                    done: true,
                    reason: format!(
                        "pull request #{} merged and completed the Task",
                        github_pr.number
                    ),
                };
                match task.lifecycle_phase {
                    crate::task::TaskLifecyclePhase::First => {
                        task.enter_loop()
                            .map_err(|error| task_error(error.to_string()))?;
                        task.enter_finally(proposal)
                            .map_err(|error| task_error(error.to_string()))?;
                    }
                    crate::task::TaskLifecyclePhase::Loop => {
                        task.enter_finally(proposal)
                            .map_err(|error| task_error(error.to_string()))?;
                    }
                    crate::task::TaskLifecyclePhase::Finally => {
                        task.gate_proposal = Some(proposal);
                        task.updated_at = now;
                    }
                }
                reconcile_pm_writeback(store, task, Some(&url)).await;
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
                &pr.branch,
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
            let outcome =
                settle_task_pr_merged_with_authority(store, &pr, authoritative_merged_at, lease)
                    .await
                    .map_err(|error| task_error(error.to_string()))?;
            if let crate::store::TaskPrMergeEvidenceOutcome::Conflict { accepted_at } = outcome {
                let reason =
                    format!("GitHub merged_at conflicts with first accepted value {accepted_at}");
                pr.github_observation = Some(GithubObservation {
                    checked_at: now,
                    result: GithubObservationResult::Partial { reason },
                });
            }
        } else if pr.is_settled() {
            settle_task_pr_with_authority(store, &pr, None, lease)
                .await
                .map_err(|error| task_error(error.to_string()))?;
        } else {
            update_task_pr_with_authority(store, &pr, lease)
                .await
                .map_err(|error| task_error(error.to_string()))?;
        }
    }
    if task.gate_proposal != previous_gate_proposal || task.pm_writeback != previous_pm_writeback {
        update_task_with_authority(store, task, lease)
            .await
            .map_err(|error| task_error(error.to_string()))?;
    }
    if pr_changed {
        if let Some(event) = pr_event {
            let should_append = match &event {
                TaskEventKind::PrOpened { .. } => previous_github.as_ref() != pr.github(),
                TaskEventKind::PrMerged { .. } => previous_phase != PrPhase::Merged,
                _ => true,
            };
            if should_append {
                append_task_event_with_authority(store, &task.id, &event, lease)
                    .await
                    .map_err(|error| task_error(error.to_string()))?;
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
/// `ensure_working_pr_with_authority` would cut. The recovery gate reads this so
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

/// The branch/worktree state a Task recovery must adopt, computed read-only
/// from the durable PR sequence and the worktree before any ownership moves.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum TaskRecoveryAdoption {
    /// An active PR owns the worktree; the body continues on its branch. A dirty
    /// working tree is allowed — ongoing work survives recovery.
    Active { branch: String },
    /// No active PR: between-PR recovery. The worktree sits on the settled
    /// branch or the deterministic next serial branch; the runner rotates.
    BetweenPrs { settled: String, next: String },
}

/// Compute every branch/worktree/PR adoption precondition for Task recovery
/// before any durable ownership moves. Read-only: it touches neither the store,
/// the lease, the PR sequence, nor the worktree, so refusal leaves the
/// predecessor, successor link, PR sequence, leases, and worktree untouched.
pub(crate) async fn task_recovery_adoption(
    store: &SharedStore,
    task: &Task,
) -> OpsResult<TaskRecoveryAdoption> {
    let worktree = &task.worktree;
    let identifier = &task.plan.identifier;
    if !worktree.exists() {
        return Err(task_error(format!(
            "Task {identifier} worktree {} is missing; recovery refused before moving any ownership",
            worktree.display()
        )));
    }
    if let Some(state) = crate::engine::git::intervention_state(worktree)
        .map_err(|error| task_error(format!("failed to inspect Task worktree state: {error}")))?
    {
        return Err(task_error(format!(
            "Task {identifier} worktree {} is mid-{state}; resolve or abort it before resuming, \
             recovery refused before moving any ownership",
            worktree.display()
        )));
    }
    let current = current_branch(worktree)
        .map_err(|error| task_error(format!("failed to inspect Task branch: {error}")))?
        .ok_or_else(|| {
            task_error(format!(
                "Task {identifier} worktree {} is detached; recovery needs a branch",
                worktree.display()
            ))
        })?;
    if !ref_exists(worktree, &format!("refs/heads/{current}"))
        .map_err(|error| task_error(format!("failed to inspect Task branch: {error}")))?
    {
        return Err(task_error(format!(
            "Task {identifier} worktree {} is on branch {current:?} which no longer exists; \
             re-create it or recover the worktree before resuming",
            worktree.display()
        )));
    }
    if let Some(active) = store
        .active_task_pr(&task.id)
        .await
        .map_err(|error| task_error(format!("failed to read active PR: {error}")))?
    {
        if current != active.branch {
            return Err(task_error(format!(
                "Task {identifier} active PR expects branch {:?}, but the worktree is on \
                 {current:?}; recovery refused before moving any ownership",
                active.branch
            )));
        }
        return Ok(TaskRecoveryAdoption::Active {
            branch: active.branch,
        });
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
    let next = deterministic_next_branch(task, &settled, None)?;
    if current != settled.branch && current != next {
        return Err(task_error(format!(
            "Task {identifier} between-PR recovery expected settled branch {:?} or next branch \
             {next:?}, but the worktree is on {current:?}; recovery refused before moving any \
             ownership",
            settled.branch
        )));
    }
    if !is_clean(worktree)
        .map_err(|error| task_error(format!("failed to inspect Task worktree: {error}")))?
    {
        return Err(task_error(format!(
            "Task {identifier} cannot recover between PRs while {} has uncommitted changes; \
             carry them forward with `lf pr next` or commit before resuming, recovery refused \
             before moving any ownership",
            worktree.display()
        )));
    }
    Ok(TaskRecoveryAdoption::BetweenPrs {
        settled: settled.branch,
        next,
    })
}

/// Refuse a dirty between-PR worktree after PR reconciliation. The runner's
/// strict rotation cannot carry a dirty tree, so catching this before the lease
/// is reaped or a successor body is launched keeps ownership put. Read-only.
pub(crate) async fn refuse_dirty_between_prs(store: &SharedStore, task: &Task) -> OpsResult<()> {
    if store
        .active_task_pr(&task.id)
        .await
        .map_err(|error| task_error(format!("failed to read active PR: {error}")))?
        .is_some()
    {
        return Ok(());
    }
    if is_clean(&task.worktree)
        .map_err(|error| task_error(format!("failed to inspect Task worktree: {error}")))?
    {
        return Ok(());
    }
    Err(task_error(format!(
        "Task {} cannot recover between PRs while {} has uncommitted changes; carry them \
         forward with `lf pr next` or commit before resuming",
        task.plan.identifier,
        task.worktree.display()
    )))
}

pub(crate) async fn ensure_working_pr(
    store: &SharedStore,
    task: &mut Task,
) -> OpsResult<Option<TaskPr>> {
    ensure_working_pr_with_authority(store, task, None, RotateOptions::runner()).await
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
/// same expression `verify_task_pr_range_with_authority` asserts before every
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
async fn heal_incoherent_base(
    store: &SharedStore,
    task: &Task,
    pr: TaskPr,
    lease: Option<&RunContext>,
) -> OpsResult<TaskPr> {
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
    match lease {
        Some(lease) => store.heal_task_pr_base_for_run(&healed, lease).await,
        None => store.heal_task_pr_base(&healed).await,
    }
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
    Some(format!(
        "Task {identifier} has no active PR to resume; {suffix}"
    ))
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

async fn ensure_working_pr_with_authority(
    store: &SharedStore,
    task: &mut Task,
    lease: Option<&RunContext>,
    rotate: RotateOptions,
) -> OpsResult<Option<TaskPr>> {
    validate_task_launch(task)?;
    reconcile_task_pr_with_authority(store, task, lease, crate::ops::pr::PrReadFreshness::Cached)
        .await?;
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
        return Ok(Some(
            heal_incoherent_base(store, task, active, lease).await?,
        ));
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
    if let Some(lease) = lease {
        validate_automated_task_authority(&task.worktree, store, task, lease).await?;
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
    push_with_upstream(&task.worktree, "origin", &branch)
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
    match settle_task_pr_with_authority(store, &settled, Some(&next), lease).await {
        Ok(()) => {
            append_task_event_with_authority(
                store,
                &task.id,
                &TaskEventKind::PrStarted {
                    pr_id: next.id.clone(),
                    sequence: next.sequence,
                    branch: next.branch.clone(),
                    base_commit: next.base_commit.clone(),
                },
                lease,
            )
            .await
            .map_err(|error| task_error(error.to_string()))?;
            Ok(Some(next))
        }
        Err(error) => {
            let recovered = store
                .task_prs(&task.id)
                .await
                .map_err(|read_error| task_error(read_error.to_string()))?
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
        let (mut task, lease) = task_for_worktree(&store, &repo)
            .await?
            .ok_or_else(|| task_error("no Task owns this worktree"))?;
        // Observe an out-of-band merge before deciding whether to rotate.
        reconcile_task_pr_with_authority(
            &store,
            &mut task,
            lease.as_ref(),
            crate::ops::pr::PrReadFreshness::Cached,
        )
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
        ensure_working_pr_with_authority(&store, &mut task, lease.as_ref(), rotate)
            .await?
            .ok_or_else(|| task_error("Task has no settled PR to rotate from"))
    })
}

pub fn task_status(issue: &str) -> OpsResult<Task> {
    block_on_task(async move {
        let store = task_store().await?;
        let mut task = store
            .get_task_by_issue(issue)
            .await
            .map_err(|error| task_error(format!("failed to read task status: {error}")))?
            .ok_or_else(|| task_error(format!("no Task exists for {issue:?}")))?;
        let launch_refusal = task_launch_refusal(&store, &task)
            .await
            .map_err(|error| task_error(format!("failed to read Task blocker: {error}")))?;
        if launch_refusal.is_none() && task_worktree_blocker(&store, &task).await?.is_none() {
            let observed = reconcile_task_pr(&store, &mut task).await?;
            let user_merged = observed.as_ref().is_some_and(|pr| {
                pr.phase() == PrPhase::Merged
                    && pr
                        .merge_request()
                        .is_some_and(|request| request.mode == PrMergeMode::User)
            });
            if user_merged {
                reconcile_task_completion(&store, &mut task, None).await?;
            }
        }
        Ok(task)
    })
}

/// Find a final Task whose only active PR is the empty artifact of rotating
/// past already-merged work. Scratch is gate evidence, not another PR slice.
pub(crate) fn find_discardable_final_task(repo: &Path) -> OpsResult<Option<String>> {
    let repo = repo.to_path_buf();
    block_on_task(async move {
        let TaskAuthority::Authority { store, task, .. } = resolve_task_authority(&repo).await?
        else {
            return Ok(None);
        };
        if task.lifecycle_phase != crate::task::TaskLifecyclePhase::Finally {
            return Ok(None);
        }
        let materially_clean = is_materially_clean(&task.worktree)
            .map_err(|error| task_error(format!("failed to inspect Task worktree: {error}")))?;
        if !materially_clean {
            return Ok(None);
        }
        let gate = task_completion_gate(&store, &task).await?;
        if !gate.satisfied || gate.discardable_successor.is_none() {
            return Ok(None);
        }
        require_approved_task_completion(&task)?;
        Ok(Some(task.plan.identifier.clone()))
    })
}

pub fn task_complete(issue: &str, summary: String) -> OpsResult<Task> {
    let summary = summary.trim().to_string();
    if summary.is_empty() {
        return Err(task_error("completion summary cannot be empty"));
    }
    complete_task(issue, TaskCompletionAuthority::Explicit { summary })
}

pub(crate) fn task_complete_approved(issue: &str) -> OpsResult<Task> {
    complete_task(issue, TaskCompletionAuthority::Approved)
}

enum TaskCompletionAuthority {
    Explicit { summary: String },
    Approved,
}

fn complete_task(issue: &str, authority: TaskCompletionAuthority) -> OpsResult<Task> {
    block_on_task(async move {
        let store = task_store().await?;
        let mut task = store
            .get_task_by_issue(issue)
            .await
            .map_err(|error| task_error(format!("failed to read Task: {error}")))?
            .ok_or_else(|| task_error(format!("no Task exists for {issue:?}")))?;
        let lease = ambient_task_run_context(&store, &task).await?;
        reconcile_task_pr_with_authority(
            &store,
            &mut task,
            lease.as_ref(),
            crate::ops::pr::PrReadFreshness::Cached,
        )
        .await?;
        let work = store
            .work_for_child(&ChildRef::Task(task.id.clone()))
            .await
            .map_err(|error| task_error(error.to_string()))?;
        match store
            .work_status(&work)
            .await
            .map_err(|error| task_error(error.to_string()))?
        {
            WorkStatus::Done => return Ok(task),
            WorkStatus::Abandoned => {
                return Err(task_error(format!(
                    "Task {} is abandoned and cannot be completed",
                    task.plan.identifier
                )))
            }
            WorkStatus::Ready | WorkStatus::Running { .. } | WorkStatus::Waiting { .. } => {}
        }
        if !is_clean(&task.worktree)
            .map_err(|error| task_error(format!("failed to inspect Task worktree: {error}")))?
        {
            return Err(task_error(
                "Task worktree has uncommitted changes; publish or explicitly abandon them first",
            ));
        }
        if matches!(&authority, TaskCompletionAuthority::Approved) {
            require_approved_task_completion(&task)?;
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
        if let TaskCompletionAuthority::Explicit { summary } = authority {
            propose_task_done(&mut task, summary)?;
        }
        // Every other condition is now proven, so the rotation's empty artifact
        // is dropped as part of completing — one transaction that deletes the row
        // and writes the terminal status together. There is no instant at which
        // the successor is gone and the Task is not yet terminal, which is the
        // only state `ensure_working_pr_with_authority` would rotate from.
        complete_task_with_authority(
            &store,
            &task,
            gate.discardable_successor.as_ref(),
            lease.as_ref(),
        )
        .await
        .map_err(|error| task_error(format!("failed to complete Task: {error}")))?;
        if lease.is_none() {
            reconcile_pm_writeback(&store, &mut task, None).await;
            store
                .update_task(&task)
                .await
                .map_err(|error| task_error(error.to_string()))?;
        }
        Ok(task)
    })
}

fn require_approved_task_completion(task: &Task) -> OpsResult<()> {
    let proposal = task
        .approved_gate_proposal()
        .map_err(|error| task_error(error.to_string()))?;
    if proposal.done {
        return Ok(());
    }
    Err(task_error(format!(
        "Task {} final gate did not approve completion: {}",
        task.plan.identifier, proposal.reason
    )))
}

fn propose_task_done(task: &mut Task, summary: String) -> OpsResult<()> {
    let proposal = crate::task::TaskGateProposal {
        done: true,
        reason: summary,
    };
    match task.lifecycle_phase {
        crate::task::TaskLifecyclePhase::First => {
            task.enter_loop()
                .map_err(|error| task_error(error.to_string()))?;
            task.enter_finally(proposal)
                .map_err(|error| task_error(error.to_string()))?;
        }
        crate::task::TaskLifecyclePhase::Loop => {
            task.enter_finally(proposal)
                .map_err(|error| task_error(error.to_string()))?;
        }
        crate::task::TaskLifecyclePhase::Finally => {
            task.gate_proposal = Some(proposal);
            task.updated_at = time::OffsetDateTime::now_utc();
        }
    }
    Ok(())
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
        crate::ops::pm::pm_link_pr_async(&task.worktree, wave.name(), &request, &prior).await;
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
    writeback_state_for(PmWritebackOperation::CompleteTask, result)
}

fn writeback_state_for(operation: PmWritebackOperation, result: OpsResult<()>) -> PmWritebackState {
    match result {
        Ok(()) => PmWritebackState::Current,
        Err(error) => PmWritebackState::Pending {
            operation,
            error: error.to_string(),
        },
    }
}

pub(crate) async fn reconcile_pm_writeback(
    store: &SharedStore,
    task: &mut Task,
    pr_url: Option<&str>,
) {
    let Ok(wave) = owning_wave(store, task).await else {
        task.pm_writeback = PmWritebackState::Pending {
            operation: PmWritebackOperation::CompleteTask,
            error: format!("owning Wave {} is not registered", task.wave_id),
        };
        return;
    };
    task.pm_writeback = writeback_state(
        crate::ops::task_pm::complete_task(
            &task.worktree,
            wave.name(),
            task.plan.id.as_str(),
            pr_url,
        )
        .await,
    );
}

async fn retry_pm_writeback(store: &SharedStore, task: &mut Task) {
    let Ok(prs) = store.task_prs(&task.id).await else {
        return;
    };
    let pr_url = prs
        .iter()
        .rev()
        .find_map(|pr| pr.github().map(|github| github.url.as_str()));
    let Ok(wave) = owning_wave(store, task).await else {
        task.pm_writeback = PmWritebackState::Pending {
            operation: PmWritebackOperation::CompleteTask,
            error: format!("owning Wave {} is not registered", task.wave_id),
        };
        return;
    };
    task.pm_writeback = {
        let operation = match &task.pm_writeback {
            PmWritebackState::Pending { operation, .. } => *operation,
            PmWritebackState::Current => PmWritebackOperation::CompleteTask,
        };
        let result = crate::ops::task_pm::retry_complete_task(
            &task.worktree,
            wave.name(),
            task.plan.id.as_str(),
            pr_url,
        )
        .await;
        writeback_state_for(operation, result)
    };
    task.updated_at = time::OffsetDateTime::now_utc();
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
    /// passes it as the completion transaction's `skipped_pr`, which drops the
    /// row and writes the terminal status together. Discarding it any earlier
    /// would leave a non-terminal Task with no active PR — the state
    /// [`ensure_working_pr_with_authority`] rotates another empty PR from.
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
    if let Some(blocker) = task_worktree_blocker(store, task).await? {
        gate.satisfied = false;
        gate.blockers.push(blocker.reason);
        return Ok(gate);
    }
    let work_done = task_work_status(store, task).await? == WorkStatus::Done;

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

async fn advance_completion_after_gate(
    store: &SharedStore,
    task: &mut Task,
    lease: Option<&RunContext>,
) -> OpsResult<bool> {
    if let Some(lease) = lease {
        validate_automated_task_authority(&task.worktree, store, task, lease).await?;
    }
    let work = store
        .work_for_child(&ChildRef::Task(task.id.clone()))
        .await
        .map_err(|error| task_error(error.to_string()))?;
    match store
        .work_status(&work)
        .await
        .map_err(|error| task_error(error.to_string()))?
    {
        WorkStatus::Done | WorkStatus::Abandoned => return Ok(false),
        WorkStatus::Running { .. } if lease.is_none() => return Ok(false),
        WorkStatus::Ready | WorkStatus::Waiting { .. } | WorkStatus::Running { .. } => {}
    }
    let Some(pr) = merged_completing_pr(store, task).await? else {
        return Ok(false);
    };
    let gate = task_completion_gate(store, task).await?;
    if !gate.satisfied {
        return Ok(false);
    }
    // This path completes through `complete_task_after_pr`, which settles
    // the merged PR and has no `skipped_pr`, so it cannot drop a successor in the
    // same transaction. Rather than complete and leave the row active — a
    // completed Task still holding an active PR — decline and let `lf task
    // complete` own this shape. Unreachable on current rows: a `CompleteTask`
    // merge no longer rotates, so it never has a successor to discard.
    if gate.discardable_successor.is_some() {
        return Ok(false);
    }
    let url = pr.github().map(|github| github.url.clone());
    let summary = format!(
        "pull request #{} merged and completed the Task",
        pr.github().map(|github| github.number).unwrap_or_default()
    );
    propose_task_done(task, summary.clone())?;
    complete_task_after_pr_with_authority(store, task, &pr, lease)
        .await
        .map_err(|error| task_error(error.to_string()))?;
    if lease.is_none() {
        reconcile_pm_writeback(store, task, url.as_deref()).await;
        store
            .update_task(task)
            .await
            .map_err(|error| task_error(error.to_string()))?;
    }
    Ok(true)
}

pub(crate) async fn reconcile_task_completion(
    store: &SharedStore,
    task: &mut Task,
    lease: Option<&RunContext>,
) -> OpsResult<()> {
    let status = task_work_status(store, task).await?;
    if status == WorkStatus::Done && matches!(task.pm_writeback, PmWritebackState::Pending { .. }) {
        retry_pm_writeback(store, task).await;
        if let Some(lease) = lease {
            store
                .update_task_for_run(task, lease)
                .await
                .map_err(|error| task_error(error.to_string()))?;
        } else {
            store
                .update_task(task)
                .await
                .map_err(|error| task_error(error.to_string()))?;
        }
        return Ok(());
    }
    if !matches!(status, WorkStatus::Done | WorkStatus::Abandoned) {
        advance_completion_after_gate(store, task, lease).await?;
    }
    Ok(())
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
        let run = store
            .current_run(&work)
            .await
            .map_err(|error| task_error(error.to_string()))?;
        let invocation = match &run {
            Some(run) => store
                .open_invocation_for_run(&run.id)
                .await
                .map_err(|error| task_error(error.to_string()))?,
            None => None,
        };
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
        let work_status = store
            .work_status(&work)
            .await
            .map_err(|error| task_error(format!("failed to derive Task Work status: {error}")))?;
        let launch_refusal = if worktree_blocker.is_some() {
            None
        } else {
            task_configuration_refusal(&task)
                .or_else(|| task_event_launch_refusal(latest_event.as_ref()).map(str::to_string))
        };
        let current = crate::child::observe_current_work(
            &store,
            &work,
            &work_status,
            time::OffsetDateTime::now_utc(),
        )
        .await
        .map_err(|error| task_error(error.to_string()))?;
        let action_evidence = TaskActionEvidence {
            status: work_status.clone(),
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
            current_state: current.state,
            predecessor_phase,
            abandon_intent: task.abandon_intent.is_some(),
            local_progress_unsettled: None,
            launch_refusal: launch_refusal.as_deref(),
        };
        let actions = derive_task_actions(&action_evidence);
        Ok(TaskSnapshot {
            issue_id: task.plan.id.as_str().to_string(),
            issue_identifier: task.plan.identifier,
            task_id: task.id.to_string(),
            external_project_id: project.plan.id.as_str().to_string(),
            project: project.plan.slug,
            pm_snapshot_synced_at: task.plan.pm_snapshot_synced_at,
            pm_writeback: task.pm_writeback,
            wave: wave.name().to_string(),
            project_id: task.project_id.to_string(),
            status: work_status,
            current,
            worktree: task.worktree.display().to_string(),
            workspace_slug: task.workspace_slug,
            lifecycle: task.lifecycle,
            lifecycle_phase: task.lifecycle_phase,
            phase_epoch: task.phase_epoch,
            phase_cursor: task.phase_cursor,
            phase_iteration: task.phase_iteration,
            gate_cycle: task.gate_cycle,
            gate_proposal: task.gate_proposal,
            prs,
            active_pr,
            agent: task.agent,
            provider: task.provider,
            provider_session_id: task.provider_session_id,
            invocation,
            latest_event,
            created_at: task.created_at,
            updated_at: task.updated_at,
            observation: task.observation,
            actions,
        })
    })
}

pub fn task_changes(issue: &str) -> OpsResult<TaskChangesSnapshot> {
    let task = task_status(issue)?;
    let pr = active_pr(&task)?;
    changes_snapshot(TaskWorkspace::new(&task, &pr))
}

fn changes_snapshot(workspace: TaskWorkspace<'_>) -> OpsResult<TaskChangesSnapshot> {
    let mut files = BTreeMap::<String, TaskChangedFile>::new();
    record_changed_paths(
        workspace.worktree,
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
        workspace.worktree,
        &["diff", "--cached", "--name-only", "-z"],
        &mut files,
        |file| file.staged = true,
    )?;
    record_changed_paths(
        workspace.worktree,
        &["diff", "--name-only", "-z"],
        &mut files,
        |file| file.unstaged = true,
    )?;
    record_changed_paths(
        workspace.worktree,
        &["ls-files", "--others", "--exclude-standard", "-z"],
        &mut files,
        |file| file.untracked = true,
    )?;
    let head_commit = git_output(workspace.worktree, &["rev-parse", "HEAD"])?
        .trim()
        .to_string();
    Ok(TaskChangesSnapshot {
        issue_identifier: workspace.issue_identifier.to_string(),
        task_id: workspace.task_id.to_string(),
        base_commit: workspace.base_commit.to_string(),
        head_commit,
        files: files.into_values().collect(),
    })
}

pub fn task_diff(issue: &str, path: Option<&str>) -> OpsResult<TaskDiffSnapshot> {
    let task = task_status(issue)?;
    let pr = active_pr(&task)?;
    diff_snapshot(TaskWorkspace::new(&task, &pr), path)
}

fn diff_snapshot(workspace: TaskWorkspace<'_>, path: Option<&str>) -> OpsResult<TaskDiffSnapshot> {
    const MAX_PATCH_BYTES: usize = 1_000_000;

    let relative = path.map(validate_task_relative_path).transpose()?;
    let mut args = vec![
        "diff".to_string(),
        "--no-ext-diff".to_string(),
        "--no-color".to_string(),
        workspace.base_commit.to_string(),
        "--".to_string(),
    ];
    if let Some(path) = &relative {
        args.push(path.clone());
    }
    let mut patch = git_output_owned(workspace.worktree, &args)?;
    let untracked = untracked_paths(workspace.worktree)?;
    let include_untracked = untracked
        .into_iter()
        .filter(|candidate| relative.as_ref().is_none_or(|path| path == candidate));
    for path in include_untracked {
        let output = Command::new("git")
            .current_dir(workspace.worktree)
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
    let truncated = patch.len() > MAX_PATCH_BYTES;
    if truncated {
        patch.truncate(MAX_PATCH_BYTES);
    }
    let patch = String::from_utf8_lossy(&patch).into_owned();
    let binary = patch.contains("Binary files ") || patch.contains("GIT binary patch");
    Ok(TaskDiffSnapshot {
        issue_identifier: workspace.issue_identifier.to_string(),
        task_id: workspace.task_id.to_string(),
        path: relative,
        patch,
        binary,
        truncated,
    })
}

pub fn task_file(issue: &str, path: &str) -> OpsResult<TaskFileSnapshot> {
    let task = task_status(issue)?;
    let pr = active_pr(&task)?;
    file_snapshot(TaskWorkspace::new(&task, &pr), path)
}

fn file_snapshot(workspace: TaskWorkspace<'_>, path: &str) -> OpsResult<TaskFileSnapshot> {
    const MAX_FILE_BYTES: usize = 1_000_000;

    let relative = validate_task_relative_path(path)?;
    let root = workspace
        .worktree
        .canonicalize()
        .map_err(|error| task_error(format!("cannot resolve Task worktree: {error}")))?;
    let absolute = root
        .join(&relative)
        .canonicalize()
        .map_err(|error| task_error(format!("cannot open Task file {relative:?}: {error}")))?;
    if !absolute.starts_with(&root) || !absolute.is_file() {
        return Err(task_error(format!(
            "Task file {relative:?} does not resolve to a file inside the Task worktree"
        )));
    }
    let bytes = std::fs::read(&absolute)
        .map_err(|error| task_error(format!("cannot read Task file {relative:?}: {error}")))?;
    let size_bytes = bytes.len() as u64;
    let binary = bytes.iter().take(8_192).any(|byte| *byte == 0);
    let truncated = bytes.len() > MAX_FILE_BYTES;
    let visible = &bytes[..bytes.len().min(MAX_FILE_BYTES)];
    let content = (!binary).then(|| String::from_utf8_lossy(visible).into_owned());
    Ok(TaskFileSnapshot {
        issue_identifier: workspace.issue_identifier.to_string(),
        task_id: workspace.task_id.to_string(),
        path: relative,
        content,
        binary,
        size_bytes,
        truncated,
    })
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
        let file = files.entry(path.clone()).or_insert(TaskChangedFile {
            path,
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

fn git_output_owned(worktree: &Path, args: &[String]) -> OpsResult<Vec<u8>> {
    let output = Command::new("git")
        .current_dir(worktree)
        .args(args)
        .output()
        .map_err(|error| task_error(format!("failed to run git diff: {error}")))?;
    if !output.status.success() {
        return Err(task_error(format!(
            "git diff failed: {}",
            String::from_utf8_lossy(&output.stderr).trim()
        )));
    }
    Ok(output.stdout)
}

fn queue_task_steer(issue: &str, message: String) -> OpsResult<TaskControlResult> {
    block_on_task(async move {
        let store = task_store().await?;
        let mut task = store
            .get_task_by_issue(issue)
            .await
            .map_err(|error| task_error(format!("failed to resolve task: {error}")))?
            .ok_or_else(|| task_error(format!("no Task exists for {issue:?}")))?;
        reconcile_task_pr(&store, &mut task).await?;
        reconcile_process_liveness(&store, &mut task).await?;
        let receipt =
            super::child::append_steer(&store, ChildRef::Task(task.id.clone()), &message).await?;
        if !matches!(
            task_work_status(&store, &task).await?,
            WorkStatus::Running { .. }
        ) {
            relaunch_inactive_process(&store, &mut task).await?;
        }
        Ok(TaskControlResult {
            issue_id: task.plan.identifier.clone(),
            task_id: task.id.to_string(),
            receipt: super::child::WorkControlReceipt::Steer { receipt },
            observation: task.observation.clone(),
        })
    })
}

pub fn task_steer(issue: &str, message: String) -> OpsResult<TaskControlResult> {
    queue_task_steer(issue, message)
}

pub fn task_interrupt(issue: &str) -> OpsResult<TaskControlResult> {
    block_on_task(async move {
        let store = task_store().await?;
        let mut task = store
            .get_task_by_issue(issue)
            .await
            .map_err(|error| task_error(format!("failed to resolve task: {error}")))?
            .ok_or_else(|| task_error(format!("no Task exists for {issue:?}")))?;
        reconcile_task_pr(&store, &mut task).await?;
        reconcile_process_liveness(&store, &mut task).await?;
        let work = store
            .work_for_child(&ChildRef::Task(task.id.clone()))
            .await
            .map_err(|error| task_error(error.to_string()))?;
        let run = store
            .current_run(&work)
            .await
            .map_err(|error| task_error(error.to_string()))?
            .ok_or_else(|| task_error("Task has no active Run to interrupt"))?;
        let receipt = store
            .interrupt(None, &work, &run.id)
            .await
            .map_err(|error| task_error(error.to_string()))?;
        Ok(TaskControlResult {
            issue_id: task.plan.identifier.clone(),
            task_id: task.id.to_string(),
            receipt: super::child::WorkControlReceipt::Interrupt { receipt },
            observation: task.observation,
        })
    })
}

/// Recover an abandoned Task as one linked successor that adopts its worktree
/// and serial PR history.
pub fn task_recover(issue: &str, reason: Option<String>) -> OpsResult<Task> {
    let issue = issue.to_string();
    block_on_task(async move {
        let store = task_store().await?;
        _recover_abandoned_task(&store, &issue, reason).await
    })
}

async fn _recover_abandoned_task(
    store: &SharedStore,
    issue: &str,
    reason: Option<String>,
) -> OpsResult<Task> {
    let caller = crate::ops::ambient_run_context(store).await?;
    let reason = reason
        .map(|value| value.trim().to_string())
        .map(|value| {
            if value.is_empty() {
                Err(task_error("recovery reason cannot be empty"))
            } else {
                Ok(value)
            }
        })
        .transpose()?;
    let predecessor = store
        .get_task_by_issue(issue)
        .await
        .map_err(|error| task_error(format!("failed to resolve task: {error}")))?
        .ok_or_else(|| task_error(format!("no Task exists for {issue:?}")))?;
    match task_work_status(store, &predecessor).await? {
        WorkStatus::Done => {
            return Err(task_error(format!(
                "Task {} is completed; start a new Task rather than recovering it",
                predecessor.plan.identifier
            )));
        }
        WorkStatus::Abandoned => {}
        WorkStatus::Ready | WorkStatus::Running { .. } | WorkStatus::Waiting { .. } => {
            return Ok(predecessor)
        }
    }

    // Refuse every unsafe worktree/branch/PR shape before moving ownership.
    task_recovery_adoption(store, &predecessor)
        .await
        .map_err(|error| task_error(format!("validate Task recovery: {error}")))?;
    let mut carried = store
        .boundary_seed_for_child(&ChildRef::Task(predecessor.id.clone()))
        .await
        .map_err(|error| task_error(error.to_string()))?
        .render();
    if carried.is_empty() {
        carried = format!(
            "Continue {}: {}",
            predecessor.plan.identifier, predecessor.plan.title
        );
    }
    let now = time::OffsetDateTime::now_utc();
    let _reason = reason;
    let mut task = predecessor;
    task.lifecycle_phase = crate::task::TaskLifecyclePhase::First;
    task.phase_epoch += 1;
    task.phase_cursor = 0;
    task.phase_iteration = 0;
    task.gate_cycle = 0;
    task.gate_proposal = None;
    task.provider_session_id = None;
    task.abandon_intent = None;
    task.updated_at = now;
    task.observation = Observation::NotRequired;
    match caller.as_ref() {
        Some(caller) => store.reopen_task(Some(caller), &task, None, &carried).await,
        None => store.reopen_task(None, &task, None, &carried).await,
    }
    .map_err(|error| task_error(format!("failed to recover Task: {error}")))?;
    Ok(task)
}

pub fn task_resume(
    issue: &str,
    model: Option<String>,
    reason: Option<String>,
) -> OpsResult<TaskControlResult> {
    let issue = issue.to_string();
    block_on_task(async move { resume_task_async(&issue, model, reason).await })
}

/// Async core of [`task_resume`], reusable from callers already inside a runtime.
pub(crate) async fn resume_task_async(
    issue: &str,
    model: Option<String>,
    reason: Option<String>,
) -> OpsResult<TaskControlResult> {
    let store = task_store().await?;
    let mut task = store
        .get_task_by_issue(issue)
        .await
        .map_err(|error| task_error(format!("failed to resolve task: {error}")))?
        .ok_or_else(|| task_error(format!("no Task exists for {issue:?}")))?;
    validate_task_launch(&task)?;
    let latest_event = store
        .latest_task_event(&task.id)
        .await
        .map_err(|error| task_error(format!("failed to read Task blocker: {error}")))?;
    if let Some(blocker) = task_event_launch_refusal(latest_event.as_ref()) {
        let intervention = reason
            .as_deref()
            .is_some_and(|reason| !reason.trim().is_empty());
        if !intervention {
            return Err(task_error(format!(
                "{blocker}\nThe same execution cannot be resumed. Correct the capability, then use `lf task resume {} --reason \"<what changed>\"` so the new durable input starts a fresh boundary.",
                task.plan.identifier
            )));
        }
    }
    // Compute every branch/worktree/PR adoption precondition before moving any
    // durable ownership — a no-active-PR recovery must not commit the successor
    // before PR rotation rejects an unrelated branch.
    task_recovery_adoption(&store, &task).await?;
    reconcile_task_pr(&store, &mut task).await?;
    let prs = store
        .task_prs(&task.id)
        .await
        .map_err(|error| task_error(format!("failed to read Task PRs: {error}")))?;
    let latest = prs.last();
    let active = prs.iter().find(|pr| pr.is_active());
    if let Some(refusal) = no_active_pr_resume_refusal(&task.plan.identifier, active, latest) {
        return Err(task_error(refusal));
    }
    {
        let _mutation = lock_task_pr_mutation(&task.worktree)?;
        clear_task_pr_merge(&store, &task, None, &task.worktree, true).await?;
    }
    // Reconcile may settle an active PR that merged out of band, moving the
    // worktree into a between-PR state; refuse a dirty between-PR before the
    // lease is reaped or a successor body is launched.
    refuse_dirty_between_prs(&store, &task).await?;
    reconcile_process_liveness(&store, &mut task).await?;
    let issue_id = task.plan.identifier.clone();
    let observation = task.observation.clone();
    let task_id = task.id.to_string();
    let run = super::child::resume_child(
        &store,
        super::child::Child::Task(Box::new(task)),
        model,
        reason,
    )
    .await?;
    Ok(TaskControlResult {
        issue_id,
        task_id,
        receipt: super::child::WorkControlReceipt::Resume { run },
        observation,
    })
}

pub fn task_abandon(issue: &str, reason: String) -> OpsResult<TaskControlResult> {
    let reason = reason.trim();
    if reason.is_empty() {
        return Err(task_error("`lf task abandon --reason` cannot be empty"));
    }
    let reason = reason.to_string();
    block_on_task(async move {
        let store = task_store().await?;
        let mut task = store
            .get_task_by_issue(issue)
            .await
            .map_err(|error| task_error(format!("failed to resolve task: {error}")))?
            .ok_or_else(|| task_error(format!("no Task exists for {issue:?}")))?;
        reconcile_task_pr(&store, &mut task).await?;
        reconcile_process_liveness(&store, &mut task).await?;
        let work = store
            .work_for_child(&ChildRef::Task(task.id.clone()))
            .await
            .map_err(|error| task_error(error.to_string()))?;
        let basis = store
            .current_epoch(&work)
            .await
            .map_err(|error| task_error(error.to_string()))?
            .current_basis;
        let receipt = store
            .abandon(&work, &reason, &basis)
            .await
            .map_err(|error| task_error(error.to_string()))?;
        Ok(TaskControlResult {
            issue_id: task.plan.identifier.clone(),
            task_id: task.id.to_string(),
            receipt: super::child::WorkControlReceipt::Abandon { receipt },
            observation: task.observation,
        })
    })
}

pub fn task_wait(issue: &str, until: TaskWaitUntil, timeout: Option<Duration>) -> OpsResult<Task> {
    let started = Instant::now();
    loop {
        let task = task_status(issue)?;
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
        apply_merged_task_landing, ensure_task_flow_override, ensure_working_pr,
        launch_task_process, lock_task_pr_mutation, mark_task_body_lost, preflight_task_execution,
        prepare_automatic_task_relaunch, probe_task_execution_boundary, propose_task_done,
        reconcile_process_liveness, resolve_task_lifecycle, resolve_task_start_input,
        task_event_launch_refusal, task_execution_boundary, validate_task_lifecycle,
        TaskFlowOverrides, MAX_AUTOMATIC_TASK_RECOVERY_RUNS,
    };
    use crate::child::ChildRef;
    use crate::durable::{RunTrigger, WorkRef, WorkStatus};
    use crate::engine::AgentExecutionBoundary;
    use crate::planning::{LinearIssueId, LinearProjectId, ProjectPlan, TaskPlan};
    use crate::pm::ProjectFlowPlan;
    use crate::project::{Project, ProjectId};
    use crate::store::{open_store, SharedStore, StorageConfig};
    use crate::task::{
        AfterMerge, GithubPr, Observation, PmWritebackState, PrMergeMode, PrMergeRequest,
        PrPresentation, PrPublication, Task, TaskEventKind, TaskId, TaskLifecyclePhase,
        TaskLifecyclePlan, TaskPr, TaskPrId,
    };
    use crate::wave::Wave;
    use std::ffi::OsString;

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

    async fn task_fixture(identifier: &str, loop_flow: &str) -> TaskFixture {
        let repository =
            std::fs::canonicalize(std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../.."))
                .unwrap();
        let database = tempfile::tempdir().unwrap();
        let database_path = database.path().join("registry.db");
        let store = std::sync::Arc::new(
            open_store(&StorageConfig::sqlite(database_path.clone()))
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
                id: LinearProjectId::new("task-recovery-project").unwrap(),
                slug: "task-recovery".to_string(),
                name: "Task recovery".to_string(),
                prompt_context: "Keep automatic Task recovery bounded.".to_string(),
                pm_snapshot_synced_at: now.unix_timestamp(),
            },
            wave_id: wave.id().clone(),
            iteration: 0,
            observation_cursor: 0,
            last_state_fingerprint: None,
            agent: "codex".to_string(),
            provider: "codex".to_string(),
            provider_session_id: None,
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
            lifecycle: TaskLifecyclePlan::standard("task-design", loop_flow, "ship"),
            lifecycle_phase: TaskLifecyclePhase::Loop,
            phase_epoch: 4,
            phase_cursor: 0,
            phase_iteration: 0,
            gate_cycle: 1,
            gate_proposal: None,
            agent: "codex".to_string(),
            provider: "codex".to_string(),
            provider_session_id: None,
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

    #[tokio::test]
    async fn watched_landing_completes_task_only_from_merged_pr_evidence() {
        let TaskFixture {
            _database,
            store,
            mut task,
            work,
            database_path,
            ..
        } = task_fixture("LOO-248", "slice").await;
        let now = time::OffsetDateTime::now_utc();
        task.lifecycle_phase = TaskLifecyclePhase::Finally;
        propose_task_done(&mut task, "watched pull request merged".to_string()).unwrap();
        store.update_task(&task).await.unwrap();

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
    fn piped_report_supplies_title_and_preserves_full_description() {
        let report = "\n  lf status rejects stored timestamp  \n\nstack trace\nmore evidence\n";
        let input = resolve_task_start_input(None, Some(report)).expect("resolve piped report");

        assert_eq!(input.title, "lf status rejects stored timestamp");
        assert_eq!(
            input.report,
            "lf status rejects stored timestamp  \n\nstack trace\nmore evidence"
        );
    }

    #[test]
    fn project_flows_resolve_once_with_per_task_overrides() {
        let repo = tempfile::tempdir().expect("temp repo");
        let project = ProjectFlowPlan {
            first: Some("incident".to_string()),
            loop_: Some("ship-5whys".to_string()),
            finally: Some("ship".to_string()),
        };
        let overrides = TaskFlowOverrides {
            loop_: Some("slice".to_string()),
            ..TaskFlowOverrides::default()
        };

        let plan =
            resolve_task_lifecycle(repo.path(), &project, &overrides).expect("resolve lifecycle");

        assert_eq!(plan.first.flow, "incident");
        assert_eq!(plan.loop_.flow, "slice");
        assert_eq!(plan.finally.flow, "ship");
    }

    #[test]
    fn task_lifecycle_rejects_structural_dead_ends() {
        let repo = tempfile::tempdir().expect("temp repo");
        let project = ProjectFlowPlan {
            first: Some("task-kickoff".to_string()),
            loop_: Some("design".to_string()),
            finally: Some("task-gate".to_string()),
        };

        let error = resolve_task_lifecycle(repo.path(), &project, &TaskFlowOverrides::default())
            .expect_err("INT-10 lifecycle must not launch");
        let message = error.to_string();

        assert!(message.contains(
            "loop phase flow \"design\" from Linear Project `## Flows` `loop` configuration is human-only"
        ));
        assert!(message.contains("has no autonomous skill step"));
        assert!(message.contains(
            "finally phase flow \"task-gate\" from Linear Project `## Flows` `finally` configuration"
        ));
        assert!(message.contains("does not end with `op: pr land -c`"));
        assert!(message.contains("`op: pr land -c`"));
    }

    #[test]
    fn repo_local_task_flow_needs_no_capability_frontmatter() {
        let repo = tempfile::tempdir().expect("temp repo");
        let skills = repo.path().join(".lf/skills");
        let flows = repo.path().join(".lf/flows");
        std::fs::create_dir_all(&skills).expect("create skill directory");
        std::fs::create_dir_all(&flows).expect("create flow directory");
        std::fs::write(
            skills.join("write-artifact.md"),
            "Produce the Task artifact and prove it works.\n",
        )
        .expect("write repo-local skill");
        std::fs::write(flows.join("custom-loop.yaml"), "- write-artifact\n")
            .expect("write repo-local flow");
        let overrides = TaskFlowOverrides {
            loop_: Some("custom-loop".to_string()),
            ..TaskFlowOverrides::default()
        };

        let plan = resolve_task_lifecycle(repo.path(), &ProjectFlowPlan::empty(), &overrides)
            .expect("structurally valid repo-local lifecycle");

        assert_eq!(plan.loop_.flow, "custom-loop");
        assert_eq!(plan.finally.flow, "ship-demo");
    }

    #[test]
    fn default_and_feature_tasks_gate_at_design_and_demo() {
        let repo = tempfile::tempdir().expect("temp repo");
        let defaults = resolve_task_lifecycle(
            repo.path(),
            &ProjectFlowPlan::empty(),
            &TaskFlowOverrides::default(),
        )
        .expect("resolve default lifecycle");
        let default_human = [
            &defaults.first.flow,
            &defaults.loop_.flow,
            &defaults.finally.flow,
        ]
        .into_iter()
        .flat_map(|flow| {
            let flow = crate::engine::load_flow(flow, repo.path()).unwrap();
            crate::engine::expand_flow(&flow, repo.path()).unwrap()
        })
        .filter(
            |step| matches!(step, crate::engine::ConcreteStep::Skill(skill) if skill.policy.human),
        )
        .collect::<Vec<_>>();
        assert_eq!(default_human.len(), 2);
        assert_eq!(
            default_human
                .iter()
                .filter_map(|step| match step {
                    crate::engine::ConcreteStep::Skill(skill) => skill.policy.id.as_deref(),
                    _ => None,
                })
                .collect::<Vec<_>>(),
            ["review_kickoff", "review_demo"]
        );

        let feature = resolve_task_lifecycle(
            repo.path(),
            &ProjectFlowPlan::empty(),
            &TaskFlowOverrides::for_cycle(Some(super::TaskCycle::Feature), None, None, None),
        )
        .expect("resolve feature lifecycle");
        assert_eq!(feature.first.flow, "task-design");
        assert_eq!(feature.finally.flow, "ship-demo");
        assert_eq!([
            &feature.first.flow,
            &feature.loop_.flow,
            &feature.finally.flow
        ]
        .into_iter()
        .flat_map(|flow| {
            let flow = crate::engine::load_flow(flow, repo.path()).unwrap();
            crate::engine::expand_flow(&flow, repo.path()).unwrap()
        })
        .filter(|step| matches!(step, crate::engine::ConcreteStep::Skill(skill) if skill.policy.human))
        .count(), 2);
    }

    #[test]
    fn fix_cycle_moves_the_only_human_gate_to_the_demo() {
        let repo = tempfile::tempdir().expect("temp repo");
        let plan = resolve_task_lifecycle(
            repo.path(),
            &ProjectFlowPlan::empty(),
            &TaskFlowOverrides::for_cycle(Some(super::TaskCycle::Fix), None, None, None),
        )
        .expect("resolve fix lifecycle");
        assert_eq!(plan.first.flow, "incident");
        assert_eq!(plan.loop_.flow, "slice");
        assert_eq!(plan.finally.flow, "ship-demo");

        let human = [&plan.first.flow, &plan.loop_.flow, &plan.finally.flow]
            .into_iter()
            .flat_map(|flow| {
                let flow = crate::engine::load_flow(flow, repo.path()).unwrap();
                crate::engine::expand_flow(&flow, repo.path()).unwrap()
            })
            .filter(
                |step| matches!(step, crate::engine::ConcreteStep::Skill(skill) if skill.policy.human),
            )
            .collect::<Vec<_>>();
        assert_eq!(human.len(), 1);
        assert!(matches!(
            &human[0],
            crate::engine::ConcreteStep::Skill(skill)
                if skill.policy.id.as_deref() == Some("review_demo")
        ));

        let explicit_wins = TaskFlowOverrides::for_cycle(
            Some(super::TaskCycle::Fix),
            Some("task-design".to_string()),
            None,
            None,
        );
        assert_eq!(explicit_wins.first.as_deref(), Some("task-design"));
        assert_eq!(explicit_wins.finally.as_deref(), Some("ship-demo"));
    }

    #[test]
    fn finally_flow_rejects_ops_before_agent_work() {
        let repo = tempfile::tempdir().expect("temp repo");
        let flows = repo.path().join(".lf/flows");
        std::fs::create_dir_all(&flows).expect("create flow directory");
        std::fs::write(
            flows.join("unsafe-finally.yaml"),
            "- op: pr land -c\n- gate\n",
        )
        .expect("write flow");
        let project = ProjectFlowPlan {
            first: None,
            loop_: None,
            finally: Some("unsafe-finally".to_string()),
        };

        let error = resolve_task_lifecycle(repo.path(), &project, &TaskFlowOverrides::default())
            .expect_err("reject unsafe finally flow");

        assert!(error
            .to_string()
            .contains("one or more skills followed by optional ops"));
    }

    #[test]
    fn task_lifecycle_settlement_must_be_the_terminal_finally_step() {
        let repo = tempfile::tempdir().expect("temp repo");
        let flows = repo.path().join(".lf/flows");
        std::fs::create_dir_all(&flows).expect("create flow directory");
        std::fs::write(
            flows.join("parks-after-land.yaml"),
            "- gate\n- op: pr land -c\n- op: pr publish --title stale\n",
        )
        .expect("write flow");
        let project = ProjectFlowPlan {
            first: None,
            loop_: None,
            finally: Some("parks-after-land".to_string()),
        };

        let error = resolve_task_lifecycle(repo.path(), &project, &TaskFlowOverrides::default())
            .expect_err("settlement followed by more work cannot terminate a Task");

        assert!(error
            .to_string()
            .contains("does not end with `op: pr land -c`"));
    }

    #[test]
    fn execution_boundary_probes_the_actual_required_root() {
        let directory = tempfile::tempdir().unwrap();
        let file = directory.path().join("not-a-directory");
        std::fs::write(&file, "occupied").unwrap();

        let error = probe_task_execution_boundary(&AgentExecutionBoundary {
            writable_roots: vec![file.clone()],
        })
        .expect_err("a descriptive root that cannot accept a file is not a capability");

        assert!(error.to_string().contains(&file.display().to_string()));
        assert!(error.to_string().contains("required writable authority"));
    }

    #[test]
    fn execution_boundary_resolves_linked_git_and_control_roots() {
        let _env_lock = crate::journal::test_env_lock();
        let _restore = EnvRestore::capture(&[
            "LF_BIN",
            "LF_HOME",
            "LF_DB_PATH",
            "LF_CONTROL_HOME",
            "LF_CONTROL_DB_PATH",
        ]);
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
        let database = control.join("registry.db");
        std::env::set_var("LF_BIN", std::env::current_exe().unwrap());
        std::env::set_var("LF_HOME", &control);
        std::env::set_var("LF_DB_PATH", &database);
        std::env::set_var("LF_CONTROL_HOME", &control);
        std::env::set_var("LF_CONTROL_DB_PATH", &database);

        let boundary = task_execution_boundary(&worktree, "codex").unwrap();

        assert_eq!(boundary.writable_roots.len(), 2);
        assert!(boundary
            .writable_roots
            .contains(&main.join(".git").canonicalize().unwrap()));
        assert!(boundary.writable_roots.contains(&control));
    }

    #[cfg(unix)]
    #[tokio::test]
    #[allow(clippy::await_holding_lock)] // the env lock is the test serializer
    async fn task_preflight_refuses_account_id_null_before_reserving_a_run() {
        let _env_lock = crate::journal::test_env_lock();
        let _restore = EnvRestore::capture(&[
            "LF_BIN",
            "LF_HOME",
            "LF_DB_PATH",
            "LF_CONTROL_HOME",
            "LF_CONTROL_DB_PATH",
            "LF_ACCOUNT_LEASE",
        ]);
        let directory = tempfile::tempdir().unwrap();
        let repo = directory.path().join("repo");
        std::fs::create_dir(&repo).unwrap();
        let output = std::process::Command::new("git")
            .current_dir(&repo)
            .args(["init", "-b", "main"])
            .output()
            .unwrap();
        assert!(output.status.success());
        let home = directory.path().join("lf-home");
        std::fs::create_dir(&home).unwrap();
        let database = home.join("missing.db");
        std::env::set_var("LF_BIN", std::env::current_exe().unwrap());
        std::env::set_var("LF_HOME", &home);
        std::env::set_var("LF_DB_PATH", &database);
        std::env::set_var("LF_CONTROL_HOME", &home);
        std::env::set_var("LF_CONTROL_DB_PATH", &database);
        std::env::remove_var("LF_ACCOUNT_LEASE");

        let error = preflight_task_execution(&repo, "codex")
            .await
            .expect_err("headless Task launch requires an explicit account route");

        assert!(error.to_string().contains("account_id=null"));
        assert!(!database.exists(), "preflight must not create a registry");
    }

    #[test]
    fn started_task_rejects_a_different_flow_override() {
        let repo = tempfile::tempdir().expect("temp repo");

        ensure_task_flow_override(
            repo.path(),
            "INF-123",
            TaskLifecyclePhase::Loop,
            Some("slice"),
            "slice",
        )
        .expect("same pinned flow is idempotent");
        let error = ensure_task_flow_override(
            repo.path(),
            "INF-123",
            TaskLifecyclePhase::Loop,
            Some("ship-5whys"),
            "slice",
        )
        .expect_err("different flow must be rejected");

        assert!(error
            .to_string()
            .contains("Task INF-123 already pins loop flow \"slice\""));
    }

    #[tokio::test]
    async fn unavailable_persisted_flow_is_rejected_before_every_run_reservation() {
        let TaskFixture {
            store,
            mut task,
            work,
            ..
        } = task_fixture("TEST-STALE", "retired-task-flow").await;

        for _ in 0..2 {
            let error = launch_task_process(&store, &mut task, None)
                .await
                .expect_err("the unavailable flow must fail startup validation");
            assert!(error.to_string().contains(
                "Task TEST-STALE cannot launch: pinned loop flow \"retired-task-flow\" is invalid: failed to load Task flow \"retired-task-flow\": flow not found: retired-task-flow"
            ));
            assert!(store.current_run(&work).await.unwrap().is_none());
        }
        assert!(store
            .recent_task_events(&task.id, 10)
            .await
            .unwrap()
            .is_empty());
    }

    #[tokio::test]
    async fn lost_body_parks_an_invalid_lifecycle_without_relaunching() {
        let TaskFixture {
            store,
            mut task,
            work,
            ..
        } = task_fixture("TEST-STALE-BODY", "retired-task-flow").await;

        mark_task_body_lost(&store, &mut task).await.unwrap();

        let event = store.latest_task_event(&task.id).await.unwrap().unwrap();
        assert!(matches!(
            event.kind,
            TaskEventKind::Failed {
                error,
                resumable: true,
            } if error.contains("retired-task-flow")
        ));
        assert!(store.current_run(&work).await.unwrap().is_none());
        assert_eq!(store.task_prs(&task.id).await.unwrap().len(), 1);
    }

    #[tokio::test]
    async fn invalid_persisted_lifecycle_cannot_rotate_a_serial_pr() {
        let TaskFixture {
            store, mut task, ..
        } = task_fixture("INT-10", "slice").await;
        task.lifecycle = TaskLifecyclePlan::standard("task-kickoff", "design", "task-gate");
        store.update_task(&task).await.unwrap();
        let mut pr = store.active_task_pr(&task.id).await.unwrap().unwrap();
        pr.abandoned_at = Some(time::OffsetDateTime::now_utc());
        store.settle_task_pr(&pr, None).await.unwrap();

        let error = ensure_working_pr(&store, &mut task)
            .await
            .expect_err("invalid lifecycle must stop before rotation");

        assert!(error
            .to_string()
            .contains("Task INT-10 cannot launch: Task lifecycle cannot converge"));
        assert!(error.to_string().contains(
            "Persisted Task lifecycle selection is immutable; abandon INT-10 and start a replacement Task with the corrected flows"
        ));
        assert_eq!(store.task_prs(&task.id).await.unwrap().len(), 1);
        assert!(store.active_task_pr(&task.id).await.unwrap().is_none());
    }

    #[tokio::test]
    async fn repaired_persisted_task_flows_validate_through_the_generic_path() {
        for identifier in ["LOO-167", "LOO-193", "LOO-195"] {
            let TaskFixture {
                _database,
                database_path,
                store,
                task,
                ..
            } = task_fixture(identifier, "task").await;
            rusqlite::Connection::open(database_path)
                .unwrap()
                .execute_batch(&crate::store::migrations::migration_sql_for_test(
                    "repair_legacy_task_flow",
                ))
                .unwrap();
            let repaired = store.get_task(&task.id).await.unwrap().unwrap();

            assert_eq!(repaired.lifecycle.loop_.flow, "slice");
            validate_task_lifecycle(&repaired)
                .expect("a repaired Task satisfies the generic lifecycle contract");
        }
    }

    #[tokio::test]
    async fn automatic_task_relaunch_exhausts_until_durable_progress_or_user_input() {
        let TaskFixture {
            store, task, work, ..
        } = task_fixture("TEST-RECOVERY", "slice").await;
        let basis = store.current_epoch(&work).await.unwrap().current_basis;
        let (initial, initial_lease) = store
            .reserve_run(
                &work,
                RunTrigger::Input {
                    basis: basis.clone(),
                },
            )
            .await
            .unwrap();
        store
            .append_task_event_for_run(&task.id, &initial_lease, &TaskEventKind::Started)
            .await
            .unwrap();
        store
            .fail_task_run(&task.id, &initial_lease, "always fails", true)
            .await
            .unwrap();

        let mut prior_run_id = initial.id;
        for recovery in 1..=MAX_AUTOMATIC_TASK_RECOVERY_RUNS {
            let trigger = prepare_automatic_task_relaunch(&store, &task)
                .await
                .unwrap()
                .expect("automatic recovery remains within its budget");
            assert_eq!(
                trigger,
                RunTrigger::Recovery {
                    prior_run_id: prior_run_id.clone()
                }
            );
            let (run, lease) = store.reserve_run(&work, trigger).await.unwrap();
            assert_eq!(run.retry_of, Some(prior_run_id));
            store
                .append_task_event_for_run(&task.id, &lease, &TaskEventKind::Started)
                .await
                .unwrap();
            store
                .fail_task_run(
                    &task.id,
                    &lease,
                    &format!("automatic recovery {recovery} failed"),
                    true,
                )
                .await
                .unwrap();
            prior_run_id = run.id;
        }

        assert!(prepare_automatic_task_relaunch(&store, &task)
            .await
            .unwrap()
            .is_none());
        assert!(matches!(
            store.work_status(&work).await.unwrap(),
            WorkStatus::Waiting { .. }
        ));
        let parked_run = store.latest_run(&work).await.unwrap().unwrap();
        for _ in 0..10 {
            assert!(prepare_automatic_task_relaunch(&store, &task)
                .await
                .unwrap()
                .is_none());
            assert_eq!(
                store.latest_run(&work).await.unwrap().unwrap().id,
                parked_run.id
            );
        }

        let (user_run, user_lease) = store
            .reserve_run(&work, RunTrigger::User)
            .await
            .expect("explicit User input clears the exhausted wait");
        store
            .append_task_event_for_run(&task.id, &user_lease, &TaskEventKind::Started)
            .await
            .unwrap();
        store
            .fail_task_run(&task.id, &user_lease, "User-started Run failed", true)
            .await
            .unwrap();
        let trigger = prepare_automatic_task_relaunch(&store, &task)
            .await
            .unwrap()
            .expect("User input starts a fresh recovery budget");
        assert_eq!(
            trigger,
            RunTrigger::Recovery {
                prior_run_id: user_run.id
            }
        );

        let (_, progress_lease) = store.reserve_run(&work, trigger).await.unwrap();
        store
            .append_task_event_for_run(&task.id, &progress_lease, &TaskEventKind::Started)
            .await
            .unwrap();
        store
            .append_task_event_for_run(
                &task.id,
                &progress_lease,
                &TaskEventKind::Progress {
                    summary: "durable progress".to_string(),
                },
            )
            .await
            .unwrap();
        store
            .fail_task_run(
                &task.id,
                &progress_lease,
                "failed after durable progress",
                true,
            )
            .await
            .unwrap();
        assert_eq!(
            prepare_automatic_task_relaunch(&store, &task)
                .await
                .unwrap(),
            Some(RunTrigger::Input { basis })
        );
    }

    #[tokio::test]
    async fn nonresumable_execution_blocker_never_enters_automatic_recovery() {
        let TaskFixture {
            store,
            mut task,
            work,
            ..
        } = task_fixture("TEST-BOUNDARY", "slice").await;
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

        for _ in 0..10 {
            assert!(prepare_automatic_task_relaunch(&store, &task)
                .await
                .unwrap()
                .is_none());
            assert!(store.latest_run(&work).await.unwrap().is_none());
        }
        reconcile_process_liveness(&store, &mut task).await.unwrap();
        let event = store.latest_task_event(&task.id).await.unwrap().unwrap();
        assert_eq!(event, settled);
        assert_eq!(task_event_launch_refusal(Some(&event)), Some(blocker));
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
