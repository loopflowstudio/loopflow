//! Durable input and execution identity shared by Wave, Project, and Task Work.

use serde::{Deserialize, Serialize};
use time::OffsetDateTime;

use crate::id::WaveId;
use crate::steer::Steer;

use crate::id::prefixed_uuid_id;

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum DurableDataError {
    #[error("invalid durable id: {0}")]
    InvalidId(String),
}

// Identity of one selected repository plan, never derived from a code remote.
prefixed_uuid_id!(
    RepositoryId,
    "repo_",
    DurableDataError,
    DurableDataError::InvalidId
);

/// One selected plan and its retained Machine-local locators, read together.
/// Locators prove explicit association, not portable Work or execution authority.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RepositoryIdentity {
    pub id: RepositoryId,
    pub locators: Vec<RepositoryId>,
}
prefixed_uuid_id!(
    ProjectId,
    "proj_",
    DurableDataError,
    DurableDataError::InvalidId
);
prefixed_uuid_id!(
    TaskId,
    "task_",
    DurableDataError,
    DurableDataError::InvalidId
);
// Machine identity retains its released home_ prefix; job labels are separate locators.
prefixed_uuid_id!(
    MachineId,
    "home_",
    DurableDataError,
    DurableDataError::InvalidId
);
prefixed_uuid_id!(
    ToolResponseId,
    "response_",
    DurableDataError,
    DurableDataError::InvalidId
);
prefixed_uuid_id!(
    CronReceiptId,
    "cron_",
    DurableDataError,
    DurableDataError::InvalidId
);

impl TaskId {
    /// Linear UUIDs are also the creation UUID used by locally authored Tasks.
    pub(crate) fn from_issue(id: &str) -> Self {
        let uuid = uuid::Uuid::parse_str(id).unwrap_or_else(|_| {
            uuid::Uuid::new_v5(
                &uuid::Uuid::NAMESPACE_URL,
                format!("https://linear.app/issue/{id}").as_bytes(),
            )
        });
        Self(format!("task_{}", uuid.simple()))
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(tag = "kind", content = "id", rename_all = "snake_case")]
pub enum WorkRef {
    Wave(WaveId),
    Project(ProjectId),
    Task(TaskId),
}

impl WorkRef {
    pub fn kind(&self) -> &'static str {
        match self {
            Self::Wave(_) => "wave",
            Self::Project(_) => "project",
            Self::Task(_) => "task",
        }
    }

    pub fn id(&self) -> &str {
        match self {
            Self::Wave(id) => id.as_str(),
            Self::Project(id) => id.as_str(),
            Self::Task(id) => id.as_str(),
        }
    }
}

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

/// A launch destination read from planning and recorded execution, not a claim.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TaskExecutionRoute {
    pub task_id: TaskId,
    pub checkout: Option<String>,
    pub repository_id: RepositoryId,
    pub machine_id: MachineId,
    pub source: TaskExecutionSource,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TaskExecutionSource {
    RecordedCheckout,
    Admission,
    EffectiveDelegation,
}

/// Immutable first-start coordinator, carried with the Task's planning identity.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TaskAdmissionAuthority {
    pub machine_id: MachineId,
    pub repository_id: RepositoryId,
}

/// A request-bound reading from the coordinator. A positive owner never expires.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TaskAdmission {
    pub request: String,
    pub task_id: TaskId,
    pub authority: TaskAdmissionAuthority,
    pub owner: Option<MachineId>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Machine {
    pub label: Option<String>,
    pub repo: Option<String>,
    pub id: MachineId,
    pub route: String,
    #[serde(with = "time::serde::rfc3339")]
    pub created_at: OffsetDateTime,
    #[serde(with = "time::serde::rfc3339")]
    pub observed_at: OffsetDateTime,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Placement {
    pub work: WorkRef,
    /// The ancestor (or Work itself) supplying this assignment.
    pub source: WorkRef,
    pub provenance: PlacementProvenance,
    pub machine_id: MachineId,
    #[serde(with = "time::serde::rfc3339")]
    pub placed_at: OffsetDateTime,
}

/// How the source assignment was established; inheritance is `source != work`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PlacementProvenance {
    Explicit,
    /// Released rows cannot distinguish an explicit choice from a copied parent.
    Legacy,
    /// No authored assignment exists in the ancestry.
    LocalDefault,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", content = "id", rename_all = "snake_case")]
pub enum Author {
    User,
    Named(String),
    Captured(i64),
    /// Historical selector preserved from pre-event attribution.
    #[serde(alias = "run")]
    Imported(String),
}

impl std::fmt::Display for Author {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::User => f.write_str("attribution unresolved"),
            Self::Named(name) => f.write_str(name),
            Self::Captured(id) => write!(f, "session input {id}"),
            Self::Imported(selector) => write!(f, "imported {selector}"),
        }
    }
}

/// A steer comment surfaced for the cross-Work `lf history` timeline: the read
/// model plus the Work it targets and when it was issued.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SteerComment {
    pub work: WorkRef,
    pub steer: Steer,
    pub issued_at: OffsetDateTime,
}

