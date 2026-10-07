//! Task run legality, independent of Workflow selection and execution history.
use crate::durable::WorkStatus;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TaskRunControl {
    pub unavailable: Option<String>,
}

/// Facts beyond the Flow record that decide control legality.
#[derive(Debug)]
pub(crate) struct TaskRunGate<'a> {
    /// `None` when no durable Task Work exists yet.
    pub status: Option<&'a WorkStatus>,
    pub plan_terminal_reason: Option<&'a str>,
    pub worktree_blocker: Option<&'a str>,
    pub launch_refusal: Option<&'a str>,
}

pub(crate) fn task_run_control(gate: &TaskRunGate) -> TaskRunControl {
    let terminal = match gate.status {
        Some(WorkStatus::Done) => Some("Task is complete"),
        Some(WorkStatus::Abandoned) => Some("Task is abandoned; recover it before running a Flow"),
        _ => gate.plan_terminal_reason,
    };
    TaskRunControl {
        unavailable: terminal
            .or(gate.worktree_blocker)
            .or(gate.launch_refusal)
            .map(str::to_string),
    }
}

#[cfg(test)]
mod tests {
    use super::{task_run_control, TaskRunGate};
    use crate::durable::WorkStatus;

    #[test]
    fn run_legality_preserves_terminal_checkout_and_process_refusals() {
        for (status, plan, checkout, process, expected) in [
            (None, None, None, None, None),
            (
                Some(WorkStatus::Done),
                None,
                None,
                None,
                Some("Task is complete"),
            ),
            (None, Some("withdrawn"), None, None, Some("withdrawn")),
            (
                None,
                None,
                Some("missing checkout"),
                None,
                Some("missing checkout"),
            ),
            (
                None,
                None,
                None,
                Some("active process"),
                Some("active process"),
            ),
        ] {
            let control = task_run_control(&TaskRunGate {
                status: status.as_ref(),
                plan_terminal_reason: plan,
                worktree_blocker: checkout,
                launch_refusal: process,
            });
            assert_eq!(control.unavailable.as_deref(), expected);
        }
    }
}
