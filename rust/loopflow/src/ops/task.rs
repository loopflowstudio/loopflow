use std::path::Path;
use std::sync::Arc;
use std::time::{Duration, Instant};

use crate::engine::config::{load_config_or_default, parse_agent};
use crate::engine::git::{get_default_branch, rev_parse};
use crate::engine::worktrees::{
    create_from_placement_plan, main_repo_root, plan_placement, PlacementRequest,
    PlacementStrategy, WorktreeSegment,
};
use crate::lfd::executor::helpers::{
    resolve_lf_binary, spawn_detached_lf_with_env, tmux_session_exists, tmux_session_slug,
};
use crate::lfd::id::LfdId;
use crate::lfdb::{open_existing_store, SharedStore, StoreError};
use crate::ops::error::{OpsError, OpsResult};
use crate::task::{
    LinearIssueId, LinearIssueRef, LinearProjectId, LinearProjectRef, PmWritebackOperation,
    PmWritebackState, TaskCommand, TaskCommandKind, TaskCommandSource, TaskEventKind, TaskSession,
    TaskSessionStatus,
};
use sha2::{Digest, Sha256};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TaskWaitUntil {
    Submitted,
    Terminal,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct TaskControlResult {
    pub issue_id: String,
    pub session_id: String,
    pub command_id: String,
    pub delivery: String,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct ProjectRunResult {
    pub project_id: String,
    pub project: String,
    pub wave: String,
    pub delivery: String,
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

fn command_source(session: &TaskSession) -> TaskCommandSource {
    std::env::var(crate::lf::session::WAVE_ID_ENV)
        .ok()
        .and_then(|value| LfdId::parse(&value).ok())
        .filter(|wave_id| wave_id == &session.wave_id)
        .map(TaskCommandSource::Wave)
        .unwrap_or(TaskCommandSource::Human)
}

pub fn task_run(repo: &Path, issue: &str) -> OpsResult<TaskSession> {
    if let Some(existing) = block_on_task(async {
        let store = task_store().await?;
        store
            .get_task_session_by_issue(issue)
            .await
            .map_err(|error| task_error(format!("failed to read task registry: {error}")))
    })? {
        return Ok(existing);
    }
    let resolved = crate::ops::task_pm::resolve_task(repo, issue, crate::ops::pm::PmRefresh::Auto)?;
    let main_repo = main_repo_root(repo).map_err(|error| task_error(error.to_string()))?;
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
            pm_snapshot_synced_at: resolved.snapshot.synced_at,
            pm_snapshot_warning: resolved.snapshot.refresh_warning.clone(),
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

        match store
            .reserve_task_session(&session, wave.workers.max(1))
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

        if let Err(error) = create_from_placement_plan(&main_repo, &plan) {
            session.set_status(
                TaskSessionStatus::Failed,
                format!("worktree creation failed: {error}"),
            );
            store.update_task_session(&session).await.map_err(|store_error| {
                task_error(format!(
                    "worktree creation failed ({error}); recording failure also failed: {store_error}"
                ))
            })?;
            store
                .append_task_event(
                    &session.id,
                    &TaskEventKind::Failed {
                        error: error.to_string(),
                        resumable: true,
                    },
                )
                .await
                .map_err(|store_error| task_error(store_error.to_string()))?;
            return Err(task_error(format!(
                "failed to create task worktree: {error}"
            )));
        }

        launch_task_process(&store, &mut session).await?;
        wait_until_running(&store, &session.id).await
    })
}

fn project_context(project: &crate::lfd::pm::PmProject) -> String {
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

pub fn task_start(repo: &Path, title: String, project_id: &str) -> OpsResult<TaskSession> {
    let project =
        crate::ops::task_pm::resolve_project(repo, project_id, crate::ops::pm::PmRefresh::Auto)?;
    let marker = format!(
        "<!-- loopflow-task-start:{} -->",
        hex::encode(Sha256::digest(
            format!("{}\0{}", project.project.id, title).as_bytes()
        ))
    );
    let created = crate::ops::task_pm::create_and_load_task(
        repo,
        &project.snapshot.wave,
        &project.project.slug,
        &title,
        &marker,
    )?;
    task_run(repo, &created.item.id)
}

pub fn project_run(repo: &Path, project_id: &str) -> OpsResult<ProjectRunResult> {
    let project =
        crate::ops::task_pm::resolve_project(repo, project_id, crate::ops::pm::PmRefresh::Auto)?;
    let message = format!(
        "Run Linear Project {} ({}) under wave/{}. Evaluate its definition and KRs, then select or create concrete Linear tasks and execute each through a Task Session.",
        project.project.name, project.project.id, project.snapshot.wave
    );
    let wave = project.snapshot.wave.clone();
    let live = block_on_task(async move {
        crate::lf::commands::chat::post_to_named_wave(&wave, &message)
            .await
            .map_err(|error| task_error(format!("failed to queue Project directive: {error}")))
    })?;
    Ok(ProjectRunResult {
        project_id: project.project.id,
        project: project.project.name,
        wave: project.snapshot.wave,
        delivery: if live { "live" } else { "queued" }.to_string(),
    })
}

pub fn project_start(repo: &Path, title: &str, wave: Option<&str>) -> OpsResult<ProjectRunResult> {
    let project = crate::ops::pm::pm_create_project(repo, wave, title)?;
    project_run(repo, &project.project.id)
}

async fn launch_task_process(store: &SharedStore, session: &mut TaskSession) -> OpsResult<()> {
    let tmux_name = format!(
        "lf-task-{}-{}",
        tmux_session_slug(&session.issue.identifier),
        &session.id.as_str()[3..11]
    );
    let from = session.status;
    let generation = session.begin_generation(tmux_name.clone());
    store
        .update_task_session(session)
        .await
        .map_err(|error| task_error(format!("failed to record task launch: {error}")))?;
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
        spawn_detached_lf_with_env(&tmux_name, &session.worktree, &argv, &environment).await
    {
        session.set_status(
            TaskSessionStatus::Failed,
            format!("task process launch failed: {error}"),
        );
        store
            .update_task_session(session)
            .await
            .map_err(|store_error| task_error(store_error.to_string()))?;
        store
            .append_task_event(
                &session.id,
                &TaskEventKind::Failed {
                    error: error.to_string(),
                    resumable: true,
                },
            )
            .await
            .map_err(|store_error| task_error(store_error.to_string()))?;
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

pub fn task_status(issue: &str) -> OpsResult<TaskSession> {
    block_on_task(async move {
        let store = task_store().await?;
        let mut session = store
            .get_task_session_by_issue(issue)
            .await
            .map_err(|error| task_error(format!("failed to read task status: {error}")))?
            .ok_or_else(|| task_error(format!("no Task Session exists for {issue:?}")))?;
        if session.status.is_process_active() {
            let alive = match session.process.as_ref() {
                Some(process) => tmux_session_exists(&process.tmux_name)
                    .await
                    .map_err(|error| task_error(error.to_string()))?,
                None => false,
            };
            if !alive {
                let from = session.status;
                session.set_status(
                    TaskSessionStatus::Failed,
                    "task process is missing; resume the same Task Session with `lf task resume`",
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
                            to: TaskSessionStatus::Failed,
                            reason: session.status_reason.clone(),
                        },
                    )
                    .await
                    .map_err(|error| task_error(error.to_string()))?;
                store
                    .append_task_event(
                        &session.id,
                        &TaskEventKind::Failed {
                            error: session.status_reason.clone(),
                            resumable: true,
                        },
                    )
                    .await
                    .map_err(|error| task_error(error.to_string()))?;
                crate::lf::commands::chat::post_to_named_wave(
                    &session.wave,
                    &format!(
                        "Task {} → failed: {}",
                        session.issue.identifier, session.status_reason
                    ),
                )
                .await
                .map_err(|error| {
                    task_error(format!("failed to mirror task failure to Wave: {error}"))
                })?;
                return Ok(session);
            }
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
                        .append_task_event(&session.id, &event)
                        .await
                        .map_err(|error| task_error(error.to_string()))?;
                    crate::lf::commands::chat::post_to_named_wave(
                        &session.wave,
                        &format!(
                            "Task {} → {}: {}",
                            session.issue.identifier,
                            session.status.as_str(),
                            session.status_reason
                        ),
                    )
                    .await
                    .map_err(|error| task_error(error.to_string()))?;
                }
            }
        }
        Ok(session)
    })
}

async fn reconcile_pm_writeback(session: &mut TaskSession) {
    let Some(pull_request) = session.pull_request.as_ref() else {
        return;
    };
    session.pm_writeback = match crate::ops::task_pm::complete_task(
        &session.worktree,
        &session.wave,
        session.issue.id.as_str(),
        &pull_request.url,
    )
    .await
    {
        Ok(()) => PmWritebackState::Current,
        Err(error) => PmWritebackState::Pending {
            operation: PmWritebackOperation::CompleteTask,
            error: error.to_string(),
        },
    };
}

async fn retry_pm_writeback(session: &mut TaskSession) {
    session.pm_writeback = match crate::ops::task_pm::retry_complete_task(
        &session.worktree,
        &session.wave,
        session.issue.id.as_str(),
    )
    .await
    {
        Ok(()) => PmWritebackState::Current,
        Err(error) => PmWritebackState::Pending {
            operation: PmWritebackOperation::CompleteTask,
            error: error.to_string(),
        },
    };
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
        if session.status.is_terminal() {
            return Err(task_error(format!(
                "task {} is {}; terminal Task Sessions cannot accept commands",
                session.issue.identifier,
                session.status.as_str()
            )));
        }
        let command = TaskCommand::new(session.id.clone(), command_source(&session), kind);
        store
            .create_task_command(&command)
            .await
            .map_err(|error| task_error(format!("failed to persist task command: {error}")))?;
        crate::lf::commands::chat::post_to_named_wave(
            &session.wave,
            &format!(
                "Task command {} → {} ({})",
                command.id,
                session.issue.identifier,
                command_kind_name(&command.kind)
            ),
        )
        .await
        .map_err(|error| task_error(format!("failed to mirror task command to Wave: {error}")))?;
        if let TaskCommandKind::Abandon { reason } = &command.kind {
            if !session.status.is_process_active() {
                let from = session.status;
                store
                    .acknowledge_task_command(&command.id)
                    .await
                    .map_err(|error| task_error(error.to_string()))?;
                store
                    .append_task_event(
                        &session.id,
                        &TaskEventKind::CommandAccepted {
                            command_id: command.id.clone(),
                        },
                    )
                    .await
                    .map_err(|error| task_error(error.to_string()))?;
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
                return Ok(TaskControlResult {
                    issue_id: session.issue.identifier,
                    session_id: session.id.to_string(),
                    command_id: command.id.to_string(),
                    delivery: "accepted".to_string(),
                });
            }
        }
        let delivery = if session.status.is_process_active() {
            if session.status == TaskSessionStatus::Running && session.provider == "codex" {
                "live"
            } else {
                "queued"
            }
        } else {
            launch_task_process(&store, &mut session).await?;
            "queued"
        };
        Ok(TaskControlResult {
            issue_id: session.issue.identifier,
            session_id: session.id.to_string(),
            command_id: command.id.to_string(),
            delivery: delivery.to_string(),
        })
    })
}

