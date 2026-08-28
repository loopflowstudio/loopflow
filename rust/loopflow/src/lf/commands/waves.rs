//! `lf ls`, `lf status`, and `lf roadmap` — read the wave registry (`store`).
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

use std::io::IsTerminal;
use std::path::Path;

use anyhow::{anyhow, Result};
use serde::{Deserialize, Serialize};

use crate::child::{ChildRef, ObservationRecipient};
use crate::controller::wave::metrics::{
    MetricContractIssueDto, MetricEvidenceDto, MetricFreshnessDto, MetricPortfolioDto,
    MetricReadingDto, MetricStage, MetricTarget, MetricUnknownCauseDto,
};
use crate::controller::wave::server::live_endpoint;
use crate::durable::{Home, WorkRef, WorkStatus};
use crate::engine::wave_home::{HomeActionDto, HomeRuntimeDto, HomeState};
use crate::lf::commands::runs::{format_tokens, RunSnapshot};
use crate::lf::output::Colors;
use crate::pm::{PmItem, PmKr, PmPortfolioValidator, PmProject, PmSnapshot, ProjectFlowPlan};
use crate::store::{open_existing_store, SharedStore};
use crate::work::project::Project;
use crate::work::task::{
    AfterMerge, CiObservation, CiState, PrMergeMode, PrMergeRequest, PrPhase, Task, TaskPr,
};
use crate::work::wave::Wave;

/// One wave's registry snapshot — the `lf ls` row and the `wave` field of
/// `lf status`. Wire type consumed by Loopflow: every field is required or
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
    /// Non-terminal Projects owned by this Wave.
    pub active_projects: u32,
    /// Whether a wave server answered `/health` at the discovery endpoint.
    pub live: bool,
    /// Whether authored policy currently refuses new turn starts.
    pub paused: bool,
    /// Whether this Home is allowed to keep the Wave running.
    pub enabled: bool,
    /// Loopback endpoint of the live server, `null` when stopped.
    pub endpoint: Option<String>,
    /// RFC3339 creation time, `null` when the row predates the column.
    pub created_at: Option<String>,
    /// Parent wave id in the chord tree, `null` for a root wave.
    pub parent_wave_id: Option<String>,
    /// Tombstone time for stable-id history; active locator reads exclude it.
    pub retired_at: Option<String>,
    pub superseded_by_wave_id: Option<String>,
    pub retirement_reason: Option<String>,
    /// Stable execution authority and its currently observed route.
    pub home: Home,
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
    /// Project-owned live evidence derived once by Rust for every consumer.
    pub metric_portfolio: MetricPortfolioDto,
    /// Durable Project Work that cannot join the current PM plan, including
    /// non-terminal Tasks stranded under a terminal historical Project.
    pub unavailable_projects: Vec<UnavailableProjectEvidence>,
    /// This Wave's Home-local Run records, newest first.
    pub runs: Evidence<RunSnapshot>,
    /// Work whose next move belongs to someone other than itself.
    pub attention: Evidence<AttentionItem>,
    /// The Wave's Home probed for liveness: state, evidence, attach endpoint, and
    /// the one contextual action a conductor surface should offer. Probed for the
    /// focused Wave only — `lf ls` stays placement-only.
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

/// One Work item waiting on somebody, derived from durable state.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AttentionItem {
    pub kind: AttentionKind,
    /// Work id — the drill-down key.
    pub id: String,
    /// Project slug or Task identifier.
    pub subject: String,
    /// Who has to move next.
    pub owner: NextMoveOwner,
    /// Why, in Work's durable words — or the audit finding when durable state
    /// and the machine disagree.
    pub reason: String,
    /// RFC3339 time Work entered this state; empty when unrecorded.
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
    pub flows: ProjectFlowPlan,
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
    User,
    Wave,
    Project,
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
pub struct ProjectRuntimeSnapshot {
    pub work_id: String,
    pub status: WorkStatus,
    pub reason: String,
    pub updated_at: String,
    pub iteration: u32,
    pub pending_observations: u32,
    pub provider: String,
    pub last_failure: Option<crate::work::project::HistoricalFailure>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TaskRuntimeSnapshot {
    pub work_id: String,
    pub project_id: String,
    /// The live Project this Task routes to (successor when the
    /// historical owner is terminal). `None` when the chain is broken. The app
    /// derives "routed to a successor" by comparing this to `project_id`.
    pub routing_project_id: Option<String>,
    pub status: WorkStatus,
    pub reason: String,
    pub updated_at: String,
    pub provider: String,
}

/// The compact Task attention signal shared by terminal and app surfaces. The
/// names are deliberately the product's visual vocabulary: consumers do not
/// reinterpret planning and local-progress evidence into their own colors.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TaskAttentionLevel {
    Red,
    Blue,
    Black,
    Unknown,
}

pub use crate::ops::task_actions::{
    ci_failure_reason, derive_task_actions, TaskAction, TaskActionEvidence, TaskActionModel,
};

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

struct TaskAttentionEvidence {
    local_progress: LocalProgressEvidence,
    user_ask: bool,
}

/// A Task's shared attention projection and the evidence that proves it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TaskAttentionSnapshot {
    pub level: TaskAttentionLevel,
    pub reason: String,
    /// RFC3339 time local workspace evidence was sampled.
    pub observed_at: String,
    /// Age of the durable Work evidence at that sample, if Work exists.
    pub evidence_age_secs: Option<i64>,
    pub next_owner: NextMoveOwner,
    pub actions: TaskActionModel,
    pub pm_completed: bool,
    pub work_status: Option<WorkStatus>,
    pub local_progress: LocalProgressEvidence,
    pub active_pr_phase: Option<PrPhase>,
}

/// Stable references for one Task, shared verbatim by `lf status` and
/// `lf roadmap`. The issue URL is cached PM evidence. Workspace evidence comes
/// from the durable Task and outlives its execution and final PR.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TaskReferenceSnapshot {
    pub issue_url: Option<String>,
    pub workspace: Option<TaskWorkspaceSnapshot>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TaskWorkspaceSnapshot {
    pub slug: String,
    /// Full branch name from the active PR, or the last recorded PR after the
    /// Task settles. `None` is explicit for legacy Tasks with no PR record.
    pub branch: Option<String>,
    pub worktree: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TaskDetailSnapshot {
    pub task: PmTaskSummary,
    pub reference: TaskReferenceSnapshot,
    pub runtime: Option<TaskRuntimeSnapshot>,
    pub direction: Option<DirectionSnapshot>,
    pub next_move: NextMove,
    pub attention: TaskAttentionSnapshot,
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

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProjectDetailSnapshot {
    pub project: PmProjectSummary,
    pub runtime: Option<ProjectRuntimeSnapshot>,
    pub direction: Option<DirectionSnapshot>,
    pub next_move: NextMove,
    pub tasks: Vec<TaskDetailSnapshot>,
}

/// One durable Project Work row that cannot join the current PM snapshot.
/// Identity, reason, and recovery stay structured so clients never parse prose.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct UnavailableProjectEvidence {
    pub work_id: String,
    pub project_id: String,
    pub project_slug: String,
    pub status: WorkStatus,
    pub owner: NextMoveOwner,
    pub reason: String,
    pub recovery: String,
    pub tasks: Vec<UnavailableTaskEvidence>,
}

/// Non-terminal durable Task Work whose historical Project is no longer in the
/// current PM snapshot.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct UnavailableTaskEvidence {
    pub work_id: String,
    pub task_id: String,
    pub task_identifier: String,
    pub status: WorkStatus,
    pub owner: NextMoveOwner,
    pub reason: String,
    pub recovery: String,
}

/// Where a row's next move sends the reader's attention. A coarse view over
/// durable intent and next-owner evidence, derived once so every surface
/// buckets identically.
///
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RoadmapSection {
    /// This Work's own planner must move next.
    Now,
    /// Someone else must move: review, a User, or the supervising Project/Wave.
    NeedsAttention,
    /// Filed, not started, not complete — ready for someone to pick up.
    Available,
    /// Done or dormant: terminal Work and completed plan rows.
    Later,
}

