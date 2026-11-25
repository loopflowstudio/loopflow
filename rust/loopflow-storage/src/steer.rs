use serde::{Deserialize, Serialize};

/// Belongs to one agent; replacement creates another capability.
#[derive(Clone, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct SteerAdapter {
    pub session: String,
    pub agent_process_id: loopflow_model::id::AgentProcessId,
    pub capability: String,
}

/// Where a provider context callback fires, which decides whether it may move
/// the Session's selected native conversation.
#[derive(Debug, Clone, Copy)]
pub enum NativeBoundary<'a> {
    /// The provider declares the conversation it now runs (Claude/Codex
    /// `SessionStart`: startup, resume, clear, compact).
    Select {
        source: &'a str,
    },
    /// Fires on every request, including subagents', so it only fills a
    /// missing selection (OpenCode transforms, Pi).
    Activate {
        source: &'a str,
    },
    Turn,
}

/// The endpoints recorded for an idle managed session.
#[derive(Debug)]
pub struct IdleEndpoints {
    pub endpoint: Option<std::path::PathBuf>,
    pub inbox: Option<std::path::PathBuf>,
    pub socket: Option<std::path::PathBuf>,
}

#[derive(Debug)]
pub struct IdleCandidate {
    pub adapter: SteerAdapter,
    pub thread: String,
    pub cwd: std::path::PathBuf,
    pub provider: Option<String>,
    pub endpoints: IdleEndpoints,
    pub identity: Option<(u32, i64)>,
}

/// Where code the provider loads takes a message for an idle conversation,
/// as its hook, plugin or extension names it at a boundary.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(rename_all = "snake_case", deny_unknown_fields)]
pub enum Inbox {
    /// The key file Claude publishes for peers of its cross-session inbox,
    /// `sessions/<pid>.<sha256 of socket>.key` in its configuration
    /// directory; the server reads the key when it sends.
    Claude(std::path::PathBuf),
    /// The Unix socket an OpenCode plugin or Pi extension listens on.
    Socket(std::path::PathBuf),
}

impl std::fmt::Debug for SteerAdapter {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SteerAdapter")
            .field("session", &self.session)
            .field("agent_process_id", &self.agent_process_id)
            .finish_non_exhaustive()
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
pub struct Steer {
    pub id: loopflow_model::id::SteerId,
    pub sequence: i64,
    pub source: String,
    pub revision: String,
    pub author: loopflow_model::durable::Author,
    pub text: String,
}

/// One steer as a Session sees it.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionSteerStatus {
    pub steer: Steer,
    /// When the Session read it, in Unix seconds; null while it waits.
    #[serde(deserialize_with = "Option::deserialize")]
    pub delivered_at: Option<i64>,
}

/// A bounded page of the steers that apply to a Session or that it read.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionSteerPage {
    pub items: Vec<SessionSteerStatus>,
    #[serde(deserialize_with = "Option::deserialize")]
    pub next_after: Option<i64>,
    pub pending: i64,
}

/// A steer the server read that names `session`, an interactive Session
/// that has not read it; `identity` is its unfinished AgentProcess, if any.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SteerWake {
    pub steer: String,
    pub session: String,
    pub identity: Option<(u32, i64)>,
}
