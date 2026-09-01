use std::path::Path;
use std::sync::Arc;
use std::time::{Duration, Instant};

use crate::child::ChildRef;
use crate::controller::project::State as ProjectControllerState;
use crate::durable::{Author, WorkStatus};
use crate::engine::config::{load_config_or_default, parse_agent};
use crate::engine::git::{current_branch, get_default_branch, is_clean, worktree_root};
use crate::engine::process::{
    resolve_lf_binary, start_lf_session, tmux_session_exists, tmux_session_slug,
};
use crate::id::WaveId;
use crate::ops::{OpsError, OpsResult};
use crate::planning::{LinearProjectId, ProjectPlan};
use crate::store::{open_existing_store, SharedStore, Store};
use crate::work::project::{Project, ProjectId};
use crate::work::task::Task;
use crate::work::wave::Wave;

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct ProjectSnapshot {
    pub id: String,
    pub external_project_id: String,
    pub project_slug: String,
    pub project_name: String,
    pub wave: String,
    pub status: WorkStatus,
    pub reason: String,
    pub iteration: Option<u32>,
    pub observation_cursor: Option<i64>,
    pub pending_observations: u32,
    pub agent: Option<String>,
    pub provider: Option<String>,
    pub provider_session_id: Option<String>,
    pub latest_event: Option<crate::work::project::ProjectEvent>,
    pub last_failure: Option<crate::work::project::HistoricalFailure>,
    pub created_at: time::OffsetDateTime,
    pub updated_at: time::OffsetDateTime,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct ProjectControlResult {
    pub id: String,
    pub external_project_id: String,
    pub receipt_id: String,
    pub action: String,
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

async fn owning_wave(store: &SharedStore, project: &Project) -> OpsResult<Wave> {
    store
        .get_wave(&project.wave_id)
        .await
        .map_err(|error| project_error(format!("failed to read owning Wave: {error}")))?
        .ok_or_else(|| project_error(format!("owning Wave {} is not registered", project.wave_id)))
}

async fn project_work_status(store: &Store, project: &Project) -> OpsResult<WorkStatus> {
    let work = store
        .work_for_child(&ChildRef::Project(project.id.clone()))
        .await
        .map_err(|error| project_error(error.to_string()))?;
    store
        .work_status(&work)
        .await
        .map_err(|error| project_error(error.to_string()))
}

fn default_project_controller_state(
    project: &Project,
    repo: &Path,
    now: time::OffsetDateTime,
) -> ProjectControllerState {
    let config = load_config_or_default(Some(repo));
    let agent = config.agent().to_string();
    let (provider, _) = parse_agent(&agent);
    ProjectControllerState {
        project_id: project.id.clone(),
        iteration: 0,
        observation_cursor: 0,
        last_state_fingerprint: None,
        agent,
        provider,
        provider_session_id: None,
        updated_at: now,
    }
}

fn project_session_name(project: &Project) -> String {
    format!(
        "lf-project-{}-{}",
        tmux_session_slug(&project.plan.slug),
        &project.id.as_str()[3..11]
    )
}

async fn project_session_live(project: &Project) -> OpsResult<bool> {
    tmux_session_exists(&project_session_name(project))
        .await
        .map_err(|error| project_error(error.to_string()))
}

pub(crate) fn require_registered_wave(repo: &Path, wave: &str) -> OpsResult<Wave> {
    let locator = crate::work::wave::WaveLocator::discover(repo, wave)
        .map_err(|error| project_error(error.to_string()))?;
    let wave = wave.to_string();
    block_on_project(async move {
        project_store()
            .await?
            .get_wave_at(&locator)
            .await
            .map_err(|error| project_error(format!("failed to read owning Wave: {error}")))?
            .ok_or_else(|| {
                project_error(format!(
                    "owning Wave {wave:?} is not registered; start it with `lf wave {wave}` first"
                ))
            })
    })
}

pub fn project_run(repo: &Path, project_id: &str, directive: Option<String>) -> OpsResult<Project> {
    let directive = normalize_directive(directive)?;
    if let Some(existing) = block_on_project(async {
        let store = project_store().await?;
        let Some(existing) = store
            .get_project_by_project(project_id)
            .await
            .map_err(|error| project_error(format!("failed to read Project: {error}")))?
        else {
            return Ok(None);
        };
        let status = project_work_status(&store, &existing).await?;
        if matches!(status, WorkStatus::Done | WorkStatus::Abandoned) {
            return Ok(None);
        }
        if directive.is_some() {
            return Err(project_error(format!(
                "Project {} already exists; use `lf project steer {} <new-direction>`",
                existing.plan.slug, existing.plan.slug,
            )));
        }
        if store
            .project_controller_state(&existing.id)
            .await
            .map_err(|error| project_error(error.to_string()))?
            .is_none()
        {
            store
                .put_project_controller_state(&default_project_controller_state(
                    &existing,
                    repo,
                    time::OffsetDateTime::now_utc(),
                ))
                .await
                .map_err(|error| project_error(error.to_string()))?;
        }
        let live = project_session_live(&existing).await?;
        Ok(Some((existing, live)))
    })? {
        let (existing, live) = existing;
        if live {
            return Ok(existing);
        }
        return block_on_project(async move {
            let store = project_store().await?;
            launch_project_process(&store, &existing).await?;
            wait_until_project_running(&store, &existing.id).await
        });
    }

    let repo = ensure_clean_main(repo, "Project")?;

    let resolved =
        crate::ops::task_pm::resolve_project(&repo, project_id, crate::ops::pm::PmRefresh::Auto)?;
    let project = reserve_project(&repo, resolved, directive, true)?;
    block_on_project(async move {
        let store = project_store().await?;
        if project_session_live(&project).await? {
            return Ok(project);
        }
        launch_project_process(&store, &project).await?;
        wait_until_project_running(&store, &project.id).await
    })
}

pub fn project_prepare(
    repo: &Path,
    project_id: &str,
    directive: Option<String>,
) -> OpsResult<Project> {
    let directive = normalize_directive(directive)?;
    if let Some(existing) = block_on_project(async {
        let store = project_store().await?;
        let Some(existing) = store
            .get_project_by_project(project_id)
            .await
            .map_err(|error| project_error(format!("failed to read Project: {error}")))?
        else {
            return Ok(None);
        };
        if matches!(
            project_work_status(&store, &existing).await?,
            WorkStatus::Done | WorkStatus::Abandoned
        ) {
            return Ok(None);
        }
        if directive.is_some() {
            return Err(project_error(format!(
                "Project {} already exists; use `lf project steer {} <new-direction>`",
                existing.plan.slug, existing.plan.slug,
            )));
        }
        Ok(Some(existing))
    })? {
        return Ok(existing);
    }

    let repo = ensure_clean_main(repo, "Project prepare")?;
    let resolved =
        crate::ops::task_pm::resolve_project(&repo, project_id, crate::ops::pm::PmRefresh::Auto)?;
    reserve_project(&repo, resolved, directive, false)
}

pub(crate) fn reserve_project(
    repo: &Path,
    resolved: crate::ops::task_pm::ResolvedProject,
    directive: Option<String>,
    install_controller: bool,
) -> OpsResult<Project> {
    let locator = crate::work::wave::WaveLocator::discover(repo, &resolved.snapshot.wave)
        .map_err(|error| project_error(error.to_string()))?;
    let controller_route = install_controller.then(|| {
        let config = load_config_or_default(Some(repo));
        let agent = config.agent().to_string();
        let (provider, _) = parse_agent(&agent);
        (agent, provider)
    });
    let directive = directive.unwrap_or_else(|| {
        format!(
            "Pursue {}.\n\n{}",
            resolved.project.name, resolved.project.definition
        )
    });
    block_on_project(async move {
        let store = project_store().await?;
        let predecessor = store
            .get_project_by_project(&resolved.project.id)
            .await
            .map_err(|error| project_error(format!("failed to read Project: {error}")))?;
        if let Some(existing) = &predecessor {
            if !matches!(
                project_work_status(&store, existing).await?,
                WorkStatus::Done | WorkStatus::Abandoned
            ) {
                if let Some((agent, provider)) = controller_route.as_ref() {
                    if store
                        .project_controller_state(&existing.id)
                        .await
                        .map_err(|error| project_error(error.to_string()))?
                        .is_none()
                    {
                        store
                            .put_project_controller_state(&ProjectControllerState {
                                project_id: existing.id.clone(),
                                iteration: 0,
                                observation_cursor: 0,
                                last_state_fingerprint: None,
                                agent: agent.clone(),
                                provider: provider.clone(),
                                provider_session_id: None,
                                updated_at: time::OffsetDateTime::now_utc(),
                            })
                            .await
                            .map_err(|error| project_error(error.to_string()))?;
                    }
                }
                return Ok(existing.clone());
            }
        }
        let wave = store
            .get_wave_at(&locator)
            .await
            .map_err(|error| project_error(format!("failed to read owning Wave: {error}")))?
            .ok_or_else(|| {
                project_error(format!(
                    "owning Wave {:?} is not registered",
                    resolved.snapshot.wave
                ))
            })?;
        let now = time::OffsetDateTime::now_utc();
        let plan = project_plan(&resolved.project, resolved.snapshot.synced_at)?;
        let project = Project {
            id: predecessor
                .as_ref()
                .map(|project| project.id.clone())
                .unwrap_or_else(ProjectId::new),
            plan,
            wave_id: wave.id().clone(),
            abandon_intent: None,
            created_at: predecessor
                .as_ref()
                .map_or(now, |project| project.created_at),
            updated_at: now,
        };
        let reserved = if predecessor.is_some() {
            store
                .reopen_project(&project, Author::User, &directive)
                .await
        } else {
            store
                .create_project_with_steer(&project, Author::User, &directive)
                .await
        };
        if let Err(error) = reserved {
            if let Some(existing) = store
                .get_project_by_project(project.plan.id.as_str())
                .await
                .map_err(|read_error| project_error(read_error.to_string()))?
            {
                if !matches!(
                    project_work_status(&store, &existing).await?,
                    WorkStatus::Done | WorkStatus::Abandoned
                ) {
                    return Ok(existing);
                }
            }
            return Err(project_error(format!("failed to reserve Project: {error}")));
        }
        if let Some((agent, provider)) = controller_route {
            store
                .put_project_controller_state(&ProjectControllerState {
                    project_id: project.id.clone(),
                    iteration: 0,
                    observation_cursor: 0,
                    last_state_fingerprint: None,
                    agent,
                    provider,
                    provider_session_id: None,
                    updated_at: now,
                })
                .await
                .map_err(|error| {
                    project_error(format!("failed to install Project controller: {error}"))
                })?;
        }
        Ok(project)
    })
}

pub(crate) fn project_plan(
    project: &crate::pm::PmProject,
    pm_snapshot_synced_at: i64,
) -> OpsResult<ProjectPlan> {
    Ok(ProjectPlan {
        id: LinearProjectId::new(project.id.clone())
            .map_err(|error| project_error(error.to_string()))?,
        slug: project.slug.clone(),
        name: project.name.clone(),
        prompt_context: crate::ops::task::project_context(project),
        pm_snapshot_synced_at,
    })
}

pub(crate) fn ensure_project_for_task(
    repo: &Path,
    resolved: crate::ops::task_pm::ResolvedProject,
) -> OpsResult<Project> {
    let project = reserve_project(repo, resolved, None, false)?;
    Ok(project)
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
) -> OpsResult<Project> {
    let main = ensure_clean_main(repo, "Project start")?;
    let wave = crate::work::wave::context::resolve_managed_wave_sync(Some(&main), wave).map_err(
        |err| match err {
            crate::work::wave::context::WaveResolveError::NoContext => {
                project_error("cannot determine wave; pass --wave <name>")
            }
            other => project_error(other.to_string()),
        },
    )?;
    let wave = wave.name().to_string();
    let project = crate::ops::pm::pm_create_project(&main, Some(&wave), title)?;
    if let Err(error) =
        crate::ops::task_pm::load_wave(&main, &wave, crate::ops::pm::PmRefresh::Force)
    {
        return Err(project_error(format!(
            "Linear Project {} is committed, but the local wave/{wave} snapshot could not refresh: {error}. No new Project was created. Run `lf pm sync --wave {wave}`, then `lf project run {}`. Retrying `lf project start` is also safe because Project titles are reconciled before creation.",
            project.project.id, project.project.id
        )));
    }
    project_run(&main, &project.project.id, directive)
}

pub(crate) async fn launch_project_process(
    store: &SharedStore,
    project: &Project,
) -> OpsResult<()> {
    // Re-check at the launch boundary: commands and observations can wake a
    // stopped Project long after its initial reservation.
    let wave = owning_wave(store, project).await?;
    ensure_clean_main(Path::new(wave.repo()), "Project turn")?;
    if project_session_live(project).await? {
        return Ok(());
    }
    crate::ops::launch_work(crate::ops::WorkLaunch {
        work: crate::durable::WorkRef::Project(project.id.clone()),
        wave_id: project.wave_id.clone(),
        cwd: Path::new(wave.repo()).to_path_buf(),
        tmux_name: project_session_name(project),
        environment: Vec::new(),
    })
    .await
    .map_err(|error| project_error(error.to_string()))
}

async fn wait_until_project_running(
    store: &SharedStore,
    project_id: &ProjectId,
) -> OpsResult<Project> {
    let deadline = tokio::time::Instant::now() + super::child::CHILD_STARTUP_GRACE;
    loop {
        let project = store
            .get_project(project_id)
            .await
            .map_err(|error| project_error(error.to_string()))?
            .ok_or_else(|| project_error("Project disappeared during startup"))?;
        if project_session_live(&project).await? {
            return Ok(project);
        }
        match project_work_status(store, &project).await? {
            WorkStatus::Done | WorkStatus::Abandoned => {
                return Err(project_error(format!(
                    "Project {} ended during startup",
                    project.plan.slug
                )))
            }
            WorkStatus::Ready => {}
        }
        if tokio::time::Instant::now() >= deadline {
            return Err(project_error(format!(
                "Project {} did not become running within 10s",
                project.plan.slug
            )));
        }
        tokio::time::sleep(Duration::from_millis(100)).await;
    }
}

pub fn project_status(project: &str) -> OpsResult<Project> {
    block_on_project(async move {
        let store = project_store().await?;
        let project = store
            .get_project_by_project(project)
            .await
            .map_err(|error| project_error(error.to_string()))?
            .ok_or_else(|| project_error(format!("no Project exists for {project:?}")))?;
        Ok(project)
    })
}

pub fn project_snapshot(project: &Project) -> OpsResult<ProjectSnapshot> {
    let project = project.clone();
    block_on_project(async move {
        let store = project_store().await?;
        let wave = owning_wave(&store, &project).await?;
        let work = store
            .work_for_child(&ChildRef::Project(project.id.clone()))
            .await
            .map_err(|error| project_error(error.to_string()))?;
        let latest_event = store
            .latest_project_event(&project.id)
            .await
            .map_err(|error| project_error(error.to_string()))?;
        let pending_observations = store
            .pending_project_observations(&project.id)
            .await
            .map_err(|error| project_error(error.to_string()))?
            .len() as u32;
        let status = store
            .work_status(&work)
            .await
            .map_err(|error| project_error(error.to_string()))?;
        let reason = status.reason().to_string();
        let controller = store
            .project_controller_state(&project.id)
            .await
            .map_err(|error| project_error(error.to_string()))?;
        Ok(ProjectSnapshot {
            id: project.id.to_string(),
            external_project_id: project.plan.id.as_str().to_string(),
            project_slug: project.plan.slug,
            project_name: project.plan.name,
            wave: wave.name().to_string(),
            status,
            reason,
            iteration: controller.as_ref().map(|state| state.iteration),
            observation_cursor: controller.as_ref().map(|state| state.observation_cursor),
            pending_observations,
            agent: controller.as_ref().map(|state| state.agent.clone()),
            provider: controller.as_ref().map(|state| state.provider.clone()),
            provider_session_id: controller.and_then(|state| state.provider_session_id),
            latest_event,
            last_failure: store
                .latest_project_failure(&project.id)
                .await
                .map_err(|error| project_error(error.to_string()))?,
            created_at: project.created_at,
            updated_at: project.updated_at,
        })
    })
}

fn queue_project_steer(project: &str, message: String) -> OpsResult<ProjectControlResult> {
    block_on_project(async move {
        let store = project_store().await?;
        let project = store
            .get_project_by_project(project)
            .await
            .map_err(|error| project_error(error.to_string()))?
            .ok_or_else(|| project_error(format!("no Project exists for {project:?}")))?;
        let steer =
            super::child::append_steer(&store, ChildRef::Project(project.id.clone()), &message)
                .await?;
        let has_controller = store
            .project_controller_state(&project.id)
            .await
            .map_err(|error| project_error(error.to_string()))?
            .is_some();
        if has_controller && !project_session_live(&project).await? {
            launch_project_process(&store, &project).await?;
        }
        Ok(ProjectControlResult {
            id: project.id.to_string(),
            external_project_id: project.plan.id.as_str().to_string(),
            receipt_id: steer.id.to_string(),
            action: "steered".to_string(),
        })
    })
}

pub fn project_steer(project: &str, message: String) -> OpsResult<ProjectControlResult> {
    queue_project_steer(project, message)
}

pub fn project_interrupt(project: &str) -> OpsResult<ProjectControlResult> {
    Err(project_error(format!(
        "cannot interrupt Project {project}: its controller has no exact process owner; attach to the live Project and use /interrupt"
    )))
}

pub fn project_resume(
    project: &str,
    model: Option<String>,
    reason: Option<String>,
) -> OpsResult<ProjectControlResult> {
    block_on_project(async move {
        let store = project_store().await?;
        let project = store
            .get_project_by_project(project)
            .await
            .map_err(|error| project_error(error.to_string()))?
            .ok_or_else(|| project_error(format!("no Project exists for {project:?}")))?;
        if store
            .project_controller_state(&project.id)
            .await
            .map_err(|error| project_error(error.to_string()))?
            .is_none()
        {
            let wave = owning_wave(&store, &project).await?;
            store
                .put_project_controller_state(&default_project_controller_state(
                    &project,
                    Path::new(wave.repo()),
                    time::OffsetDateTime::now_utc(),
                ))
                .await
                .map_err(|error| {
                    project_error(format!("failed to install Project controller: {error}"))
                })?;
        }
        if let Some(model) = model {
            let request = super::child::handoff_request(&model, reason.as_deref())?;
            let controller = store
                .project_controller_state(&project.id)
                .await
                .map_err(|error| project_error(error.to_string()))?
                .ok_or_else(|| {
                    project_error(format!(
                        "Project {} has no end-to-end controller state",
                        project.plan.slug
                    ))
                })?;
            if controller.agent != request.agent {
                store
                    .handoff_project_controller(&project.id, &request)
                    .await
                    .map_err(|error| project_error(error.to_string()))?;
            }
        }
        let external_project_id = project.plan.id.as_str().to_string();
        let id = project.id.to_string();
        launch_project_process(&store, &project).await?;
        let session = project_session_name(&project);
        Ok(ProjectControlResult {
            id,
            external_project_id,
            receipt_id: session,
            action: "resumed".to_string(),
        })
    })
}

pub fn project_abandon(project: &str, reason: String) -> OpsResult<ProjectControlResult> {
    block_on_project(async move {
        let store = project_store().await?;
        let project = store
            .get_project_by_project(project)
            .await
            .map_err(|error| project_error(error.to_string()))?
            .ok_or_else(|| project_error(format!("no Project exists for {project:?}")))?;
        let work = store
            .work_for_child(&ChildRef::Project(project.id.clone()))
            .await
            .map_err(|error| project_error(error.to_string()))?;
        let receipt = store
            .abandon(&work, &reason)
            .await
            .map_err(|error| project_error(error.to_string()))?;
        Ok(ProjectControlResult {
            id: project.id.to_string(),
            external_project_id: project.plan.id.as_str().to_string(),
            receipt_id: receipt.work.id().to_string(),
            action: "abandoned".to_string(),
        })
    })
}

pub fn project_wait(
    project: &str,
    until: ProjectWaitUntil,
    timeout: Option<Duration>,
) -> OpsResult<Project> {
    let start = Instant::now();
    loop {
        let project = project_status(project)?;
        let (status, live) = block_on_project(async {
            let store = project_store().await?;
            Ok((
                project_work_status(&store, &project).await?,
                project_session_live(&project).await?,
            ))
        })?;
        let done = match until {
            ProjectWaitUntil::Waiting => !live,
            ProjectWaitUntil::Terminal => {
                matches!(status, WorkStatus::Done | WorkStatus::Abandoned)
            }
        };
        if done {
            return Ok(project);
        }
        if timeout.is_some_and(|timeout| start.elapsed() >= timeout) {
            return Ok(project);
        }
        std::thread::sleep(Duration::from_millis(200));
    }
}

pub fn project_attach(project: &str) -> OpsResult<()> {
    let project = project_status(project)?;
    if !block_on_project(project_session_live(&project))? {
        return Err(project_error(format!(
            "Project {} is not running; run `lf project resume {}` first",
            project.plan.slug,
            project.plan.id.as_str()
        )));
    }
    let name = project_session_name(&project);
    let status = std::process::Command::new("tmux")
        .args(["attach-session", "-t", &name])
        .status()
        .map_err(|error| project_error(format!("failed to attach Project: {error}")))?;
    if !status.success() {
        return Err(project_error("tmux attach failed"));
    }
    Ok(())
}

pub(crate) async fn wake_project(project_id: &ProjectId) -> OpsResult<()> {
    let store = project_store().await?;
    let project = store
        .get_project(project_id)
        .await
        .map_err(|error| project_error(error.to_string()))?
        .ok_or_else(|| project_error(format!("Project {project_id} not found")))?;
    if store
        .project_controller_state(&project.id)
        .await
        .map_err(|error| project_error(error.to_string()))?
        .is_none()
    {
        return Ok(());
    }
    // A wake is a supervisor restart: a Task observation arrived and the Project
    // may want to judge it. That is never a reason to revive a Project whose end
    // was already decided.
    if let Some(bar) = crate::controller::project::automatic_restart_bar(&project) {
        tracing::info!(project = %project_id, "not waking Project: {bar}");
        return Ok(());
    }
    if !project_session_live(&project).await? {
        launch_project_process(&store, &project).await?;
    }
    Ok(())
}

pub(crate) async fn wake_task_project_route(_store: &Store, task: &Task) -> OpsResult<()> {
    wake_project(&task.project_id).await
}

/// Persist the child's ancestry before the authored promotion flow creates its
/// first Initiative or Project. Completion records the promotion occurrence.
pub fn prepare_promotion(repo: &Path, parent: &str, child: &str) -> OpsResult<()> {
    let origin = crate::work::wave::context::wave_origin(repo);
    block_on_project(async {
        let store = open_existing_store().await.ok_or_else(|| {
            OpsError::Message(
                "project promotion requires the wave registry; start the parent wave first"
                    .to_string(),
            )
        })?;
        prepare_promotion_with_store(&store, &origin, parent, child).await
    })
}

async fn prepare_promotion_with_store(
    store: &Store,
    repo: &Path,
    parent: &str,
    child: &str,
) -> OpsResult<()> {
    let parent_locator = crate::work::wave::WaveLocator::discover(repo, parent)
        .map_err(|error| project_error(error.to_string()))?;
    let child_locator = crate::work::wave::WaveLocator::discover(repo, child)
        .map_err(|error| project_error(error.to_string()))?;
    let parent = store
        .get_wave_at(&parent_locator)
        .await
        .map_err(|error| project_error(format!("failed to read parent Wave: {error}")))?
        .ok_or_else(|| project_error(format!("parent Wave '{parent}' is not registered")))?;
    let existing = store
        .get_wave_at(&child_locator)
        .await
        .map_err(|error| project_error(format!("failed to read child Wave: {error}")))?;
    if let Some(mut child_wave) = existing {
        if child_wave
            .parent_wave_id()
            .is_some_and(|id| id != parent.id())
        {
            return Err(project_error(format!(
                "child Wave '{child}' already belongs to another parent"
            )));
        }
        if child_wave.parent_wave_id().is_none() {
            child_wave = child_wave.with_parent(parent.id().clone());
            store.update_wave(&child_wave).await.map_err(|error| {
                project_error(format!("failed to prepare child Wave ancestry: {error}"))
            })?;
        }
        return Ok(());
    }

    let child_wave = Wave::new(
        WaveId::new(),
        child_locator.slug().to_string(),
        child_locator.repo().to_string(),
    )
    .with_parent(parent.id().clone());
    store
        .create_wave(&child_wave)
        .await
        .map_err(|error| project_error(format!("failed to prepare child Wave ancestry: {error}")))
}

/// Complete the mechanical half of an authored project-promotion flow: record
/// the promotion and ancestry, start the child residency, and wait for its endpoint.
pub fn complete_promotion(repo: &Path, parent: &str, child: &str) -> OpsResult<String> {
    let origin = crate::work::wave::context::wave_origin(repo);
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
        record_promotion(&store, &origin, parent, child).await?;

        if crate::controller::wave::server::live_endpoint(&origin, child)
            .await
            .is_none()
        {
            launch_residency(&origin, child).await?;
        }
        for _ in 0..100 {
            if let Some(endpoint) =
                crate::controller::wave::server::live_endpoint(&origin, child).await
            {
                wake_child_observer(&endpoint, parent, child).await;
                return Ok(promotion_session_name(&origin, child));
            }
            tokio::time::sleep(Duration::from_millis(100)).await;
        }
        Err(OpsError::Message(format!(
            "child wave '{child}' did not publish .wave-endpoint within 10s"
        )))
    })
}

