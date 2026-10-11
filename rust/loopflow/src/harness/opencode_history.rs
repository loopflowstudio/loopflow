//! OpenCode user messages correlate requests; assistant steps remain subordinate.

use crate::id::AgentSessionId;
use std::collections::BTreeMap;

use anyhow::Result;
use serde_json::{json, Value};

use crate::chat::types::{ConversationEvent, Lifecycle};
use crate::session::{SessionEventKind, SessionTurnOrigin};
use crate::store::sqlite::SqliteStore;

#[derive(Debug, Default)]
pub(super) struct History {
    pub(super) owner: Option<super::agent_process::AttachmentOwner>,
    requests: BTreeMap<String, Request>,
    attention: super::attention::Attention,
}

#[derive(Debug, Default)]
struct Request {
    origin: Option<SessionTurnOrigin>,
    started: bool,
    completed: bool,
}

impl History {
    pub(super) fn new(owner: Option<super::agent_process::AttachmentOwner>) -> Self {
        Self {
            owner,
            ..Self::default()
        }
    }
    /// The attachment that fences native writes; absent until the server starts.
    pub(super) fn owner(&self) -> Result<super::agent_process::AttachmentOwner> {
        self.owner
            .clone()
            .ok_or_else(|| anyhow::anyhow!("OpenCode server not started"))
    }

    /// One server event of conversation `thread`, read for attention only.
    pub(super) fn attend(&mut self, thread: &AgentSessionId, event: &Value) {
        if let Some((store, session, attachment)) = &self.owner {
            self.attention.record(
                store,
                session,
                attachment,
                super::attention::opencode(event, thread.as_str()),
            );
        }
    }

    pub(super) fn request(&mut self) -> Result<String> {
        let id = format!("msg_{}", uuid::Uuid::new_v4().simple());
        let origin = self
            .owner
            .as_ref()
            .map(|(store, session, attachment)| store.session_turn_origin(session, attachment))
            .transpose()?;
        self.requests.insert(
            id.clone(),
            Request {
                origin,
                ..Request::default()
            },
        );
        Ok(id)
    }

    pub(super) fn observe(
        &mut self,
        thread: &AgentSessionId,
        messages: &[Value],
    ) -> Result<Vec<ConversationEvent>> {
        let mut events = Vec::new();
        let receipts = native_receipts(thread, messages);
        for (request, receipt) in receipts {
            let submitted = self.requests.get_mut(&request);
            if let Some(submitted) = submitted.as_ref().filter(|submitted| !submitted.started) {
                if let (Some((store, _, _)), Some(origin)) = (&self.owner, &submitted.origin) {
                    store.record_session_turn_origin(thread, &request, origin)?;
                }
                events.push(ConversationEvent::TurnStarted {
                    turn_id: request.clone(),
                });
            }
            if let Some((store, session, _)) = &self.owner {
                record_receipts(store, session, thread, &request, &receipt)?;
            }
            let Some(submitted) = submitted else { continue };
            submitted.started = true;
            if !receipt.completion.is_null() && !submitted.completed {
                submitted.completed = true;
                let status = match receipt.completion["status"].as_str() {
                    Some("completed") => Lifecycle::Completed,
                    Some("interrupted") => Lifecycle::Interrupted,
                    _ => Lifecycle::Failed,
                };
                if status != Lifecycle::Completed {
                    let error = &receipt.completion["error"];
                    let message = error["data"]["message"]
                        .as_str()
                        .or_else(|| error["message"].as_str())
                        .unwrap_or("OpenCode request did not complete");
                    events.push(ConversationEvent::Error {
                        code: "opencode_error".into(),
                        message: crate::harness::opencode::sanitize_error_message(message),
                        evidence: None,
                    });
                }
                events.push(ConversationEvent::TurnCompleted {
                    turn_id: request,
                    status,
                });
            }
        }
        Ok(events)
    }

    pub(super) fn admitted(&self, request: &str) -> bool {
        self.requests
            .get(request)
            .is_some_and(|request| request.started)
    }
}

#[derive(Debug, Default)]
struct Receipt {
    messages: Vec<Value>,
    completion: Value,
    output: Option<Value>,
}

