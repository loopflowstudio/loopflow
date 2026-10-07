//! A Task's Workflow: the workflow definition it took up, fixed from then
//! on, and where the Task stands on it. Position is live state a person or a
//! conversation moves: at a node, or on an edge with the Task run carrying
//! it. `lf task run` chooses an edge, starts the edge's Flow, again when a
//! Flow process fails, and writes the arrival when one succeeds; `lf task move`
//! sets a node outright. Every
//! change is appended to the Task's history. Nothing here launches or
//! controls a Flow.

use serde::{Deserialize, Serialize};

use crate::engine::workflow::{WorkflowDefinition, END, START};

/// One Task's Workflow as `lf task status --json` carries it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Workflow {
    #[serde(flatten)]
    pub definition: WorkflowDefinition,
    pub position: WorkflowPosition,
    /// Edges `lf task run` can choose now, by their place among the edges:
    /// those leaving the Task's node, or the node its stopped edge left.
    pub outgoing: Vec<u32>,
    /// Every move of this Task, across the definitions it has taken up.
    pub history: Vec<WorkflowMove>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum WorkflowPosition {
    /// The Task waits on a person at this node, or stands at `start` or `end`.
    Node { node: String },
    /// The Task is on this edge. `process_lfid` is the `lf task run` carrying it;
    /// each Flow process it starts is that Process's child. `running` is read from
    /// that Process: an edge whose Task run has ended has stopped and holds the
    /// Task until it is chosen again or the Task is moved.
    Edge {
        edge: u32,
        process_lfid: String,
        running: bool,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
#[non_exhaustive]
pub enum WorkflowMoveKind {
    /// The Task took up this definition at `start`.
    TookUp,
    /// An edge leaving `from` was chosen; an edge with a Flow then carries the Task.
    Chose,
    /// The edge's Flow succeeded and its process put the Task at `to`.
    Arrived,
    /// The Task was put at `to` without running anything.
    Set,
}

impl WorkflowMoveKind {
    pub(crate) fn as_str(self) -> &'static str {
        match self {
            Self::TookUp => "took_up",
            Self::Chose => "chose",
            Self::Arrived => "arrived",
            Self::Set => "set",
        }
    }

    pub(crate) fn parse(kind: &str) -> Option<Self> {
        [Self::TookUp, Self::Chose, Self::Arrived, Self::Set]
            .into_iter()
            .find(|candidate| candidate.as_str() == kind)
    }
}

/// Who made a move.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
#[non_exhaustive]
pub enum WorkflowActor {
    Person,
    Conversation,
    /// An edge's Flow ending.
    Edge,
}

/// One change to a Task's position.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WorkflowMove {
    /// The definition the Task was on.
    pub workflow: String,
    pub kind: WorkflowMoveKind,
    pub from: String,
    pub to: String,
    pub edge: Option<u32>,
    /// The `lf` process that made the move; for an edge, its Task run.
    /// `None` when no registered process made it.
    pub process_lfid: Option<String>,
    pub actor: WorkflowActor,
    /// The conversation that asked, when `actor` is one.
    pub session_id: Option<String>,
    pub note: Option<String>,
    pub at: i64,
}

impl Workflow {
    pub(crate) fn new(
        definition: WorkflowDefinition,
        position: WorkflowPosition,
        history: Vec<WorkflowMove>,
    ) -> Self {
        let outgoing = match &position {
            WorkflowPosition::Edge { running: true, .. } => Vec::new(),
            WorkflowPosition::Node { node } => leaving(&definition, node),
            WorkflowPosition::Edge { edge, .. } => {
                leaving(&definition, &definition.edges[*edge as usize].from)
            }
        };
        Self {
            definition,
            position,
            outgoing,
            history,
        }
    }

    /// Where the Task waits, or the node its edge left.
    pub(crate) fn node(&self) -> &str {
        match &self.position {
            WorkflowPosition::Node { node } => node,
            WorkflowPosition::Edge { edge, .. } => &self.definition.edges[*edge as usize].from,
        }
    }

    /// Whether the Task can be put at `node`.
    pub(crate) fn names_node(&self, node: &str) -> bool {
        node == START || node == END || self.definition.node(node).is_some()
    }

