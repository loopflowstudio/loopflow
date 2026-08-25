//! Durable input and execution identity shared by Wave, Project, and Task Work.

use std::path::PathBuf;

use serde::{Deserialize, Serialize};
use time::OffsetDateTime;

use crate::id::WaveId;

/// The exact active Run named by an in-Run process.
pub const RUN_ID_ENV: &str = "LF_RUN_ID";

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
    #[error("invalid Ask state: {0}")]
    InvalidAskState(String),
}

durable_id!(ProjectId, "proj_");
durable_id!(TaskId, "task_");
durable_id!(RunId, "run_");
durable_id!(AskId, "ask_");
durable_id!(HomeId, "home_");
durable_id!(SteerId, "steer_");
durable_id!(ToolResponseId, "response_");
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
    pub source_run_id: Option<RunId>,
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
    pub active_run_id: Option<RunId>,
    #[serde(with = "time::serde::rfc3339::option")]
    pub ready_at: Option<OffsetDateTime>,
    #[serde(with = "time::serde::rfc3339::option")]
    pub presented_at: Option<OffsetDateTime>,
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
    pub run_id: RunId,
    pub needs_launch: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AskSession {
    pub ask_id: AskId,
    pub run_id: RunId,
    pub home_route: String,
    pub attach_argv: Vec<String>,
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
    pub author: Author,
    pub text: String,
    #[serde(with = "time::serde::rfc3339")]
    pub issued_at: OffsetDateTime,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SteerReceipt {
    pub steer: Steer,
}

pub fn render_steers(steers: &[Steer]) -> String {
    if steers.is_empty() {
        return String::new();
    }
    let direction = steers
        .iter()
        .map(|steer| format!("- {}", steer.text))
        .collect::<Vec<_>>()
        .join("\n");
    format!("<lf:steers>\n{direction}\n</lf:steers>")
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
    pub request_id: String,
    pub choice: String,
    #[serde(with = "time::serde::rfc3339")]
    pub responded_at: OffsetDateTime,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum WorkStatus {
    Ready,
    Done,
    Abandoned,
}

impl WorkStatus {
    pub(crate) fn label(&self) -> &'static str {
        match self {
            Self::Ready => "ready",
            Self::Done => "done",
            Self::Abandoned => "abandoned",
        }
    }

    pub(crate) fn reason(&self) -> &'static str {
        self.label()
    }
}

impl std::fmt::Display for WorkStatus {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(self.label())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FlowPosition {
    pub work: WorkRef,
    pub flow: String,
    pub step: String,
    pub node_id: Option<String>,
    pub human: bool,
    pub step_index: u32,
    pub iteration: u32,
    #[serde(with = "time::serde::rfc3339")]
    pub updated_at: OffsetDateTime,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AbandonReceipt {
    pub work: WorkRef,
    pub reason: String,
    #[serde(with = "time::serde::rfc3339")]
    pub abandoned_at: OffsetDateTime,
}

#[cfg(test)]
mod tests {
    use super::{render_steers, AskSession, Steer, WorkStatus};
    use crate::durable::{Author, ProjectId, SteerId, WorkRef};

    #[test]
    fn ordered_steers_render_as_one_input_projection() {
        let work = WorkRef::Project(ProjectId::new());
        let steer = |text: &str| Steer {
            id: SteerId::new(),
            work: work.clone(),
            author: Author::User,
            text: text.to_string(),
            issued_at: time::OffsetDateTime::UNIX_EPOCH,
        };
        let steers = vec![steer("first"), steer("second")];

        assert_eq!(
            render_steers(&steers),
            "<lf:steers>\n- first\n- second\n</lf:steers>"
        );
    }

    #[test]
    fn ask_session_fixture_round_trips_generic_run_identity() {
        let fixture = include_str!("../../../tests/fixtures/dto/ask_session.json");
        let session: AskSession = serde_json::from_str(fixture).unwrap();
        assert_eq!(
            session.ask_id.as_str(),
            "ask_00000000000000000000000000000001"
        );
        assert_eq!(
            session.run_id.as_str(),
            "run_00000000000000000000000000000002"
        );

        let encoded = serde_json::to_string(&session).unwrap();
        let decoded: AskSession = serde_json::from_str(&encoded).unwrap();
        assert_eq!(decoded, session);
    }

    #[test]
    fn work_status_fixture_round_trips_every_current_variant() {
        let fixture = include_str!("../../../tests/fixtures/dto/work_statuses.json");
        let statuses: Vec<WorkStatus> = serde_json::from_str(fixture).unwrap();

        assert!(matches!(statuses[0], WorkStatus::Ready));
        assert!(matches!(statuses[1], WorkStatus::Done));
        assert!(matches!(statuses[2], WorkStatus::Abandoned));

        let encoded = serde_json::to_string(&statuses).unwrap();
        let decoded: Vec<WorkStatus> = serde_json::from_str(&encoded).unwrap();
        assert_eq!(decoded, statuses);
    }
}
