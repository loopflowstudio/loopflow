//! A Task's workflow and where the Task stands on it. The record is the
//! workflow as captured when the Task took it up, plus one row per edge
//! `lf task run` set out on. Position is read from those rows and their Execs:
//! on an edge while its driver runs, at the edge's `to` once it succeeded, back
//! at `from` when it stopped. Nothing here launches or controls a Flow.

use serde::{Deserialize, Serialize};

use crate::engine::workflow::{Workflow, WorkflowEdge, WorkflowStage, START};
use crate::exec::Exec;
use crate::work::task::Task;

/// A Task's workflow as the store holds it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct TaskWorkflowRecord {
    pub id: i64,
    pub workflow: Workflow,
    /// Edges set out on, oldest first, each with the Exec that ran it.
    pub traversals: Vec<(u32, Exec)>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum WorkflowPosition {
    /// The Task waits on a person at this stage, or stands at `start` or `end`.
    Stage { stage: String },
    /// This edge is running. `exec_id` is its driver, which names its Flow.
    Edge { edge: u32, exec_id: String },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WorkflowTraversal {
    pub edge: u32,
    pub exec_id: String,
}

/// The workflow graph, the Task's position on it, and every edge set out on.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TaskWorkflowSnapshot {
    pub name: String,
    pub stages: Vec<WorkflowStage>,
    pub edges: Vec<WorkflowEdge>,
    pub position: WorkflowPosition,
    pub traversals: Vec<WorkflowTraversal>,
}

impl TaskWorkflowRecord {
    /// `running` says whether an Exec with no recorded exit still has its process.
    pub(crate) fn position(&self, running: impl Fn(&Exec) -> bool) -> WorkflowPosition {
        let mut stage = START;
        for (index, (edge, exec)) in self.traversals.iter().enumerate() {
            let Some(authored) = self.workflow.edges.get(*edge as usize) else {
                continue;
            };
            if exec.outcome.as_deref() == Some("succeeded") {
                stage = &authored.to;
            } else if index + 1 == self.traversals.len()
                && exec.completed_at.is_none()
                && running(exec)
            {
                return WorkflowPosition::Edge {
                    edge: *edge,
                    exec_id: exec.id.to_string(),
                };
            }
        }
        WorkflowPosition::Stage {
            stage: stage.to_string(),
        }
    }

    /// The stage whose outgoing edges a new traversal chooses among: where the
    /// Task stands, or where its running edge left from.
    pub(crate) fn departure(&self, position: &WorkflowPosition) -> String {
        match position {
            WorkflowPosition::Stage { stage } => stage.clone(),
            WorkflowPosition::Edge { edge, .. } => self.workflow.edges[*edge as usize].from.clone(),
        }
    }

    pub(crate) fn snapshot(self, position: WorkflowPosition) -> TaskWorkflowSnapshot {
        TaskWorkflowSnapshot {
            name: self.workflow.name,
            stages: self.workflow.stages,
            edges: self.workflow.edges,
            position,
            traversals: self
                .traversals
                .into_iter()
                .map(|(edge, exec)| WorkflowTraversal {
                    edge,
                    exec_id: exec.id.to_string(),
                })
                .collect(),
        }
    }
}

impl TaskWorkflowSnapshot {
    /// One line for text status and the Task conversation.
    pub fn summary(&self) -> String {
        match &self.position {
            WorkflowPosition::Stage { stage } => format!("Workflow {}: at {stage}", self.name),
            WorkflowPosition::Edge { edge, .. } => {
                let edge = &self.edges[*edge as usize];
                format!(
                    "Workflow {}: running {} ({} → {})",
                    self.name,
                    edge.flow.as_deref().unwrap_or("no flow"),
                    edge.from,
                    edge.to
                )
            }
        }
    }
}

/// What the Task conversation needs at its current stage: the stage's skill
/// and the commands that set out on each outgoing edge.
pub(crate) fn guidance(store: &crate::store::sqlite::SqliteStore, task: &Task) -> Option<String> {
    let record = store.task_workflow(&task.id).ok()??;
    let position = record.position(|exec| store.exec_may_run(exec));
    Some(record.guidance(position, &task.plan.identifier))
}

