use std::path::Path;
use std::sync::Arc;
use std::time::Duration;

use tokio::task::JoinHandle;
use tokio_util::sync::CancellationToken;

use super::{enqueue_pending_activation, spawn_immediate_activation, ActivationEnvelope};
use crate::engine::worktrees::worktree_path;
use crate::lfd::events::EventHub;
use crate::lfd::executor::WaveExecutor;
use crate::lfd::scheduler::Scheduler;
use crate::lfd::store::SharedStore;
use crate::lfd::types::ActivationSource;
use crate::lfd::types::{Stimulus, Wave, WaveStatus};

pub fn spawn_loop_ticker(
    scheduler: Arc<Scheduler>,
    executor: WaveExecutor,
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
                    tick_loop_waves(&scheduler, &executor, &store, &event_hub).await;
                }
            }
        }
    })
}

async fn tick_loop_waves(
    scheduler: &Arc<Scheduler>,
    executor: &WaveExecutor,
    store: &SharedStore,
    event_hub: &EventHub,
) {
    let waves = match store.list_loopable_waves().await {
        Ok(waves) => waves,
        Err(err) => {
            tracing::error!(error = %err, "failed to list loopable waves");
            return;
        }
    };

    for wave in waves {
        if scheduler.has_active_session(wave.id().as_str()) {
            continue;
        }

        // Skip if there's already an active run or pending activation.
        // Loop mode means "keep running whenever idle", so we only re-trigger
        // when no runs are active.
        if let Ok(Some(_)) = store.get_active_wave_run(wave.id()).await {
            continue;
        }
        if let Ok(pending) = store.list_pending_activations(wave.id()).await {
            if !pending.is_empty() {
                continue;
            }
        }

        let worktree = worktree_path(Path::new(wave.repo()), wave.name());
        let wave_dir = worktree.join("wave").join(wave.name());

        // Skip if wave dir was removed (cycle complete).
        if worktree.exists() && !wave_dir.exists() {
            tracing::info!(wave = %wave.name(), "wave dir removed, skipping loop tick");
            continue;
        }

        // Safety valve: check max_iterations on any stimulus for this wave.
        if let Ok(stimuli) = store.list_stimuli(Some(wave.id())).await {
            if let Some(stimulus) = stimuli.iter().find(|s| s.max_iterations.is_some()) {
                if should_pause_for_max_iterations(stimulus, &wave) {
                    tracing::warn!(
                        wave = %wave.name(),
                        iteration = wave.iteration(),
                        max = stimulus.max_iterations.unwrap_or(0),
                        "max iterations exceeded, pausing wave"
                    );
                    let mut paused_wave = wave.clone();
                    paused_wave.status = WaveStatus::Paused;
                    if let Err(err) = store.update_wave(&paused_wave).await {
                        tracing::error!(
                            wave_id = %paused_wave.id,
                            error = %err,
                            "failed to pause wave after max iterations"
                        );
                    }
                    continue;
                }
            }
        }

        let envelope = ActivationEnvelope::new(
            wave.id(),
            None,
            ActivationSource::Poll,
            "loop ticker observed idle wave",
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
                Some(wave.loop_flow().to_string()),
                envelope,
            )
            .await;
        }
    }
}

fn should_pause_for_max_iterations(stimulus: &Stimulus, wave: &Wave) -> bool {
    let Some(max) = stimulus.max_iterations else {
        return false;
    };
    let cycle_iterations = wave
        .iteration()
        .saturating_sub(wave.cycle_start_iteration())
        + 1;
    max > 0 && cycle_iterations >= max
}
