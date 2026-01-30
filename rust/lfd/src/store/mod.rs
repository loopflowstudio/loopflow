use std::sync::Arc;

use crate::proto::control::PendingActivation;
use crate::proto::control::StepRun;
use crate::proto::control::Stimulus;
use crate::proto::control::Wave;

pub mod sqlite;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ForkRunStatus {
    Pending = 0,
    Running = 1,
    Completed = 2,
    Failed = 3,
}

impl ForkRunStatus {
    pub fn from_i64(value: i64) -> Option<Self> {
        match value {
            0 => Some(Self::Pending),
            1 => Some(Self::Running),
            2 => Some(Self::Completed),
            3 => Some(Self::Failed),
            _ => None,
        }
    }
}

#[derive(Debug, Clone)]
pub struct ForkRun {
    pub id: String,
    pub wave_id: String,
    pub step_index: u32,
    pub branch_index: u32,
    pub status: ForkRunStatus,
    pub worktree: String,
}

#[derive(Debug, thiserror::Error)]
pub enum StoreError {
    #[error("sqlite error: {0}")]
    Sqlite(#[from] rusqlite::Error),
    #[error("serialization error: {0}")]
    Serde(#[from] serde_json::Error),
    #[error("not found")]
    NotFound,
    #[error("invalid data: {0}")]
    InvalidData(String),
}

pub type StoreResult<T> = Result<T, StoreError>;

#[allow(dead_code)]
pub trait RunStore: Send + Sync {
    fn health_check(&self) -> StoreResult<()>;
    fn schema_version(&self) -> StoreResult<u32>;

    // Wave management
    fn list_waves(&self, repo: Option<&str>) -> StoreResult<Vec<Wave>>;
    fn get_wave(&self, wave_id: &str) -> StoreResult<Option<Wave>>;
    fn create_wave(&self, wave: &Wave) -> StoreResult<()>;
    fn update_wave(&self, wave: &Wave) -> StoreResult<()>;
    fn delete_wave(&self, wave_id: &str) -> StoreResult<()>;

    // Stimulus management (many:1 with waves)
    fn list_stimuli(&self, wave_id: Option<&str>) -> StoreResult<Vec<Stimulus>>;
    fn list_stimuli_by_kind(&self, kind: i32) -> StoreResult<Vec<Stimulus>>;
    fn get_stimulus(&self, stimulus_id: &str) -> StoreResult<Option<Stimulus>>;
    fn create_stimulus(&self, stimulus: &Stimulus) -> StoreResult<()>;
    fn update_stimulus(&self, stimulus: &Stimulus) -> StoreResult<()>;
    fn delete_stimulus(&self, stimulus_id: &str) -> StoreResult<()>;
    fn delete_stimuli_for_wave(&self, wave_id: &str) -> StoreResult<u32>;

    // Pending activations (for coalescing triggers)
    fn list_pending_activations(&self, wave_id: &str) -> StoreResult<Vec<PendingActivation>>;
    fn create_pending_activation(&self, activation: &PendingActivation) -> StoreResult<()>;
    fn update_pending_activation(&self, activation: &PendingActivation) -> StoreResult<()>;
    fn delete_pending_activations(&self, wave_id: &str) -> StoreResult<u32>;
    fn get_pending_for_stimulus(
        &self,
        wave_id: &str,
        stimulus_id: &str,
    ) -> StoreResult<Option<PendingActivation>>;

    // Fork runs
    fn list_fork_runs(&self, wave_id: &str, step_index: u32) -> StoreResult<Vec<ForkRun>>;
    fn upsert_fork_run(&self, fork_run: &ForkRun) -> StoreResult<()>;
    fn delete_fork_runs(&self, wave_id: &str, step_index: u32) -> StoreResult<u32>;

    // Step runs
    fn list_step_runs(&self) -> StoreResult<Vec<StepRun>>;
    fn list_step_run_history(
        &self,
        worktree: Option<&str>,
        repo: Option<&str>,
        limit: Option<u32>,
    ) -> StoreResult<Vec<StepRun>>;
    fn get_step_run(&self, step_run_id: &str) -> StoreResult<Option<StepRun>>;
    fn get_waiting_step_run(&self, wave_id: &str) -> StoreResult<Option<StepRun>>;
    fn start_step_run(&self, step_run: &StepRun) -> StoreResult<()>;
    fn update_step_run_status(
        &self,
        step_run_id: &str,
        status: i32,
        pid: Option<u32>,
    ) -> StoreResult<()>;
    fn end_step_run(&self, step_run_id: &str, status: i32, ended_at: i64) -> StoreResult<()>;
    fn get_stuck_step_runs(&self, older_than_secs: u64) -> StoreResult<Vec<StepRun>>;
}

pub type SharedStore = Arc<dyn RunStore>;
