use std::collections::HashMap;

use serde_json::Value;

use crate::chat::types::{
    ConversationEvent, ConversationItem, FailureEvidence, FileEdit, Lifecycle,
};
use crate::id::AgentSessionId;

#[derive(Debug)]
pub(super) struct ReaderState {
    session_id: AgentSessionId,
    current_turn_id: Option<String>,
    tools: HashMap<String, ToolLifecycle>,
    text: HashMap<String, String>,
    deltas: HashMap<String, String>,
    messages: HashMap<String, String>,
    reasoning: std::collections::HashSet<String>,
    turn_substantive: bool,
    turn_open: bool,
    /// The configured agent model (e.g. `opencode/glm-5.2`), from `AgentConfig`.
    /// Carried so disconnect evidence can name the model without re-reading
    /// config at the failure site.
    model: Option<String>,
    /// The harness/provider name (e.g. `opencode`). Static for the harness.
    provider: &'static str,
    /// Last successfully parsed SSE event type (e.g. `session.status`).
    last_event_type: Option<String>,
    /// 0-based count of accepted SSE events so far. `None` until the first
    /// accepted event, so a pre-content disconnect is distinguishable.
    last_event_seq: Option<u64>,
    /// When the current turn started, ms since epoch. Reset each turn.
    turn_started_at: Option<i64>,
}

impl ReaderState {
    pub(super) fn new(
        session_id: AgentSessionId,
        model: Option<String>,
        provider: &'static str,
    ) -> Self {
        Self {
            session_id,
            turn_open: false,
            text: HashMap::new(),
            deltas: HashMap::new(),
            messages: HashMap::new(),
            reasoning: std::collections::HashSet::new(),
            current_turn_id: None,
            tools: HashMap::new(),
            turn_substantive: false,
            model,
            provider,
            last_event_type: None,
            last_event_seq: None,
            turn_started_at: None,
        }
    }

    fn current_turn_id(&self) -> Option<&str> {
        self.current_turn_id.as_deref()
    }

    /// The configured agent model, for disconnect evidence.
    pub(super) fn model(&self) -> Option<&str> {
        self.model.as_deref()
    }

    /// The harness/provider name, for disconnect evidence.
    pub(super) fn provider(&self) -> &'static str {
        self.provider
    }

    /// Last successfully parsed SSE event type, for disconnect evidence.
    pub(super) fn last_event_type(&self) -> Option<&str> {
        self.last_event_type.as_deref()
    }

    /// 0-based count of accepted SSE events, for disconnect evidence.
    pub(super) fn last_event_seq(&self) -> Option<u64> {
        self.last_event_seq
    }

    /// When the current turn started, ms since epoch, for disconnect evidence.
    pub(super) fn turn_started_at(&self) -> Option<i64> {
        self.turn_started_at
    }

    /// Record that the current turn produced usable assistant work. Called at
    /// every content-bearing emit point so hollowness is measured by what the
    /// turn actually said, not by the status opencode chose to report.
    fn mark_substantive(&mut self) {
        self.turn_substantive = true;
    }

    /// Whether a turn is open (used by the harness to close an orphaned turn
    /// when the SSE stream disconnects mid-turn).
    pub(super) fn turn_is_open(&self) -> bool {
        self.turn_open
    }

    /// Whether the open turn has produced any usable work yet. Lets the harness
    /// distinguish a pre-content disconnect from a mid-stream one.
    pub(super) fn turn_has_content(&self) -> bool {
        self.turn_substantive
    }

    pub(super) fn observe_messages(&mut self, messages: &[Value]) -> Vec<ConversationEvent> {
        let mut events = Vec::new();
        for message in messages {
            let info = &message["info"];
            if info["sessionID"] != self.session_id.as_str() || info["role"] != "assistant" {
                continue;
            }
            let Some(request) = info["parentID"].as_str() else {
                continue;
            };
            if let Some(id) = info["id"].as_str() {
                self.messages.insert(id.into(), request.into());
            }
            self.current_turn_id = Some(request.into());
            for part in message["parts"].as_array().into_iter().flatten() {
                map_part_updated(part, self, &mut events);
            }
        }
        events
    }

    pub(super) fn observe_lifecycle(&mut self, event: &ConversationEvent) {
        match event {
            ConversationEvent::TurnStarted { turn_id } => {
                self.current_turn_id = Some(turn_id.clone());
                self.turn_open = true;
                self.turn_substantive = false;
                self.turn_started_at = Some(chrono::Utc::now().timestamp_millis());
            }
            ConversationEvent::TurnCompleted { .. } => self.turn_open = false,
            _ => {}
        }
    }

    fn accepts(&self, properties: &Value) -> bool {
        match session_id(properties) {
            Some(event_session_id) => event_session_id == self.session_id.as_str(),
            None => {
                tracing::debug!(
                    properties = ?properties,
                    "opencode event missing canonical properties.sessionID"
                );
                false
            }
        }
    }
}

