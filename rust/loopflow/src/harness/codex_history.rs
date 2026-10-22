//! Provider observations survive the client that happened to receive them.
//! These receipts confer no conversational or Graph mutation authority.

use crate::id::AgentSessionId;
use std::collections::HashMap;

use serde_json::{json, Value};

use crate::process::SessionAttachment;
use crate::session::{SessionEventKind, SessionTurnOrigin};
use crate::store::sqlite::SqliteStore;
use crate::store::StoreResult;

/// Connection-local correlation only. Native turn/start can return an already
/// active turn; only a new turn/started notification paired with its reply
/// establishes this client's origin. Resumed history does not establish it.
#[derive(Debug, Default)]
pub(super) struct History {
    sequence: u64,
    active_turn: Option<String>,
    turns: HashMap<String, Turn>,
    requests: HashMap<String, (u64, Option<SessionTurnOrigin>)>,
    activity: super::activity_tracker::SessionActivityTracker,
}

/// All connection-local evidence for one native turn. Seeing a turn before
/// submitting a request prevents that request from claiming its origin.
#[derive(Debug, Default)]
struct Turn {
    first_seen: u64,
    started: bool,
    origin: Option<SessionTurnOrigin>,
    attributed: bool,
    final_answer: Option<String>,
}

impl Turn {
    fn correlate(
        &mut self,
        store: &SqliteStore,
        thread: &AgentSessionId,
        turn: &str,
    ) -> StoreResult<()> {
        if self.started && !self.attributed {
            if let Some(origin) = &self.origin {
                store.record_session_turn_origin(thread, turn, origin)?;
                self.attributed = true;
                self.origin = None;
            }
        }
        Ok(())
    }
}

impl History {
    pub(super) fn request(
        &mut self,
        rpc: &Value,
        owner: Option<(&SqliteStore, &str, &SessionAttachment)>,
    ) -> StoreResult<()> {
        self.activity
            .apply(super::activity_tracker::codex(rpc, true));
        if rpc["method"] == "turn/start" && !rpc["id"].is_null() {
            self.active_turn = None;
            self.activity.preview = None;
            if let Some((store, session, attachment)) = owner {
                self.activity.record(
                    store,
                    session,
                    attachment,
                    vec![super::activity_tracker::Signal::Working],
                );
            }
            let origin = owner
                .map(|(store, session, attachment)| store.session_turn_origin(session, attachment))
                .transpose()?;
            self.sequence += 1;
            self.requests
                .insert(rpc["id"].to_string(), (self.sequence, origin));
        }
        Ok(())
    }

    fn observe(&mut self, turn: &str) -> &mut Turn {
        self.turns.entry(turn.to_owned()).or_insert_with(|| Turn {
            first_seen: self.sequence,
            ..Turn::default()
        })
    }

