use std::str::FromStr;
use std::time::Duration;

use chrono::{DateTime, Utc};
use cron::Schedule;
use tokio::task::JoinHandle;
use tokio_util::sync::CancellationToken;

use crate::id::LfdId;
use crate::proto::control::{PendingActivation, StimulusKind, WaveStatus};
use crate::store::SharedStore;

pub fn spawn_cron_poller(store: SharedStore, cancel: CancellationToken) -> JoinHandle<()> {
    tokio::spawn(async move {
        let mut interval = tokio::time::interval(Duration::from_secs(30));
        loop {
            tokio::select! {
                _ = cancel.cancelled() => {
                    tracing::info!("cron_poller shutting down");
                    break;
                }
                _ = interval.tick() => {
                    check_cron_stimuli(&store);
                }
            }
        }
    })
}

fn check_cron_stimuli(store: &SharedStore) {
    let stimuli = match store.list_stimuli_by_kind(StimulusKind::StimulusCron as i32) {
        Ok(stimuli) => stimuli,
        Err(err) => {
            tracing::error!(error = %err, "failed to list cron stimuli");
            return;
        }
    };

    for stimulus in stimuli {
        if !stimulus.enabled || stimulus.cron.is_empty() {
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

        // Use stimulus.last_triggered_at for scheduling
        let last_triggered = stimulus
            .last_triggered_at
            .and_then(|ts| DateTime::<Utc>::from_timestamp(ts, 0));

        if should_activate_cron(&stimulus.cron, last_triggered) {
            // Update stimulus.last_triggered_at
            let mut stimulus = stimulus.clone();
            stimulus.last_triggered_at = Some(Utc::now().timestamp());
            if let Err(err) = store.update_stimulus(&stimulus) {
                tracing::error!(stimulus_id = %stimulus.id, error = %err, "failed to update stimulus");
                continue;
            }

            // Activate or queue the wave
            if wave.status == WaveStatus::WaveRunning as i32
                || wave.status == WaveStatus::WaveWaiting as i32
            {
                // Wave is busy - queue a pending activation (coalesce if exists)
                let stimulus_id = LfdId::parse(&stimulus.id)
                    .unwrap_or_else(|_| LfdId::from_raw(stimulus.id.clone()));
                queue_or_coalesce_activation(store, &wave_id, &stimulus_id);
                tracing::debug!(wave_id = %wave.id, stimulus_id = %stimulus.id, "cron: queued activation");
            } else if wave.status == WaveStatus::WaveIdle as i32 {
                // Activate the wave
                let mut wave = wave.clone();
                wave.status = WaveStatus::WaveRunning as i32;
                if let Err(err) = store.update_wave(&wave) {
                    tracing::error!(wave_id = %wave.id, error = %err, "failed to activate wave");
                    continue;
                }
                tracing::info!(wave_id = %wave.id, cron = %stimulus.cron, "cron: activated");
            }
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
    // Check if there's already a pending activation for this stimulus
    match store.get_pending_for_stimulus(wave_id, stimulus_id) {
        Ok(Some(_existing)) => {
            // Already queued - cron doesn't have SHA ranges to update, so just skip
            tracing::debug!(wave_id = %wave_id, stimulus_id = %stimulus_id, "cron activation already queued");
        }
        Ok(None) => {
            // Create new pending activation
            let activation = PendingActivation {
                id: LfdId::new().to_string(),
                wave_id: wave_id.to_string(),
                stimulus_id: stimulus_id.to_string(),
                from_sha: String::new(),
                to_sha: String::new(),
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
