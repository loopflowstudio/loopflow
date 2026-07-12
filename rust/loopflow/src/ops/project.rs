use std::path::Path;
use std::sync::Arc;
use std::time::{Duration, Instant};

use crate::engine::config::{load_config_or_default, parse_agent};
use crate::engine::process::{
    resolve_lf_binary, start_lf_session, start_lf_session_with_env, tmux_session_exists,
    tmux_session_slug,
};
use crate::lfd::id::LfdId;
use crate::lfd::types::Wave;
use crate::lfdb::{open_existing_store, SharedStore, Store};
use crate::ops::{OpsError, OpsResult};
use crate::project_session::{
    ProjectCommand, ProjectCommandId, ProjectCommandKind, ProjectCommandSource, ProjectDecisionId,
    ProjectEventKind, ProjectSession, ProjectSessionId, ProjectSessionStatus,
};
use crate::task::{LinearProjectId, LinearProjectRef, TaskCommandEffect, TaskCommandState};

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct ProjectSessionSnapshot {
    pub project_id: String,
    pub project_slug: String,
    pub project_name: String,
    pub session_id: String,
    pub wave: String,
    pub status: String,
    pub status_reason: String,
    pub status_at: time::OffsetDateTime,
    pub iteration: u32,
    pub task_event_cursor: i64,
    pub pending_observations: u32,
    pub agent: String,
    pub provider: String,
    pub provider_session_id: Option<String>,
    pub process_alive: bool,
    pub process: Option<crate::project_session::ProjectProcess>,
    pub latest_event: Option<crate::project_session::ProjectEvent>,
    pub created_at: time::OffsetDateTime,
    pub updated_at: time::OffsetDateTime,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct ProjectControlResult {
    pub project_id: String,
    pub session_id: String,
    pub command_id: String,
    pub state: TaskCommandState,
    pub effect: Option<TaskCommandEffect>,
    pub generation: Option<u32>,
    pub accepted_at: Option<time::OffsetDateTime>,
    pub error: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProjectReceiptRead {
    pub receipt: ProjectControlResult,
    pub timed_out: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct ProjectDecisionResult {
    pub project_id: String,
    pub session_id: String,
    pub decision_id: String,
    pub resolved: bool,
    pub choice: Option<String>,
    pub message: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProjectWaitUntil {
    Waiting,
    Terminal,
}

fn project_error(message: impl Into<String>) -> OpsError {
    OpsError::Message(message.into())
}

fn block_on_project<T>(future: impl std::future::Future<Output = OpsResult<T>>) -> OpsResult<T> {
    tokio::runtime::Runtime::new()
        .map_err(|error| project_error(format!("failed to build project runtime: {error}")))?
        .block_on(future)
}

async fn project_store() -> OpsResult<SharedStore> {
    open_existing_store().await.map(Arc::new).ok_or_else(|| {
        project_error("no Loopflow registry on this machine; serve the owning Wave first")
    })
}

pub fn project_run(repo: &Path, project_id: &str) -> OpsResult<ProjectSession> {
    if let Some(existing) = block_on_project(async {
        let store = project_store().await?;
        let mut existing = store
            .get_project_session_by_project(project_id)
            .await
            .map_err(|error| project_error(format!("failed to read Project Session: {error}")))?;
        if let Some(session) = &mut existing {
            reconcile_project_liveness(&store, session).await?;
        }
        Ok(existing)
    })? {
        if existing.status.is_terminal() || existing.status.is_process_active() {
            return Ok(existing);
        }
        return block_on_project(async move {
            let store = project_store().await?;
            let mut existing = existing;
            launch_project_process(&store, &mut existing).await?;
            wait_until_project_running(&store, &existing.id).await
        });
    }

    let resolved =
        crate::ops::task_pm::resolve_project(repo, project_id, crate::ops::pm::PmRefresh::Auto)?;
    let config = load_config_or_default(Some(repo));
    let agent = config.agent.as_deref().unwrap_or("codex");
    let (provider, _) = parse_agent(agent);
    let agent = agent.to_string();
    let repo = crate::engine::worktrees::main_repo_root(repo)
        .map_err(|error| project_error(error.to_string()))?;

    block_on_project(async move {
        let store = project_store().await?;
        if let Some(existing) = store
            .get_project_session_by_project(&resolved.project.id)
            .await
            .map_err(|error| project_error(format!("failed to read Project Session: {error}")))?
        {
            return Ok(existing);
        }
        let wave = store
            .get_wave_by_name(&resolved.snapshot.wave)
            .await
            .map_err(|error| project_error(format!("failed to read owning Wave: {error}")))?
            .ok_or_else(|| {
                project_error(format!(
                    "owning Wave {:?} is not registered",
                    resolved.snapshot.wave
                ))
            })?;
        let now = time::OffsetDateTime::now_utc();
        let context = crate::ops::task::project_context(&resolved.project);
        let mut session = ProjectSession {
            id: ProjectSessionId::new(),
            project: LinearProjectRef {
                id: LinearProjectId::new(resolved.project.id.clone())
                    .map_err(|error| project_error(error.to_string()))?,
                slug: resolved.project.slug,
                name: resolved.project.name,
                context,
            },
            wave_id: wave.id().clone(),
            wave: resolved.snapshot.wave,
            repo: repo.display().to_string(),
            pm_snapshot_synced_at: resolved.snapshot.synced_at,
            status: ProjectSessionStatus::Created,
            status_reason: "Linear Project reserved for pursuit".to_string(),
            status_at: now,
            iteration: 0,
            task_event_cursor: 0,
            state_fingerprint: None,
            agent,
            provider,
            provider_session_id: None,
            process: None,
            created_at: now,
            updated_at: now,
        };
        if let Err(error) = store.create_project_session(&session).await {
            if let Some(existing) = store
                .get_project_session_by_project(session.project.id.as_str())
                .await
                .map_err(|read_error| project_error(read_error.to_string()))?
            {
                return Ok(existing);
            }
            return Err(project_error(format!(
                "failed to reserve Project Session: {error}"
            )));
        }
        launch_project_process(&store, &mut session).await?;
        wait_until_project_running(&store, &session.id).await
    })
}

pub fn project_start(repo: &Path, title: &str, wave: Option<&str>) -> OpsResult<ProjectSession> {
    let project = crate::ops::pm::pm_create_project(repo, wave, title)?;
    project_run(repo, &project.project.id)
}

async fn launch_project_process(
    store: &SharedStore,
    session: &mut ProjectSession,
) -> OpsResult<()> {
    let tmux_name = format!(
        "lf-project-{}-{}",
        tmux_session_slug(&session.project.slug),
        &session.id.as_str()[3..11]
    );
    let from = session.status;
    let mut launch = session.clone();
    let generation = launch.begin_generation(tmux_name.clone());
    let reserved = store
        .reserve_project_process(&launch, from)
        .await
        .map_err(|error| project_error(format!("failed to reserve project process: {error}")))?;
    if !reserved {
        let current = store
            .get_project_session(&session.id)
            .await
            .map_err(|error| project_error(error.to_string()))?
            .ok_or_else(|| project_error("Project Session disappeared during launch"))?;
        if current.status.is_process_active() {
            *session = current;
            return Ok(());
        }
        return Err(project_error(format!(
            "Project Session {} changed while its process was reserved; retry",
            session.id
        )));
    }
    *session = launch;
    store
        .append_project_event(
            &session.id,
            &ProjectEventKind::StatusChanged {
                from,
                to: ProjectSessionStatus::Starting,
                reason: session.status_reason.clone(),
            },
        )
        .await
        .map_err(|error| project_error(error.to_string()))?;

    let argv = vec![
        resolve_lf_binary().to_string_lossy().to_string(),
        "__project".to_string(),
        session.id.to_string(),
        "--generation".to_string(),
        generation.to_string(),
    ];
    let generation_text = generation.to_string();
    let environment = [
        (crate::lf::session::WAVE_ID_ENV, session.wave_id.as_str()),
        ("LFD_PROJECT_SESSION_ID", session.id.as_str()),
        ("LFD_PROJECT_GENERATION", generation_text.as_str()),
    ];
    if let Err(error) =
        start_lf_session_with_env(&tmux_name, Path::new(&session.repo), &argv, &environment).await
    {
        let reason = format!("project process launch failed: {error}");
        session.set_status(ProjectSessionStatus::Failed, reason.clone());
        store
            .update_project_session(session)
            .await
            .map_err(|store_error| project_error(store_error.to_string()))?;
        store
            .append_project_event(
                &session.id,
                &ProjectEventKind::Failed {
                    error: reason.clone(),
                    resumable: true,
                },
            )
            .await
            .map_err(|store_error| project_error(store_error.to_string()))?;
        return Err(project_error(reason));
    }
    Ok(())
}

async fn wait_until_project_running(
    store: &SharedStore,
    session_id: &ProjectSessionId,
) -> OpsResult<ProjectSession> {
    let deadline = tokio::time::Instant::now() + Duration::from_secs(10);
    loop {
        let session = store
            .get_project_session(session_id)
            .await
            .map_err(|error| project_error(error.to_string()))?
            .ok_or_else(|| project_error("Project Session disappeared during startup"))?;
        if session.status != ProjectSessionStatus::Starting {
            return if session.status == ProjectSessionStatus::Running {
                Ok(session)
            } else {
                Err(project_error(format!(
                    "Project {} did not start: {}",
                    session.project.slug, session.status_reason
                )))
            };
        }
        if tokio::time::Instant::now() >= deadline {
            return Err(project_error(format!(
                "Project {} did not become running within 10s",
                session.project.slug
            )));
        }
        tokio::time::sleep(Duration::from_millis(100)).await;
    }
}

async fn reconcile_project_liveness(
    store: &SharedStore,
    session: &mut ProjectSession,
) -> OpsResult<()> {
    if !session.status.is_process_active() {
        return Ok(());
    }
    let alive = match session.process.as_ref() {
        Some(process) => tmux_session_exists(&process.tmux_name)
            .await
            .map_err(|error| project_error(error.to_string()))?,
        None => false,
    };
    if alive {
        return Ok(());
    }
    let from = session.status;
    session.set_status(
        ProjectSessionStatus::Failed,
        "project process is not running; resume the Project Session",
    );
    store
        .update_project_session(session)
        .await
        .map_err(|error| project_error(error.to_string()))?;
    store
        .append_project_event(
            &session.id,
            &ProjectEventKind::StatusChanged {
                from,
                to: ProjectSessionStatus::Failed,
                reason: session.status_reason.clone(),
            },
        )
        .await
        .map_err(|error| project_error(error.to_string()))?;
    Ok(())
}

pub fn project_status(project: &str) -> OpsResult<ProjectSession> {
    block_on_project(async move {
        let store = project_store().await?;
        let mut session = store
            .get_project_session_by_project(project)
            .await
            .map_err(|error| project_error(error.to_string()))?
            .ok_or_else(|| project_error(format!("no Project Session exists for {project:?}")))?;
        reconcile_project_liveness(&store, &mut session).await?;
        Ok(session)
    })
}

pub fn project_snapshot(session: &ProjectSession) -> OpsResult<ProjectSessionSnapshot> {
    let session = session.clone();
    block_on_project(async move {
        let store = project_store().await?;
        let process_alive = if session.status.is_process_active() {
            match session.process.as_ref() {
                Some(process) => tmux_session_exists(&process.tmux_name)
                    .await
                    .map_err(|error| project_error(error.to_string()))?,
                None => false,
            }
        } else {
            false
        };
        let latest_event = store
            .project_events_after(&session.id, 0)
            .await
            .map_err(|error| project_error(error.to_string()))?
            .into_iter()
            .last();
        let pending_observations = store
            .pending_observations(&crate::project_session::SessionSupervisor::Project {
                session_id: session.id.clone(),
            })
            .await
            .map_err(|error| project_error(error.to_string()))?
            .len() as u32;
        Ok(ProjectSessionSnapshot {
            project_id: session.project.id.as_str().to_string(),
            project_slug: session.project.slug,
            project_name: session.project.name,
            session_id: session.id.to_string(),
            wave: session.wave,
            status: session.status.as_str().to_string(),
            status_reason: session.status_reason,
            status_at: session.status_at,
            iteration: session.iteration,
            task_event_cursor: session.task_event_cursor,
            pending_observations,
            agent: session.agent,
            provider: session.provider,
            provider_session_id: session.provider_session_id,
            process_alive,
            process: session.process,
            latest_event,
            created_at: session.created_at,
            updated_at: session.updated_at,
        })
    })
}

fn project_command_source(session: &ProjectSession) -> OpsResult<ProjectCommandSource> {
    match std::env::var(crate::lf::session::WAVE_ID_ENV) {
        Ok(value) => {
            let wave_id = LfdId::parse(&value)
                .map_err(|error| project_error(format!("invalid ambient Wave id: {error}")))?;
            if wave_id != session.wave_id {
                return Err(project_error(format!(
                    "Wave {wave_id} cannot control Project {} owned by wave/{}",
                    session.project.slug, session.wave
                )));
            }
            Ok(ProjectCommandSource::Wave(wave_id))
        }
        Err(std::env::VarError::NotPresent) => Ok(ProjectCommandSource::Human),
        Err(std::env::VarError::NotUnicode(_)) => {
            Err(project_error("ambient Wave id is not valid UTF-8"))
        }
    }
}

fn queue_project_command(
    project: &str,
    kind: ProjectCommandKind,
) -> OpsResult<ProjectControlResult> {
    block_on_project(async move {
        let store = project_store().await?;
        let mut session = store
            .get_project_session_by_project(project)
            .await
            .map_err(|error| project_error(error.to_string()))?
            .ok_or_else(|| project_error(format!("no Project Session exists for {project:?}")))?;
        reconcile_project_liveness(&store, &mut session).await?;
        if session.status.is_terminal() {
            return Err(project_error(format!(
                "Project {} is {}; terminal Project Sessions cannot accept commands",
                session.project.slug,
                session.status.as_str()
            )));
        }
        let command =
            ProjectCommand::new(session.id.clone(), project_command_source(&session)?, kind);
        let (command, created, superseded) =
            if matches!(command.kind, ProjectCommandKind::Decide { .. }) {
                let (command, created) = store
                    .ensure_project_decision_command(&command)
                    .await
                    .map_err(|error| project_error(error.to_string()))?;
                (command, created, Vec::new())
            } else if matches!(command.kind, ProjectCommandKind::Interrupt { .. }) {
                let superseded = store
                    .supersede_and_create_project_command(&command)
                    .await
                    .map_err(|error| project_error(error.to_string()))?;
                (command, true, superseded)
            } else {
                store
                    .create_project_command(&command)
                    .await
                    .map_err(|error| project_error(error.to_string()))?;
                (command, true, Vec::new())
            };
        if !created {
            if !command.state.is_terminal() && !session.status.is_process_active() {
                launch_project_process(&store, &mut session).await?;
            }
            let receipt = wait_for_project_receipt(&store, &command.id, Duration::from_secs(2))
                .await?
                .0;
            return Ok(project_control_result(&session, &command, receipt));
        }
        for command_id in superseded {
            append_project_command_event(
                &store,
                &session,
                command_id,
                TaskCommandState::Superseded,
                None,
            )
            .await?;
        }
        append_project_command_event(
            &store,
            &session,
            command.id.clone(),
            TaskCommandState::Persisted,
            command.effect,
        )
        .await?;
        if !session.status.is_process_active() {
            launch_project_process(&store, &mut session).await?;
        }
        let receipt = wait_for_project_receipt(&store, &command.id, Duration::from_secs(2))
            .await?
            .0;
        Ok(project_control_result(&session, &command, receipt))
    })
}

async fn append_project_command_event(
    store: &SharedStore,
    session: &ProjectSession,
    command_id: ProjectCommandId,
    state: TaskCommandState,
    effect: Option<TaskCommandEffect>,
) -> OpsResult<()> {
    store
        .append_project_event(
            &session.id,
            &ProjectEventKind::CommandChanged {
                command_id,
                state,
                effect,
                error: None,
            },
        )
        .await
        .map_err(|error| project_error(error.to_string()))?;
    Ok(())
}

fn project_control_result(
    session: &ProjectSession,
    command: &ProjectCommand,
    receipt: ProjectCommand,
) -> ProjectControlResult {
    ProjectControlResult {
        project_id: session.project.id.as_str().to_string(),
        session_id: session.id.to_string(),
        command_id: command.id.to_string(),
        state: receipt.state,
        effect: receipt.effect,
        generation: receipt.claimed_by_generation,
        accepted_at: receipt.accepted_at,
        error: receipt.error,
    }
}

async fn wait_for_project_receipt(
    store: &SharedStore,
    command_id: &ProjectCommandId,
    timeout: Duration,
) -> OpsResult<(ProjectCommand, bool)> {
    let deadline = tokio::time::Instant::now() + timeout;
    loop {
        let command = store
            .get_project_command(command_id)
            .await
            .map_err(|error| project_error(error.to_string()))?
            .ok_or_else(|| project_error(format!("project command {command_id} disappeared")))?;
        if command.state.is_terminal() {
            return Ok((command, false));
        }
        if tokio::time::Instant::now() >= deadline {
            return Ok((command, true));
        }
        tokio::time::sleep(Duration::from_millis(50)).await;
    }
}

pub fn project_follow_up(project: &str, message: String) -> OpsResult<ProjectControlResult> {
    queue_project_command(project, ProjectCommandKind::FollowUp { text: message })
}

pub fn project_steer(project: &str, message: String) -> OpsResult<ProjectControlResult> {
    queue_project_command(project, ProjectCommandKind::Steer { text: message })
}

pub fn project_interrupt(
    project: &str,
    replacement: Option<String>,
) -> OpsResult<ProjectControlResult> {
    queue_project_command(project, ProjectCommandKind::Interrupt { replacement })
}

pub fn project_resume(project: &str, message: Option<String>) -> OpsResult<ProjectControlResult> {
    queue_project_command(project, ProjectCommandKind::Resume { message })
}

pub fn project_decide(
    project: &str,
    decision_id: &str,
    choice: String,
    message: Option<String>,
) -> OpsResult<ProjectControlResult> {
    let decision_id =
        ProjectDecisionId::parse(decision_id).map_err(|error| project_error(error.to_string()))?;
    let choice = choice.trim().to_string();
    if choice.is_empty() {
        return Err(project_error("decision choice cannot be empty"));
    }
    let options = block_on_project(async {
        let store = project_store().await?;
        let session = store
            .get_project_session_by_project(project)
            .await
            .map_err(|error| project_error(error.to_string()))?
            .ok_or_else(|| project_error(format!("no Project Session exists for {project:?}")))?;
        store
            .project_events_after(&session.id, 0)
            .await
            .map_err(|error| project_error(error.to_string()))?
            .into_iter()
            .find_map(|event| match event.kind {
                ProjectEventKind::DecisionRequested {
                    decision_id: requested,
                    options,
                    ..
                } if requested == decision_id => Some(options),
                _ => None,
            })
            .ok_or_else(|| project_error(format!("decision {decision_id} was not requested")))
    })?;
    if !options.iter().any(|option| option == &choice) {
        return Err(project_error(format!(
            "choice {choice:?} is not one of: {}",
            options.join(", ")
        )));
    }
    queue_project_command(
        project,
        ProjectCommandKind::Decide {
            decision_id,
            choice,
            message,
        },
    )
}

pub fn project_request_decision(
    project: &str,
    prompt: String,
    options: Vec<String>,
    wait: bool,
    timeout: Duration,
) -> OpsResult<ProjectDecisionResult> {
    let prompt = prompt.trim().to_string();
    let options: Vec<String> = options
        .into_iter()
        .map(|option| option.trim().to_string())
        .filter(|option| !option.is_empty())
        .collect();
    if prompt.is_empty() || options.len() < 2 {
        return Err(project_error(
            "a decision requires a non-empty prompt and at least two options",
        ));
    }
    block_on_project(async move {
        let store = project_store().await?;
        let session = store
            .get_project_session_by_project(project)
            .await
            .map_err(|error| project_error(error.to_string()))?
            .ok_or_else(|| project_error(format!("no Project Session exists for {project:?}")))?;
        let ambient = std::env::var("LFD_PROJECT_SESSION_ID").map_err(|_| {
            project_error("decision requests must run inside the owning Project Session")
        })?;
        if ambient != session.id.as_str() {
            return Err(project_error(format!(
                "Project Session {ambient} cannot request a decision for {}",
                session.id
            )));
        }
        let decision_id = ProjectDecisionId::new();
        store
            .append_project_event(
                &session.id,
                &ProjectEventKind::DecisionRequested {
                    decision_id: decision_id.clone(),
                    prompt,
                    options,
                },
            )
            .await
            .map_err(|error| project_error(error.to_string()))?;
        if !wait {
            return Ok(ProjectDecisionResult {
                project_id: session.project.id.as_str().to_string(),
                session_id: session.id.to_string(),
                decision_id: decision_id.to_string(),
                resolved: false,
                choice: None,
                message: None,
            });
        }
        let deadline = tokio::time::Instant::now() + timeout;
        loop {
            let resolution = store
                .project_events_after(&session.id, 0)
                .await
                .map_err(|error| project_error(error.to_string()))?
                .into_iter()
                .find_map(|event| match event.kind {
                    ProjectEventKind::DecisionResolved {
                        decision_id: resolved,
                        choice,
                        message,
                    } if resolved == decision_id => Some((choice, message)),
                    _ => None,
                });
            if let Some((choice, message)) = resolution {
                return Ok(ProjectDecisionResult {
                    project_id: session.project.id.as_str().to_string(),
                    session_id: session.id.to_string(),
                    decision_id: decision_id.to_string(),
                    resolved: true,
                    choice: Some(choice),
                    message,
                });
            }
            if tokio::time::Instant::now() >= deadline {
                return Ok(ProjectDecisionResult {
                    project_id: session.project.id.as_str().to_string(),
                    session_id: session.id.to_string(),
                    decision_id: decision_id.to_string(),
                    resolved: false,
                    choice: None,
                    message: None,
                });
            }
            tokio::time::sleep(Duration::from_millis(100)).await;
        }
    })
}

pub fn project_abandon(project: &str, reason: String) -> OpsResult<ProjectControlResult> {
    queue_project_command(project, ProjectCommandKind::Abandon { reason })
}

pub fn project_receipt(
    command_id: &str,
    wait: bool,
    timeout: Duration,
) -> OpsResult<ProjectReceiptRead> {
    let command_id =
        ProjectCommandId::parse(command_id).map_err(|error| project_error(error.to_string()))?;
    block_on_project(async move {
        let store = project_store().await?;
        let (command, timed_out) = if wait {
            wait_for_project_receipt(&store, &command_id, timeout).await?
        } else {
            let command = store
                .get_project_command(&command_id)
                .await
                .map_err(|error| project_error(error.to_string()))?
                .ok_or_else(|| project_error(format!("project command {command_id} not found")))?;
            (command, false)
        };
        let session = store
            .get_project_session(&command.session_id)
            .await
            .map_err(|error| project_error(error.to_string()))?
            .ok_or_else(|| project_error("Project Session disappeared"))?;
        Ok(ProjectReceiptRead {
            receipt: project_control_result(&session, &command, command.clone()),
            timed_out,
        })
    })
}

pub fn project_wait(
    project: &str,
    until: ProjectWaitUntil,
    timeout: Option<Duration>,
) -> OpsResult<ProjectSession> {
    let start = Instant::now();
    loop {
        let session = project_status(project)?;
        let done = match until {
            ProjectWaitUntil::Waiting => !session.status.is_process_active(),
            ProjectWaitUntil::Terminal => session.status.is_terminal(),
        };
        if done {
            return Ok(session);
        }
        if timeout.is_some_and(|timeout| start.elapsed() >= timeout) {
            return Ok(session);
        }
        std::thread::sleep(Duration::from_millis(200));
    }
}

pub fn project_attach(project: &str) -> OpsResult<()> {
    let session = project_status(project)?;
    let process = session.process.ok_or_else(|| {
        project_error(format!(
            "Project {} has no process; run `lf project resume {}` first",
            session.project.slug,
            session.project.id.as_str()
        ))
    })?;
    if !session.status.is_process_active() {
        return Err(project_error(format!(
            "Project {} is {}; run `lf project resume {}` first",
            session.project.slug,
            session.status.as_str(),
            session.project.id.as_str()
        )));
    }
    let status = std::process::Command::new("tmux")
        .args(["attach-session", "-t", &process.tmux_name])
        .status()
        .map_err(|error| project_error(format!("failed to attach Project Session: {error}")))?;
    if !status.success() {
        return Err(project_error("tmux attach failed"));
    }
    Ok(())
}

pub(crate) async fn wake_project_session(session_id: &ProjectSessionId) -> OpsResult<()> {
    let store = project_store().await?;
    let mut session = store
        .get_project_session(session_id)
        .await
        .map_err(|error| project_error(error.to_string()))?
        .ok_or_else(|| project_error(format!("Project Session {session_id} not found")))?;
    if !session.status.is_process_active() && !session.status.is_terminal() {
        launch_project_process(&store, &mut session).await?;
    }
    Ok(())
}

/// Complete the mechanical half of an authored project-promotion flow: pin
/// the registry ancestry, start the child residency, and wait for its endpoint.
pub fn complete_promotion(repo: &Path, parent: &str, child: &str) -> OpsResult<String> {
    let origin = crate::engine::wave_context::wave_origin(repo);
    let goal = origin.join("wave").join(child).join("GOAL.md");
    if !goal.is_file() {
        return Err(OpsError::Message(format!(
            "promotion is authored but not visible to the wave listener at {}; land the migration before starting residency",
            goal.display()
        )));
    }

    let runtime = tokio::runtime::Runtime::new()
        .map_err(|err| OpsError::Message(format!("failed to build promotion runtime: {err}")))?;
    runtime.block_on(async {
        let store = open_existing_store().await.ok_or_else(|| {
            OpsError::Message(
                "project promotion requires the wave registry; start the parent wave first"
                    .to_string(),
            )
        })?;
        link_parent(&store, &origin, parent, child).await?;

        if crate::wave::server::live_endpoint(&origin, child)
            .await
            .is_none()
        {
            launch_residency(&origin, child).await?;
        }
        for _ in 0..100 {
            if crate::wave::server::live_endpoint(&origin, child)
                .await
                .is_some()
            {
                wake_child(&origin, child).await?;
                return Ok(promotion_session_name(&origin, child));
            }
            tokio::time::sleep(Duration::from_millis(100)).await;
        }
        Err(OpsError::Message(format!(
            "child wave '{child}' did not publish .wave-endpoint within 10s"
        )))
    })
}

/// Wake the promoted child through its thread door, not the bus: a fresh
/// mind's bus cursor attaches at the head, so a row published during boot
/// could land before the ear exists. The thread door is synchronous with the
/// live listener — no race. The message is the steward's own voice, so it is
/// unattributed like every human turn.
async fn wake_child(repo: &Path, wave: &str) -> OpsResult<()> {
    let status = tokio::process::Command::new(resolve_lf_binary())
        .args([
            "chat",
            "--wave",
            wave,
            "Promotion complete. Run the first child-wave pass, report what you now own in this thread, then publish the same concise report to the parent with `lf radio pub --parent`.",
        ])
        .current_dir(repo)
        .status()
        .await
        .map_err(|err| OpsError::Message(format!("failed to wake promoted wave: {err}")))?;
    if !status.success() {
        return Err(OpsError::Message(format!(
            "promoted wave '{wave}' started, but its bootstrap message failed"
        )));
    }
    Ok(())
}

async fn link_parent(store: &Store, repo: &Path, parent: &str, child: &str) -> OpsResult<()> {
    let parent = store
        .get_wave_by_name(parent)
        .await
        .map_err(|err| OpsError::Message(format!("failed to read parent wave: {err}")))?
        .ok_or_else(|| OpsError::Message(format!("parent wave '{parent}' is not registered")))?;
    let mut child_wave = match store
        .get_wave_by_name(child)
        .await
        .map_err(|err| OpsError::Message(format!("failed to read child wave: {err}")))?
    {
        Some(wave) => wave,
        None => Wave::new(LfdId::new(), child.to_string(), repo.display().to_string()),
    };
    if child_wave
        .parent_wave_id()
        .is_some_and(|current| current != parent.id())
    {
        return Err(OpsError::Message(format!(
            "child wave '{child}' already belongs to another parent"
        )));
    }
    child_wave.parent_wave_id = Some(parent.id().clone());
    if store
        .get_wave(child_wave.id())
        .await
        .map_err(|err| OpsError::Message(format!("failed to check child wave: {err}")))?
        .is_some()
    {
        store
            .update_wave(&child_wave)
            .await
            .map_err(|err| OpsError::Message(format!("failed to link child wave: {err}")))?;
    } else {
        store
            .create_wave(&child_wave)
            .await
            .map_err(|err| OpsError::Message(format!("failed to register child wave: {err}")))?;
    }
    Ok(())
}

/// Promotion grants residency, so it boots a listener with `lf serve`. The
/// child is spawned through tmux, which inherits the promoting
/// pass's environment (`WAVE_SERVER_ENDPOINT`, `RESIDENT_TOKEN`). Naming the
/// listener explicitly is what keeps that inheritance from deciding which half
/// of the wave the child becomes.
fn residency_argv(executable: &Path, wave: &str) -> Vec<String> {
    vec![
        executable.display().to_string(),
        "serve".to_string(),
        wave.to_string(),
    ]
}

async fn launch_residency(repo: &Path, wave: &str) -> OpsResult<()> {
    let argv = residency_argv(&resolve_lf_binary(), wave);
    start_lf_session(&promotion_session_name(repo, wave), repo, &argv)
        .await
        .map_err(|err| OpsError::Message(format!("failed to start child wave residency: {err}")))
}

fn promotion_session_name(repo: &Path, wave: &str) -> String {
    let repo = repo
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("repo");
    format!("lf-{}-{}", tmux_session_slug(repo), tmux_session_slug(wave))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::lf::{Cli, Commands};
    use crate::lfdb::{open_store, StorageConfig};
    use clap::Parser;

    /// Promotion grants residency: the spawned child must be the steerable
    /// half. A one-shot task runner would never publish an endpoint.
    #[test]
    fn promotion_spawns_a_listener_not_a_batch_loop() {
        let argv = residency_argv(Path::new("/opt/lf"), "release-stability");
        assert_eq!(argv, ["/opt/lf", "serve", "release-stability"]);

        let full = std::iter::once("lf".to_string()).chain(argv.into_iter().skip(1));
        assert!(
            matches!(
                Cli::try_parse_from(full).expect("promotion argv parses").command,
                Some(Commands::Serve { name, force: false }) if name == "release-stability"
            ),
            "what promotion spawns must parse as the serve entrypoint"
        );
    }

    #[tokio::test]
    async fn link_parent_registers_the_promoted_wave_as_a_child() {
        let tmp = tempfile::tempdir().unwrap();
        let store = open_store(&StorageConfig::sqlite(tmp.path().join("lfd.db")))
            .await
            .unwrap();
        let parent = Wave::new(
            LfdId::new(),
            "platform".into(),
            tmp.path().display().to_string(),
        );
        store.create_wave(&parent).await.unwrap();

        link_parent(&store, tmp.path(), "platform", "release-stability")
            .await
            .unwrap();

        let child = store
            .get_wave_by_name("release-stability")
            .await
            .unwrap()
            .unwrap();
        assert_eq!(child.parent_wave_id(), Some(parent.id()));
        assert_eq!(child.repo(), tmp.path().display().to_string());
    }
}
