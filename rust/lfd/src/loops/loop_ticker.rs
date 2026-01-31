use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

use loopflow_engine::error::StoreError as CoreStoreError;
use loopflow_engine::runtime::{FlowRun, FlowRunStatus, RunId, TickResult};
use loopflow_engine::store as core_store;
use tokio::task::JoinHandle;
use tokio_util::sync::CancellationToken;

use crate::id::LfdId;
use crate::proto::control::{Agent, AgentStatus, StimulusKind, Wave, WaveStatus};
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
    // Query stimuli with kind=LOOP
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

        // Get the wave for this stimulus
        let wave_id = match LfdId::parse(&stimulus.wave_id) {
            Ok(id) => id,
            Err(err) => {
                tracing::warn!(stimulus_id = %stimulus.id, error = %err, "invalid wave id");
                continue;
            }
        };
        let wave = match store.get_wave(&wave_id) {
            Ok(Some(wave)) => wave,
            Ok(None) => {
                tracing::warn!(stimulus_id = %stimulus.id, "stimulus references missing wave");
                continue;
            }
            Err(err) => {
                tracing::error!(stimulus_id = %stimulus.id, error = %err, "failed to get wave");
                continue;
            }
        };

        if wave.paused || wave.status != WaveStatus::WaveRunning as i32 {
            continue;
        }

        if scheduler.has_active_session(&wave.id) {
            tracing::debug!(wave_id = %wave.id, "skipping tick while session active");
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
) -> Result<TickResult, loopflow_engine::error::CoreError> {
    let adapter = LfCoreStoreAdapter::new(store.clone());
    let run_id = RunId::new(wave.id.clone());
    loopflow_engine::runtime::tick_flow(&run_id, &adapter)
}

