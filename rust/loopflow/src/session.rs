//! Conversation identity and execution history. An invocation never owns feedback.

use serde::{Deserialize, Serialize};

use crate::durable::{RunId, TaskId};
use crate::id::WaveId;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Session {
    pub id: String,
    pub current_run_id: RunId,
    pub title: String,
    pub title_source: TitleSource,
    pub ready_summary: Option<String>,
    pub completed_at: Option<i64>,
    pub created_at: i64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TitleSource {
    Generated,
    Human,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Run {
    pub id: RunId,
    pub session_id: Option<String>,
    pub invocation_id: Option<String>,
    pub task_id: Option<TaskId>,
    pub wave_id: Option<WaveId>,
    pub created_at: i64,
    pub published: bool,
    pub cwd: std::path::PathBuf,
    pub skill: Option<String>,
}
