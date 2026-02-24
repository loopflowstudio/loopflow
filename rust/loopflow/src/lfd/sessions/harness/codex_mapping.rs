use serde_json::{json, Value};

use crate::lfd::sessions::types::{FileEdit, ItemDelta, ItemStatus, SessionItem, TurnStatus};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum ItemPhase {
    Started,
    Completed,
}

pub(super) fn extract_turn_id(params: &Value) -> Option<String> {
    params
        .get("turn")
        .and_then(|t| t.get("id"))
        .and_then(Value::as_str)
        .or_else(|| params.get("turnId").and_then(Value::as_str))
        .map(ToString::to_string)
}

pub(super) fn map_turn_status(params: &Value) -> TurnStatus {
    let turn_status = params
        .get("turn")
        .and_then(|t| t.get("status"))
        .and_then(Value::as_str)
        .or_else(|| params.get("status").and_then(Value::as_str))
        .unwrap_or_default();
    match turn_status {
        "interrupted" | "cancelled" => TurnStatus::Interrupted,
        "failed" | "error" => TurnStatus::Failed,
        _ => TurnStatus::Completed,
    }
}

pub(super) fn map_item_id(params: &Value) -> String {
    item_payload(params)
        .get("id")
        .and_then(Value::as_str)
        .or_else(|| params.get("itemId").and_then(Value::as_str))
        .or_else(|| params.get("id").and_then(Value::as_str))
        .unwrap_or("unknown")
        .to_string()
}

