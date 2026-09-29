use rusqlite::{params, OptionalExtension, TransactionBehavior};
use serde_json::Value;
use sha2::{Digest, Sha256};

use crate::durable::RunId;
use crate::id::ExecId;
use crate::session::{SessionEvent, SessionEventKind};
use crate::store::{StoreError, StoreResult};

use super::SqliteStore;

impl SqliteStore {
    pub(crate) fn input_provider_session(
        &self,
        input: &RunId,
    ) -> StoreResult<Option<crate::run_record::ProviderSessionRef>> {
        let Some(session) = self.session_for_run(input)? else {
            return Ok(None);
        };
        crate::run_record::provider_session_from_history(
            self.summary_for_input(&session.id, input)?
                .into_iter()
                .map(|event| event.payload),
        )
        .map_err(|error| StoreError::InvalidData(error.to_string()))
    }

    /// Original input order survives importing earlier observations after later ones.
    pub(crate) fn input_events(&self, input: &RunId) -> StoreResult<Vec<Value>> {
        let conn = self.conn.lock().expect("store mutex poisoned");
        let mut query = conn.prepare(
            "SELECT e.payload FROM session_events e
             JOIN agent_session_inputs i ON i.session_id=e.session_id AND i.input_id=?1
             WHERE e.kind='observed' AND substr(e.receipt_key,1,length(?1)+14)=?1||':events.jsonl:'
             ORDER BY CAST(substr(e.receipt_key,length(?1)+15) AS INTEGER),e.seq",
        )?;
        let rows = query.query_map([input.as_str()], |row| row.get::<_, String>(0))?;
        rows.map(|row| Ok(serde_json::from_str::<Value>(&row?)?["evidence"].clone()))
            .collect()
    }

    pub(crate) fn input_final_answer(
        &self,
        input: &RunId,
    ) -> StoreResult<Option<crate::run_record::FinalAnswer>> {
        crate::run_record::final_answer(self.input_events(input)?)
            .map_err(|error| StoreError::InvalidData(error.to_string()))
    }

    pub(crate) fn retain_session_observation(
        &self,
        session: &crate::session::AgentSession,
        observation: &crate::session::SessionObservation,
    ) -> StoreResult<()> {
        let mut conn = self.conn.lock().expect("store mutex poisoned");
        let tx = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
        super::sessions::retain_history_in(&tx, session, std::slice::from_ref(observation))?;
        tx.commit()?;
        Ok(())
    }

    /// Retain a provider observation even when its conversational driver has
    /// changed. Observation grants neither native write nor Flow authority.
    pub(crate) fn record_session_event(
        &self,
        session: &str,
        thread: &str,
        turn: &str,
        kind: SessionEventKind,
        payload: &Value,
    ) -> StoreResult<i64> {
        let payload = serde_json::to_string(payload)?;
        let receipt = if kind == SessionEventKind::Usage {
            format!("{:x}", Sha256::digest(payload.as_bytes()))
        } else {
            String::new()
        };
        let mut conn = self.conn.lock().expect("store mutex poisoned");
        let tx = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
        let existing: Option<(i64, String)> = tx.query_row(
            "SELECT seq,payload FROM session_events
             WHERE session_id=?1 AND provider_thread=?2 AND provider_turn=?3 AND kind=?4 AND receipt_key=?5",
            params![session, thread, turn, kind.as_str(), receipt],
            |row| Ok((row.get(0)?, row.get(1)?)),
        ).optional()?;
        if let Some((seq, saved)) = existing {
            if saved != payload {
                return Err(StoreError::InvalidData(format!(
                    "Conflicting {} receipt for conversation {session}, turn {turn}",
                    kind.as_str()
                )));
            }
            return Ok(seq);
        }
        // Attribution follows the observed start. A completion recovered without
        // that start retains missing attribution instead of borrowing today's bind.
        let attribution: (Option<String>, Option<String>, Option<String>) =
            if kind == SessionEventKind::Started {
                tx.query_row(
                    "SELECT task_id,wave_id,input_id FROM agent_sessions WHERE id=?1",
                    [session],
                    |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
                )?
            } else {
                (None, None, None)
            };
        tx.execute(
            "INSERT INTO session_events(session_id,provider_thread,provider_turn,kind,receipt_key,task_id,wave_id,observed_at,payload,input_id)
             VALUES(?1,?2,?3,?4,?5,?6,?7,?8,?9,?10)",
            params![session, thread, turn, kind.as_str(), receipt, attribution.0, attribution.1,
                time::OffsetDateTime::now_utc().unix_timestamp(), payload, attribution.2],
        )?;
        let seq = tx.last_insert_rowid();
        tx.commit()?;
        Ok(seq)
    }

