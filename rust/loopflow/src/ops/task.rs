use std::collections::BTreeMap;
use std::path::{Component, Path, PathBuf};
use std::process::Command;
use std::sync::Arc;
use std::time::{Duration, Instant};

use crate::child_session::{
    body_progress_age, observe, plan_body_recovery, task_write_lease_from_env, BodyEvidence,
    BodyRecoveryPlan, ChildBodyOutcome, ChildCommandEffect, ChildCommandId, ChildCommandKind,
    ChildCommandSource, ChildCommandState, ChildDecisionId, ChildDirective, ChildLeaseState,
    ChildProcessGeneration, ChildRef, ChildWriteLease, DEFAULT_STALL_AFTER,
};
use crate::engine::config::{load_config_or_default, parse_agent};
use crate::engine::git::{
    checkout, checkout_new_branch_from, cherry_pick_range, current_branch, delete_local_branch,
    fetch, get_default_branch, is_ancestor, is_clean, push_with_upstream, ref_exists, rev_parse,
    stash_including_untracked, stash_pop,
};
use crate::engine::naming::sanitize_for_branch;
use crate::engine::process::{
    start_lf_session_with_env, tmux_installed, tmux_live_sessions, tmux_session_exists,
    tmux_session_slug,
};
use crate::engine::worktrees::{
    create_from_placement_plan, plan_placement, PlacementStrategy, WorktreeSegment,
};
use crate::engine::{expand_flow, load_flow, ConcreteStep};
use crate::interaction_review::{
    InteractionReview, InteractionReviewDisposition, InteractionReviewId,
};
use crate::ops::error::{OpsError, OpsResult};
use crate::session_context::{
    LinearIssueId, LinearIssueSnapshot, LinearProjectId, LinearProjectSnapshot, TaskLaunchReceipt,
};
use crate::store::{
    open_existing_store, open_registry_for_authority, RegistryUnavailable, SharedStore, StoreError,
};
use crate::task::{
    AfterMerge, CiCheck, CiObservation, CiState, GithubObservation, GithubObservationResult,
    GithubPr, Observation, PmWritebackOperation, PmWritebackState, PrPhase, PrPublication,
    TaskEventKind, TaskPr, TaskPrId, TaskSession, TaskSessionStatus,
};
use crate::wave::Wave;
use sha2::{Digest, Sha256};

