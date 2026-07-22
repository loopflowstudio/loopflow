use std::path::Path;
use std::sync::Arc;
use std::time::{Duration, Instant};

use crate::child::ChildRef;
use crate::durable::{
    AgentInvocation, AuthenticatedRequest, Author, Containment, ContainmentObservation, ControlCtx,
    RunState, WorkStatus,
};
use crate::engine::config::{load_config_or_default, parse_agent};
use crate::engine::git::{current_branch, get_default_branch, is_clean, worktree_root};
use crate::engine::process::{
    resolve_lf_binary, start_lf_session, tmux_session_exists, tmux_session_slug,
};
use crate::id::WaveId;
use crate::ops::{OpsError, OpsResult};
use crate::planning::{LinearProjectId, ProjectPlan};
use crate::project::{Project, ProjectId};
use crate::store::{open_existing_store, SharedStore, Store};
use crate::task::Task;
use crate::wave::Wave;

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct ProjectSnapshot {
    pub id: String,
    pub external_project_id: String,
    pub project_slug: String,
    pub project_name: String,
    pub wave: String,
    pub status: WorkStatus,
    pub iteration: u32,
    pub observation_cursor: i64,
    pub pending_observations: u32,
    pub agent: String,
    pub provider: String,
    pub provider_session_id: Option<String>,
    pub process_alive: bool,
    pub invocation: Option<AgentInvocation>,
    pub latest_event: Option<crate::project::ProjectEvent>,
    pub created_at: time::OffsetDateTime,
    pub updated_at: time::OffsetDateTime,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct ProjectControlResult {
    pub id: String,
    pub external_project_id: String,
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

pub(crate) fn require_registered_wave(repo: &Path, wave: &str) -> OpsResult<Wave> {
    let locator = crate::wave::WaveLocator::discover(repo, wave)
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
        let Some(mut existing) = store
            .get_project_by_project(project_id)
            .await
            .map_err(|error| project_error(format!("failed to read Project: {error}")))?
        else {
            return Ok(None);
        };
        reconcile_project_liveness(&store, &mut existing).await?;
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
        Ok(Some((existing, status)))
    })? {
        let (existing, status) = existing;
        if matches!(status, WorkStatus::Running { .. }) {
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
    let mut project = reserve_project(&repo, resolved, directive)?;
    block_on_project(async move {
        let store = project_store().await?;
        if matches!(
            project_work_status(&store, &project).await?,
            WorkStatus::Running { .. }
        ) {
            return Ok(project);
        }
        launch_project_process(&store, &mut project).await?;
        wait_until_project_running(&store, &project.id).await
    })
}

pub(crate) fn reserve_project(
    repo: &Path,
    resolved: crate::ops::task_pm::ResolvedProject,
    directive: Option<String>,
) -> OpsResult<Project> {
    let locator = crate::wave::WaveLocator::discover(repo, &resolved.snapshot.wave)
        .map_err(|error| project_error(error.to_string()))?;
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
            .get_project_by_project(&resolved.project.id)
            .await
            .map_err(|error| project_error(format!("failed to read Project: {error}")))?;
        if let Some(existing) = &predecessor {
            if !matches!(
                project_work_status(&store, existing).await?,
                WorkStatus::Done | WorkStatus::Abandoned
            ) {
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
        let context = crate::ops::task::project_context(&resolved.project);
        let project = Project {
            id: predecessor
                .as_ref()
                .map(|project| project.id.clone())
                .unwrap_or_else(ProjectId::new),
            plan: ProjectPlan {
                id: LinearProjectId::new(resolved.project.id.clone())
                    .map_err(|error| project_error(error.to_string()))?,
                slug: resolved.project.slug,
                name: resolved.project.name,
                prompt_context: context,
                pm_snapshot_synced_at: resolved.snapshot.synced_at,
            },
            wave_id: wave.id().clone(),
            iteration: 0,
            observation_cursor: 0,
            last_state_fingerprint: None,
            agent,
            provider,
            provider_session_id: None,
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
        Ok(project)
    })
}

pub(crate) fn ensure_project_for_task(
    repo: &Path,
    resolved: crate::ops::task_pm::ResolvedProject,
) -> OpsResult<Project> {
    let project = reserve_project(repo, resolved, None)?;
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
    let wave = crate::engine::wave_context::resolve_managed_wave_sync(Some(&main), wave).map_err(
        |err| match err {
            crate::engine::wave_context::WaveResolveError::NoContext => {
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
    project: &mut Project,
) -> OpsResult<()> {
    // Re-check at the launch boundary: commands and observations can wake a
    // stopped Project long after its initial reservation.
    let wave = owning_wave(store, project).await?;
    ensure_clean_main(Path::new(wave.repo()), "Project turn")?;
    let work = store
        .work_for_child(&ChildRef::Project(project.id.clone()))
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
        tmux_session_slug(&project.plan.slug),
        &project.id.as_str()[3..11],
        &run.id.as_str()[4..12]
    );
    store
        .update_project_for_run(project, &lease)
        .await
        .map_err(|error| project_error(error.to_string()))?;
    crate::ops::launch_in_run(
        store,
        &lease,
        crate::ops::RunLaunch {
            work: crate::durable::WorkRef::Project(project.id.clone()),
            wave_id: project.wave_id.clone(),
            cwd: Path::new(wave.repo()).to_path_buf(),
            tmux_name,
            agent: project.agent.clone(),
            account_id: None,
            resume_token: project.provider_session_id.clone(),
        },
    )
    .await
    .map(|_| ())
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
        match project_work_status(store, &project).await? {
            WorkStatus::Running { .. } => return Ok(project),
            WorkStatus::Done | WorkStatus::Abandoned => {
                return Err(project_error(format!(
                    "Project {} ended during startup",
                    project.plan.slug
                )))
            }
            WorkStatus::Ready | WorkStatus::Waiting { .. } => {}
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

async fn reconcile_project_liveness(store: &SharedStore, project: &mut Project) -> OpsResult<()> {
    let work = store
        .work_for_child(&ChildRef::Project(project.id.clone()))
        .await
        .map_err(|error| project_error(error.to_string()))?;
    let Some(run) = store
        .current_run(&work)
        .await
        .map_err(|error| project_error(error.to_string()))?
    else {
        return Ok(());
    };
    if run.state == RunState::Reserved {
        let still_starting =
            run.created_at + time::Duration::seconds(10) > time::OffsetDateTime::now_utc();
        if still_starting {
            return Ok(());
        }
    }
    if let Some(containment) = &run.containment {
        let alive = match containment {
            Containment::Tmux { name } => tmux_session_exists(name)
                .await
                .map_err(|error| project_error(error.to_string()))?,
            Containment::ProcessGroup { .. } => true,
        };
        if alive {
            return Ok(());
        }
        if run.started_at.is_some_and(|started_at| {
            started_at + time::Duration::seconds(10) > time::OffsetDateTime::now_utc()
        }) {
            return Ok(());
        }
    }
    store
        .recover_run(&run.id, ContainmentObservation::Absent)
        .await
        .map_err(|error| project_error(error.to_string()))?;
    Ok(())
}

pub fn project_status(project: &str) -> OpsResult<Project> {
    block_on_project(async move {
        let store = project_store().await?;
        let mut project = store
            .get_project_by_project(project)
            .await
            .map_err(|error| project_error(error.to_string()))?
            .ok_or_else(|| project_error(format!("no Project exists for {project:?}")))?;
        reconcile_project_liveness(&store, &mut project).await?;
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
        let run = store
            .current_run(&work)
            .await
            .map_err(|error| project_error(error.to_string()))?;
        let invocation = match &run {
            Some(run) => store
                .open_invocation_for_run(&run.id)
                .await
                .map_err(|error| project_error(error.to_string()))?,
            None => None,
        };
        let process_alive = match run.as_ref().and_then(|run| run.containment.as_ref()) {
            Some(Containment::Tmux { name }) => tmux_session_exists(name)
                .await
                .map_err(|error| project_error(error.to_string()))?,
            Some(Containment::ProcessGroup { .. }) => true,
            None => false,
        };
        let latest_event = store
            .project_events_after(&project.id, 0)
            .await
            .map_err(|error| project_error(error.to_string()))?
            .into_iter()
            .last();
        let pending_observations = store
            .pending_project_observations(&project.id)
            .await
            .map_err(|error| project_error(error.to_string()))?
            .len() as u32;
        let status = store
            .work_status(&work)
            .await
            .map_err(|error| project_error(error.to_string()))?;
        Ok(ProjectSnapshot {
            id: project.id.to_string(),
            external_project_id: project.plan.id.as_str().to_string(),
            project_slug: project.plan.slug,
            project_name: project.plan.name,
            wave: wave.name().to_string(),
            status,
            iteration: project.iteration,
            observation_cursor: project.observation_cursor,
            pending_observations,
            agent: project.agent,
            provider: project.provider,
            provider_session_id: project.provider_session_id,
            process_alive,
            invocation,
            latest_event,
            created_at: project.created_at,
            updated_at: project.updated_at,
        })
    })
}

fn queue_project_steer(project: &str, message: String) -> OpsResult<ProjectControlResult> {
    block_on_project(async move {
        let store = project_store().await?;
        let mut project = store
            .get_project_by_project(project)
            .await
            .map_err(|error| project_error(error.to_string()))?
            .ok_or_else(|| project_error(format!("no Project exists for {project:?}")))?;
        reconcile_project_liveness(&store, &mut project).await?;
        let receipt =
            super::child::append_steer(&store, ChildRef::Project(project.id.clone()), &message)
                .await?;
        if !matches!(
            project_work_status(&store, &project).await?,
            WorkStatus::Running { .. }
        ) {
            launch_project_process(&store, &mut project).await?;
        }
        Ok(ProjectControlResult {
            id: project.id.to_string(),
            external_project_id: project.plan.id.as_str().to_string(),
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
        let mut project = store
            .get_project_by_project(project)
            .await
            .map_err(|error| project_error(error.to_string()))?
            .ok_or_else(|| project_error(format!("no Project exists for {project:?}")))?;
        reconcile_project_liveness(&store, &mut project).await?;
        let work = store
            .work_for_child(&ChildRef::Project(project.id.clone()))
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
            id: project.id.to_string(),
            external_project_id: project.plan.id.as_str().to_string(),
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
        let mut project = store
            .get_project_by_project(project)
            .await
            .map_err(|error| project_error(error.to_string()))?
            .ok_or_else(|| project_error(format!("no Project exists for {project:?}")))?;
        reconcile_project_liveness(&store, &mut project).await?;
        let external_project_id = project.plan.id.as_str().to_string();
        let id = project.id.to_string();
        let run = super::child::resume_child(
            &store,
            super::child::Child::Project(Box::new(project)),
            model,
            reason,
        )
        .await?;
        Ok(ProjectControlResult {
            id,
            external_project_id,
            receipt: super::child::WorkControlReceipt::Resume { run },
        })
    })
}

pub fn project_abandon(project: &str, reason: String) -> OpsResult<ProjectControlResult> {
    block_on_project(async move {
        let store = project_store().await?;
        let mut project = store
            .get_project_by_project(project)
            .await
            .map_err(|error| project_error(error.to_string()))?
            .ok_or_else(|| project_error(format!("no Project exists for {project:?}")))?;
        reconcile_project_liveness(&store, &mut project).await?;
        let work = store
            .work_for_child(&ChildRef::Project(project.id.clone()))
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
        Ok(ProjectControlResult {
            id: project.id.to_string(),
            external_project_id: project.plan.id.as_str().to_string(),
            receipt: super::child::WorkControlReceipt::Abandon { receipt },
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
        let status = block_on_project(async {
            let store = project_store().await?;
            project_work_status(&store, &project).await
        })?;
        let done = match until {
            ProjectWaitUntil::Waiting => !matches!(status, WorkStatus::Running { .. }),
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
    let status = block_on_project(async {
        let store = project_store().await?;
        project_work_status(&store, &project).await
    })?;
    if !matches!(status, WorkStatus::Running { .. }) {
        return Err(project_error(format!(
            "Project {} is not running; run `lf project resume {}` first",
            project.plan.slug,
            project.plan.id.as_str()
        )));
    }
    let run = block_on_project(async {
        let store = project_store().await?;
        let work = store
            .work_for_child(&ChildRef::Project(project.id.clone()))
            .await
            .map_err(|error| project_error(error.to_string()))?;
        let run = store
            .current_run(&work)
            .await
            .map_err(|error| project_error(error.to_string()))?
            .ok_or_else(|| project_error("Project has no active Run"))?;
        Ok(run)
    })?;
    let Some(Containment::Tmux { name }) = run.containment else {
        return Err(project_error("Project Run is not attachable"));
    };
    let status = std::process::Command::new("tmux")
        .args(["attach-project", "-t", &name])
        .status()
        .map_err(|error| project_error(format!("failed to attach Project: {error}")))?;
    if !status.success() {
        return Err(project_error("tmux attach failed"));
    }
    Ok(())
}

pub(crate) async fn wake_project(project_id: &ProjectId) -> OpsResult<()> {
    let store = project_store().await?;
    let mut project = store
        .get_project(project_id)
        .await
        .map_err(|error| project_error(error.to_string()))?
        .ok_or_else(|| project_error(format!("Project {project_id} not found")))?;
    // A wake is a supervisor restart: a Task observation arrived and the Project
    // may want to judge it. That is never a reason to revive a Project whose end
    // was already decided.
    if let Some(bar) = project.supervisor_restart_bar() {
        tracing::info!(project = %project_id, "not waking Project: {bar}");
        return Ok(());
    }
    if !matches!(
        project_work_status(&store, &project).await?,
        WorkStatus::Running { .. }
    ) {
        launch_project_process(&store, &mut project).await?;
    }
    Ok(())
}

pub(crate) async fn wake_task_project_route(_store: &Store, task: &Task) -> OpsResult<()> {
    wake_project(&task.project_id).await
}

/// Persist the child's ancestry before the authored promotion flow creates its
/// first Initiative or Project. Completion records the promotion occurrence.
pub fn prepare_promotion(repo: &Path, parent: &str, child: &str) -> OpsResult<()> {
    let origin = crate::engine::wave_context::wave_origin(repo);
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
    let parent_locator = crate::wave::WaveLocator::discover(repo, parent)
        .map_err(|error| project_error(error.to_string()))?;
    let child_locator = crate::wave::WaveLocator::discover(repo, child)
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
        record_promotion(&store, &origin, parent, child).await?;

        if crate::wave::server::live_endpoint(&origin, child)
            .await
            .is_none()
        {
            launch_residency(&origin, child).await?;
        }
        for _ in 0..100 {
            if let Some(endpoint) = crate::wave::server::live_endpoint(&origin, child).await {
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
    let parent_locator = crate::wave::WaveLocator::discover(repo, parent)
        .map_err(|error| OpsError::Message(error.to_string()))?;
    let child_locator = crate::wave::WaveLocator::discover(repo, child)
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
            open_store(&StorageConfig::sqlite(home.path().join("loopflow.db")))
                .await
                .unwrap(),
        );
        let now = time::OffsetDateTime::now_utc();
        let mut project = Project {
            id: ProjectId::new(),
            plan: ProjectPlan {
                id: LinearProjectId::new("project-no-pin").unwrap(),
                slug: "no-pin".to_string(),
                name: "No pin".to_string(),
                prompt_context: String::new(),
                pm_snapshot_synced_at: now.unix_timestamp(),
            },
            wave_id: WaveId::new(),
            iteration: 1,
            observation_cursor: 0,
            last_state_fingerprint: None,
            agent: "claude".to_string(),
            provider: "claude".to_string(),
            provider_session_id: None,
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
        let result = super::launch_project_process(&store, &mut project).await;
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
        let store = open_store(&StorageConfig::sqlite(database.clone()))
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
            .get_wave_at(&crate::wave::WaveLocator::discover(tmp.path(), "infrastructure").unwrap())
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
        let store = open_store(&StorageConfig::sqlite(database.clone()))
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

        let locator = crate::wave::WaveLocator::discover(tmp.path(), "release-stability").unwrap();
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
