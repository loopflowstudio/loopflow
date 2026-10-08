//! `lf wave list`, `lf wave status`, and `lf roadmap` — read the wave registry (`store`).
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
use std::sync::Mutex;

use anyhow::{anyhow, Result};
use serde::{Deserialize, Serialize};

use crate::durable::{Machine, TaskState, WorkRef, WorkStatus};
use crate::lf::commands::session_history::format_tokens;
use crate::lf::output::Colors;
use crate::ops::task_execution::{TaskExecutionSnapshot, TaskExecutionState};
use crate::pm::{PmItem, PmPortfolioValidator, PmSnapshot};
use crate::session_record::SessionHistory;
use crate::store::{open_existing_store, SharedStore};
use crate::work::project::Project;
use crate::work::task::{
    AfterMerge, CiObservation, CiState, PrMergeMode, PrMergeRequest, PrPhase, Task, TaskPr,
};
use crate::work::wave::metrics::{
    MetricContractIssueDto, MetricEvidenceDto, MetricFreshnessDto, MetricPortfolioDto,
    MetricReadingDto, MetricStage, MetricTarget, MetricUnknownCauseDto,
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
    /// Stable execution authority and its currently observed route.
    pub machine: Machine,
}

/// `lf wave status <wave>`: current planning, Task conditions and Session history.
/// Wire type; no defaults.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WaveDetailSnapshot {
    pub project_readiness: crate::store::sqlite::ProjectReadiness,
    pub wave: WaveSnapshot,
    pub projects: Evidence<ProjectSummary>,
    pub tasks: Evidence<TaskDetailSnapshot>,
    /// Wave-owned live evidence derived once by Rust for every consumer.
    pub metric_portfolio: MetricPortfolioDto,
    /// Durable Project Work that cannot join the current PM plan, including
    /// non-terminal Tasks stranded under a terminal historical Project.
    pub unavailable_tasks: Vec<UnavailableTaskEvidence>,
    /// This Wave's Machine-local Session history, newest first.
    pub history: Evidence<SessionHistory>,
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

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PmKrSummary {
    pub text: String,
    pub holds: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PmTaskSummary {
    pub id: String,
    pub identifier: String,
    pub name: String,
    pub description: String,
    pub rank: u32,
    pub completed: bool,
    pub state: Option<String>,
    pub completed_at: Option<String>,
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
pub struct DirectionSnapshot {
    pub text: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TaskRuntimeSnapshot {
    pub work_id: String,
    /// Read from the Task's Workflow position; abandoned is its own mark.
    pub status: TaskState,
    /// Linear calls the Task complete while it is active here.
    pub planning_conflict: Option<String>,
    pub reason: String,
    pub updated_at: String,
    pub provider: String,
    /// Durable evidence that work began: a started Session, a worker report or
    /// finished Flow, or a published PR. `false` means none is recorded, not
    /// proof that nothing ever ran; preparing a checkout never sets it.
    pub started: bool,
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

use crate::durable::FlowProcessDetail;
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
    /// its Flow is not idle, or its checkout holds observed unsettled work.
    /// Keeps a terminal Task in the working set.
    pub unresolved_execution: bool,
}

/// Stable references for one Task, shared verbatim by `lf wave status` and
/// `lf roadmap`. The issue URL is cached PM evidence. Workspace evidence comes
/// from the durable Task and outlives its execution and final PR.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TaskReferenceSnapshot {
    pub issue_url: Option<String>,
    pub workspace: Option<TaskWorktreeSnapshot>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TaskWorktreeSnapshot {
    pub machine_id: Option<crate::durable::MachineId>,
    pub slug: String,
    /// Full branch name from the active PR, or the last recorded PR after the
    /// Task settles. `None` is explicit for legacy Tasks with no PR record.
    pub branch: Option<String>,
    pub worktree: String,
    /// Existence on the reading Machine; unknown when the filesystem check fails.
    pub local_exists: Option<bool>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TaskDetailSnapshot {
    pub task: PmTaskSummary,
    pub reference: TaskReferenceSnapshot,
    pub runtime: Option<TaskRuntimeSnapshot>,
    pub direction: Option<DirectionSnapshot>,
    pub next_move: NextMove,
    pub condition: TaskConditionSnapshot,
    pub actions: TaskActionModel,
    pub workflow_name: Option<String>,
    pub latest_flow_process: Option<FlowProcessDetail>,
    pub execution: Option<TaskExecutionSnapshot>,
    pub run_control: TaskRunControl,
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
    pub after_merge: AfterMerge,
    pub next_slug: Option<String>,
}

impl From<&PrMergeRequest> for PrMergeRequestSnapshot {
    fn from(request: &PrMergeRequest) -> Self {
        Self {
            mode: request.mode,
            requested_at: format_time(request.requested_at)
                .expect("PR merge request timestamp formats as RFC 3339"),
            head_sha: request.head_sha.clone(),
            after_merge: request.after_merge,
            next_slug: request.next_slug.clone(),
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
    Available,
    /// Done or dormant: terminal Work and completed plan rows.
    Later,
}

/// `lf roadmap` — every Wave's plan joined to durable Work and local delivery
/// evidence, bucketed by planning section.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RoadmapSnapshot {
    /// RFC3339 time this read was taken.
    pub generated_at: String,
    pub waves: Vec<WaveRoadmap>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WaveRoadmap {
    pub project_readiness: crate::store::sqlite::ProjectReadiness,
    pub wave: WaveSnapshot,
    pub projects: Evidence<ProjectSummary>,
    pub metric_portfolio: MetricPortfolioDto,
    pub tasks: Evidence<RoadmapTask>,
    pub unavailable_tasks: Vec<UnavailableTaskEvidence>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProjectSummary {
    pub current: bool,
    pub id: String,
    pub work_id: Option<String>,
    pub slug: String,
    pub name: String,
    pub workflow: String,
    pub status: crate::pm::ProjectStatus,
    pub metric_targets: Vec<crate::pm::ChapterMetricTarget>,
    pub krs: Vec<crate::pm::PmKr>,
}

async fn project_planning(store: &SharedStore, wave: &Wave) -> Evidence<ProjectSummary> {
    let result = async {
        let personal = crate::ops::task::local_wave_plan(store, wave.id()).await?;
        let row = store.pm_snapshot(wave.id()).await?;
        let (observed, partial) = match personal.or_else(|| row.map(|row| row.snapshot)) {
            Some(plan) => (plan.projects, false),
            None => {
                let projects = store.sqlite.accepted_projects(wave.id())?;
                if projects.is_empty() {
                    return Err(anyhow!("Project planning has not been synced"));
                }
                (projects, true)
            }
        };
        let current = crate::store::sqlite::project_selection::read_project_binding(
            &store.sqlite,
            wave.id(),
        )?;
        let registered = store.list_projects(Some(wave.id())).await?;
        let projects = observed
            .into_iter()
            .map(|project| ProjectSummary {
                current: current.as_deref() == Some(project.id.as_str()),
                work_id: registered
                    .iter()
                    .find(|work| {
                        work.id.as_str() == project.id
                            || work
                                .plan
                                .linear_id
                                .as_ref()
                                .is_some_and(|id| id.as_str() == project.id)
                    })
                    .map(|work| work.id.to_string()),
                id: project.id,
                slug: project.slug,
                name: project.name,
                workflow: project.workflow,
                status: project.status,
                metric_targets: project.metric_targets,
                krs: project.krs,
            })
            .collect();
        Ok((projects, partial))
    }
    .await;
    Evidence::from_result(result)
}

fn print_projects(projects: &Evidence<ProjectSummary>) {
    match projects {
        Evidence::Unavailable { reason } => println!("  projects unavailable: {reason}"),
        Evidence::Ok { items, .. } => {
            for project in items.iter().filter(|project| project.current) {
                println!(
                    "  project   {} ({}) · workflow {}",
                    project.name, project.id, project.workflow
                );
                for kr in &project.krs {
                    println!("  [{}] {}", if kr.holds { "x" } else { " " }, kr.text);
                }
            }
        }
    }
}

/// One Task in the roadmap: plan row, durable Task Work when it exists, its
/// section, and its active PR. `runtime: None` is a Task nobody has started.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RoadmapTask {
    pub task: PmTaskSummary,
    pub reference: TaskReferenceSnapshot,
    pub runtime: Option<TaskRuntimeSnapshot>,
    pub next_move: NextMove,
    pub condition: TaskConditionSnapshot,
    pub actions: TaskActionModel,
    pub workflow_name: Option<String>,
    pub latest_flow_process: Option<FlowProcessDetail>,
    pub execution: Option<TaskExecutionSnapshot>,
    pub run_control: TaskRunControl,
    pub active_pr: Option<PrSnapshot>,
    pub section: RoadmapSection,
}

/// `lf wave list` — every durable Wave identity the registry knows.
/// Keep only Waves whose repository matches the current working directory,
/// collapsing worktrees to their main checkout. `all` (or a cwd outside any git
/// repo, where there is nothing to scope to) returns every Wave unchanged.
fn scope_waves_to_repo(waves: Vec<Wave>, all: bool) -> Result<Vec<Wave>> {
    if all {
        return Ok(waves);
    }
    let Some(scope) = crate::repository::CanonicalRepo::current()? else {
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
        let Some(store) = open_existing_store().await.map(std::sync::Arc::new) else {
            return no_registry(json, "[]");
        };
        let snapshots = wave_snapshots(&store, all, current).await?;
        if json {
            println!("{}", serde_json::to_string(&snapshots)?);
        } else {
            print_wave_table(&snapshots);
        }
        Ok(())
    })
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
    let waves = scope_waves_to_repo(waves, all)?;
    let mut repositories = HashMap::new();
    let mut snapshots = Vec::with_capacity(waves.len());
    for wave in waves {
        let snapshot = snapshot_wave(store, &wave, &mut repositories).await?;
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

/// `lf wave status [wave]` — one Wave's current plan, Tasks and Session evidence.
pub fn status(wave: Option<&str>, json: bool) -> Result<()> {
    let rt = tokio::runtime::Runtime::new()?;
    rt.block_on(async {
        let Some(store) = open_existing_store().await.map(std::sync::Arc::new) else {
            return no_registry(json, "null");
        };
        let wave = resolve_status_wave(&store, wave).await?;
        let status = wave_detail(&store, &wave).await?;
        if json {
            println!("{}", serde_json::to_string(&status)?);
        } else {
            print_status(&status);
        }
        Ok(())
    })
}

/// One Wave's `lf wave status` reading.
pub(crate) async fn wave_detail(store: &SharedStore, wave: &Wave) -> Result<WaveDetailSnapshot> {
    let repository_waves = store
        .list_waves(Some(wave.repo()))
        .await
        .map_err(|err| anyhow!("failed to read repository Waves: {err}"))?;
    let mut repositories = HashMap::new();
    validate_pm_portfolio(store, &repository_waves, &mut repositories).await?;
    let snapshot = snapshot_wave(store, wave, &mut repositories).await?;
    let shared = SharedTaskReads::read(store).await?;
    let task_snapshots = wave_tasks(store, wave, true, None, &shared).await?;
    let metric_portfolio = crate::ops::metrics::wave_metric_portfolio(store, wave, now()).await?;
    Ok(WaveDetailSnapshot {
        project_readiness: store.sqlite.project_readiness(wave.id())?,
        history: Evidence::from_result(
            crate::lf::commands::session_history::collect_recent_history(
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
    })
}

/// `lf roadmap [wave]` — the machine-wide intent plane. Every Wave (or one, when
/// scoped) with its plan joined to live evidence and each row bucketed into a
/// section. Deterministic and local: one runtime observation for the whole
/// read, bounded Git probes for Task Work, and no network. `lf wave status`
/// answers "is it healthy"; this answers "what is being worked on and what
/// could be".
pub fn roadmap(wave: Option<&str>, task: Option<&str>, json: bool, all: bool) -> Result<()> {
    let include_history = wave.is_some() || task.is_some();
    let rt = tokio::runtime::Runtime::new()?;
    rt.block_on(async {
        let evaluation_time = now();
        let Some(store) = open_existing_store().await.map(std::sync::Arc::new) else {
            if task.is_some() {
                anyhow::bail!("Task lookup unavailable: local registry could not be opened");
            }
            let roadmap = RoadmapSnapshot {
                generated_at: format_time(evaluation_time)
                    .expect("current timestamp formats as RFC 3339"),
                waves: Vec::new(),
            };
            if json {
                println!("{}", serde_json::to_string(&roadmap)?);
            } else {
                print_roadmap(&roadmap);
            }
            return Ok(());
        };
        // An explicit all-repositories query must not inherit the Wave of
        // the process that launched the GUI. An explicit --wave still wins.
        let env_wave_id = if all || task.is_some() {
            None
        } else {
            std::env::var(crate::work::wave::context::WAVE_ID_ENV).ok()
        };
        let repo = crate::repo::find_repo_root().ok();
        let waves = match crate::work::wave::context::resolve_managed_wave(
            Some(&store),
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
                    let scope =
                        crate::repository::CanonicalRepo::discover(&std::env::current_dir()?)?;
                    waves
                        .into_iter()
                        .filter(|wave| scope.contains(Path::new(wave.repo())))
                        .collect()
                } else {
                    scope_waves_to_repo(waves, all)?
                }
            }
            Err(other) => return Err(anyhow!(other)),
        };
        let roadmap =
            roadmap_snapshot(&store, waves, include_history, task, evaluation_time).await?;
        if json {
            println!("{}", serde_json::to_string(&roadmap)?);
        } else {
            print_roadmap(&roadmap);
        }
        Ok(())
    })
}

/// `lf roadmap --all`: every current Wave in every repository.
pub(crate) async fn roadmap_all(store: &SharedStore) -> Result<RoadmapSnapshot> {
    let waves = store
        .list_waves(None)
        .await
        .map_err(|err| anyhow!("failed to read wave registry: {err}"))?;
    roadmap_snapshot(store, waves, false, None, now()).await
}

async fn roadmap_snapshot(
    store: &SharedStore,
    waves: Vec<Wave>,
    include_history: bool,
    task: Option<&str>,
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
    let mut repositories = HashMap::new();
    validate_pm_portfolio(store, &ownership_waves, &mut repositories).await?;
    let shared = SharedTaskReads::read(store).await?;
    let mut roadmaps = Vec::with_capacity(waves.len());
    for wave in &waves {
        let snapshot = snapshot_wave(store, wave, &mut repositories).await?;
        if !include_history && !current_wave(&snapshot) {
            continue;
        }
        let task_snapshots = wave_tasks(store, wave, false, task, &shared)
            .await
            .unwrap_or_else(|error| WaveTasks {
                tasks: Evidence::Unavailable {
                    reason: error.to_string(),
                },
                unavailable_tasks: Vec::new(),
            });
        if task.is_some()
            && matches!(&task_snapshots.tasks, Evidence::Ok { items, .. } if items.is_empty())
        {
            continue;
        }
        roadmaps.push(WaveRoadmap {
            project_readiness: store.sqlite.project_readiness(wave.id())?,
            wave: snapshot,
            projects: project_planning(store, wave).await,
            tasks: match task_snapshots.tasks {
                Evidence::Ok { items, truncated } => Evidence::Ok {
                    items: items.into_iter().map(roadmap_task).collect(),
                    truncated,
                },
                Evidence::Unavailable { reason } => Evidence::Unavailable { reason },
            },
            unavailable_tasks: task_snapshots.unavailable_tasks,
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
    })
}

/// What a reading asks once and every Task's detail then shares, so the cost
/// of a plan does not grow by a statement per Task.
#[derive(Debug)]
struct SharedTaskReads {
    checkouts: Vec<crate::store::sqlite::TaskCheckout>,
    local_machine: crate::durable::MachineId,
}

impl SharedTaskReads {
    async fn read(store: &SharedStore) -> Result<Self> {
        let checkouts = store.task_checkouts().await?;
        ask_checkouts_ahead(&checkouts);
        Ok(Self {
            checkouts,
            local_machine: store.local_machine().await?.id,
        })
    }
}

/// Git is asked about each existing checkout in turn by the Task details. A
/// process that retains answers asks about several checkouts at once first,
/// so the details find them waiting; any other process would only ask twice.
fn ask_checkouts_ahead(checkouts: &[crate::store::sqlite::TaskCheckout]) {
    const AT_ONCE: usize = 8;
    if !crate::engine::git::retains_reads() {
        return;
    }
    let existing = checkouts
        .iter()
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
                    let _ = crate::engine::git::is_clean(worktree);
                    let _ = crate::engine::git::rev_parse(worktree, "HEAD");
                    let _ = crate::engine::git::worktree_root(worktree);
                    let _ = crate::engine::worktrees::git_common_dir(worktree);
                }
            });
        }
    });
}

#[derive(Debug)]
struct WaveTasks {
    tasks: Evidence<TaskDetailSnapshot>,
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
    let mut tasks = store.list_tasks(Some(wave.id())).await?;
    // Selection does not discard other Projects' backlog or ongoing work.
    let planning_read = match crate::ops::task::local_wave_plan(store, wave.id()).await? {
        Some(plan) => Ok(Some(plan)),
        None => store
            .pm_snapshot(wave.id())
            .await
            .map(|row| row.map(|row| row.snapshot)),
    };
    let (mut planning, unavailable) = match planning_read {
        Ok(Some(planning)) => (planning, None),
        result => (
            PmSnapshot {
                projects: Vec::new(),
                items: Vec::new(),
            },
            Some(match result {
                Err(error) => error.to_string(),
                _ => format!(
                    "no local Project plan; run `lf repo refresh {}`",
                    wave.slug()
                ),
            }),
        ),
    };
    if let Some(identifier) = identifier {
        tasks.retain(|task| {
            task.plan.identifier == identifier
                || task.id.as_str() == identifier
                || task
                    .plan
                    .linear_id
                    .as_ref()
                    .is_some_and(|id| id.as_str() == identifier)
        });
        planning
            .items
            .retain(|item| item.identifier == identifier || item.id == identifier);
        // A failed read cannot establish that an unregistered planning Task is absent.
        if let Some(reason) = unavailable.as_ref().filter(|_| tasks.is_empty()) {
            return Ok(WaveTasks {
                tasks: Evidence::Unavailable {
                    reason: reason.clone(),
                },
                unavailable_tasks: Vec::new(),
            });
        }
    }
    let (details, unavailable_tasks) = snapshot_tasks(
        store,
        projects,
        tasks,
        planning,
        probe_pr_empty,
        identifier.is_some(),
        shared,
    )
    .await?;
    Ok(WaveTasks {
        tasks: match unavailable {
            Some(reason) if identifier.is_none() => Evidence::Unavailable { reason },
            _ => Evidence::complete(details),
        },
        unavailable_tasks,
    })
}

fn roadmap_task(detail: TaskDetailSnapshot) -> RoadmapTask {
    let section = task_section(&detail);
    let active_pr = detail
        .active_pr
        .as_ref()
        .and_then(|id| detail.prs.iter().find(|pr| &pr.id == id).cloned());
    RoadmapTask {
        task: detail.task,
        reference: detail.reference,
        runtime: detail.runtime,
        next_move: detail.next_move,
        condition: detail.condition,
        actions: detail.actions,
        workflow_name: detail.workflow_name,
        latest_flow_process: detail.latest_flow_process,
        execution: detail.execution,
        run_control: detail.run_control,
        active_pr,
        section,
    }
}

/// A Task's section, from the same planning primitives the row already carries.
fn task_section(task: &TaskDetailSnapshot) -> RoadmapSection {
    let Some(runtime) = &task.runtime else {
        return if crate::pm::terminal_reason(task.task.state.as_deref(), task.task.completed)
            .is_some()
        {
            RoadmapSection::Later
        } else {
            RoadmapSection::Available
        };
    };
    // Linear completing a Task that never left `start` withdraws it from what
    // is offered; an active one keeps its place and shows the conflict.
    let withdrawn = matches!(runtime.status, TaskState::NotReady | TaskState::Ready)
        && crate::pm::terminal_reason(task.task.state.as_deref(), task.task.completed).is_some();
    if runtime.status.is_terminal() || withdrawn {
        return RoadmapSection::Later;
    }
    match task.next_move.owner {
        NextMoveOwner::Task => RoadmapSection::Now,
        _ => RoadmapSection::Waiting,
    }
}

/// The wave `lf wave status` is about: the name the caller typed, else the wave this
/// process is running inside.
async fn resolve_status_wave(store: &SharedStore, requested: Option<&str>) -> Result<Wave> {
    // One shared rule for `--wave` and ambient `LF_WAVE_ID`: durable UUID or
    // repository-scoped registered name. Status consumes the resolved row
    // directly, so no second lookup can cross repositories.
    let repo = crate::repo::find_repo_root().ok();
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
fn wave_repository(wave: &Wave, repositories: &mut HashMap<String, PathBuf>) -> PathBuf {
    repositories
        .entry(wave.repo().to_string())
        .or_insert_with(|| {
            crate::engine::worktrees::main_repo_root(Path::new(wave.repo()))
                .unwrap_or_else(|_| Path::new(wave.repo()).to_path_buf())
        })
        .clone()
}

/// Build the registry snapshot for one wave, probing its discovery endpoint
/// for liveness.
pub(crate) async fn snapshot_wave(
    store: &SharedStore,
    wave: &Wave,
    repositories: &mut HashMap<String, PathBuf>,
) -> Result<WaveSnapshot> {
    let repo = wave.repo().to_string();
    let goal_repo = wave_repository(wave, repositories);
    let tasks = store
        .list_tasks(Some(wave.id()))
        .await
        .map_err(|err| anyhow!("failed to count active Tasks: {err}"))?;
    let mut active_tasks = 0;
    for task in tasks {
        let state = store.task_state(&task.id).await?;
        active_tasks += u32::from(!state.is_terminal());
    }
    let placement = store
        .placement(&WorkRef::Wave(wave.id().clone()))
        .await
        .map_err(|error| anyhow!("failed to read Wave Machine placement: {error}"))?;
    let machine = store
        .machine_by_id(&placement.machine_id)
        .await
        .map_err(|error| anyhow!("failed to read Wave Machine: {error}"))?
        .ok_or_else(|| anyhow!("Machine {} was not found", placement.machine_id))?;
    let status = store
        .work_status(&WorkRef::Wave(wave.id().clone()))
        .await
        .map_err(|error| anyhow!("failed to read Wave Work status: {error}"))?;
    Ok(WaveSnapshot {
        id: wave.id().to_string(),
        name: wave.slug().to_string(),
        status,
        goal: if let Some(definition) = store.sqlite.personal_wave_definition(wave.id())? {
            definition.goal
        } else if wave.is_retired() {
            wave.slug().to_string()
        } else {
            crate::work::wave::config::read_wave_summary(&goal_repo, wave.slug())
                .unwrap_or_else(|_| wave.slug().to_string())
        },
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
    planning_conflict: Option<String>,
    started: bool,
) -> TaskRuntimeSnapshot {
    let config = crate::engine::config::load_config_or_default(task.worktree.as_deref());
    let (provider, _) = crate::engine::config::parse_agent(config.agent());
    TaskRuntimeSnapshot {
        work_id: task.id.to_string(),
        reason: if status.is_terminal() {
            status.label().to_string()
        } else {
            execution.reason.clone()
        },
        status,
        planning_conflict,
        updated_at: format_time(task.updated_at).unwrap_or_default(),
        provider,
        started,
    }
}

async fn validate_pm_portfolio(
    store: &SharedStore,
    waves: &[Wave],
    repositories: &mut HashMap<String, PathBuf>,
) -> Result<()> {
    let mut ownership = std::collections::HashMap::<_, PmPortfolioValidator>::new();
    for wave in waves {
        let repo = wave_repository(wave, repositories);
        let repo = std::fs::canonicalize(&repo).unwrap_or(repo);
        let row = match store.pm_snapshot(wave.id()).await {
            Ok(Some(row)) => row,
            // Each Wave reports its own unavailable planning. A malformed
            // entity must not hide that Wave's execution or readable siblings.
            Ok(None) | Err(_) => continue,
        };
        let planning = row.snapshot;
        let expected_team = crate::ops::pm::repository_team_for_snapshot_validation(&repo)?;
        ownership.entry(repo).or_default().validate(
            wave.slug(),
            &row.initiative,
            expected_team.as_deref(),
            &planning.projects,
            &planning.items,
        )?;
    }
    Ok(())
}

async fn snapshot_tasks(
    store: &SharedStore,
    projects: Vec<Project>,
    tasks: Vec<Task>,
    planning: PmSnapshot,
    probe_pr_empty: bool,
    include_retained: bool,
    shared: &SharedTaskReads,
) -> Result<(Vec<TaskDetailSnapshot>, Vec<UnavailableTaskEvidence>)> {
    let mut requests = Vec::new();
    let mut unavailable_tasks = Vec::new();
    for item in planning.items {
        let task = tasks.iter().find(|task| {
            task.id.as_str() == item.id
                || task
                    .plan
                    .linear_id
                    .as_ref()
                    .is_some_and(|id| id.as_str() == item.id)
                || task.plan.identifier == item.identifier
        });
        let recommended = recommended_flow(&planning.projects, item.project_id.as_deref());
        requests.push(TaskDetailRequest {
            item,
            task,
            recommended,
        });
    }

    for task in &tasks {
        if requests.iter().any(|request| {
            task.plan
                .linear_id
                .as_ref()
                .is_some_and(|id| id.as_str() == request.item.id)
                || request.item.identifier == task.plan.identifier
        }) {
            continue;
        }
        let status = store.task_state(&task.id).await?;
        let parent = projects
            .iter()
            .find(|project| project.id == task.project_id);
        let current_plan = parent.and_then(|parent| {
            planning.projects.iter().find(|plan| {
                parent
                    .plan
                    .linear_id
                    .as_ref()
                    .is_some_and(|id| id.as_str() == plan.id)
            })
        });
        if current_plan.is_none() {
            if include_retained || !status.is_terminal() {
                unavailable_tasks.push(unavailable_task(task, status));
            }
            if !include_retained {
                continue;
            }
        }
        let parent = parent
            .ok_or_else(|| anyhow!("Task {} has no owning Project {}", task.id, task.project_id))?;
        let item = PmItem {
            branch_name: None,
            revision: None,
            id: task.plan.linear_id()?.as_str().to_string(),
            identifier: task.plan.identifier.clone(),
            url: None,
            name: task.plan.title.clone(),
            description: task.plan.description.clone(),
            rank: u32::MAX,
            // Missing planning is unknown, even when local execution has settled.
            completed: false,
            completed_at: None,
            state: None,
            project_id: Some(parent.plan.linear_id()?.as_str().to_string()),
            project: Some(parent.plan.slug.clone()),
            team_id: None,
            assignee: None,
        };
        let recommended = current_plan
            .map_or("", |plan| plan.workflow.as_str())
            .to_string();
        requests.push(TaskDetailRequest {
            item,
            task: Some(task),
            recommended,
        });
    }
    let mut details = snapshot_task_details(store, requests, probe_pr_empty, shared)?;
    details.sort_by(|left, right| {
        left.task
            .completed
            .cmp(&right.task.completed)
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

struct TaskDetailRequest<'a> {
    item: PmItem,
    task: Option<&'a Task>,
    recommended: String,
}

/// Each detail waits on fresh Git observations of its own checkout. Gather
/// them side by side: one after another, those children are most of a
/// roadmap read. Details and the first error keep their request order.
fn snapshot_task_details(
    store: &SharedStore,
    requests: Vec<TaskDetailRequest<'_>>,
    probe_pr_empty: bool,
    shared: &SharedTaskReads,
) -> Result<Vec<TaskDetailSnapshot>> {
    // `git status` is parallel itself: past four, overlapping children cost
    // more CPU than the wall time they save.
    const MAX_WORKERS: usize = 4;
    let runtime = tokio::runtime::Handle::current();
    let workers = std::thread::available_parallelism()
        .map_or(1, std::num::NonZero::get)
        .min(MAX_WORKERS)
        .min(requests.len());
    let requests: Vec<_> = requests
        .into_iter()
        .map(|request| Mutex::new(Some(request)))
        .collect();
    let next = AtomicUsize::new(0);
    let mut details: Vec<_> = std::thread::scope(|scope| {
        let handles: Vec<_> = (0..workers)
            .map(|_| {
                scope.spawn(|| {
                    let mut details = Vec::new();
                    loop {
                        let index = next.fetch_add(1, Ordering::Relaxed);
                        let Some(request) = requests.get(index) else {
                            return details;
                        };
                        let request = request
                            .lock()
                            .expect("Task detail request mutex poisoned")
                            .take()
                            .expect("each Task detail request is claimed once");
                        let detail = runtime.block_on(snapshot_task_detail(
                            store,
                            request.item,
                            request.task,
                            request.recommended,
                            probe_pr_empty,
                            shared,
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

fn unavailable_task(task: &Task, status: TaskState) -> UnavailableTaskEvidence {
    const REASON: &str = "Task's owning Project is absent from the current PM snapshot";
    UnavailableTaskEvidence {
        work_id: task.id.to_string(),
        task_id: task.id.to_string(),
        task_identifier: task.plan.identifier.clone(),
        status,
        owner: NextMoveOwner::Wave,
        reason: REASON.to_string(),
        recovery: format!("lf task status {} --json", task.id),
    }
}

fn recommended_flow(projects: &[crate::pm::PmProject], project_id: Option<&str>) -> String {
    projects
        .iter()
        .find(|project| Some(project.id.as_str()) == project_id)
        .map_or("", |project| project.workflow.as_str())
        .to_string()
}

async fn snapshot_task_detail(
    store: &SharedStore,
    item: PmItem,
    task: Option<&Task>,
    recommended: String,
    probe_pr_empty: bool,
    shared: &SharedTaskReads,
) -> Result<TaskDetailSnapshot> {
    let prs = match task {
        Some(task) => store.task_prs(&task.id).await?,
        None => Vec::new(),
    };
    let latest = prs.last();
    let active = prs.iter().find(|pr| pr.is_active());
    let observed_at = now();
    let (runtime, execution, flow_record) = match task {
        Some(task) => {
            let (execution, flow_record) =
                crate::ops::task_execution::task_execution_and_flow(store, &task.id).await?;
            let status = store.task_state(&task.id).await?;
            let conflict =
                crate::ops::task::planning_conflict_of(status, &item, &task.plan.identifier);
            let started = store.task_started(&task.id).await?
                || prs
                    .iter()
                    .any(|pr| pr.publication.is_some() || pr.merge_commit.is_some());
            (
                Some(snapshot_task_runtime(
                    &execution, task, status, conflict, started,
                )),
                Some(execution),
                flow_record,
            )
        }
        None => (None, None, None),
    };
    let machine_id = task
        .and_then(|task| shared.checkouts.iter().find(|row| row.task_id == task.id))
        .and_then(|row| row.machine_id.clone());
    let reference = task_reference(&item, task, active, &prs, machine_id, &shared.local_machine);
    let worktree_blocker = match task {
        Some(task) => crate::ops::task::task_worktree_blocker(store, task).await?,
        None => None,
    };
    let launch_refusal = match (task, worktree_blocker.as_ref()) {
        (Some(task), None) => crate::ops::task::task_process_refusal(store, task).await?,
        (Some(_), Some(_)) | (None, _) => None,
    };
    let next_move = task.map(|_| {
        if let Some(execution) = execution.as_ref().filter(|execution| {
            execution.state != TaskExecutionState::Idle
                && runtime
                    .as_ref()
                    .is_some_and(|runtime| !runtime.status.is_terminal())
        }) {
            return NextMove {
                owner: match execution.state {
                    TaskExecutionState::Blocked | TaskExecutionState::Unknown => {
                        NextMoveOwner::Wave
                    }
                    _ => NextMoveOwner::Task,
                },
                reason: execution.reason.clone(),
            };
        }
        next_move_for_task(
            &runtime
                .as_ref()
                .expect("Task runtime exists when the durable Task exists")
                .status
                .work_status(),
            active.map(TaskPr::phase),
            active
                .filter(|pr| pr.phase() == PrPhase::Open)
                .map(|pr| pr.presentation().is_some()),
            active.and_then(|pr| pr.fresh_ci()),
            active.and_then(TaskPr::merge_request),
            launch_refusal.as_deref(),
        )
    });
    let next_move = match next_move {
        Some(next_move) => next_move,
        None => NextMove {
            owner: NextMoveOwner::Wave,
            reason: item
                .terminal_reason()
                .unwrap_or("Task is ready to start")
                .to_string(),
        },
    };
    let local_progress =
        task_local_progress(task, runtime.as_ref(), active, worktree_blocker.as_ref());
    let completion_refusal = match (task, runtime.as_ref()) {
        (Some(task), Some(runtime)) if !runtime.status.is_terminal() => {
            crate::ops::task::task_completion_gate(store, task)
                .await?
                .refusal(&task.plan.identifier)
        }
        _ => None,
    };
    let resume_refusal = worktree_blocker
        .as_ref()
        .map(|blocker| blocker.reason.clone())
        .or_else(|| {
            task.and_then(|task| {
                crate::ops::task::no_active_pr_resume_refusal(&task.plan.identifier, active, latest)
            })
        });
    let action_evidence = match (task, runtime.as_ref()) {
        (Some(task), Some(runtime)) => {
            let predecessor_phase = match active.and_then(|pr| pr.parent_pr_id.as_ref()) {
                Some(parent_id) => store.get_task_pr(parent_id).await?.map(|pr| pr.phase()),
                None => None,
            };
            Some(TaskActionEvidence {
                status: runtime.status.work_status(),
                execution: execution.as_ref(),
                latest_pr_phase: latest.map(TaskPr::phase),
                latest_pr_after_merge: latest
                    .filter(|pr| pr.phase() == PrPhase::Merged)
                    .map(TaskPr::after_merge),
                latest_pr_merge_request: latest.and_then(TaskPr::merge_request),
                latest_pr_presentation_current: latest
                    .filter(|pr| pr.phase() == PrPhase::Open)
                    .map(|pr| pr.presentation().is_some()),
                completion_refusal: completion_refusal.as_deref(),
                resume_refusal: resume_refusal.as_deref(),
                ci: active.and_then(|pr| pr.fresh_ci()),
                predecessor_phase,
                abandon_intent: task.abandon_intent.is_some(),
                launch_refusal: launch_refusal.as_deref(),
            })
        }
        _ => None,
    };
    let condition = derive_task_condition(
        runtime.as_ref(),
        &next_move,
        local_progress,
        action_evidence.as_ref(),
        execution.as_ref(),
        observed_at,
    );
    let actions = action_evidence.as_ref().map_or_else(
        || TaskActionModel {
            recommended: None,
            reason: next_move.reason.clone(),
        },
        derive_task_actions,
    );
    let direction = match task {
        Some(task) => current_direction(store, &task.id).await?,
        None => None,
    };
    let work_status = runtime.as_ref().map(|runtime| runtime.status.work_status());
    let run_control =
        crate::ops::task_run::task_run_control(&crate::ops::task_run::TaskRunEvidence {
            status: work_status.as_ref(),
            // Linear completing a Task that never left `start` withdraws it.
            plan_terminal_reason: item.terminal_reason().filter(|_| {
                runtime.as_ref().is_none_or(|runtime| {
                    matches!(runtime.status, TaskState::NotReady | TaskState::Ready)
                })
            }),
            worktree_blocker: worktree_blocker
                .as_ref()
                .map(|blocker| blocker.reason.as_str()),
            launch_refusal: launch_refusal.as_deref(),
        });
    let workflow_name = match task {
        Some(task) => store
            .sqlite
            .workflow(&task.id)?
            .map(|workflow| workflow.definition.name),
        None => None,
    }
    .or_else(|| (!recommended.is_empty()).then_some(recommended));
    Ok(TaskDetailSnapshot {
        task: task_summary(item),
        reference,
        runtime,
        direction,
        next_move,
        condition,
        actions,
        workflow_name,
        latest_flow_process: flow_record,
        execution,
        run_control,
        prs: prs
            .iter()
            .map(|pr| {
                // PR emptiness is an execution-plane fact (`lf wave status`); it costs
                // an additional Git comparison, so `lf roadmap` opts out. The
                // Task condition already carries the progress evidence it needs.
                let empty = match (task, active) {
                    (Some(task), Some(active)) if probe_pr_empty && active.id == pr.id => {
                        task_pr_empty(task, pr)
                    }
                    _ => None,
                };
                PrSnapshot::new(pr, empty)
            })
            .collect(),
        active_pr: active.map(|pr| pr.id.to_string()),
    })
}

fn task_local_progress(
    task: Option<&Task>,
    runtime: Option<&TaskRuntimeSnapshot>,
    active_pr: Option<&TaskPr>,
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
    inspect_task_local_progress(
        &runtime
            .map(|runtime| runtime.status.work_status())
            .expect("Task runtime exists when the durable Task exists"),
        worktree,
        active_pr.map(|pr| pr.base_commit.as_str()),
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
    let dirty = match crate::engine::git::is_clean(worktree) {
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
        Some(base) => match crate::engine::git::rev_parse(worktree, "HEAD") {
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
        !runtime.status.is_terminal()
            || execution.is_some_and(|execution| execution.state != TaskExecutionState::Idle)
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
    let (state, reason) = if let Some(execution) = execution
        .filter(|execution| execution.state != TaskExecutionState::Idle)
        .filter(|_| runtime.is_none_or(|runtime| !runtime.status.is_terminal()))
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
    item: &PmItem,
    task: Option<&Task>,
    active_pr: Option<&TaskPr>,
    prs: &[TaskPr],
    machine_id: Option<crate::durable::MachineId>,
    local_machine: &crate::durable::MachineId,
) -> TaskReferenceSnapshot {
    let workspace = task.and_then(|task| {
        let task_worktree = task.worktree.as_ref()?;
        let branch = active_pr
            .or_else(|| prs.iter().max_by_key(|pr| pr.sequence))
            .map(|pr| pr.branch.clone());
        let local = machine_id.as_ref() == Some(local_machine);
        // A removed checkout has no root to resolve.
        let worktree = if local && task_worktree.is_dir() {
            crate::engine::git::worktree_root(task_worktree)
                .ok()
                .and_then(|root| root.canonicalize().ok())
                .unwrap_or_else(|| task_worktree.clone())
        } else {
            task_worktree.clone()
        };
        Some(TaskWorktreeSnapshot {
            machine_id,
            slug: task.workspace_slug.clone(),
            branch,
            worktree: worktree.display().to_string(),
            local_exists: local.then(|| worktree.try_exists().ok()).flatten(),
        })
    });
    TaskReferenceSnapshot {
        issue_url: item.url.clone(),
        workspace,
    }
}

fn task_pr_empty(task: &Task, pr: &TaskPr) -> Option<bool> {
    let worktree = task.worktree.as_ref()?;
    if !worktree.exists() {
        return None;
    }
    let clean = crate::engine::git::is_clean(worktree).ok()?;
    if !clean {
        return Some(false);
    }
    let head = crate::engine::git::rev_parse(worktree, "HEAD").ok()?;
    Some(head == pr.base_commit)
}

async fn current_direction(
    store: &SharedStore,
    task_id: &crate::work::task::TaskId,
) -> Result<Option<DirectionSnapshot>> {
    let steers = store
        .task_steers(task_id)
        .await
        .map_err(|err| anyhow!("failed to read Task comments: {err}"))?;
    let text = crate::durable::render_steers(&steers);
    if text.is_empty() {
        return Ok(None);
    }
    Ok(Some(DirectionSnapshot { text }))
}

fn task_summary(item: PmItem) -> PmTaskSummary {
    PmTaskSummary {
        id: item.id,
        identifier: item.identifier,
        name: item.name,
        description: item.description,
        rank: item.rank,
        completed: item.completed,
        state: item.state,
        completed_at: item.completed_at,
        assignee: item.assignee,
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
            let repairable_failure =
                ci.state == CiState::Failing && !ci.only_land_time_preconditions();
            if repairable_failure {
                return NextMove {
                    owner: NextMoveOwner::Ci,
                    reason: ci_failure_reason(ci),
                };
            }
        }
        if let Some(request) = merge {
            if ci
                .is_none_or(|ci| ci.state == CiState::Pending && !ci.only_land_time_preconditions())
            {
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
            machine = truncate(&wave.machine.route, 16),
        );
    }
}

fn work_status_is_terminal(status: &WorkStatus) -> bool {
    matches!(status, WorkStatus::Done | WorkStatus::Abandoned)
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
    println!(
        "  machine      {} ({})",
        wave.machine.id, wave.machine.route
    );
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
                    .unwrap_or(if task.task.completed {
                        "completed"
                    } else {
                        "unstarted"
                    });
                println!(
                    "  {}  {:<10}  {}  {}",
                    task.task.identifier, state, task.task.name, task.next_move.reason
                );
                if let Some(workspace) = &task.reference.workspace {
                    println!("    workspace  {}  {}", workspace.slug, workspace.worktree);
                }
                if let Some(url) = &task.reference.issue_url {
                    println!("    issue      {url}");
                }
                for pr in &task.prs {
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
            if wave_id == &metric.identity.wave_id
        )
    }) {
        return "unavailable for this chapter".into();
    }
    match metric.target {
        None => "unset for this chapter".into(),
        Some(MetricTarget::AtLeast { value }) => {
            format!(">= {}", format_metric_number(value, &metric.unit))
        }
        Some(MetricTarget::AtMost { value }) => {
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
    owner: NextMoveOwner,
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
        owner: task.next_move.owner,
        pr: task.active_pr.as_ref().map(pr_label),
        workspace: task
            .reference
            .workspace
            .as_ref()
            .map(|workspace| workspace.slug.clone()),
        condition: Some(task.condition.state),
        reason: task.condition.reason.clone(),
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
        RoadmapSection::Available => "AVAILABLE",
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
        let tasks = match &wave.tasks {
            Evidence::Unavailable { reason } => {
                println!("  tasks unavailable: {reason}");
                continue;
            }
            Evidence::Ok { items, .. } => items,
        };
        let rows: Vec<RoadmapRow> = tasks
            .iter()
            .map(|task| task_roadmap_row(task, now))
            .collect();
        if rows.is_empty() {
            println!("  (no plan rows)");
            continue;
        }
        for section in [
            RoadmapSection::Now,
            RoadmapSection::Waiting,
            RoadmapSection::Available,
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
                    "    {id}  {rank:>4}  {owner:<8}  {age:>5}  {condition:<7}  {workspace:<20}  {pr:<24}  {title}",
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
                    owner = owner_label(&row.owner),
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
        MetricReadingDto, MetricStage, MetricTarget, MetricUnknownCauseDto,
    };
    use crate::work::wave::Wave;

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
        let planning: crate::pm::PmSnapshot = serde_json::from_str(include_str!(
            "../../../../../tests/fixtures/dto/task_history_planning.json"
        ))
        .unwrap();
        store
            .put_pm_snapshot(
                crate::store::PmSnapshotRow {
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
        let stored = store.pm_snapshot(wave.id()).await.unwrap().unwrap();
        let (details, gaps) = super::snapshot_tasks(
            &store,
            vec![],
            vec![],
            stored.snapshot,
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
                .find(|item| item.id == detail.task.id)
                .unwrap();
            assert_eq!(detail.task.state, item.state);
            assert_eq!(detail.task.completed_at, item.completed_at);
            assert_eq!(detail.task.completed, item.completed);
            let terminal = item.terminal_reason();
            assert_eq!(
                matches!(super::task_section(detail), super::RoadmapSection::Later),
                terminal.is_some()
            );
            assert_eq!(detail.run_control.unavailable.as_deref(), terminal);
            if let Some(reason) = terminal {
                assert_eq!(detail.next_move.reason, reason);
                assert_eq!(detail.actions.reason, reason);
            }
        }
        let mut rows = details
            .into_iter()
            .map(super::roadmap_task)
            .collect::<Vec<_>>();
        rows.sort_by_key(|row| row.task.rank);
        for row in &mut rows {
            row.condition.observed_at = "2026-10-02T12:00:00Z".into();
        }
        let expected: serde_json::Value = serde_json::from_str(include_str!(
            "../../../../../tests/fixtures/dto/task_history_rows.json"
        ))
        .unwrap();
        assert_eq!(serde_json::to_value(rows).unwrap(), expected);
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
        let resolve = || super::wave_repository(&wave, &mut Default::default());
        assert_eq!(resolve(), alias);
        std::os::unix::fs::symlink(first.path(), &alias).unwrap();
        assert_eq!(resolve(), first.path().canonicalize().unwrap());
        std::fs::remove_file(&alias).unwrap();
        std::os::unix::fs::symlink(second.path(), &alias).unwrap();
        assert_eq!(resolve(), second.path().canonicalize().unwrap());
    }

    #[tokio::test]
    async fn portfolio_ownership_is_scoped_to_repository() {
        let directory = tempfile::tempdir().unwrap();
        let store = crate::store::open_ephemeral_store(&crate::store::StorageConfig::sqlite(
            directory.path().join("registry.db"),
        ))
        .await
        .unwrap();
        let store = Arc::new(store);
        let mut waves = Vec::new();
        for (name, repository) in [("ear", "first"), ("ear", "second"), ("duplicate", "first")] {
            let wave = Wave::new(
                crate::id::WaveId::new(),
                name.into(),
                directory.path().join(repository).display().to_string(),
            );
            store.create_wave(&wave).await.unwrap();
            store
                .put_pm_snapshot(
                    crate::store::PmSnapshotRow {
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
        super::validate_pm_portfolio(&store, &waves[..2], &mut Default::default())
            .await
            .unwrap();
        let error = super::validate_pm_portfolio(&store, &waves, &mut Default::default())
            .await
            .unwrap_err();
        assert!(error.to_string().contains("bound by both"), "{error}");
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
        let mut item = crate::pm::PmItem {
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
            state: Some("unstarted".into()),
            project_id: Some("current".into()),
            project: Some("current".into()),
            team_id: Some("team".into()),
            assignee: None,
        };
        let mut items = vec![item.clone()];
        item.id = "completed".into();
        item.identifier = "FIX-2".into();
        item.completed = true;
        item.state = Some("completed".into());
        items.push(item);
        let snapshot = crate::store::PmSnapshotRow {
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
        store.put_pm_snapshot(snapshot.clone(), None).await.unwrap();
        // Old applied abandonment receipts do not imply native deletion.
        let shared = super::SharedTaskReads::read(&store).await.unwrap();
        let before = super::wave_tasks(&store, &wave, false, None, &shared)
            .await
            .unwrap();
        assert!(matches!(before.tasks, super::Evidence::Ok { items, .. } if items.len() == 2));
        store
            .confirm_task_deletion(wave.id(), "removed", "FIX-1")
            .await
            .unwrap();
        for project_id in ["current", "next"] {
            let mut stale = snapshot.clone();
            if project_id == "next" {
                let mut successor = stale.snapshot.projects[0].clone();
                successor.id = "next".into();
                successor.slug = "next".into();
                successor.name = "Next chapter".into();
                stale.snapshot.projects[0].status = crate::pm::ProjectStatus::Completed;
                stale.snapshot.projects[0].revision = Some("2026-09-30T00:00:01Z".into());
                stale.snapshot.projects.push(successor);
                for item in &mut stale.snapshot.items {
                    item.project_id = Some(project_id.to_string());
                    item.revision = Some("2026-09-30T00:00:01Z".into());
                }
            }
            store.put_pm_snapshot(stale, None).await.unwrap();
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
            let roadmap = items
                .into_iter()
                .map(super::roadmap_task)
                .collect::<Vec<_>>();
            assert_eq!(roadmap[0].task.id, "completed");
        }
    }

    #[tokio::test]
    async fn wave_reads_project_plan_and_preserves_unavailable_evidence() {
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
            super::Evidence::Unavailable { .. }
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
        store.put_pm_snapshot(crate::store::PmSnapshotRow {
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
        assert_eq!(items[0].status, crate::pm::ProjectStatus::Started);
        let mut unreadable = store.pm_snapshot(wave.id()).await.unwrap().unwrap();
        unreadable.snapshot.projects.clear();
        unreadable.snapshot.items.clear();
        assert!(store.put_pm_snapshot(unreadable, None).await.is_err());
        assert!(matches!(
            super::project_planning(&store, &wave).await,
            super::Evidence::Ok { items, .. } if items.len() == 1
        ));
        // Corrupt the stored entity directly; ingestion rejects malformed plans.
        rusqlite::Connection::open(directory.path().join("registry.db"))
            .unwrap()
            .execute("UPDATE pm_projects SET body='not-json'", [])
            .unwrap();
        assert!(matches!(
            super::project_planning(&store, &wave).await,
            super::Evidence::Unavailable { .. }
        ));
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
        let project = |id: &str, name: &str| crate::pm::PmProject {
            id: id.into(),
            name: name.into(),
            slug: name.into(),
            summary: String::new(),
            revision: None,
            workflow: String::new(),
            status: crate::pm::ProjectStatus::Started,
            metric_targets: Vec::new(),
            krs: Vec::new(),
            initiative_ids: vec!["initiative".into()],
            team_ids: vec!["team".into()],
        };
        let item = crate::pm::PmItem {
            id: "issue".into(),
            identifier: "FIX-1".into(),
            name: "Unreviewed backlog".into(),
            description: String::new(),
            rank: 0,
            completed: false,
            completed_at: None,
            state: Some("unstarted".into()),
            project_id: Some(predecessor.into()),
            project: Some("Old ordinary plan".into()),
            team_id: Some("team".into()),
            assignee: None,
            branch_name: None,
            revision: None,
            url: None,
        };
        store
            .put_pm_snapshot(
                crate::store::PmSnapshotRow {
                    wave_id: wave.id().clone(),
                    provider: "linear".into(),
                    initiative: "initiative".into(),
                    synced_at: 2,
                    snapshot: crate::pm::PmSnapshot {
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
        let before = store.pm_snapshot(wave.id()).await.unwrap().unwrap();
        for _ in 0..2 {
            let current = crate::ops::project::current_project(&store, &wave).unwrap();
            assert_eq!(current.id, selected);
            assert!(current.workflow.is_empty());
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
                vec![selected]
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
            assert_eq!(items[0].task.id, item.id);
            assert_eq!(super::roadmap_task(items[0].clone()).task.id, item.id);
            assert!(tasks.unavailable_tasks.is_empty());
        }
        let after = store.pm_snapshot(wave.id()).await.unwrap().unwrap();
        assert_eq!(before.snapshot, after.snapshot);
        assert_eq!(before.synced_at, after.synced_at);
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
            matches!(retained.tasks, super::Evidence::Ok { items, .. } if items.len() == 1 && items[0].task.id == item.id)
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
                        wave_id: metric.identity.wave_id.clone(),
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
                identity: MetricIdentity {
                    wave_id: "product".to_string(),
                    metric_id: name.to_lowercase().replace(' ', "-"),
                },
                contract_revision: "0".repeat(64),
                name: name.to_string(),
                description: "Reviewed metric meaning.".to_string(),
                stage,
                instrumented: true,
                instrument: "scorecard".to_string(),
                unit: "ratio".to_string(),
                target: Some(MetricTarget::AtLeast { value: 1.0 }),
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
                started: true,
                planning_conflict: None,
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
        assert!(unresolved(TaskState::Active, none, missing, true));
        assert!(unresolved(TaskState::Done, review.as_ref(), missing, false));
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
            status: TaskState::Active,
            reason: "ready".to_string(),
            updated_at: "2026-07-21T00:00:00Z".to_string(),
            provider: "codex".to_string(),
            started: true,
            planning_conflict: None,
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
                latest_pr_after_merge: None,
                latest_pr_merge_request: None,
                latest_pr_presentation_current: None,
                completion_refusal: None,
                resume_refusal: None,
                ci: None,
                predecessor_phase: None,
                abandon_intent: false,
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
                        started: true,
                        planning_conflict: None,
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
                    assert_eq!(terminal.reason, "Task is terminal");
                }
            }
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
            latest_pr_after_merge: None,
            latest_pr_merge_request: None,
            latest_pr_presentation_current: Some(true),
            completion_refusal: None,
            resume_refusal: None,
            ci: None,
            predecessor_phase: None,
            abandon_intent: false,
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
                after_merge: crate::work::task::AfterMerge::ContinueTask,
                next_slug: None,
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
    fn land_only_failure_leaves_the_requested_merge_with_its_operator() {
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
            after_merge: crate::work::task::AfterMerge::ContinueTask,
            next_slug: None,
        };

        let next = next_move_for_task(
            &WorkStatus::Ready,
            Some(PrPhase::Open),
            Some(true),
            Some(&ci),
            Some(&request),
            None,
        );

        assert_eq!(next.owner, NextMoveOwner::User);
    }
}
