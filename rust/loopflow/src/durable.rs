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

/// Where a Task's commands execute: its owner's Machine, or the Machine that
/// may claim it when it is unowned or its owner's Machine is unknown.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TaskExecutionRoute {
    pub task_id: TaskId,
    pub checkout: Option<String>,
    pub repository_id: RepositoryId,
    pub machine_id: MachineId,
    pub source: TaskExecutionSource,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TaskExecutionSource {
    Owner,
    OwningMachine(OwningMachineSource),
}

/// Where your Machine for an unowned Task or a scope's Session came from.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", content = "wave", rename_all = "snake_case")]
pub enum OwningMachineSource {
    /// `lf machine use <machine> <wave>`, set on the Task's Wave or a parent Wave.
    Wave(String),
    /// `lf machine use <machine> --repo`, set on the Task's repository.
    Repository,
    /// No default is set: the Machine running the command.
    ThisMachine,
}

/// Where a person sets a default above a single Task. A Task's own Machine is
/// its ownership claim, not a default. Resolution walks Wave → parent Waves →
/// repository → the setting's fallback.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(tag = "kind", content = "id", rename_all = "snake_case")]
pub enum DefaultScope {
    Wave(crate::id::WaveId),
    Repository(RepositoryId),
}

/// Which part of a Wave or repository a Machine default places.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum MachineSetting {
    /// Where the scope's new Tasks run.
    Tasks,
    /// Where the scope's primary Session runs.
    Session,
}

/// One of your Machine defaults, with the scope's name on this Machine.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct MachineDefault {
    pub scope: DefaultScope,
    /// The Wave's name, or the repository's path on this Machine.
    pub name: String,
    pub setting: MachineSetting,
    pub machine_id: MachineId,
}

prefixed_uuid_id!(
    OwnershipEntryId,
    "own_",
    DurableDataError,
    DurableDataError::InvalidId
);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum OwnershipKind {
    Claim,
    /// Moves the claim to `machine_id`, keeping the person.
    Handoff,
    Unclaim,
    /// What a Machine knew before ownership existed; never delivered.
    Recorded,
}

impl OwnershipKind {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Claim => "claim",
            Self::Handoff => "handoff",
            Self::Unclaim => "unclaim",
            Self::Recorded => "recorded",
        }
    }
}

/// One immutable entry in a Task's ownership log. `machine_id` names the
/// claimant, the handoff target, or the unclaiming Machine.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TaskOwnershipEntry {
    pub id: OwnershipEntryId,
    pub task_id: TaskId,
    pub kind: OwnershipKind,
    pub person: Option<String>,
    /// The person's stable key; names are display only.
    pub person_key: Option<String>,
    pub machine_id: Option<MachineId>,
    /// The Machine's label when written, or as best known on this Machine.
    pub machine_label: Option<String>,
    pub at: i64,
}

/// A Machine for display; a missing id reads as "Machine unknown".
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MachineRef {
    pub id: Option<MachineId>,
    pub label: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TaskOwner {
    pub person: Option<String>,
    pub person_key: Option<String>,
    pub machine: MachineRef,
}

/// A person for display: their name, else their key, else "Someone".
pub fn person_text<'a>(person: Option<&'a str>, person_key: Option<&'a str>) -> &'a str {
    person.or(person_key).unwrap_or("Someone")
}

impl TaskOwner {
    pub fn who(&self) -> &str {
        person_text(self.person.as_deref(), self.person_key.as_deref())
    }
}

impl TaskOwnershipEntry {
    /// The person and Machine this entry names.
    pub fn owner(&self) -> TaskOwner {
        TaskOwner {
            person: self.person.clone(),
            person_key: self.person_key.clone(),
            machine: MachineRef {
                id: self.machine_id.clone(),
                label: self.machine_label.clone(),
            },
        }
    }
}

impl std::fmt::Display for TaskOwner {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let person = self.who();
        match (&self.machine.label, &self.machine.id) {
            (Some(label), _) => write!(f, "{person} on {label}"),
            (None, Some(id)) => write!(f, "{person} on {id}"),
            (None, None) => write!(f, "{person} (Machine unknown)"),
        }
    }
}

/// This Machine's reading of a Task's ownership. `superseded_by` names a later
/// claim that every other Machine follows while this Machine keeps its own.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TaskOwnership {
    pub owner: Option<TaskOwner>,
    pub superseded_by: Option<TaskOwner>,
    pub log: Vec<TaskOwnershipEntry>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Machine {
    pub label: Option<String>,
    pub id: MachineId,
    pub route: String,
    #[serde(with = "time::serde::rfc3339")]
    pub created_at: OffsetDateTime,
    #[serde(with = "time::serde::rfc3339")]
    pub observed_at: OffsetDateTime,
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

/// Query values for Graph discovery; none carries attachment authority.
#[derive(Debug, Clone, Default)]
pub struct GraphProcessFilter {
    pub repo: Option<String>,
    pub task_id: Option<TaskId>,
    pub wave_id: Option<crate::id::WaveId>,
    pub taskless: bool,
    pub state: Option<crate::session::GraphProcessSummaryState>,
    pub search: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GraphProcessInventoryEntry {
    #[serde(flatten)]
    pub summary: crate::session::GraphProcessSummary,
    pub repo: Option<String>,
    pub ended_at: Option<i64>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GraphProcessPage {
    pub entries: Vec<GraphProcessInventoryEntry>,
    pub next: Option<String>,
}

/// One Graph drawn from its Processes: the authored graph while its steps still fit
/// it, else the sequence the steps recorded.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GraphProcessDetail {
    pub entry: GraphProcessInventoryEntry,
    pub compiled: crate::graph::compiled::CompiledGraph,
    pub current: Option<u32>,
    pub completed: Vec<u32>,
    pub returns: Vec<crate::graph::compiled::GraphReturn>,
    /// Per-edge counts at each active nesting level, outermost first.
    pub iterations: Vec<Vec<u32>>,
    pub cwd: Option<std::path::PathBuf>,
    /// Every step the Graph process launched, in order.
    pub steps: Vec<GraphStepProcess>,
}

/// One launched step and how its process ended.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GraphStepProcess {
    pub lf_process_id: crate::id::LfProcessId,
    pub label: String,
    pub key: u32,
    pub iterations: Vec<Vec<u32>>,
    pub started_at: i64,
    pub completed_at: Option<i64>,
    pub outcome: Option<String>,
    pub exit_code: Option<i32>,
}

impl GraphStepProcess {
    /// The step's label with the pass any returned loop is on: "implement · pass 2".
    pub fn position(&self) -> String {
        match crate::graph::compiled::loop_passes(&self.iterations) {
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
