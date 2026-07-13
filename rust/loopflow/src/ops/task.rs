use std::path::Path;
use std::sync::Arc;
use std::time::{Duration, Instant};

use crate::engine::config::{load_config_or_default, parse_agent};
use crate::engine::git::{get_default_branch, rev_parse};
use crate::engine::process::{
    resolve_lf_binary, start_lf_session_with_env, tmux_session_exists, tmux_session_slug,
};
use crate::engine::worktrees::{
    create_from_placement_plan, plan_placement, PlacementRequest, PlacementStrategy,
    WorktreeSegment,
};
use crate::lfd::id::LfdId;
use crate::lfdb::{open_existing_store, SharedStore, StoreError};
use crate::ops::error::{OpsError, OpsResult};
use crate::project_session::SessionSupervisor;
use crate::task::{
    ChildDirective, ChildRef, LinearIssueId, LinearIssueRef, LinearProjectId, LinearProjectRef,
    PmWritebackOperation, PmWritebackState, TaskCommand, TaskCommandEffect, TaskCommandId,
    TaskCommandKind, TaskCommandSource, TaskCommandState, TaskDecisionId, TaskEventKind,
    TaskSession, TaskSessionStatus,
};
use sha2::{Digest, Sha256};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TaskWaitUntil {
    Submitted,
    Terminal,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, clap::ValueEnum)]
