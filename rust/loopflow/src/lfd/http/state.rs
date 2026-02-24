use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::sync::Mutex as StdMutex;

use time::OffsetDateTime;
use tokio::sync::broadcast;
use tokio::sync::Mutex;

use crate::chat::AgentEvent;
use crate::lfd::auth::{AuthFailureThrottle, AuthProvider};
use crate::lfd::config::{GitHubConfig, HttpSecurityConfig};
use crate::lfd::events::EventHub;
use crate::lfd::executor::WaveExecutor;
use crate::lfd::id::LfdId;
use crate::lfd::output::OutputHub;
use crate::lfd::registration::RegistrationClient;
use crate::lfd::scheduler::Scheduler;
use crate::lfd::sessions::SessionManager;
use crate::lfd::store::SharedStore;

#[derive(Debug)]
pub struct ChatTurnStream {
    sender: broadcast::Sender<AgentEvent>,
    completed: AtomicBool,
}

impl ChatTurnStream {
    fn new() -> Self {
        let (sender, _) = broadcast::channel(256);
        Self {
            sender,
            completed: AtomicBool::new(false),
        }
    }

    pub fn publish(&self, event: AgentEvent) {
        let _ = self.sender.send(event);
    }

    pub fn mark_completed(&self) {
        self.completed.store(true, Ordering::Relaxed);
    }

    pub fn is_completed(&self) -> bool {
        self.completed.load(Ordering::Relaxed)
    }

    pub fn subscribe(&self) -> broadcast::Receiver<AgentEvent> {
        self.sender.subscribe()
    }
}

#[derive(Clone, Debug, Default)]
pub struct ChatTurnRegistry {
    streams: Arc<StdMutex<HashMap<LfdId, Arc<ChatTurnStream>>>>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
#[non_exhaustive]
pub enum ChatTurnStartError {
    #[error("chat turn already running")]
    AlreadyRunning,
    #[error("chat turn registry unavailable")]
    Unavailable,
}

impl ChatTurnRegistry {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn start_for_wave(
        &self,
        wave_id: LfdId,
    ) -> Result<Arc<ChatTurnStream>, ChatTurnStartError> {
        let mut streams = self
            .streams
            .lock()
            .map_err(|_| ChatTurnStartError::Unavailable)?;

        if let Some(existing) = streams.get(&wave_id) {
            if !existing.is_completed() {
                return Err(ChatTurnStartError::AlreadyRunning);
            }
        }

        let stream = Arc::new(ChatTurnStream::new());
        streams.insert(wave_id, stream.clone());
        Ok(stream)
    }

    pub fn get(&self, wave_id: &LfdId) -> Option<Arc<ChatTurnStream>> {
        self.streams
            .lock()
            .ok()
            .and_then(|streams| streams.get(wave_id).cloned())
    }
}

#[derive(Clone)]
pub struct HttpState {
    pub store: SharedStore,
    pub scheduler: Arc<Scheduler>,
    pub executor: Arc<WaveExecutor>,
    pub event_hub: EventHub,
    #[allow(dead_code)] // Reserved for output streaming endpoints.
    pub output_hub: OutputHub,
    pub auth: AuthProvider,
    pub session_token: Option<String>,
    pub registration: Option<RegistrationClient>,
    pub started_at: OffsetDateTime,
    pub github: GitHubConfig,
    pub http_security: HttpSecurityConfig,
    pub auth_failure_throttle: AuthFailureThrottle,
    pub ci_failure_cache: Arc<Mutex<std::collections::HashSet<String>>>,
    pub chat_turns: ChatTurnRegistry,
    pub sessions: SessionManager,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn start_for_wave_rejects_concurrent_turns() {
        let registry = ChatTurnRegistry::new();
        let wave_id = LfdId::from_raw("wave-1");

        let _first = registry
            .start_for_wave(wave_id.clone())
            .expect("first turn should start");
        let second = registry.start_for_wave(wave_id);
        assert!(matches!(second, Err(ChatTurnStartError::AlreadyRunning)));
    }

    #[test]
    fn start_for_wave_allows_new_turn_after_completion() {
        let registry = ChatTurnRegistry::new();
        let wave_id = LfdId::from_raw("wave-1");

        let first = registry
            .start_for_wave(wave_id.clone())
            .expect("first turn should start");
        first.mark_completed();

        let second = registry
            .start_for_wave(wave_id)
            .expect("completed turn should be replaceable");
        assert!(!Arc::ptr_eq(&first, &second));
    }
}
