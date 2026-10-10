//! Stream input UUIDs, echoed by Claude, correlate each native result.
use crate::id::AgentSessionId;
use std::collections::VecDeque;

use anyhow::Result;
use serde_json::{json, Value};

use crate::session::SessionEventKind;

#[derive(Debug)]
pub(super) struct History {
    owner: super::agent_process::AttachmentOwner,
    pending: VecDeque<(AgentSessionId, String)>,
    attention: super::attention::Attention,
}

impl History {
    pub(super) fn new(owner: super::agent_process::AttachmentOwner) -> Result<Self> {
        let pending = owner
            .0
            .pending_session_turns(&owner.1, &owner.2.agent_process_id)?
            .into();
        Ok(Self {
            owner,
            pending,
            attention: Default::default(),
        })
    }

    pub fn record(&mut self, line: &str) -> Result<()> {
        let Ok(value) = serde_json::from_str::<Value>(line) else {
            return Ok(());
        };
        let (store, session, attachment) = &self.owner;
        self.attention
            .record(store, session, attachment, super::attention::claude(&value));
        if value["type"] == "user" {
            let (Some(thread), Some(turn)) = (value["session_id"].as_str(), value["uuid"].as_str())
            else {
                return Ok(());
            };
            let thread = AgentSessionId::from(thread);
            let Some((origin, completed)) = store.session_request(session, &thread, turn)? else {
                return Ok(());
            };
            if origin.agent_process_id != attachment.agent_process_id || completed {
                return Ok(());
            }
            store.record_session_turn_origin(&thread, turn, &origin)?;
            let pending = (thread, turn.to_owned());
            if !self.pending.contains(&pending) {
                self.pending.push_back(pending);
            }
        } else if value["type"] == "result" {
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

    use crate::id::LfProcessId;
    use crate::session::SessionEventKind;
    use crate::store::sqlite::SqliteStore;
    use serde_json::json;

    #[test]
    fn reconstructed_reader_retains_pending_origins_across_takeover_and_new_input() {
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
        store
            .record_session_request(None, "request", &origin)
            .unwrap();
        let replacement = LfProcessId::new();
        conn.execute(
            "INSERT INTO processes(id,trace_id,started_at) VALUES(?1,'fixture',1)",
            [&replacement],
        )
        .unwrap();
        let current = store
            .claim_session_attachment("conversation", Some(&attachment), &replacement, false)
            .unwrap();
        assert_eq!(current.agent_process_id, attachment.agent_process_id);
        assert!(store
            .session_turn_origin("conversation", &attachment)
            .is_err());
        drop(store);
        let store = SqliteStore::open_ephemeral(&path).unwrap();
        let mut history =
            History::new((store.clone(), "conversation".into(), current.clone())).unwrap();
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
        next.artifact_key = crate::session_record::new_artifact_key();
        let next = store
            .replace_session_input(original.captured, next)
            .unwrap();
        history
            .record(&json!({"type":"user","uuid":"request","session_id":"thread"}).to_string())
            .unwrap();
        assert!(store
            .session_request("conversation", &"other-thread".into(), "request")
            .unwrap()
            .is_none());
        // Lose the reader again after admission, before the result. Repeated
        // native echoes must not enqueue a second completion for the same input.
        drop(history);
        drop(store);
        let store = SqliteStore::open_ephemeral(&path).unwrap();
        let mut history =
            History::new((store.clone(), "conversation".into(), current.clone())).unwrap();
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
            .all(|event| event.lf_process_id.as_deref() == Some(process.as_str())));
        assert!(store
            .pending_session_turns("conversation", &current.agent_process_id)
            .unwrap()
            .is_empty());
        let count = store.session_history("conversation", 0, 0).unwrap().len();
        history
            .record(&json!({"type":"user","uuid":"request","session_id":"thread"}).to_string())
            .unwrap();
        history.record(&json!({"type":"result","uuid":"duplicate","session_id":"thread","subtype":"success","result":"must not settle another input"}).to_string()).unwrap();
        assert_eq!(
            store.session_history("conversation", 0, 0).unwrap().len(),
            count
        );
        assert!(events
            .iter()
            .all(|e| e.provider_turn.as_deref() == Some("request")));
        assert_eq!(events[1].kind, SessionEventKind::Output);
        assert_eq!(events[1].payload["value"], value);
        assert!(store
            .input_final_answer(&next.artifact_key)
            .unwrap()
            .is_none());
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
    #[test]
    fn request_from_another_agent_process_cannot_admit_a_native_turn() {
        let dir = tempfile::tempdir().unwrap();
        let store = SqliteStore::open_ephemeral(&dir.path().join("history.db")).unwrap();
        store.test_session("conversation", &crate::session_record::new_artifact_key());
        let process = LfProcessId::new();
        let conn = rusqlite::Connection::open(dir.path().join("history.db")).unwrap();
        conn.execute(
            "INSERT INTO processes(id,trace_id,started_at) VALUES(?1,'fixture',1)",
            [&process],
        )
        .unwrap();
        let attachment = store
            .claim_session_attachment("conversation", None, &process, false)
            .unwrap();
        let mut origin = store
            .session_turn_origin("conversation", &attachment)
            .unwrap();
        origin.agent_process_id = LfProcessId::new();
        store
            .record_session_request(None, "foreign", &origin)
            .unwrap();
        let mut history = History::new((store.clone(), "conversation".into(), attachment)).unwrap();
        history
            .record(&json!({"type":"user","uuid":"foreign","session_id":"thread"}).to_string())
            .unwrap();
        history.record(&json!({"type":"result","uuid":"result","session_id":"thread","subtype":"success","result":"foreign"}).to_string()).unwrap();
        assert!(store
            .session_history("conversation", 0, 0)
            .unwrap()
            .iter()
            .all(|event| matches!(
                event.kind,
                SessionEventKind::Captured | SessionEventKind::Observed
            )));
    }
}
