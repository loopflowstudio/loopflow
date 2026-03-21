use std::path::Path;
use std::sync::Arc;
use std::time::Duration;

use anyhow::anyhow;
use tokio::task::JoinHandle;
use tokio_util::sync::CancellationToken;

use super::common::{create_parallel_wave_run, create_wave_run_with_id, spawn_run_task_with_slot};
use crate::lfd::events::EventHub;
use crate::lfd::executor::cleanup_workspace_worktree;
use crate::lfd::executor::WaveExecutor;
use crate::lfd::id::LfdId;
use crate::lfd::scheduler::Scheduler;
use crate::lfd::store::SharedStore;
use crate::lfd::types::{
    ActivationLog, ActivationOutcome, Event, PendingActivation, Wave, WaveRun, WaveStatus,
};
use crate::ops::{ingest, IngestOptions, NullProgress};

pub const DEFAULT_ACTIVATION_QUEUE_LIMIT: usize = 20;

fn activation_requires_manual_resolution(error: &anyhow::Error) -> bool {
    error
        .to_string()
        .to_ascii_lowercase()
        .contains("manual resolution required")
}

async fn pause_wave_after_activation_conflict(
    store: &SharedStore,
    event_hub: &EventHub,
    wave: &Wave,
    error: &anyhow::Error,
) {
    let Some(mut paused_wave) = store.get_wave(wave.id()).await.ok().flatten() else {
        return;
    };
    if paused_wave.status == WaveStatus::Paused {
        return;
    }
    paused_wave.status = WaveStatus::Paused;
    if let Err(update_error) = store.update_wave(&paused_wave).await {
        tracing::warn!(
            wave_id = %wave.id(),
            error = %update_error,
            "failed to pause wave after activation conflict"
        );
        return;
    }
    tracing::warn!(
        wave_id = %wave.id(),
        error = %error,
        "paused wave after activation conflict to stop retry loop"
    );
    event_hub.send(Event::wave_updated(wave.id().clone()));
}

async fn create_wave_run(
    store: &SharedStore,
    wave: &Wave,
    run_id: &LfdId,
    target_branch: Option<&str>,
) -> anyhow::Result<WaveRun> {
    if wave.workers() == 1 {
        create_wave_run_with_id(store, wave, run_id, target_branch).await
    } else {
        create_parallel_wave_run(store, wave, run_id, target_branch).await
    }
}

#[derive(Debug, Clone)]
pub struct ActivationEnvelope {
    pub wave_id: LfdId,
    pub trigger_id: Option<LfdId>,
    pub reason: String,
    pub from_sha: String,
    pub to_sha: String,
    pub target_branch: String,
}

