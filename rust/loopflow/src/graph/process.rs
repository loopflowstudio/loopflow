//! Saved Graph execution records.
use crate::graph::compiled::CompiledGraph;
use crate::process::LfProcess;

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
