//! `lf wave list`, `lf wave status`, and `lf wave show` — read the wave registry (`store`).
//!
//! `lf wave list` lists durable Wave identities, authored goals, Task counts and
//! Machine placement. `lf wave status [wave]` adds current Projects, Task conditions,
//! metric readings and Session history; it never reads process health or a live
//! loop. With no argument it resolves the ambient Wave. Reads preserve missing
//! evidence; `--json` is the dashboard contract.
//!
//! Evidence the machine could not read stays [`Evidence::Unavailable`] — an
//! audit surface that renders "I could not look" as "nothing happened" is worse
//! than one that says nothing at all.

use std::collections::HashMap;
use std::io::IsTerminal;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering};

use anyhow::{anyhow, Result};
use serde::{Deserialize, Serialize};

use crate::durable::{Machine, TaskState, WorkRef, WorkStatus};
use crate::lf::commands::session::SessionFacts;
use crate::lf::commands::session_history::format_tokens;
use crate::lf::commands::task_row::{TaskDetail, TaskGraphRow, TaskNames, TaskRow, TaskThread};
use crate::lf::output::Colors;
use crate::ops::task_execution::{TaskExecutionSnapshot, TaskExecutionState};
use crate::session_record::SessionHistory;
use crate::store::{open_existing_store, SharedStore};
use crate::work::project::Project;
use crate::work::task::{
    CiObservation, CiState, PrMergeMode, PrMergeRequest, PrPhase, Task, TaskPr,
};
use crate::work::wave::metrics::{
    MetricContractIssueDto, MetricEvidenceDto, MetricFreshnessDto, MetricPortfolioDto,
    MetricReadingDto, MetricStage, MetricUnknownCauseDto, Threshold,
};
use crate::work::wave::Wave;

/// One wave's registry snapshot — the `lf wave list` row and the `wave` field of
/// `lf wave status`. Wire type consumed by Loopflow: every field is required or
/// explicitly Optional, no serde defaults.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WaveSnapshot {
    pub id: String,
    pub name: String,
    /// Current planning lifecycle derived from stable Work and concrete facts.
    pub status: WorkStatus,
    pub goal: String,
    /// Primary repo path.
    pub repo: String,
    /// Non-terminal Tasks owned by this Wave.
    pub active_tasks: u32,
    /// RFC3339 creation time, `null` when the row predates the column.
    pub created_at: Option<String>,
    /// Parent wave id in the chord tree, `null` for a root wave.
    pub parent_wave_id: Option<String>,
    /// Tombstone time for stable-id history; active locator reads exclude it.
    pub retired_at: Option<String>,
    pub superseded_by_wave_id: Option<String>,
    pub retirement_reason: Option<String>,
    /// Local execution placement and its route; absent for unplaced planning.
    pub machine: Option<Machine>,
}

/// `lf wave status <wave>`: current planning, Task conditions and Session history.
/// Wire type; no defaults.
/// `T` is the Task row: `TaskRow` for `lf wave status`, `RoadmapTask` for
/// `lf desktop wave`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WaveDetailSnapshot<T = RoadmapTask> {
    pub project_readiness: crate::store::sqlite::ProjectReadiness,
    pub wave: WaveSnapshot,
    pub projects: Evidence<ProjectSummary>,
    pub tasks: Evidence<T>,
    /// Wave-owned live evidence derived once by Rust for every consumer.
    pub metric_portfolio: MetricPortfolioDto,
    /// Durable Project Work that cannot join the current PM plan, including
    /// non-terminal Tasks stranded under a terminal historical Project.
    pub unavailable_tasks: Vec<UnavailableTaskEvidence>,
    /// This Wave's Machine-local Session history, newest first.
    pub history: Evidence<SessionHistory>,
    /// What the Wave's Tasks run, where, and which logins pay, with each source.
    pub setup: crate::work::wave::setup::WaveSetup,
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
    pub(crate) fn map<U>(self, row: impl FnMut(T) -> U) -> Evidence<U> {
        match self {
            Self::Ok { items, truncated } => Evidence::Ok {
                items: items.into_iter().map(row).collect(),
                truncated,
            },
            Self::Unavailable { reason } => Evidence::Unavailable { reason },
        }
    }

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

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TaskSummary {
    pub repository_id: String,
    pub repo: String,
    pub project_id: Option<String>,
    pub wave_id: Option<String>,
    pub acquisition: crate::store::TaskAcquisition,
    pub sync: crate::store::planning_sync::PlanningSyncStatus,
    pub id: String,
    pub linear_id: Option<String>,
    pub identifier: String,
    pub name: String,
    pub description: String,
    pub rank: u32,
    pub state: Option<String>,
    pub completed_at: Option<String>,
    pub due_date: Option<String>,
    pub assignee: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum NextMoveOwner {
    User,
    Wave,
    Task,
    Ci,
    External,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NextMove {
    pub owner: NextMoveOwner,
    pub reason: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TaskRuntimeSnapshot {
    pub work_id: String,
    /// Completion and abandonment are independent of Graph position.
    pub status: TaskState,
    /// A durable completion request awaiting settlement.
    pub completion_pending: Option<String>,
    pub reason: String,
    pub updated_at: String,
    pub provider: String,
    /// Who owns the Task on which Machine, as this Machine reads the ownership
    /// log; absent when the Task is unowned.
    pub owner: Option<crate::durable::TaskOwner>,
}

/// A Task's derived operating condition. Sessions own review actions; this state
/// only lets Work surfaces explain whether the Task is clear, waiting on
/// another actor, blocked, or unreadable.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TaskConditionState {
    Waiting,
    Blocked,
    Clear,
    Unknown,
}

use crate::durable::GraphProcessDetail;
pub use crate::ops::task_actions::{
    ci_failure_reason, derive_task_actions, TaskAction, TaskActionEvidence, TaskActionModel,
};
use crate::ops::task_run::TaskRunControl;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LocalProgressEvidenceState {
    Observed,
    Missing,
    NotApplicable,
    Unavailable,
}

/// The one definition of unsettled local Task progress. Every constituent is
/// explicit so a popover can explain the fold without running Git again.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LocalProgressEvidence {
    pub state: LocalProgressEvidenceState,
    pub unsettled: Option<bool>,
    pub dirty: Option<bool>,
    pub authored_commits: Option<bool>,
    pub recovery_required: Option<bool>,
    pub reason: Option<String>,
}

/// A Task's shared condition and the evidence that proves it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TaskConditionSnapshot {
    pub state: TaskConditionState,
    pub reason: String,
    /// RFC3339 time local workspace evidence was sampled.
    pub observed_at: String,
    /// Age of the durable Work evidence at that sample, if Work exists.
    pub evidence_age_secs: Option<i64>,
    pub local_progress: LocalProgressEvidence,
    /// Started work a finished plan has not settled: the Task is still ready,
    /// its Graph is not idle, or its checkout holds observed unsettled work.
    /// Keeps a terminal Task in the working set.
    pub unresolved_execution: bool,
}

/// Stable references for one Task, shared verbatim by `lf wave status` and
/// `lf wave show`. The issue URL is cached PM evidence. Workspace evidence comes
/// from the durable Task and outlives its execution and final PR.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TaskReferenceSnapshot {
    pub issue_url: Option<String>,
    pub workspace: Option<TaskWorktreeSnapshot>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TaskWorktreeSnapshot {
    pub machine_id: Option<crate::durable::MachineId>,
    pub machine_label: Option<String>,
    pub slug: String,
    pub branch: Option<String>,
    pub base_commit: String,
    pub worktree: String,
    /// Existence on the reading Machine; unknown when the filesystem check fails.
    pub local_exists: Option<bool>,
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
    pub presentation: Option<PrPresentationSnapshot>,
    pub github: Option<GithubPrSnapshot>,
    pub merge: Option<PrMergeRequestSnapshot>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PrPresentationSnapshot {
    pub title: String,
    pub body: String,
    pub head_sha: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PrMergeRequestSnapshot {
    pub mode: PrMergeMode,
    pub requested_at: String,
    pub head_sha: String,
}

impl From<&PrMergeRequest> for PrMergeRequestSnapshot {
    fn from(request: &PrMergeRequest) -> Self {
        Self {
            mode: request.mode,
            requested_at: format_time(request.requested_at)
                .expect("PR merge request timestamp formats as RFC 3339"),
            head_sha: request.head_sha.clone(),
        }
    }
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
                    presentation: publication.presentation.as_ref().map(|copy| {
                        PrPresentationSnapshot {
                            title: copy.title.clone(),
                            body: copy.body.clone(),
                            head_sha: copy.head_sha.clone(),
                        }
                    }),
                    github: publication.github.as_ref().map(|github| GithubPrSnapshot {
                        number: github.number,
                        url: github.url.clone(),
                    }),
                    merge: publication.merge.as_ref().map(PrMergeRequestSnapshot::from),
                }),
            merge_commit: pr.merge_commit.clone(),
            abandoned_at: pr.abandoned_at.and_then(format_time),
        }
    }
}

/// Non-terminal durable Task Work whose historical Project is no longer in the
/// current PM snapshot.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct UnavailableTaskEvidence {
    pub work_id: String,
    pub task_id: String,
    pub task_identifier: String,
    pub status: TaskState,
    pub owner: NextMoveOwner,
    pub reason: String,
    pub recovery: String,
}

/// Where a row sits in the plan, derived once so every surface buckets it
/// identically.
///
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RoadmapSection {
    /// This Work's own planner must move next.
    Now,
    /// Someone else must move: a Session, User, CI, or supervising Work.
    Waiting,
    /// Filed, not started, not complete — ready for someone to pick up.
    NotStarted,
    /// Done or dormant: terminal Work and completed plan rows.
    Later,
}

/// `lf wave show` — every Wave's plan joined to durable Work and local delivery
/// evidence, bucketed by planning section.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RoadmapSnapshot<T = RoadmapTask> {
    /// Every Task once; each names its Wave.
    pub tasks: Evidence<T>,
    /// RFC3339 time this read was taken.
    pub generated_at: String,
    pub waves: Vec<WaveRoadmap>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WaveRoadmap {
    /// The graph the Wave's Tasks take up by default.
    pub graph: Option<String>,
    pub project_readiness: crate::store::sqlite::ProjectReadiness,
    pub wave: WaveSnapshot,
    pub projects: Evidence<ProjectSummary>,
    pub metric_portfolio: MetricPortfolioDto,
    pub unavailable_tasks: Vec<UnavailableTaskEvidence>,
}

/// Who a read prints for.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Shape {
    /// Ordinary reads: one row per Task (`TaskRow`) or Session (`SessionRow`).
    Tasks,
    /// `lf desktop`: Desktop's presentation of each record.
    Desktop,
}

impl Shape {
    /// `value` as this audience reads it; `rows` turns it into Task rows.
    async fn print<T, U>(
        self,
        store: Option<&SharedStore>,
        value: T,
        rows: impl FnOnce(T, &TaskNames) -> U,
    ) -> Result<serde_json::Value>
    where
        T: Serialize,
        U: Serialize,
    {
        Ok(match self {
            Self::Desktop => serde_json::to_value(value)?,
            Self::Tasks => serde_json::to_value(rows(value, &TaskNames::read(store).await?))?,
        })
    }
}

impl RoadmapSnapshot {
    fn rows(self, names: &TaskNames) -> RoadmapSnapshot<TaskRow> {
        RoadmapSnapshot {
            tasks: self.tasks.map(|task| names.row(task)),
            generated_at: self.generated_at,
            waves: self.waves,
        }
    }

    /// The Tasks of one Wave, in plan order.
    pub fn wave_tasks<'a>(&'a self, wave: &WaveRoadmap) -> Evidence<&'a RoadmapTask> {
        match &self.tasks {
            Evidence::Ok { items, truncated } => Evidence::Ok {
                items: items
                    .iter()
                    .filter(|task| task.task.wave_id.as_deref() == Some(wave.wave.id.as_str()))
                    .collect(),
                truncated: *truncated,
            },
            Evidence::Unavailable { reason } => Evidence::Unavailable {
                reason: reason.clone(),
            },
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProjectSummary {
    pub sync: crate::store::planning_sync::PlanningSyncStatus,
    pub current: bool,
    pub id: String,
    pub linear_id: Option<String>,
    pub slug: String,
    pub name: String,
    pub status: crate::work::project::ProjectStatus,
    pub metric_targets: Vec<crate::work::wave::metrics::MetricTarget>,
    pub krs: Vec<crate::work::project::ProjectKR>,
}

async fn project_planning(store: &SharedStore, wave: &Wave) -> Evidence<ProjectSummary> {
    let result = async {
        let projects = store.list_projects(Some(wave.id())).await?;
        let current = crate::store::sqlite::project_selection::read_project_binding(
            &store.sqlite,
            wave.id(),
        )?;
        let projects = projects
            .into_iter()
            .map(|project| -> Result<ProjectSummary> {
                Ok(ProjectSummary {
                    sync: store.sqlite.project_planning_sync(&project.id)?,
                    current: current.as_deref().is_some_and(|id| {
                        id == project.id.as_str()
                            || project
                                .linear_id
                                .as_ref()
                                .is_some_and(|linear| linear.as_str() == id)
                    }),
                    linear_id: project.linear_id.map(|id| id.as_str().to_owned()),
                    id: project.id.to_string(),
                    slug: project.slug,
                    name: project.name,
                    status: project.status,
                    metric_targets: project.metric_targets,
                    krs: project.krs,
                })
            })
            .collect::<Result<Vec<_>>>()?;
        Ok((projects, false))
    }
    .await;
    Evidence::from_result(result)
}

fn print_projects(projects: &Evidence<ProjectSummary>) {
    match projects {
        Evidence::Unavailable { reason } => println!("  projects unavailable: {reason}"),
        Evidence::Ok { items, .. } => {
            for project in items {
                println!(
                    "  chapter   {} ({}) · {} · {}",
                    project.name,
                    project.id,
                    if project.current {
                        "current"
                    } else {
                        "historical"
                    },
                    project.status.as_str()
                );
                for line in project.sync.lines() {
                    println!("    {line}");
                }
                for target in &project.metric_targets {
                    use crate::work::wave::metrics::Threshold;
                    let comparison = match target.target {
                        Threshold::AtLeast { .. } => ">=",
                        Threshold::AtMost { .. } => "<=",
                    };
                    println!(
                        "    target {} {comparison} {}",
                        target.metric_id,
                        target.target.value()
                    );
                }
                for kr in &project.krs {
                    println!("  [{}] {}", if kr.holds { "x" } else { " " }, kr.text);
                }
            }
        }
    }
}

/// One saved Task, its local execution evidence, section and active PR.
/// Runtime availability is independent of whether the Task has started.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RoadmapTask {
    pub task: TaskSummary,
    pub reference: TaskReferenceSnapshot,
    pub runtime: Option<TaskRuntimeSnapshot>,
    pub next_move: NextMove,
    pub condition: TaskConditionSnapshot,
    pub actions: TaskActionModel,
    /// The graph the Task took up, else the one its first `lf task run` takes up.
    pub graph_name: Option<String>,
    /// The layer that chose it: `task`, `personal`, `wave`, `repo`, `global` or `default`.
    pub graph_source: Option<crate::work::wave::setup::SetupSource>,
    pub latest_graph_process: Option<GraphProcessDetail>,
    pub execution: Option<TaskExecutionSnapshot>,
    pub run_control: TaskRunControl,
    pub ship_control: TaskRunControl,
    pub shipping: Option<crate::ops::task::ship::ShippingSnapshot>,
    pub pr: Option<PrSnapshot>,
    pub section: RoadmapSection,
    /// The graph step the Task stands at; `None` while a run carries it.
    /// Carried to `TaskRow`, not on Desktop's wire.
    #[serde(skip)]
    pub(crate) step: Option<String>,
}

/// `lf wave list` — every durable Wave identity the registry knows.
/// Keep only Waves whose repository matches the current working directory,
/// collapsing worktrees to their main checkout. `all` (or a cwd outside any git
/// repo, where there is nothing to scope to) returns every Wave unchanged.
fn scope_waves_to_repo(waves: Vec<Wave>, directory: &Path, all: bool) -> Result<Vec<Wave>> {
    if all {
        return Ok(waves);
    }
    let Some(scope) = crate::repository::CanonicalRepo::containing(directory)? else {
        return Ok(waves);
    };
    Ok(waves
        .into_iter()
        .filter(|wave| scope.contains(Path::new(wave.repo())))
        .collect())
}

pub fn ls(json: bool, all: bool, current: bool) -> Result<()> {
    let rt = tokio::runtime::Runtime::new()?;
    rt.block_on(async {
        let Some(snapshots) = list(all, current).await? else {
            return no_registry(json, "[]");
        };
        if json {
            println!("{}", serde_json::to_string(&snapshots)?);
        } else {
            print_wave_table(&snapshots);
        }
        Ok(())
    })
}

/// `lf wave list`'s rows; `None` before this Machine has a registry.
pub async fn list(all: bool, current: bool) -> Result<Option<Vec<WaveSnapshot>>> {
    let Some(store) = open_existing_store().await.map(std::sync::Arc::new) else {
        return Ok(None);
    };
    Ok(Some(wave_snapshots(&store, all, current).await?))
}

/// The rows of `lf wave list`, in its order.
pub(crate) async fn wave_snapshots(
    store: &SharedStore,
    all: bool,
    current: bool,
) -> Result<Vec<WaveSnapshot>> {
    let waves = store
        .list_waves(None)
        .await
        .map_err(|err| anyhow!("failed to read wave registry: {err}"))?;
    let waves = scope_waves_to_repo(waves, &std::env::current_dir()?, all)?;
    let mut snapshots = Vec::with_capacity(waves.len());
    let mut checkouts = Checkouts::default();
    for wave in waves {
        let checkout = wave_repository(&wave, &mut checkouts)?;
        let snapshot = snapshot_wave(store, &wave, &checkout).await?;
        if !current || current_wave(&snapshot) {
            snapshots.push(snapshot);
        }
    }
    snapshots.sort_by(|a, b| a.repo.cmp(&b.repo).then(a.name.cmp(&b.name)));
    Ok(snapshots)
}

