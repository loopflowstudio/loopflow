use std::collections::{HashMap, HashSet};
use std::sync::{Arc, Mutex};

use tokio::sync::{OwnedSemaphorePermit, Semaphore};
use tokio::task::JoinHandle;
use tokio_util::sync::CancellationToken;

use crate::lfd::config::GitHubConfig;
use crate::lfd::events::EventHub;
use crate::lfd::executor::WaveExecutor;
use crate::lfd::store::SharedStore;
use crate::lfd::triggers;

#[derive(Debug)]
pub struct SchedulerSlotGuard {
    scheduler: Arc<Scheduler>,
    run_id: String,
    release_on_drop: bool,
}

impl Drop for SchedulerSlotGuard {
    fn drop(&mut self) {
        if self.release_on_drop {
            self.scheduler.release(&self.run_id);
        }
    }
}

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

    pub async fn acquire_guard(
        self: &Arc<Self>,
        run_id: &str,
    ) -> Result<SchedulerSlotGuard, String> {
        let mut active = self.active.lock().expect("scheduler mutex poisoned");
        if active.contains_key(run_id) {
            return Ok(SchedulerSlotGuard {
                scheduler: self.clone(),
                run_id: run_id.to_string(),
                release_on_drop: false,
            });
        }
        if let Ok(permit) = self.semaphore.clone().try_acquire_owned() {
            active.insert(run_id.to_string(), permit);
            return Ok(SchedulerSlotGuard {
                scheduler: self.clone(),
                run_id: run_id.to_string(),
                release_on_drop: true,
            });
        }
        Err("no slots available".to_string())
    }

    fn release(&self, run_id: &str) -> u32 {
        let mut active = self.active.lock().expect("scheduler mutex poisoned");
        active.remove(run_id);
        active.len() as u32
    }

    #[allow(dead_code)] // Reserved for interactive session management.
    pub fn register_session(&self, wave_id: &str) -> bool {
        let mut sessions = self.sessions.lock().expect("scheduler mutex poisoned");
        if sessions.contains(wave_id) {
            return false;
        }
        sessions.insert(wave_id.to_string());
        true
    }

    #[allow(dead_code)] // Reserved for interactive session management.
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
        event_hub: EventHub,
        github: GitHubConfig,
        cancel: CancellationToken,
    ) -> Vec<JoinHandle<()>> {
        vec![
            triggers::spawn_ci_failure_handler(executor.clone(), event_hub.clone(), cancel.clone()),
            triggers::spawn_activation_dispatcher(
                store.clone(),
                executor.clone(),
                self.clone(),
                event_hub.clone(),
                cancel.clone(),
            ),
            triggers::spawn_loop_ticker(
                self.clone(),
                store.clone(),
                event_hub.clone(),
                cancel.clone(),
            ),
            triggers::spawn_watch_poller(store.clone(), event_hub.clone(), cancel.clone()),
            triggers::spawn_cron_poller(store.clone(), event_hub.clone(), cancel.clone()),
            triggers::spawn_queue_reconciler(store.clone(), github, cancel.clone()),
            triggers::spawn_recovery_loop(store.clone(), executor.clone(), cancel.clone()),
            triggers::spawn_summary_refresh(store, executor, event_hub, cancel),
        ]
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use super::Scheduler;

    #[tokio::test]
    async fn acquire_guard_releases_slot_on_drop() {
        let scheduler = Arc::new(Scheduler::new(1));
        let guard = scheduler
            .acquire_guard("run-1")
            .await
            .expect("first guard should acquire");
        assert_eq!(scheduler.slots_used(), 1);
        drop(guard);
        assert_eq!(scheduler.slots_used(), 0);
    }

    #[tokio::test]
    async fn acquire_guard_is_noop_when_slot_already_held_for_run() {
        let scheduler = Arc::new(Scheduler::new(1));
        let first = scheduler
            .acquire_guard("run-1")
            .await
            .expect("first guard should acquire");
        let second = scheduler
            .acquire_guard("run-1")
            .await
            .expect("second guard should be idempotent");
        assert_eq!(scheduler.slots_used(), 1);
        drop(second);
        assert_eq!(scheduler.slots_used(), 1);
        drop(first);
        assert_eq!(scheduler.slots_used(), 0);
    }

    #[tokio::test]
    async fn acquire_guard_respects_slot_limit() {
        let scheduler = Arc::new(Scheduler::new(1));
        let guard = scheduler
            .acquire_guard("run-1")
            .await
            .expect("first guard should acquire");
        assert!(scheduler.acquire_guard("run-2").await.is_err());
        drop(guard);
        scheduler
            .acquire_guard("run-2")
            .await
            .expect("should acquire after release");
    }

    #[test]
    fn sessions_enforce_single_active_wave() {
        let scheduler = Scheduler::new(1);

        assert!(scheduler.register_session("wave-1"));
        assert!(!scheduler.register_session("wave-1"));

        scheduler.unregister_session("wave-1");
        assert!(scheduler.register_session("wave-1"));
    }
}
