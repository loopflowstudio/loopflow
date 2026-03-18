use std::sync::Arc;

use tokio::task::JoinHandle;
use tokio_util::sync::CancellationToken;

use super::activation::{enqueue_pending_activation, ActivationEnvelope, EnqueueOutcome};
use super::spawn_immediate_activation;
use crate::lfd::events::EventHub;
use crate::lfd::executor::WaveExecutor;
use crate::lfd::scheduler::Scheduler;
use crate::lfd::store::SharedStore;
use crate::lfd::types::{Event, Signal, Wave, WaveStatus};
use time::OffsetDateTime;

#[derive(Debug)]
struct BlockActivation {
    wave_id: crate::lfd::id::LfdId,
    reason: String,
}

pub fn spawn_block_handler(
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
                    tracing::info!("block_handler shutting down");
                    break;
                }
                event = rx.recv() => {
                    match event {
                        Ok(Event::WaveBlocked {
                            wave_id, reason, ..
                        }) => {
                            let activation = BlockActivation { wave_id, reason };
                            if let Err(err) = handle_block_event(
                                &store,
                                &executor,
                                &scheduler,
                                &event_hub,
                                activation,
                            ).await {
                                tracing::warn!(error = %err, "failed handling wave block activation");
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

async fn handle_block_event(
    store: &SharedStore,
    executor: &WaveExecutor,
    scheduler: &Arc<Scheduler>,
    event_hub: &EventHub,
    activation: BlockActivation,
) -> Result<(), String> {
    let source_wave = store
        .get_wave(&activation.wave_id)
        .await
        .map_err(|err| err.to_string())?
        .ok_or_else(|| format!("wave {} not found", activation.wave_id))?;

    let triggers = store
        .list_triggers_by_signal(Signal::Block.as_i32())
        .await
        .map_err(|err| err.to_string())?;

    for mut trigger in triggers {
        if !trigger.enabled {
            continue;
        }

        let listener_wave = match store.get_wave(&trigger.wave_id).await {
            Ok(Some(wave)) => wave,
            Ok(None) => continue,
            Err(err) => {
                tracing::warn!(trigger_id = %trigger.id, error = %err, "failed to load listening wave");
                continue;
            }
        };

        if !matches_area_member(&listener_wave, &source_wave)
            || listener_wave.status() == WaveStatus::Paused
        {
            continue;
        }

        let reason = format!(
            "member wave {} blocked: {}",
            source_wave.name(),
            activation.reason
        );
        let envelope = ActivationEnvelope::new(
            listener_wave.id(),
            Some(&trigger.id),
            reason,
            "",
            "",
            "main",
        );

        let activated = if listener_wave.serialized {
            let outcome = enqueue_pending_activation(store, event_hub, envelope).await;
            if matches!(
                outcome,
                Some(EnqueueOutcome::Queued | EnqueueOutcome::Coalesced)
            ) {
                let _ = super::dispatch_wave_if_ready(
                    store,
                    executor,
                    scheduler,
                    event_hub,
                    &listener_wave,
                )
                .await;
                true
            } else {
                false
            }
        } else {
            spawn_immediate_activation(
                store,
                executor,
                scheduler,
                event_hub,
                &listener_wave,
                trigger.flow.clone(),
                envelope,
            )
            .await
            .is_some()
        };

        if activated {
            trigger.last_triggered_at = Some(OffsetDateTime::now_utc().unix_timestamp());
            if let Err(err) = store.update_trigger(&trigger).await {
                tracing::warn!(
                    trigger_id = %trigger.id,
                    error = %err,
                    "failed to update block trigger last_triggered_at"
                );
            }
        }
    }
    Ok(())
}

fn matches_area_member(listener_wave: &Wave, source_wave: &Wave) -> bool {
    if listener_wave.repo() != source_wave.repo() {
        return false;
    }
    let expected = format!("wave/{}/", source_wave.name());
    let without_slash = expected.trim_end_matches('/');
    listener_wave.area().iter().any(|entry| {
        let trimmed = entry.trim();
        trimmed == expected || trimmed == without_slash
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::lfd::config::{ExecutorConfig, GitHubConfig};
    use crate::lfd::id::LfdId;
    use crate::lfd::output::OutputHub;
    use crate::lfd::sessions::SessionManager;
    use crate::lfd::store::{open_store, StorageConfig};
    use crate::lfd::types::{Trigger, WaveMode};
    use std::sync::Arc;

    async fn create_store() -> SharedStore {
        let path = std::env::temp_dir().join(format!("lfd-block-test-{}.db", LfdId::new()));
        open_store(&StorageConfig::sqlite(path))
            .await
            .map(Arc::new)
            .expect("store")
    }

    fn make_wave(repo: &str, name: &str, area: Vec<&str>, serialized: bool) -> Wave {
        Wave {
            id: LfdId::new(),
            name: name.to_string(),
            repo: repo.to_string(),
            mode: WaveMode::Loop,
            primary_flow: "tend".to_string(),
            cron: None,
            direction: Vec::new(),
            area: area.into_iter().map(ToString::to_string).collect(),
            status: WaveStatus::Idle,
            iteration: 0,
            cycle_start_iteration: 0,
            created_at: Some(OffsetDateTime::now_utc()),
            serialized,
        }
    }

    #[tokio::test]
    async fn block_signal_fires_chord_tend() {
        let store = create_store().await;
        let scheduler = Arc::new(Scheduler::new(2));
        let output_dir = tempfile::tempdir().expect("output dir");
        let output = OutputHub::new(16, output_dir.path().to_path_buf());
        let event_hub = EventHub::new(64);
        let executor = WaveExecutor::new(
            store.clone(),
            scheduler.clone(),
            output,
            event_hub.clone(),
            SessionManager::new(store.clone()),
            ExecutorConfig::default(),
            GitHubConfig::default(),
        )
        .expect("executor");

        let member = make_wave("/repo", "signals", Vec::new(), false);
        let chord = make_wave("/repo", "redesign", vec!["wave/signals/"], true);
        store.create_wave(&member).await.expect("member wave");
        store.create_wave(&chord).await.expect("chord wave");

        let trigger = Trigger {
            id: LfdId::new(),
            wave_id: chord.id.clone(),
            source_wave_id: None,
            signal: Signal::Block,
            flow: Some("tend".to_string()),
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
        let active_run = crate::lfd::types::WaveRun {
            id: LfdId::new(),
            wave_id: chord.id.clone(),
            snapshot: crate::lfd::types::WaveRunSnapshot {
                repo: chord.repo.clone(),
                flow: chord.primary_flow.clone(),
                direction: Vec::new(),
                area: chord.area.clone(),
                pr: None,
            },
            iteration: 0,
            step_index: 0,
            status: crate::lfd::types::WaveRunStatus::Running,
            worktree: chord.repo.clone(),
            branch: "main".to_string(),
            started_at: Some(OffsetDateTime::now_utc()),
            ended_at: None,
            error: None,
            flow_parents: Vec::new(),
            activation_log_id: None,
            parent_run_id: None,
            parent_pr_number: None,
            stack_position: 0,
            stack_group_id: chord.id.to_string(),
            stack_status: crate::lfd::types::WaveRunStackStatus::Active,
            lineage_inferred: false,
            target_branch: "main".to_string(),
        };
        store
            .create_wave_run(&active_run)
            .await
            .expect("active run");

        handle_block_event(
            &store,
            &executor,
            &scheduler,
            &event_hub,
            BlockActivation {
                wave_id: member.id.clone(),
                reason: "rebase_conflict".to_string(),
            },
        )
        .await
        .expect("handle block");

        let pending = store
            .get_pending_for_trigger(&chord.id, Some(&trigger.id))
            .await
            .expect("pending lookup")
            .expect("pending activation");
        assert_eq!(
            pending.reason,
            "member wave signals blocked: rebase_conflict"
        );
    }
}
