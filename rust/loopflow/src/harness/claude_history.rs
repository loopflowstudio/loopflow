//! Stream input UUIDs, echoed by Claude, correlate each native result.
use crate::id::{AgentSessionId, LfProcessId};

use anyhow::Result;
use serde_json::{json, Value};
use sha2::{Digest, Sha256};

use crate::session::{SessionEvent, SessionEventKind};

#[derive(Debug)]
pub(super) struct History {
    store: crate::store::sqlite::SqliteStore,
    session: String,
    agent_process_id: LfProcessId,
    attention: super::attention::Attention,
}

impl History {
    pub(super) fn new(owner: super::agent_process::AttachmentOwner) -> Self {
        let (store, session, attachment) = owner;
        let agent_process_id = attachment.agent_process_id;
        Self {
            store,
            session,
            agent_process_id,
            attention: Default::default(),
        }
    }

    /// Results return their committed receipt, not a second client-side judgment.
    /// Replayed results return the same sequence and original turn (or absence of
    /// one), so a client can resume from history without consuming a later input.
    pub fn record(&mut self, line: &str) -> Result<Option<SessionEvent>> {
        let Ok(value) = serde_json::from_str::<Value>(line) else {
            return Ok(None);
        };
        let store = &self.store;
        let session = &self.session;
        // This reader observes one immutable provider stream. Following its
        // current display attachment must never refresh a caller's write token.
        let signals = super::attention::claude(&value);
        match store.session_attachment(session) {
            Ok(Some(attachment))
                if attachment.agent_process_id == self.agent_process_id
                    && attachment.lf_process_id.is_some() =>
            {
                self.attention.record(store, session, &attachment, signals);
            }
            current => {
                if let Err(error) = current {
                    tracing::warn!(%error, session, "failed to read Session activity attachment");
                }
                // Retain open tools while detached, but publish nothing for a
                // replacement provider (or manufacture an attached owner).
                self.attention.apply(signals);
            }
        }
        if value["type"] == "user" {
            let (Some(thread), Some(turn)) = (value["session_id"].as_str(), value["uuid"].as_str())
            else {
                return Ok(None);
            };
            let thread = AgentSessionId::from(thread);
            let Some((origin, completed)) = store.session_request(session, &thread, turn)? else {
                return Ok(None);
            };
            if origin.agent_process_id != self.agent_process_id || completed {
                return Ok(None);
            }
            store.record_session_turn_origin(&thread, turn, &origin)?;
        } else if value["type"] == "result" {
            let thread = value["session_id"].as_str().map(AgentSessionId::from);
            let mut events = Vec::new();
            let status = if value["subtype"] == "success" && value["is_error"] != true {
                "completed"
            } else {
                "failed"
            };
            if let Some(output) = value.get("structured_output").filter(|v| !v.is_null()) {
                events.push((SessionEventKind::Output, json!({"value": output})));
            } else if let Some(text) = value["result"].as_str() {
                events.push((SessionEventKind::Output, json!({"text": text})));
            }
            if !value["usage"].is_null() {
                events.push((
                    SessionEventKind::Usage,
                    json!({"provider":"claude","usage":value["usage"]}),
                ));
            }
            let result_digest = format!("{:x}", Sha256::digest(value.to_string().as_bytes()));
            let seq = store.record_ordered_session_result(
                session,
                &self.agent_process_id,
                thread.as_ref(),
                &events,
                &json!({"status":status,"result_id":value["uuid"],"result_digest":result_digest,"errors":value["errors"],"subtype":value["subtype"]}),
            )?;
            let receipt = store
                .session_history(session, seq - 1, 1)?
                .pop()
                .filter(|event| event.seq == seq)
                .ok_or_else(|| anyhow::anyhow!("committed Claude result {seq} is missing"))?;
            return Ok(Some(receipt));
        }
        Ok(None)
    }
}

#[cfg(test)]
mod tests {
    use super::History;

    use crate::id::LfProcessId;
    use crate::session::SessionEventKind;
    use crate::store::sqlite::SqliteStore;
    use serde_json::json;

