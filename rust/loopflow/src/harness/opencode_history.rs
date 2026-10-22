//! Native input IDs freeze ownership; ordered execution markers settle initiating turns.

use std::collections::{BTreeMap, BTreeSet};

use anyhow::Result;
use serde_json::{json, Value};

use crate::chat::types::{ConversationEvent, Lifecycle};
use crate::id::AgentSessionId;
use crate::session::{SessionEventKind, SessionTurnOrigin};

#[derive(Debug, Default)]
pub(super) struct History {
    pub(super) owner: Option<super::agent_process::AttachmentOwner>,
    requests: BTreeMap<String, Request>,
    baseline: BTreeSet<String>,
    interactive: bool,
    activity: super::activity_tracker::SessionActivityTracker,
}

#[derive(Debug, Default)]
struct Request {
    origin: Option<SessionTurnOrigin>,
    steer: bool,
    started: bool,
    completed: bool,
}

#[derive(Debug, Default)]
pub(super) struct Observation<'a> {
    pub(super) events: Vec<ConversationEvent>,
    // Display, permission replies and usage share the same initiating-input join.
    pub(super) messages: Vec<(&'a str, &'a Value)>,
}

impl History {
    pub(super) fn new(owner: Option<super::agent_process::AttachmentOwner>) -> Self {
        Self {
            owner,
            ..Self::default()
        }
    }

    pub(super) fn owner(&self) -> Result<super::agent_process::AttachmentOwner> {
        self.owner
            .clone()
            .ok_or_else(|| anyhow::anyhow!("OpenCode server not started"))
    }

    pub(super) fn seed(&mut self, messages: &[Value], interactive: bool) {
        self.baseline.extend(
            messages
                .iter()
                .filter_map(|m| m["id"].as_str().map(str::to_owned)),
        );
        self.interactive = interactive;
    }

    pub(super) fn attend(&mut self, thread: &AgentSessionId, event: &Value) {
        if let Some((store, session, attachment)) = &self.owner {
            self.activity.record(
                store,
                session,
                attachment,
                super::activity_tracker::opencode(event, thread.as_str()),
            );
        }
    }

    pub(super) fn request(&mut self) -> Result<String> {
        self.register(false)
    }

    pub(super) fn steer(&mut self) -> Result<String> {
        self.register(true)
    }

    fn register(&mut self, steer: bool) -> Result<String> {
        let id = format!("msg_{}", uuid::Uuid::new_v4().simple());
        self.register_id(&id, steer)?;
        Ok(id)
    }

    fn register_id(&mut self, id: &str, steer: bool) -> Result<()> {
        let origin = self
            .owner
            .as_ref()
            .map(|(store, session, attachment)| store.session_turn_origin(session, attachment))
            .transpose()?;
        self.requests.insert(
            id.into(),
            Request {
                origin,
                steer,
                ..Request::default()
            },
        );
        Ok(())
    }

