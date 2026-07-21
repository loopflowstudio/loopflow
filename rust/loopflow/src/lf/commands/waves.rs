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

use crate::child::{
    body_progress_age, observe, BodyEvidence, BodyIntent, BodyObservation, ChildRef,
    ObservationRecipient, DEFAULT_STALL_AFTER,
};
use crate::durable::{Containment, Home, WorkRef, WorkStatus};
use crate::engine::wave_home::{HomeActionDto, HomeRuntimeDto, HomeState};
use crate::lf::commands::runs::{format_tokens, SkillRunEntry};
use crate::lf::output::Colors;
use crate::pm::{PmItem, PmKr, PmProject, ProjectFlowPlan};
use crate::project::Project;
use crate::store::{open_existing_store, SharedStore};
use crate::task::{
    AfterMerge, CiObservation, CiState, PrMergeMode, PrMergeRequest, PrPhase, Task, TaskPr,
};
use crate::wave::server::live_endpoint;
use crate::wave::Wave;

/// One wave's registry snapshot — the `lf ls` row and the `wave` field of
/// `lf status`. Wire type consumed by Loopflow: every field is required or
/// explicitly Optional, no serde defaults.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WaveSnapshot {
    pub id: String,
    pub name: String,
    /// Current Work lifecycle derived from Epoch, Run, and Wait facts.
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
    /// Loopback endpoint of the live server, `null` when stopped.
    pub endpoint: Option<String>,
    /// RFC3339 creation time, `null` when the row predates the column.
    pub created_at: Option<String>,
    /// Parent wave id in the chord tree, `null` for a root wave.
    pub parent_wave_id: Option<String>,
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
    /// This wave's agent-backed skill runs, newest first.
    pub runs: Evidence<SkillRunEntry>,
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
pub struct BoundarySeedSnapshot {
    pub basis: String,
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
    pub process_alive: bool,
    /// The observed state of this Project's current body: durable intent
    /// (`status`) and body observation are separate evidence, never one string.
    pub observation: BodyObservation,
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
    pub process_alive: bool,
    /// The observed state of this Task's current body, derived from durable
    /// intent, body liveness, and how long since its last durable event.
    pub observation: BodyObservation,
}

/// The compact Task attention signal shared by terminal and app surfaces. The
/// names are deliberately the product's visual vocabulary: consumers do not
/// reinterpret Work/process combinations into their own colors.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TaskAttentionLevel {
    Green,
    Red,
    Blue,
    Black,
    Unknown,
}

pub use crate::task::actions::{
    ci_failure_reason, derive_task_actions, TaskAction, TaskActionEvidence, TaskActionModel,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TaskProcessEvidenceState {
    Observed,
    NotExpected,
    NotApplicable,
    Unavailable,
}

/// Raw process constituent behind the attention fold. `alive` is present only
/// when this machine could look; absence never means dead.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TaskProcessEvidence {
    pub state: TaskProcessEvidenceState,
    pub alive: Option<bool>,
    pub reason: Option<String>,
}

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
    process: TaskProcessEvidence,
    local_progress: LocalProgressEvidence,
    user_ask: bool,
}

/// A Task's shared attention projection and the evidence that proves it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TaskAttentionSnapshot {
    pub level: TaskAttentionLevel,
    pub reason: String,
    /// RFC3339 time process/workspace evidence was sampled.
    pub observed_at: String,
    /// Age of the durable Work evidence at that sample, if Work exists.
    pub evidence_age_secs: Option<i64>,
    pub next_owner: NextMoveOwner,
    pub actions: TaskActionModel,
    pub pm_completed: bool,
    pub work_status: Option<WorkStatus>,
    pub process: TaskProcessEvidence,
    pub local_progress: LocalProgressEvidence,
    pub active_pr_phase: Option<PrPhase>,
}

