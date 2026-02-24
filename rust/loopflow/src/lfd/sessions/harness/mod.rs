pub mod claude;
pub mod codex;

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
    matches!(provider, "codex" | "claude")
}

pub fn default_create_harness(
    provider: &str,
    event_tx: broadcast::Sender<SessionEvent>,
) -> Result<Box<dyn SessionHarness>> {
    match provider {
        "codex" => Ok(Box::new(codex::CodexHarness::new(event_tx))),
        "claude" => Ok(Box::new(claude::ClaudeHarness::new(event_tx))),
        other => anyhow::bail!("unsupported session provider: {other}"),
    }
}