pub fn render_steers(steers: &[Steer]) -> String {
    if steers.is_empty() {
        return String::new();
    }
    let direction = steers
        .iter()
        .map(|steer| format!("- {}", steer.text))
        .collect::<Vec<_>>()
        .join("\n");
    format!("<lf:steers>\n{direction}\n</lf:steers>")
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ToolResponseWrite {
    pub request_id: String,
    pub choice: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ToolResponseReceipt {
    pub id: ToolResponseId,
    pub work: WorkRef,
    pub request_id: String,
    pub choice: String,
    #[serde(with = "time::serde::rfc3339")]
    pub responded_at: OffsetDateTime,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum WorkStatus {
    Ready,
    Done,
    Abandoned,
}

impl WorkStatus {
    pub(crate) fn label(&self) -> &'static str {
        match self {
            Self::Ready => "ready",
            Self::Done => "done",
            Self::Abandoned => "abandoned",
        }
    }

    pub(crate) fn reason(&self) -> &'static str {
        self.label()
    }
}

impl std::fmt::Display for WorkStatus {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(self.label())
    }
}

/// Durable completion and abandonment, with Workflow readiness for unfinished Tasks.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TaskState {
    /// Explicit hold; history and Workflow are retained.
    Paused,
    /// The Task has taken up no Workflow.
    NotReady,
    /// At `start`.
    Ready,
    /// Past start, including end with completion pending.
    Active,
    /// Completed independently of Workflow position.
    Done,
    Abandoned,
}

impl TaskState {
    pub(crate) fn label(&self) -> &'static str {
        match self {
            Self::Paused => "paused",
            Self::NotReady => "not_ready",
            Self::Ready => "ready",
            Self::Active => "active",
            Self::Done => "done",
            Self::Abandoned => "abandoned",
        }
    }

    pub(crate) fn parse(label: &str) -> Option<Self> {
        [
            Self::Paused,
            Self::NotReady,
            Self::Ready,
            Self::Active,
            Self::Done,
            Self::Abandoned,
        ]
        .into_iter()
        .find(|state| state.label() == label)
    }

    pub fn is_terminal(&self) -> bool {
        matches!(self, Self::Done | Self::Abandoned)
    }

    /// The Task as Work: open until it is done or abandoned.
    pub(crate) fn work_status(&self) -> WorkStatus {
        match self {
            Self::Paused | Self::NotReady | Self::Ready | Self::Active => WorkStatus::Ready,
            Self::Done => WorkStatus::Done,
            Self::Abandoned => WorkStatus::Abandoned,
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

/// Query values for Flow discovery; none carries attachment authority.
#[derive(Debug, Clone, Default)]
pub struct FlowProcessFilter {
    pub repo: Option<String>,
    pub task_id: Option<TaskId>,
    pub wave_id: Option<crate::id::WaveId>,
    pub taskless: bool,
    pub state: Option<crate::session::FlowProcessSummaryState>,
    pub search: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FlowProcessInventoryEntry {
    #[serde(flatten)]
    pub summary: crate::session::FlowProcessSummary,
    pub repo: Option<String>,
    pub ended_at: Option<i64>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FlowProcessPage {
    pub entries: Vec<FlowProcessInventoryEntry>,
    pub next: Option<String>,
}

/// One Flow drawn from its Processes: the authored graph while its steps still fit
/// it, else the sequence the steps recorded.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FlowProcessDetail {
    pub entry: FlowProcessInventoryEntry,
    pub graph: crate::flow::graph::FlowGraph,
    pub current: Option<u32>,
    pub completed: Vec<u32>,
    pub returns: Vec<crate::flow::graph::FlowReturn>,
    /// Per-edge counts at each active nesting level, outermost first.
    pub iterations: Vec<Vec<u32>>,
    pub cwd: Option<std::path::PathBuf>,
    /// Every step the Flow process launched, in order.
    pub steps: Vec<FlowStepProcess>,
}

/// One launched step and how its process ended.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FlowStepProcess {
    pub lf_process_id: crate::id::LfProcessId,
    pub label: String,
    pub key: u32,
    pub iterations: Vec<Vec<u32>>,
    pub started_at: i64,
    pub completed_at: Option<i64>,
    pub outcome: Option<String>,
    pub exit_code: Option<i32>,
}

impl FlowStepProcess {
    /// The step's label with the pass any returned loop is on: "implement · pass 2".
    pub fn position(&self) -> String {
        match crate::flow::graph::loop_passes(&self.iterations) {
            Some(passes) => format!("{} · {passes}", self.label),
            None => self.label.clone(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{render_steers, WorkStatus};
    use crate::durable::Author;
    use crate::steer::Steer;

    #[test]
    fn ordered_steers_render_as_one_input_projection() {
        let steer = |sequence: i64, text: &str| Steer {
            id: crate::id::SteerId::new(),
            sequence,
            source: format!("fixture:{sequence}"),
            revision: "1".into(),
            author: Author::User,
            text: text.to_string(),
        };
        let steers = vec![steer(1, "first"), steer(2, "second")];

        assert_eq!(
            render_steers(&steers),
            "<lf:steers>\n- first\n- second\n</lf:steers>"
        );
    }

    #[test]
    fn work_status_fixture_round_trips_every_current_variant() {
        let fixture = include_str!("../../../tests/fixtures/dto/work_statuses.json");
        let statuses: Vec<WorkStatus> = serde_json::from_str(fixture).unwrap();

        assert!(matches!(statuses[0], WorkStatus::Ready));
        assert!(matches!(statuses[1], WorkStatus::Done));
        assert!(matches!(statuses[2], WorkStatus::Abandoned));

        let encoded = serde_json::to_string(&statuses).unwrap();
        let decoded: Vec<WorkStatus> = serde_json::from_str(&encoded).unwrap();
        assert_eq!(decoded, statuses);
    }
}
