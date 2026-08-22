//! Compatibility types shared by Project and Task control surfaces.

use std::path::PathBuf;
use std::time::Duration;

use serde::{Deserialize, Serialize};
use time::OffsetDateTime;

use crate::durable::{RunLivenessEvidence, WaitOn, WorkRef, WorkStatus};
use crate::id::WaveId;
use crate::project::ProjectId;
use crate::store::{Store, StoreError, StoreResult};
use crate::task::TaskId;

macro_rules! prefixed_uuid_id {
    ($name:ident, $prefix:literal, $error:ty, $invalid:path) => {
        #[derive(Debug, Clone, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
        #[serde(transparent)]
        pub struct $name(String);

        impl $name {
            pub fn new() -> Self {
                Self(format!("{}{}", $prefix, uuid::Uuid::new_v4().simple()))
            }

            pub fn parse(value: &str) -> Result<Self, $error> {
                let suffix = value
                    .strip_prefix($prefix)
                    .ok_or_else(|| $invalid(format!("expected {} id", $prefix)))?;
                uuid::Uuid::parse_str(suffix).map_err(|error| $invalid(error.to_string()))?;
                Ok(Self::from_raw(value.to_string()))
            }

            pub fn as_str(&self) -> &str {
                &self.0
            }

            pub(crate) fn from_raw(value: impl Into<String>) -> Self {
                Self(value.into())
            }
        }

        impl Default for $name {
            fn default() -> Self {
                Self::new()
            }
        }

        impl std::fmt::Display for $name {
            fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                formatter.write_str(&self.0)
            }
        }

        impl std::str::FromStr for $name {
            type Err = $error;

            fn from_str(value: &str) -> Result<Self, Self::Err> {
                Self::parse(value)
            }
        }
    };
}

pub(crate) use prefixed_uuid_id;

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum ChildDataError {
    #[error("invalid child Work id: {0}")]
    InvalidId(String),
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum ObservationRecipient {
    Wave { wave_id: WaveId },
    Project { project_id: ProjectId },
}

/// Requested replacement of the agent/provider body acting for a durable
/// Project or Task.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ChildBodyHandoffRequest {
    pub agent: String,
    pub provider: String,
    pub reason: String,
}

/// Typed audit record for a body handoff. Work and Run identity are intentionally
/// absent: they do not change during this transition.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ChildBodyHandoff {
    pub from_agent: String,
    pub to_agent: String,
    pub from_provider: String,
    pub to_provider: String,
    pub reason: String,
}

/// The `lf` binary, store, and home a child Work launch uses, resolved fresh at
/// the launch boundary from the current Home — never persisted as Work state. A
/// transient bundle carried from the Ask session to the tmux spawn; Work no longer
/// pins a binary of its own.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ChildExecutionContext {
    pub lf_bin: PathBuf,
    pub db_path: PathBuf,
    pub lf_home: PathBuf,
}

/// A recorded request to end Work, durable from the moment the Abandon
/// command is queued rather than from the moment a runner consumes it.
///
/// The gap between those two moments is where a supervisor used to restart a
/// Work item someone had already ended: the pending command carried the intent,
/// but the Work row still read `Running`, and every launch path reads that Work.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AbandonIntent {
    pub requested_at: OffsetDateTime,
    pub reason: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", content = "id", rename_all = "snake_case")]
pub enum ChildRef {
    Project(ProjectId),
    Task(TaskId),
}

impl ChildRef {
    pub fn target_kind(&self) -> &'static str {
        match self {
            Self::Project(_) => "project",
            Self::Task(_) => "task",
        }
    }

    pub fn target_id(&self) -> &str {
        match self {
            Self::Project(id) => id.as_str(),
            Self::Task(id) => id.as_str(),
        }
    }
}

// ── Current Work observation ────────────────────────────────────────────────
//
// Work status is durable intent. A Run receipt is Home-owned process evidence.
// `CurrentWorkObservation` is their one presentation projection, never another
// stored lifecycle.

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
#[non_exhaustive]
pub enum CurrentWorkState {
    Working,
    Stalled,
    /// Active intent paired with fresh proof that its body is absent. Normal
    /// Home reconciliation atomically settles this shape to `Ready`; this
    /// variant keeps interrupted or inconsistent read evidence honest.
    Stopped,
    Unobservable,
    Ready,
    Waiting,
    Done,
    Abandoned,
}

