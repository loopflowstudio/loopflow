use std::collections::HashMap;

use serde::{Deserialize, Serialize};
use serde_json::Value;
use time::OffsetDateTime;

use crate::engine::prompt::{
    ContextBreakdown, DiffTier, DocumentSource, Surface, DEFAULT_CONTEXT_BUDGET,
};
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

    pub(crate) fn as_i32(self) -> i32 {
        match self {
            Self::Starting => 0,
            Self::Active => 1,
            Self::Ending => 2,
            Self::Ended => 3,
            Self::Failed => 4,
        }
    }

    pub(crate) fn from_i32(value: i32) -> Self {
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
    /// Generic fallback for harnesses that don't distinguish item types.
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

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
#[non_exhaustive]
pub enum ItemDelta {
    Output { content: String },
    PlanText { content: String },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SuggestedActionPayload {
    pub label: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
}

/// Token usage for a single agent turn.
#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq)]
pub struct TurnUsage {
    pub input_tokens: u64,
    pub output_tokens: u64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reasoning_tokens: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cache_read_tokens: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cache_write_tokens: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub model: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cost_usd: Option<f64>,
}

/// Prompt composition snapshot at session start.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ContextSnapshot {
    /// Tokens per source category ("step", "direction", "diff", "area", "repo_doc", etc.)
    pub sources: HashMap<String, u64>,
    /// Total context budget available.
    pub budget: u64,
    /// Total tokens used.
    pub total: u64,
    /// Diff representation tier ("UnifiedDiff", "StatOnly", "None").
    pub diff_tier: String,
}

impl From<&ContextBreakdown> for ContextSnapshot {
    fn from(breakdown: &ContextBreakdown) -> Self {
        Self {
            sources: breakdown
                .source_tokens
                .iter()
                .map(|(source, tokens)| (source_key(*source), *tokens as u64))
                .collect(),
            budget: DEFAULT_CONTEXT_BUDGET as u64,
            total: breakdown.total() as u64,
            diff_tier: diff_tier_key(&breakdown.diff_tier).to_string(),
        }
    }
}

fn source_key(source: DocumentSource) -> String {
    match source {
        DocumentSource::Step => "step",
        DocumentSource::Direction => "direction",
        DocumentSource::Wave => "wave",
        DocumentSource::WaveMemory => "wave_memory",
        DocumentSource::Area => "area",
        DocumentSource::Diff => "diff",
        DocumentSource::Clipboard => "clipboard",
        DocumentSource::RepoDoc => "repo_doc",
        DocumentSource::Summary => "summary",
    }
    .to_string()
}

fn diff_tier_key(diff_tier: &DiffTier) -> &'static str {
    match diff_tier {
        DiffTier::UnifiedDiff => "UnifiedDiff",
        DiffTier::StatOnly => "StatOnly",
        DiffTier::None => "None",
    }
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
    /// Token usage for a completed turn. Emitted after TurnCompleted.
    TurnUsage {
        turn_id: String,
        usage: TurnUsage,
    },
    /// Prompt composition snapshot. Emitted once at session start, before first TurnStarted.
    ContextSnapshot {
        snapshot: ContextSnapshot,
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
    SuggestedActions {
        turn_id: String,
        actions: Vec<SuggestedActionPayload>,
    },

    // Session-level
    StatusChanged {
        status: SessionStatus,
    },
    Error {
        code: String,
        message: String,
    },

    // Internal (not persisted or forwarded to SSE clients)
    ProviderSessionId {
        provider_session_id: String,
    },
}

impl SessionEvent {
    pub fn event_type(&self) -> &'static str {
        match self {
            Self::TurnStarted { .. } => "turn_started",
            Self::TurnCompleted { .. } => "turn_completed",
            Self::TurnUsage { .. } => "turn_usage",
            Self::ContextSnapshot { .. } => "context_snapshot",
            Self::ItemStarted { .. } => "item_started",
            Self::ItemUpdated { .. } => "item_updated",
            Self::ItemCompleted { .. } => "item_completed",
            Self::TextDelta { .. } => "text_delta",
            Self::ReasoningDelta { .. } => "reasoning_delta",
            Self::DiffUpdated { .. } => "diff_updated",
            Self::SuggestedActions { .. } => "suggested_actions",
            Self::StatusChanged { .. } => "status_changed",
            Self::Error { .. } => "error",
            Self::ProviderSessionId { .. } => "provider_session_id",
        }
    }
}

// -- Session config and record --

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct SessionConfig {
    #[serde(default)]
    pub step: String,
    #[serde(default)]
    pub repo_root: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub directions: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub area: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub wave: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub message: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub surface: Option<Surface>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub model: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cwd: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_turns: Option<u32>,
    #[serde(default)]
    pub yolo_mode: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub client_has_ui: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub client_compact: Option<bool>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Session {
    pub id: LfdId,
    pub harness: String,
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
    pub harness: String,
    pub wave_run_id: Option<String>,
    pub config: SessionConfig,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn context_snapshot_from_breakdown_preserves_source_tokens() {
        let breakdown = ContextBreakdown {
            source_tokens: HashMap::from([
                (DocumentSource::Step, 120),
                (DocumentSource::Direction, 80),
                (DocumentSource::Diff, 450),
            ]),
            system_tokens: 25,
            diff_tier: DiffTier::StatOnly,
            ..ContextBreakdown::default()
        };

        let snapshot = ContextSnapshot::from(&breakdown);
        assert_eq!(snapshot.sources.get("step"), Some(&120));
        assert_eq!(snapshot.sources.get("direction"), Some(&80));
        assert_eq!(snapshot.sources.get("diff"), Some(&450));
        assert_eq!(snapshot.total, 675);
        assert_eq!(snapshot.budget, DEFAULT_CONTEXT_BUDGET as u64);
        assert_eq!(snapshot.diff_tier, "StatOnly");
    }

    #[test]
    fn turn_usage_round_trips_through_json() {
        let usage = TurnUsage {
            input_tokens: 123,
            output_tokens: 45,
            reasoning_tokens: Some(12),
            cache_read_tokens: Some(17),
            cache_write_tokens: Some(9),
            model: Some("claude-3-7-sonnet".to_string()),
            cost_usd: Some(0.021),
        };

        let value = serde_json::to_value(&usage).expect("serialize usage");
        let decoded: TurnUsage = serde_json::from_value(value).expect("deserialize usage");
        assert_eq!(decoded, usage);
    }

    #[test]
    fn context_snapshot_round_trips_through_json() {
        let snapshot = ContextSnapshot {
            sources: HashMap::from([("step".to_string(), 200), ("direction".to_string(), 50)]),
            budget: 75_000,
            total: 250,
            diff_tier: "UnifiedDiff".to_string(),
        };

        let value = serde_json::to_value(&snapshot).expect("serialize snapshot");
        let decoded: ContextSnapshot = serde_json::from_value(value).expect("deserialize snapshot");
        assert_eq!(decoded, snapshot);
    }
}
