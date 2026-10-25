//! A Graph process is one lf Process and the step Processes it starts. It writes
//! a GraphProcess: the Graph's name and graph as compiled at launch, then one row
//! per step it starts. The record is append-only and only the Graph process writes
//! it. Each step is an ordinary command that knows nothing of its Graph; whether
//! the Graph or a step is running, finished or failed is read from their Processes.
use crate::graph::compiled::CompiledGraph;
use crate::id::LfProcessId;
use crate::process::LfProcess;

/// A step's agent sees which Graph started it: the Graph process's id. Steps of
/// one Graph can share notes under it. It configures nothing in lf.
pub(crate) const GRAPH_ID_ENV: &str = "LF_GRAPH_ID";

/// The Task's latest interrupt when its Task run began. Each Graph attempt
/// compares later interrupts against it; steps never inherit it.
pub(crate) const TASK_INTERRUPT_CURSOR_ENV: &str = "LF_TASK_INTERRUPT_CURSOR";

/// One step its Graph process started.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct GraphProcessStep {
    pub process: LfProcess,
    /// The step's node in the compiled Graph, counted in preorder.
    pub key: u32,
    /// Returns taken on each loop edge, per nesting level, outermost first.
    pub iterations: Vec<Vec<u32>>,
}

/// One Graph process as it recorded itself.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct GraphProcess {
    pub process: LfProcess,
    pub name: String,
    pub compiled: CompiledGraph,
    /// Steps in launch order. A repeated or corrected step is another entry.
    pub steps: Vec<GraphProcessStep>,
}

impl GraphProcess {
    pub(crate) fn id(&self) -> &LfProcessId {
        &self.process.id
    }

    pub(crate) fn latest(&self) -> Option<&GraphProcessStep> {
        self.steps.last()
    }

    /// The graph label of a recorded step.
    pub(crate) fn label(&self, step: &GraphProcessStep) -> String {
        self.compiled
            .node_at(step.key)
            .map(|node| node.label.clone())
            .unwrap_or_default()
    }

    /// How far the Graph got on the graph it launched with.
    pub(crate) fn detail(
        &self,
        entry: crate::durable::GraphProcessInventoryEntry,
    ) -> crate::durable::GraphProcessDetail {
        let finished = entry.summary.state == crate::session::GraphProcessSummaryState::Completed;
        let latest = self.latest();
        let projection = crate::graph::compiled::project_position(
            &self.compiled,
            latest.map_or(0, |step| step.key),
            latest.map_or(&[], |step| &step.iterations),
            finished,
        );
        crate::durable::GraphProcessDetail {
            entry,
            compiled: self.compiled.clone(),
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
                .map(|step| crate::durable::GraphStepProcess {
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
