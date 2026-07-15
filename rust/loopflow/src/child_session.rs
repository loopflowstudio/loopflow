//! Durable control shared by Project and Task Sessions.
//!
//! This module owns typed child identity, control attribution, process
//! generations, commands, directives, decisions, and observation envelopes.
//! Project and Task keep their own lifecycle states, events, runners, and
//! public CLI nouns; there is no generic child lifecycle.

use std::path::PathBuf;
use std::time::Duration;

use serde::{Deserialize, Serialize};
use time::OffsetDateTime;

use crate::id::WaveId;
use crate::project_session::ProjectSessionId;
use crate::task::TaskSessionId;

pub(crate) const PROJECT_LEASE_TOKEN_ENV: &str = "LF_PROJECT_LEASE_TOKEN";
pub(crate) const PROJECT_GENERATION_ENV: &str = "LF_PROJECT_GENERATION";
pub(crate) const TASK_LEASE_TOKEN_ENV: &str = "LF_TASK_LEASE_TOKEN";
pub(crate) const TASK_GENERATION_ENV: &str = "LF_TASK_GENERATION";

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
    #[error("invalid child write lease: {0}")]
    InvalidWriteLease(String),
}

prefixed_uuid_id!(
    ChildCommandId,
    "cc_",
    ChildSessionDataError,
    ChildSessionDataError::InvalidId
);
prefixed_uuid_id!(
    ChildDecisionId,
    "cd_",
    ChildSessionDataError,
    ChildSessionDataError::InvalidId
);
prefixed_uuid_id!(
    ChildDirectiveId,
    "dir_",
    ChildSessionDataError,
    ChildSessionDataError::InvalidId
);

/// Opaque capability held only by the body allowed to write for a Session.
///
/// The token deliberately implements neither serialization nor display. Its
/// debug representation is redacted so an error path cannot turn the lease
/// into ambient authority.
#[derive(Clone, PartialEq, Eq)]
pub(crate) struct ChildLeaseToken(String);

impl ChildLeaseToken {
    pub(crate) fn new() -> Self {
        Self(format!("cl_{}", uuid::Uuid::new_v4().simple()))
    }

    pub(crate) fn parse(value: &str) -> Result<Self, ChildSessionDataError> {
        let suffix = value.strip_prefix("cl_").ok_or_else(|| {
            ChildSessionDataError::InvalidId("expected child lease token".to_string())
        })?;
        uuid::Uuid::parse_str(suffix)
            .map_err(|error| ChildSessionDataError::InvalidId(error.to_string()))?;
        Ok(Self(value.to_string()))
    }

    pub(crate) fn as_str(&self) -> &str {
        &self.0
    }
}

impl std::fmt::Debug for ChildLeaseToken {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str("ChildLeaseToken([REDACTED])")
    }
}

#[derive(Clone, PartialEq, Eq)]
pub(crate) struct ChildWriteLease {
    pub(crate) generation: u32,
    pub(crate) token: ChildLeaseToken,
}

impl std::fmt::Debug for ChildWriteLease {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("ChildWriteLease")
            .field("generation", &self.generation)
            .field("token", &self.token)
            .finish()
    }
}

pub(crate) fn task_write_lease_from_env() -> Result<ChildWriteLease, ChildSessionDataError> {
    child_write_lease_from_env("Task", TASK_GENERATION_ENV, TASK_LEASE_TOKEN_ENV)
}

pub(crate) fn project_write_lease_from_env() -> Result<ChildWriteLease, ChildSessionDataError> {
    child_write_lease_from_env("Project", PROJECT_GENERATION_ENV, PROJECT_LEASE_TOKEN_ENV)
}

fn child_write_lease_from_env(
    kind: &str,
    generation_env: &str,
    token_env: &str,
) -> Result<ChildWriteLease, ChildSessionDataError> {
    let generation = std::env::var(generation_env)
        .map_err(|_| {
            ChildSessionDataError::InvalidWriteLease(format!("{kind} body has no generation"))
        })?
        .parse::<u32>()
        .map_err(|_| {
            ChildSessionDataError::InvalidWriteLease(format!(
                "{kind} body generation is not an unsigned integer"
            ))
        })?;
    let token = std::env::var(token_env).map_err(|_| {
        ChildSessionDataError::InvalidWriteLease(format!("{kind} body has no lease token"))
    })?;
    Ok(ChildWriteLease {
        generation,
        token: ChildLeaseToken::parse(&token).map_err(|_| {
            ChildSessionDataError::InvalidWriteLease(format!("{kind} body lease token is invalid"))
        })?,
    })
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
}

