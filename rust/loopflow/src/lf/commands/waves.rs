//! `lf ls` and `lf status` — read the wave registry (`store`).
//!
//! `lf ls` lists every durable Wave registry row and projects authored policy
//! from `GOAL.md` plus current listener presence. `lf status [wave]` adds the
//! Wave's Project/Task hierarchy, the runs it has produced, what is waiting on
//! somebody, and live loop state; with no argument it reports the Wave this
//! process is running inside. Both are read-only; `--json` is the dashboard
//! contract. A stopped Wave remains visible, inert, and restartable.
//!
//! Evidence the machine could not read stays [`Evidence::Unavailable`] — an
//! audit surface that renders "I could not look" as "nothing happened" is worse
//! than one that says nothing at all.

use std::path::Path;

use anyhow::{anyhow, Result};
use serde::{Deserialize, Serialize};

use crate::child_session::{ChildRef, DirectiveKind, ObservationRecipient};
use crate::engine::wave_home::{HomeActionDto, HomeRuntimeDto, HomeState, WaveHomeDto};
use crate::lf::commands::runs::RunLedgerEntry;
use crate::lf::output::Colors;
use crate::pm::{PmItem, PmKr, PmProject};
use crate::project_session::{ProjectSession, ProjectSessionStatus};
use crate::store::{open_existing_store, SharedStore};
use crate::task::{AfterMerge, PrPhase, TaskPr, TaskSession, TaskSessionStatus};
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
    /// Execution home: `local` or one SSH target. Lets a consumer distinguish a
    /// local Wave from a remote-home one without owning transport.
    pub home: WaveHomeDto,
}

/// `lf status <wave>` snapshot: native work hierarchy, the wave's runs, what
/// needs attention, and — when a server is live — loop state. Wire type; no
/// defaults.
#[derive(Debug, Serialize, Deserialize)]
pub struct WaveDetailSnapshot {
    pub wave: WaveSnapshot,
    /// Resident loop state name from the live server's `/health`
    /// (`idle | turning | interrupting | failed`), `null` when stopped or
    /// serving dormant.
    pub loop_state: Option<String>,
    pub projects: Vec<ProjectDetailSnapshot>,
    /// This wave's runs from the local ledger, newest first.
    pub runs: Evidence<RunLedgerEntry>,
    /// Work whose next move belongs to someone other than itself.
    pub attention: Evidence<AttentionItem>,
    /// The Wave's Home probed for liveness: state, evidence, attach endpoint, and
    /// the one contextual action a conductor surface should offer. Probed for the
    /// focused Wave only — `lf ls` stays address-only.
    pub home_runtime: HomeRuntimeDto,
}

/// A reading, or the reason there is none. "We looked and found nothing" and
/// "we could not look" are different facts, and an audit surface that renders
/// them the same is lying — so the wire says which.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "state", rename_all = "snake_case")]
pub enum Evidence<T> {
    /// The source answered. `truncated` says a cap hid older items, so a full
    /// page never reads as "that was all there was".
    Ok { items: Vec<T>, truncated: bool },
    /// The source could not be read. Never rendered as emptiness.
    Unavailable { reason: String },
}

impl<T> Evidence<T> {
    fn complete(items: Vec<T>) -> Self {
        Self::Ok {
            items,
            truncated: false,
        }
    }

