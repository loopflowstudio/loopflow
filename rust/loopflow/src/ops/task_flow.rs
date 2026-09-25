//! A Task's Flow as surfaces draw it: the recommended definition before start,
//! the pinned invocation and its saved cursor once started, and the Flow
//! controls that are legal now. Legality and reasons live here so clients
//! render them without a lifecycle matrix of their own.

use serde::{Deserialize, Serialize};

use crate::durable::{FlowPosition, WorkStatus};
use crate::engine::flow_graph::{project_cursor, FlowGraph, FlowReturn};
use crate::ops::task_execution::{TaskExecutionSnapshot, TaskExecutionState};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TaskFlowSnapshot {
    /// The Flow a Start without a selection runs: the chapter recommendation,
    /// else the builtin default.
    pub recommended: String,
    pub record: TaskFlowRecord,
    pub controls: Vec<TaskFlowControl>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum TaskFlowRecord {
    /// No Flow position or finished Flow is recorded for this Task. Runtime
    /// `started` separately reports whether any other execution is recorded.
    None,
    /// The captured invocation and where its saved cursor stands.
    Pinned(PinnedTaskFlow),
    /// The Flow finished; its captured definition is not retained, so no
    /// topology is drawn in its place.
    Finished { flow: String },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PinnedTaskFlow {
    pub invocation_id: String,
    pub graph: FlowGraph,
    pub current: Option<String>,
    pub completed: Vec<String>,
    pub returns: Vec<FlowReturn>,
    pub execution: TaskExecutionState,
    pub reason: String,
    /// A durable blocker that only a restart can clear.
    pub restart_required: bool,
}

impl PinnedTaskFlow {
    pub(crate) fn new(position: &FlowPosition, execution: &TaskExecutionSnapshot) -> Self {
        let projection = project_cursor(&position.invocation.steps, &position.cursor);
        Self {
            invocation_id: position.invocation.id.clone(),
            graph: FlowGraph::new(&position.invocation.flow, &position.invocation.steps),
            current: projection.current,
            completed: projection.completed,
            returns: projection.returns,
            execution: execution.state,
            reason: execution.reason.clone(),
            restart_required: position
                .failure
                .as_ref()
                .is_some_and(|failure| failure.restart_required),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TaskFlowControlKind {
    /// `lf task run ISSUE --flow FLOW`
    Start,
    /// `lf task resume ISSUE`
    Resume,
    /// `lf task restart ISSUE --flow FLOW`
    Restart,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TaskFlowControl {
    pub kind: TaskFlowControlKind,
    /// Why the control is unavailable now; `None` when it may be invoked.
    pub unavailable: Option<String>,
}

/// Facts beyond the Flow record that decide control legality.
#[derive(Debug)]
pub(crate) struct TaskFlowGate<'a> {
    pub identifier: &'a str,
    /// `None` when no durable Task Work exists yet.
    pub status: Option<&'a WorkStatus>,
    pub plan_completed: bool,
    pub execution: Option<&'a TaskExecutionSnapshot>,
    pub worktree_blocker: Option<&'a str>,
    pub launch_refusal: Option<&'a str>,
    pub resume_refusal: Option<&'a str>,
}

pub(crate) fn task_flow_controls(
    record: &TaskFlowRecord,
    gate: &TaskFlowGate,
) -> Vec<TaskFlowControl> {
    let terminal = match gate.status {
        Some(WorkStatus::Done) => Some("Task is complete"),
        Some(WorkStatus::Abandoned) => Some("Task is abandoned; recover it before running a Flow"),
        None if gate.plan_completed => Some("Linear Task is complete"),
        _ => None,
    };
    let control = |kind, unavailable: Option<String>| TaskFlowControl {
        kind,
        unavailable: terminal.map(str::to_string).or(unavailable),
    };
    let pinned = matches!(record, TaskFlowRecord::Pinned(_));
    let blocker = gate.worktree_blocker.or(gate.launch_refusal);

    let start = if pinned {
        Some("A Flow is already pinned; resume it or stop and restart with another".to_string())
    } else {
        blocker.map(str::to_string)
    };

    let resume = match record {
        TaskFlowRecord::Pinned(flow) => match flow.execution {
            TaskExecutionState::Running | TaskExecutionState::Starting => {
                Some("The Task worker is already advancing this Flow".to_string())
            }
            TaskExecutionState::Human => Some(format!(
                "{}; continue it through its Session",
                flow.reason
            )),
            TaskExecutionState::Unknown => Some(flow.reason.clone()),
            TaskExecutionState::Blocked if flow.restart_required => Some(format!(
                "{}. Only Stop & restart can clear this blocker",
                flow.reason
            )),
            TaskExecutionState::Blocked => Some(format!(
                "{}. After correcting it, resume with a stated reason: `lf task resume {} --reason \"<what changed>\"`",
                flow.reason, gate.identifier
            )),
            TaskExecutionState::Idle => gate.resume_refusal.map(str::to_string),
        },
        TaskFlowRecord::None | TaskFlowRecord::Finished { .. } => {
            Some("No pinned Flow to resume; start one".to_string())
        }
    };

    let restart = if gate.status.is_none() {
        Some("Task has no Work yet; start a Flow instead".to_string())
    } else if gate
        .execution
        .is_some_and(|execution| execution.state == TaskExecutionState::Unknown)
    {
        gate.execution.map(|execution| execution.reason.clone())
    } else {
        gate.worktree_blocker.map(str::to_string)
    };

    vec![
        control(TaskFlowControlKind::Start, start),
        control(TaskFlowControlKind::Resume, resume),
        control(TaskFlowControlKind::Restart, restart),
    ]
}

#[cfg(test)]
mod tests {
    use super::{
        task_flow_controls, PinnedTaskFlow, TaskFlowControlKind, TaskFlowGate, TaskFlowRecord,
        TaskFlowSnapshot,
    };
    use crate::durable::WorkStatus;
    use crate::engine::flow_graph::FlowGraph;
    use crate::ops::task_execution::TaskExecutionState;

    fn pinned(execution: TaskExecutionState, restart_required: bool) -> TaskFlowRecord {
        TaskFlowRecord::Pinned(PinnedTaskFlow {
            invocation_id: "inv".into(),
            graph: FlowGraph::new("feature", &[]),
            current: Some("2".into()),
            completed: vec!["0".into(), "1".into()],
            returns: Vec::new(),
            execution,
            reason: "Waiting for your review at demo".into(),
            restart_required,
        })
    }

    fn available(record: &TaskFlowRecord, status: Option<&WorkStatus>) -> Vec<TaskFlowControlKind> {
        task_flow_controls(
            record,
            &TaskFlowGate {
                identifier: "LOO-1",
                status,
                plan_completed: false,
                execution: None,
                worktree_blocker: None,
                launch_refusal: None,
                resume_refusal: None,
            },
        )
        .into_iter()
        .filter(|control| control.unavailable.is_none())
        .map(|control| control.kind)
        .collect()
    }

    #[test]
    fn controls_follow_the_saved_boundary_not_a_client_matrix() {
        let ready = WorkStatus::Ready;
        assert_eq!(
            available(&TaskFlowRecord::None, None),
            [TaskFlowControlKind::Start]
        );
        assert_eq!(
            available(&pinned(TaskExecutionState::Idle, false), Some(&ready)),
            [TaskFlowControlKind::Resume, TaskFlowControlKind::Restart]
        );
        for busy in [
            TaskExecutionState::Running,
            TaskExecutionState::Human,
            TaskExecutionState::Blocked,
        ] {
            assert_eq!(
                available(&pinned(busy, false), Some(&ready)),
                [TaskFlowControlKind::Restart],
                "{busy:?}"
            );
        }
        let done = WorkStatus::Done;
        assert!(available(&pinned(TaskExecutionState::Idle, false), Some(&done)).is_empty());
    }

    #[test]
    fn flow_snapshot_fixture_round_trips_without_defaults() {
        let value: serde_json::Value = serde_json::from_str(include_str!(
            "../../../../tests/fixtures/dto/task_flow.json"
        ))
        .unwrap();
        let snapshots: Vec<TaskFlowSnapshot> = serde_json::from_value(value.clone()).unwrap();
        assert!(matches!(snapshots[0].record, TaskFlowRecord::None));
        assert!(matches!(snapshots[1].record, TaskFlowRecord::Pinned(_)));
        assert!(matches!(
            snapshots[2].record,
            TaskFlowRecord::Finished { .. }
        ));
        assert_eq!(serde_json::to_value(&snapshots).unwrap(), value);
        let mut missing = value[1].clone();
        missing["record"].as_object_mut().unwrap().remove("returns");
        assert!(serde_json::from_value::<TaskFlowSnapshot>(missing).is_err());
    }
}
