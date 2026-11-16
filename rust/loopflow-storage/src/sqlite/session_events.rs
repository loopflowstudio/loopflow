use rusqlite::{params, OptionalExtension, TransactionBehavior};
use serde_json::Value;

use crate::session::{SessionEvent, SessionEventKind};
use crate::session_record::ProviderSessionRef;
use crate::{ProviderAccountId, StoreError, StoreResult};
use loopflow_model::id::AgentSessionId;

use super::SqliteStore;

/// A Session's current provider conversation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CurrentConversation {
    pub provider: String,
    pub thread: AgentSessionId,
}

impl SqliteStore {
    pub fn input_provider_session(&self, input: &str) -> StoreResult<Option<ProviderSessionRef>> {
        // Only the input's own observations, keyed `<input>:<source>`, name its
        // provider session; a Session switching conversation reads these at a
        // turn boundary, so read that key range rather than its whole history.
        let conn = self.conn.lock().expect("store mutex poisoned");
        let payloads = conn
            .prepare(
                "SELECT payload FROM session_events INDEXED BY session_observation_receipt
                 WHERE session_id=(SELECT session_id FROM session_events
                     WHERE kind='captured' AND receipt_key=?1)
                 AND kind='observed' AND receipt_key>?1||':' AND receipt_key<?1||';'
                 ORDER BY seq",
            )?
            .query_map([input], |row| row.get::<_, String>(0))?
            .map(|payload| Ok(serde_json::from_str(&payload?)?))
            .collect::<StoreResult<Vec<Value>>>()?;
        drop(conn);
        crate::session_record::provider_session_from_history(payloads)
            .map_err(|error| StoreError::InvalidData(error.to_string()))
    }

