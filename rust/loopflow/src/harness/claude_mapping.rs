use std::collections::HashMap;

use serde_json::Value;
use tokio::sync::mpsc;

use crate::chat::types::{ConversationEvent, ConversationItem, FileEdit, Lifecycle, TurnUsage};
use crate::harness::lf_tag::LfTagParser;
use crate::provider_account::RateLimitSignal;

/// Reader-local state for tracking in-flight content blocks.
#[derive(Debug, Default)]
pub(super) struct ReaderState {
    /// Map block index -> tool use state for streaming deltas.
    tool_blocks: HashMap<usize, ToolUseState>,
    /// Map tool_use_id -> block index for direct completion lookup.
    tool_indexes: HashMap<String, usize>,
    /// Streaming parser for `<lf:...>` tagged payloads.
    tag_parser: LfTagParser,
    /// Vendor session id from the turn's `system` event; the harness drains
    /// it for `--resume` and persistence.
    provider_session_id: Option<String>,
}

#[derive(Debug, Default)]
struct ToolUseState {
    id: String,
    name: String,
    input_json: String,
    input: Option<Value>,
}

impl ToolUseState {
    fn parsed_input(&self) -> Option<Value> {
        if self.input_json.is_empty() {
            return self.input.clone();
        }

        serde_json::from_str(&self.input_json)
            .ok()
            .or_else(|| self.input.clone())
    }
}

impl ReaderState {
    fn track_tool(
        &mut self,
        index: usize,
        tool_id: &str,
        tool_name: &str,
        input: Option<Value>,
    ) -> bool {
        if let Some(existing_index) = self.tool_indexes.get(tool_id).copied() {
            if let Some(existing) = self.tool_blocks.get_mut(&existing_index) {
                if existing.input.is_none() {
                    existing.input = input;
                }
            }
            return false;
        }

        self.tool_indexes.insert(tool_id.to_string(), index);
        self.tool_blocks.insert(
            index,
            ToolUseState {
                id: tool_id.to_string(),
                name: tool_name.to_string(),
                input_json: String::new(),
                input,
            },
        );
        true
    }

    fn append_input_json(&mut self, index: usize, partial_json: &str) {
        if let Some(tool) = self.tool_blocks.get_mut(&index) {
            tool.input_json.push_str(partial_json);
        }
    }

    fn tool_by_id(&self, tool_use_id: &str) -> Option<&ToolUseState> {
        let index = self.tool_indexes.get(tool_use_id)?;
        self.tool_blocks.get(index)
    }

    /// Drain tools still in flight when the turn ended without a result,
    /// stamped with the turn's terminal status (Failed for a crash,
    /// Interrupted for a kill we asked for).
    pub(super) fn drain_open_items(&mut self, status: Lifecycle) -> Vec<ConversationItem> {
        let mut tools: Vec<_> = self.tool_blocks.drain().map(|(_, tool)| tool).collect();
        self.tool_indexes.clear();
        tools.sort_by(|left, right| left.id.cmp(&right.id));
        tools
            .into_iter()
            .map(|tool| build_item(&tool.name, &tool.id, tool.parsed_input(), status, None))
            .collect()
    }

    pub(super) fn take_provider_session_id(&mut self) -> Option<String> {
        self.provider_session_id.take()
    }
}

