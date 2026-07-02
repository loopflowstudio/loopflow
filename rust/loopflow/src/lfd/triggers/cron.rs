use std::str::FromStr;
use std::sync::Arc;
use std::time::Duration;

use chrono::{DateTime, Utc};
use cron::Schedule;
use tokio::task::JoinHandle;
use tokio_util::sync::CancellationToken;

use super::{spawn_immediate_activation, ActivationEnvelope, ImmediateActivation};
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
    let crons = match store.list_all_active_crons().await {
        Ok(crons) => crons,
        Err(err) => {
            tracing::error!(error = %err, "failed to list active wave crons");
            return;
        }
    };

    for cron in crons {
        let last_triggered = cron
            .last_triggered_at
            .and_then(|value| DateTime::<Utc>::from_timestamp(value, 0));
        if !should_activate_cron(&cron.schedule, last_triggered) {
            continue;
        }

        let wave = match store.get_wave(&cron.wave_id).await {
            Ok(Some(wave)) => wave,
            Ok(None) => continue,
            Err(err) => {
                tracing::warn!(wave_id = %cron.wave_id, cron_id = %cron.id, error = %err, "failed to load wave for cron");
                continue;
            }
        };

        let reason = format!("cron [{}] schedule {} due", cron.flow, cron.schedule);
        let envelope = ActivationEnvelope::new(wave.id(), None, reason, "", "", "main");
        match spawn_immediate_activation(
            store,
            executor,
            scheduler,
            event_hub,
            ImmediateActivation {
                wave: &wave,
                flow_override: Some(cron.flow.clone()),
                roadmap_item: None,
                force_parallel: true,
                envelope,
            },
        )
        .await
        {
            Ok(Some(_)) | Ok(None) => {
                let now = Utc::now().timestamp();
                if let Err(err) = store
                    .update_wave_cron_last_triggered(&cron.id, Some(now))
                    .await
                {
                    tracing::warn!(wave_id = %wave.id(), cron_id = %cron.id, error = %err, "failed to update cron last_triggered_at");
                }
            }
            Err(err) => {
                tracing::warn!(wave_id = %wave.id(), cron_id = %cron.id, error = %err, "cron activation failed");
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

    schedule
        .after(&check_from)
        .next()
        .is_some_and(|scheduled| scheduled <= now)
}

#[cfg(test)]
mod tests {
    use super::{check_cron_waves, should_activate_cron};
    use chrono::{Datelike, Duration, Timelike, Utc};
    use tempfile::tempdir;
    use time::OffsetDateTime;

    use crate::lfd::http::routes::test_helpers::test_http_state;
    use crate::lfd::id::LfdId;
    use crate::lfd::types::{Wave, WaveCron, WaveMode, WaveStatus};

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

    #[tokio::test]
    async fn cron_check_records_activation_log_and_last_triggered() {
        let state = test_http_state().await;
        let repo_tmp = tempdir().expect("tempdir");
        crate::lfd::http::routes::test_helpers::init_git_repo(repo_tmp.path());

        let wave = Wave {
            id: LfdId::new(),
            name: "cron-wave".to_string(),
            repo: repo_tmp.path().to_string_lossy().to_string(),
            mode: WaveMode::Manual,
            primary_flow: "build".to_string(),
            goal: "ship-roadmap".to_string(),
            metrics: Vec::new(),
            crons: Vec::new(),
            direction: Vec::new(),
            area: Vec::new(),
            status: WaveStatus::Idle,
            iteration: 0,
            cycle_start_iteration: 0,
            created_at: Some(OffsetDateTime::now_utc()),
            workers: 0,
        };
        state.store.create_wave(&wave).await.expect("create wave");

        let now = Utc::now();
        let cron = WaveCron {
            id: LfdId::new(),
            wave_id: wave.id().clone(),
            flow: "build".to_string(),
            schedule: format!("0 {} {} * * * *", now.minute(), now.hour()),
            last_triggered_at: None,
            created_at: Some(OffsetDateTime::now_utc()),
        };
        state
            .store
            .create_wave_cron(&cron)
            .await
            .expect("create cron");
        let _slot = state
            .scheduler
            .clone()
            .acquire_guard("busy-run")
            .await
            .expect("occupy scheduler slot");

        check_cron_waves(
            &state.store,
            &state.executor,
            &state.scheduler,
            &state.event_hub,
        )
        .await;

        let logs = state
            .store
            .list_activation_log(wave.id(), 10)
            .await
            .expect("list activation log");
        assert!(logs.iter().any(|log| {
            log.reason.contains("cron [build] schedule") && log.reason.contains(&cron.schedule)
        }));

        let updated = state
            .store
            .list_wave_crons(wave.id())
            .await
            .expect("list wave crons");
        assert_eq!(updated.len(), 1);
        assert!(updated[0].last_triggered_at.is_some());
    }
}
