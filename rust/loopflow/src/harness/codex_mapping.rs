//! Mapping from codex app-server (codex-cli 0.142.5) wire shapes to
//! conversation events. Shapes verified against a live session and the
//! bundle from `codex app-server generate-json-schema` (v2).

use serde_json::Value;

use crate::chat::types::{ConversationItem, FileEdit, ItemDelta, Lifecycle, TurnUsage};
use crate::provider_account::RateLimitSignal;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum ItemPhase {
    Started,
    Completed,
}

/// Turn id from notification params: item/turn-scoped notifications carry a
/// top-level `turnId`; turn/started and turn/completed nest it as `turn.id`.
pub(super) fn extract_turn_id(params: &Value) -> Option<String> {
    params
        .get("turnId")
        .and_then(Value::as_str)
        .or_else(|| {
            params
                .get("turn")
                .and_then(|t| t.get("id"))
                .and_then(Value::as_str)
        })
        .map(ToString::to_string)
}

/// Vendor thread id from a `thread/started` notification's params or a
/// `thread/start` response's result — both nest it as `thread.id`.
pub(super) fn extract_thread_id(value: &Value) -> Option<String> {
    value
        .get("thread")
        .and_then(|t| t.get("id"))
        .and_then(Value::as_str)
        .map(ToString::to_string)
}

/// TurnStatus from turn/completed params: completed | interrupted | failed.
pub(super) fn map_turn_status(params: &Value) -> Lifecycle {
    match params
        .pointer("/turn/status")
        .and_then(Value::as_str)
        .unwrap_or_default()
    {
        "interrupted" => Lifecycle::Interrupted,
        "failed" => Lifecycle::Failed,
        _ => Lifecycle::Completed,
    }
}

/// Usage from thread/tokenUsage/updated params. `total` is cumulative for the
/// thread; the wave pipeline is turn-grained, so the latest snapshot before
/// turn/completed is what gets reported for the turn.
pub(super) fn map_token_usage(params: &Value) -> TurnUsage {
    let total = params.pointer("/tokenUsage/total");
    let last = params.pointer("/tokenUsage/last");
    let field = |key: &str| {
        total
            .and_then(|t| t.get(key))
            .and_then(Value::as_u64)
            .unwrap_or(0)
    };
    let peak_input_tokens = last
        .and_then(|usage| usage.get("inputTokens"))
        .and_then(Value::as_u64);
    TurnUsage {
        input_tokens: field("inputTokens"),
        output_tokens: field("outputTokens"),
        total_input_tokens: Some(field("inputTokens")),
        peak_input_tokens,
        context_window_tokens: params
            .pointer("/tokenUsage/modelContextWindow")
            .and_then(Value::as_u64),
        reasoning_tokens: Some(field("reasoningOutputTokens")),
        cache_read_tokens: Some(field("cachedInputTokens")),
        cache_write_tokens: None,
        model: None,
        cost_usd: None,
    }
}

/// Name a codex rate-limit window by its duration: anything under a day is
/// the rolling session window, the rest is the weekly cap.
pub(crate) fn window_name(duration_mins: Option<u64>) -> &'static str {
    match duration_mins {
        Some(mins) if mins < 1_440 => "session",
        _ => "weekly",
    }
}

fn limit_window(value: &Value, plan: Option<&str>) -> Option<crate::store::AccountLimitWindow> {
    let used_percent = value.get("usedPercent").and_then(Value::as_u64)?;
    Some(crate::store::AccountLimitWindow {
        window: window_name(value.get("windowDurationMins").and_then(Value::as_u64)).to_string(),
        used_percent: used_percent.min(100) as u8,
        resets_at: value
            .get("resetsAt")
            .and_then(Value::as_i64)
            .map(normalize_timestamp),
        plan: plan.map(str::to_string),
    })
}

fn normalize_timestamp(timestamp: i64) -> i64 {
    if timestamp > 100_000_000_000 {
        timestamp / 1000
    } else {
        timestamp
    }
}

