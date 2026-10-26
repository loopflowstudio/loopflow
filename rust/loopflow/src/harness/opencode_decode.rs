//! OpenCode server events and native message snapshots, decoded once each.
//! Native input IDs freeze ownership; ordered execution markers settle
//! initiating turns. The server reports every conversation on one event
//! stream, so events are filtered to this decoder's conversation.

use std::collections::{BTreeMap, BTreeSet, HashMap};

use serde_json::{json, Value};

use super::fact::{self, Decoded, ProviderFact, ProviderTurn, Signal};
use crate::chat::types::{ConversationEvent, ConversationItem, FileEdit, Lifecycle};
use crate::id::AgentSessionId;

#[derive(Debug)]
pub(super) struct OpenCodeDecoder {
    thread: AgentSessionId,
    display: ReaderState,
    requests: BTreeMap<String, Request>,
    baseline: BTreeSet<String>,
    interactive: bool,
}

#[derive(Debug, Default)]
struct Request {
    steer: bool,
    started: bool,
    completed: bool,
}

/// What one native message snapshot established.
#[derive(Debug, Default)]
pub(super) struct Snapshot<'a> {
    pub(super) decoded: Vec<Decoded>,
    /// Assistant messages joined to the initiating input they answer. Display
    /// and permission replies share this join.
    pub(super) messages: Vec<(&'a str, &'a Value)>,
}

impl OpenCodeDecoder {
    pub(super) fn new(thread: AgentSessionId, model: Option<String>) -> Self {
        Self {
            display: ReaderState::new(thread.clone(), model),
            thread,
            requests: BTreeMap::new(),
            baseline: BTreeSet::new(),
            interactive: false,
        }
    }

    pub(super) fn display(&self) -> &ReaderState {
        &self.display
    }

    /// Messages before this launch belong to earlier work. An interactive
    /// terminal also adopts input the person submits natively.
    pub(super) fn seed(&mut self, messages: &[Value], interactive: bool) {
        self.baseline.extend(
            messages
                .iter()
                .filter_map(|m| m["id"].as_str().map(str::to_owned)),
        );
        self.interactive = interactive;
    }

    /// A native input ID for a new request or a steer into the running one.
    pub(super) fn request(&mut self, steer: bool) -> String {
        let id = format!("msg_{}", uuid::Uuid::new_v4().simple());
        self.requests.insert(
            id.clone(),
            Request {
                steer,
                ..Request::default()
            },
        );
        id
    }

    /// One server event: availability for this conversation.
    pub(super) fn event(&mut self, event: &Value) -> Vec<Decoded> {
        self.display.observe_event(event);
        fact::activity(signals(event, self.thread.as_str()))
    }

