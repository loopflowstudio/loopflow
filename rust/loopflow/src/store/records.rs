//! Saved acquisition and ownership facts shared by storage commands.

use crate::store::work::wave::WaveLocator;
use loopflow_model::id::WaveId;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProcessIdentityEvidence {
    Live,
    Dead,
    Unknown,
}

/// Availability of retained planning facts, independent of execution permission.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize, schemars::JsonSchema,
)]
#[serde(rename_all = "snake_case")]
#[non_exhaustive]
pub enum PlanningState {
    Available,
    Invalid,
    Removed,
    Absent,
    Unavailable,
}

/// Acquisition evidence is independent of saved Work and outbound delivery.
#[derive(
    Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize, schemars::JsonSchema,
)]
pub struct TaskAcquisition {
    pub state: PlanningState,
    pub observed_at: Option<i64>,
}

/// Retained Linear acquisition facts, including observations without owned Work.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LinearIssueEvidence {
    pub issue_id: Option<String>,
    pub revision: Option<String>,
    pub observed_at: Option<i64>,
    pub project_id: Option<String>,
    pub initiative_ids: Vec<String>,
    pub project_team_ids: Vec<String>,
    pub team_id: Option<String>,
    pub state: PlanningState,
}

#[derive(Debug, Clone)]
pub(crate) struct WaveLocatorUpdate {
    pub wave_id: WaveId,
    pub expected_repo: String,
    pub expected_slug: String,
    pub target: WaveLocator,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum TaskPrMergeEvidenceOutcome {
    Accepted,
    Repeated,
    Missing,
    Conflict { accepted_at: i64 },
    SchemaUnavailable,
}

/// The Wave's planning lock, retained by every queued writer using this
/// acquisition and released when the last one drops it.
#[derive(Debug, Clone)]
pub struct PlanningLocks {
    _wave: std::sync::Arc<std::fs::File>,
}

impl PlanningLocks {
    pub(crate) fn new(wave: std::fs::File) -> Self {
        Self {
            _wave: std::sync::Arc::new(wave),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct CiIncidentReportRow {
    pub incident: crate::store::work::task::CiIncident,
    pub wave: Option<String>,
    pub task: Option<String>,
    pub task_started_at: Option<time::OffsetDateTime>,
    pub task_status: Option<String>,
    pub human_assisted: bool,
}

/// The serde representation of a bare time::OffsetDateTime.
pub(crate) type TimeTuple = (i32, u16, u8, u8, u8, u32, i8, i8, i8);
