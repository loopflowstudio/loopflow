use std::fmt;
use std::str::FromStr;

use serde::{Deserialize, Serialize};
use time::OffsetDateTime;
use uuid::Uuid;

use crate::lfd::id::LfdId;
use crate::task::{
    ChildCommandEffect, ChildCommandId, ChildCommandState, ChildDecisionId, ChildDirectiveId,
    DirectiveKind, LinearProjectRef, TaskEventKind, TaskSessionId,
};

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

            pub fn parse(value: &str) -> Result<Self, ProjectDataError> {
                let suffix = value.strip_prefix($prefix).ok_or_else(|| {
                    ProjectDataError::InvalidId(format!("expected {} id", $prefix))
                })?;
                Uuid::parse_str(suffix)
                    .map_err(|error| ProjectDataError::InvalidId(error.to_string()))?;
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
            type Err = ProjectDataError;

            fn from_str(value: &str) -> Result<Self, Self::Err> {
                Self::parse(value)
            }
        }
    };
}

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum ProjectDataError {
    #[error("invalid project-session id: {0}")]
    InvalidId(String),
    #[error("invalid project-session status: {0}")]
    InvalidStatus(String),
}

string_id!(ProjectSessionId, "ps_");

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum SessionSupervisor {
    Wave { wave_id: LfdId },
    Project { session_id: ProjectSessionId },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
#[non_exhaustive]
pub enum ProjectSessionStatus {
    Created,
    Starting,
    Running,
    Waiting,
    Blocked,
    Failed,
    Completed,
    Abandoned,
}

impl ProjectSessionStatus {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Created => "created",
            Self::Starting => "starting",
            Self::Running => "running",
            Self::Waiting => "waiting",
            Self::Blocked => "blocked",
            Self::Failed => "failed",
            Self::Completed => "completed",
            Self::Abandoned => "abandoned",
        }
    }

    pub fn is_terminal(self) -> bool {
        matches!(self, Self::Completed | Self::Abandoned)
    }

    pub fn is_process_active(self) -> bool {
        matches!(self, Self::Starting | Self::Running)
    }
}