pub(super) fn rate_limit_signal(line: &str) -> Option<RateLimitSignal> {
    let value = serde_json::from_str::<Value>(line).ok()?;
    let event = value
        .get("stream_event")
        .and_then(|stream| stream.get("event"))
        .unwrap_or(&value);
    if event.get("type").and_then(Value::as_str)? != "rate_limit_event" {
        return None;
    }
    let info = event
        .get("rate_limit_info")
        .or_else(|| event.get("rateLimitInfo"))
        .or_else(|| event.get("rate_limit"))
        .unwrap_or(event);
    let status = info
        .get("status")
        .and_then(Value::as_str)
        .unwrap_or("rate_limit_event");
    let limited = matches!(
        status.to_ascii_lowercase().as_str(),
        "rejected" | "blocked" | "limited" | "rate_limit_reached"
    );
    let utilization_percent = info
        .get("utilization")
        .or_else(|| info.get("used_percent"))
        .or_else(|| info.get("usedPercent"))
        .and_then(Value::as_f64)
        .map(|value| {
            let percent = if value <= 1.0 { value * 100.0 } else { value };
            percent.round().clamp(0.0, 100.0) as u8
        });
    let resets_at = info
        .get("resets_at")
        .or_else(|| info.get("resetsAt"))
        .or_else(|| info.get("reset_at"))
        .or_else(|| info.get("reset_timestamp"))
        .and_then(parse_reset_timestamp);
    let rate_type = info
        .get("rate_limit_type")
        .or_else(|| info.get("rateLimitType"))
        .and_then(Value::as_str);
    let reason = rate_type
        .map(|rate_type| format!("{status}: {rate_type}"))
        .unwrap_or_else(|| status.to_string());
    Some(RateLimitSignal {
        utilization_percent,
        resets_at,
        limited,
        reason,
    })
}

fn parse_reset_timestamp(value: &Value) -> Option<i64> {
    if let Some(value) = value.as_i64() {
        return Some(if value > 100_000_000_000 {
            value / 1000
        } else {
            value
        });
    }
    let value = value.as_str()?;
    if let Ok(timestamp) = value.parse::<i64>() {
        return Some(if timestamp > 100_000_000_000 {
            timestamp / 1000
        } else {
            timestamp
        });
    }
    time::OffsetDateTime::parse(value, &time::format_description::well_known::Rfc3339)
        .ok()
        .map(|timestamp| timestamp.unix_timestamp())
}

/// Parse a single NDJSON line and emit conversation events.
pub(super) fn process_line(
    line: &str,
    turn_id: &str,
    events: &mpsc::UnboundedSender<ConversationEvent>,
    state: &mut ReaderState,
) -> bool {
    let Ok(value) = serde_json::from_str::<Value>(line) else {
        return false;
    };
    let event = value
        .get("stream_event")
        .and_then(|stream| stream.get("event"))
        .unwrap_or(&value);

    let Some(event_type) = event.get("type").and_then(Value::as_str) else {
        return false;
    };

    match event_type {
        "system" => {
            if let Some(session_id) = event
                .get("session_id")
                .or_else(|| value.get("session_id"))
                .and_then(Value::as_str)
            {
                state.provider_session_id = Some(session_id.to_string());
            }
        }

        "content_block_start" => {
            let Some(index) = event.get("index").and_then(Value::as_u64) else {
                return false;
            };
            let Some(block) = event.get("content_block") else {
                return false;
            };
            let Some(block_type) = block.get("type").and_then(Value::as_str) else {
                return false;
            };
            let index = index as usize;

            if block_type == "tool_use" {
                let Some(tool_id) = block.get("id").and_then(Value::as_str) else {
                    return false;
                };
                let Some(tool_name) = block.get("name").and_then(Value::as_str) else {
                    return false;
                };
                if state.track_tool(index, tool_id, tool_name, None) {
                    let item = infer_item(tool_name, tool_id, None);
                    let _ = events.send(ConversationEvent::ItemStarted {
                        turn_id: turn_id.to_string(),
                        item,
                    });
                }
            }
        }

        "content_block_delta" => {
            let index = event
                .get("index")
                .and_then(Value::as_u64)
                .map(|v| v as usize);
            let Some(delta) = event.get("delta") else {
                return false;
            };
            let Some(delta_type) = delta.get("type").and_then(Value::as_str) else {
                return false;
            };

            match delta_type {
                "text_delta" => {
                    if let Some(text) = delta.get("text").and_then(Value::as_str) {
                        emit_text_delta(events, state, turn_id, text);
                    }
                }
                "thinking_delta" | "summary_delta" => {
                    if let Some(text) = delta
                        .get("thinking")
                        .or_else(|| delta.get("summary"))
                        .and_then(Value::as_str)
                    {
                        let _ = events.send(ConversationEvent::ReasoningDelta {
                            turn_id: turn_id.to_string(),
                            content: text.to_string(),
                        });
                    }
                }
                "input_json_delta" => {
                    if let (Some(idx), Some(json_chunk)) =
                        (index, delta.get("partial_json").and_then(Value::as_str))
                    {
                        state.append_input_json(idx, json_chunk);
                    }
                }
                _ => {}
            }
        }

        "content_block_stop" => {
            // Tool blocks will get completed when we see the tool result.
            // Text blocks don't need explicit completion.
        }

        "result" => {
            flush_text_delta_parser(events, state, turn_id);
            let usage = map_turn_usage(event);
            let status = if event
                .get("is_error")
                .and_then(Value::as_bool)
                .unwrap_or(false)
            {
                Lifecycle::Failed
            } else {
                Lifecycle::Completed
            };
            let _ = events.send(ConversationEvent::TurnCompleted {
                turn_id: turn_id.to_string(),
                status,
            });
            let _ = events.send(ConversationEvent::TurnUsage {
                turn_id: turn_id.to_string(),
                usage,
            });
            return true;
        }

        // Claude emits "assistant" events with full message content.
        // These contain tool_use blocks and text blocks.
        "assistant" => {
            process_assistant_message(event, turn_id, events, state);
        }

        // Tool result events: type "user" with tool_result content.
        "user" => {
            process_user_message(event, turn_id, events, state);
        }

        _ => {}
    }

    false
}

