//! Supervision of subagent runs.
//!
//! Subagents form a tree with *independent lifetimes*. There are two spawn
//! sources — the always-running progress arm, and the server reacting to an
//! event — and a subagent may spawn its own children. A parent finishing must
//! never kill a still-working child, so the supervisor does not model
//! parent/child ownership as task ownership: every run is its own detached
//! tokio task, tracked here only so the server can report how many are alive
//! and abort *all* of them on shutdown.
//!
//! This is deliberately a flat registry, not a one-at-a-time bounded pass: the
//! set can hold many concurrent runs regardless of who spawned them.

use std::future::Future;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};

use tokio::task::JoinHandle;

/// Identifies one subagent run within a running server.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct SubagentId(pub u64);

impl std::fmt::Display for SubagentId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "sub-{}", self.0)
    }
}

#[derive(Debug)]
struct Entry {
    id: SubagentId,
    label: String,
    handle: JoinHandle<()>,
}

/// Tracks every live subagent run, independent of who spawned it.
#[derive(Debug)]
pub struct Supervisor {
    next_id: AtomicU64,
    running: Mutex<Vec<Entry>>,
}

impl Supervisor {
    pub fn new() -> Arc<Self> {
        Arc::new(Self {
            next_id: AtomicU64::new(1),
            running: Mutex::new(Vec::new()),
        })
    }

    /// Spawn a run as a detached task and register it. The returned id is
    /// stable for the life of the run. The task's lifetime is independent of
    /// its spawner — dropping or finishing whoever called `spawn` does not
    /// touch this task.
    pub fn spawn<F>(&self, label: impl Into<String>, fut: F) -> SubagentId
    where
        F: Future<Output = ()> + Send + 'static,
    {
        let id = SubagentId(self.next_id.fetch_add(1, Ordering::SeqCst));
        let handle = tokio::spawn(fut);
        self.running
            .lock()
            .expect("supervisor lock poisoned")
            .push(Entry {
                id,
                label: label.into(),
                handle,
            });
        id
    }

    /// Drop finished runs from the registry. Returns the count still alive.
    pub fn reap(&self) -> usize {
        let mut running = self.running.lock().expect("supervisor lock poisoned");
        running.retain(|e| !e.handle.is_finished());
        running.len()
    }

    /// How many runs are currently tracked (alive or not-yet-reaped).
    pub fn count(&self) -> usize {
        self.running.lock().expect("supervisor lock poisoned").len()
    }

    /// Labels of the tracked runs, for status reporting.
    pub fn labels(&self) -> Vec<String> {
        self.running
            .lock()
            .expect("supervisor lock poisoned")
            .iter()
            .map(|e| format!("{}:{}", e.id, e.label))
            .collect()
    }

    /// Abort every run. Called on server shutdown — the one place a run's
    /// lifetime is ended from outside itself.
    pub fn shutdown_all(&self) {
        let mut running = self.running.lock().expect("supervisor lock poisoned");
        for entry in running.drain(..) {
            entry.handle.abort();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    #[tokio::test]
    async fn tracks_and_reaps_runs() {
        let sup = Supervisor::new();
        sup.spawn("quick", async {});
        let slow = sup.spawn("slow", async {
            tokio::time::sleep(Duration::from_secs(30)).await;
        });
        assert_eq!(slow, SubagentId(2));
        assert_eq!(sup.count(), 2);

        // Give the quick task a chance to finish, then reap it.
        tokio::time::sleep(Duration::from_millis(20)).await;
        assert_eq!(sup.reap(), 1);

        sup.shutdown_all();
        assert_eq!(sup.count(), 0);
    }

    #[tokio::test]
    async fn child_outlives_parent() {
        let sup = Supervisor::new();
        let done = Arc::new(std::sync::atomic::AtomicBool::new(false));
        let child_done = done.clone();
        let sup2 = sup.clone();

        // Parent spawns a child, then returns immediately. The child keeps
        // running and completes on its own — proving lifetimes are independent.
        sup.spawn("parent", async move {
            sup2.spawn("child", async move {
                tokio::time::sleep(Duration::from_millis(30)).await;
                child_done.store(true, Ordering::SeqCst);
            });
        });

        tokio::time::sleep(Duration::from_millis(80)).await;
        assert!(
            done.load(Ordering::SeqCst),
            "child ran to completion after parent ended"
        );
    }
}
