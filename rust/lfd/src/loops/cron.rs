use std::collections::HashSet;
use std::str::FromStr;
use std::time::Duration;

use chrono::{DateTime, Utc};
use cron::Schedule;
use tokio::task::JoinHandle;
use tokio_util::sync::CancellationToken;

use super::common::{create_wave_run_with_id, spawn_run_task_with_slot};
use crate::executor::WaveExecutor;
use crate::id::LfdId;
use crate::proto::control::{PendingActivation, StimulusKind};
use crate::scheduler::Scheduler;
use crate::store::SharedStore;

pub fn spawn_cron_poller(
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
                    tracing::info!("cron_poller shutting down");
                    break;
                }
                _ = interval.tick() => {
                    check_cron_stimuli(&store, &executor, &scheduler).await;
                }
            }
        }
    })
}

async fn check_cron_stimuli(
    store: &SharedStore,
    executor: &WaveExecutor,
    scheduler: &std::sync::Arc<Scheduler>,
) {
    let stimuli = match store.list_stimuli_by_kind(StimulusKind::StimulusCron as i32) {
        Ok(stimuli) => stimuli,
        Err(err) => {
            tracing::error!(error = %err, "failed to list cron stimuli");
            return;
        }
    };

    let mut started = HashSet::new();

    for stimulus in stimuli {
        if !stimulus.enabled || stimulus.cron.is_empty() {
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
                            "scheduler at capacity; cron activation deferred"
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

        let last_triggered = stimulus
            .last_triggered_at
            .and_then(|ts| DateTime::<Utc>::from_timestamp(ts, 0));

        if should_activate_cron(&stimulus.cron, last_triggered) {
            let mut stimulus = stimulus.clone();
            stimulus.last_triggered_at = Some(Utc::now().timestamp());
            let _ = store.update_stimulus(&stimulus);

            if let Ok(Some(_)) = store.get_active_wave_run(&wave_id) {
                let stimulus_id = LfdId::parse(&stimulus.id)
                    .unwrap_or_else(|_| LfdId::from_raw(stimulus.id.clone()));
                queue_or_coalesce_activation(store, &wave_id, &stimulus_id);
                continue;
            }

            let run_id = LfdId::new();
            let (acquired, reason) = scheduler.acquire(run_id.as_str()).await;
            if !acquired {
                tracing::warn!(
                    wave_id = %wave.id,
                    reason = ?reason,
                    "scheduler at capacity; cron activation deferred"
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
    }
}

fn should_activate_cron(cron_expr: &str, last_triggered: Option<DateTime<Utc>>) -> bool {
    let schedule = match Schedule::from_str(cron_expr) {
        Ok(schedule) => schedule,
        Err(_) => return false,
    };

    let now = Utc::now();
    let grace_period = chrono::Duration::hours(24);
    let check_from = last_triggered.unwrap_or(now - grace_period);

    if let Some(scheduled) = schedule.after(&check_from).next() {
        if scheduled <= now {
            return true;
        }
    }

    false
}

fn queue_or_coalesce_activation(store: &SharedStore, wave_id: &LfdId, stimulus_id: &LfdId) {
    match store.get_pending_for_stimulus(wave_id, stimulus_id) {
        Ok(Some(_existing)) => {
            tracing::debug!(wave_id = %wave_id, stimulus_id = %stimulus_id, "cron activation already queued");
        }
        Ok(None) => {
            let activation = PendingActivation {
                id: LfdId::new().to_string(),
                wave_id: wave_id.to_string(),
                stimulus_id: stimulus_id.to_string(),
                from_sha: String::new(),
                to_sha: String::new(),
                queued_at: Utc::now().timestamp(),
            };
            let _ = store.create_pending_activation(&activation);
        }
        Err(err) => {
            tracing::error!(wave_id = %wave_id, error = %err, "failed to check pending activation");
        }
    }
}