/// Stable references for one Task, shared verbatim by `lf status` and
/// `lf roadmap`. The issue URL is cached PM evidence. Workspace evidence comes
/// from the durable Task and outlives its process and final PR.
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
    pub direction: Option<BoundarySeedSnapshot>,
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
    pub github: Option<GithubPrSnapshot>,
    pub merge: Option<PrMergeRequestSnapshot>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PrMergeRequestSnapshot {
    pub mode: PrMergeMode,
    pub requested_at: String,
    pub head_sha: String,
    pub after_merge: AfterMerge,
    pub next_slug: Option<String>,
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
                    github: publication.github.as_ref().map(|github| GithubPrSnapshot {
                        number: github.number,
                        url: github.url.clone(),
                    }),
                    merge: publication
                        .merge
                        .as_ref()
                        .map(|request| PrMergeRequestSnapshot {
                            mode: request.mode,
                            requested_at: format_time(request.requested_at)
                                .expect("PR merge request timestamp formats as RFC 3339"),
                            head_sha: request.head_sha.clone(),
                            after_merge: request.after_merge,
                            next_slug: request.next_slug.clone(),
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
    pub direction: Option<BoundarySeedSnapshot>,
    pub next_move: NextMove,
    pub tasks: Vec<TaskDetailSnapshot>,
}

/// Where a row's next move sends the reader's attention. A coarse view lens over
/// the same primitives `lf status` exposes (durable intent × liveness ×
/// ownership) — deliberately *not* a runtime-state taxonomy. It is derived once,
/// here in Rust, and stamped on the wire so CLI, Mac, and iOS bucket identically
/// without each re-deriving the rule.
///
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RoadmapSection {
    /// A live body is advancing this work itself.
    Now,
    /// Someone other than the running body must move: review, a User, the
    /// supervising Project or Wave — or the process died mid-flight.
    NeedsAttention,
    /// Filed, not started, not complete — ready for someone to pick up.
    Available,
    /// Done or dormant: terminal Work and completed plan rows.
    Later,
}

/// `lf roadmap` — the machine-wide intent plane. Every Wave's plan joined to
/// whatever live execution evidence exists, bucketed by attention section. Wire
/// type consumed by Loopflow; every field required or explicitly Optional, no
/// serde defaults.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RoadmapSnapshot {
    /// RFC3339 time this read was taken.
    pub generated_at: String,
    pub waves: Vec<WaveRoadmap>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WaveRoadmap {
    pub wave: WaveSnapshot,
    /// The Wave's plan joined to live evidence, or the reason there is none — a
    /// Wave with no local PM snapshot reads "unavailable", never an empty plan.
    pub projects: Evidence<RoadmapProject>,
}

/// One Project in the roadmap: its plan, live Project Work evidence when a
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

/// One Task in the roadmap: plan row, live Task Work evidence when a Task
/// exists, its section, and its active PR. `runtime: None` is a Task nobody has
/// started — never confused with a dead process.
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
            .list_projects(Some(wave.id()))
            .await
            .map_err(|err| anyhow!("failed to read Projects: {err}"))?;
        let stored_tasks = store
            .list_tasks(Some(wave.id()))
            .await
            .map_err(|err| anyhow!("failed to read Tasks: {err}"))?;
        let liveness = TmuxLiveness::snapshot().await;
        let planning = read_pm_planning(&store, &wave).await?.unwrap_or_default();
        let projects = snapshot_projects(
            &store,
            &wave,
            stored_projects,
            stored_tasks,
            planning,
            &liveness,
            true,
        )
        .await?;
        let attention = Evidence::complete(attention(&projects, now(), liveness.liveness()));
        // Probe the focused Wave's Home once so the detail carries live evidence
        // and the single contextual action (Open/Attach, Start, or reason).
        let home_runtime =
            crate::ops::home::probe_home(wave.name(), &snapshot.home, Path::new(wave.repo())).await;
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

/// `lf roadmap [wave]` — the machine-wide intent plane. Every Wave (or one, when
/// scoped) with its plan joined to live evidence and each row bucketed into a
/// section. Deterministic and local: one `tmux list-sessions` for the whole
/// read, bounded Git probes for Task Work, and no network. `lf status`
/// answers "is it healthy"; this answers "what is being worked on and what
/// could be".
pub fn roadmap(wave: Option<&str>, json: bool) -> Result<()> {
    let rt = tokio::runtime::Runtime::new()?;
    rt.block_on(async {
        let Some(store) = open_existing_store().await.map(std::sync::Arc::new) else {
            let roadmap = RoadmapSnapshot {
                generated_at: format_time(now()).expect("current timestamp formats as RFC 3339"),
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
        // UUID → registry name, hand-set name as fallback). Roadmap is the one
        // command where `NoContext` is a valid default — it lists every wave.
        // A stale UUID is a loud error, never a silent drop to global scope.
        let env_wave_id = std::env::var(crate::engine::wave_context::WAVE_ID_ENV).ok();
        let waves = match crate::engine::wave_context::resolve_managed_wave_name(
            Some(&store),
            wave,
            env_wave_id.as_deref(),
        )
        .await
        {
            Ok(name) => vec![store
                .get_wave_by_name(&name)
                .await
                .map_err(|err| anyhow!("failed to read wave registry: {err}"))?
                .ok_or_else(|| anyhow!("wave '{name}' is not in the registry"))?],
            Err(crate::engine::wave_context::WaveResolveError::NoContext) => store
                .list_waves(None)
                .await
                .map_err(|err| anyhow!("failed to read wave registry: {err}"))?,
            Err(other) => return Err(anyhow!(other)),
        };
        // One tmux reading for every Work process on the machine, taken once.
        let liveness = TmuxLiveness::snapshot().await;
        let mut roadmaps = Vec::with_capacity(waves.len());
        for wave in &waves {
            let snapshot = snapshot_wave(&store, wave).await?;
            let projects = wave_roadmap_projects(&store, wave, &liveness).await;
            roadmaps.push(WaveRoadmap {
                wave: snapshot,
                projects,
            });
        }
        roadmaps.sort_by(|a, b| a.wave.name.cmp(&b.wave.name));
        let roadmap = RoadmapSnapshot {
            generated_at: format_time(now()).expect("current timestamp formats as RFC 3339"),
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
    liveness: &TmuxLiveness,
) -> Evidence<RoadmapProject> {
    let planning = match read_pm_planning(store, wave).await {
        Ok(Some(planning)) => planning,
        Ok(None) => {
            return Evidence::Unavailable {
                reason: format!(
                    "no local PM snapshot for wave/{}; run `lf pm sync`",
                    wave.name()
                ),
            }
        }
        Err(err) => {
            return Evidence::Unavailable {
                reason: err.to_string(),
            }
        }
    };
    let projects = match store.list_projects(Some(wave.id())).await {
        Ok(projects) => projects,
        Err(err) => {
            return Evidence::Unavailable {
                reason: format!("failed to read Projects: {err}"),
            }
        }
    };
    let tasks = match store.list_tasks(Some(wave.id())).await {
        Ok(tasks) => tasks,
        Err(err) => {
            return Evidence::Unavailable {
                reason: format!("failed to read Tasks: {err}"),
            }
        }
    };
    // `probe_pr_empty: false` — PR emptiness is `lf status`'s execution detail.
    // Roadmap's bounded Git reads belong only to the shared attention evidence.
    match snapshot_projects(store, wave, projects, tasks, planning, liveness, false).await {
        Ok(details) => Evidence::complete(
            details
                .into_iter()
                .map(|detail| roadmap_project(detail, liveness.liveness()))
                .collect(),
        ),
        Err(err) => Evidence::Unavailable {
            reason: err.to_string(),
        },
    }
}

/// Project a `lf status` project detail into its roadmap row, deriving the
/// section for it and each of its Tasks.
fn roadmap_project(detail: ProjectDetailSnapshot, liveness: Liveness) -> RoadmapProject {
    let section = project_section(&detail, liveness);
    let tasks = detail
        .tasks
        .into_iter()
        .map(|task| roadmap_task(task, liveness))
        .collect();
    RoadmapProject {
        project: detail.project,
        runtime: detail.runtime,
        next_move: detail.next_move,
        section,
        tasks,
    }
}

fn roadmap_task(detail: TaskDetailSnapshot, liveness: Liveness) -> RoadmapTask {
    let section = task_section(&detail, liveness);
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

/// A Task's section, from the same primitives the row already carries. Order is
/// load-bearing: a dead process outranks a terminal record (an audit finding),
/// and terminal Work is `Later` before its owner is consulted.
fn task_section(task: &TaskDetailSnapshot, liveness: Liveness) -> RoadmapSection {
    let Some(runtime) = &task.runtime else {
        return if task.task.completed {
            RoadmapSection::Later
        } else {
            RoadmapSection::Available
        };
    };
    if liveness.is_gone(
        work_status_is_running(&runtime.status),
        runtime.process_alive,
    ) {
        return RoadmapSection::NeedsAttention;
    }
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
fn project_section(project: &ProjectDetailSnapshot, liveness: Liveness) -> RoadmapSection {
    let Some(runtime) = &project.runtime else {
        let all_krs_hold =
            !project.project.krs.is_empty() && project.project.krs.iter().all(|kr| kr.holds);
        return if all_krs_hold {
            RoadmapSection::Later
        } else {
            RoadmapSection::Available
        };
    };
    if liveness.is_gone(
        work_status_is_running(&runtime.status),
        runtime.process_alive,
    ) {
        return RoadmapSection::NeedsAttention;
    }
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
    // One shared rule for `--wave` and ambient `LF_WAVE_ID` (durable UUID
    // first, hand-set name as an intentional fallback). Status needs the row,
    // so it resolves the name, then requires a registry row for it — a wave
    // with no row has no runs to report.
    let name = crate::engine::wave_context::resolve_managed_wave_name(
        Some(&**store),
        requested,
        ambient_wave().as_deref(),
    )
    .await
    .map_err(|err| anyhow!("{err}"))?;
    store
        .get_wave_by_name(&name)
        .await
        .map_err(|err| anyhow!("failed to read wave registry: {err}"))?
        .ok_or_else(|| anyhow!("wave '{name}' is not in this machine's registry"))
}

/// One tmux reading for the whole command. `lf status` checks a handful of tmux
/// sessions and `lf roadmap` checks every tmux session on the machine; both take
/// a single `tmux list-sessions` snapshot here and look each name up in the set,
/// never a `has-session` fork per name. `installed` is kept distinct from an
/// empty `live` set: no tmux means liveness is *unknowable*, not *nothing alive*.
#[derive(Debug, Clone)]
struct TmuxLiveness {
    installed: bool,
    live: std::collections::HashSet<String>,
}

impl TmuxLiveness {
    async fn snapshot() -> Self {
        let installed = crate::engine::process::tmux_installed();
        let live = if installed {
            crate::engine::process::tmux_live_sessions()
                .await
                .unwrap_or_default()
        } else {
            std::collections::HashSet::new()
        };
        Self { installed, live }
    }

    fn is_alive(&self, tmux_name: &str) -> bool {
        self.installed && self.live.contains(tmux_name)
    }

    fn liveness(&self) -> Liveness {
        Liveness::probe(self.installed)
    }
}

/// Whether this machine can tell a live Work process from a dead one. Without
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

    /// Work that records a live process the machine looked for and did not
    /// find.
    fn is_gone(self, claims_process: bool, process_alive: bool) -> bool {
        self == Self::Observable && claims_process && !process_alive
    }
}

/// What in this wave is waiting on somebody. Two rules, both read from Work:
///
/// 1. Work's next move belongs to someone other than itself, or
/// 2. Work claims a live Run whose process the machine cannot find — the
///    kind of disagreement an audit surface exists to show.
///
/// Plan rows with no active Work are not attention: an unstarted backlog item is not
/// waiting on you.
fn attention(
    projects: &[ProjectDetailSnapshot],
    now: time::OffsetDateTime,
    liveness: Liveness,
) -> Vec<AttentionItem> {
    let mut items = Vec::new();
    for project in projects {
        if let Some(runtime) = &project.runtime {
            let dead = liveness.is_gone(
                work_status_is_running(&runtime.status),
                runtime.process_alive,
            );
            let self_owned = matches!(project.next_move.owner, NextMoveOwner::Project);
            if dead || !(self_owned || work_status_is_terminal(&runtime.status)) {
                items.push(AttentionItem {
                    kind: AttentionKind::Project,
                    id: runtime.work_id.clone(),
                    subject: project.project.slug.clone(),
                    owner: if dead {
                        NextMoveOwner::Wave
                    } else {
                        project.next_move.owner
                    },
                    reason: attention_reason(
                        dead,
                        work_status_label(&runtime.status),
                        &runtime.reason,
                    ),
                    since: runtime.updated_at.clone(),
                    age_secs: age_secs(&runtime.updated_at, now),
                });
            }
        }
        for task in &project.tasks {
            let Some(runtime) = &task.runtime else {
                continue;
            };
            let dead = liveness.is_gone(
                work_status_is_running(&runtime.status),
                runtime.process_alive,
            );
            if !dead && matches!(task.next_move.owner, NextMoveOwner::Task) {
                continue;
            }
            if !dead && work_status_is_terminal(&runtime.status) {
                continue;
            }
            items.push(AttentionItem {
                kind: AttentionKind::Task,
                id: runtime.work_id.clone(),
                subject: task.task.identifier.clone(),
                owner: if dead {
                    NextMoveOwner::Wave
                } else {
                    task.next_move.owner
                },
                reason: attention_reason(dead, work_status_label(&runtime.status), &runtime.reason),
                since: runtime.updated_at.clone(),
                age_secs: age_secs(&runtime.updated_at, now),
            });
        }
    }
    items.sort_by_key(|item| std::cmp::Reverse(item.age_secs));
    items
}

fn attention_reason(dead: bool, status: &str, recorded: &str) -> String {
    if dead {
        format!("process is gone but the Work still records '{status}'")
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
pub(crate) async fn snapshot_wave(store: &SharedStore, wave: &Wave) -> Result<WaveSnapshot> {
    let repo = wave.repo().to_string();
    let endpoint = if repo.is_empty() {
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
    store: &SharedStore,
    task: &Task,
    status: WorkStatus,
    liveness: &TmuxLiveness,
    now: time::OffsetDateTime,
) -> Result<TaskRuntimeSnapshot> {
    let process_alive = work_status_is_running(&status)
        && child_run_alive(store, &ChildRef::Task(task.id.clone()), liveness).await?;
    let latest_event_at = store
        .latest_task_event_at(&task.id)
        .await
        .map_err(|err| anyhow!("failed to read Task event log: {err}"))?;
    let evidence = BodyEvidence {
        intent: work_status_body_intent(&status),
        observable: liveness.liveness() == Liveness::Observable,
        process_alive,
        progress_age: body_progress_age(latest_event_at, task.updated_at, now),
        step: Some(task.lifecycle_phase.as_str().to_string()),
        reason: work_status_reason(&status),
    };
    let routing_project_id = Some(task.project_id.to_string());
    Ok(TaskRuntimeSnapshot {
        work_id: task.id.to_string(),
        project_id: task.project_id.to_string(),
        routing_project_id,
        reason: work_status_reason(&status),
        status,
        updated_at: format_time(task.updated_at).unwrap_or_default(),
        provider: task.provider.clone(),
        process_alive,
        observation: observe(&evidence, DEFAULT_STALL_AFTER),
    })
}

async fn snapshot_project_runtime(
    store: &SharedStore,
    project: &Project,
    status: WorkStatus,
    liveness: &TmuxLiveness,
    now: time::OffsetDateTime,
) -> Result<ProjectRuntimeSnapshot> {
    let process_alive = work_status_is_running(&status)
        && child_run_alive(store, &ChildRef::Project(project.id.clone()), liveness).await?;
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
    let latest_event_at = store
        .latest_project_event_at(&project.id)
        .await
        .map_err(|err| anyhow!("failed to read Project event log: {err}"))?;
    let evidence = BodyEvidence {
        intent: work_status_body_intent(&status),
        observable: liveness.liveness() == Liveness::Observable,
        process_alive,
        progress_age: body_progress_age(latest_event_at, project.updated_at, now),
        step: Some(format!("iteration {}", project.iteration)),
        reason: work_status_reason(&status),
    };
    Ok(ProjectRuntimeSnapshot {
        work_id: project.id.to_string(),
        reason: work_status_reason(&status),
        status,
        updated_at: format_time(project.updated_at).unwrap_or_default(),
        iteration: project.iteration,
        pending_observations,
        provider: project.provider.clone(),
        process_alive,
        observation: observe(&evidence, DEFAULT_STALL_AFTER),
    })
}

async fn child_run_alive(
    store: &SharedStore,
    child: &ChildRef,
    liveness: &TmuxLiveness,
) -> Result<bool> {
    let work = store
        .work_for_child(child)
        .await
        .map_err(|error| anyhow!("failed to resolve child Work: {error}"))?;
    let Some(run) = store
        .current_run(&work)
        .await
        .map_err(|error| anyhow!("failed to read child Run: {error}"))?
    else {
        return Ok(false);
    };
    Ok(match run.containment {
        None => false,
        Some(Containment::Tmux { name }) => liveness.is_alive(&name),
        Some(Containment::ProcessGroup { .. }) => true,
    })
}

#[derive(Debug, Default, Deserialize)]
struct CachedPmSnapshot {
    projects: Vec<PmProject>,
    items: Vec<PmItem>,
}

/// The wave's local PM snapshot, or `None` when none has been synced. `None` is
/// a real, readable state ("no plan on this machine yet") — a caller that must
/// tell it apart from "the plan is empty" keeps the `Option`; `lf status`
/// flattens it to an empty plan, `lf roadmap` renders it as unavailable.
async fn read_pm_planning(store: &SharedStore, wave: &Wave) -> Result<Option<CachedPmSnapshot>> {
    let repo = crate::engine::worktrees::main_repo_root(Path::new(wave.repo()))
        .unwrap_or_else(|_| Path::new(wave.repo()).to_path_buf());
    let repo = std::fs::canonicalize(&repo).unwrap_or(repo);
    let Some(row) = store
        .pm_snapshot(repo.to_string_lossy().into_owned(), wave.name().to_string())
        .await
        .map_err(|err| anyhow!("failed to read PM snapshot: {err}"))?
    else {
        return Ok(None);
    };
    let planning = serde_json::from_str::<CachedPmSnapshot>(&row.payload).map_err(|err| {
        anyhow!(
            "invalid PM snapshot for wave/{}; run `lf pm sync`: {err}",
            wave.name()
        )
    })?;
    Ok(Some(planning))
}

async fn snapshot_projects(
    store: &SharedStore,
    wave: &Wave,
    projects: Vec<Project>,
    tasks: Vec<Task>,
    planning: CachedPmSnapshot,
    liveness: &TmuxLiveness,
    probe_pr_empty: bool,
) -> Result<Vec<ProjectDetailSnapshot>> {
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

    for project in &projects {
        let status = child_work_status(store, &ChildRef::Project(project.id.clone())).await?;
        let Some(index) =
            find_project_index(&details, project.plan.id.as_str(), &project.plan.slug)
        else {
            if !work_status_is_terminal(&status) {
                details.push(
                    stored_project_detail(store, project, status, liveness, wave.name()).await?,
                );
            }
            continue;
        };
        if details[index].runtime.is_some() {
            continue;
        }
        details[index].next_move = next_move_for_project(&status);
        details[index].runtime =
            Some(snapshot_project_runtime(store, project, status, liveness, now()).await?);
        details[index].direction =
            current_direction(store, ChildRef::Project(project.id.clone())).await?;
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
        let runtime_task = tasks.iter().find(|task| {
            task.plan.id.as_str() == item.id || task.plan.identifier == item.identifier
        });
        details[index]
            .tasks
            .push(snapshot_task_detail(store, item, runtime_task, liveness, probe_pr_empty).await?);
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
        let project_index =
            match find_project_index(&details, parent.plan.id.as_str(), &parent.plan.slug) {
                Some(index) => index,
                None if work_status_is_terminal(&status) => continue,
                None => {
                    let parent_status =
                        child_work_status(store, &ChildRef::Project(parent.id.clone())).await?;
                    details.push(
                        stored_project_detail(store, parent, parent_status, liveness, wave.name())
                            .await?,
                    );
                    details.len() - 1
                }
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
            project: Some(parent.plan.slug.clone()),
            assignee: None,
        };
        details[project_index].tasks.push(
            snapshot_task_detail(store, item, Some(runtime_task), liveness, probe_pr_empty).await?,
        );
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
    find_project_index(projects, id, slug)
        .ok_or_else(|| {
            anyhow!(
                "Project {slug} ({id}) is not present in the current PM snapshot; run `lf pm sync` before reading the Wave work map"
            )
        })
}

async fn stored_project_detail(
    store: &SharedStore,
    project: &Project,
    status: WorkStatus,
    liveness: &TmuxLiveness,
    wave: &str,
) -> Result<ProjectDetailSnapshot> {
    Ok(ProjectDetailSnapshot {
        project: PmProjectSummary {
            id: project.plan.id.as_str().to_string(),
            slug: project.plan.slug.clone(),
            name: project.plan.name.clone(),
            summary: format!(
                "Project Work is absent from the current PM snapshot; run `lf pm sync --wave {wave}`"
            ),
            definition: project.plan.prompt_context.clone(),
            flows: ProjectFlowPlan::empty(),
            krs: Vec::new(),
        },
        next_move: NextMove {
            owner: NextMoveOwner::Wave,
            reason: "Reconcile Project Work with the current PM snapshot".to_string(),
        },
        runtime: Some(
            snapshot_project_runtime(store, project, status, liveness, now()).await?,
        ),
        direction: current_direction(store, ChildRef::Project(project.id.clone())).await?,
        tasks: Vec::new(),
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
    liveness: &TmuxLiveness,
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
            Some(snapshot_task_runtime(store, task, status, liveness, observed_at).await?)
        }
        None => None,
    };
    let reference = task_reference(&item, task, active, &prs);
    let next_move = match task {
        Some(_) => next_move_for_task(
            &runtime
                .as_ref()
                .expect("Task runtime exists when the durable Task exists")
                .status,
            active.map(TaskPr::phase),
            active.and_then(|pr| pr.fresh_ci()),
            active.and_then(TaskPr::merge_request),
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
    let process = task_process_evidence(runtime.as_ref(), liveness);
    let local_progress = task_local_progress(task, runtime.as_ref(), active, &process);
    let completion_refusal = match (task, runtime.as_ref()) {
        (Some(task), Some(runtime)) if !work_status_is_terminal(&runtime.status) => {
            crate::ops::task::task_completion_gate(store, task)
                .await?
                .refusal(&task.plan.identifier)
        }
        _ => None,
    };
    let resume_refusal = task.and_then(|task| {
        crate::ops::task::no_active_pr_resume_refusal(&task.plan.identifier, active, latest)
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
                    completion_refusal: completion_refusal.as_deref(),
                    resume_refusal: resume_refusal.as_deref(),
                    ci: active.and_then(|pr| pr.fresh_ci()),
                    process_alive: process.alive,
                    predecessor_phase,
                    abandon_intent: task.abandon_intent.is_some(),
                    local_progress_unsettled: local_progress.unsettled,
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
            process,
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

fn task_process_evidence(
    runtime: Option<&TaskRuntimeSnapshot>,
    liveness: &TmuxLiveness,
) -> TaskProcessEvidence {
    let Some(runtime) = runtime else {
        return TaskProcessEvidence {
            state: TaskProcessEvidenceState::NotApplicable,
            alive: None,
            reason: None,
        };
    };
    if !work_status_is_running(&runtime.status) {
        return TaskProcessEvidence {
            state: TaskProcessEvidenceState::NotExpected,
            alive: None,
            reason: None,
        };
    }
    if !liveness.installed {
        return TaskProcessEvidence {
            state: TaskProcessEvidenceState::Unavailable,
            alive: None,
            reason: Some("tmux is unavailable; this machine cannot observe the Task body".into()),
        };
    }
    TaskProcessEvidence {
        state: TaskProcessEvidenceState::Observed,
        alive: Some(runtime.process_alive),
        reason: None,
    }
}

fn task_local_progress(
    task: Option<&Task>,
    runtime: Option<&TaskRuntimeSnapshot>,
    active_pr: Option<&TaskPr>,
    process: &TaskProcessEvidence,
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
        process,
    )
}

fn inspect_task_local_progress(
    status: &WorkStatus,
    worktree: &Path,
    active_pr_base: Option<&str>,
    process: &TaskProcessEvidence,
) -> LocalProgressEvidence {
    let recovery_required = if work_status_is_running(status) {
        process.alive.map(|alive| !alive)
    } else {
        Some(false)
    };
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
        process,
        local_progress,
        user_ask,
    } = evidence;
    let active_pr_phase = action_evidence
        .and_then(|e| e.latest_pr_phase)
        .filter(|phase| phase.is_active());
    let live = process.alive == Some(true);
    let user_attention = next_move.owner == NextMoveOwner::User;
    let (level, reason) = if user_ask {
        (
            TaskAttentionLevel::Blue,
            "Waiting for your answer".to_string(),
        )
    } else if live && user_attention {
        (TaskAttentionLevel::Red, next_move.reason.clone())
    } else if live {
        (TaskAttentionLevel::Green, next_move.reason.clone())
    } else if user_attention {
        (TaskAttentionLevel::Red, next_move.reason.clone())
    } else if process.state == TaskProcessEvidenceState::Unavailable
        && runtime.is_some_and(|runtime| work_status_is_running(&runtime.status))
    {
        (
            TaskAttentionLevel::Unknown,
            process
                .reason
                .clone()
                .unwrap_or_else(|| "Task body evidence is unavailable".into()),
        )
    } else if local_progress.unsettled == Some(true) {
        let reason = if local_progress.dirty == Some(true) {
            "Task body stopped with uncommitted work".to_string()
        } else if local_progress.authored_commits == Some(true) {
            match active_pr_phase {
                Some(PrPhase::Open) | Some(PrPhase::Publishing) => next_move.reason.clone(),
                _ => "Task body stopped with unsettled commits".to_string(),
            }
        } else if let Some(reason) = &local_progress.reason {
            reason.clone()
        } else {
            "no live Task body; local progress requires recovery".to_string()
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
        process,
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
) -> Result<Option<BoundarySeedSnapshot>> {
    let seed = store
        .boundary_seed_for_child(&target)
        .await
        .map_err(|err| anyhow!("failed to read boundary seed: {err}"))?;
    let text = seed.render();
    if text.is_empty() {
        return Ok(None);
    }
    Ok(Some(BoundarySeedSnapshot {
        basis: format!("{}:{}", seed.basis.epoch_id, seed.basis.revision),
        text,
    }))
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
        WorkStatus::Ready | WorkStatus::Running { .. } | WorkStatus::Waiting { .. } => {
            NextMoveOwner::Project
        }
        WorkStatus::Done | WorkStatus::Abandoned => NextMoveOwner::Wave,
    };
    NextMove {
        owner,
        reason: work_status_reason(status),
    }
}

fn next_move_for_task(
    status: &WorkStatus,
    pr_phase: Option<PrPhase>,
    ci: Option<&CiObservation>,
    merge: Option<&PrMergeRequest>,
) -> NextMove {
    if pr_phase == Some(PrPhase::Open) {
        if let Some(ci) = ci {
            let repairable_failure =
                ci.state == CiState::Failing && !ci.only_land_time_preconditions();
            let fixing = work_status_is_running(status) && repairable_failure;
            if fixing {
                return NextMove {
                    owner: NextMoveOwner::Task,
                    reason: "fixing CI".to_string(),
                };
            }
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
    }
    let owner = match status {
        WorkStatus::Running { .. } => NextMoveOwner::Task,
        WorkStatus::Ready
        | WorkStatus::Waiting { .. }
        | WorkStatus::Done
        | WorkStatus::Abandoned => NextMoveOwner::Project,
    };
    NextMove {
        owner,
        reason: work_status_reason(status),
    }
}

/// The invoking context's wave id: `LF_WAVE_ID`, else `None` (the caller
/// errors). Kept minimal — `lf status` with no arg is a convenience, not the
/// resolution surface `lf chat` owns.
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
            status = work_status_label(&wave.status),
            live = if wave.live { "yes" } else { "no" },
            tasks = wave.active_tasks,
            projects = wave.active_projects,
            home = truncate(&wave.home.route, 16),
            endpoint = wave.endpoint.as_deref().unwrap_or("-"),
        );
    }
}

fn work_status_label(status: &WorkStatus) -> &'static str {
    match status {
        WorkStatus::Ready => "ready",
        WorkStatus::Running { .. } => "running",
        WorkStatus::Waiting { .. } => "waiting",
        WorkStatus::Done => "done",
        WorkStatus::Abandoned => "abandoned",
    }
}

fn work_status_is_running(status: &WorkStatus) -> bool {
    matches!(status, WorkStatus::Running { .. })
}

fn work_status_is_terminal(status: &WorkStatus) -> bool {
    matches!(status, WorkStatus::Done | WorkStatus::Abandoned)
}

fn work_status_body_intent(status: &WorkStatus) -> BodyIntent {
    match status {
        WorkStatus::Running { .. } => BodyIntent::Active,
        WorkStatus::Ready | WorkStatus::Waiting { .. } => BodyIntent::Waiting,
        WorkStatus::Done | WorkStatus::Abandoned => BodyIntent::Terminal,
    }
}

fn work_status_reason(status: &WorkStatus) -> String {
    match status {
        WorkStatus::Running { run_id } => format!("Run {run_id} is active"),
        WorkStatus::Waiting { .. } => "waiting for input or an event".to_string(),
        other => work_status_label(other).to_string(),
    }
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
        status = work_status_label(&wave.status),
        loop_state = status
            .loop_state
            .as_deref()
            .map(|m| format!("  loop:{m}"))
            .unwrap_or_default(),
    );
    println!("  goal      {}", wave.goal);
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
    if status.projects.is_empty() {
        println!("  projects  none");
    } else {
        println!("  projects");
        for project in &status.projects {
            let (project_status, iteration, reason) = match &project.runtime {
                Some(runtime) => (
                    work_status_label(&runtime.status),
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
                    Some(runtime) => (work_status_label(&runtime.status), runtime.reason.as_str()),
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

fn print_runs(runs: &Evidence<SkillRunEntry>) {
    match runs {
        Evidence::Unavailable { reason } => println!("  runs unavailable: {reason}"),
        Evidence::Ok { items, .. } if items.is_empty() => {
            println!("  runs       no skills in the ledger window")
        }
        Evidence::Ok { items, truncated } => {
            println!("  runs");
            for run in items {
                println!(
                    "    {label:<24}  {status:<8}  ctx {context:>7}  tok {tokens:>7}  {age:>7} ago",
                    label = truncate(&run.label(), 24),
                    status = run.status,
                    context = format_tokens(run.supplied_context_tokens),
                    tokens = format_tokens(run.total_tokens()),
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
        TaskAttentionLevel::Green => "green",
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
            status = work_status_label(&wave.wave.status),
        );
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

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use time::OffsetDateTime;

    use super::{
        derive_task_attention, next_move_for_task, LocalProgressEvidence,
        LocalProgressEvidenceState, NextMove, NextMoveOwner, TaskAttentionEvidence,
        TaskAttentionLevel, TaskProcessEvidence, TaskProcessEvidenceState, TaskRuntimeSnapshot,
    };
    use crate::child::{observe, BodyEvidence, BodyIntent};
    use crate::durable::WorkStatus;
    use crate::task::{CiObservation, CiState, PrMergeMode, PrMergeRequest, PrPhase};

    #[test]
    fn only_a_durable_ask_marks_a_running_task_as_waiting_on_the_user() {
        let runtime = TaskRuntimeSnapshot {
            work_id: "task-1".to_string(),
            project_id: "project-1".to_string(),
            routing_project_id: Some("project-1".to_string()),
            status: WorkStatus::Running {
                run_id: crate::durable::RunId::new(),
            },
            reason: "Run is active".to_string(),
            updated_at: "2026-07-21T00:00:00Z".to_string(),
            provider: "codex".to_string(),
            process_alive: true,
            observation: observe(
                &BodyEvidence {
                    intent: BodyIntent::Active,
                    observable: true,
                    process_alive: true,
                    progress_age: Duration::ZERO,
                    step: Some("demo".to_string()),
                    reason: "Run is active".to_string(),
                },
                Duration::from_secs(30 * 60),
            ),
        };
        let next_move = NextMove {
            owner: NextMoveOwner::Task,
            reason: "Run is active".to_string(),
        };
        let evidence = |user_ask| TaskAttentionEvidence {
            process: TaskProcessEvidence {
                state: TaskProcessEvidenceState::Observed,
                alive: Some(true),
                reason: None,
            },
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

        assert_eq!(advisory.level, TaskAttentionLevel::Green);
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
        let published = next_move_for_task(
            &WorkStatus::Ready,
            Some(PrPhase::Open),
            Some(&passing),
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
                after_merge: crate::task::AfterMerge::ContinueTask,
                next_slug: None,
            };
            let next = next_move_for_task(
                &WorkStatus::Ready,
                Some(PrPhase::Open),
                Some(&passing),
                Some(&request),
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
            failing_checks: vec![crate::task::CiCheck {
                name: "scratch-clear".to_string(),
                url: None,
            }],
            observed_at: OffsetDateTime::now_utc(),
        };
        let request = PrMergeRequest {
            mode: PrMergeMode::User,
            requested_at: OffsetDateTime::now_utc(),
            head_sha: ci.head_sha.clone(),
            after_merge: crate::task::AfterMerge::ContinueTask,
            next_slug: None,
        };

        let next = next_move_for_task(
            &WorkStatus::Ready,
            Some(PrPhase::Open),
            Some(&ci),
            Some(&request),
        );

        assert_eq!(next.owner, NextMoveOwner::User);
    }
}