pub(crate) fn rate_limit_signal(params: &Value) -> Option<RateLimitSignal> {
    let snapshot = params.get("rateLimits").unwrap_or(params);
    let reached = snapshot.get("rateLimitReachedType").and_then(Value::as_str);
    let plan = snapshot.get("planType").and_then(Value::as_str);
    let primary = snapshot.get("primary").filter(|value| !value.is_null());
    let secondary = snapshot.get("secondary").filter(|value| !value.is_null());
    let individual = snapshot
        .get("individualLimit")
        .filter(|value| !value.is_null());
    let windows = [primary, secondary]
        .into_iter()
        .flatten()
        .filter_map(|window| limit_window(window, plan))
        .collect::<Vec<_>>();
    let utilization_percent = [primary, secondary]
        .into_iter()
        .flatten()
        .filter_map(|window| window.get("usedPercent").and_then(Value::as_u64))
        .chain(
            individual
                .and_then(|limit| limit.get("remainingPercent"))
                .and_then(Value::as_u64)
                .map(|remaining| 100_u64.saturating_sub(remaining)),
        )
        .max()
        .map(|percent| percent.min(100) as u8);
    let resets_at = [primary, secondary, individual]
        .into_iter()
        .flatten()
        .filter_map(|window| window.get("resetsAt").and_then(Value::as_i64))
        .map(|timestamp| {
            if timestamp > 100_000_000_000 {
                timestamp / 1000
            } else {
                timestamp
            }
        })
        .max();
    if reached.is_none() && utilization_percent.is_none() && resets_at.is_none() {
        return None;
    }
    Some(RateLimitSignal {
        utilization_percent,
        resets_at,
        limited: reached.is_some(),
        reason: reached.unwrap_or("rate limit update").to_string(),
        windows,
    })
}

pub(super) fn map_item_id(params: &Value) -> String {
    params
        .pointer("/item/id")
        .and_then(Value::as_str)
        .or_else(|| params.get("itemId").and_then(Value::as_str))
        .unwrap_or("unknown")
        .to_string()
}

pub(super) fn map_item_type(params: &Value) -> &str {
    params
        .pointer("/item/type")
        .and_then(Value::as_str)
        .unwrap_or("unknown")
}

pub(super) fn build_item(params: &Value, phase: ItemPhase) -> ConversationItem {
    let id = map_item_id(params);
    let item_type = map_item_type(params);
    let item = params.get("item").unwrap_or(params);
    let status = map_item_status(item);
    let completed = phase == ItemPhase::Completed;

    match item_type {
        "commandExecution" => ConversationItem::Command {
            id,
            // 0.142.5 sends the raw command line as a single string.
            command: text_field(item, "command")
                .map(|c| vec![c])
                .unwrap_or_default(),
            cwd: required_text_field(item, "cwd"),
            status,
            output: if completed {
                text_field(item, "aggregatedOutput")
            } else {
                None
            },
            exit_code: if completed {
                item.get("exitCode")
                    .and_then(Value::as_i64)
                    .map(|v| v as i32)
            } else {
                None
            },
            duration_ms: if completed {
                item.get("durationMs").and_then(Value::as_u64)
            } else {
                None
            },
        },
        "fileChange" => ConversationItem::File {
            id,
            changes: parse_file_changes(item),
            status,
        },
        "mcpToolCall" => ConversationItem::Tool {
            id,
            name: mcp_tool_name(item),
            status,
            input: item.get("arguments").cloned(),
            output: if completed {
                mcp_tool_output(item)
            } else {
                None
            },
        },
        "agentMessage" => ConversationItem::Message {
            id,
            text: required_text_field(item, "text"),
            phase: text_field(item, "phase"),
        },
        "plan" => ConversationItem::Thought {
            id,
            text: required_text_field(item, "text"),
        },
        "reasoning" => ConversationItem::Thought {
            id,
            text: string_array(item, "summary").join("\n"),
        },
        _ => ConversationItem::Tool {
            id,
            name: item_type.to_string(),
            status,
            input: None,
            output: None,
        },
    }
}

pub(super) fn map_item_delta(method: &str, params: &Value) -> Option<ItemDelta> {
    let content = delta_content(params)?;
    match method {
        "item/commandExecution/outputDelta" | "item/fileChange/outputDelta" => {
            Some(ItemDelta::Output { content })
        }
        "item/plan/delta" => Some(ItemDelta::PlanText { content }),
        _ => None,
    }
}

/// Streaming text: every 0.142.5 delta notification carries a `delta` field.
pub(super) fn delta_content(params: &Value) -> Option<String> {
    params
        .get("delta")
        .and_then(Value::as_str)
        .map(ToString::to_string)
}

