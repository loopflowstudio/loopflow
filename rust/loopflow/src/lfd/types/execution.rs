//! Execution process types.

use serde::{Deserialize, Serialize};
use time::OffsetDateTime;

use crate::lfd::id::LfdId;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
#[non_exhaustive]
pub enum ExecutionProcessStatus {
    #[default]
    Unspecified = 0,
    Running = 1,
    Waiting = 2,
    Completed = 3,
    Failed = 4,
}

impl ExecutionProcessStatus {
    pub fn from_i32(value: i32) -> Self {
        match value {
            1 => Self::Running,
            2 => Self::Waiting,
            3 => Self::Completed,
            4 => Self::Failed,
            _ => Self::Unspecified,
        }
    }

    pub fn as_i32(&self) -> i32 {
        *self as i32
    }

    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Unspecified => "unspecified",
            Self::Running => "running",
            Self::Waiting => "waiting",
            Self::Completed => "completed",
            Self::Failed => "failed",
        }
    }
}

/// Process lifecycle record for an invocation backend.
///
/// `ExecutionProcess` tracks execution state (running/waiting/completed), process IDs,
/// and container metadata. User-facing control state lives in `Session`.
/// The two are linked through shared wave execution lineage (`run_id`).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExecutionProcess {
    pub id: LfdId,
    pub step: String,
    pub repo: String,
    pub worktree: String,
    pub run_id: Option<LfdId>,
    pub status: ExecutionProcessStatus,
    #[serde(with = "time::serde::rfc3339::option")]
    pub started_at: Option<OffsetDateTime>,
    #[serde(with = "time::serde::rfc3339::option")]
    pub ended_at: Option<OffsetDateTime>,
    pub pid: Option<u32>,
    pub container_id: Option<String>,
    pub agent: String,
    pub run_mode: String,
}
