//! Durable pursuit of one Linear Project's KRs.
//!
//! A Project coordinates Tasks from the owning Wave's clean
//! control checkout. It owns no worktree, shipping branch, PR, permanent
//! memory, cadence, or human chat. Waiting persists without a process; child
//! observations wake the same provider transcript when judgment is useful.
use serde::{Deserialize, Serialize};
use time::OffsetDateTime;

use crate::child::{AbandonIntent, ChildRef, ObservationRecipient};
pub use crate::durable::ProjectId;
use crate::durable::RunId;
use crate::id::WaveId;
use crate::planning::ProjectPlan;
use crate::task::{TaskEventKind, TaskId};

pub mod runner;

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum ProjectDataError {
    #[error("invalid Project id: {0}")]
    InvalidId(String),
    #[error("invalid Project: {0}")]
    InvalidInvariant(String),
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Project {
    pub id: ProjectId,
    /// Current planning facts from the PM system.
    pub plan: ProjectPlan,
    /// Current ownership. Wave name and checkout are resolved from this id.
    pub wave_id: WaveId,
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
    /// path may start a Run for Project Work carrying this.
    pub abandon_intent: Option<AbandonIntent>,
    pub created_at: OffsetDateTime,
    pub updated_at: OffsetDateTime,
}

impl Project {
    pub fn supervisor_restart_bar(&self) -> Option<String> {
        self.abandon_intent.as_ref().map(|intent| {
            format!(
                "Project {} is being abandoned: {}",
                self.plan.slug, intent.reason
            )
        })
    }

    pub fn validate(&self) -> Result<(), ProjectDataError> {
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum ProjectEventKind {
    Started,
    BodyHandedOff {
        handoff: crate::child::ChildBodyHandoff,
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

    fn failure_reason(&self) -> Option<&str> {
        match self {
            Self::Failed { error, .. } => Some(error),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProjectEvent {
    pub id: i64,
    pub project_id: ProjectId,
    pub kind: ProjectEventKind,
    pub run_id: Option<RunId>,
    pub created_at: OffsetDateTime,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct HistoricalFailure {
    pub message: String,
    #[serde(with = "time::serde::rfc3339")]
    pub occurred_at: OffsetDateTime,
    pub run_id: Option<RunId>,
}

impl HistoricalFailure {
    pub(crate) fn from_event(event: &ProjectEvent) -> Option<Self> {
        Some(Self {
            message: event.kind.failure_reason()?.to_string(),
            occurred_at: event.created_at,
            run_id: event.run_id.clone(),
        })
    }
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
