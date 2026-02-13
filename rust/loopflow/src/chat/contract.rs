use anyhow::Context;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
#[non_exhaustive]
pub enum UserMessagePhase {
    Progress,
    Final,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct SendMessageArgs {
    pub content: String,
    pub phase: UserMessagePhase,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct WorkspaceSnapshot {
    pub branch: String,
    pub head_sha_at_start: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ChatTurnRequest {
    pub wave_id: String,
    pub content: String,
    pub token_history_budget: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
pub struct MemoryEditLog {
    pub op: String,
    pub block: String,
    pub detail: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ToolCallLog {
    pub tool: String,
    pub args: serde_json::Value,
    pub result_summary: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
pub struct ContextSnapshot {
    pub memory_tokens: u32,
    pub history_tokens: u32,
    pub total_tokens: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ChatTurnResult {
    pub id: String,
    pub response: String,
    pub final_message_seen: bool,
    pub memory_edits: Vec<MemoryEditLog>,
    pub tool_calls: Vec<ToolCallLog>,
    pub context: ContextSnapshot,
    pub snapshot: WorkspaceSnapshot,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(tag = "type", rename_all = "snake_case")]
#[non_exhaustive]
pub enum AgentEvent {
    Message {
        content: String,
        phase: UserMessagePhase,
    },
    ToolCall {
        tool: String,
        args: serde_json::Value,
    },
    ToolResult {
        tool: String,
        summary: String,
    },
    MemoryEdit {
        op: String,
        block: String,
        detail: String,
    },
    Done {
        context: ContextSnapshot,
    },
    Failed {
        code: String,
        message: String,
    },
}

/// Parse and validate send_message tool args from raw JSON.
///
/// # Errors
///
/// Returns an error when the payload is not valid `SendMessageArgs` JSON.
pub fn parse_send_message_args(raw: &str) -> anyhow::Result<SendMessageArgs> {
    serde_json::from_str(raw).context("invalid send_message args")
}