fn handle_tick_result(
    wave: &Wave,
    result: Result<TickResult, loopflow_engine::error::CoreError>,
    store: &SharedStore,
) {
    let wave_id = match LfdId::parse(&wave.id) {
        Ok(id) => id,
        Err(err) => {
            tracing::warn!(wave_id = %wave.id, error = %err, "invalid wave id");
            return;
        }
    };
    let mut wave = match store.get_wave(&wave_id) {
        Ok(Some(wave)) => wave,
        Ok(None) => return,
        Err(err) => {
            tracing::error!(wave_id = %wave.id, error = %err, "failed to reload wave");
            return;
        }
    };

    match result {
        Ok(TickResult::StepComplete) => {
            finish_agent(store, &wave.id, AgentStatus::AgentCompleted);
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
            finish_agent(store, &wave.id, AgentStatus::AgentFailed);
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

fn finish_agent(store: &SharedStore, wave_id: &str, status: AgentStatus) {
    let wave_id = match LfdId::parse(wave_id) {
        Ok(id) => id,
        Err(_) => {
            tracing::warn!(wave_id = %wave_id, "invalid wave_id when finishing step run");
            return;
        }
    };
    let ended_at = time::OffsetDateTime::now_utc().unix_timestamp();
    match store.end_active_agent_for_wave(&wave_id, status as i32, ended_at) {
        Ok(()) => {}
        Err(StoreError::NotFound) => {
            tracing::debug!(wave_id = %wave_id, "no active step run found when finishing");
        }
        Err(err) => {
            tracing::warn!(wave_id = %wave_id, error = %err, "failed to finish step run");
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
        let wave_id =
            LfdId::parse(id.as_str()).map_err(|err| CoreStoreError::Other(err.to_string()))?;
        let wave = self
            .store
            .get_wave(&wave_id)
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
        let wave_id =
            LfdId::parse(run.id.as_str()).map_err(|err| CoreStoreError::Other(err.to_string()))?;
        let mut wave = self
            .store
            .get_wave(&wave_id)
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

    fn create_agent(&self, agent: &loopflow_engine::runtime::Agent) -> Result<(), CoreStoreError> {
        let status = match agent.status {
            loopflow_engine::runtime::AgentStatus::Running => AgentStatus::AgentRunning as i32,
            loopflow_engine::runtime::AgentStatus::Waiting => AgentStatus::AgentWaiting as i32,
            loopflow_engine::runtime::AgentStatus::Completed => AgentStatus::AgentCompleted as i32,
            loopflow_engine::runtime::AgentStatus::Failed => AgentStatus::AgentFailed as i32,
        };

        let agent = Agent {
            id: agent.id.clone(),
            step: agent.step.clone(),
            repo: agent.repo.clone(),
            worktree: agent.worktree.clone(),
            flow_run_id: agent
                .flow_run_id
                .as_ref()
                .map(|run_id| run_id.as_str().to_string()),
            wave_id: agent
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
            .start_agent(&agent)
            .map_err(|err| CoreStoreError::Other(err.to_string()))?;
        Ok(())
    }

    fn list_fork_runs(
        &self,
        run_id: &RunId,
        step_index: usize,
    ) -> Result<Vec<loopflow_engine::runtime::ForkRun>, CoreStoreError> {
        let run_id =
            LfdId::parse(run_id.as_str()).map_err(|err| CoreStoreError::Other(err.to_string()))?;
        let fork_runs = self
            .store
            .list_fork_runs(&run_id, step_index as u32)
            .map_err(|err| CoreStoreError::Other(err.to_string()))?;
        Ok(fork_runs
            .into_iter()
            .map(|fork_run| loopflow_engine::runtime::ForkRun {
                id: fork_run.id.to_string(),
                run_id: RunId::new(fork_run.wave_id.to_string()),
                step_index: fork_run.step_index as usize,
                branch_index: fork_run.branch_index as usize,
                status: map_fork_status(fork_run.status),
                worktree: fork_run.worktree,
            })
            .collect())
    }

    fn upsert_fork_run(
        &self,
        fork_run: &loopflow_engine::runtime::ForkRun,
    ) -> Result<(), CoreStoreError> {
        let status = map_fork_status_back(fork_run.status);
        let record = crate::store::ForkRun {
            id: LfdId::parse(&fork_run.id).expect("fork_run.id should be valid UUID"),
            wave_id: LfdId::parse(fork_run.run_id.as_str()).expect("run_id should be valid UUID"),
            step_index: fork_run.step_index as u32,
            branch_index: fork_run.branch_index as u32,
            status,
            worktree: fork_run.worktree.clone(),
        };
        self.store
            .upsert_fork_run(&record)
            .map_err(|err| CoreStoreError::Other(err.to_string()))?;
        Ok(())
    }

    fn delete_fork_runs(&self, run_id: &RunId, step_index: usize) -> Result<(), CoreStoreError> {
        let run_id =
            LfdId::parse(run_id.as_str()).map_err(|err| CoreStoreError::Other(err.to_string()))?;
        self.store
            .delete_fork_runs(&run_id, step_index as u32)
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

fn map_fork_status(status: crate::store::ForkRunStatus) -> loopflow_engine::runtime::ForkRunStatus {
    match status {
        crate::store::ForkRunStatus::Pending => loopflow_engine::runtime::ForkRunStatus::Pending,
        crate::store::ForkRunStatus::Running => loopflow_engine::runtime::ForkRunStatus::Running,
        crate::store::ForkRunStatus::Completed => {
            loopflow_engine::runtime::ForkRunStatus::Completed
        }
        crate::store::ForkRunStatus::Failed => loopflow_engine::runtime::ForkRunStatus::Failed,
    }
}

fn map_fork_status_back(
    status: loopflow_engine::runtime::ForkRunStatus,
) -> crate::store::ForkRunStatus {
    match status {
        loopflow_engine::runtime::ForkRunStatus::Pending => crate::store::ForkRunStatus::Pending,
        loopflow_engine::runtime::ForkRunStatus::Running => crate::store::ForkRunStatus::Running,
        loopflow_engine::runtime::ForkRunStatus::Completed => {
            crate::store::ForkRunStatus::Completed
        }
        loopflow_engine::runtime::ForkRunStatus::Failed => crate::store::ForkRunStatus::Failed,
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
    use crate::id::LfdId;
    use crate::proto::control::{AgentStatus, Wave, WaveStatus};
    use crate::store::{ForkRun, RunStore, SharedStore, StoreError, StoreResult};

    #[derive(Debug, Default)]
    struct TestStore {
        wave: Mutex<Wave>,
        ended: Mutex<Vec<(String, i32)>>,
    }

    fn unused<T>() -> StoreResult<T> {
        Err(StoreError::InvalidData("unused".to_string()))
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
            unused()
        }

        fn get_wave(&self, wave_id: &LfdId) -> StoreResult<Option<Wave>> {
            let wave = self.wave.lock().expect("wave mutex poisoned");
            if wave.id == wave_id.as_str() {
                Ok(Some(wave.clone()))
            } else {
                Ok(None)
            }
        }

        fn create_wave(&self, _wave: &Wave) -> StoreResult<()> {
            unused()
        }

        fn update_wave(&self, wave: &Wave) -> StoreResult<()> {
            let mut stored = self.wave.lock().expect("wave mutex poisoned");
            *stored = wave.clone();
            Ok(())
        }

        fn delete_wave(&self, _wave_id: &LfdId) -> StoreResult<()> {
            unused()
        }

        // Stimulus methods
        fn list_stimuli(
            &self,
            _wave_id: Option<&LfdId>,
        ) -> StoreResult<Vec<crate::proto::control::Stimulus>> {
            unused()
        }

        fn list_stimuli_by_kind(
            &self,
            _kind: i32,
        ) -> StoreResult<Vec<crate::proto::control::Stimulus>> {
            unused()
        }

        fn get_stimulus(
            &self,
            _stimulus_id: &LfdId,
        ) -> StoreResult<Option<crate::proto::control::Stimulus>> {
            unused()
        }

        fn create_stimulus(&self, _stimulus: &crate::proto::control::Stimulus) -> StoreResult<()> {
            unused()
        }

        fn update_stimulus(&self, _stimulus: &crate::proto::control::Stimulus) -> StoreResult<()> {
            unused()
        }

        fn delete_stimulus(&self, _stimulus_id: &LfdId) -> StoreResult<()> {
            unused()
        }

        fn delete_stimuli_for_wave(&self, _wave_id: &LfdId) -> StoreResult<u32> {
            unused()
        }

        // Pending activation methods
        fn list_pending_activations(
            &self,
            _wave_id: &LfdId,
        ) -> StoreResult<Vec<crate::proto::control::PendingActivation>> {
            unused()
        }

        fn create_pending_activation(
            &self,
            _activation: &crate::proto::control::PendingActivation,
        ) -> StoreResult<()> {
            unused()
        }

        fn update_pending_activation(
            &self,
            _activation: &crate::proto::control::PendingActivation,
        ) -> StoreResult<()> {
            unused()
        }

        fn delete_pending_activations(&self, _wave_id: &LfdId) -> StoreResult<u32> {
            unused()
        }

        fn get_pending_for_stimulus(
            &self,
            _wave_id: &LfdId,
            _stimulus_id: &LfdId,
        ) -> StoreResult<Option<crate::proto::control::PendingActivation>> {
            unused()
        }

        // Step run methods
        fn list_agents(&self) -> StoreResult<Vec<crate::proto::control::Agent>> {
            unused()
        }

        fn list_agent_history(
            &self,
            _worktree: Option<&str>,
            _repo: Option<&str>,
            _limit: Option<u32>,
        ) -> StoreResult<Vec<crate::proto::control::Agent>> {
            unused()
        }

        fn list_fork_runs(&self, _wave_id: &LfdId, _step_index: u32) -> StoreResult<Vec<ForkRun>> {
            unused()
        }

        fn upsert_fork_run(&self, _fork_run: &ForkRun) -> StoreResult<()> {
            unused()
        }

        fn delete_fork_runs(&self, _wave_id: &LfdId, _step_index: u32) -> StoreResult<u32> {
            unused()
        }

        fn get_agent(
            &self,
            _agent_id: &LfdId,
        ) -> StoreResult<Option<crate::proto::control::Agent>> {
            unused()
        }

        fn get_waiting_agent(
            &self,
            _wave_id: &LfdId,
        ) -> StoreResult<Option<crate::proto::control::Agent>> {
            unused()
        }

        fn start_agent(&self, _agent: &crate::proto::control::Agent) -> StoreResult<()> {
            unused()
        }

        fn update_agent_status(
            &self,
            _agent_id: &LfdId,
            _status: i32,
            _pid: Option<u32>,
        ) -> StoreResult<()> {
            unused()
        }

        fn end_agent(&self, agent_id: &LfdId, status: i32, _ended_at: i64) -> StoreResult<()> {
            let mut ended = self.ended.lock().expect("ended mutex poisoned");
            ended.push((agent_id.to_string(), status));
            Ok(())
        }

        fn end_active_agent_for_wave(
            &self,
            wave_id: &LfdId,
            status: i32,
            _ended_at: i64,
        ) -> StoreResult<()> {
            let mut ended = self.ended.lock().expect("ended mutex poisoned");
            ended.push((wave_id.to_string(), status));
            Ok(())
        }

        fn get_stuck_agents(
            &self,
            _older_than_secs: u64,
        ) -> StoreResult<Vec<crate::proto::control::Agent>> {
            unused()
        }
    }

    fn base_wave() -> Wave {
        let id = LfdId::new().to_string();
        Wave {
            id: id.clone(),
            name: id,
            repo: "/tmp".to_string(),
            flow: "ship".to_string(),
            direction: Vec::new(),
            area: Vec::new(),
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
            consecutive_failures: 0,
            pending_activations: 0,
        }
    }

    #[test]
    fn handle_tick_result_ends_agent_on_success() {
        let wave = base_wave();
        let store = std::sync::Arc::new(TestStore::new(wave.clone()));
        let shared: SharedStore = store.clone();

        handle_tick_result(
            &wave,
            Ok(loopflow_engine::runtime::TickResult::StepComplete),
            &shared,
        );

        let ended = store.ended.lock().expect("ended mutex poisoned").clone();

        assert_eq!(ended, vec![(wave.id, AgentStatus::AgentCompleted as i32)]);
    }

    #[test]
    fn handle_tick_result_ends_agent_on_failure() {
        let wave = base_wave();
        let store = std::sync::Arc::new(TestStore::new(wave.clone()));
        let shared: SharedStore = store.clone();

        handle_tick_result(
            &wave,
            Ok(loopflow_engine::runtime::TickResult::StepFailed),
            &shared,
        );

        let ended = store.ended.lock().expect("ended mutex poisoned").clone();

        assert_eq!(ended, vec![(wave.id, AgentStatus::AgentFailed as i32)]);
    }
}
