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
    prefixed_uuid_id, AbandonIntent, ChildCommandEffect, ChildCommandId, ChildCommandState,
    ChildLeaseState, ChildProcessGeneration, ChildRef, ObservationRecipient,
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

    /// Coarsen durable intent to the shared shape the body projection reads, so
    /// one `observe` serves Project and Task Sessions alike.
    pub fn body_intent(self) -> crate::child_session::BodyIntent {
        use crate::child_session::BodyIntent;
        match self {
            Self::Created | Self::Starting | Self::Running => BodyIntent::Active,
            Self::Waiting => BodyIntent::Waiting,
            Self::Blocked => BodyIntent::Blocked,
            Self::Failed => BodyIntent::Failed,
            Self::Completed | Self::Abandoned => BodyIntent::Terminal,
        }
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
    pub status: ProjectSessionStatus,
    pub status_reason: String,
    pub status_at: OffsetDateTime,
    pub iteration: u32,
    pub observation_cursor: i64,
    pub last_state_fingerprint: Option<String>,
    /// Provider/model selection for the next body generation. This is mutable
    /// lease state, not Project Session identity.
    pub agent: String,
    /// Harness family for the next/current body generation.
    pub provider: String,
    /// Transcript handle reusable only by a compatible provider generation.
    pub provider_session_id: Option<String>,
    /// Latest launch generation, retained after that process exits. Its
    /// [`crate::child_session::BinaryProvenance`] is the audit record of which
    /// lf launched it — a Session no longer pins a binary of its own.
    pub latest_process: Option<ChildProcessGeneration>,
    /// Set when abandonment is *requested*, not when it is applied. No launch
    /// path may start a process for a Session carrying this.
    pub abandon_intent: Option<AbandonIntent>,
    pub created_at: OffsetDateTime,
    pub updated_at: OffsetDateTime,
}

impl ProjectSession {
    /// Why a supervisor must not start another process generation, if it must not.
    /// A Project has no PR, so PR review does not apply — see
    /// [`crate::task::TaskSession::supervisor_restart_bar`].
    pub fn supervisor_restart_bar(&self) -> Option<String> {
        if self.status.is_terminal() {
            return Some(format!(
                "Project {} is {}; terminal Project Sessions do not restart",
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
        if self.status.is_process_active() && self.latest_process.is_none() {
            return Err(ProjectDataError::InvalidInvariant(format!(
                "{} requires a latest process generation",
                self.status.as_str()
            )));
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
            process_group_id: None,
            tmux_name,
            agent: self.agent.clone(),
            provider: self.provider.clone(),
            provider_session_id: self.provider_session_id.clone(),
            started_at: now,
            state: ChildLeaseState::Reserved,
            outcome: None,
            provenance: None,
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
    BodyHandedOff {
        handoff: crate::child_session::ChildBodyHandoff,
    },
    BodyLeaseChanged {
        process: ChildProcessGeneration,
    },
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
    TaskObserved {
        task_session_id: TaskSessionId,
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
        match self {
            Self::Started | Self::TaskObserved { .. } => false,
            Self::BodyLeaseChanged { process } => matches!(
                process.state,
                ChildLeaseState::Revoked | ChildLeaseState::Finished
            ),
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
            abandon_intent: None,
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
    fn project_session_rejects_impossible_process_state() {
        let mut session = project_session();
        session.status = ProjectSessionStatus::Running;
        assert!(session.validate().is_err());
    }
}
