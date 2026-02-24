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

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[non_exhaustive]
#[serde(rename_all = "snake_case")]
pub enum ItemStatus {
    InProgress,
    Completed,
    Failed,
    Declined,
}

// -- Typed items --

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileEdit {
    pub path: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub kind: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub diff: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
#[non_exhaustive]
pub enum SessionItem {
    Command {
        id: String,
        #[serde(default)]
        command: Vec<String>,
        #[serde(default)]
        cwd: String,
        status: ItemStatus,
        #[serde(skip_serializing_if = "Option::is_none")]
        output: Option<String>,
        #[serde(skip_serializing_if = "Option::is_none")]
        exit_code: Option<i32>,
        #[serde(skip_serializing_if = "Option::is_none")]
        duration_ms: Option<u64>,
    },
    File {
        id: String,
        #[serde(default)]
        changes: Vec<FileEdit>,
        status: ItemStatus,
    },
    Message {
        id: String,
        #[serde(default)]
        text: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        phase: Option<String>,
    },
    Thought {
        id: String,
        #[serde(default)]
        text: String,
    },
    /// Generic fallback for providers that don't distinguish item types.
    Tool {
        id: String,
        name: String,
        status: ItemStatus,
        #[serde(skip_serializing_if = "Option::is_none")]
        input: Option<Value>,
        #[serde(skip_serializing_if = "Option::is_none")]
        output: Option<String>,
    },
}

impl SessionItem {
    pub fn id(&self) -> &str {
        match self {
            Self::Command { id, .. }
            | Self::File { id, .. }
            | Self::Message { id, .. }
            | Self::Thought { id, .. }
            | Self::Tool { id, .. } => id,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
#[non_exhaustive]
pub enum ItemDelta {
    Output { content: String },
    PlanText { content: String },
}

// -- Event stream --

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
#[non_exhaustive]
pub enum SessionEvent {
    // Turn boundaries
    TurnStarted {
        turn_id: String,
    },
    TurnCompleted {
        turn_id: String,
        status: TurnStatus,
    },

    // Item lifecycle
    ItemStarted {
        turn_id: String,
        item: SessionItem,
    },
    ItemUpdated {
        turn_id: String,
        item_id: String,
        data: ItemDelta,
    },
    ItemCompleted {
        turn_id: String,
        item: SessionItem,
    },

    // High-frequency streaming
    TextDelta {
        turn_id: String,
        content: String,
    },
    ReasoningDelta {
        turn_id: String,
        content: String,
    },

    // Turn-level aggregates
    DiffUpdated {
        turn_id: String,
        diff: String,
    },

    // Session-level
    StatusChanged {
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
            Self::TurnStarted { .. } => "turn_started",
            Self::TurnCompleted { .. } => "turn_completed",
            Self::ItemStarted { .. } => "item_started",
            Self::ItemUpdated { .. } => "item_updated",
            Self::ItemCompleted { .. } => "item_completed",
            Self::TextDelta { .. } => "text_delta",
            Self::ReasoningDelta { .. } => "reasoning_delta",
            Self::DiffUpdated { .. } => "diff_updated",
            Self::StatusChanged { .. } => "status_changed",
            Self::Error { .. } => "error",
        }
    }
}

// -- Session config and record --

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct SessionConfig {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub model: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cwd: Option<String>,
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
