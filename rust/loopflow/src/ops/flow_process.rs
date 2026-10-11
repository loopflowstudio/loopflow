//! A Flow process is one lf Process and the step Processes it starts. It writes
//! a FlowProcess: the Flow's name and graph as compiled at launch, then one row
//! per step it starts. The record is append-only and only the Flow process writes
//! it. Each step is an ordinary command that knows nothing of its Flow; whether
//! the Flow or a step is running, finished or failed is read from their Processes.
use crate::flow::graph::FlowGraph;
use crate::id::LfProcessId;
use crate::process::LfProcess;

/// A step's agent sees which Flow started it: the Flow process's id. Steps of
/// one Flow can share notes under it. It configures nothing in lf.
pub(crate) const FLOW_ID_ENV: &str = "LF_FLOW_ID";

/// One step its Flow process started.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct FlowProcessStep {
    pub process: LfProcess,
    /// The step's node in the Flow's graph, counted in preorder.
    pub key: u32,
    /// Returns taken on each loop edge, per nesting level, outermost first.
    pub iterations: Vec<Vec<u32>>,
}

/// One Flow process as it recorded itself.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct FlowProcess {
    pub process: LfProcess,
    pub name: String,
    pub graph: FlowGraph,
    /// Steps in launch order. A repeated or corrected step is another entry.
    pub steps: Vec<FlowProcessStep>,
}

impl FlowProcess {
    pub(crate) fn id(&self) -> &LfProcessId {
        &self.process.id
    }

    pub(crate) fn latest(&self) -> Option<&FlowProcessStep> {
        self.steps.last()
    }

    /// The graph label of a recorded step.
    pub(crate) fn label(&self, step: &FlowProcessStep) -> String {
        self.graph
            .node_at(step.key)
            .map(|node| node.label.clone())
            .unwrap_or_default()
    }

    /// How far the Flow got on the graph it launched with.
    pub(crate) fn detail(
        &self,
        entry: crate::durable::FlowProcessInventoryEntry,
    ) -> crate::durable::FlowProcessDetail {
        let finished = entry.summary.state == crate::session::FlowProcessSummaryState::Completed;
        let latest = self.latest();
        let projection = crate::flow::graph::project_position(
            &self.graph,
            latest.map_or(0, |step| step.key),
            latest.map_or(&[], |step| &step.iterations),
            finished,
        );
        crate::durable::FlowProcessDetail {
            entry,
            graph: self.graph.clone(),
            current: projection.current,
            completed: projection.completed,
            returns: projection.returns,
            iterations: latest
                .map(|step| step.iterations.clone())
                .unwrap_or_default(),
            cwd: self.process.cwd.as_ref().map(std::path::PathBuf::from),
            steps: self
                .steps
                .iter()
                .map(|step| crate::durable::FlowStepProcess {
                    lf_process_id: step.process.id.clone(),
                    label: self.label(step),
                    key: step.key,
                    iterations: step.iterations.clone(),
                    started_at: step.process.started_at,
                    completed_at: step.process.completed_at,
                    outcome: step.process.outcome.clone(),
                    exit_code: step.process.exit_code,
                })
                .collect(),
        }
    }
}