/// Nudge the existing typed observer after the durable promotion is visible.
async fn wake_child_observer(endpoint: &str, parent: &str, wave: &str) {
    let response = reqwest::Client::new()
        .post(format!("http://{endpoint}/observations"))
        .json(&serde_json::json!({ "promotion": { "parent": parent } }))
        .send()
        .await;
    match response {
        Ok(response) if response.status() == reqwest::StatusCode::NO_CONTENT => {}
        Ok(response) => tracing::warn!(
            wave,
            status = %response.status(),
            "promoted Wave is resident, but its immediate observer nudge was refused; heartbeat remains available"
        ),
        Err(error) => tracing::warn!(
            wave,
            %error,
            "promoted Wave is resident, but its immediate observer nudge failed; heartbeat remains available"
        ),
    }
}

async fn record_promotion(store: &Store, repo: &Path, parent: &str, child: &str) -> OpsResult<()> {
    let parent_locator = crate::work::wave::WaveLocator::discover(repo, parent)
        .map_err(|error| OpsError::Message(error.to_string()))?;
    let child_locator = crate::work::wave::WaveLocator::discover(repo, child)
        .map_err(|error| OpsError::Message(error.to_string()))?;
    let parent = store
        .get_wave_at(&parent_locator)
        .await
        .map_err(|err| OpsError::Message(format!("failed to read parent wave: {err}")))?
        .ok_or_else(|| OpsError::Message(format!("parent wave '{parent}' is not registered")))?;
    let mut child_wave = match store
        .get_wave_at(&child_locator)
        .await
        .map_err(|err| OpsError::Message(format!("failed to read child wave: {err}")))?
    {
        Some(wave) => wave,
        None => Wave::new(
            WaveId::new(),
            child_locator.slug().to_string(),
            child_locator.repo().to_string(),
        ),
    };
    child_wave
        .record_promotion(parent.id(), time::OffsetDateTime::now_utc())
        .map_err(OpsError::Message)?;
    if store
        .get_wave(child_wave.id())
        .await
        .map_err(|err| OpsError::Message(format!("failed to check child wave: {err}")))?
        .is_some()
    {
        store
            .update_wave(&child_wave)
            .await
            .map_err(|err| OpsError::Message(format!("failed to record child promotion: {err}")))?;
    } else {
        store
            .create_wave(&child_wave)
            .await
            .map_err(|err| OpsError::Message(format!("failed to register child wave: {err}")))?;
    }
    let recorded = store
        .get_wave(child_wave.id())
        .await
        .map_err(|err| OpsError::Message(format!("failed to verify child promotion: {err}")))?
        .ok_or_else(|| OpsError::Message("promoted child wave disappeared".to_string()))?;
    if recorded.parent_wave_id() != Some(parent.id()) || recorded.promoted_at().is_none() {
        return Err(OpsError::Message(format!(
            "child wave '{child}' promotion did not persist atomically"
        )));
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
    use crate::store::StorageConfig;
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

    #[test]
    fn project_interrupt_refuses_without_exact_process_ownership() {
        let error = project_interrupt("project-no-owner")
            .expect_err("a deterministic tmux name is not signal authority");

        assert!(error.to_string().contains("no exact process owner"));
        assert!(error.to_string().contains("use /interrupt"));
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
    async fn promotion_observer_nudge_is_best_effort() {
        let listener = std::net::TcpListener::bind("127.0.0.1:0").expect("bind dead endpoint");
        let endpoint = listener.local_addr().expect("endpoint").to_string();
        drop(listener);

        wake_child_observer(&endpoint, "platform", "ship").await;
    }

    /// The launch resolver for Projects ignores `LF_CONTROL_BIN`. It
    /// names a real, existing binary (the historical pin) while the current
    /// Home `LF_BIN` is gone; the launch fails at binary resolution, proving the
    /// control pin was never consulted.
    #[tokio::test]
    async fn launch_project_process_ignores_control_bin_and_resolves_current_home() {
        let home = tempfile::tempdir().unwrap();
        let repo = tempfile::tempdir().unwrap();
        for args in [
            vec!["init", "-b", "main"],
            vec!["config", "user.email", "test@example.com"],
            vec!["config", "user.name", "Test"],
        ] {
            assert!(Command::new("git")
                .args(args)
                .current_dir(repo.path())
                .status()
                .unwrap()
                .success());
        }
        std::fs::write(repo.path().join("README.md"), "test\n").unwrap();
        assert!(Command::new("git")
            .args(["add", "."])
            .current_dir(repo.path())
            .status()
            .unwrap()
            .success());
        assert!(Command::new("git")
            .args(["commit", "-m", "initial"])
            .current_dir(repo.path())
            .status()
            .unwrap()
            .success());
        let store: SharedStore = Arc::new(
            crate::store::open_ephemeral_store(&StorageConfig::sqlite(
                home.path().join("loopflow.db"),
            ))
            .await
            .unwrap(),
        );
        let now = time::OffsetDateTime::now_utc();
        let project = Project {
            id: ProjectId::new(),
            plan: ProjectPlan {
                id: LinearProjectId::new("project-no-pin").unwrap(),
                slug: "no-pin".to_string(),
                name: "No pin".to_string(),
                prompt_context: String::new(),
                pm_snapshot_synced_at: now.unix_timestamp(),
            },
            wave_id: WaveId::new(),
            abandon_intent: None,
            created_at: now,
            updated_at: now,
        };
        store
            .create_wave(&Wave::new(
                project.wave_id.clone(),
                "no-pin".to_string(),
                repo.path().display().to_string(),
            ))
            .await
            .unwrap();
        store.create_project(&project).await.unwrap();

        let previous_control_bin = std::env::var_os("LF_CONTROL_BIN");
        let previous_lf_bin = std::env::var_os("LF_BIN");
        std::env::set_var("LF_CONTROL_BIN", "/bin/sh");
        std::env::set_var("LF_BIN", "/loopflow-test/does-not-exist/lf");
        let result = super::launch_project_process(&store, &project).await;
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
    }

    #[tokio::test]
    async fn prepare_promotion_persists_ancestry_without_occurrence() {
        let tmp = tempfile::tempdir().unwrap();
        let database = tmp.path().join("loopflow.db");
        let store = crate::store::open_ephemeral_store(&StorageConfig::sqlite(database.clone()))
            .await
            .unwrap();
        let parent = Wave::new(
            WaveId::new(),
            "survival".into(),
            tmp.path().display().to_string(),
        );
        store.create_wave(&parent).await.unwrap();

        prepare_promotion_with_store(&store, tmp.path(), "survival", "infrastructure")
            .await
            .unwrap();
        prepare_promotion_with_store(&store, tmp.path(), "survival", "infrastructure")
            .await
            .unwrap();

        let child = store
            .get_wave_at(
                &crate::work::wave::WaveLocator::discover(tmp.path(), "infrastructure").unwrap(),
            )
            .await
            .unwrap()
            .unwrap();
        assert_eq!(child.parent_wave_id(), Some(parent.id()));
        assert_eq!(child.promoted_at(), None);

        let other = Wave::new(
            WaveId::new(),
            "other".into(),
            tmp.path().display().to_string(),
        );
        store.create_wave(&other).await.unwrap();
        let error = prepare_promotion_with_store(&store, tmp.path(), "other", "infrastructure")
            .await
            .unwrap_err();
        assert!(error.to_string().contains("another parent"));
    }

    #[tokio::test]
    async fn record_promotion_persists_occurrence_and_ancestry() {
        let tmp = tempfile::tempdir().unwrap();
        let database = tmp.path().join("loopflow.db");
        let store = crate::store::open_ephemeral_store(&StorageConfig::sqlite(database.clone()))
            .await
            .unwrap();
        let parent = Wave::new(
            WaveId::new(),
            "platform".into(),
            tmp.path().display().to_string(),
        );
        store.create_wave(&parent).await.unwrap();

        record_promotion(&store, tmp.path(), "platform", "release-stability")
            .await
            .unwrap();

        let locator =
            crate::work::wave::WaveLocator::discover(tmp.path(), "release-stability").unwrap();
        let child = store.get_wave_at(&locator).await.unwrap().unwrap();
        assert_eq!(child.parent_wave_id(), Some(parent.id()));
        let promoted_at = child.promoted_at().expect("promotion occurrence");
        assert_eq!(child.repo(), locator.repo().to_string());

        record_promotion(&store, tmp.path(), "platform", "release-stability")
            .await
            .unwrap();
        let replayed = store.get_wave_at(&locator).await.unwrap().unwrap();
        assert_eq!(replayed.promoted_at(), Some(promoted_at));

        let stale = replayed.with_parent(WaveId::new());
        store.update_wave(&stale).await.unwrap();
        let preserved = store.get_wave_at(&locator).await.unwrap().unwrap();
        assert_eq!(preserved.parent_wave_id(), Some(parent.id()));
        assert_eq!(preserved.promoted_at(), Some(promoted_at));
    }
}
