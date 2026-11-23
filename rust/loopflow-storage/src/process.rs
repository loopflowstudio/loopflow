use serde::{Deserialize, Serialize};

use loopflow_model::durable::TaskId;
use loopflow_model::id::{AgentProcessId, LfProcessId, LfSessionId, TraceId};

/// One recorded lf process. Unknown historical caller and exit evidence stays absent.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
pub struct LfProcess {
    pub id: LfProcessId,
    pub pid: Option<u32>,
    pub os_started_at: Option<i64>,
    pub trace_id: TraceId,
    pub parent_lf_process_id: Option<LfProcessId>,
    pub via_agent: Option<bool>,
    pub caller_session_id: Option<LfSessionId>,
    pub caller_agent_process_id: Option<AgentProcessId>,
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

/// The lf taking a Session's attachment: what attaching records of it.
#[derive(Debug, Clone)]
pub struct AttachingLf {
    pub lf_process_id: LfProcessId,
    pub holder: AttachmentHolder,
    pub terminal: Option<loopflow_model::id::TerminalId>,
}

/// One recorded provider OS process, launched under the lf process that
/// reserved it. It never has a receipt or an agent caller of its own.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
pub struct AgentProcess {
    pub id: AgentProcessId,
    pub pid: Option<u32>,
    pub os_started_at: Option<i64>,
    pub agent_session_id: Option<LfSessionId>,
    pub trace_id: TraceId,
    pub parent_lf_process_id: Option<LfProcessId>,
    pub command: Option<String>,
    pub repo: Option<String>,
    pub cwd: Option<String>,
    pub started_at: i64,
    pub completed_at: Option<i64>,
    pub exit_code: Option<i32>,
    pub signal: Option<String>,
    pub error: Option<String>,
}

/// One row of the process inventory: both kinds share one history and paging order.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum ProcessEntry {
    Lf(LfProcess),
    Agent(AgentProcess),
}

impl ProcessEntry {
    pub fn id(&self) -> &str {
        match self {
            Self::Lf(process) => process.id.as_str(),
            Self::Agent(process) => process.id.as_str(),
        }
    }

    pub fn started_at(&self) -> i64 {
        match self {
            Self::Lf(process) => process.started_at,
            Self::Agent(process) => process.started_at,
        }
    }

    pub fn completed_at(&self) -> Option<i64> {
        match self {
            Self::Lf(process) => process.completed_at,
            Self::Agent(process) => process.completed_at,
        }
    }

    pub fn pid(&self) -> Option<u32> {
        match self {
            Self::Lf(process) => process.pid,
            Self::Agent(process) => process.pid,
        }
    }

    pub fn os_started_at(&self) -> Option<i64> {
        match self {
            Self::Lf(process) => process.os_started_at,
            Self::Agent(process) => process.os_started_at,
        }
    }

    pub fn parent_lf_process_id(&self) -> Option<&LfProcessId> {
        match self {
            Self::Lf(process) => process.parent_lf_process_id.as_ref(),
            Self::Agent(process) => process.parent_lf_process_id.as_ref(),
        }
    }

    pub fn trace_id(&self) -> &TraceId {
        match self {
            Self::Lf(process) => &process.trace_id,
            Self::Agent(process) => &process.trace_id,
        }
    }

    pub fn command(&self) -> Option<&str> {
        match self {
            Self::Lf(process) => process.command.as_deref(),
            Self::Agent(process) => process.command.as_deref(),
        }
    }

    pub fn repo(&self) -> Option<&str> {
        match self {
            Self::Lf(process) => process.repo.as_deref(),
            Self::Agent(process) => process.repo.as_deref(),
        }
    }

    pub fn cwd(&self) -> Option<&str> {
        match self {
            Self::Lf(process) => process.cwd.as_deref(),
            Self::Agent(process) => process.cwd.as_deref(),
        }
    }

    /// How an lf process ended. A provider process records its exit code and
    /// signal, and no outcome.
    pub fn outcome(&self) -> Option<&str> {
        match self {
            Self::Lf(process) => process.outcome.as_deref(),
            Self::Agent(_) => None,
        }
    }

