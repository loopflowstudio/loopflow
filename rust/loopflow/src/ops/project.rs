use std::path::Path;
use std::sync::Arc;
use std::time::{Duration, Instant};

use crate::child_session::ChildRef;
use crate::durable::{
    AuthenticatedRequest, Author, Containment, ContainmentObservation, ControlCtx, Launch, RunState,
};
use crate::engine::config::{load_config_or_default, parse_agent};
use crate::engine::git::{current_branch, get_default_branch, is_clean, worktree_root};
use crate::engine::process::{
    resolve_lf_binary, start_lf_session, tmux_session_exists, tmux_session_slug,
};
use crate::id::WaveId;
use crate::ops::{OpsError, OpsResult};
use crate::project_session::{
    ProjectEventKind, ProjectSession, ProjectSessionId, ProjectSessionStatus,
};
use crate::session_context::{LinearProjectId, LinearProjectSnapshot, ProjectLaunchReceipt};
use crate::store::{open_existing_store, SharedStore, Store};
use crate::task::TaskSession;
use crate::wave::Wave;

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct ProjectSessionSnapshot {
    pub project_id: String,
    pub project_slug: String,
    pub project_name: String,
    pub session_id: String,
    pub predecessor_session_id: Option<String>,
    pub successor_session_id: Option<String>,
    pub wave: String,
    pub status: ProjectSessionStatus,
    pub status_reason: String,
    pub status_at: time::OffsetDateTime,
    pub iteration: u32,
    pub observation_cursor: i64,
    pub pending_observations: u32,
    pub agent: String,
    pub provider: String,
    pub provider_session_id: Option<String>,
    pub process_alive: bool,
    pub launch: Option<Launch>,
    pub latest_event: Option<crate::project_session::ProjectEvent>,
    pub created_at: time::OffsetDateTime,
    pub updated_at: time::OffsetDateTime,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct ProjectControlResult {
    pub project_id: String,
    pub session_id: String,
    pub receipt: super::child::WorkControlReceipt,
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
        project_error("no Loopflow registry on this machine; start the owning Wave first")
    })
}

async fn owning_wave(store: &SharedStore, session: &ProjectSession) -> OpsResult<Wave> {
    store
        .get_wave(&session.wave_id)
        .await
        .map_err(|error| project_error(format!("failed to read owning Wave: {error}")))?
        .ok_or_else(|| project_error(format!("owning Wave {} is not registered", session.wave_id)))
}

pub(crate) fn require_registered_wave(wave: &str) -> OpsResult<Wave> {
    let wave = wave.to_string();
    block_on_project(async move {
        project_store()
            .await?
            .get_wave_by_name(&wave)
            .await
            .map_err(|error| project_error(format!("failed to read owning Wave: {error}")))?
            .ok_or_else(|| {
                project_error(format!(
                    "owning Wave {wave:?} is not registered; start it with `lf wave {wave}` first"
                ))
            })
    })
}

pub fn project_run(
    repo: &Path,
    project_id: &str,
    directive: Option<String>,
) -> OpsResult<ProjectSession> {
    let directive = normalize_directive(directive)?;
    if let Some(existing) = block_on_project(async {
        let store = project_store().await?;
        let Some(mut existing) = store
            .get_project_session_by_project(project_id)
            .await
            .map_err(|error| project_error(format!("failed to read Project Session: {error}")))?
        else {
            return Ok(None);
        };
        reconcile_project_liveness(&store, &mut existing).await?;
        if existing.status.is_terminal() {
            return Ok(None);
        }
        if directive.is_some() {
            return Err(project_error(format!(
                "Project {} already exists; use `lf project steer {} <new-direction>`",
                existing.launch.project.slug, existing.launch.project.slug,
            )));
        }
        Ok(Some(existing))
    })? {
        if existing.status.is_process_active() {
            return Ok(existing);
        }
        return block_on_project(async move {
            let store = project_store().await?;
            let mut existing = existing;
            launch_project_process(&store, &mut existing).await?;
            wait_until_project_running(&store, &existing.id).await
        });
    }

    let repo = ensure_clean_main(repo, "Project")?;

    let resolved =
        crate::ops::task_pm::resolve_project(&repo, project_id, crate::ops::pm::PmRefresh::Auto)?;
    let mut session = reserve_project_session(&repo, resolved, directive)?;
    if session.status.is_terminal() || session.status.is_process_active() {
        return Ok(session);
    }
    block_on_project(async move {
        let store = project_store().await?;
        launch_project_process(&store, &mut session).await?;
        wait_until_project_running(&store, &session.id).await
    })
}