    fn fixture_process(path: &std::path::Path) -> LfProcessId {
        let process = LfProcessId::new();
        rusqlite::Connection::open(path)
            .unwrap()
            .execute(
                "INSERT INTO processes(id,trace_id,started_at) VALUES(?1,'fixture',1)",
                [&process],
            )
            .unwrap();
        process
    }

    fn session_owner(path: &std::path::Path) -> super::super::agent_process::AttachmentOwner {
        let store = SqliteStore::open_ephemeral(path).unwrap();
        store.test_session("conversation", "run_00000000000000000000000000000001");
        let attachment = store
            .claim_session_attachment("conversation", None, &fixture_process(path), false)
            .unwrap();
        (store, "conversation".into(), attachment)
    }

    fn pending_pair(path: &std::path::Path) -> super::super::agent_process::AttachmentOwner {
        let owner = session_owner(path);
        let (store, session, attachment) = &owner;
        let origin = store.session_turn_origin(session, attachment).unwrap();
        let mut history = History::new(owner.clone());
        for request in ["first", "second"] {
            store
                .record_session_request(None, request, &origin)
                .unwrap();
            history
                .record(&json!({"type":"user","uuid":request,"session_id":"thread"}).to_string())
                .unwrap();
        }
        owner
    }

    #[test]
    fn uncorrelated_result_cannot_complete_later_input_after_reader_restart() {
        let home = tempfile::tempdir().unwrap();
        let owner = session_owner(&home.path().join("history.db"));
        let mut result = json!({"type":"result","uuid":"early-result","session_id":"thread",
            "subtype":"success","result":"old answer","usage":{"input_tokens":10}});
        let receipt = History::new(owner.clone())
            .record(&result.to_string())
            .unwrap()
            .unwrap();
        let events = owner.0.session_history("conversation", 0, 0).unwrap();
        let observation = events
            .iter()
            .find(|event| event.payload["uncorrelated_result"]["result_id"] == "early-result")
            .unwrap();
        assert_eq!(&receipt, observation);
        assert_eq!(observation.kind, SessionEventKind::Observed);
        assert_eq!(observation.provider_turn, None);
        assert_eq!(observation.payload["events"][0][1]["text"], "old answer");
        assert_eq!(
            observation.payload["events"][1][1]["usage"]["input_tokens"],
            10
        );

        let origin = owner
            .0
            .session_turn_origin("conversation", &owner.2)
            .unwrap();
        owner
            .0
            .record_session_request(None, "later", &origin)
            .unwrap();
        let mut reader = History::new(owner.clone());
        reader
            .record(&json!({"type":"user","uuid":"later","session_id":"thread"}).to_string())
            .unwrap();
        assert_eq!(reader.record(&result.to_string()).unwrap(), Some(receipt));
        result["usage"]["input_tokens"] = json!(99);
        assert!(reader.record(&result.to_string()).is_err());
        result["usage"]["input_tokens"] = json!(10);
        result["session_id"] = json!("different-thread");
        assert!(reader.record(&result.to_string()).is_err());
        let events = owner.0.session_history("conversation", 0, 0).unwrap();
        assert_eq!(
            events
                .iter()
                .filter(|event| event.payload.get("uncorrelated_result").is_some())
                .count(),
            1
        );
        assert!(events.iter().all(|event| !matches!(
            event.kind,
            SessionEventKind::Output | SessionEventKind::Usage | SessionEventKind::Completed
        )));

        reader
            .record(
                &json!({"type":"result","uuid":"later-result","session_id":"thread",
            "subtype":"success","result":"new answer"})
                .to_string(),
            )
            .unwrap();
        let events = owner.0.session_history("conversation", 0, 0).unwrap();
        let completion = events
            .iter()
            .find(|event| event.kind == SessionEventKind::Completed)
            .unwrap();
        assert_eq!(completion.provider_turn.as_deref(), Some("later"));
        assert_eq!(completion.payload["result_id"], "later-result");
    }

