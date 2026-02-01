use crate::error::StoreError;
use crate::runtime::{Agent, ForkRun, RunId, WaveRun};

/// Persistence layer for wave runs.
///
/// Implement this trait to store run state. The engine calls these methods
/// during `tick_flow` to load, update, and create run records.
pub trait RunStore {
    /// Load a wave run by ID. Returns `StoreError::RunNotFound` if missing.
    fn get_run(&self, id: &RunId) -> Result<WaveRun, StoreError>;

    /// Persist changes to a wave run (status, step_index, error, etc.).
    fn update_run(&self, run: &WaveRun) -> Result<(), StoreError>;

    /// Create a new step run record.
    fn create_agent(&self, agent: &Agent) -> Result<(), StoreError>;

    /// List fork runs for a flow run and step index.
    fn list_fork_runs(&self, run_id: &RunId, step_index: usize)
        -> Result<Vec<ForkRun>, StoreError>;

    /// Upsert a fork run record.
    fn upsert_fork_run(&self, fork_run: &ForkRun) -> Result<(), StoreError>;

    /// Delete all fork runs for a flow run and step index.
    fn delete_fork_runs(&self, run_id: &RunId, step_index: usize) -> Result<(), StoreError>;
}
