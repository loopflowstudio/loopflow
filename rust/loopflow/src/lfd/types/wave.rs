//! Durable Wave identity and authored launch policy.

use serde::{Deserialize, Serialize};
use time::OffsetDateTime;

use crate::lfd::id::LfdId;

fn default_task_capacity() -> u32 {
    1
}

pub const DEFAULT_WAVE_FLOW: &str = "ship-roadmap";

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

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Wave {
    pub id: LfdId,
    pub name: String,
    pub goal: String,
    pub metrics: Vec<String>,
    /// The single repo this wave targets. A wave = exactly one repo.
    pub repo: String,
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
    /// Maximum number of active Task Sessions this Wave can have at once.
    #[serde(default = "default_task_capacity")]
    pub task_capacity: u32,
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
            goal: "ship-roadmap".to_string(),
            metrics: Vec::new(),
            repo,
            status: WaveStatus::Idle,
            iteration: 0,
            cycle_start_iteration: 0,
            direction: Vec::new(),
            area: Vec::new(),
            paused: false,
            created_at: Some(OffsetDateTime::now_utc()),
            task_capacity: default_task_capacity(),
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

    pub fn task_capacity(&self) -> u32 {
        self.task_capacity
    }
}