    #[test]
    fn unidentified_result_refuses_even_without_admissions() {
        let home = tempfile::tempdir().unwrap();
        let owner = session_owner(&home.path().join("history.db"));
        assert!(History::new(owner.clone())
            .record(
                &json!({"type":"result",
            "session_id":"thread","subtype":"success","result":"unkeyed"})
                .to_string()
            )
            .is_err());
        assert!(owner
            .0
            .session_history("conversation", 0, 0)
            .unwrap()
            .iter()
            .all(|event| event.kind == SessionEventKind::Captured));
    }

    #[test]
    fn reopened_readers_cannot_assign_a_repeated_result_to_the_next_turn() {
        let home = tempfile::tempdir().unwrap();
        let owner = pending_pair(&home.path().join("history.db"));
        // Both readers reconstruct before the first result commits.
        let mut first = History::new(owner.clone());
        let mut second = History::new(owner.clone());
        let result = json!({"type":"result","uuid":"result-one","session_id":"thread","subtype":"success","result":"first answer"}).to_string();
        let first_receipt = first.record(&result).unwrap().unwrap();
        assert_eq!(first_receipt.kind, SessionEventKind::Completed);
        assert_eq!(first_receipt.provider_turn.as_deref(), Some("first"));
        assert_eq!(
            second.record(&result).unwrap().as_ref(),
            Some(&first_receipt)
        );
        assert_eq!(
            History::new(owner.clone())
                .record(&result)
                .unwrap()
                .as_ref(),
            Some(&first_receipt)
        );
        let completed = || {
            owner
                .0
                .session_history("conversation", 0, 0)
                .unwrap()
                .into_iter()
                .filter(|event| event.kind == SessionEventKind::Completed)
                .collect::<Vec<_>>()
        };
        // Same result identity with changed usage is conflicting evidence,
        // not another usage receipt or permission to consume the next turn.
        assert!(second.record(&json!({"type":"result","uuid":"result-one","session_id":"thread","subtype":"success","result":"first answer","usage":{"input_tokens":99}}).to_string()).is_err());
        assert!(second.record(&json!({"type":"result","session_id":"thread","subtype":"success","result":"no identity"}).to_string()).is_err());
        let events = completed();
        assert_eq!(events.len(), 1);
        assert_eq!(events[0].provider_turn.as_deref(), Some("first"));
        let second_receipt = second.record(&json!({"type":"result","uuid":"result-two","session_id":"thread","subtype":"success","result":"second answer"}).to_string()).unwrap().unwrap();
        let events = completed();
        assert_eq!(events, vec![first_receipt.clone(), second_receipt.clone()]);
        assert_eq!(second_receipt.provider_turn.as_deref(), Some("second"));
        assert!(second_receipt.seq > first_receipt.seq);
        // A delayed replay still names the original completion, not the latest
        // client turn. Reading after the first receipt yields only the second.
        assert_eq!(first.record(&result).unwrap(), Some(first_receipt.clone()));
        let unread_completions: Vec<_> = owner
            .0
            .session_history("conversation", first_receipt.seq, 0)
            .unwrap()
            .into_iter()
            .filter(|event| event.kind == SessionEventKind::Completed)
            .collect();
        assert_eq!(unread_completions, vec![second_receipt]);
    }

    #[test]
    fn concurrent_readers_return_the_same_committed_turn_after_takeover() {
        let home = tempfile::tempdir().unwrap();
        let path = home.path().join("history.db");
        let owner = pending_pair(&path);
        let replacement = fixture_process(&path);
        let attachment = owner
            .0
            .claim_session_attachment("conversation", Some(&owner.2), &replacement, false)
            .unwrap();
        let barrier = std::sync::Arc::new(std::sync::Barrier::new(2));
        let readers: Vec<_> = [owner.2.clone(), attachment]
            .into_iter()
            .map(|attachment| {
                // Separate connections exercise SQLite serialization, not a shared
                // Rust mutex. Old and current display owners observe the same stream.
                let store = SqliteStore::open_ephemeral(&path).unwrap();
                let barrier = barrier.clone();
                std::thread::spawn(move || {
                    let mut history = History::new((store, "conversation".into(), attachment));
                    barrier.wait();
                    history
                        .record(
                            &json!({"type":"result","uuid":"shared-result",
                    "session_id":"thread","subtype":"success","result":"answer"})
                            .to_string(),
                        )
                        .unwrap()
                        .unwrap()
                })
            })
            .collect();
        let receipts: Vec<_> = readers
            .into_iter()
            .map(|reader| reader.join().unwrap())
            .collect();
        assert_eq!(receipts[0], receipts[1]);
        assert_eq!(receipts[0].kind, SessionEventKind::Completed);
        assert_eq!(receipts[0].provider_turn.as_deref(), Some("first"));
        assert_eq!(
            receipts[0].lf_process_id.as_deref(),
            owner.2.lf_process_id.as_ref().map(LfProcessId::as_str)
        );
        let completions: Vec<_> = owner
            .0
            .session_history("conversation", 0, 0)
            .unwrap()
            .into_iter()
            .filter(|event| event.kind == SessionEventKind::Completed)
            .collect();
        assert_eq!(completions, vec![receipts[0].clone()]);
    }

