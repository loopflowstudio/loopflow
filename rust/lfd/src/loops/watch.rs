use std::path::Path;
use std::time::Duration;

use git2::Repository;
use tokio::task::JoinHandle;
use tokio_util::sync::CancellationToken;

use crate::proto::control::{StimulusKind, Wave, WaveStatus};
use crate::store::SharedStore;

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
                    check_watch_waves(&store);
                }
            }
        }
    })
}

fn check_watch_waves(store: &SharedStore) {
    let waves = match store.list_waves_by_stimulus(StimulusKind::StimulusWatch as i32) {
        Ok(waves) => waves,
        Err(err) => {
            tracing::error!(error = %err, "failed to list watch waves");
            return;
        }
    };

    for wave in waves {
        if wave.paused {
            continue;
        }

        match check_watch_stimulus(&wave) {
            Ok(result) => {
                if !result.update_sha && !result.trigger {
                    continue;
                }

                let mut wave = wave.clone();
                if result.update_sha {
                    wave.last_main_sha = Some(result.current_sha);
                }

                if result.trigger {
                    if wave.status == WaveStatus::WaveRunning as i32
                        || wave.status == WaveStatus::WaveWaiting as i32
                    {
                        if let Ok(count) = store.increment_pending_activations(&wave.id) {
                            wave.pending_activations = count;
                        }
                        tracing::debug!(wave_id = %wave.id, "watch: queued activation");
                    } else if wave.status == WaveStatus::WaveIdle as i32 {
                        wave.status = WaveStatus::WaveRunning as i32;
                        let _ = store.update_wave(&wave);
                        tracing::info!(wave_id = %wave.id, "watch: activated");
                        continue;
                    } else {
                        let _ = store.update_wave(&wave);
                        continue;
                    }
                }

                let _ = store.update_wave(&wave);
            }
            Err(err) => {
                tracing::warn!(wave_id = %wave.id, error = %err, "watch check failed");
            }
        }
    }
}

struct WatchCheck {
    current_sha: String,
    trigger: bool,
    update_sha: bool,
}

fn check_watch_stimulus(wave: &Wave) -> Result<WatchCheck, git2::Error> {
    let repo = Repository::open(&wave.repo)?;

    let mut remote = repo.find_remote("origin")?;
    remote.fetch(&["main"], None, None)?;

    let reference = repo.find_reference("refs/remotes/origin/main")?;
    let current_sha = reference.peel_to_commit()?.id().to_string();

    let last_sha = wave.last_main_sha.as_deref();
    if last_sha.is_none() {
        return Ok(WatchCheck {
            current_sha,
            trigger: false,
            update_sha: true,
        });
    }

    if Some(current_sha.as_str()) == last_sha {
        return Ok(WatchCheck {
            current_sha,
            trigger: false,
            update_sha: false,
        });
    }

    let prev = match last_sha {
        Some(value) => value,
        None => {
            return Ok(WatchCheck {
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
                current_sha,
                trigger: false,
                update_sha: true,
            })
        }
    };

    let area_match = if wave.area.is_empty() {
        true
    } else {
        diff.deltas().any(|delta| {
            let path = delta.new_file().path().unwrap_or(Path::new(""));
            wave.area.iter().any(|area| path.starts_with(area))
        })
    };

    Ok(WatchCheck {
        current_sha,
        trigger: area_match,
        update_sha: true,
    })
}
