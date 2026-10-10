//! Observed Task membership grants no attachment, process or Flow authority.

use serde::{Deserialize, Serialize};

use crate::durable::FlowProcessInventoryEntry;
use crate::process::LfProcess;

/// All retained work in a checkout, plus explicitly attributed work elsewhere.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TaskWork {
    pub sessions: Vec<TaskSession>,
    pub flow_processes: Vec<FlowProcessInventoryEntry>,
    pub processes: Vec<LfProcess>,
    /// The Task's Workflow; `None` when it runs only ad hoc Flows.
    pub workflow: Option<crate::ops::workflow::Workflow>,
}

/// Conversation identity and completion, without loading its input or transcript.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TaskSession {
    pub id: String,
    pub title: String,
    pub interactive: bool,
    /// The Flow whose step opened its current input.
    pub flow_lf_process_id: Option<String>,
    pub completed_at: Option<i64>,
}
