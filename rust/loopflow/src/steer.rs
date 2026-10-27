//! Launch-owned authority for provider context callbacks.
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

pub(crate) const PROVIDER_CONTEXT_ENV: &str = "LF_PROVIDER_CONTEXT";
/// The provider process's seen marker; see [`latest_marker`].
pub(crate) const STEER_SEEN_ENV: &str = "LF_STEER_SEEN";

/// Touched whenever a steer is saved. A tool-boundary hook skips lf entirely
/// while its provider's seen marker is strictly newer, since no steer arrived
/// after its last check. Prompt boundaries always check, which also catches
/// Sessions newly associated with earlier steers.
pub(crate) fn latest_marker(home: &Path) -> PathBuf {
    home.join("steers").join("latest")
}

pub(crate) fn seen_marker(home: &Path, process: &crate::id::LfProcessId) -> PathBuf {
    home.join("steers").join("seen").join(process.as_str())
}

pub(crate) fn touch(path: &Path) -> std::io::Result<()> {
    std::fs::create_dir_all(path.parent().expect("marker has a parent"))?;
    std::fs::OpenOptions::new()
        .create(true)
        .truncate(false)
        .write(true)
        .open(path)?
        .set_modified(std::time::SystemTime::now())
}

/// Survives attachment-only handoff; replacement creates another capability.
#[derive(Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SteerAdapter {
    pub session: String,
    pub agent_process_id: crate::id::LfProcessId,
    pub capability: String,
}

/// Where a provider context callback fires, which decides whether it may move
/// the Session's selected native conversation.
#[derive(Debug, Clone, Copy)]
pub(crate) enum NativeBoundary<'a> {
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

impl std::fmt::Debug for SteerAdapter {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SteerAdapter")
            .field("session", &self.session)
            .field("agent_process_id", &self.agent_process_id)
            .finish_non_exhaustive()
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Steer {
    pub id: crate::id::SteerId,
    pub sequence: i64,
    pub source: String,
    pub revision: String,
    pub author: crate::durable::Author,
    pub text: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionSteerStatus {
    pub steer: Steer,
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
    pub excerpts: Vec<crate::id::SteerId>,
}
