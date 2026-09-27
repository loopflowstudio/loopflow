//! Read-only evidence from historical Wave Flow journals. No execution authority.

use crate::chat::turns::BodyProvenance;
use crate::engine::invocation::{deserialize_steps, QueuedInvocation, StepRef};
use crate::engine::ConcreteStep;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct InvocationState {
    pub id: String,
    pub flow: String,
    #[serde(deserialize_with = "deserialize_steps")]
    pub steps: Vec<ConcreteStep>,
    pub cursor: u32,
    pub iteration: u32,
    pub queue: Vec<QueuedInvocation>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Playhead {
    pub stack: Vec<InvocationState>,
    pub active: Option<BodyProvenance>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum StepOutcome {
    Completed,
    Skipped,
    Failed,
    Interrupted,
}

impl StepOutcome {
    pub fn name(self) -> &'static str {
        match self {
            Self::Completed => "completed",
            Self::Skipped => "skipped",
            Self::Failed => "failed",
            Self::Interrupted => "interrupted",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum PlayheadEvent {
    FlowEnqueued {
        parent_invocation_id: String,
        invocation_id: String,
        flow: String,
    },
    InvocationStarted {
        invocation_id: String,
        flow: String,
    },
    InvocationCompleted {
        invocation_id: String,
        flow: String,
    },
    // Reader-only evidence from v1 Wave journals.
    DefinitionReset,
    StepStarted {
        step: StepRef,
        body_id: String,
    },
    BodySessionUpdated {
        body_id: String,
        session_id: String,
    },
    StepFinished {
        step: StepRef,
        body_id: String,
        outcome: StepOutcome,
        reason: String,
    },
}