impl std::fmt::Display for CurrentWorkState {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(match self {
            Self::Working => "working",
            Self::Stalled => "stalled",
            Self::Stopped => "stopped",
            Self::Unobservable => "unobservable",
            Self::Ready => "ready",
            Self::Waiting => "waiting",
            Self::Done => "done",
            Self::Abandoned => "abandoned",
        })
    }
}

/// Who must act next on the body. Separate from desired-intent ownership.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
#[non_exhaustive]
pub enum CurrentWorkOwner {
    /// Work's own running body advances it.
    Work,
    /// Loopflow supervision will recover or (re)start it.
    Loopflow,
    /// A User must decide, resume, or abandon.
    User,
    /// Nobody: terminal intent.
    Nobody,
    /// Unknown: the body cannot be observed from here.
    Unknown,
}

/// A control a surface may legally offer for a body in a given category.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
#[non_exhaustive]
pub enum CurrentWorkControl {
    Attach,
    Steer,
    Interrupt,
    Stop,
    Extend,
    Resume,
    Decide,
    Abandon,
}

/// The observed body state plus the evidence, owner, and legal controls behind
/// it. Every surface (CLI, Mac, iOS, chat, Now/Roadmap) reads this one shape.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CurrentWorkObservation {
    pub state: CurrentWorkState,
    /// One-line evidence for this category.
    pub reason: String,
    pub owner: CurrentWorkOwner,
    /// Controls a surface may offer, ordered most to least routine.
    pub controls: Vec<CurrentWorkControl>,
    /// Seconds since the last durable mutation, when a live body is observed.
    pub progress_age_secs: Option<u64>,
    /// Seconds until the progress deadline; negative once overdue. Present only
    /// when a live body is running against a deadline.
    pub deadline_in_secs: Option<i64>,
    /// The flow step or command the body is on, when known.
    pub step: Option<String>,
    /// Home-owned process evidence exists only while Work claims a Run.
    pub liveness: Option<crate::durable::RunLivenessEvidence>,
}

/// Durable Work intent, coarsened to what the current projection needs.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CurrentWorkIntent {
    Active,
    Ready,
    Waiting { user_owned: bool },
    Done,
    Abandoned,
}

/// The evidence a body observation is derived from, gathered by the caller so
/// [`observe`] stays a pure, clock-free function.
#[derive(Debug, Clone)]
pub struct CurrentWorkEvidence {
    pub intent: CurrentWorkIntent,
    pub liveness: Option<crate::durable::RunLivenessEvidence>,
    /// Age of the last durable mutation (last event, else the last status change).
    pub progress_age: Duration,
    /// The step or command the body is on, if known.
    pub step: Option<String>,
    /// The Work reason, carried through for working and terminal rows.
    pub reason: String,
}

/// Default: a live body with no durable mutation for this long is Stalled. The
/// precise, per-flow deadline arrives with the write lease (PR2/PR3); until then
/// this is the honest coarse bound the read model can prove.
pub const DEFAULT_STALL_AFTER: Duration = Duration::from_secs(30 * 60);

/// Age of the freshest durable progress evidence, clamped at zero so clock
/// skew never turns a future event into a negative duration.
pub(crate) fn body_progress_age(
    latest_event_at: Option<OffsetDateTime>,
    status_at: OffsetDateTime,
    now: OffsetDateTime,
) -> Duration {
    let progress_at = latest_event_at.map_or(status_at, |event_at| event_at.max(status_at));
    let seconds = (now - progress_at).whole_seconds().max(0);
    Duration::from_secs(seconds as u64)
}

pub(crate) fn work_status_reason(status: &WorkStatus) -> String {
    match status {
        WorkStatus::Running { run_id } => format!("Run {run_id} is active"),
        WorkStatus::Waiting { .. } => "waiting for input or an event".to_string(),
        WorkStatus::Ready => "ready".to_string(),
        WorkStatus::Done => "done".to_string(),
        WorkStatus::Abandoned => "abandoned".to_string(),
    }
}

