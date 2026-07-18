//! Compatibility types for Project and Task process containment.

use std::path::PathBuf;
use std::time::Duration;

use serde::{Deserialize, Serialize};
use time::OffsetDateTime;

use crate::id::WaveId;
use crate::project_session::ProjectSessionId;
use crate::task::TaskSessionId;

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
pub enum ChildSessionDataError {
    #[error("invalid child-session id: {0}")]
    InvalidId(String),
    #[error("invalid child lease state: {0}")]
    InvalidLeaseState(String),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
#[non_exhaustive]
pub enum ChildLeaseState {
    Legacy,
    Reserved,
    Active,
    Revoked,
    Finished,
}

impl ChildLeaseState {
    pub(crate) fn as_str(self) -> &'static str {
        match self {
            Self::Legacy => "legacy",
            Self::Reserved => "reserved",
            Self::Active => "active",
            Self::Revoked => "revoked",
            Self::Finished => "finished",
        }
    }

    pub(crate) fn parse(value: &str) -> Result<Self, ChildSessionDataError> {
        match value {
            "legacy" => Ok(Self::Legacy),
            "reserved" => Ok(Self::Reserved),
            "active" => Ok(Self::Active),
            "revoked" => Ok(Self::Revoked),
            "finished" => Ok(Self::Finished),
            value => Err(ChildSessionDataError::InvalidLeaseState(value.to_string())),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
#[non_exhaustive]
pub enum ChildBodyOutcome {
    Completed,
    Interrupted { reason: String },
    Failed { reason: String },
    Lost { reason: String },
    Superseded { reason: String },
    LegacyStopped { reason: String },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum ObservationRecipient {
    Wave { wave_id: WaveId },
    Project { session_id: ProjectSessionId },
}

/// Immutable audit record for the lf binary that launched a process generation.
/// Provenance says what ran; it never selects what runs next.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BinaryProvenance {
    pub version: String,
    pub provenance: String,
    pub source_identity: String,
}

impl BinaryProvenance {
    pub fn current() -> Self {
        Self {
            version: env!("CARGO_PKG_VERSION").to_string(),
            provenance: crate::build_info::provenance().to_string(),
            source_identity: crate::build_info::source_identity(),
        }
    }

    #[cfg(test)]
    pub fn for_tests() -> Self {
        Self {
            version: "0.0.0-test".to_string(),
            provenance: "development".to_string(),
            source_identity: "test".to_string(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
/// Durable receipt for one child-process write lease. `generation` is the
/// monotonically increasing fencing token: only the current generation may
/// act for the Session. The latest receipt stays after the process exits so a
/// replacement body advances rather than assuming the old body's identity.
pub struct ChildProcessGeneration {
    pub generation: u32,
    pub pid: Option<u32>,
    /// Group that must be reaped in addition to the outer tmux/PID identity:
    /// the provider's isolated group when it has one, otherwise the runner's.
    pub process_group_id: Option<u32>,
    pub tmux_name: String,
    pub agent: String,
    pub provider: String,
    pub provider_session_id: Option<String>,
    pub started_at: OffsetDateTime,
    pub state: ChildLeaseState,
    pub outcome: Option<ChildBodyOutcome>,
    /// Immutable binary provenance: which lf actually booted this generation,
    /// stamped by that process itself at boot. `None` until the generation has
    /// booted (a reserved-but-never-started generation ran nothing), and for
    /// generations recorded before this field was added.
    pub provenance: Option<BinaryProvenance>,
}

/// Requested replacement of the agent/provider body acting for a durable
/// Project or Task Session.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ChildBodyHandoffRequest {
    pub agent: String,
    pub provider: String,
    pub reason: String,
}

/// Typed audit record for a body handoff. Session identity and durable work are
/// intentionally absent: they do not change during this transition.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ChildBodyHandoff {
    pub from_agent: String,
    pub to_agent: String,
    pub from_provider: String,
    pub to_provider: String,
    pub reason: String,
}

/// The `lf` binary, store, and home a Session launches through, resolved fresh
/// at the launch boundary from the current Home — never persisted as Session
/// state. A transient bundle carried from the resolver to the tmux spawn; a
/// Session no longer pins a binary of its own.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ChildExecutionContext {
    pub lf_bin: PathBuf,
    pub db_path: PathBuf,
    pub lf_home: PathBuf,
}

/// A recorded request to end a Session, durable from the moment the Abandon
/// command is queued rather than from the moment a runner consumes it.
///
/// The gap between those two moments is where a supervisor used to restart a
/// Session someone had already ended: the pending command carried the intent,
/// but the Session row still read `Running`, and every launch path reads the
/// Session.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AbandonIntent {
    pub requested_at: OffsetDateTime,
    pub reason: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", content = "id", rename_all = "snake_case")]
pub enum ChildRef {
    Project(ProjectSessionId),
    Task(TaskSessionId),
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

// ── Body observation ────────────────────────────────────────────────────────
//
// A Session is durable intent; a body is the disposable process that acts for
// it. `status` records intent; `BodyObservation` records what the *current body*
// is observed doing. They are different evidence and must not collapse into one
// string: a Session can be `Running` (intent) while its body is Stalled, Stopped,
// or Unobservable. The observation is *derived*, never stored — a projection over
// the durable status, whether the body is alive, and how long since it last made
// meaningful progress. No second monitor store (see the W2-135 design).

/// The observed state of a Session's current body, distinct from durable intent.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
#[non_exhaustive]
pub enum BodyCategory {
    /// A body holds the Session and made meaningful progress recently.
    Working,
    /// A body is alive but has made no meaningful progress past its deadline.
    Stalled,
    /// Loopflow revoked a lost/stalled body and is starting its successor.
    Recovering,
    /// A human decision or resume is required; Loopflow will not proceed blindly.
    NeedsInput,
    /// No live body, intent not terminal; a wake will adopt or start one.
    Stopped,
    /// The last body failed and the flow itself failed.
    Failed,
    /// Completed or Abandoned. Never restarts.
    Terminal,
    /// This machine cannot tell whether a body is alive. Never asserted as gone.
    Unobservable,
}

/// Who must act next on the body. Separate from desired-intent ownership.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
#[non_exhaustive]
pub enum BodyOwner {
    /// The Session's own running body advances it.
    Session,
    /// Loopflow supervision will recover or (re)start it.
    Loopflow,
    /// A human must decide, resume, or abandon.
    Human,
    /// Nobody: terminal intent.
    Nobody,
    /// Unknown: the body cannot be observed from here.
    Unknown,
}

/// A control a surface may legally offer for a body in a given category.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
#[non_exhaustive]
pub enum BodyControl {
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
pub struct BodyObservation {
    pub category: BodyCategory,
    /// Human one-liner naming the evidence for this category.
    pub reason: String,
    pub owner: BodyOwner,
    /// Controls a surface may offer, ordered most to least routine.
    pub controls: Vec<BodyControl>,
    /// Seconds since the last durable mutation, when a live body is observed.
    pub progress_age_secs: Option<u64>,
    /// Seconds until the progress deadline; negative once overdue. Present only
    /// when a live body is running against a deadline.
    pub deadline_in_secs: Option<i64>,
    /// The flow step or command the body is on, when known.
    pub step: Option<String>,
}

/// Durable intent, coarsened to what the body projection needs. Both
/// `TaskSessionStatus` and `ProjectSessionStatus` map onto this shared shape so
/// one projection serves Wave, Project, and Task Sessions.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BodyIntent {
    /// Created / Starting / Running — a body should be advancing.
    Active,
    /// Waiting on a decision or input.
    Waiting,
    /// Blocked on a dependency.
    Blocked,
    /// The flow itself failed.
    Failed,
    /// Completed or Abandoned.
    Terminal,
}

/// The evidence a body observation is derived from, gathered by the caller so
/// [`observe`] stays a pure, clock-free function.
#[derive(Debug, Clone)]
pub struct BodyEvidence {
    pub intent: BodyIntent,
    /// Whether this machine can observe body liveness at all (tmux present).
    pub observable: bool,
    /// Whether a body the Session claims is running was found alive.
    pub process_alive: bool,
    /// Age of the last durable mutation (last event, else the last status change).
    pub progress_age: Duration,
    /// The step or command the body is on, if known.
    pub step: Option<String>,
    /// The Session's own reason string, carried through for Working/terminal rows.
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

/// Derive the visible body state from durable intent and liveness evidence.
pub fn observe(evidence: &BodyEvidence, stall_after: Duration) -> BodyObservation {
    let make = |category, reason: &str, owner, controls, progress, deadline| BodyObservation {
        category,
        reason: reason.to_string(),
        owner,
        controls,
        progress_age_secs: progress,
        deadline_in_secs: deadline,
        step: evidence.step.clone(),
    };
    match evidence.intent {
        BodyIntent::Terminal => make(
            BodyCategory::Terminal,
            &evidence.reason,
            BodyOwner::Nobody,
            vec![],
            None,
            None,
        ),
        BodyIntent::Failed => make(
            BodyCategory::Failed,
            &evidence.reason,
            BodyOwner::Human,
            vec![BodyControl::Resume, BodyControl::Abandon],
            None,
            None,
        ),
        BodyIntent::Waiting => make(
            BodyCategory::NeedsInput,
            &evidence.reason,
            BodyOwner::Human,
            vec![
                BodyControl::Decide,
                BodyControl::Resume,
                BodyControl::Abandon,
            ],
            None,
            None,
        ),
        BodyIntent::Blocked => make(
            BodyCategory::Stopped,
            &evidence.reason,
            BodyOwner::Loopflow,
            vec![BodyControl::Resume, BodyControl::Stop, BodyControl::Abandon],
            None,
            None,
        ),
        BodyIntent::Active => {
            if !evidence.observable {
                return make(
                    BodyCategory::Unobservable,
                    "this machine cannot observe the body",
                    BodyOwner::Unknown,
                    vec![],
                    None,
                    None,
                );
            }
            if !evidence.process_alive {
                return make(
                    BodyCategory::Stopped,
                    "no live body for active intent; a wake will adopt or start one",
                    BodyOwner::Loopflow,
                    vec![BodyControl::Resume, BodyControl::Stop],
                    None,
                    None,
                );
            }
            let progress = Some(evidence.progress_age.as_secs());
            let remaining = stall_after.as_secs() as i64 - evidence.progress_age.as_secs() as i64;
            if evidence.progress_age > stall_after {
                make(
                    BodyCategory::Stalled,
                    "alive but no meaningful progress past the deadline",
                    BodyOwner::Loopflow,
                    vec![
                        BodyControl::Attach,
                        BodyControl::Extend,
                        BodyControl::Interrupt,
                        BodyControl::Stop,
                    ],
                    progress,
                    Some(remaining),
                )
            } else {
                make(
                    BodyCategory::Working,
                    &evidence.reason,
                    BodyOwner::Session,
                    vec![
                        BodyControl::Attach,
                        BodyControl::Steer,
                        BodyControl::Interrupt,
                        BodyControl::Stop,
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
        body_progress_age, observe, BodyCategory, BodyControl, BodyEvidence, BodyIntent, BodyOwner,
        Duration, DEFAULT_STALL_AFTER,
    };

    fn evidence(intent: BodyIntent, alive: bool, progress: Duration) -> BodyEvidence {
        BodyEvidence {
            intent,
            observable: true,
            process_alive: alive,
            progress_age: progress,
            step: Some("task_pursue".to_string()),
            reason: "running".to_string(),
        }
    }

    #[test]
    fn a_live_body_that_just_progressed_is_working() {
        let obs = observe(
            &evidence(BodyIntent::Active, true, Duration::from_secs(60)),
            DEFAULT_STALL_AFTER,
        );
        assert_eq!(obs.category, BodyCategory::Working);
        assert_eq!(obs.owner, BodyOwner::Session);
        assert_eq!(obs.progress_age_secs, Some(60));
        assert!(obs.controls.contains(&BodyControl::Steer));
        // Deadline is still ahead while working.
        assert!(obs.deadline_in_secs.unwrap() > 0);
    }

    #[test]
    fn a_live_body_past_its_deadline_is_stalled() {
        let stalled = observe(
            &evidence(BodyIntent::Active, true, Duration::from_secs(31 * 60)),
            DEFAULT_STALL_AFTER,
        );
        assert_eq!(stalled.category, BodyCategory::Stalled);
        assert_eq!(stalled.owner, BodyOwner::Loopflow);
        assert_eq!(stalled.progress_age_secs, Some(31 * 60));
        // Overdue: the deadline is behind us.
        assert!(stalled.deadline_in_secs.unwrap() < 0);
        assert!(stalled.controls.contains(&BodyControl::Extend));
    }

    #[test]
    fn the_stall_boundary_is_the_threshold_exactly() {
        let clock = Duration::from_secs(10);
        // At the threshold: still Working. One tick past: Stalled.
        assert_eq!(
            observe(&evidence(BodyIntent::Active, true, clock), clock).category,
            BodyCategory::Working,
        );
        assert_eq!(
            observe(
                &evidence(BodyIntent::Active, true, clock + Duration::from_secs(1)),
                clock,
            )
            .category,
            BodyCategory::Stalled,
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
            &evidence(BodyIntent::Active, false, Duration::from_secs(5)),
            DEFAULT_STALL_AFTER,
        );
        assert_eq!(obs.category, BodyCategory::Stopped);
        assert_eq!(obs.owner, BodyOwner::Loopflow);
        assert_eq!(obs.progress_age_secs, None);
    }

    #[test]
    fn an_unobservable_body_is_never_asserted_gone() {
        let mut ev = evidence(BodyIntent::Active, false, Duration::from_secs(5));
        ev.observable = false;
        let obs = observe(&ev, DEFAULT_STALL_AFTER);
        assert_eq!(obs.category, BodyCategory::Unobservable);
        assert_eq!(obs.owner, BodyOwner::Unknown);
        assert!(obs.controls.is_empty());
    }

    #[test]
    fn waiting_intent_needs_human_input() {
        let obs = observe(
            &evidence(BodyIntent::Waiting, false, Duration::from_secs(5)),
            DEFAULT_STALL_AFTER,
        );
        assert_eq!(obs.category, BodyCategory::NeedsInput);
        assert_eq!(obs.owner, BodyOwner::Human);
        assert!(obs.controls.contains(&BodyControl::Decide));
    }

    #[test]
    fn terminal_intent_owns_nobody_and_offers_no_controls() {
        let obs = observe(
            &evidence(BodyIntent::Terminal, false, Duration::from_secs(0)),
            DEFAULT_STALL_AFTER,
        );
        assert_eq!(obs.category, BodyCategory::Terminal);
        assert_eq!(obs.owner, BodyOwner::Nobody);
        assert!(obs.controls.is_empty());
    }

    #[test]
    fn a_failed_flow_asks_a_human_to_resume_or_abandon() {
        let obs = observe(
            &evidence(BodyIntent::Failed, false, Duration::from_secs(0)),
            DEFAULT_STALL_AFTER,
        );
        assert_eq!(obs.category, BodyCategory::Failed);
        assert_eq!(obs.owner, BodyOwner::Human);
        assert_eq!(
            obs.controls,
            vec![BodyControl::Resume, BodyControl::Abandon]
        );
    }
}
