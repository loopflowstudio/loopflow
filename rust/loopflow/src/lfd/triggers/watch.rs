use std::path::Path;
use std::sync::Arc;
use std::time::Duration;

use git2::Repository;
use tokio::task::JoinHandle;
use tokio_util::sync::CancellationToken;

use super::{enqueue_pending_activation, spawn_immediate_activation, ActivationEnvelope};
use crate::lfd::events::EventHub;
use crate::lfd::executor::WaveExecutor;
use crate::lfd::scheduler::Scheduler;
use crate::lfd::store::SharedStore;
use crate::lfd::types::{ActivationSource, Signal, Stimulus, Wave, WaveStatus};

pub fn spawn_watch_poller(
    store: SharedStore,
    executor: WaveExecutor,
    scheduler: Arc<Scheduler>,
    event_hub: EventHub,
    cancel: CancellationToken,
) -> JoinHandle<()> {
    tokio::spawn(async move {
        let mut interval = tokio::time::interval(Duration::from_secs(30));
        loop {
            tokio::select! {
                _ = cancel.cancelled() => {
                    tracing::info!("watch_poller shutting down");
                    break;
                }
                _ = interval.tick() => {
                    check_watch_stimuli(&store, &executor, &scheduler, &event_hub).await;
                }
            }
        }
    })
}

async fn check_watch_stimuli(
    store: &SharedStore,
    executor: &WaveExecutor,
    scheduler: &Arc<Scheduler>,
    event_hub: &EventHub,
) {
    let stimuli = match store.list_stimuli_by_signal(Signal::Watch.as_i32()).await {
        Ok(stimuli) => stimuli,
        Err(err) => {
            tracing::error!(error = %err, "failed to list watch stimuli");
            return;
        }
    };

    for stimulus in stimuli {
        if !stimulus.enabled {
            continue;
        }

        let wave = match store.get_wave(&stimulus.wave_id).await {
            Ok(Some(wave)) => wave,
            Ok(None) => continue,
            Err(err) => {
                tracing::error!(stimulus_id = %stimulus.id, error = %err, "failed to get wave");
                continue;
            }
        };

        if wave.status() == WaveStatus::Paused {
            continue;
        }

        match check_watch_stimulus(&wave, &stimulus) {
            Ok(result) => {
                if result.update_sha {
                    let mut stimulus = stimulus.clone();
                    stimulus.last_main_sha = Some(result.current_sha.clone());
                    let _ = store.update_stimulus(&stimulus).await;
                }

                if !result.trigger {
                    continue;
                }
                let reason = format!(
                    "origin/main advanced {}..{}",
                    result.from_sha, result.current_sha
                );
                let envelope = ActivationEnvelope::new(
                    &stimulus.wave_id,
                    &stimulus.id,
                    ActivationSource::Poll,
                    reason,
                    &result.from_sha,
                    &result.current_sha,
                    "main",
                );
                if wave.serialized {
                    let _ = enqueue_pending_activation(store, event_hub, envelope).await;
                } else {
                    let _ = spawn_immediate_activation(
                        store,
                        executor,
                        scheduler,
                        event_hub,
                        &wave,
                        stimulus.flow.clone(),
                        envelope,
                    )
                    .await;
                }
            }
            Err(err) => {
                tracing::warn!(wave_id = %wave.id(), stimulus_id = %stimulus.id, error = %err, "watch check failed");
            }
        }
    }
}

struct WatchCheck {
    from_sha: String,
    current_sha: String,
    trigger: bool,
    update_sha: bool,
}

fn check_watch_stimulus(wave: &Wave, stimulus: &Stimulus) -> Result<WatchCheck, git2::Error> {
    let repo = Repository::open(wave.repo())?;

    let mut remote = repo.find_remote("origin")?;
    remote.fetch(&["main"], None, None)?;

    let reference = repo.find_reference("refs/remotes/origin/main")?;
    let current_sha = reference.peel_to_commit()?.id().to_string();

    let last_sha = stimulus.last_main_sha.as_deref();
    if last_sha.is_none() {
        return Ok(WatchCheck {
            from_sha: String::new(),
            current_sha,
            trigger: false,
            update_sha: true,
        });
    }

    if Some(current_sha.as_str()) == last_sha {
        return Ok(WatchCheck {
            from_sha: String::new(),
            current_sha,
            trigger: false,
            update_sha: false,
        });
    }

    let prev = last_sha.unwrap_or("");
    let prev_oid = git2::Oid::from_str(prev)?;
    let curr_oid = git2::Oid::from_str(&current_sha)?;

    let prev_commit = repo.find_commit(prev_oid)?;
    let curr_commit = repo.find_commit(curr_oid)?;

    let prev_tree = prev_commit.tree()?;
    let curr_tree = curr_commit.tree()?;

    let diff = match repo.diff_tree_to_tree(Some(&prev_tree), Some(&curr_tree), None) {
        Ok(diff) => diff,
        Err(_) => {
            return Ok(WatchCheck {
                from_sha: prev.to_string(),
                current_sha,
                trigger: false,
                update_sha: true,
            })
        }
    };

    let area_match = if wave.area().is_empty() {
        true
    } else {
        diff.deltas().any(|delta| {
            let path = delta.new_file().path().unwrap_or(Path::new(""));
            wave.area().iter().any(|area| path.starts_with(area))
        })
    };

    Ok(WatchCheck {
        from_sha: prev.to_string(),
        current_sha,
        trigger: area_match,
        update_sha: true,
    })
}