    #[test]
    fn failed_result_recording_leaves_no_partial_output_or_completion() {
        let home = tempfile::tempdir().unwrap();
        let path = home.path().join("history.db");
        let owner = pending_pair(&path);
        let conn = rusqlite::Connection::open(&path).unwrap();
        conn.execute_batch(
            "CREATE TRIGGER fail_completion BEFORE INSERT ON session_events
            WHEN NEW.kind='completed' BEGIN SELECT RAISE(ABORT,'lost recording'); END;",
        )
        .unwrap();
        let mut history = History::new(owner.clone());
        let result = json!({"type":"result","uuid":"result-one","session_id":"thread","subtype":"success","result":"answer","usage":{"input_tokens":10}}).to_string();
        assert!(history.record(&result).is_err());
        assert!(owner
            .0
            .session_history("conversation", 0, 0)
            .unwrap()
            .iter()
            .all(|event| !matches!(
                event.kind,
                SessionEventKind::Output | SessionEventKind::Usage | SessionEventKind::Completed
            )));
        conn.execute_batch("DROP TRIGGER fail_completion").unwrap();
        let receipt = History::new(owner.clone())
            .record(&result)
            .unwrap()
            .unwrap();
        let events = owner.0.session_history("conversation", 0, 0).unwrap();
        let completed = events
            .iter()
            .filter(|event| event.kind == SessionEventKind::Completed)
            .collect::<Vec<_>>();
        assert_eq!(completed, vec![&receipt]);
        assert_eq!(completed[0].provider_turn.as_deref(), Some("first"));
    }

