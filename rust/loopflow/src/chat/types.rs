use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[non_exhaustive]
#[serde(rename_all = "snake_case")]
pub enum ConversationStatus {
    Starting,
    Active,
    Ending,
    Ended,
    Failed,
}

/// Lifecycle of a turn or item, shared across the wire and the harness.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[non_exhaustive]
#[serde(rename_all = "snake_case")]
pub enum Lifecycle {
    Pending,
    Running,
    Completed,
    Failed,
    Interrupted,
}

impl Lifecycle {
    /// The console/wire name, identical to the serde representation.
    pub fn name(&self) -> &'static str {
        match self {
            Self::Pending => "pending",
            Self::Running => "running",
            Self::Completed => "completed",
            Self::Failed => "failed",
            Self::Interrupted => "interrupted",
        }
    }
}

// -- Typed items --

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct FileEdit {
    pub path: String,
    pub kind: Option<String>,
    pub diff: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
#[non_exhaustive]
pub enum ConversationItem {
    Command {
        id: String,
        /// argv when the vendor provides argv; otherwise the raw command line
        /// as a single element.
        command: Vec<String>,
        cwd: String,
        status: Lifecycle,
        output: Option<String>,
        exit_code: Option<i32>,
        duration_ms: Option<u64>,
    },
    File {
        id: String,
        changes: Vec<FileEdit>,
        status: Lifecycle,
    },
    Message {
        id: String,
        text: String,
        phase: Option<String>,
    },
    Thought {
        id: String,
        text: String,
    },
    /// Generic fallback for harnesses that don't distinguish item types.
    Tool {
        id: String,
        name: String,
        status: Lifecycle,
        input: Option<Value>,
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
    /// Provider-normalized input processed across the reported agent turn or
    /// session snapshot. Cached input is included exactly once.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub total_input_tokens: Option<u64>,
    /// Largest single provider request observed in this usage interval.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub peak_input_tokens: Option<u64>,
    /// Model context window corresponding to `peak_input_tokens`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub context_window_tokens: Option<u64>,
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

// -- Event stream --

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
#[non_exhaustive]
pub enum ConversationEvent {
    // Turn boundaries
    TurnStarted {
        turn_id: String,
    },
    TurnCompleted {
        turn_id: String,
        status: Lifecycle,
    },
    /// Token usage for a completed turn. Emitted after TurnCompleted.
    TurnUsage {
        turn_id: String,
        usage: TurnUsage,
    },

    // Item lifecycle
    ItemStarted {
        turn_id: String,
        item: ConversationItem,
    },
    ItemUpdated {
        turn_id: String,
        item_id: String,
        data: ItemDelta,
    },
    ItemCompleted {
        turn_id: String,
        item: ConversationItem,
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

    // Conversation-level
    StatusChanged {
        status: ConversationStatus,
    },
    Error {
        code: String,
        message: String,
    },
}

impl ConversationEvent {
    pub fn event_type(&self) -> &'static str {
        match self {
            Self::TurnStarted { .. } => "turn_started",
            Self::TurnCompleted { .. } => "turn_completed",
            Self::TurnUsage { .. } => "turn_usage",
            Self::ItemStarted { .. } => "item_started",
            Self::ItemUpdated { .. } => "item_updated",
            Self::ItemCompleted { .. } => "item_completed",
            Self::TextDelta { .. } => "text_delta",
            Self::ReasoningDelta { .. } => "reasoning_delta",
            Self::DiffUpdated { .. } => "diff_updated",
            Self::SuggestedActions { .. } => "suggested_actions",
            Self::StatusChanged { .. } => "status_changed",
            Self::Error { .. } => "error",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn turn_usage_round_trips_through_json() {
        let usage = TurnUsage {
            input_tokens: 123,
            output_tokens: 45,
            total_input_tokens: Some(149),
            peak_input_tokens: Some(80),
            context_window_tokens: Some(200),
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
}