    fn from_result(result: Result<(Vec<T>, bool)>) -> Self {
        match result {
            Ok((items, truncated)) => Self::Ok { items, truncated },
            Err(error) => Self::Unavailable {
                reason: error.to_string(),
            },
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AttentionKind {
    Project,
    Task,
}

/// One Session waiting on somebody. Derived from the durable Session registry —
/// every field traces to a recorded state, none is inferred.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AttentionItem {
    pub kind: AttentionKind,
    /// Session id — the drill-down key.
    pub id: String,
    /// Project slug or Task identifier.
    pub subject: String,
    /// Who has to move next.
    pub owner: NextMoveOwner,
    /// Why, in the Session's own recorded words — or the audit finding when the
    /// Session's record and the machine disagree.
    pub reason: String,
    /// RFC3339 time the Session entered this state; empty when unrecorded.
    pub since: String,
    /// How long it has been waiting. `null` when `since` cannot be read — an
    /// unknown age is never a zero one.
    pub age_secs: Option<i64>,
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

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
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
    pub project_session_id: String,
    pub status: TaskSessionStatus,
    pub reason: String,
    pub status_at: String,
    pub worktree: String,
    pub branch: Option<String>,
    pub provider: String,
    pub process_alive: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TaskDetailSnapshot {
    pub task: PmTaskSummary,
    pub runtime: Option<TaskRuntimeSnapshot>,
    pub directive: Option<DirectiveSnapshot>,
    pub next_move: NextMove,
    pub prs: Vec<PrSnapshot>,
    pub active_pr: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PrSnapshot {
    pub id: String,
    pub sequence: u32,
    pub slug: String,
    pub branch: String,
    pub base_commit: String,
    pub phase: PrPhase,
    pub empty: Option<bool>,
    pub publication: Option<PrPublicationSnapshot>,
    pub merge_commit: Option<String>,
    pub abandoned_at: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PrPublicationSnapshot {
    pub requested_at: String,
    pub after_merge: AfterMerge,
    pub next_slug: Option<String>,
    pub github: Option<GithubPrSnapshot>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GithubPrSnapshot {
    pub number: u32,
    pub url: String,
}

impl PrSnapshot {
    fn new(pr: &TaskPr, empty: Option<bool>) -> Self {
        Self {
            id: pr.id.to_string(),
            sequence: pr.sequence,
            slug: pr.slug.clone(),
            branch: pr.branch.clone(),
            base_commit: pr.base_commit.clone(),
            phase: pr.phase(),
            empty,
            publication: pr
                .publication
                .as_ref()
                .map(|publication| PrPublicationSnapshot {
                    requested_at: format_time(publication.requested_at)
                        .expect("PR publication timestamp formats as RFC 3339"),
                    after_merge: publication.after_merge,
                    next_slug: publication.next_slug.clone(),
                    github: publication.github.as_ref().map(|github| GithubPrSnapshot {
                        number: github.number,
                        url: github.url.clone(),
                    }),
                }),
            merge_commit: pr.merge_commit.clone(),
            abandoned_at: pr.abandoned_at.and_then(format_time),
        }
    }
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

/// `lf status [wave]` — one wave's work hierarchy, runs, attention, and loop.
pub fn status(wave: Option<&str>, json: bool) -> Result<()> {
    let rt = tokio::runtime::Runtime::new()?;
    rt.block_on(async {
        let Some(store) = open_existing_store().await.map(std::sync::Arc::new) else {
            return no_registry(json, "null");
        };
        let wave = resolve_status_wave(&store, wave).await?;
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
        let attention = Evidence::complete(attention(
            &projects,
            now(),
            Liveness::probe(crate::engine::process::tmux_installed()),
        ));
        // Probe the focused Wave's Home once so the detail carries live evidence
        // and the single contextual action (Open/Attach, Start, or reason).
        let home = crate::engine::wave_config::read_wave_home(Path::new(wave.repo()), wave.name());
        let home_runtime =
            crate::ops::home::probe_home(wave.name(), &home, Path::new(wave.repo())).await;
        let status = WaveDetailSnapshot {
            runs: Evidence::from_result(crate::lf::commands::runs::wave_runs(wave.name())),
            attention,
            wave: snapshot,
            loop_state,
            projects,
            home_runtime,
        };
        if json {
            println!("{}", serde_json::to_string(&status)?);
        } else {
            print_status(&status);
        }
        Ok(())
    })
}

/// The wave `lf status` is about: the name the caller typed, else the wave this
/// process is running inside.
async fn resolve_status_wave(store: &SharedStore, requested: Option<&str>) -> Result<Wave> {
    if let Some(name) = requested {
        return store
            .get_wave_by_name(name)
            .await
            .map_err(|err| anyhow!("failed to read wave registry: {err}"))?
            .ok_or_else(|| anyhow!("wave '{name}' is not in the registry"));
    }
    let ambient = ambient_wave()
        .ok_or_else(|| anyhow!("no wave given and none in context; pass a wave name"))?;
    // `LF_WAVE_ID` carries the durable wave *id*. Read it as one — a name lookup
    // is the fallback so a hand-set `LF_WAVE_ID=<name>` still works.
    if let Ok(id) = ambient.parse::<crate::id::WaveId>() {
        if let Some(wave) = store
            .get_wave(&id)
            .await
            .map_err(|err| anyhow!("failed to read wave registry: {err}"))?
        {
            return Ok(wave);
        }
    }
    store
        .get_wave_by_name(&ambient)
        .await
        .map_err(|err| anyhow!("failed to read wave registry: {err}"))?
        .ok_or_else(|| {
            anyhow!(
                "ambient wave '{ambient}' ({}) is not in this machine's registry; the context is stale — pass a wave name",
                crate::engine::wave_context::WAVE_ID_ENV
            )
        })
}

/// Whether this machine can tell a live Session process from a dead one. Without
/// tmux there is no way to look, and `process_alive: false` means "unknown", not
/// "gone" — a surface that reports the difference as a finding is inventing one.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Liveness {
    Observable,
    Unknowable,
}

impl Liveness {
    fn probe(tmux_installed: bool) -> Self {
        if tmux_installed {
            Self::Observable
        } else {
            Self::Unknowable
        }
    }

    /// A Session that records a live process the machine looked for and did not
    /// find.
    fn is_gone(self, claims_process: bool, process_alive: bool) -> bool {
        self == Self::Observable && claims_process && !process_alive
    }
}

/// What in this wave is waiting on somebody. Two rules, both read straight off
/// the Session registry:
///
/// 1. the Session's next move belongs to someone other than itself, or
/// 2. the Session's record claims a live process the machine cannot find — the
///    kind of disagreement an audit surface exists to show.
///
/// Plan rows with no Session are not attention: an unstarted backlog item is not
/// waiting on you.
fn attention(
    projects: &[ProjectDetailSnapshot],
    now: time::OffsetDateTime,
    liveness: Liveness,
) -> Vec<AttentionItem> {
    let mut items = Vec::new();
    for project in projects {
        if let Some(runtime) = &project.runtime {
            let dead = liveness.is_gone(runtime.status.is_process_active(), runtime.process_alive);
            let self_owned = matches!(project.next_move.owner, NextMoveOwner::Project);
            if dead || !(self_owned || runtime.status.is_terminal()) {
                items.push(AttentionItem {
                    kind: AttentionKind::Project,
                    id: runtime.session_id.clone(),
                    subject: project.project.slug.clone(),
                    owner: if dead {
                        NextMoveOwner::Wave
                    } else {
                        project.next_move.owner
                    },
                    reason: attention_reason(dead, runtime.status.as_str(), &runtime.reason),
                    since: runtime.status_at.clone(),
                    age_secs: age_secs(&runtime.status_at, now),
                });
            }
        }
        for task in &project.tasks {
            let Some(runtime) = &task.runtime else {
                continue;
            };
            let dead = liveness.is_gone(runtime.status.is_process_active(), runtime.process_alive);
            if !dead && matches!(task.next_move.owner, NextMoveOwner::Task) {
                continue;
            }
            if !dead && runtime.status.is_terminal() {
                continue;
            }
            items.push(AttentionItem {
                kind: AttentionKind::Task,
                id: runtime.session_id.clone(),
                subject: task.task.identifier.clone(),
                owner: if dead {
                    NextMoveOwner::Wave
                } else {
                    task.next_move.owner
                },
                reason: attention_reason(dead, runtime.status.as_str(), &runtime.reason),
                since: runtime.status_at.clone(),
                age_secs: age_secs(&runtime.status_at, now),
            });
        }
    }
    items.sort_by_key(|item| std::cmp::Reverse(item.age_secs));
    items
}

fn attention_reason(dead: bool, status: &str, recorded: &str) -> String {
    if dead {
        format!("process is gone but the Session still records '{status}'")
    } else {
        recorded.to_string()
    }
}

fn age_secs(since: &str, now: time::OffsetDateTime) -> Option<i64> {
    let since =
        time::OffsetDateTime::parse(since, &time::format_description::well_known::Rfc3339).ok()?;
    Some((now - since).whole_seconds().max(0))
}

fn now() -> time::OffsetDateTime {
    time::OffsetDateTime::now_utc()
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
    let home = WaveHomeDto::from(
        &config
            .as_ref()
            .and_then(|config| config.home_authored())
            .unwrap_or_else(|| crate::engine::wave_config::default_local_home(Path::new(&repo))),
    );
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
        home,
    })
}

async fn snapshot_task_runtime(
    task: &TaskSession,
    active_pr: Option<&TaskPr>,
) -> TaskRuntimeSnapshot {
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
        project_session_id: task.project_session_id.to_string(),
        status: task.status,
        reason: task.status_reason.clone(),
        status_at: format_time(task.status_at).unwrap_or_default(),
        worktree: task.worktree.display().to_string(),
        branch: active_pr.map(|pr| pr.branch.clone()),
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
        .pending_observations(&ObservationRecipient::Project {
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
        if details[index].runtime.is_some() {
            continue;
        }
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
    let prs = match session {
        Some(session) => store.task_prs(&session.id).await?,
        None => Vec::new(),
    };
    let active = prs.iter().find(|pr| pr.is_active());
    let runtime = match session {
        Some(session) => Some(snapshot_task_runtime(session, active).await),
        None => None,
    };
    let next_move = match session {
        Some(session) => next_move_for_task(
            session.status,
            active.map(TaskPr::phase),
            &session.status_reason,
        ),
        None if item.completed => NextMove {
            owner: NextMoveOwner::Project,
            reason: "Linear Task is complete".to_string(),
        },
        None => NextMove {
            owner: NextMoveOwner::Project,
            reason: "Task is ready to start".to_string(),
        },
    };
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
        prs: prs
            .iter()
            .map(|pr| {
                let empty = match (session, active) {
                    (Some(session), Some(active)) if active.id == pr.id => {
                        task_pr_empty(session, pr)
                    }
                    _ => None,
                };
                PrSnapshot::new(pr, empty)
            })
            .collect(),
        active_pr: active.map(|pr| pr.id.to_string()),
    })
}

fn task_pr_empty(session: &TaskSession, pr: &TaskPr) -> Option<bool> {
    if !session.worktree.exists() {
        return None;
    }
    let clean = crate::engine::git::is_clean(&session.worktree).ok()?;
    if !clean {
        return Some(false);
    }
    let head = crate::engine::git::rev_parse(&session.worktree, "HEAD").ok()?;
    Some(head == pr.base_commit)
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
    pr_phase: Option<PrPhase>,
    reason: &str,
) -> NextMove {
    let owner = if pr_phase == Some(PrPhase::Open) {
        NextMoveOwner::Review
    } else {
        match status {
            TaskSessionStatus::Created
            | TaskSessionStatus::Starting
            | TaskSessionStatus::Running => NextMoveOwner::Task,
            TaskSessionStatus::Waiting | TaskSessionStatus::Blocked | TaskSessionStatus::Failed => {
                NextMoveOwner::Project
            }
            TaskSessionStatus::Completed | TaskSessionStatus::Abandoned => NextMoveOwner::Project,
        }
    };
    NextMove {
        owner,
        reason: reason.to_string(),
    }
}

/// The invoking context's wave id: `LF_WAVE_ID`, else `None` (the caller
/// errors). Kept minimal — `lf status` with no arg is a convenience, not the
/// resolution surface `lf chat`/`lf radio sub` own.
fn ambient_wave() -> Option<String> {
    std::env::var(crate::engine::wave_context::WAVE_ID_ENV)
        .ok()
        .map(|value| value.trim().to_string())
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
        "{bold}{name:<16}  {status:<8}  {live:<5}  {tasks:>5}  {projects:>8}  {home:<16}  ENDPOINT{reset}",
        bold = colors.bold,
        reset = colors.reset,
        name = "WAVE",
        status = "STATUS",
        live = "LIVE",
        tasks = "TASKS",
        projects = "PROJECTS",
        home = "HOME",
    );
    for wave in snapshots {
        println!(
            "{name:<16}  {status:<8}  {live:<5}  {tasks:>5}  {projects:>8}  {home:<16}  {endpoint}",
            name = truncate(&wave.name, 16),
            status = wave.status,
            live = if wave.live { "yes" } else { "no" },
            tasks = wave.active_tasks,
            projects = wave.active_projects,
            home = truncate(&wave.home.address, 16),
            endpoint = wave.endpoint.as_deref().unwrap_or("-"),
        );
    }
}

fn home_state_label(state: HomeState) -> &'static str {
    match state {
        HomeState::Unreachable => "unreachable",
        HomeState::Stopped => "stopped",
        HomeState::Running => "running",
        HomeState::Unknown => "unknown",
    }
}

/// The single contextual action a surface should offer, rendered for the CLI.
fn home_action_label(action: &HomeActionDto) -> String {
    match action {
        HomeActionDto::Attach { endpoint } => format!("Attach ({endpoint})"),
        HomeActionDto::Start { home } => format!("Start on {home}"),
        HomeActionDto::Reason { message } => message.clone(),
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
        "  home      {}  [{}]",
        wave.home.address,
        home_state_label(status.home_runtime.state)
    );
    println!(
        "  action    {}",
        home_action_label(&status.home_runtime.action)
    );
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
    print_attention(&status.attention);
    print_runs(&status.runs);
}

fn print_attention(attention: &Evidence<AttentionItem>) {
    match attention {
        Evidence::Unavailable { reason } => println!("  attention unavailable: {reason}"),
        Evidence::Ok { items, .. } if items.is_empty() => println!("  attention  nothing waiting"),
        Evidence::Ok { items, .. } => {
            println!("  attention");
            for item in items {
                println!(
                    "    {subject:<14}  {owner:<8}  {age:>7}  {reason}",
                    subject = truncate(&item.subject, 14),
                    owner = owner_label(&item.owner),
                    age = item
                        .age_secs
                        .map(format_age)
                        .unwrap_or_else(|| "-".to_string()),
                    reason = item.reason,
                );
            }
        }
    }
}

fn print_runs(runs: &Evidence<RunLedgerEntry>) {
    match runs {
        Evidence::Unavailable { reason } => println!("  runs unavailable: {reason}"),
        Evidence::Ok { items, .. } if items.is_empty() => {
            println!("  runs       none in the ledger window")
        }
        Evidence::Ok { items, truncated } => {
            println!("  runs");
            for run in items {
                println!(
                    "    {label:<24}  {status:<8}  {age:>7} ago",
                    label = truncate(&run.label, 24),
                    status = run.status,
                    age = format_age(now().unix_timestamp() - run.started),
                );
            }
            if *truncated {
                println!("    (older runs beyond the window cap are not shown)");
            }
        }
    }
}

fn owner_label(owner: &NextMoveOwner) -> &'static str {
    match owner {
        NextMoveOwner::Human => "human",
        NextMoveOwner::Wave => "wave",
        NextMoveOwner::Project => "project",
        NextMoveOwner::Task => "task",
        NextMoveOwner::Review => "review",
        NextMoveOwner::Ci => "ci",
        NextMoveOwner::External => "external",
    }
}

fn format_age(secs: i64) -> String {
    let secs = secs.max(0);
    if secs >= 86_400 {
        format!("{}d", secs / 86_400)
    } else if secs >= 3600 {
        format!("{}h", secs / 3600)
    } else if secs >= 60 {
        format!("{}m", secs / 60)
    } else {
        format!("{secs}s")
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
    fn swift_fixture_preserves_active_pr_publication_disposition() {
        let fixture = include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../tests/fixtures/dto/wave_detail.json"
        ));
        let snapshot: WaveDetailSnapshot = serde_json::from_str(fixture).unwrap();
        let task = &snapshot.projects[0].tasks[0];
        let pr = &task.prs[0];

        assert_eq!(task.active_pr.as_deref(), Some(pr.id.as_str()));
        assert_eq!(pr.phase, PrPhase::Open);
        assert_eq!(
            pr.publication.as_ref().unwrap().after_merge,
            AfterMerge::CompleteTask
        );
        assert_eq!(
            pr.publication
                .as_ref()
                .unwrap()
                .github
                .as_ref()
                .unwrap()
                .number,
            912
        );
    }

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
            home: WaveHomeDto::from(
                &crate::engine::wave_home::WaveHome::parse("ssh://jack@mini-heart").unwrap(),
            ),
        };
        let value: serde_json::Value = serde_json::to_value(&snapshot).expect("serialize");
        assert_eq!(value["name"], "goals");
        assert_eq!(value["status"], "running");
        assert_eq!(value["live"], true);
        assert_eq!(value["endpoint"], "127.0.0.1:5678");
        assert_eq!(value["active_tasks"], 2);
        // A remote-home wave is distinguishable in the wire shape: the canonical
        // address plus structured owner/location.
        assert_eq!(value["home"]["address"], "ssh://jack@mini-heart");
        assert_eq!(value["home"]["owner"], "jack");
        assert_eq!(value["home"]["location"]["kind"], "ssh");
        assert_eq!(value["home"]["location"]["host"], "mini-heart");
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
                home: WaveHomeDto::from(
                    &crate::engine::wave_home::WaveHome::parse("jack@local").unwrap(),
                ),
            },
            loop_state: None,
            runs: Evidence::complete(Vec::new()),
            attention: Evidence::complete(Vec::new()),
            home_runtime: HomeRuntimeDto::new(
                &crate::engine::wave_home::WaveHome::parse("jack@local").unwrap(),
                HomeState::Stopped,
                "reachable (local); no resident is serving this Wave".into(),
                None,
            ),
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
                    prs: Vec::new(),
                    active_pr: None,
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
        // The promised evidence is present, and an empty reading says so
        // explicitly rather than going missing.
        assert_eq!(value["runs"]["state"], "ok");
        assert_eq!(value["runs"]["items"], serde_json::json!([]));
        assert_eq!(value["runs"]["truncated"], false);
        assert_eq!(value["attention"]["state"], "ok");
    }

    /// "We could not read the ledger" must never reach a client as "this wave
    /// has no runs".
    #[test]
    fn unavailable_evidence_is_a_state_not_an_empty_list() {
        let runs: Evidence<RunLedgerEntry> =
            Evidence::from_result(Err(anyhow!("run ledger unavailable: disk is gone")));
        let value = serde_json::to_value(&runs).expect("serialize");
        assert_eq!(value["state"], "unavailable");
        assert_eq!(value["reason"], "run ledger unavailable: disk is gone");
        assert!(value.get("items").is_none());
    }

    fn at(offset_secs: i64) -> String {
        format_time(now() - time::Duration::seconds(offset_secs)).expect("format")
    }

    fn project_detail(
        slug: &str,
        runtime: Option<ProjectRuntimeSnapshot>,
        next_move: NextMove,
        tasks: Vec<TaskDetailSnapshot>,
    ) -> ProjectDetailSnapshot {
        ProjectDetailSnapshot {
            project: PmProjectSummary {
                id: format!("{slug}-id"),
                slug: slug.to_string(),
                name: slug.to_string(),
                summary: String::new(),
                definition: String::new(),
                krs: Vec::new(),
            },
            runtime,
            directive: None,
            next_move,
            tasks,
        }
    }

    fn task_detail(
        identifier: &str,
        runtime: Option<TaskRuntimeSnapshot>,
        next_move: NextMove,
    ) -> TaskDetailSnapshot {
        TaskDetailSnapshot {
            task: PmTaskSummary {
                id: format!("{identifier}-id"),
                identifier: identifier.to_string(),
                name: identifier.to_string(),
                description: String::new(),
                rank: 1,
                completed: false,
                assignee: None,
            },
            runtime,
            directive: None,
            next_move,
            prs: Vec::new(),
            active_pr: None,
        }
    }

    fn task_runtime(
        status: TaskSessionStatus,
        reason: &str,
        status_at: String,
        process_alive: bool,
    ) -> TaskRuntimeSnapshot {
        TaskRuntimeSnapshot {
            session_id: format!("ts_{}", status.as_str()),
            project_session_id: "ps_1".to_string(),
            status,
            reason: reason.to_string(),
            status_at,
            worktree: "/repo".to_string(),
            branch: Some("b".to_string()),
            provider: "claude".to_string(),
            process_alive,
        }
    }

    #[test]
    fn attention_reports_work_waiting_on_someone_else_with_its_reason_and_age() {
        let projects = vec![project_detail(
            "auditability",
            None,
            NextMove {
                owner: NextMoveOwner::Wave,
                reason: "Project is ready to start".into(),
            },
            vec![
                task_detail(
                    "W2-133",
                    Some(task_runtime(
                        TaskSessionStatus::Running,
                        "pursuing the design",
                        at(60),
                        true,
                    )),
                    NextMove {
                        owner: NextMoveOwner::Task,
                        reason: "pursuing the design".into(),
                    },
                ),
                task_detail(
                    "W2-129",
                    Some(task_runtime(
                        TaskSessionStatus::Waiting,
                        "PR is open",
                        at(7200),
                        false,
                    )),
                    NextMove {
                        owner: NextMoveOwner::Review,
                        reason: "PR is open".into(),
                    },
                ),
            ],
        )];

        let items = attention(&projects, now(), Liveness::Observable);

        // The unstarted Project is a backlog row, not something waiting on you;
        // the running Task owns its own next move.
        assert_eq!(
            items
                .iter()
                .map(|item| item.subject.as_str())
                .collect::<Vec<_>>(),
            ["W2-129"]
        );
        assert_eq!(items[0].owner, NextMoveOwner::Review);
        assert_eq!(items[0].reason, "PR is open");
        assert!(items[0].age_secs.expect("age") >= 7200);
    }

    /// A Session claiming a live process the machine cannot find is exactly what
    /// an audit surface exists to show — not a running Task.
    #[test]
    fn a_session_whose_process_is_gone_needs_attention_and_says_so() {
        let projects = vec![project_detail(
            "auditability",
            None,
            NextMove {
                owner: NextMoveOwner::Wave,
                reason: "Project is ready to start".into(),
            },
            vec![task_detail(
                "W2-130",
                Some(task_runtime(
                    TaskSessionStatus::Running,
                    "implementing",
                    at(300),
                    false,
                )),
                NextMove {
                    owner: NextMoveOwner::Task,
                    reason: "implementing".into(),
                },
            )],
        )];

        let items = attention(&projects, now(), Liveness::Observable);

        assert_eq!(items.len(), 1);
        assert_eq!(items[0].owner, NextMoveOwner::Wave);
        assert_eq!(
            items[0].reason,
            "process is gone but the Session still records 'running'"
        );
        assert!(items[0].age_secs.expect("age") >= 300);
    }

    /// Without tmux the machine cannot look for the process, and "I could not
    /// look" must not be reported as "it is gone".
    #[test]
    fn a_machine_that_cannot_see_processes_reports_no_dead_process_findings() {
        let projects = vec![project_detail(
            "auditability",
            None,
            NextMove {
                owner: NextMoveOwner::Wave,
                reason: "Project is ready to start".into(),
            },
            vec![task_detail(
                "W2-130",
                Some(task_runtime(
                    TaskSessionStatus::Running,
                    "implementing",
                    at(300),
                    false,
                )),
                NextMove {
                    owner: NextMoveOwner::Task,
                    reason: "implementing".into(),
                },
            )],
        )];

        assert!(attention(&projects, now(), Liveness::Unknowable).is_empty());
    }

    #[test]
    fn a_completed_task_is_done_not_waiting() {
        let projects = vec![project_detail(
            "auditability",
            None,
            NextMove {
                owner: NextMoveOwner::Wave,
                reason: "Project is ready to start".into(),
            },
            vec![task_detail(
                "W2-100",
                Some(task_runtime(
                    TaskSessionStatus::Completed,
                    "merged",
                    at(10),
                    false,
                )),
                NextMove {
                    owner: NextMoveOwner::Project,
                    reason: "merged".into(),
                },
            )],
        )];

        assert!(attention(&projects, now(), Liveness::Observable).is_empty());
    }
}