fn current_wave(wave: &WaveSnapshot) -> bool {
    wave.status != WorkStatus::Abandoned && wave.retired_at.is_none()
}

/// Publication is local; Session claims and receipts never enter planning sync.
pub async fn save_steer(
    store: &SharedStore,
    wave: &str,
    author: &str,
    message: &str,
) -> Result<crate::steer::Steer> {
    let wave = resolve_status_wave(store, Some(wave)).await?;
    Ok(store.sqlite.save_wave_steer(wave.id(), author, message)?)
}

/// `lf wave status [wave]` — one Wave's current plan, Tasks and Session evidence.
pub fn status(wave: Option<&str>, json: bool, shape: Shape) -> Result<()> {
    let rt = tokio::runtime::Runtime::new()?;
    rt.block_on(async {
        let Some(store) = open_existing_store().await.map(std::sync::Arc::new) else {
            return no_registry(json, "null");
        };
        let status = synced_status(&store, &std::env::current_dir()?, wave).await?;
        if json {
            let status = shape
                .print(Some(&store), status, WaveDetailSnapshot::rows)
                .await?;
            println!("{}", serde_json::to_string(&status)?);
        } else {
            print_status(&status);
        }
        Ok(())
    })
}

/// `lf wave status`' reading from `directory`, after syncing the Wave's
/// repository with its tracker.
async fn synced_status(
    store: &SharedStore,
    directory: &Path,
    wave: Option<&str>,
) -> Result<WaveDetailSnapshot> {
    let wave = resolve_wave_in(store, directory, wave).await?;
    crate::ops::linear_observe::sync_repository(store, wave.repo()).await;
    wave_detail(store, &wave).await
}

/// `lf --cwd <directory> desktop wave <wave>`: one Wave's plan; None
/// without a registry.
pub fn desktop_status(directory: &Path, wave: &str) -> Result<Option<WaveDetailSnapshot>> {
    tokio::runtime::Runtime::new()?.block_on(async {
        let Some(store) = open_existing_store().await.map(std::sync::Arc::new) else {
            return Ok(None);
        };
        Ok(Some(synced_status(&store, directory, Some(wave)).await?))
    })
}

/// `lf --cwd <directory> desktop done --wave <wave> [--after <after>]`:
/// one page of a Wave's closed Tasks.
pub fn desktop_closed_tasks(
    directory: &Path,
    wave: &str,
    after: Option<&str>,
) -> Result<ClosedTaskPageSnapshot> {
    let repo = crate::repo::working_directory_in(directory)?;
    tokio::runtime::Runtime::new()?.block_on(async {
        let store = crate::store::read_existing_registry()?.map(std::sync::Arc::new);
        closed_task_page(
            store.as_ref(),
            &repo,
            Some(wave),
            None,
            after,
            CLOSED_TASK_PAGE,
        )
        .await
    })
}

/// `lf desktop done`'s default `--limit`.
pub const CLOSED_TASK_PAGE: usize = 50;

/// `lf wave status --watch`: `lf wave status --json` now and each time it
/// changes. The watch reads the store; it does not observe the tracker.
pub fn watch_status(wave: Option<String>, shape: Shape) -> Result<()> {
    super::watch::run(
        (),
        super::watch::Watched::following_checkouts(super::watch::PLANNING_CLOCK),
        |store, ()| {
            let wave = wave.clone();
            async move {
                let store = super::watch::require_store(store)?;
                let wave = resolve_status_wave(&store, wave.as_deref()).await?;
                let status = wave_detail(&store, &wave).await?;
                shape
                    .print(Some(&store), status, WaveDetailSnapshot::rows)
                    .await
            }
        },
    )
}

/// One Wave's `lf wave status` reading.
pub(crate) async fn wave_detail(store: &SharedStore, wave: &Wave) -> Result<WaveDetailSnapshot> {
    let repository_waves = store
        .list_waves(Some(wave.repo()))
        .await
        .map_err(|err| anyhow!("failed to read repository Waves: {err}"))?;
    let mut checkouts = Checkouts::default();
    validate_linear_ownership(store, &repository_waves, &mut checkouts).await?;
    let checkout = wave_repository(wave, &mut checkouts)?;
    let snapshot = snapshot_wave(store, wave, &checkout).await?;
    let shared = SharedTaskReads::read(store).await?;
    let task_snapshots = wave_tasks(store, wave, true, None, &shared).await?;
    let metric_portfolio = crate::ops::metrics::wave_metric_portfolio(store, wave, now()).await?;
    Ok(WaveDetailSnapshot {
        project_readiness: store.sqlite.project_readiness(wave.id())?,
        history: Evidence::from_result(
            crate::lf::commands::session_history::collect_recent_history(
                &store.sqlite,
                crate::lf::commands::WorkFilter {
                    wave: Some(wave.slug()),
                    project: None,
                    task: None,
                },
            ),
        ),
        wave: snapshot,
        projects: project_planning(store, wave).await,
        tasks: task_snapshots.tasks,
        metric_portfolio,
        unavailable_tasks: task_snapshots.unavailable_tasks,
        setup: crate::work::wave::setup::read_wave_setup(store, wave, &checkout).await?,
    })
}

/// `lf wave show [wave]` — the machine-wide intent plane. Every Wave (or one, when
/// scoped) with its plan joined to live evidence and each row bucketed into a
/// section. Deterministic and local: one runtime observation for the whole
/// read and bounded Git probes for Task Work, after a fresh team check. `lf wave status`
/// answers "is it healthy"; this answers "what is being worked on and what
/// could be".
pub fn show(
    wave: Option<&str>,
    task: Option<&str>,
    json: bool,
    all: bool,
    shape: Shape,
) -> Result<()> {
    let directory = std::env::current_dir()?;
    if json {
        println!("{}", show_json(&directory, wave, task, all, shape)?);
        return Ok(());
    }
    let rt = tokio::runtime::Runtime::new()?;
    rt.block_on(async {
        let store = crate::store::read_existing_registry()?.map(std::sync::Arc::new);
        let roadmap = show_roadmap(store.as_ref(), &directory, wave, task, all, true).await?;
        print_roadmap(&roadmap);
        Ok(())
    })
}

/// What `lf wave show --json` prints in `directory`, after asking the
/// tracker about each repository it reads.
pub(crate) fn show_json(
    directory: &Path,
    wave: Option<&str>,
    task: Option<&str>,
    all: bool,
    shape: Shape,
) -> Result<String> {
    tokio::runtime::Runtime::new()?.block_on(async {
        let store = crate::store::read_existing_registry()?.map(std::sync::Arc::new);
        let roadmap = show_roadmap(store.as_ref(), directory, wave, task, all, true).await?;
        let roadmap = shape
            .print(store.as_ref(), roadmap, RoadmapSnapshot::rows)
            .await?;
        Ok(serde_json::to_string(&roadmap)?)
    })
}

/// `lf wave show --watch` in `directory`: `lf wave show --json` now and each
/// time it changes. The watch reads the store; it does not observe the tracker.
pub fn watch_show(
    directory: std::path::PathBuf,
    wave: Option<String>,
    task: Option<String>,
    all: bool,
    shape: Shape,
    transport: super::watch::Transport,
) -> Result<Option<super::watch::Subscription>> {
    super::watch::start(
        (),
        super::watch::Watched::following_checkouts(super::watch::PLANNING_CLOCK),
        move |store, ()| {
            let (wave, task, directory) = (wave.clone(), task.clone(), directory.clone());
            async move {
                let roadmap = show_roadmap(
                    store.as_ref(),
                    &directory,
                    wave.as_deref(),
                    task.as_deref(),
                    all,
                    false,
                )
                .await?;
                shape
                    .print(store.as_ref(), roadmap, RoadmapSnapshot::rows)
                    .await
            }
        },
        transport,
    )
}

/// What `lf wave show` prints. `observe` first asks the tracker about each
/// repository read.
async fn show_roadmap(
    store: Option<&SharedStore>,
    directory: &Path,
    wave: Option<&str>,
    task: Option<&str>,
    all: bool,
    observe: bool,
) -> Result<RoadmapSnapshot> {
    let include_history = wave.is_some() || task.is_some();
    let evaluation_time = now();
    let Some(store) = store else {
        if task.is_some() {
            anyhow::bail!("Task lookup unavailable: local registry could not be opened");
        }
        return Ok(RoadmapSnapshot {
            generated_at: format_time(evaluation_time)
                .expect("current timestamp formats as RFC 3339"),
            waves: Vec::new(),
            tasks: Evidence::complete(Vec::new()),
        });
    };
    // An explicit all-repositories query must not inherit the Wave of
    // the process that launched the GUI. An explicit --wave still wins.
    let env_wave_id = if all || task.is_some() {
        None
    } else {
        std::env::var(crate::work::wave::context::WAVE_ID_ENV).ok()
    };
    let repo = crate::repo::discover_repo_root(directory)
        .ok()
        .flatten()
        .or_else(|| (task.is_some() && !all).then(|| directory.to_owned()));
    let waves = match crate::work::wave::context::resolve_managed_wave(
        Some(&**store),
        repo.as_deref(),
        wave,
        env_wave_id.as_deref(),
    )
    .await
    {
        Ok(wave) => vec![wave],
        Err(crate::work::wave::context::WaveResolveError::NoContext) => {
            let waves = store
                .list_waves(None)
                .await
                .map_err(|err| anyhow!("failed to read wave registry: {err}"))?;
            if task.is_some() && !all {
                // Exact destinations carry a repository even when only its
                // cached registration survives, without Git metadata.
                let scope = crate::repository::CanonicalRepo::discover(directory)?;
                waves
                    .into_iter()
                    .filter(|wave| scope.contains(Path::new(wave.repo())))
                    .collect()
            } else {
                scope_waves_to_repo(waves, directory, all)?
            }
        }
        Err(other) => return Err(anyhow!(other)),
    };
    if observe {
        let mut checked = std::collections::HashSet::new();
        for wave in &waves {
            if checked.insert(wave.repo()) {
                crate::ops::linear_observe::sync_repository(store, wave.repo()).await;
            }
        }
    }
    roadmap_snapshot(
        store,
        waves,
        include_history,
        task,
        if all { None } else { repo.as_deref() },
        evaluation_time,
    )
    .await
}

/// What `lf task list` narrows the repository's open Tasks to; the flags of
/// the same name, which a `--watch` request can replace.
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TaskListArgs {
    /// Contained in the title or identifier, ignoring case; absent matches every Task.
    pub query: Option<String>,
    pub wave: Option<String>,
    /// Project ID, provider ID, slug or name.
    pub project: Option<String>,
    /// An open `TaskState` label: deferred, unstarted, idle, active, moving or shipping.
    pub state: Option<String>,
}

/// `lf task list`: the repository's open Tasks matching `query`, as the plan
/// gives them.
pub fn list_tasks(repo: &Path, query: &TaskListArgs, json: bool) -> Result<()> {
    let runtime = tokio::runtime::Runtime::new()?;
    runtime.block_on(async {
        let store = crate::store::read_existing_registry()?.map(std::sync::Arc::new);
        let tasks = match &store {
            Some(store) => open_tasks(store, repo, query).await?,
            None => Vec::new(),
        };
        if json {
            let names = TaskNames::read(store.as_ref()).await?;
            let rows: Vec<TaskRow> = tasks.into_iter().map(|task| names.row(task)).collect();
            println!("{}", serde_json::to_string(&rows)?);
        } else {
            for task in tasks {
                println!(
                    "{} · {} · {}",
                    task.task.identifier,
                    owner_text(task.runtime.as_ref()),
                    task.task.name
                );
            }
        }
        Ok(())
    })
}

/// `lf task list --watch`: `lf task list --json` now and each time it changes.
pub fn watch_tasks(repo: &Path, args: TaskListArgs) -> Result<()> {
    super::watch::run(
        args,
        super::watch::Watched::following_checkouts(super::watch::PLANNING_CLOCK),
        |store, args| {
            let repo = repo.to_owned();
            async move {
                let Some(store) = store else {
                    return Ok(Vec::new());
                };
                let tasks = open_tasks(&store, &repo, &args).await?;
                let names = TaskNames::read(Some(&store)).await?;
                Ok(tasks.into_iter().map(|task| names.row(task)).collect())
            }
        },
    )
}

/// What a `lf task status --watch` request can select instead.
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
struct TaskStatusArgs {
    issue: String,
}

/// `lf task status <issue> --watch`: `lf task status --json` now and each time
/// it changes. Only the first reading exchanges planning with the tracker.
pub fn watch_task_status(
    repo: &Path,
    issue: String,
    shape: Shape,
    transport: super::watch::Transport,
) -> Result<Option<super::watch::Subscription>> {
    let repo = repo.to_owned();
    super::watch::start(
        TaskStatusArgs { issue },
        super::watch::Watched::every(super::watch::ACTIVITY_CLOCK),
        move |store, args| {
            let repo = repo.to_owned();
            async move {
                let store = super::watch::require_store(store)?;
                let status = crate::ops::task::read_task_status(&store, &repo, &args.issue).await?;
                Ok(match shape {
                    Shape::Desktop => serde_json::to_value(task_pane(&store, status).await?)?,
                    Shape::Tasks => serde_json::to_value(task_detail(&store, status).await?)?,
                })
            }
        },
        transport,
    )
}

impl WaveDetailSnapshot {
    fn rows(self, names: &TaskNames) -> WaveDetailSnapshot<TaskRow> {
        WaveDetailSnapshot {
            project_readiness: self.project_readiness,
            wave: self.wave,
            projects: self.projects,
            tasks: self.tasks.map(|task| names.row(task)),
            metric_portfolio: self.metric_portfolio,
            unavailable_tasks: self.unavailable_tasks,
            history: self.history,
            setup: self.setup,
        }
    }
}

impl ClosedTaskPageSnapshot {
    fn rows(self, names: &TaskNames) -> ClosedTaskPageSnapshot<TaskRow> {
        ClosedTaskPageSnapshot {
            tasks: self.tasks.into_iter().map(|task| names.row(task)).collect(),
            next: self.next,
        }
    }
}

/// `lf desktop task`: `lf task status`'s reading plus the Task's Desktop row,
/// which keeps the pane current after the Task leaves the plan.
#[derive(Debug, Serialize, Deserialize)]
pub struct DesktopTaskPane {
    #[serde(flatten)]
    pub status: crate::ops::task::TaskStatus,
    pub row: Option<RoadmapTask>,
}

pub(crate) async fn task_pane(
    store: &SharedStore,
    status: crate::ops::task::TaskStatus,
) -> Result<DesktopTaskPane> {
    let row = match &status.task {
        Some(task) => {
            let shared = SharedTaskReads::read(store).await?;
            snapshot_task_details(store, vec![task], false, &shared)?.pop()
        }
        None => None,
    };
    Ok(DesktopTaskPane { status, row })
}

/// `lf task status --json`.
pub fn print_task_detail(status: crate::ops::task::TaskStatus) -> Result<()> {
    let rt = tokio::runtime::Runtime::new()?;
    rt.block_on(async {
        let store = crate::store::read_existing_registry()?
            .map(std::sync::Arc::new)
            .ok_or_else(|| anyhow!("no Loopflow store exists on this Machine"))?;
        let detail = task_detail(&store, status).await?;
        println!("{}", serde_json::to_string_pretty(&detail)?);
        Ok(())
    })
}

/// `lf task status`'s reading of one registered Task.
pub(crate) async fn task_detail(
    store: &SharedStore,
    status: crate::ops::task::TaskStatus,
) -> Result<TaskDetail> {
    let task = status
        .task
        .ok_or_else(|| anyhow!("this Task is not registered in Loopflow"))?;
    let shared = SharedTaskReads::read(store).await?;
    let row = snapshot_task_details(store, vec![&task], false, &shared)?
        .pop()
        .expect("a registered Task has a row");
    let mut sessions = Vec::new();
    for session in status
        .overview
        .map(|overview| overview.sessions)
        .unwrap_or_default()
    {
        if let Some(record) = crate::ops::human_session::listed(store, session.id.as_str())? {
            sessions.push(SessionFacts::of(record));
        }
    }
    Ok(TaskDetail {
        task: TaskNames::read(Some(store)).await?.row(row),
        sessions,
        graph_processes: status
            .graph_processes
            .into_iter()
            .map(TaskGraphRow::of)
            .collect(),
        comments: status.comments.map(TaskThread::from),
    })
}

/// The open Tasks `query` selects, read from the store's filter rather than
/// from a whole plan, in plan order.
async fn open_tasks(
    store: &SharedStore,
    repo: &Path,
    query: &TaskListArgs,
) -> Result<Vec<RoadmapTask>> {
    let Some(scope) = repository_scope(store, repo, query.wave.as_deref()).await? else {
        return Ok(Vec::new());
    };
    let tasks = store.sqlite.open_tasks_matching(
        &scope.id,
        query.query.as_deref().unwrap_or_default(),
        scope.wave.as_ref().map(Wave::id),
        query.project.as_deref(),
    )?;
    let shared = SharedTaskReads::read(store).await?;
    let (details, _) = snapshot_tasks(
        store,
        store.list_projects(None).await?,
        tasks,
        false,
        false,
        &shared,
    )
    .await?;
    Ok(details
        .into_iter()
        .filter(|task| {
            query
                .state
                .as_deref()
                .is_none_or(|state| task_state(task).label() == state)
        })
        .collect())
}

/// The repository `lf task list` reads, and the Wave it narrows to.
struct RepositoryScope {
    repo: String,
    id: crate::durable::RepositoryId,
    wave: Option<Wave>,
}