impl TaskWorkflowRecord {
    fn guidance(self, position: WorkflowPosition, issue: &str) -> String {
        let workflow = self.workflow.clone();
        let summary = self.snapshot(position.clone()).summary();
        let WorkflowPosition::Stage { stage } = &position else {
            return format!("{summary}. A person takes part again when that Flow finishes.");
        };
        let mut lines = vec![match workflow.stage(stage) {
            Some(authored) => format!(
                "{summary}. The Task conversation works this stage with the {} skill.",
                authored.skill
            ),
            None => format!("{summary}."),
        }];
        for (_, edge) in workflow.outgoing(stage) {
            lines.push(match &edge.flow {
                Some(flow) => format!(
                    "- `lf -b task run {issue} {flow}` moves the Task to {}",
                    edge.to
                ),
                None => format!("- `lf task run {issue}` moves the Task to {}", edge.to),
            });
        }
        if lines.len() > 1 {
            lines.insert(1, "The person's feedback in the conversation chooses the edge; background the command with your own tool:".into());
        }
        lines.join("\n")
    }
}

/// The outgoing edges of `stage`, as a caller would name them.
pub(crate) fn describe_edges(workflow: &Workflow, stage: &str) -> String {
    let edges: Vec<String> = workflow
        .outgoing(stage)
        .map(|(_, edge)| match &edge.flow {
            Some(flow) => format!("{flow} (to {})", edge.to),
            None => format!("no flow (to {})", edge.to),
        })
        .collect();
    if edges.is_empty() {
        "none".to_string()
    } else {
        edges.join(", ")
    }
}

#[cfg(test)]
mod tests {
    use super::{TaskWorkflowRecord, WorkflowPosition};
    use crate::engine::workflow::load_workflow;
    use crate::exec::Exec;

    fn exec(outcome: Option<&str>) -> Exec {
        Exec {
            id: crate::id::ExecId::new(),
            trace_id: crate::id::TraceId::new(),
            parent_exec_id: None,
            via_agent: None,
            caller_session_id: None,
            caller_provider_generation: None,
            command: None,
            repo: None,
            cwd: None,
            started_at: 1,
            completed_at: outcome.map(|_| 2),
            outcome: outcome.map(str::to_string),
            exit_code: None,
            signal: None,
            error: None,
        }
    }

    #[test]
    fn position_follows_how_each_traversed_edge_ended() {
        let repo = tempfile::tempdir().unwrap();
        let workflow = load_workflow("feature", repo.path()).unwrap().unwrap();
        let record = |traversals: Vec<(u32, Exec)>| TaskWorkflowRecord {
            id: 1,
            workflow: workflow.clone(),
            traversals,
        };
        let at = |stage: &str| WorkflowPosition::Stage {
            stage: stage.into(),
        };
        let live = |_: &Exec| true;
        assert_eq!(record(vec![]).position(live), at("start"));
        // start → design, design → demo, then a failed landing edge returns to demo.
        let landed = record(vec![
            (0, exec(Some("succeeded"))),
            (2, exec(Some("succeeded"))),
            (4, exec(Some("failed"))),
        ]);
        assert_eq!(landed.position(live), at("demo"));
        let running = record(vec![(0, exec(Some("succeeded"))), (2, exec(None))]);
        let WorkflowPosition::Edge { edge, .. } = running.position(live) else {
            panic!("a live driver is on its edge");
        };
        assert_eq!(edge, 2);
        assert_eq!(running.departure(&running.position(live)), "design");
        // A driver that died without an exit record stopped: back at `from`.
        assert_eq!(running.position(|_| false), at("design"));
        // At a stage the conversation is told its skill and each way out;
        // on an edge, only that the Flow runs.
        let guidance = landed.guidance(at("demo"), "INF-1");
        assert!(guidance.contains("with the demo skill"), "{guidance}");
        assert!(guidance.contains("`lf -b task run INF-1 ship` moves the Task to end"));
        let position = running.position(live);
        assert!(!running.guidance(position, "INF-1").contains("task run"));
    }
}
