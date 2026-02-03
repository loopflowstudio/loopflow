use std::time::Duration;

use tokio::task::JoinHandle;
use tokio_util::sync::CancellationToken;

use crate::executor::WaveExecutor;
use crate::id::LfdId;
use crate::proto::control::{StimulusKind, WaveRun, WaveRunStatus};
use crate::scheduler::Scheduler;
use crate::store::SharedStore;

pub fn spawn_loop_ticker(
    scheduler: std::sync::Arc<Scheduler>,
    store: SharedStore,
    executor: WaveExecutor,
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
                    tick_loop_waves(&scheduler, &store, &executor).await;
                }
            }
        }
    })
}

async fn tick_loop_waves(
    scheduler: &std::sync::Arc<Scheduler>,
    store: &SharedStore,
    executor: &WaveExecutor,
) {
    let stimuli = match store.list_stimuli_by_kind(StimulusKind::StimulusLoop as i32) {
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

        let wave_id = match LfdId::parse(&stimulus.wave_id) {
            Ok(id) => id,
            Err(err) => {
                tracing::warn!(stimulus_id = %stimulus.id, error = %err, "invalid wave id");
                continue;
            }
        };
        let wave = match store.get_wave(&wave_id) {
            Ok(Some(wave)) => wave,
            Ok(None) => continue,
            Err(err) => {
                tracing::error!(wave_id = %wave_id, error = %err, "failed to load wave");
                continue;
            }
        };

        if wave.paused {
            continue;
        }

        if scheduler.has_active_session(&wave.id) {
            continue;
        }

        if let Ok(Some(_)) = store.get_active_wave_run(&wave_id) {
            continue;
        }

        let run_id = LfdId::new();
        let (acquired, _) = scheduler.acquire(run_id.as_str()).await;
        if !acquired {
            continue;
        }

        let run = match create_wave_run(store, &wave, &run_id) {
            Ok(run) => run,
            Err(err) => {
                scheduler.release(run_id.as_str());
                tracing::error!(wave_id = %wave.id, error = %err, "failed to create wave run");
                continue;
            }
        };

        let exec = executor.clone();
        let store = store.clone();
        let scheduler = scheduler.clone();
        tokio::spawn(async move {
            let run_id = LfdId::parse(&run.id).expect("run id should be valid");
            if let Err(err) = exec.execute(&run_id).await {
                tracing::error!(run_id = %run.id, error = %err, "run execution failed");
                if let Ok(Some(mut run)) = store.get_wave_run(&run_id) {
                    run.status = WaveRunStatus::WaveRunFailed as i32;
                    run.error = Some(err.to_string());
                    run.ended_at = Some(now_timestamp());
                    let _ = store.update_wave_run(&run);
                }
            }
            scheduler.release(&run.id);
        });
    }
}

fn create_wave_run(
    store: &SharedStore,
    wave: &crate::proto::control::Wave,
    run_id: &LfdId,
) -> anyhow::Result<WaveRun> {
    let wave_id = LfdId::parse(&wave.id)?;
    let last_run = store
        .list_wave_runs(Some(&wave_id), Some(1))?
        .into_iter()
        .next();
    let iteration = last_run.map(|run| run.iteration + 1).unwrap_or(0);

    let run = WaveRun {
        id: run_id.to_string(),
        wave_id: wave.id.clone(),
        iteration,
        step_index: 0,
        status: WaveRunStatus::WaveRunRunning as i32,
        worktree: wave.repo.clone(),
        branch: String::new(),
        started_at: Some(now_timestamp()),
        ended_at: None,
        error: None,
    };
    store.create_wave_run(&run)?;
    Ok(run)
}

fn now_timestamp() -> prost_types::Timestamp {
    let now = time::OffsetDateTime::now_utc();
    prost_types::Timestamp {
        seconds: now.unix_timestamp(),
        nanos: 0,
    }
}
