//! Saved Graph execution records.
use crate::graph::CompiledGraph;
use crate::process::LfProcess;
use loopflow_model::id::LfProcessId;

/// One step its Graph process started.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GraphProcessStep {
    pub process: LfProcess,
    /// The step's node in the compiled Graph, counted in preorder.
    pub key: u32,
    /// Returns taken on each loop edge, per nesting level, outermost first.
    pub iterations: Vec<Vec<u32>>,
}

/// One Graph process as it recorded itself.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GraphProcess {
    pub process: LfProcess,
    pub name: String,
    pub compiled: CompiledGraph,
    /// Steps in launch order. A repeated or corrected step is another entry.
    pub steps: Vec<GraphProcessStep>,
}

impl GraphProcess {
    pub fn id(&self) -> &LfProcessId {
        &self.process.id
    }

    pub fn latest(&self) -> Option<&GraphProcessStep> {
        self.steps.last()
    }

    /// The graph label of a recorded step.
    pub fn label(&self, step: &GraphProcessStep) -> String {
        self.compiled
            .node_at(step.key)
            .map(|node| node.label.clone())
            .unwrap_or_default()
    }

    pub fn step_detail(&self, step: &GraphProcessStep) -> crate::execution::GraphStepProcess {
        crate::execution::GraphStepProcess {
            lf_process_id: step.process.id.clone(),
            label: self.label(step),
            key: step.key,
            iterations: step.iterations.clone(),
            started_at: step.process.started_at,
            completed_at: step.process.completed_at,
            outcome: step.process.outcome.clone(),
            exit_code: step.process.exit_code,
        }
    }

    /// How far the Graph got on the graph it launched with.
    pub fn detail(
        &self,
        entry: crate::execution::GraphProcessInventoryEntry,
    ) -> crate::execution::GraphProcessDetail {
        let finished = entry.summary.state == crate::session::GraphProcessSummaryState::Completed;
        let latest = self.latest();
        let projection = crate::graph::project_position(
            &self.compiled,
            latest.map_or(0, |step| step.key),
            latest.map_or(&[], |step| &step.iterations),
            finished,
        );
        crate::execution::GraphProcessDetail {
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
                .map(|step| self.step_detail(step))
                .collect(),
        }
    }
}
