//! Observed Task membership grants no driver, process or Flow authority.

use serde::{Deserialize, Serialize};

use crate::durable::FlowInventoryEntry;
use crate::exec::Exec;
use crate::session::SessionKind;

/// All retained work in a checkout, plus explicitly attributed work elsewhere.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TaskWork {
    pub sessions: Vec<TaskSession>,
    pub flows: Vec<FlowInventoryEntry>,
    pub execs: Vec<Exec>,
}

/// Conversation identity and completion, without loading its input or transcript.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TaskSession {
    pub id: String,
    pub title: String,
    pub kind: SessionKind,
    pub interactive: bool,
    pub flow_session_id: Option<String>,
    pub completed_at: Option<i64>,
    pub managed: bool,
}
