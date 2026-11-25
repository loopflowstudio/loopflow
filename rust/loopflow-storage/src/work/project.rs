//! A Project: a group of a Wave's Tasks that starts and ends, mirrored to a
//! tracker Project when one is connected. It owns no worktree, shipping branch,
//! PR, permanent memory, cadence, chat, or controller state.
use serde::{Deserialize, Serialize};
use time::OffsetDateTime;

use crate::planning::{LinearBindingError, LinearProjectId};
pub use loopflow_model::durable::ProjectId;
use loopflow_model::id::WaveId;

/// Shared Project status, independent of a provider’s display label.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
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

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, schemars::JsonSchema)]
pub struct Project {
    pub id: ProjectId,
    pub summary: String,
    pub slug: String,
    pub name: String,
    pub description: String,
    pub status: ProjectStatus,
    /// The record’s ID in the repository's tracker.
    #[serde(deserialize_with = "Option::deserialize")]
    pub tracker_id: Option<LinearProjectId>,
    /// Time of the last successful tracker refresh.
    #[serde(deserialize_with = "Option::deserialize")]
    pub tracker_synced_at: Option<i64>,
    /// Current ownership. Wave name and checkout are resolved from this id.
    pub wave_id: WaveId,
    #[schemars(with = "String")]
    #[serde(with = "time::serde::rfc3339")]
    pub created_at: OffsetDateTime,
    #[schemars(with = "String")]
    #[serde(with = "time::serde::rfc3339")]
    pub updated_at: OffsetDateTime,
}

impl Project {
    pub fn tracker_id(&self) -> Result<&LinearProjectId, LinearBindingError> {
        self.tracker_id
            .as_ref()
            .ok_or(LinearBindingError::NoLinearMapping)
    }
}
