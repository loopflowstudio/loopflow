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
use crate::lfd::types::{Signal, Stimulus, Wave, WaveStatus};

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
    // Collect stimuli that can trigger re-runs: Loop and Cron.
    let mut stimuli = match store.list_stimuli_by_signal(Signal::Loop.as_i32()).await {
        Ok(stimuli) => stimuli,
        Err(err) => {
            tracing::error!(error = %err, "failed to list loop stimuli");
            return;
        }
    };

    // Cron waves with active wave state should also loop within a cycle.
    match store.list_stimuli_by_signal(Signal::Cron.as_i32()).await {
        Ok(cron_stimuli) => stimuli.extend(cron_stimuli),
        Err(err) => {
            tracing::error!(error = %err, "failed to list cron stimuli");
        }
    }

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

        // For serialized waves, skip if there's already an active run or pending activation.
        // For non-serialized waves, the loop ticker still checks for idle state — a loop
        // stimulus means "keep running whenever idle", so we only re-trigger when no runs
        // are active.
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

        // For Loop stimuli: skip if wave dir was removed (cycle complete).
        // For Cron stimuli: only re-trigger if wave dir still exists (mid-cycle).
        if stimulus.signal == Signal::Loop {
            if worktree.exists() && !wave_dir.exists() {
                tracing::info!(wave = %wave.name(), "wave dir removed, skipping loop tick");
                continue;
            }
        } else if stimulus.signal == Signal::Cron {
            // Cron stimuli only loop within a cycle — wave state presence is the signal.
            if !wave_dir.exists() {
                continue;
            }
        }

        // Safety valve: check max_iterations before re-triggering.
        if should_pause_for_max_iterations(&stimulus, &wave) {
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

        let envelope = ActivationEnvelope::new(
            wave.id(),
            &stimulus.id,
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
                stimulus.flow.clone(),
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
    max > 0 && wave.iteration() >= max
}