fn map_turn_usage(event: &Value) -> TurnUsage {
    TurnUsage {
        input_tokens: event
            .pointer("/usage/input_tokens")
            .and_then(Value::as_u64)
            .unwrap_or(0),
        output_tokens: event
            .pointer("/usage/output_tokens")
            .and_then(Value::as_u64)
            .unwrap_or(0),
        reasoning_tokens: event
            .pointer("/usage/reasoning_tokens")
            .and_then(Value::as_u64),
        cache_read_tokens: event
            .pointer("/usage/cache_read_input_tokens")
            .and_then(Value::as_u64),
        cache_write_tokens: event
            .pointer("/usage/cache_creation_input_tokens")
            .and_then(Value::as_u64),
        model: event
            .get("model")
            .and_then(Value::as_str)
            .map(ToString::to_string),
        cost_usd: event
            .get("cost_usd")
            .or_else(|| event.get("total_cost_usd"))
            .and_then(Value::as_f64),
    }
}

/// Map Claude tool name to a ConversationItem category.
fn infer_item(tool_name: &str, tool_use_id: &str, input: Option<Value>) -> ConversationItem {
    build_item(tool_name, tool_use_id, input, Lifecycle::Running, None)
}

fn build_item(
    tool_name: &str,
    tool_use_id: &str,
    input: Option<Value>,
    status: Lifecycle,
    output: Option<String>,
) -> ConversationItem {
    match tool_name {
        "Bash" => ConversationItem::Command {
            id: tool_use_id.to_string(),
            command: command_from_input(input.as_ref()),
            cwd: String::new(),
            status,
            output,
            exit_code: None,
            duration_ms: None,
        },
        "Edit" | "Write" | "NotebookEdit" => ConversationItem::File {
            id: tool_use_id.to_string(),
            changes: file_changes_from_input(tool_name, input.as_ref()),
            status,
        },
        _ => ConversationItem::Tool {
            id: tool_use_id.to_string(),
            name: tool_name.to_string(),
            status,
            input,
            output,
        },
    }
}

fn command_from_input(input: Option<&Value>) -> Vec<String> {
    let command = input
        .and_then(|value| value.get("command"))
        .and_then(Value::as_str)
        .unwrap_or_default();
    if command.is_empty() {
        Vec::new()
    } else {
        vec![command.to_string()]
    }
}

fn file_changes_from_input(tool_name: &str, input: Option<&Value>) -> Vec<FileEdit> {
    let Some(input) = input else {
        return Vec::new();
    };
    let path = input
        .get("file_path")
        .or_else(|| input.get("notebook_path"))
        .and_then(Value::as_str)
        .unwrap_or_default();
    if path.is_empty() {
        return Vec::new();
    }

    let diff = if tool_name == "Edit" {
        synthesize_edit_diff(path, input)
    } else {
        None
    };

    vec![FileEdit {
        path: path.to_string(),
        kind: Some(tool_name.to_lowercase()),
        diff,
    }]
}

