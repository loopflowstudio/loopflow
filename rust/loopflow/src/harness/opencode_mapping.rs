use std::collections::HashMap;

use serde_json::Value;

use crate::chat::types::{ConversationEvent, ConversationItem, FileEdit, Lifecycle};
use crate::id::AgentSessionId;

#[derive(Debug)]
pub(super) struct ReaderState {
    session_id: AgentSessionId,
    tools: HashMap<String, ToolLifecycle>,
    text: HashMap<String, String>,
    turn_substantive: bool,
    turn_open: bool,
    model: Option<String>,
    last_event_type: Option<String>,
    last_event_seq: Option<u64>,
}
impl ReaderState {
    pub(super) fn new(session_id: AgentSessionId, model: Option<String>) -> Self {
        Self {
            session_id,
            model,
            tools: HashMap::new(),
            text: HashMap::new(),
            turn_substantive: false,
            turn_open: false,
            last_event_type: None,
            last_event_seq: None,
        }
    }
    pub(super) fn model(&self) -> Option<&str> {
        self.model.as_deref()
    }
    pub(super) fn last_event_type(&self) -> Option<&str> {
        self.last_event_type.as_deref()
    }
    pub(super) fn last_event_seq(&self) -> Option<u64> {
        self.last_event_seq
    }
    pub(super) fn turn_is_open(&self) -> bool {
        self.turn_open
    }
    pub(super) fn turn_has_content(&self) -> bool {
        self.turn_substantive
    }
    pub(super) fn observe_event(&mut self, event: &Value) {
        if event["data"]["sessionID"] == self.session_id.as_str() {
            self.last_event_type = event["type"].as_str().map(str::to_owned);
            self.last_event_seq = Some(self.last_event_seq.map_or(0, |seq| seq + 1));
        }
    }
    pub(super) fn observe_lifecycle(&mut self, event: &ConversationEvent) {
        match event {
            ConversationEvent::TurnStarted { .. } => {
                self.turn_open = true;
                self.turn_substantive = false;
            }
            ConversationEvent::TurnCompleted { .. } => self.turn_open = false,
            _ => {}
        }
    }
    pub(super) fn observe_messages(
        &mut self,
        messages: &[(&str, &Value)],
    ) -> Vec<ConversationEvent> {
        let mut events = Vec::new();
        for (turn, message) in messages {
            for (index, part) in message["content"]
                .as_array()
                .into_iter()
                .flatten()
                .enumerate()
            {
                if part["type"] == "tool" {
                    map_tool_part(turn, part, self, &mut events);
                    continue;
                }
                let Some(text) = part["text"].as_str() else {
                    continue;
                };
                let id = format!("{}:{index}", message["id"].as_str().unwrap_or_default());
                let previous = self.text.entry(id).or_default();
                if let Some(delta) = text
                    .strip_prefix(previous.as_str())
                    .filter(|d| !d.is_empty())
                {
                    let content = delta.to_owned();
                    *previous = text.into();
                    self.turn_substantive = true;
                    events.push(if part["type"] == "reasoning" {
                        ConversationEvent::ReasoningDelta {
                            turn_id: (*turn).into(),
                            content,
                        }
                    } else {
                        ConversationEvent::TextDelta {
                            turn_id: (*turn).into(),
                            content,
                        }
                    });
                }
            }
        }
        events
    }
}
#[derive(Debug, Default)]
struct ToolLifecycle {
    started: bool,
    completed: bool,
}
fn map_tool_part(
    turn_id: &str,
    part: &Value,
    state: &mut ReaderState,
    events: &mut Vec<ConversationEvent>,
) {
    let Some(tool_id) = part["id"].as_str() else {
        tracing::debug!(part = ?part, "opencode tool part missing canonical id");
        return;
    };
    let Some(status) = tool_status(part) else {
        tracing::debug!(part = ?part, "opencode tool part missing canonical state");
        return;
    };
    // A canonical tool part is the model calling a tool — usable work.
    state.turn_substantive = true;
    let lifecycle = state.tools.entry(tool_id.into()).or_default();

    if !lifecycle.started {
        lifecycle.started = true;
        events.push(ConversationEvent::ItemStarted {
            turn_id: turn_id.into(),
            item: build_tool_item(part, tool_id, Lifecycle::Running),
        });
    }

    if matches!(status, Lifecycle::Completed | Lifecycle::Failed) && !lifecycle.completed {
        lifecycle.completed = true;
        events.push(ConversationEvent::ItemCompleted {
            turn_id: turn_id.into(),
            item: build_tool_item(part, tool_id, status),
        });
    }
}

fn tool_status(part: &Value) -> Option<Lifecycle> {
    let raw = part
        .pointer("/state/status")
        .and_then(Value::as_str)?
        .to_ascii_lowercase();
    match raw.as_str() {
        "running" => Some(Lifecycle::Running),
        "completed" => Some(Lifecycle::Completed),
        "error" => Some(Lifecycle::Failed),
        // Declined tool calls map to Failed for now; Decisions will give
        // declined a real home on the wire.
        "declined" => Some(Lifecycle::Failed),
        _ => {
            tracing::debug!(state = %raw, "opencode tool part had unknown canonical state");
            None
        }
    }
}

fn build_tool_item(part: &Value, tool_id: &str, status: Lifecycle) -> ConversationItem {
    let input = &part["state"]["input"];
    let output = if status != Lifecycle::Running {
        tool_output(part)
    } else {
        None
    };

    if let Some(command) = input.get("command") {
        return ConversationItem::Command {
            id: tool_id.to_string(),
            command: command_args(command),
            cwd: input["cwd"].as_str().unwrap_or_default().to_owned(),
            status,
            output,
            exit_code: part["state"]["metadata"]["exit"].as_i64().map(|n| n as i32),
            duration_ms: part["time"]["completed"]
                .as_u64()
                .zip(part["time"]["created"].as_u64())
                .and_then(|(end, start)| end.checked_sub(start)),
        };
    }

    if let Some(path) = input["path"].as_str().map(str::to_owned) {
        return ConversationItem::File {
            id: tool_id.to_string(),
            changes: if path.is_empty() {
                Vec::new()
            } else {
                vec![FileEdit {
                    path,
                    kind: None,
                    diff: None,
                }]
            },
            status,
        };
    }

    ConversationItem::Tool {
        id: tool_id.to_string(),
        name: tool_name(part),
        status,
        input: part["state"].get("input").cloned(),
        output,
    }
}

fn tool_name(part: &Value) -> String {
    part.get("name")
        .and_then(Value::as_str)
        .map(ToString::to_string)
        .unwrap_or_else(|| {
            tracing::debug!(part = ?part, "opencode tool part missing canonical tool");
            "tool".to_string()
        })
}

fn tool_output(part: &Value) -> Option<String> {
    if let Some(content) = part["state"]["content"].as_array() {
        return Some(
            content
                .iter()
                .filter_map(|part| part["text"].as_str())
                .collect::<Vec<_>>()
                .join("\n"),
        );
    }
    part["state"].get("error").map(Value::to_string)
}

fn command_args(command: &Value) -> Vec<String> {
    if let Some(array) = command.as_array() {
        return array
            .iter()
            .filter_map(Value::as_str)
            .map(ToString::to_string)
            .collect();
    }
    // A string command is a whole command line, not argv — keep it as a
    // single element so quoted arguments survive.
    if let Some(text) = command.as_str() {
        if text.is_empty() {
            return Vec::new();
        }
        return vec![text.to_string()];
    }
    Vec::new()
}
