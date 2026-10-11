//! Durable tracking for one Project's KRs.
//!
//! A Wave selects its current Project; saved planning facts include status and
//! workflow. A Project owns no worktree, shipping branch, PR, permanent
//! memory, cadence, chat, or controller state.
use serde::{Deserialize, Serialize};
use time::OffsetDateTime;

use crate::child::AbandonIntent;
pub use crate::durable::ProjectId;
use crate::id::WaveId;
use crate::planning::{LinearProjectId, PlanningError};
use crate::work::task::{TaskEventKind, TaskId};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProjectKR {
    pub text: String,
    pub holds: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Project {
    pub id: ProjectId,
    pub summary: String,
    pub slug: String,
    pub name: String,
    pub prompt_context: String,
    pub workflow: String,
    pub status: crate::pm::ProjectStatus,
    /// Optional Linear binding, not a second owner of this record.
    pub linear_id: Option<LinearProjectId>,
    /// Acquisition time of the retained Linear observation, never a write clock.
    pub pm_snapshot_synced_at: Option<i64>,
    /// Current ownership. Wave name and checkout are resolved from this id.
    pub wave_id: WaveId,
    /// Set when abandonment is *requested*, not when it is applied. No launch
    /// path may start a Run for Project Work carrying this.
    pub abandon_intent: Option<AbandonIntent>,
    pub created_at: OffsetDateTime,
    pub updated_at: OffsetDateTime,
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
    /// A durable steer: direction handed to the Project. Folded into the run's
    /// seed and injected into a live turn; not a report out, so it never
    /// crosses to the parent Wave.
    Steer {
        author: crate::durable::Author,
        text: String,
    },
    /// A request to end the Project's current turn so the next re-reads its
    /// direction immediately. Acted on only by a live run, and not a report out.
    Interrupt,
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
        !matches!(
            self,
            Self::Started | Self::TaskObserved { .. } | Self::Steer { .. } | Self::Interrupt
        )
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

impl Project {
    pub fn linear_id(&self) -> Result<&LinearProjectId, PlanningError> {
        self.linear_id
            .as_ref()
            .ok_or(PlanningError::NoLinearMapping)
    }
}