pub enum ChildReceiptUntil {
    Applied,
    Incorporated,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct TaskControlResult {
    pub issue_id: String,
    pub session_id: String,
    pub command_id: String,
    pub directive_version: Option<u32>,
    pub state: TaskCommandState,
    pub effect: Option<TaskCommandEffect>,
    pub incorporated: bool,
    pub generation: Option<u32>,
    pub accepted_at: Option<time::OffsetDateTime>,
    pub incorporated_at: Option<time::OffsetDateTime>,
    pub error: Option<String>,
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
pub struct TaskDeliveryView {
    pub kind: String,
    pub base: String,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct TaskSessionSnapshot {
    pub issue_id: String,
    pub issue_identifier: String,
    pub session_id: String,
    pub project_id: String,
    pub project: String,
    pub pm_snapshot_synced_at: i64,
    pub pm_snapshot_warning: Option<String>,
    pub pm_writeback: crate::task::PmWritebackState,
    pub wave: String,
    pub supervisor: SessionSupervisor,
    pub current_directive_version: u32,
    pub incorporated_directive_version: u32,
    pub status: String,
    pub status_reason: String,
    pub status_at: time::OffsetDateTime,
    pub worktree: String,
    pub branch: String,
    pub base_commit: String,
    pub delivery: TaskDeliveryView,
    pub agent: String,
    pub provider: String,
    pub provider_session_id: Option<String>,
    pub process_alive: bool,
    pub process: Option<crate::task::TaskProcess>,
    pub pull_request: Option<crate::task::PullRequestRef>,
    pub latest_event: Option<crate::task::TaskEvent>,
    pub created_at: time::OffsetDateTime,
    pub updated_at: time::OffsetDateTime,
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
        task_error("no Loopflow registry on this machine; serve the owning Wave first")
    })
}

fn command_source(session: &TaskSession) -> OpsResult<TaskCommandSource> {
    match std::env::var("LFD_PROJECT_SESSION_ID") {
        Ok(value) => {
            let project_id =
                crate::project_session::ProjectSessionId::parse(&value).map_err(|error| {
                    task_error(format!("invalid ambient Project Session id: {error}"))
                })?;
            return match &session.supervisor {
                SessionSupervisor::Project { session_id } if session_id == &project_id => {
                    Ok(TaskCommandSource::Project(project_id))
                }
                SessionSupervisor::Project { session_id } => Err(task_error(format!(
                    "Project Session {project_id} cannot control Task {}; its supervisor is {session_id}",
                    session.issue.identifier
                ))),
                SessionSupervisor::Wave { .. } => Err(task_error(format!(
                    "Project Session {project_id} cannot control directly supervised Task {}",
                    session.issue.identifier
                ))),
            };
        }
        Err(std::env::VarError::NotPresent) => {}
        Err(std::env::VarError::NotUnicode(_)) => {
            return Err(task_error("ambient Project Session id is not valid UTF-8"))
        }
    }
    let ambient = match std::env::var(crate::lf::session::WAVE_ID_ENV) {
        Ok(value) => Some(
            LfdId::parse(&value)
                .map_err(|error| task_error(format!("invalid ambient Wave id: {error}")))?,
        ),
        Err(std::env::VarError::NotPresent) => None,
        Err(std::env::VarError::NotUnicode(_)) => {
            return Err(task_error("ambient Wave id is not valid UTF-8"))
        }
    };
    command_source_for_wave(
        ambient,
        &session.wave_id,
        &session.issue.identifier,
        &session.wave,
    )
}

fn command_source_for_wave(
    ambient: Option<LfdId>,
    owning_wave_id: &LfdId,
    issue_identifier: &str,
    owning_wave: &str,
) -> OpsResult<TaskCommandSource> {
    match ambient {
        Some(wave_id) if &wave_id == owning_wave_id => Ok(TaskCommandSource::Wave(wave_id)),
        Some(wave_id) => Err(task_error(format!(
            "Wave {wave_id} cannot control Task {issue_identifier} owned by wave/{owning_wave}"
        ))),
        None => Ok(TaskCommandSource::Human),
    }
}

pub fn task_run(repo: &Path, issue: &str, directive: Option<String>) -> OpsResult<TaskSession> {
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
        let existing = store
            .get_task_session_by_issue(issue)
            .await
            .map_err(|error| task_error(format!("failed to read task registry: {error}")))?;
        if let Some(session) = &existing {
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
                        session.issue.identifier,
                        current.version,
                        session.issue.identifier,
                    )));
                }
            }
        }
        Ok(existing)
    })? {
        return Ok(existing);
    }
    let main_repo = crate::ops::project::ensure_clean_main(repo, "Task start")
        .map_err(|error| task_error(error.to_string()))?;
    let resolved =
        crate::ops::task_pm::resolve_task(&main_repo, issue, crate::ops::pm::PmRefresh::Auto)?;
    let segment = WorktreeSegment::parse(&resolved.item.identifier)
        .map_err(|error| task_error(error.to_string()))?;
    let plan = plan_placement(&main_repo, PlacementRequest::Main { segment })
        .map_err(|error| task_error(format!("failed to plan task worktree: {error}")))?;
    if plan.strategy != PlacementStrategy::CreateRoot {
        return Err(task_error(format!(
            "task worktree or branch already exists without a Task Session: {} ({})",
            plan.worktree_path.display(),
            plan.branch
        )));
    }
    let default_branch =
        get_default_branch(&main_repo).map_err(|error| task_error(error.to_string()))?;
    let base_commit = rev_parse(&main_repo, &default_branch)
        .map_err(|error| task_error(format!("failed to resolve task base: {error}")))?;
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
        let wave = store
            .get_wave_by_name(&resolved.snapshot.wave)
            .await
            .map_err(|error| task_error(format!("failed to read owning Wave: {error}")))?
            .ok_or_else(|| {
                task_error(format!(
                    "owning Wave {:?} is not registered",
                    resolved.snapshot.wave
                ))
            })?;
        let now = time::OffsetDateTime::now_utc();
        let supervisor = task_supervisor(&store, wave.id(), &resolved.project.id).await?;
        let mut session = TaskSession {
            id: crate::task::TaskSessionId::new(),
            issue: LinearIssueRef {
                id: LinearIssueId::new(resolved.item.id.clone())
                    .map_err(|error| task_error(error.to_string()))?,
                identifier: resolved.item.identifier.clone(),
                title: resolved.item.name.clone(),
                description: resolved.item.description.clone(),
            },
            project: LinearProjectRef {
                id: LinearProjectId::new(resolved.project.id.clone())
                    .map_err(|error| task_error(error.to_string()))?,
                slug: resolved.project.slug.clone(),
                name: resolved.project.name.clone(),
                context: project_context(&resolved.project),
            },
            wave_id: wave.id().clone(),
            supervisor,
            current_directive_version: 1,
            incorporated_directive_version: 0,
            pm_snapshot_synced_at: resolved.snapshot.synced_at,
            // TODO(product-pm): main's `load_show_snapshot` logs a soft-stale
            // fallback to progress but does not return it on `PmShowResult`, so
            // there is no warning to capture at launch yet. Restore this once the
            // PM read surfaces the stale-cache warning on its result.
            pm_snapshot_warning: None,
            pm_writeback: PmWritebackState::Current,
            wave: resolved.snapshot.wave.clone(),
            status: TaskSessionStatus::Created,
            status_reason: "Linear task reserved before placement".to_string(),
            status_at: now,
            worktree: plan.worktree_path.clone(),
            branch: plan.branch.clone(),
            base_commit,
            agent,
            provider,
            provider_session_id: None,
            process: None,
            pull_request: None,
            created_at: now,
            updated_at: now,
        };

        let initial = ChildDirective::initial(
            ChildRef::Task(session.id.clone()),
            directive,
            command_source(&session)?,
        );
        match store
            .reserve_task_session_with_directive(&session, &initial, wave.workers.max(1))
            .await
        {
            Ok(true) => {}
            Ok(false) => {
                return Err(task_error(format!(
                    "wave/{} has reached its {} active Task Session limit",
                    wave.name(),
                    wave.workers.max(1)
                )))
            }
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

        launch_task_process(&store, &mut session, None).await?;
        wait_until_running(&store, &session.id).await
    })
}