    /// What the store kept of each provider conversation `session` used, in
    /// the order it first used them: each turn's start, settlement and final
    /// answer from native receipts, and the account it ran on.
    pub fn session_turn_facts(
        &self,
        session: &str,
    ) -> StoreResult<Vec<crate::transcript::ThreadFacts>> {
        use crate::transcript::{ThreadFacts, TurnFacts};
        let mut connection = self.conn.lock().expect("store mutex poisoned");
        let conn = connection.savepoint()?;
        let mut threads: Vec<ThreadFacts> = Vec::new();
        {
            let mut query = conn.prepare(
                "SELECT e.provider_thread,e.provider_turn,e.kind,e.observed_at,e.payload,
                        json_extract(c.payload,'$.provider')
                 FROM session_events e
                 LEFT JOIN session_events c ON c.seq=e.captured_event AND c.kind='captured'
                 WHERE e.session_id=?1 AND e.kind IN ('started','completed','output')
                   AND e.provider_thread IS NOT NULL
                 ORDER BY e.seq",
            )?;
            let mut rows = query.query([session])?;
            while let Some(row) = rows.next()? {
                let thread: String = row.get(0)?;
                let turn: Option<String> = row.get(1)?;
                let kind: String = row.get(2)?;
                let at: i64 = row.get(3)?;
                let payload: Value = serde_json::from_str(&row.get::<_, String>(4)?)?;
                let harness: Option<String> = row.get(5)?;
                let index = match threads.iter().position(|t| t.thread.as_str() == thread) {
                    Some(index) => index,
                    None => {
                        threads.push(ThreadFacts {
                            thread: thread.as_str().into(),
                            harness: None,
                            account: None,
                            turns: Vec::new(),
                        });
                        threads.len() - 1
                    }
                };
                let facts = &mut threads[index];
                if facts.harness.is_none() {
                    facts.harness = harness;
                }
                let turn = turn.unwrap_or_default();
                let position = match facts.turns.iter().position(|t| t.turn == turn) {
                    Some(position) => position,
                    None => {
                        facts.turns.push(TurnFacts {
                            turn,
                            started_at: None,
                            settled_at: None,
                            status: None,
                            final_answer: None,
                        });
                        facts.turns.len() - 1
                    }
                };
                let entry = &mut facts.turns[position];
                match kind.as_str() {
                    "started" => entry.started_at = Some(at),
                    "completed" => {
                        entry.settled_at = Some(at);
                        entry.status = payload["status"].as_str().map(str::to_owned);
                    }
                    _ => {
                        entry.final_answer = match payload.get("value") {
                            Some(value) => Some(serde_json::to_string(value)?),
                            None => payload["text"].as_str().map(str::to_owned),
                        }
                    }
                }
            }
        }
        let (current, provider): (Option<String>, Option<String>) = conn
            .query_row(
                "SELECT provider_thread,provider FROM agent_sessions WHERE id=?1",
                [session],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .optional()?
            .unwrap_or_default();
        if let Some(current) = current {
            if !threads.iter().any(|t| t.thread.as_str() == current) {
                threads.push(ThreadFacts {
                    thread: current.as_str().into(),
                    harness: provider.clone(),
                    account: None,
                    turns: Vec::new(),
                });
            }
        }
        let mut account = conn.prepare(
            "SELECT account_id FROM provider_session_accounts
             WHERE provider=?1 AND provider_session_id=?2",
        )?;
        for facts in &mut threads {
            if facts.harness.is_none() {
                facts.harness = provider.clone();
            }
            facts.account = account
                .query_row(params![facts.harness, facts.thread.as_str()], |row| {
                    row.get::<_, String>(0)
                })
                .optional()?
                .and_then(|account| ProviderAccountId::parse(&account).ok());
        }
        Ok(threads)
    }

    /// `session`'s current provider conversation; `None` when it has none.
    pub fn current_conversation(&self, session: &str) -> StoreResult<Option<CurrentConversation>> {
        let conn = self.conn.lock().expect("store mutex poisoned");
        Ok(conn
            .query_row(
                "SELECT provider,provider_thread FROM agent_sessions
                 WHERE id=?1 AND provider IS NOT NULL AND provider_thread IS NOT NULL",
                [session],
                |row| {
                    Ok(CurrentConversation {
                        provider: row.get(0)?,
                        thread: row.get(1)?,
                    })
                },
            )
            .optional()?)
    }

    /// When turn `turn` of `thread` was recorded complete, and its status.
    pub fn recorded_turn_end(
        &self,
        session: &str,
        thread: &AgentSessionId,
        turn: &str,
    ) -> StoreResult<Option<(i64, Option<String>)>> {
        let conn = self.conn.lock().expect("store mutex poisoned");
        let end = conn
            .query_row(
                "SELECT observed_at,payload FROM session_events
                 WHERE session_id=?1 AND provider_thread=?2 AND provider_turn=?3
                   AND kind='completed'
                 ORDER BY seq DESC LIMIT 1",
                params![session, thread, turn],
                |row| Ok((row.get::<_, i64>(0)?, row.get::<_, String>(1)?)),
            )
            .optional()?;
        end.map(|(at, payload)| {
            let payload: Value = serde_json::from_str(&payload)?;
            Ok((at, payload["status"].as_str().map(str::to_owned)))
        })
        .transpose()
    }

    /// The newest Session whose current provider conversation each of
    /// `threads` is.
    pub fn sessions_of_provider_threads(
        &self,
        threads: &[String],
    ) -> StoreResult<std::collections::HashMap<String, String>> {
        let conn = self.conn.lock().expect("store mutex poisoned");
        let mut query = conn.prepare_cached(
            "SELECT id FROM agent_sessions WHERE provider_thread=?1
             ORDER BY created_at DESC LIMIT 1",
        )?;
        let mut sessions = std::collections::HashMap::new();
        for thread in threads {
            if let Some(session) = query.query_row([thread], |row| row.get(0)).optional()? {
                sessions.insert(thread.clone(), session);
            }
        }
        Ok(sessions)
    }

    /// Every Session with its current provider conversation and the account
    /// it ran on, for the retention summary.
    pub fn retention_sessions(&self) -> StoreResult<Vec<crate::transcript::RetainedSession>> {
        let conn = self.conn.lock().expect("store mutex poisoned");
        let mut query = conn.prepare(
            "SELECT s.id,s.title,s.provider,s.provider_thread,s.cwd,
                    MAX(s.created_at,COALESCE(s.completed_at,0)),
                    (SELECT account_id FROM provider_session_accounts a
                     WHERE a.provider=s.provider AND a.provider_session_id=s.provider_thread)
             FROM agent_sessions s WHERE s.provider IS NOT NULL ORDER BY s.created_at,s.id",
        )?;
        let rows = query.query_map([], |row| {
            Ok(crate::transcript::RetainedSession {
                id: row.get(0)?,
                title: row.get(1)?,
                harness: row.get(2)?,
                thread: row
                    .get::<_, Option<String>>(3)?
                    .map(|thread| thread.as_str().into()),
                cwd: row.get::<_, String>(4)?.into(),
                last_at: row.get(5)?,
                account: row
                    .get::<_, Option<String>>(6)?
                    .and_then(|account| ProviderAccountId::parse(&account).ok()),
            })
        })?;
        Ok(rows.collect::<Result<_, _>>()?)
    }

    /// Original input order survives importing earlier observations after later ones.
    pub fn input_events(&self, input: &str) -> StoreResult<Vec<Value>> {
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

    /// Each silence the recorder observed in one input, as `(since, until)`.
    pub fn input_silences(&self, input: &str) -> StoreResult<Vec<(i64, i64)>> {
        let conn = self.conn.lock().expect("store mutex poisoned");
        let mut query = conn.prepare(
            "SELECT json_extract(e.payload,'$.evidence.since'),json_extract(e.payload,'$.evidence.observed_at')
             FROM session_events e
             JOIN session_events c ON c.seq=e.captured_event AND c.kind='captured' AND c.receipt_key=?1
             WHERE e.kind='observed' AND substr(e.receipt_key,1,length(?1)+9)=?1||':silence:'
             ORDER BY CAST(substr(e.receipt_key,length(?1)+10) AS INTEGER),e.seq",
        )?;
        let rows = query.query_map([input], |row| Ok((row.get(0)?, row.get(1)?)))?;
        Ok(rows.collect::<Result<_, _>>()?)
    }

    /// What filled the context of each provider turn one input started, in
    /// order; `None` for a turn completed before turns retained it.
    pub fn input_turn_context(
        &self,
        input: &str,
    ) -> StoreResult<Vec<Option<crate::session::TurnContext>>> {
        let conn = self.conn.lock().expect("store mutex poisoned");
        let mut query = conn.prepare(
            "SELECT json_extract(done.payload,'$.context') FROM session_events start
             JOIN session_events done ON done.session_id=start.session_id
               AND done.provider_thread=start.provider_thread AND done.provider_turn=start.provider_turn
               AND done.kind='completed'
             WHERE start.kind='started' AND start.captured_event=(SELECT seq FROM session_events WHERE kind='captured' AND receipt_key=?1)
             ORDER BY done.seq",
        )?;
        let rows = query.query_map([input], |row| row.get::<_, Option<String>>(0))?;
        rows.map(|row| {
            row?.map(|context| serde_json::from_str(&context))
                .transpose()
                .map_err(Into::into)
        })
        .collect()
    }

    pub fn input_final_answer(&self, input: &str) -> StoreResult<Option<String>> {
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
            return Ok(Some(text));
        }
        Ok(None)
    }

    /// One `store-gap` observation for boundaries missed from `first_miss` to
    /// `last_miss`.
    pub fn record_recording_gap(
        &self,
        session: &str,
        first_miss: i64,
        last_miss: i64,
    ) -> StoreResult<()> {
        let session = self.session(session)?.ok_or(StoreError::NotFound)?;
        let input = session
            .input_key()
            .map_err(|error| StoreError::InvalidData(error.to_string()))?
            .clone();
        self.retain_session_observation(
            &session,
            &crate::session::SessionObservation {
                artifact_key: input,
                source: format!("store-gap:{first_miss}:{last_miss}"),
                observed_at: first_miss,
                task_id: session.task_id.clone(),
                wave_id: session.wave_id.clone(),
                payload: serde_json::json!({"source":"store-gap",
                    "first_miss":first_miss,"last_miss":last_miss}),
            },
        )
    }

    pub fn retain_session_observation(
        &self,
        session: &crate::session::LfSession,
        observation: &crate::session::SessionObservation,
    ) -> StoreResult<()> {
        let mut conn = self.conn.lock().expect("store mutex poisoned");
        let tx = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
        super::sessions::retain_history_in(&tx, session, std::slice::from_ref(observation))?;
        tx.commit()?;
        Ok(())
    }

    /// Keep one fact of `session`'s captured `input` at stream position `seq`.
    pub fn record_capture_fact(
        &self,
        session: &crate::session::LfSession,
        input: &str,
        seq: u64,
        at: time::OffsetDateTime,
        fact: &crate::session_record::CaptureFact,
    ) -> StoreResult<()> {
        let (source, evidence) = fact.observation(seq, at);
        self.retain_session_observation(
            session,
            &crate::session::SessionObservation {
                artifact_key: input.to_owned(),
                payload: serde_json::json!({"input_id": input, "source": source, "evidence": evidence}),
                source,
                observed_at: at.unix_timestamp(),
                task_id: session.task_id.clone(),
                wave_id: session.wave_id.clone(),
            },
        )
    }

    /// Replace the Session's reading with its current attachment's.
    pub fn record_session_activity(
        &self,
        session: &str,
        attachment: &crate::process::SessionAttachment,
        activity: &crate::session::SessionActivity,
    ) -> StoreResult<()> {
        let conn = self.conn.lock().expect("store mutex poisoned");
        conn.execute(
            "INSERT INTO session_activity(session_id,attachment_token,observed_at,open_tools,pending_input,yielded,agent_process_id,preview)
             SELECT ?1,?2,?3,?4,?5,?6,?7,?8 FROM agent_sessions s JOIN processes p ON p.id=s.agent_process_id WHERE s.id=?1 AND p.attachment_token=?2 AND p.id=?7
             ON CONFLICT(session_id) DO UPDATE SET attachment_token=excluded.attachment_token,
                observed_at=excluded.observed_at,open_tools=excluded.open_tools,
                pending_input=excluded.pending_input,yielded=excluded.yielded,
                program_status=CASE WHEN session_activity.agent_process_id IS excluded.agent_process_id THEN session_activity.program_status END,
                agent_process_id=excluded.agent_process_id,preview=excluded.preview",
            params![session, attachment.token, activity.observed_at,
                activity.open_tools as i64, activity.pending_input as i64, activity.yielded, attachment.agent_process_id, activity.preview.as_ref().map(serde_json::to_string).transpose()?],
        )?;
        Ok(())
    }

    /// The launching process's latest CPU sample of its provider body. Only
    /// the attachment that owns the reading may update it, and a Session that
    /// has not spoken yet has no row to update.
    pub fn record_session_cpu(
        &self,
        session: &str,
        attachment: &crate::process::SessionAttachment,
        sampled_at: i64,
        quiet_since: Option<i64>,
    ) -> StoreResult<()> {
        let conn = self.conn.lock().expect("store mutex poisoned");
        conn.execute(
            "UPDATE session_activity SET cpu_sampled_at=?3,cpu_quiet_since=?4
             WHERE session_id=?1 AND attachment_token=?2",
            params![session, attachment.token, sampled_at, quiet_since],
        )?;
        Ok(())
    }

    /// The current attachment's last provider signal and CPU sample.
    pub fn session_liveness(
        &self,
        session: &str,
    ) -> StoreResult<Option<crate::session::SessionLiveness>> {
        let conn = self.conn.lock().expect("store mutex poisoned");
        Ok(conn
            .query_row(
                "SELECT act.observed_at,act.cpu_sampled_at,act.cpu_quiet_since FROM session_activity act
                 JOIN agent_sessions s ON s.id=act.session_id
                 JOIN processes p ON p.id=s.agent_process_id
                 WHERE act.session_id=?1 AND act.attachment_token=p.attachment_token
                 AND act.agent_process_id=p.id",
                [session],
                |row| {
                    Ok(crate::session::SessionLiveness {
                        observed_at: row.get(0)?,
                        cpu_sampled_at: row.get(1)?,
                        cpu_quiet_since: row.get(2)?,
                    })
                },
            )
            .optional()?)
    }

    /// Retain a provider observation even when its attachment has
    /// changed. Observation grants neither native write nor Graph authority.
    pub fn record_session_event(
        &self,
        session: &str,
        thread: &AgentSessionId,
        turn: &str,
        kind: SessionEventKind,
        payload: &Value,
    ) -> StoreResult<i64> {
        let mut conn = self.conn.lock().expect("store mutex poisoned");
        let payload = serde_json::to_string(payload)?;
        // Providers replay receipts they have sent before; a retained one
        // answers without the write lock.
        if let Some(seq) = retained_event_in(&conn, session, thread, turn, kind, &payload)? {
            return Ok(seq);
        }
        let tx = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
        let seq = record_event_in(&tx, session, thread, turn, kind, &payload)?;
        tx.commit()?;
        Ok(seq)
    }

    /// Whether the turn's completion is already retained; a provider snapshot
    /// replays completions of turns it has finished before.
    pub fn session_turn_completed(
        &self,
        session: &str,
        thread: &AgentSessionId,
        turn: &str,
    ) -> StoreResult<bool> {
        let conn = self.conn.lock().expect("store mutex poisoned");
        Ok(conn
            .query_row(
                "SELECT 1 FROM session_events WHERE session_id=?1 AND provider_thread=?2
                 AND provider_turn=?3 AND kind=?4",
                params![session, thread, turn, SessionEventKind::Completed.as_str()],
                |_| Ok(()),
            )
            .optional()?
            .is_some())
    }

    /// Each completion recorded in `thread`: its turn and when it was observed.
    pub fn session_completions(
        &self,
        session: &str,
        thread: &AgentSessionId,
    ) -> StoreResult<Vec<(String, i64)>> {
        let conn = self.conn.lock().expect("store mutex poisoned");
        let mut query = conn.prepare_cached(
            "SELECT provider_turn,observed_at FROM session_events
             WHERE session_id=?1 AND provider_thread=?2 AND kind='completed'
               AND provider_turn IS NOT NULL",
        )?;
        let rows = query.query_map(params![session, thread], |row| {
            Ok((row.get(0)?, row.get(1)?))
        })?;
        rows.map(|row| row.map_err(StoreError::from)).collect()
    }

    /// Record `turn`'s usage as its provider's transcript shows it, dated
    /// `at`. A turn keeps the usage row it has; returns whether one was written.
    pub fn record_turn_usage(
        &self,
        session: &str,
        thread: &AgentSessionId,
        turn: &str,
        at: i64,
        usage: &Value,
    ) -> StoreResult<bool> {
        let conn = self.conn.lock().expect("store mutex poisoned");
        let written = conn.execute(
            "INSERT INTO session_events(session_id,provider_thread,provider_turn,kind,receipt_key,observed_at,payload)
             SELECT ?1,?2,?3,'usage',?4,?5,?6 WHERE NOT EXISTS(
                SELECT 1 FROM session_events
                WHERE session_id=?1 AND provider_thread=?2 AND provider_turn=?3 AND kind='usage')",
            params![
                session,
                thread,
                turn,
                "",
                at,
                serde_json::to_string(usage)?
            ],
        )?;
        Ok(written > 0)
    }

    /// Record `turn`'s completion as its provider's transcript shows it,
    /// dated `at`, when the turn ended.
    pub fn record_turn_completion(
        &self,
        session: &str,
        thread: &AgentSessionId,
        turn: &str,
        at: i64,
        completed: &Value,
    ) -> StoreResult<()> {
        let conn = self.conn.lock().expect("store mutex poisoned");
        conn.execute(
            INSERT_NATIVE_EVENT,
            params![
                session,
                thread,
                turn,
                SessionEventKind::Completed.as_str(),
                "",
                at,
                serde_json::to_string(completed)?
            ],
        )?;
        Ok(())
    }

    /// Freeze the request's attribution while its attachment still owns dispatch.
    pub fn session_turn_origin(
        &self,
        session: &str,
        attachment: &crate::process::SessionAttachment,
    ) -> StoreResult<crate::session::SessionTurnOrigin> {
        let process = attachment.lf_process_id.as_ref().ok_or_else(|| {
            StoreError::InvalidAuthority("Native request has no attached Process".into())
        })?;
        let conn = self.conn.lock().expect("store mutex poisoned");
        conn.query_row(
            "SELECT s.task_id,s.wave_id,s.current_capture FROM agent_sessions s JOIN processes p ON p.id=s.agent_process_id
             WHERE s.id=?1 AND p.attachment_token=?2 AND p.attached_lf_process_id=?3
               AND p.id=?4",
            params![
                session,
                attachment.token,
                process,
                attachment.agent_process_id
            ],
            |row| {
                Ok(crate::session::SessionTurnOrigin {
                    session_id: session.into(),
                    lf_process_id: process.clone(),
                    agent_process_id: attachment.agent_process_id.clone(),
                    task_id: row.get(0)?,
                    wave_id: row.get(1)?,
                    captured_event: row.get(2)?,
                })
            },
        )
        .optional()?
        .ok_or_else(|| StoreError::InvalidAuthority("Session attachment changed".into()))
    }

    /// Retain correlated origin after takeover without reading current assignment.
    pub fn record_session_turn_origin(
        &self,
        thread: &AgentSessionId,
        turn: &str,
        origin: &crate::session::SessionTurnOrigin,
    ) -> StoreResult<i64> {
        let mut conn = self.conn.lock().expect("store mutex poisoned");
        let tx = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
        let seq = record_event_in(
            &tx,
            &origin.session_id,
            thread,
            turn,
            SessionEventKind::Started,
            "{}",
        )?;
        // An uncorrelated observation can acquire its origin once. Repeated
        // evidence must match every field, including an explicitly absent bind.
        let values = params![
            seq,
            origin.lf_process_id,
            origin.agent_process_id,
            origin.task_id,
            origin.wave_id,
            origin.captured_event
        ];
        let matches: Option<bool> = tx
            .query_row(
                "SELECT lf_process_id IS ?2 AND agent_process_id IS ?3
                AND task_id IS ?4 AND wave_id IS ?5 AND captured_event IS ?6
             FROM session_events WHERE seq=?1 AND lf_process_id IS NOT NULL",
                values,
                |row| row.get(0),
            )
            .optional()?;
        match matches {
            Some(true) => return Ok(seq),
            Some(false) => {
                return Err(StoreError::InvalidData(
                    "Native turn has different initiating evidence".into(),
                ))
            }
            None => {
                tx.execute(
                    "UPDATE session_events SET lf_process_id=?2,agent_process_id=?3,
                        task_id=?4,wave_id=?5,captured_event=?6 WHERE seq=?1",
                    values,
                )?;
            }
        }
        tx.commit()?;
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
        thread: Option<&AgentSessionId>,
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
        orphaned: Option<(Option<&AgentSessionId>, Option<&str>)>,
    ) -> StoreResult<Vec<SessionEvent>> {
        let task = scope.2.map(loopflow_model::id::TaskSelector::parse);
        let conn = self.conn.lock().expect("store mutex poisoned");
        let mut query = conn.prepare(
            "SELECT e.seq,e.session_id,e.provider_thread,e.provider_turn,e.kind,
                    CASE WHEN e.kind IN ('observed','captured') THEN e.agent_process_id ELSE origin.agent_process_id END,
                    CASE WHEN e.kind IN ('observed','captured') THEN e.lf_process_id ELSE origin.lf_process_id END,
                    CASE WHEN e.kind IN ('observed','captured') THEN e.task_id ELSE origin.task_id END,
                    CASE WHEN e.kind IN ('observed','captured') THEN e.wave_id ELSE origin.wave_id END,e.observed_at,e.payload
             FROM session_events e LEFT JOIN session_events origin
               ON origin.session_id=e.session_id AND origin.provider_thread=e.provider_thread
               AND origin.provider_turn=e.provider_turn AND origin.kind='started'
             WHERE e.session_id=?11 AND e.seq>?2
             AND (NOT ?8 OR (e.kind IN ('started','usage','completed','output') AND COALESCE(origin.captured_event,e.captured_event) IS NULL
                  AND e.provider_thread IS ?9 AND e.provider_turn IS ?10))
             AND (?4 IS NULL OR (e.kind='observed' AND substr(e.receipt_key,1,length(?4)+1)=?4||':')
                  OR (e.kind!='observed' AND COALESCE(origin.captured_event,e.captured_event)=(SELECT seq FROM session_events WHERE kind='captured' AND receipt_key=?4)))
             AND ((e.kind='observed' AND CASE WHEN json_valid(e.payload) THEN json_extract(e.payload,'$.source') END IN ('manifest.json','runs','terminal.json')) OR (
                 (?5 IS NULL OR (CASE WHEN e.kind IN ('observed','captured') THEN e.wave_id ELSE origin.wave_id END)
                     IN (SELECT id FROM wave_addresses WHERE id=?12 OR slug=?5))
                 AND (?6 IS NULL OR (CASE WHEN e.kind IN ('observed','captured') THEN e.task_id ELSE origin.task_id END)
                     IN (SELECT t.id FROM projects p CROSS JOIN tasks t ON t.project_id=p.id
                         WHERE p.id=?13 OR p.project_slug=?6 OR p.external_project_id=?14))
                 AND (?7 IS NULL OR (CASE WHEN e.kind IN ('observed','captured') THEN e.task_id ELSE origin.task_id END)
                     IN (SELECT id FROM tasks WHERE id=?15 OR issue_identifier=?16 OR external_issue_id=?17))))
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
                orphaned.and_then(|(_, turn)| turn),
                loopflow_model::id::canonical_session(session),
                scope.0.map(loopflow_model::id::canonical_uuid),
                scope
                    .1
                    .map(|project| loopflow_model::id::canonical_prefixed(project, "proj_")),
                scope.1.map(loopflow_model::id::canonical_uuid),
                task.as_ref().map(|task| &task.id),
                task.as_ref().map(|task| &task.label),
                task.as_ref().map(|task| &task.external),
            ],
            |row| {
                Ok((
                    row.get::<_, i64>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, Option<AgentSessionId>>(2)?,
                    row.get::<_, Option<String>>(3)?,
                    row.get::<_, String>(4)?,
                    row.get::<_, Option<String>>(5)?,
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
                agent_session,
                provider_turn,
                kind,
                agent_process_id,
                lf_process_id,
                task_id,
                wave_id,
                observed_at,
                payload,
            ) = row?;
            Ok(SessionEvent {
                seq,
                session_id: session_id.into(),
                agent_session,
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
                agent_process_id,
                lf_process_id,
                task_id,
                wave_id,
                observed_at,
                payload: serde_json::from_str(&payload)?,
            })
        })
        .collect()
    }
}

const INSERT_NATIVE_EVENT: &str = "INSERT INTO session_events(session_id,provider_thread,provider_turn,kind,receipt_key,observed_at,payload)
    VALUES(?1,?2,?3,?4,?5,?6,?7)";

/// Insert only the observation. Work and capture require correlated origin evidence.
/// The retained receipt's sequence, or an error when it says something else.
fn retained_event_in(
    conn: &rusqlite::Connection,
    session: &str,
    thread: &AgentSessionId,
    turn: &str,
    kind: SessionEventKind,
    payload: &str,
) -> StoreResult<Option<i64>> {
    let existing: Option<(i64, String)> = conn
        .query_row(
            "SELECT seq,payload FROM session_events
             WHERE session_id=?1 AND provider_thread=?2 AND provider_turn=?3 AND kind=?4 AND receipt_key=''",
            params![session, thread, turn, kind.as_str()],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .optional()?;
    match existing {
        None => Ok(None),
        Some((seq, saved)) if saved == payload => Ok(Some(seq)),
        Some(_) => Err(StoreError::InvalidData(format!(
            "Conflicting {} receipt for conversation {session}, turn {turn}",
            kind.as_str()
        ))),
    }
}

fn record_event_in(
    tx: &rusqlite::Transaction<'_>,
    session: &str,
    thread: &AgentSessionId,
    turn: &str,
    kind: SessionEventKind,
    payload: &str,
) -> StoreResult<i64> {
    if let Some(seq) = retained_event_in(tx, session, thread, turn, kind, payload)? {
        return Ok(seq);
    }
    tx.execute(
        INSERT_NATIVE_EVENT,
        params![
            session,
            thread,
            turn,
            kind.as_str(),
            "",
            time::OffsetDateTime::now_utc().unix_timestamp(),
            payload
        ],
    )?;
    Ok(tx.last_insert_rowid())
}

#[cfg(test)]
mod tests {

    use crate::session::SessionEventKind;
    use crate::sqlite::SqliteStore;
    use crate::RecordingGap;
    use serde_json::json;

    #[test]
    fn native_event_preparation_stays_bounded_and_keeps_identity_checks() {
        let home = tempfile::tempdir().unwrap();
        let store = crate::test_fixture::open(&home.path().join("store.db")).unwrap();
        store.test_session("conversation", &crate::session_record::new_artifact_key());
        {
            let conn = store.conn.lock().unwrap();
            // Count the real writer's compiled program, including all triggers.
            // The incident expanded this to over 41,000 instructions per receipt
            // while BEGIN IMMEDIATE held every other writer outside the store.
            let mut explain = conn
                .prepare(&format!("EXPLAIN {}", super::INSERT_NATIVE_EVENT))
                .unwrap();
            let instructions = explain
                .query_map(
                    rusqlite::params!["conversation", "thread", "turn", "started", "", 1, "{}"],
                    |_| Ok(()),
                )
                .unwrap()
                .collect::<Result<Vec<_>, _>>()
                .unwrap();
            assert!(
                instructions.len() < 2_000,
                "native INSERT compiled {} instructions",
                instructions.len()
            );
        }
        for turn in 0..100 {
            let turn = turn.to_string();
            let payload = json!({"text": "native output", "opaque": "ABCDEF"});
            let seq = store
                .record_session_event(
                    "conversation",
                    &"Provider-Thread".into(),
                    &turn,
                    SessionEventKind::Output,
                    &payload,
                )
                .unwrap();
            assert_eq!(
                seq,
                store
                    .record_session_event(
                        "conversation",
                        &"Provider-Thread".into(),
                        &turn,
                        SessionEventKind::Output,
                        &payload
                    )
                    .unwrap()
            );
        }
        let invalid =
            json!({"type": "legacy_flow", "flow_id": "ABCDEFAB-1234-5678-ABCD-ABCDEFABCDEF"});
        assert!(store
            .record_session_event(
                "conversation",
                &"thread".into(),
                "invalid",
                SessionEventKind::Output,
                &invalid
            )
            .unwrap_err()
            .to_string()
            .contains("canonical identity"));
        let valid =
            json!({"type": "legacy_flow", "flow_id": "abcdefab-1234-5678-abcd-abcdefabcdef"});
        store
            .record_session_event(
                "conversation",
                &"thread".into(),
                "valid",
                SessionEventKind::Output,
                &valid,
            )
            .unwrap();
        let events = store.session_history("conversation", 0, 0).unwrap();
        assert_eq!(
            events
                .iter()
                .filter(|event| event.kind == SessionEventKind::Output)
                .count(),
            101
        );
    }

    /// What a hook and the server do together: the hook sends its span, the
    /// server records it, and the hook clears its marker.
    fn close(store: &SqliteStore, gap: &mut RecordingGap) {
        if let Some(span) = gap.span() {
            store
                .record_recording_gap("session", span.first_miss, span.last_miss)
                .unwrap();
        }
        gap.recovered();
    }

    /// A boundary lf could not record enters the Session's history as unknown
    /// at the next one that succeeds (Jack Heart, 2026-11-05).
    #[test]
    fn a_recovered_gap_enters_the_sessions_history_once() {
        let home = tempfile::tempdir().unwrap();
        let store = crate::test_fixture::open(&home.path().join("store.db")).unwrap();
        store.test_session("session", &crate::session_record::new_artifact_key());
        let marker = home.path().join("thread.store-gap");
        let gap = || RecordingGap::marked(marker.clone());
        let observations = || -> i64 {
            store
                .conn
                .lock()
                .unwrap()
                .query_row(
                    "SELECT COUNT(*) FROM session_events WHERE session_id='session'
                     AND kind='observed' AND json_extract(payload,'$.source')='store-gap'",
                    [],
                    |row| row.get(0),
                )
                .unwrap()
        };
        // A boundary that records with no gap open writes nothing extra.
        close(&store, &mut gap());
        assert_eq!(observations(), 0);
        // Two hook processes miss the store; the next one that reaches it records
        // the hole once, and later boundaries have nothing left to say.
        gap().note("turn evidence", &"database is locked");
        gap().note("steers", &"database is locked");
        let mut open = gap();
        let span = open.span().unwrap().clone();
        close(&store, &mut open);
        assert!(open.span().is_none());
        assert_eq!(observations(), 1);
        let payload: String = store
            .conn
            .lock()
            .unwrap()
            .query_row(
                "SELECT payload FROM session_events WHERE kind='observed'
                 AND json_extract(payload,'$.source')='store-gap'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        let payload: serde_json::Value = serde_json::from_str(&payload).unwrap();
        assert_eq!(payload["first_miss"], span.first_miss);
        assert_eq!(payload["last_miss"], span.last_miss);
        close(&store, &mut gap());
        assert_eq!(observations(), 1);
    }

    #[test]
    fn provider_identity_uses_original_event_order_and_fresh_publications() {
        let home = tempfile::tempdir().unwrap();
        let store = crate::test_fixture::open(&home.path().join("store.db")).unwrap();
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
        assert_eq!(reference.agent_session, "second-thread".into());
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
        assert_eq!(reference.agent_session, "second-thread".into());
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
        assert_eq!(reference.agent_session, "fresh-thread".into());
        assert_eq!(reference.account_id, None);
    }

    #[test]
    fn silences_follow_input_order() {
        let home = tempfile::tempdir().unwrap();
        let store = crate::test_fixture::open(&home.path().join("store.db")).unwrap();
        store.test_session("conversation", "run_00000000000000000000000000000001");
        let input =
            crate::session_record::parse_artifact_key("run_00000000000000000000000000000001")
                .unwrap();
        let session = store.session("conversation").unwrap().unwrap();
        // Retained out of order: the later silence is imported first.
        for (seq, since, until) in [(12, 5_000, 6_000), (3, 100, 900)] {
            let source = format!("silence:{seq}");
            store
                .retain_session_observation(
                    &session,
                    &crate::session::SessionObservation {
                        artifact_key: input.clone(),
                        source: source.clone(),
                        observed_at: until,
                        task_id: None,
                        wave_id: None,
                        payload: json!({"input_id":input,"source":source,
                            "evidence":{"since":since,"observed_at":until}}),
                    },
                )
                .unwrap();
        }
        assert_eq!(
            store.input_silences(&input).unwrap(),
            [(100, 900), (5_000, 6_000)]
        );
    }

    #[test]
    fn native_summary_selects_the_recorded_input_and_keeps_unknown_origin_unassigned() {
        let home = tempfile::tempdir().unwrap();
        let store = crate::test_fixture::open(&home.path().join("store.db")).unwrap();
        let input = crate::session_record::new_artifact_key();
        let session = store.test_session("conversation", &input);
        let owner = loopflow_model::id::LfProcessId::new();
        store
            .conn
            .lock()
            .unwrap()
            .execute(
                "INSERT INTO processes(id,trace_id,started_at) VALUES(?1,'fixture',1)",
                [&owner],
            )
            .unwrap();
        let attachment = store
            .attach_session(
                &session.id,
                None,
                &owner,
                false,
                None,
                &crate::sqlite::SessionCaller {
                    agent: None,
                    capture_key: None,
                },
            )
            .unwrap();
        store
            .record_session_turn_origin(
                &"thread".into(),
                "turn",
                &store.session_turn_origin(&session.id, &attachment).unwrap(),
            )
            .unwrap();
        let mut replacement = session.clone();
        replacement.artifact_key = Some(crate::session_record::new_artifact_key());
        store
            .replace_session_input_prepared(session.captured, replacement.clone(), None, None)
            .unwrap();
        for turn in ["turn", "unknown"] {
            store
                .record_session_event(
                    &session.id,
                    &"thread".into(),
                    turn,
                    SessionEventKind::Usage,
                    &json!({"total":{"inputTokens":20,"outputTokens":5}}),
                )
                .unwrap();
        }
        let events = store
            .summary_for_input_matching(&session.id, &input, (None, None, None))
            .unwrap();
        assert_eq!(events.len(), 2);
        assert!(events.iter().all(
            |event| event.lf_process_id.as_deref() == Some(owner.as_str())
                && event.agent_process_id.as_deref() == Some(attachment.agent_process_id.as_str())
        ));
        assert!(store
            .summary_for_input_matching(
                &session.id,
                replacement.input_key().unwrap(),
                (None, None, None)
            )
            .unwrap()
            .is_empty());
        assert_eq!(
            store
                .session_history(&session.id, 0, 0)
                .unwrap()
                .last()
                .unwrap()
                .lf_process_id,
            None
        );
    }

    #[test]
    fn a_second_usage_receipt_for_a_turn_conflicts() {
        let home = tempfile::tempdir().unwrap();
        let store = crate::test_fixture::open(&home.path().join("store.db")).unwrap();
        store.test_session("conversation", &crate::session_record::new_artifact_key());
        let usage = |total: u64| {
            let counts = json!({"inputTokens":total,"outputTokens":0});
            store.record_session_event(
                "conversation",
                &"thread".into(),
                "a",
                SessionEventKind::Usage,
                &json!({"total":counts,"last":counts}),
            )
        };
        let first = usage(10).unwrap();
        assert_eq!(usage(10).unwrap(), first);
        assert!(usage(20).is_err());
    }

    #[test]
    fn deleting_a_session_event_probes_captured_event_by_index() {
        let home = tempfile::tempdir().unwrap();
        let store = crate::test_fixture::open(&home.path().join("store.db")).unwrap();
        let plan: Vec<String> = {
            let conn = store.conn.lock().unwrap();
            let mut statement = conn
                .prepare("EXPLAIN QUERY PLAN SELECT 1 FROM session_events WHERE captured_event=?1")
                .unwrap();
            statement
                .query_map([1], |row| row.get::<_, String>(3))
                .unwrap()
                .map(Result::unwrap)
                .collect()
        };
        assert!(
            plan.iter()
                .any(|step| step.contains("session_events_captured_event")),
            "{plan:?}"
        );
    }
}
