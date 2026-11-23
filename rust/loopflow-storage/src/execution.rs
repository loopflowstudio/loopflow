//! Execution views and resolved placement. Shared Work records live in loopflow-model.

use serde::{Deserialize, Serialize};
use time::OffsetDateTime;

pub use loopflow_model::durable::{
    person_text, Author, CapacityDefault, DefaultScope, DurableDataError, Machine, MachineDefault,
    MachineId, MachineRef, MachineSetting, OwnershipEntryId, OwnershipKind, ProjectId,
    RepositoryId, RepositoryIdentity, TaskId, TaskOwner, TaskOwnership, TaskOwnershipEntry,
    WorkRef, WorkStatus,
};

/// A fresh, Machine-local observation, never replicated as planning or a start claim.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TaskLocationObservation {
    pub request: String,
    pub repository_id: RepositoryId,
    pub task_id: TaskId,
    pub machine_id: MachineId,
    pub observed_at: i64,
    pub location: TaskLocation,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "state", rename_all = "snake_case")]
#[non_exhaustive]
pub enum TaskLocation {
    Recorded {
        task_id: TaskId,
        checkout: Option<String>,
    },
    /// Only this Machine has no retained execution; this is not global absence.
    Unrecorded,
    Unavailable {
        reason: String,
    },
}

impl TaskLocation {
    pub fn description(&self) -> String {
        match self {
            Self::Recorded { task_id, checkout } => format!(
                "recorded Task {task_id}: {}",
                checkout.as_deref().unwrap_or("checkout unavailable")
            ),
            Self::Unrecorded => "no local execution record".into(),
            Self::Unavailable { reason } => format!("unavailable: {reason}"),
        }
    }
}

/// A Task's state, derived from its checkout, Graph position, whether a
/// Graph runs for it. Completion and abandonment
/// are durable; the open states follow what the Task is doing now. Whether
/// anyone owns it is ownership, not state.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum TaskState {
    /// No checkout yet.
    Unstarted,
    /// A checkout, no Graph running and not at an interactive step: before its first run,
    /// or after a stopped one.
    Idle,
    /// Between Graphs at an interactive step, in a conversation in its Task Session.
    Active,
    /// A Graph runs for it. Its auto setting says whether the run stands in
    /// at interactive steps or stops there for a person.
    Moving,
    /// Execution is held until explicitly unpaused.
    Paused,
    /// Completed independently of Graph position.
    Done,
    Abandoned,
}

impl TaskState {
    pub fn label(&self) -> &'static str {
        match self {
            Self::Unstarted => "unstarted",
            Self::Idle => "idle",
            Self::Active => "active",
            Self::Moving => "moving",
            Self::Paused => "paused",
            Self::Done => "done",
            Self::Abandoned => "abandoned",
        }
    }

    pub fn parse(label: &str) -> Option<Self> {
        [
            Self::Unstarted,
            Self::Idle,
            Self::Active,
            Self::Moving,
            Self::Paused,
            Self::Done,
            Self::Abandoned,
        ]
        .into_iter()
        .find(|state| state.label() == label)
    }

    pub fn is_terminal(&self) -> bool {
        matches!(self, Self::Done | Self::Abandoned)
    }

    /// Has a checkout and is still open and unheld.
    pub fn is_started(&self) -> bool {
        matches!(self, Self::Idle | Self::Active | Self::Moving)
    }

    /// Planning withdraws never-started work; started work follows execution.
    pub fn lifecycle(self, plan_state: Option<&str>) -> Self {
        match (self, plan_state) {
            (Self::Unstarted, Some("completed")) => Self::Done,
            (Self::Unstarted, Some("canceled" | "duplicate")) => Self::Abandoned,
            _ => self,
        }
    }

    /// The Task as Work: open until it is done or abandoned.
    pub fn work_status(&self) -> WorkStatus {
        match self {
            Self::Done => WorkStatus::Done,
            Self::Abandoned => WorkStatus::Abandoned,
            _ => WorkStatus::Ready,
        }
    }
}

/// Where a Task sits on the Wave board. The one bucketing rule: the board, the
/// sidebar's Working Tasks, Wave counts and `lf wave show` all read it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum TaskSection {
    /// Waiting for someone to pick it up: unstarted, or started and unassigned.
    Backlog,
    /// Someone's idle, active or moving Task.
    Working,
    Paused,
    Done,
    Abandoned,
}

