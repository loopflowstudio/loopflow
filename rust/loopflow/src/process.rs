//! Causal command provenance never grants process or Flow control authority.

use serde::{Deserialize, Serialize};

use crate::durable::TaskId;
use crate::id::{ProcessLfid, TraceId, WaveId};

/// One recorded lf process. Unknown historical caller and exit evidence stays absent.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Process {
    pub lfid: ProcessLfid,
    pub pid: Option<u32>,
    pub kind: ProcessKind,
    pub agent_session_id: Option<String>,
    pub os_started_at: Option<i64>,
    pub trace_id: TraceId,
    pub parent_process_lfid: Option<ProcessLfid>,
    pub via_agent: Option<bool>,
    pub caller_session_id: Option<String>,
    pub caller_provider_generation: Option<i64>,
    pub command: Option<String>,
    pub repo: Option<String>,
    pub cwd: Option<String>,
    pub started_at: i64,
    pub completed_at: Option<i64>,
    pub outcome: Option<String>,
    pub exit_code: Option<i32>,
    pub signal: Option<String>,
    pub error: Option<String>,
}

/// Both kinds have one durable identity and one inventory entry.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ProcessKind {
    Lf,
    Agent,
}

/// Command discovery filters. Work means recorded work, never today's caller binding.
/// Contains searches are literal; command case folding follows SQLite lower().
#[derive(Debug, Clone, Default)]
pub struct ProcessFilter {
    pub lfid: Option<ProcessLfid>,
    pub repo: Option<String>,
    pub parent_process_lfid: Option<ProcessLfid>,
    pub caller_session_id: Option<String>,
    pub command_contains: Option<String>,
    pub identity_contains: Option<String>,
    pub outcome: Option<ProcessOutcomeFilter>,
    pub performed_work: Option<ProcessWorkFilter>,
}

/// Unknown means no terminal observation, not an OS liveness judgment.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum ProcessOutcomeFilter {
    Succeeded,
    Failed,
    Interrupted,
    Unknown,
}

/// IDs are already resolved by the caller, independently of launch eligibility.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum ProcessWorkFilter {
    Task(TaskId),
    Wave(WaveId),
}

/// Exclusive continuation in started_at DESC, lfid ASC order. Reuse the same filters.
/// A cursor is not a cross-request snapshot: late observations or changed outcomes may
/// change membership. Refresh from the first page to observe those changes.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProcessCursor {
    pub started_at: i64,
    pub lfid: ProcessLfid,
}

/// At most the requested number of command rows; no Session or Flow payloads.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProcessPage {
    pub entries: Vec<Process>,
    pub next: Option<ProcessCursor>,
}

pub const AGENT_CALLER_ENV: &str = "LF_AGENT_CALLER";

/// A command already rendered its diagnostic and selected its process status.
#[derive(Debug, thiserror::Error)]
#[error("command exited with status {0}")]
pub struct CommandExit(pub u8);

/// A Flow stopped where a person or a watcher takes over: blocked, or short of
/// its last step. Its process exits with [`FlowHeld::EXIT`], so a caller can
/// tell it from a Flow that failed.
#[derive(Debug, thiserror::Error)]
#[error("{0}")]
pub struct FlowHeld(pub String);

impl FlowHeld {
    pub const EXIT: u8 = 3;
}

/// Stable provenance installed in one provider conversation's tool environment.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AgentCaller {
    pub session_id: String,
    pub provider_generation: i64,
    #[serde(rename = "origin_exec_id")] // Retained provider environments use this format.
    pub origin_process_lfid: ProcessLfid,
}

/// An exact attachment capability, not another process or lifecycle owner.
/// Every claim gets a new token, even when the same lf process reattaches.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SessionAttachment {
    pub agent_process_lfid: ProcessLfid,
    pub process_lfid: Option<ProcessLfid>,
    pub token: crate::id::AttachmentToken,
    pub provider_generation: i64,
    pub provider_process_lfid: ProcessLfid,
}

impl SessionAttachment {
    pub fn caller(&self, session_id: String) -> AgentCaller {
        AgentCaller {
            session_id,
            provider_generation: self.provider_generation,
            origin_process_lfid: self.provider_process_lfid.clone(),
        }
    }
}

/// The provider OS process, including its current attachment. History survives
/// release and replacement; attachment and parent are independent identities.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct AgentProcess {
    pub process: Process,
    pub attached_process_lfid: Option<ProcessLfid>,
    pub attachment_token: Option<crate::id::AttachmentToken>,
    pub provider: Option<String>,
    pub interactive: bool,
}
