//! Wave and Run types.

use serde::{Deserialize, Serialize};
use time::OffsetDateTime;

use crate::lfd::id::LfdId;

fn default_workers() -> u32 {
    1
}

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
    type Err = String;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        match value {
            "idle" => Ok(Self::Idle),
            "running" => Ok(Self::Running),
            "waiting" => Ok(Self::Waiting),
            "paused" => Ok(Self::Paused),
            "failed" => Ok(Self::Failed),
            _ => Err(format!("unknown wave status: {value}")),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum RunStatus {
    #[default]
    Unspecified = 0,
    Pending = 1,
    Running = 2,
    Waiting = 3,
    Completed = 4,
    Failed = 5,
}

impl RunStatus {
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
pub enum RunStackStatus {
    #[default]
    Active = 1,
    Superseded = 2,
    Merged = 3,
}

impl RunStackStatus {
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

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Wave {
    pub id: LfdId,
    pub name: String,
    pub primary_flow: String,
    pub goal: String,
    pub metrics: Vec<String>,
    /// The single repo this wave targets. A wave = exactly one repo.
    pub repo: String,
    /// Worktree path for the wave's repo, `""` when none.
    pub worktree: String,
    /// Branch name for the wave's repo, `""` when none.
    pub branch: String,
    /// Execution status of the wave's repo. Rolled into `status()` together with
    /// the wave-level `paused` flag.
    pub status: WaveStatus,
    pub iteration: u32,
    pub cycle_start_iteration: u32,
    pub direction: Vec<String>,
    pub area: Vec<String>,
    /// Wave-level pause flag. Rolled into `status()` (a paused wave reports
    /// `Paused` regardless of the repo's execution state).
    pub paused: bool,
    #[serde(with = "time::serde::rfc3339::option")]
    pub created_at: Option<OffsetDateTime>,
    /// Maximum number of active runs this wave can have at once.
    #[serde(default = "default_workers")]
    pub workers: u32,
    /// Parent wave in the chord tree. `None` for a root wave. A chord is simply
    /// a wave that has children (`children_of(id)` non-empty) — there is no
    /// `wave_type` discriminator.
    #[serde(default)]
    pub parent_wave_id: Option<LfdId>,
}

impl Wave {
    pub fn new(id: LfdId, name: String, repo: String) -> Self {
        Self {
            id,
            name,
            primary_flow: "ship-roadmap".to_string(),
            goal: "ship-roadmap".to_string(),
            metrics: Vec::new(),
            repo,
            worktree: String::new(),
            branch: String::new(),
            status: WaveStatus::Idle,
            iteration: 0,
            cycle_start_iteration: 0,
            direction: Vec::new(),
            area: Vec::new(),
            paused: false,
            created_at: Some(OffsetDateTime::now_utc()),
            workers: default_workers(),
            parent_wave_id: None,
        }
    }

    /// Attach this wave to a parent, making it a child in the chord tree.
    pub fn with_parent(mut self, parent: LfdId) -> Self {
        self.parent_wave_id = Some(parent);
        self
    }

    pub fn id(&self) -> &LfdId {
        &self.id
    }

    /// Parent wave in the chord tree, `None` for a root wave.
    pub fn parent_wave_id(&self) -> Option<&LfdId> {
        self.parent_wave_id.as_ref()
    }

    pub fn name(&self) -> &String {
        &self.name
    }

    /// The repo this wave targets.
    pub fn repo(&self) -> &str {
        &self.repo
    }

    pub fn primary_flow(&self) -> &String {
        &self.primary_flow
    }

    pub fn goal(&self) -> &str {
        &self.goal
    }

    pub fn metrics(&self) -> &Vec<String> {
        &self.metrics
    }

    pub fn direction(&self) -> &Vec<String> {
        &self.direction
    }

    pub fn area(&self) -> &Vec<String> {
        &self.area
    }

    /// Wave-level status. `Paused` is a wave-level flag; otherwise the status is
    /// the repo's execution status.
    pub fn status(&self) -> WaveStatus {
        if self.paused {
            return WaveStatus::Paused;
        }
        self.status
    }

    pub fn iteration(&self) -> u32 {
        self.iteration
    }

    pub fn cycle_start_iteration(&self) -> u32 {
        self.cycle_start_iteration
    }

    /// Set the wave's execution status. Toggles the wave-level `paused` flag and
    /// writes the execution status, so reads through `status()` agree.
    pub fn set_status(&mut self, status: WaveStatus) {
        self.paused = status == WaveStatus::Paused;
        self.status = status;
    }

    pub fn created_at(&self) -> Option<OffsetDateTime> {
        self.created_at
    }

    pub fn workers(&self) -> u32 {
        self.workers
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

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Run {
    pub id: LfdId,
    pub wave_id: LfdId,
    pub repo: String,
    pub flow: String,
    pub task: Option<String>,
    pub direction: Vec<String>,
    pub area: Vec<String>,
    pub iteration: u32,
    pub step_index: u32,
    pub status: RunStatus,
    pub worktree: String,
    pub branch: String,
    #[serde(with = "time::serde::rfc3339::option")]
    pub started_at: Option<OffsetDateTime>,
    #[serde(with = "time::serde::rfc3339::option")]
    pub ended_at: Option<OffsetDateTime>,
    pub error: Option<String>,
    pub flow_parents: Vec<String>,
    #[serde(skip)]
    pub execution_cursor: Option<String>,
    pub parent_run_id: Option<LfdId>,
    pub parent_pr_number: Option<u32>,
    pub stack_position: u32,
    pub stack_group_id: String,
    #[serde(default)]
    pub stack_status: RunStackStatus,
    #[serde(default)]
    pub lineage_inferred: bool,
    /// The branch this run targets. "main" means new branch off main (produce
    /// PR). Any other value means check out that branch and push to it (no PR).
    #[serde(default = "default_target_branch")]
    pub target_branch: String,
    /// When set, this run is a repair attempt for the referenced failed run.
    /// Written by dispatchers that retry failed work; nothing acts on it
    /// automatically since the repair chain died with the daemon's organs.
    pub repair_of: Option<LfdId>,
    /// The pull request created or associated with this run.
    /// Set when the run creates a PR (auto-create or land --create-pr).
    pub pr: Option<PullRequest>,
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

impl Run {
    pub fn new(id: LfdId, wave_id: LfdId) -> Self {
        let stack_group_id = wave_id.to_string();
        Self {
            id,
            wave_id,
            repo: String::new(),
            flow: String::new(),
            task: None,
            direction: Vec::new(),
            area: Vec::new(),
            iteration: 0,
            step_index: 0,
            status: RunStatus::Pending,
            worktree: String::new(),
            branch: String::new(),
            started_at: Some(OffsetDateTime::now_utc()),
            ended_at: None,
            error: None,
            flow_parents: Vec::new(),
            execution_cursor: None,
            parent_run_id: None,
            parent_pr_number: None,
            stack_position: 0,
            stack_group_id,
            stack_status: RunStackStatus::Active,
            lineage_inferred: false,
            target_branch: "main".to_string(),
            repair_of: None,
            pr: None,
        }
    }
}
