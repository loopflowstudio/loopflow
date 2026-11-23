use crate::ProviderAccountId;
use loopflow_model::id::AgentSessionId;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

/// What the store kept of one provider conversation.
#[derive(Debug, Clone, PartialEq)]
pub struct ThreadFacts {
    pub thread: AgentSessionId,
    pub harness: Option<String>,
    pub account: Option<ProviderAccountId>,
    pub turns: Vec<TurnFacts>,
}

/// One native turn's receipts.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TurnFacts {
    pub turn: String,
    pub started_at: Option<i64>,
    pub settled_at: Option<i64>,
    pub status: Option<String>,
}

impl ThreadFacts {
    pub fn last_at(&self) -> Option<i64> {
        self.turns
            .iter()
            .flat_map(|turn| [turn.started_at, turn.settled_at])
            .flatten()
            .max()
    }
}

#[derive(Debug, Clone)]
pub struct RetainedSession {
    pub id: String,
    pub title: String,
    pub harness: String,
    pub thread: Option<AgentSessionId>,
    pub cwd: PathBuf,
    /// Its last recorded activity, Unix seconds.
    pub last_at: i64,
    pub account: Option<ProviderAccountId>,
}
