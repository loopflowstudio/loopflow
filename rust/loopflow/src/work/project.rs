//! Durable tracking for one Linear Project's KRs.
//!
//! A Project coordinates Tasks from the owning Wave's clean
//! control checkout. It owns no worktree, shipping branch, PR, permanent
//! memory, cadence, human chat, or controller state.
use serde::{Deserialize, Serialize};
use time::OffsetDateTime;

use crate::child::{AbandonIntent, ChildRef, ObservationRecipient};
pub use crate::durable::ProjectId;
use crate::id::WaveId;
use crate::planning::ProjectPlan;
use crate::work::task::{TaskEventKind, TaskId};

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
    /// Set when abandonment is *requested*, not when it is applied. No launch
    /// path may start a Run for Project Work carrying this.
    pub abandon_intent: Option<AbandonIntent>,
    pub created_at: OffsetDateTime,
    pub updated_at: OffsetDateTime,
}

impl Project {
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
    pub created_at: OffsetDateTime,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct HistoricalFailure {
    pub message: String,
    #[serde(with = "time::serde::rfc3339")]
    pub occurred_at: OffsetDateTime,
}

impl HistoricalFailure {
    pub(crate) fn from_event(event: &ProjectEvent) -> Option<Self> {
        Some(Self {
            message: event.kind.failure_reason()?.to_string(),
            occurred_at: event.created_at,
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
