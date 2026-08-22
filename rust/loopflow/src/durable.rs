//! Durable input and execution identity shared by Wave, Project, and Task Work.

use std::path::PathBuf;

use serde::{Deserialize, Serialize};
use time::OffsetDateTime;

use crate::id::WaveId;

/// The exact active Run named by an in-Run process.
pub const RUN_ID_ENV: &str = "LF_RUN_ID";

/// The exact core invocation started for an in-Run agent entrypoint.
pub const AGENT_INVOCATION_ENV: &str = "LF_AGENT_INVOCATION_ID";

/// Marks a process as an in-Run agent entrypoint even if its Run id was
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
    #[error("invalid invocation state: {0}")]
    InvalidInvocationState(String),
    #[error("invalid boundary state: {0}")]
    InvalidBoundaryState(String),
    #[error("invalid Ask state: {0}")]
    InvalidAskState(String),
}

durable_id!(ProjectId, "proj_");
durable_id!(TaskId, "task_");
durable_id!(EpochId, "epoch_");
durable_id!(RunId, "run_");
durable_id!(AgentInvocationId, "invocation_");
durable_id!(TurnId, "turn_");
durable_id!(AskId, "ask_");
durable_id!(WaitId, "wait_");
durable_id!(HomeId, "home_");
durable_id!(SteerId, "steer_");
durable_id!(SendId, "send_");
durable_id!(ToolResponseId, "response_");
durable_id!(DoneProposalId, "done_");
durable_id!(CronReceiptId, "cron_");

impl ProjectId {
    pub(crate) fn from_raw(value: impl Into<String>) -> Self {
        Self(value.into())
    }
}

