//! Provider observations survive the client that happened to receive them.
//! These receipts confer no conversational or Flow mutation authority.

use std::collections::{HashMap, HashSet};

use serde_json::{json, Value};

use crate::exec::SessionDriver;
use crate::session::SessionEventKind;
use crate::store::sqlite::SqliteStore;
use crate::store::StoreResult;

/// Connection-local correlation only. Native turn/start can return an already
/// active turn; only a new turn/started notification paired with its reply
/// establishes this client's origin. Resumed history does not establish it.
#[derive(Debug, Default)]
pub(super) struct History {
    sequence: u64,
    final_answers: HashMap<String, String>,
    known: HashMap<String, u64>,
    requests: HashMap<String, u64>,
    started: HashSet<String>,
    replies: HashMap<String, SessionDriver>,
    attributed: HashSet<String>,
}

impl History {
    pub(super) fn request(&mut self, rpc: &Value) {
        if rpc["method"] == "turn/start" && !rpc["id"].is_null() {
            self.sequence += 1;
            self.requests.insert(rpc["id"].to_string(), self.sequence);
        }
    }

    fn observe(&mut self, turn: &str) {
        self.known.entry(turn.to_owned()).or_insert(self.sequence);
    }

    pub(super) fn record(
        &mut self,
        store: &SqliteStore,
        session: &str,
        driver: Option<&SessionDriver>,
        expected_thread: Option<&str>,
        rpc: &Value,
    ) -> StoreResult<()> {
        self.sequence += 1;
        let request = if rpc.get("method").is_none() {
            self.requests.remove(&rpc["id"].to_string())
        } else {
            None
        };
        let method = rpc
            .get("method")
            .and_then(Value::as_str)
            .unwrap_or_default();
        if !matches!(
            method,
            "turn/started" | "turn/completed" | "thread/tokenUsage/updated" | "item/completed"
        ) && rpc.pointer("/result/turn").is_none()
            && rpc.pointer("/result/thread/turns").is_none()
            && rpc.pointer("/result/data").is_none()
        {
            return Ok(());
        }
        let params = &rpc["params"];
        let result = &rpc["result"];
        let stored_thread = if expected_thread.is_none() {
            store.session_thread(session)?
        } else {
            None
        };
        let observed_thread = params["threadId"]
            .as_str()
            .or(result["thread"]["id"].as_str());
        let thread = expected_thread
            .or(stored_thread.as_deref())
            .or(observed_thread);
        let Some(thread) = thread else { return Ok(()) };
        if observed_thread.is_some_and(|observed| observed != thread) {
            return Ok(());
        }
        let turn = super::codex_mapping::extract_turn_id(params);
        if let Some(turn) = turn {
            self.observe(&turn);
            match method {
                "turn/started" => {
                    self.started.insert(turn.clone());
                    store.record_session_event(
                        session,
                        thread,
                        &turn,
                        SessionEventKind::Started,
                        &json!({}),
                    )?;
                }
                "thread/tokenUsage/updated" => {
                    // Keep the provider's cumulative and last-request counters.
                    // Missing baseline is not permission to attribute all lifetime
                    // usage to the client which reconnected halfway through a turn.
                    store.record_session_event(
                        session,
                        thread,
                        &turn,
                        SessionEventKind::Usage,
                        &params["tokenUsage"],
                    )?;
                }
                "item/completed" => {
                    if let Some(text) = final_text(&params["item"]) {
                        self.final_answers.insert(turn, text.to_owned());
                    }
                }
                "turn/completed" => {
                    completion(store, session, thread, &params["turn"])?;
                    if let Some(text) = self.final_answers.remove(&turn) {
                        store.record_session_event(
                            session,
                            thread,
                            &turn,
                            SessionEventKind::Output,
                            &json!({"text": text}),
                        )?;
                    }
                }
                _ => {}
            }
        }
        if let (Some(turn), Some(driver), Some(request)) =
            (result["turn"]["id"].as_str(), driver, request)
        {
            let existing = self.known.get(turn).is_some_and(|seen| *seen < request);
            self.observe(turn);
            if !existing && !self.attributed.contains(turn) {
                self.replies
                    .entry(turn.to_owned())
                    .or_insert_with(|| driver.clone());
            }
        }
        let correlated: Vec<_> = self
            .started
            .iter()
            .filter(|turn| self.replies.contains_key(*turn) && !self.attributed.contains(*turn))
            .cloned()
            .collect();
        for turn in correlated {
            let driver = &self.replies[&turn];
            if let Some(exec) = &driver.exec_id {
                store.record_session_turn_origin(
                    session,
                    thread,
                    &turn,
                    driver.provider_generation,
                    exec,
                )?;
            }
            self.attributed.insert(turn.clone());
            self.replies.remove(&turn);
        }
        if let Some(turns) = result["thread"]["turns"]
            .as_array()
            .or(result["data"].as_array())
        {
            for turn in turns {
                if let Some(id) = turn["id"].as_str() {
                    self.observe(id);
                }
                // Snapshot recovery knows completion, not which client started the
                // turn or which provider generation was active at that earlier time.
                completion(store, session, thread, turn)?;
            }
        }
        Ok(())
    }
}

