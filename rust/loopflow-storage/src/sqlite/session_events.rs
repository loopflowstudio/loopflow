use rusqlite::{params, OptionalExtension, TransactionBehavior};
use serde_json::Value;

use crate::session::{SessionEvent, SessionEventKind};
use crate::session_record::ProviderSessionRef;
use crate::{ProviderAccountId, StoreError, StoreResult};
use loopflow_model::id::AgentSessionId;

use super::SqliteStore;

/// The provider turn that answered an input, and where its provider kept it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AnsweredTurn {
    pub session_id: String,
    pub provider: Option<String>,
    pub thread: AgentSessionId,
    pub turn: String,
    pub cwd: std::path::PathBuf,
}

/// A Session's current provider conversation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CurrentConversation {
    pub provider: String,
    pub thread: AgentSessionId,
}

impl SqliteStore {
    /// The provider conversation that resumes `input`, as its captured row
    /// keeps it.
    pub fn input_provider_session(&self, input: &str) -> StoreResult<Option<ProviderSessionRef>> {
        let conn = self.conn.lock().expect("store mutex poisoned");
        let reference: Option<(Option<AgentSessionId>, Option<String>)> = conn
            .query_row(
                "SELECT provider_thread,provider_account FROM session_events
                 WHERE kind='captured' AND receipt_key=?1",
                [input],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .optional()?;
        let Some((Some(thread), account)) = reference else {
            return Ok(None);
        };
        let account = account
            .map(|account| ProviderAccountId::parse(&account))
            .transpose()
            .map_err(|error| StoreError::InvalidData(error.to_string()))?;
        Ok(Some(ProviderSessionRef::new(thread, account)))
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
                 FROM history.session_history e
                 LEFT JOIN session_events c ON c.seq=e.captured_event AND c.kind='captured'
                 WHERE e.session_id=?1 AND e.kind IN ('started','completed')
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
                        });
                        facts.turns.len() - 1
                    }
                };
                let entry = &mut facts.turns[position];
                if kind == "started" {
                    entry.started_at = Some(at);
                } else {
                    entry.settled_at = Some(at);
                    entry.status = payload["status"].as_str().map(str::to_owned);
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
                "SELECT observed_at,payload FROM history.session_history
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
            "SELECT e.payload FROM session_events c
             JOIN history.session_history e ON c.seq=e.captured_event AND c.kind='captured' AND c.receipt_key=?1
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
             FROM session_events c
             JOIN history.session_history e ON c.seq=e.captured_event AND c.kind='captured' AND c.receipt_key=?1
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
            "SELECT json_extract(done.payload,'$.context') FROM history.session_history start
             JOIN history.session_history done ON done.session_id=start.session_id
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

    /// The provider turn that last answered `input`: the newest turn it
    /// started that completed. Its text is in the provider's transcript.
    pub fn input_answered_turn(&self, input: &str) -> StoreResult<Option<AnsweredTurn>> {
        let conn = self.conn.lock().expect("store mutex poisoned");
        Ok(conn
            .query_row(
                "SELECT c.session_id,COALESCE(json_extract(c.payload,'$.provider'),s.provider),
                        start.provider_thread,start.provider_turn,COALESCE(json_extract(c.payload,'$.cwd'),s.cwd)
                 FROM session_events c JOIN agent_sessions s ON s.id=c.session_id
                 CROSS JOIN history.session_history start ON start.captured_event=c.seq AND start.kind='started'
                 JOIN history.session_history done ON done.session_id=start.session_id
                   AND done.provider_thread=start.provider_thread AND done.provider_turn=start.provider_turn
                   AND done.kind='completed' AND json_extract(done.payload,'$.status')='completed'
                 WHERE c.kind='captured' AND c.receipt_key=?1
                 ORDER BY done.seq DESC LIMIT 1",
                [input],
                |row| {
                    Ok(AnsweredTurn {
                        session_id: row.get(0)?,
                        provider: row.get(1)?,
                        thread: row.get(2)?,
                        turn: row.get(3)?,
                        cwd: row.get::<_, String>(4)?.into(),
                    })
                },
            )
            .optional()?)
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
        let history = {
            let mut conn = self.conn.lock().expect("store mutex poisoned");
            let tx = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
            let history = super::sessions::retain_history_in(
                &tx,
                session,
                std::slice::from_ref(observation),
            )?;
            tx.commit()?;
            history
        };
        self.append_history("session history", &history);
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
    ) -> StoreResult<()> {
        let row = native_row(
            session,
            thread,
            turn,
            kind,
            time::OffsetDateTime::now_utc().unix_timestamp(),
            serde_json::to_string(payload)?,
        );
        self.append_history("provider turn", &[row]);
        Ok(())
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
                "SELECT 1 FROM history.session_history WHERE session_id=?1 AND provider_thread=?2
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
            "SELECT provider_turn,observed_at FROM history.session_history
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
        let usage = serde_json::to_string(usage)?;
        Ok(self
            .write_history("turn usage", |tx| {
                let kept = tx
                    .query_row(
                        "SELECT 1 FROM session_history WHERE session_id=?1 AND provider_thread=?2
                         AND provider_turn=?3 AND kind='usage'",
                        params![session, thread, turn],
                        |_| Ok(()),
                    )
                    .optional()?
                    .is_some();
                if kept {
                    return Ok(false);
                }
                let row = native_row(session, thread, turn, SessionEventKind::Usage, at, usage);
                crate::history::append_in(tx, &row)?;
                Ok(true)
            })
            .unwrap_or(false))
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
        let row = native_row(
            session,
            thread,
            turn,
            SessionEventKind::Completed,
            at,
            serde_json::to_string(completed)?,
        );
        self.append_history("turn completion", &[row]);
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

    /// Retain correlated origin after takeover without reading current
    /// assignment. An uncorrelated start acquires its origin once; repeated
    /// evidence must match every field, including an explicitly absent bind.
    pub fn record_session_turn_origin(
        &self,
        thread: &AgentSessionId,
        turn: &str,
        origin: &crate::session::SessionTurnOrigin,
    ) -> StoreResult<()> {
        let started = native_row(
            &origin.session_id,
            thread,
            turn,
            SessionEventKind::Started,
            time::OffsetDateTime::now_utc().unix_timestamp(),
            "{}".into(),
        );
        self.write_history("turn origin", |tx| {
            let seq = crate::history::append_in(tx, &started)?;
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
                     FROM session_history WHERE seq=?1 AND lf_process_id IS NOT NULL",
                    values,
                    |row| row.get(0),
                )
                .optional()?;
            match matches {
                Some(true) => Ok(()),
                Some(false) => Err(StoreError::InvalidData(
                    "Native turn has different initiating evidence".into(),
                )),
                None => {
                    tx.execute(
                        "UPDATE session_history SET lf_process_id=?2,agent_process_id=?3,
                            task_id=?4,wave_id=?5,captured_event=?6 WHERE seq=?1",
                        values,
                    )?;
                    Ok(())
                }
            }
        });
        Ok(())
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
        // History rows answer to their own scope, provider turn rows to their
        // turn's origin. Inputs are Session state in the store, not history.
        let mut query = conn.prepare(
            "SELECT e.seq,e.session_id,e.provider_thread,e.provider_turn,e.kind,
                    CASE WHEN e.kind='observed' THEN e.agent_process_id ELSE origin.agent_process_id END,
                    CASE WHEN e.kind='observed' THEN e.lf_process_id ELSE origin.lf_process_id END,
                    CASE WHEN e.kind='observed' THEN e.task_id ELSE origin.task_id END,
                    CASE WHEN e.kind='observed' THEN e.wave_id ELSE origin.wave_id END,e.observed_at,e.payload
             FROM history.session_history e LEFT JOIN history.session_history origin
               ON origin.session_id=e.session_id AND origin.provider_thread=e.provider_thread
               AND origin.provider_turn=e.provider_turn AND origin.kind='started'
             WHERE e.session_id=?11 AND e.seq>?2
             AND (NOT ?8 OR (e.kind IN ('started','usage','completed') AND COALESCE(origin.captured_event,e.captured_event) IS NULL
                  AND e.provider_thread IS ?9 AND e.provider_turn IS ?10))
             AND (?4 IS NULL OR (e.kind='observed' AND substr(e.receipt_key,1,length(?4)+1)=?4||':')
                  OR (e.kind!='observed' AND COALESCE(origin.captured_event,e.captured_event)=(SELECT seq FROM session_events WHERE kind='captured' AND receipt_key=?4)))
             AND ((e.kind='observed' AND CASE WHEN json_valid(e.payload) THEN json_extract(e.payload,'$.source') END IN ('manifest.json','runs','terminal.json')) OR (
                 (?5 IS NULL OR (CASE WHEN e.kind='observed' THEN e.wave_id ELSE origin.wave_id END)
                     IN (SELECT id FROM wave_addresses WHERE id=?12 OR slug=?5))
                 AND (?6 IS NULL OR (CASE WHEN e.kind='observed' THEN e.task_id ELSE origin.task_id END)
                     IN (SELECT t.id FROM projects p CROSS JOIN tasks t ON t.project_id=p.id
                         WHERE p.id=?13 OR p.project_slug=?6 OR p.external_project_id=?14))
                 AND (?7 IS NULL OR (CASE WHEN e.kind='observed' THEN e.task_id ELSE origin.task_id END)
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
                    "observed" => SessionEventKind::Observed,
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

/// A provider turn's row: its receipt is the turn and kind.
fn native_row(
    session: &str,
    thread: &AgentSessionId,
    turn: &str,
    kind: SessionEventKind,
    observed_at: i64,
    payload: String,
) -> crate::history::Row {
    crate::history::Row {
        session_id: session.into(),
        provider_thread: Some(thread.as_str().into()),
        provider_turn: Some(turn.into()),
        kind: kind.as_str(),
        receipt_key: String::new(),
        observed_at,
        payload,
        ..Default::default()
    }
}

#[cfg(test)]
mod tests {

    use crate::session::SessionEventKind;
    use crate::sqlite::SqliteStore;
    use crate::RecordingGap;
    use loopflow_model::id::AgentSessionId;
    use serde_json::json;

    #[test]
    fn a_replayed_provider_receipt_keeps_one_history_row() {
        let home = tempfile::tempdir().unwrap();
        let store = crate::test_fixture::open(&home.path().join("store.db")).unwrap();
        store.test_session("conversation", &crate::session_record::new_artifact_key());
        let record = |turn: &str, text: &str| {
            store
                .record_session_event(
                    "conversation",
                    &"Provider-Thread".into(),
                    turn,
                    SessionEventKind::Completed,
                    &json!({ "text": text }),
                )
                .unwrap()
        };
        for turn in 0..100 {
            record(&turn.to_string(), "done");
            record(&turn.to_string(), "done");
        }
        // A receipt that says something else is a gap, never a second answer.
        record("0", "other");
        let events = store.session_history("conversation", 0, 0).unwrap();
        let completions: Vec<_> = events
            .iter()
            .filter(|event| event.kind == SessionEventKind::Completed)
            .collect();
        assert_eq!(completions.len(), 100);
        assert!(completions
            .iter()
            .all(|event| event.payload["text"] == "done"));
        // Provider turns live only in the history file.
        let kept: i64 = store
            .conn
            .lock()
            .unwrap()
            .query_row(
                "SELECT COUNT(*) FROM session_events WHERE kind!='captured'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(kept, 0);
    }

    /// Every Session history kind is read from `history.db` only: rows an
    /// older lf left in `loopflow.db` are invisible until the move pass
    /// carries them over, and new rows never land in the store.
    #[test]
    fn history_kinds_are_read_from_the_history_file_only() {
        let home = tempfile::tempdir().unwrap();
        let store = crate::test_fixture::open(&home.path().join("store.db")).unwrap();
        let input = crate::session_record::new_artifact_key();
        let session = store.test_session("conversation", &input);
        let observed = |source: &str| crate::session::SessionObservation {
            artifact_key: input.clone(),
            source: source.into(),
            observed_at: 5,
            task_id: None,
            wave_id: None,
            payload: json!({"source": source, "evidence": {"since": 1, "observed_at": 5}}),
        };
        let left_in_store = || -> i64 {
            store
                .conn
                .lock()
                .unwrap()
                .query_row(
                    "SELECT count(*) FROM session_events WHERE kind!='captured'",
                    [],
                    |row| row.get(0),
                )
                .unwrap()
        };
        // What a released lf wrote into the store.
        {
            let conn = store.conn.lock().unwrap();
            for (thread, turn, kind, receipt) in [
                (None, None, "observed", format!("{input}:events.jsonl:1")),
                (None, None, "observed", format!("{input}:silence:2")),
                (None, None, "observed", "provider-transition:old".to_owned()),
                (None, None, "observed", "legacy_review_feedback".to_owned()),
                (Some("thread"), Some("old"), "started", String::new()),
                (Some("thread"), Some("old"), "completed", String::new()),
                (Some("thread"), Some("old"), "usage", String::new()),
            ] {
                conn.execute(
                    "INSERT INTO session_events(session_id,provider_thread,provider_turn,kind,receipt_key,observed_at,payload,captured_event)
                     VALUES('conversation',?1,?2,?3,?4,1,'{\"type\":\"provider_transition\",\"summary\":\"old\",\"status\":\"completed\"}',?5)",
                    rusqlite::params![thread, turn, kind, receipt, session.captured],
                )
                .unwrap();
            }
        }
        let old = AgentSessionId::from("thread");
        assert!(store
            .session_history("conversation", 0, 0)
            .unwrap()
            .is_empty());
        assert!(store.input_events(&input).unwrap().is_empty());
        assert!(store.input_silences(&input).unwrap().is_empty());
        assert!(store
            .provider_transitions("conversation")
            .unwrap()
            .is_empty());
        assert!(!store
            .session_turn_completed("conversation", &old, "old")
            .unwrap());
        assert_eq!(
            store
                .recorded_turn_end("conversation", &old, "old")
                .unwrap(),
            None
        );
        assert_eq!(
            store
                .session("conversation")
                .unwrap()
                .unwrap()
                .ready_summary,
            None
        );
        assert_eq!(store.input_answered_turn(&input).unwrap(), None);

        // What this lf writes goes to the history file and is read from it.
        for source in ["events.jsonl:1", "silence:2"] {
            store
                .retain_session_observation(&session, &observed(source))
                .unwrap();
        }
        crate::test_fixture::record_captured_start(&store, "conversation", "new");
        for (kind, payload) in [
            (
                SessionEventKind::Usage,
                json!({"total": {"inputTokens": 1}}),
            ),
            (SessionEventKind::Completed, json!({"status": "completed"})),
        ] {
            store
                .record_session_event("conversation", &old, "new", kind, &payload)
                .unwrap();
        }
        assert_eq!(left_in_store(), 7);
        let kinds: Vec<SessionEventKind> = store
            .session_history("conversation", 0, 0)
            .unwrap()
            .into_iter()
            .map(|event| event.kind)
            .collect();
        assert_eq!(
            kinds,
            [
                SessionEventKind::Observed,
                SessionEventKind::Observed,
                SessionEventKind::Started,
                SessionEventKind::Usage,
                SessionEventKind::Completed
            ]
        );
        assert_eq!(store.input_events(&input).unwrap().len(), 1);
        assert_eq!(store.input_silences(&input).unwrap(), [(1, 5)]);
        assert!(store
            .session_turn_completed("conversation", &old, "new")
            .unwrap());
        assert_eq!(
            store
                .input_answered_turn(&input)
                .unwrap()
                .map(|turn| turn.turn),
            Some("new".to_owned())
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
                    "SELECT COUNT(*) FROM history.session_history WHERE session_id='session'
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
                "SELECT payload FROM history.session_history WHERE kind='observed'
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
    fn provider_identity_is_the_newest_published_reference() {
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
        // Account and thread facts in the event stream are not a resume
        // reference; only a published `provider-session:*` is.
        retain(
            "events.jsonl:2",
            json!({"schema_version":1,"seq":2,"observed_at":"2026-09-29T00:00:00Z",
                "type":"provider_session_observed","attempt_key":"second","provider_session_id":"second-thread"}),
        );
        assert_eq!(store.input_provider_session(&input).unwrap(), None);
        // A legacy `provider-session.json` import is not a publication and
        // cannot replace the newest one, including its unknown account.
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
