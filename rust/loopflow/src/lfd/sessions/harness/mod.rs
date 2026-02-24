pub mod claude;
mod claude_mapping;
pub mod codex;
mod codex_mapping;
mod common;

use anyhow::Result;
use async_trait::async_trait;
use tokio::sync::broadcast;

use crate::lfd::prompt::PreparedPrompt;
use crate::lfd::sessions::types::SessionEvent;

#[derive(Debug, thiserror::Error)]
pub enum SessionHarnessError {
    #[error("turn already in progress")]
    TurnAlreadyInProgress,
}

pub fn is_turn_in_progress(err: &anyhow::Error) -> bool {
    matches!(
        err.downcast_ref::<SessionHarnessError>(),
        Some(SessionHarnessError::TurnAlreadyInProgress)
    )
}

#[async_trait]
pub trait SessionHarness: Send + Sync {
    async fn start(&mut self, prompt: &PreparedPrompt) -> Result<()>;
    async fn send_input(&mut self, content: &str) -> Result<()>;
    async fn stop(&mut self) -> Result<()>;
    fn set_provider_session_id(&mut self, _provider_session_id: Option<String>) {}
}

/// Constructor fn: `(provider, event_tx) -> harness`.
pub type CreateHarnessFn =
    fn(&str, broadcast::Sender<SessionEvent>) -> Result<Box<dyn SessionHarness>>;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HarnessProvider {
    Codex,
    Claude,
}

impl HarnessProvider {
    fn parse(provider: &str) -> Option<Self> {
        match provider.trim().to_ascii_lowercase().as_str() {
            "codex" => Some(Self::Codex),
            "claude" => Some(Self::Claude),
            _ => None,
        }
    }

    fn as_str(self) -> &'static str {
        match self {
            Self::Codex => "codex",
            Self::Claude => "claude",
        }
    }

    fn create(self, event_tx: broadcast::Sender<SessionEvent>) -> Box<dyn SessionHarness> {
        match self {
            Self::Codex => Box::new(codex::CodexHarness::new(event_tx)),
            Self::Claude => Box::new(claude::ClaudeHarness::new(event_tx)),
        }
    }
}

pub fn canonical_provider(provider: &str) -> Option<&'static str> {
    HarnessProvider::parse(provider).map(HarnessProvider::as_str)
}

pub fn default_create_harness(
    provider: &str,
    event_tx: broadcast::Sender<SessionEvent>,
) -> Result<Box<dyn SessionHarness>> {
    if let Some(provider) = HarnessProvider::parse(provider) {
        return Ok(provider.create(event_tx));
    }
    anyhow::bail!(
        "unsupported session provider: {}",
        provider.trim().to_lowercase()
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn canonical_provider_is_case_insensitive_and_trimmed() {
        assert_eq!(canonical_provider(" claUDe "), Some("claude"));
        assert_eq!(canonical_provider(" CODEX"), Some("codex"));
        assert_eq!(canonical_provider("lfharness"), None);
    }

    #[test]
    fn default_create_harness_rejects_unknown_provider() {
        let (tx, _rx) = broadcast::channel(16);
        match default_create_harness("lfharness", tx) {
            Ok(_) => panic!("should reject provider"),
            Err(err) => assert!(err.to_string().contains("unsupported session provider")),
        }
    }
}
