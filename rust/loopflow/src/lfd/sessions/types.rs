use serde::{Deserialize, Serialize};
use serde_json::Value;
use time::OffsetDateTime;

use crate::lfd::id::LfdId;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[non_exhaustive]
#[serde(rename_all = "snake_case")]
pub enum SessionStatus {
    Starting,
    Active,
    Ending,
    Ended,
    Failed,
}

impl SessionStatus {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Starting => "starting",
            Self::Active => "active",
            Self::Ending => "ending",
            Self::Ended => "ended",
            Self::Failed => "failed",
        }
    }

    pub fn as_i32(self) -> i32 {
        match self {
            Self::Starting => 0,
            Self::Active => 1,
            Self::Ending => 2,
            Self::Ended => 3,
            Self::Failed => 4,
        }
    }

    pub fn from_i32(value: i32) -> Self {
        match value {
            0 => Self::Starting,
            1 => Self::Active,
            2 => Self::Ending,
            3 => Self::Ended,
            4 => Self::Failed,
            _ => Self::Failed,
        }
    }

    pub fn is_terminal(self) -> bool {
        matches!(self, Self::Ended | Self::Failed)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[non_exhaustive]
#[serde(rename_all = "snake_case")]
pub enum TurnStatus {
    Completed,
    Interrupted,
    Failed,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct SessionConfig {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub model: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cwd: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
#[non_exhaustive]
pub enum SessionEvent {
    TextDelta {
        content: String,
    },
    TextDone {
        content: String,
    },
    ToolStarted {
        tool_id: String,
        name: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        input: Option<Value>,
    },
    ToolOutput {
        tool_id: String,
        content: String,
    },
    ToolDone {
        tool_id: String,
    },
    TurnStarted,
    TurnCompleted {
        status: TurnStatus,
    },
    SessionStatus {
        status: SessionStatus,
    },
    Error {
        code: String,
        message: String,
    },
}

impl SessionEvent {
    pub fn event_type(&self) -> &'static str {
        match self {
            Self::TextDelta { .. } => "text_delta",
            Self::TextDone { .. } => "text_done",
            Self::ToolStarted { .. } => "tool_started",
            Self::ToolOutput { .. } => "tool_output",
            Self::ToolDone { .. } => "tool_done",
            Self::TurnStarted => "turn_started",
            Self::TurnCompleted { .. } => "turn_completed",
            Self::SessionStatus { .. } => "session_status",
            Self::Error { .. } => "error",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Session {
    pub id: LfdId,
    pub provider: String,
    pub status: SessionStatus,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub wave_run_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub provider_session_id: Option<String>,
    pub config: SessionConfig,
    pub created_at: OffsetDateTime,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ended_at: Option<OffsetDateTime>,
}

#[derive(Debug, Clone)]
pub struct PersistedSessionEvent {
    pub session_id: LfdId,
    pub seq: i64,
    pub event: SessionEvent,
    pub created_at: OffsetDateTime,
}

#[derive(Debug, Clone)]
pub struct CreateSessionParams {
    pub provider: String,
    pub wave_run_id: Option<String>,
    pub config: SessionConfig,
}