#[derive(Debug, Default)]
struct ToolLifecycle {
    started: bool,
    completed: bool,
}

pub(super) fn map_event(raw: &Value, state: &mut ReaderState) -> Vec<ConversationEvent> {
    let mut events = Vec::new();
    let event_type = raw.get("type").and_then(Value::as_str).unwrap_or_default();
    let properties = raw.get("properties").unwrap_or(raw);

    if !state.accepts(properties) {
        return events;
    }

    // Track the last accepted event for disconnect evidence. The seq is
    // 0-based: the first accepted event is seq 0.
    state.last_event_seq = Some(state.last_event_seq.map_or(0, |n| n + 1));
    state.last_event_type = Some(event_type.to_string());

    match event_type {
        "message.part.delta" => {
            if let (Some(request), Some(part), Some(delta)) = (
                properties["messageID"]
                    .as_str()
                    .and_then(|id| state.messages.get(id))
                    .cloned(),
                properties["partID"].as_str(),
                properties["delta"].as_str(),
            ) {
                if properties["field"] == "text" {
                    let text = state.deltas.entry(part.into()).or_default();
                    text.push_str(delta);
                    let text = text.clone();
                    let kind = if state.reasoning.contains(part) {
                        "reasoning"
                    } else {
                        "text"
                    };
                    state.current_turn_id = Some(request);
                    map_part_updated(
                        &serde_json::json!({"type":kind,"id":part,"text":text}),
                        state,
                        &mut events,
                    );
                }
            }
        }
        "session.diff" => map_diff(properties, state, &mut events),
        "session.error" => map_error(properties, state, &mut events),
        _ => {}
    }

    events
}

fn map_part_updated(
    properties: &Value,
    state: &mut ReaderState,
    events: &mut Vec<ConversationEvent>,
) {
    let part = properties.get("part").unwrap_or(properties);
    let part_type = part
        .get("type")
        .and_then(Value::as_str)
        .unwrap_or_default()
        .to_ascii_lowercase();

    if matches!(part_type.as_str(), "reasoning" | "text") {
        let (Some(turn_id), Some(id), Some(text)) = (
            state.current_turn_id().map(str::to_owned),
            part["id"].as_str(),
            part["text"].as_str(),
        ) else {
            return;
        };
        if part_type == "reasoning" {
            state.reasoning.insert(id.into());
        }
        let previous = state.text.entry(id.into()).or_default();
        if let Some(delta) = text
            .strip_prefix(previous.as_str())
            .filter(|delta| !delta.is_empty())
        {
            let content = delta.to_owned();
            *previous = text.into();
            state.mark_substantive();
            events.push(if part_type == "reasoning" {
                ConversationEvent::ReasoningDelta { turn_id, content }
            } else {
                ConversationEvent::TextDelta { turn_id, content }
            });
        }
        return;
    }

    if part_type.contains("tool") {
        map_tool_part(part, state, events);
    }
}

