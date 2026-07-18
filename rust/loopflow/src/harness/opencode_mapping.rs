use std::collections::HashMap;

use serde_json::Value;

use crate::chat::types::{
    ConversationEvent, ConversationItem, FailureEvidence, FileEdit, Lifecycle, TurnUsage,
};

#[derive(Debug, Default)]
pub(super) struct ReaderState {
    session_id: String,
    status: SessionState,
    current_turn_id: Option<String>,
    tools: HashMap<String, ToolLifecycle>,
    /// Whether the current turn has emitted any usable assistant work (text,
    /// reasoning, a tool call, or a diff). A turn that closes `idle` without
    /// having produced any is a hollow body — see [`Self::mark_substantive`]
    /// and the hollow-close branch in [`map_status`].
    turn_substantive: bool,
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

/// Error codes for turns that close without usable work. Distinct from
/// `opencode_disconnected` (the harness's own event stream dropping): these mark
/// a turn opencode itself reported as finished (`idle`) but that carried no
/// assistant output — the hollow-body failure the SSE incident produced.
pub(crate) const HOLLOW_BODY_CODE: &str = "opencode_hollow_body";
pub(crate) const DECODE_GAP_CODE: &str = "opencode_decode_gap";

impl ReaderState {
    pub(super) fn new(session_id: String, model: Option<String>, provider: &'static str) -> Self {
        Self {
            session_id,
            status: SessionState::Unknown,
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
        self.status == SessionState::Active && self.current_turn_id.is_some()
    }

    /// Whether the open turn has produced any usable work yet. Lets the harness
    /// distinguish a pre-content disconnect from a mid-stream one.
    pub(super) fn turn_has_content(&self) -> bool {
        self.turn_substantive
    }

    /// Close a turn left open when the SSE stream disconnected. Returns
    /// `TurnCompleted { Failed }` + `TurnUsage` so every `TurnStarted` gets a
    /// terminal close and the journal never carries an open turn past a
    /// disconnect. The body still fails once via the consumer's `Error`
    /// handler — this is ledger honesty, not a second failure.
    pub(super) fn close_orphaned_turn(&mut self) -> Vec<ConversationEvent> {
        let turn_id = self
            .current_turn_id
            .take()
            .unwrap_or_else(|| "unknown".to_string());
        self.tools.clear();
        self.turn_substantive = false;
        // A disconnect reports no usage. Closing the turn must not invent a
        // zeroed reading; the stream's accumulated totals are what stand.
        vec![ConversationEvent::TurnCompleted {
            turn_id,
            status: Lifecycle::Failed,
        }]
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
    pub(super) events: Vec<ConversationEvent>,
    pub(super) permission_requests: Vec<String>,
}

pub(super) fn map_event(raw: &Value, state: &mut ReaderState) -> MappedEvent {
    let mut mapped = MappedEvent::default();
    let event_type = raw.get("type").and_then(Value::as_str).unwrap_or_default();
    let properties = raw.get("properties").unwrap_or(raw);

    if !state.accepts(properties) {
        return mapped;
    }

    // Track the last accepted event for disconnect evidence. The seq is
    // 0-based: the first accepted event is seq 0.
    state.last_event_seq = Some(state.last_event_seq.map_or(0, |n| n + 1));
    state.last_event_type = Some(event_type.to_string());

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
            state.turn_substantive = false;
            state.turn_started_at = Some(chrono::Utc::now().timestamp_millis());
            mapped
                .events
                .push(ConversationEvent::TurnStarted { turn_id });
        }
        SessionState::Idle => {
            if was_active {
                let usage = map_turn_usage(properties);
                if state.turn_substantive {
                    complete_turn(state, Lifecycle::Completed, usage, mapped);
                } else {
                    // opencode reported the turn finished, but it emitted no
                    // usable assistant work. This is the hollow body an SSE
                    // disconnect (typically the upstream model stream, not our
                    // own /event stream) produces when opencode maps a
                    // truncated response to `idle` rather than `error`. Never
                    // let it read as Completed.
                    complete_hollow_turn(state, usage, mapped);
                }
            }
        }
        SessionState::Error => {
            if was_active {
                complete_turn(state, Lifecycle::Failed, None, mapped);
            }
        }
        SessionState::Unknown => {}
    }
}

fn complete_turn(
    state: &mut ReaderState,
    status: Lifecycle,
    usage: Option<TurnUsage>,
    mapped: &mut MappedEvent,
) {
    let turn_id = state
        .current_turn_id
        .take()
        .unwrap_or_else(|| "unknown".to_string());
    state.tools.clear();
    mapped.events.push(ConversationEvent::TurnCompleted {
        turn_id: turn_id.clone(),
        status,
    });
    // Only report usage the provider actually reported. A defaulted TurnUsage
    // claims "the provider measured zero", and the trace capture takes a
    // reported usage as authoritative — it replaces the totals accumulated from
    // the stream rather than merging them. Emitting one here on every turn
    // therefore erased real token counts.
    if let Some(usage) = usage {
        mapped
            .events
            .push(ConversationEvent::TurnUsage { turn_id, usage });
    }
}

/// Close a turn that opencode reported `idle` but that produced no usable work.
/// Emits `TurnCompleted { Failed }` + usage + an `Error` carrying an actionable
/// reason — never a `Completed`. When usage claims output tokens the model did
/// produce content we failed to map: that is a harness decode gap, reported
/// distinctly so a mapping regression cannot masquerade as an empty-but-fine
/// turn.
fn complete_hollow_turn(
    state: &mut ReaderState,
    usage: Option<TurnUsage>,
    mapped: &mut MappedEvent,
) {
    let produced_tokens = usage.as_ref().is_some_and(|usage| usage.output_tokens > 0);
    let (code, message, terminal_class) = if produced_tokens {
        (
            DECODE_GAP_CODE,
            "OpenCode turn reported output tokens but no assistant content reached the harness"
                .to_string(),
            "decode_gap",
        )
    } else {
        (
            HOLLOW_BODY_CODE,
            "OpenCode turn completed with no assistant output (hollow body after stream truncation)"
                .to_string(),
            "hollow_idle",
        )
    };
    let provider_output_tokens = usage
        .as_ref()
        .map(|u| u.output_tokens as i64)
        .filter(|&t| t > 0);
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
        terminal_error_class: Some(terminal_class.to_string()),
        terminal_error_message: Some(message.clone()),
        provider_output_tokens,
    };
    complete_turn(state, Lifecycle::Failed, usage, mapped);
    mapped.events.push(ConversationEvent::Error {
        code: code.to_string(),
        message,
        evidence: Some(evidence),
    });
}

