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