fn map_tool_part(part: &Value, state: &mut ReaderState, events: &mut Vec<ConversationEvent>) {
    let Some(turn_id) = state.current_turn_id() else {
        return;
    };
    let turn_id = turn_id.to_string();

    let Some(tool_id) = tool_id(part) else {
        tracing::debug!(part = ?part, "opencode tool part missing canonical id");
        return;
    };
    let Some(status) = tool_status(part) else {
        tracing::debug!(part = ?part, "opencode tool part missing canonical state");
        return;
    };
    // A canonical tool part is the model calling a tool — usable work.
    state.mark_substantive();
    let lifecycle = state.tools.entry(tool_id.clone()).or_default();

    if !lifecycle.started {
        lifecycle.started = true;
        events.push(ConversationEvent::ItemStarted {
            turn_id: turn_id.clone(),
            item: build_tool_item(part, &tool_id, Lifecycle::Running, false),
        });
    }

    if matches!(status, Lifecycle::Completed | Lifecycle::Failed) && !lifecycle.completed {
        lifecycle.completed = true;
        events.push(ConversationEvent::ItemCompleted {
            turn_id,
            item: build_tool_item(part, &tool_id, status, true),
        });
    }
}

fn map_diff(properties: &Value, state: &mut ReaderState, events: &mut Vec<ConversationEvent>) {
    let Some(turn_id) = state.current_turn_id().map(str::to_string) else {
        return;
    };
    let Some(diff) = properties
        .get("diff")
        .and_then(Value::as_str)
        .map(ToString::to_string)
    else {
        return;
    };
    state.mark_substantive();
    events.push(ConversationEvent::DiffUpdated { turn_id, diff });
}

fn map_error(properties: &Value, state: &mut ReaderState, events: &mut Vec<ConversationEvent>) {
    let code = properties
        .get("code")
        .and_then(Value::as_str)
        .map(ToString::to_string)
        .unwrap_or_else(|| "opencode_error".to_string());
    let message = properties
        .get("message")
        .and_then(Value::as_str)
        .map(ToString::to_string)
        .unwrap_or_else(|| "opencode error".to_string());

    let stream_ended_at = chrono::Utc::now().timestamp_millis();
    let evidence = FailureEvidence {
        model: state.model().map(ToString::to_string),
        provider: Some(state.provider().to_string()),
        endpoint_class: Some("upstream_provider".to_string()),
        stream_started_at: state.turn_started_at(),
        stream_ended_at: Some(stream_ended_at),
        duration_ms: state.turn_started_at().map(|s| stream_ended_at - s),
        last_event_type: state.last_event_type().map(ToString::to_string),
        last_event_seq: state.last_event_seq(),
        terminal_error_class: Some("session_error".to_string()),
        terminal_error_message: Some(crate::harness::opencode::sanitize_error_message(&message)),
        provider_output_tokens: None,
    };
    events.push(ConversationEvent::Error {
        code,
        message,
        evidence: Some(evidence),
    });
}

fn value_by_keys<'a>(value: &'a Value, keys: &[&str]) -> Option<&'a Value> {
    keys.iter().find_map(|key| value.get(*key))
}

fn string_by_keys(value: &Value, keys: &[&str]) -> Option<String> {
    value_by_keys(value, keys)
        .and_then(Value::as_str)
        .map(ToString::to_string)
}

fn part_or_input_value<'a>(
    part: &'a Value,
    input: Option<&'a Value>,
    keys: &[&str],
) -> Option<&'a Value> {
    value_by_keys(part, keys).or_else(|| input.and_then(|value| value_by_keys(value, keys)))
}

fn part_or_input_text(part: &Value, input: Option<&Value>, keys: &[&str]) -> Option<String> {
    part_or_input_value(part, input, keys)
        .and_then(Value::as_str)
        .map(ToString::to_string)
}

