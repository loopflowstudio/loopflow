//! Saved Graph execution records.
use crate::graph::CompiledGraph;
use loopflow_model::id::LfProcessId;

/// One lf run, as the run and its spawner recorded it: a Graph run or one of
/// its steps. `pid` and `os_started_at` name the OS process while it runs
/// and are cleared at its end.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Execution {
    pub id: LfProcessId,
    pub started_at: i64,
    pub ended_at: Option<i64>,
    pub pid: Option<u32>,
    pub os_started_at: Option<i64>,
    pub exit_code: Option<i32>,
    pub error: Option<String>,
}

/// The record a run is written on. Each run has one, and only that table
/// records its start and end.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum RunOwner {
    /// `graph_processes`: a Graph run.
    GraphRun,
    /// `graph_process_steps`: one step a Graph run spawned.
    GraphStep,
    /// `waves`: a Wave's Project activation.
    ProjectActivation,
    /// `task_graphs`: the lf carrying a Task's run. It records no result of
    /// its own; its Graph run's is the Task's.
    TaskCarrier,
}

/// How a run ended, derived from its end, exit code and error.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExecutionResult {
    Running,
    Succeeded,
    /// Exit 130 with no error: someone interrupted it.
    Interrupted,
    Failed,
    /// It ended without lf seeing how: killed, or its exit was never written.
    Unknown,
}

impl ExecutionResult {
    pub fn of(ended_at: Option<i64>, exit_code: Option<i32>, error: Option<&str>) -> Self {
        match (ended_at, exit_code, error) {
            (None, _, _) => Self::Running,
            (Some(_), Some(0), _) => Self::Succeeded,
            (Some(_), Some(130), None) => Self::Interrupted,
            (Some(_), Some(_), _) => Self::Failed,
            (Some(_), None, _) => Self::Unknown,
        }
    }

    /// The wire spelling; a running run has none.
    pub fn outcome(self) -> Option<&'static str> {
        match self {
            Self::Running => None,
            Self::Succeeded => Some("succeeded"),
            Self::Interrupted => Some("interrupted"),
            Self::Failed => Some("failed"),
            Self::Unknown => Some("unknown"),
        }
    }
}

/// What a run's spawner, or the run itself, knows as it starts.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExecutionStart {
    pub started_at: i64,
    pub pid: Option<u32>,
    pub os_started_at: Option<i64>,
    /// The Session whose agent started this run, when one did. Owners that
    /// show who started them (Graph runs, auto) record it.
    pub caller_session: Option<loopflow_model::id::LfSessionId>,
}

impl ExecutionStart {
    /// A run its spawner records before it has an OS process.
    pub fn reserved(started_at: i64) -> Self {
        Self {
            started_at,
            pid: None,
            os_started_at: None,
            caller_session: None,
        }
    }
}

impl Execution {
    pub fn result(&self) -> ExecutionResult {
        ExecutionResult::of(self.ended_at, self.exit_code, self.error.as_deref())
    }
}

/// The lf carrying a Task's run. `pid` and `os_started_at` name its OS
/// process while it runs and are cleared at its end; the id stays, naming the
/// run the next `lf task run` sets out from.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TaskCarrier {
    pub id: LfProcessId,
    pub pid: Option<u32>,
    pub os_started_at: Option<i64>,
    /// When retention found its OS process gone, having never exited itself.
    pub died_at: Option<i64>,
}

/// One step its Graph run started.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GraphStep {
    pub run: Execution,
    /// The step's node in the compiled Graph, counted in preorder.
    pub key: u32,
    /// Returns taken on each loop edge, per nesting level, outermost first.
    pub iterations: Vec<Vec<u32>>,
}

/// One Graph run as it recorded itself.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GraphExecution {
    pub task_id: Option<loopflow_model::durable::TaskId>,
    pub run: Execution,
    /// The repository and directory the Graph ran in.
    pub repo: Option<String>,
    pub cwd: Option<String>,
    /// The Session whose agent started it, when one did.
    pub caller_session: Option<loopflow_model::id::LfSessionId>,
    pub name: String,
    pub compiled: CompiledGraph,
    /// Steps in launch order. A repeated or corrected step is another entry.
    pub steps: Vec<GraphStep>,
}

impl GraphExecution {
    pub fn id(&self) -> &LfProcessId {
        &self.run.id
    }

    pub fn latest(&self) -> Option<&GraphStep> {
        self.steps.last()
    }

    /// The graph label of a recorded step.
    pub fn label(&self, step: &GraphStep) -> String {
        self.compiled
            .node_at(step.key)
            .map(|node| node.label.clone())
            .unwrap_or_default()
    }

    pub fn step_detail(&self, step: &GraphStep) -> crate::execution::GraphStepProcess {
        crate::execution::GraphStepProcess {
            lf_process_id: step.run.id.clone(),
            label: self.label(step),
            key: step.key,
            iterations: step.iterations.clone(),
            started_at: step.run.started_at,
            completed_at: step.run.ended_at,
            outcome: step.run.result().outcome().map(str::to_owned),
            exit_code: step.run.exit_code,
            error: step.run.error.clone(),
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
            cwd: self.cwd.as_ref().map(std::path::PathBuf::from),
            steps: self
                .steps
                .iter()
                .map(|step| self.step_detail(step))
                .collect(),
        }
    }
}
