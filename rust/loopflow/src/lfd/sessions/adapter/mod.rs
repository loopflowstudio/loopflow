pub mod codex;

use std::sync::Arc;

use anyhow::Result;
use async_trait::async_trait;
use tokio::sync::broadcast;

use crate::lfd::sessions::types::{SessionConfig, SessionEvent};

#[async_trait]
pub trait SessionAdapter: Send + Sync {
    async fn start(&mut self, config: &SessionConfig) -> Result<()>;
    async fn send_input(&mut self, content: &str) -> Result<()>;
    async fn stop(&mut self) -> Result<()>;
}

pub trait SessionAdapterFactory: Send + Sync {
    fn create(
        &self,
        provider: &str,
        event_tx: broadcast::Sender<SessionEvent>,
    ) -> Result<Box<dyn SessionAdapter>>;
}

#[derive(Debug, Default)]
pub struct DefaultSessionAdapterFactory;

impl SessionAdapterFactory for DefaultSessionAdapterFactory {
    fn create(
        &self,
        provider: &str,
        event_tx: broadcast::Sender<SessionEvent>,
    ) -> Result<Box<dyn SessionAdapter>> {
        match provider {
            "codex" => Ok(Box::new(codex::CodexAdapter::new(event_tx))),
            other => anyhow::bail!("unsupported session provider: {other}"),
        }
    }
}

pub type SharedSessionAdapterFactory = Arc<dyn SessionAdapterFactory>;
