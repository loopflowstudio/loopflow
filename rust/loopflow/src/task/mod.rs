use std::fmt;
use std::path::PathBuf;
use std::str::FromStr;

use serde::{Deserialize, Serialize};
use time::OffsetDateTime;
use uuid::Uuid;

use crate::lfd::id::LfdId;
use crate::project_session::SessionSupervisor;

pub mod runner;

macro_rules! string_id {
    ($name:ident, $prefix:literal) => {
        #[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
        #[serde(transparent)]
        pub struct $name(String);

        impl $name {
            pub fn new() -> Self {
                Self(format!("{}{}", $prefix, Uuid::new_v4().simple()))
            }

            pub fn parse(value: &str) -> Result<Self, TaskDataError> {
                let suffix = value
                    .strip_prefix($prefix)
                    .ok_or_else(|| TaskDataError::InvalidId(format!("expected {} id", $prefix)))?;
                Uuid::parse_str(suffix)
                    .map_err(|error| TaskDataError::InvalidId(error.to_string()))?;
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

        impl fmt::Display for $name {
            fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
                formatter.write_str(&self.0)
            }
        }

        impl FromStr for $name {
            type Err = TaskDataError;

            fn from_str(value: &str) -> Result<Self, Self::Err> {
                Self::parse(value)
            }
        }
    };
}

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum TaskDataError {
    #[error("invalid task id: {0}")]
    InvalidId(String),
    #[error("invalid task session status: {0}")]
    InvalidStatus(String),
}

string_id!(TaskSessionId, "ts_");
string_id!(ChildCommandId, "cc_");
pub type TaskCommandId = ChildCommandId;
string_id!(ChildDecisionId, "cd_");
pub type TaskDecisionId = ChildDecisionId;

/// Opaque Linear identifier: non-empty, provider-assigned (no prefix grammar of
/// our own, so distinct from `string_id!`).
macro_rules! validated_string_id {
    ($name:ident, $label:literal) => {
        #[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
        #[serde(transparent)]
        pub struct $name(String);

        impl $name {
            pub fn new(value: impl Into<String>) -> Result<Self, TaskDataError> {
                let value = value.into();
                if value.trim().is_empty() {
                    return Err(TaskDataError::InvalidId(format!(
                        "{} cannot be empty",
                        $label
                    )));
                }
                Ok(Self(value))
            }

            pub fn as_str(&self) -> &str {
                &self.0
            }

            pub(crate) fn from_raw(value: impl Into<String>) -> Self {
                Self(value.into())
            }
        }
    };
}

validated_string_id!(LinearIssueId, "Linear issue id");
validated_string_id!(LinearProjectId, "Linear project id");

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LinearIssueRef {
    pub id: LinearIssueId,
    pub identifier: String,
    pub title: String,
    pub description: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LinearProjectRef {
    pub id: LinearProjectId,
    pub slug: String,
    pub name: String,
    /// Definition and proof-shaped KRs captured when the Task Session starts.
    pub context: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
#[non_exhaustive]
pub enum TaskSessionStatus {
    Created,
    Starting,
    Running,
    Waiting,
    Submitted,
    Blocked,
    Failed,
    Merged,
    Abandoned,
}

impl TaskSessionStatus {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Created => "created",
            Self::Starting => "starting",
            Self::Running => "running",
            Self::Waiting => "waiting",
            Self::Submitted => "submitted",
            Self::Blocked => "blocked",
            Self::Failed => "failed",
            Self::Merged => "merged",
            Self::Abandoned => "abandoned",
        }
    }

    pub fn is_terminal(self) -> bool {
        matches!(self, Self::Merged | Self::Abandoned)
    }

    pub fn is_process_active(self) -> bool {
        matches!(self, Self::Starting | Self::Running)
    }
}

impl FromStr for TaskSessionStatus {
    type Err = TaskDataError;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        match value {
            "created" => Ok(Self::Created),
            "starting" => Ok(Self::Starting),
            "running" => Ok(Self::Running),
            "waiting" => Ok(Self::Waiting),
            "submitted" => Ok(Self::Submitted),
            "blocked" => Ok(Self::Blocked),
            "failed" => Ok(Self::Failed),
            "merged" => Ok(Self::Merged),
            "abandoned" => Ok(Self::Abandoned),
            _ => Err(TaskDataError::InvalidStatus(value.to_string())),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TaskProcess {
    pub generation: u32,
    pub pid: Option<u32>,
    pub tmux_name: String,
    pub started_at: OffsetDateTime,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PullRequestRef {
    pub number: u32,
    pub url: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PmWritebackOperation {
    CompleteTask,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "state", rename_all = "snake_case")]
pub enum PmWritebackState {
    Current,
    Pending {
        operation: PmWritebackOperation,
        error: String,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TaskSession {
    pub id: TaskSessionId,
    pub issue: LinearIssueRef,
    pub project: LinearProjectRef,
    pub pm_snapshot_synced_at: i64,
    pub pm_snapshot_warning: Option<String>,
    pub pm_writeback: PmWritebackState,
    pub wave_id: LfdId,
    pub wave: String,
    pub supervisor: SessionSupervisor,
    pub status: TaskSessionStatus,
    pub status_reason: String,
    pub status_at: OffsetDateTime,
    pub worktree: PathBuf,
    pub branch: String,
    pub base_commit: String,
    /// Provider/model selection captured when the session is reserved.
    pub agent: String,
    pub provider: String,
    pub provider_session_id: Option<String>,
    pub process: Option<TaskProcess>,
    pub pull_request: Option<PullRequestRef>,
    pub created_at: OffsetDateTime,
    pub updated_at: OffsetDateTime,
}

impl TaskSession {
    pub fn set_status(&mut self, status: TaskSessionStatus, reason: impl Into<String>) {
        let now = OffsetDateTime::now_utc();
        self.status = status;
        self.status_reason = reason.into();
        self.status_at = now;
        self.updated_at = now;
    }

    pub fn begin_generation(&mut self, tmux_name: String) -> u32 {
        let generation = self
            .process
            .as_ref()
            .map_or(1, |process| process.generation + 1);
        let now = OffsetDateTime::now_utc();
        self.process = Some(TaskProcess {
            generation,
            pid: None,
            tmux_name,
            started_at: now,
        });
        self.set_status(TaskSessionStatus::Starting, "task process is starting");
        generation
    }
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
        decision_id: TaskDecisionId,
        choice: String,
        message: Option<String>,
    },
    Abandon {
        reason: String,
    },
}
pub type TaskCommandKind = ChildCommandKind;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
#[non_exhaustive]
pub enum TaskCommandState {
    Persisted,
    Claimed,
    Accepted,
    Failed,
    Superseded,
}

impl TaskCommandState {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Persisted => "persisted",
            Self::Claimed => "claimed",
            Self::Accepted => "accepted",
            Self::Failed => "failed",
            Self::Superseded => "superseded",
        }
    }

    pub fn is_terminal(self) -> bool {
        matches!(self, Self::Accepted | Self::Failed | Self::Superseded)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
#[non_exhaustive]
pub enum TaskCommandEffect {
    LiveSteer,
    NextTurn,
    Replacement,
    Decision,
}

impl TaskCommandEffect {
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
    Wave(LfdId),
    Project(crate::project_session::ProjectSessionId),
    Human,
    Attachment,
    System,
}
pub type TaskCommandSource = ChildCommandSource;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TaskCommand {
    pub id: TaskCommandId,
    pub session_id: TaskSessionId,
    pub source: TaskCommandSource,
    pub kind: TaskCommandKind,
    pub state: TaskCommandState,
    pub effect: Option<TaskCommandEffect>,
    pub created_at: OffsetDateTime,
    pub claimed_by_generation: Option<u32>,
    pub accepted_at: Option<OffsetDateTime>,
    pub error: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BoundaryResult {
    Commands(Vec<TaskCommand>),
    Stopped(Box<TaskSession>),
}

impl TaskCommand {
    pub fn new(
        session_id: TaskSessionId,
        source: TaskCommandSource,
        kind: TaskCommandKind,
    ) -> Self {
        let effect = match &kind {
            TaskCommandKind::FollowUp { .. } | TaskCommandKind::Resume { message: Some(_) } => {
                Some(TaskCommandEffect::NextTurn)
            }
            TaskCommandKind::Interrupt {
                replacement: Some(_),
            } => Some(TaskCommandEffect::Replacement),
            TaskCommandKind::Decide { .. } => Some(TaskCommandEffect::Decision),
            TaskCommandKind::Steer { .. }
            | TaskCommandKind::Interrupt { replacement: None }
            | TaskCommandKind::Resume { message: None }
            | TaskCommandKind::Abandon { .. } => None,
        };
        Self {
            id: TaskCommandId::new(),
            session_id,
            source,
            kind,
            state: TaskCommandState::Persisted,
            effect,
            created_at: OffsetDateTime::now_utc(),
            claimed_by_generation: None,
            accepted_at: None,
            error: None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum TaskEventKind {
    Started,
    StatusChanged {
        from: TaskSessionStatus,
        to: TaskSessionStatus,
        reason: String,
    },
    CommandChanged {
        command_id: TaskCommandId,
        state: TaskCommandState,
        effect: Option<TaskCommandEffect>,
        error: Option<String>,
    },
    DecisionRequested {
        decision_id: TaskDecisionId,
        prompt: String,
        options: Vec<String>,
    },
    DecisionResolved {
        decision_id: TaskDecisionId,
        choice: String,
        message: Option<String>,
    },
    Progress {
        summary: String,
    },
    PullRequestOpened {
        number: u32,
        url: String,
    },
    Completed {
        pull_request: PullRequestRef,
        summary: String,
    },
    Failed {
        error: String,
        resumable: bool,
    },
}

impl TaskEventKind {
    pub fn is_wave_observable(&self) -> bool {
        !matches!(self, Self::Started | Self::Progress { .. })
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TaskEvent {
    pub id: i64,
    pub session_id: TaskSessionId,
    pub kind: TaskEventKind,
    pub created_at: OffsetDateTime,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TaskObservation {
    pub session_id: TaskSessionId,
    pub issue_identifier: String,
    pub event_id: i64,
    pub event: TaskEventKind,
}

impl TaskObservation {
    pub fn inbox_id(&self) -> String {
        format!("task-{}-{}", self.session_id, self.event_id)
    }

    pub fn prompt(&self) -> String {
        let event = serde_json::to_string(&self.event)
            .expect("TaskEventKind always serializes to structured JSON");
        format!(
            "<task_observation session_id=\"{}\" issue=\"{}\" event_id=\"{}\">\n{}\n</task_observation>",
            self.session_id, self.issue_identifier, self.event_id, event
        )
    }
}

#[cfg(test)]
mod tests {
    use super::{
        PmWritebackOperation, PmWritebackState, TaskCommandId, TaskDecisionId, TaskObservation,
        TaskSessionId, TaskSessionStatus,
    };

    #[test]
    fn task_ids_are_prefixed_and_round_trip() {
        let session = TaskSessionId::new();
        let command = TaskCommandId::new();
        let decision = TaskDecisionId::new();

        assert_eq!(TaskSessionId::parse(session.as_str()).unwrap(), session);
        assert_eq!(TaskCommandId::parse(command.as_str()).unwrap(), command);
        assert_eq!(TaskDecisionId::parse(decision.as_str()).unwrap(), decision);
    }

    #[test]
    fn task_observation_has_a_stable_structured_inbox_identity() {
        let observation = TaskObservation {
            session_id: TaskSessionId::from_raw("ts_example"),
            issue_identifier: "INF-123".to_string(),
            event_id: 42,
            event: super::TaskEventKind::Failed {
                error: "provider stopped".to_string(),
                resumable: true,
            },
        };

        assert_eq!(observation.inbox_id(), "task-ts_example-42");
        assert!(observation.prompt().contains("<task_observation"));
        assert!(observation.prompt().contains("\"kind\":\"failed\""));
    }

    #[test]
    fn only_merged_and_abandoned_are_terminal() {
        assert!(TaskSessionStatus::Merged.is_terminal());
        assert!(TaskSessionStatus::Abandoned.is_terminal());
        assert!(!TaskSessionStatus::Submitted.is_terminal());
        assert!(!TaskSessionStatus::Failed.is_terminal());
    }

    #[test]
    fn pending_pm_writeback_has_a_stable_json_shape() {
        let state = PmWritebackState::Pending {
            operation: PmWritebackOperation::CompleteTask,
            error: "offline".to_string(),
        };

        assert_eq!(
            serde_json::to_value(state).unwrap(),
            serde_json::json!({
                "state": "pending",
                "operation": "complete_task",
                "error": "offline"
            })
        );
    }
}
