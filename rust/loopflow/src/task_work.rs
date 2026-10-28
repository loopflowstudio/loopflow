//! Observed Task membership grants no attachment, process or Graph authority.

use serde::{Deserialize, Serialize};

use crate::durable::GraphProcessInventoryEntry;
use crate::process::LfProcess;

/// All retained work in a checkout, plus explicitly attributed work elsewhere.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TaskWork {
    pub sessions: Vec<TaskSession>,
    pub graph_processes: Vec<GraphProcessInventoryEntry>,
    pub processes: Vec<LfProcess>,
    /// The graph the Task took up; `None` until its first `lf task run`.
    pub graph: Option<crate::task_graph::TaskGraph>,
}

/// Workspace facts needed before opening diagnostic Process history.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TaskOverview {
    pub sessions: Vec<TaskSession>,
    pub graph_processes: Vec<GraphProcessInventoryEntry>,
    pub graph: Option<crate::task_graph::TaskGraph>,
}

/// Conversation identity and completion, without loading its input or transcript.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TaskSession {
    pub id: crate::id::LfSessionId,
    pub title: String,
    pub interactive: bool,
    /// The Graph whose step opened its current input.
    #[serde(deserialize_with = "crate::id::deserialize_uuid_option")]
    pub graph_lf_process_id: Option<String>,
    pub completed_at: Option<i64>,
}