    /// The edges leaving the Task's node, as a caller would name them.
    pub(crate) fn describe_outgoing(&self) -> String {
        let edges: Vec<String> = self
            .definition
            .outgoing(self.node())
            .map(|(_, edge)| match &edge.flow {
                Some(flow) => format!("{flow} (to {})", edge.to),
                None => format!("{} (runs no Flow)", edge.to),
            })
            .collect();
        if edges.is_empty() {
            "none".to_string()
        } else {
            edges.join(", ")
        }
    }

    /// One line for text status.
    pub fn summary(&self) -> String {
        let name = &self.definition.name;
        match &self.position {
            WorkflowPosition::Node { node } => format!("Workflow {name}: at {node}"),
            WorkflowPosition::Edge { edge, running, .. } => {
                let edge = &self.definition.edges[*edge as usize];
                format!(
                    "Workflow {name}: {} {} ({} → {})",
                    if *running { "running" } else { "stopped on" },
                    edge.name(),
                    edge.from,
                    edge.to
                )
            }
        }
    }

    /// What the Task conversation needs: its node's skill and the command
    /// for each edge.
    pub(crate) fn guidance(&self, issue: &str) -> String {
        let summary = self.summary();
        let mut lines = match &self.position {
            WorkflowPosition::Edge { running: true, .. } => {
                return format!("{summary}. A person takes part again when that Flow finishes.");
            }
            WorkflowPosition::Edge { .. } => vec![format!(
                "{summary}. Read `lf flow list --for-task {issue}` before choosing: run it again, take another edge leaving {}, or move the Task.",
                self.node()
            )],
            WorkflowPosition::Node { node } => vec![match self.definition.node(node) {
                Some(authored) => format!(
                    "{summary}. The Task conversation works this node with the {} skill.",
                    authored.skill
                ),
                None => format!("{summary}."),
            }],
        };
        if !self.outgoing.is_empty() {
            lines.push("The person's feedback in the conversation chooses the edge; background the command with your own tool:".into());
        }
        for index in &self.outgoing {
            let edge = &self.definition.edges[*index as usize];
            let runs = match edge.flow {
                Some(_) => "",
                None => " and runs nothing",
            };
            lines.push(format!(
                "- `lf -b task run {issue} {}` moves the Task to {}{runs}",
                edge.name(),
                edge.to
            ));
        }
        lines.push(format!(
            "`lf task move {issue} <node>` puts the Task at a node without running anything: to go back, skip ahead, or record work that finished elsewhere."
        ));
        lines.join("\n")
    }
}

fn leaving(definition: &WorkflowDefinition, node: &str) -> Vec<u32> {
    definition.outgoing(node).map(|(index, _)| index).collect()
}

#[cfg(test)]
mod tests {
    use super::{Workflow, WorkflowPosition};
    use crate::engine::workflow::load_workflow;

    #[test]
    fn a_stopped_edge_holds_the_task_and_guidance_names_every_edge() {
        let repo = tempfile::tempdir().unwrap();
        let definition = load_workflow("feature", repo.path()).unwrap().unwrap();
        let at = |position| Workflow::new(definition.clone(), position, Vec::new());
        // At a node the conversation is told its skill and each edge.
        let waiting = at(WorkflowPosition::Node {
            node: "demo".into(),
        });
        assert_eq!(waiting.outgoing, [3, 4]);
        let guidance = waiting.guidance("INF-1");
        assert!(guidance.contains("with the demo skill"), "{guidance}");
        assert!(guidance.contains("`lf -b task run INF-1 ship` moves the Task to end"));
        assert!(guidance.contains("`lf task move INF-1 <node>`"));

        // On a running edge there is nothing to choose, only that the Flow runs.
        let on_edge = |running| {
            at(WorkflowPosition::Edge {
                edge: 2,
                process_lfid: "process".into(),
                running,
            })
        };
        let running = on_edge(true);
        assert!(running.outgoing.is_empty());
        assert!(!running.guidance("INF-1").contains("task run"));

        // A stopped edge holds the Task; the edges it can take are those of the node
        // it left, the stopped edge among them.
        let stopped = on_edge(false);
        assert_eq!(stopped.outgoing, [1, 2]);
        let guidance = stopped.guidance("INF-1");
        assert!(
            guidance.contains("stopped on pursue (design → demo)"),
            "{guidance}"
        );
        assert!(guidance.contains("`lf flow list --for-task INF-1`"));
        assert!(guidance.contains("`lf -b task run INF-1 pursue`"));
        assert!(guidance.contains("`lf -b task run INF-1 task-design`"));
    }
}