    #[test]
    fn reconstructed_reader_retains_pending_origins_across_takeover_and_new_input() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("history.db");
        let (store, _, attachment) = session_owner(&path);
        let process = attachment.lf_process_id.clone().unwrap();
        let origin = store
            .session_turn_origin("conversation", &attachment)
            .unwrap();
        store
            .record_session_request(None, "request", &origin)
            .unwrap();
        let replacement = fixture_process(&path);
        let current = store
            .claim_session_attachment("conversation", Some(&attachment), &replacement, false)
            .unwrap();
        assert_eq!(current.agent_process_id, attachment.agent_process_id);
        assert!(store
            .session_turn_origin("conversation", &attachment)
            .is_err());
        drop(store);
        let store = SqliteStore::open_ephemeral(&path).unwrap();
        let mut history = History::new((store.clone(), "conversation".into(), current.clone()));
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
        let mut history = History::new((store.clone(), "conversation".into(), current.clone()));
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
        let count = store.session_history("conversation", 0, 0).unwrap().len();
        history
            .record(&json!({"type":"user","uuid":"request","session_id":"thread"}).to_string())
            .unwrap();
        history.record(&json!({"type":"result","uuid":"duplicate","session_id":"thread","subtype":"success","result":"must not settle another input"}).to_string()).unwrap();
        let retained = store.session_history("conversation", 0, 0).unwrap();
        assert_eq!(retained.len(), count + 1);
        let uncorrelated = retained.last().unwrap();
        assert_eq!(uncorrelated.kind, SessionEventKind::Observed);
        assert_eq!(uncorrelated.provider_turn, None);
        assert_eq!(
            uncorrelated.payload["uncorrelated_result"]["result_id"],
            "duplicate"
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
    fn surviving_reader_reports_current_attention_without_reviving_caller_authority() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("history.db");
        let (store, _, first) = session_owner(&path);
        let sql = rusqlite::Connection::open(&path).unwrap();
        sql.execute("UPDATE agent_sessions SET interactive=1", [])
            .unwrap();
        let a = first.lf_process_id.clone().unwrap();
        let b = fixture_process(&path);
        let mut reader = History::new((store.clone(), "conversation".into(), first.clone()));
        let waiting = || {
            store
                .session_summary(
                    "conversation",
                    time::OffsetDateTime::now_utc().unix_timestamp(),
                )
                .unwrap()
                .unwrap()
                .waiting
        };
        let yielded = json!({"type":"result","uuid":"yielded","subtype":"success"}).to_string();
        reader.record(&yielded).unwrap();
        assert!(waiting());
        let second = store
            .claim_session_attachment("conversation", Some(&first), &b, false)
            .unwrap();
        assert!(!waiting());
        // Identical evidence must reach the new owner even inside the attention
        // save interval, without rebuilding this stream's reader.
        reader.record(&yielded).unwrap();
        assert!(waiting());
        let third = store
            .claim_session_attachment("conversation", Some(&second), &a, false)
            .unwrap();
        assert!(!waiting());
        reader.record(&yielded).unwrap();
        assert!(waiting());
        for stale in [&first, &second] {
            assert!(store
                .with_session_attachment::<()>("conversation", stale, || {
                    panic!("observation refreshed stale dispatch authority")
                })
                .is_err());
            assert!(store
                .finish_session_attachment("conversation", stale, "interrupted", || {
                    panic!("observation refreshed stale stop authority")
                })
                .is_err());
        }
        let detached = store
            .release_session_attachment("conversation", &third)
            .unwrap();
        reader.record(&yielded).unwrap();
        assert!(!waiting());
        assert_eq!(
            store.session_attachment("conversation").unwrap(),
            Some(detached.clone())
        );
        // Continue observing tools while no client is attached. Reattaching
        // must not lose a running tool or classify its provider as waiting.
        reader
            .record(
                &json!({"type":"assistant","message":{"content":[
                    {"type":"tool_use","id":"tool"}
                ]}})
                .to_string(),
            )
            .unwrap();
        let fourth = store
            .claim_session_attachment("conversation", Some(&detached), &b, false)
            .unwrap();
        reader
            .record(&json!({"type":"content_block_delta"}).to_string())
            .unwrap();
        assert!(
            !store
                .session_summary("conversation", i64::MAX)
                .unwrap()
                .unwrap()
                .waiting
        );
        reader
            .record(
                &json!({"type":"user","message":{"content":[
                    {"type":"tool_result","tool_use_id":"tool"}
                ]}})
                .to_string(),
            )
            .unwrap();
        reader.record(&yielded).unwrap();
        assert!(waiting());
        let replacement = store
            .claim_session_attachment("conversation", Some(&fourth), &a, true)
            .unwrap();
        assert_ne!(replacement.agent_process_id, first.agent_process_id);
        let mut replacement_reader =
            History::new((store.clone(), "conversation".into(), replacement));
        replacement_reader
            .record(
                &json!({"type":"assistant","message":{"content":[
                    {"type":"tool_use","id":"replacement-tool"}
                ]}})
                .to_string(),
            )
            .unwrap();
        reader.record(&yielded).unwrap();
        assert!(
            !store
                .session_summary("conversation", i64::MAX)
                .unwrap()
                .unwrap()
                .waiting,
            "an old provider's output replaced the current provider's activity"
        );
    }

    #[test]
    fn request_from_another_agent_process_cannot_admit_a_native_turn() {
        let dir = tempfile::tempdir().unwrap();
        let (store, _, attachment) = session_owner(&dir.path().join("history.db"));
        let mut origin = store
            .session_turn_origin("conversation", &attachment)
            .unwrap();
        origin.agent_process_id = LfProcessId::new();
        store
            .record_session_request(None, "foreign", &origin)
            .unwrap();
        let mut history = History::new((store.clone(), "conversation".into(), attachment));
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
