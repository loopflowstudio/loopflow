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
}

impl WaveRun {
    #[allow(dead_code)] // Convenience constructor for tests and future use.
    pub fn new(id: LfdId, wave_id: LfdId) -> Self {
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
        }
    }

    pub fn is_main(&self) -> bool {
        self.run_kind == WaveRunKind::Main
    }
}
