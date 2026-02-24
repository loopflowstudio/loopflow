pub mod claude;
pub mod codex;

use anyhow::Result;
use async_trait::async_trait;
use tokio::sync::broadcast;

use crate::lfd::sessions::types::{SessionConfig, SessionEvent};

#[derive(Debug, thiserror::Error)]
pub enum SessionAdapterError {
    #[error("turn already in progress")]
    TurnAlreadyInProgress,
}

pub fn is_turn_in_progress(err: &anyhow::Error) -> bool {
    matches!(
        err.downcast_ref::<SessionAdapterError>(),
        Some(SessionAdapterError::TurnAlreadyInProgress)
    )
}

#[async_trait]
pub trait SessionAdapter: Send + Sync {
    async fn start(&mut self, config: &SessionConfig) -> Result<()>;
    async fn send_input(&mut self, content: &str) -> Result<()>;
    async fn stop(&mut self) -> Result<()>;
    fn set_provider_session_id(&mut self, _provider_session_id: Option<String>) {}
}

/// Constructor fn: `(provider, event_tx) -> adapter`.
pub type CreateAdapterFn =
    fn(&str, broadcast::Sender<SessionEvent>) -> Result<Box<dyn SessionAdapter>>;

pub fn supports_provider(provider: &str) -> bool {
    matches!(provider, "codex" | "claude")
}

pub fn default_create_adapter(
    provider: &str,
    event_tx: broadcast::Sender<SessionEvent>,
) -> Result<Box<dyn SessionAdapter>> {
    match provider {
        "codex" => Ok(Box::new(codex::CodexAdapter::new(event_tx))),
        "claude" => Ok(Box::new(claude::ClaudeAdapter::new(event_tx))),
        other => anyhow::bail!("unsupported session provider: {other}"),
    }
}
