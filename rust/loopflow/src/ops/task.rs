use std::collections::{BTreeMap, BTreeSet};
use std::fs::{File, OpenOptions};
use std::path::{Component, Path, PathBuf};
use std::process::Command;
use std::sync::Arc;
use std::time::{Duration, Instant};

use crate::child::ChildRef;
use crate::durable::{
    AuthenticatedRequest, Containment, ContainmentObservation, ControlCtx, Launch, RunLease,
    RunState, WorkRef, WorkStatus,
};
use crate::engine::config::{load_config_or_default, parse_agent};
use crate::engine::git::{
    checkout, checkout_new_branch_from, cherry_pick_range, current_branch, delete_local_branch,
    fetch, get_default_branch, is_ancestor, is_clean, merge_base, push_with_upstream, ref_exists,
    rev_parse, stash_including_untracked, stash_pop,
};
use crate::engine::naming::sanitize_for_branch;
use crate::engine::process::{tmux_session_exists, tmux_session_slug};
use crate::engine::worktrees::{
    create_from_placement_plan, plan_placement, PlacementStrategy, WorktreeSegment,
};
use crate::engine::{expand_flow, load_flow, ConcreteStep};
use crate::ops::error::{OpsError, OpsResult};
use crate::planning::{LinearIssueId, TaskPlan};
use crate::store::{
    open_existing_store, open_registry_for_authority, RegistryUnavailable, SharedStore, Store,
    StoreError,
};
use crate::task::actions::{derive_task_actions, TaskActionEvidence, TaskActionModel};
use crate::task::{
    AfterMerge, CiCheck, CiIncident, CiObservation, CiState, FeedbackReviewer, GithubObservation,
    GithubObservationResult, GithubPr, Observation, PmWritebackOperation, PmWritebackState,
    PrMergeMode, PrMergeRequest, PrPhase, PrPublication, Task, TaskEventKind, TaskId, TaskPr,
    TaskPrId,
};
use crate::wave::Wave;
use fs2::FileExt;
use sha2::{Digest, Sha256};

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

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct TaskLaunchOptions {
    pub name: Option<String>,
    pub flows: TaskFlowOverrides,
    pub stack_on: Option<String>,
    pub directive: Option<String>,
    pub reviewer: Option<FeedbackReviewer>,
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
    pub process_alive: bool,
    pub launch: Option<Launch>,
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

fn _set_task_reviewer(task: &mut Task, reviewer: FeedbackReviewer) -> bool {
    if task.lifecycle.all_reviewed_by(reviewer) {
        return false;
    }
    task.lifecycle.set_reviewer(reviewer);
    task.updated_at = time::OffsetDateTime::now_utc();
    true
}

pub fn task_run(repo: &Path, issue: &str, options: TaskLaunchOptions) -> OpsResult<Task> {
    let TaskLaunchOptions {
        name,
        flows,
        stack_on,
        directive,
        reviewer,
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
    let (existing, terminal_predecessor_id) = block_on_task(async {
        let store = task_store().await?;
        let mut existing = store
            .get_task_by_issue(issue)
            .await
            .map_err(|error| task_error(format!("failed to read task registry: {error}")))?;
        if let Some(task) = &mut existing {
            let status = task_work_status(&store, task).await?;
            if matches!(status, WorkStatus::Done | WorkStatus::Abandoned) {
                // A terminal Task leaves its direction to a successor
                // created below; do not return its status here. The placement
                // path re-resolves the issue, derives a distinct worktree slug
                // from this predecessor's id, and carries the cursor and comment
                // ledger onto the reopened Task Work.
                let predecessor_id = task.id.clone();
                return Ok((None, Some(predecessor_id)));
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
                "first",
                flows.first.as_deref(),
                &task.lifecycle.first.flow,
            )?;
            ensure_task_flow_override(
                &task.worktree,
                &task.plan.identifier,
                "loop",
                flows.loop_.as_deref(),
                &task.lifecycle.loop_.flow,
            )?;
            ensure_task_flow_override(
                &task.worktree,
                &task.plan.identifier,
                "finally",
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
            if reviewer.is_some_and(|reviewer| _set_task_reviewer(task, reviewer)) {
                store
                    .update_task(task)
                    .await
                    .map_err(|error| task_error(error.to_string()))?;
            }
        }
        Ok((existing, None))
    })?;
    if let Some(existing) = existing {
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
    let lifecycle = resolve_task_lifecycle(&main_repo, &project_flows, &flows, reviewer)?;
    let segment = match name.as_deref() {
        Some(name) => parse_workspace_slug(name)?,
        None => match &terminal_predecessor_id {
            // A terminal predecessor may still occupy its worktree and branch on
            // disk (e.g. after `lf task complete`), so the successor places a
            // fresh worktree under a slug derived from the predecessor's id.
            Some(predecessor_id) => succession_workspace_slug(&resolved.item.name, predecessor_id)?,
            None => derive_workspace_slug(&resolved.item.name)?,
        },
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
        // the Task in the gap. Non-terminal Work wins — return it. A
        // terminal one is the predecessor whose direction the successor carries;
        // None means this is the first Task for the issue.
        let predecessor = match store
            .get_task_by_issue(&resolved.item.id)
            .await
            .map_err(|error| task_error(format!("failed to read task registry: {error}")))?
        {
            Some(existing)
                if !matches!(
                    task_work_status(&store, &existing).await?,
                    WorkStatus::Done | WorkStatus::Abandoned
                ) =>
            {
                return Ok(existing)
            }
            Some(terminal) => Some(terminal),
            None => None,
        };
        let now = time::OffsetDateTime::now_utc();
        let task_id = predecessor
            .as_ref()
            .map(|task| task.id.clone())
            .unwrap_or_else(crate::task::TaskId::new);
        let sequence = if let Some(predecessor) = &predecessor {
            store
                .task_prs(&predecessor.id)
                .await
                .map_err(|error| task_error(format!("failed to read Task PR history: {error}")))?
                .last()
                .map_or(1, |pr| pr.sequence + 1)
        } else {
            1
        };
        let mut task = Task {
            id: task_id,
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
            created_at: predecessor.as_ref().map_or(now, |task| task.created_at),
            updated_at: now,
            observation: crate::task::Observation::NotRequired,
        };
        let pr = TaskPr {
            id: TaskPrId::new(),
            task_id: task.id.clone(),
            sequence,
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

        if predecessor.is_some() {
            store
                .reopen_task(&task, Some(&pr), crate::durable::Author::User, &directive)
                .await
                .map_err(|error| task_error(format!("failed to reopen Task: {error}")))?;
        } else {
            match store
                .create_task_with_steer(&task, &pr, crate::durable::Author::User, &directive)
                .await
            {
                Ok(()) => {}
                Err(StoreError::Sqlite(_)) => {
                    if let Some(existing) = store
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
            }
        }

        store
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
            .map_err(|error| task_error(error.to_string()))?;

        if let Err(error) = create_from_placement_plan(&main_repo, &plan) {
            record_task_failure(
                &store,
                &mut task,
                format!("worktree creation failed: {error}"),
                error.to_string(),
            )
            .await?;
            return Err(task_error(format!(
                "failed to create task worktree: {error}"
            )));
        }

        launch_task_process(&store, &mut task, None).await?;
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
    crate::ops::project::require_registered_wave(&project.snapshot.wave)
        .map_err(|error| task_error(error.to_string()))?;
    let project_flows = project
        .project
        .flows
        .clone()
        .unwrap_or_else(crate::pm::ProjectFlowPlan::empty);
    resolve_task_lifecycle(&main, &project_flows, &options.flows, options.reviewer)?;
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
    phase: &str,
    requested: Option<&str>,
    pinned: &str,
) -> OpsResult<()> {
    let Some(requested) = requested else {
        return Ok(());
    };
    let requested = resolve_task_flow(repo, requested, phase == "finally")?;
    if requested != pinned {
        return Err(task_error(format!(
            "Task {} already pins {phase} flow {:?}",
            issue_identifier, pinned
        )));
    }
    Ok(())
}

fn resolve_task_lifecycle(
    repo: &Path,
    project: &crate::pm::ProjectFlowPlan,
    overrides: &TaskFlowOverrides,
    reviewer: Option<FeedbackReviewer>,
) -> OpsResult<crate::task::TaskLifecyclePlan> {
    let first = overrides
        .first
        .as_deref()
        .or(project.first.as_deref())
        .unwrap_or("task-design");
    let loop_flow = overrides
        .loop_
        .as_deref()
        .or(project.loop_.as_deref())
        .unwrap_or("slice");
    let finally = overrides
        .finally
        .as_deref()
        .or(project.finally.as_deref())
        .unwrap_or("ship");
    let first = resolve_task_flow(repo, first, false)?;
    let loop_flow = resolve_task_flow(repo, loop_flow, false)?;
    let finally = resolve_task_flow(repo, finally, true)?;
    let mut lifecycle = crate::task::TaskLifecyclePlan::standard(first, loop_flow, finally);
    if let Some(reviewer) = reviewer {
        lifecycle.set_reviewer(reviewer);
    }
    Ok(lifecycle)
}

fn resolve_task_flow(repo: &Path, requested: &str, allow_ops: bool) -> OpsResult<String> {
    let definition = load_flow(requested, repo)
        .map_err(|error| task_error(format!("failed to load Task flow {requested:?}: {error}")))?;
    let steps = expand_flow(&definition, repo).map_err(|error| {
        task_error(format!("failed to expand Task flow {requested:?}: {error}"))
    })?;
    if steps.is_empty() {
        return Err(task_error(format!("Task flow {requested:?} has no steps")));
    }
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
    Ok(definition.name)
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

/// A successor's worktree and branch must differ from its terminal
/// predecessor's, which may still occupy its checkout and branch on disk (for
/// example after `lf task complete`). Append a short, per-predecessor suffix
/// derived from the predecessor's id so the successor places a fresh worktree
/// without colliding. The base is capped at four words so the suffix word keeps
/// the segment within the 2-5 word limit.
fn succession_workspace_slug(title: &str, predecessor_id: &TaskId) -> OpsResult<WorktreeSegment> {
    let base = derive_workspace_slug_with_cap(title, 4)?;
    let id = predecessor_id.as_str();
    let tail = &id[id.len().saturating_sub(8)..];
    parse_workspace_slug(&format!("{}-s{}", base.as_str(), tail))
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
    lease: Option<&RunLease>,
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
    lease: Option<&RunLease>,
) -> Result<(), StoreError> {
    match lease {
        Some(lease) => store.settle_task_pr_for_run(settled, next, lease).await,
        None => store.settle_task_pr(settled, next).await,
    }
}

async fn append_task_event_with_authority(
    store: &SharedStore,
    task_id: &crate::task::TaskId,
    event: &TaskEventKind,
    lease: Option<&RunLease>,
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
    lease: Option<&RunLease>,
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
    lease: Option<&RunLease>,
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
    lease: Option<&RunLease>,
) -> Result<(), StoreError> {
    match lease {
        Some(lease) => store.complete_task_for_run(task, skipped_pr, lease).await,
        None => store.complete_task(task, skipped_pr).await,
    }
}

async fn ambient_task_run_lease(store: &SharedStore, task: &Task) -> OpsResult<Option<RunLease>> {
    let Some(lease) = crate::ops::ambient_run_lease(store).await? else {
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
) -> OpsResult<Option<(Task, Option<RunLease>)>> {
    let checkout = repo.canonicalize().unwrap_or_else(|_| repo.to_path_buf());
    if let Some(lease) = crate::ops::ambient_run_lease(store).await? {
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
        lease: Option<RunLease>,
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
        Some((task, lease)) => Ok(TaskAuthority::Authority {
            store,
            task: Box::new(task),
            lease,
        }),
        None => Ok(TaskAuthority::NotATaskWorktree),
    }
}

pub(crate) fn request_task_pr_publication(repo: &Path) -> OpsResult<bool> {
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
    lease: Option<&RunLease>,
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
        let feedback = feedback_gate(&store, &task).await?;
        if !feedback.satisfied {
            return Err(task_error(format!(
                "Task {} cannot request a pull request merge while {}",
                task.plan.identifier,
                feedback.reason()
            )));
        }
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
            head_sha,
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
async fn verify_task_pr_range_with_authority(
    store: &SharedStore,
    task: &Task,
    lease: Option<&RunLease>,
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
    lease: Option<&RunLease>,
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
    lease: Option<&RunLease>,
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
    lease: Option<&RunLease>,
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
            store
                .validate_run_lease(lease)
                .await
                .map_err(|error| task_error(format!("Task body lost write authority: {error}")))?;
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
    let Some(_) = ensure_working_pr(store, task).await? else {
        return Err(task_error(format!(
            "Task {} is terminal and cannot start a Run",
            task.plan.identifier
        )));
    };
    launch_task_process(store, task, None).await
}

async fn relaunch_for_ci_incident(
    store: &SharedStore,
    task: &mut Task,
    incident_id: String,
) -> OpsResult<()> {
    let Some(_) = ensure_working_pr(store, task).await? else {
        return Err(task_error(format!(
            "Task {} is terminal and cannot repair CI",
            task.plan.identifier
        )));
    };
    launch_task_process(
        store,
        task,
        Some(crate::durable::RunTrigger::CiIncident { incident_id }),
    )
    .await
}

async fn launch_task_process(
    store: &SharedStore,
    task: &mut Task,
    trigger: Option<crate::durable::RunTrigger>,
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
    let trigger = match trigger {
        Some(trigger) => trigger,
        None => crate::durable::RunTrigger::Input {
            basis: store
                .current_epoch(&work)
                .await
                .map_err(|error| task_error(error.to_string()))?
                .current_basis,
        },
    };
    let (run, lease) = store
        .reserve_run(&work, trigger)
        .await
        .map_err(|error| task_error(format!("failed to reserve Task Run: {error}")))?;
    let tmux_name = format!(
        "lf-task-{}-{}-{}",
        tmux_session_slug(&task.plan.identifier),
        &task.id.as_str()[3..11],
        &run.id.as_str()[4..12]
    );
    store
        .update_task_for_run(task, &lease)
        .await
        .map_err(|error| task_error(error.to_string()))?;
    crate::ops::launch_in_run(
        store,
        &lease,
        crate::ops::RunLaunch {
            work: WorkRef::Task(task.id.clone()),
            wave_id: task.wave_id.clone(),
            cwd: task.worktree.clone(),
            tmux_name,
            agent: task.agent.clone(),
            account_id: None,
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
    let launch = store
        .current_launch_for_run(&run.id)
        .await
        .map_err(|error| task_error(error.to_string()))?;
    if run.state == RunState::Reserved && launch.is_none() {
        let still_starting =
            run.created_at + time::Duration::seconds(10) > time::OffsetDateTime::now_utc();
        if still_starting {
            return Ok(());
        }
    }
    if let Some(launch) = &launch {
        let alive = match &launch.containment {
            Containment::Tmux { name } => tmux_session_exists(name)
                .await
                .map_err(|error| task_error(error.to_string()))?,
            Containment::ProcessGroup { .. } => true,
        };
        if alive {
            return Ok(());
        }
        if launch.state == crate::durable::LaunchState::Starting
            && launch.started_at + time::Duration::seconds(10) > time::OffsetDateTime::now_utc()
        {
            return Ok(());
        }
    }
    store
        .recover_run(
            &run.id,
            launch.as_ref().map(|launch| &launch.id),
            ContainmentObservation::Absent,
        )
        .await
        .map_err(|error| task_error(error.to_string()))?;
    mark_task_body_lost(store, task).await
}

async fn mark_task_body_lost(store: &SharedStore, task: &mut Task) -> OpsResult<()> {
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
    relaunch_inactive_process(store, task).await
}

/// Let one live Project body supervise the progress leases of its Task bodies.
///
/// This is deliberately parent-driven: Project and Tasks do not grow a
/// second watchdog process. A live Project runner calls this on its existing
/// control tick and recovers only children whose durable progress deadline has
/// passed on a machine that can still observe their tmux body.
pub(crate) async fn reconcile_project_tasks(
    store: &SharedStore,
    project: &crate::project::Project,
) -> OpsResult<Vec<Task>> {
    let project_tasks = |tasks: Vec<Task>| {
        tasks
            .into_iter()
            .filter(|task| task.project_id == project.id)
            .collect::<Vec<_>>()
    };
    let mut tasks = project_tasks(
        store
            .list_tasks(Some(&project.wave_id))
            .await
            .map_err(|error| task_error(format!("failed to list supervised Tasks: {error}")))?,
    );
    for task in &mut tasks {
        if matches!(
            task_work_status(store, task).await?,
            WorkStatus::Done | WorkStatus::Abandoned
        ) {
            continue;
        }
        if let Err(error) = task_recovery_adoption(store, task).await {
            tracing::warn!(
                task = %task.plan.identifier,
                %error,
                "supervisor skipped Task recovery: unsafe worktree/branch state"
            );
            continue;
        }
        let observed = reconcile_task_pr(store, task).await?;
        refuse_dirty_between_prs(store, task).await?;
        reconcile_process_liveness(store, task).await?;
        reconcile_task_completion(store, task, None).await?;
        if matches!(
            task_work_status(store, task).await?,
            WorkStatus::Done | WorkStatus::Abandoned
        ) {
            continue;
        }
        let no_active_pr = if observed.is_none() {
            store
                .active_task_pr(&task.id)
                .await
                .map_err(|error| task_error(format!("failed to read active PR: {error}")))?
                .is_none()
        } else {
            false
        };
        let settled = observed.as_ref().is_some_and(TaskPr::is_settled) || no_active_pr;
        let completing = observed
            .as_ref()
            .is_some_and(|pr| pr.is_settled() && pr.after_merge() == AfterMerge::CompleteTask);
        if settled && !completing {
            ensure_working_pr(store, task).await?;
            if !matches!(
                task_work_status(store, task).await?,
                WorkStatus::Running { .. }
            ) {
                relaunch_inactive_process(store, task).await?;
            }
        } else {
            let Some(pr) = observed.as_ref() else {
                continue;
            };
            route_ci_incident(store, task, pr).await?;
            if pr.merge_request().is_none()
                && !matches!(
                    task_work_status(store, task).await?,
                    WorkStatus::Running { .. }
                )
            {
                // Publication is not settlement. Keep executing the authored
                // Task flow unless submit/land requested a merge for this head.
                relaunch_inactive_process(store, task).await?;
            }
        }
    }

    let refreshed = store
        .list_tasks(Some(&project.wave_id))
        .await
        .map_err(|error| task_error(format!("failed to reread supervised Tasks: {error}")))?;
    Ok(project_tasks(refreshed))
}

pub(crate) async fn supervise_project_task_bodies(
    store: &SharedStore,
    project: &crate::project::Project,
) -> OpsResult<usize> {
    // Project supervision reconciles each child's durable Run. Live
    // containment is never killed merely for being quiet; a missing Launch is
    // recovered by exact Run/Launch identity in reconcile_process_liveness.
    reconcile_project_tasks(store, project).await?;
    Ok(0)
}
pub(crate) async fn reconcile_task_pr(
    store: &SharedStore,
    task: &mut Task,
) -> OpsResult<Option<TaskPr>> {
    reconcile_task_pr_with_authority(store, task, None, crate::ops::pr::PrReadFreshness::Cached)
        .await
}

/// Status for a Task whose PR is open, decided after a body turn completed.
///
/// Only the runner may call this: it is the sole caller that knows whether the
/// turn that just ran pushed anything. A passive reconcile has no such evidence
/// and leaves an open PR `Waiting`, so the `ci-fix` wake stays armed.
///
/// `head_advanced` is true when the PR head moved during the iteration — the
/// body pushed a fix, so the Task waits for CI to resolve even if the reading is
/// currently `Failing`. A failing head the body did *not* move is a repair that
/// did not happen: block rather than report false progress. `github_degraded`
/// carries `Observation::Degraded`'s reason and dominates — a turn that ran
/// blind to GitHub cannot be said to have repaired anything.
/// What an open PR makes the Task wait for.
///
/// This is the durable vocabulary for the triage policy below. It deliberately
/// does not name a lifecycle status: every arm is a Wait, and the arms differ by
/// the fact that would end the wait, which is what `WaitOn` records. Keeping
/// them distinct matters because "we cannot see CI" and "CI is red and nobody
/// fixed it" need different operator responses even though the old enum
/// flattened both to `Blocked`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum OpenPrDisposition {
    /// GitHub observation is degraded, so the PR's real state is unknown.
    /// Resolved by the capability recovering, not by anyone authoring anything.
    ObservationDegraded,
    /// CI is failing and the body did not move the head. New authored direction
    /// is required before another repair can be useful.
    NeedsDirection,
}

/// Triage an open PR into the exact thing it is waiting for.
///
/// Pure policy: evidence in, disposition plus its operator-facing reason out.
/// Degraded observation dominates a healthy CI reading, because a reading we
/// could not take is not a passing reading.
pub(crate) fn decide_open_pr_status(
    pr: &TaskPr,
    github_degraded: Option<&str>,
    head_advanced: bool,
) -> (Option<OpenPrDisposition>, String) {
    let number = pr
        .github()
        .expect("open Task PR requires a GitHub PR record")
        .number;
    if let Some(reason) = github_degraded {
        return (
            Some(OpenPrDisposition::ObservationDegraded),
            format!(
                "ci-fix blocked by github-observation: {reason}. Resume when GitHub recovers; pull request #{number} stays attached."
            ),
        );
    }
    let failing = pr
        .ci_observation
        .as_ref()
        .is_some_and(|observation| observation.state == CiState::Failing);
    if failing && !head_advanced {
        return (
            Some(OpenPrDisposition::NeedsDirection),
            format!(
                "CI failing on pull request #{number}; the Task body did not repair the head. Needs a new directive; pull request #{number} stays attached."
            ),
        );
    }
    let reason = match pr.merge_request() {
        Some(request) if request.mode == PrMergeMode::User => {
            let short = request.head_sha.chars().take(12).collect::<String>();
            format!("pull request #{number} awaits the user's explicit merge of head {short}")
        }
        Some(request) => {
            let short = request.head_sha.chars().take(12).collect::<String>();
            format!("pull request #{number} awaits GitHub auto-merge of head {short}")
        }
        None => format!("pull request #{number} is published; no merge was requested"),
    };
    (None, reason)
}

/// The incident this PR's current reading warrants, if any.
///
/// The single mint point for a wake's identity. The enqueue and the arm must
/// derive it the same way or a claimed wake could never be matched to the failure
/// it names, so both call this rather than composing the parts themselves.
/// `None` when the current head is not failing a required check — including when
/// the head has moved past the reading (`fresh_ci`), which makes any wake for the
/// old head moot.
pub(crate) fn current_ci_incident(pr: &TaskPr) -> Option<CiIncident> {
    let observation = pr.fresh_ci().filter(|reading| reading.wake_legal())?;
    ci_incident(pr, observation)
}

/// Route current actionable CI into the Task's one control lane.
///
/// An active Run observes the incident on its next boundary. An idle Task
/// reserves exactly one Run whose typed trigger names the incident.
pub(crate) async fn route_ci_incident(
    store: &SharedStore,
    task: &Task,
    pr: &TaskPr,
) -> OpsResult<()> {
    let Some(incident) = current_ci_incident(pr) else {
        return Ok(());
    };
    if !matches!(
        task_work_status(store, task).await?,
        WorkStatus::Running { .. }
    ) {
        let mut task = task.clone();
        relaunch_for_ci_incident(store, &mut task, incident.identity.clone()).await?;
    }
    let work = store
        .work_for_child(&ChildRef::Task(task.id.clone()))
        .await
        .map_err(|error| task_error(error.to_string()))?;
    let run = store
        .current_run(&work)
        .await
        .map_err(|error| task_error(error.to_string()))?
        .ok_or_else(|| task_error("actionable CI has no active Run"))?;
    store
        .claim_ci_incident(&incident.identity, &run.id, time::OffsetDateTime::now_utc())
        .await
        .map_err(|error| task_error(error.to_string()))?;
    Ok(())
}

pub(crate) async fn reconcile_task_pr_for_run(
    store: &SharedStore,
    task: &mut Task,
    lease: &RunLease,
) -> OpsResult<Option<TaskPr>> {
    reconcile_task_pr_with_authority(
        store,
        task,
        Some(lease),
        crate::ops::pr::PrReadFreshness::Cached,
    )
    .await
}

/// Reconcile the PR reading GitHub live, bypassing both the store observation
/// cache and `gh`'s HTTP cache. The ci-fix settlement path uses this so it judges
/// head advancement against the authoritative remote head the repair body just
/// pushed — never a warm pre-turn observation.
pub(crate) async fn reconcile_task_pr_fresh_for_run(
    store: &SharedStore,
    task: &mut Task,
    lease: &RunLease,
) -> OpsResult<Option<TaskPr>> {
    reconcile_task_pr_with_authority(
        store,
        task,
        Some(lease),
        crate::ops::pr::PrReadFreshness::Fresh,
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
    let checks = crate::ops::pr::merge_gate_state(worktree, branch)?;
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
        // Seed with the actionable leaf failures, never the required aggregate:
        // a ci-fix turn needs the broken job, not the roll-up.
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

fn ci_incident(pr: &TaskPr, observation: &CiObservation) -> Option<CiIncident> {
    if observation.state != CiState::Failing {
        return None;
    }
    let github = pr.github()?;
    let repo = github_repo_slug_from_pr_url(&github.url)?;
    let failure_set = observation.failure_set();
    let mut digest = Sha256::new();
    for check in &failure_set {
        digest.update(check.as_bytes());
        digest.update([0]);
    }
    Some(CiIncident {
        identity: format!(
            "github:ci:{}:{}:{}:{}",
            repo,
            github.number,
            observation.head_sha,
            hex::encode(digest.finalize())
        ),
        task_id: pr.task_id.clone(),
        pr_id: pr.id.clone(),
        repo,
        pr_number: github.number,
        failed_head_sha: observation.head_sha.clone(),
        repaired_head_sha: None,
        failure_set,
        provider_completed_at: None,
        poll_observed_at: Some(observation.observed_at),
        webhook_received_at: None,
        claimed_run_id: None,
        responded_at: None,
        green_at: None,
        merged_at: None,
        blocked_at: None,
        blocked_reason: None,
        created_at: observation.observed_at,
        updated_at: observation.observed_at,
    })
}

fn github_repo_slug_from_pr_url(url: &str) -> Option<String> {
    let path = url.split("github.com/").nth(1)?;
    let mut parts = path.split('/');
    let owner = parts.next().filter(|part| !part.is_empty())?;
    let repo = parts.next().filter(|part| !part.is_empty())?;
    Some(format!("{owner}/{repo}"))
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
            GithubObservationResult::Fresh => PR_OBSERVATION_TTL,
            GithubObservationResult::Degraded { .. } => PR_OBSERVATION_DEGRADED_BACKOFF,
        };
    if retry_at <= now {
        return None;
    }
    Some(match &observation.result {
        GithubObservationResult::Fresh => Observation::Cached {
            observed_at: observation.checked_at,
        },
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
    lease: Option<&RunLease>,
    freshness: crate::ops::pr::PrReadFreshness,
) -> OpsResult<Option<TaskPr>> {
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
    // A `Fresh` read (ci-fix settlement) must never be served from the store's
    // observation cache: the repair body may have pushed a new head seconds ago,
    // and the warm cache still names the pre-turn head.
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
        github: None,
        merge: None,
    });
    invalidate_stale_merge_request(&task.worktree, publication, &github_pr)?;
    publication.github = Some(GithubPr {
        number,
        url: url.clone(),
        head_sha: github_pr.head_sha.clone(),
    });

    let mut observed_incident = None;
    let mut green_at = None;
    let mut merged_at = None;
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
            merged_at = Some(now);
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
                // Even with the PR merged, an authored Feedback checkpoint must
                // be continued before the Task can complete in the PM. The PR is
                // settling in flight, so only that Feedback fact is checked here;
                // do not bypass it or infer merge from a green head.
                let gate = feedback_gate(store, task).await?;
                if gate.satisfied {
                    let proposal = crate::task::TaskGateProposal {
                        done: true,
                        reason: format!(
                            "pull request #{} merged and completed the Task",
                            github_pr.number
                        ),
                    };
                    match task.lifecycle_phase {
                        crate::task::TaskLifecyclePhase::First => {
                            task
                                .enter_loop()
                                .map_err(|error| task_error(error.to_string()))?;
                            task
                                .enter_finally(proposal)
                                .map_err(|error| task_error(error.to_string()))?;
                        }
                        crate::task::TaskLifecyclePhase::Loop => {
                            task
                                .enter_finally(proposal)
                                .map_err(|error| task_error(error.to_string()))?;
                        }
                        crate::task::TaskLifecyclePhase::Finally => {
                            task.gate_proposal = Some(proposal);
                            task.updated_at = now;
                        }
                    }
                    reconcile_pm_writeback(store, task, Some(&url)).await;
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
                &pr.branch,
                github_pr.head_sha.as_deref(),
                now,
            ) {
                if ci_observation.state == CiState::Passing {
                    green_at = Some(ci_observation.observed_at);
                } else {
                    observed_incident = ci_incident(&pr, &ci_observation);
                }
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
        if pr.is_settled() {
            settle_task_pr_with_authority(store, &pr, None, lease)
                .await
                .map_err(|error| task_error(error.to_string()))?;
        } else {
            update_task_pr_with_authority(store, &pr, lease)
                .await
                .map_err(|error| task_error(error.to_string()))?;
        }
    }
    if let Some(incident) = observed_incident {
        store
            .observe_ci_incident(&incident)
            .await
            .map_err(|error| task_error(error.to_string()))?;
    }
    if let Some(green_at) = green_at {
        store
            .mark_ci_incidents_green(&pr.id, green_at)
            .await
            .map_err(|error| task_error(error.to_string()))?;
    }
    if let Some(merged_at) = merged_at {
        store
            .mark_ci_incidents_merged(&pr.id, merged_at)
            .await
            .map_err(|error| task_error(error.to_string()))?;
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

pub(crate) async fn ensure_working_pr_for_run(
    store: &SharedStore,
    task: &mut Task,
    lease: &RunLease,
) -> OpsResult<Option<TaskPr>> {
    ensure_working_pr_with_authority(store, task, Some(lease), RotateOptions::runner()).await
}

/// How a serial-PR rotation treats the worktree. The runner rotates only a clean
/// tree (`carry_dirty = false`); the operator's `lf pr next` carries the
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
    lease: Option<&RunLease>,
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
    lease: Option<&RunLease>,
    rotate: RotateOptions,
) -> OpsResult<Option<TaskPr>> {
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
    // A settled completing PR normally never rotates: completion is pending on
    // its explicit Feedback checkpoint, not on a follow-up PR.
    // `reconcile_task_completion` commits Work completion once that checkpoint
    // is continued. Two things
    // independently authorize one more serial PR: follow-up committed past the
    // merged tip, which the completion gate refuses to settle over, and a
    // pending directive, which the successor exists to incorporate.
    if settled.after_merge() == AfterMerge::CompleteTask
        && !matches!(&committed_carry, CommittedFollowUp::Range { .. })
    {
        return Ok(None);
    }
    if let Some(lease) = lease {
        store
            .validate_run_lease(lease)
            .await
            .map_err(|error| task_error(format!("Task body lost write authority: {error}")))?;
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
        reconcile_task_pr(&store, &mut task).await?;
        reconcile_process_liveness(&store, &mut task).await?;
        reconcile_task_completion(&store, &mut task, None).await?;
        Ok(task)
    })
}

pub fn task_complete(issue: &str, summary: String) -> OpsResult<Task> {
    let summary = summary.trim().to_string();
    if summary.is_empty() {
        return Err(task_error("completion summary cannot be empty"));
    }
    block_on_task(async move {
        let store = task_store().await?;
        let mut task = store
            .get_task_by_issue(issue)
            .await
            .map_err(|error| task_error(format!("failed to read Task: {error}")))?
            .ok_or_else(|| task_error(format!("no Task exists for {issue:?}")))?;
        reconcile_task_pr(&store, &mut task).await?;
        let lease = ambient_task_run_lease(&store, &task).await?;
        if let Some(lease) = lease.as_ref() {
            store
                .validate_run_lease(lease)
                .await
                .map_err(|error| task_error(format!("Task body lost write authority: {error}")))?;
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
        // The completion gate requires every active PR to be settled and any
        // authored Feedback checkpoint to be continued before PM completion.
        // Do not bypass either fact or infer merge from a green head.
        let gate = task_completion_gate(&store, &task).await?;
        if let Some(refusal) = gate.refusal(&task.plan.identifier) {
            // Nothing has been written. A refusal here — open Feedback, a
            // committed follow-up, anything — leaves a discardable successor
            // active, so the Task keeps its PR and no rotation is provoked.
            return Err(task_error(refusal));
        }
        propose_task_done(&mut task, summary.clone())?;
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
            let (_run, completion_lease) = store
                .reserve_run(&work, crate::durable::RunTrigger::User)
                .await
                .map_err(|error| {
                    task_error(format!("failed to reserve completion Run: {error}"))
                })?;
            let basis = store
                .current_epoch(&work)
                .await
                .map_err(|error| task_error(error.to_string()))?
                .current_basis;
            store
                .done(&completion_lease, &basis)
                .await
                .map_err(|error| task_error(format!("failed to complete Work: {error}")))?;
            reconcile_pm_writeback(&store, &mut task, None).await;
            store
                .update_task(&task)
                .await
                .map_err(|error| task_error(error.to_string()))?;
            append_task_event_with_authority(
                &store,
                &task.id,
                &TaskEventKind::Completed { summary },
                None,
            )
            .await
            .map_err(|error| task_error(error.to_string()))?;
        }
        Ok(task)
    })
}

fn propose_task_done(task: &mut Task, summary: String) -> OpsResult<()> {
    let proposal = crate::task::TaskGateProposal {
        done: true,
        reason: summary,
    };
    match task.lifecycle_phase {
        crate::task::TaskLifecyclePhase::First => {
            task
                .enter_loop()
                .map_err(|error| task_error(error.to_string()))?;
            task
                .enter_finally(proposal)
                .map_err(|error| task_error(error.to_string()))?;
        }
        crate::task::TaskLifecyclePhase::Loop => {
            task
                .enter_finally(proposal)
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
// (merged or explicitly abandoned) AND no Feedback boundary remains open.
// Every path that sets a Task to `Completed` and
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
    /// Classification only, and exactly one thing acts on it: [`task_complete`]
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

impl CompletionGate {
    fn unsatisfied(blockers: Vec<String>) -> Self {
        Self {
            satisfied: false,
            blockers,
            discardable_successor: None,
        }
    }
}

/// Open Feedback blocks terminal completion. Continuing it
/// advances the playhead; no historical disposition participates in closure.
async fn feedback_gate(store: &SharedStore, task: &Task) -> OpsResult<CompletionGate> {
    let work = store
        .work_for_child(&ChildRef::Task(task.id.clone()))
        .await
        .map_err(|error| task_error(error.to_string()))?;
    let feedback = store
        .feedback(&work)
        .await
        .map_err(|error| task_error(error.to_string()))?;
    Ok(if feedback.is_none() {
        CompletionGate {
            satisfied: true,
            blockers: Vec::new(),
            discardable_successor: None,
        }
    } else {
        CompletionGate::unsatisfied(vec!["current Feedback has not continued".to_string()])
    })
}

/// Evaluate the completion gate against the Task's durable PR and Feedback
/// state. Pure over store state: running it twice changes nothing. Use this from
/// paths where the PR state is already persisted (`task_complete`, the
/// reconcile advance, the repair). The merge-reconcile path uses
/// [`feedback_gate`] first, since the PR it is settling is not yet on disk.
pub(crate) async fn task_completion_gate(
    store: &SharedStore,
    task: &Task,
) -> OpsResult<CompletionGate> {
    let mut gate = feedback_gate(store, task).await?;
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
    lease: Option<&RunLease>,
) -> OpsResult<bool> {
    let work = store
        .work_for_child(&ChildRef::Task(task.id.clone()))
        .await
        .map_err(|error| task_error(error.to_string()))?;
    if matches!(
        store
            .work_status(&work)
            .await
            .map_err(|error| task_error(error.to_string()))?,
        WorkStatus::Done | WorkStatus::Abandoned
    ) {
        return Ok(false);
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
        let (_run, completion_lease) = store
            .reserve_run(&work, crate::durable::RunTrigger::User)
            .await
            .map_err(|error| task_error(format!("failed to reserve completion Run: {error}")))?;
        let basis = store
            .current_epoch(&work)
            .await
            .map_err(|error| task_error(error.to_string()))?
            .current_basis;
        store
            .done(&completion_lease, &basis)
            .await
            .map_err(|error| task_error(format!("failed to complete Work: {error}")))?;
        reconcile_pm_writeback(store, task, url.as_deref()).await;
        store
            .update_task(task)
            .await
            .map_err(|error| task_error(error.to_string()))?;
        store
            .append_task_event(&task.id, &TaskEventKind::Completed { summary })
            .await
            .map_err(|error| task_error(error.to_string()))?;
    }
    Ok(true)
}

pub(crate) async fn reconcile_task_completion(
    store: &SharedStore,
    task: &mut Task,
    lease: Option<&RunLease>,
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
        let launch = match store
            .current_run(&work)
            .await
            .map_err(|error| task_error(error.to_string()))?
        {
            Some(run) => store
                .current_launch_for_run(&run.id)
                .await
                .map_err(|error| task_error(error.to_string()))?,
            None => None,
        };
        let process_alive = match launch.as_ref().map(|launch| &launch.containment) {
            Some(Containment::Tmux { name }) => tmux_session_exists(name)
                .await
                .map_err(|error| task_error(error.to_string()))?,
            Some(Containment::ProcessGroup { .. }) => true,
            None => false,
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
        let resume_refusal = no_active_pr_resume_refusal(&task.plan.identifier, active, latest);
        let work_status = store
            .work_status(&work)
            .await
            .map_err(|error| task_error(format!("failed to derive Task Work status: {error}")))?;
        let action_evidence = TaskActionEvidence {
            status: work_status.clone(),
            latest_pr_phase: latest.map(|pr| pr.phase()),
            latest_pr_after_merge: latest
                .filter(|pr| pr.phase() == PrPhase::Merged)
                .map(TaskPr::after_merge),
            latest_pr_merge_request: latest.and_then(TaskPr::merge_request),
            completion_refusal: completion_refusal.as_deref(),
            resume_refusal: resume_refusal.as_deref(),
            ci: active.and_then(|pr| pr.fresh_ci()),
            process_alive: if matches!(work_status, WorkStatus::Running { .. }) {
                Some(process_alive)
            } else {
                None
            },
            predecessor_phase,
            abandon_intent: task.abandon_intent.is_some(),
            local_progress_unsettled: None,
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
            process_alive,
            launch,
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
        let request = AuthenticatedRequest::cli();
        let receipt = store
            .interrupt(&ControlCtx::User(&request), &work, &run.id)
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
    store
        .reopen_task(&task, None, crate::durable::Author::User, &carried)
        .await
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
        ensure_task_flow_override, lock_task_pr_mutation, resolve_task_lifecycle,
        resolve_task_start_input, TaskFlowOverrides,
    };
    use crate::pm::ProjectFlowPlan;

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

        let plan = resolve_task_lifecycle(repo.path(), &project, &overrides, None)
            .expect("resolve lifecycle");

        assert_eq!(plan.first.flow, "incident");
        assert_eq!(plan.loop_.flow, "slice");
        assert_eq!(plan.finally.flow, "ship");
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

        let error =
            resolve_task_lifecycle(repo.path(), &project, &TaskFlowOverrides::default(), None)
                .expect_err("reject unsafe finally flow");

        assert!(error
            .to_string()
            .contains("one or more skills followed by optional ops"));
    }

    #[test]
    fn started_task_rejects_a_different_flow_override() {
        let repo = tempfile::tempdir().expect("temp repo");

        ensure_task_flow_override(repo.path(), "INF-123", "loop", Some("slice"), "slice")
            .expect("same pinned flow is idempotent");
        let error =
            ensure_task_flow_override(repo.path(), "INF-123", "loop", Some("ship-5whys"), "slice")
                .expect_err("different flow must be rejected");

        assert!(error
            .to_string()
            .contains("Task INF-123 already pins loop flow \"slice\""));

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
