//! Committed inspection facts. FlowPosition remains the execution authority.

use serde::{Deserialize, Serialize};

use crate::controller::wave::playhead::QueuedInvocation;
use crate::durable::{FlowPosition, RunId, TaskFlowBlocker};
use crate::work::task::TaskEvent;

#[derive(Debug)]
pub(crate) struct TaskFlowHistory {
    pub position: Option<FlowPosition>,
    pub position_available: bool,
    pub events: Vec<TaskEvent>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TaskFlowStage {
    pub invocation_id: String,
    pub step_index: u32,
    pub iteration: u32,
}

impl From<&FlowPosition> for TaskFlowStage {
    fn from(position: &FlowPosition) -> Self {
        Self {
            invocation_id: position.invocation.id.clone(),
            step_index: position.step_index,
            iteration: position.iteration,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
#[non_exhaustive]
pub enum TaskFlowTransition {
    Advanced,
    Approved,
    Iterated,
    Retried,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
#[non_exhaustive]
pub enum TaskFlowSettlement {
    Completed,
    Approved,
    Replaced,
    Reopened,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
#[non_exhaustive]
pub enum TaskFlowEvent {
    InvocationSelected {
        invocation: QueuedInvocation,
    },
    StageEntered {
        stage: TaskFlowStage,
    },
    RunBound {
        stage: TaskFlowStage,
        run_id: RunId,
    },
    StageReady {
        stage: TaskFlowStage,
        summary: String,
    },
    StageBlocked {
        stage: TaskFlowStage,
        failure: TaskFlowBlocker,
    },
    Transition {
        from: TaskFlowStage,
        to: TaskFlowStage,
        reason: TaskFlowTransition,
    },
    InvocationSettled {
        stage: TaskFlowStage,
        outcome: TaskFlowSettlement,
    },
}
