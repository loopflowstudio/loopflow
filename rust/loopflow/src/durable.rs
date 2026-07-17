//! Durable input and execution identity shared by Wave, Project, and Task Work.

use std::path::PathBuf;

use serde::{Deserialize, Serialize};
use time::OffsetDateTime;

use crate::id::WaveId;

/// The one opaque capability inherited by every command inside an active Run.
pub const RUN_LEASE_ENV: &str = "LF_RUN_LEASE";

/// Marks a process as an in-Run agent entrypoint even if its capability was
/// accidentally stripped. That case must fail closed rather than become User.
pub const RUN_CONTEXT_ENV: &str = "LF_RUN_CONTEXT";

macro_rules! durable_id {
    ($name:ident, $prefix:literal) => {
        #[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
        #[serde(transparent)]
        pub struct $name(String);

        impl $name {
            pub fn new() -> Self {
                Self(format!("{}{}", $prefix, uuid::Uuid::new_v4().simple()))
            }

            pub fn parse(value: &str) -> Result<Self, DurableDataError> {
                let suffix = value.strip_prefix($prefix).ok_or_else(|| {
                    DurableDataError::InvalidId(format!("expected {} id", $prefix))
                })?;
                uuid::Uuid::parse_str(suffix)
                    .map_err(|error| DurableDataError::InvalidId(error.to_string()))?;
                Ok(Self(value.to_string()))
            }

            pub fn as_str(&self) -> &str {
                &self.0
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
            type Err = DurableDataError;

            fn from_str(value: &str) -> Result<Self, Self::Err> {
                Self::parse(value)
            }
        }
    };
}

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum DurableDataError {
    #[error("invalid durable id: {0}")]
    InvalidId(String),
    #[error("invalid epoch state: {0}")]
    InvalidEpochState(String),
    #[error("invalid send state: {0}")]
    InvalidSendState(String),
    #[error("invalid run state: {0}")]
    InvalidRunState(String),
    #[error("invalid launch state: {0}")]
    InvalidLaunchState(String),
    #[error("invalid boundary state: {0}")]
    InvalidBoundaryState(String),
}

durable_id!(ProjectId, "proj_");
durable_id!(TaskId, "task_");
durable_id!(EpochId, "epoch_");
durable_id!(RunId, "run_");
durable_id!(LaunchId, "launch_");
durable_id!(TurnId, "turn_");
durable_id!(WaitId, "wait_");
durable_id!(HomeId, "home_");
durable_id!(SteerId, "steer_");
durable_id!(SendId, "send_");
durable_id!(ToolResponseId, "response_");
durable_id!(DoneProposalId, "done_");

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(tag = "kind", content = "id", rename_all = "snake_case")]
pub enum WorkRef {
    Wave(WaveId),
    Project(ProjectId),
    Task(TaskId),
}

