use rusqlite::{params, OptionalExtension, TransactionBehavior};
use serde_json::Value;
use sha2::{Digest, Sha256};

use crate::id::ExecId;
use crate::session::{SessionEvent, SessionEventKind};
use crate::session_record::{FinalAnswer, ProviderSessionRef};
use crate::store::{StoreError, StoreResult};

use super::SqliteStore;

impl SqliteStore {
    pub(crate) fn input_provider_session(
        &self,
        input: &str,
    ) -> StoreResult<Option<ProviderSessionRef>> {
        let Some(session) = self.session_for_artifact(input)? else {
            return Ok(None);
        };
        crate::session_record::provider_session_from_history(
            self.summary_for_input(&session.id, input)?
                .into_iter()
                .map(|event| event.payload),
        )
        .map_err(|error| StoreError::InvalidData(error.to_string()))
    }

    /// Original input order survives importing earlier observations after later ones.
    pub(crate) fn input_events(&self, input: &str) -> StoreResult<Vec<Value>> {
        let conn = self.conn.lock().expect("store mutex poisoned");
        let mut query = conn.prepare(
            "SELECT e.payload FROM session_events e
             JOIN session_events c ON c.seq=e.captured_event AND c.kind='captured' AND c.receipt_key=?1
             WHERE e.kind='observed' AND substr(e.receipt_key,1,length(?1)+14)=?1||':events.jsonl:'
             ORDER BY CAST(substr(e.receipt_key,length(?1)+15) AS INTEGER),e.seq",
        )?;
        let rows = query.query_map([input], |row| row.get::<_, String>(0))?;
        rows.map(|row| Ok(serde_json::from_str::<Value>(&row?)?["evidence"].clone()))
            .collect()
    }

    /// When each retained event of one input was observed, in input order.
    pub(crate) fn input_event_times(&self, input: &str) -> StoreResult<Vec<i64>> {
        let conn = self.conn.lock().expect("store mutex poisoned");
        let mut query = conn.prepare(
            "SELECT unixepoch(json_extract(e.payload,'$.evidence.observed_at')) AS observed FROM session_events e
             JOIN session_events c ON c.seq=e.captured_event AND c.kind='captured' AND c.receipt_key=?1
             WHERE e.kind='observed' AND substr(e.receipt_key,1,length(?1)+14)=?1||':events.jsonl:' AND observed IS NOT NULL
             ORDER BY CAST(substr(e.receipt_key,length(?1)+15) AS INTEGER),e.seq",
        )?;
        let rows = query.query_map([input], |row| row.get(0))?;
        Ok(rows.collect::<Result<_, _>>()?)
    }

