//! `lf ls` and `lf status` — read the wave registry (`store`).
//!
//! `lf ls` lists every durable Wave registry row and projects authored policy
//! from `GOAL.md` plus current listener presence. `lf status <wave>` adds the
//! Wave's Project/Task hierarchy and live loop state. Both are
//! read-only; `--json` is the dashboard contract. A stopped Wave remains
//! visible, inert, and restartable.

use std::path::Path;

use anyhow::{anyhow, Result};
use serde::{Deserialize, Serialize};

use crate::child_session::{ChildRef, DirectiveKind, SessionSupervisor};
use crate::lf::output::Colors;
use crate::pm::{PmItem, PmKr, PmProject};
use crate::project_session::{ProjectSession, ProjectSessionStatus};
use crate::store::{open_existing_store, SharedStore};
use crate::task::{TaskSession, TaskSessionStatus};
use crate::wave::server::live_endpoint;
use crate::wave::Wave;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum WavePresence {
    Idle,
    Running,
    Paused,
}

impl WavePresence {
    fn as_str(self) -> &'static str {
        match self {
            Self::Idle => "idle",
            Self::Running => "running",
            Self::Paused => "paused",
        }
    }
}

impl std::fmt::Display for WavePresence {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(self.as_str())
    }
}

/// One wave's registry snapshot — the `lf ls` row and the `wave` field of
/// `lf status`. Wire type consumed by Loopflow: every field is required or
/// explicitly Optional, no serde defaults.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WaveSnapshot {
    pub id: String,
    pub name: String,
    /// Wave presence (`idle | running | paused`). Detailed resident condition
    /// is reported separately by `WaveDetailSnapshot::loop_state`.
    pub status: WavePresence,
    pub paused: bool,
    pub goal: String,
    /// Primary repo path.
    pub repo: String,
    /// Non-terminal Task Sessions owned by this Wave.
    pub active_tasks: u32,
    /// Non-terminal Project Sessions owned by this Wave.
    pub active_projects: u32,
    /// Whether a wave server answered `/health` at the discovery endpoint.
    pub live: bool,
    /// Loopback endpoint of the live server, `null` when stopped.
    pub endpoint: Option<String>,
    /// RFC3339 creation time, `null` when the row predates the column.
    pub created_at: Option<String>,
    /// Parent wave id in the chord tree, `null` for a root wave.
    pub parent_wave_id: Option<String>,
}