/// `None` when the store has never seen the repository.
async fn repository_scope(
    store: &SharedStore,
    repo: &Path,
    wave: Option<&str>,
) -> Result<Option<RepositoryScope>> {
    let canonical = crate::repository::CanonicalRepo::discover(repo)?.to_string();
    let Some(id) = store.sqlite.repository_id(&canonical)? else {
        return Ok(None);
    };
    let wave = match wave {
        Some(name) => Some(
            crate::work::wave::context::resolve_managed_wave(
                Some(store),
                Some(repo),
                Some(name),
                None,
            )
            .await?,
        ),
        None => None,
    };
    Ok(Some(RepositoryScope {
        repo: canonical,
        id,
        wave,
    }))
}

/// `lf task list --done --json`: one page of closed Tasks.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClosedTaskPageSnapshot<T = RoadmapTask> {
    pub tasks: Vec<T>,
    /// Pass as `--after` for the next page; absent on the last page.
    pub next: Option<String>,
}

/// `lf task list --done`: closed Tasks of the repository (or one Wave), most
/// recently closed first, with the details the plan gives open Tasks.
pub fn list_closed_tasks(
    repo: &Path,
    wave: Option<&str>,
    since: Option<&str>,
    after: Option<&str>,
    limit: usize,
    json: bool,
    shape: Shape,
) -> Result<()> {
    let runtime = tokio::runtime::Runtime::new()?;
    runtime.block_on(async {
        let store = crate::store::read_existing_registry()?.map(std::sync::Arc::new);
        let page = closed_task_page(store.as_ref(), repo, wave, since, after, limit).await?;
        if json {
            let page = shape
                .print(store.as_ref(), page, ClosedTaskPageSnapshot::rows)
                .await?;
            println!("{}", serde_json::to_string(&page)?);
        } else {
            for task in &page.tasks {
                println!(
                    "{} · {} · {}",
                    task.task.identifier,
                    task.task.completed_at.as_deref().unwrap_or("-"),
                    task.task.name
                );
            }
            if let Some(next) = &page.next {
                println!("more: lf task list --done --after {next} (keep the same filters)");
            }
        }
        Ok(())
    })
}

/// One page of closed Tasks in `repo`; none without a registry.
async fn closed_task_page(
    store: Option<&SharedStore>,
    repo: &Path,
    wave: Option<&str>,
    since: Option<&str>,
    after: Option<&str>,
    limit: usize,
) -> Result<ClosedTaskPageSnapshot> {
    let empty = || ClosedTaskPageSnapshot {
        tasks: Vec::new(),
        next: None,
    };
    let Some(store) = store else {
        return Ok(empty());
    };
    let since = since
        .map(|since| -> Result<String> {
            let since = super::util::parse_since(since, now())?
                .to_offset(time::UtcOffset::UTC)
                .replace_nanosecond(0)
                .expect("zero nanoseconds are valid");
            Ok(format_time(since).expect("a UTC timestamp formats as RFC 3339"))
        })
        .transpose()?;
    let Some(scope) = repository_scope(store, repo, wave).await? else {
        return Ok(empty());
    };
    let after = after
        .map(|selector| {
            store
                .sqlite
                .resolve_task_id(selector, Some(&scope.repo))?
                .ok_or_else(|| anyhow!("no Task {selector} to continue after"))
        })
        .transpose()?;
    let page = store.sqlite.closed_tasks_page(
        &scope.id,
        scope.wave.as_ref().map(Wave::id),
        since.as_deref(),
        after.as_ref(),
        limit,
    )?;
    let shared = SharedTaskReads::read(store).await?;
    // Details keep the page's closing order; the plan's rank order does not apply.
    let details = snapshot_task_details(store, page.tasks.iter().collect(), false, &shared)?;
    Ok(ClosedTaskPageSnapshot {
        tasks: details,
        next: page.next.map(|id| id.to_string()),
    })
}

async fn roadmap_snapshot(
    store: &SharedStore,
    waves: Vec<Wave>,
    include_history: bool,
    task: Option<&str>,
    repo_scope: Option<&Path>,
    evaluation_time: time::OffsetDateTime,
) -> Result<RoadmapSnapshot> {
    // A Wave filter narrows presentation, not repository ownership checks.
    let ownership_waves = if waves.len() == 1 {
        store
            .list_waves(Some(waves[0].repo()))
            .await
            .map_err(|err| anyhow!("failed to read repository Waves: {err}"))?
    } else {
        waves.clone()
    };
    let mut checkouts = Checkouts::default();
    validate_linear_ownership(store, &ownership_waves, &mut checkouts).await?;
    let shared = SharedTaskReads::read(store).await?;
    // The plan carries open Tasks; closed ones are paged by `lf task list --done`.
    // An exact Task lookup still finds a closed Task.
    let mut tasks = match task {
        Some(_) => store.list_tasks(None).await?,
        None => store.list_open_tasks(None).await?,
    };
    if let Some(repo) = repo_scope {
        let repo = crate::repository::CanonicalRepo::discover(repo)?;
        let repository = store.sqlite.repository_id(&repo.to_string())?;
        tasks.retain(|task| Some(&task.repository_id) == repository.as_ref());
    }
    if let Some(selector) = task {
        // Identifiers may repeat across repositories. Resolve within each owner,
        // including cached repositories whose Git checkout is unavailable.
        let mut selected = std::collections::HashSet::new();
        let repositories: std::collections::HashSet<_> =
            tasks.iter().map(|task| &task.repository_id).collect();
        for repository in repositories {
            let path = store
                .sqlite
                .repository_path(repository)?
                .ok_or_else(|| anyhow!("Task repository {repository} has no local path"))?;
            if let Some(id) = store.sqlite.resolve_task_id(selector, Some(&path))? {
                selected.insert(id);
            }
        }
        tasks.retain(|task| selected.contains(&task.id));
    }
    let inventory = snapshot_tasks(
        store,
        store.list_projects(None).await?,
        tasks,
        false,
        true,
        &shared,
    )
    .await;
    let tasks: Evidence<RoadmapTask> =
        Evidence::from_result(inventory.map(|(tasks, _)| (tasks, false)));
    let mut roadmaps = Vec::with_capacity(waves.len());
    for wave in &waves {
        // Repository scoping can repair an alias in the registry. Use that
        // refreshed address for Task selection rather than the pre-repair copy.
        let wave = ownership_waves
            .iter()
            .find(|current| current.id() == wave.id())
            .unwrap_or(wave);
        let checkout = wave_repository(wave, &mut checkouts)?;
        let snapshot = snapshot_wave(store, wave, &checkout).await?;
        if !include_history && !current_wave(&snapshot) {
            continue;
        }
        if task.is_some()
            && matches!(&tasks, Evidence::Ok { items, .. }
                if !items.iter().any(|task| task.task.wave_id.as_deref() == Some(wave.id().as_str())))
        {
            continue;
        }
        roadmaps.push(WaveRoadmap {
            graph: crate::work::wave::config::wave_graph(&checkout, wave.slug())?,
            project_readiness: store.sqlite.project_readiness(wave.id())?,
            wave: snapshot,
            projects: project_planning(store, wave).await,
            unavailable_tasks: Vec::new(),
            metric_portfolio: crate::ops::metrics::wave_metric_portfolio(
                store,
                wave,
                evaluation_time,
            )
            .await?,
        });
    }
    roadmaps.sort_by(|a, b| a.wave.name.cmp(&b.wave.name));
    Ok(RoadmapSnapshot {
        generated_at: format_time(evaluation_time).expect("current timestamp formats as RFC 3339"),
        waves: roadmaps,
        tasks,
    })
}

/// What a reading asks once and every Task's detail then shares, so the cost
/// of a plan does not grow by a statement per Task.
#[derive(Debug)]
struct SharedTaskReads {
    checkouts: Vec<crate::store::sqlite::TaskCheckout>,
    machines: Vec<crate::durable::Machine>,
    local_machine: crate::durable::MachineId,
}

impl SharedTaskReads {
    async fn read(store: &SharedStore) -> Result<Self> {
        let checkouts = store.task_checkouts().await?;
        let local_machine = store.local_machine().await?.id;
        ask_checkouts_ahead(&checkouts, &local_machine);
        Ok(Self {
            checkouts,
            machines: store.machines().await?,
            local_machine,
        })
    }
}

/// Git is asked about each existing checkout in turn by the Task details. A
/// process that retains answers asks about several checkouts at once first,
/// so the details find them waiting; any other process would only ask twice.
fn ask_checkouts_ahead(
    checkouts: &[crate::store::sqlite::TaskCheckout],
    local_machine: &crate::durable::MachineId,
) {
    const AT_ONCE: usize = 8;
    if !crate::git::retains_reads() {
        return;
    }
    let existing = checkouts
        .iter()
        .filter(|checkout| checkout.machine_id.as_ref() == Some(local_machine))
        .map(|checkout| checkout.worktree.as_path())
        .filter(|worktree| worktree.is_dir())
        .collect::<Vec<_>>();
    let next = std::sync::atomic::AtomicUsize::new(0);
    std::thread::scope(|scope| {
        for _ in 0..AT_ONCE.min(existing.len()) {
            scope.spawn(|| {
                while let Some(worktree) =
                    existing.get(next.fetch_add(1, std::sync::atomic::Ordering::Relaxed))
                {
                    // The answers are kept by Git's reader; failures are the details' to report.
                    let _ = crate::git::is_clean(worktree);
                    let _ = crate::git::rev_parse(worktree, "HEAD");
                    let _ = crate::git::worktree_root(worktree);
                    let _ = crate::git::worktrees::git_common_dir(worktree);
                }
            });
        }
    });
}

#[derive(Debug)]
struct WaveTasks {
    tasks: Evidence<RoadmapTask>,
    unavailable_tasks: Vec<UnavailableTaskEvidence>,
}

/// Both views retain durable Tasks when the current plan cannot be read.
async fn wave_tasks(
    store: &SharedStore,
    wave: &Wave,
    probe_pr_empty: bool,
    identifier: Option<&str>,
    shared: &SharedTaskReads,
) -> Result<WaveTasks> {
    let projects = store.list_projects(Some(wave.id())).await?;
    let mut tasks = match identifier {
        Some(_) => store.list_tasks(Some(wave.id())).await?,
        None => store.list_open_tasks(Some(wave.id())).await?,
    };
    // Selection does not discard other Projects' backlog or ongoing work.
    if let Some(identifier) = identifier {
        let selected = store
            .sqlite
            .resolve_task_id(identifier, Some(wave.repo()))?;
        tasks.retain(|task| selected.as_ref() == Some(&task.id));
    }
    let (details, unavailable_tasks) = snapshot_tasks(
        store,
        projects,
        tasks,
        probe_pr_empty,
        identifier.is_some(),
        shared,
    )
    .await?;
    Ok(WaveTasks {
        tasks: Evidence::complete(details),
        unavailable_tasks,
    })
}

/// A Task's one state; a Task with no recorded work has not started.
pub(crate) fn task_state(task: &RoadmapTask) -> TaskState {
    task.runtime
        .as_ref()
        .map_or(TaskState::Unstarted, |runtime| runtime.status)
}

/// A Task's section, from the same planning primitives the row already carries.
fn task_section(
    state: Option<&str>,
    runtime: Option<&TaskRuntimeSnapshot>,
    next_move: &NextMove,
) -> RoadmapSection {
    let Some(runtime) = runtime else {
        return if crate::linear::terminal_reason(state).is_some() {
            RoadmapSection::Later
        } else {
            RoadmapSection::NotStarted
        };
    };
    // Linear completing a Task that never left `start` withdraws it from what
    // is offered; an active one keeps its place and shows the conflict.
    let withdrawn = matches!(runtime.status, TaskState::Unstarted)
        && crate::linear::terminal_reason(state).is_some();
    if runtime.status.is_terminal() || runtime.status == TaskState::Deferred || withdrawn {
        return RoadmapSection::Later;
    }
    match next_move.owner {
        NextMoveOwner::Task => RoadmapSection::Now,
        _ => RoadmapSection::Waiting,
    }
}

/// The wave `lf wave status` is about: the name the caller typed, else the wave this
/// process is running inside.
async fn resolve_status_wave(store: &SharedStore, requested: Option<&str>) -> Result<Wave> {
    resolve_wave_in(store, &std::env::current_dir()?, requested).await
}

/// The Wave `requested` names, as a command run in `directory` resolves it.
async fn resolve_wave_in(
    store: &SharedStore,
    directory: &Path,
    requested: Option<&str>,
) -> Result<Wave> {
    // One shared rule for `--wave` and ambient `LF_WAVE_ID`: durable UUID or
    // repository-scoped registered name. Status consumes the resolved row
    // directly, so no second lookup can cross repositories.
    let repo = crate::repo::discover_repo_root(directory)?;
    crate::work::wave::context::resolve_managed_wave(
        Some(&**store),
        repo.as_deref(),
        requested,
        ambient_wave().as_deref(),
    )
    .await
    .map_err(|err| anyhow!("{err}"))
}

fn age_secs(since: &str, now: time::OffsetDateTime) -> Option<i64> {
    let since =
        time::OffsetDateTime::parse(since, &time::format_description::well_known::Rfc3339).ok()?;
    Some((now - since).whole_seconds().max(0))
}

fn now() -> time::OffsetDateTime {
    time::OffsetDateTime::now_utc()
}

// Resolve each recorded repository once per read. Sharing across validation and
// display avoids one Git process per Wave without retaining facts across reads.
/// Checkouts resolved during one reading: each repository address once, and
/// the current directory's checkout once, however many Waves share them.
#[derive(Debug, Default)]
struct Checkouts {
    current: Option<Option<(PathBuf, crate::repository::CanonicalRepo)>>,
    repositories: HashMap<String, PathBuf>,
}

fn wave_repository(wave: &Wave, checkouts: &mut Checkouts) -> Result<PathBuf> {
    if let Some(checkout) = checkouts.repositories.get(wave.repo()) {
        return Ok(checkout.clone());
    }
    let checkout = wave_checkout(wave, checkouts)?;
    checkouts
        .repositories
        .insert(wave.repo().to_string(), checkout.clone());
    Ok(checkout)
}

// A registry address identifies the repository, not the checkout's document bytes.
fn wave_checkout(wave: &Wave, checkouts: &mut Checkouts) -> Result<PathBuf> {
    let Ok(main) = crate::git::worktrees::main_repo_root(Path::new(wave.repo())) else {
        return Ok(PathBuf::from(wave.repo()));
    };
    if checkouts.current.is_none() {
        let current = match crate::repo::discover_repo_root(&std::env::current_dir()?)? {
            Some(checkout) => {
                let repo = crate::repository::CanonicalRepo::discover(&checkout)?;
                Some((checkout, repo))
            }
            None => None,
        };
        checkouts.current = Some(current);
    }
    if let Some(Some((checkout, repo))) = &checkouts.current {
        if main.canonicalize().is_ok_and(|main| main == repo.as_path()) {
            return Ok(checkout.clone());
        }
    }
    Ok(main)
}

/// Read planning and your Machine for the Wave without allocating execution.
async fn snapshot_wave(store: &SharedStore, wave: &Wave, checkout: &Path) -> Result<WaveSnapshot> {
    let repo = wave.repo().to_string();
    let active_tasks = store
        .sqlite
        .open_task_count(wave.id())
        .map_err(|err| anyhow!("failed to count active Tasks: {err}"))?;
    // Only a chosen Machine for the Wave's Tasks is named; unset runs on the reader.
    let (machine_id, source) = store
        .wave_owning_machine(wave.id(), crate::durable::MachineSetting::Tasks)
        .await
        .map_err(|error| anyhow!("failed to read your Wave Machine: {error}"))?;
    let machine = match source {
        crate::durable::OwningMachineSource::ThisMachine => None,
        _ => store
            .machine_by_id(&machine_id)
            .await
            .map_err(|error| anyhow!("failed to read Wave Machine: {error}"))?,
    };
    let status = store
        .work_status(&WorkRef::Wave(wave.id().clone()))
        .await
        .map_err(|error| anyhow!("failed to read Wave Work status: {error}"))?;
    Ok(WaveSnapshot {
        id: wave.id().to_string(),
        name: wave.slug().to_string(),
        status,
        goal: crate::work::wave::config::read_wave_summary(checkout, wave.slug())?,
        repo,
        active_tasks,
        created_at: wave.created_at().and_then(format_time),
        parent_wave_id: wave.parent_wave_id().map(ToString::to_string),
        retired_at: wave.retired_at().and_then(format_time),
        superseded_by_wave_id: wave.superseded_by_wave_id().map(ToString::to_string),
        retirement_reason: wave.retirement_reason().map(str::to_string),
        machine,
    })
}

fn snapshot_task_runtime(
    execution: &crate::ops::task_execution::TaskExecutionSnapshot,
    task: &Task,
    status: TaskState,
    completion_pending: Option<String>,
    owner: Option<crate::durable::TaskOwner>,
    provider: String,
) -> TaskRuntimeSnapshot {
    TaskRuntimeSnapshot {
        work_id: task.id.to_string(),
        reason: if status.is_terminal() {
            status.label().to_string()
        } else {
            execution.reason.clone()
        },
        status,
        completion_pending,
        updated_at: format_time(task.updated_at).unwrap_or_default(),
        provider,
        owner,
    }
}

async fn validate_linear_ownership(
    store: &SharedStore,
    waves: &[Wave],
    checkouts: &mut Checkouts,
) -> Result<()> {
    let mut ownership = HashMap::<PathBuf, Vec<_>>::new();
    for wave in waves {
        let repo = wave_repository(wave, checkouts)?;
        let repo = std::fs::canonicalize(&repo).unwrap_or(repo);
        ownership
            .entry(repo)
            .or_default()
            .push((wave.id(), wave.slug()));
    }
    for (repo, waves) in ownership {
        let expected_team = crate::ops::linear::selected_linear_team(&repo)
            .map_err(|error| crate::store::StoreError::InvalidData(error.to_string()));
        store
            .sqlite
            .validate_linear_ownership(&waves, expected_team)?;
    }
    Ok(())
}

