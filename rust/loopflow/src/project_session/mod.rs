//! Durable pursuit of one Linear Project's KRs.
//!
//! A Project Session coordinates Task Sessions from the owning Wave's clean
//! control checkout. It owns no worktree, shipping branch, PR, permanent
//! memory, cadence, or human chat. Waiting persists without a process; child
//! observations wake the same provider transcript when judgment is useful.

use std::str::FromStr;

use serde::{Deserialize, Serialize};
use time::OffsetDateTime;

use crate::child_session::{
    prefixed_uuid_id, ChildCommandEffect, ChildCommandId, ChildCommandState, ChildDecisionId,
    ChildDirectiveId, ChildProcessGeneration, ChildRef, DirectiveKind, ObservationRecipient,
};
use crate::id::WaveId;
use crate::session_context::ProjectLaunchReceipt;
use crate::task::{TaskEventKind, TaskSessionId};

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

prefixed_uuid_id!(
    ProjectSessionId,
    "ps_",
    ProjectDataError,
    ProjectDataError::InvalidId
);

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
pub struct ProjectSession {
    pub id: ProjectSessionId,
    /// Immutable PM evidence from before the first Project turn.
    pub launch: ProjectLaunchReceipt,
    /// Current ownership. Wave name and checkout are resolved from this id.
    pub wave_id: WaveId,
    pub current_directive_version: u32,
    pub incorporated_directive_version: u32,
    pub status: ProjectSessionStatus,
    pub status_reason: String,
    pub status_at: OffsetDateTime,
    pub iteration: u32,
    pub observation_cursor: i64,
    pub last_state_fingerprint: Option<String>,
    pub agent: String,
    pub provider: String,
    pub provider_session_id: Option<String>,
    /// Latest launch generation, retained after that process exits.
    pub latest_process: Option<ChildProcessGeneration>,
    pub created_at: OffsetDateTime,
    pub updated_at: OffsetDateTime,
}

impl ProjectSession {
    pub fn validate(&self) -> Result<(), ProjectDataError> {
        if self.status.is_process_active() && self.latest_process.is_none() {
            return Err(ProjectDataError::InvalidInvariant(format!(
                "{} requires a latest process generation",
                self.status.as_str()
            )));
        }
        if self.incorporated_directive_version > self.current_directive_version {
            return Err(ProjectDataError::InvalidInvariant(
                "incorporated directive version exceeds current direction".to_string(),
            ));
        }
        Ok(())
    }

    pub fn set_status(&mut self, status: ProjectSessionStatus, reason: impl Into<String>) {
        let now = OffsetDateTime::now_utc();
        self.status = status;
        self.status_reason = reason.into();
        self.status_at = now;
        self.updated_at = now;
    }

    pub fn begin_generation(&mut self, tmux_name: String) -> u32 {
        let generation = self
            .latest_process
            .as_ref()
            .map_or(1, |process| process.generation + 1);
        let now = OffsetDateTime::now_utc();
        self.latest_process = Some(ChildProcessGeneration {
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
    pub control_source: Option<crate::child_session::ChildCommandSource>,
    pub event: ProjectEventKind,
}

impl ProjectObservation {
    pub fn inbox_id(&self) -> String {
        format!("project-{}-{}", self.session_id, self.event_id)
    }

    pub fn prompt(&self) -> String {
        let payload = serde_json::to_string(&serde_json::json!({
            "control_source": self.control_source,
            "event": self.event,
        }))
        .expect("Project observation always serializes to structured JSON");
        format!(
            "<project_observation session_id=\"{}\" project=\"{}\" event_id=\"{}\">\n{}\n</project_observation>",
            self.session_id, self.project, self.event_id, payload
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
    use super::{
        ChildCommandId, ProjectEventKind, ProjectSession, ProjectSessionId, ProjectSessionStatus,
    };
    use crate::child_session::ChildCommandState;
    use crate::session_context::{LinearProjectId, LinearProjectSnapshot, ProjectLaunchReceipt};

    fn project_session() -> ProjectSession {
        let now = time::OffsetDateTime::now_utc();
        ProjectSession {
            id: ProjectSessionId::new(),
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
            current_directive_version: 1,
            incorporated_directive_version: 0,
            status: ProjectSessionStatus::Created,
            status_reason: "created".to_string(),
            status_at: now,
            iteration: 0,
            observation_cursor: 0,
            last_state_fingerprint: None,
            agent: "codex".to_string(),
            provider: "codex".to_string(),
            provider_session_id: None,
            latest_process: None,
            created_at: now,
            updated_at: now,
        }
    }

    #[test]
    fn project_ids_are_prefixed_and_round_trip() {
        let session = ProjectSessionId::new();
        assert_eq!(ProjectSessionId::parse(session.as_str()).unwrap(), session);
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
        assert!(!event(ChildCommandState::Delivering).is_wave_observable());
        assert!(event(ChildCommandState::Accepted).is_wave_observable());
        assert!(event(ChildCommandState::Failed).is_wave_observable());
        assert!(event(ChildCommandState::Uncertain).is_wave_observable());
    }

    #[test]
    fn project_session_rejects_impossible_process_and_directive_state() {
        let mut session = project_session();
        session.status = ProjectSessionStatus::Running;
        assert!(session.validate().is_err());

        session.status = ProjectSessionStatus::Waiting;
        session.incorporated_directive_version = 2;
        assert!(session.validate().is_err());
    }
}
