//! Compatibility types for Project and Task process containment.

use std::path::PathBuf;
use std::time::Duration;

use serde::{Deserialize, Serialize};
use time::OffsetDateTime;

use crate::id::WaveId;
use crate::project_session::ProjectSessionId;
use crate::task::{TaskEvent, TaskEventKind, TaskSessionId};

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

impl ChildProcessGeneration {
    /// Stamp the running process's identity as it boots this generation: its
    /// pid, process group, and the provenance of the binary that is actually
    /// executing it. Called from inside the child body, so `current()` reflects
    /// the booting binary — provenance describes what ran, not what launched it.
    pub(crate) fn mark_booted(&mut self) {
        self.pid = Some(std::process::id());
        self.process_group_id = crate::engine::process::current_process_group_id();
        self.state = ChildLeaseState::Active;
        self.provenance = Some(BinaryProvenance::current());
    }

    pub(crate) fn observe_provider(
        &mut self,
        provider: &str,
        provider_session_id: Option<String>,
        process_group_id: Option<u32>,
    ) {
        self.provider = provider.to_string();
        self.provider_session_id = provider_session_id;
        if let Some(process_group_id) = process_group_id {
            self.process_group_id = Some(process_group_id);
        }
    }
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

/// What supervision may do after observing a stalled body.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum BodyRecoveryPlan {
    /// The observation is not a stall, so recovery has no work.
    LeaveAlone,
    /// Revoke, reap, and start another Run from durable Work input.
    Restart,
}

pub(crate) fn plan_body_recovery(observation: &BodyObservation) -> BodyRecoveryPlan {
    if observation.category != BodyCategory::Stalled {
        return BodyRecoveryPlan::LeaveAlone;
    }
    BodyRecoveryPlan::Restart
}

/// Consecutive automatic recovery attempts a strand gets before Loopflow stops
/// and says why.
///
/// Deliberately not the process generation, which also increments on legitimate
/// PR rotation — a healthy serial Task reaches generation 17 without ever having
/// been recovered, and bounding on it would strand exactly the Sessions that are
/// working.
pub(crate) const MAX_RECOVERY_ATTEMPTS: u32 = 3;

/// What supervision may do about a Session whose durable status claims a body
/// that does not exist.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum StrandedPlan {
    /// The body is alive, unobservable, deliberately parked, or terminal.
    LeaveAlone,
    /// A strand a resume fixes. Redispatch as this attempt.
    Redispatch { attempt: u32 },
    /// A strand a resume cannot fix, or recovery is spent. Say so once.
    Surface { reason: String },
}

/// Whether the body behind a bodyless Session left a strand a resume can fix.
enum StrandVerdict {
    Recoverable,
    Terminal(String),
    NotStranded,
}

/// Decide whether a Session with no live body is stranded, and whether a resume
/// can fix it.
///
/// Keyed on durable *status* (through [`BodyIntent`]), never on the `Lost` label
/// alone. A Task that COMPLETED successfully also reaps its body as `Lost`, so
/// anything keyed on the tag by itself chases finished work forever. Intent is
/// consulted first and rejects terminal work before the tag is read at all.
///
/// `Waiting`/`Blocked` are parked on purpose, not stranded. That is what
/// preserves the W2-129 open-PR bar here: a Task that delivered a PR parks at
/// `Waiting`, so recovery never restarts delivered work and never needs to
/// consult the PR phase — which is a lagging, mutable signal that can change
/// long after a strand forms.
///
/// The caller must skip a reservation still inside its startup grace
/// ([`crate::ops::child::child_body_reservation_is_fresh`]); a relaunch already
/// in flight is not a strand, and re-entering it here would double-dispatch.
pub(crate) fn plan_stranded_recovery(
    intent: BodyIntent,
    observable: bool,
    process_alive: bool,
    process: Option<&ChildProcessGeneration>,
    attempts: u32,
) -> StrandedPlan {
    // An unobservable machine never asserts a body is gone, and a live body
    // belongs to the stall path in `plan_body_recovery`.
    if !observable || process_alive {
        return StrandedPlan::LeaveAlone;
    }
    match intent {
        BodyIntent::Terminal | BodyIntent::Waiting | BodyIntent::Blocked => {
            return StrandedPlan::LeaveAlone;
        }
        BodyIntent::Active | BodyIntent::Failed => {}
    }
    let Some(process) = process else {
        return StrandedPlan::LeaveAlone;
    };
    match strand_verdict(process) {
        StrandVerdict::NotStranded => StrandedPlan::LeaveAlone,
        StrandVerdict::Terminal(reason) => StrandedPlan::Surface { reason },
        StrandVerdict::Recoverable if attempts >= MAX_RECOVERY_ATTEMPTS => StrandedPlan::Surface {
            reason: format!(
                "body generation {} did not survive {MAX_RECOVERY_ATTEMPTS} automatic recovery attempts; \
                 resume it explicitly or hand it to another agent with `--model`",
                process.generation
            ),
        },
        StrandVerdict::Recoverable => StrandedPlan::Redispatch {
            attempt: attempts + 1,
        },
    }
}

