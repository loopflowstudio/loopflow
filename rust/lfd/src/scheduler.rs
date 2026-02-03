use std::collections::{HashMap, HashSet};
use std::sync::{Arc, Mutex};

use tokio::sync::{OwnedSemaphorePermit, Semaphore};
use tokio::task::JoinHandle;
use tokio_util::sync::CancellationToken;

use crate::executor::WaveExecutor;
use crate::loops;
use crate::store::SharedStore;

#[derive(Debug)]
pub struct Scheduler {
    max_slots: usize,
    semaphore: Arc<Semaphore>,
    active: Mutex<HashMap<String, OwnedSemaphorePermit>>,
    sessions: Mutex<HashSet<String>>,
}

impl Scheduler {
    pub fn new(max_slots: usize) -> Self {
        Self {
            max_slots,
            semaphore: Arc::new(Semaphore::new(max_slots)),
            active: Mutex::new(HashMap::new()),
            sessions: Mutex::new(HashSet::new()),
        }
    }

    pub fn max_slots(&self) -> usize {
        self.max_slots
    }

    pub fn slots_used(&self) -> u32 {
        let active = self.active.lock().expect("scheduler mutex poisoned");
        active.len() as u32
    }

    pub async fn acquire(&self, run_id: &str) -> (bool, Option<String>) {
        let mut active = self.active.lock().expect("scheduler mutex poisoned");
        if active.contains_key(run_id) {
            return (true, None);
        }
        if let Ok(permit) = self.semaphore.clone().try_acquire_owned() {
            active.insert(run_id.to_string(), permit);
            return (true, None);
        }
        (false, Some("no slots available".to_string()))
    }

    pub fn release(&self, run_id: &str) -> u32 {
        let mut active = self.active.lock().expect("scheduler mutex poisoned");
        active.remove(run_id);
        active.len() as u32
    }

    pub fn register_session(&self, wave_id: &str) -> bool {
        let mut sessions = self.sessions.lock().expect("scheduler mutex poisoned");
        if sessions.contains(wave_id) {
            return false;
        }
        sessions.insert(wave_id.to_string());
        true
    }

    pub fn unregister_session(&self, wave_id: &str) {
        let mut sessions = self.sessions.lock().expect("scheduler mutex poisoned");
        sessions.remove(wave_id);
    }

    pub fn has_active_session(&self, wave_id: &str) -> bool {
        let sessions = self.sessions.lock().expect("scheduler mutex poisoned");
        sessions.contains(wave_id)
    }

    pub fn start_loops(
        self: Arc<Self>,
        store: SharedStore,
        executor: WaveExecutor,
        cancel: CancellationToken,
    ) -> Vec<JoinHandle<()>> {
        vec![
            loops::spawn_loop_ticker(
                self.clone(),
                store.clone(),
                executor.clone(),
                cancel.clone(),
            ),
            loops::spawn_watch_poller(store.clone(), executor.clone(), cancel.clone()),
            loops::spawn_cron_poller(store.clone(), executor.clone(), cancel.clone()),
            loops::spawn_recovery_loop(store, executor, cancel),
        ]
    }
}