    /// A correlated turn/start reply identifies its initiating Exec. A broadcast
    /// alone does not: several connected observers can receive the same start.
    pub(crate) fn record_session_turn_origin(
        &self,
        session: &str,
        thread: &str,
        turn: &str,
        generation: i64,
        exec: &ExecId,
    ) -> StoreResult<i64> {
        let seq = self.record_session_event(
            session,
            thread,
            turn,
            SessionEventKind::Started,
            &serde_json::json!({}),
        )?;
        let conn = self.conn.lock().expect("store mutex poisoned");
        let changed = conn.execute(
            "UPDATE session_events SET exec_id=?4,provider_generation=?5 WHERE session_id=?1 AND provider_thread=?2 AND provider_turn=?3
             AND kind='started' AND (exec_id IS NULL OR exec_id=?4)
             AND (provider_generation IS NULL OR provider_generation=?5)",
            params![session, thread, turn, exec, generation],
        )?;
        if changed != 1 {
            return Err(StoreError::InvalidData(
                "Native turn has a different initiating Exec".into(),
            ));
        }
        Ok(seq)
    }

    pub fn session_history(
        &self,
        session: &str,
        after: i64,
        limit: usize,
    ) -> StoreResult<Vec<SessionEvent>> {
        self.read_history(session, after, limit, None)
    }

    /// Keep usage, provenance and unknown evidence; transcript payloads belong to detail.
    pub(super) fn summary_for_input(
        &self,
        session: &str,
        input: &RunId,
    ) -> StoreResult<Vec<SessionEvent>> {
        self.read_history(session, 0, 0, Some(input.as_str()))
    }

