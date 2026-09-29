//! Conversation identity and execution history. An invocation never owns feedback.

use serde::{Deserialize, Serialize};

use crate::durable::{RunId, TaskId};
use crate::id::WaveId;

/// Immutable native evidence. Missing start, attribution or usage stays missing.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SessionEvent {
    pub seq: i64,
    pub session_id: String,
    pub provider_thread: Option<String>,
    pub provider_turn: Option<String>,
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
    /// Evidence without an exact native turn; cannot settle a Flow.
    Observed,
}

/// Captured or imported evidence; no execution identity or authority.
#[derive(Debug)]
pub(crate) struct SessionObservation {
    pub input_id: RunId,
    pub source: String,
    pub observed_at: i64,
    pub task_id: Option<TaskId>,
    pub wave_id: Option<WaveId>,
    pub payload: serde_json::Value,
}

impl SessionEventKind {
    pub(crate) fn as_str(self) -> &'static str {
        match self {
            Self::Started => "started",
            Self::Usage => "usage",
            Self::Completed => "completed",
            Self::Observed => "observed",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AgentSession {
    pub id: String,
    /// Immutable captured input, not a resumable execution identity.
    pub input_id: RunId,
    /// Causal input reference; grants neither driver nor Flow authority.
    pub caller_input_id: Option<RunId>,
    pub input_published: bool,
    pub cwd: std::path::PathBuf,
    pub skill: Option<String>,
    pub provider: Option<String>,
    pub model: Option<String>,
    pub node: Option<u32>,
    pub iterations: Option<Vec<Vec<u32>>>,
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

/// The Work a launch names. Admission fills a Task's Wave.
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

/// Passive inventory values. Request, transcript, capture and native identity
/// validation belong to exact detail/actions, never to this row projection.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct SessionSummary {
    pub id: String,
    pub input_id: RunId,
    pub title: String,
    pub title_source: TitleSource,
    pub ready_summary: Option<String>,
    pub completed_at: Option<i64>,
    pub kind: SessionKind,
    pub interactive: bool,
    pub task_id: Option<TaskId>,
    pub wave_id: Option<WaveId>,
    pub flow_session_id: Option<String>,
    pub cwd: std::path::PathBuf,
    pub skill: Option<String>,
    pub provider: Option<String>,
    pub model: Option<String>,
    pub node: Option<u32>,
    pub iterations: Option<Vec<Vec<u32>>>,
    pub flow: Option<FlowSummary>,
    pub independent: bool,
    pub wave_name: Option<String>,
    pub task_identifier: Option<String>,
    pub managed: bool,
    pub home_id: Option<crate::durable::HomeId>,
    pub home_route: Option<String>,
}

/// Recorded Flow facts; Current says nothing about a live driver or process.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct FlowSummary {
    pub id: String,
    pub name: Option<String>,
    pub state: FlowSummaryState,
    pub current_input: Option<RunId>,
    pub pending_session: Option<String>,
    pub task_id: Option<TaskId>,
    pub wave_id: Option<WaveId>,
    pub updated_at: i64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum FlowSummaryState {
    Current,
    Completed,
    Replaced,
}