fn text_field(value: &Value, key: &str) -> Option<String> {
    value
        .get(key)
        .and_then(Value::as_str)
        .map(ToString::to_string)
}

fn required_text_field(value: &Value, key: &str) -> String {
    text_field(value, key).unwrap_or_default()
}

fn string_array(value: &Value, key: &str) -> Vec<String> {
    value
        .get(key)
        .and_then(Value::as_array)
        .map(|arr| {
            arr.iter()
                .filter_map(Value::as_str)
                .map(ToString::to_string)
                .collect()
        })
        .unwrap_or_default()
}

/// Item status enums (CommandExecutionStatus, PatchApplyStatus,
/// McpToolCallStatus): inProgress | completed | failed | declined.
fn map_item_status(item: &Value) -> Lifecycle {
    match item
        .get("status")
        .and_then(Value::as_str)
        .unwrap_or("inProgress")
    {
        "completed" => Lifecycle::Completed,
        "failed" => Lifecycle::Failed,
        // Declined tool calls map to Failed for now; Decisions will give
        // declined a real home on the wire.
        "declined" => Lifecycle::Failed,
        _ => Lifecycle::Running,
    }
}

fn mcp_tool_name(item: &Value) -> String {
    let tool = required_text_field(item, "tool");
    let server = required_text_field(item, "server");
    match (server.is_empty(), tool.is_empty()) {
        (true, true) => "mcp_tool_call".to_string(),
        (true, false) => tool,
        (false, true) => server,
        (false, false) => format!("{server}/{tool}"),
    }
}

/// McpToolCallResult is `{content: [MCP blocks]}`; join the text blocks.
/// McpToolCallError is `{message}`.
fn mcp_tool_output(item: &Value) -> Option<String> {
    if let Some(result) = item.get("result").filter(|v| !v.is_null()) {
        let texts: Vec<&str> = result
            .get("content")
            .and_then(Value::as_array)
            .map(|blocks| {
                blocks
                    .iter()
                    .filter_map(|b| b.get("text").and_then(Value::as_str))
                    .collect()
            })
            .unwrap_or_default();
        if texts.is_empty() {
            return Some(result.to_string());
        }
        return Some(texts.join("\n"));
    }
    item.pointer("/error/message")
        .and_then(Value::as_str)
        .map(ToString::to_string)
}