    fn read_history(
        &self,
        session: &str,
        after: i64,
        limit: usize,
        input: Option<&str>,
    ) -> StoreResult<Vec<SessionEvent>> {
        let conn = self.conn.lock().expect("store mutex poisoned");
        let mut query = conn.prepare(
            "SELECT e.seq,e.session_id,e.provider_thread,e.provider_turn,e.kind,
                    origin.provider_generation,origin.exec_id,
                    CASE WHEN e.kind='observed' THEN e.task_id ELSE origin.task_id END,
                    CASE WHEN e.kind='observed' THEN e.wave_id ELSE origin.wave_id END,e.observed_at,e.payload
             FROM session_events e LEFT JOIN session_events origin
               ON origin.session_id=e.session_id AND origin.provider_thread=e.provider_thread
               AND origin.provider_turn=e.provider_turn AND origin.kind='started'
             WHERE e.session_id=?1 AND e.seq>?2
             AND (?4 IS NULL OR (e.kind='observed' AND substr(e.receipt_key,1,length(?4)+1)=?4||':')
                  OR (e.kind!='observed' AND origin.input_id=?4))
             AND (?4 IS NULL OR CASE WHEN json_valid(e.payload) THEN
                 COALESCE(json_extract(e.payload,'$.evidence.schema_version'),0)!=1 OR
                 COALESCE(json_extract(e.payload,'$.evidence.type'),'') NOT IN
                     ('activity','handoff','user_input','conversation','text','tool_use','result','provider_output')
                 ELSE 1 END)
             ORDER BY e.seq LIMIT ?3",
        )?;
        let rows = query.query_map(
            params![
                session,
                after,
                if limit == 0 { -1 } else { limit as i64 },
                input
            ],
            |row| {
                Ok((
                    row.get::<_, i64>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, Option<String>>(2)?,
                    row.get::<_, Option<String>>(3)?,
                    row.get::<_, String>(4)?,
                    row.get::<_, Option<i64>>(5)?,
                    row.get::<_, Option<String>>(6)?,
                    row.get::<_, Option<String>>(7)?,
                    row.get::<_, Option<String>>(8)?,
                    row.get::<_, i64>(9)?,
                    row.get::<_, String>(10)?,
                ))
            },
        )?;
        rows.map(|row| {
            let (
                seq,
                session_id,
                provider_thread,
                provider_turn,
                kind,
                provider_generation,
                exec_id,
                task_id,
                wave_id,
                observed_at,
                payload,
            ) = row?;
            Ok(SessionEvent {
                seq,
                session_id,
                provider_thread,
                provider_turn,
                kind: match kind.as_str() {
                    "started" => SessionEventKind::Started,
                    "usage" => SessionEventKind::Usage,
                    "completed" => SessionEventKind::Completed,
                    "observed" => SessionEventKind::Observed,
                    _ => {
                        return Err(StoreError::InvalidData(format!(
                            "Unknown Session event {kind}"
                        )))
                    }
                },
                provider_generation,
                exec_id,
                task_id,
                wave_id,
                observed_at,
                payload: serde_json::from_str(&payload)?,
            })
        })
        .collect()
    }
}

#[cfg(test)]
mod tests {
    use crate::session::SessionEventKind;
    use crate::store::sqlite::SqliteStore;
    use serde_json::json;

    #[test]
    fn provider_identity_uses_original_event_order_and_fresh_publications() {
        let home = tempfile::tempdir().unwrap();
        let store = SqliteStore::open_ephemeral(&home.path().join("store.db")).unwrap();
        store.conn.lock().unwrap().execute_batch(
            "INSERT INTO agent_sessions(id,input_id,title,title_source,created_at,kind,interactive,input_published,cwd)
             VALUES('conversation','run_00000000000000000000000000000001','Retained','human',1,'conversation',1,1,'/fixture');
             INSERT INTO agent_session_inputs(input_id,session_id) VALUES('run_00000000000000000000000000000001','conversation');"
        ).unwrap();
        let input = crate::durable::RunId::parse("run_00000000000000000000000000000001").unwrap();
        let session = store.session("conversation").unwrap().unwrap();
        let retain = |source: &str, evidence: serde_json::Value| {
            store
                .retain_session_observation(
                    &session,
                    &crate::session::SessionObservation {
                        input_id: input.clone(),
                        source: source.into(),
                        observed_at: 1,
                        task_id: None,
                        wave_id: None,
                        payload: json!({"input_id":input,"source":source,"evidence":evidence}),
                    },
                )
                .unwrap();
        };
        let event = |seq, fields: serde_json::Value| {
            let mut value =
                json!({"schema_version":1,"seq":seq,"observed_at":"2026-09-29T00:00:00Z"});
            value
                .as_object_mut()
                .unwrap()
                .extend(fields.as_object().unwrap().clone());
            value
        };
        // The thread arrives in SQL before its earlier account selection.
        retain(
            "events.jsonl:2",
            event(
                2,
                json!({"type":"provider_session_observed","attempt_key":"second","provider_session_id":"second-thread"}),
            ),
        );
        retain(
            "events.jsonl:1",
            event(
                1,
                json!({"type":"provider_account_selected","attempt_key":"second","account_id":"recorded"}),
            ),
        );
        let reference = store.input_provider_session(&input).unwrap().unwrap();
        assert_eq!(reference.provider_session_id, "second-thread");
        assert_eq!(reference.account_id.unwrap().as_str(), "recorded");
        // Importing an older thread later cannot replace that pair.
        retain(
            "events.jsonl:0",
            event(
                0,
                json!({"type":"provider_session_observed","attempt_key":"first","provider_session_id":"first-thread"}),
            ),
        );
        let reference = store.input_provider_session(&input).unwrap().unwrap();
        assert_eq!(reference.provider_session_id, "second-thread");
        assert_eq!(reference.account_id.unwrap().as_str(), "recorded");
        retain(
            "events.jsonl:3",
            event(
                3,
                json!({"type":"provider_session_observed","attempt_key":"unknown","provider_session_id":"unknown-account"}),
            ),
        );
        assert_eq!(
            store
                .input_provider_session(&input)
                .unwrap()
                .unwrap()
                .account_id,
            None
        );
        // A late import has no native observation timestamp and cannot overwrite
        // an already published fresh identity, including its unknown account.
        retain(
            "provider-session:fresh",
            json!({"schema_version":1,"provider_session_id":"fresh-thread","account_id":null}),
        );
        retain(
            "provider-session.json",
            json!({"schema_version":1,"provider_session_id":"legacy-thread","account_id":"legacy"}),
        );
        let reference = store.input_provider_session(&input).unwrap().unwrap();
        assert_eq!(reference.provider_session_id, "fresh-thread");
        assert_eq!(reference.account_id, None);
    }

