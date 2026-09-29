//! Causal command provenance never grants process or Flow control authority.

use serde::{Deserialize, Serialize};

use crate::durable::TaskId;
use crate::id::{ExecId, TraceId, WaveId};

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

/// Command discovery filters. Work means recorded work, never today's caller binding.
/// Contains searches are literal; command case folding follows SQLite lower().
#[derive(Debug, Clone, Default)]
pub struct ExecFilter {
    pub id: Option<ExecId>,
    pub repo: Option<String>,
    pub parent_exec_id: Option<ExecId>,
    pub caller_session_id: Option<String>,
    pub command_contains: Option<String>,
    pub identity_contains: Option<String>,
    pub outcome: Option<ExecOutcomeFilter>,
    pub performed_work: Option<ExecWorkFilter>,
}

/// Unknown means no terminal observation, not an OS liveness judgment.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum ExecOutcomeFilter {
    Succeeded,
    Failed,
    Interrupted,
    Unknown,
}

/// IDs are already resolved by the caller, independently of launch eligibility.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum ExecWorkFilter {
    Task(TaskId),
    Wave(WaveId),
}

/// Exclusive continuation in started_at DESC, id ASC order. Reuse the same filters.
/// A cursor is not a cross-request snapshot: late imports or changed outcomes may
/// change membership. Refresh from the first page to observe those changes.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ExecCursor {
    pub started_at: i64,
    pub id: ExecId,
}

/// At most the requested number of command rows; no Session or Flow payloads.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ExecPage {
    pub entries: Vec<Exec>,
    pub next: Option<ExecCursor>,
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

/// Read-local ownership evidence; never a liveness or control claim.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct SessionProcessObservation {
    pub id: String,
    pub title: String,
    pub work: Option<crate::durable::WorkRef>,
    pub driver_exec_id: Option<ExecId>,
    pub driver_trace_id: Option<String>,
    pub driver_generation: i64,
    pub provider_exec_id: Option<ExecId>,
    pub provider_pid: Option<u32>,
    pub provider_started_at: Option<i64>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct SessionProcessOwnership {
    pub sessions: Vec<SessionProcessObservation>,
    pub inputs: std::collections::BTreeMap<String, String>,
}
