//! Durable input and execution identity shared by Wave, Project, and Task Work.

use serde::{Deserialize, Serialize};
use time::OffsetDateTime;

use crate::id::WaveId;

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

impl ProjectId {
    /// A Wave's one Project shares the Wave's UUID, so every Machine that
    /// makes it makes the same record. A historical Wave ID that is no UUID
    /// names one through UUIDv5.
    pub fn for_wave(wave: &WaveId) -> Self {
        let uuid = uuid::Uuid::parse_str(wave.as_str()).unwrap_or_else(|_| {
            uuid::Uuid::new_v5(&uuid::Uuid::NAMESPACE_OID, wave.as_str().as_bytes())
        });
        Self(format!("proj_{}", uuid.simple()))
    }

    /// Whether `external` is the tracker id this Project's own `create` made:
    /// Loopflow sends the Project's UUID with it.
    pub fn made_as(&self, external: &str) -> bool {
        let own = self
            .0
            .strip_prefix("proj_")
            .and_then(|id| uuid::Uuid::parse_str(id).ok());
        own.is_some() && own == uuid::Uuid::parse_str(external).ok()
    }
}

impl TaskId {
    /// Linear UUIDs are also the creation UUID used by locally authored Tasks.
    pub fn from_issue(id: &str) -> Self {
        let uuid = uuid::Uuid::parse_str(id).unwrap_or_else(|_| {
            uuid::Uuid::new_v5(
                &uuid::Uuid::NAMESPACE_URL,
                format!("https://linear.app/issue/{id}").as_bytes(),
            )
        });
        Self(format!("task_{}", uuid.simple()))
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize, schemars::JsonSchema)]
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

/// Where a person sets a default above a single Task. A Task's own Machine is
/// its ownership claim, not a default. Resolution walks Wave → parent Waves →
/// repository → the setting's fallback.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, schemars::JsonSchema)]
#[serde(tag = "kind", content = "id", rename_all = "snake_case")]
pub enum DefaultScope {
    Wave(crate::id::WaveId),
    Repository(RepositoryId),
}

/// Which part of a Wave or repository a Machine default places.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, schemars::JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum MachineSetting {
    /// Where the scope's new Tasks run.
    Tasks,
    /// Where the scope's primary Session runs.
    Session,
}

/// One of your Machine defaults, with the scope's name on this Machine.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, schemars::JsonSchema)]
pub struct MachineDefault {
    pub scope: DefaultScope,
    /// The Wave's name, or the repository's path on this Machine.
    pub name: String,
    pub setting: MachineSetting,
    pub machine_id: MachineId,
}

/// A scope whose new Tasks take a machine from a pool or an on-demand.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct CapacityDefault {
    pub scope: DefaultScope,
    /// The Wave's name, or the repository's path on this Machine.
    pub name: String,
    /// The pool or on-demand.
    pub capacity: String,
}

prefixed_uuid_id!(
    OwnershipEntryId,
    "own_",
    DurableDataError,
    DurableDataError::InvalidId
);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum OwnershipKind {
    /// Puts the Task on `machine_id`. A move is a claim by the same person
    /// naming another Machine; entries an earlier lf wrote as `handoff`, and
    /// the never-delivered `recorded` entries, read as claims.
    #[serde(alias = "handoff", alias = "recorded")]
    Claim,
    /// Stored and sent as `unclaim`, its released spelling.
    #[serde(rename = "unclaim")]
    Unassign,
}

impl OwnershipKind {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Claim => "claim",
            Self::Unassign => "unclaim",
        }
    }

    /// The command that wrote it, as history shows it.
    pub fn label(self) -> &'static str {
        match self {
            Self::Unassign => "unassign",
            kind => kind.as_str(),
        }
    }
}

/// One immutable entry in a Task's ownership log. `machine_id` names the
/// Machine the Task is on after a claim, or the unclaiming Machine;
/// `writer_machine_id` the Machine that wrote it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
pub struct TaskOwnershipEntry {
    pub id: OwnershipEntryId,
    pub task_id: TaskId,
    pub kind: OwnershipKind,
    pub person: Option<String>,
    /// The person's stable key; names are display only.
    pub person_key: Option<String>,
    /// The person's `people` id, their tracker's identity for them.
    /// Absent from entries released lf wrote and where the writer knew none.
    pub person_id: Option<String>,
    pub machine_id: Option<MachineId>,
    /// The Machine's label when written, or as best known on this Machine.
    pub machine_label: Option<String>,
    /// Absent from entries lf before `task_machine_local` wrote, which name no writer.
    pub writer_machine_id: Option<MachineId>,
    pub at: i64,
    /// Where the entry sorts in the Task's log on every Machine, from its
    /// writer's plan clock. Absent with Linear or Asana, whose comment order
    /// decides, and from entries written before claims were first-wins.
    pub order: Option<i64>,
    /// The entry in effect when this one was written, or this entry itself
    /// when there was none. Present exactly when `order` is. A claim whose
    /// writer had not seen the claim holding the Task loses to it.
    pub seen: Option<OwnershipEntryId>,
}

