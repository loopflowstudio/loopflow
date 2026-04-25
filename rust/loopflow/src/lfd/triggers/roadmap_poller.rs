//! Roadmap poller — watches PM-configured waves discovered in the
//! filesystem and emits `RoadmapUpdated` events when their Asana state
//! changes. Operates independently of lfd's wave store: even unmanaged
//! waves get polled, so Concerto can show live roadmap state for
//! anything in the codebase.

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

use sha2::{Digest, Sha256};
use tokio::sync::Mutex;
use tokio::task::JoinHandle;
use tokio_util::sync::CancellationToken;

use crate::lfd::events::EventHub;
use crate::lfd::id::LfdId;
use crate::lfd::store::SharedStore;
use crate::lfd::types::Event;
use crate::ops::pm::{discover_waves, fetch_roadmap, RoadmapResult};
use crate::ops::NullProgress;

const BASE_INTERVAL: Duration = Duration::from_secs(60);
const ERROR_BACKOFF_MULTIPLIER: u32 = 5;
const MAX_BACKOFF_TICKS: u32 = ERROR_BACKOFF_MULTIPLIER;

/// State tracked per (repo_path, wave_name) across poller ticks.
#[derive(Debug, Default)]
struct WaveState {
    /// Hash of the last successful fetch so we only emit on change.
    last_hash: Option<String>,
    /// Number of consecutive ticks to skip (set after an error).
    backoff_remaining: u32,
}

#[derive(Default)]
struct PollerState {
    waves: HashMap<(PathBuf, String), WaveState>,
}

pub fn spawn_roadmap_poller(
    store: SharedStore,
    event_hub: EventHub,
    cancel: CancellationToken,
) -> JoinHandle<()> {
    spawn_roadmap_poller_with_interval(store, event_hub, cancel, BASE_INTERVAL)
}

pub(crate) fn spawn_roadmap_poller_with_interval(
    store: SharedStore,
    event_hub: EventHub,
    cancel: CancellationToken,
    interval: Duration,
) -> JoinHandle<()> {
    let state = Arc::new(Mutex::new(PollerState::default()));
    tokio::spawn(async move {
        let mut ticker = tokio::time::interval(interval);
        ticker.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);
        loop {
            tokio::select! {
                _ = cancel.cancelled() => {
                    tracing::info!("roadmap_poller shutting down");
                    break;
                }
                _ = ticker.tick() => {
                    poll_once(&store, &event_hub, &state).await;
                }
            }
        }
    })
}

async fn poll_once(store: &SharedStore, event_hub: &EventHub, state: &Arc<Mutex<PollerState>>) {
    let repos = match store.list_repos().await {
        Ok(repos) => repos,
        Err(err) => {
            tracing::warn!(error = %err, "roadmap_poller: list_repos failed");
            return;
        }
    };

    // Build the wave-id index once so emitted events can carry it when
    // the wave is also being managed.
    let managed = match store.list_waves(None).await {
        Ok(waves) => waves,
        Err(err) => {
            tracing::warn!(error = %err, "roadmap_poller: list_waves failed");
            Vec::new()
        }
    };
    let managed_index: HashMap<(String, String), LfdId> = managed
        .iter()
        .map(|w| ((w.repo().clone(), w.name().clone()), w.id().clone()))
        .collect();

    let mut discovered_keys: Vec<(PathBuf, String)> = Vec::new();
    for repo in repos {
        let repo_path = PathBuf::from(&repo.path);
        let waves = match tokio::task::spawn_blocking({
            let repo_path = repo_path.clone();
            move || discover_waves(&repo_path)
        })
        .await
        {
            Ok(Ok(waves)) => waves,
            Ok(Err(err)) => {
                tracing::warn!(repo = %repo.path, error = %err, "discover_waves failed");
                continue;
            }
            Err(err) => {
                tracing::warn!(repo = %repo.path, error = %err, "discover_waves task panicked");
                continue;
            }
        };

        for wave in waves {
            // Asana project must be set before we can poll for items.
            if wave.project_id.is_none() {
                continue;
            }
            let key = (repo_path.clone(), wave.wave_name.clone());
            discovered_keys.push(key.clone());

            let should_poll = {
                let mut guard = state.lock().await;
                let entry = guard.waves.entry(key.clone()).or_default();
                if entry.backoff_remaining > 0 {
                    entry.backoff_remaining -= 1;
                    false
                } else {
                    true
                }
            };
            if !should_poll {
                continue;
            }

            let wave_name = wave.wave_name.clone();
            let repo_for_fetch = repo_path.clone();
            let fetch = tokio::task::spawn_blocking(move || {
                fetch_roadmap(&repo_for_fetch, &wave_name, &NullProgress)
            })
            .await;

            match fetch {
                Ok(Ok(roadmap)) => {
                    let new_hash = roadmap_hash(&roadmap);
                    let mut guard = state.lock().await;
                    let entry = guard.waves.entry(key.clone()).or_default();
                    let changed = entry.last_hash.as_deref() != Some(new_hash.as_str());
                    entry.last_hash = Some(new_hash);
                    entry.backoff_remaining = 0;
                    drop(guard);

                    if changed {
                        let wave_id = managed_index
                            .get(&(repo.path.clone(), wave.wave_name.clone()))
                            .cloned();
                        event_hub.send(Event::roadmap_updated(
                            repo.path.clone(),
                            wave.wave_name.clone(),
                            wave_id,
                        ));
                    }
                }
                Ok(Err(err)) => {
                    tracing::warn!(
                        repo = %repo.path,
                        wave = %wave.wave_name,
                        error = %err,
                        "fetch_roadmap failed; backing off"
                    );
                    let mut guard = state.lock().await;
                    let entry = guard.waves.entry(key).or_default();
                    entry.backoff_remaining = MAX_BACKOFF_TICKS;
                }
                Err(err) => {
                    tracing::warn!(
                        repo = %repo.path,
                        wave = %wave.wave_name,
                        error = %err,
                        "fetch_roadmap task panicked"
                    );
                    let mut guard = state.lock().await;
                    let entry = guard.waves.entry(key).or_default();
                    entry.backoff_remaining = MAX_BACKOFF_TICKS;
                }
            }
        }
    }

    // Drop state for waves that vanished from discovery. Avoids unbounded
    // growth if waves are frequently created and deleted.
    let mut guard = state.lock().await;
    let live: std::collections::HashSet<_> = discovered_keys.into_iter().collect();
    guard.waves.retain(|key, _| live.contains(key));
}

