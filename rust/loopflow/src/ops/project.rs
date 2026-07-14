use std::path::Path;
use std::sync::Arc;
use std::time::{Duration, Instant};

use crate::child_session::{
    ChildCommandEffect, ChildCommandId, ChildCommandKind, ChildCommandSource, ChildCommandState,
    ChildDecisionId, ChildDirective, ChildProcessGeneration, ChildRef,
};
use crate::engine::config::{load_config_or_default, parse_agent};
use crate::engine::git::{current_branch, get_default_branch, is_clean, worktree_root};
use crate::engine::process::{
    resolve_lf_binary, start_lf_session, start_lf_session_with_env, tmux_session_exists,
    tmux_session_slug,
};
use crate::id::WaveId;
use crate::ops::{ChildReceiptUntil, OpsError, OpsResult};
use crate::project_session::{
    ProjectEventKind, ProjectSession, ProjectSessionId, ProjectSessionStatus,
};
use crate::session_context::{LinearProjectId, LinearProjectSnapshot, ProjectLaunchReceipt};
use crate::store::{open_existing_store, SharedStore, Store};
use crate::wave::Wave;

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct ProjectSessionSnapshot {
    pub project_id: String,
    pub project_slug: String,
    pub project_name: String,
    pub session_id: String,
    pub wave: String,
    pub current_directive_version: u32,
    pub incorporated_directive_version: u32,
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
    pub latest_process: Option<ChildProcessGeneration>,
    pub latest_event: Option<crate::project_session::ProjectEvent>,
    pub created_at: time::OffsetDateTime,
    pub updated_at: time::OffsetDateTime,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct ProjectControlResult {
    pub project_id: String,
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

fn project_control_result(
    project_id: String,
    result: super::child::ChildControlResult,
) -> ProjectControlResult {
    ProjectControlResult {
        project_id,
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
        let mut existing = store
            .get_project_session_by_project(project_id)
            .await
            .map_err(|error| project_error(format!("failed to read Project Session: {error}")))?;
        if let Some(session) = &mut existing {
            reconcile_project_liveness(&store, session).await?;
            if let Some(requested) = directive.as_deref() {
                let current = store
                    .child_directives(&ChildRef::Project(session.id.clone()))
                    .await
                    .map_err(|error| project_error(error.to_string()))?
                    .into_iter()
                    .find(|value| value.version == session.current_directive_version)
                    .ok_or_else(|| project_error("Project Session has no current directive"))?;
                if current.text != requested {
                    return Err(project_error(format!(
                        "Project {} already exists with directive v{}; use `lf project steer {} <new-direction>` to replace it",
                        session.launch.project.slug,
                        current.version,
                        session.launch.project.slug,
                    )));
                }
            }
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
    let agent = config.agent.as_deref().unwrap_or("codex");
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
            current_directive_version: 1,
            incorporated_directive_version: 0,
            status: ProjectSessionStatus::Created,
            status_reason: "Linear Project reserved for pursuit".to_string(),
            status_at: now,
            iteration: 0,
            observation_cursor: 0,
            last_state_fingerprint: None,
            agent,
            provider,
            provider_session_id: None,
            latest_process: None,
            created_at: now,
            updated_at: now,
        };
        let initial = ChildDirective::initial(
            ChildRef::Project(session.id.clone()),
            directive,
            project_command_source(&session)?,
        );
        if let Err(error) = store
            .create_project_session_with_directive(&session, &initial)
            .await
        {
            if let Some(existing) = store
                .get_project_session_by_project(session.launch.project.id.as_str())
                .await
                .map_err(|read_error| project_error(read_error.to_string()))?
            {
                return Ok(existing);
            }
            return Err(project_error(format!(
                "failed to reserve Project Session: {error}"
            )));
        }
        store
            .append_project_event(
                &session.id,
                &ProjectEventKind::DirectiveChanged {
                    directive_id: initial.id,
                    version: initial.version,
                    directive_kind: initial.kind,
                },
            )
            .await
            .map_err(|error| project_error(error.to_string()))?;
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
    let wave = crate::ops::resolve_wave_name(wave)
        .ok_or_else(|| project_error("cannot determine wave; pass --wave <name>"))?;
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
    let tmux_name = format!(
        "lf-project-{}-{}",
        tmux_session_slug(&session.launch.project.slug),
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
        (
            crate::engine::wave_context::WAVE_ID_ENV,
            session.wave_id.as_str(),
        ),
        ("LF_PROJECT_SESSION_ID", session.id.as_str()),
        ("LF_PROJECT_GENERATION", generation_text.as_str()),
    ];
    if let Err(error) =
        start_lf_session_with_env(&tmux_name, Path::new(wave.repo()), &argv, &environment).await
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
    if !session.status.is_process_active() {
        return Ok(());
    }
    let alive = match session.latest_process.as_ref() {
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
        let wave = owning_wave(&store, &session).await?;
        let process_alive = if session.status.is_process_active() {
            match session.latest_process.as_ref() {
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
            .pending_observations(&crate::child_session::ObservationRecipient::Project {
                session_id: session.id.clone(),
            })
            .await
            .map_err(|error| project_error(error.to_string()))?
            .len() as u32;
        Ok(ProjectSessionSnapshot {
            project_id: session.launch.project.id.as_str().to_string(),
            project_slug: session.launch.project.slug,
            project_name: session.launch.project.name,
            session_id: session.id.to_string(),
            wave: wave.name().to_string(),
            current_directive_version: session.current_directive_version,
            incorporated_directive_version: session.incorporated_directive_version,
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
            latest_process: session.latest_process,
            latest_event,
            created_at: session.created_at,
            updated_at: session.updated_at,
        })
    })
}

fn project_command_source(session: &ProjectSession) -> OpsResult<ChildCommandSource> {
    match std::env::var(crate::engine::wave_context::WAVE_ID_ENV) {
        Ok(value) => {
            let wave_id = WaveId::parse(&value)
                .map_err(|error| project_error(format!("invalid ambient Wave id: {error}")))?;
            if wave_id != session.wave_id {
                return Err(project_error(format!(
                    "Wave {wave_id} cannot control Project {} owned by Wave {}",
                    session.launch.project.slug, session.wave_id
                )));
            }
            Ok(ChildCommandSource::Wave(wave_id))
        }
        Err(std::env::VarError::NotPresent) => Ok(ChildCommandSource::Human),
        Err(std::env::VarError::NotUnicode(_)) => {
            Err(project_error("ambient Wave id is not valid UTF-8"))
        }
    }
}

fn queue_project_command(project: &str, kind: ChildCommandKind) -> OpsResult<ProjectControlResult> {
    block_on_project(async move {
        let store = project_store().await?;
        let mut session = store
            .get_project_session_by_project(project)
            .await
            .map_err(|error| project_error(error.to_string()))?
            .ok_or_else(|| project_error(format!("no Project Session exists for {project:?}")))?;
        reconcile_project_liveness(&store, &mut session).await?;
        let project_id = session.launch.project.id.as_str().to_string();
        let source = project_command_source(&session)?;
        let result = super::child::queue_command(
            &store,
            super::child::ChildSession::Project(Box::new(session)),
            source,
            kind,
        )
        .await?;
        Ok(project_control_result(project_id, result))
    })
}

pub fn project_follow_up(project: &str, message: String) -> OpsResult<ProjectControlResult> {
    queue_project_command(project, ChildCommandKind::FollowUp { text: message })
}

pub fn project_steer(project: &str, message: String) -> OpsResult<ProjectControlResult> {
    queue_project_command(project, ChildCommandKind::Steer { text: message })
}

pub fn project_interrupt(
    project: &str,
    replacement: Option<String>,
) -> OpsResult<ProjectControlResult> {
    queue_project_command(project, ChildCommandKind::Interrupt { replacement })
}

pub fn project_resume(project: &str, message: Option<String>) -> OpsResult<ProjectControlResult> {
    queue_project_command(project, ChildCommandKind::Resume { message })
}

pub fn project_decide(
    project: &str,
    decision_id: &str,
    choice: String,
    message: Option<String>,
) -> OpsResult<ProjectControlResult> {
    let decision_id =
        ChildDecisionId::parse(decision_id).map_err(|error| project_error(error.to_string()))?;
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
        ChildCommandKind::Decide {
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
        let ambient = std::env::var("LF_PROJECT_SESSION_ID").map_err(|_| {
            project_error("decision requests must run inside the owning Project Session")
        })?;
        if ambient != session.id.as_str() {
            return Err(project_error(format!(
                "Project Session {ambient} cannot request a decision for {}",
                session.id
            )));
        }
        let decision_id = ChildDecisionId::new();
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
                project_id: session.launch.project.id.as_str().to_string(),
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
                    project_id: session.launch.project.id.as_str().to_string(),
                    session_id: session.id.to_string(),
                    decision_id: decision_id.to_string(),
                    resolved: true,
                    choice: Some(choice),
                    message,
                });
            }
            if tokio::time::Instant::now() >= deadline {
                return Ok(ProjectDecisionResult {
                    project_id: session.launch.project.id.as_str().to_string(),
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

pub fn project_acknowledge(
    project: &str,
    version: u32,
    summary: String,
) -> OpsResult<ChildDirective> {
    let summary = summary.trim().to_string();
    if summary.is_empty() {
        return Err(project_error(
            "directive acknowledgement summary cannot be empty",
        ));
    }
    block_on_project(async move {
        let store = project_store().await?;
        let session = store
            .get_project_session_by_project(project)
            .await
            .map_err(|error| project_error(error.to_string()))?
            .ok_or_else(|| project_error(format!("no Project Session exists for {project:?}")))?;
        let ambient = std::env::var("LF_PROJECT_SESSION_ID").map_err(|_| {
            project_error("directive acknowledgements must run inside the owning Project Session")
        })?;
        if ambient != session.id.as_str() {
            return Err(project_error(format!(
                "Project Session {ambient} cannot acknowledge a directive for {}",
                session.id
            )));
        }
        let (directive, incorporated) = store
            .incorporate_child_directive(&ChildRef::Project(session.id.clone()), version, &summary)
            .await
            .map_err(|error| project_error(format!("failed to acknowledge directive: {error}")))?;
        if incorporated {
            store
                .append_project_event(
                    &session.id,
                    &ProjectEventKind::DirectiveIncorporated {
                        directive_id: directive.id.clone(),
                        version,
                        summary,
                    },
                )
                .await
                .map_err(|error| project_error(error.to_string()))?;
        }
        Ok(directive)
    })
}

pub fn project_abandon(project: &str, reason: String) -> OpsResult<ProjectControlResult> {
    queue_project_command(project, ChildCommandKind::Abandon { reason })
}

pub fn project_receipt(
    command_id: &str,
    until: Option<ChildReceiptUntil>,
    timeout: Duration,
) -> OpsResult<ProjectReceiptRead> {
    let command_id =
        ChildCommandId::parse(command_id).map_err(|error| project_error(error.to_string()))?;
    block_on_project(async move {
        let store = project_store().await?;
        let (command, timed_out) = if let Some(until) = until {
            super::child::wait_for_receipt_condition(&store, &command_id, until, timeout).await?
        } else {
            (
                super::child::read_receipt(&store, &command_id).await?,
                false,
            )
        };
        let ChildRef::Project(session_id) = &command.target else {
            return Err(project_error(format!(
                "command {command_id} belongs to a Task Session"
            )));
        };
        let session = store
            .get_project_session(session_id)
            .await
            .map_err(|error| project_error(error.to_string()))?
            .ok_or_else(|| project_error("Project Session disappeared"))?;
        let result = super::child::control_result(&store, &command, command.clone()).await?;
        Ok(ProjectReceiptRead {
            receipt: project_control_result(session.launch.project.id.as_str().to_string(), result),
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
    let process = session.latest_process.ok_or_else(|| {
        project_error(format!(
            "Project {} has no process; run `lf project resume {}` first",
            session.launch.project.slug,
            session.launch.project.id.as_str()
        ))
    })?;
    if !session.status.is_process_active() {
        return Err(project_error(format!(
            "Project {} is {}; run `lf project resume {}` first",
            session.launch.project.slug,
            session.status.as_str(),
            session.launch.project.id.as_str()
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
}
