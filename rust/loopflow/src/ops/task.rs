use std::collections::BTreeMap;
use std::path::{Component, Path, PathBuf};
use std::process::Command;
use std::sync::Arc;
use std::time::{Duration, Instant};

use crate::child_session::{
    task_write_lease_from_env, ChildCommandEffect, ChildCommandId, ChildCommandKind,
    ChildCommandSource, ChildCommandState, ChildDecisionId, ChildDirective, ChildProcessGeneration,
    ChildRef, ChildWriteLease,
};
use crate::engine::config::{load_config_or_default, parse_agent};
use crate::engine::git::{
    checkout_new_branch_from, current_branch, fetch, get_default_branch, is_clean,
    push_with_upstream, ref_exists, rev_parse,
};
use crate::engine::naming::sanitize_for_branch;
use crate::engine::process::{start_lf_session_with_env, tmux_session_exists, tmux_session_slug};
use crate::engine::worktrees::{
    create_from_placement_plan, plan_placement, PlacementStrategy, WorktreeSegment,
};
use crate::engine::{expand_flow, load_flow, ConcreteStep};
use crate::id::WaveId;
use crate::interaction_review::{
    InteractionReview, InteractionReviewDisposition, InteractionReviewId,
};
use crate::ops::error::{OpsError, OpsResult};
use crate::session_context::{
    LinearIssueId, LinearIssueSnapshot, LinearProjectId, LinearProjectSnapshot, TaskLaunchReceipt,
};
use crate::store::{open_existing_store, SharedStore, StoreError};
use crate::task::{
    AfterMerge, CiCheck, CiObservation, CiState, GithubPr, PmWritebackOperation, PmWritebackState,
    PrPhase, PrPublication, TaskEventKind, TaskPr, TaskPrId, TaskSession, TaskSessionStatus,
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
}

