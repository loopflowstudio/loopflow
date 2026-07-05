pub mod claude;
mod claude_mapping;
pub mod codex;
mod codex_mapping;
mod common;
#[cfg(test)]
mod conformance_tests;
mod lf_tag;
pub mod opencode;
mod opencode_mapping;

use anyhow::Result;
use async_trait::async_trait;
use tokio::sync::mpsc;

use crate::engine::agent::AgentConfig;
use crate::lfd::conversations::types::ConversationEvent;

#[derive(Debug, thiserror::Error)]
pub enum HarnessError {
    #[error("turn already in progress")]
    TurnAlreadyInProgress,
}

pub fn is_turn_in_progress(err: &anyhow::Error) -> bool {
    matches!(
        err.downcast_ref::<HarnessError>(),
        Some(HarnessError::TurnAlreadyInProgress)
    )
}

/// A stream error that means the harness session itself is dead (not just the
/// turn). Claude has no such code by construction: it runs one subprocess per
/// turn, so a crash fails the turn (`TurnCompleted { Failed }`) and the next
/// turn spawns fresh via `--resume`.
pub fn is_terminal_harness_error(code: &str) -> bool {
    matches!(code, "codex_disconnected" | "opencode_disconnected")
}

/// What a driver can honestly do, reported per instance so callers degrade
/// instead of probing vendor behavior out of band.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Capabilities {
    /// `send_input` during a turn injects into the running turn (steer).
    /// When false, mid-turn input fails with `TurnAlreadyInProgress` and the
    /// caller must queue.
    pub supports_steer: bool,
}

/// How a harness answers vendor approval/permission requests.
///
/// `AutoApprove` is the only variant until Decisions land; the enum exists so
/// approval behavior is an explicit construction-time policy instead of a
/// constant buried in each transport.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ApprovalPolicy {
    /// Approve every request the vendor asks about.
    AutoApprove,
}

#[async_trait]
pub trait Harness: Send + Sync {
    async fn start(&mut self, config: &AgentConfig) -> Result<()>;
    async fn send_input(&mut self, content: &str) -> Result<()>;
    /// Cancel the in-flight turn but keep the session alive for the next
    /// turn. The interrupted turn surfaces as a
    /// `TurnCompleted { status: Interrupted }` terminal event. No-op when no
    /// turn is in flight.
    async fn interrupt(&mut self) -> Result<()>;
    /// Full teardown: cancel any in-flight turn and end the vendor session.
    async fn stop(&mut self) -> Result<()>;
    fn capabilities(&self) -> Capabilities;
    /// Vendor session/thread id, once the vendor has announced it. Codex and
    /// opencode announce it by the time `start` returns; claude announces it
    /// on the first turn's stream. Callers persist this before driving turns.
    fn provider_session_id(&self) -> Option<String>;
    /// Seed a previously persisted vendor session id so the next turn resumes
    /// it. Drivers that take resume state at `start` instead ignore this.
    fn set_provider_session_id(&mut self, _provider_session_id: Option<String>) {}
}

/// Constructor fn: `(harness_kind, approval, event_tx) -> harness`.
pub type CreateHarnessFn =
    fn(&str, ApprovalPolicy, mpsc::UnboundedSender<ConversationEvent>) -> Result<Box<dyn Harness>>;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HarnessKind {
    Codex,
    Claude,
    OpenCode,
}

impl HarnessKind {
    pub fn parse(name: &str) -> Option<Self> {
        match name.trim().to_ascii_lowercase().as_str() {
            "codex" => Some(Self::Codex),
            "claude" => Some(Self::Claude),
            "opencode" => Some(Self::OpenCode),
            _ => None,
        }
    }

    fn as_str(self) -> &'static str {
        match self {
            Self::Codex => "codex",
            Self::Claude => "claude",
            Self::OpenCode => "opencode",
        }
    }

    fn create(
        self,
        approval: ApprovalPolicy,
        event_tx: mpsc::UnboundedSender<ConversationEvent>,
    ) -> Box<dyn Harness> {
        match self {
            Self::Codex => Box::new(codex::CodexHarness::new(event_tx, approval)),
            // Claude approvals ride the per-turn CLI flags built from
            // AgentConfig, not a runtime channel; no policy to thread.
            Self::Claude => Box::new(claude::ClaudeHarness::new(event_tx)),
            Self::OpenCode => Box::new(opencode::OpenCodeHarness::new(event_tx, approval)),
        }
    }
}

pub fn canonical_harness(name: &str) -> Option<&'static str> {
    HarnessKind::parse(name).map(HarnessKind::as_str)
}

pub fn default_create_harness(
    name: &str,
    approval: ApprovalPolicy,
    event_tx: mpsc::UnboundedSender<ConversationEvent>,
) -> Result<Box<dyn Harness>> {
    if let Some(kind) = HarnessKind::parse(name) {
        return Ok(kind.create(approval, event_tx));
    }
    anyhow::bail!(
        "unsupported session harness: {}",
        name.trim().to_lowercase()
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn canonical_harness_is_case_insensitive_and_trimmed() {
        assert_eq!(canonical_harness(" claUDe "), Some("claude"));
        assert_eq!(canonical_harness(" CODEX"), Some("codex"));
        assert_eq!(canonical_harness("OpenCode"), Some("opencode"));
        assert_eq!(canonical_harness("lfharness"), None);
    }

    #[test]
    fn default_create_harness_rejects_unknown() {
        let (tx, _rx) = mpsc::unbounded_channel();
        match default_create_harness("lfharness", ApprovalPolicy::AutoApprove, tx) {
            Ok(_) => panic!("should reject unknown harness"),
            Err(err) => assert!(err.to_string().contains("unsupported session harness")),
        }
    }

    #[test]
    fn terminal_harness_error_recognizes_disconnects_only() {
        assert!(is_terminal_harness_error("opencode_disconnected"));
        assert!(is_terminal_harness_error("codex_disconnected"));
        assert!(!is_terminal_harness_error("opencode_error"));
        // Claude has no session-terminal code: per-turn subprocess.
        assert!(!is_terminal_harness_error("claude_harness_crashed"));
    }

    #[tokio::test]
    async fn capabilities_steer_is_codex_only() {
        let (tx, _rx) = mpsc::unbounded_channel();
        for (name, steer) in [("codex", true), ("claude", false), ("opencode", false)] {
            let harness = default_create_harness(name, ApprovalPolicy::AutoApprove, tx.clone())
                .expect("known harness");
            let caps = harness.capabilities();
            assert_eq!(caps.supports_steer, steer, "steer for {name}");
        }
    }
}