fn roadmap_hash(roadmap: &RoadmapResult) -> String {
    let mut hasher = Sha256::new();
    for task in &roadmap.tasks {
        hasher.update(task.asana_id.as_bytes());
        hasher.update([0u8]);
        hasher.update(task.title.as_bytes());
        hasher.update([0u8]);
        hasher.update(task.description.as_bytes());
        hasher.update([0u8]);
        hasher.update([task.priority as u8]);
        hasher.update([task.completed as u8]);
        hasher.update([0xffu8]);
    }
    format!("{:x}", hasher.finalize())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::lfd::pm::{PmProviderKind, PriorityBucket};
    use crate::ops::pm::RoadmapTask;

    fn task(id: &str, title: &str, priority: PriorityBucket, completed: bool) -> RoadmapTask {
        RoadmapTask {
            asana_id: id.to_string(),
            title: title.to_string(),
            description: String::new(),
            priority,
            completed,
            local_filename: None,
        }
    }

    fn roadmap(tasks: Vec<RoadmapTask>) -> RoadmapResult {
        RoadmapResult {
            wave: "test".to_string(),
            provider: PmProviderKind::Asana,
            tasks,
        }
    }

    #[test]
    fn roadmap_hash_is_stable_for_identical_inputs() {
        let a = roadmap(vec![task("t-1", "Alpha", PriorityBucket::High, false)]);
        let b = roadmap(vec![task("t-1", "Alpha", PriorityBucket::High, false)]);
        assert_eq!(roadmap_hash(&a), roadmap_hash(&b));
    }

    #[test]
    fn roadmap_hash_changes_when_task_added() {
        let a = roadmap(vec![task("t-1", "Alpha", PriorityBucket::High, false)]);
        let b = roadmap(vec![
            task("t-1", "Alpha", PriorityBucket::High, false),
            task("t-2", "Beta", PriorityBucket::Medium, false),
        ]);
        assert_ne!(roadmap_hash(&a), roadmap_hash(&b));
    }

    #[test]
    fn roadmap_hash_changes_when_task_completes() {
        let pending = roadmap(vec![task("t-1", "Alpha", PriorityBucket::High, false)]);
        let done = roadmap(vec![task("t-1", "Alpha", PriorityBucket::High, true)]);
        assert_ne!(roadmap_hash(&pending), roadmap_hash(&done));
    }

    #[test]
    fn roadmap_hash_changes_when_priority_shifts() {
        let high = roadmap(vec![task("t-1", "Alpha", PriorityBucket::High, false)]);
        let low = roadmap(vec![task("t-1", "Alpha", PriorityBucket::Low, false)]);
        assert_ne!(roadmap_hash(&high), roadmap_hash(&low));
    }

    #[test]
    fn roadmap_hash_changes_when_title_edited() {
        let before = roadmap(vec![task("t-1", "Alpha", PriorityBucket::High, false)]);
        let after = roadmap(vec![task(
            "t-1",
            "Alpha (renamed)",
            PriorityBucket::High,
            false,
        )]);
        assert_ne!(roadmap_hash(&before), roadmap_hash(&after));
    }
}
