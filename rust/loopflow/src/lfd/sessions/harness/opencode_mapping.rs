use std::collections::HashMap;

use serde_json::Value;

use crate::lfd::sessions::types::{FileEdit, ItemStatus, SessionEvent, SessionItem, TurnStatus};

#[derive(Debug, Default)]
pub(super) struct ReaderState {
    session_id: String,
    status: SessionState,
    current_turn_id: Option<String>,
    tools: HashMap<String, ToolLifecycle>,
}

impl ReaderState {
    pub(super) fn new(session_id: String) -> Self {
        Self {
            session_id,
            status: SessionState::Unknown,
            current_turn_id: None,
            tools: HashMap::new(),
        }
    }

    fn current_turn_id(&self) -> Option<String> {
        self.current_turn_id.clone()
    }

    fn accepts(&self, properties: &Value) -> bool {
        match session_id(properties) {
            Some(event_session_id) => event_session_id == self.session_id,
            None => true,
        }
    }
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
enum SessionState {
    #[default]
    Unknown,
    Idle,
    Active,
    Error,
}

#[derive(Debug, Default)]
struct ToolLifecycle {
    started: bool,
    completed: bool,
}

#[derive(Debug, Default)]
pub(super) struct MappedEvent {
    pub(super) events: Vec<SessionEvent>,
    pub(super) permission_requests: Vec<String>,
}

pub(super) fn map_event(raw: &Value, state: &mut ReaderState) -> MappedEvent {
    let mut mapped = MappedEvent::default();
    let event_type = raw.get("type").and_then(Value::as_str).unwrap_or_default();
    let properties = raw.get("properties").unwrap_or(raw);

    if !state.accepts(properties) {
        return mapped;
    }

    match event_type {
        "session.status" => map_status(properties, state, &mut mapped),
        "message.part.updated" => map_part_updated(properties, state, &mut mapped),
        "permission.asked" => map_permission(properties, &mut mapped),
        "session.diff" => map_diff(properties, state, &mut mapped),
        "session.error" => map_error(properties, &mut mapped),
        _ => {}
    }

    mapped
}

fn map_status(properties: &Value, state: &mut ReaderState, mapped: &mut MappedEvent) {
    let next_state = parse_session_state(properties);
    if next_state == SessionState::Unknown || next_state == state.status {
        return;
    }

    let was_active = state.status == SessionState::Active;
    state.status = next_state;

    match next_state {
        SessionState::Active => {
            let turn_id = format!("turn_{}", uuid::Uuid::new_v4());
            state.current_turn_id = Some(turn_id.clone());
            state.tools.clear();
            mapped.events.push(SessionEvent::TurnStarted { turn_id });
        }
        SessionState::Idle => {
            if was_active {
                complete_turn(state, TurnStatus::Completed, mapped);
            }
        }
        SessionState::Error => {
            if was_active {
                complete_turn(state, TurnStatus::Failed, mapped);
            }
        }
        SessionState::Unknown => {}
    }
}

fn complete_turn(state: &mut ReaderState, status: TurnStatus, mapped: &mut MappedEvent) {
    let turn_id = state
        .current_turn_id
        .take()
        .unwrap_or_else(|| "unknown".to_string());
    state.tools.clear();
    mapped
        .events
        .push(SessionEvent::TurnCompleted { turn_id, status });
}

fn parse_session_state(properties: &Value) -> SessionState {
    let value = properties
        .get("status")
        .or_else(|| properties.get("value"))
        .and_then(Value::as_str)
        .unwrap_or_default()
        .to_ascii_lowercase();

    match value.as_str() {
        "active" | "running" | "busy" => SessionState::Active,
        "idle" => SessionState::Idle,
        "error" | "failed" => SessionState::Error,
        _ => SessionState::Unknown,
    }
}

fn map_part_updated(properties: &Value, state: &mut ReaderState, mapped: &mut MappedEvent) {
    let part = properties.get("part").unwrap_or(properties);
    let part_type = part
        .get("type")
        .and_then(Value::as_str)
        .unwrap_or_default()
        .to_ascii_lowercase();

    if part_type.contains("reasoning") || part_type.contains("thinking") {
        if let (Some(turn_id), Some(content)) = (state.current_turn_id(), delta_text(part)) {
            mapped
                .events
                .push(SessionEvent::ReasoningDelta { turn_id, content });
        }
        return;
    }

    if part_type.contains("text") && !part_type.contains("tool") {
        if let (Some(turn_id), Some(content)) = (state.current_turn_id(), delta_text(part)) {
            mapped
                .events
                .push(SessionEvent::TextDelta { turn_id, content });
        }
        return;
    }

    if part_type.contains("tool") {
        map_tool_part(part, state, mapped);
    }
}

fn map_tool_part(part: &Value, state: &mut ReaderState, mapped: &mut MappedEvent) {
    let Some(turn_id) = state.current_turn_id() else {
        return;
    };

    let tool_id = tool_id(part);
    let status = tool_status(part);
    let lifecycle = state.tools.entry(tool_id.clone()).or_default();

    if !lifecycle.started {
        lifecycle.started = true;
        mapped.events.push(SessionEvent::ItemStarted {
            turn_id: turn_id.clone(),
            item: build_tool_item(part, &tool_id, ItemStatus::InProgress, false),
        });
    }

    if matches!(
        status,
        ItemStatus::Completed | ItemStatus::Failed | ItemStatus::Declined
    ) && !lifecycle.completed
    {
        lifecycle.completed = true;
        mapped.events.push(SessionEvent::ItemCompleted {
            turn_id,
            item: build_tool_item(part, &tool_id, status, true),
        });
    }
}

fn map_permission(properties: &Value, mapped: &mut MappedEvent) {
    let request_id = properties
        .get("requestID")
        .or_else(|| properties.get("requestId"))
        .or_else(|| properties.get("id"))
        .and_then(Value::as_str);
    if let Some(request_id) = request_id {
        mapped.permission_requests.push(request_id.to_string());
    }
}

fn map_diff(properties: &Value, state: &ReaderState, mapped: &mut MappedEvent) {
    let Some(turn_id) = state.current_turn_id() else {
        return;
    };
    let Some(diff) = properties
        .get("diff")
        .and_then(Value::as_str)
        .map(ToString::to_string)
    else {
        return;
    };
    mapped
        .events
        .push(SessionEvent::DiffUpdated { turn_id, diff });
}

fn map_error(properties: &Value, mapped: &mut MappedEvent) {
    let code = properties
        .get("code")
        .and_then(Value::as_str)
        .unwrap_or("opencode_error")
        .to_string();
    let message = properties
        .get("message")
        .or_else(|| properties.get("error"))
        .and_then(Value::as_str)
        .unwrap_or("opencode error")
        .to_string();
    mapped.events.push(SessionEvent::Error { code, message });
}

fn delta_text(part: &Value) -> Option<String> {
    part.get("delta")
        .or_else(|| part.get("text"))
        .and_then(Value::as_str)
        .filter(|text| !text.is_empty())
        .map(ToString::to_string)
}

fn tool_id(part: &Value) -> String {
    part.get("id")
        .or_else(|| part.get("toolCallID"))
        .or_else(|| part.get("toolUseID"))
        .and_then(Value::as_str)
        .unwrap_or("tool_unknown")
        .to_string()
}

fn tool_status(part: &Value) -> ItemStatus {
    let raw = part
        .get("state")
        .or_else(|| part.get("status"))
        .and_then(Value::as_str)
        .unwrap_or("running")
        .to_ascii_lowercase();
    match raw.as_str() {
        "completed" | "complete" | "done" | "success" => ItemStatus::Completed,
        "failed" | "error" => ItemStatus::Failed,
        "declined" | "rejected" => ItemStatus::Declined,
        _ => ItemStatus::InProgress,
    }
}

fn build_tool_item(
    part: &Value,
    tool_id: &str,
    status: ItemStatus,
    include_output: bool,
) -> SessionItem {
    let input = tool_input(part);
    let output = if include_output {
        tool_output(part)
    } else {
        None
    };

    if has_command_field(part, input.as_ref()) {
        return SessionItem::Command {
            id: tool_id.to_string(),
            command: command_args(part, input.as_ref()),
            cwd: text_field(part, "cwd")
                .or_else(|| input.as_ref().and_then(|value| text_field(value, "cwd")))
                .unwrap_or_default(),
            status,
            output,
            exit_code: integer_field(part, "exitCode"),
            duration_ms: unsigned_field(part, "durationMs"),
        };
    }

    if has_file_path_field(part, input.as_ref()) {
        let path = text_field(part, "file")
            .or_else(|| text_field(part, "path"))
            .or_else(|| input.as_ref().and_then(|value| text_field(value, "file")))
            .or_else(|| input.as_ref().and_then(|value| text_field(value, "path")))
            .unwrap_or_default();
        return SessionItem::File {
            id: tool_id.to_string(),
            changes: if path.is_empty() {
                Vec::new()
            } else {
                vec![FileEdit {
                    path,
                    kind: text_field(part, "kind"),
                    diff: text_field(part, "diff"),
                }]
            },
            status,
        };
    }

    SessionItem::Tool {
        id: tool_id.to_string(),
        name: tool_name(part),
        status,
        input,
        output,
    }
}

fn tool_name(part: &Value) -> String {
    if let Some(name) = part
        .get("name")
        .or_else(|| part.get("toolName"))
        .or_else(|| part.get("tool"))
        .and_then(Value::as_str)
    {
        return name.to_string();
    }

    if let Some(name) = part
        .get("tool")
        .and_then(|value| value.get("name"))
        .and_then(Value::as_str)
    {
        return name.to_string();
    }

    "tool".to_string()
}

fn tool_input(part: &Value) -> Option<Value> {
    part.get("input")
        .cloned()
        .or_else(|| part.get("arguments").cloned())
        .or_else(|| part.get("args").cloned())
}

fn tool_output(part: &Value) -> Option<String> {
    part.get("output")
        .or_else(|| part.get("result"))
        .or_else(|| part.get("error"))
        .and_then(value_as_string)
}

fn value_as_string(value: &Value) -> Option<String> {
    match value {
        Value::Null => None,
        Value::String(text) => Some(text.clone()),
        other => Some(other.to_string()),
    }
}

fn command_args(part: &Value, input: Option<&Value>) -> Vec<String> {
    if let Some(command) = part
        .get("command")
        .or_else(|| input.and_then(|value| value.get("command")))
    {
        if let Some(array) = command.as_array() {
            return array
                .iter()
                .filter_map(Value::as_str)
                .map(ToString::to_string)
                .collect();
        }
        if let Some(text) = command.as_str() {
            return text.split_whitespace().map(ToString::to_string).collect();
        }
    }
    Vec::new()
}

fn has_command_field(part: &Value, input: Option<&Value>) -> bool {
    part.get("command").is_some() || input.and_then(|value| value.get("command")).is_some()
}

fn has_file_path_field(part: &Value, input: Option<&Value>) -> bool {
    part.get("file").is_some()
        || part.get("path").is_some()
        || input.and_then(|value| value.get("file")).is_some()
        || input.and_then(|value| value.get("path")).is_some()
}

fn text_field(value: &Value, key: &str) -> Option<String> {
    value
        .get(key)
        .and_then(Value::as_str)
        .map(ToString::to_string)
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
        .or_else(|| properties.get("sessionId"))
        .or_else(|| properties.get("session_id"))
        .and_then(Value::as_str)
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    #[test]
    fn session_status_transitions_emit_turn_boundaries() {
        let mut state = ReaderState::new("session_1".to_string());

        let started = map_event(
            &json!({
                "type": "session.status",
                "properties": { "sessionID": "session_1", "status": "active" }
            }),
            &mut state,
        );
        assert_eq!(started.events.len(), 1);
        let started_turn_id = match &started.events[0] {
            SessionEvent::TurnStarted { turn_id } => turn_id.clone(),
            other => panic!("expected TurnStarted, got {other:?}"),
        };

        let completed = map_event(
            &json!({
                "type": "session.status",
                "properties": { "sessionID": "session_1", "status": "idle" }
            }),
            &mut state,
        );
        assert_eq!(completed.events.len(), 1);
        match &completed.events[0] {
            SessionEvent::TurnCompleted { turn_id, status } => {
                assert_eq!(turn_id, &started_turn_id);
                assert_eq!(*status, TurnStatus::Completed);
            }
            other => panic!("expected TurnCompleted, got {other:?}"),
        }
    }

    #[test]
    fn text_part_maps_to_text_delta() {
        let mut state = ReaderState::new("session_1".to_string());
        let _ = map_event(
            &json!({
                "type": "session.status",
                "properties": { "sessionID": "session_1", "status": "active" }
            }),
            &mut state,
        );

        let mapped = map_event(
            &json!({
                "type": "message.part.updated",
                "properties": {
                    "sessionID": "session_1",
                    "part": { "id": "part_1", "type": "TextPart", "delta": "hello" }
                }
            }),
            &mut state,
        );

        assert_eq!(mapped.events.len(), 1);
        match &mapped.events[0] {
            SessionEvent::TextDelta { content, .. } => assert_eq!(content, "hello"),
            other => panic!("expected TextDelta, got {other:?}"),
        }
    }

    #[test]
    fn tool_part_deduplicates_lifecycle_events() {
        let mut state = ReaderState::new("session_1".to_string());
        let _ = map_event(
            &json!({
                "type": "session.status",
                "properties": { "sessionID": "session_1", "status": "active" }
            }),
            &mut state,
        );

        let started = map_event(
            &json!({
                "type": "message.part.updated",
                "properties": {
                    "sessionID": "session_1",
                    "part": {
                        "id": "tool_1",
                        "type": "ToolPart",
                        "state": "running",
                        "name": "Bash",
                        "command": ["echo", "ok"]
                    }
                }
            }),
            &mut state,
        );
        assert_eq!(started.events.len(), 1);
        assert!(matches!(
            started.events[0],
            SessionEvent::ItemStarted { .. }
        ));

        let duplicate_start = map_event(
            &json!({
                "type": "message.part.updated",
                "properties": {
                    "sessionID": "session_1",
                    "part": {
                        "id": "tool_1",
                        "type": "ToolPart",
                        "state": "running",
                        "name": "Bash",
                        "command": ["echo", "ok"]
                    }
                }
            }),
            &mut state,
        );
        assert!(duplicate_start.events.is_empty());

        let completed = map_event(
            &json!({
                "type": "message.part.updated",
                "properties": {
                    "sessionID": "session_1",
                    "part": {
                        "id": "tool_1",
                        "type": "ToolPart",
                        "state": "completed",
                        "name": "Bash",
                        "command": ["echo", "ok"],
                        "output": "ok"
                    }
                }
            }),
            &mut state,
        );
        assert_eq!(completed.events.len(), 1);
        match &completed.events[0] {
            SessionEvent::ItemCompleted {
                item: SessionItem::Command { output, .. },
                ..
            } => assert_eq!(output.as_deref(), Some("ok")),
            other => panic!("expected ItemCompleted command, got {other:?}"),
        }

        let duplicate_complete = map_event(
            &json!({
                "type": "message.part.updated",
                "properties": {
                    "sessionID": "session_1",
                    "part": {
                        "id": "tool_1",
                        "type": "ToolPart",
                        "state": "completed",
                        "name": "Bash",
                        "command": ["echo", "ok"],
                        "output": "ok"
                    }
                }
            }),
            &mut state,
        );
        assert!(duplicate_complete.events.is_empty());
    }

    #[test]
    fn permission_event_collects_request_id() {
        let mut state = ReaderState::new("session_1".to_string());
        let mapped = map_event(
            &json!({
                "type": "permission.asked",
                "properties": { "sessionID": "session_1", "requestID": "perm_1" }
            }),
            &mut state,
        );
        assert_eq!(mapped.permission_requests, vec!["perm_1".to_string()]);
        assert!(mapped.events.is_empty());
    }

    #[test]
    fn ignores_events_for_other_sessions() {
        let mut state = ReaderState::new("session_1".to_string());
        let mapped = map_event(
            &json!({
                "type": "session.status",
                "properties": { "sessionID": "session_2", "status": "active" }
            }),
            &mut state,
        );
        assert!(mapped.events.is_empty());
    }
}