    #[test]
    fn summary_reads_usage_without_hydrating_conversation_text() {
        let home = tempfile::tempdir().unwrap();
        let store = SqliteStore::open_ephemeral(&home.path().join("store.db")).unwrap();
        store.conn.lock().unwrap().execute_batch(
            "INSERT INTO agent_sessions(id,input_id,title,title_source,created_at,kind,interactive,input_published,cwd)
             VALUES('conversation','run_00000000000000000000000000000001','Retained','human',1,'conversation',1,1,'/fixture');
             INSERT INTO agent_session_inputs(input_id,session_id) VALUES('run_00000000000000000000000000000001','conversation');"
        ).unwrap();
        let input = crate::durable::RunId::parse("run_00000000000000000000000000000001").unwrap();
        let session = store.session("conversation").unwrap().unwrap();
        let text = "retained transcript ".repeat(4096);
        let events = [
            json!({"schema_version":1,"seq":0,"type":"conversation","event":{"type":"text_delta","turn_id":"turn","content":text}}),
            json!({"schema_version":1,"seq":1,"type":"usage","usage":{"input_tokens":12}}),
            json!({"unparsed":"partial historical event"}),
        ];
        for (seq, evidence) in events.iter().enumerate() {
            let source = format!("events.jsonl:{seq}");
            store
                .retain_session_observation(
                    &session,
                    &crate::session::SessionObservation {
                        input_id: input.clone(),
                        source: source.clone(),
                        observed_at: 1,
                        task_id: None,
                        wave_id: None,
                        payload: json!({"input_id":input,"source":source,"evidence":evidence}),
                    },
                )
                .unwrap();
        }
        let summary = store.summary_for_input(&session.id, &input).unwrap();
        assert_eq!(summary.len(), 2);
        assert_eq!(summary[0].payload["evidence"], events[1]);
        assert_eq!(summary[1].payload["evidence"], events[2]);
        assert_eq!(store.input_events(&input).unwrap(), events);
        assert_eq!(store.session_history(&session.id, 0, 0).unwrap().len(), 3);
    }