fn native_receipts(thread: &AgentSessionId, messages: &[Value]) -> BTreeMap<String, Receipt> {
    let mut receipts = BTreeMap::<String, Receipt>::new();
    for message in messages {
        let info = &message["info"];
        if info["sessionID"] != thread.as_str() || info["role"] != "assistant" {
            continue;
        }
        let Some(parent) = info["parentID"].as_str() else {
            continue;
        };
        receipts
            .entry(parent.into())
            .or_default()
            .messages
            .push(message.clone());
    }
    for receipt in receipts.values_mut() {
        receipt.messages.sort_by(|a, b| {
            a["info"]["time"]["created"]
                .as_i64()
                .cmp(&b["info"]["time"]["created"].as_i64())
                .then_with(|| a["info"]["id"].as_str().cmp(&b["info"]["id"].as_str()))
        });
        let Some(last) = receipt.messages.last() else {
            continue;
        };
        let info = &last["info"];
        if info["time"]["completed"].is_null() {
            continue;
        }
        let pending = receipt.messages.iter().any(|message| {
            message["parts"].as_array().is_some_and(|parts| {
                parts.iter().any(|part| {
                    part["type"] == "tool"
                        && matches!(
                            part["state"]["status"].as_str(),
                            Some("pending" | "running")
                        )
                })
            })
        });
        if pending {
            continue;
        }
        let status = if !info["error"].is_null() {
            if info["error"]["name"] == "MessageAbortedError" {
                "interrupted"
            } else {
                "failed"
            }
        } else if info["finish"]
            .as_str()
            .is_some_and(|finish| !matches!(finish, "" | "tool-calls" | "unknown"))
        {
            "completed"
        } else {
            continue;
        };
        receipt.output = info
            .get("structured_output")
            .filter(|value| !value.is_null())
            .map(|value| json!({"value": value}));
        if receipt.output.is_none() {
            receipt.output = last["parts"]
                .as_array()
                .and_then(|parts| parts.iter().rev().find(|part| part["type"] == "text"))
                .and_then(|part| part["text"].as_str())
                .map(|text| json!({"text": text}));
        }
        receipt.completion = json!({"status":status,"error":info["error"],
            "started_at":receipt.messages[0]["info"]["time"]["created"],
            "completed_at":info["time"]["completed"],"message_id":info["id"],"provider":"opencode"});
    }
    receipts
}

fn record_receipts(
    store: &SqliteStore,
    session: &str,
    thread: &AgentSessionId,
    request: &str,
    receipt: &Receipt,
) -> Result<()> {
    for message in &receipt.messages {
        let info = &message["info"];
        if !info["time"]["completed"].is_null() {
            store.record_session_event(
                session,
                thread,
                request,
                SessionEventKind::Usage,
                &json!({"provider":"opencode","message":info}),
            )?;
        }
    }
    if let Some(output) = &receipt.output {
        store.record_session_event(session, thread, request, SessionEventKind::Output, output)?;
    }
    if !receipt.completion.is_null() {
        store.record_session_event(
            session,
            thread,
            request,
            SessionEventKind::Completed,
            &receipt.completion,
        )?;
    }
    Ok(())
}

pub(super) async fn read_messages(
    client: &reqwest::Client,
    endpoint: &str,
    thread: &AgentSessionId,
) -> Result<Vec<Value>> {
    Ok(client
        .get(format!("{endpoint}/session/{thread}/message"))
        .timeout(std::time::Duration::from_secs(10))
        .send()
        .await?
        .error_for_status()?
        .json()
        .await?)
}

