use std::collections::HashSet;
use std::path::Path;
use std::time::Duration;

use chrono::Utc;
use git2::Repository;
use tokio::task::JoinHandle;
use tokio_util::sync::CancellationToken;

use super::common::{create_wave_run_with_id, spawn_run_task_with_slot};
use crate::executor::WaveExecutor;
use crate::id::LfdId;
use crate::proto::control::{PendingActivation, Stimulus, StimulusKind};
use crate::scheduler::Scheduler;
use crate::store::SharedStore;

pub fn spawn_watch_poller(
    store: SharedStore,
    executor: WaveExecutor,
    scheduler: std::sync::Arc<Scheduler>,
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
                    check_watch_stimuli(&store, &executor, &scheduler).await;
                }
            }
        }
    })
}

async fn check_watch_stimuli(
    store: &SharedStore,
    executor: &WaveExecutor,
    scheduler: &std::sync::Arc<Scheduler>,
) {
    let stimuli = match store.list_stimuli_by_kind(StimulusKind::StimulusWatch as i32) {
        Ok(stimuli) => stimuli,
        Err(err) => {
            tracing::error!(error = %err, "failed to list watch stimuli");
            return;
        }
    };

    let mut started = HashSet::new();

    for stimulus in stimuli {
        if !stimulus.enabled {
            continue;
        }

        let wave_id = match LfdId::parse(&stimulus.wave_id) {
            Ok(id) => id,
            Err(err) => {
                tracing::warn!(stimulus_id = %stimulus.id, error = %err, "invalid wave id");
                continue;
            }
        };
        let wave = match store.get_wave(&wave_id) {
            Ok(Some(wave)) => wave,
            Ok(None) => continue,
            Err(err) => {
                tracing::error!(stimulus_id = %stimulus.id, error = %err, "failed to get wave");
                continue;
            }
        };

        if wave.paused {
            continue;
        }

        if started.contains(&wave.id) {
            continue;
        }

        if store.get_active_wave_run(&wave_id).ok().flatten().is_none() {
            if let Ok(pending) = store.list_pending_activations(&wave_id) {
                if !pending.is_empty() {
                    let run_id = LfdId::new();
                    let (acquired, reason) = scheduler.acquire(run_id.as_str()).await;
                    if !acquired {
                        tracing::warn!(
                            wave_id = %wave.id,
                            reason = ?reason,
                            "scheduler at capacity; watch activation deferred"
                        );
                        continue;
                    }

                    if let Ok(run) = create_wave_run_with_id(store, &wave, &run_id) {
                        let _ = store.delete_pending_activations(&wave_id);
                        started.insert(wave.id.clone());
                        spawn_run_task_with_slot(
                            store.clone(),
                            executor.clone(),
                            scheduler.clone(),
                            run,
                        );
                        continue;
                    }

                    scheduler.release(run_id.as_str());
                }
            }
        }

        match check_watch_stimulus(&wave, &stimulus) {
            Ok(result) => {
                if result.update_sha {
                    let mut stimulus = stimulus.clone();
                    stimulus.last_main_sha = Some(result.current_sha.clone());
                    let _ = store.update_stimulus(&stimulus);
                }

                if !result.trigger {
                    continue;
                }

                if let Ok(Some(_)) = store.get_active_wave_run(&wave_id) {
                    let stimulus_id = LfdId::parse(&stimulus.id)
                        .unwrap_or_else(|_| LfdId::from_raw(stimulus.id.clone()));
                    queue_or_coalesce_activation(
                        store,
                        &wave_id,
                        &stimulus_id,
                        &result.from_sha,
                        &result.current_sha,
                    );
                    continue;
                }

                let run_id = LfdId::new();
                let (acquired, reason) = scheduler.acquire(run_id.as_str()).await;
                if !acquired {
                    tracing::warn!(
                        wave_id = %wave.id,
                        reason = ?reason,
                        "scheduler at capacity; watch activation deferred"
                    );
                    continue;
                }

                let run = match create_wave_run_with_id(store, &wave, &run_id) {
                    Ok(run) => run,
                    Err(err) => {
                        scheduler.release(run_id.as_str());
                        tracing::error!(wave_id = %wave.id, error = %err, "failed to create wave run");
                        continue;
                    }
                };
                spawn_run_task_with_slot(store.clone(), executor.clone(), scheduler.clone(), run);
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

fn check_watch_stimulus(
    wave: &crate::proto::control::Wave,
    stimulus: &Stimulus,
) -> Result<WatchCheck, git2::Error> {
    let repo = Repository::open(&wave.repo)?;

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
    match store.get_pending_for_stimulus(wave_id, stimulus_id) {
        Ok(Some(mut existing)) => {
            existing.to_sha = to_sha.to_string();
            let _ = store.update_pending_activation(&existing);
        }
        Ok(None) => {
            let activation = PendingActivation {
                id: LfdId::new().to_string(),
                wave_id: wave_id.to_string(),
                stimulus_id: stimulus_id.to_string(),
                from_sha: from_sha.to_string(),
                to_sha: to_sha.to_string(),
                queued_at: Utc::now().timestamp(),
            };
            let _ = store.create_pending_activation(&activation);
        }
        Err(err) => {
            tracing::error!(wave_id = %wave_id, error = %err, "failed to check pending activation");
        }
    }
}
