//! Causal command provenance never grants process or Graph control authority.

use serde::{Deserialize, Serialize};

use crate::durable::TaskId;
use crate::id::{LfProcessId, LfSessionId, TraceId, WaveId};

/// One recorded lf process. Unknown historical caller and exit evidence stays absent.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LfProcess {
    pub id: LfProcessId,
    pub pid: Option<u32>,
    pub kind: ProcessKind,
    pub agent_session_id: Option<LfSessionId>,
    pub os_started_at: Option<i64>,
    pub trace_id: TraceId,
    pub parent_lf_process_id: Option<LfProcessId>,
    pub via_agent: Option<bool>,
    pub caller_session_id: Option<LfSessionId>,
    pub caller_agent_process_id: Option<LfProcessId>,
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

/// Who started a Process, derived from its ancestry: the first Session an
/// agent ran it from, else the person at the root of its chain.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
#[non_exhaustive]
pub enum Starter {
    /// A person ran it. `name` is absent when no participant is configured or
    /// the chain is missing rows: attribution unresolved.
    Person { name: Option<String> },
    /// An agent in this Session ran it, for its person.
    Session {
        id: LfSessionId,
        name: Option<String>,
        requester: Option<String>,
    },
}

impl std::fmt::Display for Starter {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Person { name: Some(name) } => f.write_str(name),
            Self::Person { name: None } => f.write_str("attribution unresolved"),
            Self::Session {
                id,
                name,
                requester,
            } => {
                match name {
                    Some(name) => write!(f, "Session {name:?}")?,
                    None => write!(f, "Session {id}")?,
                }
                match requester {
                    Some(requester) => write!(f, " for {requester}"),
                    None => Ok(()),
                }
            }
        }
    }
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
pub struct LfProcessFilter {
    pub id: Option<LfProcessId>,
    pub repo: Option<String>,
    pub parent_lf_process_id: Option<LfProcessId>,
    pub caller_session_id: Option<LfSessionId>,
    pub command_contains: Option<String>,
    pub identity_contains: Option<String>,
    pub outcome: Option<LfProcessOutcomeFilter>,
    pub performed_work: Option<LfProcessWorkFilter>,
}

/// Unknown means no terminal observation, not an OS liveness judgment.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum LfProcessOutcomeFilter {
    Succeeded,
    Failed,
    Interrupted,
    Unknown,
}

/// IDs are already resolved by the caller, independently of launch eligibility.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum LfProcessWorkFilter {
    Task(TaskId),
    Wave(WaveId),
}

/// Exclusive continuation in started_at DESC, id ASC order. Reuse the same filters.
/// A cursor is not a cross-request snapshot: late observations or changed outcomes may
/// change membership. Refresh from the first page to observe those changes.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LfProcessCursor {
    pub started_at: i64,
    pub id: LfProcessId,
}

/// At most the requested number of command rows; no Session or Graph payloads.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LfProcessPage {
    pub entries: Vec<LfProcess>,
    pub next: Option<LfProcessCursor>,
}

pub const AGENT_CALLER_ENV: &str = "LF_AGENT_CALLER";

/// A command already rendered its diagnostic and selected its process status.
#[derive(Debug, thiserror::Error)]
#[error("command exited with status {0}")]
pub struct CommandExit(pub u8);

/// A Graph stopped where a person or a watcher takes over: blocked, or short of
/// its last step. Its process exits with [`GraphHeld::EXIT`], so a caller can
/// tell it from a Graph that failed.
#[derive(Debug, thiserror::Error)]
#[error("{0}")]
pub struct GraphHeld(pub String);

impl GraphHeld {
    pub const EXIT: u8 = 3;
}

/// Stable provenance installed in one provider conversation's tool environment.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AgentCaller {
    pub session_id: LfSessionId,
    /// Absent in an environment installed before AgentProcess records: such a
    /// caller keeps its causal history and is never the current provider.
    pub agent_process_id: Option<LfProcessId>,
    #[serde(rename = "origin_exec_id")] // Retained provider environments use this format.
    pub origin_lf_process_id: LfProcessId,
}

/// An exact attachment capability, not another process or lifecycle owner.
/// Every claim gets a new token, even when the same lf process reattaches.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SessionAttachment {
    pub agent_process_id: LfProcessId,
    pub lf_process_id: Option<LfProcessId>,
    pub token: crate::id::AttachmentToken,
    pub provider_lf_process_id: LfProcessId,
}

impl SessionAttachment {
    pub fn caller(&self, session_id: LfSessionId) -> AgentCaller {
        AgentCaller {
            session_id,
            agent_process_id: Some(self.agent_process_id.clone()),
            origin_lf_process_id: self.provider_lf_process_id.clone(),
        }
    }
}

/// Where a conversation went when a mover ended its attachment. Recorded with
/// the stop itself, so the old terminal can say so as it closes.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub(crate) enum Departure {
    /// Another terminal on this Machine, on the same login, named as people
    /// see it (`Desktop`, `tmux %3`, `ttys004`) when the mover knew.
    Terminal { to: Option<String> },
    Account {
        account: crate::store::ProviderAccountId,
    },
    Machine {
        machine: crate::durable::MachineId,
        task: Option<TaskId>,
    },
}

/// The provider OS process, including its current attachment. History survives
/// release and replacement; attachment and parent are independent identities.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct AgentProcess {
    pub process: LfProcess,
    pub attached_lf_process_id: Option<LfProcessId>,
    pub attachment_token: Option<crate::id::AttachmentToken>,
    pub provider: Option<String>,
    pub interactive: bool,
    pub spawn_reserved: bool,
}
