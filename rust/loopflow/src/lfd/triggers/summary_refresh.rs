use std::collections::HashMap;
use std::time::{Duration, Instant};

use tokio::task::JoinHandle;
use tokio_util::sync::CancellationToken;

use crate::lfd::events::EventHub;
use crate::lfd::executor::WaveExecutor;
use crate::lfd::store::SharedStore;
use crate::lfd::types::{Event, WaveStatus};

const DEBOUNCE_SECS: u64 = 60;

pub fn spawn_summary_refresh(
    store: SharedStore,
    executor: WaveExecutor,
    event_hub: EventHub,
    cancel: CancellationToken,
) -> JoinHandle<()> {
    let mut rx = event_hub.subscribe();

    tokio::spawn(async move {
        let mut last_refresh: HashMap<String, Instant> = HashMap::new();

        loop {
            tokio::select! {
                _ = cancel.cancelled() => {
                    tracing::info!("summary_refresh shutting down");
                    break;
                }
                event = rx.recv() => {
                    match event {
                        Ok(Event::WorktreeUpdated { repo, .. }) => {
                            refresh_summaries_for_repo(
                                &store, &executor, &repo, &mut last_refresh
                            ).await;
                        }
                        Err(tokio::sync::broadcast::error::RecvError::Closed) => break,
                        _ => {}
                    }
                }
            }
        }
    })
}

async fn refresh_summaries_for_repo(
    store: &SharedStore,
    executor: &WaveExecutor,
    repo: &str,
    last_refresh: &mut HashMap<String, Instant>,
) {
    let waves = match store.list_waves(Some(repo)) {
        Ok(waves) => waves,
        Err(err) => {
            tracing::error!(error = %err, "failed to list waves for summary refresh");
            return;
        }
    };

    for wave in waves {
        if wave.area.is_empty() {
            continue;
        }

        // Skip waves that are actively running — the execute loop handles those.
        if wave.status == WaveStatus::Running {
            continue;
        }

        // Debounce per wave.
        let wave_key = wave.id.to_string();
        if let Some(last) = last_refresh.get(&wave_key) {
            if last.elapsed() < Duration::from_secs(DEBOUNCE_SECS) {
                continue;
            }
        }

        // Need an active run to get the worktree path.
        let run = match store.get_active_wave_run(&wave.id) {
            Ok(Some(run)) => run,
            _ => continue,
        };

        last_refresh.insert(wave_key, Instant::now());

        if let Err(err) = executor.ensure_summary_fresh(&wave, &run).await {
            tracing::warn!(
                wave = %wave.name,
                error = %err,
                "proactive summary refresh failed"
            );
        }
    }
}