pub(crate) fn reserve_project_session(
    repo: &Path,
    resolved: crate::ops::task_pm::ResolvedProject,
    directive: Option<String>,
) -> OpsResult<ProjectSession> {
    let config = load_config_or_default(Some(repo));
    let agent = config.agent();
    let (provider, _) = parse_agent(agent);
    let agent = agent.to_string();
    let directive = directive.unwrap_or_else(|| {
        format!(
            "Pursue {}.\n\n{}",
            resolved.project.name, resolved.project.definition
        )
    });
    block_on_project(async move {
        let store = project_store().await?;
        let predecessor = store
            .get_project_session_by_project(&resolved.project.id)
            .await
            .map_err(|error| project_error(format!("failed to read Project Session: {error}")))?;
        if let Some(existing) = &predecessor {
            if !existing.status.is_terminal() {
                return Ok(existing.clone());
            }
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
        let session = ProjectSession {
            id: ProjectSessionId::new(),
            launch: ProjectLaunchReceipt {
                project: LinearProjectSnapshot {
                    id: LinearProjectId::new(resolved.project.id.clone())
                        .map_err(|error| project_error(error.to_string()))?,
                    slug: resolved.project.slug,
                    name: resolved.project.name,
                    prompt_context: context,
                },
                pm_snapshot_synced_at: resolved.snapshot.synced_at,
            },
            wave_id: wave.id().clone(),
            status: ProjectSessionStatus::Created,
            status_reason: predecessor.as_ref().map_or_else(
                || "Linear Project reserved for pursuit".to_string(),
                |previous| format!("Project pursuit succeeds terminal Session {}", previous.id),
            ),
            status_at: now,
            iteration: 0,
            observation_cursor: 0,
            last_state_fingerprint: None,
            agent,
            provider,
            provider_session_id: None,
            latest_process: None,
            abandon_intent: None,
            created_at: now,
            updated_at: now,
        };
        if let Err(error) = store
            .create_project_session_with_steer(&session, Author::User, &directive)
            .await
        {
            if let Some(existing) = store
                .get_project_session_by_project(session.launch.project.id.as_str())
                .await
                .map_err(|read_error| project_error(read_error.to_string()))?
            {
                if !existing.status.is_terminal() && existing.id != session.id {
                    return Ok(existing);
                }
            }
            return Err(project_error(format!(
                "failed to reserve Project Session: {error}"
            )));
        }
        Ok(session)
    })
}

pub(crate) fn ensure_project_session_for_task(
    repo: &Path,
    resolved: crate::ops::task_pm::ResolvedProject,
) -> OpsResult<ProjectSession> {
    let session = reserve_project_session(repo, resolved, None)?;
    if session.status.is_terminal() {
        return Err(project_error(format!(
            "cannot start a Task under {}: Project Session {} is {}; create or select an active Project",
            session.launch.project.slug,
            session.id,
            session.status.as_str()
        )));
    }
    Ok(session)
}

fn normalize_directive(directive: Option<String>) -> OpsResult<Option<String>> {
    directive
        .map(|directive| {
            let directive = directive.trim().to_string();
            if directive.is_empty() {
                Err(project_error("directive cannot be empty"))
            } else {
                Ok(directive)
            }
        })
        .transpose()
}

pub(crate) fn ensure_clean_main(repo: &Path, subject: &str) -> OpsResult<std::path::PathBuf> {
    let worktree = worktree_root(repo).map_err(|error| project_error(error.to_string()))?;
    let main = crate::engine::worktrees::main_repo_root(repo)
        .map_err(|error| project_error(error.to_string()))?;
    let worktree = std::fs::canonicalize(&worktree).unwrap_or(worktree);
    let main = std::fs::canonicalize(&main).unwrap_or(main);
    if worktree != main {
        return Err(project_error(format!(
            "cannot run {subject} from {}: Wave and Project turns require the canonical main checkout; run existing work with `lf task run <issue-id>` or create it with `lf task start \"<title>\" --project <project>`",
            worktree.display()
        )));
    }
    let default_branch =
        get_default_branch(&main).map_err(|error| project_error(error.to_string()))?;
    let branch = current_branch(&main).map_err(|error| project_error(error.to_string()))?;
    if branch.as_deref() != Some(default_branch.as_str()) {
        return Err(project_error(format!(
            "cannot run {subject}: canonical checkout is on {}, expected {default_branch}",
            branch.as_deref().unwrap_or("detached HEAD")
        )));
    }
    if !is_clean(&main).map_err(|error| project_error(error.to_string()))? {
        return Err(project_error(format!(
            "cannot run {subject}: canonical {default_branch} checkout is dirty; Wave and Project turns never edit repository files"
        )));
    }
    Ok(main)
}

pub fn project_start(
    repo: &Path,
    title: &str,
    wave: Option<&str>,
    directive: Option<String>,
) -> OpsResult<ProjectSession> {
    let main = ensure_clean_main(repo, "Project start")?;
    let wave = crate::engine::wave_context::resolve_managed_wave_name_sync(wave).map_err(
        |err| match err {
            crate::engine::wave_context::WaveResolveError::NoContext => {
                project_error("cannot determine wave; pass --wave <name>")
            }
            other => project_error(other.to_string()),
        },
    )?;
    require_registered_wave(&wave)?;
    let project = crate::ops::pm::pm_create_project(&main, Some(&wave), title)?;
    if let Err(error) =
        crate::ops::task_pm::load_wave(&main, &wave, crate::ops::pm::PmRefresh::Force)
    {
        return Err(project_error(format!(
            "Linear Project {} is committed, but the local wave/{wave} snapshot could not refresh: {error}. No new Project Session was created. Run `lf pm sync --wave {wave}`, then `lf project run {}`. Retrying `lf project start` is also safe because Project titles are reconciled before creation.",
            project.project.id, project.project.id
        )));
    }
    project_run(&main, &project.project.id, directive)
}

pub(crate) async fn launch_project_process(
    store: &SharedStore,
    session: &mut ProjectSession,
) -> OpsResult<()> {
    // Re-check at the launch boundary: commands and observations can wake a
    // stopped Project long after its initial reservation.
    let wave = owning_wave(store, session).await?;
    ensure_clean_main(Path::new(wave.repo()), "Project turn")?;
    let work = store
        .work_for_child(&ChildRef::Project(session.id.clone()))
        .await
        .map_err(|error| project_error(error.to_string()))?;
    if store
        .current_run(&work)
        .await
        .map_err(|error| project_error(error.to_string()))?
        .is_some()
    {
        return Ok(());
    }
    let basis = store
        .current_epoch(&work)
        .await
        .map_err(|error| project_error(error.to_string()))?
        .current_basis;
    let (run, lease) = store
        .reserve_run(&work, crate::durable::RunTrigger::Input { basis })
        .await
        .map_err(|error| project_error(format!("failed to reserve Project Run: {error}")))?;
    let tmux_name = format!(
        "lf-project-{}-{}-{}",
        tmux_session_slug(&session.launch.project.slug),
        &session.id.as_str()[3..11],
        &run.id.as_str()[4..12]
    );
    session.set_status(
        ProjectSessionStatus::Starting,
        "project process is starting",
    );
    store
        .update_project_session_for_run(session, &lease)
        .await
        .map_err(|error| project_error(error.to_string()))?;
    crate::ops::launch_in_run(
        store,
        &lease,
        crate::ops::RunLaunch {
            kind: "project",
            legacy_id: session.id.to_string(),
            wave_id: session.wave_id.clone(),
            cwd: Path::new(wave.repo()).to_path_buf(),
            tmux_name,
            agent: session.agent.clone(),
            resume_token: session.provider_session_id.clone(),
        },
    )
    .await
    .map(|_| ())
    .map_err(|error| project_error(error.to_string()))
}

async fn wait_until_project_running(
    store: &SharedStore,
    session_id: &ProjectSessionId,
) -> OpsResult<ProjectSession> {
    let deadline = tokio::time::Instant::now() + super::child::CHILD_STARTUP_GRACE;
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
                    session.launch.project.slug, session.status_reason
                )))
            };
        }
        if tokio::time::Instant::now() >= deadline {
            return Err(project_error(format!(
                "Project {} did not become running within 10s",
                session.launch.project.slug
            )));
        }
        tokio::time::sleep(Duration::from_millis(100)).await;
    }
}

