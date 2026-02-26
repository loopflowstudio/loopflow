use std::str::FromStr;
use std::time::Duration;

use chrono::{DateTime, Utc};
use cron::Schedule;
use tokio::task::JoinHandle;
use tokio_util::sync::CancellationToken;

use super::{enqueue_pending_activation, ActivationEnvelope};
use crate::lfd::events::EventHub;
use crate::lfd::store::SharedStore;
use crate::lfd::types::{ActivationSource, Signal, WaveStatus};

pub fn spawn_cron_poller(
    store: SharedStore,
    event_hub: EventHub,
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
                    check_cron_stimuli(&store, &event_hub).await;
                }
            }
        }
    })
}

async fn check_cron_stimuli(store: &SharedStore, event_hub: &EventHub) {
    let stimuli = match store.list_stimuli_by_signal(Signal::Cron.as_i32()).await {
        Ok(stimuli) => stimuli,
        Err(err) => {
            tracing::error!(error = %err, "failed to list cron stimuli");
            return;
        }
    };

    for stimulus in stimuli {
        if !stimulus.enabled {
            continue;
        }

        let cron_expr = match &stimulus.cron {
            Some(c) if !c.is_empty() => c,
            _ => continue,
        };

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

        let last_triggered = stimulus
            .last_triggered_at
            .and_then(|ts| DateTime::<Utc>::from_timestamp(ts, 0));

        if should_activate_cron(cron_expr, last_triggered) {
            let mut stimulus = stimulus.clone();
            stimulus.last_triggered_at = Some(Utc::now().timestamp());
            let _ = store.update_stimulus(&stimulus).await;

            let reason = format!("cron schedule {cron_expr} due");
            let _ = enqueue_pending_activation(
                store,
                event_hub,
                ActivationEnvelope::new(
                    &stimulus.wave_id,
                    &stimulus.id,
                    ActivationSource::Poll,
                    reason,
                    "",
                    "",
                ),
            )
            .await;
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