impl FromStr for ProjectSessionStatus {
    type Err = ProjectDataError;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        match value {
            "created" => Ok(Self::Created),
            "starting" => Ok(Self::Starting),
            "running" => Ok(Self::Running),
            "waiting" => Ok(Self::Waiting),
            "blocked" => Ok(Self::Blocked),
            "failed" => Ok(Self::Failed),
            "completed" => Ok(Self::Completed),
            "abandoned" => Ok(Self::Abandoned),
            _ => Err(ProjectDataError::InvalidStatus(value.to_string())),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProjectProcess {
    pub generation: u32,
    pub pid: Option<u32>,
    pub tmux_name: String,
    pub started_at: OffsetDateTime,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProjectSession {
    pub id: ProjectSessionId,
    pub project: LinearProjectRef,
    pub wave_id: LfdId,
    pub wave: String,
    pub repo: String,
    pub pm_snapshot_synced_at: i64,
    pub current_directive_version: u32,
    pub incorporated_directive_version: u32,
    pub status: ProjectSessionStatus,
    pub status_reason: String,
    pub status_at: OffsetDateTime,
    pub iteration: u32,
    pub task_event_cursor: i64,
    pub state_fingerprint: Option<String>,
    pub agent: String,
    pub provider: String,
    pub provider_session_id: Option<String>,
    pub process: Option<ProjectProcess>,
    pub created_at: OffsetDateTime,
    pub updated_at: OffsetDateTime,
}

impl ProjectSession {
    pub fn set_status(&mut self, status: ProjectSessionStatus, reason: impl Into<String>) {
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
        self.process = Some(ProjectProcess {
            generation,
            pid: None,
            tmux_name,
            started_at: now,
        });
        self.set_status(
            ProjectSessionStatus::Starting,
            "project process is starting",
        );
        generation
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum ProjectEventKind {
    Started,
    StatusChanged {
        from: ProjectSessionStatus,
        to: ProjectSessionStatus,
        reason: String,
    },
    CommandChanged {
        command_id: ChildCommandId,
        state: ChildCommandState,
        effect: Option<ChildCommandEffect>,
        error: Option<String>,
    },
    DirectiveChanged {
        directive_id: ChildDirectiveId,
        version: u32,
        directive_kind: DirectiveKind,
    },
    DirectiveIncorporated {
        directive_id: ChildDirectiveId,
        version: u32,
        summary: String,
    },
    TaskObserved {
        task_session_id: TaskSessionId,
        task_event_id: i64,
        event: Box<TaskEventKind>,
    },
    DecisionRequested {
        decision_id: ChildDecisionId,
        prompt: String,
        options: Vec<String>,
    },
    DecisionResolved {
        decision_id: ChildDecisionId,
        choice: String,
        message: Option<String>,
    },
    IterationCompleted {
        iteration: u32,
        summary: String,
    },
    Completed {
        summary: String,
    },
    Failed {
        error: String,
        resumable: bool,
    },
}

impl ProjectEventKind {
    pub fn is_wave_observable(&self) -> bool {
        match self {
            Self::Started | Self::TaskObserved { .. } => false,
            Self::CommandChanged { state, .. } => state.is_terminal(),
            _ => true,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProjectEvent {
    pub id: i64,
    pub session_id: ProjectSessionId,
    pub kind: ProjectEventKind,
    pub created_at: OffsetDateTime,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProjectObservation {
    pub session_id: ProjectSessionId,
    pub project: String,
    pub event_id: i64,
    pub event: ProjectEventKind,
}

impl ProjectObservation {
    pub fn inbox_id(&self) -> String {
        format!("project-{}-{}", self.session_id, self.event_id)
    }

    pub fn prompt(&self) -> String {
        let event = serde_json::to_string(&self.event)
            .expect("ProjectEventKind always serializes to structured JSON");
        format!(
            "<project_observation session_id=\"{}\" project=\"{}\" event_id=\"{}\">\n{}\n</project_observation>",
            self.session_id, self.project, self.event_id, event
        )
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum ChildSessionRef {
    Project { session_id: ProjectSessionId },
    Task { session_id: TaskSessionId },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum ChildEventPayload {
    Project { event: ProjectEventKind },
    Task { event: TaskEventKind },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ObservationOutboxRow {
    pub id: i64,
    pub supervisor: SessionSupervisor,
    pub source: ChildSessionRef,
    pub event_id: i64,
    pub payload: ChildEventPayload,
    pub delivered_at: Option<OffsetDateTime>,
}

#[cfg(test)]
mod tests {
    use super::{ChildCommandId, ProjectEventKind, ProjectSessionId, ProjectSessionStatus};
    use crate::task::ChildCommandState;

    #[test]
    fn project_ids_are_prefixed_and_round_trip() {
        let session = ProjectSessionId::new();
        let command = ChildCommandId::new();
        assert_eq!(ProjectSessionId::parse(session.as_str()).unwrap(), session);
        assert_eq!(ChildCommandId::parse(command.as_str()).unwrap(), command);
    }

    #[test]
    fn completed_and_abandoned_are_terminal() {
        assert!(ProjectSessionStatus::Completed.is_terminal());
        assert!(ProjectSessionStatus::Abandoned.is_terminal());
        assert!(!ProjectSessionStatus::Waiting.is_terminal());
    }

    #[test]
    fn wave_observes_project_control_outcomes_not_transport_chatter() {
        let event = |state| ProjectEventKind::CommandChanged {
            command_id: ChildCommandId::new(),
            state,
            effect: None,
            error: None,
        };

        assert!(!event(ChildCommandState::Persisted).is_wave_observable());
        assert!(!event(ChildCommandState::Claimed).is_wave_observable());
        assert!(event(ChildCommandState::Accepted).is_wave_observable());
        assert!(event(ChildCommandState::Failed).is_wave_observable());
    }
}
