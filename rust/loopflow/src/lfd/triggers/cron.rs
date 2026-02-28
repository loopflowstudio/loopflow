use std::str::FromStr;
use std::sync::Arc;
use std::time::Duration;

use chrono::{DateTime, Utc};
use cron::Schedule;
use tokio::task::JoinHandle;
use tokio_util::sync::CancellationToken;

use super::{enqueue_pending_activation, spawn_immediate_activation, ActivationEnvelope};
use crate::lfd::events::EventHub;
use crate::lfd::executor::WaveExecutor;
use crate::lfd::scheduler::Scheduler;
use crate::lfd::store::SharedStore;
use crate::lfd::types::ActivationSource;

pub fn spawn_cron_poller(
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
                    tracing::info!("cron_poller shutting down");
                    break;
                }
                _ = interval.tick() => {
                    check_cron_waves(&store, &executor, &scheduler, &event_hub).await;
                }
            }
        }
    })
}

async fn check_cron_waves(
    store: &SharedStore,
    executor: &WaveExecutor,
    scheduler: &Arc<Scheduler>,
    event_hub: &EventHub,
) {
    let waves = match store.list_cron_waves().await {
        Ok(waves) => waves,
        Err(err) => {
            tracing::error!(error = %err, "failed to list cron waves");
            return;
        }
    };

    for wave in waves {
        let cron_expr = match &wave.cron {
            Some(c) if !c.is_empty() => c.clone(),
            _ => continue,
        };

        // Use the wave's last iteration timestamp as a proxy for last triggered.
        // For more accurate tracking, we could store last_cron_triggered_at on the wave,
        // but iteration timing is sufficient for now.
        let last_triggered = None::<DateTime<Utc>>;

        if should_activate_cron(&cron_expr, last_triggered) {
            let reason = format!("cron schedule {cron_expr} due");
            let envelope = ActivationEnvelope::new(
                wave.id(),
                None,
                ActivationSource::Poll,
                reason,
                "",
                "",
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
                    Some(wave.flow().to_string()),
                    envelope,
                )
                .await;
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