/// Read the strand from the lease and the outcome tag — structurally, never by
/// parsing `status_reason`, which is free-form prose written from many sites and
/// would rot the first time a provider reworded an error.
///
/// Every outcome gets an arm. A fall-through here silently strands whole classes
/// of body, which is the defect this whole path exists to remove.
fn strand_verdict(process: &ChildProcessGeneration) -> StrandVerdict {
    match process.state {
        // A dead body still holding its lease recorded no outcome at all. The
        // reaper writes `Lost` for exactly this, so it is recoverable by
        // construction — no need to wait a tick for the tag to appear.
        ChildLeaseState::Legacy | ChildLeaseState::Reserved | ChildLeaseState::Active => {
            StrandVerdict::Recoverable
        }
        // A revoked lease is a reap that began and never finished — in practice,
        // a kill that failed (EPERM) and returned before
        // `finish_revoked_task_process` could run. `reserve_task_process` CASes
        // on `IS NULL OR = 'finished'`, so a revoked lease can never reserve
        // another generation: redispatching would burn the whole attempt budget
        // on a CAS that cannot pass, and surface a generic exhaustion instead of
        // the real cause. Say the real cause the first time.
        //
        // The release runs before this plan, so a lease still at `revoked` here
        // is one whose body could not be proven gone — present, or unprovable.
        // A resume cannot help that; only the body actually leaving can.
        ChildLeaseState::Revoked => StrandVerdict::Terminal(format!(
            "body generation {} is pinned by a lease stuck at `revoked`: its reap never completed \
             and its body could not be proven gone, so no new generation can be reserved. \
             The lease releases itself once the body is verifiably absent",
            process.generation
        )),
        ChildLeaseState::Finished => match &process.outcome {
            // Loopflow's own reap verdict: the body vanished without recording
            // why it stopped. Nobody chose this, so a resume is the honest
            // answer — and it is what revived 13/13 hand-swept Sessions.
            Some(ChildBodyOutcome::Lost { .. }) => StrandVerdict::Recoverable,
            // Supervision revoked this body *intending* to start its successor
            // ("recovering the same Task Session"), and the successor never came
            // — a successful one would have replaced this generation with gen+1,
            // reserved and outcome-free. So a `Superseded` tag still sitting on
            // the latest generation proves the intended recovery aborted. The
            // system already decided it wanted a new body; give it one.
            Some(ChildBodyOutcome::Superseded { .. }) => StrandVerdict::Recoverable,
            // The body recorded why it stopped. Something already decided this
            // is terminal, so a blind resume just re-fails.
            Some(ChildBodyOutcome::Failed { reason }) => StrandVerdict::Terminal(reason.clone()),
            // The body ran to a clean end. Its Session is delivered or parked,
            // and its status — not this tag — decides what happens next.
            Some(ChildBodyOutcome::Completed) => StrandVerdict::NotStranded,
            // A human stopped this body on purpose. Restarting it would overrule
            // an explicit command.
            Some(ChildBodyOutcome::Interrupted { .. }) => StrandVerdict::NotStranded,
            // A relic from before explicit leases. Nothing here is trustworthy
            // enough to restart a body on.
            Some(ChildBodyOutcome::LegacyStopped { .. }) => StrandVerdict::NotStranded,
            // A finished lease always carries an outcome; a missing one is a
            // write that lost a race, and the next pass will read it.
            None => StrandVerdict::NotStranded,
        },
    }
}