impl TaskSection {
    /// `owned`: someone holds the Task's claim. `plan_state` is planning's state
    /// type; closing a Task there withdraws it only if it never started.
    pub fn of(state: TaskState, owned: bool, plan_state: Option<&str>) -> Self {
        match state.lifecycle(plan_state) {
            TaskState::Done => Self::Done,
            TaskState::Abandoned => Self::Abandoned,
            TaskState::Unstarted => Self::Backlog,
            TaskState::Paused => Self::Paused,
            _ if !owned => Self::Backlog,
            TaskState::Idle | TaskState::Active | TaskState::Moving => Self::Working,
        }
    }
}

impl std::fmt::Display for TaskState {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(&self.label().replace('_', " "))
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AbandonReceipt {
    pub work: WorkRef,
    pub reason: String,
    #[serde(with = "time::serde::rfc3339")]
    pub abandoned_at: OffsetDateTime,
}

/// Query values for Graph discovery; none carries attachment authority.
#[derive(Debug, Clone, Default)]
pub struct GraphProcessFilter {
    pub repo: Option<String>,
    pub task_id: Option<TaskId>,
    pub wave_id: Option<loopflow_model::id::WaveId>,
    pub taskless: bool,
    pub state: Option<crate::session::GraphProcessSummaryState>,
    pub search: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
pub struct GraphProcessInventoryEntry {
    #[serde(flatten)]
    pub summary: crate::session::GraphProcessSummary,
    pub repo: Option<String>,
    pub ended_at: Option<i64>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
pub struct GraphProcessPage {
    pub entries: Vec<GraphProcessInventoryEntry>,
    pub next: Option<String>,
}

/// One Graph drawn from its Processes: the authored graph while its steps still fit
/// it, else the sequence the steps recorded.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
pub struct GraphProcessDetail {
    pub entry: GraphProcessInventoryEntry,
    pub compiled: crate::graph::CompiledGraph,
    pub current: Option<u32>,
    pub completed: Vec<u32>,
    pub returns: Vec<crate::graph::GraphReturn>,
    /// Per-edge counts at each active nesting level, outermost first.
    pub iterations: Vec<Vec<u32>>,
    pub cwd: Option<std::path::PathBuf>,
    /// Every step the Graph process launched, in order.
    pub steps: Vec<GraphStepProcess>,
}

/// One launched step and how its process ended.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
pub struct GraphStepProcess {
    pub lf_process_id: loopflow_model::id::LfProcessId,
    pub label: String,
    pub key: u32,
    pub iterations: Vec<Vec<u32>>,
    pub started_at: i64,
    pub completed_at: Option<i64>,
    pub outcome: Option<String>,
    pub exit_code: Option<i32>,
    /// Why the step failed, as the step recorded it.
    pub error: Option<String>,
}

impl GraphStepProcess {
    /// The step's label with the pass any returned loop is on: "implement · pass 2".
    pub fn position(&self) -> String {
        match crate::graph::loop_passes(&self.iterations) {
            Some(passes) => format!("{} · {passes}", self.label),
            None => self.label.clone(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{TaskSection, TaskState};

    #[test]
    fn lifecycle_withdraws_only_never_started_work() {
        for (planning, terminal) in [
            ("completed", TaskState::Done),
            ("canceled", TaskState::Abandoned),
            ("duplicate", TaskState::Abandoned),
        ] {
            assert_eq!(TaskState::Unstarted.lifecycle(Some(planning)), terminal);
            for runtime in [TaskState::Idle, TaskState::Active, TaskState::Moving] {
                assert_eq!(runtime.lifecycle(Some(planning)), runtime);
                assert_eq!(
                    TaskSection::of(runtime, false, Some(planning)),
                    TaskSection::Backlog
                );
            }
        }
    }

    #[test]
    fn task_sections_put_unassigned_and_withdrawn_tasks_where_the_board_does() {
        let of = TaskSection::of;
        assert_eq!(of(TaskState::Idle, true, None), TaskSection::Working);
        assert_eq!(
            of(TaskState::Moving, true, Some("completed")),
            TaskSection::Working
        );
        assert_eq!(of(TaskState::Idle, false, None), TaskSection::Backlog);
        assert_eq!(of(TaskState::Moving, false, None), TaskSection::Backlog);
        assert_eq!(of(TaskState::Unstarted, true, None), TaskSection::Backlog);
        assert_eq!(
            of(TaskState::Unstarted, false, Some("completed")),
            TaskSection::Done
        );
        assert_eq!(
            of(TaskState::Unstarted, false, Some("canceled")),
            TaskSection::Abandoned
        );
        assert_eq!(of(TaskState::Abandoned, true, None), TaskSection::Abandoned);
    }
}