impl TaskId {
    pub(crate) fn from_raw(value: impl Into<String>) -> Self {
        Self(value.into())
    }
}

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
    Migration,
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
    Recovery {
        prior_run_id: RunId,
    },
    HomeUpgrade {
        upgrade_id: String,
        prior_run_id: Option<RunId>,
    },
    User,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Run {
    pub id: RunId,
    pub work: WorkRef,
    pub epoch_id: EpochId,
    pub home_id: HomeId,
    pub runtime_generation: Option<u64>,
    pub state: RunState,
    pub trigger: RunTrigger,
    pub retry_of: Option<RunId>,
    pub containment: Option<Containment>,
    pub cwd: Option<PathBuf>,
    #[serde(with = "time::serde::rfc3339")]
    pub created_at: OffsetDateTime,
    #[serde(with = "time::serde::rfc3339::option")]
    pub started_at: Option<OffsetDateTime>,
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
pub struct Placement {
    pub work: WorkRef,
    pub home_id: HomeId,
    pub enabled: bool,
    #[serde(with = "time::serde::rfc3339")]
    pub placed_at: OffsetDateTime,
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
pub struct InvocationRoute {
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
                    DurableDataError::InvalidRunState(format!(
                        "invalid process group {id:?}: {error}"
                    ))
                }),
            "tmux" => Ok(Self::Tmux { name: id }),
            value => Err(DurableDataError::InvalidRunState(format!(
                "invalid containment kind: {value}"
            ))),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AgentInvocation {
    pub id: AgentInvocationId,
    pub supervising_run_id: Option<RunId>,
    pub answer_ask_id: Option<AskId>,
    pub route: InvocationRoute,
    pub surface: String,
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
            value => Err(DurableDataError::InvalidInvocationState(format!(
                "invalid Invocation handback state: {value}"
            ))),
        }
    }

    pub(crate) fn as_invocation_outcome(self) -> &'static str {
        match self {
            Self::Starting | Self::Active => "running",
            Self::Succeeded => "completed",
            Self::Failed => "failed",
            Self::Interrupted => "interrupted",
            Self::Unknown => "unknown",
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
    pub invocation_id: AgentInvocationId,
    pub basis: Basis,
    pub state: BoundaryState,
    pub provider_turn_id: Option<String>,
    pub root_output: Option<String>,
    #[serde(with = "time::serde::rfc3339")]
    pub started_at: OffsetDateTime,
    #[serde(with = "time::serde::rfc3339::option")]
    pub ended_at: Option<OffsetDateTime>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", content = "work", rename_all = "snake_case")]
pub enum AskTarget {
    User,
    Parent(WorkRef),
}

impl std::fmt::Display for AskTarget {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::User => formatter.write_str("user"),
            Self::Parent(work) => {
                write!(formatter, "parent:{}:{}", work.kind(), work.id())
            }
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AskState {
    Queued,
    Claimed,
    Resolved,
    Declined,
    Cancelled,
}

impl AskState {
    pub fn is_terminal(self) -> bool {
        matches!(self, Self::Resolved | Self::Declined | Self::Cancelled)
    }

    pub(crate) fn as_str(self) -> &'static str {
        match self {
            Self::Queued => "queued",
            Self::Claimed => "claimed",
            Self::Resolved => "resolved",
            Self::Declined => "declined",
            Self::Cancelled => "cancelled",
        }
    }

    pub(crate) fn parse(value: &str) -> Result<Self, DurableDataError> {
        match value {
            "queued" => Ok(Self::Queued),
            "claimed" => Ok(Self::Claimed),
            "resolved" => Ok(Self::Resolved),
            "declined" => Ok(Self::Declined),
            "cancelled" => Ok(Self::Cancelled),
            value => Err(DurableDataError::InvalidAskState(value.to_string())),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AskOrigin {
    pub work: WorkRef,
    pub run_id: RunId,
    pub turn_id: Option<TurnId>,
    pub invocation_id: Option<AgentInvocationId>,
    pub home_id: HomeId,
    pub cwd: PathBuf,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum AskBody {
    Intervention {
        prompt: String,
    },
    FlowStep {
        flow: String,
        node_id: String,
        skill: String,
        iteration: u32,
    },
}

impl std::fmt::Display for AskBody {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Intervention { prompt } => formatter.write_str(prompt),
            Self::FlowStep {
                flow,
                node_id,
                skill,
                ..
            } => write!(formatter, "{flow}:{node_id} ({skill})"),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum AskResult {
    Resolved { summary: String },
    Declined { reason: String },
    Cancelled { reason: String },
}

impl AskResult {
    pub(crate) fn state(&self) -> AskState {
        match self {
            Self::Resolved { .. } => AskState::Resolved,
            Self::Declined { .. } => AskState::Declined,
            Self::Cancelled { .. } => AskState::Cancelled,
        }
    }

    pub fn text(&self) -> &str {
        match self {
            Self::Resolved { summary } => summary,
            Self::Declined { reason } | Self::Cancelled { reason } => reason,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Ask {
    pub id: AskId,
    pub origin: AskOrigin,
    pub target: AskTarget,
    pub request: AskBody,
    pub state: AskState,
    pub active_invocation_id: Option<AgentInvocationId>,
    pub result: Option<AskResult>,
    pub terminal_author: Option<Author>,
    #[serde(with = "time::serde::rfc3339")]
    pub asked_at: OffsetDateTime,
    #[serde(with = "time::serde::rfc3339::option")]
    pub terminal_at: Option<OffsetDateTime>,
}

/// One target-authorized Ask claim.
#[derive(Debug)]
pub struct AskClaim {
    pub invocation_id: AgentInvocationId,
    pub needs_launch: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RunAdvance {
    RunStarting {
        containment: Containment,
        cwd: PathBuf,
    },
    InvocationStarting {
        route: InvocationRoute,
        surface: String,
        resume_token: Option<String>,
        answer_ask_id: Option<AskId>,
    },
    InvocationEnded {
        invocation_id: AgentInvocationId,
        outcome: BoundaryState,
    },
    TurnStarting {
        invocation_id: AgentInvocationId,
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
    Invocation(AgentInvocation),
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

/// The current Run identity and input basis used to fence writes.
#[derive(Debug, Clone)]
pub struct RunContext {
    pub run_id: RunId,
    pub work: WorkRef,
    pub basis: Basis,
}

/// Direct lifecycle control observed by the exact active Run.
///
/// This is a projection over Run, Epoch, AgentInvocation, and Turn state. It is not a
/// stored inbox: the mutation already landed on the authority it controls.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RunControl {
    Interrupt,
    Quiesce { upgrade_id: String, deadline: i64 },
    Abandon { reason: String },
}

impl RunContext {
    pub(crate) fn new(run_id: RunId, work: WorkRef, basis: Basis) -> Self {
        Self {
            run_id,
            work,
            basis,
        }
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
    pub node_id: Option<String>,
    pub human: bool,
    pub step_index: u32,
    pub iteration: u32,
    #[serde(with = "time::serde::rfc3339")]
    pub updated_at: OffsetDateTime,
}

/// The generic surface for reopening an opaque or provider-backed Invocation.
/// Attach is a route projection; it does not create identity or liveness.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct InvocationSurface {
    pub invocation: AgentInvocation,
    pub run: Run,
    pub work: WorkRef,
    pub wave_id: WaveId,
    pub home_route: String,
    pub handback: Option<BoundaryState>,
    pub attach_argv: Option<Vec<String>>,
    pub current: InvocationObservation,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ContainmentObservation {
    Absent,
    Present,
    Unprovable,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RunLivenessState {
    Present,
    Absent,
    Unprovable,
}

impl From<ContainmentObservation> for RunLivenessState {
    fn from(value: ContainmentObservation) -> Self {
        match value {
            ContainmentObservation::Present => Self::Present,
            ContainmentObservation::Absent => Self::Absent,
            ContainmentObservation::Unprovable => Self::Unprovable,
        }
    }
}

impl From<RunLivenessState> for ContainmentObservation {
    fn from(value: RunLivenessState) -> Self {
        match value {
            RunLivenessState::Present => Self::Present,
            RunLivenessState::Absent => Self::Absent,
            RunLivenessState::Unprovable => Self::Unprovable,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RunLivenessReceipt {
    pub run_id: RunId,
    pub home_id: HomeId,
    pub state: RunLivenessState,
    pub observed_at: OffsetDateTime,
    pub runtime_generation: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RunLivenessEvidence {
    pub state: RunLivenessState,
    #[serde(with = "time::serde::rfc3339::option")]
    pub observed_at: Option<OffsetDateTime>,
    pub fresh: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum InvocationObservationState {
    Live,
    Unverified,
    History,
}

impl std::fmt::Display for InvocationObservationState {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(match self {
            Self::Live => "live",
            Self::Unverified => "unverified",
            Self::History => "history",
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct InvocationObservation {
    pub state: InvocationObservationState,
    pub reason: String,
    pub liveness: Option<RunLivenessEvidence>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum StopCause {
    Requested,
    Interrupted,
    Failed { reason: String },
    Recovery,
    HomeUpgrade { upgrade_id: String, deadline: i64 },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct StopReceipt {
    pub run: Run,
    pub containment: ContainmentObservation,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct InterruptReceipt {
    pub run_id: RunId,
    pub turn_ids: Vec<TurnId>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EpochReceipt {
    pub epoch: Epoch,
}

#[cfg(test)]
mod tests {
    use super::{Basis, BoundarySeed, EpochId, InvocationSurface, Steer, WaitOn, WorkStatus};
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
    fn invocation_surface_fixture_round_trips() {
        let fixture = include_str!("../../../tests/fixtures/dto/invocation_surface.json");
        let surface: InvocationSurface = serde_json::from_str(fixture).unwrap();
        assert_eq!(surface.invocation.route.provider, "opaque");
        assert_eq!(surface.work.kind(), "task");

        let encoded = serde_json::to_string(&surface).unwrap();
        let decoded: InvocationSurface = serde_json::from_str(&encoded).unwrap();
        assert_eq!(decoded, surface);
    }

    #[test]
    fn work_status_fixture_round_trips_every_variant() {
        let fixture = include_str!("../../../tests/fixtures/dto/work_statuses.json");
        let statuses: Vec<WorkStatus> = serde_json::from_str(fixture).unwrap();

        assert!(matches!(statuses[0], WorkStatus::Ready));
        assert!(matches!(statuses[1], WorkStatus::Running { .. }));
        let wait_kinds = statuses[2..8]
            .iter()
            .map(|status| match status {
                WorkStatus::Waiting { wait } => match &wait.on {
                    WaitOn::Input { .. } => "input",
                    WaitOn::Time { .. } => "time",
                    WaitOn::Event { .. } => "event",
                    WaitOn::Child { .. } => "child",
                    WaitOn::Capability { .. } => "capability",
                    WaitOn::Effect { .. } => "effect",
                },
                _ => panic!("expected waiting status"),
            })
            .collect::<Vec<_>>();
        assert_eq!(
            wait_kinds,
            ["input", "time", "event", "child", "capability", "effect"]
        );
        assert!(matches!(statuses[8], WorkStatus::Done));
        assert!(matches!(statuses[9], WorkStatus::Abandoned));

        let encoded = serde_json::to_string(&statuses).unwrap();
        let decoded: Vec<WorkStatus> = serde_json::from_str(&encoded).unwrap();
        assert_eq!(decoded, statuses);
    }
}
