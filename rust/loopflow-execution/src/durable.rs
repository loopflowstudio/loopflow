pub use loopflow_storage::execution::*;
use serde::{Deserialize, Serialize};

/// Where a Task's commands execute: its owner's Machine, or the Machine that
/// may claim it when it is unowned or its owner's Machine is unknown.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TaskExecutionRoute {
    pub task_id: TaskId,
    pub checkout: Option<String>,
    pub repository_id: RepositoryId,
    pub machine_id: MachineId,
    pub source: TaskExecutionSource,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TaskExecutionSource {
    Owner,
    OwningMachine(OwningMachineSource),
}

/// Where your Machine for an unowned Task or a scope's Session came from.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", content = "declaration", rename_all = "snake_case")]
pub enum OwningMachineSource {
    Preference(Box<crate::config::settings::SettingCandidate>),
    /// No default is set: the Machine running the command.
    ThisMachine,
}