    pub(super) fn messages<'a>(&mut self, messages: &'a [Value]) -> Snapshot<'a> {
        let mut facts = Vec::new();
        let mut lifecycle = Vec::new();
        let mut joined = Vec::new();
        let mut active = None;
        let mut assistants = Vec::new();
        let turn = |id: &str| ProviderTurn {
            thread: self.thread.clone(),
            turn: id.to_owned(),
        };
        for message in messages {
            let id = message["id"].as_str().unwrap_or_default();
            if self.baseline.contains(id) {
                continue;
            }
            if message["type"] == "user" {
                if self.interactive && !self.requests.contains_key(id) {
                    self.requests.insert(
                        id.into(),
                        Request {
                            steer: active.is_some(),
                            ..Request::default()
                        },
                    );
                }
                if let Some(request) = self.requests.get_mut(id) {
                    if !request.started {
                        facts.push(Decoded::from(ProviderFact::TurnAnswered {
                            turn: turn(id),
                            request: id.into(),
                        }));
                        request.started = true;
                        if !request.steer {
                            lifecycle.push(ConversationEvent::TurnStarted { turn_id: id.into() });
                        }
                    }
                    if !request.steer {
                        // A subsequent explicit input cannot complete interrupted work.
                        active = Some(id);
                        assistants.clear();
                    }
                }
            }
            let Some(request_id) = active else {
                continue;
            };
            if message["type"] == "assistant" {
                joined.push((request_id, message));
                assistants.push(message);
                if !message["time"]["completed"].is_null() {
                    facts.push(
                        ProviderFact::Usage {
                            turn: turn(request_id),
                            receipt: json!({"provider":"opencode","message":message}),
                        }
                        .into(),
                    );
                }
            }
            if message["type"] != "idle" {
                continue;
            }
            let request = self
                .requests
                .get_mut(request_id)
                .expect("active request was admitted");
            if !request.completed {
                let output = assistants.last().and_then(|m| {
                    m["content"].as_array()?.iter().rev().find_map(|part| {
                        (part["type"] == "text")
                            .then(|| part["text"].as_str())
                            .flatten()
                            .filter(|text| !text.trim().is_empty())
                            .map(str::to_owned)
                    })
                });
                let status = match message["outcome"].as_str() {
                    Some("succeeded")
                        if output.is_some()
                            && assistants.iter().all(|m| {
                                m["error"].is_null() && !m["time"]["completed"].is_null()
                            }) =>
                    {
                        Lifecycle::Completed
                    }
                    Some("interrupted") => Lifecycle::Interrupted,
                    _ => Lifecycle::Failed,
                };
                let error = if status == Lifecycle::Completed {
                    Value::Null
                } else {
                    message.get("error").filter(|error| !error.is_null()).cloned().unwrap_or_else(
                        || json!({"message":"OpenCode execution ended without a successful answer"}))
                };
                if let Some(text) = &output {
                    facts.push(
                        ProviderFact::Output {
                            turn: turn(request_id),
                            answer: json!({"text":text}),
                        }
                        .into(),
                    );
                }
                facts.push(ProviderFact::TurnCompleted {
                    turn: Some(turn(request_id)),
                    status,
                    detail: json!({"status": match status { Lifecycle::Completed => "completed", Lifecycle::Interrupted => "interrupted", _ => "failed" },
                        "error":error, "completed_at":message["time"]["created"],"message_id":id,"provider":"opencode"}),
                }.into());
                if status != Lifecycle::Completed {
                    lifecycle.push(ConversationEvent::Error {
                        code: "opencode_error".into(),
                        message: super::opencode::sanitize_error_message(
                            error["message"]
                                .as_str()
                                .unwrap_or("OpenCode execution failed"),
                        ),
                        evidence: None,
                    });
                }
                lifecycle.push(ConversationEvent::TurnCompleted {
                    turn_id: request_id.into(),
                    status,
                });
                request.completed = true;
            }
            active = None;
            assistants.clear();
        }
        // Starts display before the content they frame; ends after it.
        let (started, ended): (Vec<_>, Vec<_>) = lifecycle
            .into_iter()
            .partition(|event| matches!(event, ConversationEvent::TurnStarted { .. }));
        for event in &started {
            self.display.observe_lifecycle(event);
        }
        facts.extend(started.into_iter().map(Decoded::from));
        facts.extend(
            self.display
                .observe_messages(&joined)
                .into_iter()
                .map(Decoded::from),
        );
        for event in &ended {
            self.display.observe_lifecycle(event);
        }
        facts.extend(ended.into_iter().map(Decoded::from));
        Snapshot {
            decoded: facts,
            messages: joined,
        }
    }
}