fn tool_id(part: &Value) -> Option<String> {
    part.get("id")
        .and_then(Value::as_str)
        .map(ToString::to_string)
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

fn build_tool_item(
    part: &Value,
    tool_id: &str,
    status: Lifecycle,
    include_output: bool,
) -> ConversationItem {
    let input = tool_input(part);
    let input_ref = input.as_ref();
    let output = if include_output {
        tool_output(part)
    } else {
        None
    };

    if let Some(command) = part_or_input_value(part, input_ref, &["command"]) {
        return ConversationItem::Command {
            id: tool_id.to_string(),
            command: command_args(command),
            cwd: part_or_input_text(part, input_ref, &["cwd"]).unwrap_or_default(),
            status,
            output,
            exit_code: integer_field(&part["state"]["metadata"], "exit"),
            duration_ms: unsigned_field(&part["state"]["time"], "end")
                .zip(unsigned_field(&part["state"]["time"], "start"))
                .and_then(|(end, start)| end.checked_sub(start)),
        };
    }

    if let Some(path) = part_or_input_text(part, input_ref, &["file", "path"]) {
        return ConversationItem::File {
            id: tool_id.to_string(),
            changes: if path.is_empty() {
                Vec::new()
            } else {
                vec![FileEdit {
                    path,
                    kind: string_by_keys(part, &["kind"]),
                    diff: string_by_keys(part, &["diff"]),
                }]
            },
            status,
        };
    }

    ConversationItem::Tool {
        id: tool_id.to_string(),
        name: tool_name(part),
        status,
        input,
        output,
    }
}

fn tool_name(part: &Value) -> String {
    part.get("tool")
        .and_then(Value::as_str)
        .map(ToString::to_string)
        .unwrap_or_else(|| {
            tracing::debug!(part = ?part, "opencode tool part missing canonical tool");
            "tool".to_string()
        })
}

fn tool_input(part: &Value) -> Option<Value> {
    part.pointer("/state/input").cloned()
}

fn tool_output(part: &Value) -> Option<String> {
    value_by_keys(&part["state"], &["output", "error"]).and_then(value_as_string)
}

fn value_as_string(value: &Value) -> Option<String> {
    match value {
        Value::Null => None,
        Value::String(text) => Some(text.clone()),
        other => Some(other.to_string()),
    }
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

fn integer_field(value: &Value, key: &str) -> Option<i32> {
    value
        .get(key)
        .and_then(Value::as_i64)
        .map(|number| number as i32)
}

fn unsigned_field(value: &Value, key: &str) -> Option<u64> {
    value.get(key).and_then(Value::as_u64)
}

fn session_id(properties: &Value) -> Option<&str> {
    properties
        .get("sessionID")
        .or_else(|| properties.get("part")?.get("sessionID"))
        .or_else(|| properties.get("info")?.get("sessionID"))
        .and_then(Value::as_str)
}

#[cfg(test)]
mod tests {
    use super::{map_event, ReaderState};
    use crate::chat::types::ConversationEvent;
    use serde_json::json;

    #[test]
    fn native_request_correlates_output_without_idle_completion() {
        let mut state = ReaderState::new("session".into(), None, "opencode");
        let message = |text: &str| json!({"info":{"id":"assistant","sessionID":"session","role":"assistant","parentID":"request"},"parts":[{"id":"part","sessionID":"session","messageID":"assistant","type":"text","text":text}]});
        let first = state.observe_messages(&[message("Final")]);
        assert!(
            matches!(&first[..], [ConversationEvent::TextDelta {turn_id,content}] if turn_id=="request" && content=="Final")
        );
        assert!(state.observe_messages(&[message("Final")]).is_empty());
        let rest = state.observe_messages(&[message("Final answer")]);
        assert!(
            matches!(&rest[..], [ConversationEvent::TextDelta {turn_id,content}] if turn_id=="request" && content==" answer")
        );
        let delta = map_event(
            &json!({"type":"message.part.delta","properties":{"sessionID":"session","messageID":"assistant","partID":"part","field":"text","delta":"Final answer"}}),
            &mut state,
        );
        assert!(delta.is_empty(), "snapshot already delivered this delta");
        for status in ["busy", "idle"] {
            assert!(map_event(&json!({"type":"session.status","properties":{"sessionID":"session","status":{"type":status}}}), &mut state).is_empty());
        }
    }
}
