//! Durable delivery of one Linear Task.
//!
//! A Task Session is the sole domain owner of its immutable worktree, branch,
//! provider transcript, one PR to main, and review-through-merge lifecycle.
//! Process generations may stop and resume without changing Task identity.

use std::path::PathBuf;
use std::str::FromStr;

use serde::{Deserialize, Serialize};
use time::OffsetDateTime;

use crate::child_session::{
    prefixed_uuid_id, ChildCommandEffect, ChildCommandId, ChildCommandState, ChildDecisionId,
    ChildDirectiveId, ChildProcessGeneration, DirectiveKind, SessionSupervisor,
};
use crate::lfd::id::LfdId;
use crate::session_context::{LinearIssueSnapshot, LinearProjectSnapshot};

pub mod runner;

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum TaskDataError {
    #[error("invalid task id: {0}")]
    InvalidId(String),
    #[error("invalid task session status: {0}")]
    InvalidStatus(String),
}

prefixed_uuid_id!(
    TaskSessionId,
    "ts_",
    TaskDataError,
    TaskDataError::InvalidId
);

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
    pub issue: LinearIssueSnapshot,
    pub project: LinearProjectSnapshot,
    pub pm_snapshot_synced_at: i64,
    pub pm_writeback: PmWritebackState,
    pub wave_id: LfdId,
    #[serde(rename = "wave")]
    pub wave_name: String,
    pub supervisor: SessionSupervisor,
    pub current_directive_version: u32,
    pub incorporated_directive_version: u32,
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
    /// Latest launch generation, retained after that process exits.
    #[serde(rename = "process")]
    pub latest_process: Option<ChildProcessGeneration>,
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
        self.set_status(TaskSessionStatus::Starting, "task process is starting");
        generation
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
        match self {
            Self::Started | Self::Progress { .. } => false,
            Self::CommandChanged { state, .. } => state.is_terminal(),
            _ => true,
        }
    }

    /// Whether a Project-supervised Task event also belongs in the root Wave.
    /// Routine decisions stay at the immediate Project boundary; a Project
    /// escalates by emitting its own `DecisionRequested` event.
    pub fn is_root_wave_observable(&self) -> bool {
        self.is_wave_observable()
            && !matches!(
                self,
                Self::DecisionRequested { .. } | Self::DecisionResolved { .. }
            )
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
        ChildCommandId, ChildCommandState, PmWritebackOperation, PmWritebackState, TaskEventKind,
        TaskObservation, TaskSessionId, TaskSessionStatus,
    };
    use crate::child_session::ChildDecisionId;

    #[test]
    fn task_ids_are_prefixed_and_round_trip() {
        let session = TaskSessionId::new();
        assert_eq!(TaskSessionId::parse(session.as_str()).unwrap(), session);
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
    fn wave_observes_command_outcomes_not_transport_chatter() {
        let event = |state| TaskEventKind::CommandChanged {
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

    #[test]
    fn project_supervision_keeps_routine_task_decisions_out_of_the_root_wave() {
        let requested = TaskEventKind::DecisionRequested {
            decision_id: ChildDecisionId::new(),
            prompt: "Which parser shape?".to_string(),
            options: vec!["strict".to_string(), "permissive".to_string()],
        };
        let resolved = TaskEventKind::DecisionResolved {
            decision_id: ChildDecisionId::new(),
            choice: "strict".to_string(),
            message: None,
        };

        assert!(requested.is_wave_observable());
        assert!(resolved.is_wave_observable());
        assert!(!requested.is_root_wave_observable());
        assert!(!resolved.is_root_wave_observable());
        assert!(TaskEventKind::Failed {
            error: "provider stopped".to_string(),
            resumable: true,
        }
        .is_root_wave_observable());
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
