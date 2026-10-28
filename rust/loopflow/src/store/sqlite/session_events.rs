use rusqlite::{params, OptionalExtension, TransactionBehavior};
use serde_json::Value;

use crate::id::AgentSessionId;
use crate::session::{SessionEvent, SessionEventKind};
use crate::session_record::ProviderSessionRef;
use crate::store::{ProviderAccountId, StoreError, StoreResult};

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

    /// What the store kept of each provider conversation `session` used, in
    /// the order it first used them: each turn's start, settlement and final
    /// answer from native receipts, and the account it ran on.
    pub(crate) fn session_turn_facts(
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

    /// Every Session with its current provider conversation and the account
    /// it ran on, for the retention summary.
    pub(crate) fn retention_sessions(
        &self,
    ) -> StoreResult<Vec<crate::transcript::summary::RetainedSession>> {
        let conn = self.conn.lock().expect("store mutex poisoned");
        let mut query = conn.prepare(
            "SELECT s.id,s.title,s.provider,s.provider_thread,s.cwd,
                    MAX(s.created_at,COALESCE(s.completed_at,0)),
                    (SELECT account_id FROM provider_session_accounts a
                     WHERE a.provider=s.provider AND a.provider_session_id=s.provider_thread)
             FROM agent_sessions s WHERE s.provider IS NOT NULL ORDER BY s.created_at,s.id",
        )?;
        let rows = query.query_map([], |row| {
            Ok(crate::transcript::summary::RetainedSession {
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

    /// Each silence the recorder observed in one input, as `(since, until)`.
    pub(crate) fn input_silences(&self, input: &str) -> StoreResult<Vec<(i64, i64)>> {
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
    pub(crate) fn input_turn_context(
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

    pub(crate) fn input_final_answer(&self, input: &str) -> StoreResult<Option<String>> {
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

    pub(crate) fn retain_session_observation(
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
    pub(crate) fn record_capture_fact(
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

    /// Keep initiating failures without unbounded growth during reconnect storms.
    pub(crate) fn retain_codex_relay_failure(
        &self,
        input: &str,
        attachment: Option<&crate::process::SessionAttachment>,
        payload: &Value,
    ) -> StoreResult<()> {
        let mut conn = self.conn.lock().expect("store mutex poisoned");
        let tx = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
        let captured = super::sessions::capture_seq_in(&tx, input)?;
        let count: i64 = tx.query_row(
            "SELECT COUNT(*) FROM session_events WHERE captured_event=?1 AND kind='observed'
             AND json_extract(payload,'$.source')='codex_relay'",
            [captured],
            |row| row.get(0),
        )?;
        if count < 16 {
            tx.execute(
                "INSERT INTO session_events(session_id,kind,receipt_key,agent_process_id,lf_process_id,
                 task_id,wave_id,observed_at,payload,captured_event)
                 SELECT session_id,'observed',?2,?3,?4,task_id,wave_id,?5,?6,seq
                 FROM session_events WHERE seq=?1",
                params![captured,format!("{input}:codex_relay:{count}"),
                    attachment.map(|a| &a.agent_process_id),attachment.and_then(|a| a.lf_process_id.as_ref()),
                    time::OffsetDateTime::now_utc().unix_timestamp(),serde_json::to_string(payload)?],
            )?;
        }
        tx.commit()?;
        Ok(())
    }

    /// Replace the Session's reading with its current attachment's.
    pub(crate) fn record_session_activity(
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
    pub(crate) fn record_session_cpu(
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
    pub(crate) fn session_liveness(
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
    pub(crate) fn record_session_event(
        &self,
        session: &str,
        thread: &AgentSessionId,
        turn: &str,
        kind: SessionEventKind,
        payload: &Value,
    ) -> StoreResult<i64> {
        let mut conn = self.conn.lock().expect("store mutex poisoned");
        let tx = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
        let seq = record_event_in(&tx, session, thread, turn, kind, payload)?;
        tx.commit()?;
        Ok(seq)
    }

    /// Whether the turn's completion is already retained; a provider snapshot
    /// replays completions of turns it has finished before.
    pub(crate) fn session_turn_completed(
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

    /// Freeze the request's attribution while its attachment still owns dispatch.
    pub(crate) fn session_turn_origin(
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
    pub(crate) fn record_session_turn_origin(
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
            &serde_json::json!({}),
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
    pub(crate) fn summary_for_input(
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
        let task = scope.2.map(crate::id::TaskSelector::parse);
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
                     IN (SELECT t.id FROM tasks t JOIN projects p ON p.id=t.project_id
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
                crate::id::canonical_session(session),
                scope.0.map(crate::id::canonical_uuid),
                scope
                    .1
                    .map(|project| crate::id::canonical_prefixed(project, "proj_")),
                scope.1.map(crate::id::canonical_uuid),
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

/// Insert only the observation. Work and capture require correlated origin evidence.
fn record_event_in(
    tx: &rusqlite::Transaction<'_>,
    session: &str,
    thread: &AgentSessionId,
    turn: &str,
    kind: SessionEventKind,
    payload: &Value,
) -> StoreResult<i64> {
    let saved = |receipt: &str| -> StoreResult<Option<(i64, String)>> {
        Ok(tx.query_row(
            "SELECT seq,payload FROM session_events
             WHERE session_id=?1 AND provider_thread=?2 AND provider_turn=?3 AND kind=?4 AND receipt_key=?5",
            params![session,thread,turn,kind.as_str(),receipt],
            |row| Ok((row.get(0)?,row.get(1)?)),
        ).optional()?)
    };
    let mut receipt = if kind == SessionEventKind::Usage {
        crate::session_record::native_usage_key(payload)
    } else {
        String::new()
    };
    let mut existing = saved(&receipt)?;
    if kind == SessionEventKind::Usage {
        // The next turn's baseline reads this turn's usage from before it
        // started; a receipt arriving later keeps its own row after that start.
        if let Some((seq, _)) = &existing {
            let next: Option<i64> = tx.query_row(
                "SELECT max(seq) FROM session_events WHERE session_id=?1 AND provider_thread=?2
                 AND provider_turn!=?3 AND kind='started' AND seq>?4",
                params![session, thread, turn, seq],
                |row| row.get(0),
            )?;
            if let Some(next) = next {
                receipt = crate::session_record::late_usage_key(&receipt, next);
                existing = saved(&receipt)?;
            }
        }
    }
    let mut payload = serde_json::to_string(payload)?;
    if let Some((seq, saved)) = existing {
        if saved == payload {
            return Ok(seq);
        }
        if kind != SessionEventKind::Usage {
            return Err(StoreError::InvalidData(format!(
                "Conflicting {} receipt for conversation {session}, turn {turn}",
                kind.as_str()
            )));
        }
        // One usage row per turn: a later receipt replaces it.
        let merged = crate::session_record::merge_native_usage(
            &serde_json::from_str(&saved)?,
            &serde_json::from_str(&payload)?,
        );
        payload = serde_json::to_string(&merged)?;
        tx.execute(
            "UPDATE session_events SET payload=?2, observed_at=?3 WHERE seq=?1",
            params![
                seq,
                payload,
                time::OffsetDateTime::now_utc().unix_timestamp()
            ],
        )?;
        return Ok(seq);
    }
    tx.execute(
        "INSERT INTO session_events(session_id,provider_thread,provider_turn,kind,receipt_key,observed_at,payload)
         VALUES(?1,?2,?3,?4,?5,?6,?7)",
        params![session,thread,turn,kind.as_str(),receipt,
            time::OffsetDateTime::now_utc().unix_timestamp(),payload],
    )?;
    Ok(tx.last_insert_rowid())
}

#[cfg(test)]
mod tests {
    use crate::session::SessionEventKind;
    use crate::store::sqlite::SqliteStore;
    use serde_json::json;

    fn record_captured_start(store: &SqliteStore, session: &str, turn: &str) -> i64 {
        let attachment = store
            .session_attachment(session)
            .unwrap()
            .unwrap_or_else(|| {
                let process = crate::id::LfProcessId::new();
                store
                    .conn
                    .lock()
                    .unwrap()
                    .execute(
                        "INSERT INTO processes(id,trace_id,started_at) VALUES(?1,'fixture',1)",
                        [&process],
                    )
                    .unwrap();
                store
                    .claim_session_attachment(session, None, &process, false)
                    .unwrap()
            });
        let origin = store.session_turn_origin(session, &attachment).unwrap();
        store
            .record_session_turn_origin(&"thread".into(), turn, &origin)
            .unwrap()
    }

    #[test]
    fn delayed_origin_preserves_pre_bind_assignment_and_rejects_conflicting_repeat() {
        let home = tempfile::tempdir().unwrap();
        let store = SqliteStore::open_ephemeral(&home.path().join("store.db")).unwrap();
        let session =
            store.test_session("conversation", &crate::session_record::new_artifact_key());
        let process = crate::id::LfProcessId::new();
        let wave = crate::id::WaveId::new();
        let project = crate::work::project::ProjectId::new();
        let task = crate::durable::TaskId::new();
        {
            let conn = store.conn.lock().unwrap();
            conn.execute(
                "INSERT INTO processes(id,trace_id,started_at) VALUES(?1,'fixture',1)",
                [&process],
            )
            .unwrap();
            conn.execute(
                "INSERT INTO waves(id,name,repo,created_at) VALUES(?1,'proof','/repo',1)",
                [&wave],
            )
            .unwrap();
            conn.execute("INSERT INTO projects(id,wave_id,external_project_id,created_at) VALUES(?1,?2,'project',1)", rusqlite::params![project.as_str(),wave]).unwrap();
            conn.execute("INSERT INTO repository_plans(id,repo,selected) SELECT 'repo_'||lower(hex(randomblob(16))),w.repo,1 FROM waves w WHERE NOT EXISTS(SELECT 1 FROM repository_plans r WHERE r.repo=w.repo AND r.selected=1) GROUP BY w.repo", []).unwrap();
            conn.execute("INSERT INTO tasks(repository_id,id,project_id,external_issue_id,issue_identifier,issue_title,issue_description,workspace_slug,worktree,created_at,updated_at) VALUES((SELECT r.id FROM projects p JOIN waves w ON w.id=p.wave_id JOIN repository_plans r ON r.repo=w.repo AND r.selected=1 WHERE p.id=?2),?1,?2,'issue','PROOF-1','Proof','','',NULL,1,1)", rusqlite::params![task.as_str(),project.as_str()]).unwrap();
        }
        let attachment = store
            .claim_session_attachment(&session.id, None, &process, false)
            .unwrap();
        let before = store.session_turn_origin(&session.id, &attachment).unwrap();
        store
            .bind_session(&session.id, session.captured, &task)
            .unwrap();
        let after = store.session_turn_origin(&session.id, &attachment).unwrap();
        let seq = store
            .record_session_turn_origin(&"thread".into(), "late", &before)
            .unwrap();
        let revisions = store.revisions().unwrap();
        assert_eq!(
            store
                .record_session_turn_origin(&"thread".into(), "late", &before)
                .unwrap(),
            seq
        );
        assert_eq!(store.revisions().unwrap(), revisions);
        assert!(store
            .record_session_turn_origin(&"thread".into(), "late", &after)
            .is_err());
        store
            .record_session_turn_origin(&"thread".into(), "new", &after)
            .unwrap();
        let events = store.session_history(&session.id, 0, 0).unwrap();
        let late = events
            .iter()
            .find(|event| event.provider_turn.as_deref() == Some("late"))
            .unwrap();
        let new = events
            .iter()
            .find(|event| event.provider_turn.as_deref() == Some("new"))
            .unwrap();
        assert_eq!(late.task_id, None);
        assert_eq!(late.wave_id, None);
        assert_eq!(new.task_id.as_deref(), Some(task.as_str()));
        assert_eq!(new.wave_id.as_deref(), Some(wave.as_str()));
        store.move_task(&task, Some(&project), None).unwrap();
        let detached = store.session_turn_origin(&session.id, &attachment).unwrap();
        store
            .record_session_turn_origin(&"thread".into(), "dispatched-before-move", &after)
            .unwrap();
        store
            .record_session_turn_origin(&"thread".into(), "dispatched-after-move", &detached)
            .unwrap();
        let history = store.session_history(&session.id, 0, 0).unwrap();
        let before_move = history
            .iter()
            .find(|event| event.provider_turn.as_deref() == Some("dispatched-before-move"))
            .unwrap();
        let after_move = history
            .iter()
            .find(|event| event.provider_turn.as_deref() == Some("dispatched-after-move"))
            .unwrap();
        assert_eq!(before_move.wave_id.as_deref(), Some(wave.as_str()));
        assert_eq!(after_move.wave_id, None);
        assert_eq!(after_move.task_id.as_deref(), Some(task.as_str()));
    }

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
    fn silences_follow_input_order() {
        let home = tempfile::tempdir().unwrap();
        let store = SqliteStore::open_ephemeral(&home.path().join("store.db")).unwrap();
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
    fn history_limits_precede_payload_reads_and_keep_unfinished_work_and_exact_callers() {
        let home = tempfile::tempdir().unwrap();
        let store = SqliteStore::open_ephemeral(&home.path().join("store.db")).unwrap();
        let mut template = store.test_session("seed", "run_000000000000000000000000000000ff");
        template.captured = None;
        template.caller_artifact_key = Some(template.input_key().unwrap().clone());
        let attached = crate::id::LfProcessId::new();
        store
            .conn
            .lock()
            .unwrap()
            .execute(
                "INSERT INTO processes(id,trace_id,started_at) VALUES(?1,'fixture',1)",
                [&attached],
            )
            .unwrap();
        for at in 1..=60 {
            let mut session = template.clone();
            session.id = format!("conversation-{at}").into();
            session.artifact_key = Some(format!("run_{at:032x}"));
            session.created_at = at;
            let session = store
                .create_session(session, (at == 1).then_some(&attached))
                .unwrap();
            record_captured_start(&store, &session.id, &at.to_string());
            if at != 1 {
                store
                    .record_session_event(
                        &session.id,
                        &"thread".into(),
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
                Some(template.input_key().unwrap().as_str()),
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
        // A turn left open by an attachment that has exited is over: it stops
        // holding a place in the recent list.
        store
            .conn
            .lock()
            .unwrap()
            .execute(
                "UPDATE processes SET completed_at=100 WHERE id IN (SELECT lf_process_id FROM session_events WHERE session_id='conversation-1' AND kind='started')",
                [],
            )
            .unwrap();
        assert_eq!(
            store
                .recent_conversation_history(None, None, None, 0, 5)
                .unwrap()
                .0
                .iter()
                .map(|row| row.observed_at)
                .collect::<Vec<_>>(),
            [60, 59, 58, 57, 1]
        );
        assert!(store
            .conversation_history(
                None,
                None,
                None,
                Some(template.input_key().unwrap().as_str()),
                0,
                false
            )
            .is_err());
    }

    #[test]
    fn process_completion_ends_inputs_without_exit_events() {
        for captured in [true, false] {
            let home = tempfile::tempdir().unwrap();
            let store = SqliteStore::open_ephemeral(&home.path().join("store.db")).unwrap();
            let mut session =
                store.test_session("seed", &crate::session_record::new_artifact_key());
            session.id = "conversation".into();
            session.captured = None;
            session.artifact_key = Some(crate::session_record::new_artifact_key());
            let process = crate::id::LfProcessId::new();
            let creator = crate::id::LfProcessId::new();
            {
                let conn = store.conn.lock().unwrap();
                conn.execute(
                    "INSERT INTO processes(id,trace_id,started_at) VALUES(?1,'fixture',1)",
                    [&process],
                )
                .unwrap();
            }
            store.conn.lock().unwrap().execute(
                "INSERT INTO processes(id,trace_id,started_at,completed_at) VALUES(?1,'fixture',1,25)", [&creator],
            ).unwrap();
            // Preparation may exit before another lf attaches and starts the turn.
            store.create_session(session, Some(&creator)).unwrap();
            {
                let conn = store.conn.lock().unwrap();
                conn.execute("UPDATE agent_sessions SET input_published=?1", [captured])
                    .unwrap();
                conn.execute(
                    "INSERT INTO session_events(session_id,provider_thread,provider_turn,kind,receipt_key,observed_at,payload,captured_event,lf_process_id)
                     SELECT id,'thread','turn','started','',10,'{}',CASE WHEN ?1 THEN current_capture END,?2 FROM agent_sessions WHERE id='conversation'",
                    rusqlite::params![captured,process],
                ).unwrap();
            }
            let completed_since = || {
                store
                    .conversation_history(None, None, None, None, 20, true)
                    .unwrap()
            };
            assert!(completed_since().is_empty());
            // A turn with unknown ownership must not fall back to its ended creator.
            store.conn.lock().unwrap().execute(
                "UPDATE session_events SET lf_process_id=NULL WHERE session_id='conversation' AND kind='started'",
                [],
            ).unwrap();
            assert!(completed_since().is_empty());
            store.conn.lock().unwrap().execute(
                "UPDATE session_events SET lf_process_id=?1 WHERE session_id='conversation' AND kind='started'",
                [&process],
            ).unwrap();
            // An unrelated attachment's exit cannot finish this input.
            store
                .conn
                .lock()
                .unwrap()
                .execute(
                    "INSERT INTO session_events(session_id,kind,receipt_key,observed_at,payload)
                 VALUES('conversation','observed','attachment:other:exit',30,'{}')",
                    [],
                )
                .unwrap();
            assert!(completed_since().is_empty());
            store
                .conn
                .lock()
                .unwrap()
                .execute(
                    "UPDATE processes SET completed_at=30 WHERE id=?1",
                    [&process],
                )
                .unwrap();
            let history = completed_since();
            assert_eq!(history.len(), 1, "captured={captured}");
            assert_eq!(history[0].captured.is_some(), captured);
            assert!(
                history[0].providers[0].outcome.is_none(),
                "process death is not provider success"
            );
        }
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
                    &"thread".into(),
                    turn,
                    SessionEventKind::Usage,
                    &json!({"total":counts,"last":counts}),
                )
                .unwrap();
            store
                .record_session_event(
                    "conversation",
                    &"thread".into(),
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
        assert!(provider.lf_process_id.is_none());
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
    fn session_exit_closes_agent_process_unless_ownership_transferred() {
        use std::os::unix::process::CommandExt;
        use std::process::Command;

        for transfer in [false, true] {
            let home = tempfile::tempdir().unwrap();
            let store = SqliteStore::open_ephemeral(&home.path().join("store.db")).unwrap();
            let session =
                store.test_session("conversation", &crate::session_record::new_artifact_key());
            let first = crate::id::LfProcessId::new();
            let second = crate::id::LfProcessId::new();
            {
                let conn = store.conn.lock().unwrap();
                for process in [&first, &second] {
                    // The AgentProcess inherits this trace; the inventory reads it typed.
                    conn.execute(
                        "INSERT INTO processes(id,trace_id,started_at) VALUES(?1,?2,1)",
                        rusqlite::params![process, crate::id::TraceId::new()],
                    )
                    .unwrap();
                }
                conn.execute(
                    "UPDATE agent_sessions SET provider='codex',interactive=0 WHERE id=?1",
                    [&session.id],
                )
                .unwrap();
            }
            let original = store
                .claim_session_attachment(&session.id, None, &first, true)
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
                .record_agent_process_identity(&session.id, &original, child.id(), started)
                .unwrap();
            // An AgentProcess whose socket has gone must still be reaped on exit.
            store
                .record_session_connection(
                    &session.id,
                    &original,
                    home.path().join("agent.sock").to_str().unwrap(),
                    &"saved-thread".into(),
                )
                .unwrap();
            let current = if transfer {
                let replacement = store
                    .claim_session_attachment(&session.id, Some(&original), &second, false)
                    .unwrap();
                assert!(matches!(
                    crate::session_record::finish_session_attachment(
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
            crate::session_record::finish_session_attachment(
                &store,
                &session.id,
                &current,
                "completed",
            )
            .unwrap();
            // Close leaves reaping to the parent; an unreaped zombie is dead.
            assert_eq!(
                crate::journal::process_identity_evidence(child.id(), started),
                crate::journal::ProcessIdentityEvidence::Dead
            );
            let _ = child.wait();
            assert!(store.session_connection(&session.id).unwrap().is_none());
            assert_eq!(
                store.session_thread(&session.id).unwrap(),
                Some("saved-thread".into())
            );
            assert!(store
                .session_attachment(&session.id)
                .unwrap()
                .unwrap()
                .lf_process_id
                .is_none());
        }
    }

    #[test]
    fn session_exit_retires_orphans_but_preserves_primary_and_task_conversations() {
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
            conn.execute("INSERT INTO repository_plans(id,repo,selected) SELECT 'repo_'||lower(hex(randomblob(16))),w.repo,1 FROM waves w WHERE NOT EXISTS(SELECT 1 FROM repository_plans r WHERE r.repo=w.repo AND r.selected=1) GROUP BY w.repo", []).unwrap();
            conn.execute(
                "INSERT INTO tasks(repository_id,id,project_id,external_issue_id,issue_identifier,created_at) VALUES((SELECT r.id FROM projects p JOIN waves w ON w.id=p.wave_id JOIN repository_plans r ON r.repo=w.repo AND r.selected=1 WHERE p.id=?2),?1,?2,'issue-proof','PROOF-1',1)",
                rusqlite::params![task.as_str(), project.as_str()],
            ).unwrap();
        }
        for (id, primary, retired) in [
            ("orphan", None, true),
            ("primary", Some("repository"), false),
            ("task", None, false),
        ] {
            let session = store.test_session(id, &crate::session_record::new_artifact_key());
            let process = crate::id::LfProcessId::new();
            {
                let conn = store.conn.lock().unwrap();
                conn.execute(
                    "INSERT INTO processes(id,trace_id,started_at) VALUES(?1,'fixture',1)",
                    [process.as_str()],
                )
                .unwrap();
                conn.execute(
                    "UPDATE agent_sessions SET primary_scope=?2 WHERE id=?1",
                    rusqlite::params![id, primary],
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
            let attachment = store
                .claim_session_attachment(id, None, &process, true)
                .unwrap();
            store
                .finish_session_attachment(id, &attachment, "interrupted", None, || Ok(false))
                .unwrap();
            let saved = store.session(id).unwrap().unwrap();
            assert_eq!(saved.completed_at.is_some(), retired);
            assert_eq!(saved.captured, session.captured);
            let history = store.session_history(id, 0, 100).unwrap();
            assert!(history
                .iter()
                .any(|event| event.payload["type"] == "attachment_exit"
                    && event.payload["outcome"] == "interrupted"));
            assert!(!history
                .iter()
                .any(|event| event.kind == SessionEventKind::Completed));
        }
        assert!(!store
            .session_summaries(&crate::session::SessionFilter::default(), 0)
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
                    &"thread".into(),
                    "turn",
                    kind,
                    &json!({"status": "completed"}),
                )
                .unwrap();
        }
        for _ in 0..2 {
            let rows = store
                .session_summaries(&crate::session::SessionFilter::default(), 0)
                .unwrap();
            assert_eq!(rows.len(), 1);
            assert_eq!(rows[0].id, session.id.as_str());
            assert!(rows[0].completed_at.is_none());
        }
    }

    #[test]
    fn stopped_turn_and_stale_attachment_cannot_retire_a_resumed_conversation() {
        let home = tempfile::tempdir().unwrap();
        let store = SqliteStore::open_ephemeral(&home.path().join("store.db")).unwrap();
        let session =
            store.test_session("conversation", &crate::session_record::new_artifact_key());
        let first = crate::id::LfProcessId::new();
        let second = crate::id::LfProcessId::new();
        for process in [&first, &second] {
            store
                .conn
                .lock()
                .unwrap()
                .execute(
                    "INSERT INTO processes(id,trace_id,started_at) VALUES(?1,'fixture',1)",
                    [process.as_str()],
                )
                .unwrap();
        }
        let original = store
            .claim_session_attachment(&session.id, None, &first, true)
            .unwrap();
        record_captured_start(&store, &session.id, "turn");
        store
            .record_session_event(
                &session.id,
                &"thread".into(),
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
        let resumed = store
            .claim_session_attachment(&session.id, Some(&original), &second, true)
            .unwrap();
        assert!(store
            .finish_session_attachment(&session.id, &original, "interrupted", None, || Ok(false))
            .is_err());
        assert!(store
            .session(&session.id)
            .unwrap()
            .unwrap()
            .completed_at
            .is_none());
        assert_eq!(
            store.session_attachment(&session.id).unwrap().unwrap(),
            resumed
        );
        store
            .finish_session_attachment(&session.id, &resumed, "completed", None, || Ok(false))
            .unwrap();
        assert!(store
            .session(&session.id)
            .unwrap()
            .unwrap()
            .completed_at
            .is_some());
    }

    #[test]
    fn native_usage_survives_attachment_and_input_replacement_without_double_counting() {
        let home = tempfile::tempdir().unwrap();
        let store = SqliteStore::open_ephemeral(&home.path().join("store.db")).unwrap();
        store.test_session("conversation", "run_00000000000000000000000000000001");
        let first_input =
            crate::session_record::parse_artifact_key("run_00000000000000000000000000000001")
                .unwrap();
        let session = store.session("conversation").unwrap().unwrap();
        let first = crate::id::LfProcessId::new();
        let second = crate::id::LfProcessId::new();
        for process in [&first, &second] {
            store
                .conn
                .lock()
                .unwrap()
                .execute(
                    "INSERT INTO processes(id,trace_id,started_at) VALUES(?1,'fixture',1)",
                    [process.as_str()],
                )
                .unwrap();
        }
        let attachment = store
            .claim_session_attachment(&session.id, None, &first, false)
            .unwrap();
        store
            .record_session_turn_origin(
                &"thread".into(),
                "turn",
                &store.session_turn_origin(&session.id, &attachment).unwrap(),
            )
            .unwrap();
        let counts = |input, output| {
            json!({"inputTokens":input,"outputTokens":output,
            "cachedInputTokens":2,"reasoningOutputTokens":1})
        };
        store
            .record_session_event(
                &session.id,
                &"thread".into(),
                "turn",
                SessionEventKind::Usage,
                &json!({"total":counts(12,3),"last":counts(12,3)}),
            )
            .unwrap();
        let mut replacement = session.clone();
        replacement.artifact_key = Some(crate::session_record::new_artifact_key());
        store
            .replace_session_input(session.captured, replacement.clone())
            .unwrap();
        store
            .claim_session_attachment(&session.id, Some(&attachment), &second, true)
            .unwrap();
        // The replacement AgentProcess observes the first one's surviving turn. The recorder never saw its usage.
        store
            .record_session_event(
                &session.id,
                &"thread".into(),
                "turn",
                SessionEventKind::Usage,
                &json!({"total":counts(20,5),"last":counts(8,2)}),
            )
            .unwrap();
        store
            .record_session_event(
                &session.id,
                &"thread".into(),
                "turn",
                SessionEventKind::Completed,
                &json!({"status":"completed"}),
            )
            .unwrap();
        // Start, one usage row for both receipts, and completion.
        let history = store.summary_for_input(&session.id, &first_input).unwrap();
        assert_eq!(history.len(), 3);
        assert!(history.iter().all(
            |event| event.lf_process_id.as_deref() == Some(first.as_str())
                && event.agent_process_id.as_deref() == Some(attachment.agent_process_id.as_str())
        ));
        assert!(store
            .summary_for_input(&session.id, replacement.input_key().unwrap())
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
        // A stream usage copy an earlier lf stored is not usage: only native rows count.
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
        assert_eq!(old.usage.cost_usd, None);
        assert_eq!(old.usage.final_streams, 0);
        // An unobserved start cannot borrow the replacement's input or attribution.
        store
            .record_session_event(
                &session.id,
                &"thread".into(),
                "unknown",
                SessionEventKind::Usage,
                &json!({"total":counts(100,50),"last":counts(10,5)}),
            )
            .unwrap();
        assert!(store
            .summary_for_input(&session.id, replacement.input_key().unwrap())
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
        // Even with a start, a late first usage snapshot is a lower bound, not the thread total.
        record_captured_start(&store, &session.id, "partial");
        store
            .record_session_event(
                &session.id,
                &"thread".into(),
                "partial",
                SessionEventKind::Usage,
                &json!({"total":counts(100,50),"last":counts(10,5)}),
            )
            .unwrap();
        let partial = store
            .input_history(replacement.input_key().unwrap().as_str())
            .unwrap();
        assert_eq!(partial.usage.total_input_tokens, Some(10));
        assert_eq!(partial.usage.output_tokens, Some(5));
        assert!(partial.usage.gaps > 0);
        record_captured_start(&store, &session.id, "decrease");
        for (total, last) in [(20, 20), (40, 20), (30, 10)] {
            store.record_session_event(&session.id,&"thread".into(),"decrease",SessionEventKind::Usage,
                &json!({"total":{"inputTokens":total,"outputTokens":0,"cachedInputTokens":0,"reasoningOutputTokens":0},
                    "last":{"inputTokens":last,"outputTokens":0,"cachedInputTokens":0,"reasoningOutputTokens":0}})).unwrap();
        }
        let decreased = store
            .input_history(replacement.input_key().unwrap().as_str())
            .unwrap();
        assert_eq!(
            decreased.usage.total_input_tokens,
            Some(50),
            "retain 40 observed tokens plus the separate 10-token partial turn"
        );
        // The turn keeps one row; the regression survives as an evidence gap.
        assert!(decreased.evidence_gaps > partial.evidence_gaps);
        store
            .record_session_event(
                &session.id,
                &"thread".into(),
                "decrease",
                SessionEventKind::Completed,
                &json!({"status":"completed"}),
            )
            .unwrap();
        record_captured_start(&store, &session.id, "next");
        // Reconnect missed one request: 60 lifetime minus the retained 40 peak,
        // not merely this final request's 10 tokens or the regressed 30 baseline.
        store.record_session_event(&session.id,&"thread".into(),"next",SessionEventKind::Usage,
            &json!({"total":{"inputTokens":60,"outputTokens":0,"cachedInputTokens":0,"reasoningOutputTokens":0},
                "last":{"inputTokens":10,"outputTokens":0,"cachedInputTokens":0,"reasoningOutputTokens":0}})).unwrap();
        assert_eq!(
            store
                .input_history(replacement.input_key().unwrap().as_str())
                .unwrap()
                .usage
                .total_input_tokens,
            Some(70)
        );
    }

    #[test]
    fn native_usage_keeps_one_row_per_turn_until_the_next_turn_starts() {
        let home = tempfile::tempdir().unwrap();
        let store = SqliteStore::open_ephemeral(&home.path().join("store.db")).unwrap();
        store.test_session("conversation", &crate::session_record::new_artifact_key());
        let usage = |turn: &str, total: u64| {
            let counts = json!({"inputTokens":total,"outputTokens":0});
            store
                .record_session_event(
                    "conversation",
                    &"thread".into(),
                    turn,
                    SessionEventKind::Usage,
                    &json!({"total":counts,"last":counts}),
                )
                .unwrap();
        };
        record_captured_start(&store, "conversation", "a");
        usage("a", 10);
        usage("a", 20);
        record_captured_start(&store, "conversation", "b");
        // Late receipts for "a" stay after "b" began, where they arrived.
        usage("a", 30);
        usage("a", 40);
        let keys: Vec<String> = store
            .conn
            .lock()
            .unwrap()
            .prepare("SELECT receipt_key FROM session_events WHERE kind='usage' ORDER BY seq")
            .unwrap()
            .query_map([], |row| row.get(0))
            .unwrap()
            .collect::<Result<_, _>>()
            .unwrap();
        assert_eq!(keys.len(), 2);
        assert_eq!(keys[0], "");
        assert!(keys[1].starts_with(":after:"));
    }

    #[test]
    fn a_later_usage_receipt_replaces_its_row_in_place() {
        let home = tempfile::tempdir().unwrap();
        let store = SqliteStore::open_ephemeral(&home.path().join("store.db")).unwrap();
        store.test_session("conversation", &crate::session_record::new_artifact_key());
        record_captured_start(&store, "conversation", "a");
        let usage = |total: u64| {
            let counts = json!({"inputTokens":total,"outputTokens":0});
            store
                .record_session_event(
                    "conversation",
                    &"thread".into(),
                    "a",
                    SessionEventKind::Usage,
                    &json!({"total":counts,"last":counts}),
                )
                .unwrap()
        };
        let first = usage(10);
        assert_eq!(usage(20), first);
        let payload: String = store
            .conn
            .lock()
            .unwrap()
            .query_row(
                "SELECT payload FROM session_events WHERE seq=?1",
                [first],
                |row| row.get(0),
            )
            .unwrap();
        assert!(payload.contains("20"));
    }

    #[test]
    fn deleting_a_session_event_probes_captured_event_by_index() {
        let home = tempfile::tempdir().unwrap();
        let store = SqliteStore::open_ephemeral(&home.path().join("store.db")).unwrap();
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
            record_captured_start(&store, "conversation", turn);
            if let Some(total) = total {
                store
                    .record_session_event(
                        "conversation",
                        &"thread".into(),
                        turn,
                        SessionEventKind::Usage,
                        &json!({"total":counts(total),"last":counts(last)}),
                    )
                    .unwrap();
            }
            store
                .record_session_event(
                    "conversation",
                    &"thread".into(),
                    turn,
                    SessionEventKind::Completed,
                    &json!({"status":"completed"}),
                )
                .unwrap();
        }
        // B's usage is unknown: 20 for A and C's 10-token suffix.
        let usage = store.input_history(&input).unwrap().usage;
        assert_eq!(usage.total_input_tokens, Some(30));
        assert_eq!(usage.streams, 2);
        assert_eq!(usage.final_streams, 0);
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
                &"thread".into(),
                "turn",
                SessionEventKind::Completed,
                &completed,
            )
            .unwrap();
        assert_eq!(
            store
                .record_session_event(
                    "conversation",
                    &"thread".into(),
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
                &"thread".into(),
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
        assert_eq!(history[0].agent_process_id, None);
        assert_eq!(history[0].lf_process_id, None);
        assert!(store
            .session_history("conversation", seq, 100)
            .unwrap()
            .is_empty());

        let first = crate::id::LfProcessId::new();
        let second = crate::id::LfProcessId::new();
        for process in [&first, &second] {
            store
                .conn
                .lock()
                .unwrap()
                .execute(
                    "INSERT INTO processes(id,trace_id,started_at) VALUES(?1,'fixture',1)",
                    [process.as_str()],
                )
                .unwrap();
        }
        let original = store
            .claim_session_attachment("conversation", None, &first, false)
            .unwrap();
        store
            .record_session_connection(
                "conversation",
                &original,
                "/original.sock",
                &"thread".into(),
            )
            .unwrap();
        store
            .record_agent_process_identity("conversation", &original, 12345, 12)
            .unwrap();
        store
            .record_session_turn_origin(
                &"thread".into(),
                "later",
                &store
                    .session_turn_origin("conversation", &original)
                    .unwrap(),
            )
            .unwrap();
        let replacement = store
            .claim_session_attachment("conversation", Some(&original), &second, true)
            .unwrap();
        assert_ne!(replacement.agent_process_id, original.agent_process_id);
        assert!(store.session_connection("conversation").unwrap().is_none());
        assert!(store
            .agent_process_identity("conversation")
            .unwrap()
            .is_none());
        assert_eq!(
            store.session_thread("conversation").unwrap(),
            Some("thread".into())
        );
        assert!(store
            .record_session_connection(
                "conversation",
                &original,
                "/stale.sock",
                &"wrong-thread".into()
            )
            .is_err());
        assert!(store
            .record_agent_process_identity("conversation", &original, 12346, 13)
            .is_err());
        store
            .record_session_event(
                "conversation",
                &"thread".into(),
                "later",
                SessionEventKind::Usage,
                &json!({"total":{"inputTokens":40}}),
            )
            .unwrap();
        store
            .record_session_event(
                "conversation",
                &"thread".into(),
                "unobserved",
                SessionEventKind::Usage,
                &json!({"total":{"inputTokens":60}}),
            )
            .unwrap();
        let events = store.session_history("conversation", seq, 100).unwrap();
        assert_eq!(
            events[1].agent_process_id.as_deref(),
            Some(original.agent_process_id.as_str())
        );
        assert_eq!(events[1].lf_process_id.as_deref(), Some(first.as_str()));
        assert_eq!(events[2].agent_process_id, None);
        assert_eq!(events[2].lf_process_id, None);
    }
}
