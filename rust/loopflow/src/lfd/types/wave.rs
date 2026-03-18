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
            "failed" => Ok(Self::Failed),
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

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
#[non_exhaustive]
pub enum WaveMode {
    #[default]
    Loop,
    Cron,
    Manual,
    Managed,
}

impl WaveMode {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Loop => "loop",
            Self::Cron => "cron",
            Self::Manual => "manual",
            Self::Managed => "managed",
        }
    }
}

impl std::str::FromStr for WaveMode {
    type Err = String;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        match value {
            "loop" => Ok(Self::Loop),
            "cron" => Ok(Self::Cron),
            "manual" => Ok(Self::Manual),
            "managed" => Ok(Self::Managed),
            _ => Err(format!("unknown wave mode: {value}")),
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

    pub fn is_persistent(self) -> bool {
        matches!(
            self,
            Self::ScratchDirty | Self::RebaseConflict | Self::PromotionFailed
        )
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

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Wave {
    pub id: LfdId,
    pub name: String,
    pub repo: String,
    pub mode: WaveMode,
    pub primary_flow: String,
    pub cron: Option<String>,
    pub direction: Vec<String>,
    pub area: Vec<String>,
    pub status: WaveStatus,
    pub iteration: u32,
    /// First iteration of the current cycle. Used with `max_iterations` to
    /// bound re-triggers within a single cycle rather than across the wave's
    /// lifetime.
    #[serde(default)]
    pub cycle_start_iteration: u32,
    #[serde(with = "time::serde::rfc3339::option")]
    pub created_at: Option<OffsetDateTime>,
    /// When true, only one run at a time — activations queue and dispatch
    /// sequentially. When false (default), triggers spawn runs immediately
    /// and git coordinates concurrent execution.
    #[serde(default)]
    pub serialized: bool,
}

impl Wave {
    pub fn new(id: LfdId, name: String, repo: String) -> Self {
        Self {
            id,
            name,
            repo,
            mode: WaveMode::Loop,
            primary_flow: "ship-roadmap".to_string(),
            cron: None,
            direction: Vec::new(),
            area: Vec::new(),
            status: WaveStatus::Idle,
            iteration: 0,
            cycle_start_iteration: 0,
            created_at: Some(OffsetDateTime::now_utc()),
            serialized: false,
        }
    }

    pub fn id(&self) -> &LfdId {
        &self.id
    }

    pub fn name(&self) -> &String {
        &self.name
    }

    pub fn repo(&self) -> &String {
        &self.repo
    }

    pub fn mode(&self) -> WaveMode {
        self.mode
    }

    pub fn primary_flow(&self) -> &String {
        &self.primary_flow
    }

    pub fn direction(&self) -> &Vec<String> {
        &self.direction
    }

    pub fn area(&self) -> &Vec<String> {
        &self.area
    }

    pub fn status(&self) -> WaveStatus {
        self.status
    }

    pub fn iteration(&self) -> u32 {
        self.iteration
    }

    pub fn cycle_start_iteration(&self) -> u32 {
        self.cycle_start_iteration
    }

    pub fn created_at(&self) -> Option<OffsetDateTime> {
        self.created_at
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
    pub activation_log_id: Option<LfdId>,
    pub parent_run_id: Option<LfdId>,
    pub parent_pr_number: Option<u32>,
    pub stack_position: u32,
    pub stack_group_id: String,
    #[serde(default)]
    pub stack_status: WaveRunStackStatus,
    #[serde(default)]
    pub lineage_inferred: bool,
    /// The branch this run targets. "main" means new branch off main (produce
    /// PR). Any other value means check out that branch and push to it (no PR).
    #[serde(default = "default_target_branch")]
    pub target_branch: String,
}

fn default_target_branch() -> String {
    "main".to_string()
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
            activation_log_id: None,
            parent_run_id: None,
            parent_pr_number: None,
            stack_position: 0,
            stack_group_id,
            stack_status: WaveRunStackStatus::Active,
            lineage_inferred: false,
            target_branch: "main".to_string(),
        }
    }
}