async fn snapshot_tasks(
    store: &SharedStore,
    projects: Vec<Project>,
    tasks: Vec<Task>,
    probe_pr_empty: bool,
    include_retained: bool,
    shared: &SharedTaskReads,
) -> Result<(Vec<RoadmapTask>, Vec<UnavailableTaskEvidence>)> {
    let mut requests = Vec::new();
    let mut unavailable_tasks = Vec::new();
    for task in &tasks {
        if task
            .project_id
            .as_ref()
            .is_some_and(|id| !projects.iter().any(|p| &p.id == id))
        {
            let status = store.task_state(&task.id).await?;
            if include_retained || !status.is_terminal() {
                unavailable_tasks.push(unavailable_task(task, status));
            }
            continue;
        }
        requests.push(task);
    }
    let mut details = snapshot_task_details(store, requests, probe_pr_empty, shared)?;
    details.sort_by(|left, right| {
        let done = |task: &TaskSummary| task.state.as_deref() == Some("completed");
        done(&left.task)
            .cmp(&done(&right.task))
            .then(left.task.rank.cmp(&right.task.rank))
            .then(left.task.identifier.cmp(&right.task.identifier))
    });
    unavailable_tasks.sort_by(|left, right| {
        left.task_identifier
            .cmp(&right.task_identifier)
            .then(left.work_id.cmp(&right.work_id))
    });
    Ok((details, unavailable_tasks))
}

/// Each detail waits on fresh Git observations of its own checkout. Gather
/// them side by side: one after another, those children are most of a
/// roadmap read. Details and the first error keep their request order.
fn snapshot_task_details(
    store: &SharedStore,
    requests: Vec<&Task>,
    probe_pr_empty: bool,
    shared: &SharedTaskReads,
) -> Result<Vec<RoadmapTask>> {
    // `git status` is parallel itself: past four, overlapping children cost
    // more CPU than the wall time they save.
    const MAX_WORKERS: usize = 4;
    let reads = DetailReads::read(store, &requests, shared)?;
    let reads = &reads;
    let runtime = tokio::runtime::Handle::current();
    let workers = std::thread::available_parallelism()
        .map_or(1, std::num::NonZero::get)
        .min(MAX_WORKERS)
        .min(requests.len());
    let next = AtomicUsize::new(0);
    let mut details: Vec<_> = std::thread::scope(|scope| {
        let handles: Vec<_> = (0..workers)
            .map(|_| {
                scope.spawn(|| {
                    let mut details = Vec::new();
                    loop {
                        let index = next.fetch_add(1, Ordering::Relaxed);
                        let Some(task) = requests.get(index) else {
                            return details;
                        };
                        let detail = runtime.block_on(snapshot_task_detail(
                            store,
                            task,
                            probe_pr_empty,
                            reads,
                        ));
                        details.push((index, detail));
                    }
                })
            })
            .collect();
        handles
            .into_iter()
            .flat_map(|handle| handle.join().expect("Task detail worker panicked"))
            .collect()
    });
    details.sort_by_key(|(index, _)| *index);
    details.into_iter().map(|(_, detail)| detail).collect()
}

/// Everything the Task details of one reading share: the reading's own
/// reads, the durable facts of exactly these Tasks, and each Task's Workflow.
struct DetailReads<'a> {
    shared: &'a SharedTaskReads,
    facts: crate::store::sqlite::TaskDetailFacts,
    /// Each Task's own Workflow, else its Wave's, repository's or the builtin
    /// default, with that source; a default is resolved once per Wave and repository.
    selections: HashMap<crate::durable::TaskId, (String, crate::work::wave::setup::SetupSource)>,
    /// Each Task Wave's name, which selects its agent; read once per Wave.
    wave_names: HashMap<crate::id::WaveId, String>,
    /// Each Task Wave's and repository's checkout with your `lf set` values,
    /// which select its agent; read once per Wave and repository.
    personal: HashMap<
        (Option<crate::id::WaveId>, crate::durable::RepositoryId),
        (std::path::PathBuf, crate::config::PersonalSettings),
    >,
    /// Each handed-off Task's Auto-Ship progress.
    shipping: HashMap<crate::durable::TaskId, crate::ops::task::ship::ShippingSnapshot>,
    /// Each Task's latest Graph and what its execution reads, asked once.
    graphs: HashMap<crate::durable::TaskId, crate::ops::graph_process::GraphProcess>,
    execution: crate::store::sqlite::GraphExecutionFacts,
}

impl<'a> DetailReads<'a> {
    fn read(store: &SharedStore, tasks: &[&Task], shared: &'a SharedTaskReads) -> Result<Self> {
        let facts = store.sqlite.task_detail_facts(tasks)?;
        let graphs = store
            .sqlite
            .latest_task_graphs(&tasks.iter().map(|task| &task.id).collect::<Vec<_>>())?;
        let execution = store.sqlite.graph_execution_facts(graphs.values())?;
        let mut defaults = HashMap::<_, (String, crate::work::wave::setup::SetupSource)>::new();
        let mut selections = HashMap::new();
        let mut wave_names = HashMap::new();
        let mut personal = HashMap::new();
        for task in tasks {
            let selection = match facts.graph_names.get(&task.id) {
                Some(name) => (name.clone(), crate::work::wave::setup::SetupSource::Task),
                None => match defaults.entry((task.wave_id.clone(), task.repository_id.clone())) {
                    std::collections::hash_map::Entry::Occupied(entry) => entry.get().clone(),
                    std::collections::hash_map::Entry::Vacant(entry) => entry
                        .insert(crate::ops::task::task_graph_default(store, task)?)
                        .clone(),
                },
            };
            selections.insert(task.id.clone(), selection);
            if let Some(id) = task
                .wave_id
                .as_ref()
                .filter(|id| !wave_names.contains_key(*id))
            {
                if let Some(wave) = store.sqlite.get_wave(id)? {
                    wave_names.insert(id.clone(), wave.slug().to_owned());
                }
            }
            if let std::collections::hash_map::Entry::Vacant(entry) =
                personal.entry((task.wave_id.clone(), task.repository_id.clone()))
            {
                let repo = crate::ops::task::repository_for_task(store, task)?;
                let wave = task
                    .wave_id
                    .as_ref()
                    .and_then(|id| wave_names.get(id))
                    .map(String::as_str);
                let values = crate::config::PersonalSettings::read(Some(&repo), wave);
                entry.insert((repo, values));
            }
        }
        let mut shipping = HashMap::new();
        for task in tasks.iter().filter(|task| facts.shipped.contains(&task.id)) {
            let status = *facts
                .states
                .get(&task.id)
                .ok_or_else(|| anyhow!("Task {} has no state", task.id))?;
            let graph = store.sqlite.task_graph(&task.id)?;
            if let Some(snapshot) = crate::ops::task::ship::read_shipping(
                &store.sqlite,
                &task.id,
                graph.as_ref(),
                status,
            )? {
                shipping.insert(task.id.clone(), snapshot);
            }
        }
        Ok(Self {
            shared,
            facts,
            selections,
            wave_names,
            personal,
            shipping,
            graphs,
            execution,
        })
    }
}

fn unavailable_task(task: &Task, status: TaskState) -> UnavailableTaskEvidence {
    const REASON: &str = "Task's owning Project is unavailable";
    UnavailableTaskEvidence {
        work_id: task.id.to_string(),
        task_id: task.id.to_string(),
        task_identifier: task.identifier.clone(),
        status,
        owner: NextMoveOwner::Wave,
        reason: REASON.to_string(),
        recovery: format!("lf task status {} --json", task.id),
    }
}

async fn snapshot_task_detail(
    store: &SharedStore,
    task: &Task,
    probe_pr_empty: bool,
    reads: &DetailReads<'_>,
) -> Result<RoadmapTask> {
    let facts = &reads.facts;
    let shared = reads.shared;
    let pr = facts.active_prs.get(&task.id).cloned();
    let active = pr.as_ref().filter(|pr| pr.is_active());
    let observed_at = now();
    let (execution, graph_record) = crate::ops::task_execution::execution_of_latest_graph(
        &store.sqlite,
        task,
        reads.graphs.get(&task.id),
        &reads.execution,
    )
    .await?;
    let status = *facts
        .states
        .get(&task.id)
        .ok_or_else(|| anyhow!("Task {} has no state", task.id))?;
    let pending = facts
        .completion_pending
        .get(&task.id)
        .map(|reason| crate::ops::task::completion_pending_text(task, reason));
    let owner = facts.owners.get(&task.id).cloned();
    let wave = task
        .wave_id
        .as_ref()
        .and_then(|id| reads.wave_names.get(id))
        .map(String::as_str);
    let (repo, personal) = reads
        .personal
        .get(&(task.wave_id.clone(), task.repository_id.clone()))
        .ok_or_else(|| anyhow!("Task {} has no setup reading", task.id))?;
    let agent = crate::ops::task::resolve_task_agent(
        task.worktree.as_deref().unwrap_or(repo),
        wave,
        personal,
        task.agent.as_deref(),
        None,
    );
    let (provider, _) = crate::config::parse_agent(&agent);
    let runtime = snapshot_task_runtime(&execution, task, status, pending, owner, provider);
    let machine_id = shared
        .checkouts
        .iter()
        .find(|row| row.task_id == task.id)
        .and_then(|row| row.machine_id.clone());
    let reference = task_reference(
        facts.issue_urls.get(&task.id).cloned(),
        Some(task),
        machine_id,
        &shared.machines,
        &shared.local_machine,
    );
    let acquisition = facts
        .acquisitions
        .get(&task.id)
        .cloned()
        .ok_or_else(|| anyhow!("Task {} has no planning acquisition", task.id))?;
    let latest_event = facts.latest_events.get(&task.id);
    let worktree_blocker = crate::ops::task::task_worktree_blocker_after(task, latest_event)?;
    let launch_refusal = if runtime.status == TaskState::Deferred {
        Some("Task is deferred; resume it before running a Graph".to_string())
    } else if worktree_blocker.is_none() {
        crate::ops::task::task_process_refusal_after(task, wave, personal, latest_event).or_else(
            || {
                matches!(
                    acquisition.state,
                    crate::store::PlanningState::Invalid | crate::store::PlanningState::Removed
                )
                .then(|| {
                    "Task planning has invalidation or removal evidence; refresh its planning"
                        .to_string()
                })
            },
        )
    } else {
        None
    };
    let next_move = if runtime.status == TaskState::Deferred {
        NextMove {
            owner: NextMoveOwner::User,
            reason: "Deferred; resume explicitly when wanted".into(),
        }
    } else if execution.state != TaskExecutionState::Idle && !runtime.status.is_terminal() {
        NextMove {
            owner: match execution.state {
                TaskExecutionState::Blocked | TaskExecutionState::Unknown => NextMoveOwner::Wave,
                _ => NextMoveOwner::Task,
            },
            reason: execution.reason.clone(),
        }
    } else if pr.as_ref().is_some_and(|pr| pr.phase() == PrPhase::Merged)
        && !runtime.status.is_terminal()
    {
        NextMove {
            owner: NextMoveOwner::Wave,
            reason: "Merged; complete the Task".into(),
        }
    } else {
        next_move_for_task(
            &runtime.status.work_status(),
            active.map(TaskPr::phase),
            active
                .filter(|pr| pr.phase() == PrPhase::Open)
                .map(|pr| pr.presentation().is_some()),
            active.and_then(|pr| pr.fresh_ci()),
            active.and_then(TaskPr::merge_request),
            launch_refusal.as_deref(),
        )
    };
    let local_progress = task_local_progress(
        Some(task),
        Some(&runtime),
        pr.as_ref(),
        worktree_blocker.as_ref(),
    );
    let completion_refusal = if runtime.status != TaskState::Abandoned {
        crate::ops::task::completion_refusal(pr.as_ref(), &task.identifier)
    } else {
        None
    };
    let resume_refusal = worktree_blocker
        .as_ref()
        .map(|blocker| blocker.reason.clone());
    let predecessor_phase = task
        .parent_pr_id
        .as_ref()
        .and_then(|parent_id| facts.parent_prs.get(parent_id))
        .map(TaskPr::phase);
    let action_evidence = TaskActionEvidence {
        status: runtime.status.work_status(),
        execution: Some(&execution),
        latest_pr_phase: pr.as_ref().map(TaskPr::phase),
        latest_pr_merge_request: pr.as_ref().and_then(TaskPr::merge_request),
        latest_pr_presentation_current: pr
            .as_ref()
            .filter(|pr| pr.phase() == PrPhase::Open)
            .map(|pr| pr.presentation().is_some()),
        completion_refusal: completion_refusal.as_deref(),
        resume_refusal: resume_refusal.as_deref(),
        ci: active.and_then(|pr| pr.fresh_ci()),
        predecessor_phase,
        launch_refusal: launch_refusal.as_deref(),
    };
    let condition = derive_task_condition(
        Some(&runtime),
        &next_move,
        local_progress,
        Some(&action_evidence),
        Some(&execution),
        observed_at,
    );
    let actions = derive_task_actions(&action_evidence);
    let work_status = runtime.status.work_status();
    let run_control =
        crate::ops::task_run::task_run_control(&crate::ops::task_run::TaskRunEvidence {
            status: Some(&work_status),
            // Shared completion withdraws an unstarted Task, never moves its Graph.
            plan_terminal_reason: crate::linear::terminal_reason(task.state.as_deref())
                .filter(|_| matches!(runtime.status, TaskState::Unstarted)),
            worktree_blocker: worktree_blocker
                .as_ref()
                .map(|blocker| blocker.reason.as_str()),
            launch_refusal: launch_refusal.as_deref(),
        });
    let shipping = reads.shipping.get(&task.id).cloned();
    let ship_control = TaskRunControl {
        unavailable: run_control.unavailable.clone().or_else(|| {
            if shipping.is_some() {
                Some("Task is already shipping".to_string())
            } else if facts
                .steps
                .get(&task.id)
                .is_some_and(|step| step == crate::task_graph::END)
            {
                Some("Task is at the end of its graph".to_string())
            } else {
                None
            }
        }),
    };
    let (graph_name, graph_source) = reads
        .selections
        .get(&task.id)
        .cloned()
        .ok_or_else(|| anyhow!("Task {} has no graph", task.id))?;
    let section = task_section(task.state.as_deref(), Some(&runtime), &next_move);
    Ok(RoadmapTask {
        task: task_summary(
            task,
            facts
                .planning_sync
                .get(&task.id)
                .cloned()
                .ok_or_else(|| anyhow!("Task {} has no planning sync", task.id))?,
            acquisition,
            facts
                .repository_paths
                .get(&task.repository_id)
                .cloned()
                .ok_or_else(|| anyhow!("Task {} repository is unavailable", task.id))?,
        ),
        reference,
        runtime: Some(runtime),
        next_move,
        condition,
        actions,
        graph_name: Some(graph_name),
        graph_source: Some(graph_source),
        latest_graph_process: graph_record,
        execution: Some(execution),
        run_control,
        ship_control,
        shipping,
        pr: pr.as_ref().map(|pr| {
            let empty = (probe_pr_empty && pr.is_active())
                .then(|| task_pr_empty(task, pr))
                .flatten();
            PrSnapshot::new(pr, empty)
        }),
        section,
        step: facts.steps.get(&task.id).cloned(),
    })
}

fn task_local_progress(
    task: Option<&Task>,
    runtime: Option<&TaskRuntimeSnapshot>,
    pr: Option<&TaskPr>,
    worktree_blocker: Option<&crate::ops::task::TaskWorktreeBlocker>,
) -> LocalProgressEvidence {
    let Some(worktree) = task.and_then(|task| task.worktree.as_deref()) else {
        return LocalProgressEvidence {
            state: LocalProgressEvidenceState::NotApplicable,
            unsettled: Some(false),
            dirty: None,
            authored_commits: None,
            recovery_required: None,
            reason: None,
        };
    };
    let status = runtime
        .map(|runtime| runtime.status.work_status())
        .expect("Task runtime exists when the durable Task exists");
    let unsettled_branch =
        pr.is_some_and(TaskPr::is_active) || (pr.is_none() && !work_status_is_terminal(&status));
    inspect_task_local_progress(
        &status,
        worktree,
        task.filter(|_| unsettled_branch)
            .map(|task| task.base_commit.as_str()),
        worktree_blocker,
    )
}

fn inspect_task_local_progress(
    status: &WorkStatus,
    worktree: &Path,
    active_pr_base: Option<&str>,
    worktree_blocker: Option<&crate::ops::task::TaskWorktreeBlocker>,
) -> LocalProgressEvidence {
    let recovery_required = Some(false);
    if let Some(blocker) = worktree_blocker {
        return LocalProgressEvidence {
            state: LocalProgressEvidenceState::Missing,
            unsettled: Some(!blocker.initializing),
            dirty: None,
            authored_commits: None,
            recovery_required: Some(!blocker.initializing),
            reason: Some(blocker.reason.clone()),
        };
    }
    if !worktree.exists() {
        if work_status_is_terminal(status) && active_pr_base.is_none() {
            return LocalProgressEvidence {
                state: LocalProgressEvidenceState::NotApplicable,
                unsettled: Some(false),
                dirty: None,
                authored_commits: None,
                recovery_required: Some(false),
                reason: Some("terminal Task delivery is settled; no worktree remains".into()),
            };
        }
        return LocalProgressEvidence {
            state: LocalProgressEvidenceState::Missing,
            unsettled: Some(true),
            dirty: None,
            authored_commits: None,
            recovery_required: Some(true),
            reason: Some(format!("Task worktree is missing: {}", worktree.display())),
        };
    }
    let dirty = match crate::git::is_clean(worktree) {
        Ok(clean) => !clean,
        Err(error) => {
            return LocalProgressEvidence {
                state: LocalProgressEvidenceState::Unavailable,
                unsettled: None,
                dirty: None,
                authored_commits: None,
                recovery_required,
                reason: Some(format!("failed to inspect Task worktree: {error}")),
            }
        }
    };
    let authored_commits = match active_pr_base {
        Some(base) => match crate::git::rev_parse(worktree, "HEAD") {
            Ok(head) => Some(head != base),
            Err(error) => {
                return LocalProgressEvidence {
                    state: LocalProgressEvidenceState::Unavailable,
                    unsettled: dirty.then_some(true),
                    dirty: Some(dirty),
                    authored_commits: None,
                    recovery_required,
                    reason: Some(format!("failed to inspect Task HEAD: {error}")),
                }
            }
        },
        // Merged or abandoned PR history is settled delivery. With no active
        // PR only new dirty changes can still require local recovery.
        None => Some(false),
    };
    let unsettled = match recovery_required {
        Some(recovery) => Some(dirty || authored_commits == Some(true) || recovery),
        None if dirty || authored_commits == Some(true) => Some(true),
        None => None,
    };
    LocalProgressEvidence {
        state: LocalProgressEvidenceState::Observed,
        unsettled,
        dirty: Some(dirty),
        authored_commits,
        recovery_required,
        reason: None,
    }
}

