//! OpenCode user messages correlate requests; assistant steps remain subordinate.

use std::collections::{BTreeMap, HashSet};

use anyhow::{Context, Result};
use serde_json::{json, Value};

use crate::chat::types::{ConversationEvent, Lifecycle};
use crate::durable::FlowTurnSelection;
use crate::exec::SessionDriver;
use crate::session::SessionEventKind;
use crate::store::sqlite::SqliteStore;

#[derive(Debug, Default)]
pub(super) struct History {
    pub(super) owner: Option<(SqliteStore, String, SessionDriver)>,
    pub(super) selection: Option<FlowTurnSelection>,
    requests: HashSet<String>,
    started: HashSet<String>,
    completed: HashSet<String>,
}

impl History {
    pub(super) fn new(
        owner: Option<(SqliteStore, String, SessionDriver)>,
        selection: Option<FlowTurnSelection>,
    ) -> Self {
        Self {
            owner,
            selection,
            ..Self::default()
        }
    }
    pub(super) fn request(&mut self) -> String {
        let id = format!("msg_{}", uuid::Uuid::new_v4().simple());
        self.requests.insert(id.clone());
        id
    }

    pub(super) fn observe(
        &mut self,
        thread: &str,
        messages: &[Value],
    ) -> Result<Vec<ConversationEvent>> {
        let mut events = Vec::new();
        let receipts = native_receipts(thread, messages);
        for (request, receipt) in receipts {
            if self.requests.contains(&request) && !self.started.contains(&request) {
                if let Some((store, session, driver)) = &self.owner {
                    let exec = driver
                        .exec_id
                        .as_ref()
                        .context("OpenCode request has no driving Exec")?;
                    let start = store.record_session_turn_origin(
                        session,
                        thread,
                        &request,
                        driver.provider_generation,
                        exec,
                    )?;
                    if let Some(selection) = &self.selection {
                        store.select_flow_turn(selection, session, driver, start)?;
                        self.selection = None;
                    }
                }
                self.started.insert(request.clone());
                events.push(ConversationEvent::TurnStarted {
                    turn_id: request.clone(),
                });
            }
            if let Some((store, session, _)) = &self.owner {
                record_receipts(store, session, thread, &request, &receipt)?;
            }
            if self.requests.contains(&request)
                && !receipt.completion.is_null()
                && self.completed.insert(request.clone())
            {
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
        self.requests.contains(request) && self.started.contains(request)
    }
}

#[derive(Debug, Default)]
struct Receipt {
    messages: Vec<Value>,
    completion: Value,
}

fn native_receipts(thread: &str, messages: &[Value]) -> BTreeMap<String, Receipt> {
    let mut receipts = BTreeMap::<String, Receipt>::new();
    for message in messages {
        let info = &message["info"];
        if info["sessionID"] != thread || info["role"] != "assistant" {
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
        receipt.completion = json!({"status":status,"error":info["error"],
            "started_at":receipt.messages[0]["info"]["time"]["created"],
            "completed_at":info["time"]["completed"],"message_id":info["id"],"provider":"opencode"});
    }
    receipts
}

fn record_receipts(
    store: &SqliteStore,
    session: &str,
    thread: &str,
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

pub(crate) async fn recover(
    store: &SqliteStore,
    session: &str,
    endpoint: &str,
    thread: &str,
) -> Result<()> {
    let messages = read_messages(&reqwest::Client::new(), endpoint, thread).await?;
    for (request, receipt) in native_receipts(thread, &messages) {
        // Readback cannot assign an initiating Exec or manufacture missing starts.
        record_receipts(store, session, thread, &request, &receipt)?;
    }
    Ok(())
}

pub(super) async fn read_messages(
    client: &reqwest::Client,
    endpoint: &str,
    thread: &str,
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

// Native submission is bounded and serialized with driver transfer. An
// uncertain HTTP result is retained as uncertain; never submit it twice here.
pub(super) async fn post(
    owner: Option<(SqliteStore, String, SessionDriver)>,
    url: String,
    payload: Value,
) -> Result<()> {
    tokio::task::spawn_blocking(move || {
        let write = || {
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
        };
        if let Some((store, session, driver)) = owner {
            store.with_session_driver(&session, &driver, write)
        } else {
            write()
        }
    })
    .await??;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::{native_receipts, record_receipts, History};
    use crate::durable::RunId;
    use crate::id::ExecId;
    use crate::store::sqlite::SqliteStore;
    use serde_json::json;

    #[test]
    fn native_steps_recover_once_without_reassigning_usage_or_inventing_completion() {
        let home = tempfile::tempdir().unwrap();
        let path = home.path().join("store.db");
        let store = SqliteStore::open_ephemeral(&path).unwrap();
        let input = RunId::new();
        let exec = ExecId::new();
        let sql = rusqlite::Connection::open(&path).unwrap();
        sql.execute(
            "INSERT INTO execs(id,trace_id,started_at) VALUES(?1,'fixture',1)",
            [exec.as_str()],
        )
        .unwrap();
        sql.execute("INSERT INTO agent_sessions(id,input_id,title,title_source,created_at,kind,interactive,input_published,cwd) VALUES('session',?1,'Fixture','human',1,'conversation',0,1,'/fixture')", [input.as_str()]).unwrap();
        sql.execute(
            "INSERT INTO agent_session_inputs(input_id,session_id) VALUES(?1,'session')",
            [input.as_str()],
        )
        .unwrap();
        let driver = store
            .claim_session_driver("session", None, &exec, false)
            .unwrap();
        let mut history = History::new(
            Some((store.clone(), "session".into(), driver.clone())),
            None,
        );
        let request = history.request();
        let message = |id: &str, input: u64, finish: &str| {
            json!({
            "info":{"id":id,"sessionID":"thread","parentID":request,"role":"assistant",
                "time":{"created":if id=="assistant-a" {1} else {3},"completed":if id=="assistant-a" {2} else {4}},
                "finish":finish,"tokens":{"input":input,"output":5,"reasoning":0,"cache":{"read":0,"write":0}}},
            "parts":[]})
        };
        for input in [20, 40, 30] {
            let events = history
                .observe("thread", &[message("assistant-a", input, "tool-calls")])
                .unwrap();
            assert!(!events.iter().any(|event| matches!(
                event,
                crate::chat::types::ConversationEvent::TurnCompleted { .. }
            )));
        }
        let session = store.session("session").unwrap().unwrap();
        let mut replacement = session.clone();
        replacement.input_id = RunId::new();
        store
            .replace_session_input(&input, replacement.clone())
            .unwrap();
        let second = ExecId::new();
        sql.execute(
            "INSERT INTO execs(id,trace_id,started_at) VALUES(?1,'fixture',1)",
            [second.as_str()],
        )
        .unwrap();
        store
            .claim_session_driver("session", Some(&driver), &second, true)
            .unwrap();
        let messages = [
            message("assistant-a", 30, "tool-calls"),
            message("assistant-b", 10, "stop"),
        ];
        for _ in 0..2 {
            for (request, receipt) in native_receipts("thread", &messages) {
                record_receipts(&store, "session", "thread", &request, &receipt).unwrap();
            }
        }
        let recovered = store.input_snapshot(input.as_str()).unwrap();
        assert_eq!(recovered.usage.input_tokens, Some(50));
        assert_eq!(recovered.usage.output_tokens, Some(10));
        assert!(
            recovered.usage.gaps > 0,
            "native decrease retains the existing reducer gap"
        );
        assert_eq!(
            recovered.outcome, None,
            "provider completion does not complete its Exec"
        );
        assert_eq!(
            store
                .input_snapshot(replacement.input_id.as_str())
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
            .all(|row| row.exec_id.as_deref() == Some(exec.as_str())));
    }
}
