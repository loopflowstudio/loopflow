use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::sync::Mutex as StdMutex;

use time::OffsetDateTime;
use tokio::sync::broadcast;
use tokio::sync::Mutex;

use crate::chat::AgentEvent;
use crate::lfd::auth::AuthProvider;
use crate::lfd::config::GitHubConfig;
use crate::lfd::events::EventHub;
use crate::lfd::executor::WaveExecutor;
use crate::lfd::id::LfdId;
use crate::lfd::output::OutputHub;
use crate::lfd::registration::RegistrationClient;
use crate::lfd::scheduler::Scheduler;
use crate::lfd::store::SharedStore;

#[derive(Debug)]
pub struct ChatTurnStream {
    wave_id: LfdId,
    sender: broadcast::Sender<AgentEvent>,
    history: StdMutex<Vec<AgentEvent>>,
    completed: AtomicBool,
}

impl ChatTurnStream {
    fn new(wave_id: LfdId) -> Self {
        let (sender, _) = broadcast::channel(256);
        Self {
            wave_id,
            sender,
            history: StdMutex::new(Vec::new()),
            completed: AtomicBool::new(false),
        }
    }

    pub fn wave_id(&self) -> &LfdId {
        &self.wave_id
    }

    pub fn publish(&self, event: AgentEvent) {
        if let Ok(mut history) = self.history.lock() {
            history.push(event.clone());
        }
        let _ = self.sender.send(event);
    }

    pub fn mark_completed(&self) {
        self.completed.store(true, Ordering::Relaxed);
    }

    pub fn is_completed(&self) -> bool {
        self.completed.load(Ordering::Relaxed)
    }

    pub fn history(&self) -> Vec<AgentEvent> {
        self.history
            .lock()
            .map(|events| events.clone())
            .unwrap_or_default()
    }

    pub fn subscribe(&self) -> broadcast::Receiver<AgentEvent> {
        self.sender.subscribe()
    }
}

#[derive(Clone, Debug, Default)]
pub struct ChatTurnRegistry {
    turns: Arc<StdMutex<HashMap<String, Arc<ChatTurnStream>>>>,
}

impl ChatTurnRegistry {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn create(&self, wave_id: LfdId) -> (LfdId, Arc<ChatTurnStream>) {
        let turn_id = LfdId::new();
        let stream = Arc::new(ChatTurnStream::new(wave_id));

        if let Ok(mut turns) = self.turns.lock() {
            turns.insert(turn_id.to_string(), stream.clone());
        }

        (turn_id, stream)
    }

    pub fn get(&self, wave_id: &LfdId, turn_id: &LfdId) -> Option<Arc<ChatTurnStream>> {
        self.turns
            .lock()
            .ok()
            .and_then(|turns| turns.get(&turn_id.to_string()).cloned())
            .filter(|stream| stream.wave_id() == wave_id)
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
    pub ci_failure_cache: Arc<Mutex<std::collections::HashSet<String>>>,
    pub chat_turns: ChatTurnRegistry,
}
