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

use crate::child_session::{
    body_progress_age, observe, BodyEvidence, BodyObservation, ChildRef, DirectiveKind,
    ObservationRecipient, DEFAULT_STALL_AFTER,
};
#[cfg(test)]
use crate::child_session::{BodyCategory, BodyControl, BodyOwner};
use crate::engine::wave_home::{HomeActionDto, HomeRuntimeDto, HomeState, WaveHomeDto};
use crate::interaction_review::{
    InteractionReview, InteractionReviewDisposition, InteractionReviewStatus,
};
use crate::lf::commands::runs::{format_tokens, SkillRunEntry};
use crate::lf::output::Colors;
use crate::pm::{PmItem, PmKr, PmProject};
use crate::project_session::{ProjectSession, ProjectSessionStatus};
use crate::store::{open_existing_store, SharedStore};
use crate::task::{
    AfterMerge, CiObservation, CiState, PrPhase, TaskPr, TaskSession, TaskSessionStatus,
};
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
    /// This wave's agent-backed skill runs, newest first.
    pub runs: Evidence<SkillRunEntry>,
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
    /// The observed state of this Project's current body: durable intent
    /// (`status`) and body observation are separate evidence, never one string.
    pub observation: BodyObservation,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TaskRuntimeSnapshot {
    pub session_id: String,
    pub project_session_id: String,
    /// The live Project Session this Task routes to (successor when the
    /// historical owner is terminal). `None` when the chain is broken. The app
    /// derives "routed to a successor" by comparing this to `project_session_id`.
    pub routing_project_session_id: Option<String>,
    pub status: TaskSessionStatus,
    pub reason: String,
    pub status_at: String,
    pub provider: String,
    pub process_alive: bool,
    /// The observed state of this Task's current body, derived from durable
    /// intent, body liveness, and how long since its last durable event.
    pub observation: BodyObservation,
}

/// The compact Task attention signal shared by terminal and app surfaces. The
/// names are deliberately the product's visual vocabulary: consumers do not
/// reinterpret Session/process combinations into their own colors.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TaskAttentionLevel {
    Green,
    Red,
    Black,
    Unknown,
}

pub use crate::task::actions::{
    ci_failure_reason, derive_task_actions, ReviewGateState, TaskAction, TaskActionEvidence,
    TaskActionModel, TaskActionStatus,
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

/// A Task's shared attention projection and the evidence that proves it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TaskAttentionSnapshot {
    pub level: TaskAttentionLevel,
    pub reason: String,
    /// RFC3339 time process/workspace evidence was sampled.
    pub observed_at: String,
    /// Age of the durable Session state at that sample, if a Session exists.
    pub evidence_age_secs: Option<i64>,
    pub next_owner: NextMoveOwner,
    pub actions: TaskActionModel,
    pub pm_completed: bool,
    pub session_status: Option<TaskSessionStatus>,
    pub process: TaskProcessEvidence,
    pub local_progress: LocalProgressEvidence,
    pub active_pr_phase: Option<PrPhase>,
}

