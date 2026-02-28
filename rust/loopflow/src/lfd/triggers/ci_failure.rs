use std::sync::Arc;

use tokio::task::JoinHandle;
use tokio_util::sync::CancellationToken;

use super::activation::{enqueue_pending_activation, ActivationEnvelope, EnqueueOutcome};
use super::spawn_immediate_activation;
use crate::lfd::events::EventHub;
use crate::lfd::executor::WaveExecutor;
use crate::lfd::id::LfdId;
use crate::lfd::scheduler::Scheduler;
use crate::lfd::store::SharedStore;
use crate::lfd::types::{ActivationSource, Event, Signal, Stimulus, CI_FIX_FLOW};
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
    let stimulus = resolve_ci_failure_stimulus(store, &activation.wave_id).await?;
    let reason = format!(
        "CI failure for PR #{} on {} ({}): {}",
        activation.pr_number, activation.branch, activation.check_name, activation.logs_url
    );
    let envelope = ActivationEnvelope::new(
        &activation.wave_id,
        Some(&stimulus.id),
        ActivationSource::Push,
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

    if wave.serialized {
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
                stimulus_id = %stimulus.id,
                "dropped CI failure activation because queue is full"
            );
        }
    } else {
        let _ = spawn_immediate_activation(
            store,
            executor,
            scheduler,
            event_hub,
            &wave,
            stimulus.flow.clone(),
            envelope,
        )
        .await;
    }
    Ok(())
}

async fn resolve_ci_failure_stimulus(
    store: &SharedStore,
    wave_id: &LfdId,
) -> Result<Stimulus, String> {
    let stimuli = store
        .list_stimuli(Some(wave_id))
        .await
        .map_err(|err| err.to_string())?;
    if let Some(existing) = stimuli
        .into_iter()
        .find(|stimulus| stimulus.signal == Signal::CiFailure)
    {
        return Ok(existing);
    }

    let stimulus = Stimulus {
        id: LfdId::new(),
        wave_id: wave_id.clone(),
        source_wave_id: None,
        signal: Signal::CiFailure,
        flow: Some(CI_FIX_FLOW.to_string()),
        cron: None,
        last_main_sha: None,
        last_triggered_at: Some(OffsetDateTime::now_utc().unix_timestamp()),
        created_at: Some(OffsetDateTime::now_utc()),
        enabled: true,
        max_iterations: None,
    };
    store
        .create_stimulus(&stimulus)
        .await
        .map_err(|err| err.to_string())?;
    Ok(stimulus)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::lfd::scheduler::Scheduler;
    use crate::lfd::store::{open_store, StorageConfig};
    use crate::lfd::types::{Wave, WaveMode, WaveStatus};
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
            flow: "build".to_string(),
            loop_flow: "ship-roadmap".to_string(),
            cron: None,
            direction: Vec::new(),
            area: Vec::new(),
            status: WaveStatus::Idle,
            iteration: 0,
            cycle_start_iteration: 0,
            created_at: Some(OffsetDateTime::now_utc()),
            serialized: false,
        };
        store.create_wave(&wave).await.expect("create wave");
        wave
    }

    async fn create_ci_failure_stimulus(store: &SharedStore, wave: &Wave) -> Stimulus {
        let stimulus = Stimulus {
            id: LfdId::new(),
            wave_id: wave.id.clone(),
            source_wave_id: None,
            signal: Signal::CiFailure,
            flow: Some(CI_FIX_FLOW.to_string()),
            cron: None,
            last_main_sha: None,
            last_triggered_at: None,
            created_at: Some(OffsetDateTime::now_utc()),
            enabled: true,
            max_iterations: None,
        };
        store
            .create_stimulus(&stimulus)
            .await
            .expect("create stimulus");
        stimulus
    }

    #[tokio::test]
    async fn resolve_ci_failure_stimulus_reuses_existing() {
        let store = create_store().await;
        let wave = create_wave(&store).await;
        let existing = create_ci_failure_stimulus(&store, &wave).await;

        let stimulus = resolve_ci_failure_stimulus(&store, &wave.id)
            .await
            .expect("resolve stimulus");
        assert_eq!(stimulus.id, existing.id);

        let stimuli = store
            .list_stimuli(Some(&wave.id))
            .await
            .expect("list stimuli");
        assert_eq!(stimuli.len(), 1);
    }

    #[tokio::test]
    async fn resolve_ci_failure_stimulus_creates_default_ci_fix_flow() {
        let store = create_store().await;
        let wave = create_wave(&store).await;

        let stimulus = resolve_ci_failure_stimulus(&store, &wave.id)
            .await
            .expect("resolve stimulus");

        assert_eq!(stimulus.signal, Signal::CiFailure);
        assert_eq!(stimulus.flow.as_deref(), Some("ci-fix"));
        assert!(stimulus.enabled);
    }

    async fn create_serialized_wave(store: &SharedStore) -> Wave {
        let wave = Wave {
            id: LfdId::new(),
            name: "ci-wave-serialized".to_string(),
            repo: ".".to_string(),
            mode: WaveMode::Loop,
            flow: "build".to_string(),
            loop_flow: "ship-roadmap".to_string(),
            cron: None,
            direction: Vec::new(),
            area: Vec::new(),
            status: WaveStatus::Idle,
            iteration: 0,
            cycle_start_iteration: 0,
            created_at: Some(OffsetDateTime::now_utc()),
            serialized: true,
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
                _ctx: AgentRunContext<'_>,
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
        let stimulus = create_ci_failure_stimulus(&store, &wave).await;
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
        assert_eq!(pending[0].stimulus_id, Some(stimulus.id));
        assert_eq!(pending[0].source, ActivationSource::Push);
        assert_eq!(pending[0].from_sha, "abc123");
        assert_eq!(pending[0].to_sha, "abc123");
        assert!(pending[0].reason.contains("PR #42"));
        assert!(pending[0].reason.contains("rust-test"));
    }
}
