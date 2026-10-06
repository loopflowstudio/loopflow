//! Stream input UUIDs, echoed by Claude, correlate each native result.
use std::collections::{HashSet, VecDeque};
use std::sync::{Arc, Mutex};

use anyhow::{Context, Result};
use serde_json::{json, Value};

use crate::exec::SessionDriver;
use crate::session::SessionEventKind;
use crate::store::sqlite::SqliteStore;

#[derive(Debug)]
pub(super) struct History {
    pub owner: Option<(SqliteStore, String, SessionDriver)>,
    pub requests: Arc<Mutex<HashSet<String>>>,
    pub pending: VecDeque<(String, String)>,
    pub attention: super::attention::Attention,
}

impl History {
    pub fn record(&mut self, line: &str) -> Result<()> {
        let Ok(value) = serde_json::from_str::<Value>(line) else {
            return Ok(());
        };
        let Some((store, session, driver)) = &self.owner else {
            return Ok(());
        };
        self.attention
            .record(store, session, driver, super::attention::claude(&value));
        if value["type"] == "user" {
            let (Some(thread), Some(turn)) = (value["session_id"].as_str(), value["uuid"].as_str())
            else {
                return Ok(());
            };
            if !self
                .requests
                .lock()
                .expect("Claude request lock poisoned")
                .remove(turn)
            {
                return Ok(());
            }
            let exec = driver
                .exec_id
                .as_ref()
                .context("Claude request has no driving Exec")?;
            store.record_session_turn_origin(
                session,
                thread,
                turn,
                driver.provider_generation,
                exec,
            )?;
            self.pending.push_back((thread.to_owned(), turn.to_owned()));
        } else if value["type"] == "result" {
            let Some((thread, turn)) = self.pending.front() else {
                return Ok(());
            };
            anyhow::ensure!(
                value["session_id"] == *thread,
                "Claude result changed conversation"
            );
            let status = if value["subtype"] == "success" && value["is_error"] != true {
                "completed"
            } else {
                "failed"
            };
            if let Some(output) = value.get("structured_output").filter(|v| !v.is_null()) {
                store.record_session_event(
                    session,
                    thread,
                    turn,
                    SessionEventKind::Output,
                    &json!({"value": output}),
                )?;
            } else if let Some(text) = value["result"].as_str() {
                store.record_session_event(
                    session,
                    thread,
                    turn,
                    SessionEventKind::Output,
                    &json!({"text": text}),
                )?;
            }
            if !value["usage"].is_null() {
                store.record_session_event(
                    session,
                    thread,
                    turn,
                    SessionEventKind::Usage,
                    &json!({"provider":"claude","usage":value["usage"]}),
                )?;
            }
            store.record_session_event(
                session,
                thread,
                turn,
                SessionEventKind::Completed,
                &json!({"status":status,"result_id":value["uuid"],"errors":value["errors"],"subtype":value["subtype"]}),
            )?;
            self.pending.pop_front();
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::History;

    use crate::id::ExecId;
    use crate::session::SessionEventKind;
    use crate::store::sqlite::SqliteStore;
    use serde_json::json;

    #[test]
    fn native_echo_and_result_retain_exact_output_without_borrowing_another_request() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("history.db");
        let store = SqliteStore::open_ephemeral(&path).unwrap();
        let conn = rusqlite::Connection::open(&path).unwrap();
        store.test_session("conversation", "run_00000000000000000000000000000001");
        let exec = ExecId::new();
        conn.execute(
            "INSERT INTO execs(id,trace_id,started_at) VALUES(?1,'fixture',1)",
            [&exec],
        )
        .unwrap();
        let driver = store
            .claim_session_driver("conversation", None, &exec, false)
            .unwrap();
        let mut history = History {
            owner: Some((store.clone(), "conversation".into(), driver)),
            requests: std::sync::Arc::new(std::sync::Mutex::new(["request".to_string()].into())),
            pending: Default::default(),
            attention: Default::default(),
        };
        history
            .record(&json!({"type":"user","uuid":"old","session_id":"thread"}).to_string())
            .unwrap();
        history.record(&json!({"type":"result","uuid":"old-result","session_id":"thread","subtype":"success","result":"old"}).to_string()).unwrap();
        assert!(store
            .session_history("conversation", 0, 0)
            .unwrap()
            .iter()
            .all(|event| event.kind == SessionEventKind::Captured));
        history
            .record(&json!({"type":"user","uuid":"request","session_id":"thread"}).to_string())
            .unwrap();
        let value = json!({"decision":"advance","summary":"native proof"});
        history.record(&json!({"type":"result","uuid":"result","session_id":"thread","subtype":"success","structured_output":value}).to_string()).unwrap();
        let events: Vec<_> = store
            .session_history("conversation", 0, 0)
            .unwrap()
            .into_iter()
            .filter(|event| event.kind != SessionEventKind::Captured)
            .collect();
        assert_eq!(events.len(), 3);
        assert!(events
            .iter()
            .all(|e| e.provider_turn.as_deref() == Some("request")));
        assert_eq!(events[1].kind, SessionEventKind::Output);
        assert_eq!(events[1].payload["value"], value);
        let answer = store
            .input_final_answer(
                &crate::session_record::parse_artifact_key("run_00000000000000000000000000000001")
                    .unwrap(),
            )
            .unwrap()
            .unwrap();
        assert!(answer.exact);
        assert_eq!(
            serde_json::from_str::<serde_json::Value>(&answer.text).unwrap(),
            value
        );
    }
}
