//! Agent types.

use serde::{Deserialize, Serialize};
use time::OffsetDateTime;

use crate::lfd::id::LfdId;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum AgentStatus {
    #[default]
    Unspecified = 0,
    Running = 1,
    Waiting = 2,
    Completed = 3,
    Failed = 4,
}

impl AgentStatus {
    pub fn from_i32(value: i32) -> Self {
        match value {
            1 => Self::Running,
            2 => Self::Waiting,
            3 => Self::Completed,
            4 => Self::Failed,
            _ => Self::Unspecified,
        }
    }

    pub fn as_i32(&self) -> i32 {
        *self as i32
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Agent {
    pub id: LfdId,
    pub step: String,
    pub repo: String,
    pub worktree: String,
    pub wave_run_id: Option<LfdId>,
    pub status: AgentStatus,
    #[serde(with = "time::serde::rfc3339::option")]
    pub started_at: Option<OffsetDateTime>,
    #[serde(with = "time::serde::rfc3339::option")]
    pub ended_at: Option<OffsetDateTime>,
    pub pid: Option<u32>,
    pub model: String,
    pub run_mode: String,
}

impl Agent {
    #[allow(dead_code)] // Convenience constructor for tests and future use.
    pub fn new(id: LfdId, step: String, repo: String, worktree: String) -> Self {
        Self {
            id,
            step,
            repo,
            worktree,
            wave_run_id: None,
            status: AgentStatus::Running,
            started_at: Some(OffsetDateTime::now_utc()),
            ended_at: None,
            pid: None,
            model: String::new(),
            run_mode: String::new(),
        }
    }
}