fn synthesize_edit_diff(path: &str, input: &Value) -> Option<String> {
    let old = input.get("old_string").and_then(Value::as_str)?;
    let new = input.get("new_string").and_then(Value::as_str)?;
    if old == new {
        return None;
    }

    let mut lines = Vec::new();
    lines.push(format!("--- a/{path}"));
    lines.push(format!("+++ b/{path}"));
    for line in old.lines() {
        lines.push(format!("-{line}"));
    }
    for line in new.lines() {
        lines.push(format!("+{line}"));
    }
    Some(lines.join("\n"))
}

fn process_assistant_message(
    value: &Value,
    turn_id: &str,
    events: &mpsc::UnboundedSender<ConversationEvent>,
    state: &mut ReaderState,
) {
    let Some(blocks) = message_content(value) else {
        return;
    };

    for (idx, block) in blocks.iter().enumerate() {
        let Some(block_type) = block.get("type").and_then(Value::as_str) else {
            continue;
        };
        match block_type {
            "tool_use" => {
                let tool_id = block
                    .get("id")
                    .and_then(Value::as_str)
                    .unwrap_or_default()
                    .to_string();
                let tool_name = block
                    .get("name")
                    .and_then(Value::as_str)
                    .unwrap_or_default()
                    .to_string();
                let input = block.get("input").cloned();

                // If we haven't already emitted ItemStarted via streaming,
                // emit both started and track it.
                if state.track_tool(idx, &tool_id, &tool_name, input.clone()) {
                    let item = infer_item(&tool_name, &tool_id, input);
                    let _ = events.send(ConversationEvent::ItemStarted {
                        turn_id: turn_id.to_string(),
                        item,
                    });
                }
            }
            "text" => {
                if let Some(text) = block.get("text").and_then(Value::as_str) {
                    if !text.is_empty() {
                        emit_text_delta(events, state, turn_id, text);
                    }
                }
            }
            _ => {}
        }
    }
}

fn process_user_message(
    value: &Value,
    turn_id: &str,
    events: &mpsc::UnboundedSender<ConversationEvent>,
    state: &mut ReaderState,
) {
    let Some(blocks) = message_content(value) else {
        return;
    };

    for block in blocks {
        let Some(block_type) = block.get("type").and_then(Value::as_str) else {
            continue;
        };
        match block_type {
            "tool_result" => {
                let tool_use_id = block
                    .get("tool_use_id")
                    .and_then(Value::as_str)
                    .unwrap_or_default();

                let Some(tool) = state.tool_by_id(tool_use_id) else {
                    continue;
                };

                // Extract output from content array or text.
                let output = extract_tool_result_text(block);
                let input = tool.parsed_input();
                let completed_item =
                    build_item(&tool.name, &tool.id, input, Lifecycle::Completed, output);
                let _ = events.send(ConversationEvent::ItemCompleted {
                    turn_id: turn_id.to_string(),
                    item: completed_item,
                });
            }
            // Text blocks in user messages are Claude echoing back input the
            // client already displayed. Emitting them would duplicate messages.
            "text" => {}
            _ => {}
        }
    }
}

fn message_content(value: &Value) -> Option<&[Value]> {
    value
        .get("message")
        .and_then(|m| m.get("content"))
        .and_then(Value::as_array)
        .map(Vec::as_slice)
}

fn extract_tool_result_text(block: &Value) -> Option<String> {
    // Try nested content array first.
    if let Some(content) = block.get("content").and_then(Value::as_array) {
        let texts: Vec<&str> = content
            .iter()
            .filter_map(|c| {
                if c.get("type").and_then(Value::as_str) == Some("text") {
                    c.get("text").and_then(Value::as_str)
                } else {
                    None
                }
            })
            .collect();
        if !texts.is_empty() {
            return Some(texts.join("\n"));
        }
    }
    // Try direct text field.
    block
        .get("content")
        .and_then(Value::as_str)
        .map(String::from)
}