async fn reconcile_project_liveness(
    store: &SharedStore,
    session: &mut ProjectSession,
) -> OpsResult<()> {
    let work = store
        .work_for_child(&ChildRef::Project(session.id.clone()))
        .await
        .map_err(|error| project_error(error.to_string()))?;
    let Some(run) = store
        .current_run(&work)
        .await
        .map_err(|error| project_error(error.to_string()))?
    else {
        if session.status.is_process_active() {
            mark_project_body_lost(store, session).await?;
        }
        return Ok(());
    };
    let launch = store
        .current_launch_for_run(&run.id)
        .await
        .map_err(|error| project_error(error.to_string()))?;
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
                .map_err(|error| project_error(error.to_string()))?,
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
        .map_err(|error| project_error(error.to_string()))?;
    if !session.status.is_process_active() {
        return Ok(());
    }
    mark_project_body_lost(store, session).await
}

async fn mark_project_body_lost(
    store: &SharedStore,
    session: &mut ProjectSession,
) -> OpsResult<()> {
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
        let wave = owning_wave(&store, &session).await?;
        let work = store
            .work_for_child(&ChildRef::Project(session.id.clone()))
            .await
            .map_err(|error| project_error(error.to_string()))?;
        let launch = match store
            .current_run(&work)
            .await
            .map_err(|error| project_error(error.to_string()))?
        {
            Some(run) => store
                .current_launch_for_run(&run.id)
                .await
                .map_err(|error| project_error(error.to_string()))?,
            None => None,
        };
        let process_alive = match launch.as_ref().map(|launch| &launch.containment) {
            Some(Containment::Tmux { name }) => tmux_session_exists(name)
                .await
                .map_err(|error| project_error(error.to_string()))?,
            Some(Containment::ProcessGroup { .. }) => true,
            None => false,
        };
        let latest_event = store
            .project_events_after(&session.id, 0)
            .await
            .map_err(|error| project_error(error.to_string()))?
            .into_iter()
            .last();
        let pending_observations = if session.status.is_terminal() {
            // A terminal predecessor owns no live observations; its own-id
            // count drains to 0 as the successor consumes the chain.
            store
                .pending_observations(&crate::child_session::ObservationRecipient::Project {
                    session_id: session.id.clone(),
                })
                .await
                .map_err(|error| project_error(error.to_string()))?
                .len() as u32
        } else {
            // The live successor counts the whole project chain: observations
            // addressed to a terminal predecessor are routed to it.
            store
                .pending_project_observations_for_chain(session.launch.project.id.as_str())
                .await
                .map_err(|error| project_error(error.to_string()))?
                .len() as u32
        };
        let mut history = store
            .list_project_sessions(Some(wave.id()))
            .await
            .map_err(|error| project_error(error.to_string()))?
            .into_iter()
            .filter(|candidate| candidate.launch.project.id == session.launch.project.id)
            .collect::<Vec<_>>();
        history.sort_by(|left, right| {
            left.created_at
                .cmp(&right.created_at)
                .then(left.id.as_str().cmp(right.id.as_str()))
        });
        let position = history
            .iter()
            .position(|candidate| candidate.id == session.id)
            .ok_or_else(|| project_error("Project Session disappeared from its own history"))?;
        let predecessor_session_id = position
            .checked_sub(1)
            .and_then(|index| history.get(index))
            .map(|candidate| candidate.id.to_string());
        let successor_session_id = history
            .get(position + 1)
            .map(|candidate| candidate.id.to_string());
        Ok(ProjectSessionSnapshot {
            project_id: session.launch.project.id.as_str().to_string(),
            project_slug: session.launch.project.slug,
            project_name: session.launch.project.name,
            session_id: session.id.to_string(),
            predecessor_session_id,
            successor_session_id,
            wave: wave.name().to_string(),
            status: session.status,
            status_reason: session.status_reason,
            status_at: session.status_at,
            iteration: session.iteration,
            observation_cursor: session.observation_cursor,
            pending_observations,
            agent: session.agent,
            provider: session.provider,
            provider_session_id: session.provider_session_id,
            process_alive,
            launch,
            latest_event,
            created_at: session.created_at,
            updated_at: session.updated_at,
        })
    })
}

