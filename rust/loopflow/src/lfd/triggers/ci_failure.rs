use std::sync::Arc;

use tokio::task::JoinHandle;
use tokio_util::sync::CancellationToken;

use super::activation::{
    enqueue_pending_activation, ActivationEnvelope, EnqueueOutcome, ImmediateActivation,
};
use super::spawn_immediate_activation;
use crate::lfd::events::EventHub;
use crate::lfd::executor::WaveExecutor;
use crate::lfd::id::LfdId;
use crate::lfd::scheduler::Scheduler;
use crate::lfd::store::SharedStore;
use crate::lfd::types::{Event, Signal, Trigger, CI_FIX_FLOW};
use time::OffsetDateTime;

#[derive(Debug)]
struct CiFailureActivation {
    wave_id: LfdId,
    pr_number: u32,
    branch: String,
    commit_sha: String,
    check_name: String,
    logs_url: String,
}

pub fn spawn_ci_failure_handler(
    store: SharedStore,
    executor: WaveExecutor,
    scheduler: Arc<Scheduler>,
    event_hub: EventHub,
    cancel: CancellationToken,
) -> JoinHandle<()> {
    let mut rx = event_hub.subscribe();

    tokio::spawn(async move {
        loop {
            tokio::select! {
                _ = cancel.cancelled() => {
                    tracing::info!("ci_failure_handler shutting down");
                    break;
                }
                event = rx.recv() => {
                    match event {
                        Ok(Event::CiFailure {
                            wave_id,
                            pr_number,
                            branch,
                            commit_sha,
                            check_name,
                            logs_url,
                            ..
                        }) => {
                            let activation = CiFailureActivation {
                                wave_id,
                                pr_number,
                                branch,
                                commit_sha,
                                check_name,
                                logs_url,
                            };
                            if let Err(err) =
                                handle_ci_failure_event(&store, &executor, &scheduler, &event_hub, activation).await
                            {
                                tracing::warn!(error = %err, "failed handling CI failure activation");
                            }
                        }
                        Err(tokio::sync::broadcast::error::RecvError::Closed) => break,
                        _ => {}
                    }
                }
            }
        }
    })
}

async fn handle_ci_failure_event(
    store: &SharedStore,
    executor: &WaveExecutor,
    scheduler: &Arc<Scheduler>,
    event_hub: &EventHub,
    activation: CiFailureActivation,
) -> Result<(), String> {
    let trigger = resolve_ci_failure_trigger(store, &activation.wave_id).await?;
    let reason = format!(
        "CI failure for PR #{} on {} ({}): {}",
        activation.pr_number, activation.branch, activation.check_name, activation.logs_url
    );
    let envelope = ActivationEnvelope::new(
        &activation.wave_id,
        Some(&trigger.id),
        reason,
        &activation.commit_sha,
        &activation.commit_sha,
        &activation.branch,
    );

    let wave = store
        .get_wave(&activation.wave_id)
        .await
        .map_err(|err| err.to_string())?
        .ok_or_else(|| format!("wave {} not found", activation.wave_id))?;

    if wave.workers() == 1 {
        let outcome = enqueue_pending_activation(store, event_hub, envelope).await;
        if outcome.is_none() {
            return Err(format!(
                "failed to enqueue CI failure activation for wave {}",
                activation.wave_id
            ));
        }
        if matches!(outcome, Some(EnqueueOutcome::Dropped)) {
            tracing::warn!(
                wave_id = %activation.wave_id,
                trigger_id = %trigger.id,
                "dropped CI failure activation because queue is full"
            );
        }
    } else if let Err(err) = spawn_immediate_activation(
        store,
        executor,
        scheduler,
        event_hub,
        ImmediateActivation {
            wave: &wave,
            flow_override: trigger.flow.clone(),
            roadmap_item: None,
            envelope,
        },
    )
    .await
    {
        tracing::warn!(wave_id = %wave.id(), trigger_id = %trigger.id, error = %err, "ci failure activation failed");
    }
    Ok(())
}

