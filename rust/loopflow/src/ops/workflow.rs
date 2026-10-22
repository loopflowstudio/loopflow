//! A Task's Workflow: the graph it took up, fixed from then on, and where the
//! Task stands on it. Position is live state a person or a conversation
//! moves: at a stage, or on a route with the Task run carrying it. `lf task
//! run` chooses a route, runs its slice of the graph, again when a Flow
//! process fails, and writes the arrival when one succeeds; `lf task workflow
//! move` sets a stage outright. Every change is appended to the Task's
//! history. Nothing here launches or controls a Flow.

use serde::{Deserialize, Serialize};

use crate::workflow::{Route, WorkflowDefinition, END, START};

/// One Task's Workflow as `lf task status --json` carries it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Workflow {
    #[serde(flatten)]
    pub definition: WorkflowDefinition,
    pub position: WorkflowPosition,
    /// Edges `lf task run` can choose now, by their place among the edges:
    /// those leaving the Task's stage, or the stage its stopped edge left.
    pub outgoing: Vec<u32>,
    /// Every move of this Task, across the graphs it has taken up.
    pub history: Vec<WorkflowMove>,
    /// Where each edge's run begins in the captured steps; projected, never
    /// carried on the wire.
    #[serde(skip)]
    pub(crate) routes: Vec<Route>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum WorkflowPosition {
    /// The Task waits on a person at this stage, or stands at `start` or `end`.
    Node { node: String },
    /// The Task is on this edge. `lf_process_id` is the `lf task run` carrying it;
    /// each Flow process it starts is that Process's child. `running` is read from
    /// that Process: an edge whose Task run has ended has stopped and holds the
    /// Task until it is chosen again or the Task is moved.
    Edge {
        edge: u32,
        lf_process_id: String,
        running: bool,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
#[non_exhaustive]
pub enum WorkflowMoveKind {
    /// The Task took up this graph, at `to`.
    TookUp,
    /// An edge leaving `from` was chosen; an edge that runs steps then carries the Task.
    Chose,
    /// The edge's run succeeded and its process put the Task at `to`.
    Arrived,
    /// The Task was put at `to` without running anything.
    Set,
    /// Someone shipped the Task from `from`: `lf ship` runs every remaining step,
    /// standing in at each stage, until the Task reaches `end`.
    Shipped,
}

impl WorkflowMoveKind {
    pub(crate) fn as_str(self) -> &'static str {
        match self {
            Self::TookUp => "took_up",
            Self::Chose => "chose",
            Self::Arrived => "arrived",
            Self::Set => "set",
            Self::Shipped => "shipped",
        }
    }

    pub(crate) fn parse(kind: &str) -> Option<Self> {
        [
            Self::TookUp,
            Self::Chose,
            Self::Arrived,
            Self::Set,
            Self::Shipped,
        ]
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
    /// An edge's run ending.
    Edge,
}

/// One change to a Task's position.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WorkflowMove {
    /// The graph the Task was on.
    pub workflow: String,
    pub kind: WorkflowMoveKind,
    pub from: String,
    pub to: String,
    pub edge: Option<u32>,
    /// The `lf` process that made the move; for an edge, its Task run.
    /// `None` when no registered process made it.
    pub lf_process_id: Option<crate::id::LfProcessId>,
    pub actor: WorkflowActor,
    /// The conversation that asked, when `actor` is one.
    pub session_id: Option<crate::id::LfSessionId>,
    pub note: Option<String>,
    pub at: i64,
}

impl Workflow {
    pub(crate) fn new(
        definition: WorkflowDefinition,
        routes: Vec<Route>,
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
            routes,
        }
    }

    /// Where the Task waits, or the stage its edge left.
    pub(crate) fn node(&self) -> &str {
        match &self.position {
            WorkflowPosition::Node { node } => node,
            WorkflowPosition::Edge { edge, .. } => &self.definition.edges[*edge as usize].from,
        }
    }

    /// The stage where a person takes part now, such as `review-design` or `demo`.
    pub(crate) fn person_node(&self) -> Option<&str> {
        match &self.position {
            WorkflowPosition::Node { node } if self.definition.node(node).is_some() => Some(node),
            _ => None,
        }
    }

    /// Whether the Task can be put at `node`.
    pub(crate) fn names_node(&self, node: &str) -> bool {
        node == START || node == END || self.definition.node(node).is_some()
    }

    /// Whether `edge` runs steps, or enters `end` running nothing.
    pub(crate) fn edge_runs(&self, edge: u32) -> bool {
        self.routes
            .get(edge as usize)
            .is_some_and(|route| route.start.is_some())
    }

    /// The edges leaving the Task's stage, as a caller would name them.
    pub(crate) fn describe_outgoing(&self) -> String {
        let edges: Vec<String> = self
            .definition
            .outgoing(self.node())
            .map(|(index, edge)| {
                if self.edge_runs(index) {
                    format!("{} (to {})", edge.name, edge.to)
                } else {
                    format!("{} (runs nothing)", edge.name)
                }
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
                    edge.name,
                    edge.from,
                    edge.to
                )
            }
        }
    }

    /// What the Task conversation needs: its stage's skill and the command
    /// for each edge.
    pub(crate) fn guidance(&self, issue: &str) -> String {
        let summary = self.summary();
        let mut lines = match &self.position {
            WorkflowPosition::Edge { running: true, .. } => {
                return format!("{summary}. A person takes part again when that run finishes.");
            }
            WorkflowPosition::Edge { .. } => vec![format!(
                "{summary}. Read `lf flow list --for-task {issue}` before choosing: run it again, take another edge leaving {}, or move the Task.",
                self.node()
            )],
            WorkflowPosition::Node { node } => vec![match self.definition.node(node) {
                Some(stage) => format!(
                    "{summary}. The Task conversation works this stage with the {} skill.",
                    stage.skill
                ),
                None => format!("{summary}."),
            }],
        };
        if !self.outgoing.is_empty() {
            lines.push("The person's feedback in the conversation chooses the edge; background the command with your own tool:".into());
        }
        for index in &self.outgoing {
            let edge = &self.definition.edges[*index as usize];
            let runs = if self.edge_runs(*index) {
                ""
            } else {
                " and runs nothing"
            };
            lines.push(format!(
                "- `lf -b task run {issue} {}` moves the Task to {}{runs}",
                edge.name, edge.to
            ));
        }
        lines.push(format!(
            "`lf task workflow move {issue} <stage>` puts the Task at a stage without running anything: to go back, skip ahead, or record work that finished elsewhere. `lf ship` runs every remaining step, standing in for the person at each stage."
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
    use crate::workflow::CapturedGraph;

    #[test]
    fn a_stopped_edge_holds_the_task_and_guidance_names_every_edge() {
        let repo = tempfile::tempdir().unwrap();
        let graph = CapturedGraph::load("feature", repo.path())
            .unwrap()
            .unwrap();
        let (definition, routes) = graph.project().unwrap();
        let at = |position| Workflow::new(definition.clone(), routes.clone(), position, Vec::new());
        // At a stage the conversation is told its skill and each edge.
        let waiting = at(WorkflowPosition::Node {
            node: "demo".into(),
        });
        assert_eq!(waiting.outgoing, [3, 4]);
        let guidance = waiting.guidance("INF-1");
        assert!(guidance.contains("with the demo skill"), "{guidance}");
        assert!(guidance.contains("`lf -b task run INF-1 deliver` moves the Task to end"));
        assert!(guidance.contains("`lf task workflow move INF-1 <stage>`"));

        // On a running edge there is nothing to choose, only that the run runs.
        let on_edge = |running| {
            at(WorkflowPosition::Edge {
                edge: 2,
                lf_process_id: "process".into(),
                running,
            })
        };
        let running = on_edge(true);
        assert!(running.outgoing.is_empty());
        assert!(!running.guidance("INF-1").contains("task run"));

        // A stopped edge holds the Task; the edges it can take are those of the
        // stage it left, the stopped edge among them.
        let stopped = on_edge(false);
        assert_eq!(stopped.outgoing, [1, 2]);
        let guidance = stopped.guidance("INF-1");
        assert!(
            guidance.contains("stopped on pursue (review-design → demo)"),
            "{guidance}"
        );
        assert!(guidance.contains("`lf flow list --for-task INF-1`"));
        assert!(guidance.contains("`lf -b task run INF-1 pursue`"));
        assert!(guidance.contains("`lf -b task run INF-1 kickoff`"));
        // The research graph's last edge runs nothing.
        let research = CapturedGraph::load("research", repo.path())
            .unwrap()
            .unwrap();
        let (definition, routes) = research.project().unwrap();
        let findings = Workflow::new(
            definition,
            routes,
            WorkflowPosition::Node {
                node: "research".into(),
            },
            Vec::new(),
        );
        assert!(findings.describe_outgoing().contains("end (runs nothing)"));
    }
}
