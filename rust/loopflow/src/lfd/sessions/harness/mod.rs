pub mod claude;
mod claude_mapping;
pub mod codex;
mod codex_mapping;
mod common;
pub mod lfharness;

use anyhow::Result;
use async_trait::async_trait;
use tokio::sync::broadcast;

use crate::lfd::sessions::types::{SessionConfig, SessionEvent};

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
    async fn start(&mut self, config: &SessionConfig) -> Result<()>;
    async fn send_input(&mut self, content: &str) -> Result<()>;
    async fn stop(&mut self) -> Result<()>;
    fn set_provider_session_id(&mut self, _provider_session_id: Option<String>) {}
}

/// Constructor fn: `(provider, event_tx) -> harness`.
pub type CreateHarnessFn =
    fn(&str, broadcast::Sender<SessionEvent>) -> Result<Box<dyn SessionHarness>>;

pub fn supports_provider(provider: &str) -> bool {
    matches!(provider, "codex" | "claude" | "lfharness")
}

pub fn default_create_harness(
    provider: &str,
    event_tx: broadcast::Sender<SessionEvent>,
) -> Result<Box<dyn SessionHarness>> {
    match provider {
        "codex" => Ok(Box::new(codex::CodexHarness::new(event_tx))),
        "claude" => Ok(Box::new(claude::ClaudeHarness::new(event_tx))),
        "lfharness" => Ok(Box::new(lfharness::LfHarness::new(event_tx))),
        other => anyhow::bail!("unsupported session provider: {other}"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn supports_provider_includes_lfharness() {
        assert!(supports_provider("lfharness"));
    }

    #[test]
    fn default_create_harness_supports_lfharness() {
        let (tx, _rx) = broadcast::channel(16);
        let harness = default_create_harness("lfharness", tx).expect("lfharness should construct");
        drop(harness);
    }
}