    pub(crate) fn input_final_answer(&self, input: &str) -> StoreResult<Option<FinalAnswer>> {
        let native: Option<String> = {
            let conn = self.conn.lock().expect("store mutex poisoned");
            conn.query_row(
                "SELECT output.payload FROM session_events start
                 JOIN session_events done ON done.session_id=start.session_id
                   AND done.provider_thread=start.provider_thread AND done.provider_turn=start.provider_turn
                   AND done.kind='completed' AND json_extract(done.payload,'$.status')='completed'
                 JOIN session_events output ON output.session_id=start.session_id
                   AND output.provider_thread=start.provider_thread AND output.provider_turn=start.provider_turn AND output.kind='output'
                 WHERE start.kind='started' AND start.captured_event=(SELECT seq FROM session_events WHERE kind='captured' AND receipt_key=?1) ORDER BY done.seq DESC LIMIT 1",
                 [input], |row| row.get(0)).optional()?
        };
        if let Some(payload) = native {
            let value: Value = serde_json::from_str(&payload)?;
            let text = match value.get("value") {
                Some(output) => serde_json::to_string(output)?,
                None => value["text"]
                    .as_str()
                    .ok_or_else(|| {
                        StoreError::InvalidData("native output has no value or text".into())
                    })?
                    .to_owned(),
            };
            return Ok(Some(FinalAnswer { text, exact: true }));
        }
        crate::session_record::final_answer(self.input_events(input)?)
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
        let attribution: (Option<String>, Option<String>, Option<i64>) =
            if kind == SessionEventKind::Started {
                tx.query_row(
                    "SELECT task_id,wave_id,current_capture FROM agent_sessions WHERE id=?1",
                    [session],
                    |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
                )?
            } else {
                (None, None, None)
            };
        tx.execute(
            "INSERT INTO session_events(session_id,provider_thread,provider_turn,kind,receipt_key,task_id,wave_id,observed_at,payload,captured_event)
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
        self.read_history(session, after, limit, None, (None, None, None), None)
    }

    /// Keep usage, provenance and unknown evidence; transcript payloads belong to detail.
    pub(super) fn summary_for_input(
        &self,
        session: &str,
        input: &str,
    ) -> StoreResult<Vec<SessionEvent>> {
        self.read_history(session, 0, 0, Some(input), (None, None, None), None)
    }

    pub(super) fn summary_for_input_matching(
        &self,
        session: &str,
        input: &str,
        scope: (Option<&str>, Option<&str>, Option<&str>),
    ) -> StoreResult<Vec<SessionEvent>> {
        self.read_history(session, 0, 0, Some(input), scope, None)
    }

    pub(super) fn orphaned_native_history(
        &self,
        session: &str,
        scope: (Option<&str>, Option<&str>, Option<&str>),
        thread: Option<&str>,
        turn: Option<&str>,
    ) -> StoreResult<Vec<SessionEvent>> {
        self.read_history(session, 0, 0, None, scope, Some((thread, turn)))
    }

    fn read_history(
        &self,
        session: &str,
        after: i64,
        limit: usize,
        input: Option<&str>,
        scope: (Option<&str>, Option<&str>, Option<&str>),
        orphaned: Option<(Option<&str>, Option<&str>)>,
    ) -> StoreResult<Vec<SessionEvent>> {
        let conn = self.conn.lock().expect("store mutex poisoned");
        let mut query = conn.prepare(
            "SELECT e.seq,e.session_id,e.provider_thread,e.provider_turn,e.kind,
                    origin.provider_generation,CASE WHEN e.kind='captured' THEN e.exec_id ELSE origin.exec_id END,
                    CASE WHEN e.kind IN ('observed','captured') THEN e.task_id ELSE origin.task_id END,
                    CASE WHEN e.kind IN ('observed','captured') THEN e.wave_id ELSE origin.wave_id END,e.observed_at,e.payload
             FROM session_events e LEFT JOIN session_events origin
               ON origin.session_id=e.session_id AND origin.provider_thread=e.provider_thread
               AND origin.provider_turn=e.provider_turn AND origin.kind='started'
             WHERE e.session_id=?1 AND e.seq>?2
             AND (NOT ?8 OR (e.kind IN ('started','usage','completed','output') AND origin.captured_event IS NULL
                  AND e.provider_thread IS ?9 AND e.provider_turn IS ?10))
             AND (?4 IS NULL OR (e.kind='observed' AND substr(e.receipt_key,1,length(?4)+1)=?4||':')
                  OR (e.kind!='observed' AND origin.captured_event=(SELECT seq FROM session_events WHERE kind='captured' AND receipt_key=?4)))
             AND (?4 IS NULL OR CASE WHEN json_valid(e.payload) THEN
                 COALESCE(json_extract(e.payload,'$.evidence.schema_version'),0)!=1 OR
                 COALESCE(json_extract(e.payload,'$.evidence.type'),'') NOT IN
                     ('activity','handoff','user_input','conversation','text','tool_use','result','provider_output')
                 ELSE 1 END)
             AND ((e.kind='observed' AND CASE WHEN json_valid(e.payload) THEN json_extract(e.payload,'$.source') END IN ('manifest.json','runs','terminal.json')) OR (
                 (?5 IS NULL OR (CASE WHEN e.kind IN ('observed','captured') THEN e.wave_id ELSE origin.wave_id END)
                     IN (SELECT id FROM wave_addresses WHERE id=?5 OR slug=?5))
                 AND (?6 IS NULL OR (CASE WHEN e.kind IN ('observed','captured') THEN e.task_id ELSE origin.task_id END)
                     IN (SELECT t.id FROM tasks t JOIN projects p ON p.id=t.project_id
                         WHERE p.id=?6 OR p.project_slug=?6 OR p.external_project_id=?6))
                 AND (?7 IS NULL OR (CASE WHEN e.kind IN ('observed','captured') THEN e.task_id ELSE origin.task_id END)
                     IN (SELECT id FROM tasks WHERE id=?7 OR issue_identifier=?7 OR external_issue_id=?7))))
             ORDER BY e.seq LIMIT ?3",
        )?;
        let rows = query.query_map(
            params![
                session,
                after,
                if limit == 0 { -1 } else { limit as i64 },
                input,
                scope.0,
                scope.1,
                scope.2,
                orphaned.is_some(),
                orphaned.and_then(|(thread, _)| thread),
                orphaned.and_then(|(_, turn)| turn)
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
                    "output" => SessionEventKind::Output,
                    "observed" => SessionEventKind::Observed,
                    "captured" => SessionEventKind::Captured,
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
        store.test_session("conversation", "run_00000000000000000000000000000001");
        let input =
            crate::session_record::parse_artifact_key("run_00000000000000000000000000000001")
                .unwrap();
        let session = store.session("conversation").unwrap().unwrap();
        let retain = |source: &str, evidence: serde_json::Value| {
            store
                .retain_session_observation(
                    &session,
                    &crate::session::SessionObservation {
                        artifact_key: input.clone(),
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
    fn first_provider_attempt_follows_original_input_order_after_import() {
        let home = tempfile::tempdir().unwrap();
        let store = SqliteStore::open_ephemeral(&home.path().join("store.db")).unwrap();
        store.test_session("conversation", "run_00000000000000000000000000000001");
        let input =
            crate::session_record::parse_artifact_key("run_00000000000000000000000000000001")
                .unwrap();
        let session = store.session("conversation").unwrap().unwrap();
        assert_eq!(
            store
                .input_history(&input)
                .unwrap()
                .first_provider_attempt_at,
            None
        );
        let pr = crate::work::task::TaskPrId::new();
        let manifest = json!({"schema_version":1,"artifact_key":input,
            "created_at":"1970-01-01T00:00:01Z","harness":"codex","surface":"headless",
            "cwd":"/fixture","subjects":[],"host":"fixture",
            "flow":{"kind":"step","task_pr_id":pr,"invocation_id":"retained",
                "flow":"feature","step":"review"}});
        store
            .retain_session_observation(
                &session,
                &crate::session::SessionObservation {
                    artifact_key: input.clone(),
                    source: "manifest.json".into(),
                    observed_at: 1,
                    task_id: None,
                    wave_id: None,
                    payload: json!({"input_id":input,"source":"manifest.json","evidence":manifest}),
                },
            )
            .unwrap();
        for seq in [1, 0] {
            let source = format!("events.jsonl:{seq}");
            let time = time::OffsetDateTime::from_unix_timestamp(10 + seq * 10).unwrap();
            let evidence = json!({"schema_version":1,"seq":seq,
                "observed_at":time.format(&time::format_description::well_known::Rfc3339).unwrap(),
                "type":"provider_attempt_started","attempt_key":format!("attempt-{seq}"),
                "provider":"codex","model":null});
            store
                .retain_session_observation(
                    &session,
                    &crate::session::SessionObservation {
                        artifact_key: input.clone(),
                        source: source.clone(),
                        observed_at: 100 + seq,
                        task_id: None,
                        wave_id: None,
                        payload: json!({"input_id":input,"source":source,"evidence":evidence}),
                    },
                )
                .unwrap();
        }
        let snapshot = store.input_history(&input).unwrap();
        assert_eq!(snapshot.task_pr_id, Some(pr));
        assert_eq!(snapshot.observed_at, 1);
        assert_eq!(snapshot.first_provider_attempt_at, Some(10));
        assert_eq!(snapshot.usage.gaps, 0);
    }

    #[test]
    fn summary_reads_usage_without_hydrating_conversation_text() {
        let home = tempfile::tempdir().unwrap();
        let store = SqliteStore::open_ephemeral(&home.path().join("store.db")).unwrap();
        store.test_session("conversation", "run_00000000000000000000000000000001");
        let input =
            crate::session_record::parse_artifact_key("run_00000000000000000000000000000001")
                .unwrap();
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
                        artifact_key: input.clone(),
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
        assert_eq!(store.session_history(&session.id, 0, 0).unwrap().len(), 4);
    }

    #[test]
    fn event_times_follow_input_order_and_skip_untimed_events() {
        let home = tempfile::tempdir().unwrap();
        let store = SqliteStore::open_ephemeral(&home.path().join("store.db")).unwrap();
        store.test_session("conversation", "run_00000000000000000000000000000001");
        let input =
            crate::session_record::parse_artifact_key("run_00000000000000000000000000000001")
                .unwrap();
        let session = store.session("conversation").unwrap().unwrap();
        // Retained out of order: the later event is imported first.
        for (seq, evidence) in [
            (
                2,
                json!({"observed_at":"2026-10-01T00:20:00.5Z","type":"usage"}),
            ),
            (
                0,
                json!({"observed_at":"2026-10-01T00:00:00Z","type":"user_input"}),
            ),
            (1, json!({"unparsed":"partial historical event"})),
        ] {
            let source = format!("events.jsonl:{seq}");
            store
                .retain_session_observation(
                    &session,
                    &crate::session::SessionObservation {
                        artifact_key: input.clone(),
                        source: source.clone(),
                        observed_at: 1,
                        task_id: None,
                        wave_id: None,
                        payload: json!({"input_id":input,"source":source,"evidence":evidence}),
                    },
                )
                .unwrap();
        }
        assert_eq!(
            store.input_event_times(&input).unwrap(),
            [1_790_812_800, 1_790_814_000]
        );
    }

    #[test]
    fn history_limits_precede_payload_reads_and_keep_unfinished_work_and_exact_callers() {
        let home = tempfile::tempdir().unwrap();
        let store = SqliteStore::open_ephemeral(&home.path().join("store.db")).unwrap();
        let mut template = store.test_session("seed", "run_000000000000000000000000000000ff");
        template.captured = None;
        template.caller_artifact_key = Some(template.artifact_key.clone());
        for at in 1..=60 {
            let mut session = template.clone();
            session.id = format!("conversation-{at}");
            session.artifact_key = format!("run_{at:032x}");
            session.created_at = at;
            let session = store.create_session(session, None).unwrap();
            store
                .record_session_event(
                    &session.id,
                    "thread",
                    &at.to_string(),
                    SessionEventKind::Started,
                    &json!({}),
                )
                .unwrap();
            if at != 1 {
                store
                    .record_session_event(
                        &session.id,
                        "thread",
                        &at.to_string(),
                        SessionEventKind::Completed,
                        &json!({"status":"completed"}),
                    )
                    .unwrap();
            }
        }
        let (recent, truncated) = store
            .recent_conversation_history(None, None, None, 0, 5)
            .unwrap();
        assert!(truncated);
        assert_eq!(recent.len(), 5);
        assert_eq!(
            recent.iter().map(|row| row.observed_at).collect::<Vec<_>>(),
            [60, 59, 58, 1, 1]
        );
        let exact = store
            .conversation_history(
                None,
                None,
                None,
                Some(template.artifact_key.as_str()),
                0,
                false,
            )
            .unwrap();
        assert_eq!(
            exact.len(),
            60,
            "an exact caller drill has no presentation budget"
        );
        let boundary = store
            .conversation_history(None, None, None, None, 59, false)
            .unwrap();
        assert_eq!(boundary.len(), 2);
        assert_eq!(boundary[1].observed_at, 59);
        assert_eq!(
            store
                .conversation_history(None, None, None, None, 59, true)
                .unwrap()
                .len(),
            59,
            "completed native turns in the window retain older captures"
        );
        // Corrupt body evidence outside the selected budget cannot break a list;
        // exact detail still reports that real error instead of hiding it.
        store.conn.lock().unwrap().execute(
            "INSERT INTO session_events(session_id,kind,receipt_key,observed_at,payload,captured_event)
             SELECT session_id,'observed',receipt_key||':events.jsonl:1',10,'{',seq FROM session_events
             WHERE kind='captured' AND session_id='conversation-10'", []).unwrap();
        assert_eq!(
            store
                .recent_conversation_history(None, None, None, 0, 5)
                .unwrap()
                .0,
            recent
        );
        assert!(store
            .input_history("run_0000000000000000000000000000000a")
            .is_err());
        assert!(store
            .conversation_history(
                None,
                None,
                None,
                Some(template.artifact_key.as_str()),
                0,
                false
            )
            .is_err());
    }

    #[test]
    fn aggregate_history_discovers_unattributed_native_receipts_without_a_capture() {
        let home = tempfile::tempdir().unwrap();
        let store = SqliteStore::open_ephemeral(&home.path().join("store.db")).unwrap();
        store.test_session("conversation", "run_00000000000000000000000000000001");
        let counts = json!({"inputTokens":12,"outputTokens":3,
            "cachedInputTokens":0,"reasoningOutputTokens":0});
        for (turn, at) in [("old", 10), ("recent", 100)] {
            store
                .record_session_event(
                    "conversation",
                    "thread",
                    turn,
                    SessionEventKind::Usage,
                    &json!({"total":counts,"last":counts}),
                )
                .unwrap();
            store
                .record_session_event(
                    "conversation",
                    "thread",
                    turn,
                    SessionEventKind::Completed,
                    &json!({"status":"completed"}),
                )
                .unwrap();
            // Simulated observation times exercise the public reader's window.
            store
                .conn
                .lock()
                .unwrap()
                .execute(
                    "UPDATE session_events SET observed_at=?1 WHERE provider_turn=?2",
                    (at, turn),
                )
                .unwrap();
        }
        let rows = store
            .conversation_history(None, None, None, None, 50, true)
            .unwrap();
        assert_eq!(
            rows.len(),
            1,
            "an older unlinked turn cannot hide or join the recent one"
        );
        let history = &rows[0];
        assert!(history.captured.is_none());
        assert!(history.artifact_key.is_none());
        assert!(history.task_id.is_none());
        assert!(history.wave_id.is_none());
        assert!(history.recorded_outcome.is_none());
        assert_eq!(history.observed_at, 100);
        assert_eq!(history.providers.len(), 1);
        let provider = &history.providers[0];
        assert!(
            matches!(&provider.reference, crate::session_record::ProviderHistoryReference::NativeTurn {
            turn, start_seq: None, completion_seq: Some(_), .. } if turn == "recent")
        );
        assert!(provider.exec_id.is_none());
        assert!(provider.task_id.is_none());
        assert!(provider.started_at.is_none());
        assert_eq!(provider.outcome.as_deref(), Some("completed"));
        assert_eq!(history.usage.input_tokens, Some(12));
        assert_eq!(history.usage.streams, 1);
        assert_eq!(
            history.usage.final_streams, 0,
            "a missing start keeps usage coverage partial"
        );
        assert!(store
            .conversation_history(None, None, Some("missing-task"), None, 0, true)
            .unwrap()
            .is_empty());
        let (recent, truncated) = store
            .recent_conversation_history(None, None, None, 50, 1)
            .unwrap();
        assert_eq!(recent, vec![history.clone()]);
        assert!(!truncated);
        assert_eq!(
            store
                .conversation_history(None, None, None, None, 0, true)
                .unwrap()
                .len(),
            3,
            "both unlinked turns and the reserved capture retain independent evidence"
        );
        let wave = crate::id::WaveId::new();
        store
            .create_wave(&crate::work::wave::Wave::new(
                wave.clone(),
                "proof".into(),
                "/repo".into(),
            ))
            .unwrap();
        // Imported native history can establish Work without a capture. That
        // turn cannot lend its attribution to another unlinked receipt.
        store.conn.lock().unwrap().execute(
            "INSERT INTO session_events(session_id,provider_thread,provider_turn,kind,receipt_key,wave_id,observed_at,payload)
             VALUES('conversation','thread','owned','started','',?1,110,'{}')", [wave.as_str()],
        ).unwrap();
        let scoped = store
            .conversation_history(Some(wave.as_str()), None, None, None, 0, true)
            .unwrap();
        assert_eq!(scoped.len(), 1);
        assert_eq!(scoped[0].providers.len(), 1);
        assert_eq!(scoped[0].wave_id.as_ref(), Some(&wave));
    }

    #[test]
    #[cfg(unix)]
    fn session_exit_closes_engine_unless_ownership_transferred() {
        use std::os::unix::process::CommandExt;
        use std::process::Command;

        for transfer in [false, true] {
            let home = tempfile::tempdir().unwrap();
            let store = SqliteStore::open_ephemeral(&home.path().join("store.db")).unwrap();
            let session =
                store.test_session("conversation", &crate::session_record::new_artifact_key());
            let first = crate::id::ExecId::new();
            let second = crate::id::ExecId::new();
            {
                let conn = store.conn.lock().unwrap();
                for exec in [&first, &second] {
                    conn.execute(
                        "INSERT INTO execs(id,trace_id,started_at) VALUES(?1,'fixture',1)",
                        [exec.as_str()],
                    )
                    .unwrap();
                }
                conn.execute(
                    "UPDATE agent_sessions SET provider='codex' WHERE id=?1",
                    [&session.id],
                )
                .unwrap();
            }
            let original = store
                .claim_session_driver(&session.id, None, &first, true)
                .unwrap();
            let mut child = Command::new("sleep")
                .arg("30")
                .process_group(0)
                .spawn()
                .unwrap();
            let started = crate::journal::process_started_at(child.id())
                .unwrap()
                .unwrap();
            store
                .record_session_provider_process(&session.id, &original, child.id(), started)
                .unwrap();
            // An engine whose socket has gone must still be reaped on exit.
            store
                .record_session_connection(
                    &session.id,
                    &original,
                    home.path().join("engine.sock").to_str().unwrap(),
                    "saved-thread",
                )
                .unwrap();
            let current = if transfer {
                let replacement = store
                    .claim_session_driver(&session.id, Some(&original), &second, false)
                    .unwrap();
                assert!(matches!(
                    crate::session_record::finish_session_driver(
                        &store,
                        &session.id,
                        &original,
                        "completed"
                    ),
                    Err(crate::store::StoreError::InvalidAuthority(_))
                ));
                assert!(child.try_wait().unwrap().is_none());
                assert!(store.session_connection(&session.id).unwrap().is_some());
                replacement
            } else {
                original
            };
            crate::session_record::finish_session_driver(
                &store,
                &session.id,
                &current,
                "completed",
            )
            .unwrap();
            assert!(crate::journal::process_started_at(child.id())
                .unwrap()
                .is_none());
            // Shutdown may already have reaped this exact child.
            let _ = child.wait();
            assert!(store.session_connection(&session.id).unwrap().is_none());
            assert_eq!(
                store.session_thread(&session.id).unwrap().as_deref(),
                Some("saved-thread")
            );
            assert!(store
                .session_driver(&session.id)
                .unwrap()
                .unwrap()
                .exec_id
                .is_none());
        }
    }

    #[test]
    fn session_exit_retires_orphans_but_preserves_primary_and_review_obligations() {
        let home = tempfile::tempdir().unwrap();
        let store = SqliteStore::open_ephemeral(&home.path().join("store.db")).unwrap();
        let wave = crate::id::WaveId::new();
        let project = crate::work::project::ProjectId::new();
        let task = crate::work::task::TaskId::new();
        {
            let conn = store.conn.lock().unwrap();
            conn.execute(
                "INSERT INTO waves(id,name,repo,created_at) VALUES(?1,'proof','/repo',1)",
                [&wave],
            )
            .unwrap();
            conn.execute(
                "INSERT INTO projects(id,wave_id,external_project_id,created_at) VALUES(?1,?2,'project-proof',1)",
                rusqlite::params![project.as_str(), wave],
            ).unwrap();
            conn.execute(
                "INSERT INTO tasks(id,project_id,external_issue_id,issue_identifier,created_at) VALUES(?1,?2,'issue-proof','PROOF-1',1)",
                rusqlite::params![task.as_str(), project.as_str()],
            ).unwrap();
        }
        for (id, kind, primary, retired) in [
            ("orphan", "conversation", None, true),
            ("primary", "conversation", Some("repository"), false),
            ("task", "conversation", None, false),
            ("review", "flow_review", None, false),
        ] {
            let session = store.test_session(id, &crate::session_record::new_artifact_key());
            let exec = crate::id::ExecId::new();
            {
                let conn = store.conn.lock().unwrap();
                conn.execute(
                    "INSERT INTO execs(id,trace_id,started_at) VALUES(?1,'fixture',1)",
                    [exec.as_str()],
                )
                .unwrap();
                conn.execute(
                    "UPDATE agent_sessions SET kind=?2,primary_scope=?3 WHERE id=?1",
                    rusqlite::params![id, kind, primary],
                )
                .unwrap();
                if id == "task" {
                    conn.execute(
                        "UPDATE agent_sessions SET task_id=?2,wave_id=?3 WHERE id=?1",
                        rusqlite::params![id, task.as_str(), wave],
                    )
                    .unwrap();
                }
            }
            let driver = store.claim_session_driver(id, None, &exec, true).unwrap();
            store
                .finish_session_driver(id, &driver, "interrupted", || Ok(false))
                .unwrap();
            let saved = store.session(id).unwrap().unwrap();
            assert_eq!(saved.completed_at.is_some(), retired);
            assert_eq!(saved.captured, session.captured);
            let summary = store.session_summary(id).unwrap().unwrap();
            assert_eq!(summary.driver_outcome.as_deref(), Some("interrupted"));
            let history = store.session_history(id, 0, 100).unwrap();
            assert!(history
                .iter()
                .any(|event| event.payload["type"] == "driver_exit"
                    && event.payload["outcome"] == "interrupted"));
            assert!(!history
                .iter()
                .any(|event| event.kind == SessionEventKind::Completed));
        }
        assert!(!store
            .session_summaries(&crate::session::SessionFilter::default())
            .unwrap()
            .iter()
            .any(|session| session.id == "orphan"));
    }

    #[test]
    fn completed_turn_does_not_complete_the_conversation() {
        let home = tempfile::tempdir().unwrap();
        let store = SqliteStore::open_ephemeral(&home.path().join("store.db")).unwrap();
        let session = store.test_session("unfinished", &crate::session_record::new_artifact_key());
        for kind in [SessionEventKind::Started, SessionEventKind::Completed] {
            store
                .record_session_event(
                    &session.id,
                    "thread",
                    "turn",
                    kind,
                    &json!({"status": "completed"}),
                )
                .unwrap();
        }
        for _ in 0..2 {
            let rows = store
                .session_summaries(&crate::session::SessionFilter::default())
                .unwrap();
            assert_eq!(rows.len(), 1);
            assert_eq!(rows[0].id, session.id);
            assert!(rows[0].completed_at.is_none());
            assert_eq!(rows[0].latest_turn.as_deref(), Some("completed"));
        }
    }

    #[test]
    fn stopped_turn_and_stale_driver_cannot_retire_a_resumed_conversation() {
        let home = tempfile::tempdir().unwrap();
        let store = SqliteStore::open_ephemeral(&home.path().join("store.db")).unwrap();
        let session =
            store.test_session("conversation", &crate::session_record::new_artifact_key());
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
            .claim_session_driver(&session.id, None, &first, true)
            .unwrap();
        store
            .record_session_event(
                &session.id,
                "thread",
                "turn",
                SessionEventKind::Started,
                &json!({}),
            )
            .unwrap();
        store
            .record_session_event(
                &session.id,
                "thread",
                "turn",
                SessionEventKind::Completed,
                &json!({"status":"interrupted"}),
            )
            .unwrap();
        assert!(store
            .session(&session.id)
            .unwrap()
            .unwrap()
            .completed_at
            .is_none());
        assert_eq!(
            store
                .session_summary(&session.id)
                .unwrap()
                .unwrap()
                .latest_turn
                .as_deref(),
            Some("interrupted")
        );
        let resumed = store
            .claim_session_driver(&session.id, Some(&original), &second, true)
            .unwrap();
        assert!(store
            .finish_session_driver(&session.id, &original, "interrupted", || Ok(false))
            .is_err());
        assert!(store
            .session(&session.id)
            .unwrap()
            .unwrap()
            .completed_at
            .is_none());
        assert_eq!(store.session_driver(&session.id).unwrap().unwrap(), resumed);
        store
            .finish_session_driver(&session.id, &resumed, "completed", || Ok(false))
            .unwrap();
        assert!(store
            .session(&session.id)
            .unwrap()
            .unwrap()
            .completed_at
            .is_some());
    }

    #[test]
    fn native_usage_survives_driver_and_input_replacement_without_double_counting() {
        let home = tempfile::tempdir().unwrap();
        let store = SqliteStore::open_ephemeral(&home.path().join("store.db")).unwrap();
        store.test_session("conversation", "run_00000000000000000000000000000001");
        let first_input =
            crate::session_record::parse_artifact_key("run_00000000000000000000000000000001")
                .unwrap();
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
        replacement.artifact_key = crate::session_record::new_artifact_key();
        store
            .replace_session_input(session.captured, replacement.clone())
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
            .summary_for_input(&session.id, &replacement.artifact_key)
            .unwrap()
            .is_empty());
        let old = store.input_history(&first_input).unwrap();
        assert_eq!(old.usage.input_tokens, Some(18));
        assert_eq!(old.usage.output_tokens, Some(5));
        assert_eq!(old.usage.cost_usd, None);
        assert_eq!(old.usage.final_streams, 0);
        assert_eq!(
            old.recorded_outcome, None,
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
                artifact_key:first_input.clone(), source:source.clone(), observed_at:1, task_id:None,wave_id:None,
                payload:json!({"input_id":first_input,"source":source,"evidence":evidence})
            }).unwrap();
        }
        let old = store.input_history(&first_input).unwrap();
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
            .summary_for_input(&session.id, &replacement.artifact_key)
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
        let partial = store
            .input_history(replacement.artifact_key.as_str())
            .unwrap();
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
        let decreased = store
            .input_history(replacement.artifact_key.as_str())
            .unwrap();
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
                .input_history(replacement.artifact_key.as_str())
                .unwrap()
                .usage
                .total_input_tokens,
            Some(70)
        );
    }

    #[test]
    fn native_usage_cannot_skip_a_turn_without_a_notification_for_its_baseline() {
        let home = tempfile::tempdir().unwrap();
        let store = SqliteStore::open_ephemeral(&home.path().join("store.db")).unwrap();
        let input = crate::session_record::new_artifact_key();
        store.test_session("conversation", &input);
        let counts = |value| {
            json!({"inputTokens":value,"outputTokens":0,
            "cachedInputTokens":0,"reasoningOutputTokens":0,"cacheWriteInputTokens":0})
        };
        for (turn, total, last) in [("a", Some(20), 20), ("b", None, 30), ("c", Some(60), 10)] {
            store
                .record_session_event(
                    "conversation",
                    "thread",
                    turn,
                    SessionEventKind::Started,
                    &json!({}),
                )
                .unwrap();
            if let Some(total) = total {
                store
                    .record_session_event(
                        "conversation",
                        "thread",
                        turn,
                        SessionEventKind::Usage,
                        &json!({"total":counts(total),"last":counts(last)}),
                    )
                    .unwrap();
            }
            store
                .record_session_event(
                    "conversation",
                    "thread",
                    turn,
                    SessionEventKind::Completed,
                    &json!({"status":"completed"}),
                )
                .unwrap();
        }
        let session = store.session("conversation").unwrap().unwrap();
        for (seq, event) in [
            json!({"type":"provider_session_observed","attempt_key":"attempt","provider_session_id":"thread"}),
            json!({"type":"usage","provider":"codex","model":null,"attempt_key":"attempt","turn_key":"b",
                "usage_stream_id":"recorder-b","observation_seq":1,"counter_kind":"cumulative","start_known":true,
                "final_receipt":true,"usage":{"input_tokens":30,"output_tokens":0,"total_input_tokens":30}})
        ].into_iter().enumerate() {
            let mut evidence = json!({"schema_version":1,"seq":seq,"observed_at":"2026-09-29T00:00:00Z"});
            evidence.as_object_mut().unwrap().extend(event.as_object().unwrap().clone());
            let source = format!("events.jsonl:{seq}");
            store.retain_session_observation(&session, &crate::session::SessionObservation {
                artifact_key:input.clone(), source:source.clone(), observed_at:1, task_id:None,wave_id:None,
                payload:json!({"input_id":input,"source":source,"evidence":evidence})
            }).unwrap();
        }
        let usage = store.input_history(&input).unwrap().usage;
        assert_eq!(usage.total_input_tokens, Some(60));
        assert_eq!(usage.streams, 3);
        assert_eq!(usage.final_streams, 1);
        assert!(
            usage.gaps > 0,
            "C has only its observed suffix, not a complete baseline"
        );
    }

    #[test]
    fn recovered_completion_keeps_missing_start_and_usage_and_rejects_conflicts() {
        let home = tempfile::tempdir().unwrap();
        let store = SqliteStore::open_ephemeral(&home.path().join("store.db")).unwrap();
        store.test_session("conversation", "run_00000000000000000000000000000001");
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
        let history: Vec<_> = store
            .session_history("conversation", 0, 100)
            .unwrap()
            .into_iter()
            .filter(|event| event.kind != SessionEventKind::Captured)
            .collect();
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
