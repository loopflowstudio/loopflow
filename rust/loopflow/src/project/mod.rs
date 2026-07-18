//! Durable pursuit of one Linear Project's KRs.
//!
//! A Project coordinates Tasks from the owning Wave's clean
//! control checkout. It owns no worktree, shipping branch, PR, permanent
//! memory, cadence, or human chat. Waiting persists without a process; child
//! observations wake the same provider transcript when judgment is useful.

use std::str::FromStr;

use serde::{Deserialize, Serialize};
use time::OffsetDateTime;

use crate::child::{AbandonIntent, ChildRef, ObservationRecipient};
pub use crate::durable::ProjectId;
use crate::id::WaveId;
use crate::session_context::ProjectLaunchReceipt;
use crate::task::{TaskEventKind, TaskId};

pub mod runner;

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum ProjectDataError {
    #[error("invalid project-session id: {0}")]
    InvalidId(String),
    #[error("invalid project-session status: {0}")]
    InvalidStatus(String),
    #[error("invalid project session: {0}")]
    InvalidInvariant(String),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
#[non_exhaustive]
pub enum ProjectStatus {
    Created,
    Starting,
    Running,
    Waiting,
    Blocked,
    Failed,
    Completed,
    Abandoned,
}

impl ProjectStatus {
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

    /// Coarsen durable intent to the shared shape the body projection reads, so
    /// one `observe` serves Project and Tasks alike.
    pub fn body_intent(self) -> crate::child::BodyIntent {
        use crate::child::BodyIntent;
        match self {
            Self::Created | Self::Starting | Self::Running => BodyIntent::Active,
            Self::Waiting => BodyIntent::Waiting,
            Self::Blocked => BodyIntent::Blocked,
            Self::Failed => BodyIntent::Failed,
            Self::Completed | Self::Abandoned => BodyIntent::Terminal,
        }
    }
}

impl FromStr for ProjectStatus {
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
pub struct Project {
    pub id: ProjectId,
    /// Immutable PM evidence from before the first Project turn.
    pub launch: ProjectLaunchReceipt,
    /// Current ownership. Wave name and checkout are resolved from this id.
    pub wave_id: WaveId,
    pub status: ProjectStatus,
    pub status_reason: String,
    pub status_at: OffsetDateTime,
    pub iteration: u32,
    pub observation_cursor: i64,
    pub last_state_fingerprint: Option<String>,
    /// Provider/model selection for the next body generation. This is mutable
    /// lease state, not Project identity.
    pub agent: String,
    /// Harness family for the next/current body generation.
    pub provider: String,
    /// Transcript handle reusable only by a compatible provider generation.
    pub provider_session_id: Option<String>,
    /// Set when abandonment is *requested*, not when it is applied. No launch
    /// path may start a process for a Session carrying this.
    pub abandon_intent: Option<AbandonIntent>,
    pub created_at: OffsetDateTime,
    pub updated_at: OffsetDateTime,
}

impl Project {
    /// Why a supervisor must not start another process generation, if it must not.
    /// A Project has no PR, so PR review does not apply — see
    /// [`crate::task::Task::supervisor_restart_bar`].
    pub fn supervisor_restart_bar(&self) -> Option<String> {
        if self.status.is_terminal() {
            return Some(format!(
                "Project {} is {}; terminal Projects do not restart",
                self.launch.project.slug,
                self.status.as_str()
            ));
        }
        self.abandon_intent.as_ref().map(|intent| {
            format!(
                "Project {} is being abandoned: {}",
                self.launch.project.slug, intent.reason
            )
        })
    }

    pub fn validate(&self) -> Result<(), ProjectDataError> {
        Ok(())
    }

    pub fn set_status(&mut self, status: ProjectStatus, reason: impl Into<String>) {
        let now = OffsetDateTime::now_utc();
        self.status = status;
        self.status_reason = reason.into();
        self.status_at = now;
        self.updated_at = now;
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum ProjectEventKind {
    Started,
    BodyHandedOff {
        handoff: crate::child::ChildBodyHandoff,
    },
    StatusChanged {
        from: ProjectStatus,
        to: ProjectStatus,
        reason: String,
    },
    TaskObserved {
        task_id: TaskId,
        task_event_id: i64,
        event: Box<TaskEventKind>,
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
        !matches!(self, Self::Started | Self::TaskObserved { .. })
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProjectEvent {
    pub id: i64,
    pub project_id: ProjectId,
    pub kind: ProjectEventKind,
    pub created_at: OffsetDateTime,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProjectObservation {
    pub project_id: ProjectId,
    pub project: String,
    pub event_id: i64,
    pub event: ProjectEventKind,
}

impl ProjectObservation {
    pub fn inbox_id(&self) -> String {
        format!("project-{}-{}", self.project_id, self.event_id)
    }

    pub fn prompt(&self) -> String {
        let payload = serde_json::to_string(&self.event)
            .expect("Project observation always serializes to structured JSON");
        format!(
            "<project_observation project_id=\"{}\" project=\"{}\" event_id=\"{}\">\n{}\n</project_observation>",
            self.project_id, self.project, self.event_id, payload
        )
    }
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
    pub recipient: ObservationRecipient,
    pub source: ChildRef,
    pub event_id: i64,
    pub payload: ChildEventPayload,
    pub delivered_at: Option<OffsetDateTime>,
}

#[cfg(test)]
mod tests {
    use super::{Project, ProjectId, ProjectStatus};
    use crate::session_context::{LinearProjectId, LinearProjectSnapshot, ProjectLaunchReceipt};

    fn project() -> Project {
        let now = time::OffsetDateTime::now_utc();
        Project {
            id: ProjectId::new(),
            launch: ProjectLaunchReceipt {
                project: LinearProjectSnapshot {
                    id: LinearProjectId::new("project-1").unwrap(),
                    slug: "runtime".to_string(),
                    name: "Runtime".to_string(),
                    prompt_context: "Definition".to_string(),
                },
                pm_snapshot_synced_at: 1,
            },
            wave_id: crate::id::WaveId::new(),
            status: ProjectStatus::Created,
            status_reason: "created".to_string(),
            status_at: now,
            iteration: 0,
            observation_cursor: 0,
            last_state_fingerprint: None,
            agent: "codex".to_string(),
            provider: "codex".to_string(),
            provider_session_id: None,
            abandon_intent: None,
            created_at: now,
            updated_at: now,
        }
    }

    #[test]
    fn project_ids_are_prefixed_and_round_trip() {
        let session = ProjectId::new();
        assert_eq!(ProjectId::parse(session.as_str()).unwrap(), session);
    }

    #[test]
    fn completed_and_abandoned_are_terminal() {
        assert!(ProjectStatus::Completed.is_terminal());
        assert!(ProjectStatus::Abandoned.is_terminal());
        assert!(!ProjectStatus::Waiting.is_terminal());
    }

    #[test]
    fn project_rejects_impossible_process_state() {
        let mut session = project();
        session.status = ProjectStatus::Running;
        assert!(session.validate().is_err());
    }
}
