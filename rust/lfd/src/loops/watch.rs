use std::path::Path;
use std::time::Duration;

use crate::id::LfdId;
use crate::proto::control::{PendingActivation, Stimulus, StimulusKind, Wave, WaveStatus};
use crate::store::SharedStore;
use chrono::Utc;
use git2::Repository;
use tokio::task::JoinHandle;
use tokio_util::sync::CancellationToken;

pub fn spawn_watch_poller(store: SharedStore, cancel: CancellationToken) -> JoinHandle<()> {
    tokio::spawn(async move {
        let mut interval = tokio::time::interval(Duration::from_secs(30));
        loop {
            tokio::select! {
                _ = cancel.cancelled() => {
                    tracing::info!("watch_poller shutting down");
                    break;
                }
                _ = interval.tick() => {
                    check_watch_stimuli(&store);
                }
            }
        }
    })
}

fn check_watch_stimuli(store: &SharedStore) {
    let stimuli = match store.list_stimuli_by_kind(StimulusKind::StimulusWatch as i32) {
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

        // Get the wave for this stimulus
        let wave_id = match LfdId::parse(&stimulus.wave_id) {
            Ok(id) => id,
            Err(err) => {
                tracing::warn!(stimulus_id = %stimulus.id, error = %err, "invalid wave id");
                continue;
            }
        };
        let wave = match store.get_wave(&wave_id) {
            Ok(Some(wave)) => wave,
            Ok(None) => {
                tracing::warn!(stimulus_id = %stimulus.id, "stimulus references missing wave");
                continue;
            }
            Err(err) => {
                tracing::error!(stimulus_id = %stimulus.id, error = %err, "failed to get wave");
                continue;
            }
        };

        if wave.paused {
            continue;
        }

        match check_watch_stimulus(&wave, &stimulus) {
            Ok(result) => {
                if !result.update_sha && !result.trigger {
                    continue;
                }

                // Update stimulus.last_main_sha
                if result.update_sha {
                    let mut stimulus = stimulus.clone();
                    stimulus.last_main_sha = Some(result.current_sha.clone());
                    if let Err(err) = store.update_stimulus(&stimulus) {
                        tracing::error!(stimulus_id = %stimulus.id, error = %err, "failed to update stimulus");
                        continue;
                    }
                }

                if result.trigger {
                    if wave.status == WaveStatus::WaveRunning as i32
                        || wave.status == WaveStatus::WaveWaiting as i32
                    {
                        // Wave is busy - queue with SHA range for coalescing
                        let stimulus_id = LfdId::parse(&stimulus.id)
                            .unwrap_or_else(|_| LfdId::from_raw(stimulus.id.clone()));
                        queue_or_coalesce_activation(
                            store,
                            &wave_id,
                            &stimulus_id,
                            &result.from_sha,
                            &result.current_sha,
                        );
                        tracing::debug!(wave_id = %wave.id, stimulus_id = %stimulus.id, "watch: queued activation");
                    } else if wave.status == WaveStatus::WaveIdle as i32 {
                        // Activate the wave
                        let mut wave = wave.clone();
                        wave.status = WaveStatus::WaveRunning as i32;
                        if let Err(err) = store.update_wave(&wave) {
                            tracing::error!(wave_id = %wave.id, error = %err, "failed to activate wave");
                            continue;
                        }
                        tracing::info!(wave_id = %wave.id, stimulus_id = %stimulus.id, "watch: activated");
                    }
                }
            }
            Err(err) => {
                tracing::warn!(wave_id = %wave.id, stimulus_id = %stimulus.id, error = %err, "watch check failed");
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
    let repo = Repository::open(&wave.repo)?;

    let mut remote = repo.find_remote("origin")?;
    remote.fetch(&["main"], None, None)?;

    let reference = repo.find_reference("refs/remotes/origin/main")?;
    let current_sha = reference.peel_to_commit()?.id().to_string();

    // Use stimulus.last_main_sha for tracking
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

    let prev = match last_sha {
        Some(value) => value,
        None => {
            return Ok(WatchCheck {
                from_sha: String::new(),
                current_sha,
                trigger: false,
                update_sha: true,
            })
        }
    };

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

    // Check if diff touches wave.area
    let area_match = if wave.area.is_empty() {
        true
    } else {
        diff.deltas().any(|delta| {
            let path = delta.new_file().path().unwrap_or(Path::new(""));
            wave.area.iter().any(|area| path.starts_with(area))
        })
    };

    Ok(WatchCheck {
        from_sha: prev.to_string(),
        current_sha,
        trigger: area_match,
        update_sha: true,
    })
}

fn queue_or_coalesce_activation(
    store: &SharedStore,
    wave_id: &LfdId,
    stimulus_id: &LfdId,
    from_sha: &str,
    to_sha: &str,
) {
    // Check if there's already a pending activation for this stimulus
    match store.get_pending_for_stimulus(wave_id, stimulus_id) {
        Ok(Some(mut existing)) => {
            // Extend the SHA range (coalesce) - keep original from_sha, update to_sha
            existing.to_sha = to_sha.to_string();
            if let Err(err) = store.update_pending_activation(&existing) {
                tracing::error!(wave_id = %wave_id, error = %err, "failed to update pending activation");
            }
        }
        Ok(None) => {
            // Create new pending activation
            let activation = PendingActivation {
                id: LfdId::new().to_string(),
                wave_id: wave_id.to_string(),
                stimulus_id: stimulus_id.to_string(),
                from_sha: from_sha.to_string(),
                to_sha: to_sha.to_string(),
                queued_at: Utc::now().timestamp(),
            };
            if let Err(err) = store.create_pending_activation(&activation) {
                tracing::error!(wave_id = %wave_id, error = %err, "failed to queue activation");
            }
        }
        Err(err) => {
            tracing::error!(wave_id = %wave_id, error = %err, "failed to check pending activation");
        }
    }
}
