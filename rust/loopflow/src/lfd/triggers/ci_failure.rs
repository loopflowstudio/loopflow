use tokio::task::JoinHandle;
use tokio_util::sync::CancellationToken;

use super::activation::{enqueue_pending_activation, ActivationEnvelope, EnqueueOutcome};
use crate::lfd::events::EventHub;
use crate::lfd::id::LfdId;
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
                                handle_ci_failure_event(&store, &event_hub, activation).await
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
    event_hub: &EventHub,
    activation: CiFailureActivation,
) -> Result<(), String> {
    let stimulus_id = resolve_ci_failure_stimulus(store, &activation.wave_id).await?;
    let reason = format!(
        "CI failure for PR #{} on {} ({}): {}",
        activation.pr_number, activation.branch, activation.check_name, activation.logs_url
    );
    let outcome = enqueue_pending_activation(
        store,
        event_hub,
        ActivationEnvelope::new(
            &activation.wave_id,
            &stimulus_id,
            ActivationSource::Push,
            reason,
            &activation.commit_sha,
            &activation.commit_sha,
        ),
    )
    .await;
    if outcome.is_none() {
        return Err(format!(
            "failed to enqueue CI failure activation for wave {}",
            activation.wave_id
        ));
    }
    if matches!(outcome, Some(EnqueueOutcome::Dropped)) {
        tracing::warn!(
            wave_id = %activation.wave_id,
            stimulus_id = %stimulus_id,
            "dropped CI failure activation because queue is full"
        );
    }
    Ok(())
}

async fn resolve_ci_failure_stimulus(
    store: &SharedStore,
    wave_id: &LfdId,
) -> Result<LfdId, String> {
    let stimuli = store
        .list_stimuli(Some(wave_id))
        .await
        .map_err(|err| err.to_string())?;
    if let Some(existing) = stimuli
        .into_iter()
        .find(|stimulus| stimulus.signal == Signal::CiFailure)
    {
        return Ok(existing.id);
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
    };
    store
        .create_stimulus(&stimulus)
        .await
        .map_err(|err| err.to_string())?;
    Ok(stimulus.id)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::lfd::store::{open_store, StorageConfig};
    use crate::lfd::types::{Wave, WaveStatus};
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
            flow: "build".to_string(),
            direction: Vec::new(),
            area: Vec::new(),
            status: WaveStatus::Idle,
            iteration: 0,
            created_at: Some(OffsetDateTime::now_utc()),
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

        let stimulus_id = resolve_ci_failure_stimulus(&store, &wave.id)
            .await
            .expect("resolve stimulus");
        assert_eq!(stimulus_id, existing.id);

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

        let stimulus_id = resolve_ci_failure_stimulus(&store, &wave.id)
            .await
            .expect("resolve stimulus");

        let stimulus = store
            .get_stimulus(&stimulus_id)
            .await
            .expect("get stimulus")
            .expect("stimulus");
        assert_eq!(stimulus.signal, Signal::CiFailure);
        assert_eq!(stimulus.flow.as_deref(), Some("ci-fix"));
        assert!(stimulus.enabled);
    }

    #[tokio::test]
    async fn handle_ci_failure_event_enqueues_push_activation() {
        let store = create_store().await;
        let event_hub = EventHub::new(16);
        let wave = create_wave(&store).await;
        let stimulus = create_ci_failure_stimulus(&store, &wave).await;
        let activation = CiFailureActivation {
            wave_id: wave.id.clone(),
            pr_number: 42,
            branch: "feature/test".to_string(),
            commit_sha: "abc123".to_string(),
            check_name: "rust-test".to_string(),
            logs_url: "https://example.com/logs".to_string(),
        };

        handle_ci_failure_event(&store, &event_hub, activation)
            .await
            .expect("enqueue activation");

        let pending = store
            .list_pending_activations(&wave.id)
            .await
            .expect("list pending");
        assert_eq!(pending.len(), 1);
        assert_eq!(pending[0].stimulus_id, stimulus.id);
        assert_eq!(pending[0].source, ActivationSource::Push);
        assert_eq!(pending[0].from_sha, "abc123");
        assert_eq!(pending[0].to_sha, "abc123");
        assert!(pending[0].reason.contains("PR #42"));
        assert!(pending[0].reason.contains("rust-test"));
    }
}
