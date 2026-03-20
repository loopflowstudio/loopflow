use std::path::Path;
use std::sync::Arc;
use std::time::Duration;

use tokio::task::JoinHandle;
use tokio_util::sync::CancellationToken;

use super::{dispatch_or_enqueue_activation, ActivationEnvelope};
use crate::engine::worktrees::worktree_path;
use crate::lfd::events::EventHub;
use crate::lfd::executor::WaveExecutor;
use crate::lfd::scheduler::Scheduler;
use crate::lfd::store::SharedStore;
use crate::lfd::types::{Trigger, Wave, WaveStatus};

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
        if wave.status() == WaveStatus::Paused {
            continue;
        }
        if scheduler.has_active_session(wave.id().as_str()) {
            continue;
        }

        let active_runs = match store.count_active_wave_runs(wave.id()).await {
            Ok(count) => count,
            Err(err) => {
                tracing::error!(wave_id = %wave.id(), error = %err, "failed to count active loop runs");
                continue;
            }
        };
        let pending_count = match store.list_pending_activations(wave.id()).await {
            Ok(pending) => pending.len() as u32,
            Err(err) => {
                tracing::error!(wave_id = %wave.id(), error = %err, "failed to list pending activations");
                continue;
            }
        };
        if active_runs + pending_count >= wave.workers() {
            continue;
        }

        let worktree = worktree_path(Path::new(wave.repo()), wave.name());
        let wave_dir = worktree.join("wave").join(wave.name());

        // Skip if wave dir was removed (cycle complete).
        if worktree.exists() && !wave_dir.exists() {
            tracing::info!(wave = %wave.name(), "wave dir removed, skipping loop tick");
            continue;
        }

        // Safety valve: check max_iterations on any trigger for this wave.
        if let Ok(triggers) = store.list_triggers(Some(wave.id())).await {
            if let Some(trigger) = triggers.iter().find(|t| t.max_iterations.is_some()) {
                if should_pause_for_max_iterations(trigger, &wave) {
                    tracing::warn!(
                        wave = %wave.name(),
                        iteration = wave.iteration(),
                        max = trigger.max_iterations.unwrap_or(0),
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
            "loop ticker observed idle wave",
            "",
            "",
            "main",
        );
        let _ = dispatch_or_enqueue_activation(
            store,
            executor,
            scheduler,
            event_hub,
            &wave,
            Some(wave.primary_flow().to_string()),
            envelope,
        )
        .await;
    }
}

fn should_pause_for_max_iterations(trigger: &Trigger, wave: &Wave) -> bool {
    let Some(max) = trigger.max_iterations else {
        return false;
    };
    let cycle_iterations = wave
        .iteration()
        .saturating_sub(wave.cycle_start_iteration())
        + 1;
    max > 0 && cycle_iterations >= max
}

#[cfg(test)]
mod tests {
    use super::should_pause_for_max_iterations;
    use crate::lfd::id::LfdId;
    use crate::lfd::types::{Signal, Trigger, Wave, WaveMode, WaveStatus};
    use time::OffsetDateTime;

    fn make_trigger(max_iterations: Option<u32>) -> Trigger {
        let mut trigger = Trigger::new(LfdId::new(), LfdId::new(), Signal::Repo);
        trigger.max_iterations = max_iterations;
        trigger
    }

    fn make_wave(iteration: u32, cycle_start_iteration: u32) -> Wave {
        Wave {
            id: LfdId::new(),
            name: "loop-wave".to_string(),
            repo: "/tmp/repo".to_string(),
            mode: WaveMode::Loop,
            primary_flow: "ship-roadmap".to_string(),
            cron: None,
            direction: Vec::new(),
            area: Vec::new(),
            status: WaveStatus::Idle,
            iteration,
            cycle_start_iteration,
            created_at: Some(OffsetDateTime::now_utc()),
            workers: 1,
        }
    }

    #[test]
    fn no_max_iterations() {
        let trigger = make_trigger(None);
        let wave = make_wave(2, 0);
        assert!(!should_pause_for_max_iterations(&trigger, &wave));
    }

    #[test]
    fn max_zero() {
        let trigger = make_trigger(Some(0));
        let wave = make_wave(2, 0);
        assert!(!should_pause_for_max_iterations(&trigger, &wave));
    }

    #[test]
    fn below_limit() {
        let trigger = make_trigger(Some(5));
        let wave = make_wave(2, 0);
        assert!(!should_pause_for_max_iterations(&trigger, &wave));
    }

    #[test]
    fn at_limit() {
        let trigger = make_trigger(Some(5));
        let wave = make_wave(4, 0);
        assert!(should_pause_for_max_iterations(&trigger, &wave));
    }

    #[test]
    fn with_offset() {
        let trigger = make_trigger(Some(5));
        let wave = make_wave(7, 5);
        assert!(!should_pause_for_max_iterations(&trigger, &wave));
    }
}