    pub(super) fn record(
        &mut self,
        store: &SqliteStore,
        session: &str,
        attachment: Option<&SessionAttachment>,
        expected_thread: Option<&AgentSessionId>,
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
        let params = &rpc["params"];
        let result = &rpc["result"];
        let stored_thread = if expected_thread.is_none() {
            store.session_thread(session)?
        } else {
            None
        };
        let observed_thread = params["threadId"]
            .as_str()
            .or(result["thread"]["id"].as_str())
            .map(AgentSessionId::from);
        let thread = expected_thread
            .or(stored_thread.as_ref())
            .or(observed_thread.as_ref());
        let Some(thread) = thread else { return Ok(()) };
        if observed_thread
            .as_ref()
            .is_some_and(|observed| observed != thread)
        {
            return Ok(());
        }
        let turn = super::codex_mapping::extract_turn_id(params);
        if let Some(attachment) = attachment {
            if method == "turn/started" {
                if let Some(turn) = turn
                    .as_ref()
                    .filter(|turn| !self.turns.get(*turn).is_some_and(|seen| seen.started))
                {
                    self.active_turn = Some(turn.clone());
                    self.activity.preview = None;
                }
            }
            // Late turn events still enter history, but cannot replace current activity.
            if turn.is_none() || turn == self.active_turn {
                let mut signals = super::activity_tracker::codex(rpc, false);
                if let Some(turn) = turn.as_ref().filter(|_| {
                    method == "item/completed" && params["item"]["type"] == "agentMessage"
                }) {
                    if let Some(text) = params["item"]["text"]
                        .as_str()
                        .filter(|text| !text.trim().is_empty())
                    {
                        self.activity.preview = Some(crate::session::SessionPreview {
                            provider_thread: thread.clone(),
                            provider_turn: turn.clone(),
                            observed_at: time::OffsetDateTime::now_utc().unix_timestamp(),
                            source: "codex".into(),
                            text: text.chars().take(2048).collect(),
                        });
                    }
                    signals = vec![super::activity_tracker::Signal::Working];
                }
                self.activity.record(store, session, attachment, signals);
            }
        }
        if let Some(turn) = turn {
            let observed = self.observe(&turn);
            match method {
                "turn/started" => {
                    observed.started = true;
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
                        observed.final_answer = Some(text.to_owned());
                    }
                }
                "turn/completed" => {
                    completion(store, session, thread, &params["turn"])?;
                    if let Some(text) = observed.final_answer.take() {
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
            observed.correlate(store, thread, &turn)?;
        }
        if let (Some(turn), Some((request, Some(origin)))) =
            (result["turn"]["id"].as_str(), request)
        {
            let observed = self.observe(turn);
            if observed.first_seen >= request && !observed.attributed {
                observed.origin.get_or_insert(origin);
            }
            observed.correlate(store, thread, turn)?;
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
                // turn or which AgentProcess was active at that earlier time.
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

fn completion(
    store: &SqliteStore,
    session: &str,
    thread: &AgentSessionId,
    turn: &Value,
) -> StoreResult<()> {
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
    use crate::id::LfProcessId;
    use crate::process::SessionAttachment;
    use crate::session::SessionEventKind;
    use crate::store::sqlite::SqliteStore;
    use serde_json::json;

    #[test]
    fn preview_belongs_to_current_turn_thread_and_attachment() {
        let home = tempfile::tempdir().unwrap();
        let path = home.path().join("history.db");
        let store = SqliteStore::open_ephemeral(&path).unwrap();
        let sql = rusqlite::Connection::open(&path).unwrap();
        let session =
            store.test_session("conversation", &crate::session_record::new_artifact_key());
        let process = LfProcessId::new();
        sql.execute(
            "INSERT INTO processes(id,trace_id,started_at) VALUES(?1,'fixture',1)",
            [&process],
        )
        .unwrap();
        let attachment = store
            .claim_session_attachment(&session.id, None, &process, false)
            .unwrap();
        let mut history = History::default();
        let thread = "thread".into();
        store
            .record_session_agent_session(&session.id, &attachment, &thread)
            .unwrap();
        store
            .record_agent_process_identity(
                &session.id,
                &attachment,
                std::process::id(),
                crate::journal::process_started_at(std::process::id())
                    .unwrap()
                    .unwrap(),
            )
            .unwrap();
        let preview = || -> Option<crate::session::SessionPreview> {
            let value: Option<String> = sql
                .query_row(
                    "SELECT preview FROM session_activity WHERE session_id='conversation'",
                    [],
                    |row| row.get(0),
                )
                .unwrap();
            value.map(|value| serde_json::from_str(&value).unwrap())
        };
        for turn in ["first", "second"] {
            history
                .request(
                    &json!({"id":turn,"method":"turn/start"}),
                    Some((&store, &session.id, &attachment)),
                )
                .unwrap();
            assert_eq!(preview(), None);
            // Old output cannot return while the next turn is still starting.
            history.record(&store, &session.id, Some(&attachment), Some(&thread),
                &json!({"method":"item/completed","params":{"threadId":"thread","turnId":"first","item":{"type":"agentMessage","text":"late"}}})).unwrap();
            assert_eq!(preview(), None);
            history.record(&store, &session.id, Some(&attachment), Some(&thread),
                &json!({"method":"turn/started","params":{"threadId":"thread","turn":{"id":turn}}})).unwrap();
            assert_eq!(preview(), None);
            history.record(&store, &session.id, Some(&attachment), Some(&thread),
                &json!({"method":"item/completed","params":{"threadId":"thread","turnId":turn,"item":{"type":"agentMessage","phase":"commentary","text":turn}}})).unwrap();
            let reading = preview().unwrap();
            assert_eq!(reading.provider_turn, turn);
            assert_eq!(reading.text, turn);
            assert_eq!(
                store
                    .session_summary(&session.id, 1000)
                    .unwrap()
                    .unwrap()
                    .preview,
                Some(reading)
            );
        }
        let current = preview();
        for (provider_thread, turn) in [("thread", "first"), ("other", "second")] {
            history.record(&store, &session.id, Some(&attachment), Some(&thread),
                &json!({"method":"item/completed","params":{"threadId":provider_thread,"turnId":turn,"item":{"type":"agentMessage","text":"late"}}})).unwrap();
            assert_eq!(preview(), current);
        }
        store
            .record_session_agent_session(&session.id, &attachment, &"replacement".into())
            .unwrap();
        assert_eq!(
            store
                .session_summary(&session.id, 1000)
                .unwrap()
                .unwrap()
                .preview,
            None
        );
        store
            .record_session_agent_session(&session.id, &attachment, &thread)
            .unwrap();
        assert_eq!(preview(), None);
        let replacement = store
            .claim_session_attachment(&session.id, Some(&attachment), &process, false)
            .unwrap();
        history.record(&store, &session.id, Some(&attachment), Some(&thread),
            &json!({"method":"turn/started","params":{"threadId":"thread","turn":{"id":"stale-owner"}}})).unwrap();
        history.record(&store, &session.id, Some(&attachment), Some(&thread),
            &json!({"method":"item/completed","params":{"threadId":"thread","turnId":"stale-owner","item":{"type":"agentMessage","text":"stale owner"}}})).unwrap();
        assert_eq!(preview(), None);
        assert_ne!(replacement.token, attachment.token);
    }

    #[test]
    fn delayed_start_keeps_request_capture_across_a_b_a_takeover() {
        for reply_first in [false, true] {
            let home = tempfile::tempdir().unwrap();
            let path = home.path().join("history.db");
            let store = SqliteStore::open_ephemeral(&path).unwrap();
            let sql = rusqlite::Connection::open(&path).unwrap();
            let original =
                store.test_session("conversation", &crate::session_record::new_artifact_key());
            let a = LfProcessId::new();
            let b = LfProcessId::new();
            for process in [&a, &b] {
                sql.execute(
                    "INSERT INTO processes(id,trace_id,started_at) VALUES(?1,'fixture',1)",
                    [process],
                )
                .unwrap();
            }
            let first = store
                .claim_session_attachment(&original.id, None, &a, false)
                .unwrap();
            let mut history = History::default();
            history
                .request(
                    &json!({"id":1,"method":"turn/start"}),
                    Some((&store, &original.id, &first)),
                )
                .unwrap();
            let middle = store
                .claim_session_attachment(&original.id, Some(&first), &b, false)
                .unwrap();
            let mut next = original.clone();
            next.artifact_key = Some(crate::session_record::new_artifact_key());
            let next = store
                .replace_session_input(original.captured, next)
                .unwrap();
            let current = store
                .claim_session_attachment(&original.id, Some(&middle), &a, false)
                .unwrap();
            assert_ne!(first.token, current.token);
            assert!(store.session_turn_origin(&original.id, &first).is_err());
            // Both requests predate the start notification. A second reply for
            // the same turn must not replace the first correlated origin.
            history
                .request(
                    &json!({"id":2,"method":"turn/start"}),
                    Some((&store, &original.id, &current)),
                )
                .unwrap();
            let reply = json!({"id":1,"result":{"turn":{"id":"late"}}});
            let start = json!({"method":"turn/started","params":{"threadId":"thread","turn":{"id":"late"}}});
            let messages = if reply_first {
                [&reply, &start]
            } else {
                [&start, &reply]
            };
            for message in messages {
                history
                    .record(
                        &store,
                        &original.id,
                        Some(&first),
                        Some(&"thread".into()),
                        message,
                    )
                    .unwrap();
                assert_eq!(sql.query_row("SELECT count(*) FROM session_events WHERE kind='started' AND captured_event=?1", [next.captured], |row| row.get::<_,i64>(0)).unwrap(),0);
            }
            let before = store.session_history(&original.id, 0, 0).unwrap();
            for message in [&json!({"id":2,"result":{"turn":{"id":"late"}}}), &start] {
                history
                    .record(
                        &store,
                        &original.id,
                        Some(&current),
                        Some(&"thread".into()),
                        message,
                    )
                    .unwrap();
            }
            assert_eq!(store.session_history(&original.id, 0, 0).unwrap(), before);
            history.record(&store,&original.id,Some(&first),Some(&"thread".into()),
                &json!({"method":"thread/tokenUsage/updated","params":{"threadId":"thread","turnId":"late","tokenUsage":{"total":{"inputTokens":12},"last":{"inputTokens":12}}}})).unwrap();
            history.record(&store,&original.id,Some(&first),Some(&"thread".into()),
                &json!({"method":"turn/completed","params":{"threadId":"thread","turn":{"id":"late","status":"completed"}}})).unwrap();
            let retained = store
                .session_history(&original.id, 0, 0)
                .unwrap()
                .into_iter()
                .filter(|event| event.provider_turn.as_deref() == Some("late"))
                .collect::<Vec<_>>();
            assert_eq!(retained.len(), 3);
            assert!(retained
                .iter()
                .all(|event| event.lf_process_id.as_deref() == Some(a.as_str())));
            assert_eq!(
                store
                    .input_history(original.input_key().unwrap())
                    .unwrap()
                    .usage
                    .total_input_tokens,
                Some(12)
            );
            assert_eq!(
                store
                    .input_history(next.input_key().unwrap())
                    .unwrap()
                    .usage
                    .total_input_tokens,
                None
            );
            assert_eq!(
                store.session_attachment(&original.id).unwrap(),
                Some(current)
            );
        }
    }

    #[test]
    fn busy_turn_input_preserves_known_and_unknown_origins() {
        let home = tempfile::tempdir().unwrap();
        let path = home.path().join("history.db");
        let store = SqliteStore::open_ephemeral(&path).unwrap();
        let first = LfProcessId::new();
        let second = LfProcessId::new();
        let conn = rusqlite::Connection::open(&path).unwrap();
        store.test_session("conversation", "run_00000000000000000000000000000001");
        for process in [&first, &second] {
            conn.execute(
                "INSERT INTO processes(id,trace_id,started_at) VALUES(?1,'fixture',1)",
                [process.as_str()],
            )
            .unwrap();
        }
        let original = store
            .claim_session_attachment("conversation", None, &first, false)
            .unwrap();
        let original_origin = store
            .session_turn_origin("conversation", &original)
            .unwrap();
        let replacement = SessionAttachment {
            lf_process_id: Some(second.clone()),
            token: crate::id::AttachmentToken::new(),
            ..original.clone()
        };
        for (turn, reply_first) in [("notification-first", false), ("reply-first", true)] {
            let mut history = History::default();
            history
                .request(
                    &json!({"id":1,"method":"turn/start"}),
                    Some((&store, "conversation", &original)),
                )
                .unwrap();
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
                        Some(&"thread".into()),
                        message,
                    )
                    .unwrap();
            }
            let before = store.session_history("conversation", 0, 0).unwrap();
            assert_eq!(
                before.last().unwrap().lf_process_id.as_deref(),
                Some(first.as_str())
            );
            // A reconnect discovers the existing turn; another input receives
            // that same turn rather than a fresh native start notification.
            let mut current = History::default();
            current.record(&store,"conversation",Some(&replacement),Some(&"thread".into()),
                &json!({"result":{"thread":{"id":"thread","turns":[{"id":turn,"status":"inProgress"}]}}})).unwrap();
            current
                .request(
                    &json!({"id":2,"method":"turn/start"}),
                    Some((&store, "conversation", &original)),
                )
                .unwrap();
            current
                .record(
                    &store,
                    "conversation",
                    Some(&replacement),
                    Some(&"thread".into()),
                    &json!({"id":2,"result":{"turn":{"id":turn}}}),
                )
                .unwrap();
            assert_eq!(store.session_history("conversation", 0, 0).unwrap(), before);
        }
        let mut current = History::default();
        // Even a broadcast before this client's request does not establish
        // that this client started the turn.
        current.record(&store,"conversation",Some(&replacement),Some(&"thread".into()),
            &json!({"method":"turn/started","params":{"threadId":"thread","turn":{"id":"unknown"}}})).unwrap();
        current
            .request(
                &json!({"id":3,"method":"turn/start"}),
                Some((&store, "conversation", &original)),
            )
            .unwrap();
        current
            .record(
                &store,
                "conversation",
                Some(&replacement),
                Some(&"thread".into()),
                &json!({"id":3,"result":{"turn":{"id":"unknown"}}}),
            )
            .unwrap();
        let events = store.session_history("conversation", 0, 0).unwrap();
        let unknown = events
            .iter()
            .find(|event| event.provider_turn.as_deref() == Some("unknown"))
            .unwrap();
        assert_eq!(unknown.kind, SessionEventKind::Started);
        assert_eq!(unknown.lf_process_id, None);
        assert_eq!(unknown.agent_process_id, None);
        assert!(
            store
                .record_session_turn_origin(
                    &"thread".into(),
                    "reply-first",
                    &crate::session::SessionTurnOrigin {
                        lf_process_id: second,
                        ..original_origin
                    }
                )
                .is_err(),
            "contradictory actual origin evidence still fails"
        );
    }
}