/// A Machine for display; a missing id reads as "Machine unknown".
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
pub struct MachineRef {
    pub id: Option<MachineId>,
    pub label: Option<String>,
}

impl MachineRef {
    /// Its label, else its id.
    pub fn name(&self) -> String {
        match (&self.label, &self.id) {
            (Some(label), _) => label.clone(),
            (None, Some(id)) => id.to_string(),
            (None, None) => "Machine unknown".to_string(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
pub struct TaskOwner {
    pub person: Option<String>,
    pub person_key: Option<String>,
    /// The person's `people` id, where the entry named one.
    #[serde(deserialize_with = "Option::deserialize")]
    pub person_id: Option<String>,
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
            person_id: self.person_id.clone(),
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
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
pub struct TaskOwnership {
    pub owner: Option<TaskOwner>,
    pub superseded_by: Option<TaskOwner>,
    pub log: Vec<TaskOwnershipEntry>,
}

impl TaskOwnership {
    /// The Machine that owns the Task when it is not `local`.
    pub fn owner_elsewhere(&self, local: &MachineId) -> Option<&MachineRef> {
        let machine = &self.owner.as_ref()?.machine;
        machine
            .id
            .as_ref()
            .is_some_and(|id| id != local)
            .then_some(machine)
    }

    /// When the latest entry naming `machine` was written.
    pub fn since(&self, machine: &MachineId) -> Option<i64> {
        self.log
            .iter()
            .rev()
            .find(|entry| entry.machine_id.as_ref() == Some(machine))
            .map(|entry| entry.at)
    }

    /// Where the Task runs when another Machine than `local` owns it.
    pub fn runs_on(&self, local: &MachineId) -> Option<TaskRunsOn> {
        let machine = self.owner_elsewhere(local)?;
        Some(TaskRunsOn {
            since: self.since(machine.id.as_ref()?)?,
            machine: machine.clone(),
        })
    }
}

/// The Machine that runs a Task another Machine owns, and since when: the
/// latest ownership entry naming it. Wire type; no defaults.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
pub struct TaskRunsOn {
    /// Named as this Machine's preferences name it, else as the claim
    /// published it, else by its id.
    pub machine: MachineRef,
    /// Unix seconds.
    pub since: i64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
pub struct Machine {
    pub label: Option<String>,
    pub id: MachineId,
    pub route: String,
    #[schemars(with = "String")]
    #[serde(with = "time::serde::rfc3339")]
    pub created_at: OffsetDateTime,
    #[schemars(with = "String")]
    #[serde(with = "time::serde::rfc3339")]
    pub observed_at: OffsetDateTime,
}

impl Machine {
    /// Its label, else its id.
    pub fn name(&self) -> String {
        self.label.clone().unwrap_or_else(|| self.id.to_string())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
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

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum WorkStatus {
    Ready,
    Done,
    Abandoned,
}

impl WorkStatus {
    pub fn label(&self) -> &'static str {
        match self {
            Self::Ready => "ready",
            Self::Done => "done",
            Self::Abandoned => "abandoned",
        }
    }

    pub fn reason(&self) -> &'static str {
        self.label()
    }
}

impl std::fmt::Display for WorkStatus {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(self.label())
    }
}

#[cfg(test)]
mod tests {
    use super::{ProjectId, WaveId, WorkStatus};

    #[test]
    fn a_wave_project_is_made_as_its_wave_uuid_in_either_spelling() {
        let wave = WaveId::parse("6f9619ff-8b86-d011-b42d-00c04fc964ff").unwrap();
        let project = ProjectId::for_wave(&wave);
        assert!(project.made_as("6F9619FF-8B86-D011-B42D-00C04FC964FF"));
        assert!(project.made_as("6f9619ff8b86d011b42d00c04fc964ff"));
        assert!(!project.made_as("00000000-0000-0000-0000-000000000001"));
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