    pub(super) fn observe<'a>(
        &mut self,
        thread: &AgentSessionId,
        messages: &'a [Value],
    ) -> Result<Observation<'a>> {
        let mut observed = Observation::default();
        let mut active = None;
        let mut assistants = Vec::new();
        for message in messages {
            let id = message["id"].as_str().unwrap_or_default();
            if self.baseline.contains(id) {
                continue;
            }
            if message["type"] == "user" {
                if self.interactive && !self.requests.contains_key(id) {
                    self.register_id(id, active.is_some())?;
                }
                if let Some(request) = self.requests.get_mut(id) {
                    if !request.started {
                        if let (Some((store, _, _)), Some(origin)) = (&self.owner, &request.origin)
                        {
                            store.record_session_turn_origin(thread, id, origin)?;
                        }
                        request.started = true;
                        if !request.steer {
                            observed
                                .events
                                .push(ConversationEvent::TurnStarted { turn_id: id.into() });
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
                observed.messages.push((request_id, message));
                assistants.push(message);
                if !message["time"]["completed"].is_null() {
                    if let Some((store, session, _)) = &self.owner {
                        store.record_session_event(
                            session,
                            thread,
                            request_id,
                            SessionEventKind::Usage,
                            &json!({"provider":"opencode","message":message}),
                        )?;
                    }
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
                if let Some((store, session, _)) = &self.owner {
                    if let Some(text) = &output {
                        store.record_session_event(
                            session,
                            thread,
                            request_id,
                            SessionEventKind::Output,
                            &json!({"text":text}),
                        )?;
                    }
                    store.record_session_event(session, thread, request_id, SessionEventKind::Completed,
                        &json!({"status": match status { Lifecycle::Completed => "completed", Lifecycle::Interrupted => "interrupted", _ => "failed" },
                            "error":error, "completed_at":message["time"]["created"],"message_id":id,"provider":"opencode"}))?;
                }
                if status != Lifecycle::Completed {
                    observed.events.push(ConversationEvent::Error {
                        code: "opencode_error".into(),
                        message: super::opencode::sanitize_error_message(
                            error["message"]
                                .as_str()
                                .unwrap_or("OpenCode execution failed"),
                        ),
                        evidence: None,
                    });
                }
                observed.events.push(ConversationEvent::TurnCompleted {
                    turn_id: request_id.into(),
                    status,
                });
                request.completed = true;
            }
            active = None;
            assistants.clear();
        }
        Ok(observed)
    }
}

pub(super) async fn read_messages(
    client: &reqwest::Client,
    endpoint: &str,
    thread: &AgentSessionId,
) -> Result<Vec<Value>> {
    // Export retains completed history through compaction; context includes live
    // assistant/tool state omitted from export until settlement.
    let mut exported: Value = client
        .get(format!(
            "{endpoint}/api/experimental/session/{thread}/export"
        ))
        .timeout(std::time::Duration::from_secs(10))
        .send()
        .await?
        .error_for_status()?
        .json()
        .await?;
    let mut live: Value = client
        .get(format!("{endpoint}/api/session/{thread}/context"))
        .timeout(std::time::Duration::from_secs(10))
        .send()
        .await?
        .error_for_status()?
        .json()
        .await?;
    let mut messages = std::mem::take(
        exported
            .pointer_mut("/data/messages")
            .and_then(Value::as_array_mut)
            .ok_or_else(|| anyhow::anyhow!("OpenCode export has no messages"))?,
    );
    for message in live
        .get_mut("data")
        .and_then(Value::as_array_mut)
        .ok_or_else(|| anyhow::anyhow!("OpenCode context has no messages"))?
        .drain(..)
    {
        if let Some(existing) = messages.iter_mut().find(|m| m["id"] == message["id"]) {
            *existing = message;
        } else {
            messages.push(message);
        }
    }
    Ok(messages)
}

// Freeze dispatch under the attachment fence. Uncertain requests are never replayed.
pub(super) async fn post(
    owner: super::agent_process::AttachmentOwner,
    authorization: reqwest::header::HeaderValue,
    url: String,
    payload: Value,
) -> Result<()> {
    tokio::task::spawn_blocking(move || {
        let (store, session, attachment) = owner;
        store.with_session_dispatch(&session, &attachment, || {
            reqwest::blocking::Client::new()
                .post(url)
                .header(reqwest::header::AUTHORIZATION, authorization)
                .timeout(std::time::Duration::from_secs(10))
                .json(&payload)
                .send()
                .and_then(reqwest::blocking::Response::error_for_status)
                .map(|_| ())
                .map_err(|error| {
                    crate::store::StoreError::InvalidData(format!(
                        "OpenCode native request: {error}"
                    ))
                })
        })
    })
    .await??;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::History;
    use crate::chat::types::{ConversationEvent, Lifecycle};
    use crate::id::LfProcessId;
    use crate::store::sqlite::SqliteStore;
    use serde_json::json;

    #[test]
    fn steers_share_execution_but_keep_the_initiating_capture_after_handoff() {
        let home = tempfile::tempdir().unwrap();
        let path = home.path().join("store.db");
        let store = SqliteStore::open_ephemeral(&path).unwrap();
        let input = crate::session_record::new_artifact_key();
        store.test_session("session", &input);
        let sql = rusqlite::Connection::open(path).unwrap();
        let process = LfProcessId::new();
        sql.execute(
            "INSERT INTO processes(id,trace_id,started_at) VALUES(?1,'fixture',1)",
            [&process],
        )
        .unwrap();
        let attachment = store
            .claim_session_attachment("session", None, &process, false)
            .unwrap();
        let mut history = History::new(Some((store.clone(), "session".into(), attachment.clone())));
        let request = history.request().unwrap();
        let steer = history.steer().unwrap();
        let previous = store.session("session").unwrap().unwrap();
        let mut next = previous.clone();
        next.artifact_key = Some(crate::session_record::new_artifact_key());
        store
            .replace_session_input(previous.captured, next.clone())
            .unwrap();
        let replacement = LfProcessId::new();
        sql.execute(
            "INSERT INTO processes(id,trace_id,started_at) VALUES(?1,'fixture',1)",
            [&replacement],
        )
        .unwrap();
        store
            .claim_session_attachment("session", Some(&attachment), &replacement, true)
            .unwrap();
        let messages = vec![
            json!({"type":"user","id":request}),
            json!({"type":"assistant","id":"step1","time":{"completed":2},"content":[{"type":"tool","id":"tool","name":"shell","state":{"status":"completed"}}],"tokens":{"input":20,"output":5}}),
            json!({"type":"user","id":steer}),
            json!({"type":"assistant","id":"step2","time":{"completed":4},"content":[{"type":"text","text":"Result"}],"tokens":{"input":10,"output":5}}),
            json!({"type":"idle","id":"idle","outcome":"succeeded","time":{"created":5}}),
        ];
        let observed = history.observe(&"native".into(), &messages).unwrap();
        let events = observed.events;
        assert!(
            matches!(&events[..], [ConversationEvent::TurnStarted{turn_id}, ConversationEvent::TurnCompleted{turn_id:done,status:Lifecycle::Completed}] if turn_id == &request && done == &request)
        );
        assert!(history
            .observe(&"native".into(), &messages)
            .unwrap()
            .events
            .is_empty());
        let completion: String = sql
            .query_row(
                "SELECT payload FROM session_events WHERE kind='completed' AND provider_turn=?1",
                [&request],
                |row| row.get(0),
            )
            .unwrap();
        let completion: serde_json::Value = serde_json::from_str(&completion).unwrap();
        assert_eq!(completion["status"], "completed");
        assert!(completion["error"].is_null());
        let usage = store.input_history(input.as_str()).unwrap().usage;
        assert_eq!(usage.input_tokens, Some(30));
        assert_eq!(usage.output_tokens, Some(10));
        assert_eq!(
            store
                .input_history(next.input_key().unwrap().as_str())
                .unwrap()
                .usage
                .input_tokens,
            None
        );
        assert_eq!(observed.messages.len(), 2);
        assert!(observed.messages.iter().all(|(id, _)| *id == request));
    }

    #[test]
    fn empty_success_is_failure_and_explicit_new_input_cannot_complete_lost_work() {
        let mut history = History::default();
        let interrupted = history.request().unwrap();
        let next = history.request().unwrap();
        let messages = vec![
            json!({"type":"user","id":interrupted}),
            json!({"type":"assistant","id":"partial","content":[{"type":"text","text":"Partial"}]}),
            json!({"type":"user","id":next}),
            json!({"type":"idle","id":"empty","outcome":"succeeded"}),
        ];
        let events = history.observe(&"thread".into(), &messages).unwrap().events;
        assert!(events.iter().any(|e| matches!(e, ConversationEvent::TurnCompleted{turn_id,status:Lifecycle::Failed} if turn_id == &next)));
        assert!(!events.iter().any(
            |e| matches!(e, ConversationEvent::TurnCompleted{turn_id,..} if turn_id == &interrupted)
        ));
    }

    #[test]
    fn resumed_terminal_displays_only_new_inputs_and_keeps_steers_with_their_turn() {
        let mut messages = vec![
            json!({"type":"user","id":"old"}),
            json!({"type":"assistant","id":"old-answer"}),
        ];
        let mut history = History::default();
        history.seed(&messages, true);
        messages.extend([
            json!({"type":"user","id":"first"}),
            json!({"type":"assistant","id":"first-step"}),
            json!({"type":"user","id":"steer"}),
            json!({"type":"assistant","id":"steered-step"}),
            json!({"type":"idle","id":"boundary","outcome":"interrupted"}),
            json!({"type":"user","id":"next"}),
            json!({"type":"assistant","id":"next-step"}),
        ]);
        let observed = history.observe(&"thread".into(), &messages).unwrap();
        let output: Vec<_> = observed
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
}
