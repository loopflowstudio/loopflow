use std::sync::Arc;

use crate::proto::control::StepRun;
use crate::proto::control::Wave;

pub mod sqlite;

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

pub trait RunStore: Send + Sync {
    fn health_check(&self) -> StoreResult<()>;
    fn schema_version(&self) -> StoreResult<u32>;

    fn list_waves(&self, repo: Option<&str>) -> StoreResult<Vec<Wave>>;
    fn list_waves_by_stimulus(&self, kind: i32) -> StoreResult<Vec<Wave>>;
    fn get_wave(&self, wave_id: &str) -> StoreResult<Option<Wave>>;
    fn create_wave(&self, wave: &Wave) -> StoreResult<()>;
    fn update_wave(&self, wave: &Wave) -> StoreResult<()>;
    fn delete_wave(&self, wave_id: &str) -> StoreResult<()>;
    fn increment_pending_activations(&self, wave_id: &str) -> StoreResult<u32>;

    fn list_step_runs(&self) -> StoreResult<Vec<StepRun>>;
    fn list_step_run_history(
        &self,
        worktree: Option<&str>,
        repo: Option<&str>,
        limit: Option<u32>,
    ) -> StoreResult<Vec<StepRun>>;
    fn start_step_run(&self, step_run: &StepRun) -> StoreResult<()>;
    fn end_step_run(&self, step_run_id: &str, status: i32, ended_at: i64) -> StoreResult<()>;
    fn get_stuck_step_runs(&self, older_than_secs: u64) -> StoreResult<Vec<StepRun>>;
}

pub type SharedStore = Arc<dyn RunStore>;
