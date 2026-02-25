pub mod claude;
mod claude_mapping;
pub mod codex;
mod codex_mapping;
mod common;
#[cfg(test)]
mod conformance_tests;
pub mod opencode;
mod opencode_mapping;

use anyhow::Result;
use async_trait::async_trait;
use tokio::sync::mpsc;

use crate::engine::agent::AgentConfig;
use crate::lfd::sessions::types::SessionEvent;

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

pub fn is_terminal_harness_error(code: &str) -> bool {
    matches!(
        code,
        "codex_disconnected" | "claude_harness_crashed" | "opencode_disconnected"
    )
}

#[async_trait]
pub trait Harness: Send + Sync {
    async fn start(&mut self, config: &AgentConfig) -> Result<()>;
    async fn send_input(&mut self, content: &str) -> Result<()>;
    async fn stop(&mut self) -> Result<()>;
    fn set_provider_session_id(&mut self, _provider_session_id: Option<String>) {}
}

/// Constructor fn: `(harness_kind, event_tx) -> harness`.
pub type CreateHarnessFn =
    fn(&str, mpsc::UnboundedSender<SessionEvent>) -> Result<Box<dyn Harness>>;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HarnessKind {
    Codex,
    Claude,
    OpenCode,
}

impl HarnessKind {
    fn parse(name: &str) -> Option<Self> {
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

    fn create(self, event_tx: mpsc::UnboundedSender<SessionEvent>) -> Box<dyn Harness> {
        match self {
            Self::Codex => Box::new(codex::CodexHarness::new(event_tx)),
            Self::Claude => Box::new(claude::ClaudeHarness::new(event_tx)),
            Self::OpenCode => Box::new(opencode::OpenCodeHarness::new(event_tx)),
        }
    }
}

pub fn canonical_harness(name: &str) -> Option<&'static str> {
    HarnessKind::parse(name).map(HarnessKind::as_str)
}

pub fn default_create_harness(
    name: &str,
    event_tx: mpsc::UnboundedSender<SessionEvent>,
) -> Result<Box<dyn Harness>> {
    if let Some(kind) = HarnessKind::parse(name) {
        return Ok(kind.create(event_tx));
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
        match default_create_harness("lfharness", tx) {
            Ok(_) => panic!("should reject unknown harness"),
            Err(err) => assert!(err.to_string().contains("unsupported session harness")),
        }
    }
}
