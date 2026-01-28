use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

use lf_core::error::StoreError as CoreStoreError;
use lf_core::runtime::{FlowRun, FlowRunStatus, RunId, TickResult};
use lf_core::store as core_store;
use tokio::task::JoinHandle;
use tokio_util::sync::CancellationToken;

use crate::proto::control::{StepRun, StepRunStatus, StimulusKind, Wave, WaveStatus};
use crate::scheduler::Scheduler;
use crate::store::SharedStore;

pub fn spawn_loop_ticker(
    scheduler: Arc<Scheduler>,
    store: SharedStore,
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
                    tick_loop_waves(&scheduler, &store).await;
                }
            }
        }
    })
}

async fn tick_loop_waves(scheduler: &Scheduler, store: &SharedStore) {
    let waves = match store.list_waves_by_stimulus(StimulusKind::StimulusLoop as i32) {
        Ok(waves) => waves,
        Err(err) => {
            tracing::error!(error = %err, "failed to list loop waves");
            return;
        }
    };

    for wave in waves {
        if wave.paused || wave.status != WaveStatus::WaveRunning as i32 {
            continue;
        }

        let (acquired, _) = scheduler.acquire(&wave.id).await;
        if !acquired {
            tracing::debug!(wave_id = %wave.id, "waiting for slot");
            continue;
        }

        let result = tick_wave(&wave, store).await;
        handle_tick_result(&wave, result, store);
        scheduler.release(&wave.id);
    }
}

async fn tick_wave(
    wave: &Wave,
    store: &SharedStore,
) -> Result<TickResult, lf_core::error::CoreError> {
    let adapter = LfCoreStoreAdapter::new(store.clone());
    let run_id = RunId::new(wave.id.clone());
    lf_core::runtime::tick_flow(&run_id, &adapter)
}

fn handle_tick_result(
    wave: &Wave,
    result: Result<TickResult, lf_core::error::CoreError>,
    store: &SharedStore,
) {
    let mut wave = match store.get_wave(&wave.id) {
        Ok(Some(wave)) => wave,
        Ok(None) => return,
        Err(err) => {
            tracing::error!(wave_id = %wave.id, error = %err, "failed to reload wave");
            return;
        }
    };

    match result {
        Ok(TickResult::StepComplete) => {
            wave.consecutive_failures = 0;
            let _ = store.update_wave(&wave);
        }
        Ok(TickResult::FlowComplete) => {
            wave.status = WaveStatus::WaveIdle as i32;
            wave.iteration += 1;
            wave.step_index = 0;
            wave.consecutive_failures = 0;
            if wave.pending_activations > 0 {
                wave.pending_activations -= 1;
                wave.status = WaveStatus::WaveRunning as i32;
            }
            let _ = store.update_wave(&wave);
            tracing::info!(wave_id = %wave.id, iteration = wave.iteration, "flow complete");
        }
        Ok(TickResult::WaitingInteractive) => {
            wave.status = WaveStatus::WaveWaiting as i32;
            let _ = store.update_wave(&wave);
            tracing::info!(wave_id = %wave.id, "waiting for interactive step");
        }
        Ok(TickResult::StepFailed) | Err(_) => {
            wave.consecutive_failures += 1;
            if wave.consecutive_failures >= 3 {
                wave.status = WaveStatus::WaveError as i32;
                tracing::error!(wave_id = %wave.id, "entered error state after 3 failures");
            }
            let _ = store.update_wave(&wave);
            if let Err(err) = result {
                tracing::warn!(wave_id = %wave.id, error = %err, "tick failed");
            }
        }
    }
}

#[derive(Clone)]
struct LfCoreStoreAdapter {
    store: SharedStore,
}

impl LfCoreStoreAdapter {
    fn new(store: SharedStore) -> Self {
        Self { store }
    }
}

impl core_store::RunStore for LfCoreStoreAdapter {
    fn get_run(&self, id: &RunId) -> Result<FlowRun, CoreStoreError> {
        let wave = self
            .store
            .get_wave(id.as_str())
            .map_err(|err| CoreStoreError::Other(err.to_string()))?
            .ok_or_else(|| CoreStoreError::RunNotFound(id.as_str().to_string()))?;

        Ok(FlowRun {
            id: RunId::new(wave.id.clone()),
            flow: wave.flow,
            direction: wave.direction,
            area: wave.area,
            repo: PathBuf::from(wave.repo),
            status: flow_status_from_wave(wave.status),
            step_index: wave.step_index as usize,
            worktree: if wave.worktree.is_empty() {
                None
            } else {
                Some(wave.worktree)
            },
            current_step: None,
            error: None,
        })
    }

    fn update_run(&self, run: &FlowRun) -> Result<(), CoreStoreError> {
        let mut wave = self
            .store
            .get_wave(run.id.as_str())
            .map_err(|err| CoreStoreError::Other(err.to_string()))?
            .ok_or_else(|| CoreStoreError::RunNotFound(run.id.as_str().to_string()))?;

        wave.status = wave_status_from_flow(run.status) as i32;
        wave.step_index = run.step_index as u32;
        if let Some(worktree) = &run.worktree {
            wave.worktree = worktree.clone();
        }
        self.store
            .update_wave(&wave)
            .map_err(|err| CoreStoreError::Other(err.to_string()))?;
        Ok(())
    }

    fn create_step_run(&self, step_run: &lf_core::runtime::StepRun) -> Result<(), CoreStoreError> {
        let status = match step_run.status {
            lf_core::runtime::StepRunStatus::Running => StepRunStatus::StepRunning as i32,
            lf_core::runtime::StepRunStatus::Waiting => StepRunStatus::StepWaiting as i32,
            lf_core::runtime::StepRunStatus::Completed => StepRunStatus::StepCompleted as i32,
            lf_core::runtime::StepRunStatus::Failed => StepRunStatus::StepFailed as i32,
        };

        let step_run = StepRun {
            id: step_run.id.clone(),
            step: step_run.step.clone(),
            repo: step_run.repo.clone(),
            worktree: step_run.worktree.clone(),
            flow_run_id: step_run
                .flow_run_id
                .as_ref()
                .map(|run_id| run_id.as_str().to_string()),
            wave_id: step_run
                .flow_run_id
                .as_ref()
                .map(|run_id| run_id.as_str().to_string()),
            status,
            started_at: Some(now_timestamp()),
            ended_at: None,
            pid: None,
            model: "unknown".to_string(),
            run_mode: "auto".to_string(),
        };

        self.store
            .start_step_run(&step_run)
            .map_err(|err| CoreStoreError::Other(err.to_string()))?;
        Ok(())
    }
}

fn flow_status_from_wave(status: i32) -> FlowRunStatus {
    match WaveStatus::try_from(status) {
        Ok(WaveStatus::WaveWaiting) => FlowRunStatus::Waiting,
        Ok(WaveStatus::WaveError) => FlowRunStatus::Failed,
        Ok(WaveStatus::WaveIdle) => FlowRunStatus::Completed,
        _ => FlowRunStatus::Running,
    }
}

fn wave_status_from_flow(status: FlowRunStatus) -> WaveStatus {
    match status {
        FlowRunStatus::Running => WaveStatus::WaveRunning,
        FlowRunStatus::Waiting => WaveStatus::WaveWaiting,
        FlowRunStatus::Completed => WaveStatus::WaveIdle,
        FlowRunStatus::Failed => WaveStatus::WaveError,
    }
}

fn now_timestamp() -> prost_types::Timestamp {
    let now = time::OffsetDateTime::now_utc().unix_timestamp();
    prost_types::Timestamp {
        seconds: now,
        nanos: 0,
    }
}
