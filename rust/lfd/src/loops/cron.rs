use std::str::FromStr;
use std::time::Duration;

use chrono::{DateTime, Utc};
use cron::Schedule;
use tokio::task::JoinHandle;
use tokio_util::sync::CancellationToken;

use crate::proto::control::{StimulusKind, WaveStatus};
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
                    check_cron_waves(&store);
                }
            }
        }
    })
}

fn check_cron_waves(store: &SharedStore) {
    let waves = match store.list_waves_by_stimulus(StimulusKind::StimulusCron as i32) {
        Ok(waves) => waves,
        Err(err) => {
            tracing::error!(error = %err, "failed to list cron waves");
            return;
        }
    };

    for wave in waves {
        if wave.paused {
            continue;
        }

        let cron_expr = match &wave.stimulus {
            Some(stimulus) if !stimulus.cron.is_empty() => &stimulus.cron,
            _ => continue,
        };

        let last_run = get_last_run_end_time(store, &wave.id);

        if should_activate_cron(cron_expr, last_run) {
            let mut wave = wave.clone();
            if wave.status == WaveStatus::WaveRunning as i32
                || wave.status == WaveStatus::WaveWaiting as i32
            {
                if let Ok(count) = store.increment_pending_activations(&wave.id) {
                    wave.pending_activations = count;
                }
                let _ = store.update_wave(&wave);
                tracing::debug!(wave_id = %wave.id, "cron: queued activation");
            } else if wave.status == WaveStatus::WaveIdle as i32 {
                wave.status = WaveStatus::WaveRunning as i32;
                let _ = store.update_wave(&wave);
                tracing::info!(wave_id = %wave.id, cron = %cron_expr, "cron: activated");
            }
        }
    }
}

fn should_activate_cron(cron_expr: &str, last_run_ended: Option<DateTime<Utc>>) -> bool {
    let schedule = match Schedule::from_str(cron_expr) {
        Ok(schedule) => schedule,
        Err(_) => return false,
    };

    let now = Utc::now();
    let grace_period = chrono::Duration::hours(24);
    let check_from = last_run_ended.unwrap_or(now - grace_period);

    for scheduled in schedule.after(&check_from) {
        if scheduled > now {
            break;
        }
        return true;
    }

    false
}

fn get_last_run_end_time(store: &SharedStore, wave_id: &str) -> Option<DateTime<Utc>> {
    let runs = store.list_step_runs().ok()?;
    let mut last: Option<DateTime<Utc>> = None;
    for run in runs {
        if run.wave_id.as_deref() != Some(wave_id) {
            continue;
        }
        let ended_at = run.ended_at.as_ref()?;
        let timestamp = DateTime::<Utc>::from_timestamp(ended_at.seconds, 0)?;
        if last.map(|value| timestamp > value).unwrap_or(true) {
            last = Some(timestamp);
        }
    }
    last
}
