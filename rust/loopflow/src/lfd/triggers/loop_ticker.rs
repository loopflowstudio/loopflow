use std::path::Path;
use std::time::Duration;

use tokio::task::JoinHandle;
use tokio_util::sync::CancellationToken;

use super::{enqueue_pending_activation, ActivationEnvelope};
use crate::engine::worktrees::worktree_path;
use crate::lfd::events::EventHub;
use crate::lfd::scheduler::Scheduler;
use crate::lfd::store::SharedStore;
use crate::lfd::types::ActivationSource;
use crate::lfd::types::{StimulusKind, WaveStatus};

pub fn spawn_loop_ticker(
    scheduler: std::sync::Arc<Scheduler>,
    store: SharedStore,
    event_hub: EventHub,
    cancel: CancellationToken,
) -> JoinHandle<()> {
    tokio::spawn(async move {
        let mut interval = tokio::time::interval(Duration::from_secs(5));
        loop {
            tokio::select! {
                _ = cancel.cancelled() => {
                    tracing::info!("loop_ticker shutting down");
                    break;
                }
                _ = interval.tick() => {
                    tick_loop_waves(&scheduler, &store, &event_hub).await;
                }
            }
        }
    })
}

async fn tick_loop_waves(
    scheduler: &std::sync::Arc<Scheduler>,
    store: &SharedStore,
    event_hub: &EventHub,
) {
    let stimuli = match store
        .list_stimuli_by_kind(StimulusKind::Loop.as_i32())
        .await
    {
        Ok(stimuli) => stimuli,
        Err(err) => {
            tracing::error!(error = %err, "failed to list loop stimuli");
            return;
        }
    };

    for stimulus in stimuli {
        if !stimulus.enabled {
            continue;
        }

        let wave = match store.get_wave(&stimulus.wave_id).await {
            Ok(Some(wave)) => wave,
            Ok(None) => continue,
            Err(err) => {
                tracing::error!(wave_id = %stimulus.wave_id, error = %err, "failed to load wave");
                continue;
            }
        };

        if wave.status() == WaveStatus::Paused {
            continue;
        }

        if scheduler.has_active_session(wave.id().as_str()) {
            continue;
        }

        if let Ok(Some(_)) = store.get_active_wave_run(&stimulus.wave_id).await {
            continue;
        }
        if let Ok(pending) = store.list_pending_activations(&stimulus.wave_id).await {
            if !pending.is_empty() {
                continue;
            }
        }

        let worktree = worktree_path(Path::new(wave.repo()), wave.name());
        let wave_dir = worktree.join("wave").join(wave.name());
        if worktree.exists() && !wave_dir.exists() {
            tracing::info!(wave = %wave.name(), "wave dir removed, skipping loop tick");
            continue;
        }

        let _ = enqueue_pending_activation(
            store,
            event_hub,
            ActivationEnvelope::new(
                wave.id(),
                &stimulus.id,
                ActivationSource::Poll,
                "loop ticker observed idle wave",
                "",
                "",
            ),
        )
        .await;
    }
}