fn emit_text_delta(
    events: &mpsc::UnboundedSender<ConversationEvent>,
    state: &mut ReaderState,
    turn_id: &str,
    content: &str,
) {
    for event in state.tag_parser.consume_text(turn_id, content) {
        let _ = events.send(event);
    }
}

fn flush_text_delta_parser(
    events: &mpsc::UnboundedSender<ConversationEvent>,
    state: &mut ReaderState,
    turn_id: &str,
) {
    for event in state.tag_parser.finish_turn(turn_id) {
        let _ = events.send(event);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn infer_item_bash_to_command() {
        let item = infer_item("Bash", "tu_1", Some(json!({"command": "cargo test"})));
        match item {
            ConversationItem::Command { id, command, .. } => {
                assert_eq!(id, "tu_1");
                assert_eq!(command, vec!["cargo test"]);
            }
            other => panic!("expected Command, got {other:?}"),
        }
    }

    #[test]
    fn infer_item_edit_to_file() {
        let item = infer_item("Edit", "tu_2", Some(json!({"file_path": "src/main.rs"})));
        match item {
            ConversationItem::File { id, changes, .. } => {
                assert_eq!(id, "tu_2");
                assert_eq!(changes[0].path, "src/main.rs");
                assert_eq!(changes[0].kind.as_deref(), Some("edit"));
            }
            other => panic!("expected File, got {other:?}"),
        }
    }

    #[test]
    fn infer_item_write_to_file() {
        let item = infer_item("Write", "tu_3", Some(json!({"file_path": "new.txt"})));
        match item {
            ConversationItem::File { id, changes, .. } => {
                assert_eq!(id, "tu_3");
                assert_eq!(changes[0].path, "new.txt");
                assert_eq!(changes[0].kind.as_deref(), Some("write"));
            }
            other => panic!("expected File, got {other:?}"),
        }
    }

    #[test]
    fn infer_item_notebook_edit_to_file() {
        let item = infer_item(
            "NotebookEdit",
            "tu_4",
            Some(json!({"notebook_path": "analysis.ipynb"})),
        );
        match item {
            ConversationItem::File { id, changes, .. } => {
                assert_eq!(id, "tu_4");
                assert_eq!(changes[0].path, "analysis.ipynb");
            }
            other => panic!("expected File, got {other:?}"),
        }
    }

    #[test]
    fn infer_item_unknown_to_tool() {
        let item = infer_item("WebSearch", "tu_5", Some(json!({"query": "rust async"})));
        match item {
            ConversationItem::Tool {
                id, name, input, ..
            } => {
                assert_eq!(id, "tu_5");
                assert_eq!(name, "WebSearch");
                assert_eq!(input, Some(json!({"query": "rust async"})));
            }
            other => panic!("expected Tool, got {other:?}"),
        }
    }

    #[test]
    fn process_line_system_event_captures_session_id() {
        let (tx, rx) = mpsc::unbounded_channel();
        let mut state = ReaderState::default();
        let line = r#"{"type":"system","session_id":"sess_abc123","tools":[]}"#;

        let result = process_line(line, "turn_1", &tx, &mut state);
        assert!(!result);
        assert!(rx.is_empty(), "system event should not emit events");
        assert_eq!(
            state.take_provider_session_id().as_deref(),
            Some("sess_abc123")
        );
        assert!(state.take_provider_session_id().is_none(), "take drains");
    }

    #[test]
    fn process_line_wrapped_stream_event_captures_session_id() {
        let (tx, rx) = mpsc::unbounded_channel();
        let mut state = ReaderState::default();
        let line = r#"{"stream_event":{"event":{"type":"system","session_id":"sess_wrapped"}}}"#;

        let result = process_line(line, "turn_1", &tx, &mut state);
        assert!(!result);
        assert!(rx.is_empty(), "system event should not emit events");
        assert_eq!(
            state.take_provider_session_id().as_deref(),
            Some("sess_wrapped")
        );
    }

    #[test]
    fn rejected_rate_limit_event_marks_a_hard_limit() {
        let signal = rate_limit_signal(
            r#"{"type":"rate_limit_event","rate_limit_info":{"status":"rejected","rate_limit_type":"five_hour","utilization":1.0,"resets_at":1900000000}}"#,
        )
        .unwrap();

        assert!(signal.limited);
        assert_eq!(signal.utilization_percent, Some(100));
        assert_eq!(signal.resets_at, Some(1_900_000_000));
        assert!(signal.reason.contains("five_hour"));
    }

    #[test]
    fn rate_limit_warning_updates_utilization_without_limiting() {
        let signal = rate_limit_signal(
            r#"{"stream_event":{"event":{"type":"rate_limit_event","rate_limit_info":{"status":"allowed_warning","utilization":0.72}}}}"#,
        )
        .unwrap();

        assert!(!signal.limited);
        assert_eq!(signal.utilization_percent, Some(72));
    }

    #[test]
    fn process_line_text_delta() {
        let (tx, mut rx) = mpsc::unbounded_channel();
        let mut state = ReaderState::default();
        let line = r#"{"type":"content_block_delta","index":0,"delta":{"type":"text_delta","text":"Hello world"}}"#;

        process_line(line, "turn_1", &tx, &mut state);

        let event = rx.try_recv().expect("should have event");
        match event {
            ConversationEvent::TextDelta { turn_id, content } => {
                assert_eq!(turn_id, "turn_1");
                assert_eq!(content, "Hello world");
            }
            other => panic!("expected TextDelta, got {other:?}"),
        }
    }

    #[test]
    fn process_line_text_delta_emits_suggested_actions_event() {
        let (tx, mut rx) = mpsc::unbounded_channel();
        let mut state = ReaderState::default();
        let line = r#"{"type":"content_block_delta","index":0,"delta":{"type":"text_delta","text":"<lf:suggest_actions>[{\"label\":\"Land PR\"}]</lf:suggest_actions>"}}"#;

        process_line(line, "turn_1", &tx, &mut state);

        let event = rx.try_recv().expect("should have event");
        match event {
            ConversationEvent::SuggestedActions { turn_id, actions } => {
                assert_eq!(turn_id, "turn_1");
                assert_eq!(actions.len(), 1);
                assert_eq!(actions[0].label, "Land PR");
            }
            other => panic!("expected SuggestedActions, got {other:?}"),
        }
    }

    #[test]
    fn process_line_thinking_delta() {
        let (tx, mut rx) = mpsc::unbounded_channel();
        let mut state = ReaderState::default();
        let line = r#"{"type":"content_block_delta","index":0,"delta":{"type":"thinking_delta","thinking":"Let me think..."}}"#;

        process_line(line, "turn_1", &tx, &mut state);

        let event = rx.try_recv().expect("should have event");
        match event {
            ConversationEvent::ReasoningDelta { turn_id, content } => {
                assert_eq!(turn_id, "turn_1");
                assert_eq!(content, "Let me think...");
            }
            other => panic!("expected ReasoningDelta, got {other:?}"),
        }
    }

    #[test]
    fn process_line_result_completes_turn() {
        let (tx, mut rx) = mpsc::unbounded_channel();
        let mut state = ReaderState::default();
        let line = r#"{"type":"result","duration_ms":1234,"cost_usd":0.01,"is_error":false,"result":"done","session_id":"sess_abc"}"#;

        let result = process_line(line, "turn_1", &tx, &mut state);
        assert!(result);

        let event = rx.try_recv().expect("should have event");
        match event {
            ConversationEvent::TurnCompleted { turn_id, status } => {
                assert_eq!(turn_id, "turn_1");
                assert_eq!(status, Lifecycle::Completed);
            }
            other => panic!("expected TurnCompleted, got {other:?}"),
        }

        let usage_event = rx.try_recv().expect("should have usage event");
        match usage_event {
            ConversationEvent::TurnUsage { turn_id, usage } => {
                assert_eq!(turn_id, "turn_1");
                assert_eq!(usage.input_tokens, 0);
                assert_eq!(usage.output_tokens, 0);
                assert_eq!(usage.cost_usd, Some(0.01));
            }
            other => panic!("expected TurnUsage, got {other:?}"),
        }
    }

    #[test]
    fn process_line_result_error_marks_failed() {
        let (tx, mut rx) = mpsc::unbounded_channel();
        let mut state = ReaderState::default();
        let line = r#"{"type":"result","is_error":true,"result":"failed"}"#;

        let result = process_line(line, "turn_1", &tx, &mut state);
        assert!(result);

        let event = rx.try_recv().expect("should have event");
        match event {
            ConversationEvent::TurnCompleted { turn_id, status } => {
                assert_eq!(turn_id, "turn_1");
                assert_eq!(status, Lifecycle::Failed);
            }
            other => panic!("expected TurnCompleted, got {other:?}"),
        }
    }

    #[test]
    fn process_line_result_extracts_turn_usage_tokens() {
        let (tx, mut rx) = mpsc::unbounded_channel();
        let mut state = ReaderState::default();
        let line = r#"{"type":"result","is_error":false,"model":"claude-sonnet-4","total_cost_usd":0.2,"usage":{"input_tokens":321,"output_tokens":123,"reasoning_tokens":9,"cache_read_input_tokens":11,"cache_creation_input_tokens":5}}"#;

        let result = process_line(line, "turn_42", &tx, &mut state);
        assert!(result);

        let _completed = rx.try_recv().expect("completion event");
        let usage_event = rx.try_recv().expect("usage event");
        match usage_event {
            ConversationEvent::TurnUsage { turn_id, usage } => {
                assert_eq!(turn_id, "turn_42");
                assert_eq!(usage.input_tokens, 321);
                assert_eq!(usage.output_tokens, 123);
                assert_eq!(usage.reasoning_tokens, Some(9));
                assert_eq!(usage.cache_read_tokens, Some(11));
                assert_eq!(usage.cache_write_tokens, Some(5));
                assert_eq!(usage.model.as_deref(), Some("claude-sonnet-4"));
                assert_eq!(usage.cost_usd, Some(0.2));
            }
            other => panic!("expected TurnUsage, got {other:?}"),
        }
    }

    #[test]
    fn process_line_tool_use_start() {
        let (tx, mut rx) = mpsc::unbounded_channel();
        let mut state = ReaderState::default();
        let line = r#"{"type":"content_block_start","index":1,"content_block":{"type":"tool_use","id":"tu_bash_1","name":"Bash"}}"#;

        process_line(line, "turn_1", &tx, &mut state);

        assert!(state.tool_blocks.contains_key(&1));
        let tool = &state.tool_blocks[&1];
        assert_eq!(tool.id, "tu_bash_1");
        assert_eq!(tool.name, "Bash");
        assert_eq!(state.tool_indexes.get("tu_bash_1"), Some(&1));

        let event = rx.try_recv().expect("should have event");
        match event {
            ConversationEvent::ItemStarted { turn_id, item } => {
                assert_eq!(turn_id, "turn_1");
                assert!(matches!(item, ConversationItem::Command { .. }));
            }
            other => panic!("expected ItemStarted, got {other:?}"),
        }
    }

    #[test]
    fn process_line_input_json_delta_accumulates() {
        let (tx, _rx) = mpsc::unbounded_channel();
        let mut state = ReaderState::default();

        // Start a tool block first.
        let start_line = r#"{"type":"content_block_start","index":0,"content_block":{"type":"tool_use","id":"tu_1","name":"Bash"}}"#;
        process_line(start_line, "turn_1", &tx, &mut state);

        // Send partial input.
        let delta1 = r#"{"type":"content_block_delta","index":0,"delta":{"type":"input_json_delta","partial_json":"{\"com"}}"#;
        let delta2 = r#"{"type":"content_block_delta","index":0,"delta":{"type":"input_json_delta","partial_json":"mand\":\"ls\"}"}}"#;
        process_line(delta1, "turn_1", &tx, &mut state);
        process_line(delta2, "turn_1", &tx, &mut state);

        assert_eq!(
            state
                .tool_blocks
                .get(&0)
                .map(|tool| tool.input_json.as_str()),
            Some("{\"command\":\"ls\"}")
        );
    }

    #[test]
    fn process_line_user_text_is_dropped() {
        let (tx, rx) = mpsc::unbounded_channel();
        let mut state = ReaderState::default();
        // Text blocks in user messages are Claude echoing back input the
        // client already displayed — emitting them would duplicate messages.
        let line = r#"{"type":"user","message":{"content":[{"type":"text","text":"Design the architecture before coding."}]}}"#;

        process_line(line, "turn_1", &tx, &mut state);

        assert!(rx.is_empty(), "user text blocks should not emit events");
    }

    #[test]
    fn synthesize_edit_diff_formats_unified_lines() {
        let input = json!({
            "old_string": "fn old() {}",
            "new_string": "fn new() {}"
        });

        let diff = synthesize_edit_diff("src/main.rs", &input).expect("expected diff");

        assert_eq!(
            diff,
            "--- a/src/main.rs\n+++ b/src/main.rs\n-fn old() {}\n+fn new() {}"
        );
    }

    #[test]
    fn synthesize_edit_diff_returns_none_for_empty_or_equal_input() {
        let equal_input = json!({
            "old_string": "same",
            "new_string": "same"
        });
        assert!(synthesize_edit_diff("src/main.rs", &equal_input).is_none());

        let empty_input = json!({
            "old_string": "",
            "new_string": ""
        });
        assert!(synthesize_edit_diff("src/main.rs", &empty_input).is_none());
    }

    #[test]
    fn edit_tool_synthesizes_diff() {
        let item = infer_item(
            "Edit",
            "tu_10",
            Some(json!({
                "file_path": "src/main.rs",
                "old_string": "fn old() {}",
                "new_string": "fn new() {}"
            })),
        );
        match item {
            ConversationItem::File { changes, .. } => {
                let diff = changes[0].diff.as_deref().unwrap();
                assert!(diff.contains("--- a/src/main.rs"));
                assert!(diff.contains("+++ b/src/main.rs"));
                assert!(diff.contains("-fn old() {}"));
                assert!(diff.contains("+fn new() {}"));
            }
            other => panic!("expected File, got {other:?}"),
        }
    }

    #[test]
    fn write_tool_no_diff() {
        let item = infer_item(
            "Write",
            "tu_13",
            Some(json!({
                "file_path": "new.txt",
                "content": "hello world"
            })),
        );
        match item {
            ConversationItem::File { changes, .. } => {
                assert!(changes[0].diff.is_none());
            }
            other => panic!("expected File, got {other:?}"),
        }
    }

    #[test]
    fn synthesize_edit_diff_multiline() {
        let input = json!({
            "old_string": "fn old() {\n    println!(\"hi\");\n}",
            "new_string": "fn new() {\n    println!(\"hello\");\n    println!(\"world\");\n}"
        });

        let diff = synthesize_edit_diff("src/lib.rs", &input).expect("expected diff");
        let lines: Vec<&str> = diff.lines().collect();

        assert_eq!(lines[0], "--- a/src/lib.rs");
        assert_eq!(lines[1], "+++ b/src/lib.rs");
        assert_eq!(lines[2], "-fn old() {");
        assert_eq!(lines[3], "-    println!(\"hi\");");
        assert_eq!(lines[4], "-}");
        assert_eq!(lines[5], "+fn new() {");
        assert_eq!(lines[6], "+    println!(\"hello\");");
        assert_eq!(lines[7], "+    println!(\"world\");");
        assert_eq!(lines[8], "+}");
    }

    #[test]
    fn edit_without_old_string_no_diff() {
        let item = infer_item(
            "Edit",
            "tu_14",
            Some(json!({
                "file_path": "src/main.rs",
                "new_string": "fn new() {}"
            })),
        );
        match item {
            ConversationItem::File { changes, .. } => {
                assert!(changes[0].diff.is_none());
            }
            other => panic!("expected File, got {other:?}"),
        }
    }
}
