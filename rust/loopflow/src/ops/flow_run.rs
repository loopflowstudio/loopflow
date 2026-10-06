//! A Flow exec is one driver Exec and the step Execs it starts. Its driver
//! writes a FlowExec: the Flow's name and graph as compiled at launch, then one
//! row per step it starts. The record is append-only and only the driver writes
//! it. Each step is an ordinary command that knows nothing of its Flow; whether
//! the Flow or a step is running, finished or failed is read from their Execs.
use crate::engine::flow_graph::FlowGraph;
use crate::exec::Exec;
use crate::id::ExecId;

/// A step's agent sees which Flow started it: the driver Exec's id. Steps of
/// one Flow can share notes under it. It configures nothing in lf.
pub(crate) const FLOW_ID_ENV: &str = "LF_FLOW_ID";

/// One step its driver started.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct FlowExecStep {
    pub exec: Exec,
    /// The step's node in the Flow's graph, counted in preorder.
    pub key: u32,
    /// Returns taken on each loop edge, per nesting level, outermost first.
    pub iterations: Vec<Vec<u32>>,
}

/// One Flow exec as its driver recorded it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct FlowExec {
    pub driver: Exec,
    pub name: String,
    pub graph: FlowGraph,
    /// Steps in launch order. A repeated or corrected step is another entry.
    pub steps: Vec<FlowExecStep>,
}

impl FlowExec {
    pub(crate) fn id(&self) -> &ExecId {
        &self.driver.id
    }

    pub(crate) fn latest(&self) -> Option<&FlowExecStep> {
        self.steps.last()
    }

    /// The graph label of a recorded step.
    pub(crate) fn label(&self, step: &FlowExecStep) -> String {
        self.graph
            .node_at(step.key)
            .map(|node| node.label.clone())
            .unwrap_or_default()
    }

    /// How far the Flow got on the graph it launched with.
    pub(crate) fn detail(
        &self,
        entry: crate::durable::FlowInventoryEntry,
    ) -> crate::durable::FlowDetail {
        let finished = entry.summary.state == crate::session::FlowSummaryState::Completed;
        let latest = self.latest();
        let projection = crate::engine::flow_graph::project_position(
            &self.graph,
            latest.map_or(0, |step| step.key),
            latest.map_or(&[], |step| &step.iterations),
            finished,
        );
        crate::durable::FlowDetail {
            entry,
            graph: self.graph.clone(),
            current: projection.current,
            completed: projection.completed,
            returns: projection.returns,
            iterations: latest
                .map(|step| step.iterations.clone())
                .unwrap_or_default(),
            cwd: self.driver.cwd.as_ref().map(std::path::PathBuf::from),
            steps: self
                .steps
                .iter()
                .map(|step| crate::durable::FlowStepExec {
                    exec_id: step.exec.id.clone(),
                    label: self.label(step),
                    key: step.key,
                    iterations: step.iterations.clone(),
                    started_at: step.exec.started_at,
                    completed_at: step.exec.completed_at,
                    outcome: step.exec.outcome.clone(),
                    exit_code: step.exec.exit_code,
                })
                .collect(),
        }
    }
}
