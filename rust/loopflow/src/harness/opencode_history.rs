//! OpenCode user messages correlate requests; assistant steps remain subordinate.

use crate::id::AgentSessionId;
use std::collections::{btree_map::Entry, BTreeMap};

use anyhow::{anyhow, Context, Result};
use serde_json::{json, Value};

use crate::chat::types::{ConversationEvent, Lifecycle};
use crate::session::SessionEventKind;
use crate::store::sqlite::SqliteStore;

#[derive(Debug, Default)]
pub(super) struct History {
    pub(super) owner: Option<super::agent_process::AttachmentOwner>,
    requests: BTreeMap<String, Request>,
    attention: super::attention::Attention,
}

#[derive(Debug, Default)]
struct Request {
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
    /// Frozen launch authority for prompts, abort and stop; never refreshed from config.
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

    pub(super) fn request(&self, thread: &AgentSessionId) -> Result<String> {
        let id = format!("msg_{}", uuid::Uuid::new_v4().simple());
        let (store, session, attachment) = self.owner()?;
        let origin = store.session_turn_origin(&session, &attachment)?;
        store.record_session_request(thread, &id, &origin)?;
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
            // Origins are immutable. Recover once; subsequent observations only
            // advance native receipts and this reader's emitted boundaries.
            if let Entry::Vacant(entry) = self.requests.entry(request.clone()) {
                let Some((store, session, attachment)) = &self.owner else {
                    continue;
                };
                if let Some((origin, completed)) =
                    store.session_request(session, thread, &request)?
                {
                    // Attribution is evidence, not authority. Only requests of
                    // this same surviving provider belong to the live reader.
                    if origin.agent_process_id == attachment.agent_process_id {
                        store.record_session_turn_origin(thread, &request, &origin)?;
                        entry.insert(Request {
                            started: completed,
                            completed,
                        });
                    }
                }
            }
            if let Some((store, session, _)) = &self.owner {
                record_receipts(store, session, thread, &request, &receipt)?;
            }
            let Some(submitted) = self.requests.get_mut(&request) else {
                continue;
            };
            if !submitted.started {
                events.push(ConversationEvent::TurnStarted {
                    turn_id: request.clone(),
                });
            }
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
struct Receipt<'a> {
    messages: Vec<&'a Value>,
    completion: Value,
    output: Option<Value>,
}

fn native_receipts<'a>(
    thread: &AgentSessionId,
    messages: &'a [Value],
) -> BTreeMap<String, Receipt<'a>> {
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
            .push(message);
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
    receipt: &Receipt<'_>,
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

/// Pending native permissions own reply eligibility. SSE only wakes this read:
/// duplicate edges and reconnect cannot repeat an already attempted reply.
pub(super) async fn reply_pending_permissions(
    client: &reqwest::Client,
    endpoint: &str,
    thread: &AgentSessionId,
    owner: &super::agent_process::AttachmentOwner,
) -> Result<()> {
    let mut pending: Vec<Value> = client
        .get(format!("{endpoint}/permission"))
        .timeout(std::time::Duration::from_secs(10))
        .send()
        .await?
        .error_for_status()?
        .json()
        .await?;
    pending.retain(|permission| permission["sessionID"] == thread.as_str());
    if pending.is_empty() {
        return Ok(());
    }
    // Read after permissions: each pending tool must be present in this snapshot.
    let messages = read_messages(client, endpoint, thread).await?;
    for permission in pending {
        let id = permission["id"]
            .as_str()
            .ok_or_else(|| anyhow::anyhow!("OpenCode permission has no identity"))?;
        let assistant = permission["tool"]["messageID"].as_str().ok_or_else(|| {
            anyhow::anyhow!("OpenCode permission has no originating assistant message")
        })?;
        let request = messages
            .iter()
            .find(|message| message["info"]["id"] == assistant)
            .and_then(|message| message["info"]["parentID"].as_str())
            .ok_or_else(|| anyhow::anyhow!("OpenCode permission has no originating request"))?;
        let (store, session, attachment) = owner;
        if !store
            .session_request(session, thread, request)?
            .is_some_and(|(origin, _)| origin.agent_process_id == attachment.agent_process_id)
        {
            return Err(anyhow::anyhow!(
                "OpenCode permission belongs to an unselected request"
            ));
        }
        let owner = owner.clone();
        let endpoint = endpoint.to_string();
        let thread = thread.clone();
        let id = id.to_string();
        let request = request.to_string();
        with_attached_http(owner, move |(store, session, _), client| {
            let first_attempt =
                store.record_session_permission_reply(session, &thread, &id, &request)?;
            if first_attempt {
                let response = client
                    .post(format!("{endpoint}/permission/{id}/reply"))
                    .json(&json!({"reply":"once"}))
                    .send()
                    .and_then(reqwest::blocking::Response::error_for_status);
                if response.is_ok() {
                    return Ok(());
                }
            }
            // A lost response can follow acceptance. Readback may settle
            // absence, but a still-pending permission never permits replay.
            let pending: Vec<Value> = client
                .get(format!("{endpoint}/permission"))
                .send()
                .and_then(reqwest::blocking::Response::error_for_status)
                .and_then(|response| response.json())
                .context("OpenCode permission reply readback")?;
            if pending.iter().any(|permission| {
                permission["sessionID"] == thread.as_str() && permission["id"] == id
            }) {
                return Err(anyhow!(
                    "OpenCode permission {id} reply is uncertain; not replaying"
                ));
            }
            Ok(())
        })
        .await?;
    }
    Ok(())
}