/// Stable references for one Task, shared verbatim by `lf status` and
/// `lf roadmap`. The issue URL is cached PM evidence. Workspace evidence comes
/// from the durable Task Session and outlives its process and final PR.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TaskReferenceSnapshot {
    pub issue_url: Option<String>,
    pub workspace: Option<TaskWorkspaceSnapshot>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TaskWorkspaceSnapshot {
    pub slug: String,
    /// Full branch name from the active PR, or the last recorded PR after the
    /// Task settles. `None` is explicit for legacy Sessions with no PR record.
    pub branch: Option<String>,
    pub worktree: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TaskDetailSnapshot {
    pub task: PmTaskSummary,
    pub reference: TaskReferenceSnapshot,
    pub runtime: Option<TaskRuntimeSnapshot>,
    pub directive: Option<DirectiveSnapshot>,
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
    /// Someone other than the running body must move: review, a human, the
    /// supervising Project or Wave — or the process died mid-flight.
    NeedsAttention,
    /// Filed, not started, not complete — ready for someone to pick up.
    Available,
    /// Done or dormant: terminal Sessions and completed plan rows.
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

/// One Project in the roadmap: its plan, live Project-Session evidence when a
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

/// One Task in the roadmap: plan row, live Task-Session evidence when a Session
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
            .list_project_sessions(Some(wave.id()))
            .await
            .map_err(|err| anyhow!("failed to read Project Sessions: {err}"))?;
        let stored_tasks = store
            .list_task_sessions(Some(wave.id()))
            .await
            .map_err(|err| anyhow!("failed to read Task Sessions: {err}"))?;
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

/// `lf roadmap [wave]` — the machine-wide intent plane. Every Wave (or one, when
/// scoped) with its plan joined to live evidence and each row bucketed into a
/// section. Deterministic and local: one `tmux list-sessions` for the whole
/// read, bounded Git probes for Tasks with Sessions, and no network. `lf status`
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
        // One tmux reading for every Session on the machine, taken once.
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
    let project_sessions = match store.list_project_sessions(Some(wave.id())).await {
        Ok(sessions) => sessions,
        Err(err) => {
            return Evidence::Unavailable {
                reason: format!("failed to read Project Sessions: {err}"),
            }
        }
    };
    let task_sessions = match store.list_task_sessions(Some(wave.id())).await {
        Ok(sessions) => sessions,
        Err(err) => {
            return Evidence::Unavailable {
                reason: format!("failed to read Task Sessions: {err}"),
            }
        }
    };
    // `probe_pr_empty: false` — PR emptiness is `lf status`'s execution detail.
    // Roadmap's bounded Git reads belong only to the shared attention evidence.
    match snapshot_projects(
        store,
        wave,
        project_sessions,
        task_sessions,
        planning,
        liveness,
        false,
    )
    .await
    {
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
/// and a terminal Session is `Later` before its owner is consulted.
fn task_section(task: &TaskDetailSnapshot, liveness: Liveness) -> RoadmapSection {
    let Some(runtime) = &task.runtime else {
        return if task.task.completed {
            RoadmapSection::Later
        } else {
            RoadmapSection::Available
        };
    };
    if liveness.is_gone(runtime.status.is_process_active(), runtime.process_alive) {
        return RoadmapSection::NeedsAttention;
    }
    if runtime.status.is_terminal() {
        return RoadmapSection::Later;
    }
    if runtime.status == TaskSessionStatus::Blocked {
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
    if liveness.is_gone(runtime.status.is_process_active(), runtime.process_alive) {
        return RoadmapSection::NeedsAttention;
    }
    if runtime.status.is_terminal() {
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

/// One tmux reading for the whole command. `lf status` checks a handful of
/// Sessions and `lf roadmap` checks every Session on the machine; both take a
/// single `tmux list-sessions` snapshot here and look each name up in the set,
/// never a `has-session` fork per Session. `installed` is kept distinct from an
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
    store: &SharedStore,
    task: &TaskSession,
    liveness: &TmuxLiveness,
    now: time::OffsetDateTime,
) -> Result<TaskRuntimeSnapshot> {
    let process_alive = task.status.is_process_active()
        && task
            .latest_process
            .as_ref()
            .is_some_and(|process| liveness.is_alive(&process.tmux_name));
    let latest_event_at = store
        .latest_task_event_at(&task.id)
        .await
        .map_err(|err| anyhow!("failed to read Task event log: {err}"))?;
    let evidence = BodyEvidence {
        intent: task.status.body_intent(),
        observable: liveness.liveness() == Liveness::Observable,
        process_alive,
        progress_age: body_progress_age(latest_event_at, task.status_at, now),
        step: Some(task.lifecycle_phase.as_str().to_string()),
        reason: task.status_reason.clone(),
    };
    let routing_project_session_id =
        match crate::ops::project::resolve_task_project_route(store.as_ref(), task).await {
            Ok(route) => Some(route.current.to_string()),
            Err(_) => None,
        };
    Ok(TaskRuntimeSnapshot {
        session_id: task.id.to_string(),
        project_session_id: task.project_session_id.to_string(),
        routing_project_session_id,
        status: task.status,
        reason: task.status_reason.clone(),
        status_at: format_time(task.status_at).unwrap_or_default(),
        provider: task.provider.clone(),
        process_alive,
        observation: observe(&evidence, DEFAULT_STALL_AFTER),
    })
}

async fn snapshot_project_runtime(
    store: &SharedStore,
    project: &ProjectSession,
    liveness: &TmuxLiveness,
    now: time::OffsetDateTime,
) -> Result<ProjectRuntimeSnapshot> {
    let process_alive = project.status.is_process_active()
        && project
            .latest_process
            .as_ref()
            .is_some_and(|process| liveness.is_alive(&process.tmux_name));
    let pending_observations = if project.status.is_terminal() {
        store
            .pending_observations(&ObservationRecipient::Project {
                session_id: project.id.clone(),
            })
            .await
            .map_err(|err| anyhow!("failed to read Project observation outbox: {err}"))?
            .len() as u32
    } else {
        store
            .pending_project_observations_for_chain(project.launch.project.id.as_str())
            .await
            .map_err(|err| anyhow!("failed to read Project observation outbox: {err}"))?
            .len() as u32
    };
    let latest_event_at = store
        .latest_project_event_at(&project.id)
        .await
        .map_err(|err| anyhow!("failed to read Project event log: {err}"))?;
    let evidence = BodyEvidence {
        intent: project.status.body_intent(),
        observable: liveness.liveness() == Liveness::Observable,
        process_alive,
        progress_age: body_progress_age(latest_event_at, project.status_at, now),
        step: Some(format!("iteration {}", project.iteration)),
        reason: project.status_reason.clone(),
    };
    Ok(ProjectRuntimeSnapshot {
        session_id: project.id.to_string(),
        status: project.status,
        reason: project.status_reason.clone(),
        status_at: format_time(project.status_at).unwrap_or_default(),
        iteration: project.iteration,
        pending_observations,
        provider: project.provider.clone(),
        process_alive,
        observation: observe(&evidence, DEFAULT_STALL_AFTER),
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
    project_sessions: Vec<ProjectSession>,
    task_sessions: Vec<TaskSession>,
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
            directive: None,
            tasks: Vec::new(),
        })
        .collect::<Vec<_>>();

    for project_session in &project_sessions {
        let Some(index) = session_project_index(
            &details,
            project_session.launch.project.id.as_str(),
            &project_session.launch.project.slug,
            project_session.status.is_terminal(),
            wave.name(),
            &format!("Project Session {}", project_session.id),
            &format!(
                "lf project abandon {} --reason \"Project is absent from the current PM snapshot\"",
                project_session.launch.project.slug
            ),
        )?
        else {
            continue;
        };
        if details[index].runtime.is_some() {
            continue;
        }
        details[index].next_move =
            next_move_for_project(project_session.status, &project_session.status_reason);
        details[index].runtime =
            Some(snapshot_project_runtime(store, project_session, liveness, now()).await?);
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
        details[index].tasks.push(
            snapshot_task_detail(store, item, runtime_session, liveness, probe_pr_empty).await?,
        );
    }

    for task_session in &task_sessions {
        let Some(project_index) = session_project_index(
            &details,
            task_session.launch.project.id.as_str(),
            &task_session.launch.project.slug,
            task_session.status.is_terminal(),
            wave.name(),
            &format!("Task Session {}", task_session.id),
            &format!(
                "lf task abandon {} --reason \"Project is absent from the current PM snapshot\"",
                task_session.launch.issue.identifier
            ),
        )?
        else {
            continue;
        };
        if details[project_index].tasks.iter().any(|task| {
            task.task.id == task_session.launch.issue.id.as_str()
                || task.task.identifier == task_session.launch.issue.identifier
        }) {
            continue;
        }
        let task = PmItem {
            id: task_session.launch.issue.id.as_str().to_string(),
            identifier: task_session.launch.issue.identifier.clone(),
            url: None,
            name: task_session.launch.issue.title.clone(),
            description: task_session.launch.issue.description.clone(),
            rank: u32::MAX,
            completed: task_session.status.is_terminal(),
            project: Some(task_session.launch.project.slug.clone()),
            assignee: None,
        };
        details[project_index].tasks.push(
            snapshot_task_detail(store, task, Some(task_session), liveness, probe_pr_empty).await?,
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

fn session_project_index(
    projects: &[ProjectDetailSnapshot],
    id: &str,
    slug: &str,
    terminal: bool,
    wave: &str,
    session: &str,
    recovery: &str,
) -> Result<Option<usize>> {
    if let Some(index) = find_project_index(projects, id, slug) {
        return Ok(Some(index));
    }
    if terminal {
        return Ok(None);
    }
    Err(anyhow!(
        "{session} references Project {slug} ({id}), which is absent from the current PM snapshot; run `lf pm sync --wave {wave}`. If the Project remains absent, settle the stale Session with `{recovery}`"
    ))
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
    session: Option<&TaskSession>,
    liveness: &TmuxLiveness,
    probe_pr_empty: bool,
) -> Result<TaskDetailSnapshot> {
    let prs = match session {
        Some(session) => store.task_prs(&session.id).await?,
        None => Vec::new(),
    };
    let active = prs.iter().find(|pr| pr.is_active());
    let observed_at = now();
    let runtime = match session {
        Some(session) => Some(snapshot_task_runtime(store, session, liveness, observed_at).await?),
        None => None,
    };
    let reference = task_reference(&item, session, active, &prs);
    let next_move = match session {
        Some(session) => next_move_for_task(
            session.status,
            active.map(TaskPr::phase),
            active.and_then(|pr| pr.fresh_ci()),
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
    let process = task_process_evidence(session, runtime.as_ref(), liveness);
    let local_progress = task_local_progress(session, active, &process);
    let action_evidence = match session {
        Some(session) => {
            let predecessor_phase = match active.and_then(|pr| pr.parent_pr_id.as_ref()) {
                Some(parent_id) => store.get_task_pr(parent_id).await?.map(|pr| pr.phase()),
                None => None,
            };
            let review_gate = store
                .interaction_review_at(
                    &session.id,
                    session.phase_epoch,
                    session.phase_iteration,
                    session.phase_cursor,
                )
                .await?
                .map(|r| review_gate_from(&r));
            Some(TaskActionEvidence {
                status: session.status,
                active_pr_phase: active.map(TaskPr::phase),
                active_pr_after_merge: active
                    .and_then(|pr| pr.publication.as_ref())
                    .map(|p| p.after_merge),
                active_pr_next_slug: active
                    .and_then(|pr| pr.publication.as_ref())
                    .and_then(|p| p.next_slug.as_deref()),
                ci: active.and_then(|pr| pr.fresh_ci()),
                process_alive: process.alive,
                predecessor_phase,
                review_gate,
                abandon_intent: session.abandon_intent.is_some(),
                local_progress_unsettled: local_progress.unsettled,
            })
        }
        None => None,
    };
    let attention = derive_task_attention(
        item.completed,
        runtime.as_ref(),
        &next_move,
        process,
        local_progress,
        action_evidence.as_ref(),
        observed_at,
    );
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
        reference,
        runtime,
        directive,
        next_move,
        attention,
        prs: prs
            .iter()
            .map(|pr| {
                // PR emptiness is an execution-plane fact (`lf status`); it costs
                // an additional Git comparison, so `lf roadmap` opts out. The
                // attention fold already carries the progress evidence it needs.
                let empty = match (session, active) {
                    (Some(session), Some(active)) if probe_pr_empty && active.id == pr.id => {
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

fn task_process_evidence(
    session: Option<&TaskSession>,
    runtime: Option<&TaskRuntimeSnapshot>,
    liveness: &TmuxLiveness,
) -> TaskProcessEvidence {
    let Some(session) = session else {
        return TaskProcessEvidence {
            state: TaskProcessEvidenceState::NotApplicable,
            alive: None,
            reason: None,
        };
    };
    if !session.status.is_process_active() {
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
        alive: Some(runtime.is_some_and(|runtime| runtime.process_alive)),
        reason: None,
    }
}

fn task_local_progress(
    session: Option<&TaskSession>,
    active_pr: Option<&TaskPr>,
    process: &TaskProcessEvidence,
) -> LocalProgressEvidence {
    let Some(session) = session else {
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
        session.status,
        &session.worktree,
        active_pr.map(|pr| pr.base_commit.as_str()),
        process,
    )
}

fn inspect_task_local_progress(
    status: TaskSessionStatus,
    worktree: &Path,
    active_pr_base: Option<&str>,
    process: &TaskProcessEvidence,
) -> LocalProgressEvidence {
    let recovery_required = if status.is_process_active() {
        process.alive.map(|alive| !alive)
    } else {
        Some(false)
    };
    if !worktree.exists() {
        if status.is_terminal() && active_pr_base.is_none() {
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

fn review_gate_from(review: &InteractionReview) -> ReviewGateState {
    match review.status {
        InteractionReviewStatus::Requested => ReviewGateState::Requested,
        InteractionReviewStatus::Active => ReviewGateState::Active,
        InteractionReviewStatus::Completed => match review.disposition {
            Some(InteractionReviewDisposition::Approved) => ReviewGateState::Approved,
            Some(InteractionReviewDisposition::ChangesRequested) => {
                ReviewGateState::ChangesRequested
            }
            None => ReviewGateState::Approved,
        },
    }
}

fn derive_task_attention(
    pm_completed: bool,
    runtime: Option<&TaskRuntimeSnapshot>,
    next_move: &NextMove,
    process: TaskProcessEvidence,
    local_progress: LocalProgressEvidence,
    action_evidence: Option<&TaskActionEvidence>,
    observed_at: time::OffsetDateTime,
) -> TaskAttentionSnapshot {
    let active_pr_phase = action_evidence.map(|e| e.active_pr_phase).unwrap_or(None);
    let live = process.alive == Some(true);
    let human_handoff = matches!(
        next_move.owner,
        NextMoveOwner::Human | NextMoveOwner::Review
    );
    let failed = runtime.is_some_and(|runtime| runtime.status == TaskSessionStatus::Failed);
    let (level, reason) = if live && human_handoff {
        (TaskAttentionLevel::Red, next_move.reason.clone())
    } else if live {
        (TaskAttentionLevel::Green, next_move.reason.clone())
    } else if human_handoff || failed {
        (TaskAttentionLevel::Red, next_move.reason.clone())
    } else if process.state == TaskProcessEvidenceState::Unavailable
        && runtime.is_some_and(|runtime| runtime.status.is_process_active())
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
        None => TaskActionModel::no_session(),
        Some(evidence) => derive_task_actions(evidence),
    };
    TaskAttentionSnapshot {
        level,
        reason,
        observed_at: format_time(observed_at)
            .expect("Task attention observation time formats as RFC 3339"),
        evidence_age_secs: runtime.and_then(|runtime| age_secs(&runtime.status_at, observed_at)),
        next_owner: next_move.owner,
        actions,
        pm_completed,
        session_status: runtime.map(|runtime| runtime.status),
        process,
        local_progress,
        active_pr_phase,
    }
}

fn task_reference(
    item: &PmItem,
    session: Option<&TaskSession>,
    active_pr: Option<&TaskPr>,
    prs: &[TaskPr],
) -> TaskReferenceSnapshot {
    let workspace = session.map(|session| {
        let branch = active_pr
            .or_else(|| prs.iter().max_by_key(|pr| pr.sequence))
            .map(|pr| pr.branch.clone());
        TaskWorkspaceSnapshot {
            slug: session.workspace_slug.clone(),
            branch,
            worktree: session.worktree.display().to_string(),
        }
    });
    TaskReferenceSnapshot {
        issue_url: item.url.clone(),
        workspace,
    }
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
    ci: Option<&CiObservation>,
    reason: &str,
) -> NextMove {
    // An open PR's next move is CI-derived while a fresh required-check reading
    // exists for the current head; otherwise it is the review/merge gate.
    if pr_phase == Some(PrPhase::Open) {
        // A Blocked task with an open PR is not auto-resumable by ci-fix: the
        // body already could not repair the head, or infrastructure is down.
        // Route to the Project for a new directive or human review instead of
        // silently re-looping ci-fix through Waiting.
        if status == TaskSessionStatus::Blocked {
            return NextMove {
                owner: NextMoveOwner::Project,
                reason: reason.to_string(),
            };
        }
        if let Some(ci) = ci {
            // A live ci-fix generation (Running/Starting) owns the next move:
            // the Task is actively repairing the branch, not waiting for an
            // external CI fix. Failing + idle → Ci (the wake will fire);
            // Passing → Review regardless of process state.
            let fixing = matches!(
                status,
                TaskSessionStatus::Running | TaskSessionStatus::Starting
            ) && ci.state == CiState::Failing;
            if fixing {
                return NextMove {
                    owner: NextMoveOwner::Task,
                    reason: "fixing CI".to_string(),
                };
            }
            return match ci.state {
                CiState::Pending => NextMove {
                    owner: NextMoveOwner::Ci,
                    reason: "required checks are still running".to_string(),
                },
                CiState::Failing => NextMove {
                    owner: NextMoveOwner::Ci,
                    reason: ci_failure_reason(ci),
                },
                CiState::Passing => NextMove {
                    owner: NextMoveOwner::Review,
                    reason: "checks passed; awaiting review".to_string(),
                },
            };
        }
        return NextMove {
            owner: NextMoveOwner::Review,
            reason: reason.to_string(),
        };
    }
    let owner = match status {
        TaskSessionStatus::Created | TaskSessionStatus::Starting | TaskSessionStatus::Running => {
            NextMoveOwner::Task
        }
        TaskSessionStatus::Waiting | TaskSessionStatus::Blocked | TaskSessionStatus::Failed => {
            NextMoveOwner::Project
        }
        TaskSessionStatus::Completed | TaskSessionStatus::Abandoned => NextMoveOwner::Project,
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
            .and_then(|runtime| age_secs(&runtime.status_at, now)),
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
            status = wave.wave.status,
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
                    age_secs: age_secs(&runtime.status_at, now),
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
    use std::{process::Command, sync::Arc};

    use super::*;
    use crate::id::WaveId;
    use crate::project_session::ProjectSessionId;
    use crate::session_context::{
        LinearIssueId, LinearIssueSnapshot, LinearProjectId, LinearProjectSnapshot,
        ProjectLaunchReceipt, TaskLaunchReceipt,
    };
    use crate::store::{open_store, PmSnapshotRow, StorageConfig};
    use crate::task::{PmWritebackState, TaskLifecyclePhase, TaskLifecyclePlan, TaskSessionId};

    fn ci(state: CiState, failing: &[&str]) -> CiObservation {
        CiObservation {
            head_sha: "head".to_string(),
            state,
            failing_checks: failing
                .iter()
                .map(|name| crate::task::CiCheck {
                    name: name.to_string(),
                    url: None,
                })
                .collect(),
            observed_at: time::OffsetDateTime::now_utc(),
        }
    }

    #[test]
    fn open_pr_next_move_is_ci_derived() {
        // Pending required checks: CI owns the next move, no body burned.
        let pending = next_move_for_task(
            TaskSessionStatus::Waiting,
            Some(PrPhase::Open),
            Some(&ci(CiState::Pending, &[])),
            "pull request #900 is open for review",
        );
        assert_eq!(pending.owner, NextMoveOwner::Ci);

        // A required failure: CI owner, and the reason names the failing checks.
        let failing = next_move_for_task(
            TaskSessionStatus::Waiting,
            Some(PrPhase::Open),
            Some(&ci(CiState::Failing, &["build", "lint"])),
            "pull request #900 is open for review",
        );
        assert_eq!(failing.owner, NextMoveOwner::Ci);
        assert!(failing.reason.contains("build"));
        assert!(failing.reason.contains("lint"));

        // Green checks: back to the review/merge gate.
        let passing = next_move_for_task(
            TaskSessionStatus::Waiting,
            Some(PrPhase::Open),
            Some(&ci(CiState::Passing, &[])),
            "pull request #900 is open for review",
        );
        assert_eq!(passing.owner, NextMoveOwner::Review);

        // No CI reading yet: unchanged review waiting.
        let unknown = next_move_for_task(
            TaskSessionStatus::Waiting,
            Some(PrPhase::Open),
            None,
            "pull request #900 is open for review",
        );
        assert_eq!(unknown.owner, NextMoveOwner::Review);
    }

    #[test]
    fn open_pr_failing_with_live_generation_owns_task() {
        // A live ci-fix generation (Running) on a failing open PR: the Task
        // is fixing CI, not waiting for an external fix.
        let fixing = next_move_for_task(
            TaskSessionStatus::Running,
            Some(PrPhase::Open),
            Some(&ci(CiState::Failing, &["build"])),
            "PR #900 is open; waiting for review",
        );
        assert_eq!(fixing.owner, NextMoveOwner::Task);
        assert_eq!(fixing.reason, "fixing CI");

        // Starting (process launching) is also live.
        let starting = next_move_for_task(
            TaskSessionStatus::Starting,
            Some(PrPhase::Open),
            Some(&ci(CiState::Failing, &["build"])),
            "PR #900 is open; waiting for review",
        );
        assert_eq!(starting.owner, NextMoveOwner::Task);

        // Idle (Waiting) + failing: Ci owns — the wake has not fired yet.
        let idle = next_move_for_task(
            TaskSessionStatus::Waiting,
            Some(PrPhase::Open),
            Some(&ci(CiState::Failing, &["build"])),
            "PR #900 is open; waiting for review",
        );
        assert_eq!(idle.owner, NextMoveOwner::Ci);

        // Running + Passing: review gate, not "fixing CI".
        let green = next_move_for_task(
            TaskSessionStatus::Running,
            Some(PrPhase::Open),
            Some(&ci(CiState::Passing, &[])),
            "PR #900 is open; waiting for review",
        );
        assert_eq!(green.owner, NextMoveOwner::Review);
    }

    #[test]
    fn blocked_open_pr_next_move_is_project_not_ci() {
        // A Blocked task with an open PR is not auto-resumable by ci-fix: the
        // body already could not repair the head, or infrastructure is down.
        // Route to the Project for a directive or human review so a failing
        // ci-fix stops silently re-looping through Waiting.
        let blocked = next_move_for_task(
            TaskSessionStatus::Blocked,
            Some(PrPhase::Open),
            Some(&ci(CiState::Failing, &["build"])),
            "CI failing on pull request #900; the Task body did not repair the head",
        );
        assert_eq!(blocked.owner, NextMoveOwner::Project);
        assert!(blocked.reason.contains("did not repair"));

        // The auto ci-fix loop is preserved for a Waiting task: a failing PR
        // that is not Blocked still routes to Ci for another repair attempt.
        let waiting = next_move_for_task(
            TaskSessionStatus::Waiting,
            Some(PrPhase::Open),
            Some(&ci(CiState::Failing, &["build"])),
            "pull request #900 is open for review",
        );
        assert_eq!(waiting.owner, NextMoveOwner::Ci);
    }

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

    fn stored_project_session(
        wave_id: &WaveId,
        project_id: &str,
        project_slug: &str,
        status: ProjectSessionStatus,
    ) -> ProjectSession {
        let now = time::OffsetDateTime::now_utc();
        ProjectSession {
            id: ProjectSessionId::new(),
            launch: ProjectLaunchReceipt {
                project: LinearProjectSnapshot {
                    id: LinearProjectId::new(project_id).unwrap(),
                    slug: project_slug.to_string(),
                    name: project_slug.to_string(),
                    prompt_context: "Definition".to_string(),
                },
                pm_snapshot_synced_at: 1,
            },
            wave_id: wave_id.clone(),
            current_directive_version: 0,
            incorporated_directive_version: 0,
            status,
            status_reason: status.as_str().to_string(),
            status_at: now,
            iteration: 0,
            observation_cursor: 0,
            last_state_fingerprint: None,
            agent: "codex".to_string(),
            provider: "codex".to_string(),
            provider_session_id: None,
            latest_process: None,
            abandon_intent: None,
            created_at: now,
            updated_at: now,
        }
    }

    fn stored_task_session(
        wave_id: &WaveId,
        project_session_id: &ProjectSessionId,
        project_id: &str,
        project_slug: &str,
        status: TaskSessionStatus,
    ) -> TaskSession {
        let now = time::OffsetDateTime::now_utc();
        TaskSession {
            id: TaskSessionId::new(),
            launch: TaskLaunchReceipt {
                issue: LinearIssueSnapshot {
                    id: LinearIssueId::new("issue-134").unwrap(),
                    identifier: "W2-134".to_string(),
                    title: "Archived work".to_string(),
                    description: String::new(),
                },
                project: LinearProjectSnapshot {
                    id: LinearProjectId::new(project_id).unwrap(),
                    slug: project_slug.to_string(),
                    name: project_slug.to_string(),
                    prompt_context: "Definition".to_string(),
                },
                pm_snapshot_synced_at: 1,
            },
            pm_writeback: PmWritebackState::Current,
            wave_id: wave_id.clone(),
            project_session_id: project_session_id.clone(),
            current_directive_version: 0,
            incorporated_directive_version: 0,
            status,
            status_reason: status.as_str().to_string(),
            status_at: now,
            worktree: "/tmp/archived-work".into(),
            workspace_slug: "archived-work".to_string(),
            lifecycle: TaskLifecyclePlan::headless("task"),
            lifecycle_phase: TaskLifecyclePhase::Iterate,
            phase_epoch: 1,
            phase_cursor: 0,
            phase_iteration: 0,
            gate_cycle: 0,
            gate_proposal: None,
            agent: "codex".to_string(),
            provider: "codex".to_string(),
            provider_session_id: None,
            latest_process: None,
            abandon_intent: None,
            observation: crate::task::Observation::NotRequired,
            created_at: now,
            updated_at: now,
        }
    }

    fn unobservable_liveness() -> TmuxLiveness {
        TmuxLiveness {
            installed: false,
            live: std::collections::HashSet::new(),
        }
    }

    #[tokio::test]
    async fn terminal_sessions_for_an_absent_project_are_history_not_current_work() {
        let dir = tempfile::tempdir().expect("tempdir");
        let store = Arc::new(
            open_store(&StorageConfig::sqlite(dir.path().join("registry.db")))
                .await
                .expect("open store"),
        );
        let wave = Wave::new(
            WaveId::new(),
            "product".to_string(),
            dir.path().display().to_string(),
        );
        let project = stored_project_session(
            wave.id(),
            "project-performance-id",
            "product-performance",
            ProjectSessionStatus::Abandoned,
        );
        let task = stored_task_session(
            wave.id(),
            &project.id,
            "project-performance-id",
            "product-performance",
            TaskSessionStatus::Completed,
        );

        let projects = snapshot_projects(
            &store,
            &wave,
            vec![project],
            vec![task],
            CachedPmSnapshot::default(),
            &unobservable_liveness(),
            false,
        )
        .await
        .expect("terminal history must not poison current status");

        assert!(projects.is_empty());
    }

    #[tokio::test]
    async fn nonterminal_sessions_for_an_absent_project_fail_with_recovery_commands() {
        let dir = tempfile::tempdir().expect("tempdir");
        let store = Arc::new(
            open_store(&StorageConfig::sqlite(dir.path().join("registry.db")))
                .await
                .expect("open store"),
        );
        let wave = Wave::new(
            WaveId::new(),
            "product".to_string(),
            dir.path().display().to_string(),
        );
        let project = stored_project_session(
            wave.id(),
            "project-performance-id",
            "product-performance",
            ProjectSessionStatus::Waiting,
        );

        let project_error = snapshot_projects(
            &store,
            &wave,
            vec![project.clone()],
            Vec::new(),
            CachedPmSnapshot::default(),
            &unobservable_liveness(),
            false,
        )
        .await
        .expect_err("live missing Project must remain explicit");
        let project_error = project_error.to_string();
        assert!(project_error.contains(project.id.as_str()));
        assert!(project_error.contains("lf pm sync --wave product"));
        assert!(project_error.contains("lf project abandon product-performance"));

        let task = stored_task_session(
            wave.id(),
            &project.id,
            "project-performance-id",
            "product-performance",
            TaskSessionStatus::Waiting,
        );
        let task_error = snapshot_projects(
            &store,
            &wave,
            Vec::new(),
            vec![task.clone()],
            CachedPmSnapshot::default(),
            &unobservable_liveness(),
            false,
        )
        .await
        .expect_err("live Task under a missing Project must remain explicit");
        let task_error = task_error.to_string();
        assert!(task_error.contains(task.id.as_str()));
        assert!(task_error.contains("lf pm sync --wave product"));
        assert!(task_error.contains("lf task abandon W2-134"));
    }

    #[tokio::test]
    async fn fresh_pm_snapshot_repairs_the_status_hierarchy_without_a_restart() {
        let dir = tempfile::tempdir().expect("tempdir");
        let repo = std::fs::canonicalize(dir.path()).expect("canonical repo");
        let store = Arc::new(
            open_store(&StorageConfig::sqlite(dir.path().join("registry.db")))
                .await
                .expect("open store"),
        );
        let wave = Wave::new(
            WaveId::new(),
            "product".to_string(),
            repo.display().to_string(),
        );
        let project = stored_project_session(
            wave.id(),
            "project-performance-id",
            "product-performance",
            ProjectSessionStatus::Waiting,
        );
        store
            .put_pm_snapshot(PmSnapshotRow {
                repo: repo.display().to_string(),
                wave: wave.name().to_string(),
                provider: "linear".to_string(),
                initiative: "initiative-1".to_string(),
                synced_at: 1,
                payload: serde_json::json!({"projects": [], "items": []}).to_string(),
            })
            .await
            .expect("write stale snapshot");
        let stale = read_pm_planning(&store, &wave)
            .await
            .expect("read stale planning")
            .expect("snapshot present");
        snapshot_projects(
            &store,
            &wave,
            vec![project.clone()],
            Vec::new(),
            stale,
            &unobservable_liveness(),
            false,
        )
        .await
        .expect_err("stale snapshot must expose the missing live Project");

        store
            .put_pm_snapshot(PmSnapshotRow {
                repo: repo.display().to_string(),
                wave: wave.name().to_string(),
                provider: "linear".to_string(),
                initiative: "initiative-1".to_string(),
                synced_at: 2,
                payload: serde_json::json!({
                    "projects": [{
                        "id": "project-performance-id",
                        "slug": "product-performance",
                        "name": "Product performance",
                        "summary": "Keep the product fast",
                        "definition": "Interactive surfaces stay responsive",
                        "krs": [{"text": "Status loads immediately", "holds": false}],
                        "initiative_ids": ["initiative-1"]
                    }],
                    "items": [{
                        "id": "issue-261",
                        "identifier": "W2-261",
                        "url": null,
                        "name": "Repair status",
                        "description": "Restore current planning",
                        "rank": 1,
                        "completed": false,
                        "project": "product-performance",
                        "assignee": null
                    }]
                })
                .to_string(),
            })
            .await
            .expect("write refreshed snapshot");

        let refreshed = read_pm_planning(&store, &wave)
            .await
            .expect("read refreshed planning")
            .expect("snapshot present");
        let projects = snapshot_projects(
            &store,
            &wave,
            vec![project.clone()],
            Vec::new(),
            refreshed,
            &unobservable_liveness(),
            false,
        )
        .await
        .expect("fresh snapshot must rebuild status without restarting the Session");

        assert_eq!(projects.len(), 1);
        assert_eq!(projects[0].project.slug, "product-performance");
        assert_eq!(projects[0].project.krs.len(), 1);
        assert!(!projects[0].project.krs[0].holds);
        assert_eq!(projects[0].tasks.len(), 1);
        assert_eq!(projects[0].tasks[0].task.identifier, "W2-261");
        assert!(!projects[0].tasks[0].task.completed);
        assert_eq!(
            projects[0]
                .runtime
                .as_ref()
                .map(|runtime| runtime.session_id.as_str()),
            Some(project.id.as_str())
        );
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
                            "url": "https://linear.app/loopflow/issue/INF-123/fix-parser",
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
                            "url": null,
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

        let planning = read_pm_planning(&store, &wave)
            .await
            .expect("read planning")
            .expect("snapshot present");
        let liveness = TmuxLiveness {
            installed: false,
            live: std::collections::HashSet::new(),
        };
        let projects = snapshot_projects(
            &store,
            &wave,
            Vec::new(),
            Vec::new(),
            planning,
            &liveness,
            true,
        )
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
                    reference: TaskReferenceSnapshot {
                        issue_url: None,
                        workspace: None,
                    },
                    runtime: None,
                    directive: None,
                    next_move: NextMove {
                        owner: NextMoveOwner::Project,
                        reason: "Task is ready to start".into(),
                    },
                    attention: test_task_attention(
                        false,
                        None,
                        &NextMove {
                            owner: NextMoveOwner::Project,
                            reason: "Task is ready to start".into(),
                        },
                    ),
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
        let runs: Evidence<SkillRunEntry> =
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
        let attention = test_task_attention(false, runtime.as_ref(), &next_move);
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
            reference: TaskReferenceSnapshot {
                issue_url: None,
                workspace: runtime.as_ref().map(|_| TaskWorkspaceSnapshot {
                    slug: "task-workspace".to_string(),
                    branch: Some("jack/task-workspace".to_string()),
                    worktree: "/repo.task-workspace".to_string(),
                }),
            },
            runtime,
            directive: None,
            next_move,
            attention,
            prs: Vec::new(),
            active_pr: None,
        }
    }

    fn test_task_attention(
        pm_completed: bool,
        runtime: Option<&TaskRuntimeSnapshot>,
        next_move: &NextMove,
    ) -> TaskAttentionSnapshot {
        let process = match runtime {
            Some(runtime) if runtime.status.is_process_active() => TaskProcessEvidence {
                state: TaskProcessEvidenceState::Observed,
                alive: Some(runtime.process_alive),
                reason: None,
            },
            Some(_) => TaskProcessEvidence {
                state: TaskProcessEvidenceState::NotExpected,
                alive: None,
                reason: None,
            },
            None => TaskProcessEvidence {
                state: TaskProcessEvidenceState::NotApplicable,
                alive: None,
                reason: None,
            },
        };
        let recovery_required = runtime
            .filter(|runtime| runtime.status.is_process_active())
            .map(|runtime| !runtime.process_alive);
        let local_progress = LocalProgressEvidence {
            state: if runtime.is_some() {
                LocalProgressEvidenceState::Observed
            } else {
                LocalProgressEvidenceState::NotApplicable
            },
            unsettled: Some(recovery_required == Some(true)),
            dirty: runtime.map(|_| false),
            authored_commits: runtime.map(|_| false),
            recovery_required,
            reason: None,
        };
        let action_evidence = runtime.map(|r| TaskActionEvidence {
            status: r.status,
            active_pr_phase: None,
            active_pr_after_merge: None,
            active_pr_next_slug: None,
            ci: None,
            process_alive: process.alive,
            predecessor_phase: None,
            review_gate: None,
            abandon_intent: false,
            local_progress_unsettled: local_progress.unsettled,
        });
        derive_task_attention(
            pm_completed,
            runtime,
            next_move,
            process,
            local_progress,
            action_evidence.as_ref(),
            now(),
        )
    }

    fn local_progress(
        unsettled: Option<bool>,
        dirty: Option<bool>,
        authored_commits: Option<bool>,
        recovery_required: Option<bool>,
    ) -> LocalProgressEvidence {
        LocalProgressEvidence {
            state: if unsettled.is_some() {
                LocalProgressEvidenceState::Observed
            } else {
                LocalProgressEvidenceState::Unavailable
            },
            unsettled,
            dirty,
            authored_commits,
            recovery_required,
            reason: unsettled
                .is_none()
                .then(|| "failed to inspect Task worktree".to_string()),
        }
    }

    fn process(state: TaskProcessEvidenceState, alive: Option<bool>) -> TaskProcessEvidence {
        TaskProcessEvidence {
            state,
            alive,
            reason: (state == TaskProcessEvidenceState::Unavailable)
                .then(|| "tmux is unavailable; this machine cannot observe the Task body".into()),
        }
    }

    fn projected_attention(
        completed: bool,
        runtime: Option<&TaskRuntimeSnapshot>,
        owner: NextMoveOwner,
        reason: &str,
        phase: Option<PrPhase>,
        process: TaskProcessEvidence,
        local_progress: LocalProgressEvidence,
    ) -> TaskAttentionSnapshot {
        let action_evidence = runtime.map(|r| TaskActionEvidence {
            status: r.status,
            active_pr_phase: phase,
            active_pr_after_merge: None,
            active_pr_next_slug: None,
            ci: None,
            process_alive: process.alive,
            predecessor_phase: None,
            review_gate: None,
            abandon_intent: false,
            local_progress_unsettled: local_progress.unsettled,
        });
        derive_task_attention(
            completed,
            runtime,
            &NextMove {
                owner,
                reason: reason.into(),
            },
            process,
            local_progress,
            action_evidence.as_ref(),
            now(),
        )
    }

    #[test]
    fn shared_attention_projection_covers_the_desktop_decision_table() {
        let running = task_runtime(TaskSessionStatus::Running, "implementing", at(30), true);
        let green = projected_attention(
            false,
            Some(&running),
            NextMoveOwner::Task,
            "implementing",
            Some(PrPhase::Working),
            process(TaskProcessEvidenceState::Observed, Some(true)),
            local_progress(Some(false), Some(false), Some(false), Some(false)),
        );
        assert_eq!(green.level, TaskAttentionLevel::Green);
        assert_eq!(green.actions.recommended, Some(TaskAction::NoAction));

        let human = projected_attention(
            false,
            Some(&running),
            NextMoveOwner::Human,
            "choose the recovery boundary",
            Some(PrPhase::Working),
            process(TaskProcessEvidenceState::Observed, Some(true)),
            local_progress(Some(false), Some(false), Some(false), Some(false)),
        );
        assert_eq!(human.level, TaskAttentionLevel::Red);
        assert_eq!(human.reason, "choose the recovery boundary");

        let dead = task_runtime(TaskSessionStatus::Running, "implementing", at(300), false);
        let dirty = projected_attention(
            false,
            Some(&dead),
            NextMoveOwner::Task,
            "implementing",
            Some(PrPhase::Working),
            process(TaskProcessEvidenceState::Observed, Some(false)),
            local_progress(Some(true), Some(true), Some(false), Some(true)),
        );
        assert_eq!(dirty.level, TaskAttentionLevel::Red);
        assert_eq!(dirty.reason, "Task body stopped with uncommitted work");

        let commits = projected_attention(
            false,
            Some(&dead),
            NextMoveOwner::Review,
            "checks passed; awaiting review",
            Some(PrPhase::Open),
            process(TaskProcessEvidenceState::NotExpected, None),
            local_progress(Some(true), Some(false), Some(true), Some(false)),
        );
        assert_eq!(commits.level, TaskAttentionLevel::Red);
        assert_eq!(commits.reason, "checks passed; awaiting review");
        assert_eq!(
            commits.actions.recommended,
            Some(TaskAction::Review),
            "waiting on a passing open PR must advertise Review, not Resume"
        );
        assert!(
            !commits
                .actions
                .status(TaskAction::Resume)
                .unwrap()
                .available,
            "Resume must be blocked when the PR is open and awaiting review"
        );

        let backlog = projected_attention(
            false,
            None,
            NextMoveOwner::Project,
            "Task is ready to start",
            None,
            process(TaskProcessEvidenceState::NotApplicable, None),
            local_progress(Some(false), None, None, None),
        );
        assert_eq!(backlog.level, TaskAttentionLevel::Black);
        assert_eq!(backlog.actions.recommended, None);

        let completed_runtime =
            task_runtime(TaskSessionStatus::Completed, "merged", at(600), false);
        let completed = projected_attention(
            true,
            Some(&completed_runtime),
            NextMoveOwner::Project,
            "Linear Task is complete",
            None,
            process(TaskProcessEvidenceState::NotExpected, None),
            local_progress(Some(false), Some(false), Some(false), Some(false)),
        );
        assert_eq!(completed.level, TaskAttentionLevel::Black);
        assert_eq!(completed.actions.recommended, Some(TaskAction::NoAction));

        let stale = projected_attention(
            false,
            Some(&dead),
            NextMoveOwner::Task,
            "implementing",
            Some(PrPhase::Working),
            process(TaskProcessEvidenceState::Observed, Some(false)),
            local_progress(Some(true), Some(false), Some(false), Some(true)),
        );
        assert_eq!(stale.level, TaskAttentionLevel::Red);
        assert_eq!(
            stale.reason,
            "no live Task body; local progress requires recovery"
        );

        let unavailable = projected_attention(
            false,
            Some(&dead),
            NextMoveOwner::Task,
            "implementing",
            Some(PrPhase::Working),
            process(TaskProcessEvidenceState::Observed, Some(false)),
            local_progress(None, None, None, Some(true)),
        );
        assert_eq!(unavailable.level, TaskAttentionLevel::Unknown);
        assert_eq!(unavailable.reason, "failed to inspect Task worktree");

        let unobservable = projected_attention(
            false,
            Some(&running),
            NextMoveOwner::Task,
            "implementing",
            Some(PrPhase::Working),
            process(TaskProcessEvidenceState::Unavailable, None),
            local_progress(None, Some(false), Some(false), None),
        );
        assert_eq!(unobservable.level, TaskAttentionLevel::Unknown);
    }

    fn git(repo: &Path, args: &[&str]) -> String {
        let output = Command::new("git")
            .current_dir(repo)
            .args(args)
            .output()
            .expect("run git");
        assert!(
            output.status.success(),
            "git {:?} failed: {}",
            args,
            String::from_utf8_lossy(&output.stderr)
        );
        String::from_utf8(output.stdout)
            .expect("git stdout is UTF-8")
            .trim()
            .to_string()
    }

    #[test]
    fn local_progress_reads_dirty_and_authored_commits_from_git() {
        let repo = tempfile::tempdir().expect("temp repo");
        git(repo.path(), &["init"]);
        git(repo.path(), &["config", "user.name", "Loopflow Test"]);
        git(
            repo.path(),
            &["config", "user.email", "loopflow@example.com"],
        );
        std::fs::write(repo.path().join("state.txt"), "base\n").expect("write base");
        git(repo.path(), &["add", "state.txt"]);
        git(repo.path(), &["commit", "-m", "base"]);
        let base = git(repo.path(), &["rev-parse", "HEAD"]);
        let live = process(TaskProcessEvidenceState::Observed, Some(true));

        let clean = inspect_task_local_progress(
            TaskSessionStatus::Running,
            repo.path(),
            Some(&base),
            &live,
        );
        assert_eq!(clean.unsettled, Some(false));
        assert_eq!(clean.dirty, Some(false));
        assert_eq!(clean.authored_commits, Some(false));

        std::fs::write(repo.path().join("state.txt"), "dirty\n").expect("write change");
        let dirty = inspect_task_local_progress(
            TaskSessionStatus::Running,
            repo.path(),
            Some(&base),
            &live,
        );
        assert_eq!(dirty.unsettled, Some(true));
        assert_eq!(dirty.dirty, Some(true));

        git(repo.path(), &["add", "state.txt"]);
        git(repo.path(), &["commit", "-m", "authored"]);
        let committed = inspect_task_local_progress(
            TaskSessionStatus::Waiting,
            repo.path(),
            Some(&base),
            &process(TaskProcessEvidenceState::NotExpected, None),
        );
        assert_eq!(committed.unsettled, Some(true));
        assert_eq!(committed.dirty, Some(false));
        assert_eq!(committed.authored_commits, Some(true));
    }

    #[test]
    fn shared_attention_fixture_pins_every_desktop_state() {
        let fixture = include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../tests/fixtures/dto/task_attention_states.json"
        ));
        let tasks: std::collections::BTreeMap<String, RoadmapTask> =
            serde_json::from_str(fixture).expect("decode shared attention fixture");

        assert_eq!(tasks.len(), 8);
        assert_eq!(
            tasks["live_advancing"].attention.level,
            TaskAttentionLevel::Green
        );
        assert_eq!(
            tasks["live_human_wait"].attention.level,
            TaskAttentionLevel::Red
        );
        assert_eq!(
            tasks["dead_dirty"].attention.local_progress.dirty,
            Some(true)
        );
        assert_eq!(
            tasks["dead_authored_commits"]
                .attention
                .local_progress
                .authored_commits,
            Some(true)
        );
        assert_eq!(
            tasks["clean_backlog"].attention.level,
            TaskAttentionLevel::Black
        );
        assert_eq!(
            tasks["completed"].attention.level,
            TaskAttentionLevel::Black
        );
        assert_eq!(
            tasks["stale"].attention.local_progress.recovery_required,
            Some(true)
        );
        assert_eq!(
            tasks["unavailable"].attention.level,
            TaskAttentionLevel::Unknown
        );

        // The leased body observation now rides the runtime snapshot on the wire,
        // separate from the durable attention level: a live body advancing reads
        // Working; a live body wedged past its progress deadline reads Stalled and
        // hands ownership to Loopflow with Extend/Interrupt/Stop.
        assert_eq!(
            tasks["live_advancing"]
                .runtime
                .as_ref()
                .unwrap()
                .observation
                .category,
            BodyCategory::Working
        );
        let stalled = &tasks["live_human_wait"]
            .runtime
            .as_ref()
            .unwrap()
            .observation;
        assert_eq!(stalled.category, BodyCategory::Stalled);
        assert_eq!(stalled.owner, BodyOwner::Loopflow);
        assert!(stalled.controls.contains(&BodyControl::Extend));
        assert!(stalled.deadline_in_secs.unwrap() < 0);

        let dirty_row = task_roadmap_row(&tasks["dead_dirty"], now());
        assert_eq!(dirty_row.attention, Some(TaskAttentionLevel::Red));
        assert_eq!(
            dirty_row.reason, tasks["dead_dirty"].attention.reason,
            "CLI rows must print the shared attention reason verbatim"
        );
    }

    fn task_runtime(
        status: TaskSessionStatus,
        reason: &str,
        status_at: String,
        process_alive: bool,
    ) -> TaskRuntimeSnapshot {
        let observation = observe(
            &BodyEvidence {
                intent: status.body_intent(),
                observable: true,
                process_alive,
                progress_age: std::time::Duration::from_secs(60),
                step: Some("iterate".to_string()),
                reason: reason.to_string(),
            },
            DEFAULT_STALL_AFTER,
        );
        TaskRuntimeSnapshot {
            session_id: format!("ts_{}", status.as_str()),
            project_session_id: "ps_1".to_string(),
            routing_project_session_id: Some("ps_1".to_string()),
            status,
            reason: reason.to_string(),
            status_at,
            provider: "claude".to_string(),
            process_alive,
            observation,
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

    fn project_runtime(
        status: ProjectSessionStatus,
        status_at: String,
        process_alive: bool,
    ) -> ProjectRuntimeSnapshot {
        let observation = observe(
            &BodyEvidence {
                intent: status.body_intent(),
                observable: true,
                process_alive,
                progress_age: std::time::Duration::from_secs(60),
                step: Some("iteration 1".to_string()),
                reason: "r".to_string(),
            },
            DEFAULT_STALL_AFTER,
        );
        ProjectRuntimeSnapshot {
            session_id: "ps_1".to_string(),
            status,
            reason: "r".to_string(),
            status_at,
            iteration: 1,
            pending_observations: 0,
            provider: "codex".to_string(),
            process_alive,
            observation,
        }
    }

    #[test]
    fn roadmap_fixture_round_trips_every_section_and_the_unavailable_wave() {
        let fixture = include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../tests/fixtures/dto/roadmap_snapshot.json"
        ));
        let snapshot: RoadmapSnapshot = serde_json::from_str(fixture).unwrap();

        // Re-serialize; the shape is stable (no dropped fields, no defaults).
        let reparsed: RoadmapSnapshot =
            serde_json::from_str(&serde_json::to_string(&snapshot).unwrap()).unwrap();
        assert_eq!(reparsed.waves.len(), 2);

        let product = &snapshot.waves[0];
        assert_eq!(product.wave.name, "product");
        let Evidence::Ok { items, .. } = &product.projects else {
            panic!("product plan must be readable");
        };
        let tasks = &items[0].tasks;
        let sections: Vec<RoadmapSection> = tasks.iter().map(|task| task.section).collect();
        assert_eq!(
            sections,
            vec![
                RoadmapSection::Now,
                RoadmapSection::NeedsAttention,
                RoadmapSection::Available,
                RoadmapSection::Later,
            ]
        );
        // The review row carries its live PR number; the available row has no
        // Session at all (never a dead process).
        assert_eq!(tasks[1].active_pr.as_ref().unwrap().phase, PrPhase::Open);
        assert!(tasks[2].runtime.is_none());
        assert!(tasks[2].reference.workspace.is_none());
        assert_eq!(
            tasks[3]
                .reference
                .workspace
                .as_ref()
                .and_then(|workspace| workspace.branch.as_deref()),
            Some("jack-heart/now-available-research")
        );

        // The second wave has no local plan — unavailable, not an empty plan.
        let context = &snapshot.waves[1];
        let Evidence::Unavailable { reason } = &context.projects else {
            panic!("context plan must be unavailable, not empty");
        };
        assert!(reason.contains("lf pm sync"));
    }

    #[test]
    fn task_section_reads_the_row_the_same_way_every_surface_would() {
        let observable = Liveness::Observable;

        // No Session: ready to start, or already done — never a dead process.
        let mut unstarted = task_detail(
            "W2-1",
            None,
            NextMove {
                owner: NextMoveOwner::Project,
                reason: "Task is ready to start".into(),
            },
        );
        assert_eq!(
            task_section(&unstarted, observable),
            RoadmapSection::Available
        );
        unstarted.task.completed = true;
        assert_eq!(task_section(&unstarted, observable), RoadmapSection::Later);

        // A live, self-owned running body is Now.
        let running = task_detail(
            "W2-2",
            Some(task_runtime(
                TaskSessionStatus::Running,
                "step",
                at(60),
                true,
            )),
            NextMove {
                owner: NextMoveOwner::Task,
                reason: "step".into(),
            },
        );
        assert_eq!(task_section(&running, observable), RoadmapSection::Now);

        // Owner is someone else (review) → Needs attention.
        let in_review = task_detail(
            "W2-3",
            Some(task_runtime(
                TaskSessionStatus::Waiting,
                "pr",
                at(60),
                false,
            )),
            NextMove {
                owner: NextMoveOwner::Review,
                reason: "pr open".into(),
            },
        );
        assert_eq!(
            task_section(&in_review, observable),
            RoadmapSection::NeedsAttention
        );

        // A durable dependency block is filed work, but nobody should act on it
        // yet; it stays in Later instead of manufacturing attention.
        let blocked = task_detail(
            "W2-31",
            Some(task_runtime(
                TaskSessionStatus::Blocked,
                "waiting for W2-30",
                at(60),
                false,
            )),
            NextMove {
                owner: NextMoveOwner::Project,
                reason: "waiting for W2-30".into(),
            },
        );
        assert_eq!(task_section(&blocked, observable), RoadmapSection::Later);

        // Terminal Session → Later.
        let done = task_detail(
            "W2-4",
            Some(task_runtime(
                TaskSessionStatus::Completed,
                "merged",
                at(60),
                false,
            )),
            NextMove {
                owner: NextMoveOwner::Project,
                reason: "merged".into(),
            },
        );
        assert_eq!(task_section(&done, observable), RoadmapSection::Later);

        // A running record whose process is gone outranks everything: the audit
        // finding is Needs attention, not Now.
        let ghost = task_detail(
            "W2-5",
            Some(task_runtime(
                TaskSessionStatus::Running,
                "step",
                at(60),
                false,
            )),
            NextMove {
                owner: NextMoveOwner::Task,
                reason: "step".into(),
            },
        );
        assert_eq!(
            task_section(&ghost, observable),
            RoadmapSection::NeedsAttention
        );
        // Unobservable: the same dead-looking row is NOT asserted gone.
        assert_eq!(
            task_section(&ghost, Liveness::Unknowable),
            RoadmapSection::Now
        );
    }

    #[test]
    fn project_section_treats_an_unstarted_project_as_available_unless_it_already_holds() {
        let observable = Liveness::Observable;

        let ready = project_detail(
            "loopflow-api",
            None,
            NextMove {
                owner: NextMoveOwner::Wave,
                reason: "Project is ready to start".into(),
            },
            Vec::new(),
        );
        assert_eq!(
            project_section(&ready, observable),
            RoadmapSection::Available
        );

        let mut held = ready;
        held.project.krs = vec![PmKrSummary {
            text: "holds".into(),
            holds: true,
        }];
        assert_eq!(project_section(&held, observable), RoadmapSection::Later);

        let running = project_detail(
            "loopflow-api",
            Some(project_runtime(ProjectSessionStatus::Running, at(60), true)),
            NextMove {
                owner: NextMoveOwner::Project,
                reason: "advancing".into(),
            },
            Vec::new(),
        );
        assert_eq!(project_section(&running, observable), RoadmapSection::Now);

        let blocked = project_detail(
            "loopflow-api",
            Some(project_runtime(ProjectSessionStatus::Blocked, at(60), true)),
            NextMove {
                owner: NextMoveOwner::Wave,
                reason: "blocked".into(),
            },
            Vec::new(),
        );
        assert_eq!(
            project_section(&blocked, observable),
            RoadmapSection::NeedsAttention
        );
    }

    #[test]
    fn task_identifier_is_a_safe_clickable_terminal_link() {
        let url = "https://linear.app/loopflow/issue/W2-144/make-lf-roadmap";
        let linked = task_identifier_label("W2-144", Some(url), 12, true);
        assert!(linked.contains("\x1b]8;;https://linear.app/"));
        assert!(linked.contains("W2-144"));

        assert_eq!(
            task_identifier_label("W2-144", Some(url), 12, false),
            "W2-144      "
        );
        assert_eq!(
            task_identifier_label("W2-144", Some("javascript:bad"), 12, true),
            "W2-144      "
        );
    }
}