async fn task_supervisor(
    store: &SharedStore,
    wave_id: &LfdId,
    project_id: &str,
) -> OpsResult<SessionSupervisor> {
    match std::env::var("LFD_PROJECT_SESSION_ID") {
        Ok(value) => {
            let session_id =
                crate::project_session::ProjectSessionId::parse(&value).map_err(|error| {
                    task_error(format!("invalid ambient Project Session id: {error}"))
                })?;
            let project = store
                .get_project_session(&session_id)
                .await
                .map_err(|error| {
                    task_error(format!("failed to read ambient Project Session: {error}"))
                })?
                .ok_or_else(|| {
                    task_error(format!(
                        "ambient Project Session {session_id} does not exist"
                    ))
                })?;
            if &project.wave_id != wave_id || project.project.id.as_str() != project_id {
                return Err(task_error(format!(
                    "Project Session {session_id} cannot supervise a Task outside {}/{}",
                    project.wave, project.project.slug
                )));
            }
            Ok(SessionSupervisor::Project { session_id })
        }
        Err(std::env::VarError::NotPresent) => Ok(SessionSupervisor::Wave {
            wave_id: wave_id.clone(),
        }),
        Err(std::env::VarError::NotUnicode(_)) => {
            Err(task_error("ambient Project Session id is not valid UTF-8"))
        }
    }
}

