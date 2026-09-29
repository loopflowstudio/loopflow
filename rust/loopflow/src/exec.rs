//! Causal command provenance never grants process or Flow control authority.

use serde::{Deserialize, Serialize};

use crate::id::{ExecId, TraceId};

/// One recorded lf process. Unknown historical caller and exit evidence stays absent.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Exec {
    pub id: ExecId,
    pub trace_id: TraceId,
    pub parent_exec_id: Option<ExecId>,
    pub via_agent: Option<bool>,
    pub caller_session_id: Option<String>,
    pub caller_provider_generation: Option<i64>,
    pub caller_flow_turn: Option<String>,
    pub command: Option<String>,
    pub repo: Option<String>,
    pub cwd: Option<String>,
    pub started_at: i64,
    pub completed_at: Option<i64>,
    pub outcome: Option<String>,
    pub exit_code: Option<i32>,
    pub signal: Option<String>,
    /// Recorded command context from its trace, not work assigned to its caller later.
    pub wave: Option<String>,
}

pub const AGENT_CALLER_ENV: &str = "LF_AGENT_CALLER";

/// A command already rendered its diagnostic and selected its process status.
#[derive(Debug, thiserror::Error)]
#[error("command exited with status {0}")]
pub struct CommandExit(pub u8);

/// Stable provenance installed in one provider conversation's tool environment.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AgentCaller {
    pub session_id: String,
    pub provider_generation: i64,
    pub origin_exec_id: ExecId,
    /// Native launch correlation retained by tool descendants across retries.
    pub flow_turn: Option<String>,
}

/// Separate fences: reconnecting a driver does not replace its live provider.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SessionDriver {
    pub exec_id: Option<ExecId>,
    pub generation: i64,
    pub provider_generation: i64,
    pub provider_exec_id: ExecId,
}

impl SessionDriver {
    pub fn caller(&self, session_id: String) -> AgentCaller {
        AgentCaller {
            session_id,
            provider_generation: self.provider_generation,
            origin_exec_id: self.provider_exec_id.clone(),
            flow_turn: None,
        }
    }
}