/// `lf roadmap` — every Wave's plan joined to durable Work and local delivery
/// evidence, bucketed by attention section.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RoadmapSnapshot {
    /// RFC3339 time this read was taken.
    pub generated_at: String,
    pub waves: Vec<WaveRoadmap>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WaveRoadmap {
    pub wave: WaveSnapshot,
    /// The same Project-owned live evidence carried by focused Wave status.
    pub metric_portfolio: MetricPortfolioDto,
    /// The Wave's plan joined to Work evidence, or the reason there is none — a
    /// Wave with no local PM snapshot reads "unavailable", never an empty plan.
    pub projects: Evidence<RoadmapProject>,
    /// Durable Project Work that failed to join an otherwise readable plan,
    /// including non-terminal Tasks stranded under a historical Project.
    pub unavailable_projects: Vec<UnavailableProjectEvidence>,
}

#[derive(Debug)]
struct ProjectSnapshots {
    projects: Vec<ProjectDetailSnapshot>,
    unavailable_projects: Vec<UnavailableProjectEvidence>,
}

#[derive(Debug)]
struct RoadmapProjectSnapshots {
    projects: Evidence<RoadmapProject>,
    unavailable_projects: Vec<UnavailableProjectEvidence>,
    metric_portfolio: MetricPortfolioDto,
}

/// One Project in the roadmap: its plan, durable Project Work when a
/// loop exists, its section, and its Tasks. Reuses the same leaf snapshots
/// `lf status` emits; adds the derived `section`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RoadmapProject {
    pub project: PmProjectSummary,
    pub runtime: Option<ProjectRuntimeSnapshot>,
    pub next_move: NextMove,
    pub section: RoadmapSection,
    pub tasks: Vec<RoadmapTask>,
}

/// One Task in the roadmap: plan row, durable Task Work when it exists, its
/// section, and its active PR. `runtime: None` is a Task nobody has started.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RoadmapTask {
    pub task: PmTaskSummary,
    pub reference: TaskReferenceSnapshot,
    pub runtime: Option<TaskRuntimeSnapshot>,
    pub next_move: NextMove,
    pub attention: TaskAttentionSnapshot,
    pub active_pr: Option<PrSnapshot>,
    pub section: RoadmapSection,
}

/// `lf ls` — every wave the registry knows, running and stopped alike.
/// Keep only Waves whose repository matches the current working directory,
/// collapsing worktrees to their main checkout. `all` (or a cwd outside any git
/// repo, where there is nothing to scope to) returns every Wave unchanged.
fn scope_waves_to_repo(waves: Vec<Wave>, all: bool) -> Vec<Wave> {
    if all {
        return waves;
    }
    let Some(scope) = crate::repository::CanonicalRepo::current() else {
        return waves;
    };
    waves
        .into_iter()
        .filter(|wave| scope.contains(Path::new(wave.repo())))
        .collect()
}

