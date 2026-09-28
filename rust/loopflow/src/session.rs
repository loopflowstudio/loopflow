//! Conversation identity and execution history. An invocation never owns feedback.

use serde::{Deserialize, Serialize};

use crate::durable::{RunId, TaskId};
use crate::id::WaveId;

/// Immutable native evidence. Missing start, attribution or usage stays missing.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SessionEvent {
    pub seq: i64,
    pub session_id: String,
    pub provider_thread: String,
    pub provider_turn: String,
    pub kind: SessionEventKind,
    pub provider_generation: Option<i64>,
    pub exec_id: Option<String>,
    pub task_id: Option<String>,
    pub wave_id: Option<String>,
    pub observed_at: i64,
    pub payload: serde_json::Value,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SessionEventKind {
    Started,
    Usage,
    Completed,
}

impl SessionEventKind {
    pub(crate) fn as_str(self) -> &'static str {
        match self {
            Self::Started => "started",
            Self::Usage => "usage",
            Self::Completed => "completed",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AgentSession {
    pub id: String,
    pub current_run_id: RunId,
    pub task_id: Option<TaskId>,
    pub wave_id: Option<WaveId>,
    pub flow_session_id: Option<String>,
    pub work_source: Option<WorkSource>,
    /// Time of a prospective bind; absent for admission or unknown historical timing.
    pub bound_at: Option<i64>,
    pub kind: SessionKind,
    pub interactive: bool,
    /// Canonical local repository at admission; absent when unknown or taskless outside Git.
    pub repo: Option<String>,
    pub title: String,
    pub title_source: TitleSource,
    /// What an Ask's caller asked; absent for other kinds.
    pub request: Option<String>,
    pub ready_summary: Option<String>,
    pub completed_at: Option<i64>,
    pub created_at: i64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SessionKind {
    Conversation,
    FlowReview,
    Ask,
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
    pub node: Option<u32>,
    pub iterations: Option<Vec<Vec<u32>>>,
    pub attempt: Option<u32>,
    pub task_id: Option<TaskId>,
    pub wave_id: Option<WaveId>,
    pub work_source: Option<WorkSource>,
    pub created_at: i64,
    pub published: bool,
    pub cwd: std::path::PathBuf,
    pub skill: Option<String>,
    /// Absent on review Runs recorded before providers were stored.
    pub provider: Option<String>,
    pub model: Option<String>,
    /// The Run that asked for this one. Causality, never membership.
    pub caller_run_id: Option<RunId>,
    /// Absent until the Run settles.
    pub ended: Option<RunEnd>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RunEnd {
    pub outcome: String,
    pub at: i64,
}

/// The Work a launch names. The Run constructor fills a Task's Wave.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RunWork {
    pub task_id: Option<TaskId>,
    pub wave_id: Option<WaveId>,
    pub source: WorkSource,
}

/// How this attempt received its work attribution; absent for unrecorded history.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum WorkSource {
    Declared,
    Checkout,
    Inherited,
    Bound,
}

/// SQL selection for conversation inventory; mode and completion are independent.
#[derive(Debug, Clone)]
pub struct SessionFilter {
    pub repo: Option<String>,
    pub task: Option<String>,
    pub search: Option<String>,
    pub interactive: Option<bool>,
    pub history: bool,
    pub limit: usize,
    pub offset: usize,
}

impl Default for SessionFilter {
    fn default() -> Self {
        Self {
            repo: None,
            task: None,
            search: None,
            interactive: Some(true),
            history: false,
            limit: 100,
            offset: 0,
        }
    }
}