/// Consecutive recovery attempts behind `events` (newest first).
///
/// Progress-relative rather than absolute: a recovery of its own appends only
/// lease, status, and failure churn, so anything else in the log means a body
/// booted and did real work, which resets the count. A Session that recovers,
/// works, and later strands again therefore gets a full budget rather than
/// inheriting an old one.
///
/// `Failed` is churn here, and that is load-bearing rather than lenient. A
/// recovery whose launch fails writes one itself (`launch_task_process` reaps
/// the body and calls `record_task_failure` on its way out), so counting it as
/// progress would reset the budget on every failed launch and retry an
/// unlaunchable Session forever — the exact dead-generation loop the bound
/// exists to prevent. Treating it as churn cannot hide real work: a body that
/// booted and did something logs a non-churn event *before* it fails, and this
/// walk stops there.
pub(crate) fn count_recovery_attempts(events: &[TaskEvent]) -> u32 {
    let mut attempts = 0;
    for event in events {
        match event.kind {
            TaskEventKind::BodyRecoveryAttempted { .. } => attempts += 1,
            // The churn a recovery itself writes; keep walking past it.
            TaskEventKind::BodyLeaseChanged { .. }
            | TaskEventKind::StatusChanged { .. }
            | TaskEventKind::Failed { .. } => {}
            // Any other durable event is real progress.
            _ => break,
        }
    }
    attempts
}

