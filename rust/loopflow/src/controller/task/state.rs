use serde::{Deserialize, Serialize};
use time::OffsetDateTime;

use crate::work::task::{TaskDataError, TaskId};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
#[non_exhaustive]
pub enum TaskLifecyclePhase {
    First,
    Loop,
    Finally,
}

impl TaskLifecyclePhase {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::First => "first",
            Self::Loop => "loop",
            Self::Finally => "finally",
        }
    }

    pub(crate) fn storage_str(self) -> &'static str {
        match self {
            Self::First => "kickoff",
            Self::Loop => "iterate",
            Self::Finally => "gate",
        }
    }

    pub(crate) fn from_storage_str(value: &str) -> Result<Self, TaskDataError> {
        match value {
            "kickoff" => Ok(Self::First),
            "iterate" => Ok(Self::Loop),
            "gate" => Ok(Self::Finally),
            _ => Err(TaskDataError::InvalidInvariant(format!(
                "invalid stored Task lifecycle phase: {value}"
            ))),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TaskPhasePlan {
    pub flow: String,
}

impl TaskPhasePlan {
    fn validate(&self, phase: TaskLifecyclePhase) -> Result<(), TaskDataError> {
        if self.flow.trim().is_empty() {
            return Err(TaskDataError::InvalidInvariant(format!(
                "{} flow cannot be empty",
                phase.as_str()
            )));
        }
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TaskLifecyclePlan {
    pub first: TaskPhasePlan,
    #[serde(rename = "loop")]
    pub loop_: TaskPhasePlan,
    pub finally: TaskPhasePlan,
}

impl TaskLifecyclePlan {
    pub fn standard(
        first_flow: impl Into<String>,
        loop_flow: impl Into<String>,
        finally_flow: impl Into<String>,
    ) -> Self {
        Self {
            first: TaskPhasePlan {
                flow: first_flow.into(),
            },
            loop_: TaskPhasePlan {
                flow: loop_flow.into(),
            },
            finally: TaskPhasePlan {
                flow: finally_flow.into(),
            },
        }
    }

    pub fn defaults() -> Self {
        Self::standard("task-design", "slice", "ship-demo")
    }

    pub fn phase(&self, phase: TaskLifecyclePhase) -> &TaskPhasePlan {
        match phase {
            TaskLifecyclePhase::First => &self.first,
            TaskLifecyclePhase::Loop => &self.loop_,
            TaskLifecyclePhase::Finally => &self.finally,
        }
    }

    fn validate(&self) -> Result<(), TaskDataError> {
        self.first.validate(TaskLifecyclePhase::First)?;
        self.loop_.validate(TaskLifecyclePhase::Loop)?;
        self.finally.validate(TaskLifecyclePhase::Finally)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TaskGateProposal {
    pub done: bool,
    pub reason: String,
}

impl TaskGateProposal {
    fn validate(&self) -> Result<(), TaskDataError> {
        if self.reason.trim().is_empty() {
            return Err(TaskDataError::InvalidInvariant(
                "gate proposal reason cannot be empty".to_string(),
            ));
        }
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct State {
    pub task_id: TaskId,
    pub lifecycle: TaskLifecyclePlan,
    pub lifecycle_phase: TaskLifecyclePhase,
    pub phase_cursor: u32,
    pub phase_iteration: u32,
    pub gate_cycle: u32,
    pub gate_proposal: Option<TaskGateProposal>,
    pub agent: String,
    pub provider: String,
    pub provider_session_id: Option<String>,
    pub updated_at: OffsetDateTime,
}

impl State {
    pub(crate) fn validate(&self) -> Result<(), TaskDataError> {
        self.lifecycle.validate()?;
        if self.lifecycle_phase == TaskLifecyclePhase::Finally && self.gate_proposal.is_none() {
            return Err(TaskDataError::InvalidInvariant(
                "Task finally phase requires a proposed outcome".to_string(),
            ));
        }
        if self.lifecycle_phase != TaskLifecyclePhase::Finally && self.gate_proposal.is_some() {
            return Err(TaskDataError::InvalidInvariant(
                "Task gate proposal is valid only during finally phase".to_string(),
            ));
        }
        if let Some(proposal) = &self.gate_proposal {
            proposal.validate()?;
        }
        Ok(())
    }

    pub(crate) fn phase_plan(&self) -> &TaskPhasePlan {
        self.lifecycle.phase(self.lifecycle_phase)
    }

    pub(crate) fn enter_loop(&mut self) -> Result<(), TaskDataError> {
        if self.lifecycle_phase != TaskLifecyclePhase::First
            && self.lifecycle_phase != TaskLifecyclePhase::Finally
        {
            return Err(TaskDataError::InvalidInvariant(
                "only first or finally may enter loop".to_string(),
            ));
        }
        self.lifecycle_phase = TaskLifecyclePhase::Loop;
        self.phase_cursor = 0;
        self.phase_iteration = 0;
        self.gate_proposal = None;
        self.updated_at = OffsetDateTime::now_utc();
        Ok(())
    }

    pub(crate) fn enter_finally(
        &mut self,
        proposal: TaskGateProposal,
    ) -> Result<(), TaskDataError> {
        if self.lifecycle_phase != TaskLifecyclePhase::Loop {
            return Err(TaskDataError::InvalidInvariant(
                "only loop may enter finally".to_string(),
            ));
        }
        proposal.validate()?;
        self.lifecycle_phase = TaskLifecyclePhase::Finally;
        self.phase_cursor = 0;
        self.phase_iteration = 0;
        self.gate_cycle += 1;
        self.gate_proposal = Some(proposal);
        self.updated_at = OffsetDateTime::now_utc();
        Ok(())
    }

    pub(crate) fn approved_gate_proposal(&self) -> Result<TaskGateProposal, TaskDataError> {
        if self.lifecycle_phase != TaskLifecyclePhase::Finally {
            return Err(TaskDataError::InvalidInvariant(
                "only finally may approve a proposed outcome".to_string(),
            ));
        }
        self.gate_proposal.clone().ok_or_else(|| {
            TaskDataError::InvalidInvariant("Task gate has no proposed outcome".to_string())
        })
    }
}

#[cfg(test)]
mod tests {
    use super::{State, TaskGateProposal, TaskLifecyclePhase, TaskLifecyclePlan};

    fn state() -> State {
        State {
            task_id: crate::work::task::TaskId::new(),
            lifecycle: TaskLifecyclePlan::defaults(),
            lifecycle_phase: TaskLifecyclePhase::First,
            phase_cursor: 0,
            phase_iteration: 0,
            gate_cycle: 0,
            gate_proposal: None,
            agent: "codex".to_string(),
            provider: "codex".to_string(),
            provider_session_id: None,
            updated_at: time::OffsetDateTime::now_utc(),
        }
    }

    #[test]
    fn controller_state_repeats_loop_and_finally_until_approval() {
        let mut state = state();
        state.enter_loop().unwrap();
        let proposal = TaskGateProposal {
            done: false,
            reason: "another pass".to_string(),
        };
        state.phase_cursor = 2;
        state.phase_iteration = 3;
        state.enter_finally(proposal.clone()).unwrap();

        assert_eq!(state.lifecycle_phase, TaskLifecyclePhase::Finally);
        assert_eq!(state.gate_cycle, 1);
        assert_eq!(state.approved_gate_proposal().unwrap(), proposal);
        assert_eq!((state.phase_cursor, state.phase_iteration), (0, 0));

        state.enter_loop().unwrap();
        assert_eq!(state.lifecycle_phase, TaskLifecyclePhase::Loop);
        assert_eq!(state.gate_proposal, None);
    }

    #[test]
    fn lifecycle_storage_names_remain_stable() {
        for (phase, public, stored) in [
            (TaskLifecyclePhase::First, "first", "kickoff"),
            (TaskLifecyclePhase::Loop, "loop", "iterate"),
            (TaskLifecyclePhase::Finally, "finally", "gate"),
        ] {
            assert_eq!(phase.as_str(), public);
            assert_eq!(phase.storage_str(), stored);
            assert_eq!(TaskLifecyclePhase::from_storage_str(stored).unwrap(), phase);
        }
    }
}