/// Run bounded HTTP and its durable receipts under frozen attachment authority.
/// The worker keeps the fence through caller cancellation; it never retries I/O.
pub(super) async fn with_attached_http<T: Send + 'static>(
    owner: super::agent_process::AttachmentOwner,
    write: impl FnOnce(&super::agent_process::AttachmentOwner, &reqwest::blocking::Client) -> Result<T>
        + Send
        + 'static,
) -> Result<T> {
    tokio::task::spawn_blocking(move || {
        let client = reqwest::blocking::Client::builder()
            .timeout(std::time::Duration::from_secs(10))
            .build()?;
        let (store, session, attachment) = &owner;
        // Keep operation errors intact; the store only owns attachment validation.
        store.with_session_attachment(session, attachment, || Ok(write(&owner, &client)))?
    })
    .await?
}

// An uncertain HTTP result never permits a second submission here.
pub(super) async fn post(
    owner: super::agent_process::AttachmentOwner,
    url: String,
    payload: Value,
) -> Result<()> {
    with_attached_http(owner, move |_, client| {
        client
            .post(url)
            .json(&payload)
            .send()
            .and_then(reqwest::blocking::Response::error_for_status)
            .context("OpenCode native request")?;
        Ok(())
    })
    .await
}

#[cfg(test)]
mod tests {
    use super::History;

    use crate::id::LfProcessId;
    use crate::session::SessionEventKind;
    use crate::store::sqlite::SqliteStore;
    use serde_json::json;