fn map_turn_usage(properties: &Value) -> Option<TurnUsage> {
    properties.get("usage").map(|usage| {
        let input_tokens = usage
            .pointer("/input_tokens")
            .and_then(Value::as_u64)
            .unwrap_or(0);
        TurnUsage {
            input_tokens,
            output_tokens: usage
                .pointer("/output_tokens")
                .and_then(Value::as_u64)
                .unwrap_or(0),
            total_input_tokens: Some(input_tokens),
            model: properties
                .get("model")
                .and_then(Value::as_str)
                .map(ToString::to_string),
            cost_usd: usage.get("cost").and_then(Value::as_f64),
            ..TurnUsage::default()
        }
    })
}

fn parse_session_state(properties: &Value) -> SessionState {
    let value = properties
        .get("status")
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
        if let (Some(turn_id), Some(content)) = (
            state.current_turn_id().map(str::to_string),
            delta_text(part),
        ) {
            state.mark_substantive();
            mapped
                .events
                .push(ConversationEvent::ReasoningDelta { turn_id, content });
        }
        return;
    }

    if part_type.contains("text") && !part_type.contains("tool") {
        if let (Some(turn_id), Some(content)) = (
            state.current_turn_id().map(str::to_string),
            delta_text(part),
        ) {
            state.mark_substantive();
            mapped
                .events
                .push(ConversationEvent::TextDelta { turn_id, content });
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
    // A canonical tool part is the model calling a tool — usable work.
    state.mark_substantive();
    let lifecycle = state.tools.entry(tool_id.clone()).or_default();

    if !lifecycle.started {
        lifecycle.started = true;
        mapped.events.push(ConversationEvent::ItemStarted {
            turn_id: turn_id.clone(),
            item: build_tool_item(part, &tool_id, Lifecycle::Running, false),
        });
    }

    if matches!(status, Lifecycle::Completed | Lifecycle::Failed) && !lifecycle.completed {
        lifecycle.completed = true;
        mapped.events.push(ConversationEvent::ItemCompleted {
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

fn map_diff(properties: &Value, state: &mut ReaderState, mapped: &mut MappedEvent) {
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
    mapped
        .events
        .push(ConversationEvent::DiffUpdated { turn_id, diff });
}

fn map_error(properties: &Value, state: &mut ReaderState, mapped: &mut MappedEvent) {
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

    if state.status == SessionState::Active {
        state.status = SessionState::Error;
        complete_turn(state, Lifecycle::Failed, None, mapped);
    }

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
    mapped.events.push(ConversationEvent::Error {
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

fn tool_status(part: &Value) -> Option<Lifecycle> {
    let raw = part
        .get("state")
        .and_then(Value::as_str)?
        .to_ascii_lowercase();
    match raw.as_str() {
        "running" => Some(Lifecycle::Running),
        "completed" => Some(Lifecycle::Completed),
        "failed" => Some(Lifecycle::Failed),
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
            exit_code: integer_field(part, "exitCode"),
            duration_ms: unsigned_field(part, "durationMs"),
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
    properties.get("sessionID").and_then(Value::as_str)
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    #[test]
    fn session_status_transitions_emit_turn_boundaries() {
        let mut state = ReaderState::new("session_1".to_string(), None, "opencode");

        let started = map_event(
            &json!({
                "type": "session.status",
                "properties": { "sessionID": "session_1", "status": "active" }
            }),
            &mut state,
        );
        assert_eq!(started.events.len(), 1);
        let started_turn_id = match &started.events[0] {
            ConversationEvent::TurnStarted { turn_id } => turn_id.clone(),
            other => panic!("expected TurnStarted, got {other:?}"),
        };

        // A substantive turn: some assistant text before it closes.
        let _ = map_event(
            &json!({
                "type": "message.part.updated",
                "properties": {
                    "sessionID": "session_1",
                    "part": { "id": "part_1", "type": "TextPart", "delta": "done" }
                }
            }),
            &mut state,
        );

        let completed = map_event(
            &json!({
                "type": "session.status",
                "properties": { "sessionID": "session_1", "status": "idle" }
            }),
            &mut state,
        );
        // This idle carries no usage, so the turn closes without a usage report.
        assert_eq!(completed.events.len(), 1);
        match &completed.events[0] {
            ConversationEvent::TurnCompleted { turn_id, status } => {
                assert_eq!(turn_id, &started_turn_id);
                assert_eq!(*status, Lifecycle::Completed);
            }
            other => panic!("expected TurnCompleted, got {other:?}"),
        }
    }

    #[test]
    fn session_status_idle_with_usage_emits_turn_usage() {
        let mut state = ReaderState::new("session_1".to_string(), None, "opencode");

        let started = map_event(
            &json!({
                "type": "session.status",
                "properties": { "sessionID": "session_1", "status": "active" }
            }),
            &mut state,
        );
        let started_turn_id = match &started.events[0] {
            ConversationEvent::TurnStarted { turn_id } => turn_id.clone(),
            other => panic!("expected TurnStarted, got {other:?}"),
        };

        // Substantive content so the close is a real completion, not a decode gap.
        let _ = map_event(
            &json!({
                "type": "message.part.updated",
                "properties": {
                    "sessionID": "session_1",
                    "part": { "id": "part_1", "type": "TextPart", "delta": "answer" }
                }
            }),
            &mut state,
        );

        let completed = map_event(
            &json!({
                "type": "session.status",
                "properties": {
                    "sessionID": "session_1",
                    "status": "idle",
                    "usage": {
                        "input_tokens": 222,
                        "output_tokens": 77,
                        "cost": 0.13
                    }
                }
            }),
            &mut state,
        );

        assert_eq!(completed.events.len(), 2);
        assert!(matches!(
            &completed.events[0],
            ConversationEvent::TurnCompleted { turn_id, status }
                if turn_id == &started_turn_id && *status == Lifecycle::Completed
        ));
        assert!(matches!(
            &completed.events[1],
            ConversationEvent::TurnUsage { turn_id, usage }
                if turn_id == &started_turn_id
                    && usage.input_tokens == 222
                    && usage.output_tokens == 77
                    && usage.cost_usd == Some(0.13)
        ));
    }

    #[test]
    fn text_part_maps_to_text_delta() {
        let mut state = ReaderState::new("session_1".to_string(), None, "opencode");
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
            ConversationEvent::TextDelta { content, .. } => assert_eq!(content, "hello"),
            other => panic!("expected TextDelta, got {other:?}"),
        }
    }

    #[test]
    fn tool_part_deduplicates_lifecycle_events() {
        let mut state = ReaderState::new("session_1".to_string(), None, "opencode");
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
            ConversationEvent::ItemStarted { .. }
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
            ConversationEvent::ItemCompleted {
                item: ConversationItem::Command { output, .. },
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
        let mut state = ReaderState::new("session_1".to_string(), None, "opencode");
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
        let mut state = ReaderState::new("session_1".to_string(), None, "opencode");
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
        let mut state = ReaderState::new("session_1".to_string(), None, "opencode");
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

        // A session error reports no usage, so no usage event is invented.
        assert_eq!(mapped.events.len(), 2);
        assert!(matches!(
            mapped.events[0],
            ConversationEvent::TurnCompleted {
                status: Lifecycle::Failed,
                ..
            }
        ));
        match &mapped.events[1] {
            ConversationEvent::Error { evidence, .. } => {
                let evidence = evidence
                    .as_ref()
                    .expect("session.error must carry durable evidence");
                assert_eq!(
                    evidence.terminal_error_class.as_deref(),
                    Some("session_error"),
                    "session.error evidence must name its root-cause class: {evidence:?}"
                );
                assert_eq!(
                    evidence.endpoint_class.as_deref(),
                    Some("upstream_provider"),
                    "session.error is an upstream report: {evidence:?}"
                );
            }
            other => panic!("expected Error, got {other:?}"),
        }
    }

    fn activate(state: &mut ReaderState) {
        let _ = map_event(
            &json!({
                "type": "session.status",
                "properties": { "sessionID": "session_1", "status": "active" }
            }),
            state,
        );
    }

    fn go_idle(state: &mut ReaderState, usage: Option<Value>) -> MappedEvent {
        let mut properties = json!({ "sessionID": "session_1", "status": "idle" });
        if let Some(usage) = usage {
            properties["usage"] = usage;
        }
        map_event(
            &json!({ "type": "session.status", "properties": properties }),
            state,
        )
    }

    #[test]
    fn hollow_idle_close_fails_instead_of_completing() {
        // opencode said the turn finished (idle), but nothing was ever emitted.
        // This is the SSE-truncation hollow body: it must fail, never complete.
        let mut state = ReaderState::new("session_1".to_string(), None, "opencode");
        activate(&mut state);

        let closed = go_idle(&mut state, None);

        assert_eq!(closed.events.len(), 2, "TurnCompleted + Error");
        assert!(matches!(
            closed.events[0],
            ConversationEvent::TurnCompleted {
                status: Lifecycle::Failed,
                ..
            }
        ));
        match &closed.events[1] {
            ConversationEvent::Error { code, evidence, .. } => {
                assert_eq!(code, HOLLOW_BODY_CODE);
                let evidence = evidence
                    .as_ref()
                    .expect("hollow-idle Error must carry durable evidence");
                assert_eq!(
                    evidence.endpoint_class.as_deref(),
                    Some("upstream_provider"),
                    "hollow-idle is an upstream truncation, not a harness stream drop: {evidence:?}"
                );
                assert_eq!(
                    evidence.terminal_error_class.as_deref(),
                    Some("hollow_idle"),
                    "hollow-idle must name its root-cause class: {evidence:?}"
                );
                assert_eq!(
                    evidence.provider.as_deref(),
                    Some("opencode"),
                    "evidence must name the provider: {evidence:?}"
                );
            }
            other => panic!("expected hollow-body Error, got {other:?}"),
        }
    }

    #[test]
    fn hollow_idle_close_with_output_tokens_is_a_decode_gap() {
        // Usage claims the model produced output, but no content reached us —
        // a harness decode gap, reported distinctly from a truly empty turn.
        let mut state = ReaderState::new("session_1".to_string(), None, "opencode");
        activate(&mut state);

        let closed = go_idle(&mut state, Some(json!({ "output_tokens": 42 })));

        match closed.events.last() {
            Some(ConversationEvent::Error { code, evidence, .. }) => {
                assert_eq!(code, DECODE_GAP_CODE);
                let evidence = evidence
                    .as_ref()
                    .expect("decode-gap Error must carry durable evidence");
                assert_eq!(
                    evidence.terminal_error_class.as_deref(),
                    Some("decode_gap"),
                    "decode-gap must name its root-cause class: {evidence:?}"
                );
                assert_eq!(
                    evidence.endpoint_class.as_deref(),
                    Some("upstream_provider"),
                    "decode-gap is an upstream truncation: {evidence:?}"
                );
                let tokens = evidence
                    .provider_output_tokens
                    .expect("decode-gap evidence must carry provider_output_tokens");
                assert!(
                    tokens > 0,
                    "decode-gap must prove the model produced tokens: {evidence:?}"
                );
                assert_eq!(
                    tokens, 42,
                    "provider_output_tokens must match the usage reading: {evidence:?}"
                );
            }
            other => panic!("expected decode-gap Error, got {other:?}"),
        }
        assert!(matches!(
            closed.events[0],
            ConversationEvent::TurnCompleted {
                status: Lifecycle::Failed,
                ..
            }
        ));
    }

    #[test]
    fn tool_only_turn_is_substantive_and_completes() {
        // A turn that only ran a tool (no prose) still did usable work.
        let mut state = ReaderState::new("session_1".to_string(), None, "opencode");
        activate(&mut state);
        let _ = map_event(
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

        let closed = go_idle(&mut state, None);

        assert!(matches!(
            closed.events[0],
            ConversationEvent::TurnCompleted {
                status: Lifecycle::Completed,
                ..
            }
        ));
        assert!(
            !closed
                .events
                .iter()
                .any(|event| matches!(event, ConversationEvent::Error { .. })),
            "a tool-bearing turn is not hollow"
        );
    }

    #[test]
    fn second_turn_hollowness_is_tracked_independently() {
        // A substantive turn must not leave the next turn marked substantive.
        let mut state = ReaderState::new("session_1".to_string(), None, "opencode");
        activate(&mut state);
        let _ = map_event(
            &json!({
                "type": "message.part.updated",
                "properties": {
                    "sessionID": "session_1",
                    "part": { "id": "p1", "type": "TextPart", "delta": "hi" }
                }
            }),
            &mut state,
        );
        let first = go_idle(&mut state, None);
        assert!(matches!(
            first.events[0],
            ConversationEvent::TurnCompleted {
                status: Lifecycle::Completed,
                ..
            }
        ));

        // Second turn produces nothing → hollow.
        activate(&mut state);
        let second = go_idle(&mut state, None);
        match second.events.last() {
            Some(ConversationEvent::Error { code, .. }) => assert_eq!(code, HOLLOW_BODY_CODE),
            other => panic!("expected hollow Error on the empty second turn, got {other:?}"),
        }
    }

    #[test]
    fn close_orphaned_turn_emits_failed_turn_close() {
        // Pre-content disconnect: turn is open but produced nothing.
        // close_orphaned_turn gives it a terminal Failed close so the
        // journal never carries an open turn past a disconnect.
        let mut state = ReaderState::new("session_1".to_string(), None, "opencode");
        activate(&mut state);
        assert!(state.turn_is_open());
        assert!(!state.turn_has_content());

        let events = state.close_orphaned_turn();

        assert_eq!(events.len(), 1, "TurnCompleted");
        assert!(matches!(
            events[0],
            ConversationEvent::TurnCompleted {
                status: Lifecycle::Failed,
                ..
            }
        ));
        // The turn is no longer open after closing.
        assert!(!state.turn_is_open());
    }

    #[test]
    fn close_orphaned_turn_after_partial_content_still_fails() {
        // Mid-stream disconnect: turn produced some content, then the
        // stream died. The partial output does not make the turn
        // successful — it still closes Failed.
        let mut state = ReaderState::new("session_1".to_string(), None, "opencode");
        activate(&mut state);
        let _ = map_event(
            &json!({
                "type": "message.part.updated",
                "properties": {
                    "sessionID": "session_1",
                    "part": { "id": "p1", "type": "TextPart", "delta": "partial" }
                }
            }),
            &mut state,
        );
        assert!(state.turn_is_open());
        assert!(state.turn_has_content());

        let events = state.close_orphaned_turn();

        assert!(matches!(
            events[0],
            ConversationEvent::TurnCompleted {
                status: Lifecycle::Failed,
                ..
            }
        ));
    }

    #[test]
    fn ignores_noncanonical_session_id_shape() {
        let mut state = ReaderState::new("session_1".to_string(), None, "opencode");
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
