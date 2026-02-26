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

    fn current_turn_id(&self) -> Option<&str> {
        self.current_turn_id.as_deref()
    }

    fn accepts(&self, properties: &Value) -> bool {
        match session_id(properties) {
            Some(event_session_id) => event_session_id == self.session_id,
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
        "session.error" => map_error(properties, state, &mut mapped),
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
    let value = string_by_keys(properties, &["status"])
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
            mapped.events.push(SessionEvent::ReasoningDelta {
                turn_id: turn_id.to_string(),
                content,
            });
        }
        return;
    }

    if part_type.contains("text") && !part_type.contains("tool") {
        if let (Some(turn_id), Some(content)) = (state.current_turn_id(), delta_text(part)) {
            mapped.events.push(SessionEvent::TextDelta {
                turn_id: turn_id.to_string(),
                content,
            });
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
    let turn_id = turn_id.to_string();

    let Some(tool_id) = tool_id(part) else {
        tracing::debug!(part = ?part, "opencode tool part missing canonical id");
        return;
    };
    let Some(status) = tool_status(part) else {
        tracing::debug!(part = ?part, "opencode tool part missing canonical state");
        return;
    };
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
    if let Some(request_id) = properties.get("requestID").and_then(Value::as_str) {
        mapped.permission_requests.push(request_id.to_string());
    } else {
        tracing::debug!(
            properties = ?properties,
            "opencode permission event missing canonical requestID"
        );
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
    mapped.events.push(SessionEvent::DiffUpdated {
        turn_id: turn_id.to_string(),
        diff,
    });
}

fn map_error(properties: &Value, state: &mut ReaderState, mapped: &mut MappedEvent) {
    let code =
        string_by_keys(properties, &["code"]).unwrap_or_else(|| "opencode_error".to_string());
    let message = string_by_keys(properties, &["message", "error"])
        .unwrap_or_else(|| "opencode error".to_string());

    if state.status == SessionState::Active {
        state.status = SessionState::Error;
        complete_turn(state, TurnStatus::Failed, mapped);
    }

    mapped.events.push(SessionEvent::Error { code, message });
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

fn delta_text(part: &Value) -> Option<String> {
    part.get("delta")
        .or_else(|| part.get("text"))
        .and_then(Value::as_str)
        .filter(|text| !text.is_empty())
        .map(ToString::to_string)
}

fn tool_id(part: &Value) -> Option<String> {
    part.get("id")
        .and_then(Value::as_str)
        .map(ToString::to_string)
}

fn tool_status(part: &Value) -> Option<ItemStatus> {
    let raw = part
        .get("state")
        .and_then(Value::as_str)?
        .to_ascii_lowercase();
    match raw.as_str() {
        "running" => Some(ItemStatus::InProgress),
        "completed" => Some(ItemStatus::Completed),
        "failed" => Some(ItemStatus::Failed),
        "declined" => Some(ItemStatus::Declined),
        _ => {
            tracing::debug!(state = %raw, "opencode tool part had unknown canonical state");
            None
        }
    }
}

fn build_tool_item(
    part: &Value,
    tool_id: &str,
    status: ItemStatus,
    include_output: bool,
) -> SessionItem {
    let input = tool_input(part);
    let input_ref = input.as_ref();
    let output = if include_output {
        tool_output(part)
    } else {
        None
    };

    if let Some(command) = part_or_input_value(part, input_ref, &["command"]) {
        return SessionItem::Command {
            id: tool_id.to_string(),
            command: command_args(command),
            cwd: part_or_input_text(part, input_ref, &["cwd"]).unwrap_or_default(),
            status,
            output,
            exit_code: integer_field(part, "exitCode"),
            duration_ms: unsigned_field(part, "durationMs"),
        };
    }

    if let Some(path) = part_or_input_text(part, input_ref, &["file", "path"]) {
        return SessionItem::File {
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

    SessionItem::Tool {
        id: tool_id.to_string(),
        name: tool_name(part),
        status,
        input,
        output,
    }
}

fn tool_name(part: &Value) -> String {
    part.get("name")
        .and_then(Value::as_str)
        .map(ToString::to_string)
        .unwrap_or_else(|| {
            tracing::debug!(part = ?part, "opencode tool part missing canonical name");
            "tool".to_string()
        })
}

fn tool_input(part: &Value) -> Option<Value> {
    value_by_keys(part, &["input", "arguments", "args"]).cloned()
}

fn tool_output(part: &Value) -> Option<String> {
    value_by_keys(part, &["output", "result", "error"]).and_then(value_as_string)
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
    if let Some(text) = command.as_str() {
        return text.split_whitespace().map(ToString::to_string).collect();
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
    properties.get("sessionID").and_then(Value::as_str)
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

    #[test]
    fn session_error_while_active_completes_turn_as_failed() {
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
                "type": "session.error",
                "properties": {
                    "sessionID": "session_1",
                    "code": "boom",
                    "message": "failed"
                }
            }),
            &mut state,
        );

        assert_eq!(mapped.events.len(), 2);
        assert!(matches!(
            mapped.events[0],
            SessionEvent::TurnCompleted {
                status: TurnStatus::Failed,
                ..
            }
        ));
        assert!(matches!(mapped.events[1], SessionEvent::Error { .. }));
    }

    #[test]
    fn ignores_noncanonical_session_id_shape() {
        let mut state = ReaderState::new("session_1".to_string());
        let mapped = map_event(
            &json!({
                "type": "session.status",
                "properties": {
                    "session": { "id": "session_1", "status": "active" }
                }
            }),
            &mut state,
        );

        assert!(mapped.events.is_empty());
    }
}