fn final_text(item: &Value) -> Option<&str> {
    (item["type"] == "agentMessage" && (item["phase"].is_null() || item["phase"] == "final_answer"))
        .then(|| item["text"].as_str())
        .flatten()
}

fn completion(store: &SqliteStore, session: &str, thread: &str, turn: &Value) -> StoreResult<()> {
    let Some(id) = turn["id"].as_str() else {
        return Ok(());
    };
    let Some(status @ ("completed" | "failed" | "interrupted")) = turn["status"].as_str() else {
        return Ok(());
    };
    if let Some(text) = turn["items"]
        .as_array()
        .and_then(|items| items.iter().rev().find_map(final_text))
    {
        store.record_session_event(
            session,
            thread,
            id,
            SessionEventKind::Output,
            &json!({"text": text}),
        )?;
    }
    store.record_session_event(session, thread, id, SessionEventKind::Completed,
        &json!({"status": status, "error": turn["error"], "started_at": turn["startedAt"], "completed_at": turn["completedAt"], "duration_ms": turn["durationMs"]}))?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::History;
    use crate::exec::SessionDriver;
    use crate::id::ExecId;
    use crate::session::SessionEventKind;
    use crate::store::sqlite::SqliteStore;
    use serde_json::json;

    #[test]
    fn busy_turn_input_preserves_known_and_unknown_origins() {
        let home = tempfile::tempdir().unwrap();
        let path = home.path().join("history.db");
        let store = SqliteStore::open_ephemeral(&path).unwrap();
        let first = ExecId::new();
        let second = ExecId::new();
        let conn = rusqlite::Connection::open(&path).unwrap();
        store.test_session("conversation", "run_00000000000000000000000000000001");
        for exec in [&first, &second] {
            conn.execute(
                "INSERT INTO execs(id,trace_id,started_at) VALUES(?1,'fixture',1)",
                [exec.as_str()],
            )
            .unwrap();
        }
        let original = SessionDriver {
            exec_id: Some(first.clone()),
            generation: 1,
            provider_generation: 1,
            provider_exec_id: first.clone(),
        };
        let replacement = SessionDriver {
            exec_id: Some(second.clone()),
            generation: 2,
            ..original.clone()
        };
        for (turn, reply_first) in [("notification-first", false), ("reply-first", true)] {
            let mut history = History::default();
            history.request(&json!({"id":1,"method":"turn/start"}));
            let reply = json!({"id":1,"result":{"turn":{"id":turn}}});
            let started =
                json!({"method":"turn/started","params":{"threadId":"thread","turn":{"id":turn}}});
            let messages = if reply_first {
                [&reply, &started]
            } else {
                [&started, &reply]
            };
            for message in messages {
                history
                    .record(
                        &store,
                        "conversation",
                        Some(&original),
                        Some("thread"),
                        message,
                    )
                    .unwrap();
            }
            let before = store.session_history("conversation", 0, 0).unwrap();
            assert_eq!(
                before.last().unwrap().exec_id.as_deref(),
                Some(first.as_str())
            );
            // A reconnect discovers the existing turn; another input receives
            // that same turn rather than a fresh native start notification.
            let mut current = History::default();
            current.record(&store,"conversation",Some(&replacement),Some("thread"),
                &json!({"result":{"thread":{"id":"thread","turns":[{"id":turn,"status":"inProgress"}]}}})).unwrap();
            current.request(&json!({"id":2,"method":"turn/start"}));
            current
                .record(
                    &store,
                    "conversation",
                    Some(&replacement),
                    Some("thread"),
                    &json!({"id":2,"result":{"turn":{"id":turn}}}),
                )
                .unwrap();
            assert_eq!(store.session_history("conversation", 0, 0).unwrap(), before);
        }
        let mut current = History::default();
        // Even a broadcast before this client's request does not establish
        // that this client started the turn.
        current.record(&store,"conversation",Some(&replacement),Some("thread"),
            &json!({"method":"turn/started","params":{"threadId":"thread","turn":{"id":"unknown"}}})).unwrap();
        current.request(&json!({"id":3,"method":"turn/start"}));
        current
            .record(
                &store,
                "conversation",
                Some(&replacement),
                Some("thread"),
                &json!({"id":3,"result":{"turn":{"id":"unknown"}}}),
            )
            .unwrap();
        let events = store.session_history("conversation", 0, 0).unwrap();
        let unknown = events
            .iter()
            .find(|event| event.provider_turn.as_deref() == Some("unknown"))
            .unwrap();
        assert_eq!(unknown.kind, SessionEventKind::Started);
        assert_eq!(unknown.exec_id, None);
        assert_eq!(unknown.provider_generation, None);
        assert!(
            store
                .record_session_turn_origin("conversation", "thread", "reply-first", 1, &second)
                .is_err(),
            "contradictory actual origin evidence still fails"
        );
    }
}