fn task_control_result(
    issue_id: String,
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
pub fn task_stack(worktree: &Path) -> OpsResult<Option<StackedRebase>> {
    let key = worktree.display().to_string();
    block_on_task(async move {
        let Some(store) = open_existing_store().await.map(Arc::new) else {
            return Ok(None);
        };
        let Some(session) = store
            .get_task_session_by_worktree(&key)
            .await
            .map_err(|error| task_error(error.to_string()))?
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
        let live =
            crate::ops::pr::current_or_merged_pr_for_branch(&session.worktree, &parent.branch)?;
        let merged = parent.merge_commit.is_some()
            || live.as_ref().is_some_and(|info| info.state == "merged");
        let closed = parent.abandoned_at.is_some()
            || live.as_ref().is_some_and(|info| info.state == "closed");
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
pub fn record_stack_rebase(
    stacked: &StackedRebase,
    new_base: &str,
    clear_parent: bool,
) -> OpsResult<()> {
    let pr_id = stacked.child.id.clone();
    let new_base = new_base.to_string();
    block_on_task(async move {
        let Some(store) = open_existing_store().await.map(Arc::new) else {
            return Ok(());
        };
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

fn command_source(session: &TaskSession) -> OpsResult<ChildCommandSource> {
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
    let ambient = match std::env::var(crate::engine::wave_context::WAVE_ID_ENV) {
        Ok(value) => Some(
            WaveId::parse(&value)
                .map_err(|error| task_error(format!("invalid ambient Wave id: {error}")))?,
        ),
        Err(std::env::VarError::NotPresent) => None,
        Err(std::env::VarError::NotUnicode(_)) => {
            return Err(task_error("ambient Wave id is not valid UTF-8"))
        }
    };
    command_source_for_wave(ambient, &session.wave_id, &session.launch.issue.identifier)
}

fn command_source_for_wave(
    ambient: Option<WaveId>,
    owning_wave_id: &WaveId,
    issue_identifier: &str,
) -> OpsResult<ChildCommandSource> {
    match ambient {
        Some(wave_id) if &wave_id == owning_wave_id => Ok(ChildCommandSource::Wave(wave_id)),
        Some(wave_id) => Err(task_error(format!(
            "Wave {wave_id} cannot control Task {issue_identifier} owned by Wave {owning_wave_id}"
        ))),
        None => Ok(ChildCommandSource::Human),
    }
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
            created_at: now,
            updated_at: now,
        };

        let initial = ChildDirective::initial(
            ChildRef::Task(session.id.clone()),
            directive,
            command_source(&session)?,
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
        let Some(store) = open_existing_store().await.map(Arc::new) else {
            return Ok(false);
        };
        let Some((session, lease)) = task_for_worktree(&store, repo).await? else {
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
        if pr.github().is_none() && !task_pr_has_changes(repo)? {
            return Err(task_error(
                "Task PR has no changes to publish; complete the Task directly if the work is done",
            ));
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

/// Prove the active Task PR's range contains only Task-authored work before any
/// GitHub side effect. A worktree with no Task Session is a no-op — plain,
/// non-Task PRs are unaffected.
///
/// Let `B` = recorded `base_commit`, `O` = `origin/<default>` tip (or local
/// `<default>` when the repo has no remote), `H` = `HEAD`, and
/// `M = merge-base(O, H)`. The parity invariant is `M == B`, which guarantees
/// GitHub's range (`M..H`) equals the recorded range (`B..H`) equals
/// `lf task changes`:
/// - `M == B` — parity holds; publish.
/// - `M` ancestor of `B` — the recorded base itself carries commits absent from
///   `O` (inherited foreign ancestry, the #877/#882 shape). Refuse before any
///   push, naming the foreign commits/files and the safe rebase.
/// - `B` ancestor of `M` — `O` advanced past a stale or squash-merged base. Safe:
///   heal `base_commit → M` so the durable evidence and `lf task changes` stay
///   truthful, then publish the minimal `M..H` range.
/// - divergent — ambiguous ancestry; refuse.
///
/// Empty-range and dirty-worktree checks stay with `request_task_pr_publication`,
/// which runs after the publication path commits pending work; this gate proves
/// committed ancestry only, so it can run before the first push.
pub(crate) fn verify_task_pr_range(repo: &Path) -> OpsResult<()> {
    let repo = repo.to_path_buf();
    block_on_task(async move {
        let Some(store) = open_existing_store().await.map(Arc::new) else {
            return Ok(());
        };
        let Some((session, lease)) = task_for_worktree(&store, &repo).await? else {
            return Ok(());
        };
        verify_task_pr_range_with_authority(&store, &session, lease.as_ref(), &repo).await
    })
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
    let (base_ref, upstream) = resolve_upstream_base(repo, &default_branch)?;
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

    // Neither is an ancestor of the other: genuinely ambiguous ancestry.
    Err(task_error(format!(
        "Task {identifier} PR base {} and {base_ref} have diverged with no common lineage at \
         the recorded base. Refused before any push. Re-cut the branch from {base_ref} or run \
         git rebase --onto {base_ref} {} {branch}",
        short(&base),
        short(&base),
    )))
}

pub(crate) fn attach_task_github_pr(
    repo: &Path,
    github_pr: Option<&crate::ops::pr::PrInfo>,
) -> OpsResult<bool> {
    block_on_task(async move {
        let Some(store) = open_existing_store().await.map(Arc::new) else {
            return Ok(false);
        };
        let Some((session, lease)) = task_for_worktree(&store, repo).await? else {
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

fn task_pr_has_changes(repo: &Path) -> OpsResult<bool> {
    if !is_clean(repo)? {
        return Err(task_error(
            "Task worktree still has uncommitted changes; commit them before publishing the PR",
        ));
    }
    let default_branch = get_default_branch(repo)?;
    let base = format!("origin/{default_branch}...HEAD");
    let status = Command::new("git")
        .args(["diff", "--quiet", &base])
        .current_dir(repo)
        .status()?;
    match status.code() {
        Some(0) => Ok(false),
        Some(1) => Ok(true),
        _ => Err(task_error(format!("failed to compare Task PR with {base}"))),
    }
}

pub(crate) fn abandon_task_pr(
    repo: &Path,
    force: bool,
    progress: &impl crate::ops::progress::Progress,
) -> OpsResult<bool> {
    block_on_task(async move {
        let Some(store) = open_existing_store().await.map(Arc::new) else {
            return Ok(false);
        };
        let Some((mut session, lease)) = task_for_worktree(&store, repo).await? else {
            return Ok(false);
        };
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

pub(crate) async fn reconcile_task_pr(
    store: &SharedStore,
    session: &mut TaskSession,
) -> OpsResult<Option<TaskPr>> {
    reconcile_task_pr_with_authority(store, session, None).await
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
    now: time::OffsetDateTime,
) -> Option<CiObservation> {
    let head_sha = head_sha?.to_string();
    let checks = crate::ops::pr::required_check_state(worktree, branch)?;
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
        failing_checks: checks
            .failing_checks
            .into_iter()
            .map(|check| CiCheck {
                name: check.name,
                url: check.url,
            })
            .collect(),
        observed_at: now,
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
    let Some(github_pr) =
        crate::ops::pr::current_or_merged_pr_for_branch(&session.worktree, &pr.branch)?
    else {
        return Ok(Some(pr));
    };
    let number = u32::try_from(github_pr.number).map_err(|_| {
        task_error(format!(
            "pull request #{} exceeds supported range",
            github_pr.number
        ))
    })?;
    let url = github_pr.url.clone();
    let previous = pr.clone();
    let previous_phase = previous.phase();
    let previous_github = previous.github().cloned();
    let previous_session_status = session.status;
    let now = time::OffsetDateTime::now_utc();
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
            if pr
                .publication
                .as_ref()
                .is_some_and(|publication| publication.after_merge == AfterMerge::CompleteTask)
            {
                session.set_status(
                    TaskSessionStatus::Completed,
                    format!(
                        "pull request #{} merged and completed the Task",
                        github_pr.number
                    ),
                );
                reconcile_pm_writeback(store, session, Some(&url)).await;
            } else if !session.status.is_process_active() {
                session.set_status(
                    TaskSessionStatus::Waiting,
                    format!(
                        "pull request #{} merged; another PR may follow",
                        github_pr.number
                    ),
                );
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
            pr.ci_observation = observe_required_checks(
                &session.worktree,
                &pr.branch,
                github_pr.head_sha.as_deref(),
                now,
            );
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
            || session.pm_writeback != PmWritebackState::Current)
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
    let slug = rotate
        .slug_override
        .clone()
        .or_else(|| {
            settled
                .publication
                .as_ref()
                .and_then(|publication| publication.next_slug.clone())
        })
        .unwrap_or_else(|| sequence.to_string());
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
    let branch = format!("{author}/{}-{slug}", session.workspace_slug);
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
        if let Err(error) = checkout_new_branch_from(&session.worktree, &branch, &base_ref) {
            let recovered = current_branch(&session.worktree)
                .map_err(|read_error| {
                    task_error(format!("failed to inspect recovery branch: {read_error}"))
                })?
                .as_deref()
                == Some(branch.as_str());
            if !recovered {
                return Err(task_error(format!(
                    "failed to rotate Task worktree: {error}"
                )));
            }
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
        let source = command_source(&session)?;
        let result = super::child::queue_command(
            &store,
            super::child::ChildSession::Task(Box::new(session)),
            source,
            kind,
        )
        .await?;
        Ok(task_control_result(issue_id, result))
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
    reconcile_task_pr(&store, &mut session).await?;
    reconcile_process_liveness(&store, &mut session).await?;
    let issue_id = session.launch.issue.identifier.clone();
    let source = command_source(&session)?;
    let result = super::child::resume_session(
        &store,
        super::child::ChildSession::Task(Box::new(session)),
        source,
        message,
        model,
        reason,
    )
    .await?;
    Ok(task_control_result(issue_id, result))
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
            receipt: task_control_result(session.launch.issue.identifier, result),
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
    use std::path::Path;
    use std::process::Command;
    use std::sync::Arc;

    use super::{
        _defer_task_interactions, changes_snapshot, command_source_for_wave, derive_workspace_slug,
        diff_snapshot, ensure_working_pr, ensure_working_pr_with_authority, file_snapshot,
        parse_pr_slug, parse_workspace_slug, project_context, reconcile_process_liveness,
        refuse_if_canonical_ahead, resolve_task_flow, resolve_upstream_base,
        verify_task_pr_range_with_authority, RotateOptions, TaskControlResult, TaskWorkspace,
    };
    use crate::child_session::{ChildCommandSource, ChildProcessGeneration};
    use crate::id::WaveId;
    use crate::pm::{PmKr, PmProject};
    use crate::project_session::{ProjectSession, ProjectSessionId, ProjectSessionStatus};
    use crate::session_context::{
        LinearIssueId, LinearIssueSnapshot, LinearProjectId, LinearProjectSnapshot,
        ProjectLaunchReceipt, TaskLaunchReceipt,
    };
    use crate::store::{open_store, SharedStore, StorageConfig};
    use crate::task::{
        AfterMerge, GithubPr, PmWritebackState, PrPhase, PrPublication, TaskPr, TaskPrId,
        TaskSession, TaskSessionId, TaskSessionStatus,
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
        let home = tempfile::tempdir().expect("task home");
        let store = Arc::new(
            open_store(&StorageConfig::sqlite(home.path().join("loopflow.db")))
                .await
                .expect("open store"),
        );
        let now = OffsetDateTime::now_utc();
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
            status: TaskSessionStatus::Waiting,
            status_reason: "first PR settled".to_string(),
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
            latest_process: None,
            abandon_intent: None,
            created_at: now,
            updated_at: now,
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

    #[test]
    fn foreign_wave_cannot_be_reclassified_as_a_human_command() {
        let wave_id = crate::id::WaveId::new();

        assert!(matches!(
            command_source_for_wave(Some(wave_id.clone()), &wave_id, "INF-123").unwrap(),
            ChildCommandSource::Wave(_)
        ));
        assert!(
            command_source_for_wave(Some(crate::id::WaveId::new()), &wave_id, "INF-123").is_err()
        );
        assert_eq!(
            command_source_for_wave(None, &wave_id, "INF-123").unwrap(),
            ChildCommandSource::Human
        );
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
}