fn command_kind_name(kind: &TaskCommandKind) -> &'static str {
    match kind {
        TaskCommandKind::Message { .. } => "message",
        TaskCommandKind::Interrupt { .. } => "interrupt",
        TaskCommandKind::Resume { .. } => "resume",
        TaskCommandKind::Abandon { .. } => "abandon",
    }
}

pub fn task_send(issue: &str, message: String) -> OpsResult<TaskControlResult> {
    queue_command(issue, TaskCommandKind::Message { text: message })
}

pub fn task_interrupt(issue: &str, next_message: Option<String>) -> OpsResult<TaskControlResult> {
    queue_command(issue, TaskCommandKind::Interrupt { next_message })
}

pub fn task_resume(issue: &str, message: Option<String>) -> OpsResult<TaskControlResult> {
    queue_command(issue, TaskCommandKind::Resume { message })
}

pub fn task_abandon(issue: &str, reason: String) -> OpsResult<TaskControlResult> {
    queue_command(issue, TaskCommandKind::Abandon { reason })
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
        std::thread::sleep(Duration::from_millis(250));
    }
}

pub fn task_attach(issue: &str) -> OpsResult<()> {
    let session = task_status(issue)?;
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
    use super::project_context;
    use crate::lfd::pm::{PmKr, PmProject};

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
}
