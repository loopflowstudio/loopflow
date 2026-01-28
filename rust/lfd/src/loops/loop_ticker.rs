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
use crate::store::{SharedStore, StoreError};

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
    let step_run_index = wave.step_index;
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
            finish_step_run(
                store,
                &wave.id,
                step_run_index,
                StepRunStatus::StepCompleted,
            );
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
            finish_step_run(store, &wave.id, step_run_index, StepRunStatus::StepFailed);
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

fn finish_step_run(store: &SharedStore, wave_id: &str, step_index: u32, status: StepRunStatus) {
    let step_run_id = format!("{wave_id}:{step_index}");
    let ended_at = time::OffsetDateTime::now_utc().unix_timestamp();
    match store.end_step_run(&step_run_id, status as i32, ended_at) {
        Ok(()) => {}
        Err(StoreError::NotFound) => {
            tracing::debug!(step_run_id = %step_run_id, "step run not found when finishing");
        }
        Err(err) => {
            tracing::warn!(step_run_id = %step_run_id, error = %err, "failed to finish step run");
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

#[cfg(test)]
mod tests {
    use std::sync::Mutex;

    use super::handle_tick_result;
    use crate::proto::control::{StepRunStatus, Wave, WaveStatus};
    use crate::store::{RunStore, SharedStore, StoreError, StoreResult};

    #[derive(Debug, Default)]
    struct TestStore {
        wave: Mutex<Wave>,
        ended: Mutex<Vec<(String, i32)>>,
    }

    impl TestStore {
        fn new(wave: Wave) -> Self {
            Self {
                wave: Mutex::new(wave),
                ended: Mutex::new(Vec::new()),
            }
        }
    }

    impl RunStore for TestStore {
        fn health_check(&self) -> StoreResult<()> {
            Ok(())
        }

        fn schema_version(&self) -> StoreResult<u32> {
            Ok(1)
        }

        fn list_waves(&self, _repo: Option<&str>) -> StoreResult<Vec<Wave>> {
            Err(StoreError::InvalidData("unused".to_string()))
        }

        fn list_waves_by_stimulus(&self, _kind: i32) -> StoreResult<Vec<Wave>> {
            Err(StoreError::InvalidData("unused".to_string()))
        }

        fn get_wave(&self, wave_id: &str) -> StoreResult<Option<Wave>> {
            let wave = self.wave.lock().expect("wave mutex poisoned");
            if wave.id == wave_id {
                Ok(Some(wave.clone()))
            } else {
                Ok(None)
            }
        }

        fn create_wave(&self, _wave: &Wave) -> StoreResult<()> {
            Err(StoreError::InvalidData("unused".to_string()))
        }

        fn update_wave(&self, wave: &Wave) -> StoreResult<()> {
            let mut stored = self.wave.lock().expect("wave mutex poisoned");
            *stored = wave.clone();
            Ok(())
        }

        fn delete_wave(&self, _wave_id: &str) -> StoreResult<()> {
            Err(StoreError::InvalidData("unused".to_string()))
        }

        fn increment_pending_activations(&self, _wave_id: &str) -> StoreResult<u32> {
            Err(StoreError::InvalidData("unused".to_string()))
        }

        fn list_step_runs(&self) -> StoreResult<Vec<crate::proto::control::StepRun>> {
            Err(StoreError::InvalidData("unused".to_string()))
        }

        fn list_step_run_history(
            &self,
            _worktree: Option<&str>,
            _repo: Option<&str>,
            _limit: Option<u32>,
        ) -> StoreResult<Vec<crate::proto::control::StepRun>> {
            Err(StoreError::InvalidData("unused".to_string()))
        }

        fn start_step_run(&self, _step_run: &crate::proto::control::StepRun) -> StoreResult<()> {
            Err(StoreError::InvalidData("unused".to_string()))
        }

        fn end_step_run(&self, step_run_id: &str, status: i32, _ended_at: i64) -> StoreResult<()> {
            let mut ended = self.ended.lock().expect("ended mutex poisoned");
            ended.push((step_run_id.to_string(), status));
            Ok(())
        }

        fn get_stuck_step_runs(
            &self,
            _older_than_secs: u64,
        ) -> StoreResult<Vec<crate::proto::control::StepRun>> {
            Err(StoreError::InvalidData("unused".to_string()))
        }
    }

    fn base_wave() -> Wave {
        Wave {
            id: "wave-1".to_string(),
            name: "wave-1".to_string(),
            repo: "/tmp".to_string(),
            flow: "ship".to_string(),
            direction: Vec::new(),
            area: Vec::new(),
            stimulus: None,
            paused: false,
            status: WaveStatus::WaveRunning as i32,
            iteration: 0,
            step_index: 3,
            worktree: String::new(),
            branch: String::new(),
            pr_limit: 0,
            merge_mode: 0,
            pid: None,
            created_at: None,
            last_main_sha: None,
            consecutive_failures: 0,
            pending_activations: 0,
        }
    }

    #[test]
    fn handle_tick_result_ends_step_run_on_success() {
        let wave = base_wave();
        let store = std::sync::Arc::new(TestStore::new(wave.clone()));
        let shared: SharedStore = store.clone();

        handle_tick_result(
            &wave,
            Ok(lf_core::runtime::TickResult::StepComplete),
            &shared,
        );

        let ended = store.ended.lock().expect("ended mutex poisoned").clone();

        assert_eq!(
            ended,
            vec![("wave-1:3".to_string(), StepRunStatus::StepCompleted as i32)]
        );
    }

    #[test]
    fn handle_tick_result_ends_step_run_on_failure() {
        let wave = base_wave();
        let store = std::sync::Arc::new(TestStore::new(wave.clone()));
        let shared: SharedStore = store.clone();

        handle_tick_result(&wave, Ok(lf_core::runtime::TickResult::StepFailed), &shared);

        let ended = store.ended.lock().expect("ended mutex poisoned").clone();

        assert_eq!(
            ended,
            vec![("wave-1:3".to_string(), StepRunStatus::StepFailed as i32)]
        );
    }
}