    #[test]
    fn native_usage_survives_driver_and_input_replacement_without_double_counting() {
        let home = tempfile::tempdir().unwrap();
        let store = SqliteStore::open_ephemeral(&home.path().join("store.db")).unwrap();
        store.conn.lock().unwrap().execute_batch(
            "INSERT INTO agent_sessions(id,input_id,title,title_source,created_at,kind,interactive,input_published,cwd)
             VALUES('conversation','run_00000000000000000000000000000001','Retained','human',1,'conversation',1,1,'/fixture');
             INSERT INTO agent_session_inputs(input_id,session_id) VALUES('run_00000000000000000000000000000001','conversation');"
        ).unwrap();
        let first_input =
            crate::durable::RunId::parse("run_00000000000000000000000000000001").unwrap();
        let session = store.session("conversation").unwrap().unwrap();
        let first = crate::id::ExecId::new();
        let second = crate::id::ExecId::new();
        for exec in [&first, &second] {
            store
                .conn
                .lock()
                .unwrap()
                .execute(
                    "INSERT INTO execs(id,trace_id,started_at) VALUES(?1,'fixture',1)",
                    [exec.as_str()],
                )
                .unwrap();
        }
        let driver = store
            .claim_session_driver(&session.id, None, &first, false)
            .unwrap();
        store
            .record_session_turn_origin(&session.id, "thread", "turn", 1, &first)
            .unwrap();
        let counts = |input, output| {
            json!({"inputTokens":input,"outputTokens":output,
            "cachedInputTokens":2,"reasoningOutputTokens":1})
        };
        store
            .record_session_event(
                &session.id,
                "thread",
                "turn",
                SessionEventKind::Usage,
                &json!({"total":counts(12,3),"last":counts(12,3)}),
            )
            .unwrap();
        let mut replacement = session.clone();
        replacement.input_id = crate::durable::RunId::new();
        store
            .replace_session_input(&first_input, replacement.clone())
            .unwrap();
        store
            .claim_session_driver(&session.id, Some(&driver), &second, true)
            .unwrap();
        // Gen 2 observes the surviving Gen 1 turn. The recorder never saw its usage.
        store
            .record_session_event(
                &session.id,
                "thread",
                "turn",
                SessionEventKind::Usage,
                &json!({"total":counts(20,5),"last":counts(8,2)}),
            )
            .unwrap();
        store
            .record_session_event(
                &session.id,
                "thread",
                "turn",
                SessionEventKind::Completed,
                &json!({"status":"completed"}),
            )
            .unwrap();
        let history = store.summary_for_input(&session.id, &first_input).unwrap();
        assert_eq!(history.len(), 4);
        assert!(history
            .iter()
            .all(|event| event.exec_id.as_deref() == Some(first.as_str())
                && event.provider_generation == Some(1)));
        assert!(store
            .summary_for_input(&session.id, &replacement.input_id)
            .unwrap()
            .is_empty());
        let old = store.input_snapshot(first_input.as_str()).unwrap();
        assert_eq!(old.usage.input_tokens, Some(18));
        assert_eq!(old.usage.output_tokens, Some(5));
        assert_eq!(old.usage.cost_usd, None);
        assert_eq!(old.usage.final_streams, 0);
        assert_eq!(
            old.outcome, None,
            "native completion cannot invent the command outcome"
        );
        // A later recovered checkpoint measures this same turn, not extra work.
        for (seq, event) in [
            json!({"type":"provider_session_observed","attempt_key":"attempt","provider_session_id":"thread"}),
            json!({"type":"usage","provider":"codex","model":null,"attempt_key":"attempt","turn_key":"turn",
                "usage_stream_id":"recorder","observation_seq":1,"counter_kind":"cumulative","start_known":true,
                "final_receipt":true,"usage":{"input_tokens":18,"output_tokens":5,"cost_usd":0.2}})
        ].into_iter().enumerate() {
            let mut evidence = json!({"schema_version":1,"seq":seq,"observed_at":"2026-09-29T00:00:00Z"});
            evidence.as_object_mut().unwrap().extend(event.as_object().unwrap().clone());
            let source = format!("events.jsonl:{seq}");
            store.retain_session_observation(&session, &crate::session::SessionObservation {
                input_id:first_input.clone(), source:source.clone(), observed_at:1, task_id:None,wave_id:None,
                payload:json!({"input_id":first_input,"source":source,"evidence":evidence})
            }).unwrap();
        }
        let old = store.input_snapshot(first_input.as_str()).unwrap();
        assert_eq!(old.usage.streams, 1);
        assert_eq!(old.usage.input_tokens, Some(18));
        assert_eq!(old.usage.output_tokens, Some(5));
        assert_eq!(old.usage.cost_usd, Some(0.2));
        assert_eq!(old.usage.final_streams, 1);
        // An unobserved start cannot borrow the replacement's input or attribution.
        store
            .record_session_event(
                &session.id,
                "thread",
                "unknown",
                SessionEventKind::Usage,
                &json!({"total":counts(100,50),"last":counts(10,5)}),
            )
            .unwrap();
        assert!(store
            .summary_for_input(&session.id, &replacement.input_id)
            .unwrap()
            .is_empty());
        assert_eq!(
            store
                .session_history(&session.id, 0, 0)
                .unwrap()
                .last()
                .unwrap()
                .exec_id,
            None
        );
        // Even with a start, a late first usage snapshot is a lower bound, not the thread total.
        store
            .record_session_event(
                &session.id,
                "thread",
                "partial",
                SessionEventKind::Started,
                &json!({}),
            )
            .unwrap();
        store
            .record_session_event(
                &session.id,
                "thread",
                "partial",
                SessionEventKind::Usage,
                &json!({"total":counts(100,50),"last":counts(10,5)}),
            )
            .unwrap();
        let partial = store.input_snapshot(replacement.input_id.as_str()).unwrap();
        assert_eq!(partial.usage.total_input_tokens, Some(10));
        assert_eq!(partial.usage.output_tokens, Some(5));
        assert!(partial.usage.gaps > 0);
        store
            .record_session_event(
                &session.id,
                "thread",
                "decrease",
                SessionEventKind::Started,
                &json!({}),
            )
            .unwrap();
        for (total, last) in [(20, 20), (40, 20), (30, 10)] {
            store.record_session_event(&session.id,"thread","decrease",SessionEventKind::Usage,
                &json!({"total":{"inputTokens":total,"outputTokens":0,"cachedInputTokens":0,"reasoningOutputTokens":0},
                    "last":{"inputTokens":last,"outputTokens":0,"cachedInputTokens":0,"reasoningOutputTokens":0}})).unwrap();
        }
        let decreased = store.input_snapshot(replacement.input_id.as_str()).unwrap();
        assert_eq!(
            decreased.usage.total_input_tokens,
            Some(50),
            "retain 40 observed tokens plus the separate 10-token partial turn"
        );
        assert!(decreased.usage.gaps > partial.usage.gaps);
        store
            .record_session_event(
                &session.id,
                "thread",
                "decrease",
                SessionEventKind::Completed,
                &json!({"status":"completed"}),
            )
            .unwrap();
        store
            .record_session_event(
                &session.id,
                "thread",
                "next",
                SessionEventKind::Started,
                &json!({}),
            )
            .unwrap();
        // Reconnect missed one request: 60 lifetime minus the retained 40 peak,
        // not merely this final request's 10 tokens or the regressed 30 baseline.
        store.record_session_event(&session.id,"thread","next",SessionEventKind::Usage,
            &json!({"total":{"inputTokens":60,"outputTokens":0,"cachedInputTokens":0,"reasoningOutputTokens":0},
                "last":{"inputTokens":10,"outputTokens":0,"cachedInputTokens":0,"reasoningOutputTokens":0}})).unwrap();
        assert_eq!(
            store
                .input_snapshot(replacement.input_id.as_str())
                .unwrap()
                .usage
                .total_input_tokens,
            Some(70)
        );
    }