fn queue_project_steer(project: &str, message: String) -> OpsResult<ProjectControlResult> {
    block_on_project(async move {
        let store = project_store().await?;
        let mut session = store
            .get_project_session_by_project(project)
            .await
            .map_err(|error| project_error(error.to_string()))?
            .ok_or_else(|| project_error(format!("no Project Session exists for {project:?}")))?;
        reconcile_project_liveness(&store, &mut session).await?;
        let receipt =
            super::child::append_steer(&store, ChildRef::Project(session.id.clone()), &message)
                .await?;
        if !session.status.is_process_active() {
            launch_project_process(&store, &mut session).await?;
        }
        Ok(ProjectControlResult {
            project_id: session.launch.project.id.as_str().to_string(),
            session_id: session.id.to_string(),
            receipt: super::child::WorkControlReceipt::Steer { receipt },
        })
    })
}

pub fn project_steer(project: &str, message: String) -> OpsResult<ProjectControlResult> {
    queue_project_steer(project, message)
}

pub fn project_interrupt(project: &str) -> OpsResult<ProjectControlResult> {
    block_on_project(async move {
        let store = project_store().await?;
        let mut session = store
            .get_project_session_by_project(project)
            .await
            .map_err(|error| project_error(error.to_string()))?
            .ok_or_else(|| project_error(format!("no Project Session exists for {project:?}")))?;
        reconcile_project_liveness(&store, &mut session).await?;
        let work = store
            .work_for_child(&ChildRef::Project(session.id.clone()))
            .await
            .map_err(|error| project_error(error.to_string()))?;
        let run = store
            .current_run(&work)
            .await
            .map_err(|error| project_error(error.to_string()))?
            .ok_or_else(|| project_error("Project has no active Run to interrupt"))?;
        let request = AuthenticatedRequest::cli();
        let receipt = store
            .interrupt(&ControlCtx::User(&request), &work, &run.id)
            .await
            .map_err(|error| project_error(error.to_string()))?;
        Ok(ProjectControlResult {
            project_id: session.launch.project.id.as_str().to_string(),
            session_id: session.id.to_string(),
            receipt: super::child::WorkControlReceipt::Interrupt { receipt },
        })
    })
}