fn parse_file_changes(item: &Value) -> Vec<FileEdit> {
    item.get("changes")
        .and_then(Value::as_array)
        .map(|arr| {
            arr.iter()
                .map(|c| FileEdit {
                    path: c
                        .get("path")
                        .and_then(Value::as_str)
                        .unwrap_or_default()
                        .to_string(),
                    // PatchChangeKind is an object: {"type": "add"|"delete"|"update", ...}
                    kind: c
                        .pointer("/kind/type")
                        .and_then(Value::as_str)
                        .map(ToString::to_string),
                    diff: c
                        .get("diff")
                        .and_then(Value::as_str)
                        .map(ToString::to_string),
                })
                .collect()
        })
        .unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    #[test]
    fn build_item_maps_file_change_to_file() {
        let params = json!({
            "item": {
                "id": "item_1",
                "type": "fileChange",
                "status": "completed",
                "changes": [
                    {"path": "src/main.rs", "kind": {"type": "update", "move_path": null}, "diff": "-a\n+b"}
                ]
            }
        });

        let item = build_item(&params, ItemPhase::Completed);
        match item {
            ConversationItem::File {
                id,
                changes,
                status,
            } => {
                assert_eq!(id, "item_1");
                assert_eq!(status, Lifecycle::Completed);
                assert_eq!(changes.len(), 1);
                assert_eq!(changes[0].path, "src/main.rs");
                assert_eq!(changes[0].kind.as_deref(), Some("update"));
            }
            other => panic!("expected file item, got {other:?}"),
        }
    }

    #[test]
    fn build_item_maps_agent_message_to_message() {
        let params = json!({
            "item": {
                "id": "msg_1",
                "type": "agentMessage",
                "text": "Done",
                "phase": "final_answer",
                "memoryCitation": null
            }
        });

        let item = build_item(&params, ItemPhase::Completed);
        match item {
            ConversationItem::Message { id, text, phase } => {
                assert_eq!(id, "msg_1");
                assert_eq!(text, "Done");
                assert_eq!(phase.as_deref(), Some("final_answer"));
            }
            other => panic!("expected message item, got {other:?}"),
        }
    }

    #[test]
    fn build_item_maps_command_string_to_single_element_argv() {
        let params = json!({
            "item": {
                "id": "cmd_1",
                "type": "commandExecution",
                "command": "ls -la",
                "commandActions": [],
                "cwd": "/tmp",
                "status": "inProgress"
            }
        });

        let item = build_item(&params, ItemPhase::Started);
        match item {
            ConversationItem::Command {
                command,
                cwd,
                status,
                ..
            } => {
                assert_eq!(command, vec!["ls -la".to_string()]);
                assert_eq!(cwd, "/tmp");
                assert_eq!(status, Lifecycle::Running);
            }
            other => panic!("expected command item, got {other:?}"),
        }
    }

    #[test]
    fn build_item_maps_reasoning_to_thought() {
        let params = json!({
            "item": {
                "id": "rsn_1",
                "type": "reasoning",
                "summary": ["Check tests", "Then land"]
            }
        });

        let item = build_item(&params, ItemPhase::Completed);
        match item {
            ConversationItem::Thought { id, text } => {
                assert_eq!(id, "rsn_1");
                assert_eq!(text, "Check tests\nThen land");
            }
            other => panic!("expected thought item, got {other:?}"),
        }
    }

    #[test]
    fn build_item_maps_mcp_tool_call_to_generic_tool() {
        let params = json!({
            "item": {
                "id": "item_4",
                "type": "mcpToolCall",
                "status": "completed",
                "server": "github",
                "tool": "search",
                "arguments": { "query": "regression" },
                "result": { "content": [{"type": "text", "text": "ok"}] }
            }
        });

        let item = build_item(&params, ItemPhase::Completed);
        match item {
            ConversationItem::Tool {
                id,
                name,
                status,
                input,
                output,
            } => {
                assert_eq!(id, "item_4");
                assert_eq!(name, "github/search");
                assert_eq!(status, Lifecycle::Completed);
                assert_eq!(input, Some(json!({ "query": "regression" })));
                assert_eq!(output.as_deref(), Some("ok"));
            }
            other => panic!("expected tool item, got {other:?}"),
        }
    }

    #[test]
    fn map_token_usage_reads_total_breakdown() {
        let usage = map_token_usage(&json!({
            "threadId": "t",
            "turnId": "u",
            "tokenUsage": {
                "total": {
                    "totalTokens": 16070,
                    "inputTokens": 16065,
                    "cachedInputTokens": 9600,
                    "outputTokens": 5,
                    "reasoningOutputTokens": 0
                },
                "last": {
                    "totalTokens": 16070,
                    "inputTokens": 16065,
                    "cachedInputTokens": 9600,
                    "outputTokens": 5,
                    "reasoningOutputTokens": 0
                },
                "modelContextWindow": 258400
            }
        }));

        assert_eq!(usage.input_tokens, 16065);
        assert_eq!(usage.total_input_tokens, Some(16065));
        assert_eq!(usage.peak_input_tokens, Some(16065));
        assert_eq!(usage.context_window_tokens, Some(258400));
        assert_eq!(usage.output_tokens, 5);
        assert_eq!(usage.reasoning_tokens, Some(0));
        assert_eq!(usage.cache_read_tokens, Some(9600));
    }

    #[test]
    fn rate_limit_snapshot_maps_reached_type_and_latest_reset() {
        let signal = rate_limit_signal(&json!({
            "rateLimits": {
                "primary": {"usedPercent": 40, "resetsAt": 1900000000},
                "secondary": {"usedPercent": 80, "resetsAt": 1900000300},
                "rateLimitReachedType": "rate_limit_reached"
            }
        }))
        .unwrap();

        assert!(signal.limited);
        assert_eq!(signal.utilization_percent, Some(80));
        assert_eq!(signal.resets_at, Some(1_900_000_300));
        assert_eq!(signal.reason, "rate_limit_reached");
    }

    #[test]
    fn sparse_rate_limit_snapshot_is_ignored() {
        assert!(rate_limit_signal(&json!({"rateLimits": {}})).is_none());
    }
}
