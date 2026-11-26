use rusqlite::{params, OptionalExtension, TransactionBehavior};
use serde_json::Value;

use crate::session::{SessionEvent, SessionEventKind};
use crate::session_record::{ProviderSessionRef, SessionHistory};
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

/// How an input's newest ended turn went, as its captured row keeps it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InputTurnEnd {
    pub status: Option<String>,
    /// What failed the turn, from the provider or from lf.
    pub error: Option<String>,
}

/// A Session's current provider conversation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CurrentConversation {
    pub provider: String,
    pub thread: AgentSessionId,
}

#[derive(Debug)]
pub struct TurnSpan {
    pub turn: String,
    pub started_at: Option<i64>,
    pub ended_at: Option<i64>,
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
            let mut query = conn.prepare_cached(
                "SELECT t.provider_thread,t.provider_turn,t.started_at,t.ended_at,t.status,
                        json_extract(c.payload,'$.provider')
                 FROM history.turns t
                 LEFT JOIN session_events c ON c.seq=t.captured_event AND c.kind='captured'
                 WHERE t.session_id=?1 ORDER BY t.id",
            )?;
            let mut rows = query.query([session])?;
            while let Some(row) = rows.next()? {
                let thread: String = row.get(0)?;
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
                facts.turns.push(TurnFacts {
                    turn: row.get(1)?,
                    started_at: row.get(2)?,
                    settled_at: row.get(3)?,
                    status: row.get(4)?,
                });
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
        let mut account = conn.prepare_cached(
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

    /// When turn `turn` of `thread` ended, and its status, while it is the
    /// newest ended turn of the input it answered.
    pub fn recorded_turn_end(
        &self,
        session: &str,
        thread: &AgentSessionId,
        turn: &str,
    ) -> StoreResult<Option<(i64, Option<String>)>> {
        let conn = self.conn.lock().expect("store mutex poisoned");
        Ok(conn
            .query_row(
                "SELECT turn_ended_at,turn_status FROM session_events
                 WHERE session_id=?1 AND turn_thread=?2 AND turn_id=?3
                   AND kind='captured' AND turn_id IS NOT NULL
                 ORDER BY turn_ended_at DESC LIMIT 1",
                params![session, thread, turn],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .optional()?)
    }

    /// Record that `turn` of `thread`, answering input `captured`, ended at
    /// `at` with the provider's `detail`, as the input's newest ended turn. A
    /// turn that ended earlier than the one kept leaves it. The turn's own
    /// error replaces the row's; a turn that ended without one keeps the error
    /// lf saw on the stream, unless it completed.
    pub fn record_answered_turn_end(
        &self,
        captured: i64,
        thread: &AgentSessionId,
        turn: &str,
        at: i64,
        detail: &Value,
    ) -> StoreResult<()> {
        let error = super::turns::turn_error(detail).map(|error| {
            match (
                error.as_str(),
                error["code"].as_str(),
                error["message"].as_str(),
            ) {
                (Some(text), _, _) => text.to_owned(),
                // Read as lf's stream error for the same turn reads.
                (None, Some(code), Some(message)) => format!("{code}: {message}"),
                _ => error.to_string(),
            }
        });
        let conn = self.conn.lock().expect("store mutex poisoned");
        conn.execute(
            "UPDATE session_events SET turn_thread=?2,turn_id=?3,turn_ended_at=?4,turn_status=?5,
                turn_error=CASE WHEN ?6 IS NOT NULL THEN ?6
                                WHEN ?5='completed' THEN NULL ELSE turn_error END
             WHERE seq=?1 AND kind='captured' AND (turn_ended_at IS NULL OR turn_ended_at<=?4)",
            params![captured, thread, turn, at, detail["status"].as_str(), error],
        )?;
        Ok(())
    }

    /// Record what failed `input`'s turn, as lf saw it on the provider's
    /// stream.
    pub fn record_input_error(&self, input: &str, error: &str) -> StoreResult<()> {
        let conn = self.conn.lock().expect("store mutex poisoned");
        conn.execute(
            "UPDATE session_events SET turn_error=?2 WHERE kind='captured' AND receipt_key=?1",
            params![input, error],
        )?;
        Ok(())
    }

    /// How `input`'s newest ended turn went; `None` before one ended or
    /// failed.
    pub fn input_turn_end(&self, input: &str) -> StoreResult<Option<InputTurnEnd>> {
        let conn = self.conn.lock().expect("store mutex poisoned");
        Ok(conn
            .query_row(
                "SELECT turn_status,turn_error FROM session_events
                 WHERE kind='captured' AND receipt_key=?1
                   AND (turn_status IS NOT NULL OR turn_error IS NOT NULL)",
                [input],
                |row| {
                    Ok(InputTurnEnd {
                        status: row.get(0)?,
                        error: row.get(1)?,
                    })
                },
            )
            .optional()?)
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
        let mut query = conn.prepare_cached(
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
        let mut query = conn.prepare_cached(
            "SELECT e.payload FROM session_events c
             JOIN history.session_history e ON c.seq=e.captured_event AND c.kind='captured' AND c.receipt_key=?1
             WHERE e.kind='events'
             ORDER BY CAST(substr(e.receipt_key,length(?1)+15) AS INTEGER),e.seq",
        )?;
        let rows = query.query_map([input], |row| row.get::<_, String>(0))?;
        rows.map(|row| Ok(serde_json::from_str::<Value>(&row?)?["evidence"].clone()))
            .collect()
    }

    /// Each silence the recorder observed in one input, as `(since, until)`.
    pub fn input_silences(&self, input: &str) -> StoreResult<Vec<(i64, i64)>> {
        let conn = self.conn.lock().expect("store mutex poisoned");
        let mut query = conn.prepare_cached(
            "SELECT json_extract(e.payload,'$.evidence.since'),json_extract(e.payload,'$.evidence.observed_at')
             FROM session_events c
             JOIN history.session_history e ON c.seq=e.captured_event AND c.kind='captured' AND c.receipt_key=?1
             WHERE e.kind='silence' AND json_extract(e.payload,'$.evidence.observed_at') IS NOT NULL
             ORDER BY CAST(substr(e.receipt_key,length(?1)+10) AS INTEGER),e.seq",
        )?;
        let rows = query.query_map([input], |row| Ok((row.get(0)?, row.get(1)?)))?;
        Ok(rows.collect::<Result<_, _>>()?)
    }

    /// Each silence the recorder observed in one history row's input, as
    /// `(since, until)`; a collected input's are its origin's copies.
    pub fn history_silences(&self, history: &SessionHistory) -> StoreResult<Vec<(i64, i64)>> {
        match (&history.artifact_key, &history.origin, history.captured) {
            (Some(input), None, _) => self.input_silences(input),
            (Some(_), Some(origin), Some(captured)) => {
                self.copied_input_silences(&origin.machine_id, &history.session_id, captured)
            }
            _ => Ok(Vec::new()),
        }
    }

    /// What filled the context of each ended provider turn one history row's
    /// input started, in ending order; `None` for a turn completed before
    /// turns retained it. A collected input's turns are its origin's copies.
    pub fn history_turn_context(
        &self,
        history: &SessionHistory,
    ) -> StoreResult<Vec<Option<crate::session::TurnContext>>> {
        match (&history.artifact_key, &history.origin, history.captured) {
            (Some(input), None, _) => self.input_turn_context(input),
            (Some(_), Some(origin), Some(captured)) => {
                self.copied_input_turn_context(&origin.machine_id, &history.session_id, captured)
            }
            _ => Ok(Vec::new()),
        }
    }

    fn input_turn_context(
        &self,
        input: &str,
    ) -> StoreResult<Vec<Option<crate::session::TurnContext>>> {
        let conn = self.conn.lock().expect("store mutex poisoned");
        let mut query = conn.prepare(&format!(
            "SELECT {} FROM history.turns t
             WHERE t.captured_event=(SELECT seq FROM session_events WHERE kind='captured' AND receipt_key=?1)
               AND t.ended_at IS NOT NULL
             ORDER BY t.ended_at,t.id",
            super::turns::turn_columns("t")
        ))?;
        let rows = query.query_map([input], super::turns::read_turn)?;
        rows.map(|row| Ok(row??.context)).collect()
    }

    /// The provider turn that answered `input`: its newest ended turn, when
    /// that turn completed. Its text is in the provider's transcript. The
    /// capture names the agent the input was first sent to; when a refusal
    /// moved the input to another, the Session names the one whose thread
    /// answered.
    pub fn input_answered_turn(&self, input: &str) -> StoreResult<Option<AnsweredTurn>> {
        let conn = self.conn.lock().expect("store mutex poisoned");
        Ok(conn
            .query_row(
                "SELECT c.session_id,
                        CASE WHEN c.turn_thread=s.provider_thread THEN s.provider
                             ELSE COALESCE(json_extract(c.payload,'$.provider'),s.provider) END,
                        c.turn_thread,c.turn_id,COALESCE(json_extract(c.payload,'$.cwd'),s.cwd)
                 FROM session_events c JOIN agent_sessions s ON s.id=c.session_id
                 WHERE c.kind='captured' AND c.receipt_key=?1 AND c.turn_status='completed'
                   AND c.turn_thread IS NOT NULL AND c.turn_id IS NOT NULL",
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
             SELECT ?1,?2,?3,?4,?5,?6,?7,?8 FROM agent_sessions s WHERE s.id=?1 AND s.attachment_token=?2 AND s.agent_process_id=?7
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
                 WHERE act.session_id=?1 AND act.attachment_token=s.attachment_token
                 AND act.agent_process_id=s.agent_process_id",
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

    /// Whether history already keeps the turn's completion; a provider
    /// snapshot replays completions of turns it has finished before, and the
    /// replay must not take the live turn's tally. It guards history's
    /// numbers, so it reads history: a turn with no known input never
    /// reaches an input's row.
    pub fn session_turn_completed(
        &self,
        session: &str,
        thread: &AgentSessionId,
        turn: &str,
    ) -> StoreResult<bool> {
        let conn = self.conn.lock().expect("store mutex poisoned");
        Ok(conn
            .query_row(
                "SELECT 1 FROM history.turns WHERE session_id=?1 AND provider_thread=?2
                 AND provider_turn=?3 AND ended_at IS NOT NULL",
                params![session, thread, turn],
                |_| Ok(()),
            )
            .optional()?
            .is_some())
    }

    /// Each turn `history.turns` keeps in `thread`: its name, when it started
    /// and when it ended. The transcript pass reads it to find the turn a
    /// transcript turn was recorded as, and to fill the ends history missed.
    pub fn session_turn_spans(
        &self,
        session: &str,
        thread: &AgentSessionId,
    ) -> StoreResult<Vec<TurnSpan>> {
        let conn = self.conn.lock().expect("store mutex poisoned");
        let mut query = conn.prepare_cached(
            "SELECT provider_turn,started_at,ended_at FROM history.turns
             WHERE session_id=?1 AND provider_thread=?2",
        )?;
        let rows = query.query_map(params![session, thread], |row| {
            Ok(TurnSpan {
                turn: row.get(0)?,
                started_at: row.get(1)?,
                ended_at: row.get(2)?,
            })
        })?;
        rows.map(|row| row.map_err(StoreError::from)).collect()
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
            "SELECT s.task_id,s.wave_id,s.current_capture,t.project_id,s.repo,
                    COALESCE(json_extract(c.payload,'$.skill'),s.skill),
                    COALESCE(json_extract(c.payload,'$.provider'),s.provider),g.graph,
                    (SELECT json_extract(step.value,'$.label') FROM json_each(g.compiled,'$.steps') step
                     WHERE json_extract(step.value,'$.key')=gs.node)
             FROM agent_sessions s
             LEFT JOIN tasks t ON t.id=s.task_id
             LEFT JOIN session_events c ON c.seq=s.current_capture
             LEFT JOIN graph_process_steps gs ON gs.lf_process_id=c.lf_process_id
             LEFT JOIN graph_processes g ON g.lf_process_id=gs.graph_lf_process_id
             WHERE s.id=?1 AND s.attachment_token=?2 AND s.attached_lf_process_id=?3
               AND s.agent_process_id=?4",
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
                    project_id: row.get(3)?,
                    repository: row.get(4)?,
                    skill: row.get(5)?,
                    provider: row.get(6)?,
                    graph: row.get(7)?,
                    step: row.get(8)?,
                })
            },
        )
        .optional()?
        .ok_or_else(|| StoreError::InvalidAuthority("Session attachment changed".into()))
    }

    pub fn session_history(
        &self,
        session: &str,
        after: i64,
        limit: usize,
    ) -> StoreResult<Vec<SessionEvent>> {
        self.read_history(session, after, limit, None, (None, None, None), None)
    }

    /// Keep provenance and unknown evidence; transcript payloads belong to detail.
    pub(super) fn summary_for_input_matching(
        &self,
        session: &str,
        input: &str,
        scope: (Option<&str>, Option<&str>, Option<&str>),
        origin: Option<&loopflow_model::durable::MachineId>,
    ) -> StoreResult<Vec<SessionEvent>> {
        self.read_history(session, 0, 0, Some(input), scope, origin)
    }

    /// The turns of `session` the scope selects: those `input` started, or
    /// with no input, the one unattributed turn `orphan` names.
    pub(super) fn turns_matching(
        &self,
        session: &str,
        input: Option<&str>,
        orphan: (Option<&AgentSessionId>, Option<&str>),
        scope: (Option<&str>, Option<&str>, Option<&str>),
    ) -> StoreResult<Vec<crate::session::Turn>> {
        let task = scope.2.map(loopflow_model::id::TaskSelector::parse);
        let conn = self.conn.lock().expect("store mutex poisoned");
        let mut query = conn.prepare(&format!(
            "SELECT {} FROM history.turns t
             WHERE t.session_id=?1
             AND CASE WHEN ?2 IS NULL THEN t.captured_event IS NULL
                     AND t.provider_thread IS ?3 AND t.provider_turn IS ?4
                 ELSE t.captured_event=(SELECT seq FROM session_events WHERE kind='captured' AND receipt_key=?2) END
             AND (?5 IS NULL OR t.wave_id IN (SELECT id FROM wave_addresses WHERE id=?8 OR slug=?5))
             AND (?6 IS NULL OR t.task_id IN (SELECT tk.id FROM projects p CROSS JOIN tasks tk ON tk.project_id=p.id
                 WHERE p.id=?9 OR p.project_slug=?6 OR p.external_project_id=?10))
             AND (?7 IS NULL OR t.task_id IN (SELECT id FROM tasks WHERE id=?11 OR issue_identifier=?12 OR external_issue_id=?13))
             ORDER BY t.id",
            super::turns::turn_columns("t")
        ))?;
        let rows = query.query_map(
            params![
                loopflow_model::id::canonical_session(session),
                input,
                orphan.0,
                orphan.1,
                scope.0,
                scope.1,
                scope.2,
                scope.0.map(loopflow_model::id::canonical_uuid),
                scope
                    .1
                    .map(|project| loopflow_model::id::canonical_prefixed(project, "proj_")),
                scope.1.map(loopflow_model::id::canonical_uuid),
                task.as_ref().map(|task| &task.id),
                task.as_ref().map(|task| &task.label),
                task.as_ref().map(|task| &task.external),
            ],
            super::turns::read_turn,
        )?;
        rows.map(|row| row?).collect()
    }

    fn read_history(
        &self,
        session: &str,
        after: i64,
        limit: usize,
        input: Option<&str>,
        scope: (Option<&str>, Option<&str>, Option<&str>),
        origin: Option<&loopflow_model::durable::MachineId>,
    ) -> StoreResult<Vec<SessionEvent>> {
        let task = scope.2.map(loopflow_model::id::TaskSelector::parse);
        let conn = self.conn.lock().expect("store mutex poisoned");
        // A copy keeps its origin's rows in the shape `history.session_history` has.
        let source = match origin {
            None => "(SELECT seq,session_id,receipt_key,kind,lf_process_id,task_id,wave_id,observed_at,payload
                 FROM history.session_history WHERE ?15 IS NULL)",
            Some(_) => {
                "(SELECT json_extract(row,'$.seq') AS seq,json_extract(row,'$.session_id') AS session_id,
                    json_extract(row,'$.receipt_key') AS receipt_key,json_extract(row,'$.kind') AS kind,
                    json_extract(row,'$.lf_process_id') AS lf_process_id,json_extract(row,'$.task_id') AS task_id,
                    json_extract(row,'$.wave_id') AS wave_id,json_extract(row,'$.observed_at') AS observed_at,
                    json_extract(row,'$.payload') AS payload
                 FROM machine_records WHERE origin_machine_id=?15 AND kind='history'
                    AND json_extract(row,'$.session_id')=?8)"
            }
        };
        // Observations answer to their own scope. Inputs are Session state in
        // the store and turns are in `history.turns`, not history.
        let mut query = conn.prepare(&format!(
            "SELECT e.seq,e.session_id,NULL,NULL,
                    NULL,e.lf_process_id,e.task_id,e.wave_id,e.observed_at,e.payload
             FROM {source} e
             WHERE e.session_id=?8 AND e.seq>?2
             AND (?4 IS NULL OR substr(e.receipt_key,1,length(?4)+1)=?4||':')
             AND (e.kind IN ('manifest','runs','terminal') OR (
                 (?5 IS NULL OR e.wave_id IN (SELECT id FROM wave_addresses WHERE id=?9 OR slug=?5))
                 AND (?6 IS NULL OR e.task_id IN (SELECT t.id FROM projects p CROSS JOIN tasks t ON t.project_id=p.id
                         WHERE p.id=?10 OR p.project_slug=?6 OR p.external_project_id=?11))
                 AND (?7 IS NULL OR e.task_id IN (SELECT id FROM tasks WHERE id=?12 OR issue_identifier=?13 OR external_issue_id=?14))))
             ORDER BY e.seq LIMIT ?3"
        ))?;
        let rows = query.query_map(
            params![
                session,
                after,
                if limit == 0 { -1 } else { limit as i64 },
                input,
                scope.0,
                scope.1,
                scope.2,
                loopflow_model::id::canonical_session(session),
                scope.0.map(loopflow_model::id::canonical_uuid),
                scope
                    .1
                    .map(|project| loopflow_model::id::canonical_prefixed(project, "proj_")),
                scope.1.map(loopflow_model::id::canonical_uuid),
                task.as_ref().map(|task| &task.id),
                task.as_ref().map(|task| &task.label),
                task.as_ref().map(|task| &task.external),
                origin.map(|origin| origin.as_str()),
            ],
            |row| {
                Ok((
                    row.get::<_, i64>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, Option<AgentSessionId>>(2)?,
                    row.get::<_, Option<String>>(3)?,
                    row.get::<_, Option<String>>(4)?,
                    row.get::<_, Option<String>>(5)?,
                    row.get::<_, Option<String>>(6)?,
                    row.get::<_, Option<String>>(7)?,
                    row.get::<_, i64>(8)?,
                    row.get::<_, String>(9)?,
                ))
            },
        )?;
        rows.map(|row| {
            let (
                seq,
                session_id,
                agent_session,
                provider_turn,
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
                kind: SessionEventKind::Observed,
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

#[cfg(test)]
mod tests {

    use crate::session::SessionEventKind;
    use crate::sqlite::SqliteStore;
    use crate::RecordingGap;
    use loopflow_model::id::AgentSessionId;
    use serde_json::json;

    #[test]
    fn a_replayed_completion_keeps_one_turn() {
        let home = tempfile::tempdir().unwrap();
        let store = crate::test_fixture::open(&home.path().join("store.db")).unwrap();
        store.test_session("conversation", &crate::session_record::new_artifact_key());
        let record = |turn: &str, status: &str| {
            store.record_turn_completion(
                "conversation",
                &"Provider-Thread".into(),
                turn,
                1,
                &json!({ "status": status }),
            )
        };
        for turn in 0..100 {
            record(&turn.to_string(), "completed");
            record(&turn.to_string(), "completed");
        }
        // A completion that says something else is a gap, never a second end.
        record("0", "failed");
        let turns = store.session_turns("conversation").unwrap();
        assert_eq!(turns.len(), 100);
        assert!(turns
            .iter()
            .all(|turn| turn.status.as_deref() == Some("completed")));
        // Provider turns live in neither the store nor Session history.
        let conn = store.conn.lock().unwrap();
        let count = |sql: &str| -> i64 { conn.query_row(sql, [], |row| row.get(0)).unwrap() };
        assert_eq!(
            count("SELECT COUNT(*) FROM session_events WHERE kind!='captured'"),
            0
        );
        assert_eq!(count("SELECT COUNT(*) FROM history.session_history"), 0);
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
        assert_eq!(store.input_answered_turn(&input).unwrap(), None);

        // What this lf writes goes to the history file and is read from it.
        for source in ["events.jsonl:1", "silence:2"] {
            store
                .retain_session_observation(&session, &observed(source))
                .unwrap();
        }
        crate::test_fixture::record_captured_start(&store, "conversation", "new");
        store.record_turn_usage(
            "conversation",
            &old,
            "new",
            2,
            &json!({"total": {"inputTokens": 1}}),
        );
        store.record_turn_completion(
            "conversation",
            &old,
            "new",
            3,
            &json!({"status": "completed"}),
        );
        assert_eq!(left_in_store(), 7);
        let kinds: Vec<SessionEventKind> = store
            .session_history("conversation", 0, 0)
            .unwrap()
            .into_iter()
            .map(|event| event.kind)
            .collect();
        assert_eq!(
            kinds,
            [SessionEventKind::Observed, SessionEventKind::Observed]
        );
        let turns = store.session_turns("conversation").unwrap();
        assert_eq!(turns.len(), 1);
        assert_eq!(turns[0].turn, "new");
        assert_eq!(store.input_events(&input).unwrap().len(), 1);
        assert_eq!(store.input_silences(&input).unwrap(), [(1, 5)]);
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
                     AND json_extract(payload,'$.source')='store-gap'",
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
                "SELECT payload FROM history.session_history
                 WHERE json_extract(payload,'$.source')='store-gap'",
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
                None,
                &crate::sqlite::SessionCaller {
                    agent: None,
                    capture_key: None,
                },
            )
            .unwrap();
        store.record_session_turn_origin(
            &"thread".into(),
            "turn",
            &store.session_turn_origin(&session.id, &attachment).unwrap(),
        );
        let mut replacement = session.clone();
        replacement.artifact_key = Some(crate::session_record::new_artifact_key());
        store
            .replace_session_input_prepared(session.captured, replacement.clone(), None, None)
            .unwrap();
        for turn in ["turn", "unknown"] {
            store.record_turn_usage(
                &session.id,
                &"thread".into(),
                turn,
                2,
                &json!({"total":{"inputTokens":20,"outputTokens":5}}),
            );
        }
        let none = (None, None);
        let turns = store
            .turns_matching(&session.id, Some(&input), none, (None, None, None))
            .unwrap();
        assert_eq!(turns.len(), 1);
        assert!(turns
            .iter()
            .all(|turn| turn.lf_process_id.as_deref() == Some(owner.as_str())
                && turn.agent_process_id.as_deref() == Some(attachment.agent_process_id.as_str())));
        assert!(store
            .turns_matching(
                &session.id,
                Some(replacement.input_key().unwrap()),
                none,
                (None, None, None)
            )
            .unwrap()
            .is_empty());
        let unknown = store
            .turns_matching(
                &session.id,
                None,
                (Some(&"thread".into()), Some("unknown")),
                (None, None, None),
            )
            .unwrap();
        assert_eq!(unknown.len(), 1);
        assert_eq!(unknown[0].lf_process_id, None);
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