fn derive_task_condition(
    runtime: Option<&TaskRuntimeSnapshot>,
    next_move: &NextMove,
    local_progress: LocalProgressEvidence,
    action_evidence: Option<&TaskActionEvidence>,
    execution: Option<&TaskExecutionSnapshot>,
    observed_at: time::OffsetDateTime,
) -> TaskConditionSnapshot {
    // A removed historical checkout does not reopen settled work.
    let unresolved_execution = runtime.is_some_and(|runtime| {
        (runtime.owner.is_some() && !runtime.status.is_terminal())
            || action_evidence.is_some_and(|evidence| evidence.completion_refusal.is_some())
            || execution.is_some_and(|execution| {
                execution.state != TaskExecutionState::Idle
                    && !(runtime.status.is_terminal()
                        && execution.state == TaskExecutionState::Blocked)
            })
            || (local_progress.state == LocalProgressEvidenceState::Observed
                && local_progress.unsettled == Some(true))
    });
    let active_pr_phase = action_evidence
        .and_then(|e| e.latest_pr_phase)
        .filter(|phase| phase.is_active());
    let launch_blocked = action_evidence.is_some_and(|evidence| evidence.launch_refusal.is_some());
    let delivery_owned_by_active_pr = local_progress.dirty == Some(false)
        && local_progress.recovery_required == Some(false)
        && local_progress.authored_commits == Some(true)
        && matches!(active_pr_phase, Some(PrPhase::Open | PrPhase::Publishing));
    let execution = action_evidence.and_then(|evidence| evidence.execution);
    // Settled work keeps its retained history without asking anyone to act.
    let settled = runtime
        .map(|runtime| runtime.status)
        .filter(|status| status.is_terminal() && !unresolved_execution);
    let (state, reason) = if let Some(status) = settled {
        let reason = match status {
            TaskState::Abandoned => "Task is abandoned",
            _ => "Task is complete",
        };
        (TaskConditionState::Clear, reason.to_string())
    } else if let Some(execution) =
        execution.filter(|execution| execution.state != TaskExecutionState::Idle)
    {
        let state = match execution.state {
            TaskExecutionState::Starting | TaskExecutionState::Running => TaskConditionState::Clear,
            TaskExecutionState::Blocked | TaskExecutionState::Stalled => {
                TaskConditionState::Blocked
            }
            TaskExecutionState::Unknown => TaskConditionState::Unknown,
            TaskExecutionState::Idle => unreachable!("idle execution uses local progress"),
        };
        (state, execution.reason.clone())
    } else if launch_blocked {
        (TaskConditionState::Blocked, next_move.reason.clone())
    } else if local_progress.state == LocalProgressEvidenceState::Missing
        && local_progress.recovery_required == Some(false)
    {
        (
            TaskConditionState::Clear,
            local_progress
                .reason
                .clone()
                .unwrap_or_else(|| "Task worktree is initializing".into()),
        )
    } else if local_progress.unsettled == Some(true) && !delivery_owned_by_active_pr {
        let reason = if local_progress.dirty == Some(true) {
            "Task has uncommitted work".to_string()
        } else if local_progress.authored_commits == Some(true) {
            match active_pr_phase {
                Some(PrPhase::Open) | Some(PrPhase::Publishing) => next_move.reason.clone(),
                _ => "Task has unsettled commits".to_string(),
            }
        } else if let Some(reason) = &local_progress.reason {
            reason.clone()
        } else {
            "local Task progress requires recovery".to_string()
        };
        (TaskConditionState::Blocked, reason)
    } else if local_progress.unsettled.is_none() {
        (
            TaskConditionState::Unknown,
            local_progress
                .reason
                .clone()
                .unwrap_or_else(|| "local Task progress is unavailable".into()),
        )
    } else if next_move.owner != NextMoveOwner::Task {
        (TaskConditionState::Waiting, next_move.reason.clone())
    } else {
        (TaskConditionState::Clear, next_move.reason.clone())
    };
    TaskConditionSnapshot {
        state,
        reason,
        observed_at: format_time(observed_at)
            .expect("Task condition observation time formats as RFC 3339"),
        evidence_age_secs: runtime.and_then(|runtime| age_secs(&runtime.updated_at, observed_at)),
        local_progress,
        unresolved_execution,
    }
}

fn task_reference(
    issue_url: Option<String>,
    task: Option<&Task>,
    machine_id: Option<crate::durable::MachineId>,
    machines: &[crate::durable::Machine],
    local_machine: &crate::durable::MachineId,
) -> TaskReferenceSnapshot {
    let workspace = task.and_then(|task| {
        let task_worktree = task.worktree.as_ref()?;
        let branch = Some(task.branch.clone());
        let local = machine_id.as_ref() == Some(local_machine);
        // A removed checkout has no root to resolve.
        let worktree = if local && task_worktree.is_dir() {
            crate::git::worktree_root(task_worktree)
                .ok()
                .and_then(|root| root.canonicalize().ok())
                .unwrap_or_else(|| task_worktree.clone())
        } else {
            task_worktree.clone()
        };
        Some(TaskWorktreeSnapshot {
            machine_label: machines
                .iter()
                .find(|machine| Some(&machine.id) == machine_id.as_ref())
                .and_then(|machine| machine.label.clone()),
            machine_id,
            slug: task.workspace_slug.clone(),
            branch,
            base_commit: task.base_commit.clone(),
            worktree: worktree.display().to_string(),
            local_exists: local.then(|| worktree.try_exists().ok()).flatten(),
        })
    });
    TaskReferenceSnapshot {
        issue_url,
        workspace,
    }
}

fn task_pr_empty(task: &Task, pr: &TaskPr) -> Option<bool> {
    let worktree = task.worktree.as_ref()?;
    if !worktree.exists() {
        return None;
    }
    let clean = crate::git::is_clean(worktree).ok()?;
    if !clean {
        return Some(false);
    }
    let head = crate::git::rev_parse(worktree, "HEAD").ok()?;
    Some(head == pr.base_commit)
}

fn task_summary(
    task: &Task,
    sync: crate::store::planning_sync::PlanningSyncStatus,
    acquisition: crate::store::TaskAcquisition,
    repo: String,
) -> TaskSummary {
    TaskSummary {
        repository_id: task.repository_id.to_string(),
        repo,
        project_id: task.project_id.as_ref().map(ToString::to_string),
        wave_id: task.wave_id.as_ref().map(ToString::to_string),
        acquisition,
        due_date: task.due_date.clone(),
        sync,
        id: task.id.to_string(),
        linear_id: task.linear_id.as_ref().map(|id| id.as_str().to_string()),
        identifier: task.identifier.clone(),
        name: task.title.clone(),
        description: task.description.clone(),
        rank: task.rank,
        state: task.state.clone(),
        completed_at: task.completed_at.clone(),
        assignee: task.assignee.clone(),
    }
}

fn next_move_for_task(
    status: &WorkStatus,
    pr_phase: Option<PrPhase>,
    pr_presentation_current: Option<bool>,
    ci: Option<&CiObservation>,
    merge: Option<&PrMergeRequest>,
    launch_refusal: Option<&str>,
) -> NextMove {
    if let Some(reason) = launch_refusal.filter(|_| !work_status_is_terminal(status)) {
        return NextMove {
            owner: NextMoveOwner::User,
            reason: reason.to_string(),
        };
    }
    if pr_phase == Some(PrPhase::Open) {
        if pr_presentation_current == Some(false) {
            return NextMove {
                owner: NextMoveOwner::Wave,
                reason: "refresh reviewer-facing PR copy for the current head before settlement"
                    .to_string(),
            };
        }
        if let Some(ci) = ci {
            let repairable_failure = ci.state == CiState::Failing;
            if repairable_failure {
                return NextMove {
                    owner: NextMoveOwner::Ci,
                    reason: ci_failure_reason(ci),
                };
            }
        }
        if let Some(request) = merge {
            if ci.is_none_or(|ci| ci.state == CiState::Pending) {
                return NextMove {
                    owner: NextMoveOwner::Ci,
                    reason: "required checks have not passed for the requested merge".to_string(),
                };
            }
            let short = request.head_sha.chars().take(12).collect::<String>();
            return match request.mode {
                PrMergeMode::User => NextMove {
                    owner: NextMoveOwner::User,
                    reason: format!("merge pull request head {short} on GitHub"),
                },
                PrMergeMode::Auto => NextMove {
                    owner: NextMoveOwner::External,
                    reason: format!("GitHub auto-merge is settling head {short}"),
                },
            };
        }
        return NextMove {
            owner: NextMoveOwner::Wave,
            reason: "PR is published but settlement is not armed with `lf pr land -c`".to_string(),
        };
    }
    let owner = NextMoveOwner::Wave;
    NextMove {
        owner,
        reason: status.reason().to_string(),
    }
}

fn ambient_wave() -> Option<String> {
    std::env::var(crate::work::wave::context::WAVE_ID_ENV)
        .ok()
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
}

fn format_time(ts: time::OffsetDateTime) -> Option<String> {
    ts.format(&time::format_description::well_known::Rfc3339)
        .ok()
}

/// With no registry on this machine, `lf wave list`/`status` have nothing to read —
/// emit the empty snapshot (`[]`/`null`) or a User note, and succeed.
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
        "{bold}{name:<16}  {repo:<28}  {status:<8}  {tasks:>5}  {machine:<16}{reset}",
        bold = colors.bold,
        reset = colors.reset,
        name = "WAVE",
        repo = "REPOSITORY",
        status = "STATUS",
        tasks = "TASKS",
        machine = "MACHINE",
    );
    for wave in snapshots {
        println!(
            "{name:<16}  {repo:<28}  {status:<8}  {tasks:>5}  {machine:<16}",
            name = truncate(&wave.name, 16),
            repo = truncate_start(&wave.repo, 28),
            status = wave.status.label(),
            tasks = wave.active_tasks,
            machine = truncate(
                wave.machine
                    .as_ref()
                    .map_or("unplaced", |machine| &machine.route),
                16
            ),
        );
    }
}

fn work_status_is_terminal(status: &WorkStatus) -> bool {
    matches!(status, WorkStatus::Done | WorkStatus::Abandoned)
}

fn print_setup(setup: &crate::work::wave::setup::WaveSetup) {
    use crate::work::wave::setup::SetupValue;
    let line = |label: &str, value: &SetupValue| {
        println!(
            "  {label:<16} {} ({})",
            value.value.as_deref().unwrap_or("-"),
            value.source.as_str()
        );
    };
    println!("  setup");
    line("graph", &setup.graph);
    line("agent", &setup.agent);
    for (skill, value) in &setup.skill_agents {
        line(&format!("  {skill}"), value);
    }
    line("tasks machine", &setup.tasks_machine);
    line("session machine", &setup.session_machine);
    for (provider, route) in &setup.accounts {
        let accounts = if route.accounts.is_empty() {
            "unrouted".to_string()
        } else {
            route.accounts.join(" -> ")
        };
        println!(
            "  {:<16} {accounts} ({})",
            format!("{provider} accounts"),
            route.source.as_str()
        );
    }
}

fn print_status(status: &WaveDetailSnapshot) {
    let colors = Colors::default();
    let wave = &status.wave;
    println!(
        "{bold}{name}{reset}  {status}",
        bold = colors.bold,
        reset = colors.reset,
        name = wave.name,
        status = if wave.retired_at.is_some() {
            "retired"
        } else {
            wave.status.label()
        },
    );
    if let Some(retired_at) = &wave.retired_at {
        println!(
            "  history   retired at {retired_at}; superseded by {}: {}",
            wave.superseded_by_wave_id.as_deref().unwrap_or("-"),
            wave.retirement_reason.as_deref().unwrap_or("retired")
        );
    }
    println!("  goal      {}", wave.goal);
    match &wave.machine {
        Some(machine) => println!("  machine      {} ({})", machine.id, machine.route),
        None => println!("  machine      unplaced"),
    }
    print_setup(&status.setup);
    print_projects(&status.projects);
    print_metric_portfolio(&status.metric_portfolio);
    match &status.tasks {
        Evidence::Unavailable { reason } => println!("  tasks unavailable: {reason}"),
        Evidence::Ok { items, .. } => {
            if items.is_empty() {
                println!("  tasks     none");
            }
            for task in items {
                let state = task
                    .runtime
                    .as_ref()
                    .map(|runtime| runtime.status.label())
                    .unwrap_or(task.task.state.as_deref().unwrap_or("unstarted"));
                println!(
                    "  {}  {:<10}  {:<12}  {}  {}",
                    task.task.identifier,
                    state,
                    owner_text(task.runtime.as_ref()),
                    task.task.name,
                    task.next_move.reason
                );
                if let Some(due) = &task.task.due_date {
                    println!("    due        {due}");
                }
                for line in task.task.sync.lines() {
                    println!("    {line}");
                }
                if let Some(workspace) = &task.reference.workspace {
                    println!("    workspace  {}  {}", workspace.slug, workspace.worktree);
                }
                if let Some(url) = &task.reference.issue_url {
                    println!("    issue      {url}");
                }
                if let Some(pr) = &task.pr {
                    if let Some(github) = pr
                        .publication
                        .as_ref()
                        .and_then(|publication| publication.github.as_ref())
                    {
                        println!("    PR #{}  {}", github.number, github.url);
                    }
                }
            }
        }
    }
    print_unavailable_tasks(&status.unavailable_tasks);
    print_history(&status.history);
}

fn print_unavailable_tasks(tasks: &[UnavailableTaskEvidence]) {
    for task in tasks {
        println!(
            "  {}  unavailable: {} · {}",
            task.task_identifier, task.reason, task.recovery
        );
    }
}

fn print_metric_portfolio(portfolio: &MetricPortfolioDto) {
    print!("{}", metric_portfolio_text(portfolio));
}

pub fn metric_portfolio_text(portfolio: &MetricPortfolioDto) -> String {
    if portfolio.metrics.is_empty() && portfolio.contract_issues.is_empty() {
        return String::new();
    }
    let mut lines = vec!["  metrics".to_string()];
    let official = portfolio
        .metrics
        .iter()
        .filter(|metric| metric.stage == MetricStage::Graduated)
        .collect::<Vec<_>>();
    let mut official = official;
    official.sort_by(|left, right| {
        metric_priority(&left.evidence)
            .cmp(&metric_priority(&right.evidence))
            .then(left.name.cmp(&right.name))
    });
    for metric in official {
        append_metric_lines(&mut lines, metric, portfolio, "    ");
    }

    let mut candidates = portfolio
        .metrics
        .iter()
        .filter(|metric| metric.stage == MetricStage::Installed)
        .collect::<Vec<_>>();
    candidates.sort_by(|left, right| left.name.cmp(&right.name));
    if !candidates.is_empty() {
        lines.push("    Instrumenting".to_string());
        for metric in candidates {
            append_metric_lines(&mut lines, metric, portfolio, "      ");
        }
    }
    if !portfolio.contract_issues.is_empty() {
        lines.push("    Contract issues".to_string());
        for issue in &portfolio.contract_issues {
            lines.push(format!("      {}", metric_contract_issue(issue)));
        }
    }
    format!("{}\n", lines.join("\n"))
}

fn append_metric_lines(
    lines: &mut Vec<String>,
    metric: &MetricReadingDto,
    portfolio: &MetricPortfolioDto,
    indent: &str,
) {
    lines.push(format!(
        "{indent}{}  [{}]",
        metric.name,
        metric_evidence_label(&metric.evidence),
    ));
    lines.push(format!(
        "{indent}  Owner Wave · Value {value} · Target {target} over {window} · {freshness}",
        value = metric_value(metric),
        target = metric_target(metric, portfolio),
        window = metric.window,
        freshness = metric_freshness(&metric.freshness),
    ));
    if let Some(reason) = metric_reason(&metric.evidence) {
        lines.push(format!("{indent}  {reason}"));
    }
}

fn metric_priority(evidence: &MetricEvidenceDto) -> u8 {
    match evidence {
        MetricEvidenceDto::Missed { .. } | MetricEvidenceDto::Unavailable { .. } => 0,
        MetricEvidenceDto::Unknown { .. } | MetricEvidenceDto::Untargeted { .. } => 1,
        MetricEvidenceDto::Met { .. } => 2,
    }
}

fn metric_evidence_label(evidence: &MetricEvidenceDto) -> &'static str {
    match evidence {
        MetricEvidenceDto::Met { .. } => "met",
        MetricEvidenceDto::Untargeted { .. } => "no target",
        MetricEvidenceDto::Missed { .. } => "missed",
        MetricEvidenceDto::Unknown { .. } => "unknown",
        MetricEvidenceDto::Unavailable { .. } => "unavailable",
    }
}