use super::ChildReceiptUntil;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TaskWaitUntil {
    Open,
    Terminal,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct TaskLaunchOptions {
    pub name: Option<String>,
    pub flow: Option<String>,
    pub stack_on: Option<String>,
    pub directive: Option<String>,
    pub headless: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct TaskControlResult {
    pub issue_id: String,
    pub session_id: String,
    pub command_id: String,
    pub directive_version: Option<u32>,
    pub state: ChildCommandState,
    pub effect: Option<ChildCommandEffect>,
    pub incorporated: bool,
    pub generation: Option<u32>,
    pub accepted_at: Option<time::OffsetDateTime>,
    pub incorporated_at: Option<time::OffsetDateTime>,
    pub error: Option<String>,
    pub observation: Observation,
}

fn task_control_result(
    issue_id: String,
    observation: Observation,
    result: super::child::ChildControlResult,
) -> TaskControlResult {
    TaskControlResult {
        issue_id,
        session_id: result.session_id,
        command_id: result.command_id,
        directive_version: result.directive_version,
        state: result.state,
        effect: result.effect,
        incorporated: result.incorporated,
        generation: result.generation,
        accepted_at: result.accepted_at,
        incorporated_at: result.incorporated_at,
        error: result.error,
        observation,
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TaskReceiptRead {
    pub receipt: TaskControlResult,
    pub timed_out: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct TaskDecisionResult {
    pub issue_id: String,
    pub session_id: String,
    pub decision_id: String,
    pub resolved: bool,
    pub choice: Option<String>,
    pub message: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct TaskSessionSnapshot {
    pub issue_id: String,
    pub issue_identifier: String,
    pub session_id: String,
    pub project_id: String,
    pub project: String,
    pub pm_snapshot_synced_at: i64,
    pub pm_writeback: crate::task::PmWritebackState,
    pub wave: String,
    pub project_session_id: String,
    /// The live Project Session this Task routes to: the historical owner when
    /// it is still live, or its non-terminal successor. `None` when the recorded
    /// Project Session is terminal and no live successor exists (broken chain).
    pub routing_project_session_id: Option<String>,
    /// True when routing followed the chain to a successor — i.e. the Task's
    /// historical `project_session_id` is a terminal predecessor and a live
    /// successor now owns its observations, reviews, and reconciliation.
    pub project_route_succeeded: bool,
    pub current_directive_version: u32,
    pub incorporated_directive_version: u32,
    pub status: TaskSessionStatus,
    pub status_reason: String,
    pub status_at: time::OffsetDateTime,
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
    pub latest_process: Option<ChildProcessGeneration>,
    pub latest_event: Option<crate::task::TaskEvent>,
    pub created_at: time::OffsetDateTime,
    pub updated_at: time::OffsetDateTime,
    /// Freshness of the PR state against GitHub as of this read. `Degraded`
    /// means a bounded remote read failed and the PR fields are cached, not
    /// freshly confirmed.
    pub observation: Observation,
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
    pub session_id: String,
    pub base_commit: String,
    pub head_commit: String,
    pub files: Vec<TaskChangedFile>,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct TaskDiffSnapshot {
    pub issue_identifier: String,
    pub session_id: String,
    pub path: Option<String>,
    pub patch: String,
    pub binary: bool,
    pub truncated: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct TaskFileSnapshot {
    pub issue_identifier: String,
    pub session_id: String,
    pub path: String,
    pub content: Option<String>,
    pub binary: bool,
    pub size_bytes: u64,
    pub truncated: bool,
}

#[derive(Debug, Clone, Copy)]
struct TaskWorkspace<'a> {
    issue_identifier: &'a str,
    session_id: &'a crate::task::TaskSessionId,
    worktree: &'a Path,
    base_commit: &'a str,
}

impl<'a> TaskWorkspace<'a> {
    fn new(session: &'a TaskSession, pr: &'a TaskPr) -> Self {
        Self {
            issue_identifier: &session.launch.issue.identifier,
            session_id: &session.id,
            worktree: &session.worktree,
            base_commit: &pr.base_commit,
        }
    }
}

fn active_pr(session: &TaskSession) -> OpsResult<TaskPr> {
    let session_id = session.id.clone();
    block_on_task(async move {
        task_store()
            .await?
            .active_task_pr(&session_id)
            .await
            .map_err(|error| task_error(format!("failed to read active PR: {error}")))?
            .ok_or_else(|| task_error("Task Session has no active PR"))
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
        let TaskAuthority::Authority { store, session, .. } =
            resolve_task_authority(worktree).await?
        else {
            return Ok(None);
        };
        let Some(active) = store
            .active_task_pr(&session.id)
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
        let mut parent_session = store
            .get_task_session(&parent.task_session_id)
            .await
            .map_err(|error| task_error(error.to_string()))?
            .ok_or_else(|| task_error("stack parent Task Session is missing"))?;
        // Reuse the parent's persisted PR number and observation cache. Stack
        // resolution used to enumerate every PR on the branch independently,
        // bypassing both the Task cache and outage-tolerant reconcile.
        reconcile_task_pr(&store, &mut parent_session).await?;
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

async fn owning_wave(store: &SharedStore, session: &TaskSession) -> OpsResult<Wave> {
    store
        .get_wave(&session.wave_id)
        .await
        .map_err(|error| task_error(format!("failed to read owning Wave: {error}")))?
        .ok_or_else(|| task_error(format!("owning Wave {} is not registered", session.wave_id)))
}

async fn command_source(
    store: &SharedStore,
    session: &TaskSession,
) -> OpsResult<ChildCommandSource> {
    match std::env::var("LF_PROJECT_SESSION_ID") {
        Ok(value) => {
            let project_id =
                crate::project_session::ProjectSessionId::parse(&value).map_err(|error| {
                    task_error(format!("invalid ambient Project Session id: {error}"))
                })?;
            return if session.project_session_id == project_id {
                Ok(ChildCommandSource::Project(project_id))
            } else {
                Err(task_error(format!(
                    "Project Session {project_id} cannot control Task {}; its Project Session is {}",
                    session.launch.issue.identifier, session.project_session_id
                )))
            };
        }
        Err(std::env::VarError::NotPresent) => {}
        Err(std::env::VarError::NotUnicode(_)) => {
            return Err(task_error("ambient Project Session id is not valid UTF-8"))
        }
    }
    super::util::resolve_child_command_source(
        store,
        &session.wave_id,
        &format!("Task {}", session.launch.issue.identifier),
    )
    .await
}

fn _defer_task_interactions(session: &mut TaskSession) -> OpsResult<bool> {
    if session.lifecycle.all_interactions_deferred() {
        return Ok(false);
    }
    if session.status.is_terminal() {
        return Err(task_error(format!(
            "Task {} is {}; terminal Tasks cannot change interaction policy",
            session.launch.issue.identifier,
            session.status.as_str()
        )));
    }
    if session.status.is_process_active() {
        return Err(task_error(format!(
            "Task {} has an active body; interrupt or wait for it before marking the Task headless",
            session.launch.issue.identifier
        )));
    }
    session.lifecycle.defer_all_interactions();
    session.updated_at = time::OffsetDateTime::now_utc();
    Ok(true)
}

pub fn task_run(repo: &Path, issue: &str, options: TaskLaunchOptions) -> OpsResult<TaskSession> {
    let TaskLaunchOptions {
        name,
        flow,
        stack_on,
        directive,
        headless,
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
    if let Some(existing) = block_on_task(async {
        let store = task_store().await?;
        let mut existing = store
            .get_task_session_by_issue(issue)
            .await
            .map_err(|error| task_error(format!("failed to read task registry: {error}")))?;
        if let Some(session) = &mut existing {
            if let Some(requested) = name.as_deref() {
                let requested = parse_workspace_slug(requested)?;
                if requested.as_str() != session.workspace_slug {
                    return Err(task_error(format!(
                        "Task {} already uses workspace name {:?}",
                        session.launch.issue.identifier, session.workspace_slug
                    )));
                }
            }
            if let Some(requested) = flow.as_deref() {
                let requested = resolve_task_flow(&session.worktree, Some(requested))?;
                if requested != session.lifecycle.iterate.flow {
                    return Err(task_error(format!(
                        "Task {} already uses flow {:?}",
                        session.launch.issue.identifier, session.lifecycle.iterate.flow
                    )));
                }
            }
            if let Some(requested) = stack_on.as_deref() {
                let active = store
                    .active_task_pr(&session.id)
                    .await
                    .map_err(|error| task_error(error.to_string()))?
                    .ok_or_else(|| task_error("existing Task has no active PR"))?;
                let parent_id = active.parent_pr_id.as_ref().ok_or_else(|| {
                    task_error(format!(
                        "Task {} is rooted on main, not stacked on {requested}",
                        session.launch.issue.identifier
                    ))
                })?;
                let parent = store
                    .get_task_pr(parent_id)
                    .await
                    .map_err(|error| task_error(error.to_string()))?
                    .ok_or_else(|| task_error(format!("stack parent {parent_id} is missing")))?;
                let parent_session = store
                    .get_task_session(&parent.task_session_id)
                    .await
                    .map_err(|error| task_error(error.to_string()))?
                    .ok_or_else(|| task_error("stack parent Task Session is missing"))?;
                if requested != parent_session.launch.issue.identifier
                    && requested != parent_session.launch.issue.id.as_str()
                {
                    return Err(task_error(format!(
                        "Task {} is stacked on {}, not {requested}",
                        session.launch.issue.identifier, parent_session.launch.issue.identifier
                    )));
                }
            }
            if let Some(requested) = directive.as_deref() {
                let current = store
                    .child_directives(&ChildRef::Task(session.id.clone()))
                    .await
                    .map_err(|error| task_error(error.to_string()))?
                    .into_iter()
                    .find(|value| value.version == session.current_directive_version)
                    .ok_or_else(|| task_error("Task Session has no current directive"))?;
                if current.text != requested {
                    return Err(task_error(format!(
                        "Task {} already exists with directive v{}; use `lf task steer {} <new-direction>` to replace it",
                        session.launch.issue.identifier,
                        current.version,
                        session.launch.issue.identifier,
                    )));
                }
            }
            if headless && _defer_task_interactions(session)? {
                store
                    .update_task_session(session)
                    .await
                    .map_err(|error| task_error(error.to_string()))?;
            }
        }
        Ok(existing)
    })? {
        return task_status(existing.launch.issue.id.as_str());
    }
    let main_repo = crate::ops::project::ensure_clean_main(repo, "Task start")
        .map_err(|error| task_error(error.to_string()))?;
    let resolved_flow = resolve_task_flow(&main_repo, flow.as_deref())?;
    let resolved =
        crate::ops::task_pm::resolve_task(&main_repo, issue, crate::ops::pm::PmRefresh::Auto)?;
    let segment = match name.as_deref() {
        Some(name) => parse_workspace_slug(name)?,
        None => derive_workspace_slug(&resolved.item.name)?,
    };
    let workspace_slug = segment.as_str().to_string();
    let mut plan = plan_placement(&main_repo, segment)
        .map_err(|error| task_error(format!("failed to plan task worktree: {error}")))?;
    if plan.strategy != PlacementStrategy::Create {
        return Err(task_error(format!(
            "task worktree or branch already exists without a Task Session: {} ({})",
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
                let parent_session = store
                    .get_task_session_by_issue(parent_issue)
                    .await
                    .map_err(|error| task_error(format!("failed to read parent Task: {error}")))?
                    .ok_or_else(|| {
                        task_error(format!(
                            "stack parent {parent_issue:?} has no Task Session; run it first"
                        ))
                    })?;
                if parent_session.launch.issue.id.as_str() == resolved.item.id {
                    return Err(task_error("a Task cannot stack on itself"));
                }
                let parent = store
                    .active_task_pr(&parent_session.id)
                    .await
                    .map_err(|error| task_error(format!("failed to read parent PR: {error}")))?
                    .ok_or_else(|| task_error("stack parent has no active PR"))?;
                if parent.github().is_none() {
                    return Err(task_error(format!(
                        "open the parent PR from {} before stacking work on it",
                        parent_session.worktree.display()
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
    let project_session = crate::ops::project::ensure_project_session_for_task(
        &main_repo,
        crate::ops::task_pm::ResolvedProject {
            snapshot: resolved.snapshot.clone(),
            project: resolved.project.clone(),
        },
    )?;
    let project_session_id = task_project_session_id(&project_session)?;
    let wave_id = project_session.wave_id.clone();
    let config = load_config_or_default(Some(&main_repo));
    let agent = config.agent.as_deref().unwrap_or("claude:opus");
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
        if let Some(existing) = store
            .get_task_session_by_issue(&resolved.item.id)
            .await
            .map_err(|error| task_error(format!("failed to read task registry: {error}")))?
        {
            return Ok(existing);
        }
        let now = time::OffsetDateTime::now_utc();
        let mut session = TaskSession {
            id: crate::task::TaskSessionId::new(),
            launch: TaskLaunchReceipt {
                issue: LinearIssueSnapshot {
                    id: LinearIssueId::new(resolved.item.id.clone())
                        .map_err(|error| task_error(error.to_string()))?,
                    identifier: resolved.item.identifier.clone(),
                    title: resolved.item.name.clone(),
                    description: resolved.item.description.clone(),
                },
                project: LinearProjectSnapshot {
                    id: LinearProjectId::new(resolved.project.id.clone())
                        .map_err(|error| task_error(error.to_string()))?,
                    slug: resolved.project.slug.clone(),
                    name: resolved.project.name.clone(),
                    prompt_context: project_context(&resolved.project),
                },
                pm_snapshot_synced_at: resolved.snapshot.synced_at,
            },
            wave_id,
            project_session_id,
            current_directive_version: 1,
            incorporated_directive_version: 0,
            pm_writeback: PmWritebackState::Current,
            status: TaskSessionStatus::Created,
            status_reason: "Linear task reserved before placement".to_string(),
            status_at: now,
            worktree: plan.worktree_path.clone(),
            workspace_slug: workspace_slug.clone(),
            lifecycle: if headless {
                crate::task::TaskLifecyclePlan::headless(resolved_flow)
            } else {
                crate::task::TaskLifecyclePlan::standard(resolved_flow)
            },
            lifecycle_phase: crate::task::TaskLifecyclePhase::Kickoff,
            phase_epoch: 1,
            phase_cursor: 0,
            phase_iteration: 0,
            gate_cycle: 0,
            gate_proposal: None,
            agent,
            provider,
            provider_session_id: None,
            latest_process: None,
            abandon_intent: None,
            created_at: now,
            updated_at: now,
            observation: crate::task::Observation::NotRequired,
        };
        let pr = TaskPr {
            id: TaskPrId::new(),
            task_session_id: session.id.clone(),
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
            created_at: now,
            updated_at: now,
        };

        let initial = ChildDirective::initial(
            ChildRef::Task(session.id.clone()),
            directive,
            command_source(&store, &session).await?,
        );
        match store
            .reserve_task_session_with_directive(&session, &pr, &initial)
            .await
        {
            Ok(()) => {}
            Err(StoreError::Sqlite(_)) => {
                if let Some(existing) = store
                    .get_task_session_by_issue(&resolved.item.id)
                    .await
                    .map_err(|error| {
                        task_error(format!("failed to recover task reservation: {error}"))
                    })?
                {
                    return Ok(existing);
                }
                return Err(task_error(
                    "task reservation collided with another task placement",
                ));
            }
            Err(error) => return Err(task_error(format!("failed to reserve task: {error}"))),
        }

        store
            .append_task_event(
                &session.id,
                &TaskEventKind::PrStarted {
                    pr_id: pr.id,
                    sequence: pr.sequence,
                    branch: pr.branch,
                    base_commit: pr.base_commit,
                },
            )
            .await
            .map_err(|error| task_error(error.to_string()))?;

        store
            .append_task_event(
                &session.id,
                &TaskEventKind::DirectiveChanged {
                    directive_id: initial.id,
                    version: initial.version,
                    directive_kind: initial.kind,
                },
            )
            .await
            .map_err(|error| task_error(error.to_string()))?;

        if let Err(error) = create_from_placement_plan(&main_repo, &plan) {
            record_task_failure(
                &store,
                &mut session,
                format!("worktree creation failed: {error}"),
                error.to_string(),
            )
            .await?;
            return Err(task_error(format!(
                "failed to create task worktree: {error}"
            )));
        }

        launch_task_process(&store, &mut session).await?;
        wait_until_running(&store, &session.id).await
    })
}

fn task_project_session_id(
    project: &crate::project_session::ProjectSession,
) -> OpsResult<crate::project_session::ProjectSessionId> {
    match std::env::var("LF_PROJECT_SESSION_ID") {
        Ok(value) => {
            let session_id =
                crate::project_session::ProjectSessionId::parse(&value).map_err(|error| {
                    task_error(format!("invalid ambient Project Session id: {error}"))
                })?;
            if session_id != project.id {
                return Err(task_error(format!(
                    "Project Session {session_id} cannot supervise a Task under Project Session {}",
                    project.id
                )));
            }
            Ok(session_id)
        }
        Err(std::env::VarError::NotPresent) => Ok(project.id.clone()),
        Err(std::env::VarError::NotUnicode(_)) => {
            Err(task_error("ambient Project Session id is not valid UTF-8"))
        }
    }
}

pub(crate) fn project_context(project: &crate::pm::PmProject) -> String {
    let mut context = format!("Definition:\n{}", project.definition.trim());
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
    title: String,
    project_id: &str,
    options: TaskLaunchOptions,
) -> OpsResult<TaskSession> {
    let main = crate::ops::project::ensure_clean_main(repo, "Task start")
        .map_err(|error| task_error(error.to_string()))?;
    let project =
        crate::ops::task_pm::resolve_project(&main, project_id, crate::ops::pm::PmRefresh::Auto)?;
    crate::ops::project::require_registered_wave(&project.snapshot.wave)
        .map_err(|error| task_error(error.to_string()))?;
    let marker = format!(
        "<!-- loopflow-task-start:{} -->",
        hex::encode(Sha256::digest(
            format!("{}\0{}", project.project.id, title).as_bytes()
        ))
    );
    let created = crate::ops::task_pm::create_and_load_task(
        &main,
        &project.snapshot.wave,
        &project.project.slug,
        &title,
        &marker,
    )?;
    task_run(&main, &created.item.id, options)
}

fn resolve_task_flow(repo: &Path, requested: Option<&str>) -> OpsResult<String> {
    let requested = requested.unwrap_or("task");
    let definition = load_flow(requested, repo)
        .map_err(|error| task_error(format!("failed to load Task flow {requested:?}: {error}")))?;
    let steps = expand_flow(&definition, repo).map_err(|error| {
        task_error(format!("failed to expand Task flow {requested:?}: {error}"))
    })?;
    if steps.is_empty() {
        return Err(task_error(format!("Task flow {requested:?} has no steps")));
    }
    if let Some(step) = steps
        .iter()
        .find(|step| !matches!(step, ConcreteStep::Skill(_)))
    {
        return Err(task_error(format!(
            "Task flow {requested:?} contains {step:?}; durable Task flows currently require skills"
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
    let sanitized = sanitize_for_branch(title);
    let mut words = sanitized
        .split('-')
        .filter(|word| !word.is_empty())
        .take(5)
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
    lease: Option<&ChildWriteLease>,
) -> Result<(), StoreError> {
    match lease {
        Some(lease) => store.update_task_pr_for_lease(pr, lease).await,
        None => store.update_task_pr(pr).await,
    }
}

async fn settle_task_pr_with_authority(
    store: &SharedStore,
    settled: &TaskPr,
    next: Option<&TaskPr>,
    lease: Option<&ChildWriteLease>,
) -> Result<(), StoreError> {
    match lease {
        Some(lease) => store.settle_task_pr_for_lease(settled, next, lease).await,
        None => store.settle_task_pr(settled, next).await,
    }
}

async fn append_task_event_with_authority(
    store: &SharedStore,
    session_id: &crate::task::TaskSessionId,
    event: &TaskEventKind,
    lease: Option<&ChildWriteLease>,
) -> Result<(), StoreError> {
    match lease {
        Some(lease) => {
            store
                .append_task_event_for_lease(session_id, lease, event)
                .await?;
        }
        None => {
            store.append_task_event(session_id, event).await?;
        }
    }
    Ok(())
}

async fn update_task_session_with_authority(
    store: &SharedStore,
    session: &TaskSession,
    lease: Option<&ChildWriteLease>,
) -> Result<(), StoreError> {
    match lease {
        Some(lease) => store.update_task_session_for_lease(session, lease).await,
        None => store.update_task_session(session).await,
    }
}

async fn complete_task_session_after_pr_with_authority(
    store: &SharedStore,
    session: &TaskSession,
    pr: &TaskPr,
    lease: Option<&ChildWriteLease>,
) -> Result<(), StoreError> {
    match lease {
        Some(lease) => {
            store
                .complete_task_session_after_pr_for_lease(session, pr, lease)
                .await
        }
        None => store.complete_task_session_after_pr(session, pr).await,
    }
}

async fn complete_task_session_with_authority(
    store: &SharedStore,
    session: &TaskSession,
    skipped_pr: Option<&TaskPr>,
    lease: Option<&ChildWriteLease>,
) -> Result<(), StoreError> {
    match lease {
        Some(lease) => {
            store
                .complete_task_session_for_lease(session, skipped_pr, lease)
                .await
        }
        None => store.complete_task_session(session, skipped_pr).await,
    }
}

fn ambient_task_write_lease(session: &TaskSession) -> OpsResult<Option<ChildWriteLease>> {
    let Some(value) = std::env::var_os("LF_TASK_SESSION_ID") else {
        return Ok(None);
    };
    let value = value
        .into_string()
        .map_err(|_| task_error("ambient Task Session id is not valid UTF-8"))?;
    let id = crate::task::TaskSessionId::parse(&value)
        .map_err(|error| task_error(format!("invalid ambient Task Session id: {error}")))?;
    if id != session.id {
        return Err(task_error(format!(
            "ambient Task Session {id} cannot mutate {}",
            session.id
        )));
    }
    task_write_lease_from_env()
        .map(Some)
        .map_err(|error| task_error(format!("ambient Task Session has no authority: {error}")))
}

async fn task_for_worktree(
    store: &SharedStore,
    repo: &Path,
) -> OpsResult<Option<(TaskSession, Option<ChildWriteLease>)>> {
    let checkout = repo.canonicalize().unwrap_or_else(|_| repo.to_path_buf());
    if let Some(value) = std::env::var_os("LF_TASK_SESSION_ID") {
        let value = value
            .into_string()
            .map_err(|_| task_error("ambient Task Session id is not valid UTF-8"))?;
        let id = crate::task::TaskSessionId::parse(&value)
            .map_err(|error| task_error(format!("invalid ambient Task Session id: {error}")))?;
        let session = store
            .get_task_session(&id)
            .await
            .map_err(|error| task_error(format!("failed to read ambient Task Session: {error}")))?
            .ok_or_else(|| task_error(format!("ambient Task Session {id} is not registered")))?;
        let worktree = session
            .worktree
            .canonicalize()
            .unwrap_or_else(|_| session.worktree.clone());
        if checkout != worktree {
            return Err(task_error(format!(
                "ambient Task Session {id} owns {}, not {}",
                session.worktree.display(),
                repo.display()
            )));
        }
        let lease = task_write_lease_from_env().map_err(|error| {
            task_error(format!("ambient Task Session has no authority: {error}"))
        })?;
        return Ok(Some((session, Some(lease))));
    }

    let mut matches = store
        .list_task_sessions(None)
        .await
        .map_err(|error| task_error(format!("failed to inspect Task Sessions: {error}")))?
        .into_iter()
        .filter(|session| {
            session
                .worktree
                .canonicalize()
                .unwrap_or_else(|_| session.worktree.clone())
                == checkout
        });
    let found = matches.next();
    if matches.next().is_some() {
        return Err(task_error(format!(
            "multiple Task Sessions claim worktree {}",
            repo.display()
        )));
    }
    Ok(found.map(|session| (session, None)))
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
    /// Proven authority: the registry is healthy and a Task Session owns this
    /// worktree. `lease` is present only when an ambient Task body proved it.
    /// Boxed so the `NotATaskWorktree` no-op variant stays small.
    Authority {
        store: SharedStore,
        session: Box<TaskSession>,
        lease: Option<ChildWriteLease>,
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
/// - Registry opens and a session claims this worktree → [`TaskAuthority::Authority`].
/// - Registry opens and no session claims it → [`TaskAuthority::NotATaskWorktree`].
/// - Registry file missing and no ambient Task id → [`TaskAuthority::NotATaskWorktree`]
///   (no registry means no tasks exist, so this is provably an ordinary PR).
/// - Registry missing with an ambient Task id, or present but unopenable → refuse.
async fn resolve_task_authority(repo: &Path) -> OpsResult<TaskAuthority> {
    let ambient = std::env::var_os("LF_TASK_SESSION_ID").is_some();
    let store = match open_registry_for_authority().await {
        Ok(store) => Arc::new(store),
        Err(RegistryUnavailable::MissingFile { .. }) if !ambient => {
            return Ok(TaskAuthority::NotATaskWorktree);
        }
        Err(err) => return Err(registry_authority_error(err)),
    };
    match task_for_worktree(&store, repo).await? {
        Some((session, lease)) => Ok(TaskAuthority::Authority {
            store,
            session: Box::new(session),
            lease,
        }),
        None => Ok(TaskAuthority::NotATaskWorktree),
    }
}

pub(crate) fn request_task_pr_publication(
    repo: &Path,
    after_merge: AfterMerge,
    next_slug: Option<&str>,
) -> OpsResult<bool> {
    let next_slug = next_slug.map(parse_pr_slug).transpose()?;
    if after_merge == AfterMerge::CompleteTask && next_slug.is_some() {
        return Err(task_error("--complete and --next cannot be used together"));
    }
    block_on_task(async move {
        let TaskAuthority::Authority {
            store,
            session,
            lease,
        } = resolve_task_authority(repo).await?
        else {
            return Ok(false);
        };
        let mut pr = store
            .active_task_pr(&session.id)
            .await
            .map_err(|error| task_error(format!("failed to read active PR: {error}")))?
            .ok_or_else(|| {
                task_error(format!(
                    "Task {} has no active PR",
                    session.launch.issue.identifier
                ))
            })?;
        let branch = crate::engine::git::current_branch(repo)?
            .ok_or_else(|| task_error("Task worktree is not on a branch"))?;
        if pr.branch != branch {
            return Err(task_error(format!(
                "Task {} active PR expects branch {:?}, but the worktree is on another branch",
                session.launch.issue.identifier, pr.branch
            )));
        }
        let now = time::OffsetDateTime::now_utc();
        pr.publication = Some(PrPublication {
            requested_at: pr
                .publication
                .as_ref()
                .map_or(now, |publication| publication.requested_at),
            after_merge,
            next_slug,
            github: pr.github().cloned(),
        });
        pr.updated_at = now;
        match lease.as_ref() {
            Some(lease) => store.update_task_pr_for_lease(&pr, lease).await,
            None => store.update_task_pr(&pr).await,
        }
        .map_err(|error| task_error(format!("failed to request PR publication: {error}")))?;
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
        let TaskAuthority::Authority {
            store,
            session,
            lease,
        } = resolve_task_authority(&repo).await?
        else {
            return Ok(());
        };
        verify_task_pr_range_with_authority(&store, &session, lease.as_ref(), &repo).await
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
        let TaskAuthority::Authority {
            store,
            session,
            lease,
        } = resolve_task_authority(&repo).await?
        else {
            return Ok(());
        };
        require_task_pr_range_nonempty_with_authority(&store, &session, lease.as_ref(), &repo).await
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

/// Core parity proof. Takes the store + session explicitly so it can be
/// exercised in tests without a live LF_HOME (mirrors `ensure_working_pr`).
async fn verify_task_pr_range_with_authority(
    store: &SharedStore,
    session: &TaskSession,
    lease: Option<&ChildWriteLease>,
    repo: &Path,
) -> OpsResult<()> {
    let mut pr = store
        .active_task_pr(&session.id)
        .await
        .map_err(|error| task_error(format!("failed to read active PR: {error}")))?
        .ok_or_else(|| {
            task_error(format!(
                "Task {} has no active PR",
                session.launch.issue.identifier
            ))
        })?;
    let branch =
        current_branch(repo)?.ok_or_else(|| task_error("Task worktree is not on a branch"))?;
    if pr.branch != branch {
        return Err(task_error(format!(
            "Task {} active PR expects branch {:?}, but the worktree is on {:?}",
            session.launch.issue.identifier, pr.branch, branch
        )));
    }

    let default_branch = get_default_branch(repo)?;
    let (base_ref, upstream) = resolve_verifier_upstream(store, &pr, repo, &default_branch).await?;
    let head = rev_parse(repo, "HEAD")
        .map_err(|error| task_error(format!("failed to resolve Task HEAD: {error}")))?;
    let base = pr.base_commit.clone();
    let identifier = &session.launch.issue.identifier;
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
        pr.base_commit = merge_base.clone();
        pr.updated_at = time::OffsetDateTime::now_utc();
        match lease {
            Some(lease) => store.heal_task_pr_base_for_lease(&pr, lease).await,
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
    session: &TaskSession,
    lease: Option<&ChildWriteLease>,
    repo: &Path,
) -> OpsResult<()> {
    verify_task_pr_range_with_authority(store, session, lease, repo).await?;
    let pr = store
        .active_task_pr(&session.id)
        .await
        .map_err(|error| task_error(format!("failed to read active PR: {error}")))?
        .ok_or_else(|| {
            task_error(format!(
                "Task {} has no active PR",
                session.launch.issue.identifier
            ))
        })?;
    let base = &pr.base_commit;
    let identifier = &session.launch.issue.identifier;
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
        let TaskAuthority::Authority {
            store,
            session,
            lease,
        } = resolve_task_authority(repo).await?
        else {
            return Ok(false);
        };
        let mut pr = store
            .active_task_pr(&session.id)
            .await
            .map_err(|error| task_error(format!("failed to read active PR: {error}")))?
            .ok_or_else(|| {
                task_error(format!(
                    "Task {} has no active PR",
                    session.launch.issue.identifier
                ))
            })?;
        let github_pr = github_pr.ok_or_else(|| {
            task_error(format!(
                "GitHub PR for Task {} could not be read after creation or update",
                session.launch.issue.identifier
            ))
        })?;
        if github_pr.branch != pr.branch {
            return Err(task_error(format!(
                "Task {} active PR expects branch {:?}, but GitHub reported {:?}",
                session.launch.issue.identifier, pr.branch, github_pr.branch
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
                session.launch.issue.identifier
            ))
        })?;
        publication.github = Some(GithubPr {
            number,
            url: url.clone(),
            head_sha: github_pr.head_sha.clone(),
        });
        pr.updated_at = time::OffsetDateTime::now_utc();
        match lease.as_ref() {
            Some(lease) => store.update_task_pr_for_lease(&pr, lease).await,
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
                        .append_task_event_for_lease(&session.id, lease, &event)
                        .await
                }
                None => store.append_task_event(&session.id, &event).await,
            }
            .map_err(|error| task_error(error.to_string()))?;
        }
        Ok(true)
    })
}

pub(crate) fn abandon_task_pr(
    repo: &Path,
    force: bool,
    progress: &impl crate::ops::progress::Progress,
) -> OpsResult<bool> {
    block_on_task(async move {
        let TaskAuthority::Authority {
            store,
            session,
            lease,
        } = resolve_task_authority(repo).await?
        else {
            return Ok(false);
        };
        let mut session = session;
        let mut pr = store
            .active_task_pr(&session.id)
            .await
            .map_err(|error| task_error(format!("failed to read active PR: {error}")))?
            .ok_or_else(|| {
                task_error(format!(
                    "Task {} has no active PR to abandon",
                    session.launch.issue.identifier
                ))
            })?;
        let branch =
            current_branch(repo)?.ok_or_else(|| task_error("Task worktree is not on a branch"))?;
        if branch != pr.branch {
            return Err(task_error(format!(
                "Task {} active PR expects branch {:?}, but the worktree is on {:?}",
                session.launch.issue.identifier, pr.branch, branch
            )));
        }
        let dirty = !is_clean(repo)?;
        if dirty && !force {
            return Err(task_error("uncommitted changes; use --force"));
        }
        if let Some(lease) = lease.as_ref() {
            store
                .validate_child_write_lease(&ChildRef::Task(session.id.clone()), lease)
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
            Some(lease) => store.settle_task_pr_for_lease(&pr, None, lease).await,
            None => store.settle_task_pr(&pr, None).await,
        }
        .map_err(|error| task_error(format!("failed to settle Task PR: {error}")))?;
        if !session.status.is_process_active() {
            let from = session.status;
            session.set_status(
                TaskSessionStatus::Waiting,
                format!("PR branch {branch:?} was abandoned; another PR may follow"),
            );
            store
                .update_task_session(&session)
                .await
                .map_err(|error| task_error(error.to_string()))?;
            if from != session.status {
                store
                    .append_task_event(
                        &session.id,
                        &TaskEventKind::StatusChanged {
                            from,
                            to: session.status,
                            reason: session.status_reason.clone(),
                        },
                    )
                    .await
                    .map_err(|error| task_error(error.to_string()))?;
            }
        }
        Ok(true)
    })
}

/// Record a Task Session's transition into `Failed`: set the status, persist it,
/// and append the paired `StatusChanged` + `Failed` events. Callers keep their
/// own return value; this only writes the durable failure record.
async fn record_task_failure(
    store: &SharedStore,
    session: &mut TaskSession,
    reason: impl Into<String>,
    error: String,
) -> OpsResult<()> {
    let from = session.status;
    session.set_status(TaskSessionStatus::Failed, reason);
    store
        .update_task_session(session)
        .await
        .map_err(|store_error| task_error(store_error.to_string()))?;
    store
        .append_task_event(
            &session.id,
            &TaskEventKind::StatusChanged {
                from,
                to: TaskSessionStatus::Failed,
                reason: session.status_reason.clone(),
            },
        )
        .await
        .map_err(|store_error| task_error(store_error.to_string()))?;
    store
        .append_task_event(
            &session.id,
            &TaskEventKind::Failed {
                error,
                resumable: true,
            },
        )
        .await
        .map_err(|store_error| task_error(store_error.to_string()))?;
    Ok(())
}

/// Atomically start a fresh process generation for an inactive Session.
pub(crate) async fn relaunch_inactive_process(
    store: &SharedStore,
    session: &mut TaskSession,
) -> OpsResult<()> {
    let Some(_) = ensure_working_pr(store, session).await? else {
        return Err(task_error(format!(
            "Task {} is {}; terminal Task Sessions cannot start a process",
            session.launch.issue.identifier,
            session.status.as_str()
        )));
    };
    launch_task_process(store, session).await
}

async fn launch_task_process(store: &SharedStore, session: &mut TaskSession) -> OpsResult<()> {
    // Resolve the current Home lf before reserving anything, ignoring any
    // LF_CONTROL_* pin a legacy body carries: we always launch through the
    // current binary, store, and home. A missing or incompatible lf fails
    // without burning a generation reservation.
    let execution = crate::engine::process::current_home_execution_context()
        .map_err(|error| task_error(format!("cannot resolve current lf binary: {error}")))?;
    let tmux_name = format!(
        "lf-task-{}-{}",
        tmux_session_slug(&session.launch.issue.identifier),
        &session.id.as_str()[3..11]
    );
    let from = session.status;
    let mut launch = session.clone();
    // The reserved generation records no provenance: nothing has run yet. The
    // child stamps its own binary's provenance when it boots (mark_booted), so
    // the audit row describes what ran, never merely what launched it.
    let generation = launch.begin_generation(tmux_name.clone());
    let Some(lease) = store
        .reserve_task_process(&launch, from)
        .await
        .map_err(|error| task_error(format!("failed to reserve task process: {error}")))?
    else {
        let current = store
            .get_task_session(&session.id)
            .await
            .map_err(|error| task_error(format!("failed to reread task process: {error}")))?
            .ok_or_else(|| task_error("Task Session disappeared during process reservation"))?;
        if current.status.is_process_active() {
            *session = current;
            return Ok(());
        }
        if current.status.is_terminal() {
            return Err(task_error(format!(
                "task {} became {}; terminal Task Sessions cannot start a process",
                current.launch.issue.identifier,
                current.status.as_str()
            )));
        }
        return Err(task_error(format!(
            "task {} changed from {} to {} during process reservation; retry the command",
            current.launch.issue.identifier,
            from.as_str(),
            current.status.as_str()
        )));
    };
    *session = launch;

    // argv[0] and the store come from the Session, never from this process:
    // whoever queued the command does not get to choose the child's binary or
    // its database.
    let argv = vec![
        execution.lf_bin.to_string_lossy().to_string(),
        "__task".to_string(),
        session.id.to_string(),
        "--generation".to_string(),
        generation.to_string(),
    ];
    let generation_text = generation.to_string();
    let control_bin = execution.lf_bin.to_string_lossy().to_string();
    let db_path = execution.db_path.to_string_lossy().to_string();
    let lf_home = execution.lf_home.to_string_lossy().to_string();
    // Inherit the Wave's execution home so this Task's routed shipping commands
    // (`lf commit`, `lf pr open`) target the same host as its Wave.
    let wave_home = match owning_wave(store, session).await {
        Ok(wave) => crate::engine::wave_config::read_wave_home(Path::new(wave.repo()), wave.name())
            .to_string(),
        Err(_) => crate::engine::wave_config::default_local_home(&session.worktree).to_string(),
    };
    let environment = [
        (
            crate::engine::wave_context::WAVE_ID_ENV,
            session.wave_id.as_str(),
        ),
        ("LF_TASK_SESSION_ID", session.id.as_str()),
        ("LF_TASK_GENERATION", generation_text.as_str()),
        (
            crate::child_session::TASK_LEASE_TOKEN_ENV,
            lease.token.as_str(),
        ),
        (crate::store::CONTROL_BIN_ENV, control_bin.as_str()),
        (crate::store::CONTROL_DB_PATH_ENV, db_path.as_str()),
        (crate::store::CONTROL_HOME_ENV, lf_home.as_str()),
        (crate::engine::wave_home::WAVE_HOME_ENV, wave_home.as_str()),
    ];
    if let Err(error) =
        start_lf_session_with_env(&tmux_name, &session.worktree, &argv, &environment).await
    {
        session.latest_process = Some(
            super::child::revoke_and_reap_child_body(
                store,
                &ChildRef::Task(session.id.clone()),
                crate::child_session::ChildBodyOutcome::Lost {
                    reason: format!("task process launch failed: {error}"),
                },
            )
            .await?,
        );
        record_task_failure(
            store,
            session,
            format!("task process launch failed: {error}"),
            error.to_string(),
        )
        .await?;
        return Err(task_error(format!(
            "failed to launch task process: {error}"
        )));
    }
    Ok(())
}

async fn wait_until_running(
    store: &SharedStore,
    session_id: &crate::task::TaskSessionId,
) -> OpsResult<TaskSession> {
    let deadline = tokio::time::Instant::now() + super::child::CHILD_STARTUP_GRACE;
    loop {
        let session = store
            .get_task_session(session_id)
            .await
            .map_err(|error| task_error(format!("failed to observe task startup: {error}")))?
            .ok_or_else(|| task_error("task session disappeared during startup"))?;
        if session.status != TaskSessionStatus::Starting {
            return if session.status == TaskSessionStatus::Running {
                Ok(session)
            } else {
                Err(task_error(format!(
                    "task {} did not start: {}",
                    session.launch.issue.identifier, session.status_reason
                )))
            };
        }
        if tokio::time::Instant::now() >= deadline {
            return Err(task_error(format!(
                "task {} process did not report running within 10 seconds",
                session.launch.issue.identifier
            )));
        }
        tokio::time::sleep(Duration::from_millis(100)).await;
    }
}

pub(crate) async fn reconcile_process_liveness(
    store: &SharedStore,
    session: &mut TaskSession,
) -> OpsResult<()> {
    if session
        .latest_process
        .as_ref()
        .is_some_and(super::child::child_body_reservation_is_fresh)
    {
        return Ok(());
    }
    let alive = match session.latest_process.as_ref() {
        Some(process) => tmux_session_exists(&process.tmux_name)
            .await
            .map_err(|error| task_error(error.to_string()))?,
        None => false,
    };
    if alive {
        return Ok(());
    }
    // A dead lease is reaped regardless of Session status. A Waiting or Failed
    // Session can still carry a stale Legacy/Reserved/Active lease from a body
    // that vanished without recording a terminal outcome; an explicit resume
    // must revoke it here, or the fresh process can never reserve the slot.
    let lost_reason = "task process disappeared before recording a terminal outcome";
    if session.latest_process.as_ref().is_some_and(|process| {
        matches!(
            process.state,
            crate::child_session::ChildLeaseState::Legacy
                | crate::child_session::ChildLeaseState::Reserved
                | crate::child_session::ChildLeaseState::Active
        )
    }) {
        let outcome = super::child::lost_child_body_outcome(
            session
                .latest_process
                .as_ref()
                .expect("matched child process must still be present"),
            lost_reason,
        );
        session.latest_process = Some(
            super::child::revoke_and_reap_child_body(
                store,
                &ChildRef::Task(session.id.clone()),
                outcome,
            )
            .await?,
        );
    }
    // Only a Session whose status still claims a live process needs a terminal
    // transition here. One already Waiting or Failed keeps its status; the
    // resume that follows relaunches it against the now-reaped lease.
    if !session.status.is_process_active() {
        return Ok(());
    }
    let active = store
        .active_task_pr(&session.id)
        .await
        .map_err(|error| task_error(format!("failed to read active PR: {error}")))?;
    if active.as_ref().is_none_or(|pr| pr.phase() == PrPhase::Open) {
        // W2-144 gen 7: before settling a dead-process open-PR Task to Waiting,
        // consume a queued manual Resume. The command was queued (by
        // `lf task resume` or a queued steer) but never claimed because the
        // process died first. Relaunching lets the new generation drain and
        // honor it — one relaunch path, shared with the ci-fix wake.
        let commands = store
            .list_child_commands(&ChildRef::Task(session.id.clone()))
            .await
            .map_err(|error| task_error(format!("failed to read command queue: {error}")))?;
        let has_pending_resume = commands.iter().any(|cmd| {
            matches!(cmd.kind, ChildCommandKind::Resume { .. }) && !cmd.state.is_terminal()
        });
        if has_pending_resume {
            return relaunch_inactive_process(store, session).await;
        }
        let from = session.status;
        session.set_status(
            TaskSessionStatus::Waiting,
            match active {
                Some(pr) => format!("PR {} is open; waiting for review", pr.sequence),
                None => "the previous PR settled; another PR may follow".to_string(),
            },
        );
        store
            .update_task_session(session)
            .await
            .map_err(|error| task_error(error.to_string()))?;
        store
            .append_task_event(
                &session.id,
                &TaskEventKind::StatusChanged {
                    from,
                    to: session.status,
                    reason: session.status_reason.clone(),
                },
            )
            .await
            .map_err(|error| task_error(error.to_string()))?;
        return Ok(());
    }
    let reason = "task process is missing; resume the same Task Session with `lf task resume`";
    record_task_failure(store, session, reason, reason.to_string()).await
}

/// Let one live Project body supervise the progress leases of its Task bodies.
///
/// This is deliberately parent-driven: Project and Task Sessions do not grow a
/// second watchdog process. A live Project runner calls this on its existing
/// control tick and recovers only children whose durable progress deadline has
/// passed on a machine that can still observe their tmux body.
pub(crate) async fn supervise_project_task_bodies(
    store: &SharedStore,
    project: &crate::project_session::ProjectSession,
) -> OpsResult<usize> {
    if !tmux_installed() {
        return Ok(0);
    }
    let live_sessions = tmux_live_sessions()
        .await
        .map_err(|error| task_error(format!("failed to observe Task bodies: {error}")))?;
    let tasks = store
        .list_task_sessions(Some(&project.wave_id))
        .await
        .map_err(|error| task_error(format!("failed to list supervised Tasks: {error}")))?;
    let now = time::OffsetDateTime::now_utc();
    let mut recovered = 0;
    for task in tasks.into_iter().filter(|task| {
        task.project_session_id == project.id
            && task.status.is_process_active()
            && task.latest_process.as_ref().is_some_and(|process| {
                process.state == ChildLeaseState::Active
                    && live_sessions.contains(&process.tmux_name)
            })
    }) {
        let latest_event = store
            .latest_task_event(&task.id)
            .await
            .map_err(|error| task_error(format!("failed to read Task progress: {error}")))?;
        let observation = observe(
            &BodyEvidence {
                intent: task.status.body_intent(),
                observable: true,
                process_alive: true,
                progress_age: body_progress_age(
                    latest_event.as_ref().map(|event| event.created_at),
                    task.status_at,
                    now,
                ),
                step: Some(task.lifecycle_phase.as_str().to_string()),
                reason: task.status_reason.clone(),
            },
            DEFAULT_STALL_AFTER,
        );
        match recover_stalled_task_body(
            store,
            task,
            &observation,
            latest_event.as_ref().map(|event| event.id),
        )
        .await
        {
            Ok(true) => recovered += 1,
            Ok(false) => {}
            Err(error) => {
                tracing::warn!(
                    project_session = %project.id,
                    error = %error,
                    "Task body recovery failed"
                );
            }
        }
    }
    Ok(recovered)
}

async fn recover_stalled_task_body(
    store: &SharedStore,
    task: TaskSession,
    observation: &crate::child_session::BodyObservation,
    latest_event_id: Option<i64>,
) -> OpsResult<bool> {
    let commands = store
        .list_child_commands(&ChildRef::Task(task.id.clone()))
        .await
        .map_err(|error| task_error(format!("failed to inspect Task commands: {error}")))?;
    let generation = task
        .latest_process
        .as_ref()
        .map(|process| process.generation)
        .ok_or_else(|| task_error("stalled Task has no process generation"))?;
    let plan = plan_body_recovery(observation, &commands, generation);
    if plan == BodyRecoveryPlan::LeaveAlone {
        return Ok(false);
    }
    let active_pr = store
        .active_task_pr(&task.id)
        .await
        .map_err(|error| task_error(format!("failed to inspect Task PR: {error}")))?;
    if let Some(reason) = task.supervisor_restart_bar(active_pr.as_ref()) {
        tracing::info!(task = %task.launch.issue.identifier, "not recovering Task body: {reason}");
        return Ok(false);
    }
    let progress_age = observation.progress_age_secs.unwrap_or_default();
    let uncertain = match &plan {
        BodyRecoveryPlan::NeedsInput { commands } => Some(commands.clone()),
        BodyRecoveryPlan::Restart => None,
        BodyRecoveryPlan::LeaveAlone => unreachable!("leave-alone plan returned above"),
    };
    // A restart commits a successor body into the worktree, so it must clear the
    // same adoption preconditions as an explicit resume — refuse before the lease
    // is reaped rather than after rotation rejects the branch. An uncertain plan
    // launches nothing, so it still records the loss honestly; the explicit
    // resume that follows is where the refusal belongs.
    if uncertain.is_none() {
        if let Err(error) = task_recovery_adoption(store, &task).await {
            tracing::info!(
                task = %task.launch.issue.identifier,
                "not recovering Task body: {error}"
            );
            return Ok(false);
        }
    }
    let reason = uncertain.as_ref().map_or_else(
        || {
            format!(
                "body generation {generation} stalled after {progress_age}s without durable progress; recovering the same Task Session"
            )
        },
        |commands| {
            let commands = commands
                .iter()
                .map(ToString::to_string)
                .collect::<Vec<_>>()
                .join(", ");
            format!(
                "body generation {generation} stalled after {progress_age}s during provider delivery ({commands}); delivery outcome is uncertain, inspect the transcript and resume explicitly"
            )
        },
    );
    let outcome = if uncertain.is_some() {
        ChildBodyOutcome::Lost {
            reason: reason.clone(),
        }
    } else {
        ChildBodyOutcome::Superseded {
            reason: reason.clone(),
        }
    };
    let Some(revoked) = store
        .revoke_task_process_if_unchanged(
            &task.id,
            generation,
            task.status_at,
            latest_event_id,
            &outcome,
        )
        .await
        .map_err(|error| task_error(format!("failed to claim stalled Task body: {error}")))?
    else {
        return Ok(false);
    };
    if let Err(error) =
        super::child::reap_revoked_child_body(store, &ChildRef::Task(task.id.clone()), revoked)
            .await
    {
        let mut current = store
            .get_task_session(&task.id)
            .await
            .map_err(|store_error| task_error(store_error.to_string()))?
            .ok_or_else(|| task_error("Task Session disappeared during recovery"))?;
        let failure = format!(
            "body generation {generation} lease was revoked after a stall but its process group could not be reaped: {error}; manual cleanup is required"
        );
        record_task_failure(store, &mut current, failure.clone(), failure).await?;
        return Err(error);
    }

    let mut current = store
        .get_task_session(&task.id)
        .await
        .map_err(|error| task_error(error.to_string()))?
        .ok_or_else(|| task_error("Task Session disappeared during recovery"))?;
    if uncertain.is_some() {
        let successor_generation = generation
            .checked_add(1)
            .ok_or_else(|| task_error("Task process generation overflow"))?;
        let changed = store
            .mark_stale_child_deliveries_uncertain(
                &ChildRef::Task(task.id.clone()),
                successor_generation,
            )
            .await
            .map_err(|error| {
                task_error(format!("failed to preserve uncertain delivery: {error}"))
            })?;
        for command in changed {
            store
                .append_task_event(
                    &task.id,
                    &TaskEventKind::CommandChanged {
                        command_id: command.id,
                        state: ChildCommandState::Uncertain,
                        effect: command.effect,
                        error: command.error,
                    },
                )
                .await
                .map_err(|error| task_error(error.to_string()))?;
        }
    }
    let from = current.status;
    current.set_status(TaskSessionStatus::Waiting, reason);
    store
        .update_task_session(&current)
        .await
        .map_err(|error| task_error(error.to_string()))?;
    store
        .append_task_event(
            &current.id,
            &TaskEventKind::StatusChanged {
                from,
                to: TaskSessionStatus::Waiting,
                reason: current.status_reason.clone(),
            },
        )
        .await
        .map_err(|error| task_error(error.to_string()))?;
    if uncertain.is_none() {
        if let Err(error) = relaunch_inactive_process(store, &mut current).await {
            let mut persisted = store
                .get_task_session(&task.id)
                .await
                .map_err(|store_error| task_error(store_error.to_string()))?
                .ok_or_else(|| task_error("Task Session disappeared during relaunch"))?;
            if persisted.status == TaskSessionStatus::Waiting {
                let failure = format!(
                    "body generation {generation} was reaped after a stall but its successor could not start: {error}"
                );
                record_task_failure(store, &mut persisted, failure.clone(), failure).await?;
            }
            return Err(error);
        }
    }
    Ok(true)
}

pub(crate) async fn reconcile_task_pr(
    store: &SharedStore,
    session: &mut TaskSession,
) -> OpsResult<Option<TaskPr>> {
    reconcile_task_pr_with_authority(store, session, None).await
}

/// Wake a Task sleeping on an open PR into a `ci-fix` turn. Thin re-export of the
/// shared child-launch path so supervisors (the project loop) can trigger the
/// wake without reaching into `ops::child`. Gated by `ci_fix_restart_bar`; a
/// no-op unless the active PR's current head warrants it.
pub(crate) async fn wake_task_ci_fix(
    store: &SharedStore,
    session: &mut TaskSession,
) -> OpsResult<bool> {
    super::child::wake_task_ci_fix(store, session).await
}

pub(crate) async fn reconcile_task_pr_for_lease(
    store: &SharedStore,
    session: &mut TaskSession,
    lease: &ChildWriteLease,
) -> OpsResult<Option<TaskPr>> {
    reconcile_task_pr_with_authority(store, session, Some(lease)).await
}

/// Read the open PR's required checks and classify them for `head_sha`. Returns
/// `None` — CI state unknown, status falls back to plain review waiting — when
/// GitHub reports no head, there are no required checks, or gh is unavailable.
/// Failure dominates: any failing required check makes the head `Failing` even
/// while others are still pending.
fn observe_required_checks(
    worktree: &Path,
    branch: &str,
    head_sha: Option<&str>,
    prior: Option<&CiObservation>,
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
    let mut observation = CiObservation {
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
        woken_failure_set: None,
    };
    // Carry the dedup marker forward across reconciles: a wake already fired for
    // this exact `(head, failing set)` must not fire again on the next poll. The
    // marker only survives while both the head and the failing set are unchanged;
    // a moved head or a changed failing set is a fresh reading that re-arms.
    if let Some(prior) = prior {
        if prior.head_sha == observation.head_sha
            && prior.woken_failure_set.as_deref() == Some(observation.failure_set().as_slice())
        {
            observation.woken_failure_set = prior.woken_failure_set.clone();
        }
    }
    Some(observation)
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

async fn reconcile_task_pr_with_authority(
    store: &SharedStore,
    session: &mut TaskSession,
    lease: Option<&ChildWriteLease>,
) -> OpsResult<Option<TaskPr>> {
    let Some(mut pr) = store
        .active_task_pr(&session.id)
        .await
        .map_err(|error| task_error(format!("failed to read active PR: {error}")))?
    else {
        return Ok(None);
    };
    // GitHub is a reconciliation input, not the Task's store of record. Read the
    // one persisted PR by number (a single bounded REST call, never `gh pr
    // list`); an unpublished working PR has no number and is not read remotely.
    // Recent attempts are reused across processes. A quota/network/GitHub failure
    // opens a durable circuit and keeps the cached row rather than erroring the
    // control command that triggered reconcile.
    let Some(number) = pr.github().map(|github| github.number) else {
        session.observation = Observation::NotRequired;
        return Ok(Some(pr));
    };
    let now = time::OffsetDateTime::now_utc();
    if let Some(observation) = cached_github_observation(&pr, now) {
        session.observation = observation;
        return Ok(Some(pr));
    }
    let previous = pr.clone();
    let github_pr =
        match crate::ops::pr::observe_pr_by_number(&session.worktree, number, &pr.branch) {
            crate::ops::pr::PrObservation::Fresh(info) => {
                pr.github_observation = Some(GithubObservation {
                    checked_at: now,
                    result: GithubObservationResult::Fresh,
                });
                session.observation = Observation::Fresh { observed_at: now };
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
                session.observation = Observation::Fresh { observed_at: now };
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
                session.observation = Observation::Degraded {
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
    let previous_session_status = session.status;
    let previous_status_reason = session.status_reason.clone();
    let previous_pm_writeback = session.pm_writeback.clone();
    let publication = pr.publication.get_or_insert(PrPublication {
        requested_at: now,
        after_merge: AfterMerge::Review,
        next_slug: None,
        github: None,
    });
    publication.github = Some(GithubPr {
        number,
        url: url.clone(),
        head_sha: github_pr.head_sha.clone(),
    });

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
            // Record the merge, but withhold completion while an accepted
            // directive is unincorporated — an auto-merge armed by `lf pr land`
            // must not silently erase direction accepted after it was armed.
            let completes = pr
                .publication
                .as_ref()
                .is_some_and(|publication| publication.after_merge == AfterMerge::CompleteTask)
                && !has_pending_directive(session);
            if completes {
                session.set_status(
                    TaskSessionStatus::Completed,
                    format!(
                        "pull request #{} merged and completed the Task",
                        github_pr.number
                    ),
                );
                reconcile_pm_writeback(store, session, Some(&url)).await;
            } else if !session.status.is_process_active() {
                let reason = if has_pending_directive(session) {
                    format!(
                        "pull request #{} merged, but directive v{} is not yet incorporated; \
                         acknowledge it or re-steer before completing",
                        github_pr.number, session.current_directive_version
                    )
                } else {
                    format!(
                        "pull request #{} merged; another PR may follow",
                        github_pr.number
                    )
                };
                session.set_status(TaskSessionStatus::Waiting, reason);
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
            pr.abandoned_at = Some(now);
            pr.ci_observation = None;
            if !session.status.is_process_active() {
                session.set_status(
                    TaskSessionStatus::Waiting,
                    format!("pull request #{} closed without merge", github_pr.number),
                );
            }
            None
        }
        _ => {
            if !session.status.is_process_active() {
                session.set_status(
                    TaskSessionStatus::Waiting,
                    format!("pull request #{} is open for review", github_pr.number),
                );
            }
            if let Some(ci_observation) = observe_required_checks(
                &session.worktree,
                &pr.branch,
                github_pr.head_sha.as_deref(),
                pr.ci_observation.as_ref(),
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
    let completes_task = pr.phase() == PrPhase::Merged
        && session.status == TaskSessionStatus::Completed
        && pr
            .publication
            .as_ref()
            .is_some_and(|publication| publication.after_merge == AfterMerge::CompleteTask);
    let mut session_saved_with_pr = false;
    if pr_changed {
        pr.updated_at = now;
        if completes_task {
            complete_task_session_after_pr_with_authority(store, session, &pr, lease)
                .await
                .map_err(|error| task_error(error.to_string()))?;
            session_saved_with_pr = true;
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
    if !session_saved_with_pr
        && (session.status != previous_session_status
            || session.status_reason != previous_status_reason
            || session.pm_writeback != previous_pm_writeback)
    {
        update_task_session_with_authority(store, session, lease)
            .await
            .map_err(|error| task_error(error.to_string()))?;
    }
    if session.status != previous_session_status {
        append_task_event_with_authority(
            store,
            &session.id,
            &TaskEventKind::StatusChanged {
                from: previous_session_status,
                to: session.status,
                reason: session.status_reason.clone(),
            },
            lease,
        )
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
                append_task_event_with_authority(store, &session.id, &event, lease)
                    .await
                    .map_err(|error| task_error(error.to_string()))?;
            }
        }
    }
    if previous_session_status != TaskSessionStatus::Completed
        && session.status == TaskSessionStatus::Completed
    {
        append_task_event_with_authority(
            store,
            &session.id,
            &TaskEventKind::Completed {
                summary: "pull request merge completed the Task".to_string(),
            },
            lease,
        )
        .await
        .map_err(|error| task_error(error.to_string()))?;
    }
    Ok(Some(pr))
}

/// The slug for the next serial PR: the operator's `--next` override, else the
/// settled PR's recorded `next_slug`, else the sequence number. One computation
/// shared by the recovery gate and the rotation.
fn next_pr_slug(settled: &TaskPr, slug_override: Option<&str>) -> String {
    slug_override
        .map(str::to_string)
        .or_else(|| {
            settled
                .publication
                .as_ref()
                .and_then(|publication| publication.next_slug.clone())
        })
        .unwrap_or_else(|| (settled.sequence + 1).to_string())
}

/// The deterministic next serial branch for a settled Task PR — the same branch
/// `ensure_working_pr_with_authority` would cut. The recovery gate reads this so
/// a partial rotation (worktree already on the next branch) is adopted, not
/// refused as an unrelated branch.
fn deterministic_next_branch(
    session: &TaskSession,
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
    Ok(format!("{author}/{}-{slug}", session.workspace_slug))
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
    session: &TaskSession,
) -> OpsResult<TaskRecoveryAdoption> {
    let worktree = &session.worktree;
    let identifier = &session.launch.issue.identifier;
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
        .active_task_pr(&session.id)
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
        .task_prs(&session.id)
        .await
        .map_err(|error| task_error(format!("failed to read Task PRs: {error}")))?;
    let settled = prs
        .last()
        .cloned()
        .ok_or_else(|| task_error("Task Session has no PR history"))?;
    if !settled.is_settled() {
        return Err(task_error(format!(
            "Task PR {} is neither active nor settled",
            settled.id
        )));
    }
    let next = deterministic_next_branch(session, &settled, None)?;
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
pub(crate) async fn refuse_dirty_between_prs(
    store: &SharedStore,
    session: &TaskSession,
) -> OpsResult<()> {
    if store
        .active_task_pr(&session.id)
        .await
        .map_err(|error| task_error(format!("failed to read active PR: {error}")))?
        .is_some()
    {
        return Ok(());
    }
    if is_clean(&session.worktree)
        .map_err(|error| task_error(format!("failed to inspect Task worktree: {error}")))?
    {
        return Ok(());
    }
    Err(task_error(format!(
        "Task {} cannot recover between PRs while {} has uncommitted changes; carry them \
         forward with `lf pr next` or commit before resuming",
        session.launch.issue.identifier,
        session.worktree.display()
    )))
}

pub(crate) async fn ensure_working_pr(
    store: &SharedStore,
    session: &mut TaskSession,
) -> OpsResult<Option<TaskPr>> {
    ensure_working_pr_with_authority(store, session, None, RotateOptions::runner()).await
}

pub(crate) async fn ensure_working_pr_for_lease(
    store: &SharedStore,
    session: &mut TaskSession,
    lease: &ChildWriteLease,
) -> OpsResult<Option<TaskPr>> {
    ensure_working_pr_with_authority(store, session, Some(lease), RotateOptions::runner()).await
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

/// The commit range of follow-up work committed on the settled branch *after*
/// its PR merged, or `None` when there is nothing to carry. The merged branch
/// tip is `head_sha` — recorded by reconcile from GitHub's `headRefOid`; commits
/// reachable from the branch but not from `head_sha` are the post-merge
/// follow-up. Returns `None` when no tip was recorded, the branch has no commits
/// beyond it, or the recorded tip is not an ancestor of the branch (a rewrite,
/// or the object is absent locally) — an ambiguous cut skips the carry rather
/// than misapplying already-merged work.
fn committed_follow_up_range(
    worktree: &Path,
    settled: &TaskPr,
) -> OpsResult<Option<(String, String)>> {
    let Some(head_sha) = settled.github().and_then(|github| github.head_sha.clone()) else {
        return Ok(None);
    };
    let tip = rev_parse(worktree, &settled.branch)
        .map_err(|error| task_error(format!("failed to resolve settled branch tip: {error}")))?;
    if tip == head_sha {
        return Ok(None);
    }
    let ancestor = is_ancestor(worktree, &head_sha, &settled.branch)
        .map_err(|error| task_error(format!("failed to check follow-up ancestry: {error}")))?;
    if !ancestor {
        return Ok(None);
    }
    Ok(Some((head_sha, settled.branch.clone())))
}

/// A directive was accepted (its version advanced) but the body has not yet
/// acknowledged it. Completion — manual or an armed auto-merge — must not fire
/// while this holds, or the accepted direction is silently erased.
fn has_pending_directive(session: &TaskSession) -> bool {
    session.current_directive_version > session.incorporated_directive_version
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
    session: &mut TaskSession,
    lease: Option<&ChildWriteLease>,
    rotate: RotateOptions,
) -> OpsResult<Option<TaskPr>> {
    reconcile_task_pr_with_authority(store, session, lease).await?;
    if session.status.is_terminal() {
        return Ok(None);
    }
    if let Some(active) = store
        .active_task_pr(&session.id)
        .await
        .map_err(|error| task_error(format!("failed to read active PR: {error}")))?
    {
        return Ok(Some(active));
    }

    let prs = store
        .task_prs(&session.id)
        .await
        .map_err(|error| task_error(format!("failed to read Task PRs: {error}")))?;
    let settled = prs
        .last()
        .cloned()
        .ok_or_else(|| task_error("Task Session has no PR history"))?;
    if !settled.is_settled() {
        return Err(task_error(format!(
            "Task PR {} is neither active nor settled",
            settled.id
        )));
    }
    if let Some(lease) = lease {
        store
            .validate_child_write_lease(&ChildRef::Task(session.id.clone()), lease)
            .await
            .map_err(|error| task_error(format!("Task body lost write authority: {error}")))?;
    }
    let sequence = settled.sequence + 1;
    let slug = next_pr_slug(&settled, rotate.slug_override.as_deref());
    let branch = deterministic_next_branch(session, &settled, rotate.slug_override.as_deref())?;
    let default_branch = get_default_branch(&session.worktree)
        .map_err(|error| task_error(format!("failed to resolve default branch: {error}")))?;
    let (base_ref, base_commit) = resolve_upstream_base(&session.worktree, &default_branch)?;
    if !rotate.carry_dirty
        && !is_clean(&session.worktree)
            .map_err(|error| task_error(format!("failed to inspect Task worktree: {error}")))?
    {
        return Err(task_error(format!(
            "Task {} cannot rotate PRs while {} has uncommitted changes",
            session.launch.issue.identifier,
            session.worktree.display()
        )));
    }
    // The merged branch tip GitHub recorded (`head_sha`) is the cut between
    // already-merged work and the follow-up the worker committed on top after the
    // merge. Rotation carries that committed range forward — plus any dirty edits
    // — so no work is dropped when moving onto the next serial branch.
    let committed_carry = committed_follow_up_range(&session.worktree, &settled)?;
    let current = current_branch(&session.worktree)
        .map_err(|error| task_error(format!("failed to inspect Task branch: {error}")))?
        .ok_or_else(|| task_error("Task worktree is detached"))?;
    if current != branch {
        if current != settled.branch {
            return Err(task_error(format!(
                "Task {} expected settled branch {:?} or recovery branch {:?}, but {} is on {:?}",
                session.launch.issue.identifier,
                settled.branch,
                branch,
                session.worktree.display(),
                current
            )));
        }
        let local_ref = format!("refs/heads/{branch}");
        let remote_ref = format!("refs/remotes/origin/{branch}");
        let collision = ref_exists(&session.worktree, &local_ref)
            .map_err(|error| task_error(format!("failed to inspect branch collision: {error}")))?
            || ref_exists(&session.worktree, &remote_ref).map_err(|error| {
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
        let stashed = stash_including_untracked(&session.worktree)
            .map_err(|error| task_error(format!("failed to stash follow-up edits: {error}")))?;
        if let Err(error) = checkout_new_branch_from(&session.worktree, &branch, &base_ref) {
            let recovered = current_branch(&session.worktree)
                .map_err(|read_error| {
                    task_error(format!("failed to inspect recovery branch: {read_error}"))
                })?
                .as_deref()
                == Some(branch.as_str());
            if !recovered {
                if stashed {
                    stash_pop(&session.worktree).map_err(|recovery_error| {
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
        if let Some((from, to)) = &committed_carry {
            if let Err(error) = cherry_pick_range(&session.worktree, from, to) {
                roll_back_failed_rotation(&session.worktree, &settled.branch, &branch, stashed)
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
            stash_pop(&session.worktree).map_err(|error| {
                task_error(format!(
                    "carried the committed follow-up but could not reapply dirty edits: {error}; \
                     the recovery branch and retained stash are in {} for conflict resolution",
                    session.worktree.display()
                ))
            })?;
        }
    }
    push_with_upstream(&session.worktree, "origin", &branch)
        .map_err(|error| task_error(format!("failed to push next PR branch: {error}")))?;

    let now = time::OffsetDateTime::now_utc();
    let next = TaskPr {
        id: TaskPrId::new(),
        task_session_id: session.id.clone(),
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
        created_at: now,
        updated_at: now,
    };
    match settle_task_pr_with_authority(store, &settled, Some(&next), lease).await {
        Ok(()) => {
            append_task_event_with_authority(
                store,
                &session.id,
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
                .task_prs(&session.id)
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
        let (mut session, lease) = task_for_worktree(&store, &repo)
            .await?
            .ok_or_else(|| task_error("no Task Session owns this worktree"))?;
        // Observe an out-of-band merge before deciding whether to rotate.
        reconcile_task_pr_with_authority(&store, &mut session, lease.as_ref()).await?;
        if let Some(active) = store
            .active_task_pr(&session.id)
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
        if session.status.is_terminal() {
            return Err(task_error(format!(
                "Task {} is already {}; nothing to rotate",
                session.launch.issue.identifier,
                session.status.as_str()
            )));
        }
        let rotate = RotateOptions {
            carry_dirty: true,
            slug_override,
        };
        ensure_working_pr_with_authority(&store, &mut session, lease.as_ref(), rotate)
            .await?
            .ok_or_else(|| task_error("Task has no settled PR to rotate from"))
    })
}

pub fn task_status(issue: &str) -> OpsResult<TaskSession> {
    block_on_task(async move {
        let store = task_store().await?;
        let mut session = store
            .get_task_session_by_issue(issue)
            .await
            .map_err(|error| task_error(format!("failed to read task status: {error}")))?
            .ok_or_else(|| task_error(format!("no Task Session exists for {issue:?}")))?;
        reconcile_task_pr(&store, &mut session).await?;
        reconcile_process_liveness(&store, &mut session).await?;
        if session.status == TaskSessionStatus::Completed
            && matches!(session.pm_writeback, PmWritebackState::Pending { .. })
        {
            retry_pm_writeback(&store, &mut session).await;
            store
                .update_task_session(&session)
                .await
                .map_err(|error| task_error(error.to_string()))?;
            return Ok(session);
        }
        Ok(session)
    })
}

pub fn task_complete(issue: &str, summary: String) -> OpsResult<TaskSession> {
    let summary = summary.trim().to_string();
    if summary.is_empty() {
        return Err(task_error("completion summary cannot be empty"));
    }
    block_on_task(async move {
        let store = task_store().await?;
        let mut session = store
            .get_task_session_by_issue(issue)
            .await
            .map_err(|error| task_error(format!("failed to read Task Session: {error}")))?
            .ok_or_else(|| task_error(format!("no Task Session exists for {issue:?}")))?;
        reconcile_task_pr(&store, &mut session).await?;
        let lease = ambient_task_write_lease(&session)?;
        if let Some(lease) = lease.as_ref() {
            store
                .validate_child_write_lease(&ChildRef::Task(session.id.clone()), lease)
                .await
                .map_err(|error| task_error(format!("Task body lost write authority: {error}")))?;
        }
        if session.status == TaskSessionStatus::Completed {
            return Ok(session);
        }
        if session.status == TaskSessionStatus::Abandoned {
            return Err(task_error(format!(
                "Task {} is abandoned and cannot be completed",
                session.launch.issue.identifier
            )));
        }
        if !is_clean(&session.worktree)
            .map_err(|error| task_error(format!("failed to inspect Task worktree: {error}")))?
        {
            return Err(task_error(
                "Task worktree has uncommitted changes; publish or explicitly abandon them first",
            ));
        }
        let skipped_pr = if let Some(pr) = store
            .active_task_pr(&session.id)
            .await
            .map_err(|error| task_error(format!("failed to read active PR: {error}")))?
        {
            if pr.publication.is_some() {
                return Err(task_error(
                    "Task has an open pull request; merge it or run `lf pr abandon` first",
                ));
            }
            let head = rev_parse(&session.worktree, "HEAD")
                .map_err(|error| task_error(format!("failed to inspect Task HEAD: {error}")))?;
            if head != pr.base_commit {
                return Err(task_error(
                    "Task PR has unmerged commits; publish it or run `lf pr abandon` first",
                ));
            }
            Some(pr)
        } else {
            None
        };
        let from = session.status;
        session.set_status(TaskSessionStatus::Completed, summary.clone());
        reconcile_pm_writeback(&store, &mut session, None).await;
        complete_task_session_with_authority(&store, &session, skipped_pr.as_ref(), lease.as_ref())
            .await
            .map_err(|error| task_error(format!("failed to complete Task Session: {error}")))?;
        append_task_event_with_authority(
            &store,
            &session.id,
            &TaskEventKind::StatusChanged {
                from,
                to: TaskSessionStatus::Completed,
                reason: session.status_reason.clone(),
            },
            lease.as_ref(),
        )
        .await
        .map_err(|error| task_error(error.to_string()))?;
        append_task_event_with_authority(
            &store,
            &session.id,
            &TaskEventKind::Completed { summary },
            lease.as_ref(),
        )
        .await
        .map_err(|error| task_error(error.to_string()))?;
        Ok(session)
    })
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

pub(crate) async fn reconcile_pm_writeback(
    store: &SharedStore,
    session: &mut TaskSession,
    pr_url: Option<&str>,
) {
    let Ok(wave) = owning_wave(store, session).await else {
        session.pm_writeback = PmWritebackState::Pending {
            operation: PmWritebackOperation::CompleteTask,
            error: format!("owning Wave {} is not registered", session.wave_id),
        };
        return;
    };
    session.pm_writeback = writeback_state(
        crate::ops::task_pm::complete_task(
            &session.worktree,
            wave.name(),
            session.launch.issue.id.as_str(),
            pr_url,
        )
        .await,
    );
}

async fn retry_pm_writeback(store: &SharedStore, session: &mut TaskSession) {
    let Ok(prs) = store.task_prs(&session.id).await else {
        return;
    };
    let pr_url = prs
        .iter()
        .rev()
        .find_map(|pr| pr.github().map(|github| github.url.as_str()));
    let Ok(wave) = owning_wave(store, session).await else {
        session.pm_writeback = PmWritebackState::Pending {
            operation: PmWritebackOperation::CompleteTask,
            error: format!("owning Wave {} is not registered", session.wave_id),
        };
        return;
    };
    session.pm_writeback = writeback_state(
        crate::ops::task_pm::retry_complete_task(
            &session.worktree,
            wave.name(),
            session.launch.issue.id.as_str(),
            pr_url,
        )
        .await,
    );
    session.updated_at = time::OffsetDateTime::now_utc();
}

pub fn task_snapshot(session: &TaskSession) -> OpsResult<TaskSessionSnapshot> {
    let session = session.clone();
    block_on_task(async move {
        let store = task_store().await?;
        let wave = owning_wave(&store, &session).await?;
        let process_alive = if session.status.is_process_active() {
            match session.latest_process.as_ref() {
                Some(process) => tmux_session_exists(&process.tmux_name)
                    .await
                    .map_err(|error| task_error(error.to_string()))?,
                None => false,
            }
        } else {
            false
        };
        let latest_event = store
            .task_events_after(&session.id, 0)
            .await
            .map_err(|error| task_error(format!("failed to read task events: {error}")))?
            .into_iter()
            .last();
        let prs = store
            .task_prs(&session.id)
            .await
            .map_err(|error| task_error(format!("failed to read Task PRs: {error}")))?;
        let active_pr = prs.iter().find(|pr| pr.is_active()).map(|pr| pr.id.clone());
        // Resolve the live routing target for this Task's parent Project. The
        // historical project_session_id stays as provenance; the routing target
        // is its non-terminal successor when the historical session is terminal.
        // A broken chain (terminal historical, no live successor) surfaces as
        // `None` rather than failing status — the actionable failure belongs to
        // the routing operations (wake, review), not the read.
        let (routing_project_session_id, project_route_succeeded) =
            match crate::ops::project::resolve_task_project_route(store.as_ref(), &session).await {
                Ok(route) => (Some(route.current.to_string()), route.succeeded),
                Err(_) => (None, false),
            };
        Ok(TaskSessionSnapshot {
            issue_id: session.launch.issue.id.as_str().to_string(),
            issue_identifier: session.launch.issue.identifier,
            session_id: session.id.to_string(),
            project_id: session.launch.project.id.as_str().to_string(),
            project: session.launch.project.slug,
            pm_snapshot_synced_at: session.launch.pm_snapshot_synced_at,
            pm_writeback: session.pm_writeback,
            wave: wave.name().to_string(),
            project_session_id: session.project_session_id.to_string(),
            routing_project_session_id,
            project_route_succeeded,
            current_directive_version: session.current_directive_version,
            incorporated_directive_version: session.incorporated_directive_version,
            status: session.status,
            status_reason: session.status_reason,
            status_at: session.status_at,
            worktree: session.worktree.display().to_string(),
            workspace_slug: session.workspace_slug,
            lifecycle: session.lifecycle,
            lifecycle_phase: session.lifecycle_phase,
            phase_epoch: session.phase_epoch,
            phase_cursor: session.phase_cursor,
            phase_iteration: session.phase_iteration,
            gate_cycle: session.gate_cycle,
            gate_proposal: session.gate_proposal,
            prs,
            active_pr,
            agent: session.agent,
            provider: session.provider,
            provider_session_id: session.provider_session_id,
            process_alive,
            latest_process: session.latest_process,
            latest_event,
            created_at: session.created_at,
            updated_at: session.updated_at,
            observation: session.observation,
        })
    })
}

pub fn task_changes(issue: &str) -> OpsResult<TaskChangesSnapshot> {
    let session = task_status(issue)?;
    let pr = active_pr(&session)?;
    changes_snapshot(TaskWorkspace::new(&session, &pr))
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
        session_id: workspace.session_id.to_string(),
        base_commit: workspace.base_commit.to_string(),
        head_commit,
        files: files.into_values().collect(),
    })
}

pub fn task_diff(issue: &str, path: Option<&str>) -> OpsResult<TaskDiffSnapshot> {
    let session = task_status(issue)?;
    let pr = active_pr(&session)?;
    diff_snapshot(TaskWorkspace::new(&session, &pr), path)
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
        session_id: workspace.session_id.to_string(),
        path: relative,
        patch,
        binary,
        truncated,
    })
}

pub fn task_file(issue: &str, path: &str) -> OpsResult<TaskFileSnapshot> {
    let session = task_status(issue)?;
    let pr = active_pr(&session)?;
    file_snapshot(TaskWorkspace::new(&session, &pr), path)
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
        session_id: workspace.session_id.to_string(),
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

fn queue_command(issue: &str, kind: ChildCommandKind) -> OpsResult<TaskControlResult> {
    block_on_task(async move {
        let store = task_store().await?;
        let mut session = store
            .get_task_session_by_issue(issue)
            .await
            .map_err(|error| task_error(format!("failed to resolve task: {error}")))?
            .ok_or_else(|| task_error(format!("no Task Session exists for {issue:?}")))?;
        reconcile_task_pr(&store, &mut session).await?;
        reconcile_process_liveness(&store, &mut session).await?;
        let issue_id = session.launch.issue.identifier.clone();
        let observation = session.observation.clone();
        let source = command_source(&store, &session).await?;
        let result = super::child::queue_command(
            &store,
            super::child::ChildSession::Task(Box::new(session)),
            source,
            kind,
        )
        .await?;
        Ok(task_control_result(issue_id, observation, result))
    })
}

pub fn task_follow_up(issue: &str, message: String) -> OpsResult<TaskControlResult> {
    queue_command(issue, ChildCommandKind::FollowUp { text: message })
}

pub fn task_steer(issue: &str, message: String) -> OpsResult<TaskControlResult> {
    queue_command(issue, ChildCommandKind::Steer { text: message })
}

pub fn task_interrupt(issue: &str, replacement: Option<String>) -> OpsResult<TaskControlResult> {
    queue_command(issue, ChildCommandKind::Interrupt { replacement })
}

pub fn task_resume(
    issue: &str,
    message: Option<String>,
    model: Option<String>,
    reason: Option<String>,
) -> OpsResult<TaskControlResult> {
    let issue = issue.to_string();
    block_on_task(async move { resume_task_async(&issue, message, model, reason).await })
}

/// Async core of [`task_resume`], reusable from callers already inside a runtime
/// (e.g. `lf handoff complete` waking the parent it just resolved).
pub(crate) async fn resume_task_async(
    issue: &str,
    message: Option<String>,
    model: Option<String>,
    reason: Option<String>,
) -> OpsResult<TaskControlResult> {
    let store = task_store().await?;
    let mut session = store
        .get_task_session_by_issue(issue)
        .await
        .map_err(|error| task_error(format!("failed to resolve task: {error}")))?
        .ok_or_else(|| task_error(format!("no Task Session exists for {issue:?}")))?;
    // Compute every branch/worktree/PR adoption precondition before moving any
    // durable ownership — a no-active-PR recovery must not commit the successor
    // before PR rotation rejects an unrelated branch.
    task_recovery_adoption(&store, &session).await?;
    reconcile_task_pr(&store, &mut session).await?;
    // Reconcile may settle an active PR that merged out of band, moving the
    // worktree into a between-PR state; refuse a dirty between-PR before the
    // lease is reaped or a successor body is launched.
    refuse_dirty_between_prs(&store, &session).await?;
    reconcile_process_liveness(&store, &mut session).await?;
    let issue_id = session.launch.issue.identifier.clone();
    let observation = session.observation.clone();
    let source = command_source(&store, &session).await?;
    let result = super::child::resume_session(
        &store,
        super::child::ChildSession::Task(Box::new(session)),
        source,
        message,
        model,
        reason,
    )
    .await?;
    Ok(task_control_result(issue_id, observation, result))
}

pub fn task_receipt(
    command_id: &str,
    until: Option<ChildReceiptUntil>,
    timeout: Duration,
) -> OpsResult<TaskReceiptRead> {
    let command_id =
        ChildCommandId::parse(command_id).map_err(|error| task_error(error.to_string()))?;
    block_on_task(async move {
        let store = task_store().await?;
        let (command, timed_out) = if let Some(until) = until {
            super::child::wait_for_receipt_condition(&store, &command_id, until, timeout).await?
        } else {
            (
                super::child::read_receipt(&store, &command_id).await?,
                false,
            )
        };
        let ChildRef::Task(session_id) = &command.target else {
            return Err(task_error(format!(
                "command {command_id} belongs to a Project Session"
            )));
        };
        let session = store
            .get_task_session(session_id)
            .await
            .map_err(|error| task_error(format!("failed to read Task Session: {error}")))?
            .ok_or_else(|| task_error(format!("Task Session {session_id} disappeared")))?;
        let result = super::child::control_result(&store, &command, command.clone()).await?;
        Ok(TaskReceiptRead {
            receipt: task_control_result(
                session.launch.issue.identifier,
                Observation::NotRequired,
                result,
            ),
            timed_out,
        })
    })
}

pub fn task_decide(
    issue: &str,
    decision_id: &str,
    choice: String,
    message: Option<String>,
) -> OpsResult<TaskControlResult> {
    let decision_id =
        ChildDecisionId::parse(decision_id).map_err(|error| task_error(error.to_string()))?;
    let choice = choice.trim().to_string();
    if choice.is_empty() {
        return Err(task_error("decision choice cannot be empty"));
    }
    let decision = block_on_task(async {
        let store = task_store().await?;
        let session = store
            .get_task_session_by_issue(issue)
            .await
            .map_err(|error| task_error(format!("failed to resolve task: {error}")))?
            .ok_or_else(|| task_error(format!("no Task Session exists for {issue:?}")))?;
        let events = store
            .task_events_after(&session.id, 0)
            .await
            .map_err(|error| task_error(format!("failed to read task decisions: {error}")))?;
        events
            .into_iter()
            .find_map(|event| match event.kind {
                TaskEventKind::DecisionRequested {
                    decision_id: requested_id,
                    options,
                    ..
                } if requested_id == decision_id => Some(options),
                _ => None,
            })
            .ok_or_else(|| task_error(format!("decision {decision_id} was not requested")))
    })?;
    if !decision.iter().any(|option| option == &choice) {
        return Err(task_error(format!(
            "choice {choice:?} is not one of: {}",
            decision.join(", ")
        )));
    }
    queue_command(
        issue,
        ChildCommandKind::Decide {
            decision_id,
            choice,
            message,
        },
    )
}

pub fn task_request_decision(
    issue: &str,
    prompt: String,
    options: Vec<String>,
    wait: bool,
    timeout: Duration,
) -> OpsResult<TaskDecisionResult> {
    let prompt = prompt.trim().to_string();
    let options: Vec<String> = options
        .into_iter()
        .map(|option| option.trim().to_string())
        .filter(|option| !option.is_empty())
        .collect();
    if prompt.is_empty() || options.len() < 2 {
        return Err(task_error(
            "a decision requires a non-empty prompt and at least two options",
        ));
    }
    block_on_task(async move {
        let store = task_store().await?;
        let session = store
            .get_task_session_by_issue(issue)
            .await
            .map_err(|error| task_error(format!("failed to resolve task: {error}")))?
            .ok_or_else(|| task_error(format!("no Task Session exists for {issue:?}")))?;
        let ambient = std::env::var("LF_TASK_SESSION_ID")
            .map_err(|_| task_error("decision requests must run inside the owning Task Session"))?;
        if ambient != session.id.as_str() {
            return Err(task_error(format!(
                "Task Session {ambient} cannot request a decision for {}",
                session.id
            )));
        }
        let lease = task_write_lease_from_env()
            .map_err(|error| task_error(format!("Task body has no write authority: {error}")))?;
        let decision_id = ChildDecisionId::new();
        store
            .append_task_event_for_lease(
                &session.id,
                &lease,
                &TaskEventKind::DecisionRequested {
                    decision_id: decision_id.clone(),
                    prompt,
                    options,
                },
            )
            .await
            .map_err(|error| task_error(format!("failed to persist decision request: {error}")))?;

        let deadline = Instant::now() + timeout;
        loop {
            let events = store
                .task_events_after(&session.id, 0)
                .await
                .map_err(|error| {
                    task_error(format!("failed to read decision response: {error}"))
                })?;
            if let Some((choice, message)) = events.into_iter().find_map(|event| match event.kind {
                TaskEventKind::DecisionResolved {
                    decision_id: resolved_id,
                    choice,
                    message,
                } if resolved_id == decision_id => Some((choice, message)),
                _ => None,
            }) {
                return Ok(TaskDecisionResult {
                    issue_id: session.launch.issue.identifier.clone(),
                    session_id: session.id.to_string(),
                    decision_id: decision_id.to_string(),
                    resolved: true,
                    choice: Some(choice),
                    message,
                });
            }
            if !wait || Instant::now() >= deadline {
                return Ok(TaskDecisionResult {
                    issue_id: session.launch.issue.identifier.clone(),
                    session_id: session.id.to_string(),
                    decision_id: decision_id.to_string(),
                    resolved: false,
                    choice: None,
                    message: None,
                });
            }
            tokio::time::sleep(Duration::from_millis(50)).await;
        }
    })
}

pub fn task_review_reply(review_id: &str, text: String) -> OpsResult<InteractionReview> {
    let review_id = InteractionReviewId::parse(review_id)
        .map_err(|error| task_error(format!("invalid interaction review: {error}")))?;
    block_on_task(async move {
        let store = task_store().await?;
        let review = store
            .get_interaction_review(&review_id)
            .await
            .map_err(|error| task_error(error.to_string()))?
            .ok_or_else(|| task_error(format!("interaction review {review_id} not found")))?;
        let ambient = std::env::var("LF_TASK_SESSION_ID").map_err(|_| {
            task_error("interaction review replies must run inside the reviewed Task Session")
        })?;
        if ambient != review.task_session_id.as_str() {
            return Err(task_error(format!(
                "Task Session {ambient} cannot reply to review {review_id} for {}",
                review.task_session_id
            )));
        }
        let lease = task_write_lease_from_env()
            .map_err(|error| task_error(format!("Task body has no write authority: {error}")))?;
        store
            .reply_to_interaction_review(&review_id, &review.task_session_id, &lease, &text)
            .await
            .map_err(|error| task_error(error.to_string()))?;
        store
            .get_interaction_review(&review_id)
            .await
            .map_err(|error| task_error(error.to_string()))?
            .ok_or_else(|| task_error(format!("interaction review {review_id} disappeared")))
    })
}

fn _require_human_review_authority() -> OpsResult<()> {
    for variable in [
        "LF_TASK_SESSION_ID",
        "LF_PROJECT_SESSION_ID",
        "LF_RUN_ID",
        "LF_PROCESS_ID",
    ] {
        if std::env::var_os(variable).is_some() {
            return Err(task_error(
                "human review commands cannot run inside a Task, Project, or Wave agent session",
            ));
        }
    }
    Ok(())
}

pub fn task_review_message(review_id: &str, text: String) -> OpsResult<InteractionReview> {
    _require_human_review_authority()?;
    let review_id = InteractionReviewId::parse(review_id)
        .map_err(|error| task_error(format!("invalid interaction review: {error}")))?;
    block_on_task(async move {
        let store = task_store().await?;
        store
            .send_human_interaction_review_message(&review_id, ChildCommandSource::Human, &text)
            .await
            .map_err(|error| task_error(error.to_string()))?;
        store
            .get_interaction_review(&review_id)
            .await
            .map_err(|error| task_error(error.to_string()))?
            .ok_or_else(|| task_error(format!("interaction review {review_id} disappeared")))
    })
}

pub fn task_review_complete(
    review_id: &str,
    disposition: &str,
    outcome: String,
) -> OpsResult<InteractionReview> {
    _require_human_review_authority()?;
    let review_id = InteractionReviewId::parse(review_id)
        .map_err(|error| task_error(format!("invalid interaction review: {error}")))?;
    let disposition = disposition
        .replace('-', "_")
        .parse::<InteractionReviewDisposition>()
        .map_err(|error| task_error(error.to_string()))?;
    block_on_task(async move {
        let store = task_store().await?;
        store
            .complete_human_interaction_review(&review_id, disposition, &outcome)
            .await
            .map_err(|error| task_error(error.to_string()))
            .map(|(review, _)| review)
    })
}

pub fn task_acknowledge(issue: &str, version: u32, summary: String) -> OpsResult<ChildDirective> {
    let summary = summary.trim().to_string();
    if summary.is_empty() {
        return Err(task_error(
            "directive acknowledgement summary cannot be empty",
        ));
    }
    block_on_task(async move {
        let store = task_store().await?;
        let session = store
            .get_task_session_by_issue(issue)
            .await
            .map_err(|error| task_error(format!("failed to resolve task: {error}")))?
            .ok_or_else(|| task_error(format!("no Task Session exists for {issue:?}")))?;
        let ambient = std::env::var("LF_TASK_SESSION_ID").map_err(|_| {
            task_error("directive acknowledgements must run inside the owning Task Session")
        })?;
        if ambient != session.id.as_str() {
            return Err(task_error(format!(
                "Task Session {ambient} cannot acknowledge a directive for {}",
                session.id
            )));
        }
        let lease = task_write_lease_from_env()
            .map_err(|error| task_error(format!("Task body has no write authority: {error}")))?;
        let (directive, incorporated) = store
            .incorporate_child_directive_for_lease(
                &ChildRef::Task(session.id.clone()),
                &lease,
                version,
                &summary,
            )
            .await
            .map_err(|error| task_error(format!("failed to acknowledge directive: {error}")))?;
        if incorporated {
            store
                .append_task_event_for_lease(
                    &session.id,
                    &lease,
                    &TaskEventKind::DirectiveIncorporated {
                        directive_id: directive.id.clone(),
                        version,
                        summary,
                    },
                )
                .await
                .map_err(|error| task_error(error.to_string()))?;
        }
        Ok(directive)
    })
}

pub fn task_abandon(issue: &str, reason: String) -> OpsResult<TaskControlResult> {
    let reason = reason.trim();
    if reason.is_empty() {
        return Err(task_error("`lf task abandon --reason` cannot be empty"));
    }
    queue_command(
        issue,
        ChildCommandKind::Abandon {
            reason: reason.to_string(),
        },
    )
}

pub fn task_wait(
    issue: &str,
    until: TaskWaitUntil,
    timeout: Option<Duration>,
) -> OpsResult<TaskSession> {
    let started = Instant::now();
    loop {
        let session = task_status(issue)?;
        let reached = match until {
            TaskWaitUntil::Open => {
                session.status.is_terminal()
                    || active_pr(&session).is_ok_and(|pr| pr.phase() == PrPhase::Open)
            }
            TaskWaitUntil::Terminal => session.status.is_terminal(),
        };
        if reached || timeout.is_some_and(|limit| started.elapsed() >= limit) {
            return Ok(session);
        }
        std::thread::sleep(Duration::from_secs(1));
    }
}

pub fn task_attach(issue: &str) -> OpsResult<()> {
    let session = task_status(issue)?;
    if !session.status.is_process_active() {
        return Err(task_error(format!(
            "task {} is {}; resume it before attaching",
            session.launch.issue.identifier,
            session.status.as_str()
        )));
    }
    let tmux_name = session
        .latest_process
        .as_ref()
        .map(|process| process.tmux_name.as_str())
        .filter(|name| !name.is_empty())
        .ok_or_else(|| task_error("task has no attachable process; resume it first"))?;
    let status = std::process::Command::new("tmux")
        .args(["attach-session", "-t", tmux_name])
        .status()
        .map_err(|error| task_error(format!("failed to attach to task: {error}")))?;
    if !status.success() {
        return Err(task_error(format!("tmux attach failed for {tmux_name}")));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use std::io::{BufRead, BufReader};
    use std::os::unix::process::CommandExt;
    use std::path::Path;
    use std::process::Command;
    use std::sync::Arc;
    use std::time::Duration;

    use super::{
        _defer_task_interactions, cached_github_observation, changes_snapshot,
        derive_workspace_slug, diff_snapshot, ensure_working_pr,
        ensure_working_pr_with_authority, file_snapshot, next_pr_slug, parse_pr_slug,
        parse_workspace_slug, project_context, reconcile_process_liveness, reconcile_task_pr,
        recover_stalled_task_body, refuse_dirty_between_prs, refuse_if_canonical_ahead,
        require_task_pr_range_nonempty_with_authority, resolve_task_flow, resolve_upstream_base,
        task_recovery_adoption, verify_task_pr_range_with_authority, RotateOptions,
        TaskControlResult, TaskRecoveryAdoption, TaskWorkspace,
    };
    use crate::child_session::{
        observe, BodyEvidence, BodyIntent, ChildBodyOutcome, ChildCommand, ChildCommandKind,
        ChildCommandSource, ChildCommandState, ChildLeaseState, ChildProcessGeneration, ChildRef,
    };
    use crate::id::WaveId;
    use crate::pm::{PmKr, PmProject};
    use crate::project_session::{ProjectSession, ProjectSessionId, ProjectSessionStatus};
    use crate::session_context::{
        LinearIssueId, LinearIssueSnapshot, LinearProjectId, LinearProjectSnapshot,
        ProjectLaunchReceipt, TaskLaunchReceipt,
    };
    use crate::store::{open_store, SharedStore, StorageConfig};
    use crate::task::{
        AfterMerge, GithubObservation, GithubObservationResult, GithubPr, Observation,
        PmWritebackState, PrPhase, PrPublication, TaskEventKind, TaskPr, TaskPrId, TaskSession,
        TaskSessionId, TaskSessionStatus,
    };
    use crate::wave::Wave;
    use loopflow_test_support::TestRepo;
    use time::OffsetDateTime;

    fn git(repo: &Path, args: &[&str]) -> String {
        let output = Command::new("git")
            .current_dir(repo)
            .args(args)
            .output()
            .expect("run git");
        assert!(
            output.status.success(),
            "git {} failed: {}",
            args.join(" "),
            String::from_utf8_lossy(&output.stderr)
        );
        String::from_utf8_lossy(&output.stdout).trim().to_string()
    }

    #[test]
    fn task_flow_selection_accepts_skill_flows_and_rejects_ops() {
        let repo = tempfile::tempdir().unwrap();
        assert_eq!(
            resolve_task_flow(repo.path(), Some("code")).unwrap(),
            "code"
        );
        let error = resolve_task_flow(repo.path(), Some("deploy")).unwrap_err();
        assert!(error
            .to_string()
            .contains("durable Task flows currently require skills"));
    }

    /// The launch resolver ignores `LF_CONTROL_BIN`. A legacy body carries
    /// `LF_CONTROL_BIN` pointing at the (real, existing) binary that created it;
    /// launch must not relaunch through it. Here `LF_CONTROL_BIN` names a real
    /// binary while the current Home `LF_BIN` is gone: the launch fails at
    /// binary resolution, proving the control pin was never consulted (had it
    /// been, resolution would have succeeded through the real control binary).
    #[tokio::test]
    async fn launch_task_process_ignores_control_bin_and_resolves_current_home() {
        let home = tempfile::tempdir().unwrap();
        let store: SharedStore = Arc::new(
            open_store(&StorageConfig::sqlite(home.path().join("loopflow.db")))
                .await
                .unwrap(),
        );
        let now = OffsetDateTime::now_utc();
        let mut session = TaskSession {
            id: TaskSessionId::new(),
            launch: TaskLaunchReceipt {
                issue: LinearIssueSnapshot {
                    id: LinearIssueId::new("issue-no-pin").unwrap(),
                    identifier: "INF-NO-PIN".to_string(),
                    title: "Resolve through current lf".to_string(),
                    description: "Never read the pinned binary".to_string(),
                },
                project: LinearProjectSnapshot {
                    id: LinearProjectId::new("project-no-pin").unwrap(),
                    slug: "no-pin".to_string(),
                    name: "No pin".to_string(),
                    prompt_context: String::new(),
                },
                pm_snapshot_synced_at: now.unix_timestamp(),
            },
            pm_writeback: PmWritebackState::Current,
            wave_id: WaveId::new(),
            project_session_id: ProjectSessionId::new(),
            current_directive_version: 0,
            incorporated_directive_version: 0,
            status: TaskSessionStatus::Waiting,
            status_reason: "ready".to_string(),
            status_at: now,
            worktree: home.path().join("worktree"),
            workspace_slug: "task-no-pin".to_string(),
            lifecycle: crate::task::TaskLifecyclePlan::standard("task"),
            lifecycle_phase: crate::task::TaskLifecyclePhase::Iterate,
            phase_epoch: 1,
            phase_cursor: 0,
            phase_iteration: 0,
            gate_cycle: 0,
            gate_proposal: None,
            agent: "codex".to_string(),
            provider: "codex".to_string(),
            provider_session_id: None,
            latest_process: None,
            abandon_intent: None,
            created_at: now,
            updated_at: now,
            observation: crate::task::Observation::NotRequired,
        };

        // LF_CONTROL_BIN names a real, existing binary (the historical pin);
        // the current Home LF_BIN is gone. The launch must fail at resolution
        // without burning a generation or spawning tmux.
        let previous_control_bin = std::env::var_os("LF_CONTROL_BIN");
        let previous_lf_bin = std::env::var_os("LF_BIN");
        std::env::set_var("LF_CONTROL_BIN", "/bin/sh");
        std::env::set_var("LF_BIN", "/loopflow-test/does-not-exist/lf");
        let result = super::launch_task_process(&store, &mut session).await;
        match previous_lf_bin {
            Some(value) => std::env::set_var("LF_BIN", value),
            None => std::env::remove_var("LF_BIN"),
        }
        match previous_control_bin {
            Some(value) => std::env::set_var("LF_CONTROL_BIN", value),
            None => std::env::remove_var("LF_CONTROL_BIN"),
        }

        let error = result.expect_err("launch must fail when the current Home lf is missing");
        assert!(
            error
                .to_string()
                .contains("cannot resolve current lf binary"),
            "launch must resolve the current Home lf, not the LF_CONTROL_BIN pin: {error}"
        );
        // No generation was reserved: the session never started a process.
        assert!(session.latest_process.is_none());
    }

    fn changed_workspace() -> (tempfile::TempDir, String, TaskSessionId) {
        let repo = tempfile::tempdir().expect("create temp repo");
        git(repo.path(), &["init", "-b", "main"]);
        git(repo.path(), &["config", "user.name", "Loopflow Test"]);
        git(
            repo.path(),
            &["config", "user.email", "loopflow@example.com"],
        );
        std::fs::write(repo.path().join("tracked.txt"), "base\n").expect("write base");
        git(repo.path(), &["add", "tracked.txt"]);
        git(repo.path(), &["commit", "-m", "base"]);
        let base = git(repo.path(), &["rev-parse", "HEAD"]);

        std::fs::write(repo.path().join("committed.txt"), "committed\n")
            .expect("write committed file");
        git(repo.path(), &["add", "committed.txt"]);
        git(repo.path(), &["commit", "-m", "task commit"]);
        std::fs::write(repo.path().join("tracked.txt"), "staged\n").expect("write staged");
        git(repo.path(), &["add", "tracked.txt"]);
        std::fs::write(repo.path().join("tracked.txt"), "unstaged\n").expect("write unstaged");
        std::fs::write(repo.path().join("untracked.txt"), "untracked\n").expect("write untracked");

        (repo, base, TaskSessionId::new())
    }

    async fn rotation_task(
        repo: &TestRepo,
        branch: &str,
        base_commit: &str,
    ) -> (tempfile::TempDir, SharedStore, TaskSession, TaskPr) {
        rotation_task_with_lease(repo, branch, base_commit, None).await
    }

    /// Like `rotation_task`, but optionally seeds a dead lease + status — the
    /// shape an explicit resume must reconcile before reaping the lease and
    /// launching a successor. The worktree stays the real `repo.path()`.
    async fn rotation_task_with_lease(
        repo: &TestRepo,
        branch: &str,
        base_commit: &str,
        lease: Option<(crate::child_session::ChildLeaseState, TaskSessionStatus)>,
    ) -> (tempfile::TempDir, SharedStore, TaskSession, TaskPr) {
        let home = tempfile::tempdir().expect("task home");
        let store = Arc::new(
            open_store(&StorageConfig::sqlite(home.path().join("loopflow.db")))
                .await
                .expect("open store"),
        );
        let now = OffsetDateTime::now_utc();
        let lease_seed = lease.map(|(state, status)| {
            (
                status,
                ChildProcessGeneration {
                    generation: 1,
                    pid: None,
                    process_group_id: None,
                    // A name no tmux server knows, so the liveness probe reads
                    // it as dead.
                    tmux_name: format!("dead-lease-{}", WaveId::new()),
                    agent: "codex".to_string(),
                    provider: "codex".to_string(),
                    provider_session_id: None,
                    started_at: now - time::Duration::hours(1),
                    state,
                    outcome: None,
                    provenance: None,
                },
            )
        });
        let wave = Wave::new(
            WaveId::new(),
            "task-pr-rotation".to_string(),
            repo.path().display().to_string(),
        );
        let project = ProjectSession {
            id: ProjectSessionId::new(),
            launch: ProjectLaunchReceipt {
                project: LinearProjectSnapshot {
                    id: LinearProjectId::new(format!("project-{}", WaveId::new()))
                        .expect("project id"),
                    slug: "task-pr-rotation".to_string(),
                    name: "Task PR rotation".to_string(),
                    prompt_context: "Keep one stable worktree.".to_string(),
                },
                pm_snapshot_synced_at: now.unix_timestamp(),
            },
            wave_id: wave.id().clone(),
            current_directive_version: 0,
            incorporated_directive_version: 0,
            status: ProjectSessionStatus::Running,
            status_reason: "test project is running".to_string(),
            status_at: now,
            iteration: 1,
            observation_cursor: 0,
            last_state_fingerprint: None,
            agent: "codex".to_string(),
            provider: "codex".to_string(),
            provider_session_id: Some("task-pr-rotation".to_string()),
            latest_process: Some(ChildProcessGeneration {
                generation: 1,
                pid: None,
                process_group_id: None,
                tmux_name: "task-pr-rotation".to_string(),
                agent: "codex".to_string(),
                provider: "codex".to_string(),
                provider_session_id: Some("task-pr-rotation".to_string()),
                started_at: now,
                state: crate::child_session::ChildLeaseState::Active,
                outcome: None,
                provenance: None,
            }),
            abandon_intent: None,
            created_at: now,
            updated_at: now,
        };
        let session = TaskSession {
            id: TaskSessionId::new(),
            launch: TaskLaunchReceipt {
                issue: LinearIssueSnapshot {
                    id: LinearIssueId::new(format!("issue-{}", WaveId::new())).expect("issue id"),
                    identifier: "INF-ROTATE".to_string(),
                    title: "Rotate Task PRs".to_string(),
                    description: "Keep the worktree stable.".to_string(),
                },
                project: project.launch.project.clone(),
                pm_snapshot_synced_at: now.unix_timestamp(),
            },
            pm_writeback: PmWritebackState::Current,
            wave_id: wave.id().clone(),
            project_session_id: project.id.clone(),
            current_directive_version: 0,
            incorporated_directive_version: 0,
            status: lease_seed
                .as_ref()
                .map(|(status, _)| *status)
                .unwrap_or(TaskSessionStatus::Waiting),
            status_reason: if lease_seed.is_some() {
                "recovered from a vanished body".to_string()
            } else {
                "first PR settled".to_string()
            },
            status_at: now,
            worktree: repo.path().to_path_buf(),
            workspace_slug: "task-pr-proof".to_string(),
            lifecycle: crate::task::TaskLifecyclePlan::standard("task"),
            lifecycle_phase: crate::task::TaskLifecyclePhase::Iterate,
            phase_epoch: 1,
            phase_cursor: 0,
            phase_iteration: 0,
            gate_cycle: 0,
            gate_proposal: None,
            agent: "codex".to_string(),
            provider: "codex".to_string(),
            provider_session_id: None,
            latest_process: lease_seed.map(|(_, process)| process),
            abandon_intent: None,
            created_at: now,
            updated_at: now,
            observation: crate::task::Observation::NotRequired,
        };
        let pr = TaskPr {
            id: TaskPrId::new(),
            task_session_id: session.id.clone(),
            sequence: 1,
            slug: session.workspace_slug.clone(),
            branch: branch.to_string(),
            base_commit: base_commit.to_string(),
            parent_pr_id: None,
            publication: None,
            merge_commit: None,
            abandoned_at: None,
            created_at: now,
            updated_at: now,
            ci_observation: None,
            github_observation: None,
        };
        store.create_wave(&wave).await.expect("create wave");
        store
            .create_project_session(&project)
            .await
            .expect("create project");
        store
            .create_task_session(&session, &pr)
            .await
            .expect("create Task");
        (home, store, session, pr)
    }

    /// Settle a rotation Task PR as merged, optionally recording a `next_slug`.
    async fn settle_pr(store: &SharedStore, mut pr: TaskPr, merge: &str, next_slug: Option<&str>) {
        pr.publication = Some(PrPublication {
            requested_at: OffsetDateTime::now_utc(),
            after_merge: AfterMerge::Review,
            next_slug: next_slug.map(str::to_string),
            github: Some(GithubPr {
                number: 900,
                url: "https://example.com/pr/900".to_string(),
                head_sha: None,
            }),
        });
        pr.merge_commit = Some(merge.to_string());
        pr.updated_at = OffsetDateTime::now_utc();
        store.settle_task_pr(&pr, None).await.expect("settle PR");
    }

    /// Create a parent Task with a published (not merged) PR, then a child Task
    /// stacked on the parent's tip. The parent's branch is pushed so
    /// `resolve_verifier_upstream` can fetch `origin/<parent_branch>`. The child
    /// claims `repo.path()` (the verifier resolves by worktree); the parent is
    /// moved to a dummy path so both sessions coexist under the `worktree`
    /// UNIQUE constraint.
    async fn stacked_rotation_task(
        repo: &TestRepo,
        parent_branch: &str,
        child_branch: &str,
        parent_base: &str,
    ) -> (
        tempfile::TempDir,
        SharedStore,
        TaskSession,
        TaskPr,
        TaskSession,
        TaskPr,
    ) {
        // Parent: create the branch, commit work, push, then register.
        repo.create_branch(parent_branch);
        repo.create_file("parent.txt", "parent work\n");
        repo.stage_all();
        repo.commit("parent commit");
        repo.push_new_branch(parent_branch);
        let parent_tip = repo.head_sha();

        let (home, store, mut parent_session, mut parent_pr) =
            rotation_task(repo, parent_branch, parent_base).await;

        // Publish the parent PR (not merged) so the child's `parent_pr_id` is live.
        parent_pr.publication = Some(PrPublication {
            requested_at: OffsetDateTime::now_utc(),
            after_merge: AfterMerge::Review,
            next_slug: None,
            github: Some(GithubPr {
                number: 900,
                url: "https://example.com/pr/900".to_string(),
                head_sha: None,
            }),
        });
        parent_pr.updated_at = OffsetDateTime::now_utc();
        store
            .update_task_pr(&parent_pr)
            .await
            .expect("publish parent PR");

        // Move the parent off repo.path() so the child can claim it.
        parent_session.worktree = std::path::PathBuf::from("/dummy/parent-worktree");
        store
            .update_task_session(&parent_session)
            .await
            .expect("reparent parent worktree");

        // Child: cut from the parent's tip, claim the repo worktree.
        repo.checkout("main");
        git(repo.path(), &["branch", child_branch, &parent_tip]);
        repo.checkout(child_branch);

        let now = OffsetDateTime::now_utc();
        let child_session = TaskSession {
            id: TaskSessionId::new(),
            launch: TaskLaunchReceipt {
                issue: LinearIssueSnapshot {
                    id: LinearIssueId::new(format!("issue-{}", WaveId::new())).expect("issue id"),
                    identifier: "INF-STACK".to_string(),
                    title: "Stacked child".to_string(),
                    description: "Stacked on the parent.".to_string(),
                },
                project: parent_session.launch.project.clone(),
                pm_snapshot_synced_at: now.unix_timestamp(),
            },
            pm_writeback: PmWritebackState::Current,
            wave_id: parent_session.wave_id.clone(),
            project_session_id: parent_session.project_session_id.clone(),
            current_directive_version: 0,
            incorporated_directive_version: 0,
            status: TaskSessionStatus::Waiting,
            status_reason: "stacked child".to_string(),
            status_at: now,
            worktree: repo.path().to_path_buf(),
            workspace_slug: "stacked-child".to_string(),
            lifecycle: crate::task::TaskLifecyclePlan::standard("task"),
            lifecycle_phase: crate::task::TaskLifecyclePhase::Iterate,
            phase_epoch: 1,
            phase_cursor: 0,
            phase_iteration: 0,
            gate_cycle: 0,
            gate_proposal: None,
            agent: "codex".to_string(),
            provider: "codex".to_string(),
            provider_session_id: None,
            latest_process: None,
            abandon_intent: None,
            created_at: now,
            updated_at: now,
            observation: crate::task::Observation::NotRequired,
        };
        let child_pr = TaskPr {
            id: TaskPrId::new(),
            task_session_id: child_session.id.clone(),
            sequence: 1,
            slug: "stacked-child".to_string(),
            branch: child_branch.to_string(),
            base_commit: parent_tip,
            parent_pr_id: Some(parent_pr.id.clone()),
            publication: None,
            merge_commit: None,
            abandoned_at: None,
            ci_observation: None,
            github_observation: None,
            created_at: now,
            updated_at: now,
        };
        store
            .create_task_session(&child_session, &child_pr)
            .await
            .expect("create child Task");
        (
            home,
            store,
            parent_session,
            parent_pr,
            child_session,
            child_pr,
        )
    }

    #[tokio::test]
    async fn nonempty_refuses_when_head_is_the_recorded_base() {
        let repo = TestRepo::new();
        let base = repo.head_sha();

        let branch = "jack/empty-head";
        repo.create_branch(branch);
        let (_home, store, session, _pr) = rotation_task(&repo, branch, &base).await;

        let err =
            require_task_pr_range_nonempty_with_authority(&store, &session, None, repo.path())
                .await
                .expect_err("HEAD == base must refuse as empty");
        assert!(
            err.to_string().contains("empty"),
            "expected empty-range refusal, got: {err}"
        );
    }

    #[tokio::test]
    async fn nonempty_refuses_a_range_with_no_tree_change() {
        let repo = TestRepo::new();
        let base = repo.head_sha();

        let branch = "jack/no-tree-change";
        repo.create_branch(branch);
        repo.create_file("ephemeral.txt", "gone\n");
        repo.stage_all();
        repo.commit("add ephemeral");
        git(repo.path(), &["rm", "ephemeral.txt"]);
        repo.commit("remove ephemeral");

        let (_home, store, session, _pr) = rotation_task(&repo, branch, &base).await;

        let err =
            require_task_pr_range_nonempty_with_authority(&store, &session, None, repo.path())
                .await
                .expect_err("zero net tree change must refuse as empty");
        assert!(
            err.to_string().contains("empty"),
            "expected empty-range refusal for zero tree change, got: {err}"
        );
    }

    /// The core hole W2-254 closes: the old `task_pr_has_changes` guard skipped
    /// emptiness when `pr.github().is_some()`. The shared verifier is
    /// unconditional — an existing PR with an empty range is refused.
    #[tokio::test]
    async fn nonempty_refuses_even_when_the_pr_already_has_a_github_number() {
        let repo = TestRepo::new();
        let base = repo.head_sha();

        let branch = "jack/empty-existing";
        repo.create_branch(branch);
        let (_home, store, session, mut pr) = rotation_task(&repo, branch, &base).await;

        pr.publication = Some(PrPublication {
            requested_at: OffsetDateTime::now_utc(),
            after_merge: AfterMerge::Review,
            next_slug: None,
            github: Some(GithubPr {
                number: 925,
                url: "https://example.com/pr/925".to_string(),
                head_sha: None,
            }),
        });
        pr.updated_at = OffsetDateTime::now_utc();
        store.update_task_pr(&pr).await.expect("set github number");

        let err =
            require_task_pr_range_nonempty_with_authority(&store, &session, None, repo.path())
                .await
                .expect_err("an existing PR with an empty range must refuse");
        assert!(
            err.to_string().contains("empty"),
            "expected empty-range refusal despite github number, got: {err}"
        );
    }

    #[tokio::test]
    async fn nonempty_passes_for_a_real_range() {
        let repo = TestRepo::new();
        let base = repo.head_sha();

        let branch = "jack/real-range";
        repo.create_branch(branch);
        repo.create_file("task.txt", "real work\n");
        repo.stage_all();
        repo.commit("real task commit");
        let (_home, store, session, _pr) = rotation_task(&repo, branch, &base).await;

        require_task_pr_range_nonempty_with_authority(&store, &session, None, repo.path())
            .await
            .expect("a real range must pass the non-empty check");
    }

    #[tokio::test]
    async fn stacked_child_measures_from_live_parent_tip() {
        let repo = TestRepo::new();
        let origin_tip = repo.head_sha();

        let (_home, store, _, _, child_session, _) =
            stacked_rotation_task(&repo, "jack/stack-parent", "jack/stack-child", &origin_tip)
                .await;

        repo.create_file("child.txt", "child work\n");
        repo.stage_all();
        repo.commit("child commit");

        require_task_pr_range_nonempty_with_authority(&store, &child_session, None, repo.path())
            .await
            .expect("stacked child with own work passes against the parent tip");
    }

    #[tokio::test]
    async fn stacked_child_refuses_when_empty_against_live_parent() {
        let repo = TestRepo::new();
        let origin_tip = repo.head_sha();

        let (_home, store, _, _, child_session, _) = stacked_rotation_task(
            &repo,
            "jack/stack-parent-empty",
            "jack/stack-child-empty",
            &origin_tip,
        )
        .await;

        let err = require_task_pr_range_nonempty_with_authority(
            &store,
            &child_session,
            None,
            repo.path(),
        )
        .await
        .expect_err("empty stacked child must refuse against the parent tip");
        assert!(
            err.to_string().contains("empty"),
            "expected empty-range refusal for stacked child, got: {err}"
        );
    }

    #[tokio::test]
    async fn stacked_child_measures_from_origin_after_parent_collapsed() {
        let repo = TestRepo::new();
        let origin_tip = repo.head_sha();

        let (_home, store, _, mut parent_pr, child_session, mut child_pr) = stacked_rotation_task(
            &repo,
            "jack/collapse-parent",
            "jack/collapse-child",
            &origin_tip,
        )
        .await;

        repo.create_file("child.txt", "child work\n");
        repo.stage_all();
        repo.commit("child commit");

        // Parent merged: land the parent's work on origin/main.
        repo.checkout("main");
        repo.create_file("parent.txt", "parent work\n");
        repo.stage_all();
        repo.commit("merge parent into main");
        repo.push();
        let main_tip = repo.head_sha();

        // Collapse the child onto origin/main (replay only base..HEAD).
        repo.checkout("jack/collapse-child");
        git(
            repo.path(),
            &[
                "rebase",
                "--onto",
                "origin/main",
                &parent_pr.base_commit,
                "jack/collapse-child",
            ],
        );

        parent_pr.merge_commit = Some("merge-sha".to_string());
        parent_pr.updated_at = OffsetDateTime::now_utc();
        store
            .update_task_pr(&parent_pr)
            .await
            .expect("mark parent merged");

        // `base_commit` is part of `update_task_pr`'s optimistic identity, so
        // healing it forward needs the dedicated `heal_task_pr_base` write.
        child_pr.base_commit = main_tip;
        child_pr.updated_at = OffsetDateTime::now_utc();
        store
            .heal_task_pr_base(&child_pr)
            .await
            .expect("heal child base to main");

        require_task_pr_range_nonempty_with_authority(&store, &child_session, None, repo.path())
            .await
            .expect("collapsed child with own work passes against origin/main");
    }

    #[tokio::test]
    async fn stacked_child_refuses_when_empty_after_parent_collapsed() {
        let repo = TestRepo::new();
        let origin_tip = repo.head_sha();

        let (_home, store, _, mut parent_pr, child_session, mut child_pr) = stacked_rotation_task(
            &repo,
            "jack/collapse-parent-empty",
            "jack/collapse-child-empty",
            &origin_tip,
        )
        .await;

        // Parent merged: land the parent's work on origin/main.
        repo.checkout("main");
        repo.create_file("parent.txt", "parent work\n");
        repo.stage_all();
        repo.commit("merge parent into main");
        repo.push();
        let main_tip = repo.head_sha();

        // Collapse the child onto origin/main — no own work to replay.
        repo.checkout("jack/collapse-child-empty");
        git(
            repo.path(),
            &[
                "rebase",
                "--onto",
                "origin/main",
                &parent_pr.base_commit,
                "jack/collapse-child-empty",
            ],
        );

        parent_pr.merge_commit = Some("merge-sha".to_string());
        parent_pr.updated_at = OffsetDateTime::now_utc();
        store
            .update_task_pr(&parent_pr)
            .await
            .expect("mark parent merged");

        child_pr.base_commit = main_tip;
        child_pr.updated_at = OffsetDateTime::now_utc();
        store
            .heal_task_pr_base(&child_pr)
            .await
            .expect("heal child base to main");

        let err = require_task_pr_range_nonempty_with_authority(
            &store,
            &child_session,
            None,
            repo.path(),
        )
        .await
        .expect_err("empty collapsed child must refuse against origin/main");
        assert!(
            err.to_string().contains("empty"),
            "expected empty-range refusal for collapsed child, got: {err}"
        );
    }

    #[test]
    fn task_context_captures_project_definition_and_kr_state() {
        let project = PmProject {
            id: "project-1".to_string(),
            slug: "pr".to_string(),
            name: "PR".to_string(),
            summary: "Ship reliably".to_string(),
            definition: "Every task has one durable session.".to_string(),
            krs: vec![
                PmKr {
                    text: "Review resumes the same session".to_string(),
                    holds: true,
                },
                PmKr {
                    text: "Merge wakes the Wave".to_string(),
                    holds: false,
                },
            ],
            initiative_ids: vec!["initiative-1".to_string()],
            team_ids: None,
        };

        assert_eq!(
            project_context(&project),
            "Definition:\nEvery task has one durable session.\n\nKRs:\n- [x] Review resumes the same session\n- [ ] Merge wakes the Wave"
        );
    }

    #[tokio::test]
    async fn idle_task_can_defer_its_remaining_interactive_steps() {
        let repo = TestRepo::new();
        let base = repo.head_sha();
        let (_home, _store, mut session, _pr) =
            rotation_task(&repo, "jack/task-headless", &base).await;

        assert!(_defer_task_interactions(&mut session).unwrap());
        assert!(session.lifecycle.all_interactions_deferred());
        assert!(!_defer_task_interactions(&mut session).unwrap());

        session.lifecycle = crate::task::TaskLifecyclePlan::standard("task");
        session.status = TaskSessionStatus::Completed;
        assert!(_defer_task_interactions(&mut session)
            .unwrap_err()
            .to_string()
            .contains("terminal Tasks cannot change interaction policy"));
    }

    /// Seed a second Task under the rotation scaffolding whose durable state is a
    /// non-active status still carrying a dead lease — the exact shape an explicit
    /// resume must reconcile before it can reserve a fresh body.
    async fn dead_lease_task(
        repo: &TestRepo,
        branch: &str,
        base: &str,
        status: TaskSessionStatus,
        lease_state: crate::child_session::ChildLeaseState,
    ) -> (tempfile::TempDir, SharedStore, TaskSession) {
        let (home, store, base_session, _pr) = rotation_task(repo, branch, base).await;
        let now = OffsetDateTime::now_utc();
        let mut session = base_session.clone();
        session.id = TaskSessionId::new();
        session.workspace_slug = "dead-lease-proof".to_string();
        // Distinct worktree and issue: both columns are UNIQUE and the base Task
        // already holds the repo root and the rotation issue.
        session.worktree = repo.path().join(format!("dead-{}", session.id));
        session.launch.issue.id =
            LinearIssueId::new(format!("issue-{}", session.id)).expect("issue id");
        session.launch.issue.identifier = format!("INF-DEAD-{}", session.id);
        session.set_status(status, "recovered from a vanished body");
        session.latest_process = Some(ChildProcessGeneration {
            generation: 1,
            pid: None,
            process_group_id: None,
            // A name no tmux server knows, so the liveness probe reads it as dead.
            tmux_name: format!("dead-lease-{}", session.id),
            agent: session.agent.clone(),
            provider: session.provider.clone(),
            provider_session_id: None,
            started_at: now - time::Duration::hours(1),
            state: lease_state,
            outcome: None,
            provenance: None,
        });
        let pr = TaskPr {
            id: TaskPrId::new(),
            task_session_id: session.id.clone(),
            sequence: 1,
            slug: session.workspace_slug.clone(),
            branch: format!("{branch}-dead"),
            base_commit: base.to_string(),
            parent_pr_id: None,
            publication: None,
            merge_commit: None,
            abandoned_at: None,
            created_at: now,
            updated_at: now,
            ci_observation: None,
            github_observation: None,
        };
        store
            .create_task_session(&session, &pr)
            .await
            .expect("create dead-lease Task");
        (home, store, session)
    }

    #[tokio::test]
    async fn resume_revokes_a_dead_legacy_lease_on_a_waiting_task() {
        // W2-135: a Waiting Task still pinned by a Legacy lease whose body vanished.
        let repo = TestRepo::new();
        let base = repo.head_sha();
        let (_home, store, mut session) = dead_lease_task(
            &repo,
            "jack/w2-135",
            &base,
            TaskSessionStatus::Waiting,
            crate::child_session::ChildLeaseState::Legacy,
        )
        .await;

        reconcile_process_liveness(&store, &mut session)
            .await
            .expect("reconcile a waiting task with a dead legacy lease");

        // The dead lease is reaped so the resume can reserve a fresh body...
        assert_eq!(
            session.latest_process.as_ref().map(|process| process.state),
            Some(crate::child_session::ChildLeaseState::Finished)
        );
        // ...while the Session keeps its Waiting status for the resume that follows.
        assert_eq!(session.status, TaskSessionStatus::Waiting);

        let persisted = store.get_task_session(&session.id).await.unwrap().unwrap();
        assert_eq!(
            persisted.latest_process.map(|process| process.state),
            Some(crate::child_session::ChildLeaseState::Finished)
        );
        assert_eq!(persisted.status, TaskSessionStatus::Waiting);
    }

    #[tokio::test]
    async fn resume_revokes_a_dead_active_lease_on_a_failed_task() {
        // W2-122: a Failed Task still holding an Active lease whose body vanished.
        let repo = TestRepo::new();
        let base = repo.head_sha();
        let (_home, store, mut session) = dead_lease_task(
            &repo,
            "jack/w2-122",
            &base,
            TaskSessionStatus::Failed,
            crate::child_session::ChildLeaseState::Active,
        )
        .await;

        reconcile_process_liveness(&store, &mut session)
            .await
            .expect("reconcile a failed task with a dead active lease");

        assert_eq!(
            session.latest_process.as_ref().map(|process| process.state),
            Some(crate::child_session::ChildLeaseState::Finished)
        );
        assert_eq!(session.status, TaskSessionStatus::Failed);

        let persisted = store.get_task_session(&session.id).await.unwrap().unwrap();
        assert_eq!(
            persisted.latest_process.map(|process| process.state),
            Some(crate::child_session::ChildLeaseState::Finished)
        );
        assert_eq!(persisted.status, TaskSessionStatus::Failed);
    }

    #[tokio::test]
    async fn stalled_delivery_is_reaped_and_waits_without_replay() {
        let repo = TestRepo::new();
        let base = repo.head_sha();
        let (_home, store, mut session, _pr) =
            rotation_task(&repo, "jack/stalled-delivery", &base).await;
        session.begin_generation(format!("stalled-body-{}", session.id));
        let lease = store
            .reserve_task_process(&session, TaskSessionStatus::Waiting)
            .await
            .unwrap()
            .expect("reserve stalled body");

        let mut child = std::process::Command::new("/bin/sh")
            .arg("-c")
            .arg("sleep 60 & echo $!; wait")
            .process_group(0)
            .stdout(std::process::Stdio::piped())
            .spawn()
            .expect("spawn stalled process group");
        let group = child.id();
        let stdout = child.stdout.take().expect("capture grandchild pid");
        let mut line = String::new();
        BufReader::new(stdout)
            .read_line(&mut line)
            .expect("read grandchild pid");
        let grandchild: u32 = line.trim().parse().expect("grandchild pid");
        let waiter = std::thread::spawn(move || child.wait().expect("reap shell"));

        let process = session.latest_process.as_mut().expect("reserved process");
        process.pid = Some(group);
        process.process_group_id = Some(group);
        process.state = ChildLeaseState::Active;
        session.set_status(TaskSessionStatus::Running, "fake provider is alive");
        store.activate_task_process(&session, &lease).await.unwrap();

        let mut command = ChildCommand::new(
            ChildRef::Task(session.id.clone()),
            ChildCommandSource::Human,
            ChildCommandKind::Steer {
                text: "do the external thing".to_string(),
            },
        );
        store.create_child_command(&command).await.unwrap();
        let claimed = store
            .claim_child_commands_for_lease(&command.target, &lease)
            .await
            .unwrap();
        command = claimed.into_iter().next().expect("claimed command");
        store
            .mark_child_command_delivering_for_lease(
                &command.target,
                &lease,
                &command.id,
                crate::child_session::ChildCommandEffect::LiveSteer,
            )
            .await
            .unwrap();
        session = store.get_task_session(&session.id).await.unwrap().unwrap();

        let observation = observe(
            &BodyEvidence {
                intent: BodyIntent::Active,
                observable: true,
                process_alive: true,
                progress_age: Duration::from_secs(31 * 60),
                step: Some("task_pursue".to_string()),
                reason: "fake provider is alive".to_string(),
            },
            Duration::from_secs(30 * 60),
        );
        let latest_event_id = store
            .latest_task_event(&session.id)
            .await
            .unwrap()
            .map(|event| event.id);
        assert!(
            recover_stalled_task_body(&store, session.clone(), &observation, latest_event_id,)
                .await
                .unwrap()
        );
        waiter.join().unwrap();

        // SAFETY: signal 0 is an existence probe and uses no pointers.
        assert_ne!(unsafe { libc::kill(group as i32, 0) }, 0);
        // SAFETY: signal 0 is an existence probe and uses no pointers.
        assert_ne!(unsafe { libc::kill(grandchild as i32, 0) }, 0);
        let persisted = store.get_task_session(&session.id).await.unwrap().unwrap();
        assert_eq!(persisted.status, TaskSessionStatus::Waiting);
        assert_eq!(
            persisted.latest_process.map(|process| process.state),
            Some(ChildLeaseState::Finished)
        );
        assert!(persisted.status_reason.contains(command.id.as_str()));
        let receipt = store.get_child_command(&command.id).await.unwrap().unwrap();
        assert_eq!(receipt.state, ChildCommandState::Uncertain);
    }

    #[tokio::test]
    async fn progress_wins_the_race_against_stall_recovery() {
        let repo = TestRepo::new();
        let base = repo.head_sha();
        let (_home, store, mut session, _pr) =
            rotation_task(&repo, "jack/progress-race", &base).await;
        session.begin_generation(format!("progress-race-{}", session.id));
        let lease = store
            .reserve_task_process(&session, TaskSessionStatus::Waiting)
            .await
            .unwrap()
            .expect("reserve body");
        session
            .latest_process
            .as_mut()
            .expect("reserved process")
            .state = ChildLeaseState::Active;
        session.set_status(TaskSessionStatus::Running, "provider is alive");
        store.activate_task_process(&session, &lease).await.unwrap();
        session = store.get_task_session(&session.id).await.unwrap().unwrap();
        let observed_event_id = store
            .latest_task_event(&session.id)
            .await
            .unwrap()
            .map(|event| event.id);

        store
            .append_task_event(
                &session.id,
                &TaskEventKind::Progress {
                    summary: "body advanced before revocation".to_string(),
                },
            )
            .await
            .unwrap();
        let revoked = store
            .revoke_task_process_if_unchanged(
                &session.id,
                1,
                session.status_at,
                observed_event_id,
                &ChildBodyOutcome::Superseded {
                    reason: "stale observation".to_string(),
                },
            )
            .await
            .unwrap();

        assert!(revoked.is_none());
        let persisted = store.get_task_session(&session.id).await.unwrap().unwrap();
        assert_eq!(
            persisted.latest_process.map(|process| process.state),
            Some(ChildLeaseState::Active),
        );
        assert_eq!(persisted.status, TaskSessionStatus::Running);
    }

    #[test]
    fn readable_task_names_are_semantic_and_bounded() {
        assert_eq!(
            derive_workspace_slug("Release scoped migrations across every target")
                .unwrap()
                .as_str(),
            "release-scoped-migrations-across-every"
        );
        assert_eq!(
            derive_workspace_slug("Investigate").unwrap().as_str(),
            "investigate-task"
        );
        assert!(parse_workspace_slug("one").is_err());
        assert!(parse_workspace_slug("release_scoped-migrations").is_err());
        assert_eq!(
            parse_pr_slug("released-upgrade-proof").unwrap(),
            "released-upgrade-proof"
        );
        assert!(parse_pr_slug("released/upgrade").is_err());
    }

    #[tokio::test]
    async fn settled_pr_rotates_the_same_worktree_from_fetched_main() {
        let repo = TestRepo::new();
        let base = repo.head_sha();
        let first_branch = "jack/task-pr-proof";
        repo.create_branch(first_branch);
        repo.create_file("first.txt", "first PR\n");
        repo.stage_all();
        repo.commit("first PR");
        let (_home, store, mut session, mut first) =
            rotation_task(&repo, first_branch, &base).await;
        first.publication = Some(PrPublication {
            requested_at: OffsetDateTime::now_utc(),
            after_merge: AfterMerge::Review,
            next_slug: Some("follow-up-proof".to_string()),
            github: Some(GithubPr {
                number: 911,
                url: "https://example.com/pr/911".to_string(),
                head_sha: None,
            }),
        });
        first.merge_commit = Some("merge-911".to_string());
        first.updated_at = OffsetDateTime::now_utc();
        store
            .settle_task_pr(&first, None)
            .await
            .expect("settle first PR");

        let second = ensure_working_pr(&store, &mut session)
            .await
            .expect("rotate PR")
            .expect("working PR");

        assert_eq!(session.worktree, repo.path());
        assert_eq!(second.sequence, 2);
        assert_eq!(second.branch, "jack/task-pr-proof-follow-up-proof");
        assert_eq!(second.base_commit, base);
        assert_eq!(second.phase(), PrPhase::Working);
        assert_eq!(
            git(repo.path(), &["rev-parse", "--abbrev-ref", "HEAD"]),
            second.branch
        );
        let prs = store.task_prs(&session.id).await.expect("read PR history");
        assert_eq!(prs.iter().map(|pr| pr.sequence).collect::<Vec<_>>(), [1, 2]);
        assert_eq!(prs[0].phase(), PrPhase::Merged);
        assert_eq!(prs[1].id, second.id);
    }

    #[tokio::test]
    async fn rotate_forward_carries_uncommitted_follow_up_edits() {
        let repo = TestRepo::new();
        let base = repo.head_sha();
        let first_branch = "jack/task-pr-proof";
        repo.create_branch(first_branch);
        repo.create_file("first.txt", "first PR\n");
        repo.stage_all();
        repo.commit("first PR");
        let (_home, store, mut session, mut first) =
            rotation_task(&repo, first_branch, &base).await;
        first.publication = Some(PrPublication {
            requested_at: OffsetDateTime::now_utc(),
            after_merge: AfterMerge::Review,
            next_slug: None,
            github: Some(GithubPr {
                number: 907,
                url: "https://example.com/pr/907".to_string(),
                head_sha: None,
            }),
        });
        first.merge_commit = Some("merge-907".to_string());
        first.updated_at = OffsetDateTime::now_utc();
        store
            .settle_task_pr(&first, None)
            .await
            .expect("settle first PR");

        // The worker stopped before pushing: follow-up work sits uncommitted.
        repo.create_file("follow-up.txt", "second PR work\n");

        // The runner's strict path refuses to rotate over a dirty tree.
        assert!(ensure_working_pr(&store, &mut session.clone())
            .await
            .is_err());

        let second = ensure_working_pr_with_authority(
            &store,
            &mut session,
            None,
            RotateOptions {
                carry_dirty: true,
                slug_override: Some("keep-going".to_string()),
            },
        )
        .await
        .expect("rotate forward")
        .expect("working PR");

        assert_eq!(second.sequence, 2);
        assert_eq!(second.branch, "jack/task-pr-proof-keep-going");
        assert_eq!(second.base_commit, base);
        assert_eq!(second.phase(), PrPhase::Working);
        assert_eq!(
            git(repo.path(), &["rev-parse", "--abbrev-ref", "HEAD"]),
            second.branch
        );
        // The preserved follow-up edit survived the rotation onto the new branch.
        assert_eq!(
            std::fs::read_to_string(repo.path().join("follow-up.txt")).expect("follow-up survives"),
            "second PR work\n"
        );
        let prs = store.task_prs(&session.id).await.expect("read PR history");
        assert_eq!(prs.iter().map(|pr| pr.sequence).collect::<Vec<_>>(), [1, 2]);
        assert_eq!(prs[0].phase(), PrPhase::Merged);
    }

    #[tokio::test]
    async fn reconcile_degrades_and_preserves_cache_when_the_github_read_fails() {
        // A published PR whose remote can't be read (TestRepo's origin is a local
        // bare repo, not GitHub, so the read cannot resolve) must not error
        // reconcile: the cached row stands and freshness degrades. This is what
        // keeps follow-up/steer/status usable during a quota or network outage —
        // each funnels through reconcile, which previously `?`-failed on the read.
        let repo = TestRepo::new();
        let base = repo.head_sha();
        let branch = "jack/task-pr-proof";
        repo.create_branch(branch);
        repo.create_file("first.txt", "first PR\n");
        repo.stage_all();
        repo.commit("first PR");
        let (_home, store, mut session, mut pr) = rotation_task(&repo, branch, &base).await;
        pr.publication = Some(PrPublication {
            requested_at: OffsetDateTime::now_utc(),
            after_merge: AfterMerge::Review,
            next_slug: None,
            github: Some(GithubPr {
                number: 914,
                url: "https://example.com/pr/914".to_string(),
                head_sha: None,
            }),
        });
        pr.updated_at = OffsetDateTime::now_utc();
        store.update_task_pr(&pr).await.expect("publish PR");

        let observed = reconcile_task_pr(&store, &mut session)
            .await
            .expect("reconcile does not error on a failed GitHub read")
            .expect("the cached PR is preserved");

        // Cache stands: still Open, no merge fabricated from a failed read.
        assert_eq!(observed.phase(), PrPhase::Open);
        assert_eq!(observed.merge_commit, None);
        match &session.observation {
            Observation::Degraded { reason, .. } => assert!(!reason.is_empty()),
            other => panic!("a failed GitHub read must degrade freshness, got {other:?}"),
        }
    }

    #[tokio::test]
    async fn reconcile_skips_the_remote_read_for_an_unpublished_working_pr() {
        // A working PR has no persisted number; reconcile must neither enumerate
        // nor read remotely. Proof: a read here WOULD fail (no GitHub origin), yet
        // freshness says no read was required.
        let repo = TestRepo::new();
        let base = repo.head_sha();
        let branch = "jack/task-pr-proof";
        repo.create_branch(branch);
        let (_home, store, mut session, pr) = rotation_task(&repo, branch, &base).await;
        assert!(pr.github().is_none(), "fixture PR is unpublished");

        let observed = reconcile_task_pr(&store, &mut session)
            .await
            .expect("reconcile succeeds")
            .expect("working PR preserved");

        assert_eq!(observed.phase(), PrPhase::Working);
        assert_eq!(session.observation, Observation::NotRequired);
    }

    #[test]
    fn github_observation_cache_expires_fresh_reads_before_degraded_circuits() {
        let now = OffsetDateTime::now_utc();
        let mut pr = TaskPr {
            id: TaskPrId::new(),
            task_session_id: TaskSessionId::new(),
            sequence: 1,
            slug: "cache-proof".to_string(),
            branch: "jack/cache-proof".to_string(),
            base_commit: "base".to_string(),
            parent_pr_id: None,
            publication: None,
            merge_commit: None,
            abandoned_at: None,
            ci_observation: None,
            github_observation: Some(GithubObservation {
                checked_at: now - time::Duration::seconds(59),
                result: GithubObservationResult::Fresh,
            }),
            created_at: now,
            updated_at: now - time::Duration::hours(1),
        };
        assert!(matches!(
            cached_github_observation(&pr, now),
            Some(Observation::Cached { .. })
        ));
        pr.github_observation.as_mut().unwrap().checked_at = now - time::Duration::seconds(60);
        assert_eq!(cached_github_observation(&pr, now), None);

        pr.github_observation = Some(GithubObservation {
            checked_at: now - time::Duration::minutes(4),
            result: GithubObservationResult::Degraded {
                reason: "rate limit exhausted".to_string(),
            },
        });
        assert!(matches!(
            cached_github_observation(&pr, now),
            Some(Observation::Degraded { .. })
        ));
        pr.github_observation.as_mut().unwrap().checked_at = now - time::Duration::minutes(5);
        assert_eq!(cached_github_observation(&pr, now), None);
    }

    #[tokio::test]
    async fn rotate_carries_committed_follow_up_and_dirty_edits_after_an_out_of_band_merge() {
        // W2-166 shape: PR merged out of band, then the worker committed *unique*
        // follow-up work on the settled branch plus left a dirty edit. Rotating to
        // the next serial PR must carry BOTH the committed range and the dirty
        // edit — and never re-apply the already-merged work.
        let repo = TestRepo::new();
        let base = repo.head_sha();
        let settled_branch = "jack/task-pr-proof";
        repo.create_branch(settled_branch);
        repo.create_file("merged.txt", "merged work\n");
        repo.stage_all();
        repo.commit("merged work");
        let merged_tip = repo.head_sha();
        // The merge landed on main (simulated by advancing origin/main to the tip
        // GitHub merged), so the rotated branch bases on main which already
        // carries the merged work.
        git(repo.path(), &["push", "origin", "jack/task-pr-proof:main"]);

        // Post-merge follow-up: two committed commits, then an uncommitted edit.
        repo.create_file("follow1.txt", "follow-up one\n");
        repo.stage_all();
        repo.commit("follow-up one");
        repo.create_file("follow2.txt", "follow-up two\n");
        repo.stage_all();
        repo.commit("follow-up two");
        repo.create_file("wip.txt", "uncommitted work\n");

        let (_home, store, mut session, mut settled) =
            rotation_task(&repo, settled_branch, &base).await;
        settled.publication = Some(PrPublication {
            requested_at: OffsetDateTime::now_utc(),
            after_merge: AfterMerge::Review,
            next_slug: Some("keep-going".to_string()),
            github: Some(GithubPr {
                number: 907,
                url: "https://example.com/pr/907".to_string(),
                // The merged branch tip — the cut between merged work and follow-up.
                head_sha: Some(merged_tip.clone()),
            }),
        });
        settled.merge_commit = Some("merge-907".to_string());
        settled.updated_at = OffsetDateTime::now_utc();
        store
            .settle_task_pr(&settled, None)
            .await
            .expect("settle merged PR");

        let next = ensure_working_pr_with_authority(
            &store,
            &mut session,
            None,
            RotateOptions {
                carry_dirty: true,
                slug_override: Some("keep-going".to_string()),
            },
        )
        .await
        .expect("rotate forward")
        .expect("working PR");

        assert_eq!(next.sequence, 2);
        assert_eq!(
            git(repo.path(), &["rev-parse", "--abbrev-ref", "HEAD"]),
            next.branch
        );
        // Committed follow-up carried forward.
        assert_eq!(
            std::fs::read_to_string(repo.path().join("follow1.txt")).expect("follow1 carried"),
            "follow-up one\n"
        );
        assert_eq!(
            std::fs::read_to_string(repo.path().join("follow2.txt")).expect("follow2 carried"),
            "follow-up two\n"
        );
        // Dirty edit carried forward, still uncommitted.
        assert_eq!(
            std::fs::read_to_string(repo.path().join("wip.txt")).expect("dirty edit carried"),
            "uncommitted work\n"
        );
        // The already-merged work lives in the base, not re-applied as a commit:
        // exactly the two follow-up commits sit beyond origin/main.
        let beyond = git(
            repo.path(),
            &["log", "origin/main..HEAD", "--oneline", "--format=%s"],
        );
        let subjects: Vec<&str> = beyond.lines().collect();
        assert_eq!(subjects, vec!["follow-up two", "follow-up one"]);
        // The merged work is present exactly once (from the base), not duplicated.
        assert!(repo.path().join("merged.txt").exists());
    }

    #[tokio::test]
    async fn failed_committed_carry_restores_the_settled_branch_for_retry() {
        let repo = TestRepo::new();
        let base = repo.head_sha();
        let settled_branch = "jack/task-pr-proof";
        repo.create_branch(settled_branch);
        repo.create_file("shared.txt", "merged\n");
        repo.stage_all();
        repo.commit("merged work");
        let merged_tip = repo.head_sha();

        // Main and the post-merge follow-up edit the same line differently, so
        // carrying the follow-up onto current main must conflict.
        git(repo.path(), &["checkout", "-b", "main-update", &merged_tip]);
        repo.create_file("shared.txt", "main moved on\n");
        repo.stage_all();
        repo.commit("main update");
        git(repo.path(), &["push", "origin", "main-update:main"]);
        git(repo.path(), &["checkout", settled_branch]);
        repo.create_file("shared.txt", "follow-up edit\n");
        repo.stage_all();
        repo.commit("post-merge follow-up");
        repo.create_file("wip.txt", "dirty follow-up\n");

        let (_home, store, mut session, mut settled) =
            rotation_task(&repo, settled_branch, &base).await;
        settled.publication = Some(PrPublication {
            requested_at: OffsetDateTime::now_utc(),
            after_merge: AfterMerge::Review,
            next_slug: None,
            github: Some(GithubPr {
                number: 918,
                url: "https://example.com/pr/918".to_string(),
                head_sha: Some(merged_tip),
            }),
        });
        settled.merge_commit = Some("merge-918".to_string());
        settled.updated_at = OffsetDateTime::now_utc();
        store
            .settle_task_pr(&settled, None)
            .await
            .expect("settle merged PR");

        let error = ensure_working_pr_with_authority(
            &store,
            &mut session,
            None,
            RotateOptions {
                carry_dirty: true,
                slug_override: Some("retry".to_string()),
            },
        )
        .await
        .expect_err("conflicting carry must fail");

        assert!(error.to_string().contains("restored"));
        assert_eq!(
            git(repo.path(), &["rev-parse", "--abbrev-ref", "HEAD"]),
            settled_branch
        );
        assert_eq!(
            std::fs::read_to_string(repo.path().join("shared.txt")).expect("commit restored"),
            "follow-up edit\n"
        );
        assert_eq!(
            std::fs::read_to_string(repo.path().join("wip.txt")).expect("dirty edit restored"),
            "dirty follow-up\n"
        );
        assert_eq!(
            git(repo.path(), &["branch", "--list", "*/task-pr-proof-retry"]),
            ""
        );
        assert_eq!(git(repo.path(), &["stash", "list"]), "");
    }

    #[test]
    fn task_control_json_reports_durable_state_and_effect() {
        let result = TaskControlResult {
            issue_id: "INF-123".to_string(),
            session_id: "ts_example".to_string(),
            command_id: "cc_example".to_string(),
            directive_version: Some(2),
            state: crate::child_session::ChildCommandState::Accepted,
            effect: Some(crate::child_session::ChildCommandEffect::LiveSteer),
            incorporated: true,
            generation: Some(2),
            accepted_at: None,
            incorporated_at: None,
            error: None,
            observation: Observation::NotRequired,
        };

        assert_eq!(
            serde_json::to_value(result).unwrap(),
            serde_json::json!({
                "issue_id": "INF-123",
                "session_id": "ts_example",
                "command_id": "cc_example",
                "directive_version": 2,
                "state": "accepted",
                "effect": "live_steer",
                "incorporated": true,
                "generation": 2,
                "accepted_at": null,
                "incorporated_at": null,
                "error": null,
                "observation": {"freshness": "not_required"},
            })
        );
    }

    #[test]
    fn task_workspace_reports_committed_staged_unstaged_and_untracked_files() {
        let (repo, base, session_id) = changed_workspace();
        let workspace = TaskWorkspace {
            issue_identifier: "INF-123",
            session_id: &session_id,
            worktree: repo.path(),
            base_commit: &base,
        };

        let snapshot = changes_snapshot(workspace).expect("inspect task changes");
        let committed = snapshot
            .files
            .iter()
            .find(|file| file.path == "committed.txt")
            .expect("committed file");
        assert!(committed.committed);
        let tracked = snapshot
            .files
            .iter()
            .find(|file| file.path == "tracked.txt")
            .expect("tracked file");
        assert!(tracked.staged && tracked.unstaged);
        let untracked = snapshot
            .files
            .iter()
            .find(|file| file.path == "untracked.txt")
            .expect("untracked file");
        assert!(untracked.untracked);

        let patch = diff_snapshot(workspace, None).expect("inspect task diff");
        assert!(patch.patch.contains("committed.txt"));
        assert!(patch.patch.contains("tracked.txt"));
        assert!(patch.patch.contains("untracked.txt"));

        let untracked_patch =
            diff_snapshot(workspace, Some("./untracked.txt")).expect("inspect one file");
        assert_eq!(untracked_patch.path.as_deref(), Some("untracked.txt"));
        assert!(untracked_patch.patch.contains("+untracked"));
    }

    #[test]
    fn task_file_reads_only_files_inside_the_task_worktree() {
        let (repo, base, session_id) = changed_workspace();
        let workspace = TaskWorkspace {
            issue_identifier: "INF-123",
            session_id: &session_id,
            worktree: repo.path(),
            base_commit: &base,
        };

        let file = file_snapshot(workspace, "./tracked.txt").expect("read task file");
        assert_eq!(file.path, "tracked.txt");
        assert_eq!(file.content.as_deref(), Some("unstaged\n"));
        assert!(file_snapshot(workspace, "../outside.txt").is_err());

        #[cfg(unix)]
        {
            let outside = tempfile::NamedTempFile::new().expect("outside file");
            std::os::unix::fs::symlink(outside.path(), repo.path().join("outside-link"))
                .expect("create outside symlink");
            assert!(file_snapshot(workspace, "outside-link").is_err());
        }
    }

    #[tokio::test]
    async fn verify_refuses_a_base_carrying_a_foreign_commit() {
        // The #877/#882 shape: the branch was cut from a local main that carried
        // an unpushed commit, so the recorded base itself is off-origin.
        let repo = TestRepo::new();
        // Advance local main ahead of origin with a foreign commit; never push it.
        repo.create_file("foreign.txt", "not this task's work\n");
        repo.stage_all();
        repo.commit("foreign canonical-main commit");
        let contaminated_base = repo.head_sha();

        let branch = "jack/contaminated";
        repo.create_branch(branch);
        repo.create_file("task.txt", "task work\n");
        repo.stage_all();
        repo.commit("task commit");
        let (_home, store, session, _pr) = rotation_task(&repo, branch, &contaminated_base).await;

        let err = verify_task_pr_range_with_authority(&store, &session, None, repo.path())
            .await
            .expect_err("contaminated base must refuse");
        let message = err.to_string();
        assert!(
            message.contains("contaminated"),
            "expected contamination refusal, got: {message}"
        );
        assert!(
            message.contains("foreign canonical-main commit"),
            "refusal must name the foreign commit, got: {message}"
        );
        assert!(
            message.contains("rebase --onto"),
            "refusal must print the recovery action, got: {message}"
        );
    }

    #[tokio::test]
    async fn verify_heals_a_stale_base_after_origin_advances() {
        let repo = TestRepo::new();
        let stale_base = repo.head_sha(); // origin/main at placement time

        // origin advances: land a commit on main and push it.
        repo.create_file("upstream.txt", "landed upstream\n");
        repo.stage_all();
        repo.commit("upstream advance");
        repo.push();
        let advanced = repo.head_sha();

        // The Task branch already sits on the advanced origin (e.g. after an
        // lf pr open rebase), but the recorded base is still the pre-advance sha.
        let branch = "jack/stale-base";
        repo.create_branch(branch);
        repo.create_file("task.txt", "task work\n");
        repo.stage_all();
        repo.commit("task commit");
        let (_home, store, session, _pr) = rotation_task(&repo, branch, &stale_base).await;

        verify_task_pr_range_with_authority(&store, &session, None, repo.path())
            .await
            .expect("stale-but-compatible base verifies");

        let healed = store
            .active_task_pr(&session.id)
            .await
            .expect("read active PR")
            .expect("active PR exists");
        assert_eq!(
            healed.base_commit, advanced,
            "the stale base should heal forward to the current fork point"
        );
    }

    #[tokio::test]
    async fn verify_falls_back_to_local_main_without_a_remote() {
        let repo = TestRepo::new();
        let base = repo.head_sha();
        // Drop the remote entirely: placement fell back to local main.
        git(repo.path(), &["remote", "remove", "origin"]);

        let branch = "jack/no-remote";
        repo.create_branch(branch);
        repo.create_file("task.txt", "task work\n");
        repo.stage_all();
        repo.commit("task commit");
        let (_home, store, session, _pr) = rotation_task(&repo, branch, &base).await;

        verify_task_pr_range_with_authority(&store, &session, None, repo.path())
            .await
            .expect("no-remote repo verifies against local main");
    }

    #[tokio::test]
    async fn verify_passes_for_a_rotated_continuation_pr() {
        let repo = TestRepo::new();
        let base = repo.head_sha();
        let first_branch = "jack/task-pr-proof";
        repo.create_branch(first_branch);
        repo.create_file("first.txt", "first PR\n");
        repo.stage_all();
        repo.commit("first PR");
        let (_home, store, mut session, mut first) =
            rotation_task(&repo, first_branch, &base).await;
        first.publication = Some(PrPublication {
            requested_at: OffsetDateTime::now_utc(),
            after_merge: AfterMerge::Review,
            next_slug: Some("follow-up".to_string()),
            github: Some(GithubPr {
                number: 938,
                url: "https://example.com/pr/938".to_string(),
                head_sha: None,
            }),
        });
        first.merge_commit = Some("merge-938".to_string());
        first.updated_at = OffsetDateTime::now_utc();
        store
            .settle_task_pr(&first, None)
            .await
            .expect("settle first PR");

        // Rotation cuts PR2 from fetched origin/main; its recorded base is the
        // fork point, so parity holds by construction.
        let second = ensure_working_pr(&store, &mut session)
            .await
            .expect("rotate PR")
            .expect("working PR");
        assert_eq!(second.sequence, 2);
        repo.create_file("second.txt", "second PR work\n");
        repo.stage_all();
        repo.commit("second PR commit");

        verify_task_pr_range_with_authority(&store, &session, None, repo.path())
            .await
            .expect("rotated continuation PR verifies");
    }

    #[tokio::test]
    async fn verify_refuses_divergent_ancestry_naming_both_sides() {
        let repo = TestRepo::new();
        let origin_tip = repo.head_sha(); // P

        // A foreign commit advances local main ahead of origin (not pushed).
        repo.create_file("foreign.txt", "not this task's work\n");
        repo.stage_all();
        repo.commit("foreign canonical-main commit");
        let contaminated_base = repo.head_sha(); // F = P → F

        // Cut the task branch from the contaminated base.
        let branch = "jack/divergent";
        repo.create_branch(branch);
        repo.create_file("task.txt", "task work\n");
        repo.stage_all();
        repo.commit("task commit");

        // Undo the foreign commit on main and advance origin with a *different*
        // commit so the recorded base and origin diverge from P.
        repo.checkout("main");
        git(repo.path(), &["reset", "--hard", &origin_tip]);
        repo.create_file("upstream.txt", "landed upstream\n");
        repo.stage_all();
        repo.commit("upstream advance");
        repo.push(); // origin/main = P → U

        // Rebase the task branch onto the current origin, simulating a manual
        // recovery that forgot to update the recorded base. After this, the
        // branch history is U → task', but the record still says base = F.
        repo.checkout(branch);
        git(
            repo.path(),
            &[
                "rebase",
                "--onto",
                "origin/main",
                &contaminated_base,
                branch,
            ],
        );

        let (_home, store, session, _pr) = rotation_task(&repo, branch, &contaminated_base).await;

        let err = verify_task_pr_range_with_authority(&store, &session, None, repo.path())
            .await
            .expect_err("divergent ancestry must refuse");
        let message = err.to_string();
        assert!(
            message.contains("diverged"),
            "expected divergence refusal, got: {message}"
        );
        // The base side (M..B) names the foreign commit.
        assert!(
            message.contains("foreign canonical-main commit"),
            "refusal must name the base-side foreign commit, got: {message}"
        );
        assert!(
            message.contains("foreign.txt"),
            "refusal must name the base-side file, got: {message}"
        );
        // The upstream side (B..M) names the upstream commit.
        assert!(
            message.contains("upstream advance"),
            "refusal must name the upstream-side commit, got: {message}"
        );
        assert!(
            message.contains("upstream.txt"),
            "refusal must name the upstream-side file, got: {message}"
        );
        assert!(
            message.contains("rebase --onto"),
            "refusal must print the recovery action, got: {message}"
        );
    }

    #[tokio::test]
    async fn verify_refuses_contaminated_range_without_a_remote() {
        let repo = TestRepo::new();
        let base = repo.head_sha(); // P

        // Drop the remote: the no-remote path must still catch contamination.
        git(repo.path(), &["remote", "remove", "origin"]);

        // Advance local main with a foreign commit, cut the branch from it, then
        // reset main to P — the recorded base is off-local-main.
        repo.create_file("foreign.txt", "not this task's work\n");
        repo.stage_all();
        repo.commit("foreign local-main commit");
        let contaminated_base = repo.head_sha();

        let branch = "jack/no-remote-contaminated";
        repo.create_branch(branch);
        repo.create_file("task.txt", "task work\n");
        repo.stage_all();
        repo.commit("task commit");

        repo.checkout("main");
        git(repo.path(), &["reset", "--hard", &base]);
        repo.checkout(branch);

        let (_home, store, session, _pr) = rotation_task(&repo, branch, &contaminated_base).await;

        let err = verify_task_pr_range_with_authority(&store, &session, None, repo.path())
            .await
            .expect_err("no-remote contaminated range must refuse");
        let message = err.to_string();
        assert!(
            message.contains("contaminated"),
            "expected contamination refusal, got: {message}"
        );
        assert!(
            message.contains("foreign local-main commit"),
            "refusal must name the foreign commit, got: {message}"
        );
    }

    #[tokio::test]
    async fn verify_refuses_contaminated_range_after_squash_merged_parent() {
        // Serial PR shape: PR1 was squash-merged, so origin/main carries a
        // squash commit rather than PR1's original commits. PR2 was cut from
        // PR1's original tip (not the squash), so its recorded base carries
        // commits origin/main doesn't have — the contaminated case.
        let repo = TestRepo::new();

        // PR1: two commits cut from the initial origin tip.
        let first_branch = "jack/pr-one";
        repo.create_branch(first_branch);
        repo.create_file("pr1-a.txt", "a\n");
        repo.stage_all();
        repo.commit("PR1 first commit");
        repo.create_file("pr1-b.txt", "b\n");
        repo.stage_all();
        repo.commit("PR1 second commit");
        let pr1_tip = repo.head_sha();

        // Squash-merge PR1: origin/main gets a single squash commit on P.
        repo.checkout("main");
        git(repo.path(), &["merge", "--squash", first_branch]);
        repo.stage_all();
        repo.commit("squash-merge PR1");
        repo.push();

        // PR2 cut from PR1's original tip (the pre-squash branch), not from the
        // squash commit — the real-world mistake this test catches.
        let second_branch = "jack/pr-two";
        git(repo.path(), &["branch", second_branch, &pr1_tip]);
        repo.checkout(second_branch);
        repo.create_file("pr2.txt", "PR2 work\n");
        repo.stage_all();
        repo.commit("PR2 commit");

        let (_home, store, session, _pr) = rotation_task(&repo, second_branch, &pr1_tip).await;

        let err = verify_task_pr_range_with_authority(&store, &session, None, repo.path())
            .await
            .expect_err("squash-merged parent contamination must refuse");
        let message = err.to_string();
        assert!(
            message.contains("contaminated"),
            "expected contamination refusal after squash-merge, got: {message}"
        );
        assert!(
            message.contains("PR1 first commit"),
            "refusal must name the first pre-squash commit, got: {message}"
        );
        assert!(
            message.contains("PR1 second commit"),
            "refusal must name the second pre-squash commit, got: {message}"
        );
        assert!(
            message.contains("rebase --onto"),
            "refusal must print the recovery action, got: {message}"
        );
    }

    #[test]
    fn placement_base_anchors_on_origin_when_a_remote_exists() {
        let repo = TestRepo::new();
        let origin = repo.head_sha();
        // A local-only commit ahead of origin must NOT move the recorded base.
        repo.create_file("local.txt", "unpushed\n");
        repo.stage_all();
        repo.commit("unpushed local commit");

        let (base_ref, base_commit) =
            resolve_upstream_base(repo.path(), "main").expect("resolve base");
        assert_eq!(base_ref, "origin/main");
        assert_eq!(
            base_commit, origin,
            "the base must anchor on fetched origin, not the ahead-of-origin local tip"
        );
    }

    #[test]
    fn placement_base_falls_back_to_local_main_without_a_remote() {
        let repo = TestRepo::new();
        git(repo.path(), &["remote", "remove", "origin"]);
        repo.create_file("local.txt", "local only\n");
        repo.stage_all();
        repo.commit("local commit");
        let local_tip = repo.head_sha();

        let (base_ref, base_commit) =
            resolve_upstream_base(repo.path(), "main").expect("resolve base");
        assert_eq!(base_ref, "refs/heads/main");
        assert_eq!(base_commit, local_tip);
    }

    #[test]
    fn placement_refuses_when_canonical_main_is_ahead_of_origin() {
        let repo = TestRepo::new();
        // Simulate the #877/#882 root cause: canonical main carries an unpushed
        // commit its upstream lacks.
        repo.create_file("ahead.txt", "unpushed canonical work\n");
        repo.stage_all();
        repo.commit("unpushed canonical commit");

        let err = refuse_if_canonical_ahead(repo.path(), "main")
            .expect_err("ahead-of-origin canonical main must refuse placement");
        let message = err.to_string();
        assert!(
            message.contains("ahead of origin/main"),
            "expected control-plane refusal, got: {message}"
        );
        assert!(
            message.contains("unpushed canonical commit"),
            "refusal must name the unpushed commit, got: {message}"
        );
    }

    /// W2-144 gen 7: a dead process on an open-PR Task with a queued `Resume`
    /// must relaunch (consuming the Resume) instead of settling to `Waiting`.
    /// Without a queued Resume, the existing settle-to-Waiting behavior holds.
    #[tokio::test]
    async fn reconcile_process_liveness_consumes_queued_resume_before_settling() {
        let dir = tempfile::tempdir().unwrap();
        let store: SharedStore = Arc::new(
            open_store(&StorageConfig::sqlite(dir.path().join("registry.db")))
                .await
                .unwrap(),
        );
        let now = OffsetDateTime::now_utc();
        let wave = Wave::new(
            WaveId::new(),
            "queue-bridge".to_string(),
            dir.path().display().to_string(),
        );
        store.create_wave(&wave).await.unwrap();
        let project = ProjectSession {
            id: ProjectSessionId::new(),
            launch: ProjectLaunchReceipt {
                project: LinearProjectSnapshot {
                    id: LinearProjectId::new(format!("project-{}", WaveId::new())).unwrap(),
                    slug: "queue-bridge".to_string(),
                    name: "Queue bridge".to_string(),
                    prompt_context: "Test".to_string(),
                },
                pm_snapshot_synced_at: now.unix_timestamp(),
            },
            wave_id: wave.id().clone(),
            current_directive_version: 0,
            incorporated_directive_version: 0,
            status: ProjectSessionStatus::Running,
            status_reason: "test".to_string(),
            status_at: now,
            iteration: 1,
            observation_cursor: 0,
            last_state_fingerprint: None,
            agent: "codex".to_string(),
            provider: "codex".to_string(),
            provider_session_id: Some("thread".to_string()),
            latest_process: Some(ChildProcessGeneration {
                generation: 1,
                pid: None,
                process_group_id: None,
                tmux_name: "lf-project-test".to_string(),
                agent: "codex".to_string(),
                provider: "codex".to_string(),
                provider_session_id: Some("thread".to_string()),
                started_at: now,
                state: crate::child_session::ChildLeaseState::Active,
                outcome: None,
                provenance: None,
            }),
            abandon_intent: None,
            created_at: now,
            updated_at: now,
        };
        store.create_project_session(&project).await.unwrap();

        let task = TaskSession {
            id: TaskSessionId::new(),
            launch: TaskLaunchReceipt {
                issue: LinearIssueSnapshot {
                    id: LinearIssueId::new(format!("issue-{}", WaveId::new())).unwrap(),
                    identifier: "INF-QUEUE".to_string(),
                    title: "Queue bridge proof".to_string(),
                    description: "Resume must be consumed.".to_string(),
                },
                project: project.launch.project.clone(),
                pm_snapshot_synced_at: now.unix_timestamp(),
            },
            pm_writeback: PmWritebackState::Current,
            wave_id: wave.id().clone(),
            project_session_id: project.id.clone(),
            current_directive_version: 0,
            incorporated_directive_version: 0,
            status: TaskSessionStatus::Running,
            status_reason: "task is running".to_string(),
            status_at: now,
            worktree: dir.path().to_path_buf(),
            workspace_slug: "queue-bridge".to_string(),
            lifecycle: crate::task::TaskLifecyclePlan::standard("task"),
            lifecycle_phase: crate::task::TaskLifecyclePhase::Iterate,
            phase_epoch: 1,
            phase_cursor: 0,
            phase_iteration: 0,
            gate_cycle: 0,
            gate_proposal: None,
            agent: "codex".to_string(),
            provider: "codex".to_string(),
            provider_session_id: Some("thread".to_string()),
            latest_process: Some(ChildProcessGeneration {
                generation: 1,
                pid: None,
                process_group_id: None,
                tmux_name: "lf-task-queue-bridge".to_string(),
                agent: "codex".to_string(),
                provider: "codex".to_string(),
                provider_session_id: Some("thread".to_string()),
                started_at: now - time::Duration::seconds(30),
                state: crate::child_session::ChildLeaseState::Active,
                outcome: None,
                provenance: None,
            }),
            abandon_intent: None,
            created_at: now,
            updated_at: now,
            observation: crate::task::Observation::NotRequired,
        };

        let mut pr = TaskPr {
            id: TaskPrId::new(),
            task_session_id: task.id.clone(),
            sequence: 1,
            slug: task.workspace_slug.clone(),
            branch: "jack/queue-bridge".to_string(),
            base_commit: "0".repeat(40),
            parent_pr_id: None,
            publication: None,
            merge_commit: None,
            abandoned_at: None,
            ci_observation: None,
            github_observation: None,
            created_at: now,
            updated_at: now,
        };
        store.create_task_session(&task, &pr).await.unwrap();

        // Promote the PR to Open (published on GitHub) after creation.
        pr.publication = Some(PrPublication {
            requested_at: now,
            after_merge: AfterMerge::Review,
            next_slug: None,
            github: Some(GithubPr {
                number: 999,
                url: "https://github.com/loopflow/loopflow/pull/999".to_string(),
                head_sha: Some("head-1".to_string()),
            }),
        });
        pr.updated_at = OffsetDateTime::now_utc();
        store.update_task_pr(&pr).await.unwrap();

        // --- Without a queued Resume: settles to Waiting (existing behavior) ---
        let mut idle = store.get_task_session(&task.id).await.unwrap().unwrap();
        super::reconcile_process_liveness(&store, &mut idle)
            .await
            .expect("settle without Resume");
        assert_eq!(
            idle.status,
            TaskSessionStatus::Waiting,
            "idle task settles to Waiting"
        );

        // --- With a queued Resume: relaunches, does NOT settle to Waiting ---
        // Re-arm the task to Running with a stale process for the second pass.
        idle.set_status(TaskSessionStatus::Running, "re-armed for resume proof");
        idle.latest_process = Some(ChildProcessGeneration {
            generation: 2,
            pid: None,
            process_group_id: None,
            tmux_name: "lf-task-queue-bridge-2".to_string(),
            agent: "codex".to_string(),
            provider: "codex".to_string(),
            provider_session_id: Some("thread".to_string()),
            started_at: OffsetDateTime::now_utc() - time::Duration::seconds(30),
            state: crate::child_session::ChildLeaseState::Active,
            outcome: None,
            provenance: None,
        });
        store.update_task_session(&idle).await.unwrap();

        let resume = ChildCommand::new(
            ChildRef::Task(task.id.clone()),
            ChildCommandSource::Human,
            ChildCommandKind::Resume { message: None },
        );
        store.create_child_command(&resume).await.unwrap();

        let mut with_resume = store.get_task_session(&task.id).await.unwrap().unwrap();
        let result = super::reconcile_process_liveness(&store, &mut with_resume).await;

        // The bridge fired: relaunch was attempted. In the test env the launch
        // fails (lf_bin = /usr/bin/false), so we get an error — but the key
        // proof is that the status is NOT Waiting (it took the relaunch path).
        assert_ne!(
            with_resume.status,
            TaskSessionStatus::Waiting,
            "a queued Resume must prevent the Waiting settle"
        );
        // Either the launch failed (error returned) or a new generation started
        // (Starting). Both prove the bridge consumed the Resume.
        assert!(
            result.is_err() || with_resume.status == TaskSessionStatus::Starting,
            "bridge should relaunch, got status={}, result={:?}",
            with_resume.status.as_str(),
            result.as_ref().err().map(|e| e.to_string())
        );

        // The Resume command is still in the queue (not yet claimed by a
        // successful generation), but it was not discarded — the bridge
        // attempted a relaunch to consume it.
        let commands = store
            .list_child_commands(&ChildRef::Task(task.id))
            .await
            .unwrap();
        let resume_still_pending = commands.iter().any(|cmd| {
            matches!(cmd.kind, ChildCommandKind::Resume { .. }) && !cmd.state.is_terminal()
        });
        assert!(
            resume_still_pending,
            "the Resume command is still in the queue, not discarded"
        );
    }

    // ── Task recovery adoption preconditions (W2-251) ───────────────────────
    //
    // Recovery must refuse unsafe worktree/branch state before moving any
    // durable ownership. Each test proves one adoption precondition; the
    // unrelated-branch test additionally proves refusal leaves the predecessor,
    // successor link, PR sequence, lease, and worktree untouched.

    fn checkout_branch(repo: &TestRepo, branch: &str) {
        let status = Command::new("git")
            .current_dir(repo.path())
            .args(["checkout", branch])
            .status()
            .expect("checkout");
        assert!(status.success(), "checkout {branch} failed");
    }

    #[tokio::test]
    async fn recovery_refuses_an_unrelated_branch_before_moving_ownership() {
        let repo = TestRepo::new();
        let base = repo.head_sha();
        let first_branch = "jack/task-pr-proof";
        repo.create_branch(first_branch);
        repo.create_file("first.txt", "first PR\n");
        repo.stage_all();
        repo.commit("first PR");
        // Seed a dead Active lease so refusal's "leases untouched" is load-bearing:
        // without the gate, reconcile_process_liveness would reap it.
        let (_home, store, session, first) = rotation_task_with_lease(
            &repo,
            first_branch,
            &base,
            Some((
                crate::child_session::ChildLeaseState::Active,
                TaskSessionStatus::Waiting,
            )),
        )
        .await;
        settle_pr(&store, first, "merge-unrelated", None).await;

        // The worktree is on an unrelated branch — neither settled nor the
        // deterministic next branch.
        repo.create_branch("jack/unrelated");
        let prs_before = store.task_prs(&session.id).await.expect("read PRs");
        let lease_before = store
            .get_task_session(&session.id)
            .await
            .unwrap()
            .unwrap()
            .latest_process
            .clone()
            .expect("dead lease seeded");

        let err = task_recovery_adoption(&store, &session)
            .await
            .expect_err("unrelated branch must refuse");
        let message = err.to_string();
        assert!(
            message.contains("between-PR recovery expected settled branch"),
            "expected unrelated-branch refusal, got: {message}"
        );
        assert!(
            message.contains("jack/unrelated"),
            "refusal must name the current branch, got: {message}"
        );
        assert!(
            message.contains("refused before moving any ownership"),
            "refusal must name the contract, got: {message}"
        );

        // Refusal left the PR sequence, lease, status, and worktree untouched.
        assert_eq!(
            store.task_prs(&session.id).await.expect("reread PRs"),
            prs_before,
            "PR sequence untouched"
        );
        let after = store.get_task_session(&session.id).await.unwrap().unwrap();
        assert_eq!(
            after.latest_process,
            Some(lease_before),
            "dead lease untouched — the gate is what prevents the reap"
        );
        assert_eq!(after.status, TaskSessionStatus::Waiting, "status untouched");
        assert_eq!(
            git(repo.path(), &["rev-parse", "--abbrev-ref", "HEAD"]),
            "jack/unrelated",
            "worktree branch untouched"
        );
    }

    /// The supervisor's stall recovery restarts a body into the Task worktree, so
    /// it clears the same adoption gate as an explicit resume. Without it a stalled
    /// Task sitting on an unrelated branch has its lease reaped and a successor
    /// committed, only for rotation to reject the branch afterwards.
    #[tokio::test]
    async fn supervised_restart_refuses_an_unrelated_branch() {
        let repo = TestRepo::new();
        let base = repo.head_sha();
        let first_branch = "jack/task-pr-proof";
        repo.create_branch(first_branch);
        repo.create_file("first.txt", "first PR\n");
        repo.stage_all();
        repo.commit("first PR");
        let (_home, store, session, first) = rotation_task_with_lease(
            &repo,
            first_branch,
            &base,
            Some((ChildLeaseState::Active, TaskSessionStatus::Running)),
        )
        .await;
        settle_pr(&store, first, "merge-supervised", None).await;
        repo.create_branch("jack/unrelated");
        // Re-read as the supervisor does: a stale status_at would make the
        // revoke's compare-and-swap decline on its own and prove nothing.
        let session = store.get_task_session(&session.id).await.unwrap().unwrap();
        let lease_before = session.latest_process.clone().expect("active lease seeded");

        // No delivering command, so the plan is a plain restart: the path that
        // would otherwise commit a successor.
        let observation = observe(
            &BodyEvidence {
                intent: BodyIntent::Active,
                observable: true,
                process_alive: true,
                progress_age: Duration::from_secs(31 * 60),
                step: Some("task_pursue".to_string()),
                reason: "body is alive but stalled".to_string(),
            },
            Duration::from_secs(30 * 60),
        );
        let latest_event_id = store
            .latest_task_event(&session.id)
            .await
            .unwrap()
            .map(|event| event.id);

        assert!(
            !recover_stalled_task_body(&store, session.clone(), &observation, latest_event_id)
                .await
                .expect("an unsafe worktree declines recovery; it does not fail the supervisor"),
            "an unrelated branch must not be restarted"
        );

        let after = store.get_task_session(&session.id).await.unwrap().unwrap();
        assert_eq!(
            after.latest_process,
            Some(lease_before),
            "lease untouched — the gate is what prevents the reap"
        );
        assert_eq!(after.status, TaskSessionStatus::Running, "status untouched");
    }

    #[tokio::test]
    async fn recovery_refuses_a_dirty_worktree_between_prs() {
        let repo = TestRepo::new();
        let base = repo.head_sha();
        let first_branch = "jack/task-pr-proof";
        repo.create_branch(first_branch);
        repo.create_file("first.txt", "first PR\n");
        repo.stage_all();
        repo.commit("first PR");
        let (_home, store, session, first) =
            rotation_task_with_lease(&repo, first_branch, &base, None).await;
        settle_pr(&store, first, "merge-dirty", None).await;

        // Uncommitted follow-up work on the settled branch: the runner's strict
        // rotation cannot carry it, so recovery must refuse before moving
        // ownership and point the human at `lf pr next`.
        repo.create_file("follow-up.txt", "uncommitted\n");

        let err = task_recovery_adoption(&store, &session)
            .await
            .expect_err("dirty between-PR worktree must refuse");
        let message = err.to_string();
        assert!(
            message.contains("uncommitted changes"),
            "expected dirty refusal, got: {message}"
        );
        assert!(
            message.contains("lf pr next"),
            "refusal must name the recovery action, got: {message}"
        );
        assert!(
            message.contains("refused before moving any ownership"),
            "refusal must name the contract, got: {message}"
        );
    }

    #[tokio::test]
    async fn recovery_refuses_a_missing_branch() {
        let repo = TestRepo::new();
        let base = repo.head_sha();
        let first_branch = "jack/task-pr-proof";
        repo.create_branch(first_branch);
        repo.create_file("first.txt", "first PR\n");
        repo.stage_all();
        repo.commit("first PR");
        let (_home, store, session, first) =
            rotation_task_with_lease(&repo, first_branch, &base, None).await;
        settle_pr(&store, first, "merge-missing", None).await;

        // Delete the ref the worktree is checked out on; HEAD dangles off a
        // branch that no longer exists.
        Command::new("git")
            .current_dir(repo.path())
            .args(["update-ref", "-d", &format!("refs/heads/{first_branch}")])
            .status()
            .expect("delete branch ref");
        assert_eq!(
            git(repo.path(), &["symbolic-ref", "--short", "HEAD"]),
            first_branch,
            "HEAD still names the deleted branch"
        );

        let err = task_recovery_adoption(&store, &session)
            .await
            .expect_err("missing branch must refuse");
        let message = err.to_string();
        assert!(
            message.contains("no longer exists"),
            "expected missing-branch refusal, got: {message}"
        );
        assert!(
            message.contains(first_branch),
            "refusal must name the missing branch, got: {message}"
        );
    }

    #[tokio::test]
    async fn recovery_adopts_an_active_pr_branch_and_allows_ongoing_work() {
        let repo = TestRepo::new();
        let base = repo.head_sha();
        let branch = "jack/task-pr-proof";
        repo.create_branch(branch);
        repo.create_file("first.txt", "first PR\n");
        repo.stage_all();
        repo.commit("first PR");
        // rotation_task seeds an active (Working) PR on `branch`.
        let (_home, store, session, _pr) =
            rotation_task_with_lease(&repo, branch, &base, None).await;

        // Dirty ongoing work on the active PR's branch is allowed — recovery
        // must not drop in-progress work.
        repo.create_file("wip.txt", "ongoing\n");

        let adoption = task_recovery_adoption(&store, &session)
            .await
            .expect("active PR on its branch is adopted, dirty work allowed");
        assert_eq!(
            adoption,
            TaskRecoveryAdoption::Active {
                branch: branch.to_string()
            }
        );
    }

    #[tokio::test]
    async fn recovery_refuses_a_crash_boundary() {
        let repo = TestRepo::new();
        let base = repo.head_sha();
        let branch = "jack/task-pr-proof";
        repo.create_branch(branch);
        repo.create_file("first.txt", "first PR\n");
        repo.stage_all();
        repo.commit("first PR");
        let (_home, store, session, _pr) =
            rotation_task_with_lease(&repo, branch, &base, None).await;

        // Drive the worktree into a conflicting rebase: advance main with a
        // conflicting edit, then rebase the branch onto it.
        repo.checkout("main");
        repo.create_file("first.txt", "main wins\n");
        repo.stage_all();
        repo.commit("main advance");
        checkout_branch(&repo, branch);
        let rebase = Command::new("git")
            .current_dir(repo.path())
            .args(["rebase", "main"])
            .output()
            .expect("run rebase");
        assert!(
            !rebase.status.success(),
            "rebase must conflict to seed a crash boundary"
        );

        let err = task_recovery_adoption(&store, &session)
            .await
            .expect_err("crash boundary must refuse");
        let message = err.to_string();
        assert!(
            message.contains("mid-rebase"),
            "expected crash-boundary refusal, got: {message}"
        );
        assert!(
            message.contains("refused before moving any ownership"),
            "refusal must name the contract, got: {message}"
        );

        // Cleanup so the temp repo drops cleanly.
        let _ = Command::new("git")
            .current_dir(repo.path())
            .args(["rebase", "--abort"])
            .status();
    }

    #[tokio::test]
    async fn between_prs_recovery_selects_the_deterministic_next_branch() {
        let repo = TestRepo::new();
        let base = repo.head_sha();
        let first_branch = "jack/task-pr-proof";
        repo.create_branch(first_branch);
        repo.create_file("first.txt", "first PR\n");
        repo.stage_all();
        repo.commit("first PR");
        let (_home, store, session, first) =
            rotation_task_with_lease(&repo, first_branch, &base, None).await;
        settle_pr(&store, first, "merge-942", Some("follow-up")).await;

        // The deterministic next branch the gate must select, read from the
        // settled PR the gate reads (not the pre-settle local copy).
        let settled = store
            .task_prs(&session.id)
            .await
            .expect("read settled PR")
            .pop()
            .expect("settled PR");
        let next = format!("{first_branch}-follow-up");
        assert_eq!(next_pr_slug(&settled, None), "follow-up");

        // The worktree is already on the next branch — a partial rotation the
        // gate must adopt, not refuse as an unrelated branch.
        Command::new("git")
            .current_dir(repo.path())
            .args(["checkout", "-b", &next])
            .status()
            .expect("cut next branch");

        let adoption = task_recovery_adoption(&store, &session)
            .await
            .expect("partial rotation onto the next branch is adopted");
        assert_eq!(
            adoption,
            TaskRecoveryAdoption::BetweenPrs {
                settled: first_branch.to_string(),
                next
            }
        );
    }

    #[tokio::test]
    async fn refuse_dirty_between_prs_blocks_after_an_out_of_band_merge() {
        let repo = TestRepo::new();
        let base = repo.head_sha();
        let first_branch = "jack/task-pr-proof";
        repo.create_branch(first_branch);
        repo.create_file("first.txt", "first PR\n");
        repo.stage_all();
        repo.commit("first PR");
        let (_home, store, session, first) =
            rotation_task_with_lease(&repo, first_branch, &base, None).await;
        repo.create_file("wip.txt", "ongoing\n");

        // While the PR is active, dirty ongoing work is fine.
        refuse_dirty_between_prs(&store, &session)
            .await
            .expect("active PR with dirty work is allowed");

        // The PR merges out of band -> settled -> between-PR. The post-reconcile
        // guard must now refuse the dirty between-PR before the lease is reaped
        // or a successor body is launched.
        settle_pr(&store, first, "merge-out-of-band", None).await;
        let err = refuse_dirty_between_prs(&store, &session)
            .await
            .expect_err("dirty between-PR after merge must refuse");
        assert!(
            err.to_string().contains("lf pr next"),
            "expected dirty between-PR refusal, got: {err}"
        );
    }
}
