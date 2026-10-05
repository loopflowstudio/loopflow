//! A Task's Flow as surfaces draw it: the recommended definition before any
//! launch, the most recently launched invocation and where its cursor stands,
//! and whether a fresh launch is legal now. Every other Flow naming the Task is
//! equally its work and is listed with the Task's work.

use serde::{Deserialize, Serialize};

use crate::durable::{FlowSession, WorkStatus};
use crate::engine::flow_graph::{flow_iterations, project_cursor, FlowGraph, FlowReturn};
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
    /// No Flow has been launched for this Task. Runtime `started` separately
    /// reports whether any other execution is recorded.
    None,
    /// The most recently launched invocation and where its cursor stands.
    Latest(LatestTaskFlow),
    /// The most recently launched Flow finished.
    Finished { flow: String },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LatestTaskFlow {
    pub invocation_id: String,
    pub graph: FlowGraph,
    pub current: Option<u32>,
    pub completed: Vec<u32>,
    pub returns: Vec<FlowReturn>,
    /// Per-edge counts at each active nesting level, outermost first.
    pub iterations: Vec<Vec<u32>>,
    pub execution: TaskExecutionState,
    pub reason: String,
}

impl LatestTaskFlow {
    pub(crate) fn new(flow: &FlowSession, execution: &TaskExecutionSnapshot) -> Self {
        let graph = FlowGraph::new(&flow.invocation.flow, &flow.invocation.steps);
        let projection = project_cursor(&graph, &flow.cursor);
        Self {
            invocation_id: flow.invocation.id.clone(),
            graph,
            current: projection.current,
            completed: projection.completed,
            returns: projection.returns,
            iterations: flow_iterations(&flow.invocation.steps, &flow.cursor),
            execution: execution.state,
            reason: execution.reason.clone(),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TaskFlowControlKind {
    /// `lf task run ISSUE [FLOW]`
    Start,
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
    /// `None` when no durable Task Work exists yet.
    pub status: Option<&'a WorkStatus>,
    pub plan_terminal_reason: Option<&'a str>,
    pub worktree_blocker: Option<&'a str>,
    pub launch_refusal: Option<&'a str>,
}

pub(crate) fn task_flow_controls(gate: &TaskFlowGate) -> Vec<TaskFlowControl> {
    let terminal = match gate.status {
        Some(WorkStatus::Done) => Some("Task is complete"),
        Some(WorkStatus::Abandoned) => Some("Task is abandoned; recover it before running a Flow"),
        None => gate.plan_terminal_reason,
        _ => None,
    };
    vec![TaskFlowControl {
        kind: TaskFlowControlKind::Start,
        unavailable: terminal
            .or(gate.worktree_blocker)
            .or(gate.launch_refusal)
            .map(str::to_string),
    }]
}

#[cfg(test)]
mod tests {
    use super::{
        task_flow_controls, TaskFlowControlKind, TaskFlowGate, TaskFlowRecord, TaskFlowSnapshot,
    };
    use crate::durable::WorkStatus;
    use crate::ops::task_execution::TaskExecutionState;

    #[test]
    fn a_fresh_launch_is_legal_whenever_the_task_can_run() {
        let gate = |status, launch_refusal| TaskFlowGate {
            status,
            plan_terminal_reason: None,
            worktree_blocker: None,
            launch_refusal,
        };
        let start = |gate: &TaskFlowGate| {
            let controls = task_flow_controls(gate);
            assert_eq!(controls.len(), 1);
            assert_eq!(controls[0].kind, TaskFlowControlKind::Start);
            controls[0].unavailable.clone()
        };
        let ready = WorkStatus::Ready;
        assert_eq!(start(&gate(None, None)), None);
        assert_eq!(start(&gate(Some(&ready), None)), None);
        assert_eq!(
            start(&gate(Some(&ready), Some("agent unavailable"))).as_deref(),
            Some("agent unavailable")
        );
        let done = WorkStatus::Done;
        assert_eq!(
            start(&gate(Some(&done), None)).as_deref(),
            Some("Task is complete")
        );
    }

    #[test]
    fn flow_snapshot_fixture_round_trips_without_defaults() {
        let value: serde_json::Value = serde_json::from_str(include_str!(
            "../../../../tests/fixtures/dto/task_flow.json"
        ))
        .unwrap();
        let snapshots: Vec<TaskFlowSnapshot> = serde_json::from_value(value.clone()).unwrap();
        assert!(matches!(snapshots[0].record, TaskFlowRecord::None));
        let TaskFlowRecord::Latest(running) = &snapshots[1].record else {
            panic!("second fixture is a launched Flow");
        };
        // Both authored returns keep separate counts.
        let edges: Vec<_> = running
            .returns
            .iter()
            .map(|edge| (edge.decider, edge.traversals))
            .collect();
        assert_eq!(edges, [(3, 2), (5, 0)]);
        assert!(matches!(
            snapshots[4].record,
            TaskFlowRecord::Finished { .. }
        ));
        assert_eq!(serde_json::to_value(&snapshots).unwrap(), value);
        let mut string_node = value[1].clone();
        string_node["record"]["current"] = serde_json::json!("2");
        assert!(serde_json::from_value::<TaskFlowSnapshot>(string_node).is_err());
        let mut missing = value[1].clone();
        missing["record"].as_object_mut().unwrap().remove("returns");
        assert!(serde_json::from_value::<TaskFlowSnapshot>(missing).is_err());

        let stalled: serde_json::Value = serde_json::from_str(include_str!(
            "../../../../tests/fixtures/dto/task_flow_stalled.json"
        ))
        .unwrap();
        let snapshot: TaskFlowSnapshot = serde_json::from_value(stalled.clone()).unwrap();
        assert!(
            matches!(&snapshot.record, TaskFlowRecord::Latest(flow) if flow.execution == TaskExecutionState::Stalled)
        );
        assert_eq!(serde_json::to_value(snapshot).unwrap(), stalled);

        let catalog: serde_json::Value = serde_json::from_str(include_str!(
            "../../../../tests/fixtures/dto/flow_catalog.json"
        ))
        .unwrap();
        let entries: Vec<crate::engine::flow_graph::FlowCatalogEntry> =
            serde_json::from_value(catalog.clone()).unwrap();
        assert!(entries[0].graph.is_some() && entries[1].unavailable.is_some());
        assert_eq!(serde_json::to_value(&entries).unwrap(), catalog);
    }
}
