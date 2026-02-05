//! Wave and WaveRun types.

use serde::{Deserialize, Serialize};
use time::OffsetDateTime;

use crate::id::LfdId;

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

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Wave {
    pub id: LfdId,
    pub name: String,
    pub repo: String,
    pub flow: String,
    pub direction: Vec<String>,
    pub area: Vec<String>,
    pub paused: bool,
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
            paused: false,
            created_at: Some(OffsetDateTime::now_utc()),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WaveRun {
    pub id: LfdId,
    pub wave_id: LfdId,
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
}

impl WaveRun {
    #[allow(dead_code)] // Convenience constructor for tests and future use.
    pub fn new(id: LfdId, wave_id: LfdId) -> Self {
        Self {
            id,
            wave_id,
            iteration: 0,
            step_index: 0,
            status: WaveRunStatus::Pending,
            worktree: String::new(),
            branch: String::new(),
            started_at: Some(OffsetDateTime::now_utc()),
            ended_at: None,
            error: None,
            flow_parents: Vec::new(),
        }
    }
}