pub(crate) fn project_context(project: &crate::lfd::pm::PmProject) -> String {
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
    directive: Option<String>,
) -> OpsResult<TaskSession> {
    let main = crate::ops::project::ensure_clean_main(repo, "Task start")
        .map_err(|error| task_error(error.to_string()))?;
    let project =
        crate::ops::task_pm::resolve_project(&main, project_id, crate::ops::pm::PmRefresh::Auto)?;
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
    task_run(&main, &created.item.id, directive)
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

/// Reserve capacity from the owning Wave and start a fresh generation for a
/// session whose process is no longer active.
async fn relaunch_inactive_process(
    store: &SharedStore,
    session: &mut TaskSession,
) -> OpsResult<()> {
    let wave = store
        .get_wave(&session.wave_id)
        .await
        .map_err(|error| task_error(format!("failed to read owning Wave: {error}")))?
        .ok_or_else(|| task_error(format!("owning wave/{} is not registered", session.wave)))?;
    launch_task_process(store, session, Some(wave.workers.max(1))).await
}

async fn launch_task_process(
    store: &SharedStore,
    session: &mut TaskSession,
    max_active: Option<u32>,
) -> OpsResult<()> {
    let tmux_name = format!(
        "lf-task-{}-{}",
        tmux_session_slug(&session.issue.identifier),
        &session.id.as_str()[3..11]
    );
    let from = session.status;
    let mut launch = session.clone();
    let generation = launch.begin_generation(tmux_name.clone());
    if let Some(max_active) = max_active {
        let reserved = store
            .reserve_task_process(&launch, from, max_active)
            .await
            .map_err(|error| task_error(format!("failed to reserve task process: {error}")))?;
        if !reserved {
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
                    current.issue.identifier,
                    current.status.as_str()
                )));
            }
            if current.status != from {
                return Err(task_error(format!(
                    "task {} changed from {} to {} during process reservation; retry the command",
                    current.issue.identifier,
                    from.as_str(),
                    current.status.as_str()
                )));
            }
            return Err(task_error(format!(
                "wave/{} has reached its {max_active} active Task Session limit",
                session.wave
            )));
        }
    } else {
        store
            .update_task_session(&launch)
            .await
            .map_err(|error| task_error(format!("failed to record task launch: {error}")))?;
    }
    *session = launch;
    store
        .append_task_event(
            &session.id,
            &TaskEventKind::StatusChanged {
                from,
                to: TaskSessionStatus::Starting,
                reason: "task process is starting".to_string(),
            },
        )
        .await
        .map_err(|error| task_error(error.to_string()))?;

    let argv = vec![
        resolve_lf_binary().to_string_lossy().to_string(),
        "__task".to_string(),
        session.id.to_string(),
        "--generation".to_string(),
        generation.to_string(),
    ];
    let generation_text = generation.to_string();
    let environment = [
        (crate::lf::session::WAVE_ID_ENV, session.wave_id.as_str()),
        ("LF_TASK_SESSION_ID", session.id.as_str()),
        ("LF_TASK_GENERATION", generation_text.as_str()),
    ];
    if let Err(error) =
        start_lf_session_with_env(&tmux_name, &session.worktree, &argv, &environment).await
    {
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
    let deadline = tokio::time::Instant::now() + Duration::from_secs(10);
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
                    session.issue.identifier, session.status_reason
                )))
            };
        }
        if tokio::time::Instant::now() >= deadline {
            return Err(task_error(format!(
                "task {} process did not report running within 10 seconds",
                session.issue.identifier
            )));
        }
        tokio::time::sleep(Duration::from_millis(100)).await;
    }
}

async fn reconcile_process_liveness(
    store: &SharedStore,
    session: &mut TaskSession,
) -> OpsResult<()> {
    if !session.status.is_process_active() {
        return Ok(());
    }
    let alive = match session.process.as_ref() {
        Some(process) => tmux_session_exists(&process.tmux_name)
            .await
            .map_err(|error| task_error(error.to_string()))?,
        None => false,
    };
    if alive {
        return Ok(());
    }
    let reason = "task process is missing; resume the same Task Session with `lf task resume`";
    record_task_failure(store, session, reason, reason.to_string()).await
}

