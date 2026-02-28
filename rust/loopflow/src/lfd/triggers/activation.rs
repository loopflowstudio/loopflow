use std::sync::Arc;
use std::time::Duration;

use tokio::task::JoinHandle;
use tokio_util::sync::CancellationToken;

use super::common::{create_parallel_wave_run, create_wave_run_with_id, spawn_run_task_with_slot};
use crate::lfd::events::EventHub;
use crate::lfd::executor::WaveExecutor;
use crate::lfd::id::LfdId;
use crate::lfd::scheduler::Scheduler;
use crate::lfd::store::SharedStore;
use crate::lfd::types::{
    ActivationLog, ActivationOutcome, ActivationSource, Event, PendingActivation, Wave, WaveRun,
    WaveStatus,
};

pub const DEFAULT_ACTIVATION_QUEUE_LIMIT: usize = 20;

#[derive(Debug, Clone)]
pub struct ActivationEnvelope {
    pub wave_id: LfdId,
    pub stimulus_id: LfdId,
    pub source: ActivationSource,
    pub reason: String,
    pub from_sha: String,
    pub to_sha: String,
    pub target_branch: String,
}

impl ActivationEnvelope {
    pub fn new(
        wave_id: &LfdId,
        stimulus_id: &LfdId,
        source: ActivationSource,
        reason: impl Into<String>,
        from_sha: impl Into<String>,
        to_sha: impl Into<String>,
        target_branch: impl Into<String>,
    ) -> Self {
        Self {
            wave_id: wave_id.clone(),
            stimulus_id: stimulus_id.clone(),
            source,
            reason: reason.into(),
            from_sha: from_sha.into(),
            to_sha: to_sha.into(),
            target_branch: target_branch.into(),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EnqueueOutcome {
    Queued,
    Coalesced,
    Dropped,
}

pub fn spawn_activation_dispatcher(
    store: SharedStore,
    executor: WaveExecutor,
    scheduler: Arc<Scheduler>,
    event_hub: EventHub,
    cancel: CancellationToken,
) -> JoinHandle<()> {
    tokio::spawn(async move {
        let mut interval = tokio::time::interval(Duration::from_secs(1));
        loop {
            tokio::select! {
                _ = cancel.cancelled() => {
                    tracing::info!("activation_dispatcher shutting down");
                    break;
                }
                _ = interval.tick() => {
                    dispatch_pending_activations(&store, &executor, &scheduler, &event_hub).await;
                }
            }
        }
    })
}

pub async fn enqueue_pending_activation(
    store: &SharedStore,
    event_hub: &EventHub,
    envelope: ActivationEnvelope,
) -> Option<EnqueueOutcome> {
    match store
        .get_pending_for_stimulus(&envelope.wave_id, &envelope.stimulus_id)
        .await
    {
        Ok(Some(mut existing)) => {
            existing.source = envelope.source;
            existing.reason = envelope.reason.clone();
            if !envelope.from_sha.is_empty() && existing.from_sha.is_empty() {
                existing.from_sha = envelope.from_sha.clone();
            }
            if !envelope.to_sha.is_empty() {
                existing.to_sha = envelope.to_sha.clone();
            }
            if let Err(err) = store.update_pending_activation(&existing).await {
                tracing::error!(
                    wave_id = %envelope.wave_id,
                    stimulus_id = %envelope.stimulus_id,
                    error = %err,
                    "failed to coalesce pending activation"
                );
                return None;
            }
            let queue_depth = store
                .list_pending_activations(&envelope.wave_id)
                .await
                .map(|items| items.len() as u32)
                .unwrap_or_default();
            record_activation_log(
                store,
                event_hub,
                &envelope,
                ActivationOutcome::Coalesced,
                queue_depth,
            )
            .await;
            Some(EnqueueOutcome::Coalesced)
        }
        Ok(None) => {
            let queue_depth = match store.list_pending_activations(&envelope.wave_id).await {
                Ok(items) => items.len(),
                Err(err) => {
                    tracing::error!(
                        wave_id = %envelope.wave_id,
                        stimulus_id = %envelope.stimulus_id,
                        error = %err,
                        "failed to inspect pending activation queue"
                    );
                    return None;
                }
            };
            if queue_depth >= DEFAULT_ACTIVATION_QUEUE_LIMIT {
                record_activation_log(
                    store,
                    event_hub,
                    &envelope,
                    ActivationOutcome::Dropped,
                    queue_depth as u32,
                )
                .await;
                return Some(EnqueueOutcome::Dropped);
            }

            let activation = PendingActivation {
                id: LfdId::new(),
                wave_id: envelope.wave_id.clone(),
                stimulus_id: envelope.stimulus_id.clone(),
                source: envelope.source,
                reason: envelope.reason.clone(),
                from_sha: envelope.from_sha.clone(),
                to_sha: envelope.to_sha.clone(),
                queued_at: time::OffsetDateTime::now_utc().unix_timestamp(),
                target_branch: envelope.target_branch.clone(),
            };
            if let Err(err) = store.create_pending_activation(&activation).await {
                tracing::error!(
                    wave_id = %envelope.wave_id,
                    stimulus_id = %envelope.stimulus_id,
                    error = %err,
                    "failed to queue pending activation"
                );
                return None;
            }
            record_activation_log(
                store,
                event_hub,
                &envelope,
                ActivationOutcome::Queued,
                (queue_depth + 1) as u32,
            )
            .await;
            Some(EnqueueOutcome::Queued)
        }
        Err(err) => {
            tracing::error!(
                wave_id = %envelope.wave_id,
                stimulus_id = %envelope.stimulus_id,
                error = %err,
                "failed to read pending activation for stimulus"
            );
            None
        }
    }
}

/// Bypass the activation queue and spawn a run immediately.
///
/// Used for non-serialized (parallel) waves: the trigger creates a run
/// directly without going through the pending activation queue. Returns
/// the WaveRun if a scheduler slot was available.
pub async fn spawn_immediate_activation(
    store: &SharedStore,
    executor: &WaveExecutor,
    scheduler: &Arc<Scheduler>,
    event_hub: &EventHub,
    wave: &Wave,
    stimulus_flow_override: Option<String>,
    envelope: ActivationEnvelope,
) -> Option<WaveRun> {
    if wave.status() == WaveStatus::Paused {
        return None;
    }
    if scheduler.has_active_session(wave.id().as_str()) {
        return None;
    }

    let run_id = LfdId::new();
    let slot_guard = match scheduler.acquire_guard(run_id.as_str()).await {
        Ok(guard) => guard,
        Err(reason) => {
            tracing::debug!(
                wave_id = %wave.id(),
                reason = %reason,
                "scheduler at capacity; immediate activation deferred to queue"
            );
            // Fall back to queue when scheduler is full.
            let _ = enqueue_pending_activation(store, event_hub, envelope).await;
            return None;
        }
    };

    let dispatch_log = ActivationLog::new(
        wave.id().clone(),
        envelope.stimulus_id.clone(),
        envelope.source,
        envelope.reason.clone(),
        ActivationOutcome::Dispatched,
    );
    if let Err(err) = store.create_activation_log(&dispatch_log).await {
        tracing::error!(
            wave_id = %wave.id(),
            stimulus_id = %envelope.stimulus_id,
            error = %err,
            "failed to write immediate activation dispatch log"
        );
        return None;
    }

    let target = if envelope.target_branch.is_empty() || envelope.target_branch == "main" {
        None
    } else {
        Some(envelope.target_branch.as_str())
    };
    let mut run = match create_parallel_wave_run(store, wave, &run_id, target).await {
        Ok(run) => run,
        Err(err) => {
            tracing::error!(
                wave_id = %wave.id(),
                error = %err,
                "failed to create parallel wave run for immediate activation"
            );
            return None;
        }
    };
    if let Some(flow_override) = stimulus_flow_override {
        run.snapshot.flow = flow_override;
    }
    run.target_branch = envelope.target_branch.clone();
    run.activation_log_id = Some(dispatch_log.id.clone());
    if let Err(err) = store.update_wave_run(&run).await {
        tracing::error!(
            wave_id = %wave.id(),
            run_id = %run.id,
            error = %err,
            "failed to attach activation log to immediate run"
        );
    }

    spawn_run_task_with_slot(
        store.clone(),
        executor.clone(),
        event_hub.clone(),
        run.clone(),
        slot_guard,
    );
    Some(run)
}

pub async fn dispatch_pending_activations(
    store: &SharedStore,
    executor: &WaveExecutor,
    scheduler: &Arc<Scheduler>,
    event_hub: &EventHub,
) {
    let waves = match store.list_waves(None).await {
        Ok(waves) => waves,
        Err(err) => {
            tracing::error!(error = %err, "failed to list waves for activation dispatch");
            return;
        }
    };

    for wave in waves {
        let _ = dispatch_wave_if_ready(store, executor, scheduler, event_hub, &wave).await;
    }
}

pub async fn dispatch_wave_if_ready(
    store: &SharedStore,
    executor: &WaveExecutor,
    scheduler: &Arc<Scheduler>,
    event_hub: &EventHub,
    wave: &Wave,
) -> Option<WaveRun> {
    if wave.status() == WaveStatus::Paused {
        return None;
    }
    if scheduler.has_active_session(wave.id().as_str()) {
        return None;
    }
    if store
        .get_active_wave_run(wave.id())
        .await
        .ok()
        .flatten()
        .is_some()
    {
        return None;
    }

    let pending = match store.list_pending_activations(wave.id()).await {
        Ok(mut pending) => {
            pending.sort_by_key(|activation| activation.queued_at);
            pending
        }
        Err(err) => {
            tracing::error!(wave_id = %wave.id(), error = %err, "failed to list pending activations");
            return None;
        }
    };
    let activation = pending.into_iter().next()?;

    let run_id = LfdId::new();
    let slot_guard = match scheduler.acquire_guard(run_id.as_str()).await {
        Ok(guard) => guard,
        Err(reason) => {
            tracing::debug!(wave_id = %wave.id(), reason = %reason, "scheduler at capacity; activation dispatch deferred");
            return None;
        }
    };

    let dispatch_log = ActivationLog::new(
        wave.id().clone(),
        activation.stimulus_id.clone(),
        activation.source,
        activation.reason.clone(),
        ActivationOutcome::Dispatched,
    );
    if let Err(err) = store.create_activation_log(&dispatch_log).await {
        tracing::error!(
            wave_id = %wave.id(),
            stimulus_id = %activation.stimulus_id,
            error = %err,
            "failed to write activation dispatch log"
        );
        return None;
    }

    let stimulus_flow_override = store
        .get_stimulus(&activation.stimulus_id)
        .await
        .ok()
        .flatten()
        .and_then(|stimulus| stimulus.flow);

    let target = if activation.target_branch.is_empty() || activation.target_branch == "main" {
        None
    } else {
        Some(activation.target_branch.as_str())
    };
    let mut run = match create_wave_run_with_id(store, wave, &run_id, target).await {
        Ok(run) => run,
        Err(err) => {
            tracing::error!(wave_id = %wave.id(), error = %err, "failed to create wave run for pending activation");
            return None;
        }
    };
    if let Some(flow_override) = stimulus_flow_override {
        run.snapshot.flow = flow_override;
    }
    run.target_branch = activation.target_branch.clone();
    run.activation_log_id = Some(dispatch_log.id.clone());
    if let Err(err) = store.update_wave_run(&run).await {
        tracing::error!(wave_id = %wave.id(), run_id = %run.id, error = %err, "failed to attach activation log to run");
    }

    if let Err(err) = store.delete_pending_activation_by_id(&activation.id).await {
        tracing::error!(
            wave_id = %wave.id(),
            activation_id = %activation.id,
            error = %err,
            "failed to delete dispatched pending activation"
        );
    }

    spawn_run_task_with_slot(
        store.clone(),
        executor.clone(),
        event_hub.clone(),
        run.clone(),
        slot_guard,
    );
    Some(run)
}

async fn record_activation_log(
    store: &SharedStore,
    event_hub: &EventHub,
    envelope: &ActivationEnvelope,
    outcome: ActivationOutcome,
    queue_depth: u32,
) {
    let log = ActivationLog::new(
        envelope.wave_id.clone(),
        envelope.stimulus_id.clone(),
        envelope.source,
        envelope.reason.clone(),
        outcome,
    );
    if let Err(err) = store.create_activation_log(&log).await {
        tracing::error!(
            wave_id = %envelope.wave_id,
            stimulus_id = %envelope.stimulus_id,
            error = %err,
            "failed to write activation log"
        );
        return;
    }

    tracing::info!(
        wave_id = %envelope.wave_id,
        stimulus_id = %envelope.stimulus_id,
        source = envelope.source.as_str(),
        reason = %envelope.reason,
        queue_depth,
        outcome = outcome.as_str(),
        "activation queue event"
    );

    match outcome {
        ActivationOutcome::Queued => event_hub.send(Event::activation_queued(
            envelope.wave_id.clone(),
            envelope.stimulus_id.clone(),
            envelope.source,
            envelope.reason.clone(),
            queue_depth,
        )),
        ActivationOutcome::Coalesced => event_hub.send(Event::activation_coalesced(
            envelope.wave_id.clone(),
            envelope.stimulus_id.clone(),
            envelope.source,
            envelope.reason.clone(),
            queue_depth,
        )),
        ActivationOutcome::Dropped => event_hub.send(Event::activation_dropped(
            envelope.wave_id.clone(),
            envelope.stimulus_id.clone(),
            envelope.source,
            envelope.reason.clone(),
            queue_depth,
        )),
        ActivationOutcome::Dispatched => {}
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::lfd::store::{open_store, StorageConfig};
    use crate::lfd::types::{Signal, Stimulus, Wave};
    use time::OffsetDateTime;

    async fn create_store() -> SharedStore {
        let path = std::env::temp_dir().join(format!("lfd-activation-test-{}.db", LfdId::new()));
        open_store(&StorageConfig::sqlite(path))
            .await
            .map(Arc::new)
            .expect("store")
    }

    async fn create_wave(store: &SharedStore) -> Wave {
        let wave = Wave {
            id: LfdId::new(),
            name: "activation-wave".to_string(),
            repo: ".".to_string(),
            flow: "build".to_string(),
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

    async fn create_stimulus(store: &SharedStore, wave_id: &LfdId) -> Stimulus {
        let stimulus = Stimulus {
            id: LfdId::new(),
            wave_id: wave_id.clone(),
            source_wave_id: None,
            signal: Signal::Watch,
            flow: None,
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
    async fn enqueue_coalesces_pending_activation_for_same_stimulus() {
        let store = create_store().await;
        let event_hub = EventHub::new(16);
        let mut events = event_hub.subscribe();
        let wave = create_wave(&store).await;
        let stimulus = create_stimulus(&store, &wave.id).await;

        let first = enqueue_pending_activation(
            &store,
            &event_hub,
            ActivationEnvelope::new(
                &wave.id,
                &stimulus.id,
                ActivationSource::Poll,
                "watch poll",
                "abc",
                "def",
                "main",
            ),
        )
        .await;
        let second = enqueue_pending_activation(
            &store,
            &event_hub,
            ActivationEnvelope::new(
                &wave.id,
                &stimulus.id,
                ActivationSource::Push,
                "push webhook",
                "def",
                "fed",
                "main",
            ),
        )
        .await;

        assert_eq!(first, Some(EnqueueOutcome::Queued));
        assert_eq!(second, Some(EnqueueOutcome::Coalesced));

        let pending = store
            .list_pending_activations(&wave.id)
            .await
            .expect("list pending");
        assert_eq!(pending.len(), 1);
        assert_eq!(pending[0].source, ActivationSource::Push);
        assert_eq!(pending[0].to_sha, "fed");

        let queued = events.try_recv().expect("queued event");
        let coalesced = events.try_recv().expect("coalesced event");
        assert!(matches!(queued, Event::ActivationQueued { .. }));
        assert!(matches!(coalesced, Event::ActivationCoalesced { .. }));
    }

    #[tokio::test]
    async fn enqueue_drops_when_wave_queue_is_full() {
        let store = create_store().await;
        let event_hub = EventHub::new(64);
        let wave = create_wave(&store).await;

        for _ in 0..DEFAULT_ACTIVATION_QUEUE_LIMIT {
            let stimulus = create_stimulus(&store, &wave.id).await;
            let outcome = enqueue_pending_activation(
                &store,
                &event_hub,
                ActivationEnvelope::new(
                    &wave.id,
                    &stimulus.id,
                    ActivationSource::Poll,
                    "fill queue",
                    "",
                    "",
                    "main",
                ),
            )
            .await;
            assert_eq!(outcome, Some(EnqueueOutcome::Queued));
        }

        let overflow_stimulus = create_stimulus(&store, &wave.id).await;
        let outcome = enqueue_pending_activation(
            &store,
            &event_hub,
            ActivationEnvelope::new(
                &wave.id,
                &overflow_stimulus.id,
                ActivationSource::Push,
                "overflow",
                "",
                "",
                "main",
            ),
        )
        .await;
        assert_eq!(outcome, Some(EnqueueOutcome::Dropped));

        let pending = store
            .list_pending_activations(&wave.id)
            .await
            .expect("list pending");
        assert_eq!(pending.len(), DEFAULT_ACTIVATION_QUEUE_LIMIT);
    }
}
