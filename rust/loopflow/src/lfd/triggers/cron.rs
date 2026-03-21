use std::str::FromStr;
use std::sync::Arc;
use std::time::Duration;

use chrono::{DateTime, Utc};
use cron::Schedule;
use tokio::task::JoinHandle;
use tokio_util::sync::CancellationToken;

use super::{dispatch_or_enqueue_activation, ActivationEnvelope};
use crate::lfd::events::EventHub;
use crate::lfd::executor::WaveExecutor;
use crate::lfd::scheduler::Scheduler;
use crate::lfd::store::SharedStore;

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

        // Use the most recent activation log entry as a proxy for last triggered.
        let last_triggered = store
            .list_activation_log(wave.id(), 1)
            .await
            .ok()
            .and_then(|logs| logs.into_iter().next())
            .and_then(|log| DateTime::<Utc>::from_timestamp(log.created_at, 0));

        if should_activate_cron(&cron_expr, last_triggered) {
            let reason = format!("cron schedule {cron_expr} due");
            let envelope = ActivationEnvelope::new(wave.id(), None, reason, "", "", "main");
            let _ = dispatch_or_enqueue_activation(
                store,
                executor,
                scheduler,
                event_hub,
                &wave,
                Some(wave.primary_flow().to_string()),
                envelope,
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

    schedule
        .after(&check_from)
        .next()
        .is_some_and(|scheduled| scheduled <= now)
}

#[cfg(test)]
mod tests {
    use super::should_activate_cron;
    use chrono::{Datelike, Duration, Timelike, Utc};

    #[test]
    fn never_triggered_within_grace_period() {
        let now = Utc::now();
        let expr = format!("0 {} {} * * * *", now.minute(), now.hour());
        assert!(should_activate_cron(&expr, None));
    }

    #[test]
    fn never_triggered_outside_grace_period() {
        let now = Utc::now();
        let two_days_ago = now - Duration::days(2);
        let expr = format!(
            "0 {} {} {} {} * *",
            two_days_ago.minute(),
            two_days_ago.hour(),
            two_days_ago.day(),
            two_days_ago.month()
        );
        assert!(!should_activate_cron(&expr, None));
    }

    #[test]
    fn just_triggered() {
        let now = Utc::now();
        let last_triggered = now - Duration::minutes(1);
        let expr = format!("0 {} * * * * *", last_triggered.minute());
        assert!(!should_activate_cron(&expr, Some(last_triggered)));
    }

    #[test]
    fn past_due() {
        let now = Utc::now();
        let expr = format!("0 {} * * * * *", now.minute());
        let last_triggered = now - Duration::hours(2);
        assert!(should_activate_cron(&expr, Some(last_triggered)));
    }

    #[test]
    fn invalid_expression() {
        assert!(!should_activate_cron("not-a-cron", None));
    }
}
