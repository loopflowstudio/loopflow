//! Wave and WaveRun types.

use serde::{Deserialize, Serialize};
use time::OffsetDateTime;

use crate::lfd::id::LfdId;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum WaveStatus {
    #[default]
    Idle = 1,
    Running = 2,
    Waiting = 3,
    Paused = 4,
    Failed = 5,
}

impl WaveStatus {
    pub fn from_i32(value: i32) -> Self {
        match value {
            1 => Self::Idle,
            2 => Self::Running,
            3 => Self::Waiting,
            4 => Self::Paused,
            5 => Self::Failed,
            _ => Self::Idle,
        }
    }

    pub fn as_i32(&self) -> i32 {
        *self as i32
    }

    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Idle => "idle",
            Self::Running => "running",
            Self::Waiting => "waiting",
            Self::Paused => "paused",
            Self::Failed => "failed",
        }
    }
}

impl std::str::FromStr for WaveStatus {
    type Err = ();

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        match value {
            "idle" => Ok(Self::Idle),
            "running" => Ok(Self::Running),
            "waiting" => Ok(Self::Waiting),
            "paused" => Ok(Self::Paused),
            "failed" | "error" => Ok(Self::Failed),
            _ => Err(()),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum WaveRunStatus {
    #[default]
    Unspecified = 0,
    Pending = 1,
    Running = 2,
    Waiting = 3,
    Completed = 4,
    Failed = 5,
}

impl WaveRunStatus {
    pub fn from_i32(value: i32) -> Self {
        match value {
            1 => Self::Pending,
            2 => Self::Running,
            3 => Self::Waiting,
            4 => Self::Completed,
            5 => Self::Failed,
            _ => Self::Unspecified,
        }
    }

    pub fn as_i32(&self) -> i32 {
        *self as i32
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum WaveRunKind {
    #[default]
    Main = 1,
    Sidecar = 2,
}

impl WaveRunKind {
    pub fn from_i32(value: i32) -> Self {
        match value {
            2 => Self::Sidecar,
            _ => Self::Main,
        }
    }

    pub fn as_i32(&self) -> i32 {
        *self as i32
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum WaveRunStackStatus {
    #[default]
    Active = 1,
    Superseded = 2,
    Merged = 3,
}

impl WaveRunStackStatus {
    pub fn from_i32(value: i32) -> Self {
        match value {
            2 => Self::Superseded,
            3 => Self::Merged,
            _ => Self::Active,
        }
    }

    pub fn as_i32(&self) -> i32 {
        *self as i32
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum LivePrState {
    #[default]
    Unknown = 0,
    Open = 1,
    Closed = 2,
    Merged = 3,
}

impl LivePrState {
    pub fn from_i32(value: i32) -> Self {
        match value {
            1 => Self::Open,
            2 => Self::Closed,
            3 => Self::Merged,
            _ => Self::Unknown,
        }
    }

    pub fn as_i32(&self) -> i32 {
        *self as i32
    }

    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Unknown => "unknown",
            Self::Open => "open",
            Self::Closed => "closed",
            Self::Merged => "merged",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum QueueBlockReason {
    MissingPr,
    WaveRunning,
    ScratchDirty,
    RebaseConflict,
    PromotionFailed,
}

impl QueueBlockReason {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::MissingPr => "missing_pr",
            Self::WaveRunning => "wave_running",
            Self::ScratchDirty => "scratch_dirty",
            Self::RebaseConflict => "rebase_conflict",
            Self::PromotionFailed => "promotion_failed",
        }
    }
}

impl std::str::FromStr for QueueBlockReason {
    type Err = String;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        match value {
            "missing_pr" => Ok(Self::MissingPr),
            "wave_running" => Ok(Self::WaveRunning),
            "scratch_dirty" => Ok(Self::ScratchDirty),
            "rebase_conflict" => Ok(Self::RebaseConflict),
            "promotion_failed" => Ok(Self::PromotionFailed),
            _ => Err(format!("unknown queue block reason: {value}")),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SidecarKind {
    CiFix = 1,
}

impl SidecarKind {
    pub fn from_i32(value: i32) -> Option<Self> {
        match value {
            1 => Some(Self::CiFix),
            _ => None,
        }
    }

    pub fn as_i32(&self) -> i32 {
        *self as i32
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Wave {
    pub id: LfdId,
    pub name: String,
    pub repo: String,
    pub flow: String,
    pub direction: Vec<String>,
    pub area: Vec<String>,
    pub status: WaveStatus,
    pub iteration: u32,
    pub schema_ref: Option<String>,
    pub schema_name: Option<String>,
    #[serde(with = "time::serde::rfc3339::option")]
    pub created_at: Option<OffsetDateTime>,
}

impl Wave {
    #[allow(dead_code)] // Convenience constructor for tests and future use.
    pub fn new(id: LfdId, name: String, repo: String) -> Self {
        Self {
            id,
            name,
            repo,
            flow: String::new(),
            direction: Vec::new(),
            area: Vec::new(),
            status: WaveStatus::Idle,
            iteration: 0,
            schema_ref: None,
            schema_name: None,
            created_at: Some(OffsetDateTime::now_utc()),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PullRequest {
    pub url: String,
    pub number: Option<u32>,
    pub state: Option<String>,
    pub title: Option<String>,
    pub branch: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct WaveRunSnapshot {
    pub repo: String,
    pub flow: String,
    pub direction: Vec<String>,
    pub area: Vec<String>,
    pub pr: Option<PullRequest>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WaveRun {
    pub id: LfdId,
    pub wave_id: LfdId,
    pub snapshot: WaveRunSnapshot,
    pub iteration: u32,
    pub step_index: u32,
    pub status: WaveRunStatus,
    pub worktree: String,
    pub branch: String,
    #[serde(with = "time::serde::rfc3339::option")]
    pub started_at: Option<OffsetDateTime>,
    #[serde(with = "time::serde::rfc3339::option")]
    pub ended_at: Option<OffsetDateTime>,
    pub error: Option<String>,
    pub flow_parents: Vec<String>,
    #[serde(default)]
    pub run_kind: WaveRunKind,
    pub sidecar_kind: Option<SidecarKind>,
    pub parent_run_id: Option<LfdId>,
    pub parent_pr_number: Option<u32>,
    pub stack_position: u32,
    pub stack_group_id: String,
    #[serde(default)]
    pub stack_status: WaveRunStackStatus,
    #[serde(default)]
    pub lineage_inferred: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LivePullRequestState {
    pub repo_id: String,
    pub pr_number: u32,
    pub state: LivePrState,
    pub is_draft: bool,
    pub head_ref: String,
    pub head_sha: String,
    pub base_ref: String,
    #[serde(with = "time::serde::rfc3339")]
    pub updated_at: OffsetDateTime,
    #[serde(with = "time::serde::rfc3339::option")]
    pub merged_at: Option<OffsetDateTime>,
    #[serde(with = "time::serde::rfc3339")]
    pub synced_at: OffsetDateTime,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QueueBlock {
    pub wave_id: LfdId,
    pub run_id: LfdId,
    pub reason: QueueBlockReason,
    #[serde(with = "time::serde::rfc3339")]
    pub attempted_at: OffsetDateTime,
    pub conflict_files: Vec<String>,
    pub error: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QueueMergeEvent {
    pub wave_id: LfdId,
    pub pr_number: u32,
    #[serde(with = "time::serde::rfc3339")]
    pub merged_at: OffsetDateTime,
    #[serde(with = "time::serde::rfc3339")]
    pub processed_at: OffsetDateTime,
}

impl WaveRun {
    #[allow(dead_code)] // Convenience constructor for tests and future use.
    pub fn new(id: LfdId, wave_id: LfdId) -> Self {
        let stack_group_id = wave_id.to_string();
        Self {
            id,
            wave_id,
            snapshot: WaveRunSnapshot::default(),
            iteration: 0,
            step_index: 0,
            status: WaveRunStatus::Pending,
            worktree: String::new(),
            branch: String::new(),
            started_at: Some(OffsetDateTime::now_utc()),
            ended_at: None,
            error: None,
            flow_parents: Vec::new(),
            run_kind: WaveRunKind::Main,
            sidecar_kind: None,
            parent_run_id: None,
            parent_pr_number: None,
            stack_position: 0,
            stack_group_id,
            stack_status: WaveRunStackStatus::Active,
            lineage_inferred: false,
        }
    }

    pub fn is_main(&self) -> bool {
        self.run_kind == WaveRunKind::Main
    }
}

#[cfg(test)]
mod tests {
    use super::WaveRunKind;

    #[test]
    fn wave_run_kind_main_storage_value_is_stable() {
        assert_eq!(WaveRunKind::Main.as_i32(), 1);
        assert_eq!(WaveRunKind::from_i32(1), WaveRunKind::Main);
    }
}
