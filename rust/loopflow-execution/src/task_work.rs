//! Observed Task membership grants no attachment, process or Graph authority.

use serde::{Deserialize, Serialize};

use crate::durable::GraphProcessInventoryEntry;

/// All retained work in a checkout, plus explicitly attributed work elsewhere.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
pub struct TaskWork {
    pub sessions: Vec<TaskSession>,
    pub graph_processes: Vec<GraphProcessInventoryEntry>,
    /// The graph the Task took up; `None` until its first `lf task run`.
    pub graph: Option<crate::task_graph::TaskGraph>,
}

/// Workspace facts without the Task's full history.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
pub struct TaskOverview {
    pub sessions: Vec<TaskSession>,
    pub graph_processes: Vec<GraphProcessInventoryEntry>,
    pub graph: Option<crate::task_graph::TaskGraph>,
}

pub use loopflow_storage::task_session::TaskSession;