    #[test]
    fn recovered_completion_keeps_missing_start_and_usage_and_rejects_conflicts() {
        let home = tempfile::tempdir().unwrap();
        let store = SqliteStore::open_ephemeral(&home.path().join("store.db")).unwrap();
        store
            .conn
            .lock()
            .unwrap()
            .execute_batch(
                "BEGIN;
             INSERT INTO agent_sessions(id,input_id,title,title_source,created_at,kind,interactive,input_published,cwd)
             VALUES('conversation','run_fixture','Retained','human',1,'conversation',1,1,'/fixture');
             INSERT INTO agent_session_inputs(input_id,session_id)
             VALUES('run_fixture','conversation');
             COMMIT;",
            )
            .unwrap();
        let completed = json!({"status":"completed", "error":null, "started_at":10, "completed_at":20, "duration_ms":10000});
        let seq = store
            .record_session_event(
                "conversation",
                "thread",
                "turn",
                SessionEventKind::Completed,
                &completed,
            )
            .unwrap();
        assert_eq!(
            store
                .record_session_event(
                    "conversation",
                    "thread",
                    "turn",
                    SessionEventKind::Completed,
                    &completed
                )
                .unwrap(),
            seq
        );
        assert!(store
            .record_session_event(
                "conversation",
                "thread",
                "turn",
                SessionEventKind::Completed,
                &json!({"status":"failed"})
            )
            .is_err());
        let history = store.session_history("conversation", 0, 100).unwrap();
        assert_eq!(
            history.len(),
            1,
            "recovery cannot invent start or usage receipts"
        );
        assert_eq!(history[0].payload, completed);
        assert_eq!(history[0].provider_generation, None);
        assert_eq!(history[0].exec_id, None);
        assert!(store
            .session_history("conversation", seq, 100)
            .unwrap()
            .is_empty());

        let first = crate::id::ExecId::new();
        let second = crate::id::ExecId::new();
        for exec in [&first, &second] {
            store
                .conn
                .lock()
                .unwrap()
                .execute(
                    "INSERT INTO execs(id,trace_id,started_at) VALUES(?1,'fixture',1)",
                    [exec.as_str()],
                )
                .unwrap();
        }
        let original = store
            .claim_session_driver("conversation", None, &first, false)
            .unwrap();
        store
            .record_session_connection("conversation", &original, "/original.sock", "thread")
            .unwrap();
        store
            .record_session_provider_process("conversation", &original, 12345, 12)
            .unwrap();
        store
            .record_session_turn_origin("conversation", "thread", "later", 1, &first)
            .unwrap();
        let replacement = store
            .claim_session_driver("conversation", Some(&original), &second, true)
            .unwrap();
        assert_eq!(replacement.provider_generation, 2);
        assert!(store.session_connection("conversation").unwrap().is_none());
        assert!(store
            .session_provider_process("conversation")
            .unwrap()
            .is_none());
        assert_eq!(
            store.session_thread("conversation").unwrap().as_deref(),
            Some("thread")
        );
        assert!(store
            .record_session_connection("conversation", &original, "/stale.sock", "wrong-thread")
            .is_err());
        assert!(store
            .record_session_provider_process("conversation", &original, 12346, 13)
            .is_err());
        store
            .record_session_event(
                "conversation",
                "thread",
                "later",
                SessionEventKind::Usage,
                &json!({"total":{"inputTokens":40}}),
            )
            .unwrap();
        store
            .record_session_event(
                "conversation",
                "thread",
                "unobserved",
                SessionEventKind::Usage,
                &json!({"total":{"inputTokens":60}}),
            )
            .unwrap();
        let events = store.session_history("conversation", seq, 100).unwrap();
        assert_eq!(events[1].provider_generation, Some(1));
        assert_eq!(events[1].exec_id.as_deref(), Some(first.as_str()));
        assert_eq!(events[2].provider_generation, None);
        assert_eq!(events[2].exec_id, None);
    }
}
