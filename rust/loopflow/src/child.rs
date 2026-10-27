//! Domain types shared by Project and Task control surfaces.

use std::path::PathBuf;

use serde::{Deserialize, Serialize};

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