async fn resolve_ci_failure_trigger(
    store: &SharedStore,
    wave_id: &LfdId,
) -> Result<Trigger, String> {
    let triggers = store
        .list_triggers(Some(wave_id))
        .await
        .map_err(|err| err.to_string())?;
    if let Some(existing) = triggers
        .into_iter()
        .find(|trigger| trigger.signal == Signal::CiFailure)
    {
        return Ok(existing);
    }

    let trigger = Trigger {
        id: LfdId::new(),
        wave_id: wave_id.clone(),
        source_wave_id: None,
        signal: Signal::CiFailure,
        flow: Some(CI_FIX_FLOW.to_string()),
        last_main_sha: None,
        last_triggered_at: Some(OffsetDateTime::now_utc().unix_timestamp()),
        created_at: Some(OffsetDateTime::now_utc()),
        enabled: true,
        max_iterations: None,
    };
    store
        .create_trigger(&trigger)
        .await
        .map_err(|err| err.to_string())?;
    Ok(trigger)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::lfd::scheduler::Scheduler;
    use crate::lfd::store::{open_store, StorageConfig};
    use crate::lfd::types::{Wave, WaveMode, WaveRun, WaveRunStatus, WaveStatus};
    use std::sync::Arc;

    async fn create_store() -> SharedStore {
        let path = std::env::temp_dir().join(format!("lfd-ci-failure-test-{}.db", LfdId::new()));
        open_store(&StorageConfig::sqlite(path))
            .await
            .map(Arc::new)
            .expect("store")
    }

    async fn create_wave(store: &SharedStore) -> Wave {
        let wave = Wave {
            id: LfdId::new(),
            name: "ci-wave".to_string(),
            repo: ".".to_string(),
            mode: WaveMode::Loop,
            primary_flow: "ship-roadmap".to_string(),
            cron: None,
            direction: Vec::new(),
            area: Vec::new(),
            status: WaveStatus::Idle,
            iteration: 0,
            cycle_start_iteration: 0,
            created_at: Some(OffsetDateTime::now_utc()),
            workers: 1,
        };
        store.create_wave(&wave).await.expect("create wave");
        wave
    }

    async fn create_ci_failure_trigger(store: &SharedStore, wave: &Wave) -> Trigger {
        let trigger = Trigger {
            id: LfdId::new(),
            wave_id: wave.id.clone(),
            source_wave_id: None,
            signal: Signal::CiFailure,
            flow: Some(CI_FIX_FLOW.to_string()),
            last_main_sha: None,
            last_triggered_at: None,
            created_at: Some(OffsetDateTime::now_utc()),
            enabled: true,
            max_iterations: None,
        };
        store
            .create_trigger(&trigger)
            .await
            .expect("create trigger");
        trigger
    }

    #[tokio::test]
    async fn resolve_ci_failure_trigger_reuses_existing() {
        let store = create_store().await;
        let wave = create_wave(&store).await;
        let existing = create_ci_failure_trigger(&store, &wave).await;

        let trigger = resolve_ci_failure_trigger(&store, &wave.id)
            .await
            .expect("resolve trigger");
        assert_eq!(trigger.id, existing.id);

        let triggers = store
            .list_triggers(Some(&wave.id))
            .await
            .expect("list triggers");
        assert_eq!(triggers.len(), 1);
    }

    #[tokio::test]
    async fn resolve_ci_failure_trigger_creates_default_ci_fix_flow() {
        let store = create_store().await;
        let wave = create_wave(&store).await;

        let trigger = resolve_ci_failure_trigger(&store, &wave.id)
            .await
            .expect("resolve trigger");

        assert_eq!(trigger.signal, Signal::CiFailure);
        assert_eq!(trigger.flow.as_deref(), Some("ci-fix"));
        assert!(trigger.enabled);
    }

    async fn create_serialized_wave(store: &SharedStore) -> Wave {
        let wave = Wave {
            id: LfdId::new(),
            name: "ci-wave-serialized".to_string(),
            repo: ".".to_string(),
            mode: WaveMode::Loop,
            primary_flow: "ship-roadmap".to_string(),
            cron: None,
            direction: Vec::new(),
            area: Vec::new(),
            status: WaveStatus::Idle,
            iteration: 0,
            cycle_start_iteration: 0,
            created_at: Some(OffsetDateTime::now_utc()),
            workers: 1,
        };
        store.create_wave(&wave).await.expect("create wave");
        wave
    }

    fn stub_executor(store: SharedStore, event_hub: EventHub) -> (WaveExecutor, Arc<Scheduler>) {
        use crate::lfd::executor::{AgentExecutor, AgentRunContext};
        use crate::lfd::output::OutputHub;
        use async_trait::async_trait;
        use std::path::Path;

        struct StubRunner;

        #[async_trait]
        impl AgentExecutor for StubRunner {
            async fn run(
                &self,
                _cmd: Vec<String>,
                _cwd: &Path,
                _ctx: AgentRunContext,
            ) -> anyhow::Result<i32> {
                Ok(0)
            }
            async fn terminate(&self, _agent_id: &str) -> anyhow::Result<()> {
                Ok(())
            }
        }

        let scheduler = Arc::new(Scheduler::new(1));
        let output_dir = std::env::temp_dir().join(format!("lfd-ci-test-output-{}", LfdId::new()));
        let output = OutputHub::new(16, output_dir);
        let executor = WaveExecutor::with_runner(
            store,
            scheduler.clone(),
            output,
            event_hub,
            Arc::new(StubRunner),
        );
        (executor, scheduler)
    }

    #[tokio::test]
    async fn handle_ci_failure_event_enqueues_push_activation_for_serialized_wave() {
        let store = create_store().await;
        let event_hub = EventHub::new(16);
        let wave = create_serialized_wave(&store).await;
        let trigger = create_ci_failure_trigger(&store, &wave).await;
        let mut active_run = WaveRun::new(LfdId::new(), wave.id.clone());
        active_run.status = WaveRunStatus::Running;
        active_run.snapshot.repo = wave.repo.clone();
        active_run.snapshot.flow = wave.primary_flow.clone();
        store
            .create_wave_run(&active_run)
            .await
            .expect("seed active run");
        let activation = CiFailureActivation {
            wave_id: wave.id.clone(),
            pr_number: 42,
            branch: "feature/test".to_string(),
            commit_sha: "abc123".to_string(),
            check_name: "rust-test".to_string(),
            logs_url: "https://example.com/logs".to_string(),
        };

        let (executor, scheduler) = stub_executor(store.clone(), event_hub.clone());

        handle_ci_failure_event(&store, &executor, &scheduler, &event_hub, activation)
            .await
            .expect("enqueue activation");

        let pending = store
            .list_pending_activations(&wave.id)
            .await
            .expect("list pending");
        assert_eq!(pending.len(), 1);
        assert_eq!(pending[0].trigger_id, Some(trigger.id));
        assert_eq!(pending[0].from_sha, "abc123");
        assert_eq!(pending[0].to_sha, "abc123");
        assert!(pending[0].reason.contains("PR #42"));
        assert!(pending[0].reason.contains("rust-test"));
    }
}