pub fn ls(json: bool, all: bool) -> Result<()> {
    let rt = tokio::runtime::Runtime::new()?;
    rt.block_on(async {
        let Some(store) = open_existing_store().await.map(std::sync::Arc::new) else {
            return no_registry(json, "[]");
        };
        let waves = store
            .list_waves(None)
            .await
            .map_err(|err| anyhow!("failed to read wave registry: {err}"))?;
        let waves = scope_waves_to_repo(waves, all);
        let mut snapshots = Vec::with_capacity(waves.len());
        for wave in waves {
            snapshots.push(snapshot_wave(&store, &wave).await?);
        }
        snapshots.sort_by(|a, b| a.repo.cmp(&b.repo).then(a.name.cmp(&b.name)));
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
        let repository_waves = store
            .list_waves(Some(wave.repo()))
            .await
            .map_err(|err| anyhow!("failed to read repository Waves: {err}"))?;
        validate_pm_portfolio(&store, &repository_waves).await?;
        let snapshot = snapshot_wave(&store, &wave).await?;
        let loop_state = match &snapshot.endpoint {
            Some(endpoint) => loop_state(endpoint).await,
            None => None,
        };
        let stored_projects = store
            .list_projects(Some(wave.id()))
            .await
            .map_err(|err| anyhow!("failed to read Projects: {err}"))?;
        let stored_tasks = store
            .list_tasks(Some(wave.id()))
            .await
            .map_err(|err| anyhow!("failed to read Tasks: {err}"))?;
        let planning = read_pm_planning(&store, &wave)
            .await?
            .unwrap_or(PmSnapshot {
                projects: Vec::new(),
                items: Vec::new(),
            });
        let metric_portfolio = wave_metric_portfolio(&store, &wave, &planning, now()).await?;
        let project_snapshots =
            snapshot_projects(&store, stored_projects, stored_tasks, planning, true).await?;
        let attention = Evidence::complete(attention(&project_snapshots.projects, now()));
        // Probe the focused Wave's Home once so the detail carries live evidence
        // and the single contextual action (Open/Attach, Start, or reason).
        let home_runtime =
            crate::ops::home::probe_home(wave.name(), &snapshot.home, Path::new(wave.repo())).await;
        let status = WaveDetailSnapshot {
            runs: Evidence::from_result(crate::lf::commands::runs::wave_runs(wave.name())),
            attention,
            wave: snapshot,
            loop_state,
            projects: project_snapshots.projects,
            metric_portfolio,
            unavailable_projects: project_snapshots.unavailable_projects,
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

/// `lf roadmap [wave]` — the machine-wide intent plane. Every Wave (or one, when
/// scoped) with its plan joined to live evidence and each row bucketed into a
/// section. Deterministic and local: one `tmux list-sessions` for the whole
/// read, bounded Git probes for Task Work, and no network. `lf status`
/// answers "is it healthy"; this answers "what is being worked on and what
/// could be".
pub fn roadmap(wave: Option<&str>, json: bool, all: bool) -> Result<()> {
    let rt = tokio::runtime::Runtime::new()?;
    rt.block_on(async {
        let evaluation_time = now();
        let Some(store) = open_existing_store().await.map(std::sync::Arc::new) else {
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
        // The ONE ambient-Wave rule: `--wave` wins, else `LF_WAVE_ID` (durable
        // UUID or repository-scoped registered name). Roadmap is the one
        // command where `NoContext` is a valid default — it lists every wave.
        // A stale UUID is a loud error, never a silent drop to global scope.
        let env_wave_id = std::env::var(crate::work::wave::context::WAVE_ID_ENV).ok();
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
                scope_waves_to_repo(waves, all)
            }
            Err(other) => return Err(anyhow!(other)),
        };
        // A Wave filter narrows presentation, not repository ownership checks.
        let ownership_waves = if waves.len() == 1 {
            store
                .list_waves(Some(waves[0].repo()))
                .await
                .map_err(|err| anyhow!("failed to read repository Waves: {err}"))?
        } else {
            waves.clone()
        };
        validate_pm_portfolio(&store, &ownership_waves).await?;
        let mut roadmaps = Vec::with_capacity(waves.len());
        for wave in &waves {
            let snapshot = snapshot_wave(&store, wave).await?;
            let project_snapshots = wave_roadmap_projects(&store, wave, evaluation_time).await?;
            roadmaps.push(WaveRoadmap {
                wave: snapshot,
                projects: project_snapshots.projects,
                unavailable_projects: project_snapshots.unavailable_projects,
                metric_portfolio: project_snapshots.metric_portfolio,
            });
        }
        roadmaps.sort_by(|a, b| a.wave.name.cmp(&b.wave.name));
        let roadmap = RoadmapSnapshot {
            generated_at: format_time(evaluation_time)
                .expect("current timestamp formats as RFC 3339"),
            waves: roadmaps,
        };
        if json {
            println!("{}", serde_json::to_string(&roadmap)?);
        } else {
            print_roadmap(&roadmap);
        }
        Ok(())
    })
}

/// Build one Wave's roadmap projects, or the reason there are none. A missing PM
/// snapshot is `Unavailable` ("run `lf pm sync`"), never an empty plan; a read
/// that fails carries its error rather than reading as emptiness.
async fn wave_roadmap_projects(
    store: &SharedStore,
    wave: &Wave,
    evaluation_time: time::OffsetDateTime,
) -> Result<RoadmapProjectSnapshots> {
    let planning = match read_pm_planning(store, wave).await {
        Ok(Some(planning)) => planning,
        Ok(None) => {
            let planning = PmSnapshot {
                projects: Vec::new(),
                items: Vec::new(),
            };
            return Ok(RoadmapProjectSnapshots {
                projects: Evidence::Unavailable {
                    reason: format!(
                        "no local PM snapshot for wave/{}; run `lf pm sync`",
                        wave.name()
                    ),
                },
                unavailable_projects: Vec::new(),
                metric_portfolio: wave_metric_portfolio(store, wave, &planning, evaluation_time)
                    .await?,
            });
        }
        Err(err) => {
            let planning = PmSnapshot {
                projects: Vec::new(),
                items: Vec::new(),
            };
            return Ok(RoadmapProjectSnapshots {
                projects: Evidence::Unavailable {
                    reason: err.to_string(),
                },
                unavailable_projects: Vec::new(),
                metric_portfolio: wave_metric_portfolio(store, wave, &planning, evaluation_time)
                    .await?,
            });
        }
    };
    let metric_portfolio = wave_metric_portfolio(store, wave, &planning, evaluation_time).await?;
    let projects = match store.list_projects(Some(wave.id())).await {
        Ok(projects) => projects,
        Err(err) => {
            return Ok(RoadmapProjectSnapshots {
                projects: Evidence::Unavailable {
                    reason: format!("failed to read Projects: {err}"),
                },
                unavailable_projects: Vec::new(),
                metric_portfolio,
            });
        }
    };
    let tasks = match store.list_tasks(Some(wave.id())).await {
        Ok(tasks) => tasks,
        Err(err) => {
            return Ok(RoadmapProjectSnapshots {
                projects: Evidence::Unavailable {
                    reason: format!("failed to read Tasks: {err}"),
                },
                unavailable_projects: Vec::new(),
                metric_portfolio,
            });
        }
    };
    // `probe_pr_empty: false` — PR emptiness is `lf status`'s execution detail.
    // Roadmap's bounded Git reads belong only to the shared attention evidence.
    match snapshot_projects(store, projects, tasks, planning, false).await {
        Ok(snapshots) => Ok(RoadmapProjectSnapshots {
            projects: Evidence::complete(
                snapshots
                    .projects
                    .into_iter()
                    .map(roadmap_project)
                    .collect(),
            ),
            unavailable_projects: snapshots.unavailable_projects,
            metric_portfolio,
        }),
        Err(err) => Ok(RoadmapProjectSnapshots {
            projects: Evidence::Unavailable {
                reason: err.to_string(),
            },
            unavailable_projects: Vec::new(),
            metric_portfolio,
        }),
    }
}

async fn wave_metric_portfolio(
    store: &SharedStore,
    wave: &Wave,
    planning: &PmSnapshot,
    evaluation_time: time::OffsetDateTime,
) -> Result<MetricPortfolioDto> {
    crate::ops::metrics::wave_metric_portfolio(store, wave, &planning.projects, evaluation_time)
        .await
}

/// Project a `lf status` project detail into its roadmap row, deriving the
/// section for it and each of its Tasks.
fn roadmap_project(detail: ProjectDetailSnapshot) -> RoadmapProject {
    let section = project_section(&detail);
    let tasks = detail.tasks.into_iter().map(roadmap_task).collect();
    RoadmapProject {
        project: detail.project,
        runtime: detail.runtime,
        next_move: detail.next_move,
        section,
        tasks,
    }
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
        attention: detail.attention,
        active_pr,
        section,
    }
}

/// A Task's section, from the same planning primitives the row already carries.
fn task_section(task: &TaskDetailSnapshot) -> RoadmapSection {
    let Some(runtime) = &task.runtime else {
        return if task.task.completed {
            RoadmapSection::Later
        } else {
            RoadmapSection::Available
        };
    };
    if work_status_is_terminal(&runtime.status) {
        return RoadmapSection::Later;
    }
    match task.next_move.owner {
        NextMoveOwner::Task => RoadmapSection::Now,
        _ => RoadmapSection::NeedsAttention,
    }
}

/// A Project's section. An unstarted Project (no loop) is `Available` — the Wave
/// could start it — unless the plan says it is already done.
fn project_section(project: &ProjectDetailSnapshot) -> RoadmapSection {
    let Some(runtime) = &project.runtime else {
        let all_krs_hold =
            !project.project.krs.is_empty() && project.project.krs.iter().all(|kr| kr.holds);
        return if all_krs_hold {
            RoadmapSection::Later
        } else {
            RoadmapSection::Available
        };
    };
    if work_status_is_terminal(&runtime.status) {
        return RoadmapSection::Later;
    }
    match project.next_move.owner {
        NextMoveOwner::Project => RoadmapSection::Now,
        _ => RoadmapSection::NeedsAttention,
    }
}

/// The wave `lf status` is about: the name the caller typed, else the wave this
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

/// What in this Wave is waiting on somebody other than its planning owner.
fn attention(projects: &[ProjectDetailSnapshot], now: time::OffsetDateTime) -> Vec<AttentionItem> {
    let mut items = Vec::new();
    for project in projects {
        if let Some(runtime) = &project.runtime {
            let self_owned = matches!(project.next_move.owner, NextMoveOwner::Project);
            if !(self_owned || work_status_is_terminal(&runtime.status)) {
                items.push(AttentionItem {
                    kind: AttentionKind::Project,
                    id: runtime.work_id.clone(),
                    subject: project.project.slug.clone(),
                    owner: project.next_move.owner,
                    reason: runtime.reason.clone(),
                    since: runtime.updated_at.clone(),
                    age_secs: age_secs(&runtime.updated_at, now),
                });
            }
        }
        for task in &project.tasks {
            let Some(runtime) = &task.runtime else {
                continue;
            };
            if matches!(task.next_move.owner, NextMoveOwner::Task) {
                continue;
            }
            if work_status_is_terminal(&runtime.status) {
                continue;
            }
            items.push(AttentionItem {
                kind: AttentionKind::Task,
                id: runtime.work_id.clone(),
                subject: task.task.identifier.clone(),
                owner: task.next_move.owner,
                reason: runtime.reason.clone(),
                since: runtime.updated_at.clone(),
                age_secs: age_secs(&runtime.updated_at, now),
            });
        }
    }
    items.sort_by_key(|item| std::cmp::Reverse(item.age_secs));
    items
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
pub(crate) async fn snapshot_wave(store: &SharedStore, wave: &Wave) -> Result<WaveSnapshot> {
    let repo = wave.repo().to_string();
    let goal_repo = crate::engine::worktrees::main_repo_root(Path::new(&repo))
        .unwrap_or_else(|_| Path::new(&repo).to_path_buf());
    let paused = if wave.is_retired() {
        false
    } else {
        match crate::work::wave::config::try_read_wave_config(&goal_repo, wave.name()) {
            Ok(config) => config.and_then(|config| config.paused).unwrap_or(false),
            Err(error) => {
                tracing::warn!(wave = wave.name(), %error, "Wave policy is unavailable");
                false
            }
        }
    };
    let endpoint = if repo.is_empty() || wave.is_retired() {
        None
    } else {
        live_endpoint(Path::new(&repo), wave.name()).await
    };
    let tasks = store
        .list_tasks(Some(wave.id()))
        .await
        .map_err(|err| anyhow!("failed to count active Tasks: {err}"))?;
    let mut active_tasks = 0;
    for task in tasks {
        let status = child_work_status(store, &ChildRef::Task(task.id)).await?;
        active_tasks += u32::from(!work_status_is_terminal(&status));
    }
    let projects = store
        .list_projects(Some(wave.id()))
        .await
        .map_err(|err| anyhow!("failed to count active Projects: {err}"))?;
    let mut active_projects = 0;
    for project in projects {
        let status = child_work_status(store, &ChildRef::Project(project.id)).await?;
        active_projects += u32::from(!work_status_is_terminal(&status));
    }
    let placement = store
        .placement(&WorkRef::Wave(wave.id().clone()))
        .await
        .map_err(|error| anyhow!("failed to read Wave Home placement: {error}"))?;
    let home = store
        .home_by_id(&placement.home_id)
        .await
        .map_err(|error| anyhow!("failed to read Wave Home: {error}"))?
        .ok_or_else(|| anyhow!("Home {} was not found", placement.home_id))?;
    let status = store
        .work_status(&WorkRef::Wave(wave.id().clone()))
        .await
        .map_err(|error| anyhow!("failed to read Wave Work status: {error}"))?;
    Ok(WaveSnapshot {
        id: wave.id().to_string(),
        name: wave.name().to_string(),
        status,
        goal: if wave.is_retired() {
            wave.name().to_string()
        } else {
            crate::work::wave::config::read_wave_summary(&goal_repo, wave.name())
                .unwrap_or_else(|_| wave.name().to_string())
        },
        repo,
        active_tasks,
        active_projects,
        live: endpoint.is_some(),
        paused,
        enabled: placement.enabled,
        endpoint,
        created_at: wave.created_at().and_then(format_time),
        parent_wave_id: wave.parent_wave_id().map(ToString::to_string),
        retired_at: wave.retired_at().and_then(format_time),
        superseded_by_wave_id: wave.superseded_by_wave_id().map(ToString::to_string),
        retirement_reason: wave.retirement_reason().map(str::to_string),
        home,
    })
}

async fn snapshot_task_runtime(
    store: &SharedStore,
    task: &Task,
    status: WorkStatus,
) -> Result<TaskRuntimeSnapshot> {
    let routing_project_id = Some(task.project_id.to_string());
    let controller = store.task_controller_state(&task.id).await?;
    Ok(TaskRuntimeSnapshot {
        work_id: task.id.to_string(),
        project_id: task.project_id.to_string(),
        routing_project_id,
        reason: status.reason().to_string(),
        status,
        updated_at: format_time(task.updated_at).unwrap_or_default(),
        provider: controller.map(|state| state.provider).unwrap_or_default(),
    })
}

async fn snapshot_project_runtime(
    store: &SharedStore,
    project: &Project,
    status: WorkStatus,
) -> Result<ProjectRuntimeSnapshot> {
    let pending_observations = if work_status_is_terminal(&status) {
        store
            .pending_observations(&ObservationRecipient::Project {
                project_id: project.id.clone(),
            })
            .await
            .map_err(|err| anyhow!("failed to read Project observation outbox: {err}"))?
            .len() as u32
    } else {
        store
            .pending_project_observations(&project.id)
            .await
            .map_err(|err| anyhow!("failed to read Project observation outbox: {err}"))?
            .len() as u32
    };
    let controller = store.project_controller_state(&project.id).await?;
    Ok(ProjectRuntimeSnapshot {
        work_id: project.id.to_string(),
        reason: status.reason().to_string(),
        status,
        updated_at: format_time(project.updated_at).unwrap_or_default(),
        iteration: controller.as_ref().map_or(0, |state| state.iteration),
        pending_observations,
        provider: controller.map_or_else(String::new, |state| state.provider),
        last_failure: store
            .latest_project_failure(&project.id)
            .await
            .map_err(|err| anyhow!("failed to read Project failure history: {err}"))?,
    })
}

/// The wave's local PM snapshot, or `None` when none has been synced. `None` is
/// a real, readable state ("no plan on this machine yet") — a caller that must
/// tell it apart from "the plan is empty" keeps the `Option`; `lf status`
/// flattens it to an empty plan, `lf roadmap` renders it as unavailable.
async fn read_pm_planning(store: &SharedStore, wave: &Wave) -> Result<Option<PmSnapshot>> {
    let Some(row) = store
        .pm_snapshot(wave.id())
        .await
        .map_err(|err| anyhow!("failed to read PM snapshot: {err}"))?
    else {
        return Ok(None);
    };
    Ok(Some(decode_pm_planning(wave, &row.payload)?))
}

fn decode_pm_planning(wave: &Wave, payload: &str) -> Result<PmSnapshot> {
    serde_json::from_str(payload).map_err(|err| {
        anyhow!(
            "invalid PM snapshot for wave/{}; run `lf pm sync`: {err}",
            wave.name()
        )
    })
}

async fn validate_pm_portfolio(store: &SharedStore, waves: &[Wave]) -> Result<()> {
    let mut ownership = PmPortfolioValidator::default();
    for wave in waves {
        let repo = crate::engine::worktrees::main_repo_root(Path::new(wave.repo()))
            .unwrap_or_else(|_| Path::new(wave.repo()).to_path_buf());
        let repo = std::fs::canonicalize(&repo).unwrap_or(repo);
        let Some(row) = store
            .pm_snapshot(wave.id())
            .await
            .map_err(|err| anyhow!("failed to read PM snapshot: {err}"))?
        else {
            continue;
        };
        let Ok(planning) = decode_pm_planning(wave, &row.payload) else {
            // The Wave's own roadmap row reports its unreadable planning as
            // unavailable. Keep validating every readable sibling so one stale
            // snapshot cannot erase unrelated Work from the machine view.
            continue;
        };
        let expected_team = crate::ops::pm::repository_team_for_snapshot_validation(&repo)?;
        ownership.validate(
            wave.name(),
            &row.initiative,
            expected_team.as_deref(),
            &planning.projects,
            &planning.items,
        )?;
    }
    Ok(())
}

async fn snapshot_projects(
    store: &SharedStore,
    projects: Vec<Project>,
    tasks: Vec<Task>,
    planning: PmSnapshot,
    probe_pr_empty: bool,
) -> Result<ProjectSnapshots> {
    let mut details = planning
        .projects
        .into_iter()
        .map(|project| ProjectDetailSnapshot {
            next_move: next_move_for_unstarted_project(&project),
            project: project_summary(project),
            runtime: None,
            direction: None,
            tasks: Vec::new(),
        })
        .collect::<Vec<_>>();
    let mut unavailable_projects = Vec::new();

    for project in &projects {
        let status = child_work_status(store, &ChildRef::Project(project.id.clone())).await?;
        let Some(index) =
            find_project_index(&details, project.plan.id.as_str(), &project.plan.slug)
        else {
            if !work_status_is_terminal(&status) {
                unavailable_projects.push(unavailable_project(project, status));
            }
            continue;
        };
        if details[index].runtime.is_some() {
            continue;
        }
        details[index].next_move = next_move_for_project(&status);
        details[index].runtime = Some(snapshot_project_runtime(store, project, status).await?);
        details[index].direction =
            current_direction(store, ChildRef::Project(project.id.clone())).await?;
    }

    for item in planning.items {
        let index = project_index(&details, &item.project_id, &item.project)?;
        let runtime_task = tasks.iter().find(|task| {
            task.plan.id.as_str() == item.id || task.plan.identifier == item.identifier
        });
        details[index]
            .tasks
            .push(snapshot_task_detail(store, item, runtime_task, probe_pr_empty).await?);
    }

    for runtime_task in &tasks {
        let status = child_work_status(store, &ChildRef::Task(runtime_task.id.clone())).await?;
        let parent = projects
            .iter()
            .find(|project| project.id == runtime_task.project_id)
            .ok_or_else(|| {
                anyhow!(
                    "Task {} requires owning Project {}",
                    runtime_task.id,
                    runtime_task.project_id
                )
            })?;
        let Some(project_index) =
            find_project_index(&details, parent.plan.id.as_str(), &parent.plan.slug)
        else {
            if work_status_is_terminal(&status) {
                continue;
            }
            let unavailable_index = match unavailable_projects
                .iter()
                .position(|evidence| evidence.work_id == parent.id.as_str())
            {
                Some(index) => index,
                None => {
                    let parent_status =
                        child_work_status(store, &ChildRef::Project(parent.id.clone())).await?;
                    unavailable_projects.push(unavailable_project(parent, parent_status));
                    unavailable_projects.len() - 1
                }
            };
            unavailable_projects[unavailable_index]
                .tasks
                .push(unavailable_task(runtime_task, status));
            continue;
        };
        if details[project_index].tasks.iter().any(|detail| {
            detail.task.id == runtime_task.plan.id.as_str()
                || detail.task.identifier == runtime_task.plan.identifier
        }) {
            continue;
        }
        let item = PmItem {
            id: runtime_task.plan.id.as_str().to_string(),
            identifier: runtime_task.plan.identifier.clone(),
            url: None,
            name: runtime_task.plan.title.clone(),
            description: runtime_task.plan.description.clone(),
            rank: u32::MAX,
            completed: work_status_is_terminal(&status),
            project_id: parent.plan.id.as_str().to_string(),
            project: parent.plan.slug.clone(),
            team_id: String::new(),
            assignee: None,
        };
        details[project_index]
            .tasks
            .push(snapshot_task_detail(store, item, Some(runtime_task), probe_pr_empty).await?);
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
    for project in &mut unavailable_projects {
        project.tasks.sort_by(|left, right| {
            left.task_identifier
                .cmp(&right.task_identifier)
                .then(left.work_id.cmp(&right.work_id))
        });
    }
    unavailable_projects.sort_by(|left, right| {
        left.project_slug
            .cmp(&right.project_slug)
            .then(left.work_id.cmp(&right.work_id))
    });
    Ok(ProjectSnapshots {
        projects: details,
        unavailable_projects,
    })
}

fn unavailable_project(project: &Project, status: WorkStatus) -> UnavailableProjectEvidence {
    const REASON: &str = "Project is absent from the current PM snapshot";
    let recovery = if work_status_is_terminal(&status) {
        format!(
            "Settle the listed Tasks; Project Work is already {}",
            status.label()
        )
    } else {
        format!(
            "lf project abandon {} --reason \"{REASON}\"",
            project.plan.slug
        )
    };
    UnavailableProjectEvidence {
        work_id: project.id.to_string(),
        project_id: project.plan.id.as_str().to_string(),
        project_slug: project.plan.slug.clone(),
        status,
        owner: NextMoveOwner::Wave,
        reason: REASON.to_string(),
        recovery,
        tasks: Vec::new(),
    }
}

fn unavailable_task(task: &Task, status: WorkStatus) -> UnavailableTaskEvidence {
    const REASON: &str = "Task's owning Project is absent from the current PM snapshot";
    UnavailableTaskEvidence {
        work_id: task.id.to_string(),
        task_id: task.plan.id.as_str().to_string(),
        task_identifier: task.plan.identifier.clone(),
        status,
        owner: NextMoveOwner::Wave,
        reason: REASON.to_string(),
        recovery: format!(
            "lf work abandon task {} --reason \"Project is absent from the current PM snapshot\"",
            task.id
        ),
    }
}

fn project_index(projects: &[ProjectDetailSnapshot], id: &str, slug: &str) -> Result<usize> {
    find_project_index(projects, id, slug)
        .ok_or_else(|| {
            anyhow!(
                "Project {slug} ({id}) is not present in the current PM snapshot; run `lf pm sync` before reading the Wave work map"
            )
        })
}

fn find_project_index(projects: &[ProjectDetailSnapshot], id: &str, slug: &str) -> Option<usize> {
    projects
        .iter()
        .position(|project| project.project.id == id || project.project.slug == slug)
}

async fn snapshot_task_detail(
    store: &SharedStore,
    item: PmItem,
    task: Option<&Task>,
    probe_pr_empty: bool,
) -> Result<TaskDetailSnapshot> {
    let prs = match task {
        Some(task) => store.task_prs(&task.id).await?,
        None => Vec::new(),
    };
    let latest = prs.last();
    let active = prs.iter().find(|pr| pr.is_active());
    let observed_at = now();
    let runtime = match task {
        Some(task) => {
            let status = child_work_status(store, &ChildRef::Task(task.id.clone())).await?;
            Some(snapshot_task_runtime(store, task, status).await?)
        }
        None => None,
    };
    let reference = task_reference(&item, task, active, &prs);
    let worktree_blocker = match task {
        Some(task) => crate::ops::task::task_worktree_blocker(store, task).await?,
        None => None,
    };
    let launch_refusal = match (task, worktree_blocker.as_ref()) {
        (Some(task), None) => crate::ops::task::task_launch_refusal(store, task).await?,
        (Some(_), Some(_)) | (None, _) => None,
    };
    let next_move = task.map(|_| {
        next_move_for_task(
            &runtime
                .as_ref()
                .expect("Task runtime exists when the durable Task exists")
                .status,
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
        None if item.completed => NextMove {
            owner: NextMoveOwner::Project,
            reason: "Linear Task is complete".to_string(),
        },
        None => NextMove {
            owner: NextMoveOwner::Project,
            reason: "Task is ready to start".to_string(),
        },
    };
    let local_progress =
        task_local_progress(task, runtime.as_ref(), active, worktree_blocker.as_ref());
    let completion_refusal = match (task, runtime.as_ref()) {
        (Some(task), Some(runtime)) if !work_status_is_terminal(&runtime.status) => {
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
    let (action_evidence, user_ask) = match task {
        Some(task) => {
            let predecessor_phase = match active.and_then(|pr| pr.parent_pr_id.as_ref()) {
                Some(parent_id) => store.get_task_pr(parent_id).await?.map(|pr| pr.phase()),
                None => None,
            };
            let work = store
                .work_for_child(&ChildRef::Task(task.id.clone()))
                .await?;
            let user_ask = store.has_pending_user_ask_for_work(&work).await?;
            let work_status = store.work_status(&work).await?;
            (
                Some(TaskActionEvidence {
                    status: work_status,
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
                }),
                user_ask,
            )
        }
        None => (None, false),
    };
    let attention = derive_task_attention(
        item.completed,
        runtime.as_ref(),
        &next_move,
        TaskAttentionEvidence {
            local_progress,
            user_ask,
        },
        action_evidence.as_ref(),
        observed_at,
    );
    let direction = match task {
        Some(task) => current_direction(store, ChildRef::Task(task.id.clone())).await?,
        None => None,
    };
    Ok(TaskDetailSnapshot {
        task: task_summary(item),
        reference,
        runtime,
        direction,
        next_move,
        attention,
        prs: prs
            .iter()
            .map(|pr| {
                // PR emptiness is an execution-plane fact (`lf status`); it costs
                // an additional Git comparison, so `lf roadmap` opts out. The
                // attention fold already carries the progress evidence it needs.
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
    let Some(task) = task else {
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
        runtime
            .map(|runtime| &runtime.status)
            .expect("Task runtime exists when the durable Task exists"),
        &task.worktree,
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

fn derive_task_attention(
    pm_completed: bool,
    runtime: Option<&TaskRuntimeSnapshot>,
    next_move: &NextMove,
    evidence: TaskAttentionEvidence,
    action_evidence: Option<&TaskActionEvidence>,
    observed_at: time::OffsetDateTime,
) -> TaskAttentionSnapshot {
    let TaskAttentionEvidence {
        local_progress,
        user_ask,
    } = evidence;
    let active_pr_phase = action_evidence
        .and_then(|e| e.latest_pr_phase)
        .filter(|phase| phase.is_active());
    let user_attention = next_move.owner == NextMoveOwner::User;
    let (level, reason) = if user_ask {
        (
            TaskAttentionLevel::Blue,
            "Waiting for your answer".to_string(),
        )
    } else if user_attention {
        (TaskAttentionLevel::Red, next_move.reason.clone())
    } else if local_progress.state == LocalProgressEvidenceState::Missing
        && local_progress.recovery_required == Some(false)
    {
        (
            TaskAttentionLevel::Black,
            local_progress
                .reason
                .clone()
                .unwrap_or_else(|| "Task worktree is initializing".into()),
        )
    } else if local_progress.unsettled == Some(true) {
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
        (TaskAttentionLevel::Red, reason)
    } else if local_progress.unsettled.is_none() {
        (
            TaskAttentionLevel::Unknown,
            local_progress
                .reason
                .clone()
                .unwrap_or_else(|| "local Task progress is unavailable".into()),
        )
    } else {
        (TaskAttentionLevel::Black, next_move.reason.clone())
    };
    let actions = match action_evidence {
        None => TaskActionModel::no_task(),
        Some(evidence) => derive_task_actions(evidence),
    };
    TaskAttentionSnapshot {
        level,
        reason,
        observed_at: format_time(observed_at)
            .expect("Task attention observation time formats as RFC 3339"),
        evidence_age_secs: runtime.and_then(|runtime| age_secs(&runtime.updated_at, observed_at)),
        next_owner: next_move.owner,
        actions,
        pm_completed,
        work_status: runtime.map(|runtime| runtime.status.clone()),
        local_progress,
        active_pr_phase,
    }
}

fn task_reference(
    item: &PmItem,
    task: Option<&Task>,
    active_pr: Option<&TaskPr>,
    prs: &[TaskPr],
) -> TaskReferenceSnapshot {
    let workspace = task.map(|task| {
        let branch = active_pr
            .or_else(|| prs.iter().max_by_key(|pr| pr.sequence))
            .map(|pr| pr.branch.clone());
        TaskWorkspaceSnapshot {
            slug: task.workspace_slug.clone(),
            branch,
            worktree: task.worktree.display().to_string(),
        }
    });
    TaskReferenceSnapshot {
        issue_url: item.url.clone(),
        workspace,
    }
}

fn task_pr_empty(task: &Task, pr: &TaskPr) -> Option<bool> {
    if !task.worktree.exists() {
        return None;
    }
    let clean = crate::engine::git::is_clean(&task.worktree).ok()?;
    if !clean {
        return Some(false);
    }
    let head = crate::engine::git::rev_parse(&task.worktree, "HEAD").ok()?;
    Some(head == pr.base_commit)
}

async fn current_direction(
    store: &SharedStore,
    target: ChildRef,
) -> Result<Option<DirectionSnapshot>> {
    let steers = store
        .work_steers_for_child(&target)
        .await
        .map_err(|err| anyhow!("failed to read Work steers: {err}"))?;
    let text = crate::durable::render_steers(&steers);
    if text.is_empty() {
        return Ok(None);
    }
    Ok(Some(DirectionSnapshot { text }))
}

fn project_summary(project: PmProject) -> PmProjectSummary {
    PmProjectSummary {
        id: project.id,
        slug: project.slug,
        name: project.name,
        summary: project.summary,
        definition: project.definition,
        flows: project.flows.unwrap_or_else(ProjectFlowPlan::empty),
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

fn next_move_for_project(status: &WorkStatus) -> NextMove {
    let owner = match status {
        WorkStatus::Ready => NextMoveOwner::Project,
        WorkStatus::Done | WorkStatus::Abandoned => NextMoveOwner::Wave,
    };
    NextMove {
        owner,
        reason: status.reason().to_string(),
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
                owner: NextMoveOwner::Project,
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
            owner: NextMoveOwner::Project,
            reason: "PR is published but settlement is not armed with `lf pr land -c`".to_string(),
        };
    }
    let owner = NextMoveOwner::Project;
    NextMove {
        owner,
        reason: status.reason().to_string(),
    }
}

/// The invoking context's wave id: `LF_WAVE_ID`, else `None` (the caller
/// errors). Kept minimal — `lf status` with no arg is a convenience, not the
/// resolution surface `lf chat` owns.
fn ambient_wave() -> Option<String> {
    std::env::var(crate::work::wave::context::WAVE_ID_ENV)
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

fn historical_failure_line(failure: &crate::work::project::HistoricalFailure) -> String {
    format!(
        "last failure at {}: {}",
        format_time(failure.occurred_at).unwrap_or_else(|| failure.occurred_at.to_string()),
        failure.message
    )
}

/// With no registry on this machine, `lf ls`/`status` have nothing to read —
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
        "{bold}{name:<16}  {repo:<28}  {status:<8}  {enabled:<7}  {turns:<7}  {live:<5}  {tasks:>5}  {projects:>8}  {home:<16}  ENDPOINT{reset}",
        bold = colors.bold,
        reset = colors.reset,
        name = "WAVE",
        repo = "REPOSITORY",
        status = "STATUS",
        enabled = "ENABLED",
        turns = "TURNS",
        live = "LIVE",
        tasks = "TASKS",
        projects = "PROJECTS",
        home = "HOME",
    );
    for wave in snapshots {
        println!(
            "{name:<16}  {repo:<28}  {status:<8}  {enabled:<7}  {turns:<7}  {live:<5}  {tasks:>5}  {projects:>8}  {home:<16}  {endpoint}",
            name = truncate(&wave.name, 16),
            repo = truncate_start(&wave.repo, 28),
            status = wave.status.label(),
            enabled = if wave.enabled { "yes" } else { "no" },
            turns = if wave.paused { "paused" } else { "enabled" },
            live = if wave.live { "yes" } else { "no" },
            tasks = wave.active_tasks,
            projects = wave.active_projects,
            home = truncate(&wave.home.route, 16),
            endpoint = wave.endpoint.as_deref().unwrap_or("-"),
        );
    }
}

fn work_status_is_terminal(status: &WorkStatus) -> bool {
    matches!(status, WorkStatus::Done | WorkStatus::Abandoned)
}

async fn child_work_status(store: &SharedStore, child: &ChildRef) -> Result<WorkStatus> {
    let work = store
        .work_for_child(child)
        .await
        .map_err(|error| anyhow!("failed to resolve child Work: {error}"))?;
    store
        .work_status(&work)
        .await
        .map_err(|error| anyhow!("failed to read child Work status: {error}"))
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
        HomeActionDto::Start { home_id } => format!("Start on {home_id}"),
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
        status = if wave.retired_at.is_some() {
            "retired"
        } else {
            wave.status.label()
        },
        loop_state = status
            .loop_state
            .as_deref()
            .map(|m| format!("  loop:{m}"))
            .unwrap_or_default(),
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
        "  turns     {}",
        if wave.paused { "paused" } else { "enabled" }
    );
    println!("  enabled   {}", wave.enabled);
    println!(
        "  home      {} ({})  [{}]",
        wave.home.id,
        wave.home.route,
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
    let project_names = status
        .projects
        .iter()
        .map(|project| (project.project.id.clone(), project.project.name.clone()))
        .collect();
    print_metric_portfolio(&status.metric_portfolio, &project_names);
    if status.projects.is_empty() {
        println!("  projects  none");
    } else {
        println!("  projects");
        for project in &status.projects {
            let (project_status, iteration, reason) = match &project.runtime {
                Some(runtime) => (
                    runtime.status.label(),
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
            if let Some(failure) = project
                .runtime
                .as_ref()
                .and_then(|runtime| runtime.last_failure.as_ref())
            {
                println!("      {}", historical_failure_line(failure));
            }
            for task in &project.tasks {
                let (task_status, reason) = match &task.runtime {
                    Some(runtime) => (runtime.status.label(), runtime.reason.as_str()),
                    None if task.task.completed => ("completed", task.next_move.reason.as_str()),
                    None => ("unstarted", task.next_move.reason.as_str()),
                };
                let active_pr = task
                    .active_pr
                    .as_ref()
                    .and_then(|id| task.prs.iter().find(|pr| &pr.id == id));
                println!(
                    "      {issue}  {status:<10}  {workspace:<20}  {pr:<24}  {reason}",
                    issue = task_identifier_label(
                        &task.task.identifier,
                        task.reference.issue_url.as_deref(),
                        12,
                        std::io::stdout().is_terminal(),
                    ),
                    status = task_status,
                    workspace = truncate(&workspace_label(&task.reference), 20),
                    pr = truncate(
                        &active_pr.map(pr_label).unwrap_or_else(|| "-".to_string()),
                        24,
                    ),
                    reason = reason,
                );
            }
        }
    }
    print_unavailable_projects(&status.unavailable_projects);
    print_attention(&status.attention);
    print_runs(&status.runs);
}

fn print_metric_portfolio(
    portfolio: &MetricPortfolioDto,
    project_names: &std::collections::BTreeMap<String, String>,
) {
    print!("{}", metric_portfolio_text(portfolio, project_names));
}

fn metric_portfolio_text(
    portfolio: &MetricPortfolioDto,
    project_names: &std::collections::BTreeMap<String, String>,
) -> String {
    if portfolio.metrics.is_empty() && portfolio.contract_issues.is_empty() {
        return String::new();
    }
    let mut lines = vec!["  metrics".to_string()];
    let official = portfolio
        .metrics
        .iter()
        .filter(|metric| metric.stage == MetricStage::Graduated)
        .collect::<Vec<_>>();
    let mut project_ids = official
        .iter()
        .map(|metric| metric.project_id.as_str())
        .collect::<std::collections::BTreeSet<_>>()
        .into_iter()
        .collect::<Vec<_>>();
    project_ids.sort_by_key(|project_id| {
        project_names
            .get(*project_id)
            .map(String::as_str)
            .unwrap_or(project_id)
    });
    for project_id in project_ids {
        let owner = project_names
            .get(project_id)
            .map(String::as_str)
            .unwrap_or(project_id);
        lines.push(format!("    {owner}"));
        let mut metrics = official
            .iter()
            .copied()
            .filter(|metric| metric.project_id == project_id)
            .collect::<Vec<_>>();
        metrics.sort_by(|left, right| {
            metric_priority(&left.evidence)
                .cmp(&metric_priority(&right.evidence))
                .then(left.name.cmp(&right.name))
        });
        for metric in metrics {
            append_metric_lines(&mut lines, metric, owner, "      ");
        }
    }

    let mut candidates = portfolio
        .metrics
        .iter()
        .filter(|metric| metric.stage == MetricStage::Installed)
        .collect::<Vec<_>>();
    candidates.sort_by(|left, right| {
        let left_owner = project_names
            .get(&left.project_id)
            .map(String::as_str)
            .unwrap_or(&left.project_id);
        let right_owner = project_names
            .get(&right.project_id)
            .map(String::as_str)
            .unwrap_or(&right.project_id);
        left_owner.cmp(right_owner).then(left.name.cmp(&right.name))
    });
    if !candidates.is_empty() {
        lines.push("    Instrumenting".to_string());
        for metric in candidates {
            let owner = project_names
                .get(&metric.project_id)
                .map(String::as_str)
                .unwrap_or(&metric.project_id);
            append_metric_lines(&mut lines, metric, owner, "      ");
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
    owner: &str,
    indent: &str,
) {
    lines.push(format!(
        "{indent}{}  [{}]",
        metric.name,
        metric_evidence_label(&metric.evidence),
    ));
    lines.push(format!(
        "{indent}  Owner {owner} · Value {value} · Target {target} over {window} · {freshness}",
        value = metric_value(metric),
        target = metric_target(metric),
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
        MetricEvidenceDto::Unknown { .. } => 1,
        MetricEvidenceDto::Met { .. } => 2,
    }
}

fn metric_evidence_label(evidence: &MetricEvidenceDto) -> &'static str {
    match evidence {
        MetricEvidenceDto::Met { .. } => "met",
        MetricEvidenceDto::Missed { .. } => "missed",
        MetricEvidenceDto::Unknown { .. } => "unknown",
        MetricEvidenceDto::Unavailable { .. } => "unavailable",
    }
}

fn metric_value(metric: &MetricReadingDto) -> String {
    let value = match &metric.evidence {
        MetricEvidenceDto::Met { value, .. } | MetricEvidenceDto::Missed { value, .. } => {
            Some(*value)
        }
        MetricEvidenceDto::Unknown { cause } => match cause {
            MetricUnknownCauseDto::Incomplete { value, .. }
            | MetricUnknownCauseDto::WindowMismatch { value, .. }
            | MetricUnknownCauseDto::StaleObservation { value, .. } => Some(*value),
            _ => None,
        },
        MetricEvidenceDto::Unavailable { .. } => None,
    };
    value
        .map(|value| format_metric_number(value, &metric.unit))
        .unwrap_or_else(|| "-".to_string())
}

fn metric_target(metric: &MetricReadingDto) -> String {
    match metric.target {
        MetricTarget::AtLeast { value } => {
            format!(">= {}", format_metric_number(value, &metric.unit))
        }
        MetricTarget::AtMost { value } => {
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
        MetricEvidenceDto::Met { .. } | MetricEvidenceDto::Missed { .. } => None,
        MetricEvidenceDto::Unavailable {
            reason,
            source_as_of,
        } => Some(format!(
            "{reason} · as of {}",
            format_time(*source_as_of).unwrap_or_else(|| "unknown".to_string())
        )),
        MetricEvidenceDto::Unknown { cause } => Some(match cause {
            MetricUnknownCauseDto::Never => "no observation has arrived".to_string(),
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
        MetricContractIssueDto::MalformedContract { path, message } => {
            format!("{path}: {message}")
        }
        MetricContractIssueDto::UnresolvedOwner {
            wave_id,
            metric_id,
            project_id,
        } => format!("{wave_id}/{metric_id} names unknown Project {project_id}"),
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

fn print_unavailable_projects(projects: &[UnavailableProjectEvidence]) {
    if projects.is_empty() {
        return;
    }
    println!("  unavailable projects");
    for project in projects {
        println!(
            "    {slug:<24}  {status:<10}  {owner:<8}  {work}  {reason}",
            slug = truncate(&project.project_slug, 24),
            status = project.status.label(),
            owner = owner_label(&project.owner),
            work = project.work_id,
            reason = project.reason,
        );
        println!("      recover  {}", project.recovery);
        for task in &project.tasks {
            println!(
                "      {identifier:<12}  {status:<10}  {owner:<8}  {work}  {reason}",
                identifier = truncate(&task.task_identifier, 12),
                status = task.status.label(),
                owner = owner_label(&task.owner),
                work = task.work_id,
                reason = task.reason,
            );
            println!("        recover  {}", task.recovery);
        }
    }
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

fn print_runs(runs: &Evidence<RunSnapshot>) {
    match runs {
        Evidence::Unavailable { reason } => println!("  runs unavailable: {reason}"),
        Evidence::Ok { items, .. } if items.is_empty() => {
            println!("  runs       no Run records in the window")
        }
        Evidence::Ok { items, truncated } => {
            println!("  runs");
            for run in items {
                println!(
                    "    {label:<24}  {status:<12}  tok {tokens:>7}  {age:>7} ago",
                    label = truncate(run.label(), 24),
                    status = run.status(),
                    tokens = run
                        .total_tokens()
                        .map(format_tokens)
                        .unwrap_or_else(|| "-".to_string()),
                    age = format_age(now().unix_timestamp() - run.started),
                );
            }
            if *truncated {
                println!("    (older runs beyond the window cap are not shown)");
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
    attention: Option<TaskAttentionLevel>,
    reason: String,
}

fn task_attention_label(level: TaskAttentionLevel) -> &'static str {
    match level {
        TaskAttentionLevel::Red => "red",
        TaskAttentionLevel::Blue => "blue",
        TaskAttentionLevel::Black => "black",
        TaskAttentionLevel::Unknown => "unknown",
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
        attention: Some(task.attention.level),
        reason: task.attention.reason.clone(),
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

fn workspace_label(reference: &TaskReferenceSnapshot) -> String {
    reference
        .workspace
        .as_ref()
        .map(|workspace| workspace.slug.clone())
        .unwrap_or_else(|| "-".to_string())
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
        RoadmapSection::NeedsAttention => "NEEDS ATTENTION",
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
        let project_names = match &wave.projects {
            Evidence::Ok { items, .. } => items
                .iter()
                .map(|project| (project.project.id.clone(), project.project.name.clone()))
                .collect(),
            Evidence::Unavailable { .. } => std::collections::BTreeMap::new(),
        };
        print_metric_portfolio(&wave.metric_portfolio, &project_names);
        print_unavailable_projects(&wave.unavailable_projects);
        let details = match &wave.projects {
            Evidence::Unavailable { reason } => {
                println!("  unavailable: {reason}");
                continue;
            }
            Evidence::Ok { items, .. } => items,
        };
        let mut rows: Vec<RoadmapRow> = Vec::new();
        for project in details {
            // A Project appears as its own row only when a loop is running or
            // recorded it — an unstarted Project is just the grouping for the
            // Tasks under it, not a work item of its own.
            if let Some(runtime) = &project.runtime {
                rows.push(RoadmapRow {
                    section: project.section,
                    id: project.project.slug.clone(),
                    issue_url: None,
                    title: project.project.name.clone(),
                    rank: None,
                    age_secs: age_secs(&runtime.updated_at, now),
                    owner: project.next_move.owner,
                    pr: None,
                    workspace: None,
                    attention: None,
                    reason: project.next_move.reason.clone(),
                });
            }
            for task in &project.tasks {
                rows.push(task_roadmap_row(task, now));
            }
        }
        if rows.is_empty() {
            println!("  (no plan rows)");
            continue;
        }
        for section in [
            RoadmapSection::Now,
            RoadmapSection::NeedsAttention,
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
                    "    {id}  {rank:>4}  {owner:<8}  {age:>5}  {signal:<7}  {workspace:<20}  {pr:<24}  {title}",
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
                    signal = row
                        .attention
                        .map(task_attention_label)
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
        NextMoveOwner::Project => "project",
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
        derive_task_attention, historical_failure_line, metric_portfolio_text, next_move_for_task,
        snapshot_project_runtime, truncate_start, LocalProgressEvidence,
        LocalProgressEvidenceState, NextMove, NextMoveOwner, TaskAttentionEvidence,
        TaskAttentionLevel, TaskRuntimeSnapshot,
    };
    use crate::controller::wave::metrics::{
        MetricEvidenceDto, MetricFreshnessDto, MetricIdentity, MetricPortfolioDto,
        MetricReadingDto, MetricStage, MetricTarget, MetricUnknownCauseDto,
    };
    use crate::durable::WorkStatus;
    use crate::planning::{LinearProjectId, ProjectPlan};
    use crate::store::sqlite::SqliteStore;
    use crate::store::Store;
    use crate::work::project::{Project, ProjectEventKind, ProjectId};
    use crate::work::task::{CiObservation, CiState, PrMergeMode, PrMergeRequest, PrPhase};
    use crate::work::wave::Wave;

    #[tokio::test]
    async fn status_surfaces_keep_credential_failure_in_history() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("registry.db");
        let sqlite = SqliteStore::new(&path).unwrap();
        let now = OffsetDateTime::now_utc();
        let wave = Wave::new(
            crate::id::WaveId::new(),
            "infrastructure".to_string(),
            directory.path().display().to_string(),
        );
        sqlite.create_wave(&wave).unwrap();
        let project = Project {
            id: ProjectId::new(),
            plan: ProjectPlan {
                id: LinearProjectId::new("stability-security").unwrap(),
                slug: "stability-security".to_string(),
                name: "Stability and security".to_string(),
                prompt_context: "Keep status truthful.".to_string(),
                pm_snapshot_synced_at: now.unix_timestamp(),
            },
            wave_id: wave.id().clone(),
            abandon_intent: None,
            created_at: now,
            updated_at: now,
        };
        sqlite.insert_project(&project).unwrap();
        let failure = sqlite
            .append_project_event(
                &project.id,
                &ProjectEventKind::Failed {
                    error: "project runner failed: credential is missing".to_string(),
                    resumable: true,
                },
            )
            .unwrap();
        rusqlite::Connection::open(&path)
            .unwrap()
            .execute(
                "UPDATE project_events SET created_at=created_at-60 WHERE id=?1",
                [failure.id],
            )
            .unwrap();
        let store = Arc::new(Store::from_sqlite_for_test(sqlite));
        let status = store
            .work_status(&crate::durable::WorkRef::Project(project.id.clone()))
            .await
            .unwrap();
        let runtime = snapshot_project_runtime(&store, &project, status)
            .await
            .unwrap();
        let historical = runtime.last_failure.unwrap();

        assert_eq!(runtime.status, WorkStatus::Ready);
        assert_eq!(runtime.reason, "ready");
        assert!(!runtime.reason.contains("credential"));
        assert_eq!(
            historical.message,
            "project runner failed: credential is missing"
        );
        assert!(historical.occurred_at < now);
        assert!(historical_failure_line(&historical).starts_with("last failure at "));
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
                project_id: "project-api".to_string(),
                stage,
                instrumented: true,
                instrument: "scorecard".to_string(),
                unit: "ratio".to_string(),
                target: MetricTarget::AtLeast { value: 1.0 },
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
        let text = metric_portfolio_text(
            &portfolio,
            &std::collections::BTreeMap::from([(
                "project-api".to_string(),
                "Loopflow API".to_string(),
            )]),
        );

        let owner = text.find("    Loopflow API\n").unwrap();
        let broken = text.find("Broken  [missed]").unwrap();
        let healthy = text.find("Healthy  [met]").unwrap();
        let instrumenting = text.find("    Instrumenting\n").unwrap();
        let candidate = text.find("Candidate  [unknown]").unwrap();
        assert!(owner < broken && broken < healthy);
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
    fn only_a_durable_ask_marks_task_evidence_as_waiting_on_the_user() {
        let runtime = TaskRuntimeSnapshot {
            work_id: "task-1".to_string(),
            project_id: "project-1".to_string(),
            routing_project_id: Some("project-1".to_string()),
            status: WorkStatus::Ready,
            reason: "ready".to_string(),
            updated_at: "2026-07-21T00:00:00Z".to_string(),
            provider: "codex".to_string(),
        };
        let next_move = NextMove {
            owner: NextMoveOwner::Task,
            reason: "Task is ready".to_string(),
        };
        let evidence = |user_ask| TaskAttentionEvidence {
            local_progress: LocalProgressEvidence {
                state: LocalProgressEvidenceState::Observed,
                unsettled: Some(false),
                dirty: Some(false),
                authored_commits: Some(false),
                recovery_required: Some(false),
                reason: None,
            },
            user_ask,
        };

        let advisory = derive_task_attention(
            false,
            Some(&runtime),
            &next_move,
            evidence(false),
            None,
            OffsetDateTime::now_utc(),
        );
        let asked = derive_task_attention(
            false,
            Some(&runtime),
            &next_move,
            evidence(true),
            None,
            OffsetDateTime::now_utc(),
        );

        assert_eq!(advisory.level, TaskAttentionLevel::Black);
        assert_eq!(asked.level, TaskAttentionLevel::Blue);
        assert_eq!(asked.reason, "Waiting for your answer");
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
        assert_eq!(published.owner, NextMoveOwner::Project);

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