fn metric_value(metric: &MetricReadingDto) -> String {
    let value = match &metric.evidence {
        MetricEvidenceDto::Untargeted { value, .. }
        | MetricEvidenceDto::Met { value, .. }
        | MetricEvidenceDto::Missed { value, .. } => Some(*value),
        MetricEvidenceDto::Unknown { cause } => match cause {
            MetricUnknownCauseDto::TargetUnavailable { value, .. }
            | MetricUnknownCauseDto::Incomplete { value, .. }
            | MetricUnknownCauseDto::WindowMismatch { value, .. }
            | MetricUnknownCauseDto::StaleObservation { value, .. } => Some(*value),
            MetricUnknownCauseDto::Never
            | MetricUnknownCauseDto::RevisionMismatch { .. }
            | MetricUnknownCauseDto::StaleUnavailable { .. } => None,
        },
        MetricEvidenceDto::Unavailable { .. } => None,
    };
    value
        .map(|value| format_metric_number(value, &metric.unit))
        .unwrap_or_else(|| "-".to_string())
}

fn metric_target(metric: &MetricReadingDto, portfolio: &MetricPortfolioDto) -> String {
    if portfolio.contract_issues.iter().any(|issue| {
        matches!(issue,
            MetricContractIssueDto::ChapterUnavailable { wave_id, .. }
            if wave_id == metric.identity.wave_id()
        )
    }) {
        return "unavailable for this chapter".into();
    }
    match metric.target {
        None => "unset for this chapter".into(),
        Some(Threshold::AtLeast { value }) => {
            format!(">= {}", format_metric_number(value, &metric.unit))
        }
        Some(Threshold::AtMost { value }) => {
            format!("<= {}", format_metric_number(value, &metric.unit))
        }
    }
}

fn format_metric_number(value: f64, unit: &str) -> String {
    if unit == "ratio" {
        format!("{:.2}%", value * 100.0)
    } else {
        format!("{value:.3} {unit}")
    }
}

fn metric_freshness(freshness: &MetricFreshnessDto) -> String {
    match freshness {
        MetricFreshnessDto::Never => "never observed".to_string(),
        MetricFreshnessDto::Fresh { expires_at, .. } => format!(
            "fresh until {}",
            format_time(*expires_at).unwrap_or_else(|| "unknown".to_string())
        ),
        MetricFreshnessDto::Stale { expires_at, .. } => format!(
            "stale since {}",
            format_time(*expires_at).unwrap_or_else(|| "unknown".to_string())
        ),
    }
}

fn metric_reason(evidence: &MetricEvidenceDto) -> Option<String> {
    match evidence {
        MetricEvidenceDto::Untargeted { .. }
        | MetricEvidenceDto::Met { .. }
        | MetricEvidenceDto::Missed { .. } => None,
        MetricEvidenceDto::Unavailable {
            reason,
            source_as_of,
        } => Some(format!(
            "{reason} · as of {}",
            format_time(*source_as_of).unwrap_or_else(|| "unknown".to_string())
        )),
        MetricEvidenceDto::Unknown { cause } => Some(match cause {
            MetricUnknownCauseDto::Never => "no observation has arrived".to_string(),
            MetricUnknownCauseDto::TargetUnavailable { .. } => {
                "chapter target planning is unavailable".to_string()
            }
            MetricUnknownCauseDto::RevisionMismatch {
                expected_contract_revision,
                observed_contract_revision,
                source_time,
            } => format!(
                "evidence at {} measured revision {}, not {}",
                format_time(*source_time).unwrap_or_else(|| "unknown".to_string()),
                observed_contract_revision,
                expected_contract_revision
            ),
            MetricUnknownCauseDto::Incomplete { .. } => {
                "latest source window is incomplete".to_string()
            }
            MetricUnknownCauseDto::WindowMismatch { .. } => {
                "latest source window does not match the contract".to_string()
            }
            MetricUnknownCauseDto::StaleObservation { .. } => {
                "latest observation is stale".to_string()
            }
            MetricUnknownCauseDto::StaleUnavailable {
                reason,
                source_as_of,
            } => format!(
                "last source failure is stale: {reason} · as of {}",
                format_time(*source_as_of).unwrap_or_else(|| "unknown".to_string())
            ),
        }),
    }
}

fn metric_contract_issue(issue: &MetricContractIssueDto) -> String {
    match issue {
        MetricContractIssueDto::ChapterUnavailable { wave_id, reason } => format!("{wave_id}: chapter targets unavailable: {reason}"),
        MetricContractIssueDto::UnresolvedTarget { wave_id, metric_id } => format!("{wave_id}/{metric_id}: chapter target has no readable instrument contract"),
        MetricContractIssueDto::MalformedContract { path, message } => {
            format!("{path}: {message}")
        }
        MetricContractIssueDto::InstrumentMismatch {
            wave_id,
            metric_id,
            contract_instrument,
            registered_instrument,
        } => format!(
            "{wave_id}/{metric_id} declares {contract_instrument}, but {registered_instrument} is registered"
        ),
        MetricContractIssueDto::InvalidGraduation {
            wave_id,
            metric_id,
            reason,
            ..
        } => format!("{wave_id}/{metric_id} cannot graduate: {reason}"),
    }
}

fn print_history(history: &Evidence<SessionHistory>) {
    match history {
        Evidence::Unavailable { reason } => println!("  sessions unavailable: {reason}"),
        Evidence::Ok { items, .. } if items.is_empty() => {
            println!("  sessions   no Session history in the window")
        }
        Evidence::Ok { items, truncated } => {
            println!("  sessions");
            for run in items {
                println!(
                    "    {label:<24}  {status:<12}  tok {tokens:>7}  {age:>7} ago",
                    label = truncate(run.label(), 24),
                    status = run.status(),
                    tokens = run
                        .total_tokens()
                        .map(format_tokens)
                        .unwrap_or_else(|| "-".to_string()),
                    age = format_age(now().unix_timestamp() - run.observed_at),
                );
            }
            if *truncated {
                println!("    (older history beyond the window cap are not shown)");
            }
        }
    }
}

/// One rendered roadmap line, section already decided. Project-loop rows and
/// Task rows share the shape so a section prints them together.
struct RoadmapRow {
    section: RoadmapSection,
    id: String,
    issue_url: Option<String>,
    title: String,
    rank: Option<u32>,
    age_secs: Option<i64>,
    owner: String,
    next: NextMoveOwner,
    pr: Option<String>,
    workspace: Option<String>,
    condition: Option<TaskConditionState>,
    reason: String,
}

fn task_condition_label(state: TaskConditionState) -> &'static str {
    match state {
        TaskConditionState::Waiting => "waiting",
        TaskConditionState::Blocked => "blocked",
        TaskConditionState::Clear => "clear",
        TaskConditionState::Unknown => "unknown",
    }
}

fn task_roadmap_row(task: &RoadmapTask, now: time::OffsetDateTime) -> RoadmapRow {
    RoadmapRow {
        section: task.section,
        id: task.task.identifier.clone(),
        issue_url: task.reference.issue_url.clone(),
        title: task.task.name.clone(),
        rank: (task.task.rank != u32::MAX).then_some(task.task.rank),
        age_secs: task
            .runtime
            .as_ref()
            .and_then(|runtime| age_secs(&runtime.updated_at, now)),
        owner: owner_text(task.runtime.as_ref()).to_string(),
        next: task.next_move.owner,
        pr: task.pr.as_ref().map(pr_label),
        workspace: task
            .reference
            .workspace
            .as_ref()
            .map(|workspace| workspace.slug.clone()),
        condition: Some(task.condition.state),
        reason: std::iter::once(task.condition.reason.clone())
            .chain(task.task.sync.lines())
            .collect::<Vec<_>>()
            .join(" · "),
    }
}

fn pr_label(pr: &PrSnapshot) -> String {
    match pr
        .publication
        .as_ref()
        .and_then(|publication| publication.github.as_ref())
    {
        Some(github) => format!("#{}:{}", github.number, pr.slug),
        None => format!("pr:{}", pr.slug),
    }
}

/// Render a fixed-width Task identifier. Terminals that support OSC 8 get the
/// identifier itself as the link; redirected output stays plain and stable.
fn task_identifier_label(
    identifier: &str,
    issue_url: Option<&str>,
    width: usize,
    hyperlinks: bool,
) -> String {
    let label = format!("{:<width$}", truncate(identifier, width));
    let Some(url) = issue_url.filter(|url| {
        (url.starts_with("https://") || url.starts_with("http://"))
            && !url.chars().any(char::is_control)
    }) else {
        return label;
    };
    if !hyperlinks {
        return label;
    }
    format!("\x1b]8;;{url}\x1b\\{label}\x1b]8;;\x1b\\")
}

fn section_label(section: RoadmapSection) -> &'static str {
    match section {
        RoadmapSection::Now => "NOW",
        RoadmapSection::Waiting => "WAITING",
        RoadmapSection::NotStarted => "NOT STARTED",
        RoadmapSection::Later => "LATER",
    }
}

fn print_roadmap(roadmap: &RoadmapSnapshot) {
    let colors = Colors::default();
    if roadmap.waves.is_empty() {
        println!("No waves in the registry.");
        return;
    }
    let now = now();
    for wave in &roadmap.waves {
        println!(
            "{bold}{name}{reset}  {status}",
            bold = colors.bold,
            reset = colors.reset,
            name = wave.wave.name,
            status = wave.wave.status.label(),
        );
        print_projects(&wave.projects);
        print_metric_portfolio(&wave.metric_portfolio);
        print_unavailable_tasks(&wave.unavailable_tasks);
        let tasks = match roadmap.wave_tasks(wave) {
            Evidence::Unavailable { reason } => {
                println!("  tasks unavailable: {reason}");
                continue;
            }
            Evidence::Ok { items, .. } => items,
        };
        let rows: Vec<RoadmapRow> = tasks
            .into_iter()
            .map(|task| task_roadmap_row(task, now))
            .collect();
        if rows.is_empty() {
            println!("  (no plan rows)");
            continue;
        }
        // Wide enough for an unnamed owner's email key.
        let owner_width = rows
            .iter()
            .map(|row| row.owner.chars().count())
            .max()
            .unwrap_or(0)
            .clamp(12, 32);
        for section in [
            RoadmapSection::Now,
            RoadmapSection::Waiting,
            RoadmapSection::NotStarted,
            RoadmapSection::Later,
        ] {
            let in_section: Vec<&RoadmapRow> =
                rows.iter().filter(|row| row.section == section).collect();
            if in_section.is_empty() {
                continue;
            }
            println!("  {}", section_label(section));
            for row in in_section {
                println!(
                    "    {id}  {rank:>4}  {owner:<owner_width$}  next:{next:<8}  {age:>5}  {condition:<7}  {workspace:<20}  {pr:<24}  {title}",
                    id = task_identifier_label(
                        &row.id,
                        row.issue_url.as_deref(),
                        12,
                        std::io::stdout().is_terminal(),
                    ),
                    rank = row
                        .rank
                        .map(|r| r.to_string())
                        .unwrap_or_else(|| "-".into()),
                    owner = truncate(&row.owner, owner_width),
                    next = owner_label(&row.next),
                    age = row
                        .age_secs
                        .map(format_age)
                        .unwrap_or_else(|| "-".to_string()),
                    condition = row
                        .condition
                        .map(task_condition_label)
                        .unwrap_or("-"),
                    workspace = truncate(row.workspace.as_deref().unwrap_or("-"), 20),
                    pr = truncate(row.pr.as_deref().unwrap_or("-"), 24),
                    title = truncate(&row.title, 36),
                );
                println!("      {}", row.reason);
            }
        }
    }
}

/// Who owns a Task, for text lists: their name, else their key, or "—".
fn owner_text(runtime: Option<&TaskRuntimeSnapshot>) -> &str {
    match runtime.and_then(|runtime| runtime.owner.as_ref()) {
        Some(owner) => owner.who(),
        None => "—",
    }
}

