use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use tokio::sync::{OwnedSemaphorePermit, Semaphore};
use tokio::task::JoinHandle;
use tokio_util::sync::CancellationToken;

use crate::loops;
use crate::store::SharedStore;

#[derive(Debug)]
pub struct Scheduler {
    max_slots: usize,
    semaphore: Arc<Semaphore>,
    active: Mutex<HashMap<String, OwnedSemaphorePermit>>,
}

impl Scheduler {
    pub fn new(max_slots: usize) -> Self {
        Self {
            max_slots,
            semaphore: Arc::new(Semaphore::new(max_slots)),
            active: Mutex::new(HashMap::new()),
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

    pub fn start_loops(
        self: Arc<Self>,
        store: SharedStore,
        cancel: CancellationToken,
    ) -> Vec<JoinHandle<()>> {
        vec![
            loops::spawn_loop_ticker(self.clone(), store.clone(), cancel.clone()),
            loops::spawn_watch_poller(store.clone(), cancel.clone()),
            loops::spawn_cron_poller(store.clone(), cancel.clone()),
            loops::spawn_recovery_loop(store, cancel),
        ]
    }
}