impl ChildProcessGeneration {
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

/// The executable and store a Session runs against, pinned once when the
/// Session is created.
///
/// Re-deriving these per process is how a Session gets relaunched by a
/// worktree's `target/debug/lf` against the live registry: the launching
/// process's own binary and environment decided the child's, so whoever
/// happened to type the command chose the child's execution context. Pinning
/// makes the Session the authority, and every relaunch reproduces the context
/// the Session was born with.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ChildExecutionContext {
    pub lf_bin: PathBuf,
    pub db_path: PathBuf,
    pub lf_home: PathBuf,
}

#[cfg(test)]
impl ChildExecutionContext {
    /// A pinned context for tests that do not care which binary or store a
    /// Session names — only that it names one.
    pub(crate) fn for_tests() -> Self {
        Self {
            lf_bin: PathBuf::from("/usr/local/bin/lf"),
            db_path: PathBuf::from("/tmp/loopflow-test/loopflow.db"),
            lf_home: PathBuf::from("/tmp/loopflow-test"),
        }
    }
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
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum ChildCommandKind {
    FollowUp {
        text: String,
    },
    Steer {
        text: String,
    },
    Interrupt {
        replacement: Option<String>,
    },
    Resume {
        message: Option<String>,
    },
    Decide {
        decision_id: ChildDecisionId,
        choice: String,
        message: Option<String>,
    },
    Abandon {
        reason: String,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
#[non_exhaustive]
pub enum ChildCommandState {
    Persisted,
    Claimed,
    /// Provider delivery has begun. A process crash from here is ambiguous.
    Delivering,
    Accepted,
    Failed,
    Superseded,
    /// The process died after delivery began but before Loopflow recorded the outcome.
    Uncertain,
}

impl ChildCommandState {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Persisted => "persisted",
            Self::Claimed => "claimed",
            Self::Delivering => "delivering",
            Self::Accepted => "accepted",
            Self::Failed => "failed",
            Self::Superseded => "superseded",
            Self::Uncertain => "uncertain",
        }
    }

    pub fn is_terminal(self) -> bool {
        matches!(
            self,
            Self::Accepted | Self::Failed | Self::Superseded | Self::Uncertain
        )
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
#[non_exhaustive]
pub enum ChildCommandEffect {
    LiveSteer,
    NextTurn,
    Replacement,
    Decision,
}

impl ChildCommandEffect {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::LiveSteer => "live_steer",
            Self::NextTurn => "next_turn",
            Self::Replacement => "replacement",
            Self::Decision => "decision",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", content = "id", rename_all = "snake_case")]
pub enum ChildCommandSource {
    Wave(WaveId),
    Project(ProjectSessionId),
    Human,
    Attachment,
    System,
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

pub(crate) fn unincorporated_directive_version(
    current_version: u32,
    incorporated_version: u32,
) -> Option<u32> {
    (current_version > incorporated_version).then_some(current_version)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
#[non_exhaustive]
pub enum DirectiveKind {
    Initial,
    Replacement,
    WorkRevised,
}

impl DirectiveKind {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Initial => "initial",
            Self::Replacement => "replacement",
            Self::WorkRevised => "work_revised",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ChildDirective {
    pub id: ChildDirectiveId,
    pub target: ChildRef,
    pub version: u32,
    pub kind: DirectiveKind,
    pub text: String,
    pub source: ChildCommandSource,
    pub command_id: Option<ChildCommandId>,
    pub issued_at: OffsetDateTime,
    pub applied_at: Option<OffsetDateTime>,
    pub incorporated_at: Option<OffsetDateTime>,
    pub incorporated_summary: Option<String>,
}

impl ChildDirective {
    pub fn initial(target: ChildRef, text: String, source: ChildCommandSource) -> Self {
        Self {
            id: ChildDirectiveId::new(),
            target,
            version: 1,
            kind: DirectiveKind::Initial,
            text,
            source,
            command_id: None,
            issued_at: OffsetDateTime::now_utc(),
            applied_at: None,
            incorporated_at: None,
            incorporated_summary: None,
        }
    }

    pub fn replacement(
        target: ChildRef,
        version: u32,
        text: String,
        source: ChildCommandSource,
        command_id: ChildCommandId,
    ) -> Self {
        Self {
            id: ChildDirectiveId::new(),
            target,
            version,
            kind: DirectiveKind::Replacement,
            text,
            source,
            command_id: Some(command_id),
            issued_at: OffsetDateTime::now_utc(),
            applied_at: None,
            incorporated_at: None,
            incorporated_summary: None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ChildCommand {
    pub id: ChildCommandId,
    pub target: ChildRef,
    pub source: ChildCommandSource,
    pub kind: ChildCommandKind,
    pub state: ChildCommandState,
    pub effect: Option<ChildCommandEffect>,
    pub created_at: OffsetDateTime,
    pub claimed_by_generation: Option<u32>,
    pub accepted_at: Option<OffsetDateTime>,
    pub error: Option<String>,
}

impl ChildCommand {
    pub fn new(target: ChildRef, source: ChildCommandSource, kind: ChildCommandKind) -> Self {
        let effect = match &kind {
            ChildCommandKind::FollowUp { .. } | ChildCommandKind::Resume { message: Some(_) } => {
                Some(ChildCommandEffect::NextTurn)
            }
            ChildCommandKind::Interrupt {
                replacement: Some(_),
            } => Some(ChildCommandEffect::Replacement),
            ChildCommandKind::Decide { .. } => Some(ChildCommandEffect::Decision),
            ChildCommandKind::Steer { .. }
            | ChildCommandKind::Interrupt { replacement: None }
            | ChildCommandKind::Resume { message: None }
            | ChildCommandKind::Abandon { .. } => None,
        };
        Self {
            id: ChildCommandId::new(),
            target,
            source,
            kind,
            state: ChildCommandState::Persisted,
            effect,
            created_at: OffsetDateTime::now_utc(),
            claimed_by_generation: None,
            accepted_at: None,
            error: None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BoundaryResult<S> {
    Commands(Vec<ChildCommand>),
    Stopped(S),
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
        observe, unincorporated_directive_version, BodyCategory, BodyControl, BodyEvidence,
        BodyIntent, BodyOwner, ChildCommandId, ChildDecisionId, ChildDirectiveId, ChildLeaseState,
        ChildLeaseToken, ChildProcessGeneration, ChildWriteLease, Duration, DEFAULT_STALL_AFTER,
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

    #[test]
    fn child_ids_are_prefixed_and_round_trip() {
        let command = ChildCommandId::new();
        let decision = ChildDecisionId::new();
        let directive = ChildDirectiveId::new();

        assert_eq!(ChildCommandId::parse(command.as_str()).unwrap(), command);
        assert_eq!(ChildDecisionId::parse(decision.as_str()).unwrap(), decision);
        assert_eq!(
            ChildDirectiveId::parse(directive.as_str()).unwrap(),
            directive
        );
    }

    #[test]
    fn child_write_lease_never_prints_or_serializes_its_token() {
        let token = ChildLeaseToken::new();
        let raw = token.as_str().to_string();
        let lease = ChildWriteLease {
            generation: 7,
            token,
        };

        let debug = format!("{lease:?}");
        assert!(debug.contains("[REDACTED]"));
        assert!(!debug.contains(&raw));
        let evidence = ChildProcessGeneration {
            generation: lease.generation,
            pid: None,
            process_group_id: None,
            tmux_name: "body".to_string(),
            agent: "codex".to_string(),
            provider: "codex".to_string(),
            provider_session_id: None,
            started_at: time::OffsetDateTime::UNIX_EPOCH,
            state: ChildLeaseState::Reserved,
            outcome: None,
        };
        assert!(!serde_json::to_string(&evidence).unwrap().contains(&raw));
    }

    #[test]
    fn a_newer_directive_blocks_the_flow_boundary_until_incorporated() {
        assert_eq!(unincorporated_directive_version(2, 1), Some(2));
        assert_eq!(unincorporated_directive_version(2, 2), None);
    }
}