/// What one server event says about this conversation's availability. A
/// permission request is answered by its attached LfProcess and is not a
/// question for a person.
fn signals(event: &Value, thread: &str) -> Vec<Signal> {
    let data = if event["type"] == "form.created" {
        &event["data"]["form"]
    } else {
        &event["data"]
    };
    if data["sessionID"] != thread {
        return Vec::new();
    }
    let id = |key: &str| data[key].as_str().map(str::to_owned);
    match event["type"].as_str() {
        Some("session.execution.started" | "session.text.delta" | "session.reasoning.delta") => {
            vec![Signal::Working]
        }
        Some("session.tool.called") => id("id").map(Signal::ToolStarted).into_iter().collect(),
        Some("session.tool.success" | "session.tool.failed") => {
            id("id").map(Signal::ToolResolved).into_iter().collect()
        }
        Some("form.created") => id("id").map(Signal::InputRequested).into_iter().collect(),
        Some("form.replied" | "form.cancelled") => {
            id("id").map(Signal::InputResolved).into_iter().collect()
        }
        Some("session.execution.succeeded") => vec![Signal::Yielded(true)],
        Some("session.execution.failed" | "session.execution.interrupted") => {
            vec![Signal::Yielded(false)]
        }
        _ => Vec::new(),
    }
}

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
    fn new(session_id: AgentSessionId, model: Option<String>) -> Self {
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
    fn observe_event(&mut self, event: &Value) {
        if event["data"]["sessionID"] == self.session_id.as_str() {
            self.last_event_type = event["type"].as_str().map(str::to_owned);
            self.last_event_seq = Some(self.last_event_seq.map_or(0, |seq| seq + 1));
        }
    }
    fn observe_lifecycle(&mut self, event: &ConversationEvent) {
        match event {
            ConversationEvent::TurnStarted { .. } => {
                self.turn_open = true;
                self.turn_substantive = false;
            }
            ConversationEvent::TurnCompleted { .. } => self.turn_open = false,
            _ => {}
        }
    }
    fn observe_messages(&mut self, messages: &[(&str, &Value)]) -> Vec<ConversationEvent> {
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

#[cfg(test)]
mod tests {
    use serde_json::{json, Value};

    use super::{OpenCodeDecoder, Signal};
    use crate::chat::types::{ConversationEvent, Lifecycle};
    use crate::harness::fact::{self, Decoded, ProviderFact};

    /// Server events carry availability only; turns come from snapshots. A
    /// permission the attachment answers and another conversation's idle add
    /// nothing.
    #[test]
    fn trace_opencode_question() {
        let mut decoder = OpenCodeDecoder::new("ses_trace".into(), None);
        let decoded: Vec<Decoded> = fact::trace("opencode_question.jsonl")
            .iter()
            .flat_map(|event| decoder.event(event))
            .collect();
        assert_eq!(
            fact::fact_lines(&decoded),
            [
                "Activity Working",
                r#"Activity ToolStarted("call_read")"#,
                r#"Activity ToolResolved("call_read")"#,
                r#"Activity ToolStarted("call_question")"#,
                r#"Activity InputRequested("form_1")"#,
                r#"Activity InputResolved("form_1")"#,
                r#"Activity ToolResolved("call_question")"#,
                "Activity Yielded(true)",
            ]
        );
    }

    fn display(decoded: Vec<Decoded>) -> Vec<ConversationEvent> {
        decoded
            .into_iter()
            .filter_map(|decoded| match decoded {
                Decoded::Display(event) => Some(event),
                Decoded::Fact(_) => None,
            })
            .collect()
    }

    #[test]
    fn empty_success_is_failure_and_explicit_new_input_cannot_complete_lost_work() {
        let mut decoder = OpenCodeDecoder::new("thread".into(), None);
        let interrupted = decoder.request(false);
        let next = decoder.request(false);
        let messages = vec![
            json!({"type":"user","id":interrupted}),
            json!({"type":"assistant","id":"partial","content":[{"type":"text","text":"Partial"}]}),
            json!({"type":"user","id":next}),
            json!({"type":"idle","id":"empty","outcome":"succeeded"}),
        ];
        let events = display(decoder.messages(&messages).decoded);
        assert!(events.iter().any(|e| matches!(e, ConversationEvent::TurnCompleted{turn_id,status:Lifecycle::Failed} if turn_id == &next)));
        assert!(!events.iter().any(
            |e| matches!(e, ConversationEvent::TurnCompleted{turn_id,..} if turn_id == &interrupted)
        ));
    }

    #[test]
    fn steers_share_their_turn_and_settle_once() {
        let mut decoder = OpenCodeDecoder::new("native".into(), None);
        let request = decoder.request(false);
        let steer = decoder.request(true);
        let messages = vec![
            json!({"type":"user","id":request}),
            json!({"type":"assistant","id":"step1","time":{"completed":2},"content":[]}),
            json!({"type":"user","id":steer}),
            json!({"type":"assistant","id":"step2","time":{"completed":4},"content":[{"type":"text","text":"Result"}]}),
            json!({"type":"idle","id":"idle","outcome":"succeeded","time":{"created":5}}),
        ];
        let snapshot = decoder.messages(&messages);
        assert!(snapshot.messages.iter().all(|(id, _)| *id == request));
        let started: Vec<_> = snapshot
            .decoded
            .iter()
            .filter_map(|decoded| match decoded.fact() {
                Some(ProviderFact::TurnAnswered { request, .. }) => Some(request.clone()),
                _ => None,
            })
            .collect();
        assert_eq!(started, [request.clone(), steer]);
        assert!(
            matches!(&display(snapshot.decoded)[..], [ConversationEvent::TurnStarted{turn_id}, ConversationEvent::TextDelta{..}, ConversationEvent::TurnCompleted{turn_id:done,status:Lifecycle::Completed}] if turn_id == &request && done == &request)
        );
        assert!(decoder
            .messages(&messages)
            .decoded
            .iter()
            .all(|decoded| !matches!(
                decoded,
                Decoded::Fact(
                    ProviderFact::TurnAnswered { .. } | ProviderFact::TurnCompleted { .. }
                )
            )));
    }

    #[test]
    fn resumed_terminal_displays_only_new_inputs_and_keeps_steers_with_their_turn() {
        let mut messages = vec![
            json!({"type":"user","id":"old"}),
            json!({"type":"assistant","id":"old-answer"}),
        ];
        let mut decoder = OpenCodeDecoder::new("thread".into(), None);
        decoder.seed(&messages, true);
        messages.extend([
            json!({"type":"user","id":"first"}),
            json!({"type":"assistant","id":"first-step"}),
            json!({"type":"user","id":"steer"}),
            json!({"type":"assistant","id":"steered-step"}),
            json!({"type":"idle","id":"boundary","outcome":"interrupted"}),
            json!({"type":"user","id":"next"}),
            json!({"type":"assistant","id":"next-step"}),
        ]);
        let snapshot = decoder.messages(&messages);
        let output: Vec<_> = snapshot
            .messages
            .iter()
            .map(|(turn, message)| (*turn, message["id"].as_str().unwrap()))
            .collect();
        assert_eq!(
            output,
            [
                ("first", "first-step"),
                ("first", "steered-step"),
                ("next", "next-step")
            ]
        );
    }

    #[test]
    fn permission_requests_and_other_conversations_ask_nothing() {
        let mut decoder = OpenCodeDecoder::new("thread".into(), None);
        let signals = |decoder: &mut OpenCodeDecoder, event: Value| -> Vec<Signal> {
            decoder
                .event(&event)
                .into_iter()
                .filter_map(|decoded| match decoded {
                    Decoded::Fact(ProviderFact::Activity(signal)) => Some(signal),
                    _ => None,
                })
                .collect()
        };
        assert!(signals(
            &mut decoder,
            json!({"type": "permission.asked", "data": {"sessionID": "thread", "id": "permission"}})
        )
        .is_empty());
        assert!(signals(
            &mut decoder,
            json!({"type": "session.execution.succeeded", "data": {"sessionID": "other"}})
        )
        .is_empty());
        assert_eq!(
            signals(
                &mut decoder,
                json!({"type": "form.created", "data": {"form": {"sessionID": "thread", "id": "q"}}})
            ),
            [Signal::InputRequested("q".into())]
        );
    }
}