pub fn task_status(issue: &str) -> OpsResult<TaskSession> {
    block_on_task(async move {
        let store = task_store().await?;
        let mut session = store
            .get_task_session_by_issue(issue)
            .await
            .map_err(|error| task_error(format!("failed to read task status: {error}")))?
            .ok_or_else(|| task_error(format!("no Task Session exists for {issue:?}")))?;
        reconcile_process_liveness(&store, &mut session).await?;
        if session.status == TaskSessionStatus::Failed {
            return Ok(session);
        }
        if session.status == TaskSessionStatus::Merged
            && matches!(session.pm_writeback, PmWritebackState::Pending { .. })
        {
            retry_pm_writeback(&mut session).await;
            store
                .update_task_session(&session)
                .await
                .map_err(|error| task_error(error.to_string()))?;
            return Ok(session);
        }
        if !session.status.is_process_active() && !session.status.is_terminal() {
            if let Some(pr) = crate::ops::current_or_merged_pr(&session.worktree)? {
                let from = session.status;
                let pull_request = crate::task::PullRequestRef {
                    number: pr.number as u32,
                    url: pr.url.clone(),
                };
                session.pull_request = Some(pull_request.clone());
                let event = if pr.state == "merged" {
                    session.set_status(
                        TaskSessionStatus::Merged,
                        format!("pull request #{} merged", pr.number),
                    );
                    reconcile_pm_writeback(&mut session).await;
                    TaskEventKind::Completed {
                        pull_request,
                        summary: "merge observed by task status".to_string(),
                    }
                } else {
                    session.set_status(
                        TaskSessionStatus::Submitted,
                        format!("pull request #{} is open for review", pr.number),
                    );
                    TaskEventKind::PullRequestOpened {
                        number: pr.number as u32,
                        url: pr.url,
                    }
                };
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
                    store
                        .append_task_event(&session.id, &event)
                        .await
                        .map_err(|error| task_error(error.to_string()))?;
                }
            }
        }
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

pub(crate) async fn reconcile_pm_writeback(session: &mut TaskSession) {
    let Some(pull_request) = session.pull_request.as_ref() else {
        return;
    };
    session.pm_writeback = writeback_state(
        crate::ops::task_pm::complete_task(
            &session.worktree,
            &session.wave,
            session.issue.id.as_str(),
            &pull_request.url,
        )
        .await,
    );
}

async fn retry_pm_writeback(session: &mut TaskSession) {
    let Some(pull_request) = session.pull_request.as_ref() else {
        return;
    };
    session.pm_writeback = writeback_state(
        crate::ops::task_pm::retry_complete_task(
            &session.worktree,
            &session.wave,
            session.issue.id.as_str(),
            &pull_request.url,
        )
        .await,
    );
    session.updated_at = time::OffsetDateTime::now_utc();
}

pub fn task_snapshot(session: &TaskSession) -> OpsResult<TaskSessionSnapshot> {
    let session = session.clone();
    block_on_task(async move {
        let store = task_store().await?;
        let process_alive = if session.status.is_process_active() {
            match session.process.as_ref() {
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
        Ok(TaskSessionSnapshot {
            issue_id: session.issue.id.as_str().to_string(),
            issue_identifier: session.issue.identifier,
            session_id: session.id.to_string(),
            project_id: session.project.id.as_str().to_string(),
            project: session.project.slug,
            pm_snapshot_synced_at: session.pm_snapshot_synced_at,
            pm_snapshot_warning: session.pm_snapshot_warning,
            pm_writeback: session.pm_writeback,
            wave: session.wave,
            supervisor: session.supervisor,
            current_directive_version: session.current_directive_version,
            incorporated_directive_version: session.incorporated_directive_version,
            status: session.status.as_str().to_string(),
            status_reason: session.status_reason,
            status_at: session.status_at,
            worktree: session.worktree.display().to_string(),
            branch: session.branch,
            base_commit: session.base_commit,
            delivery: TaskDeliveryView {
                kind: "pull_request".to_string(),
                base: "main".to_string(),
            },
            agent: session.agent,
            provider: session.provider,
            provider_session_id: session.provider_session_id,
            process_alive,
            process: session.process,
            pull_request: session.pull_request,
            latest_event,
            created_at: session.created_at,
            updated_at: session.updated_at,
        })
    })
}

fn queue_command(issue: &str, kind: TaskCommandKind) -> OpsResult<TaskControlResult> {
    block_on_task(async move {
        let store = task_store().await?;
        let mut session = store
            .get_task_session_by_issue(issue)
            .await
            .map_err(|error| task_error(format!("failed to resolve task: {error}")))?
            .ok_or_else(|| task_error(format!("no Task Session exists for {issue:?}")))?;
        reconcile_process_liveness(&store, &mut session).await?;
        if session.status.is_terminal() {
            return Err(task_error(format!(
                "task {} is {}; terminal Task Sessions cannot accept commands",
                session.issue.identifier,
                session.status.as_str()
            )));
        }
        let command = TaskCommand::new(session.id.clone(), command_source(&session)?, kind);
        let wait_for_resolution = !matches!(&command.kind, TaskCommandKind::FollowUp { .. });
        let replacement = match &command.kind {
            TaskCommandKind::Steer { text } => Some(text.clone()),
            TaskCommandKind::Interrupt {
                replacement: Some(text),
            } => Some(text.clone()),
            _ => None,
        };
        let (command, created, superseded, directive_event) = if let Some(text) = replacement {
            let directive = ChildDirective::replacement(
                ChildRef::Task(session.id.clone()),
                session.current_directive_version + 1,
                text,
                command.source.clone(),
                command.id.clone(),
            );
            let superseded = store
                .create_task_command_with_directive(&command, &directive)
                .await
                .map_err(|error| {
                    task_error(format!("failed to persist task directive: {error}"))
                })?;
            let event = (directive.id, directive.version, directive.kind);
            session.current_directive_version = directive.version;
            (command, true, superseded, Some(event))
        } else if matches!(&command.kind, TaskCommandKind::Decide { .. }) {
            let (command, created) = store
                .ensure_task_decision_command(&command)
                .await
                .map_err(|error| task_error(format!("failed to persist task decision: {error}")))?;
            (command, created, Vec::new(), None)
        } else if matches!(&command.kind, TaskCommandKind::Interrupt { .. }) {
            let superseded = store
                .supersede_and_create_task_command(&command)
                .await
                .map_err(|error| task_error(format!("failed to persist task command: {error}")))?;
            (command, true, superseded, None)
        } else {
            store
                .create_task_command(&command)
                .await
                .map_err(|error| task_error(format!("failed to persist task command: {error}")))?;
            (command, true, Vec::new(), None)
        };
        if !created {
            if !command.state.is_terminal() && !session.status.is_process_active() {
                relaunch_inactive_process(&store, &mut session).await?;
            }
            let receipt = resolve_receipt(&store, &command.id, wait_for_resolution).await?;
            return control_result(&store, &session, &command, receipt).await;
        }
        for command_id in superseded {
            append_command_event(
                &store,
                &session,
                command_id,
                TaskCommandState::Superseded,
                None,
            )
            .await?;
        }
        if let Some((directive_id, version, directive_kind)) = directive_event {
            store
                .append_task_event(
                    &session.id,
                    &TaskEventKind::DirectiveChanged {
                        directive_id,
                        version,
                        directive_kind,
                    },
                )
                .await
                .map_err(|error| task_error(error.to_string()))?;
        }
        append_command_event(
            &store,
            &session,
            command.id.clone(),
            TaskCommandState::Persisted,
            command.effect,
        )
        .await?;
        if let TaskCommandKind::Abandon { reason } = &command.kind {
            if !session.status.is_process_active() {
                let from = session.status;
                store
                    .accept_task_command(&command.id, None)
                    .await
                    .map_err(|error| task_error(error.to_string()))?;
                append_command_event(
                    &store,
                    &session,
                    command.id.clone(),
                    TaskCommandState::Accepted,
                    None,
                )
                .await?;
                session.set_status(
                    TaskSessionStatus::Abandoned,
                    format!("Task Session explicitly abandoned: {reason}"),
                );
                store
                    .update_task_session(&session)
                    .await
                    .map_err(|error| task_error(error.to_string()))?;
                store
                    .append_task_event(
                        &session.id,
                        &TaskEventKind::StatusChanged {
                            from,
                            to: TaskSessionStatus::Abandoned,
                            reason: session.status_reason.clone(),
                        },
                    )
                    .await
                    .map_err(|error| task_error(error.to_string()))?;
                let receipt = wait_for_command_receipt(&store, &command.id).await?;
                return control_result(&store, &session, &command, receipt).await;
            }
        }
        if !session.status.is_process_active() {
            relaunch_inactive_process(&store, &mut session).await?;
        }
        let mut receipt = resolve_receipt(&store, &command.id, wait_for_resolution).await?;
        if matches!(
            receipt.state,
            TaskCommandState::Persisted | TaskCommandState::Claimed
        ) {
            let current = store
                .get_task_session(&session.id)
                .await
                .map_err(|error| task_error(format!("failed to reread Task Session: {error}")))?
                .ok_or_else(|| task_error("Task Session disappeared after command persistence"))?;
            if !current.status.is_process_active() && !current.status.is_terminal() {
                session = current;
                relaunch_inactive_process(&store, &mut session).await?;
                receipt = resolve_receipt(&store, &command.id, wait_for_resolution).await?;
            }
        }
        control_result(&store, &session, &command, receipt).await
    })
}

async fn control_result(
    store: &SharedStore,
    session: &TaskSession,
    command: &TaskCommand,
    receipt: TaskCommand,
) -> OpsResult<TaskControlResult> {
    let directive = store
        .child_directive_for_command(&command.id)
        .await
        .map_err(|error| task_error(format!("failed to read task directive: {error}")))?;
    Ok(TaskControlResult {
        issue_id: session.issue.identifier.clone(),
        session_id: session.id.to_string(),
        command_id: command.id.to_string(),
        directive_version: directive.as_ref().map(|directive| directive.version),
        state: receipt.state,
        effect: receipt.effect,
        incorporated: directive
            .as_ref()
            .is_some_and(|directive| directive.incorporated_at.is_some()),
        generation: receipt.claimed_by_generation,
        accepted_at: receipt.accepted_at,
        incorporated_at: directive.and_then(|directive| directive.incorporated_at),
        error: receipt.error,
    })
}

async fn resolve_receipt(
    store: &SharedStore,
    command_id: &crate::task::TaskCommandId,
    wait: bool,
) -> OpsResult<TaskCommand> {
    if wait {
        wait_for_command_receipt(store, command_id).await
    } else {
        read_command_receipt(store, command_id).await
    }
}

async fn wait_for_command_receipt(
    store: &SharedStore,
    command_id: &crate::task::TaskCommandId,
) -> OpsResult<TaskCommand> {
    Ok(
        wait_for_command_receipt_until(store, command_id, Duration::from_secs(2))
            .await?
            .0,
    )
}

async fn wait_for_command_receipt_until(
    store: &SharedStore,
    command_id: &crate::task::TaskCommandId,
    timeout: Duration,
) -> OpsResult<(TaskCommand, bool)> {
    let deadline = tokio::time::Instant::now() + timeout;
    loop {
        let command = read_command_receipt(store, command_id).await?;
        if command.state.is_terminal() {
            return Ok((command, false));
        }
        if tokio::time::Instant::now() >= deadline {
            return Ok((command, true));
        }
        tokio::time::sleep(Duration::from_millis(50)).await;
    }
}

async fn read_command_receipt(
    store: &SharedStore,
    command_id: &crate::task::TaskCommandId,
) -> OpsResult<TaskCommand> {
    store
        .get_task_command(command_id)
        .await
        .map_err(|error| task_error(format!("failed to read task command receipt: {error}")))?
        .ok_or_else(|| task_error(format!("task command {command_id} disappeared")))
}

async fn append_command_event(
    store: &SharedStore,
    session: &TaskSession,
    command_id: crate::task::TaskCommandId,
    state: TaskCommandState,
    effect: Option<TaskCommandEffect>,
) -> OpsResult<()> {
    store
        .append_task_event(
            &session.id,
            &TaskEventKind::CommandChanged {
                command_id,
                state,
                effect,
                error: None,
            },
        )
        .await
        .map_err(|error| task_error(error.to_string()))?;
    Ok(())
}

pub fn task_follow_up(issue: &str, message: String) -> OpsResult<TaskControlResult> {
    queue_command(issue, TaskCommandKind::FollowUp { text: message })
}

pub fn task_steer(issue: &str, message: String) -> OpsResult<TaskControlResult> {
    queue_command(issue, TaskCommandKind::Steer { text: message })
}

pub fn task_interrupt(issue: &str, replacement: Option<String>) -> OpsResult<TaskControlResult> {
    queue_command(issue, TaskCommandKind::Interrupt { replacement })
}

pub fn task_resume(issue: &str, message: Option<String>) -> OpsResult<TaskControlResult> {
    queue_command(issue, TaskCommandKind::Resume { message })
}

pub fn task_receipt(
    command_id: &str,
    until: Option<ChildReceiptUntil>,
    timeout: Duration,
) -> OpsResult<TaskReceiptRead> {
    let command_id =
        TaskCommandId::parse(command_id).map_err(|error| task_error(error.to_string()))?;
    block_on_task(async move {
        let store = task_store().await?;
        let (command, timed_out) = if let Some(until) = until {
            wait_for_task_receipt_condition(&store, &command_id, until, timeout).await?
        } else {
            (read_command_receipt(&store, &command_id).await?, false)
        };
        let session = store
            .get_task_session(&command.session_id)
            .await
            .map_err(|error| task_error(format!("failed to read Task Session: {error}")))?
            .ok_or_else(|| {
                task_error(format!("Task Session {} disappeared", command.session_id))
            })?;
        Ok(TaskReceiptRead {
            receipt: control_result(&store, &session, &command, command.clone()).await?,
            timed_out,
        })
    })
}

async fn wait_for_task_receipt_condition(
    store: &SharedStore,
    command_id: &TaskCommandId,
    until: ChildReceiptUntil,
    timeout: Duration,
) -> OpsResult<(TaskCommand, bool)> {
    let deadline = tokio::time::Instant::now() + timeout;
    loop {
        let command = read_command_receipt(store, command_id).await?;
        if matches!(
            command.state,
            TaskCommandState::Failed | TaskCommandState::Superseded
        ) {
            return Ok((command, false));
        }
        let settled = match until {
            ChildReceiptUntil::Applied => command.state.is_terminal(),
            ChildReceiptUntil::Incorporated => store
                .child_directive_for_command(command_id)
                .await
                .map_err(|error| task_error(format!("failed to read task directive: {error}")))?
                .ok_or_else(|| {
                    task_error(format!(
                        "task command {command_id} does not carry a directive to incorporate"
                    ))
                })?
                .incorporated_at
                .is_some(),
        };
        if settled {
            return Ok((command, false));
        }
        if tokio::time::Instant::now() >= deadline {
            return Ok((command, true));
        }
        tokio::time::sleep(Duration::from_millis(50)).await;
    }
}

pub fn task_decide(
    issue: &str,
    decision_id: &str,
    choice: String,
    message: Option<String>,
) -> OpsResult<TaskControlResult> {
    let decision_id =
        TaskDecisionId::parse(decision_id).map_err(|error| task_error(error.to_string()))?;
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
        TaskCommandKind::Decide {
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
        let decision_id = TaskDecisionId::new();
        store
            .append_task_event(
                &session.id,
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
                    issue_id: session.issue.identifier.clone(),
                    session_id: session.id.to_string(),
                    decision_id: decision_id.to_string(),
                    resolved: true,
                    choice: Some(choice),
                    message,
                });
            }
            if !wait || Instant::now() >= deadline {
                return Ok(TaskDecisionResult {
                    issue_id: session.issue.identifier.clone(),
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
        let (directive, incorporated) = store
            .incorporate_child_directive(&ChildRef::Task(session.id.clone()), version, &summary)
            .await
            .map_err(|error| task_error(format!("failed to acknowledge directive: {error}")))?;
        if incorporated {
            store
                .append_task_event(
                    &session.id,
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
        TaskCommandKind::Abandon {
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
            TaskWaitUntil::Submitted => {
                matches!(session.status, TaskSessionStatus::Submitted)
                    || session.status.is_terminal()
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
            session.issue.identifier,
            session.status.as_str()
        )));
    }
    let tmux_name = session
        .process
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
    use super::{command_source_for_wave, project_context, TaskControlResult};
    use crate::lfd::pm::{PmKr, PmProject};
    use crate::task::TaskCommandSource;

    #[test]
    fn task_context_captures_project_definition_and_kr_state() {
        let project = PmProject {
            id: "project-1".to_string(),
            slug: "delivery".to_string(),
            name: "Delivery".to_string(),
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
        };

        assert_eq!(
            project_context(&project),
            "Definition:\nEvery task has one durable session.\n\nKRs:\n- [x] Review resumes the same session\n- [ ] Merge wakes the Wave"
        );
    }

    #[test]
    fn foreign_wave_cannot_be_reclassified_as_a_human_command() {
        let wave_id = crate::lfd::id::LfdId::new();

        assert!(matches!(
            command_source_for_wave(Some(wave_id.clone()), &wave_id, "INF-123", "infrastructure")
                .unwrap(),
            TaskCommandSource::Wave(_)
        ));
        assert!(command_source_for_wave(
            Some(crate::lfd::id::LfdId::new()),
            &wave_id,
            "INF-123",
            "infrastructure"
        )
        .is_err());
        assert_eq!(
            command_source_for_wave(None, &wave_id, "INF-123", "infrastructure").unwrap(),
            TaskCommandSource::Human
        );
    }

    #[test]
    fn task_control_json_reports_durable_state_and_effect() {
        let result = TaskControlResult {
            issue_id: "INF-123".to_string(),
            session_id: "ts_example".to_string(),
            command_id: "cc_example".to_string(),
            directive_version: Some(2),
            state: crate::task::TaskCommandState::Accepted,
            effect: Some(crate::task::TaskCommandEffect::LiveSteer),
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
}