pub fn project_resume(
    project: &str,
    model: Option<String>,
    reason: Option<String>,
) -> OpsResult<ProjectControlResult> {
    block_on_project(async move {
        let store = project_store().await?;
        let mut session = store
            .get_project_session_by_project(project)
            .await
            .map_err(|error| project_error(error.to_string()))?
            .ok_or_else(|| project_error(format!("no Project Session exists for {project:?}")))?;
        reconcile_project_liveness(&store, &mut session).await?;
        let project_id = session.launch.project.id.as_str().to_string();
        let session_id = session.id.to_string();
        let run = super::child::resume_session(
            &store,
            super::child::ChildSession::Project(Box::new(session)),
            model,
            reason,
        )
        .await?;
        Ok(ProjectControlResult {
            project_id,
            session_id,
            receipt: super::child::WorkControlReceipt::Resume { run },
        })
    })
}

pub fn project_abandon(project: &str, reason: String) -> OpsResult<ProjectControlResult> {
    block_on_project(async move {
        let store = project_store().await?;
        let mut session = store
            .get_project_session_by_project(project)
            .await
            .map_err(|error| project_error(error.to_string()))?
            .ok_or_else(|| project_error(format!("no Project Session exists for {project:?}")))?;
        reconcile_project_liveness(&store, &mut session).await?;
        let work = store
            .work_for_child(&ChildRef::Project(session.id.clone()))
            .await
            .map_err(|error| project_error(error.to_string()))?;
        let basis = store
            .current_epoch(&work)
            .await
            .map_err(|error| project_error(error.to_string()))?
            .current_basis;
        let receipt = store
            .abandon(&work, &reason, &basis)
            .await
            .map_err(|error| project_error(error.to_string()))?;
        if !session.status.is_process_active() {
            session.set_status(
                ProjectSessionStatus::Abandoned,
                format!("Project explicitly abandoned: {}", reason.trim()),
            );
            store
                .update_project_session(&session)
                .await
                .map_err(|error| project_error(error.to_string()))?;
        }
        Ok(ProjectControlResult {
            project_id: session.launch.project.id.as_str().to_string(),
            session_id: session.id.to_string(),
            receipt: super::child::WorkControlReceipt::Abandon { receipt },
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
    if !session.status.is_process_active() {
        return Err(project_error(format!(
            "Project {} is {}; run `lf project resume {}` first",
            session.launch.project.slug,
            session.status.as_str(),
            session.launch.project.id.as_str()
        )));
    }
    let launch = block_on_project(async {
        let store = project_store().await?;
        let work = store
            .work_for_child(&ChildRef::Project(session.id.clone()))
            .await
            .map_err(|error| project_error(error.to_string()))?;
        let run = store
            .current_run(&work)
            .await
            .map_err(|error| project_error(error.to_string()))?
            .ok_or_else(|| project_error("Project has no active Run"))?;
        store
            .current_launch_for_run(&run.id)
            .await
            .map_err(|error| project_error(error.to_string()))?
            .ok_or_else(|| project_error("Project Run has no active Launch"))
    })?;
    let Containment::Tmux { name } = launch.containment else {
        return Err(project_error("Project Launch is not attachable"));
    };
    let status = std::process::Command::new("tmux")
        .args(["attach-session", "-t", &name])
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
    // A wake is a supervisor restart: a Task observation arrived and the Project
    // may want to judge it. That is never a reason to revive a Project whose end
    // was already decided.
    if let Some(bar) = session.supervisor_restart_bar() {
        tracing::info!(project_session = %session_id, "not waking Project Session: {bar}");
        return Ok(());
    }
    if !session.status.is_process_active() {
        launch_project_process(&store, &mut session).await?;
    }
    Ok(())
}

/// The resolved routing target for a Task Session's parent Project.
///
/// `historical` is the Project Session the Task was born under
/// (`task.project_session_id`) — provenance, preserved. `current` is the live
/// routing target: the same session when it is still live, or its non-terminal
/// successor when the historical session was abandoned/completed and replaced.
/// `succeeded` is true when routing had to follow the chain to a successor.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct TaskProjectRoute {
    pub historical: ProjectSessionId,
    pub current: ProjectSessionId,
    pub current_status: ProjectSessionStatus,
    pub succeeded: bool,
}

/// Resolve a Task Session's parent Project to its live routing target.
///
/// The historical `project_session_id` is preserved as provenance and never
/// treated as the live routing key. When it is terminal, the successor is the
/// latest session for the same Linear project id. A terminal historical session
/// with no live successor fails actionably.
pub async fn resolve_task_project_route(
    store: &Store,
    task: &TaskSession,
) -> OpsResult<TaskProjectRoute> {
    let historical = task.project_session_id.clone();
    let Some(recorded) = store
        .get_project_session(&historical)
        .await
        .map_err(|error| project_error(error.to_string()))?
    else {
        return Err(project_error(format!(
            "Task {} Project Session {historical} is not registered; cannot route observations",
            task.launch.issue.identifier
        )));
    };
    if !recorded.status.is_terminal() {
        return Ok(TaskProjectRoute {
            historical: historical.clone(),
            current: historical,
            current_status: recorded.status,
            succeeded: false,
        });
    }
    let Some(successor) = store
        .get_project_session_by_project(task.launch.project.id.as_str())
        .await
        .map_err(|error| project_error(error.to_string()))?
    else {
        return Err(actionable_dead_chain(
            &task.launch.issue.identifier,
            &historical,
            recorded.status,
            &task.launch.project,
        ));
    };
    if successor.id == historical || successor.status.is_terminal() {
        return Err(actionable_dead_chain(
            &task.launch.issue.identifier,
            &historical,
            recorded.status,
            &task.launch.project,
        ));
    }
    Ok(TaskProjectRoute {
        historical,
        current: successor.id.clone(),
        current_status: successor.status,
        succeeded: true,
    })
}

fn actionable_dead_chain(
    issue_identifier: &str,
    historical: &ProjectSessionId,
    historical_status: ProjectSessionStatus,
    project: &crate::session_context::LinearProjectSnapshot,
) -> OpsError {
    OpsError::Message(format!(
        "Task {} Project Session {historical} is {}; no live successor exists for project {} ({}). \
         Resume or restart the Project: `lf project run {}`.",
        issue_identifier,
        historical_status.as_str(),
        project.id.as_str(),
        project.slug,
        project.slug,
    ))
}

/// Wake the live successor Project Session for a Task observation, not the
/// terminal predecessor the Task was born under. Best-effort: the observation is
/// already enqueued to the historical recipient and is drained by the successor
/// through project-chain consumption, so a broken chain surfaces a warning
/// without losing the observation.
pub(crate) async fn wake_task_project_route(store: &Store, task: &TaskSession) -> OpsResult<()> {
    match resolve_task_project_route(store, task).await {
        Ok(route) => {
            if route.succeeded {
                tracing::info!(
                    task = %task.id,
                    historical_project_session = %route.historical,
                    routing_project_session = %route.current,
                    "Task observation routed to successor Project Session"
                );
            }
            wake_project_session(&route.current).await
        }
        Err(error) => {
            tracing::warn!(task = %task.id, %error, "Task observation wake could not resolve a live Project Session; observation stays queued");
            Ok(())
        }
    }
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
        None => Wave::new(WaveId::new(), child.to_string(), repo.display().to_string()),
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

/// Promotion grants residency, so it boots a listener with `lf wave`. The
/// child is spawned through tmux, which inherits the promoting
/// pass's environment (`WAVE_SERVER_ENDPOINT`, `RESIDENT_TOKEN`). Naming the
/// listener explicitly is what keeps that inheritance from deciding which half
/// of the wave the child becomes.
fn residency_argv(executable: &Path, wave: &str) -> Vec<String> {
    vec![
        executable.display().to_string(),
        "wave".to_string(),
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
    use crate::child_session::ChildProcessGeneration;
    use crate::lf::{Cli, Commands};
    use crate::store::{open_store, StorageConfig};
    use clap::Parser;
    use std::process::Command;

    #[test]
    fn control_plane_requires_a_clean_canonical_main() {
        let tmp = tempfile::tempdir().unwrap();
        for args in [
            vec!["init", "-b", "main"],
            vec!["config", "user.email", "test@example.com"],
            vec!["config", "user.name", "Test"],
        ] {
            assert!(Command::new("git")
                .args(args)
                .current_dir(tmp.path())
                .status()
                .unwrap()
                .success());
        }
        std::fs::write(tmp.path().join("README.md"), "hello\n").unwrap();
        assert!(Command::new("git")
            .args(["add", "."])
            .current_dir(tmp.path())
            .status()
            .unwrap()
            .success());
        assert!(Command::new("git")
            .args(["commit", "-m", "initial"])
            .current_dir(tmp.path())
            .status()
            .unwrap()
            .success());

        assert_eq!(
            ensure_clean_main(tmp.path(), "Project").unwrap(),
            std::fs::canonicalize(tmp.path()).unwrap()
        );
        std::fs::write(tmp.path().join("README.md"), "changed\n").unwrap();
        let error = ensure_clean_main(tmp.path(), "Project").unwrap_err();
        assert!(error
            .to_string()
            .contains("canonical main checkout is dirty"));
    }

    /// Promotion grants residency: the spawned child must be the steerable
    /// half. A one-shot task runner would never publish an endpoint.
    #[test]
    fn promotion_spawns_a_listener_not_a_batch_loop() {
        let argv = residency_argv(Path::new("/opt/lf"), "release-stability");
        assert_eq!(argv, ["/opt/lf", "wave", "release-stability"]);

        let full = std::iter::once("lf".to_string()).chain(argv.into_iter().skip(1));
        assert!(
            matches!(
                Cli::try_parse_from(full).expect("promotion argv parses").command,
                Some(Commands::Wave { name, force: false }) if name == "release-stability"
            ),
            "what promotion spawns must parse as the Wave entrypoint"
        );
    }

    /// The launch resolver for Project Sessions ignores `LF_CONTROL_BIN`. It
    /// names a real, existing binary (the historical pin) while the current
    /// Home `LF_BIN` is gone; the launch fails at binary resolution, proving the
    /// control pin was never consulted.
    #[tokio::test]
    async fn launch_project_process_ignores_control_bin_and_resolves_current_home() {
        let home = tempfile::tempdir().unwrap();
        let store: SharedStore = Arc::new(
            open_store(&StorageConfig::sqlite(home.path().join("loopflow.db")))
                .await
                .unwrap(),
        );
        let now = time::OffsetDateTime::now_utc();
        let mut session = ProjectSession {
            id: ProjectSessionId::new(),
            launch: ProjectLaunchReceipt {
                project: LinearProjectSnapshot {
                    id: LinearProjectId::new("project-no-pin").unwrap(),
                    slug: "no-pin".to_string(),
                    name: "No pin".to_string(),
                    prompt_context: String::new(),
                },
                pm_snapshot_synced_at: now.unix_timestamp(),
            },
            wave_id: WaveId::new(),
            status: ProjectSessionStatus::Created,
            status_reason: "ready".to_string(),
            status_at: now,
            iteration: 1,
            observation_cursor: 0,
            last_state_fingerprint: None,
            agent: "claude".to_string(),
            provider: "claude".to_string(),
            provider_session_id: None,
            latest_process: None,
            abandon_intent: None,
            created_at: now,
            updated_at: now,
        };

        let previous_control_bin = std::env::var_os("LF_CONTROL_BIN");
        let previous_lf_bin = std::env::var_os("LF_BIN");
        std::env::set_var("LF_CONTROL_BIN", "/bin/sh");
        std::env::set_var("LF_BIN", "/loopflow-test/does-not-exist/lf");
        let result = super::launch_project_process(&store, &mut session).await;
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
        assert!(session.latest_process.is_none());
    }

    #[tokio::test]
    async fn link_parent_registers_the_promoted_wave_as_a_child() {
        let tmp = tempfile::tempdir().unwrap();
        let store = open_store(&StorageConfig::sqlite(tmp.path().join("loopflow.db")))
            .await
            .unwrap();
        let parent = Wave::new(
            WaveId::new(),
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

    /// A non-active Project Session still carrying a dead lease — the shape an
    /// explicit resume must reconcile before it can reserve a fresh body.
    async fn project_with_dead_lease(
        store: &SharedStore,
        status: ProjectSessionStatus,
        lease_state: crate::child_session::ChildLeaseState,
    ) -> ProjectSession {
        let now = time::OffsetDateTime::now_utc();
        let wave = Wave::new(WaveId::new(), "recover".to_string(), "/repo".to_string());
        store.create_wave(&wave).await.expect("create wave");
        let session = ProjectSession {
            id: ProjectSessionId::new(),
            launch: ProjectLaunchReceipt {
                project: LinearProjectSnapshot {
                    id: LinearProjectId::new("project-uuid").expect("project id"),
                    slug: "developer-efficiency".to_string(),
                    name: "Developer Efficiency".to_string(),
                    prompt_context: "Definition:\nKeep local work fast.".to_string(),
                },
                pm_snapshot_synced_at: now.unix_timestamp(),
            },
            wave_id: wave.id().clone(),
            status,
            status_reason: "recovered from a vanished body".to_string(),
            status_at: now,
            iteration: 1,
            observation_cursor: 0,
            last_state_fingerprint: None,
            agent: "codex".to_string(),
            provider: "codex".to_string(),
            provider_session_id: Some("thread-project".to_string()),
            latest_process: Some(ChildProcessGeneration {
                generation: 1,
                pid: None,
                process_group_id: None,
                // A name no tmux server knows, so the liveness probe reads it dead.
                tmux_name: "dead-project-lease".to_string(),
                agent: "codex".to_string(),
                provider: "codex".to_string(),
                provider_session_id: Some("thread-project".to_string()),
                started_at: now - time::Duration::hours(1),
                state: lease_state,
                outcome: None,
                provenance: None,
            }),
            abandon_intent: None,
            created_at: now,
            updated_at: now,
        };
        store
            .create_project_session(&session)
            .await
            .expect("create project session");
        session
    }

    #[tokio::test]
    async fn resume_revokes_a_dead_legacy_lease_on_a_waiting_project() {
        let dir = tempfile::tempdir().unwrap();
        let store = Arc::new(
            open_store(&StorageConfig::sqlite(dir.path().join("loopflow.db")))
                .await
                .unwrap(),
        );
        let mut session = project_with_dead_lease(
            &store,
            ProjectSessionStatus::Waiting,
            crate::child_session::ChildLeaseState::Legacy,
        )
        .await;

        reconcile_project_liveness(&store, &mut session)
            .await
            .expect("reconcile a waiting project with a dead legacy lease");

        // The dead lease is reaped so a resume can reserve a fresh body...
        assert_eq!(
            session.latest_process.as_ref().map(|process| process.state),
            Some(crate::child_session::ChildLeaseState::Finished)
        );
        // ...while the Project keeps its Waiting status for the resume that follows.
        assert_eq!(session.status, ProjectSessionStatus::Waiting);

        let persisted = store
            .get_project_session(&session.id)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(
            persisted.latest_process.map(|process| process.state),
            Some(crate::child_session::ChildLeaseState::Finished)
        );
        assert_eq!(persisted.status, ProjectSessionStatus::Waiting);
    }

    #[tokio::test]
    async fn resume_revokes_a_dead_active_lease_on_a_failed_project() {
        let dir = tempfile::tempdir().unwrap();
        let store = Arc::new(
            open_store(&StorageConfig::sqlite(dir.path().join("loopflow.db")))
                .await
                .unwrap(),
        );
        let mut session = project_with_dead_lease(
            &store,
            ProjectSessionStatus::Failed,
            crate::child_session::ChildLeaseState::Active,
        )
        .await;

        reconcile_project_liveness(&store, &mut session)
            .await
            .expect("reconcile a failed project with a dead active lease");

        assert_eq!(
            session.latest_process.as_ref().map(|process| process.state),
            Some(crate::child_session::ChildLeaseState::Finished)
        );
        assert_eq!(session.status, ProjectSessionStatus::Failed);

        let persisted = store
            .get_project_session(&session.id)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(
            persisted.latest_process.map(|process| process.state),
            Some(crate::child_session::ChildLeaseState::Finished)
        );
        assert_eq!(persisted.status, ProjectSessionStatus::Failed);
    }
}