/// `lf status <wave>` snapshot: native work hierarchy and — when a
/// server is live — loop state. Wire type; no defaults.
#[derive(Debug, Serialize, Deserialize)]
pub struct WaveDetailSnapshot {
    pub wave: WaveSnapshot,
    /// Resident loop state name from the live server's `/health`
    /// (`idle | turning | interrupting | failed`), `null` when stopped or
    /// serving dormant.
    pub loop_state: Option<String>,
    pub projects: Vec<ProjectDetailSnapshot>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PmKrSummary {
    pub text: String,
    pub holds: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PmProjectSummary {
    pub id: String,
    pub slug: String,
    pub name: String,
    pub summary: String,
    pub definition: String,
    pub krs: Vec<PmKrSummary>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PmTaskSummary {
    pub id: String,
    pub identifier: String,
    pub name: String,
    pub description: String,
    pub rank: u32,
    pub completed: bool,
    pub assignee: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum NextMoveOwner {
    Human,
    Wave,
    Project,
    Task,
    Review,
    Ci,
    External,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NextMove {
    pub owner: NextMoveOwner,
    pub reason: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DirectiveSnapshot {
    pub version: u32,
    pub kind: DirectiveKind,
    pub text: String,
    pub applied_at: Option<String>,
    pub incorporated_at: Option<String>,
    pub incorporated_summary: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProjectRuntimeSnapshot {
    pub session_id: String,
    pub status: ProjectSessionStatus,
    pub reason: String,
    pub status_at: String,
    pub iteration: u32,
    pub pending_observations: u32,
    pub provider: String,
    pub process_alive: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TaskRuntimeSnapshot {
    pub session_id: String,
    pub supervisor: SessionSupervisor,
    pub status: TaskSessionStatus,
    pub reason: String,
    pub status_at: String,
    pub worktree: String,
    pub branch: String,
    pub provider: String,
    pub process_alive: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TaskDetailSnapshot {
    pub task: PmTaskSummary,
    pub runtime: Option<TaskRuntimeSnapshot>,
    pub directive: Option<DirectiveSnapshot>,
    pub next_move: NextMove,
    pub pull_request: Option<crate::task::PullRequestRef>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProjectDetailSnapshot {
    pub project: PmProjectSummary,
    pub runtime: Option<ProjectRuntimeSnapshot>,
    pub directive: Option<DirectiveSnapshot>,
    pub next_move: NextMove,
    pub tasks: Vec<TaskDetailSnapshot>,
}

/// `lf ls` — every wave the registry knows, running and stopped alike.
pub fn ls(json: bool) -> Result<()> {
    let rt = tokio::runtime::Runtime::new()?;
    rt.block_on(async {
        let Some(store) = open_existing_store().await.map(std::sync::Arc::new) else {
            return no_registry(json, "[]");
        };
        let waves = store
            .list_waves(None)
            .await
            .map_err(|err| anyhow!("failed to read wave registry: {err}"))?;
        let mut snapshots = Vec::with_capacity(waves.len());
        for wave in waves {
            snapshots.push(snapshot_wave(&store, &wave).await?);
        }
        snapshots.sort_by(|a, b| a.name.cmp(&b.name));
        if json {
            println!("{}", serde_json::to_string(&snapshots)?);
        } else {
            print_wave_table(&snapshots);
        }
        Ok(())
    })
}

/// `lf status <wave>` — one wave's work hierarchy and loop.
pub fn status(wave: Option<&str>, json: bool) -> Result<()> {
    let name = wave
        .map(str::to_string)
        .or_else(ambient_wave)
        .ok_or_else(|| anyhow!("no wave given and none in context; pass a wave name"))?;
    let rt = tokio::runtime::Runtime::new()?;
    rt.block_on(async {
        let Some(store) = open_existing_store().await.map(std::sync::Arc::new) else {
            return no_registry(json, "null");
        };
        let wave = store
            .get_wave_by_name(&name)
            .await
            .map_err(|err| anyhow!("failed to read wave registry: {err}"))?
            .ok_or_else(|| anyhow!("wave '{name}' is not in the registry"))?;
        let snapshot = snapshot_wave(&store, &wave).await?;
        let loop_state = match &snapshot.endpoint {
            Some(endpoint) => loop_state(endpoint).await,
            None => None,
        };
        let stored_projects = store
            .list_project_sessions(Some(wave.id()))
            .await
            .map_err(|err| anyhow!("failed to read Project Sessions: {err}"))?;
        let stored_tasks = store
            .list_task_sessions(Some(wave.id()))
            .await
            .map_err(|err| anyhow!("failed to read Task Sessions: {err}"))?;
        let projects = snapshot_projects(&store, &wave, stored_projects, stored_tasks).await?;
        let status = WaveDetailSnapshot {
            wave: snapshot,
            loop_state,
            projects,
        };
        if json {
            println!("{}", serde_json::to_string(&status)?);
        } else {
            print_status(&status);
        }
        Ok(())
    })
}

/// Build the registry snapshot for one wave, probing its discovery endpoint
/// for liveness.
async fn snapshot_wave(store: &SharedStore, wave: &Wave) -> Result<WaveSnapshot> {
    let repo = wave.repo().to_string();
    let endpoint = if repo.is_empty() {
        None
    } else {
        live_endpoint(Path::new(&repo), wave.name()).await
    };
    let active_tasks = store
        .list_task_sessions(Some(wave.id()))
        .await
        .map_err(|err| anyhow!("failed to count active Task Sessions: {err}"))?
        .into_iter()
        .filter(|session| !session.status.is_terminal())
        .count() as u32;
    let active_projects = store
        .list_project_sessions(Some(wave.id()))
        .await
        .map_err(|err| anyhow!("failed to count active Project Sessions: {err}"))?
        .into_iter()
        .filter(|session| !session.status.is_terminal())
        .count() as u32;
    let config = crate::engine::wave_config::read_wave_config(Path::new(&repo), wave.name());
    let paused = config.and_then(|config| config.paused).unwrap_or(false);
    let live = endpoint.is_some();
    let status = if paused {
        WavePresence::Paused
    } else if live {
        WavePresence::Running
    } else {
        WavePresence::Idle
    };
    Ok(WaveSnapshot {
        id: wave.id().to_string(),
        name: wave.name().to_string(),
        status,
        paused,
        goal: crate::engine::wave_config::read_wave_summary(Path::new(&repo), wave.name())
            .unwrap_or_else(|_| wave.name().to_string()),
        repo,
        active_tasks,
        active_projects,
        live: endpoint.is_some(),
        endpoint,
        created_at: wave.created_at().and_then(format_time),
        parent_wave_id: wave.parent_wave_id().map(ToString::to_string),
    })
}

async fn snapshot_task_runtime(task: &TaskSession) -> TaskRuntimeSnapshot {
    let process_alive = if task.status.is_process_active() {
        match task.latest_process.as_ref() {
            Some(process) => crate::engine::process::tmux_session_exists(&process.tmux_name)
                .await
                .unwrap_or(false),
            None => false,
        }
    } else {
        false
    };
    TaskRuntimeSnapshot {
        session_id: task.id.to_string(),
        supervisor: task.supervisor.clone(),
        status: task.status,
        reason: task.status_reason.clone(),
        status_at: format_time(task.status_at).unwrap_or_default(),
        worktree: task.worktree.display().to_string(),
        branch: task.branch.clone(),
        provider: task.provider.clone(),
        process_alive,
    }
}

async fn snapshot_project_runtime(
    store: &SharedStore,
    project: &ProjectSession,
) -> Result<ProjectRuntimeSnapshot> {
    let process_alive = if project.status.is_process_active() {
        match project.latest_process.as_ref() {
            Some(process) => crate::engine::process::tmux_session_exists(&process.tmux_name)
                .await
                .unwrap_or(false),
            None => false,
        }
    } else {
        false
    };
    let pending_observations = store
        .pending_observations(&SessionSupervisor::Project {
            session_id: project.id.clone(),
        })
        .await
        .map_err(|err| anyhow!("failed to read Project observation outbox: {err}"))?
        .len() as u32;
    Ok(ProjectRuntimeSnapshot {
        session_id: project.id.to_string(),
        status: project.status,
        reason: project.status_reason.clone(),
        status_at: format_time(project.status_at).unwrap_or_default(),
        iteration: project.iteration,
        pending_observations,
        provider: project.provider.clone(),
        process_alive,
    })
}

#[derive(Debug, Deserialize)]
struct CachedPmSnapshot {
    projects: Vec<PmProject>,
    items: Vec<PmItem>,
}

async fn snapshot_projects(
    store: &SharedStore,
    wave: &Wave,
    project_sessions: Vec<ProjectSession>,
    task_sessions: Vec<TaskSession>,
) -> Result<Vec<ProjectDetailSnapshot>> {
    let repo = crate::engine::worktrees::main_repo_root(Path::new(wave.repo()))
        .unwrap_or_else(|_| Path::new(wave.repo()).to_path_buf());
    let repo = std::fs::canonicalize(&repo).unwrap_or(repo);
    let planning = match store
        .pm_snapshot(repo.to_string_lossy().into_owned(), wave.name().to_string())
        .await
        .map_err(|err| anyhow!("failed to read PM snapshot: {err}"))?
    {
        Some(row) => serde_json::from_str::<CachedPmSnapshot>(&row.payload).map_err(|err| {
            anyhow!(
                "invalid PM snapshot for wave/{}; run `lf pm sync`: {err}",
                wave.name()
            )
        })?,
        None => CachedPmSnapshot {
            projects: Vec::new(),
            items: Vec::new(),
        },
    };

    let mut details = planning
        .projects
        .into_iter()
        .map(|project| ProjectDetailSnapshot {
            next_move: next_move_for_unstarted_project(&project),
            project: project_summary(project),
            runtime: None,
            directive: None,
            tasks: Vec::new(),
        })
        .collect::<Vec<_>>();

    for project_session in &project_sessions {
        let index = project_index(
            &details,
            project_session.launch.project.id.as_str(),
            &project_session.launch.project.slug,
        )?;
        details[index].next_move =
            next_move_for_project(project_session.status, &project_session.status_reason);
        details[index].runtime = Some(snapshot_project_runtime(store, project_session).await?);
        details[index].directive = current_directive(
            store,
            ChildRef::Project(project_session.id.clone()),
            project_session.current_directive_version,
        )
        .await?;
    }

    for item in planning.items {
        let project_slug = item.project.as_deref().ok_or_else(|| {
            anyhow!(
                "Task {} belongs to no Project in the PM snapshot; fix it in Linear and run `lf pm sync --wave {}`",
                item.identifier,
                wave.name()
            )
        })?;
        let index = project_index(&details, project_slug, project_slug)?;
        let runtime_session = task_sessions.iter().find(|session| {
            session.launch.issue.id.as_str() == item.id
                || session.launch.issue.identifier == item.identifier
        });
        details[index]
            .tasks
            .push(snapshot_task_detail(store, item, runtime_session).await?);
    }

    for task_session in &task_sessions {
        let project_index = project_index(
            &details,
            task_session.launch.project.id.as_str(),
            &task_session.launch.project.slug,
        )?;
        if details[project_index].tasks.iter().any(|task| {
            task.task.id == task_session.launch.issue.id.as_str()
                || task.task.identifier == task_session.launch.issue.identifier
        }) {
            continue;
        }
        let task = PmItem {
            id: task_session.launch.issue.id.as_str().to_string(),
            identifier: task_session.launch.issue.identifier.clone(),
            name: task_session.launch.issue.title.clone(),
            description: task_session.launch.issue.description.clone(),
            rank: u32::MAX,
            completed: task_session.status.is_terminal(),
            project: Some(task_session.launch.project.slug.clone()),
            assignee: None,
        };
        details[project_index]
            .tasks
            .push(snapshot_task_detail(store, task, Some(task_session)).await?);
    }

    for project in &mut details {
        project.tasks.sort_by(|left, right| {
            left.task
                .completed
                .cmp(&right.task.completed)
                .then(left.task.rank.cmp(&right.task.rank))
                .then(left.task.identifier.cmp(&right.task.identifier))
        });
    }
    Ok(details)
}

fn project_index(projects: &[ProjectDetailSnapshot], id: &str, slug: &str) -> Result<usize> {
    projects
        .iter()
        .position(|project| project.project.id == id || project.project.slug == slug)
        .ok_or_else(|| {
            anyhow!(
                "Project {slug} ({id}) is not present in the current PM snapshot; run `lf pm sync` before reading the Wave work map"
            )
        })
}

async fn snapshot_task_detail(
    store: &SharedStore,
    item: PmItem,
    session: Option<&TaskSession>,
) -> Result<TaskDetailSnapshot> {
    let runtime = match session {
        Some(session) => Some(snapshot_task_runtime(session).await),
        None => None,
    };
    let next_move = match session {
        Some(session) => {
            next_move_for_task(session.status, &session.status_reason, &session.supervisor)
        }
        None if item.completed => NextMove {
            owner: NextMoveOwner::Project,
            reason: "Linear Task is complete".to_string(),
        },
        None => NextMove {
            owner: NextMoveOwner::Project,
            reason: "Task is ready to start".to_string(),
        },
    };
    let pull_request = session.and_then(|session| session.pull_request.clone());
    let directive = match session {
        Some(session) => {
            current_directive(
                store,
                ChildRef::Task(session.id.clone()),
                session.current_directive_version,
            )
            .await?
        }
        None => None,
    };
    Ok(TaskDetailSnapshot {
        task: task_summary(item),
        runtime,
        directive,
        next_move,
        pull_request,
    })
}

async fn current_directive(
    store: &SharedStore,
    target: ChildRef,
    version: u32,
) -> Result<Option<DirectiveSnapshot>> {
    if version == 0 {
        return Ok(None);
    }
    let directive = store
        .child_directives(&target)
        .await
        .map_err(|err| anyhow!("failed to read child directives: {err}"))?
        .into_iter()
        .find(|directive| directive.version == version)
        .ok_or_else(|| {
            anyhow!(
                "{} {} points at missing directive v{version}",
                target.target_kind(),
                target.target_id()
            )
        })?;
    Ok(Some(DirectiveSnapshot {
        version: directive.version,
        kind: directive.kind,
        text: directive.text,
        applied_at: directive.applied_at.and_then(format_time),
        incorporated_at: directive.incorporated_at.and_then(format_time),
        incorporated_summary: directive.incorporated_summary,
    }))
}

fn project_summary(project: PmProject) -> PmProjectSummary {
    PmProjectSummary {
        id: project.id,
        slug: project.slug,
        name: project.name,
        summary: project.summary,
        definition: project.definition,
        krs: project.krs.into_iter().map(kr_summary).collect(),
    }
}

fn kr_summary(kr: PmKr) -> PmKrSummary {
    PmKrSummary {
        text: kr.text,
        holds: kr.holds,
    }
}

fn task_summary(item: PmItem) -> PmTaskSummary {
    PmTaskSummary {
        id: item.id,
        identifier: item.identifier,
        name: item.name,
        description: item.description,
        rank: item.rank,
        completed: item.completed,
        assignee: item.assignee,
    }
}

fn next_move_for_unstarted_project(project: &PmProject) -> NextMove {
    if !project.krs.is_empty() && project.krs.iter().all(|kr| kr.holds) {
        NextMove {
            owner: NextMoveOwner::Wave,
            reason: "Every current KR holds".to_string(),
        }
    } else {
        NextMove {
            owner: NextMoveOwner::Wave,
            reason: "Project is ready to start".to_string(),
        }
    }
}

fn next_move_for_project(status: ProjectSessionStatus, reason: &str) -> NextMove {
    let owner = match status {
        ProjectSessionStatus::Created
        | ProjectSessionStatus::Starting
        | ProjectSessionStatus::Running
        | ProjectSessionStatus::Waiting => NextMoveOwner::Project,
        ProjectSessionStatus::Blocked | ProjectSessionStatus::Failed => NextMoveOwner::Wave,
        ProjectSessionStatus::Completed | ProjectSessionStatus::Abandoned => NextMoveOwner::Wave,
    };
    NextMove {
        owner,
        reason: reason.to_string(),
    }
}

fn next_move_for_task(
    status: TaskSessionStatus,
    reason: &str,
    supervisor: &SessionSupervisor,
) -> NextMove {
    let controller = match supervisor {
        SessionSupervisor::Wave { .. } => NextMoveOwner::Wave,
        SessionSupervisor::Project { .. } => NextMoveOwner::Project,
    };
    let owner = match status {
        TaskSessionStatus::Created | TaskSessionStatus::Starting | TaskSessionStatus::Running => {
            NextMoveOwner::Task
        }
        TaskSessionStatus::Waiting | TaskSessionStatus::Blocked | TaskSessionStatus::Failed => {
            controller
        }
        TaskSessionStatus::Submitted => NextMoveOwner::Review,
        TaskSessionStatus::Merged | TaskSessionStatus::Abandoned => NextMoveOwner::Project,
    };
    NextMove {
        owner,
        reason: reason.to_string(),
    }
}

/// The invoking context's wave: `LF_WAVE_ID` env, else `None` (the caller
/// errors). Kept minimal — `lf status` with no arg is a convenience, not the
/// resolution surface `lf chat`/`lf radio sub` own.
fn ambient_wave() -> Option<String> {
    std::env::var(crate::engine::wave_context::WAVE_ID_ENV)
        .ok()
        .filter(|value| !value.is_empty())
}

/// Ask a live server for its resident loop state (`/health` `loop` field).
async fn loop_state(endpoint: &str) -> Option<String> {
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(2))
        .build()
        .ok()?;
    let body: serde_json::Value = client
        .get(format!("http://{endpoint}/health"))
        .send()
        .await
        .ok()?
        .json()
        .await
        .ok()?;
    body.get("loop_state")
        .and_then(serde_json::Value::as_str)
        .map(str::to_string)
}

fn format_time(ts: time::OffsetDateTime) -> Option<String> {
    ts.format(&time::format_description::well_known::Rfc3339)
        .ok()
}

/// With no registry on this machine, `lf ls`/`status` have nothing to read —
/// emit the empty snapshot (`[]`/`null`) or a human note, and succeed.
fn no_registry(json: bool, empty: &str) -> Result<()> {
    if json {
        println!("{empty}");
    } else {
        println!("No wave registry on this machine yet.");
    }
    Ok(())
}

fn print_wave_table(snapshots: &[WaveSnapshot]) {
    if snapshots.is_empty() {
        println!("No waves in the registry.");
        return;
    }
    let colors = Colors::default();
    println!(
        "{bold}{name:<16}  {status:<8}  {live:<5}  {tasks:>5}  {projects:>8}  ENDPOINT{reset}",
        bold = colors.bold,
        reset = colors.reset,
        name = "WAVE",
        status = "STATUS",
        live = "LIVE",
        tasks = "TASKS",
        projects = "PROJECTS",
    );
    for wave in snapshots {
        println!(
            "{name:<16}  {status:<8}  {live:<5}  {tasks:>5}  {projects:>8}  {endpoint}",
            name = truncate(&wave.name, 16),
            status = wave.status,
            live = if wave.live { "yes" } else { "no" },
            tasks = wave.active_tasks,
            projects = wave.active_projects,
            endpoint = wave.endpoint.as_deref().unwrap_or("-"),
        );
    }
}

fn print_status(status: &WaveDetailSnapshot) {
    let colors = Colors::default();
    let wave = &status.wave;
    println!(
        "{bold}{name}{reset}  {status}{loop_state}",
        bold = colors.bold,
        reset = colors.reset,
        name = wave.name,
        status = wave.status,
        loop_state = status
            .loop_state
            .as_deref()
            .map(|m| format!("  loop:{m}"))
            .unwrap_or_default(),
    );
    println!("  goal      {}", wave.goal);
    println!(
        "  endpoint  {}",
        wave.endpoint.as_deref().unwrap_or("(stopped)")
    );
    if status.projects.is_empty() {
        println!("  projects  none");
    } else {
        println!("  projects");
        for project in &status.projects {
            let (project_status, iteration, reason) = match &project.runtime {
                Some(runtime) => (
                    runtime.status.as_str(),
                    runtime.iteration,
                    runtime.reason.as_str(),
                ),
                None => ("unstarted", 0, project.next_move.reason.as_str()),
            };
            println!(
                "    {project:<24}  {status:<10}  iteration {iteration:<3}  {reason}",
                project = truncate(&project.project.slug, 24),
                status = project_status,
                iteration = iteration,
                reason = reason,
            );
            for task in &project.tasks {
                let (task_status, reason) = match &task.runtime {
                    Some(runtime) => (runtime.status.as_str(), runtime.reason.as_str()),
                    None if task.task.completed => ("completed", task.next_move.reason.as_str()),
                    None => ("unstarted", task.next_move.reason.as_str()),
                };
                println!(
                    "      {issue:<12}  {status:<10}  {reason}",
                    issue = task.task.identifier,
                    status = task_status,
                    reason = reason,
                );
            }
        }
    }
}

fn truncate(value: &str, width: usize) -> String {
    if value.chars().count() <= width {
        return value.to_string();
    }
    let head: String = value.chars().take(width.saturating_sub(1)).collect();
    format!("{head}\u{2026}")
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use super::*;
    use crate::id::WaveId;
    use crate::store::{open_store, PmSnapshotRow, StorageConfig};

    #[test]
    fn unknown_project_is_a_snapshot_error_not_a_synthetic_project() {
        let error = project_index(&[], "project-1", "missing")
            .expect_err("unknown Project must fail loudly");

        assert!(error.to_string().contains("lf pm sync"));
    }

    #[tokio::test]
    async fn cached_pm_snapshot_builds_the_native_project_task_hierarchy() {
        let dir = tempfile::tempdir().expect("tempdir");
        let repo = std::fs::canonicalize(dir.path()).expect("canonical repo");
        let store = Arc::new(
            open_store(&StorageConfig::sqlite(dir.path().join("registry.db")))
                .await
                .expect("open store"),
        );
        let wave = Wave::new(
            WaveId::new(),
            "infrastructure".to_string(),
            repo.display().to_string(),
        );
        store
            .put_pm_snapshot(PmSnapshotRow {
                repo: repo.display().to_string(),
                wave: wave.name().to_string(),
                provider: "linear".to_string(),
                initiative: "initiative-1".to_string(),
                synced_at: 1,
                payload: serde_json::json!({
                    "projects": [{
                        "id": "project-1",
                        "slug": "first-run",
                        "name": "First run",
                        "summary": "Make first run clear",
                        "definition": "A new user succeeds without help",
                        "krs": [{"text": "Parser accepts --hello", "holds": false}],
                        "initiative_ids": ["initiative-1"]
                    }],
                    "items": [
                        {
                            "id": "issue-1",
                            "identifier": "INF-123",
                            "name": "Fix parser",
                            "description": "Accept --hello",
                            "rank": 1,
                            "completed": false,
                            "project": "first-run",
                            "assignee": null
                        },
                        {
                            "id": "issue-2",
                            "identifier": "INF-124",
                            "name": "Update docs",
                            "description": "Explain --hello",
                            "rank": 2,
                            "completed": false,
                            "project": "first-run",
                            "assignee": null
                        }
                    ]
                })
                .to_string(),
            })
            .await
            .expect("write cached PM snapshot");

        let projects = snapshot_projects(&store, &wave, Vec::new(), Vec::new())
            .await
            .expect("build native hierarchy");

        assert_eq!(projects.len(), 1);
        assert_eq!(projects[0].project.slug, "first-run");
        assert_eq!(
            projects[0]
                .tasks
                .iter()
                .map(|task| task.task.identifier.as_str())
                .collect::<Vec<_>>(),
            ["INF-123", "INF-124"]
        );
        assert!(projects[0].runtime.is_none());
        assert!(projects[0].tasks.iter().all(|task| task.runtime.is_none()));
    }

    #[test]
    fn wave_snapshot_json_has_stable_keys() {
        let snapshot = WaveSnapshot {
            id: "wave-1".into(),
            name: "goals".into(),
            status: WavePresence::Running,
            paused: false,
            goal: "ship the roadmap".into(),
            repo: "/repo".into(),
            active_tasks: 2,
            active_projects: 1,
            live: true,
            endpoint: Some("127.0.0.1:5678".into()),
            created_at: Some("2026-07-06T00:00:00Z".into()),
            parent_wave_id: None,
        };
        let value: serde_json::Value = serde_json::to_value(&snapshot).expect("serialize");
        assert_eq!(value["name"], "goals");
        assert_eq!(value["status"], "running");
        assert_eq!(value["live"], true);
        assert_eq!(value["endpoint"], "127.0.0.1:5678");
        assert_eq!(value["active_tasks"], 2);
        // Explicitly-null Optional stays present (no serde skip): a stopped
        // wave's endpoint is `null`, not absent — one stable shape.
        assert!(value.as_object().unwrap().contains_key("parent_wave_id"));
        assert_eq!(value["parent_wave_id"], serde_json::Value::Null);
    }

    #[test]
    fn status_snapshot_nests_wave_work() {
        let status = WaveDetailSnapshot {
            wave: WaveSnapshot {
                id: "wave-1".into(),
                name: "goals".into(),
                status: WavePresence::Idle,
                paused: false,
                goal: "g".into(),
                repo: "/repo".into(),
                active_tasks: 0,
                active_projects: 0,
                live: false,
                endpoint: None,
                created_at: None,
                parent_wave_id: None,
            },
            loop_state: None,
            projects: vec![ProjectDetailSnapshot {
                project: PmProjectSummary {
                    id: "project-1".into(),
                    slug: "runtime".into(),
                    name: "Runtime".into(),
                    summary: "Run reliably".into(),
                    definition: "Keep execution boring".into(),
                    krs: vec![PmKrSummary {
                        text: "Survives restart".into(),
                        holds: true,
                    }],
                },
                runtime: None,
                directive: None,
                next_move: NextMove {
                    owner: NextMoveOwner::Wave,
                    reason: "Project is ready to start".into(),
                },
                tasks: vec![TaskDetailSnapshot {
                    task: PmTaskSummary {
                        id: "issue-1".into(),
                        identifier: "INF-123".into(),
                        name: "Wire it".into(),
                        description: String::new(),
                        rank: 1,
                        completed: false,
                        assignee: None,
                    },
                    runtime: None,
                    directive: None,
                    next_move: NextMove {
                        owner: NextMoveOwner::Project,
                        reason: "Task is ready to start".into(),
                    },
                    pull_request: None,
                }],
            }],
        };
        let value: serde_json::Value = serde_json::to_value(&status).expect("serialize");
        assert_eq!(value["wave"]["name"], "goals");
        assert_eq!(value["loop_state"], serde_json::Value::Null);
        assert_eq!(value["projects"][0]["project"]["slug"], "runtime");
        assert_eq!(
            value["projects"][0]["tasks"][0]["task"]["identifier"],
            "INF-123"
        );
        assert_eq!(
            value["projects"][0]["tasks"][0]["runtime"],
            serde_json::Value::Null
        );
    }
}