// Native submission is bounded and serialized with attachment handoff. An
// uncertain HTTP result is retained as uncertain; never submit it twice here.
pub(super) async fn post(
    (store, session, attachment): super::agent_process::AttachmentOwner,
    url: String,
    payload: Value,
) -> Result<()> {
    tokio::task::spawn_blocking(move || {
        store.with_session_attachment(&session, &attachment, || {
            reqwest::blocking::Client::new()
                .post(url)
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

    use crate::id::LfProcessId;
    use crate::session::SessionEventKind;
    use crate::store::sqlite::SqliteStore;
    use serde_json::json;

    #[test]
    fn native_steps_recover_once_without_reassigning_usage_or_inventing_completion() {
        let home = tempfile::tempdir().unwrap();
        let path = home.path().join("store.db");
        let store = SqliteStore::open_ephemeral(&path).unwrap();
        let input = crate::session_record::new_artifact_key();
        let process = LfProcessId::new();
        let sql = rusqlite::Connection::open(&path).unwrap();
        sql.execute(
            "INSERT INTO processes(id,trace_id,started_at) VALUES(?1,'fixture',1)",
            [process.as_str()],
        )
        .unwrap();
        store.test_session("session", &input);
        let attachment = store
            .claim_session_attachment("session", None, &process, false)
            .unwrap();
        let mut history = History::new(Some((store.clone(), "session".into(), attachment.clone())));
        let request = history.request().unwrap();
        let message = |id: &str, input: u64, finish: &str| {
            json!({
            "info":{"id":id,"sessionID":"thread","parentID":request,"role":"assistant",
                "time":{"created":if id=="assistant-a" {1} else {3},"completed":if id=="assistant-a" {2} else {4}},
                "finish":finish,"tokens":{"input":input,"output":5,"reasoning":0,"cache":{"read":0,"write":0}}},
            "parts":[]})
        };
        let session = store.session("session").unwrap().unwrap();
        let mut replacement = session.clone();
        replacement.artifact_key = crate::session_record::new_artifact_key();
        store
            .replace_session_input(session.captured, replacement.clone())
            .unwrap();
        let second = LfProcessId::new();
        sql.execute(
            "INSERT INTO processes(id,trace_id,started_at) VALUES(?1,'fixture',1)",
            [second.as_str()],
        )
        .unwrap();
        store
            .claim_session_attachment("session", Some(&attachment), &second, true)
            .unwrap();
        assert!(!history.admitted(&request));
        for (index, input) in [20, 40, 30].into_iter().enumerate() {
            let events = history
                .observe(
                    &"thread".into(),
                    &[message("assistant-a", input, "tool-calls")],
                )
                .unwrap();
            assert_eq!(
                events
                    .iter()
                    .filter(|event| matches!(
                        event,
                        crate::chat::types::ConversationEvent::TurnStarted { .. }
                    ))
                    .count(),
                usize::from(index == 0)
            );
            assert!(history.admitted(&request));
            assert!(!events.iter().any(|event| matches!(
                event,
                crate::chat::types::ConversationEvent::TurnCompleted { .. }
            )));
        }
        let messages = [
            message("assistant-a", 30, "tool-calls"),
            message("assistant-b", 10, "stop"),
        ];
        for index in 0..2 {
            let events = history.observe(&"thread".into(), &messages).unwrap();
            assert_eq!(events.len(), usize::from(index == 0));
            assert!(events.iter().all(|event| matches!(
                event,
                crate::chat::types::ConversationEvent::TurnCompleted { .. }
            )));
            assert!(history.admitted(&request));
        }
        // A reconnect can recover history without claiming these requests or
        // emitting a new local turn boundary.
        let mut reconnected = History::new(history.owner.clone());
        assert!(reconnected
            .observe(&"thread".into(), &messages)
            .unwrap()
            .is_empty());
        assert!(!reconnected.admitted(&request));
        let recovered = store.input_history(input.as_str()).unwrap();
        assert_eq!(recovered.usage.input_tokens, Some(50));
        assert_eq!(recovered.usage.output_tokens, Some(10));
        assert!(
            recovered.usage.gaps > 0,
            "native decrease retains the existing reducer gap"
        );
        assert_eq!(
            recovered.recorded_outcome, None,
            "provider completion does not complete its Process"
        );
        assert_eq!(
            store
                .input_history(replacement.artifact_key.as_str())
                .unwrap()
                .usage
                .input_tokens,
            None
        );
        let rows = store.session_history("session", 0, 0).unwrap();
        assert_eq!(
            rows.iter()
                .filter(|row| row.kind == crate::session::SessionEventKind::Completed)
                .count(),
            1
        );
        assert!(rows
            .iter()
            .filter(|row| !matches!(
                row.kind,
                SessionEventKind::Captured | SessionEventKind::Observed
            ))
            .all(|row| row.lf_process_id.as_deref() == Some(process.as_str())));
    }
}