pub(crate) async fn observe_current_work(
    store: &Store,
    work: &WorkRef,
    status: &WorkStatus,
    now: OffsetDateTime,
) -> StoreResult<CurrentWorkObservation> {
    let run = if matches!(status, WorkStatus::Running { .. }) {
        store.current_run(work).await?
    } else {
        None
    };
    let liveness = match run.as_ref() {
        Some(run) => Some(store.run_liveness_evidence(run).await?),
        None if matches!(status, WorkStatus::Running { .. }) => Some(RunLivenessEvidence {
            state: crate::durable::RunLivenessState::Unprovable,
            observed_at: None,
            fresh: false,
        }),
        None => None,
    };
    let (progress_age, step) = match work {
        WorkRef::Wave(_) => {
            let progress_age = run.as_ref().map_or(Duration::ZERO, |run| {
                let progress_at = liveness
                    .as_ref()
                    .and_then(|evidence| evidence.observed_at)
                    .or(run.started_at)
                    .unwrap_or(run.created_at);
                body_progress_age(None, progress_at, now)
            });
            (progress_age, None)
        }
        WorkRef::Project(project_id) => {
            let project = store.get_project(project_id).await?.ok_or_else(|| {
                StoreError::InvalidData(format!("Project {project_id} is not registered"))
            })?;
            let latest_event_at = store.latest_project_event_at(project_id).await?;
            (
                body_progress_age(latest_event_at, project.updated_at, now),
                Some(format!("iteration {}", project.iteration)),
            )
        }
        WorkRef::Task(task_id) => {
            let task = store.get_task(task_id).await?.ok_or_else(|| {
                StoreError::InvalidData(format!("Task {task_id} is not registered"))
            })?;
            let latest_event_at = store.latest_task_event_at(task_id).await?;
            (
                body_progress_age(latest_event_at, task.updated_at, now),
                Some(task.lifecycle_phase.as_str().to_string()),
            )
        }
    };
    let intent = match status {
        WorkStatus::Running { .. } => CurrentWorkIntent::Active,
        WorkStatus::Ready => CurrentWorkIntent::Ready,
        WorkStatus::Waiting { wait } => CurrentWorkIntent::Waiting {
            user_owned: matches!(wait.on, WaitOn::Input { .. }),
        },
        WorkStatus::Done => CurrentWorkIntent::Done,
        WorkStatus::Abandoned => CurrentWorkIntent::Abandoned,
    };
    Ok(observe(
        &CurrentWorkEvidence {
            intent,
            liveness,
            progress_age,
            step,
            reason: work_status_reason(status),
        },
        DEFAULT_STALL_AFTER,
    ))
}