    pub fn as_lf(&self) -> Option<&LfProcess> {
        match self {
            Self::Lf(process) => Some(process),
            Self::Agent(_) => None,
        }
    }

    pub fn as_agent(&self) -> Option<&AgentProcess> {
        match self {
            Self::Lf(_) => None,
            Self::Agent(process) => Some(process),
        }
    }
}

/// Who started a Process, derived from its ancestry: the first Session an
/// agent ran it from, else the person at the root of its chain.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
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
    /// A configured default set it: `auto: on`. Only an auto mark names one.
    Config { layer: ConfigLayer },
}

/// The configuration a default came from, as `lf task status` names it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum ConfigLayer {
    /// The Task's Wave's `config.yaml`, or a personal setting for that Wave.
    Wave,
    /// The repository's `.lf/config.yaml`.
    Repository,
    /// Personal configuration.
    Personal,
}

impl ConfigLayer {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Wave => "wave",
            Self::Repository => "repository",
            Self::Personal => "personal",
        }
    }
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
            Self::Config { layer } => write!(f, "{} config", layer.as_str()),
        }
    }
}

/// Stable provenance installed in one provider conversation's tool environment.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AgentCaller {
    pub session_id: LfSessionId,
    /// Absent in an environment installed before AgentProcess records: such a
    /// caller keeps its causal history and is never the current provider.
    pub agent_process_id: Option<AgentProcessId>,
    #[serde(rename = "origin_exec_id")] // Retained provider environments use this format.
    pub origin_lf_process_id: LfProcessId,
    /// The launcher's trace; the agent's commands join it under the launcher.
    /// Absent in a retained environment: such a caller starts a fresh trace.
    pub trace_id: Option<TraceId>,
}

/// An exact attachment capability, not another process or lifecycle owner.
/// Every claim starts a new agent with a new token, even when the same lf reattaches.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SessionAttachment {
    pub agent_process_id: AgentProcessId,
    pub lf_process_id: Option<LfProcessId>,
    pub token: loopflow_model::id::AttachmentToken,
    /// The lf that attached this agent; it stays after the attachment is released.
    #[serde(rename = "provider_lf_process_id")]
    // Kept exit records and held hosts use this name.
    pub launcher_lf_process_id: LfProcessId,
}

impl SessionAttachment {
    pub fn caller(&self, session_id: LfSessionId, trace_id: Option<TraceId>) -> AgentCaller {
        AgentCaller {
            session_id,
            agent_process_id: Some(self.agent_process_id.clone()),
            origin_lf_process_id: self.launcher_lf_process_id.clone(),
            trace_id,
        }
    }
}

/// Where a conversation went when a mover ended its attachment. Recorded with
/// the stop itself, so the old terminal can say so as it closes.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Departure {
    Paused {
        task: TaskId,
    },
    /// Another terminal on this Machine, on the same login, named as people
    /// see it (`Desktop`, `tmux %3`, `ttys004`) when the mover knew.
    Terminal {
        to: Option<String>,
    },
    Machine {
        machine: loopflow_model::durable::MachineId,
        task: Option<TaskId>,
    },
}

/// The OS process of the lf holding a Session's attachment, as it recorded
/// itself when it attached. A holder that recorded none cannot be confirmed.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AttachmentHolder {
    pub pid: Option<u32>,
    pub os_started_at: Option<i64>,
}

/// A Session's agent: the provider OS process it runs as, the lf that
/// launched it, and the attachment holding it now with that holder's OS
/// process. Released and replaced attachments leave the agent its launcher.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AgentProcessState {
    pub id: AgentProcessId,
    pub session_id: String,
    pub started_at: Option<i64>,
    pub ended_at: Option<i64>,
    pub pid: Option<u32>,
    pub os_started_at: Option<i64>,
    pub launcher_lf_process_id: Option<LfProcessId>,
    pub attached_lf_process_id: Option<LfProcessId>,
    pub attachment_token: Option<loopflow_model::id::AttachmentToken>,
    /// `None` when no lf holds the attachment.
    pub holder: Option<AttachmentHolder>,
    pub provider: Option<String>,
    pub interactive: bool,
    pub spawn_reserved: bool,
}
