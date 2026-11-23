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

/// A managed conversation whose last boundary was a turn end, and how the
/// Machine server reaches it there.
#[derive(Debug, Clone)]
pub struct IdleConversation {
    pub adapter: SteerAdapter,
    /// The Session's selected native conversation.
    pub thread: String,
    /// The Session's working directory, where rendered steers are saved.
    pub cwd: std::path::PathBuf,
    pub reach: IdleReach,
}

/// How the Machine server starts a turn in an idle conversation, chosen by
/// its provider from `agent_inbox` (Codex: `agent_endpoint`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum IdleReach {
    /// The inbox key file Claude publishes beside its session record.
    Claude(std::path::PathBuf),
    /// The Session's private app-server.
    Codex(std::path::PathBuf),
    /// The wake socket an OpenCode plugin or Pi extension listens on inside
    /// the provider's process.
    Plugin(std::path::PathBuf),
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