impl WorkRef {
    pub fn kind(&self) -> &'static str {
        match self {
            Self::Wave(_) => "wave",
            Self::Project(_) => "project",
            Self::Task(_) => "task",
        }
    }

    pub fn id(&self) -> &str {
        match self {
            Self::Wave(id) => id.as_str(),
            Self::Project(id) => id.as_str(),
            Self::Task(id) => id.as_str(),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EpochState {
    Open,
    Done,
    Abandoned,
}

impl EpochState {
    pub(crate) fn as_str(self) -> &'static str {
        match self {
            Self::Open => "open",
            Self::Done => "done",
            Self::Abandoned => "abandoned",
        }
    }

    pub(crate) fn parse(value: &str) -> Result<Self, DurableDataError> {
        match value {
            "open" => Ok(Self::Open),
            "done" => Ok(Self::Done),
            "abandoned" => Ok(Self::Abandoned),
            value => Err(DurableDataError::InvalidEpochState(value.to_string())),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Basis {
    pub epoch_id: EpochId,
    pub revision: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Epoch {
    pub id: EpochId,
    pub work: WorkRef,
    pub number: u32,
    pub state: EpochState,
    pub current_basis: Basis,
    #[serde(with = "time::serde::rfc3339")]
    pub created_at: OffsetDateTime,
    #[serde(with = "time::serde::rfc3339::option")]
    pub terminal_at: Option<OffsetDateTime>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RunState {
    Reserved,
    Active,
    Stopping,
    Ended,
}

impl RunState {
    pub(crate) fn parse(value: &str) -> Result<Self, DurableDataError> {
        match value {
            "reserved" => Ok(Self::Reserved),
            "active" => Ok(Self::Active),
            "stopping" => Ok(Self::Stopping),
            "ended" => Ok(Self::Ended),
            value => Err(DurableDataError::InvalidRunState(value.to_string())),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum RunTrigger {
    Input {
        basis: Basis,
    },
    Time {
        #[serde(with = "time::serde::rfc3339")]
        scheduled_at: OffsetDateTime,
    },
    Event {
        event: EventRef,
    },
    Child {
        work: WorkRef,
    },
    CiIncident {
        incident_id: String,
    },
    Recovery {
        prior_run_id: RunId,
    },
    User,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Run {
    pub id: RunId,
    pub work: WorkRef,
    pub epoch_id: EpochId,
    pub home_id: HomeId,
    pub state: RunState,
    pub trigger: RunTrigger,
    pub retry_of: Option<RunId>,
    #[serde(with = "time::serde::rfc3339")]
    pub created_at: OffsetDateTime,
    #[serde(with = "time::serde::rfc3339::option")]
    pub ended_at: Option<OffsetDateTime>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Home {
    pub id: HomeId,
    pub route: String,
    #[serde(with = "time::serde::rfc3339")]
    pub created_at: OffsetDateTime,
    #[serde(with = "time::serde::rfc3339")]
    pub observed_at: OffsetDateTime,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum WaitOn {
    Input {
        after: Basis,
    },
    Time {
        #[serde(with = "time::serde::rfc3339")]
        not_before: OffsetDateTime,
    },
    Event {
        event: EventRef,
    },
    Child {
        work: WorkRef,
    },
    Capability {
        capability: CapabilityRef,
    },
    Effect {
        effect: EffectRef,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EventRef {
    pub source: String,
    pub id: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CapabilityRef {
    pub kind: String,
    pub key: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EffectRef {
    pub kind: String,
    pub idempotency_key: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Wait {
    pub id: WaitId,
    pub work: WorkRef,
    pub epoch_id: EpochId,
    pub on: WaitOn,
    #[serde(with = "time::serde::rfc3339")]
    pub created_at: OffsetDateTime,
    #[serde(with = "time::serde::rfc3339::option")]
    pub resolved_at: Option<OffsetDateTime>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LaunchRoute {
    pub provider: String,
    pub model: Option<String>,
    pub account_id: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Containment {
    ProcessGroup { id: i64 },
    Tmux { name: String },
}

impl Containment {
    pub(crate) fn parts(&self) -> (&'static str, String) {
        match self {
            Self::ProcessGroup { id } => ("process_group", id.to_string()),
            Self::Tmux { name } => ("tmux", name.clone()),
        }
    }

    pub(crate) fn parse(kind: &str, id: String) -> Result<Self, DurableDataError> {
        match kind {
            "process_group" => id
                .parse()
                .map(|id| Self::ProcessGroup { id })
                .map_err(|error| {
                    DurableDataError::InvalidLaunchState(format!(
                        "invalid process group {id:?}: {error}"
                    ))
                }),
            "tmux" => Ok(Self::Tmux { name: id }),
            value => Err(DurableDataError::InvalidLaunchState(format!(
                "invalid containment kind: {value}"
            ))),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LaunchState {
    Starting,
    Live,
    Stopping,
    Ended,
}

impl LaunchState {
    pub(crate) fn parse(value: &str) -> Result<Self, DurableDataError> {
        match value {
            "starting" => Ok(Self::Starting),
            "live" => Ok(Self::Live),
            "stopping" => Ok(Self::Stopping),
            "ended" => Ok(Self::Ended),
            value => Err(DurableDataError::InvalidLaunchState(value.to_string())),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Launch {
    pub id: LaunchId,
    pub run_id: RunId,
    pub home_id: HomeId,
    pub route: LaunchRoute,
    pub cwd: PathBuf,
    pub surface: String,
    pub state: LaunchState,
    pub containment: Containment,
    pub opaque_basis: Option<Basis>,
    pub resume_token: Option<String>,
    #[serde(with = "time::serde::rfc3339")]
    pub started_at: OffsetDateTime,
    #[serde(with = "time::serde::rfc3339::option")]
    pub ended_at: Option<OffsetDateTime>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BoundaryState {
    Starting,
    Active,
    Succeeded,
    Failed,
    Interrupted,
    Unknown,
}

impl BoundaryState {
    pub fn is_terminal(self) -> bool {
        matches!(
            self,
            Self::Succeeded | Self::Failed | Self::Interrupted | Self::Unknown
        )
    }

    pub(crate) fn parse_handback(value: &str) -> Result<Self, DurableDataError> {
        match value {
            "succeeded" => Ok(Self::Succeeded),
            "failed" => Ok(Self::Failed),
            "interrupted" => Ok(Self::Interrupted),
            "unknown" => Ok(Self::Unknown),
            value => Err(DurableDataError::InvalidLaunchState(format!(
                "invalid Launch handback state: {value}"
            ))),
        }
    }

    pub(crate) fn as_launch_outcome(self) -> &'static str {
        match self {
            Self::Starting | Self::Active => "running",
            Self::Succeeded => "completed",
            Self::Failed | Self::Unknown => "failed",
            Self::Interrupted => "interrupted",
        }
    }

    pub(crate) fn as_turn_status(self) -> &'static str {
        match self {
            Self::Starting | Self::Active => "running",
            Self::Succeeded => "completed",
            Self::Failed => "failed",
            Self::Interrupted => "interrupted",
            Self::Unknown => "partial",
        }
    }

    pub(crate) fn parse_turn(value: &str) -> Result<Self, DurableDataError> {
        match value {
            "running" => Ok(Self::Active),
            "completed" => Ok(Self::Succeeded),
            "failed" => Ok(Self::Failed),
            "interrupted" => Ok(Self::Interrupted),
            "partial" => Ok(Self::Unknown),
            value => Err(DurableDataError::InvalidBoundaryState(value.to_string())),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Turn {
    pub id: TurnId,
    pub launch_id: LaunchId,
    pub basis: Basis,
    pub state: BoundaryState,
    pub provider_turn_id: Option<String>,
    pub root_output: Option<String>,
    #[serde(with = "time::serde::rfc3339")]
    pub started_at: OffsetDateTime,
    #[serde(with = "time::serde::rfc3339::option")]
    pub ended_at: Option<OffsetDateTime>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RunAdvance {
    LaunchStarting {
        route: LaunchRoute,
        containment: Containment,
        cwd: PathBuf,
        surface: String,
        opaque: bool,
        resume_token: Option<String>,
    },
    LaunchLive {
        launch_id: LaunchId,
    },
    LaunchEnded {
        launch_id: LaunchId,
        outcome: BoundaryState,
    },
    TurnStarting {
        launch_id: LaunchId,
    },
    TurnActive {
        turn_id: TurnId,
        provider_turn_id: Option<String>,
    },
    TurnEnded {
        turn_id: TurnId,
        outcome: BoundaryState,
    },
    Wait {
        on: WaitOn,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AdvanceReceipt {
    Run(Run),
    Launch(Launch),
    Turn(Turn),
    Wait(Wait),
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", content = "id", rename_all = "snake_case")]
pub enum Author {
    User,
    Run(RunId),
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Steer {
    pub id: SteerId,
    pub work: WorkRef,
    pub basis: Basis,
    pub author: Author,
    pub text: String,
    #[serde(with = "time::serde::rfc3339")]
    pub issued_at: OffsetDateTime,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SendVia {
    Live,
    Seed,
}

impl SendVia {
    pub(crate) fn as_str(self) -> &'static str {
        match self {
            Self::Live => "live",
            Self::Seed => "seed",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SendState {
    Sending,
    Sent,
    Failed,
    Unknown,
}

impl SendState {
    pub(crate) fn as_str(self) -> &'static str {
        match self {
            Self::Sending => "sending",
            Self::Sent => "sent",
            Self::Failed => "failed",
            Self::Unknown => "unknown",
        }
    }

    pub(crate) fn parse(value: &str) -> Result<Self, DurableDataError> {
        match value {
            "sending" => Ok(Self::Sending),
            "sent" => Ok(Self::Sent),
            "failed" => Ok(Self::Failed),
            "unknown" => Ok(Self::Unknown),
            value => Err(DurableDataError::InvalidSendState(value.to_string())),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Send {
    pub id: SendId,
    pub steer_id: SteerId,
    pub turn_id: String,
    pub via: SendVia,
    pub state: SendState,
    pub provider_turn_id: Option<String>,
    pub reason: Option<String>,
    #[serde(with = "time::serde::rfc3339")]
    pub attempted_at: OffsetDateTime,
    #[serde(with = "time::serde::rfc3339::option")]
    pub finished_at: Option<OffsetDateTime>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SteerReceipt {
    pub steer: Steer,
    pub sends: Vec<Send>,
    pub applied_by: Option<Basis>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BoundarySeed {
    pub basis: Basis,
    pub steers: Vec<Steer>,
}

impl BoundarySeed {
    pub fn render(&self) -> String {
        if self.steers.is_empty() {
            return String::new();
        }
        let direction = self
            .steers
            .iter()
            .map(|steer| format!("- [rev {}] {}", steer.basis.revision, steer.text))
            .collect::<Vec<_>>()
            .join("\n");
        format!(
            "<lf:steers basis=\"{}:{}\">\n{}\n</lf:steers>",
            self.basis.epoch_id, self.basis.revision, direction
        )
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ToolResponseWrite {
    pub request_id: String,
    pub choice: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ToolResponseReceipt {
    pub id: ToolResponseId,
    pub work: WorkRef,
    pub basis: Basis,
    pub request_id: String,
    pub choice: String,
    #[serde(with = "time::serde::rfc3339")]
    pub responded_at: OffsetDateTime,
}

/// Authenticated Home-local entrypoint. It is deliberately not serializable.
#[derive(Debug)]
pub struct AuthenticatedRequest {
    _private: (),
}

impl AuthenticatedRequest {
    pub(crate) fn cli() -> Self {
        Self { _private: () }
    }
}

/// An already-validated Run capability. Tokens never enter durable DTOs.
#[derive(Debug, Clone)]
pub struct RunLease {
    pub run_id: RunId,
    pub work: WorkRef,
    pub basis: Basis,
    _token: RunLeaseToken,
}

/// Direct lifecycle control observed by the exact active Run.
///
/// This is a projection over Run, Epoch, Launch, and Turn state. It is not a
/// stored inbox: the mutation already landed on the authority it controls.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RunControl {
    Interrupt,
    Abandon { reason: String },
}

impl RunLease {
    pub(crate) fn new(run_id: RunId, work: WorkRef, basis: Basis, token: RunLeaseToken) -> Self {
        Self {
            run_id,
            work,
            basis,
            _token: token,
        }
    }

    pub(crate) fn token_hash(&self) -> String {
        self._token.hash()
    }

    pub(crate) fn env_value(&self) -> &str {
        self._token.env_value()
    }
}

/// Opaque capability for the one active Run. It is never serialized or shown.
#[derive(Clone, PartialEq, Eq)]
pub(crate) struct RunLeaseToken(String);

impl RunLeaseToken {
    pub(crate) fn new() -> Self {
        Self(format!("rl_{}", uuid::Uuid::new_v4().simple()))
    }

    pub(crate) fn parse(value: &str) -> Result<Self, DurableDataError> {
        let value = value.trim();
        let suffix = value
            .strip_prefix("rl_")
            .ok_or_else(|| DurableDataError::InvalidId("expected opaque Run lease".to_string()))?;
        uuid::Uuid::parse_str(suffix)
            .map_err(|error| DurableDataError::InvalidId(error.to_string()))?;
        Ok(Self(value.to_string()))
    }

    pub(crate) fn env_value(&self) -> &str {
        &self.0
    }

    pub(crate) fn hash(&self) -> String {
        use sha2::{Digest, Sha256};

        format!("{:x}", Sha256::digest(self.0.as_bytes()))
    }
}

impl std::fmt::Debug for RunLeaseToken {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str("RunLeaseToken([REDACTED])")
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DoneProposal {
    pub id: DoneProposalId,
    pub run_id: RunId,
    pub basis: Basis,
    #[serde(with = "time::serde::rfc3339")]
    pub proposed_at: OffsetDateTime,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum WorkStatus {
    Ready,
    Running { run_id: RunId },
    Waiting { wait: Wait },
    Done,
    Abandoned,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FlowPosition {
    pub work: WorkRef,
    pub epoch_id: EpochId,
    pub flow: String,
    pub step: String,
    pub step_index: u32,
    pub iteration: u32,
    pub feedback: bool,
    #[serde(with = "time::serde::rfc3339")]
    pub updated_at: OffsetDateTime,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", content = "work", rename_all = "snake_case")]
pub enum AttentionRoute {
    User,
    Parent(WorkRef),
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Feedback {
    pub work: WorkRef,
    pub launch_id: LaunchId,
    pub basis: Basis,
    pub position: FlowPosition,
    /// Stable route for the entire Feedback step.
    pub attention: AttentionRoute,
    #[serde(with = "time::serde::rfc3339")]
    pub opened_at: OffsetDateTime,
    /// The current unanswered child Turn; absent while the child is acting.
    #[serde(with = "time::serde::rfc3339::option")]
    pub attention_at: Option<OffsetDateTime>,
}

/// Oldest-first parent control input reconstructed from durable child facts.
///
/// This is a query result, not an inbox row. If delivery races a boundary the
/// parent can render the same projection again from the child Feedback.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ChildFeedback {
    pub feedback: Feedback,
    pub latest_output: Option<String>,
    pub evidence: serde_json::Value,
}

/// User-facing Feedback reconstructed from current durable Work facts.
///
/// This is a query result, not a second lifecycle model or a queue row.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct UserFeedback {
    pub feedback: Feedback,
    pub surface: LaunchSurface,
    pub latest_output: Option<String>,
    pub evidence: serde_json::Value,
}

impl ChildFeedback {
    pub fn render(&self) -> String {
        let kind = self.feedback.work.kind();
        let id = self.feedback.work.id();
        let facts =
            serde_json::to_string_pretty(self).expect("Child Feedback facts must serialize");
        format!(
            "<lf:child-feedback work-kind=\"{kind}\" work-id=\"{id}\" basis=\"{}:{}\">\n\
             Service this child before background parent work. Use \
             `lf work steer {kind} {id} \"<response>\"` to continue or \
             `lf work continue {kind} {id}` to continue the flow. Delivery alone does not clear attention.\n\n\
             Durable child output and current evidence:\n{facts}\n</lf:child-feedback>",
            self.feedback.basis.epoch_id, self.feedback.basis.revision,
        )
    }
}

/// The generic surface for reopening an opaque or provider-backed Launch.
/// Attach is a route projection; it does not create identity or liveness.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LaunchSurface {
    pub launch: Launch,
    pub work: WorkRef,
    pub wave_id: WaveId,
    pub home_route: String,
    pub attention: Option<AttentionRoute>,
    #[serde(with = "time::serde::rfc3339::option")]
    pub attention_at: Option<OffsetDateTime>,
    pub handback: Option<BoundaryState>,
    pub attach_argv: Option<Vec<String>>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ContainmentObservation {
    Absent,
    Present,
    Unprovable,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum StopCause {
    Requested,
    Interrupted,
    Failed { reason: String },
    Recovery,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct StopReceipt {
    pub run: Run,
    pub containment: ContainmentObservation,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct InterruptReceipt {
    pub run_id: RunId,
    pub launch_id: LaunchId,
    pub turn_id: Option<TurnId>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EpochReceipt {
    pub epoch: Epoch,
}

#[derive(Debug)]
pub enum ControlCtx<'a> {
    User(&'a AuthenticatedRequest),
    Run(&'a RunLease),
}

#[cfg(test)]
mod tests {
    use super::{Basis, BoundarySeed, EpochId, LaunchSurface, Steer, WaitOn, WorkStatus};
    use crate::durable::{Author, ProjectId, SteerId, WorkRef};

    #[test]
    fn ordered_steers_render_as_one_basis_projection() {
        let epoch_id = EpochId::new();
        let work = WorkRef::Project(ProjectId::new());
        let steer = |revision, text: &str| Steer {
            id: SteerId::new(),
            work: work.clone(),
            basis: Basis {
                epoch_id: epoch_id.clone(),
                revision,
            },
            author: Author::User,
            text: text.to_string(),
            issued_at: time::OffsetDateTime::UNIX_EPOCH,
        };
        let seed = BoundarySeed {
            basis: Basis {
                epoch_id: epoch_id.clone(),
                revision: 3,
            },
            steers: vec![steer(2, "first"), steer(3, "second")],
        };

        assert_eq!(
            seed.render(),
            format!(
                "<lf:steers basis=\"{}:3\">\n- [rev 2] first\n- [rev 3] second\n</lf:steers>",
                seed.basis.epoch_id
            )
        );
    }

    #[test]
    fn launch_surface_fixture_round_trips() {
        let fixture = include_str!("../../../tests/fixtures/dto/launch_surface.json");
        let surface: LaunchSurface = serde_json::from_str(fixture).unwrap();
        assert_eq!(surface.launch.route.provider, "opaque");
        assert_eq!(surface.work.kind(), "task");
        assert!(matches!(
            surface.attention,
            Some(super::AttentionRoute::User)
        ));
        assert!(surface.attention_at.is_some());

        let encoded = serde_json::to_string(&surface).unwrap();
        let decoded: LaunchSurface = serde_json::from_str(&encoded).unwrap();
        assert_eq!(decoded, surface);
    }

    #[test]
    fn work_status_fixture_round_trips_every_variant() {
        let fixture = include_str!("../../../tests/fixtures/dto/work_statuses.json");
        let statuses: Vec<WorkStatus> = serde_json::from_str(fixture).unwrap();

        assert!(matches!(statuses[0], WorkStatus::Ready));
        assert!(matches!(statuses[1], WorkStatus::Running { .. }));
        assert!(matches!(
            statuses[2],
            WorkStatus::Waiting {
                wait: super::Wait {
                    on: WaitOn::Input { .. },
                    ..
                }
            }
        ));
        assert!(matches!(
            statuses[3],
            WorkStatus::Waiting {
                wait: super::Wait {
                    on: WaitOn::Time { .. },
                    ..
                }
            }
        ));
        assert!(matches!(
            statuses[4],
            WorkStatus::Waiting {
                wait: super::Wait {
                    on: WaitOn::Event { .. },
                    ..
                }
            }
        ));
        assert!(matches!(
            statuses[5],
            WorkStatus::Waiting {
                wait: super::Wait {
                    on: WaitOn::Child { .. },
                    ..
                }
            }
        ));
        assert!(matches!(
            statuses[6],
            WorkStatus::Waiting {
                wait: super::Wait {
                    on: WaitOn::Capability { .. },
                    ..
                }
            }
        ));
        assert!(matches!(
            statuses[7],
            WorkStatus::Waiting {
                wait: super::Wait {
                    on: WaitOn::Effect { .. },
                    ..
                }
            }
        ));
        assert!(matches!(statuses[8], WorkStatus::Done));
        assert!(matches!(statuses[9], WorkStatus::Abandoned));

        let encoded = serde_json::to_string(&statuses).unwrap();
        let decoded: Vec<WorkStatus> = serde_json::from_str(&encoded).unwrap();
        assert_eq!(decoded, statuses);
    }
}