impl ActivationEnvelope {
    pub fn new(
        wave_id: &LfdId,
        trigger_id: Option<&LfdId>,
        reason: impl Into<String>,
        from_sha: impl Into<String>,
        to_sha: impl Into<String>,
        target_branch: impl Into<String>,
    ) -> Self {
        Self {
            wave_id: wave_id.clone(),
            trigger_id: trigger_id.cloned(),
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
        .get_pending_for_trigger(&envelope.wave_id, envelope.trigger_id.as_ref())
        .await
    {
        Ok(Some(mut existing)) => {
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
                    trigger_id = ?envelope.trigger_id,
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
                        trigger_id = ?envelope.trigger_id,
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
                trigger_id: envelope.trigger_id.clone(),
                reason: envelope.reason.clone(),
                from_sha: envelope.from_sha.clone(),
                to_sha: envelope.to_sha.clone(),
                queued_at: time::OffsetDateTime::now_utc().unix_timestamp(),
                target_branch: envelope.target_branch.clone(),
            };
            if let Err(err) = store.create_pending_activation(&activation).await {
                tracing::error!(
                    wave_id = %envelope.wave_id,
                    trigger_id = ?envelope.trigger_id,
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
                trigger_id = ?envelope.trigger_id,
                error = %err,
                "failed to read pending activation for trigger"
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
    flow_override: Option<String>,
    roadmap_item: Option<String>,
    envelope: ActivationEnvelope,
) -> anyhow::Result<Option<WaveRun>> {
    if wave.status() == WaveStatus::Paused {
        return Ok(None);
    }
    if scheduler.has_active_session(wave.id().as_str()) {
        return Ok(None);
    }

    let run_id = LfdId::new();
    let slot_guard = match scheduler.acquire_guard(run_id.as_str()).await {
        Ok(guard) => guard,
        Err(_reason) => {
            // Fall back to queue when scheduler is full.
            let _ = enqueue_pending_activation(store, event_hub, envelope).await;
            return Ok(None);
        }
    };

    let dispatch_log = ActivationLog::new(
        wave.id().clone(),
        envelope.trigger_id.clone(),
        envelope.reason.clone(),
        ActivationOutcome::Dispatched,
    );
    if let Err(err) = store.create_activation_log(&dispatch_log).await {
        tracing::error!(
            wave_id = %wave.id(),
            trigger_id = ?envelope.trigger_id,
            error = %err,
            "failed to write immediate activation dispatch log"
        );
        return Err(anyhow!("failed to write activation log: {err}"));
    }

    let target = if envelope.target_branch.is_empty() || envelope.target_branch == "main" {
        None
    } else {
        Some(envelope.target_branch.as_str())
    };
    let create_run = if wave.serialized {
        create_wave_run_with_id(store, wave, &run_id, target).await
    } else {
        create_parallel_wave_run(store, wave, &run_id, target).await
    };

    let mut run = match create_run {
        Ok(run) => run,
        Err(err) => {
            tracing::error!(
                wave_id = %wave.id(),
                error = %err,
                "failed to create parallel wave run for immediate activation"
            );
            if activation_requires_manual_resolution(&err) {
                pause_wave_after_activation_conflict(store, event_hub, wave, &err).await;
            }
            return Err(err);
        }
    };
    if let Some(flow_override) = flow_override {
        run.snapshot.flow = flow_override;
    }
    if let Some(item) = roadmap_item {
        if let Err(err) = ingest(
            Path::new(&run.worktree),
            &IngestOptions {
                wave: Some(wave.name().clone()),
                item: Some(item.clone()),
            },
            &NullProgress,
        ) {
            tracing::error!(
                wave_id = %wave.id(),
                run_id = %run.id,
                item = %item,
                error = %err,
                "failed targeted ingest before immediate activation"
            );
            run.status = crate::lfd::types::WaveRunStatus::Failed;
            run.ended_at = Some(time::OffsetDateTime::now_utc());
            run.error = Some(format!("ingest failed: {err}"));
            let _ = store.update_wave_run(&run).await;

            if let Ok(Some(mut stored_wave)) = store.get_wave(wave.id()).await {
                stored_wave.status = WaveStatus::Idle;
                let _ = store.update_wave(&stored_wave).await;
            }

            let _ = cleanup_workspace_worktree(Path::new(&run.worktree));
            event_hub.send(Event::wave_updated(wave.id().clone()));
            return Err(anyhow!("ingest failed: {err}"));
        }
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
    Ok(Some(run))
}

pub async fn dispatch_or_enqueue_activation(
    store: &SharedStore,
    executor: &WaveExecutor,
    scheduler: &Arc<Scheduler>,
    event_hub: &EventHub,
    wave: &Wave,
    flow_override: Option<String>,
    envelope: ActivationEnvelope,
) -> bool {
    let active_runs = match store.count_active_wave_runs(wave.id()).await {
        Ok(count) => count,
        Err(err) => {
            tracing::error!(
                wave_id = %wave.id(),
                error = %err,
                "failed to count active wave runs"
            );
            return false;
        }
    };

    if active_runs >= wave.workers() {
        let outcome = enqueue_pending_activation(store, event_hub, envelope).await;
        return matches!(
            outcome,
            Some(EnqueueOutcome::Queued | EnqueueOutcome::Coalesced)
        );
    }

    let result = spawn_immediate_activation(
        store,
        executor,
        scheduler,
        event_hub,
        wave,
        flow_override,
        envelope,
    )
    .await;
    result.is_some()
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
    let active_runs = match store.count_active_wave_runs(wave.id()).await {
        Ok(count) => count,
        Err(err) => {
            tracing::error!(wave_id = %wave.id(), error = %err, "failed to count active wave runs");
            return None;
        }
    };
    if active_runs >= wave.workers() {
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
        activation.trigger_id.clone(),
        activation.reason.clone(),
        ActivationOutcome::Dispatched,
    );
    if let Err(err) = store.create_activation_log(&dispatch_log).await {
        tracing::error!(
            wave_id = %wave.id(),
            trigger_id = ?activation.trigger_id,
            error = %err,
            "failed to write activation dispatch log"
        );
        return None;
    }

    let trigger_flow_override = match &activation.trigger_id {
        Some(trigger_id) => store
            .get_trigger(trigger_id)
            .await
            .ok()
            .flatten()
            .and_then(|trigger| trigger.flow),
        None => None,
    };

    let target = if activation.target_branch.is_empty() || activation.target_branch == "main" {
        None
    } else {
        Some(activation.target_branch.as_str())
    };
    let mut run = match create_wave_run(store, wave, &run_id, target).await {
        Ok(run) => run,
        Err(err) => {
            tracing::error!(wave_id = %wave.id(), error = %err, "failed to create wave run for pending activation");
            if activation_requires_manual_resolution(&err) {
                pause_wave_after_activation_conflict(store, event_hub, wave, &err).await;
            }
            return None;
        }
    };
    if let Some(flow_override) = trigger_flow_override {
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
        envelope.trigger_id.clone(),
        envelope.reason.clone(),
        outcome,
    );
    if let Err(err) = store.create_activation_log(&log).await {
        tracing::error!(
            wave_id = %envelope.wave_id,
            trigger_id = ?envelope.trigger_id,
            error = %err,
            "failed to write activation log"
        );
        return;
    }

    tracing::info!(
        wave_id = %envelope.wave_id,
        trigger_id = ?envelope.trigger_id,
        reason = %envelope.reason,
        queue_depth,
        outcome = outcome.as_str(),
        "activation queue event"
    );

    match outcome {
        ActivationOutcome::Queued => event_hub.send(Event::activation_queued(
            envelope.wave_id.clone(),
            envelope.trigger_id.clone(),
            envelope.reason.clone(),
            queue_depth,
        )),
        ActivationOutcome::Coalesced => event_hub.send(Event::activation_coalesced(
            envelope.wave_id.clone(),
            envelope.trigger_id.clone(),
            envelope.reason.clone(),
            queue_depth,
        )),
        ActivationOutcome::Dropped => event_hub.send(Event::activation_dropped(
            envelope.wave_id.clone(),
            envelope.trigger_id.clone(),
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
    use crate::lfd::types::{Signal, Trigger, Wave, WaveMode};
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

    async fn create_trigger(store: &SharedStore, wave_id: &LfdId) -> Trigger {
        let trigger = Trigger {
            id: LfdId::new(),
            wave_id: wave_id.clone(),
            source_wave_id: None,
            signal: Signal::Repo,
            flow: None,
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
    async fn enqueue_coalesces_pending_activation_for_same_trigger() {
        let store = create_store().await;
        let event_hub = EventHub::new(16);
        let mut events = event_hub.subscribe();
        let wave = create_wave(&store).await;
        let trigger = create_trigger(&store, &wave.id).await;

        let first = enqueue_pending_activation(
            &store,
            &event_hub,
            ActivationEnvelope::new(
                &wave.id,
                Some(&trigger.id),
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
                Some(&trigger.id),
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
            let trigger = create_trigger(&store, &wave.id).await;
            let outcome = enqueue_pending_activation(
                &store,
                &event_hub,
                ActivationEnvelope::new(&wave.id, Some(&trigger.id), "fill queue", "", "", "main"),
            )
            .await;
            assert_eq!(outcome, Some(EnqueueOutcome::Queued));
        }

        let overflow_trigger = create_trigger(&store, &wave.id).await;
        let outcome = enqueue_pending_activation(
            &store,
            &event_hub,
            ActivationEnvelope::new(
                &wave.id,
                Some(&overflow_trigger.id),
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
