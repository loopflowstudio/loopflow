use crate::error::StoreError;
use crate::runtime::{FlowRun, RunId, StepRun};

/// Persistence layer for flow and step runs.
///
/// Implement this trait to store run state. The engine calls these methods
/// during `tick_flow` to load, update, and create run records.
pub trait RunStore {
    /// Load a flow run by ID. Returns `StoreError::RunNotFound` if missing.
    fn get_run(&self, id: &RunId) -> Result<FlowRun, StoreError>;

    /// Persist changes to a flow run (status, step_index, error, etc.).
    fn update_run(&self, run: &FlowRun) -> Result<(), StoreError>;

    /// Create a new step run record.
    fn create_step_run(&self, step_run: &StepRun) -> Result<(), StoreError>;
}
