use std::fmt;
use std::path::PathBuf;
use std::str::FromStr;

use serde::{Deserialize, Serialize};
use time::OffsetDateTime;
use uuid::Uuid;

use crate::lfd::id::LfdId;

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
                Ok(Self(value.to_string()))
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
string_id!(TaskCommandId, "tc_");

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct LinearIssueId(String);

impl LinearIssueId {
    pub fn new(value: impl Into<String>) -> Result<Self, TaskDataError> {
        let value = value.into();
        if value.trim().is_empty() {
            return Err(TaskDataError::InvalidId(
                "Linear issue id cannot be empty".to_string(),
            ));
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

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct LinearProjectId(String);

impl LinearProjectId {
    pub fn new(value: impl Into<String>) -> Result<Self, TaskDataError> {
        let value = value.into();
        if value.trim().is_empty() {
            return Err(TaskDataError::InvalidId(
                "Linear project id cannot be empty".to_string(),
            ));
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

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TaskSession {
    pub id: TaskSessionId,
    pub issue: LinearIssueRef,
    pub project: LinearProjectRef,
    pub wave_id: LfdId,
    pub wave: String,
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
pub enum TaskCommandKind {
    Message { text: String },
    Interrupt { next_message: Option<String> },
    Resume { message: Option<String> },
    Abandon { reason: String },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", content = "wave_id", rename_all = "snake_case")]
pub enum TaskCommandSource {
    Wave(LfdId),
    Human,
    Attachment,
    System,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TaskCommand {
    pub id: TaskCommandId,
    pub session_id: TaskSessionId,
    pub source: TaskCommandSource,
    pub kind: TaskCommandKind,
    pub created_at: OffsetDateTime,
    pub claimed_by_generation: Option<u32>,
    pub acknowledged_at: Option<OffsetDateTime>,
}

impl TaskCommand {
    pub fn new(
        session_id: TaskSessionId,
        source: TaskCommandSource,
        kind: TaskCommandKind,
    ) -> Self {
        Self {
            id: TaskCommandId::new(),
            session_id,
            source,
            kind,
            created_at: OffsetDateTime::now_utc(),
            claimed_by_generation: None,
            acknowledged_at: None,
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
    CommandAccepted {
        command_id: TaskCommandId,
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

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TaskEvent {
    pub id: i64,
    pub session_id: TaskSessionId,
    pub kind: TaskEventKind,
    pub created_at: OffsetDateTime,
}

#[cfg(test)]
mod tests {
    use super::{TaskCommandId, TaskSessionId, TaskSessionStatus};

    #[test]
    fn task_ids_are_prefixed_and_round_trip() {
        let session = TaskSessionId::new();
        let command = TaskCommandId::new();

        assert_eq!(TaskSessionId::parse(session.as_str()).unwrap(), session);
        assert_eq!(TaskCommandId::parse(command.as_str()).unwrap(), command);
    }

    #[test]
    fn only_merged_and_abandoned_are_terminal() {
        assert!(TaskSessionStatus::Merged.is_terminal());
        assert!(TaskSessionStatus::Abandoned.is_terminal());
        assert!(!TaskSessionStatus::Submitted.is_terminal());
        assert!(!TaskSessionStatus::Failed.is_terminal());
    }
}
