//! Domain types shared by Project and Task control surfaces.

use std::path::PathBuf;

use serde::{Deserialize, Serialize};
use time::OffsetDateTime;

use crate::work::project::ProjectId;
use crate::work::task::TaskId;

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum ChildDataError {
    #[error("invalid child Work id: {0}")]
    InvalidId(String),
}

/// Typed audit record for a body handoff. Work and Run identity are intentionally
/// absent: they do not change during this transition.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ChildBodyHandoff {
    pub from_agent: String,
    pub to_agent: String,
    pub from_provider: String,
    pub to_provider: String,
    pub reason: String,
}

/// The `lf` binary and Machine a child Work launch uses, resolved fresh at
/// the launch boundary from the current Machine — never persisted as Work state.
/// Work no longer pins a binary of its own.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ChildExecutionContext {
    pub lf_bin: PathBuf,
    pub lf_home: PathBuf,
}

/// A recorded request to end Work, durable from the moment the Abandon
/// command is queued rather than from the moment a runner consumes it.
///
/// The gap between those two moments is where a supervisor used to restart a
/// Work item someone had already ended: the pending command carried the intent,
/// but the Work row still read `Running`, and every launch path reads that Work.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AbandonIntent {
    pub requested_at: OffsetDateTime,
    pub reason: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", content = "id", rename_all = "snake_case")]
pub enum ChildRef {
    Project(ProjectId),
    Task(TaskId),
}

impl ChildRef {
    pub fn target_kind(&self) -> &'static str {
        match self {
            Self::Project(_) => "project",
            Self::Task(_) => "task",
        }
    }

    pub fn target_id(&self) -> &str {
        match self {
            Self::Project(id) => id.as_str(),
            Self::Task(id) => id.as_str(),
        }
    }
}