fn owner_label(owner: &NextMoveOwner) -> &'static str {
    match owner {
        NextMoveOwner::User => "user",
        NextMoveOwner::Wave => "wave",
        NextMoveOwner::Task => "task",
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

fn truncate_start(value: &str, width: usize) -> String {
    let length = value.chars().count();
    if length <= width {
        return value.to_string();
    }
    let tail = value
        .chars()
        .skip(length.saturating_sub(width.saturating_sub(1)))
        .collect::<String>();
    format!("\u{2026}{tail}")
}

#[cfg(test)]
mod tests {
    fn owned() -> crate::durable::TaskOwner {
        crate::durable::TaskOwner {
            person: Some("Jack Heart".into()),
            person_key: Some("jack@loopflow.studio".into()),
            machine: crate::durable::MachineRef {
                id: None,
                label: None,
            },
        }
    }

    use crate::test_linear_input::SeedLinear;
    use std::sync::Arc;
    use time::OffsetDateTime;

    use super::{
        derive_task_condition, metric_portfolio_text, next_move_for_task, truncate_start,
        LocalProgressEvidence, LocalProgressEvidenceState, NextMove, NextMoveOwner,
        TaskConditionState, TaskRuntimeSnapshot,
    };
    use crate::durable::{TaskState, WorkStatus};
    use crate::ops::task_actions::TaskActionEvidence;
    use crate::ops::task_execution::{TaskExecutionSnapshot, TaskExecutionState};
    use crate::work::task::{CiObservation, CiState, PrMergeMode, PrMergeRequest, PrPhase};
    use crate::work::wave::metrics::{
        MetricEvidenceDto, MetricFreshnessDto, MetricIdentity, MetricPortfolioDto,
        MetricReadingDto, MetricStage, MetricUnknownCauseDto, Threshold,
    };
    use crate::work::wave::Wave;

    #[tokio::test]
    async fn task_reference_names_recorded_checkout_machine_and_its_renames() {
        let directory = tempfile::tempdir().unwrap();
        let database = directory.path().join("registry.db");
        let store = Arc::new(
            crate::store::open_ephemeral_store(&crate::store::StorageConfig::sqlite(
                database.clone(),
            ))
            .await
            .unwrap(),
        );
        let wave = Wave::new(
            crate::id::WaveId::new(),
            "product".into(),
            directory.path().display().to_string(),
        );
        store.create_wave(&wave).await.unwrap();
        store
            .seed_inventory(
                crate::test_linear_input::InventoryFixture {
                    wave_id: wave.id().clone(),
                    provider: "linear".into(),
                    initiative: "initiative".into(),
                    synced_at: 1,
                    snapshot: serde_json::from_str(include_str!(
                        "../../../../../tests/fixtures/dto/task_history_planning.json"
                    ))
                    .unwrap(),
                },
                None,
            )
            .await
            .unwrap();
        let task = store.list_tasks(Some(wave.id())).await.unwrap().remove(0);
        let owner = crate::durable::MachineId::new();
        let assigned = crate::durable::MachineId::new();
        for (id, label) in [(&owner, "build-server"), (&assigned, "new-default")] {
            store
                .add_machine(id, &format!("{label}.invalid"), label)
                .await
                .unwrap();
        }
        let db = rusqlite::Connection::open(&database).unwrap();
        db.execute(
            "UPDATE tasks SET worktree='/remote/checkout',checkout_machine_id=?2 WHERE id=?1",
            rusqlite::params![task.id.as_str(), owner.as_str()],
        )
        .unwrap();
        let task = store.get_task(&task.id).await.unwrap().unwrap();
        let shared = super::SharedTaskReads::read(&store).await.unwrap();
        let reads = super::DetailReads::read(&store, &[&task], &shared).unwrap();
        let detail = super::snapshot_task_detail(&store, &task, false, &reads)
            .await
            .unwrap();
        let workspace = detail.reference.workspace.unwrap();
        assert_eq!(workspace.machine_id, Some(owner.clone()));
        assert_eq!(workspace.machine_label.as_deref(), Some("build-server"));
        assert_eq!(workspace.local_exists, None);
        store
            .rename_machine("build-server", "renamed-server")
            .await
            .unwrap();
        let shared = super::SharedTaskReads::read(&store).await.unwrap();
        let reads = super::DetailReads::read(&store, &[&task], &shared).unwrap();
        let detail = super::snapshot_task_detail(&store, &task, false, &reads)
            .await
            .unwrap();
        let workspace = detail.reference.workspace.unwrap();
        assert_eq!(workspace.machine_id, Some(owner));
        assert_eq!(workspace.machine_label.as_deref(), Some("renamed-server"));
    }

    #[tokio::test]
    async fn task_history_preserves_canceled_inventory_without_admitting_execution() {
        let directory = tempfile::tempdir().unwrap();
        let store = Arc::new(
            crate::store::open_ephemeral_store(&crate::store::StorageConfig::sqlite(
                directory.path().join("registry.db"),
            ))
            .await
            .unwrap(),
        );
        let wave = Wave::new(
            crate::id::WaveId::new(),
            "product".into(),
            directory.path().display().to_string(),
        );
        store.create_wave(&wave).await.unwrap();
        let planning: crate::test_linear_input::Inventory = serde_json::from_str(include_str!(
            "../../../../../tests/fixtures/dto/task_history_planning.json"
        ))
        .unwrap();
        store
            .seed_inventory(
                crate::test_linear_input::InventoryFixture {
                    wave_id: wave.id().clone(),
                    provider: "linear".into(),
                    initiative: "initiative".into(),
                    synced_at: 1,
                    snapshot: planning.clone(),
                },
                None,
            )
            .await
            .unwrap();
        let (details, gaps) = super::snapshot_tasks(
            &store,
            store.list_projects(Some(wave.id())).await.unwrap(),
            store.list_tasks(Some(wave.id())).await.unwrap(),
            false,
            false,
            &super::SharedTaskReads::read(&store).await.unwrap(),
        )
        .await
        .unwrap();
        assert!(gaps.is_empty());
        assert_eq!(details.len(), planning.items.len());
        for detail in &details {
            let item = planning
                .items
                .iter()
                .find(|item| Some(item.id.as_str()) == detail.task.linear_id.as_deref())
                .unwrap();
            assert_eq!(detail.task.state, item.state);
            assert_eq!(detail.task.completed_at, item.completed_at);
            let terminal = crate::linear::terminal_reason(detail.task.state.as_deref());
            assert_eq!(
                matches!(detail.section, super::RoadmapSection::Later),
                terminal.is_some()
            );
            if terminal.is_some() {
                assert!(detail.run_control.unavailable.is_some());
            }
        }
    }

    #[test]
    fn repository_resolution_preserves_aliases_missing_paths_and_fresh_reads() {
        let first = loopflow_test_support::TestRepo::new();
        let second = loopflow_test_support::TestRepo::new();
        let directory = tempfile::tempdir().unwrap();
        let alias = directory.path().join("repo");
        let wave = Wave::new(
            crate::id::WaveId::new(),
            "proof".into(),
            alias.display().to_string(),
        );
        let resolve = || super::wave_repository(&wave, &mut Default::default()).unwrap();
        assert_eq!(resolve(), alias);
        std::os::unix::fs::symlink(first.path(), &alias).unwrap();
        assert_eq!(resolve(), first.path().canonicalize().unwrap());
        std::fs::remove_file(&alias).unwrap();
        std::os::unix::fs::symlink(second.path(), &alias).unwrap();
        assert_eq!(resolve(), second.path().canonicalize().unwrap());
    }

    #[tokio::test]
    async fn local_ownership_read_does_not_require_repository_files() {
        let directory = tempfile::tempdir().unwrap();
        let store = Arc::new(
            crate::store::open_ephemeral_store(&crate::store::StorageConfig::sqlite(
                directory.path().join("registry.db"),
            ))
            .await
            .unwrap(),
        );
        let wave = Wave::new(
            crate::id::WaveId::new(),
            "local".into(),
            directory
                .path()
                .join("missing-checkout")
                .display()
                .to_string(),
        );
        store.create_wave(&wave).await.unwrap();
        super::validate_linear_ownership(&store, &[wave], &mut Default::default())
            .await
            .unwrap();
    }

    #[tokio::test]
    async fn portfolio_ownership_is_scoped_to_repository() {
        let _machine = crate::journal::TestLedgerGuard::new();
        let directory = tempfile::tempdir().unwrap();
        let store = crate::store::open_ephemeral_store(&crate::store::StorageConfig::sqlite(
            directory.path().join("registry.db"),
        ))
        .await
        .unwrap();
        let store = Arc::new(store);
        let mut waves = Vec::new();
        for (name, repository) in [("ear", "first"), ("ear", "second"), ("duplicate", "first")] {
            std::fs::create_dir_all(directory.path().join(repository)).unwrap();
            let wave = Wave::new(
                crate::id::WaveId::new(),
                name.into(),
                directory.path().join(repository).display().to_string(),
            );
            store.create_wave(&wave).await.unwrap();
            store
                .seed_inventory(
                    crate::test_linear_input::InventoryFixture {
                        wave_id: wave.id().clone(),
                        provider: "linear".into(),
                        initiative: "initiative".into(),
                        synced_at: 2,
                        snapshot: serde_json::from_value(serde_json::json!({"projects":[{
                    "id":format!("{repository}-{name}"), "slug":name, "name":name,
                    "summary":"", "metric_targets":[], "workflow":"feature", "status":"started",
                    "krs":[{"text":"Retained planning is readable", "holds":false}],
                    "initiative_ids":["initiative"], "team_ids":["team"]
                }], "items":[]}))
                        .unwrap(),
                    },
                    None,
                )
                .await
                .unwrap();
            waves.push(wave);
        }
        super::validate_linear_ownership(&store, &waves[..2], &mut Default::default())
            .await
            .unwrap();
        let error = super::validate_linear_ownership(&store, &waves, &mut Default::default())
            .await
            .unwrap_err();
        assert!(error.to_string().contains("bound by both"), "{error}");
        // A malformed Wave must not hide a readable sibling or its local Work.
        rusqlite::Connection::open(directory.path().join("registry.db"))
            .unwrap()
            .execute(
                "UPDATE pm_projects SET initiative_ids='not-json' WHERE id='first-ear'",
                [],
            )
            .unwrap();
        super::validate_linear_ownership(&store, &waves, &mut Default::default())
            .await
            .unwrap();
        assert_eq!(
            store
                .sqlite
                .list_projects(Some(waves[0].id()))
                .unwrap()
                .len(),
            1
        );
    }

    #[tokio::test]
    async fn chapter_deletion_stays_absent_from_wave_and_roadmap_planning() {
        let directory = tempfile::tempdir().unwrap();
        let database = directory.path().join("registry.db");
        let store = Arc::new(
            crate::store::open_ephemeral_store(&crate::store::StorageConfig::sqlite(
                database.clone(),
            ))
            .await
            .unwrap(),
        );
        let wave = Wave::new(
            crate::id::WaveId::new(),
            "product".into(),
            directory.path().display().to_string(),
        );
        store.create_wave(&wave).await.unwrap();
        let mut item = crate::linear::LinearIssue {
            branch_name: None,
            revision: None,
            id: "removed".into(),
            identifier: "FIX-1".into(),
            url: None,
            name: "Untouched backlog".into(),
            description: String::new(),
            rank: 1,
            completed: false,
            completed_at: None,
            due_date: None,
            state: Some("unstarted".into()),
            project_id: Some("current".into()),
            team_id: Some("team".into()),
            assignee: None,
        };
        let mut items = vec![item.clone()];
        item.id = "started".into();
        item.identifier = "FIX-2".into();
        item.state = Some("started".into());
        items.push(item);
        let snapshot = crate::test_linear_input::InventoryFixture {
            wave_id: wave.id().clone(),
            provider: "linear".into(),
            initiative: "initiative".into(),
            synced_at: 1,
            snapshot: serde_json::from_value(serde_json::json!({"projects":[{
                "id":"current", "slug":"current", "name":"Current chapter", "summary":"",
                "metric_targets":[], "workflow":"feature", "status":"started", "krs":[],
                "initiative_ids":["initiative"], "team_ids":["team"]
            }], "items":items}))
            .unwrap(),
        };
        store.seed_inventory(snapshot.clone(), None).await.unwrap();
        // Old applied abandonment receipts do not imply native deletion.
        let shared = super::SharedTaskReads::read(&store).await.unwrap();
        let before = super::wave_tasks(&store, &wave, false, None, &shared)
            .await
            .unwrap();
        assert!(matches!(before.tasks, super::Evidence::Ok { items, .. } if items.len() == 2));
        let removed = store.get_task_by_issue("FIX-1").await.unwrap().unwrap();
        store.sqlite.delete_task(&removed.id).unwrap();
        for project_id in ["current", "next"] {
            let mut stale = snapshot.clone();
            if project_id == "next" {
                let mut successor = stale.snapshot.projects[0].clone();
                successor.id = "next".into();
                successor.slug = "next".into();
                successor.name = "Next chapter".into();
                stale.snapshot.projects[0].status = crate::work::project::ProjectStatus::Completed;
                stale.snapshot.projects[0].revision = Some("2026-09-30T00:00:01Z".into());
                stale.snapshot.projects.push(successor);
                for item in &mut stale.snapshot.items {
                    item.project_id = Some(project_id.to_string());
                    item.revision = Some("2026-09-30T00:00:01Z".into());
                }
            }
            store.seed_inventory(stale, None).await.unwrap();
            let reopened = Arc::new(
                crate::store::open_ephemeral_store(&crate::store::StorageConfig::sqlite(
                    database.clone(),
                ))
                .await
                .unwrap(),
            );
            let detail = super::wave_tasks(&reopened, &wave, false, None, &shared)
                .await
                .unwrap();
            let super::Evidence::Ok { items, .. } = detail.tasks else {
                panic!("planning unavailable");
            };
            assert_eq!(items.len(), 1);
            assert!(detail.unavailable_tasks.is_empty());
            assert_eq!(items[0].task.linear_id.as_deref(), Some("started"));
        }
    }

    #[tokio::test]
    async fn wave_reads_owned_plan_and_preserves_invalid_provider_evidence() {
        let directory = tempfile::tempdir().unwrap();
        let store = Arc::new(
            crate::store::open_ephemeral_store(&crate::store::StorageConfig::sqlite(
                directory.path().join("registry.db"),
            ))
            .await
            .unwrap(),
        );
        let wave = Wave::new(
            crate::id::WaveId::new(),
            "product".into(),
            directory.path().display().to_string(),
        );
        store.create_wave(&wave).await.unwrap();
        assert!(matches!(
            super::project_planning(&store, &wave).await,
            super::Evidence::Ok { items, truncated: false } if items.is_empty()
        ));
        let missing =
            crate::ops::metrics::wave_metric_portfolio(&store, &wave, OffsetDateTime::now_utc())
                .await
                .unwrap();
        assert!(missing.metrics.is_empty());
        assert!(matches!(
            &missing.contract_issues[..],
            [crate::work::wave::metrics::MetricContractIssueDto::ChapterUnavailable { .. }]
        ));
        store.seed_inventory(crate::test_linear_input::InventoryFixture {
            wave_id: wave.id().clone(), provider: "linear".into(), initiative: "initiative".into(), synced_at: 2,
            snapshot: serde_json::from_value(serde_json::json!({"projects":[{
                "id":"999bdbdd-c045-41a6-8ffc-a97c4a40b0b3", "slug":"first", "name":"First chapter", "summary":"",
                "metric_targets":[], "workflow":"feature", "status":"started", "krs":[{"text":"Edited proof", "holds":false}],
                "initiative_ids":["initiative"], "team_ids":["team"]
            }], "items":[]})).unwrap(),
        }, None).await.unwrap();
        crate::store::sqlite::project_selection::write_project_binding(
            &store.sqlite,
            wave.id(),
            None,
            "999bdbdd-c045-41a6-8ffc-a97c4a40b0b3",
            &crate::store::PlanningLocks::new(tempfile::tempfile().unwrap()),
        )
        .unwrap();
        let super::Evidence::Ok { items, .. } = super::project_planning(&store, &wave).await else {
            panic!("Project plan unavailable");
        };
        assert!(items[0].current);
        assert_eq!(items[0].krs[0].text, "Edited proof");
        assert_eq!(
            items[0].status,
            crate::work::project::ProjectStatus::Started
        );
        let unreadable = crate::test_linear_input::InventoryFixture {
            wave_id: wave.id().clone(),
            provider: "linear".into(),
            initiative: "initiative".into(),
            synced_at: 3,
            snapshot: crate::test_linear_input::Inventory {
                projects: vec![],
                items: vec![],
            },
        };
        assert!(store.seed_inventory(unreadable, None).await.is_err());
        assert!(matches!(
            super::project_planning(&store, &wave).await,
            super::Evidence::Ok { items, .. } if items.len() == 1
        ));
        // Invalid provider evidence cannot erase the owned plan, but prevents metric evaluation.
        rusqlite::Connection::open(directory.path().join("registry.db"))
            .unwrap()
            .execute("UPDATE pm_projects SET initiative_ids='not-json'", [])
            .unwrap();
        let super::Evidence::Ok {
            items,
            truncated: false,
        } = super::project_planning(&store, &wave).await
        else {
            panic!("saved Project plan unavailable");
        };
        assert_eq!(items.len(), 1);
        assert!(items[0].current);
        assert_eq!(items[0].krs[0].text, "Edited proof");
        assert_eq!(
            items[0].status,
            crate::work::project::ProjectStatus::Started
        );
        let missing =
            crate::ops::metrics::wave_metric_portfolio(&store, &wave, OffsetDateTime::now_utc())
                .await
                .unwrap();
        assert!(matches!(
            &missing.contract_issues[..],
            [crate::work::wave::metrics::MetricContractIssueDto::ChapterUnavailable { .. }]
        ));
    }

    #[tokio::test]
    async fn configured_project_selection_retains_predecessor_backlog() {
        let directory = tempfile::tempdir().unwrap();
        let store = Arc::new(
            crate::store::open_ephemeral_store(&crate::store::StorageConfig::sqlite(
                directory.path().join("loopflow.db"),
            ))
            .await
            .unwrap(),
        );
        let wave = Wave::new(
            crate::id::WaveId::new(),
            "product".into(),
            directory.path().display().to_string(),
        );
        store.create_wave(&wave).await.unwrap();
        let selected = "999bdbdd-c045-41a6-8ffc-a97c4a40b0b3";
        let predecessor = "218967b6-a760-4b7c-9a46-11d9d61a42c2";
        let project = |id: &str, name: &str| crate::linear::LinearProject {
            description: None,
            id: id.into(),
            name: name.into(),
            slug: name.into(),
            summary: String::new(),
            revision: None,
            status: crate::work::project::ProjectStatus::Started,
            metric_targets: Vec::new(),
            krs: Vec::new(),
            initiative_ids: vec!["initiative".into()],
            team_ids: vec!["team".into()],
        };
        let item = crate::linear::LinearIssue {
            id: "issue".into(),
            identifier: "FIX-1".into(),
            name: "Unreviewed backlog".into(),
            description: String::new(),
            rank: 0,
            completed: false,
            completed_at: None,
            due_date: None,
            state: Some("unstarted".into()),
            project_id: Some(predecessor.into()),
            team_id: Some("team".into()),
            assignee: None,
            branch_name: None,
            revision: None,
            url: None,
        };
        store
            .seed_inventory(
                crate::test_linear_input::InventoryFixture {
                    wave_id: wave.id().clone(),
                    provider: "linear".into(),
                    initiative: "initiative".into(),
                    synced_at: 2,
                    snapshot: crate::test_linear_input::Inventory {
                        projects: vec![
                            project(predecessor, "Old ordinary plan"),
                            project(selected, "Summer work — customer requests"),
                        ],
                        items: vec![item.clone()],
                    },
                },
                None,
            )
            .await
            .unwrap();
        assert!(crate::ops::project::current_project(&store, &wave).is_err());
        assert!(!directory.path().join("waves").exists());
        let guard = crate::store::PlanningLocks::new(tempfile::tempfile().unwrap());
        crate::store::sqlite::project_selection::write_project_binding(
            &store.sqlite,
            wave.id(),
            None,
            selected,
            &guard,
        )
        .unwrap();
        let before = (
            store.list_projects(Some(wave.id())).await.unwrap(),
            store.list_tasks(Some(wave.id())).await.unwrap(),
        );
        for _ in 0..2 {
            let current = crate::ops::project::current_project(&store, &wave).unwrap();
            assert_eq!(current.linear_id.as_ref().unwrap().as_str(), selected);

            let super::Evidence::Ok {
                items: projects, ..
            } = super::project_planning(&store, &wave).await
            else {
                panic!("planning unavailable")
            };
            assert_eq!(projects.len(), 2);
            assert_eq!(
                projects
                    .iter()
                    .filter(|p| p.current)
                    .map(|p| p.id.as_str())
                    .collect::<Vec<_>>(),
                vec![current.id.as_str()]
            );
            let tasks = super::wave_tasks(
                &store,
                &wave,
                false,
                None,
                &super::SharedTaskReads::read(&store).await.unwrap(),
            )
            .await
            .unwrap();
            let super::Evidence::Ok { items, .. } = tasks.tasks else {
                panic!("Task evidence unavailable")
            };
            assert_eq!(items.len(), 1);
            let saved = store
                .get_task_by_issue(&item.identifier)
                .await
                .unwrap()
                .unwrap();
            assert_eq!(items[0].task.id, saved.id.as_str());
            assert_eq!(items[0].task.linear_id.as_deref(), Some(item.id.as_str()));
            assert!(tasks.unavailable_tasks.is_empty());
        }
        let after = (
            store.list_projects(Some(wave.id())).await.unwrap(),
            store.list_tasks(Some(wave.id())).await.unwrap(),
        );
        assert_eq!(before, after);
        // An unaccepted UUID cannot become a selection or silently pick another Project.
        assert!(
            crate::store::sqlite::project_selection::write_project_binding(
                &store.sqlite,
                wave.id(),
                Some(selected),
                "5a3aaee8-a95a-4726-9578-22a4700270ac",
                &guard,
            )
            .is_err()
        );
        let retained = super::wave_tasks(
            &store,
            &wave,
            false,
            None,
            &super::SharedTaskReads::read(&store).await.unwrap(),
        )
        .await
        .unwrap();
        assert!(
            matches!(retained.tasks, super::Evidence::Ok { items, .. } if items.len() == 1 && items[0].task.linear_id.as_deref() == Some(item.id.as_str()))
        );
    }

    #[test]
    fn repository_columns_keep_the_distinguishing_path_tail() {
        assert_eq!(
            truncate_start("/long/shared/prefix/alpha", 10),
            "\u{2026}fix/alpha"
        );
        assert_eq!(truncate_start("beta", 10), "beta");
    }

    #[test]
    fn text_metrics_preserve_values_when_chapter_targets_are_unavailable() {
        let fixture: MetricPortfolioDto = serde_json::from_str(include_str!(
            "../../../../../tests/fixtures/dto/metric_portfolio.json"
        ))
        .unwrap();
        let observed = fixture
            .metrics
            .iter()
            .find(|metric| {
                matches!(
                    metric.evidence,
                    MetricEvidenceDto::Unknown {
                        cause: MetricUnknownCauseDto::TargetUnavailable { .. }
                    }
                )
            })
            .unwrap();
        for cause in [
            MetricUnknownCauseDto::TargetUnavailable {
                value: 1.0,
                source_window_start: OffsetDateTime::UNIX_EPOCH,
                source_window_end: OffsetDateTime::UNIX_EPOCH,
            },
            MetricUnknownCauseDto::StaleObservation {
                value: 1.0,
                source_window_start: OffsetDateTime::UNIX_EPOCH,
                source_window_end: OffsetDateTime::UNIX_EPOCH,
            },
            MetricUnknownCauseDto::Never,
        ] {
            let never = matches!(cause, MetricUnknownCauseDto::Never);
            let mut metric = observed.clone();
            metric.evidence = MetricEvidenceDto::Unknown { cause };
            let portfolio = MetricPortfolioDto {
                contract_issues: vec![
                    crate::work::wave::metrics::MetricContractIssueDto::ChapterUnavailable {
                        wave_id: metric.identity.wave_id().to_string(),
                        reason: "current Project is ambiguous".into(),
                    },
                ],
                metrics: vec![metric],
            };
            let text = metric_portfolio_text(&portfolio);
            assert!(text.contains("Target unavailable for this chapter"));
            assert!(text.contains(if never { "Value -" } else { "Value 100.00%" }));
            assert!(!text.contains("unset for this chapter"));
        }
        let mut known_empty = observed.clone();
        known_empty.evidence = MetricEvidenceDto::Untargeted {
            value: 1.0,
            source_window_start: OffsetDateTime::UNIX_EPOCH,
            source_window_end: OffsetDateTime::UNIX_EPOCH,
        };
        let text = metric_portfolio_text(&MetricPortfolioDto {
            metrics: vec![known_empty],
            contract_issues: vec![
                crate::work::wave::metrics::MetricContractIssueDto::ChapterUnavailable {
                    wave_id: "another-wave".into(),
                    reason: "no saved plan".into(),
                },
            ],
        });
        assert!(text.contains("[no target]"));
        assert!(text.contains("Target unset for this chapter"));
    }

    #[test]
    fn text_metrics_group_official_signals_and_separate_candidates() {
        let metric =
            |name: &str, stage: MetricStage, evidence: MetricEvidenceDto| MetricReadingDto {
                identity: MetricIdentity::new("product", name.to_lowercase().replace(' ', "-")),
                contract_revision: "0".repeat(64),
                name: name.to_string(),
                description: "Reviewed metric meaning.".to_string(),
                stage,
                instrumented: true,
                instrument: "scorecard".to_string(),
                unit: "ratio".to_string(),
                target: Some(Threshold::AtLeast { value: 1.0 }),
                window: "7d".to_string(),
                freshness_policy: "6h".to_string(),
                freshness: MetricFreshnessDto::Never,
                evidence,
            };
        let portfolio = MetricPortfolioDto {
            metrics: vec![
                metric(
                    "Candidate",
                    MetricStage::Installed,
                    MetricEvidenceDto::Unknown {
                        cause: MetricUnknownCauseDto::Never,
                    },
                ),
                metric(
                    "Healthy",
                    MetricStage::Graduated,
                    MetricEvidenceDto::Met {
                        value: 1.0,
                        source_window_start: OffsetDateTime::UNIX_EPOCH,
                        source_window_end: OffsetDateTime::UNIX_EPOCH,
                    },
                ),
                metric(
                    "Broken",
                    MetricStage::Graduated,
                    MetricEvidenceDto::Missed {
                        value: 0.5,
                        source_window_start: OffsetDateTime::UNIX_EPOCH,
                        source_window_end: OffsetDateTime::UNIX_EPOCH,
                    },
                ),
            ],
            contract_issues: Vec::new(),
        };
        let text = metric_portfolio_text(&portfolio);

        assert!(text.contains("Owner Wave"));
        let broken = text.find("Broken  [missed]").unwrap();
        let healthy = text.find("Healthy  [met]").unwrap();
        let instrumenting = text.find("    Instrumenting\n").unwrap();
        let candidate = text.find("Candidate  [unknown]").unwrap();
        assert!(broken < healthy);
        assert!(healthy < instrumenting && instrumenting < candidate);
        assert!(!text.contains("· instrumenting"));
    }

    #[test]
    fn invalid_task_lifecycle_makes_the_user_the_next_owner() {
        let next = next_move_for_task(
            &WorkStatus::Ready,
            Some(PrPhase::Merged),
            None,
            None,
            None,
            Some("abandon INT-10 and start a replacement Task"),
        );

        assert_eq!(next.owner, NextMoveOwner::User);
        assert_eq!(next.reason, "abandon INT-10 and start a replacement Task");
    }

    #[test]
    fn a_finished_task_stays_current_only_while_execution_is_unresolved() {
        let unresolved = |status, execution: Option<&TaskExecutionSnapshot>, state, unsettled| {
            let runtime = TaskRuntimeSnapshot {
                work_id: "task-1".to_string(),
                status,
                reason: "settled".to_string(),
                updated_at: "2026-07-21T00:00:00Z".to_string(),
                provider: "codex".to_string(),
                owner: Some(owned()),
                completion_pending: None,
            };
            derive_task_condition(
                Some(&runtime),
                &NextMove {
                    owner: NextMoveOwner::Task,
                    reason: "settled".to_string(),
                },
                LocalProgressEvidence {
                    state,
                    unsettled: Some(unsettled),
                    dirty: None,
                    authored_commits: None,
                    recovery_required: Some(unsettled),
                    reason: None,
                },
                None,
                execution,
                OffsetDateTime::now_utc(),
            )
            .unresolved_execution
        };
        let review = Some(TaskExecutionSnapshot {
            state: TaskExecutionState::Blocked,
            reason: "Release target is unavailable".into(),
            step: None,
            captured: None,
        });
        let none = None;
        let missing = LocalProgressEvidenceState::Missing;
        // A removed checkout alone does not reopen settled work.
        assert!(!unresolved(TaskState::Done, none, missing, true));
        assert!(unresolved(TaskState::Moving, none, missing, true));
        // Blocked describes a stopped failed Graph, not a live Process. Later
        // Task completion must not make that retained history current forever.
        assert!(!unresolved(
            TaskState::Done,
            review.as_ref(),
            missing,
            false
        ));
        assert!(unresolved(
            TaskState::Moving,
            review.as_ref(),
            missing,
            false
        ));
        for state in [
            TaskExecutionState::Starting,
            TaskExecutionState::Running,
            TaskExecutionState::Stalled,
            TaskExecutionState::Unknown,
        ] {
            let execution = TaskExecutionSnapshot {
                state,
                reason: "Process evidence remains unresolved".into(),
                step: None,
                captured: None,
            };
            assert!(unresolved(
                TaskState::Done,
                Some(&execution),
                missing,
                false
            ));
        }
        assert!(unresolved(
            TaskState::Done,
            none,
            LocalProgressEvidenceState::Observed,
            true
        ));
    }

    #[test]
    fn task_condition_distinguishes_clear_work_from_external_waits() {
        let runtime = TaskRuntimeSnapshot {
            work_id: "task-1".to_string(),
            status: TaskState::Moving,
            reason: "ready".to_string(),
            updated_at: "2026-07-21T00:00:00Z".to_string(),
            provider: "codex".to_string(),
            owner: Some(owned()),
            completion_pending: None,
        };
        let next_move = NextMove {
            owner: NextMoveOwner::Task,
            reason: "Task is ready".to_string(),
        };
        let evidence = || LocalProgressEvidence {
            state: LocalProgressEvidenceState::Observed,
            unsettled: Some(false),
            dirty: Some(false),
            authored_commits: Some(false),
            recovery_required: Some(false),
            reason: None,
        };

        let advisory = derive_task_condition(
            Some(&runtime),
            &next_move,
            evidence(),
            None,
            None,
            OffsetDateTime::now_utc(),
        );
        assert_eq!(advisory.state, TaskConditionState::Clear);

        let delegated = derive_task_condition(
            Some(&runtime),
            &NextMove {
                owner: NextMoveOwner::Wave,
                reason: "Waiting for Project selection".to_string(),
            },
            evidence(),
            None,
            None,
            OffsetDateTime::now_utc(),
        );
        assert_eq!(delegated.state, TaskConditionState::Waiting);
        assert_eq!(delegated.reason, "Waiting for Project selection");

        let user_handoff = derive_task_condition(
            Some(&runtime),
            &NextMove {
                owner: NextMoveOwner::User,
                reason: "Merge the pull request".to_string(),
            },
            evidence(),
            None,
            None,
            OffsetDateTime::now_utc(),
        );
        assert_eq!(user_handoff.state, TaskConditionState::Waiting);
    }

    #[test]
    fn task_condition_uses_execution_evidence_before_dirty_work() {
        for (state, expected) in [
            (TaskExecutionState::Starting, TaskConditionState::Clear),
            (TaskExecutionState::Running, TaskConditionState::Clear),
            (TaskExecutionState::Blocked, TaskConditionState::Blocked),
            (TaskExecutionState::Unknown, TaskConditionState::Unknown),
        ] {
            let execution = TaskExecutionSnapshot {
                state,
                reason: "worker evidence".into(),
                step: None,
                captured: None,
            };
            let actions = TaskActionEvidence {
                status: WorkStatus::Ready,
                execution: Some(&execution),
                latest_pr_phase: None,
                latest_pr_merge_request: None,
                latest_pr_presentation_current: None,
                completion_refusal: None,
                resume_refusal: None,
                ci: None,
                predecessor_phase: None,
                launch_refusal: Some("next launch configuration is invalid"),
            };
            let condition = derive_task_condition(
                None,
                &NextMove {
                    owner: NextMoveOwner::Task,
                    reason: "ready".into(),
                },
                LocalProgressEvidence {
                    state: LocalProgressEvidenceState::Observed,
                    unsettled: Some(true),
                    dirty: Some(true),
                    authored_commits: Some(false),
                    recovery_required: Some(false),
                    reason: None,
                },
                Some(&actions),
                None,
                OffsetDateTime::now_utc(),
            );
            assert_eq!(condition.state, expected);
            assert_eq!(condition.reason, execution.reason);
            if state == TaskExecutionState::Blocked {
                for status in [TaskState::Done, TaskState::Abandoned] {
                    let runtime = TaskRuntimeSnapshot {
                        work_id: "task-1".into(),
                        status,
                        reason: "terminal".into(),
                        updated_at: "2026-07-21T00:00:00Z".into(),
                        provider: "codex".into(),
                        owner: Some(owned()),
                        completion_pending: None,
                    };
                    let terminal = derive_task_condition(
                        Some(&runtime),
                        &NextMove {
                            owner: NextMoveOwner::Wave,
                            reason: "Task is terminal".into(),
                        },
                        condition.local_progress.clone(),
                        Some(&TaskActionEvidence {
                            status: runtime.status.work_status(),
                            ..actions
                        }),
                        None,
                        OffsetDateTime::now_utc(),
                    );
                    assert_eq!(terminal.state, TaskConditionState::Blocked);
                    assert_eq!(terminal.reason, execution.reason);
                    let pending_delivery = derive_task_condition(
                        Some(&runtime),
                        &NextMove {
                            owner: NextMoveOwner::Wave,
                            reason: "Task is terminal".into(),
                        },
                        LocalProgressEvidence {
                            unsettled: Some(false),
                            dirty: Some(false),
                            ..condition.local_progress.clone()
                        },
                        Some(&TaskActionEvidence {
                            status: runtime.status.work_status(),
                            execution: None,
                            completion_refusal: Some("Pull request has not merged"),
                            ..actions
                        }),
                        None,
                        OffsetDateTime::now_utc(),
                    );
                    assert!(pending_delivery.unresolved_execution);
                }
            }
        }
    }

    #[test]
    fn settled_tasks_are_clear_with_retained_failed_history() {
        let failed = TaskExecutionSnapshot {
            state: TaskExecutionState::Blocked,
            reason: "gate: process exited with status 1".into(),
            step: None,
            captured: None,
        };
        for (status, reason) in [
            (TaskState::Done, "Task is complete"),
            (TaskState::Abandoned, "Task is abandoned"),
        ] {
            let runtime = TaskRuntimeSnapshot {
                work_id: "task-1".into(),
                status,
                reason: "terminal".into(),
                updated_at: "2026-07-21T00:00:00Z".into(),
                provider: "codex".into(),
                owner: Some(owned()),
                completion_pending: None,
            };
            let condition = derive_task_condition(
                Some(&runtime),
                &NextMove {
                    owner: NextMoveOwner::Wave,
                    reason: "done".into(),
                },
                LocalProgressEvidence {
                    state: LocalProgressEvidenceState::Missing,
                    unsettled: Some(false),
                    dirty: None,
                    authored_commits: None,
                    recovery_required: Some(true),
                    reason: Some("Task worktree is missing".into()),
                },
                Some(&TaskActionEvidence {
                    status: status.work_status(),
                    execution: Some(&failed),
                    latest_pr_phase: None,
                    latest_pr_merge_request: None,
                    latest_pr_presentation_current: None,
                    completion_refusal: None,
                    resume_refusal: None,
                    ci: None,
                    predecessor_phase: None,
                    launch_refusal: Some("worktree is missing"),
                }),
                Some(&failed),
                OffsetDateTime::now_utc(),
            );
            assert_eq!(condition.state, TaskConditionState::Clear);
            assert_eq!(condition.reason, reason);
        }
    }

    #[test]
    fn active_pr_delivery_does_not_turn_a_manual_handoff_into_a_blocker() {
        let local_progress = LocalProgressEvidence {
            state: LocalProgressEvidenceState::Observed,
            unsettled: Some(true),
            dirty: Some(false),
            authored_commits: Some(true),
            recovery_required: Some(false),
            reason: None,
        };
        let action_evidence = TaskActionEvidence {
            status: WorkStatus::Ready,
            execution: None,
            latest_pr_phase: Some(PrPhase::Open),
            latest_pr_merge_request: None,
            latest_pr_presentation_current: Some(true),
            completion_refusal: None,
            resume_refusal: None,
            ci: None,
            predecessor_phase: None,
            launch_refusal: None,
        };
        let condition = derive_task_condition(
            None,
            &NextMove {
                owner: NextMoveOwner::User,
                reason: "Merge the pull request".to_string(),
            },
            local_progress,
            Some(&action_evidence),
            None,
            OffsetDateTime::now_utc(),
        );

        assert_eq!(condition.state, TaskConditionState::Waiting);
        assert_eq!(condition.reason, "Merge the pull request");
    }

    #[test]
    fn only_an_explicit_merge_request_owns_a_healthy_open_pr() {
        let passing = CiObservation {
            head_sha: "head-1234567890".to_string(),
            state: CiState::Passing,
            failing_checks: Vec::new(),
            observed_at: OffsetDateTime::now_utc(),
        };
        let missing_copy = next_move_for_task(
            &WorkStatus::Ready,
            Some(PrPhase::Open),
            Some(false),
            Some(&passing),
            None,
            None,
        );
        assert!(missing_copy.reason.contains("reviewer-facing PR copy"));

        let published = next_move_for_task(
            &WorkStatus::Ready,
            Some(PrPhase::Open),
            Some(true),
            Some(&passing),
            None,
            None,
        );
        assert_eq!(published.owner, NextMoveOwner::Wave);

        for (mode, owner) in [
            (PrMergeMode::User, NextMoveOwner::User),
            (PrMergeMode::Auto, NextMoveOwner::External),
        ] {
            let request = PrMergeRequest {
                mode,
                requested_at: OffsetDateTime::now_utc(),
                head_sha: passing.head_sha.clone(),
            };
            let next = next_move_for_task(
                &WorkStatus::Ready,
                Some(PrPhase::Open),
                Some(true),
                Some(&passing),
                Some(&request),
                None,
            );
            assert_eq!(next.owner, owner);
            assert!(next.reason.contains("head-1234567"));
        }
    }

    #[test]
    fn repository_named_failure_stays_with_ci() {
        let ci = CiObservation {
            head_sha: "head".to_string(),
            state: CiState::Failing,
            failing_checks: vec![crate::work::task::CiCheck {
                name: "scratch-clear".to_string(),
                url: None,
            }],
            observed_at: OffsetDateTime::now_utc(),
        };
        let request = PrMergeRequest {
            mode: PrMergeMode::User,
            requested_at: OffsetDateTime::now_utc(),
            head_sha: ci.head_sha.clone(),
        };

        let next = next_move_for_task(
            &WorkStatus::Ready,
            Some(PrPhase::Open),
            Some(true),
            Some(&ci),
            Some(&request),
            None,
        );

        assert_eq!(next.owner, NextMoveOwner::Ci);
    }

    /// A store whose Task state cannot be derived still answers the plan, with
    /// its Tasks unavailable rather than empty, and keeps the Task's Workflow.
    #[test]
    fn planning_failure_keeps_the_stored_workflow_readable() {
        let _ledger = crate::journal::TestLedgerGuard::new();
        let directory = tempfile::tempdir().unwrap();
        let database = directory.path().join("released.db");
        let conn = rusqlite::Connection::open(&database).unwrap();
        crate::store::migrations::apply_before_migration(&conn, "task_paused");
        let task = crate::durable::TaskId::new();
        let fixture = include_str!("../../../tests/fixtures/migrations/work_stream_released.sql")
            .replace("{wave}", crate::id::WaveId::new().as_str())
            .replace("{project}", crate::durable::ProjectId::new().as_str())
            .replace("{task}", task.as_str())
            .replace("{repo}", directory.path().to_str().unwrap())
            // The upgrade keeps only Workflows standing at `end`.
            .replace("}]}','demo',3)", "}]}','end',3)");
        conn.execute_batch(&fixture).unwrap();
        crate::store::migrations::upgrade_from_migration(&conn, "task_paused");
        conn.execute_batch("ALTER TABLE tasks DROP COLUMN paused")
            .unwrap();
        let frontier = crate::store::migrations::latest_applied_version_sqlite(&conn).unwrap();
        let store: crate::store::SharedStore = Arc::new(crate::store::Store {
            sqlite: crate::store::sqlite::SqliteStore::open_read_only(&database).unwrap(),
        });
        let runtime = tokio::runtime::Runtime::new().unwrap();
        let roadmap = runtime
            .block_on(super::show_roadmap(
                Some(&store),
                std::path::Path::new("/"),
                None,
                None,
                true,
                false,
            ))
            .unwrap();
        assert_eq!(roadmap.waves.len(), 1);
        let super::Evidence::Unavailable { reason } = roadmap.tasks else {
            panic!(
                "the plan's Tasks are unavailable, not empty: {:?}",
                roadmap.tasks
            );
        };
        assert!(reason.contains("no such column: t.paused"), "{reason}");
        let graph = store.sqlite.task_overview(&task).unwrap().0.graph.unwrap();
        assert_eq!(graph.name, "feature");
        assert!(
            matches!(graph.position, crate::task_graph::TaskGraphPosition::At { ref step } if step == "end")
        );
        assert_eq!(
            crate::store::migrations::latest_applied_version_sqlite(&conn).unwrap(),
            frontier
        );
    }
}
