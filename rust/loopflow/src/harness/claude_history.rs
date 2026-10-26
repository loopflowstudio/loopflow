//! Stream input UUIDs, echoed by Claude, correlate each native result. Each
//! turn keeps one usage row: every assistant response updates its cumulative
//! counters, and the result makes it final with cost and context pressure.
use crate::id::AgentSessionId;
use std::collections::{HashMap, VecDeque};
use std::sync::{Arc, Mutex};

use anyhow::Result;
use serde_json::{json, Value};

use crate::session::SessionEventKind;

#[derive(Debug)]
pub(super) struct History {
    pub owner: super::agent_process::AttachmentOwner,
    pub requests: Arc<Mutex<HashMap<String, crate::session::SessionTurnOrigin>>>,
    pub pending: VecDeque<(AgentSessionId, String)>,
    pub activity: super::activity_tracker::SessionActivityTracker,
    pub usage: super::claude_mapping::ClaudeUsage,
}

impl History {
    pub fn record(&mut self, line: &str) -> Result<()> {
        let Ok(value) = serde_json::from_str::<Value>(line) else {
            return Ok(());
        };
        let (store, session, attachment) = &self.owner;
        self.activity.record(
            store,
            session,
            attachment,
            super::activity_tracker::claude(&value),
        );
        if value["type"] == "assistant" {
            let usage = self.usage.assistant(&value);
            if let (Some(usage), Some((thread, turn))) = (usage, self.pending.front()) {
                store.record_session_event(
                    session,
                    thread,
                    turn,
                    SessionEventKind::Usage,
                    &json!({"provider":"claude","usage":usage,"final":false}),
                )?;
            }
        } else if value["type"] == "user" {
            let (Some(thread), Some(turn)) = (value["session_id"].as_str(), value["uuid"].as_str())
            else {
                return Ok(());
            };
            let thread = AgentSessionId::from(thread);
            let Some(origin) = self
                .requests
                .lock()
                .expect("Claude request lock poisoned")
                .remove(turn)
            else {
                return Ok(());
            };
            store.record_session_turn_origin(&thread, turn, &origin)?;
            self.pending.push_back((thread.to_owned(), turn.to_owned()));
        } else if value["type"] == "result" {
            let usage = self.usage.result(&value);
            let Some((thread, turn)) = self.pending.front() else {
                return Ok(());
            };
            anyhow::ensure!(
                value["session_id"] == thread.as_str(),
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
            if let Some(usage) = usage {
                store.record_session_event(
                    session,
                    thread,
                    turn,
                    SessionEventKind::Usage,
                    &json!({"provider":"claude","usage":usage,"final":true}),
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

    use crate::id::LfProcessId;
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
        let process = LfProcessId::new();
        conn.execute(
            "INSERT INTO processes(id,trace_id,started_at) VALUES(?1,'fixture',1)",
            [&process],
        )
        .unwrap();
        let attachment = store
            .claim_session_attachment("conversation", None, &process, false)
            .unwrap();
        let origin = store
            .session_turn_origin("conversation", &attachment)
            .unwrap();
        let mut history = History {
            owner: (store.clone(), "conversation".into(), attachment),
            requests: std::sync::Arc::new(std::sync::Mutex::new(
                [("request".to_string(), origin)].into(),
            )),
            pending: Default::default(),
            activity: Default::default(),
            usage: Default::default(),
        };
        history
            .record(&json!({"type":"user","uuid":"old","session_id":"thread"}).to_string())
            .unwrap();
        history.record(&json!({"type":"result","uuid":"old-result","session_id":"thread","subtype":"success","result":"old"}).to_string()).unwrap();
        assert!(store
            .session_history("conversation", 0, 0)
            .unwrap()
            .iter()
            .all(|event| matches!(
                event.kind,
                SessionEventKind::Captured | SessionEventKind::Observed
            )));
        let original = store.session("conversation").unwrap().unwrap();
        let mut next = original.clone();
        next.artifact_key = Some(crate::session_record::new_artifact_key());
        let next = store
            .replace_session_input(original.captured, next)
            .unwrap();
        history
            .record(&json!({"type":"user","uuid":"request","session_id":"thread"}).to_string())
            .unwrap();
        let value = json!({"decision":"advance","summary":"native proof"});
        history.record(&json!({"type":"result","uuid":"result","session_id":"thread","subtype":"success","structured_output":value}).to_string()).unwrap();
        let events: Vec<_> = store
            .session_history("conversation", 0, 0)
            .unwrap()
            .into_iter()
            .filter(|event| {
                !matches!(
                    event.kind,
                    SessionEventKind::Captured | SessionEventKind::Observed
                )
            })
            .collect();
        assert_eq!(events.len(), 3);
        assert!(events
            .iter()
            .all(|e| e.provider_turn.as_deref() == Some("request")));
        assert_eq!(events[1].kind, SessionEventKind::Output);
        assert_eq!(events[1].payload["value"], value);
        assert!(store
            .input_final_answer(next.input_key().unwrap())
            .unwrap()
            .is_none());
        let answer = store
            .input_final_answer(
                &crate::session_record::parse_artifact_key("run_00000000000000000000000000000001")
                    .unwrap(),
            )
            .unwrap()
            .unwrap();
        assert_eq!(
            serde_json::from_str::<serde_json::Value>(&answer).unwrap(),
            value
        );
    }

    #[test]
    fn each_turn_keeps_one_usage_row_that_matches_the_live_receipt() {
        use crate::chat::types::ConversationEvent;
        use crate::harness::claude_mapping::{process_line, ReaderState};

        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("history.db");
        let store = SqliteStore::open_ephemeral(&path).unwrap();
        let conn = rusqlite::Connection::open(&path).unwrap();
        store.test_session("conversation", "run_00000000000000000000000000000001");
        let process = LfProcessId::new();
        conn.execute(
            "INSERT INTO processes(id,trace_id,started_at) VALUES(?1,'fixture',1)",
            [&process],
        )
        .unwrap();
        let attachment = store
            .claim_session_attachment("conversation", None, &process, false)
            .unwrap();
        let origin = store
            .session_turn_origin("conversation", &attachment)
            .unwrap();
        let mut history = History {
            owner: (store.clone(), "conversation".into(), attachment),
            requests: std::sync::Arc::new(std::sync::Mutex::new(
                [("request".to_string(), origin)].into(),
            )),
            pending: Default::default(),
            activity: Default::default(),
            usage: Default::default(),
        };
        let usage_rows = || {
            store
                .session_history("conversation", 0, 0)
                .unwrap()
                .into_iter()
                .filter(|event| event.kind == SessionEventKind::Usage)
                .collect::<Vec<_>>()
        };
        let lines = [
            json!({"type":"user","uuid":"request","session_id":"thread"}),
            json!({"type":"assistant","session_id":"thread","message":{"id":"msg_1","model":"claude-sonnet-4","content":[],
                "usage":{"input_tokens":10,"output_tokens":4,"cache_read_input_tokens":20}}}),
            json!({"type":"assistant","session_id":"thread","message":{"id":"msg_2","model":"claude-sonnet-4","content":[],
                "usage":{"input_tokens":3,"output_tokens":2,"cache_creation_input_tokens":5}}}),
            json!({"type":"result","uuid":"result","session_id":"thread","subtype":"success","result":"done",
                "model":"claude-sonnet-4","total_cost_usd":0.25,
                "usage":{"input_tokens":13,"output_tokens":6,"cache_read_input_tokens":20,"cache_creation_input_tokens":5},
                "modelUsage":{"claude-sonnet-4":{"contextWindow":200000}}}),
        ];
        let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel();
        let mut state = ReaderState::default();
        for (index, line) in lines.iter().enumerate() {
            history.record(&line.to_string()).unwrap();
            process_line(&line.to_string(), "turn", &tx, &mut state);
            if index == 2 {
                let rows = usage_rows();
                assert_eq!(rows.len(), 1, "the running turn has its row");
                assert_eq!(rows[0].payload["final"], false);
                assert_eq!(rows[0].payload["usage"]["total_input_tokens"], 38);
            }
        }
        let mut live = None;
        while let Ok(event) = rx.try_recv() {
            if let ConversationEvent::UsageCheckpoint {
                usage,
                final_receipt: true,
                ..
            } = event
            {
                live = Some(usage);
            }
        }
        let live = live.expect("the mapping reports the final receipt");
        let rows = usage_rows();
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].provider_turn.as_deref(), Some("request"));
        assert_eq!(rows[0].payload["final"], true);
        let recorded: crate::chat::types::TurnUsage =
            serde_json::from_value(rows[0].payload["usage"].clone()).unwrap();
        assert_eq!(recorded, live);
        assert_eq!(recorded.cost_usd, Some(0.25));
        assert_eq!(recorded.peak_input_tokens, Some(30));
        assert_eq!(recorded.context_window_tokens, Some(200000));
    }
}