    #[tokio::test]
    async fn permission_recovery_never_replays_an_uncertain_reply() {
        use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
        use std::sync::Arc;
        use tokio::io::{AsyncBufReadExt, AsyncReadExt, AsyncWriteExt, BufReader};

        for accepted in [true, false] {
            let home = tempfile::tempdir().unwrap();
            let path = home.path().join("store.db");
            let store = SqliteStore::open_ephemeral(&path).unwrap();
            let process = LfProcessId::new();
            rusqlite::Connection::open(&path)
                .unwrap()
                .execute(
                    "INSERT INTO processes(id,trace_id,started_at) VALUES(?1,'fixture',1)",
                    [process.as_str()],
                )
                .unwrap();
            store.test_session("session", &crate::session_record::new_artifact_key());
            let first = store
                .claim_session_attachment("session", None, &process, false)
                .unwrap();
            let thread = "thread".into();
            let owner = (store.clone(), "session".into(), first.clone());
            let request = History::new(Some(owner.clone())).request(&thread).unwrap();
            let messages = vec![json!({"info":{"id":"assistant","sessionID":"thread",
                "parentID":request,"role":"assistant","time":{"created":1}},"parts":[]})];
            let pending = Arc::new(AtomicBool::new(true));
            let replies = Arc::new(AtomicUsize::new(0));
            let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
            let endpoint = format!("http://{}", listener.local_addr().unwrap());
            let server = {
                let pending = pending.clone();
                let replies = replies.clone();
                let messages = messages.clone();
                tokio::spawn(async move {
                    loop {
                        let (socket, _) = listener.accept().await.unwrap();
                        let mut socket = BufReader::new(socket);
                        let mut line = String::new();
                        socket.read_line(&mut line).await.unwrap();
                        let post = line.starts_with("POST /permission/permission/reply ");
                        let read_messages = line.starts_with("GET /session/thread/message ");
                        assert!(post || read_messages || line.starts_with("GET /permission "));
                        let mut length = 0;
                        loop {
                            line.clear();
                            socket.read_line(&mut line).await.unwrap();
                            if line == "\r\n" {
                                break;
                            }
                            if let Some(value) = line.to_lowercase().strip_prefix("content-length:")
                            {
                                length = value.trim().parse::<usize>().unwrap();
                            }
                        }
                        if post {
                            let mut body = vec![0; length];
                            socket.read_exact(&mut body).await.unwrap();
                            assert_eq!(
                                serde_json::from_slice::<serde_json::Value>(&body).unwrap(),
                                json!({"reply":"once"})
                            );
                            replies.fetch_add(1, Ordering::SeqCst);
                            if accepted {
                                pending.store(false, Ordering::SeqCst);
                            }
                            // Lose the response after the provider may have applied it.
                            continue;
                        }
                        let body = if read_messages { json!(messages) } else if pending.load(Ordering::SeqCst) {
                            json!([{"id":"permission","sessionID":"thread","tool":{"messageID":"assistant"}}])
                        } else { json!([]) }.to_string();
                        socket.get_mut().write_all(format!("HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}", body.len()).as_bytes()).await.unwrap();
                    }
                })
            };
            let client = reqwest::Client::new();
            let result =
                super::reply_pending_permissions(&client, &endpoint, &thread, &owner).await;
            assert_eq!(result.is_ok(), accepted);
            assert_eq!(replies.load(Ordering::SeqCst), 1);
            let current = store
                .claim_session_attachment("session", Some(&first), &process, false)
                .unwrap();
            // Reopen without reconstructing a reader: saved intent alone supplies
            // attribution and deduplication, even before any native observation.
            let reopened = SqliteStore::open_ephemeral(&path).unwrap();
            let recovered = (reopened, "session".into(), current);
            let result =
                super::reply_pending_permissions(&client, &endpoint, &thread, &recovered).await;
            assert_eq!(result.is_ok(), accepted);
            assert_eq!(replies.load(Ordering::SeqCst), 1);
            // Even a newly pending observation cannot grant a stale reader writes.
            pending.store(true, Ordering::SeqCst);
            assert!(
                super::reply_pending_permissions(&client, &endpoint, &thread, &owner)
                    .await
                    .is_err()
            );
            assert_eq!(replies.load(Ordering::SeqCst), 1);
            server.abort();
        }
    }

    #[test]
    fn pending_request_survives_launcher_loss_without_replay_or_reassigned_usage() {
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
        let history = History::new(Some((store.clone(), "session".into(), attachment.clone())));
        let request = history.request(&"thread".into()).unwrap();
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
        let current = store
            .claim_session_attachment("session", Some(&attachment), &second, false)
            .unwrap();
        assert_eq!(current.agent_process_id, attachment.agent_process_id);
        assert_ne!(current.token, attachment.token);
        assert!(history.request(&"thread".into()).is_err());
        assert!(!history.admitted(&request));
        assert!(
            !store
                .session_history("session", 0, 0)
                .unwrap()
                .iter()
                .any(|row| {
                    matches!(
                        row.kind,
                        SessionEventKind::Started | SessionEventKind::Completed
                    )
                }),
            "pre-submission evidence cannot invent native admission"
        );
        // Lose every launcher-local object before the first native receipt.
        drop(history);
        let reopened = SqliteStore::open_ephemeral(&path).unwrap();
        let mut history = History::new(Some((reopened, "session".into(), current)));
        assert!(history
            .observe(&"other-thread".into(), &[])
            .unwrap()
            .is_empty());
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
        // A reconnect retains settled requests without emitting new turn boundaries.
        let mut reconnected = History::new(history.owner.clone());
        assert!(reconnected
            .observe(&"thread".into(), &messages)
            .unwrap()
            .is_empty());
        assert!(reconnected.admitted(&request));
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
