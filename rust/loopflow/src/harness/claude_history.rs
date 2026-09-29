//! Stream input UUIDs, echoed by Claude, correlate each native result.
use std::collections::{HashSet, VecDeque};
use std::sync::{Arc, Mutex};

use anyhow::{Context, Result};
use serde_json::{json, Value};

use crate::durable::FlowTurnSelection;
use crate::exec::SessionDriver;
use crate::session::SessionEventKind;
use crate::store::sqlite::SqliteStore;

#[derive(Debug)]
pub(super) struct History {
    pub owner: Option<(SqliteStore, String, SessionDriver)>,
    pub selection: Option<FlowTurnSelection>,
    pub requests: Arc<Mutex<HashSet<String>>>,
    pub pending: VecDeque<(String, String)>,
}

impl History {
    pub fn record(&mut self, line: &str) -> Result<()> {
        let Ok(value) = serde_json::from_str::<Value>(line) else {
            return Ok(());
        };
        let Some((store, session, driver)) = &self.owner else {
            return Ok(());
        };
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
            let start = store.record_session_turn_origin(
                session,
                thread,
                turn,
                driver.provider_generation,
                exec,
            )?;
            if let Some(selection) = &self.selection {
                store.select_flow_turn(selection, session, driver, start)?;
                self.selection = None;
            }
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
    use crate::durable::RunId;
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
        conn.execute_batch("INSERT INTO agent_sessions(id,input_id,title,title_source,created_at,kind,interactive,input_published,cwd)
            VALUES('conversation','run_00000000000000000000000000000001','proof','human',1,'conversation',0,1,'/fixture');
            INSERT INTO agent_session_inputs(input_id,session_id) VALUES('run_00000000000000000000000000000001','conversation');").unwrap();
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
            selection: None,
            requests: std::sync::Arc::new(std::sync::Mutex::new(["request".to_string()].into())),
            pending: Default::default(),
        };
        history
            .record(&json!({"type":"user","uuid":"old","session_id":"thread"}).to_string())
            .unwrap();
        history.record(&json!({"type":"result","uuid":"old-result","session_id":"thread","subtype":"success","result":"old"}).to_string()).unwrap();
        assert!(store
            .session_history("conversation", 0, 0)
            .unwrap()
            .is_empty());
        history
            .record(&json!({"type":"user","uuid":"request","session_id":"thread"}).to_string())
            .unwrap();
        let value = json!({"decision":"advance","summary":"native proof"});
        history.record(&json!({"type":"result","uuid":"result","session_id":"thread","subtype":"success","structured_output":value}).to_string()).unwrap();
        let events = store.session_history("conversation", 0, 0).unwrap();
        assert_eq!(events.len(), 3);
        assert!(events
            .iter()
            .all(|e| e.provider_turn.as_deref() == Some("request")));
        assert_eq!(events[1].kind, SessionEventKind::Output);
        assert_eq!(events[1].payload["value"], value);
        let answer = store
            .input_final_answer(&RunId::parse("run_00000000000000000000000000000001").unwrap())
            .unwrap()
            .unwrap();
        assert!(answer.exact);
        assert_eq!(
            serde_json::from_str::<serde_json::Value>(&answer.text).unwrap(),
            value
        );
    }
}
