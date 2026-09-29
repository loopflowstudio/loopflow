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
    flow_selection: Option<crate::durable::FlowTurnSelection>,
    sequence: u64,
    known: HashMap<String, u64>,
    requests: HashMap<String, u64>,
    started: HashSet<String>,
    replies: HashMap<String, SessionDriver>,
    attributed: HashSet<String>,
}

impl History {
    pub(super) fn for_flow(selection: Option<crate::durable::FlowTurnSelection>) -> Self {
        Self {
            flow_selection: selection,
            ..Self::default()
        }
    }

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
            "turn/started" | "turn/completed" | "thread/tokenUsage/updated"
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
                "turn/completed" => {
                    completion(store, session, thread, &params["turn"])?;
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
                let start = store.record_session_turn_origin(
                    session,
                    thread,
                    &turn,
                    driver.provider_generation,
                    exec,
                )?;
                if let Some(selection) = &self.flow_selection {
                    store.select_flow_turn(selection, session, driver, start)?;
                    self.flow_selection = None;
                }
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

fn completion(store: &SqliteStore, session: &str, thread: &str, turn: &Value) -> StoreResult<()> {
    let Some(id) = turn["id"].as_str() else {
        return Ok(());
    };
    let Some(status @ ("completed" | "failed" | "interrupted")) = turn["status"].as_str() else {
        return Ok(());
    };
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
        conn.execute_batch(
            "BEGIN;
             INSERT INTO agent_sessions(id,input_id,title,title_source,created_at,kind,interactive,input_published,cwd)
             VALUES('conversation','run_fixture','Retained','human',1,'conversation',1,1,'/fixture');
             INSERT INTO agent_session_inputs(input_id,session_id)
             VALUES('run_fixture','conversation');
             COMMIT;",
        )
        .unwrap();
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
