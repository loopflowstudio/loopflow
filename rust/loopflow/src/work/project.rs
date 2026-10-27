//! Durable tracking for one Project's KRs.
//!
//! A Wave selects its current Project. A Project owns no worktree, shipping branch, PR, permanent
//! memory, cadence, chat, or controller state.
use serde::{Deserialize, Serialize};
use time::OffsetDateTime;

pub use crate::durable::ProjectId;
use crate::id::WaveId;
use crate::linear::{LinearBindingError, LinearProjectId};
use crate::work::task::{TaskEventKind, TaskId};

/// Shared Project status, independent of a provider’s display label.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ProjectStatus {
    Backlog,
    Planned,
    Started,
    Paused,
    Completed,
    Canceled,
}

impl ProjectStatus {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Backlog => "backlog",
            Self::Planned => "planned",
            Self::Started => "started",
            Self::Paused => "paused",
            Self::Completed => "completed",
            Self::Canceled => "canceled",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProjectKR {
    pub text: String,
    pub holds: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Project {
    pub id: ProjectId,
    pub summary: String,
    pub slug: String,
    pub name: String,
    pub description: String,
    pub krs: Vec<ProjectKR>,
    pub metric_targets: Vec<crate::work::wave::metrics::MetricTarget>,
    pub status: ProjectStatus,
    /// Optional Linear binding, not a second owner of this record.
    pub linear_id: Option<LinearProjectId>,
    /// Acquisition time of the retained Linear observation, never a write clock.
    pub pm_snapshot_synced_at: Option<i64>,
    /// Current ownership. Wave name and checkout are resolved from this id.
    pub wave_id: WaveId,
    #[serde(with = "time::serde::rfc3339")]
    pub created_at: OffsetDateTime,
    #[serde(with = "time::serde::rfc3339")]
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
    /// Historical authored direction; the immutable publication owns its text.
    Steer {
        steer_id: crate::id::SteerId,
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
    pub fn linear_id(&self) -> Result<&LinearProjectId, LinearBindingError> {
        self.linear_id
            .as_ref()
            .ok_or(LinearBindingError::NoLinearMapping)
    }
}

pub(crate) fn validate_metric_targets(
    targets: &[crate::work::wave::metrics::MetricTarget],
) -> Result<(), String> {
    let mut ids = std::collections::BTreeSet::new();
    for metric in targets {
        if metric.metric_id.trim().is_empty()
            || !metric.target.value().is_finite()
            || !ids.insert(&metric.metric_id)
        {
            return Err(
                "Project metric targets require unique nonempty metric IDs and finite values"
                    .into(),
            );
        }
    }
    Ok(())
}
