//! Causal command provenance never grants process or Flow control authority.

use serde::{Deserialize, Serialize};

use crate::id::ExecId;

pub const AGENT_CALLER_ENV: &str = "LF_AGENT_CALLER";

/// Stable provenance installed in one provider conversation's tool environment.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AgentCaller {
    pub session_id: String,
    pub provider_generation: i64,
    pub origin_exec_id: ExecId,
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
        }
    }
}