pub(super) fn build_item(params: &Value, phase: ItemPhase) -> SessionItem {
    let id = map_item_id(params);
    let item_type = map_item_type(params);
    let item = item_payload(params);
    let status = map_item_status(params);
    let completed = phase == ItemPhase::Completed;

    match item_type {
        "commandExecution" => SessionItem::Command {
            id,
            command: parse_command(item),
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
        "fileChange" => SessionItem::File {
            id,
            changes: parse_file_changes(item),
            status,
        },
        "mcpToolCall" => SessionItem::Tool {
            id,
            name: mcp_tool_name(item),
            status,
            input: Some(item.get("arguments").cloned().unwrap_or_else(|| json!({}))),
            output: if completed {
                text_field(item, "result").or_else(|| text_field(item, "error"))
            } else {
                None
            },
        },
        "agentMessage" => SessionItem::Message {
            id,
            text: required_text_field(item, "text"),
            phase: text_field(item, "phase"),
        },
        "plan" => SessionItem::Thought {
            id,
            text: required_text_field(item, "text"),
        },
        _ => SessionItem::Tool {
            id,
            name: item_type.to_string(),
            status,
            input: item.get("input").cloned(),
            output: if completed {
                text_field(item, "output")
            } else {
                None
            },
        },
    }
}

pub(super) fn map_item_delta(method: &str, params: &Value) -> Option<ItemDelta> {
    let content = text_content(params)?;
    match method {
        "item/commandExecution/outputDelta" | "item/fileChange/outputDelta" => {
            Some(ItemDelta::Output { content })
        }
        "item/plan/delta" => Some(ItemDelta::PlanText { content }),
        _ => None,
    }
}

pub(super) fn text_content(params: &Value) -> Option<String> {
    params
        .get("content")
        .and_then(Value::as_str)
        .or_else(|| params.get("delta").and_then(Value::as_str))
        .or_else(|| params.get("output").and_then(Value::as_str))
        .map(ToString::to_string)
}

fn item_payload(params: &Value) -> &Value {
    params.get("item").unwrap_or(params)
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

fn map_item_type(params: &Value) -> &str {
    item_payload(params)
        .get("type")
        .and_then(Value::as_str)
        .or_else(|| params.get("kind").and_then(Value::as_str))
        .unwrap_or("tool")
}

fn map_item_status(params: &Value) -> ItemStatus {
    let status = item_payload(params)
        .get("status")
        .and_then(Value::as_str)
        .or_else(|| params.get("status").and_then(Value::as_str))
        .unwrap_or("in_progress");
    match status {
        "completed" => ItemStatus::Completed,
        "failed" | "error" => ItemStatus::Failed,
        "declined" => ItemStatus::Declined,
        _ => ItemStatus::InProgress,
    }
}

fn parse_command(item: &Value) -> Vec<String> {
    item.get("command")
        .and_then(Value::as_array)
        .map(|arr| {
            arr.iter()
                .filter_map(Value::as_str)
                .map(ToString::to_string)
                .collect()
        })
        .unwrap_or_default()
}

fn mcp_tool_name(item: &Value) -> String {
    let tool = required_text_field(item, "tool");
    let server = text_field(item, "server").unwrap_or_default();
    if server.is_empty() {
        if tool.is_empty() {
            "mcp_tool_call".to_string()
        } else {
            tool
        }
    } else if tool.is_empty() {
        server
    } else {
        format!("{server}/{tool}")
    }
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
                    kind: c
                        .get("kind")
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
    use super::*;
    use crate::lfd::sessions::types::SessionEvent;

    #[test]
    fn build_item_maps_file_change_to_file() {
        let params = json!({
            "item": {
                "id": "item_1",
                "type": "fileChange",
                "status": "completed",
                "changes": [
                    {"path": "src/main.rs", "kind": "update", "diff": "-a\n+b"}
                ]
            }
        });

        let item = build_item(&params, ItemPhase::Completed);
        match item {
            SessionItem::File {
                id,
                changes,
                status,
            } => {
                assert_eq!(id, "item_1");
                assert_eq!(status, ItemStatus::Completed);
                assert_eq!(changes.len(), 1);
                assert_eq!(changes[0].path, "src/main.rs");
            }
            other => panic!("expected file item, got {other:?}"),
        }
    }

    #[test]
    fn build_item_maps_agent_message_to_message() {
        let params = json!({
            "item": {
                "id": "item_2",
                "type": "agentMessage",
                "text": "Done",
                "phase": "final"
            }
        });

        let item = build_item(&params, ItemPhase::Completed);
        match item {
            SessionItem::Message { id, text, phase } => {
                assert_eq!(id, "item_2");
                assert_eq!(text, "Done");
                assert_eq!(phase.as_deref(), Some("final"));
            }
            other => panic!("expected message item, got {other:?}"),
        }
    }

    #[test]
    fn build_item_maps_plan_to_thought() {
        let params = json!({
            "item": {
                "id": "item_3",
                "type": "plan",
                "text": "Run tests first"
            }
        });

        let item = build_item(&params, ItemPhase::Completed);
        match item {
            SessionItem::Thought { id, text } => {
                assert_eq!(id, "item_3");
                assert_eq!(text, "Run tests first");
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
                "result": "ok"
            }
        });

        let item = build_item(&params, ItemPhase::Completed);
        match item {
            SessionItem::Tool {
                id,
                name,
                status,
                input,
                output,
            } => {
                assert_eq!(id, "item_4");
                assert_eq!(name, "github/search");
                assert_eq!(status, ItemStatus::Completed);
                assert_eq!(input, Some(json!({ "query": "regression" })));
                assert_eq!(output.as_deref(), Some("ok"));
            }
            other => panic!("expected tool item, got {other:?}"),
        }
    }

    #[test]
    fn codex_trace_replay_keeps_item_id_stable_across_lifecycle() {
        let turn_started = SessionEvent::TurnStarted {
            turn_id: "turn_trace".to_string(),
        };
        let started_params = json!({
            "turn": { "id": "turn_trace" },
            "item": {
                "id": "item_trace_1",
                "type": "commandExecution",
                "status": "in_progress",
                "command": ["cargo", "test"],
                "cwd": "/repo"
            }
        });
        let item_started = SessionEvent::ItemStarted {
            turn_id: extract_turn_id(&started_params).expect("turn id"),
            item: build_item(&started_params, ItemPhase::Started),
        };
        let item_updated = SessionEvent::ItemUpdated {
            turn_id: "turn_trace".to_string(),
            item_id: map_item_id(&started_params),
            data: map_item_delta(
                "item/commandExecution/outputDelta",
                &json!({"content": "running tests..."}),
            )
            .expect("delta"),
        };
        let completed_params = json!({
            "turn": { "id": "turn_trace" },
            "item": {
                "id": "item_trace_1",
                "type": "commandExecution",
                "status": "completed",
                "command": ["cargo", "test"],
                "cwd": "/repo",
                "aggregatedOutput": "ok",
                "exitCode": 0
            }
        });
        let item_completed = SessionEvent::ItemCompleted {
            turn_id: extract_turn_id(&completed_params).expect("turn id"),
            item: build_item(&completed_params, ItemPhase::Completed),
        };
        let turn_completed = SessionEvent::TurnCompleted {
            turn_id: "turn_trace".to_string(),
            status: map_turn_status(&json!({"turn": {"status": "completed"}})),
        };

        let trace = vec![
            turn_started,
            item_started,
            item_updated,
            item_completed,
            turn_completed,
        ];

        let mut item_ids = Vec::new();
        for event in trace {
            match event {
                SessionEvent::ItemStarted { item, .. }
                | SessionEvent::ItemCompleted { item, .. } => {
                    if let SessionItem::Command { id, .. } = item {
                        item_ids.push(id);
                    }
                }
                SessionEvent::ItemUpdated { item_id, .. } => item_ids.push(item_id),
                _ => {}
            }
        }

        assert_eq!(
            item_ids,
            vec![
                "item_trace_1".to_string(),
                "item_trace_1".to_string(),
                "item_trace_1".to_string(),
            ]
        );
    }
}
