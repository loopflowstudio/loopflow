use loopflow_model::durable::MachineId;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RecordKind {
    Session,
    SessionEvent,
    History,
    Turn,
    Transcript,
    Checkpoint,
}

impl RecordKind {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Session => "session",
            Self::SessionEvent => "session_event",
            Self::History => "history",
            Self::Turn => "turn",
            Self::Transcript => "transcript",
            Self::Checkpoint => "checkpoint",
        }
    }
}

/// One copied row: its table's kind, its key there, and its columns as written.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Record {
    pub kind: RecordKind,
    pub key: String,
    pub row: serde_json::Value,
}

/// Source positions in loopflow.db and history.db at the same export.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct CopyCursor {
    pub state: i64,
    pub history: i64,
    /// Source wall time before reading mutable provider files.
    pub started_at: i64,
}

/// One line of `lf history export`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "line", rename_all = "snake_case")]
pub enum ExportLine {
    /// Always first: which Machine wrote these, so a reused address cannot
    /// file another Machine's records under this one.
    Origin {
        machine_id: MachineId,
    },
    Row(Record),
    /// One file of a Session capture, base64.
    Capture {
        artifact_key: String,
        path: String,
        content: String,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct CopyCounts {
    pub sessions: usize,
}

impl std::fmt::Display for CopyCounts {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let noun = if self.sessions == 1 {
            "session"
        } else {
            "sessions"
        };
        write!(f, "{} {noun}", self.sessions)
    }
}

/// A Session as its origin Machine recorded it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct CopiedSession {
    pub origin: MachineId,
    /// The origin's label while it is still added here.
    pub origin_label: Option<String>,
    pub id: String,
    pub title: String,
    pub completed_at: Option<i64>,
}

impl CopiedSession {
    pub fn origin_name(&self) -> &str {
        self.origin_label.as_deref().unwrap_or(self.origin.as_str())
    }
}

/// A copied Session with what its origin recorded about it.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct CopiedHistory {
    pub session: CopiedSession,
    pub task_id: Option<String>,
    /// A path on the origin, which may no longer exist anywhere.
    pub cwd: Option<String>,
    pub created_at: Option<i64>,
    /// When this Machine last copied it.
    pub copied_at: i64,
    pub events: Vec<CopiedEvent>,
    pub turns: Vec<crate::session::Turn>,
    pub usage: crate::session_record::SessionUsage,
}

/// One session event as its origin wrote it.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct CopiedEvent {
    pub kind: String,
    pub observed_at: Option<i64>,
    /// The capture a `captured` event names.
    pub artifact_key: Option<String>,
    pub row: serde_json::Value,
}