/// Derive the visible body state from durable intent and liveness evidence.
pub fn observe(evidence: &CurrentWorkEvidence, stall_after: Duration) -> CurrentWorkObservation {
    let liveness = matches!(evidence.intent, CurrentWorkIntent::Active)
        .then(|| evidence.liveness.clone())
        .flatten();
    let make = |state, reason: &str, owner, controls, progress, deadline| CurrentWorkObservation {
        state,
        reason: reason.to_string(),
        owner,
        controls,
        progress_age_secs: progress,
        deadline_in_secs: deadline,
        step: evidence.step.clone(),
        liveness: liveness.clone(),
    };
    match evidence.intent {
        CurrentWorkIntent::Done => make(
            CurrentWorkState::Done,
            &evidence.reason,
            CurrentWorkOwner::Nobody,
            vec![],
            None,
            None,
        ),
        CurrentWorkIntent::Abandoned => make(
            CurrentWorkState::Abandoned,
            &evidence.reason,
            CurrentWorkOwner::Nobody,
            vec![],
            None,
            None,
        ),
        CurrentWorkIntent::Ready => make(
            CurrentWorkState::Ready,
            &evidence.reason,
            CurrentWorkOwner::Loopflow,
            vec![CurrentWorkControl::Resume, CurrentWorkControl::Abandon],
            None,
            None,
        ),
        CurrentWorkIntent::Waiting { user_owned } => make(
            CurrentWorkState::Waiting,
            &evidence.reason,
            if user_owned {
                CurrentWorkOwner::User
            } else {
                CurrentWorkOwner::Loopflow
            },
            if user_owned {
                vec![
                    CurrentWorkControl::Decide,
                    CurrentWorkControl::Resume,
                    CurrentWorkControl::Abandon,
                ]
            } else {
                vec![CurrentWorkControl::Resume, CurrentWorkControl::Abandon]
            },
            None,
            None,
        ),
        CurrentWorkIntent::Active => {
            let Some(liveness) = evidence.liveness.as_ref() else {
                return make(
                    CurrentWorkState::Unobservable,
                    "the owning Home has not recorded Run liveness",
                    CurrentWorkOwner::Unknown,
                    vec![],
                    None,
                    None,
                );
            };
            if !liveness.fresh || liveness.state == crate::durable::RunLivenessState::Unprovable {
                return make(
                    CurrentWorkState::Unobservable,
                    "the owning Home could not verify current Run liveness",
                    CurrentWorkOwner::Unknown,
                    vec![],
                    None,
                    None,
                );
            }
            if liveness.state == crate::durable::RunLivenessState::Absent {
                return make(
                    CurrentWorkState::Stopped,
                    "the owning Home proved the Run process is gone",
                    CurrentWorkOwner::Loopflow,
                    vec![CurrentWorkControl::Resume, CurrentWorkControl::Stop],
                    None,
                    None,
                );
            }
            let progress = Some(evidence.progress_age.as_secs());
            let remaining = stall_after.as_secs() as i64 - evidence.progress_age.as_secs() as i64;
            if evidence.progress_age > stall_after {
                make(
                    CurrentWorkState::Stalled,
                    "alive but no meaningful progress past the deadline",
                    CurrentWorkOwner::Loopflow,
                    vec![
                        CurrentWorkControl::Attach,
                        CurrentWorkControl::Extend,
                        CurrentWorkControl::Interrupt,
                        CurrentWorkControl::Stop,
                    ],
                    progress,
                    Some(remaining),
                )
            } else {
                make(
                    CurrentWorkState::Working,
                    &evidence.reason,
                    CurrentWorkOwner::Work,
                    vec![
                        CurrentWorkControl::Attach,
                        CurrentWorkControl::Steer,
                        CurrentWorkControl::Interrupt,
                        CurrentWorkControl::Stop,
                    ],
                    progress,
                    Some(remaining),
                )
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{
        body_progress_age, observe, CurrentWorkControl, CurrentWorkEvidence, CurrentWorkIntent,
        CurrentWorkOwner, CurrentWorkState, Duration, DEFAULT_STALL_AFTER,
    };
    use crate::durable::{RunLivenessEvidence, RunLivenessState};

    fn evidence(
        intent: CurrentWorkIntent,
        liveness: RunLivenessState,
        progress: Duration,
    ) -> CurrentWorkEvidence {
        CurrentWorkEvidence {
            intent,
            liveness: Some(RunLivenessEvidence {
                state: liveness,
                observed_at: Some(time::OffsetDateTime::UNIX_EPOCH),
                fresh: true,
            }),
            progress_age: progress,
            step: Some("task/pursue".to_string()),
            reason: "running".to_string(),
        }
    }

    #[test]
    fn a_live_body_that_just_progressed_is_working() {
        let obs = observe(
            &evidence(
                CurrentWorkIntent::Active,
                RunLivenessState::Present,
                Duration::from_secs(60),
            ),
            DEFAULT_STALL_AFTER,
        );
        assert_eq!(obs.state, CurrentWorkState::Working);
        assert_eq!(obs.owner, CurrentWorkOwner::Work);
        assert_eq!(obs.progress_age_secs, Some(60));
        assert!(obs.controls.contains(&CurrentWorkControl::Steer));
        // Deadline is still ahead while working.
        assert!(obs.deadline_in_secs.unwrap() > 0);
    }

    #[test]
    fn a_live_body_past_its_deadline_is_stalled() {
        let stalled = observe(
            &evidence(
                CurrentWorkIntent::Active,
                RunLivenessState::Present,
                Duration::from_secs(31 * 60),
            ),
            DEFAULT_STALL_AFTER,
        );
        assert_eq!(stalled.state, CurrentWorkState::Stalled);
        assert_eq!(stalled.owner, CurrentWorkOwner::Loopflow);
        assert_eq!(stalled.progress_age_secs, Some(31 * 60));
        // Overdue: the deadline is behind us.
        assert!(stalled.deadline_in_secs.unwrap() < 0);
        assert!(stalled.controls.contains(&CurrentWorkControl::Extend));
    }

    #[test]
    fn the_stall_boundary_is_the_threshold_exactly() {
        let clock = Duration::from_secs(10);
        // At the threshold: still Working. One tick past: Stalled.
        assert_eq!(
            observe(
                &evidence(CurrentWorkIntent::Active, RunLivenessState::Present, clock),
                clock,
            )
            .state,
            CurrentWorkState::Working,
        );
        assert_eq!(
            observe(
                &evidence(
                    CurrentWorkIntent::Active,
                    RunLivenessState::Present,
                    clock + Duration::from_secs(1),
                ),
                clock,
            )
            .state,
            CurrentWorkState::Stalled,
        );
    }

    #[test]
    fn progress_age_uses_the_freshest_durable_evidence() {
        let status_at = time::OffsetDateTime::UNIX_EPOCH + time::Duration::seconds(10);
        let event_at = status_at + time::Duration::seconds(5);
        let now = status_at + time::Duration::seconds(20);

        assert_eq!(
            body_progress_age(Some(event_at), status_at, now),
            Duration::from_secs(15)
        );
        assert_eq!(
            body_progress_age(Some(now + time::Duration::seconds(5)), status_at, now),
            Duration::ZERO,
        );
    }
    #[test]
    fn active_intent_with_no_live_body_is_stopped_not_gone() {
        let obs = observe(
            &evidence(
                CurrentWorkIntent::Active,
                RunLivenessState::Absent,
                Duration::from_secs(5),
            ),
            DEFAULT_STALL_AFTER,
        );
        assert_eq!(obs.state, CurrentWorkState::Stopped);
        assert_eq!(obs.owner, CurrentWorkOwner::Loopflow);
        assert_eq!(obs.progress_age_secs, None);
    }

    #[test]
    fn an_unobservable_body_is_never_asserted_gone() {
        let mut ev = evidence(
            CurrentWorkIntent::Active,
            RunLivenessState::Unprovable,
            Duration::from_secs(5),
        );
        ev.liveness.as_mut().unwrap().fresh = false;
        let obs = observe(&ev, DEFAULT_STALL_AFTER);
        assert_eq!(obs.state, CurrentWorkState::Unobservable);
        assert_eq!(obs.owner, CurrentWorkOwner::Unknown);
        assert!(obs.controls.is_empty());
    }

    #[test]
    fn a_stale_present_receipt_is_unobservable() {
        let mut ev = evidence(
            CurrentWorkIntent::Active,
            RunLivenessState::Present,
            Duration::from_secs(5),
        );
        ev.liveness.as_mut().unwrap().fresh = false;

        let obs = observe(&ev, DEFAULT_STALL_AFTER);

        assert_eq!(obs.state, CurrentWorkState::Unobservable);
        assert_eq!(obs.owner, CurrentWorkOwner::Unknown);
        assert!(obs.controls.is_empty());
    }

    #[test]
    fn waiting_intent_needs_user_input() {
        let obs = observe(
            &evidence(
                CurrentWorkIntent::Waiting { user_owned: true },
                RunLivenessState::Unprovable,
                Duration::from_secs(5),
            ),
            DEFAULT_STALL_AFTER,
        );
        assert_eq!(obs.state, CurrentWorkState::Waiting);
        assert_eq!(obs.owner, CurrentWorkOwner::User);
        assert!(obs.controls.contains(&CurrentWorkControl::Decide));
    }

    #[test]
    fn terminal_intent_owns_nobody_and_offers_no_controls() {
        let obs = observe(
            &evidence(
                CurrentWorkIntent::Done,
                RunLivenessState::Unprovable,
                Duration::from_secs(0),
            ),
            DEFAULT_STALL_AFTER,
        );
        assert_eq!(obs.state, CurrentWorkState::Done);
        assert_eq!(obs.owner, CurrentWorkOwner::Nobody);
        assert!(obs.controls.is_empty());
        assert!(obs.liveness.is_none());
    }
}