/// Derive the observed body state from durable intent and body evidence.
///
/// Liveness (a body exists) and progress (it advanced) are separate inputs;
/// `Working` versus `Stalled` is exactly their difference. Terminal intent
/// dominates every other signal. An unobservable body is never asserted gone.
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
        body_progress_age, count_recovery_attempts, observe, plan_body_recovery,
        plan_stranded_recovery, BodyCategory, BodyControl, BodyEvidence, BodyIntent, BodyOwner,
        BodyRecoveryPlan, ChildBodyOutcome, ChildLeaseState, ChildProcessGeneration, Duration,
        StrandedPlan, TaskEvent, TaskEventKind, TaskSessionId, DEFAULT_STALL_AFTER,
        MAX_RECOVERY_ATTEMPTS,
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
    fn stalled_recovery_reconstructs_from_durable_work() {
        let stalled = observe(
            &evidence(BodyIntent::Active, true, Duration::from_secs(31 * 60)),
            DEFAULT_STALL_AFTER,
        );
        let working = observe(
            &evidence(BodyIntent::Active, true, Duration::from_secs(1)),
            DEFAULT_STALL_AFTER,
        );

        assert_eq!(plan_body_recovery(&stalled), BodyRecoveryPlan::Restart);
        assert_eq!(plan_body_recovery(&working), BodyRecoveryPlan::LeaveAlone);
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

    fn body(state: ChildLeaseState, outcome: Option<ChildBodyOutcome>) -> ChildProcessGeneration {
        ChildProcessGeneration {
            generation: 9,
            pid: None,
            process_group_id: None,
            tmux_name: "lf-task-W2-135-7db82f3f".to_string(),
            agent: "claude".to_string(),
            provider: "claude".to_string(),
            provider_session_id: None,
            started_at: time::OffsetDateTime::UNIX_EPOCH,
            state,
            outcome,
            provenance: None,
        }
    }

    fn lost() -> Option<ChildBodyOutcome> {
        Some(ChildBodyOutcome::Lost {
            reason: "task process disappeared before recording a terminal outcome".to_string(),
        })
    }

    /// A dead body under a Session that still believes it is working is the
    /// strand. Nobody has to ask.
    #[test]
    fn a_dead_body_under_a_running_task_is_redispatched() {
        let process = body(ChildLeaseState::Active, None);
        assert_eq!(
            plan_stranded_recovery(BodyIntent::Active, true, false, Some(&process), 0),
            StrandedPlan::Redispatch { attempt: 1 }
        );
    }

    /// W2-135: frozen at generation 9 with a reaped `lost` body, and the metric
    /// this feature is measured by counts exactly this row. A predicate keyed on
    /// `is_process_active()` alone would ignore it forever.
    #[test]
    fn a_frozen_failed_session_with_a_lost_body_is_redispatched() {
        let process = body(ChildLeaseState::Finished, lost());
        assert_eq!(
            plan_stranded_recovery(BodyIntent::Failed, true, false, Some(&process), 0),
            StrandedPlan::Redispatch { attempt: 1 }
        );
    }

    /// The triage's central trap: a Task that COMPLETED successfully also reaps
    /// its body as `lost`. Anything keyed on the tag alone chases finished work
    /// forever, so intent is consulted first.
    #[test]
    fn a_completed_task_with_a_lost_body_is_never_touched() {
        let process = body(ChildLeaseState::Finished, lost());
        assert_eq!(
            plan_stranded_recovery(BodyIntent::Terminal, true, false, Some(&process), 0),
            StrandedPlan::LeaveAlone
        );
    }

    /// W2-212: the body recorded why it stopped. A blind resume just re-fails,
    /// so recovery says the real reason instead of retrying.
    #[test]
    fn a_body_that_recorded_its_own_failure_is_surfaced_not_retried() {
        let process = body(
            ChildLeaseState::Finished,
            Some(ChildBodyOutcome::Failed {
                reason: "codex_error: You've hit your usage limit".to_string(),
            }),
        );
        assert_eq!(
            plan_stranded_recovery(BodyIntent::Failed, true, false, Some(&process), 0),
            StrandedPlan::Surface {
                reason: "codex_error: You've hit your usage limit".to_string()
            }
        );
    }

    /// W2-230: supervision revoked this body meaning to start its successor, and
    /// the successor never came. A `superseded` tag still on the latest
    /// generation proves the intended recovery aborted.
    #[test]
    fn an_aborted_recovery_leaves_a_superseded_body_that_is_redispatched() {
        let process = body(
            ChildLeaseState::Finished,
            Some(ChildBodyOutcome::Superseded {
                reason: "body generation 2 stalled after 1804s without durable progress; \
                         recovering the same Task Session"
                    .to_string(),
            }),
        );
        assert_eq!(
            plan_stranded_recovery(BodyIntent::Active, true, false, Some(&process), 0),
            StrandedPlan::Redispatch { attempt: 1 }
        );
    }

    /// A reap that failed leaves the lease at `revoked`, which
    /// `reserve_task_process` can never CAS past. Redispatching would burn the
    /// whole budget on an impossible reservation, so say the real cause once.
    #[test]
    fn a_lease_stuck_at_revoked_surfaces_the_real_cause_instead_of_redispatching() {
        let process = body(ChildLeaseState::Revoked, lost());
        let plan = plan_stranded_recovery(BodyIntent::Active, true, false, Some(&process), 0);
        let StrandedPlan::Surface { reason } = plan else {
            panic!("a revoked lease cannot reserve, so it must surface: {plan:?}");
        };
        assert!(reason.contains("stuck at `revoked`"), "{reason}");
    }

    /// A human stopped this body on purpose; restarting it would overrule an
    /// explicit command.
    #[test]
    fn a_deliberately_stopped_body_is_never_redispatched() {
        for outcome in [
            ChildBodyOutcome::Completed,
            ChildBodyOutcome::Interrupted {
                reason: "operator interrupted".to_string(),
            },
            ChildBodyOutcome::LegacyStopped {
                reason: "body predates explicit leases".to_string(),
            },
        ] {
            let process = body(ChildLeaseState::Finished, Some(outcome.clone()));
            assert_eq!(
                plan_stranded_recovery(BodyIntent::Active, true, false, Some(&process), 0),
                StrandedPlan::LeaveAlone,
                "{outcome:?} must not restart a body"
            );
        }
    }

    /// Delivered work parks at `Waiting`. This is what preserves the W2-129
    /// open-PR bar without ever consulting the PR phase.
    #[test]
    fn a_parked_session_is_never_redispatched() {
        let process = body(ChildLeaseState::Finished, lost());
        for intent in [BodyIntent::Waiting, BodyIntent::Blocked] {
            assert_eq!(
                plan_stranded_recovery(intent, true, false, Some(&process), 0),
                StrandedPlan::LeaveAlone,
                "{intent:?} is parked on purpose, not stranded"
            );
        }
    }

    /// A live body belongs to the stall path, and a machine that cannot see
    /// bodies never asserts one is gone.
    #[test]
    fn a_live_or_unobservable_body_is_left_to_someone_else() {
        let process = body(ChildLeaseState::Active, None);
        assert_eq!(
            plan_stranded_recovery(BodyIntent::Active, true, true, Some(&process), 0),
            StrandedPlan::LeaveAlone
        );
        assert_eq!(
            plan_stranded_recovery(BodyIntent::Active, false, false, Some(&process), 0),
            StrandedPlan::LeaveAlone
        );
    }

    /// Recovery stops and says why rather than minting dead generations.
    #[test]
    fn recovery_is_bounded_and_then_surfaces() {
        let process = body(ChildLeaseState::Finished, lost());
        assert_eq!(
            plan_stranded_recovery(
                BodyIntent::Active,
                true,
                false,
                Some(&process),
                MAX_RECOVERY_ATTEMPTS - 1
            ),
            StrandedPlan::Redispatch {
                attempt: MAX_RECOVERY_ATTEMPTS
            }
        );
        let plan = plan_stranded_recovery(
            BodyIntent::Active,
            true,
            false,
            Some(&process),
            MAX_RECOVERY_ATTEMPTS,
        );
        let StrandedPlan::Surface { reason } = plan else {
            panic!("a spent budget must surface, not redispatch: {plan:?}");
        };
        assert!(reason.contains("did not survive"), "{reason}");
    }

    fn recovery_event(id: i64, attempt: u32) -> TaskEvent {
        TaskEvent {
            id,
            session_id: TaskSessionId::new(),
            kind: TaskEventKind::BodyRecoveryAttempted {
                generation: 2,
                attempt,
                reason: "died".to_string(),
            },
            created_at: time::OffsetDateTime::UNIX_EPOCH,
        }
    }

    fn other_event(id: i64, kind: TaskEventKind) -> TaskEvent {
        TaskEvent {
            id,
            session_id: TaskSessionId::new(),
            kind,
            created_at: time::OffsetDateTime::UNIX_EPOCH,
        }
    }

    /// The churn a recovery itself writes must not hide an earlier attempt.
    #[test]
    fn recovery_attempts_count_past_their_own_lease_and_status_churn() {
        let events = vec![
            other_event(
                5,
                TaskEventKind::StatusChanged {
                    from: crate::task::TaskSessionStatus::Failed,
                    to: crate::task::TaskSessionStatus::Starting,
                    reason: "recovering".to_string(),
                },
            ),
            recovery_event(4, 2),
            other_event(
                3,
                TaskEventKind::BodyLeaseChanged {
                    process: body(ChildLeaseState::Finished, lost()),
                },
            ),
            recovery_event(2, 1),
        ];
        assert_eq!(count_recovery_attempts(&events), 2);
    }

    /// A failed launch must not reset the budget.
    ///
    /// `launch_task_process` reaps the body and calls `record_task_failure` on
    /// its way out, which writes `Failed`. Counting that as progress would give
    /// an unlaunchable Session a fresh budget on every attempt and retry it
    /// forever — caught in the live dispatcher by
    /// `an_unlaunchable_strand_exhausts_instead_of_minting_dead_generations`.
    #[test]
    fn a_failed_launch_does_not_reset_the_recovery_budget() {
        let events = vec![
            other_event(
                6,
                TaskEventKind::Failed {
                    error: "cannot resolve current lf binary".to_string(),
                    resumable: true,
                },
            ),
            other_event(
                5,
                TaskEventKind::StatusChanged {
                    from: crate::task::TaskSessionStatus::Starting,
                    to: crate::task::TaskSessionStatus::Failed,
                    reason: "launch failed".to_string(),
                },
            ),
            recovery_event(4, 2),
            recovery_event(3, 1),
        ];
        assert_eq!(count_recovery_attempts(&events), 2);
    }

    /// A body that booted and did real work resets the budget, so a Session that
    /// recovers, works, and strands again later is not punished for its history.
    #[test]
    fn real_progress_resets_the_recovery_budget() {
        let events = vec![
            recovery_event(9, 1),
            other_event(
                8,
                TaskEventKind::Progress {
                    summary: "implemented the classifier".to_string(),
                },
            ),
            recovery_event(7, 3),
            recovery_event(6, 2),
        ];
        assert_eq!(count_recovery_attempts(&events), 1);
        assert_eq!(count_recovery_attempts(&[]), 0);
    }
}
