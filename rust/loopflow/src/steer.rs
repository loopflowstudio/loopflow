//! Launch-owned authority for provider context callbacks.
use serde::{Deserialize, Serialize};

pub(crate) const PROVIDER_CONTEXT_ENV: &str = "LF_PROVIDER_CONTEXT";

/// Survives attachment-only handoff; replacement creates another capability.
#[derive(Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SteerAdapter {
    pub session: String,
    pub agent_process_id: crate::id::LfProcessId,
    pub capability: String,
}

impl std::fmt::Debug for SteerAdapter {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SteerAdapter")
            .field("session", &self.session)
            .field("agent_process_id", &self.agent_process_id)
            .finish_non_exhaustive()
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SteerPublication {
    pub id: crate::id::SteerPublicationId,
    pub sequence: i64,
    pub source: String,
    pub revision: String,
    pub author: crate::durable::Author,
    pub text: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionSteerStatus {
    pub publication: SteerPublication,
    pub attempt: i64,
    pub evidence: String,
    pub batch_id: Option<crate::id::SteerBatchId>,
    pub payload_hash: Option<String>,
    pub excerpted: Option<bool>,
}

/// A bounded page of receipts; capability is observed adapter evidence, never
/// inferred from an installed provider version or a successful stdout write.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionSteerPage {
    pub items: Vec<SessionSteerStatus>,
    pub next_after: Option<i64>,
    pub pending: i64,
    pub capability: String,
}

/// The exact context offered by a callback, including which sources were excerpted.
#[derive(Debug)]
pub(crate) struct SteerContext {
    pub text: String,
    pub excerpts: Vec<crate::id::SteerPublicationId>,
}
