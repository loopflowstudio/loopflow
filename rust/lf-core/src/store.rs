use crate::error::StoreError;
use crate::runtime::{FlowRun, StepRun};

pub trait RunStore {
    fn get_run(&self, id: &str) -> Result<FlowRun, StoreError>;
    fn update_run(&self, run: &FlowRun) -> Result<(), StoreError>;
    fn create_step_run(&self, step_run: &StepRun) -> Result<(), StoreError>;
}
