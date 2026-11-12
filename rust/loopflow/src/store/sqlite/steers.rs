//! Context callbacks carry provider authority, independently of the attached client.
use std::path::Path;

use rusqlite::{params, Connection, OptionalExtension, TransactionBehavior};
use sha2::{Digest, Sha256};

use super::SqliteStore;
use crate::process::SessionAttachment;
use crate::steer::{
    IdleConversation, IdleReach, NativeBoundary, SessionSteerPage, SessionSteerStatus, Steer,
    SteerAdapter, SteerContext,
};
use crate::store::{StoreError, StoreResult};

impl SqliteStore {
    pub(crate) fn reserve_steer_adapter(
        &self,
        session: &str,
        expected: &SessionAttachment,
    ) -> StoreResult<SteerAdapter> {
        self.with_session_attachment(session, expected, || {
            let conn = self.conn.lock().expect("store mutex poisoned");
            conn.execute(
                "INSERT INTO steer_adapters(agent_process_id,capability,created_at)
                 VALUES(?1,?2,unixepoch()) ON CONFLICT DO NOTHING",
                params![expected.agent_process_id, uuid::Uuid::new_v4().to_string()],
            )?;
            Ok(SteerAdapter {
                session: session.into(),
                agent_process_id: expected.agent_process_id.clone(),
                capability: conn.query_row(
                    "SELECT capability FROM steer_adapters WHERE agent_process_id=?1",
                    [&expected.agent_process_id],
                    |row| row.get(0),
                )?,
            })
        })
    }

    /// The Session's selected native conversation is the one record callbacks
    /// follow. A provider start in the owned process selects; activation sets
    /// only a missing selection; every other boundary must match it.
    pub(crate) fn with_provider_context<T>(
        &self,
        adapter: &SteerAdapter,
        thread: &str,
        boundary: NativeBoundary<'_>,
        write: impl FnOnce() -> StoreResult<T>,
    ) -> StoreResult<T> {
        let _fence = self.lock_session_dispatch(&adapter.session)?;
        // A provider start that changes conversation also moves the resume
        // reference, on the account it already names.
        let resume = match boundary {
            NativeBoundary::Select { .. } => self.resume_reference(&adapter.session)?,
            _ => None,
        };
        // A turn in the selected, activated, already observed conversation
        // changes nothing; it takes no write lock. Any other answer goes
        // through the transaction, which checks again.
        if matches!(boundary, NativeBoundary::Turn) && !thread.is_empty() {
            let conn = self.conn.lock().expect("store mutex poisoned");
            let current: Option<bool> = conn
                .query_row(
                    "SELECT s.provider_thread IS ?4 AND a.activated_at IS NOT NULL FROM steer_adapters a
                 JOIN processes p ON p.id=a.agent_process_id
                 JOIN agent_sessions s ON s.agent_process_id=p.id
                 WHERE s.id=?1 AND p.id=?2 AND a.capability=?3 AND p.completed_at IS NULL",
                    params![
                        adapter.session,
                        adapter.agent_process_id,
                        adapter.capability,
                        thread
                    ],
                    |row| row.get(0),
                )
                .optional()?;
            if current == Some(true)
                && super::provider_transitions::observed_in(&conn, &adapter.session, thread)?
            {
                drop(conn);
                return write();
            }
        }
        {
            let mut conn = self.conn.lock().expect("store mutex poisoned");
            let tx = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
            let selected: Option<Option<String>> = tx
                .query_row(
                    "SELECT s.provider_thread FROM steer_adapters a
                 JOIN processes p ON p.id=a.agent_process_id
                 JOIN agent_sessions s ON s.agent_process_id=p.id
                 WHERE s.id=?1 AND p.id=?2 AND a.capability=?3 AND p.completed_at IS NULL",
                    params![
                        adapter.session,
                        adapter.agent_process_id,
                        adapter.capability
                    ],
                    |row| row.get(0),
                )
                .optional()?;
            let selected = selected.ok_or_else(|| {
                StoreError::InvalidAuthority("Context adapter no longer owns this provider".into())
            })?;
            if thread.is_empty() {
                return Err(StoreError::InvalidAuthority(
                    "Context callback names no native conversation".into(),
                ));
            }
            match (boundary, selected.as_deref()) {
                (_, Some(current)) if current == thread => {}
                (NativeBoundary::Select { .. }, _) | (NativeBoundary::Activate { .. }, None) => {
                    tx.execute(
                        "UPDATE agent_sessions SET provider_thread=?2 WHERE id=?1",
                        params![adapter.session, thread],
                    )?;
                }
                (_, Some(current)) => {
                    return Err(StoreError::InvalidAuthority(format!(
                        "Context callback names native conversation {thread}; this Session selects {current}"
                    )));
                }
                (NativeBoundary::Turn, None) => {
                    return Err(StoreError::InvalidAuthority(format!(
                        "Context callback names native conversation {thread}; this Session selects none yet"
                    )));
                }
            }
            let source = match boundary {
                NativeBoundary::Select { source } | NativeBoundary::Activate { source } => source,
                _ => "observed",
            };
            super::provider_transitions::observe_in(&tx, &adapter.session, thread, source)?;
            if let Some((session, input, reference)) =
                resume.filter(|(_, _, reference)| reference.agent_session.as_str() != thread)
            {
                super::sessions::retain_history_in(
                    &tx,
                    &session,
                    &[crate::session_record::provider_session_observation(
                        &session,
                        &input,
                        &thread.into(),
                        reference.account_id,
                    )],
                )?;
            }
            tx.execute(
                "UPDATE steer_adapters SET activated_at=COALESCE(activated_at,unixepoch()) WHERE agent_process_id=?1",
                [&adapter.agent_process_id],
            )?;
            tx.commit()?;
        }
        write()
    }

    /// Native hooks report turn boundaries and work that outlives a reply.
    /// A retained provider capability follows attachment handoff, but cannot
    /// report for a replacement provider or a different native conversation.
    pub(crate) fn record_native_turn(
        &self,
        adapter: &SteerAdapter,
        thread: &str,
        yielded: bool,
        open_tools: Option<usize>,
    ) -> StoreResult<()> {
        self.with_provider_context(adapter, thread, NativeBoundary::Turn, || {
            let (native, previous_work): (bool, i64) =
                self.conn.lock().expect("store mutex poisoned").query_row(
                    "SELECT s.interactive, COALESCE(act.open_tools,0)
                     FROM processes p JOIN agent_sessions s ON s.id=?2
                     LEFT JOIN session_activity act
                       ON act.agent_process_id=p.id AND act.session_id=s.id
                     WHERE p.id=?1",
                    params![adapter.agent_process_id, adapter.session],
                    |row| Ok((row.get(0)?, row.get(1)?)),
                )?;
            // A headless run reports through the stream lf owns. A person's
            // conversation reports here, Codex's included: its AgentProcess is
            // an app-server lf spawned, but a native client takes the turns.
            if !native {
                return Ok(());
            }
            let attachment = self.session_attachment(&adapter.session)?.ok_or_else(|| {
                StoreError::InvalidAuthority("Native provider has no attachment".into())
            })?;
            self.record_session_activity(
                &adapter.session,
                &attachment,
                &crate::session::SessionActivity {
                    preview: None,
                    observed_at: time::OffsetDateTime::now_utc().unix_timestamp(),
                    // An unavailable task registry cannot settle known work.
                    open_tools: open_tools.unwrap_or(previous_work as usize),
                    pending_input: 0,
                    yielded,
                },
            )
        })
    }

    /// Save where an idle Claude conversation takes messages: the inbox key
    /// file its hook found. Written only when it changes.
    pub(crate) fn record_agent_inbox(&self, adapter: &SteerAdapter, key: &Path) -> StoreResult<()> {
        let conn = self.conn.lock().expect("store mutex poisoned");
        conn.execute(
            "UPDATE processes SET inbox=?3 WHERE id=?2 AND inbox IS NOT ?3 AND EXISTS(
                SELECT 1 FROM steer_adapters a JOIN agent_sessions s ON s.agent_process_id=a.agent_process_id
                WHERE s.id=?1 AND a.agent_process_id=?2 AND a.capability=?4)",
            params![
                adapter.session,
                adapter.agent_process_id,
                key.to_string_lossy(),
                adapter.capability
            ],
        )?;
        Ok(())
    }

    /// Managed conversations whose last boundary was a turn end, on a live
    /// AgentProcess the server can reach; only `session` when given. Driven
    /// from unfinished processes: every Session ever recorded is far more.
    pub(crate) fn idle_conversations(
        &self,
        session: Option<&str>,
    ) -> StoreResult<Vec<IdleConversation>> {
        let conn = self.conn.lock().expect("store mutex poisoned");
        let mut statement = conn.prepare(
            "SELECT s.id,p.id,a.capability,s.provider_thread,s.cwd,
                CASE WHEN p.agent_provider='codex' THEN p.endpoint END,p.inbox,
                s.work_source,p.pid,p.os_started_at
             FROM processes p INDEXED BY processes_unfinished
             JOIN agent_sessions s ON s.agent_process_id=p.id
             JOIN steer_adapters a ON a.agent_process_id=p.id
             JOIN session_activity act ON act.session_id=s.id AND act.agent_process_id=p.id
             WHERE act.yielded=1 AND p.completed_at IS NULL AND s.provider_thread IS NOT NULL
               AND (p.agent_provider='codex' AND p.endpoint IS NOT NULL OR p.inbox IS NOT NULL)
               AND (?1 IS NULL OR s.id=?1)",
        )?;
        let rows = statement.query_map([session], |row| {
            let reach = match row.get::<_, Option<String>>(5)? {
                Some(endpoint) => IdleReach::Codex(endpoint.into()),
                None => IdleReach::Claude(row.get::<_, String>(6)?.into()),
            };
            let source: Option<crate::session::WorkSource> = row
                .get::<_, Option<String>>(7)?
                .map(|source| serde_json::from_value(serde_json::Value::String(source)))
                .transpose()
                .map_err(|error| {
                    rusqlite::Error::FromSqlConversionFailure(
                        7,
                        rusqlite::types::Type::Text,
                        Box::new(error),
                    )
                })?;
            let live = row
                .get::<_, Option<u32>>(8)?
                .zip(row.get::<_, Option<i64>>(9)?)
                .is_some_and(|(pid, start)| {
                    crate::journal::process_identity_evidence(pid, start)
                        == crate::journal::ProcessIdentityEvidence::Live
                });
            let conversation = IdleConversation {
                adapter: SteerAdapter {
                    session: row.get(0)?,
                    agent_process_id: row.get(1)?,
                    capability: row.get(2)?,
                },
                thread: row.get(3)?,
                cwd: row.get::<_, String>(4)?.into(),
                reach,
            };
            Ok(
                (live && source.is_some_and(crate::session::WorkSource::is_managed))
                    .then_some(conversation),
            )
        })?;
        Ok(rows
            .filter_map(Result::transpose)
            .collect::<Result<_, _>>()?)
    }

    /// A woken conversation is in a turn until its next Stop says otherwise.
    pub(crate) fn record_woken(&self, adapter: &SteerAdapter) -> StoreResult<()> {
        let conn = self.conn.lock().expect("store mutex poisoned");
        conn.execute(
            "UPDATE session_activity SET yielded=0,observed_at=unixepoch()
             WHERE session_id=?1 AND agent_process_id=?2",
            params![adapter.session, adapter.agent_process_id],
        )?;
        Ok(())
    }

    /// The Session's published resume reference with the input it belongs to.
    /// A Session that never published one leaves publication to its launch.
    fn resume_reference(
        &self,
        session: &str,
    ) -> StoreResult<
        Option<(
            crate::session::LfSession,
            String,
            crate::session_record::ProviderSessionRef,
        )>,
    > {
        let Some(session) = self.session(session)? else {
            return Ok(None);
        };
        let Some(input) = session.artifact_key.clone() else {
            return Ok(None);
        };
        Ok(self
            .input_provider_session(&input)?
            .map(|reference| (session, input, reference)))
    }
}

fn invalid(message: impl ToString) -> StoreError {
    StoreError::InvalidData(message.to_string())
}

pub(super) fn read_steer(row: &rusqlite::Row<'_>) -> rusqlite::Result<Steer> {
    Ok(Steer {
        id: row.get(0)?,
        sequence: row.get(1)?,
        source: row.get(2)?,
        revision: row.get(3)?,
        author: serde_json::from_str(&row.get::<_, String>(4)?).map_err(|error| {
            rusqlite::Error::FromSqlConversionFailure(
                4,
                rusqlite::types::Type::Text,
                Box::new(error),
            )
        })?,
        text: row.get(5)?,
    })
}
pub(super) const COLUMNS: &str = "p.id,p.seq,p.source,p.revision,p.author,p.text";

/// Callers supply exactly one existing source owner; these columns are private,
/// never a wire target or a second Work hierarchy.
pub(super) fn save_in(
    conn: &Connection,
    owner: (&str, &str),
    source: &str,
    revision: &str,
    author: &crate::durable::Author,
    text: &str,
) -> StoreResult<Steer> {
    if text.trim().is_empty() {
        return Err(invalid("Steer text cannot be empty"));
    }
    let column = match owner.0 {
        "task" => "task_id",
        "wave" => "wave_id",
        "repository" => "repo",
        "session" => "session_id",
        _ => return Err(invalid("Unsupported steer owner")),
    };
    conn.execute(
        &format!(
            "INSERT INTO steers(id,{column},source,revision,author,text,created_at)
        VALUES(?1,?2,?3,?4,?5,?6,unixepoch()) ON CONFLICT(source,revision) DO NOTHING"
        ),
        params![
            uuid::Uuid::new_v4().to_string(),
            owner.1,
            source,
            revision,
            serde_json::to_string(author)?,
            text
        ],
    )?;
    // This transaction holds the write lock, so a hook that checks after this
    // touch waits for the commit before reading pending steers.
    if let Some(home) = conn
        .path()
        .filter(|path| !path.is_empty())
        .and_then(|path| Path::new(path).parent())
    {
        crate::steer::touch(&crate::steer::latest_marker(home))
            .map_err(|error| invalid(format!("Mark saved steer for hooks: {error}")))?;
    }
    let result = conn.query_row(
        &format!("SELECT {COLUMNS} FROM steers p WHERE source=?1 AND revision=?2"),
        params![source, revision],
        read_steer,
    )?;
    let same_owner: bool = conn.query_row(
        &format!("SELECT {column} IS ?2 FROM steers WHERE id=?1"),
        params![result.id, owner.1],
        |row| row.get(0),
    )?;
    if !same_owner || result.text != text || result.author != *author {
        return Err(invalid(
            "Steer revision already has different contents or ownership",
        ));
    }
    Ok(result)
}

/// The steers that apply to Session `?1`, as CTE `applicable`.
fn scopes() -> String {
    // Membership is derived here; receipts alone retain delivery state.
    // Reuse Task inventory's association rule, including explicit binds and
    // checkout membership. UNION terminates even in malformed cyclic ancestry.
    format!("WITH RECURSIVE member_tasks(id) AS MATERIALIZED (
        SELECT tw.id FROM agent_sessions s JOIN tasks tw ON tw.id IN ({}) WHERE s.id=?1
        ), member_waves(id) AS (
        SELECT wave_id FROM agent_sessions WHERE id=?1
        UNION SELECT pr.wave_id FROM projects pr JOIN tasks t ON t.project_id=pr.id WHERE t.id IN member_tasks
        UNION SELECT w.parent_wave_id FROM waves w JOIN member_waves m ON w.id=m.id WHERE w.parent_wave_id IS NOT NULL
        ), applicable(id) AS (
        SELECT p.id FROM steers p WHERE
        (p.session_id=?1 OR p.task_id IN member_tasks OR p.wave_id IN member_waves
         OR p.repo=(SELECT repo FROM agent_sessions WHERE id=?1))
        AND NOT EXISTS(SELECT 1 FROM steers newer WHERE newer.source=p.source AND newer.seq>p.seq)
        -- A shared steer saved before this Session started and already emitted
        -- to another Session was read there; a later Session starts past it.
        AND (p.session_id=?1 OR p.created_at>=(SELECT created_at FROM agent_sessions WHERE id=?1)
         OR NOT EXISTS(SELECT 1 FROM steer_deliveries d JOIN steer_batches b ON b.id=d.batch_id
            WHERE d.steer_id=p.id AND d.session_id!=?1 AND b.evidence='emitted'))
        )", super::task_work::session_tasks("s"))
}

/// Whether some applicable steer has no delivery at its current attempt: the
/// only case in which [`reconcile`] leaves the Session anything to emit.
fn undelivered(conn: &Connection, session: &str) -> StoreResult<bool> {
    Ok(conn.query_row(
        &format!(
            "{} SELECT EXISTS(SELECT 1 FROM applicable a
            LEFT JOIN session_steers s ON s.session_id=?1 AND s.steer_id=a.id
            WHERE NOT EXISTS(SELECT 1 FROM steer_deliveries d WHERE d.session_id=?1
            AND d.steer_id=a.id AND d.attempt=COALESCE(s.attempt,0)))",
            scopes()
        ),
        [session],
        |row| row.get(0),
    )?)
}

fn reconcile(conn: &Connection, session: &str) -> StoreResult<()> {
    let exists: bool = conn.query_row(
        "SELECT EXISTS(SELECT 1 FROM agent_sessions WHERE id=?1)",
        [session],
        |r| r.get(0),
    )?;
    if !exists {
        return Err(StoreError::NotFound);
    }
    let scopes = scopes();
    conn.execute(
        &format!(
            "{scopes} UPDATE session_steers SET applicable=(steer_id IN applicable)
        WHERE session_id=?1"
        ),
        [session],
    )?;
    conn.execute(
        &format!(
            "{scopes} INSERT INTO session_steers(session_id,steer_id)
        SELECT ?1,id FROM applicable WHERE true ON CONFLICT DO NOTHING"
        ),
        [session],
    )?;
    // A revision or association change cancels the whole not-yet-written batch.
    // Its emitter checks the reservation again before exposing any bytes.
    conn.execute(
        "UPDATE steer_batches SET evidence='cancelled'
        WHERE session_id=?1 AND evidence='prepared' AND EXISTS(
            SELECT 1 FROM steer_deliveries d JOIN session_steers s
            ON s.session_id=d.session_id AND s.steer_id=d.steer_id
            WHERE d.batch_id=steer_batches.id AND s.applicable=0)",
        [session],
    )?;
    conn.execute(
        "DELETE FROM steer_deliveries WHERE session_id=?1 AND batch_id IN (
        SELECT id FROM steer_batches WHERE session_id=?1 AND evidence='cancelled')",
        [session],
    )?;
    // Emitted and uncertain receipts survive scope changes and corrections.
    conn.execute(
        "DELETE FROM session_steers WHERE session_id=?1 AND applicable=0
        AND NOT EXISTS(SELECT 1 FROM steer_deliveries d
            WHERE d.session_id=?1 AND d.steer_id=session_steers.steer_id)",
        [session],
    )?;
    Ok(())
}

fn pending_count(conn: &Connection, session: &str) -> StoreResult<i64> {
    Ok(conn.query_row(
        "SELECT count(*) FROM session_steers s WHERE s.session_id=?1 AND s.applicable=1
        AND NOT EXISTS(SELECT 1 FROM steer_deliveries d WHERE d.session_id=s.session_id
        AND d.steer_id=s.steer_id AND d.attempt=s.attempt)",
        [session],
        |row| row.get(0),
    )?)
}

impl SqliteStore {
    /// Every emitter holds a shared Session fence from reservation through write.
    /// Acquiring it exclusively proves no prepared emitter survives; elapsed time
    /// and provider liveness cannot establish that fact.
    pub(crate) fn recover_prepared_steers(&self, session: &str) -> StoreResult<()> {
        let Some(_fence) = self.try_lock_session_attachment(session)? else {
            return Ok(());
        };
        let mut conn = self.conn.lock().expect("store mutex poisoned");
        // Nearly every boundary has nothing to recover; it takes no write lock.
        let prepared: bool = conn.query_row(
            "SELECT EXISTS(SELECT 1 FROM steer_batches WHERE session_id=?1 AND evidence='prepared')",
            [session],
            |row| row.get(0),
        )?;
        if !prepared {
            return Ok(());
        }
        let tx = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
        tx.execute(
            "DELETE FROM steer_deliveries WHERE batch_id IN (
            SELECT id FROM steer_batches WHERE session_id=?1 AND evidence='prepared')",
            [session],
        )?;
        tx.execute("UPDATE steer_batches SET evidence='cancelled' WHERE session_id=?1 AND evidence='prepared'", [session])?;
        tx.commit()?;
        Ok(())
    }

    pub fn save_wave_steer(
        &self,
        wave: &crate::id::WaveId,
        author: &str,
        text: &str,
    ) -> StoreResult<Steer> {
        self.save_scope_steer(("wave", wave.as_str()), author, text)
    }

    pub fn save_repository_steer(
        &self,
        repo: &crate::repository::CanonicalRepo,
        author: &str,
        text: &str,
    ) -> StoreResult<Steer> {
        self.save_scope_steer(("repository", &repo.to_string()), author, text)
    }

    pub fn save_session_steer(
        &self,
        session: &str,
        author: &str,
        text: &str,
    ) -> StoreResult<Steer> {
        self.save_scope_steer(("session", session), author, text)
    }

    /// The steer `source` at `revision` derived, whichever scope owns it.
    pub fn steer_by_source(&self, source: &str, revision: &str) -> StoreResult<Option<Steer>> {
        let conn = self.conn.lock().expect("store mutex poisoned");
        let mut stmt = conn.prepare(&format!(
            "SELECT {COLUMNS} FROM steers p WHERE p.source=?1 AND p.revision=?2"
        ))?;
        let steer = stmt
            .query_row(params![source, revision], read_steer)
            .optional()?;
        Ok(steer)
    }

    fn save_scope_steer(
        &self,
        owner: (&str, &str),
        author: &str,
        text: &str,
    ) -> StoreResult<Steer> {
        let mut conn = self.conn.lock().expect("store mutex poisoned");
        let tx = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
        let steer = save_in(
            &tx,
            owner,
            &uuid::Uuid::new_v4().to_string(),
            "1",
            &crate::durable::Author::Named(author.into()),
            text,
        )?;
        // Reads and callbacks reconcile their own Session, including steers
        // saved before it existed. Publishing need not scan unrelated Sessions.
        tx.commit()?;
        Ok(steer)
    }

    pub fn session_steers(
        &self,
        session: &str,
        after: i64,
        limit: usize,
    ) -> StoreResult<SessionSteerPage> {
        let limit = limit.clamp(1, 100);
        self.recover_prepared_steers(session)?;
        let mut conn = self.conn.lock().expect("store mutex poisoned");
        let tx = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
        reconcile(&tx, session)?;
        let mut items = tx.prepare(&format!("SELECT {COLUMNS},s.attempt,COALESCE(b.evidence,CASE WHEN s.applicable=1 THEN 'pending' ELSE 'cancelled' END),b.id,b.payload_hash,d.excerpted
            FROM session_steers s JOIN steers p ON p.id=s.steer_id
            LEFT JOIN steer_deliveries d ON d.session_id=s.session_id AND d.steer_id=s.steer_id AND d.attempt=s.attempt
            LEFT JOIN steer_batches b ON b.id=d.batch_id WHERE s.session_id=?1 AND p.seq>?2 ORDER BY p.seq LIMIT ?3"))?
            .query_map(params![session, after, (limit + 1) as i64], |row| {
                Ok(SessionSteerStatus {
                    steer: read_steer(row)?,
                    attempt: row.get(6)?,
                    evidence: row.get(7)?,
                    batch_id: row.get(8)?,
                    payload_hash: row.get(9)?,
                    excerpted: row.get(10)?,
                })
            })?
            .collect::<rusqlite::Result<Vec<_>>>()?;
        let next_after = if items.len() > limit {
            items.truncate(limit);
            items.last().map(|item| item.steer.sequence)
        } else {
            None
        };
        let pending = pending_count(&tx, session)?;
        let capability: String = tx.query_row("SELECT CASE
            WHEN p.id IS NULL OR p.completed_at IS NOT NULL THEN 'idle; pending until next supported provider start'
            WHEN a.agent_process_id IS NULL THEN 'unavailable; provider start required to install adapter'
            WHEN a.activated_at IS NULL THEN 'capability reserved; the Machine server has recorded no callback from this conversation'
            ELSE 'authenticated provider callback observed; output acceptance unconfirmed' END
            FROM agent_sessions s LEFT JOIN processes p ON p.id=s.agent_process_id
            LEFT JOIN steer_adapters a ON a.agent_process_id=p.id WHERE s.id=?1",
            [session], |row| row.get(0))?;
        tx.commit()?;
        Ok(SessionSteerPage {
            items,
            next_after,
            pending,
            capability,
        })
    }

    pub fn session_steer_source(
        &self,
        session: &str,
        id: &crate::id::SteerId,
    ) -> StoreResult<Steer> {
        let mut conn = self.conn.lock().expect("store mutex poisoned");
        let tx = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
        reconcile(&tx, session)?;
        let result = tx
            .query_row(
                &format!(
                    "SELECT {COLUMNS} FROM steers p
            JOIN session_steers s ON s.steer_id=p.id WHERE s.session_id=?1 AND p.id=?2"
                ),
                params![session, id],
                read_steer,
            )
            .optional()?
            .ok_or(StoreError::NotFound)?;
        tx.commit()?;
        Ok(result)
    }

    pub fn retry_session_steer(
        &self,
        session: &str,
        steer_id: &crate::id::SteerId,
    ) -> StoreResult<()> {
        let _lock = self.lock_session_attachment(session)?;
        let mut conn = self.conn.lock().expect("store mutex poisoned");
        let tx = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
        reconcile(&tx, session)?;
        if tx.execute(
            "UPDATE session_steers SET attempt=attempt+1 WHERE session_id=?1 AND steer_id=?2 AND applicable=1
            AND EXISTS(SELECT 1 FROM steer_deliveries d JOIN steer_batches b ON b.id=d.batch_id
             WHERE d.session_id=?1 AND d.steer_id=?2 AND d.attempt=session_steers.attempt
             AND b.evidence IN ('uncertain','emitted','confirmed'))",
            params![session, steer_id],
        )? == 0
        {
            return Err(invalid("No applicable emitted or uncertain steer to retry"));
        }
        tx.commit()?;
        Ok(())
    }

    /// What a hook's request settles: batches it `printed` become `emitted`,
    /// and any other batch an answer carried more than `deadline_secs` ago goes
    /// back to pending, since the hook that asked for it had given up. Returns
    /// whether a newer unconfirmed batch is still waiting for its hook.
    pub(crate) fn confirm_session_steers(
        &self,
        adapter: &SteerAdapter,
        printed: &[crate::id::SteerBatchId],
        deadline_secs: u64,
    ) -> StoreResult<bool> {
        // Whole seconds: a batch counts as older only once it is past the
        // deadline whatever the fraction it was created at.
        let older = "session_id=?1 AND evidence='uncertain' AND created_at < unixepoch() - ?4";
        let owns = "EXISTS(SELECT 1 FROM steer_adapters a
            JOIN agent_sessions s ON s.agent_process_id=a.agent_process_id
            WHERE s.id=?1 AND a.agent_process_id=?2 AND a.capability=?3)";
        let waiting =
            "EXISTS(SELECT 1 FROM steer_batches WHERE session_id=?1 AND evidence='uncertain')";
        let identity = params![
            adapter.session,
            adapter.agent_process_id,
            adapter.capability,
            deadline_secs as i64
        ];
        // (whether a batch is past the deadline, whether one is unconfirmed)
        let read = |conn: &Connection| -> StoreResult<(bool, bool)> {
            let (owns, stale, waiting): (bool, bool, bool) = conn.query_row(
                &format!(
                    "SELECT {owns}, EXISTS(SELECT 1 FROM steer_batches WHERE {older}), {waiting}"
                ),
                identity,
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
            )?;
            if !owns {
                return Err(StoreError::InvalidAuthority(
                    "Context adapter no longer owns this provider".into(),
                ));
            }
            Ok((stale, waiting))
        };
        let mut conn = self.conn.lock().expect("store mutex poisoned");
        // Most boundaries confirm and cancel nothing: answer them without the
        // write lock every Session's writes share.
        let (stale, waiting) = read(&conn)?;
        if printed.is_empty() && !stale {
            return Ok(waiting);
        }
        // The closing read checks authority again; failing it rolls back.
        let tx = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
        for batch in printed {
            tx.execute(
                "UPDATE steer_batches SET evidence='emitted'
                WHERE id=?1 AND session_id=?2 AND agent_process_id=?3 AND evidence='uncertain'",
                params![batch, adapter.session, adapter.agent_process_id],
            )?;
        }
        tx.execute(
            &format!(
                "DELETE FROM steer_deliveries WHERE batch_id IN (
                SELECT id FROM steer_batches WHERE {older})"
            ),
            identity,
        )?;
        tx.execute(
            &format!("UPDATE steer_batches SET evidence='cancelled' WHERE {older}"),
            identity,
        )?;
        let (_, waiting) = read(&tx)?;
        tx.commit()?;
        Ok(waiting)
    }

    /// The Session's next batch of pending steers and its rendered text, for a
    /// hook to print. The batch stays `uncertain` until a later request
    /// confirms it through [`Self::confirm_session_steers`].
    pub(crate) fn emit_session_steers(
        &self,
        adapter: &SteerAdapter,
        provider_thread: &str,
        boundary: &str,
        render: impl FnOnce(&str, &[Steer], usize) -> StoreResult<SteerContext>,
    ) -> StoreResult<Option<(crate::id::SteerBatchId, String)>> {
        let _lock = self.lock_session_dispatch(&adapter.session)?;
        let batch = crate::id::SteerBatchId::new();
        let (steers, remaining) = {
            let mut conn = self.conn.lock().expect("store mutex poisoned");
            // Nothing to emit at most boundaries: answer without the write
            // lock. The caller already checked this adapter's authority.
            if !undelivered(&conn, &adapter.session)? {
                return Ok(None);
            }
            let tx = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
            let claim: Option<String> = tx.query_row("SELECT p.attachment_token FROM agent_sessions s
                JOIN processes p ON p.id=s.agent_process_id JOIN steer_adapters a ON a.agent_process_id=p.id
                WHERE s.id=?1 AND p.id=?2 AND a.capability=?3 AND s.provider_thread=?4
                AND p.completed_at IS NULL",params![adapter.session,adapter.agent_process_id,adapter.capability,provider_thread],|r|r.get(0)).optional()?;
            let claim = claim.ok_or_else(|| {
                StoreError::InvalidAuthority(
                    "Context adapter no longer owns this provider conversation".into(),
                )
            })?;
            reconcile(&tx, &adapter.session)?;
            let duplicate: bool = tx.query_row("SELECT EXISTS(SELECT 1 FROM steer_batches WHERE agent_process_id=?1 AND boundary=?2 AND evidence!='cancelled')",params![adapter.agent_process_id,boundary],|r|r.get(0))?;
            if duplicate {
                return Ok(None);
            }
            let rows = tx.prepare(&format!("SELECT {COLUMNS} FROM session_steers s JOIN steers p ON p.id=s.steer_id
                WHERE s.session_id=?1 AND s.applicable=1 AND NOT EXISTS(SELECT 1 FROM steer_deliveries d
                WHERE d.session_id=s.session_id AND d.steer_id=s.steer_id AND d.attempt=s.attempt)
                ORDER BY p.seq LIMIT 8"))?.query_map([&adapter.session],read_steer)?.collect::<rusqlite::Result<Vec<_>>>()?;
            if rows.is_empty() {
                return Ok(None);
            }
            tx.execute("INSERT INTO steer_batches(id,session_id,agent_process_id,provider_thread,attachment_token,boundary,evidence,created_at)
                VALUES(?1,?2,?3,?4,?5,?6,'prepared',unixepoch())",params![batch,adapter.session,adapter.agent_process_id,provider_thread,claim,boundary])?;
            for row in &rows {
                tx.execute("INSERT INTO steer_deliveries(session_id,steer_id,attempt,batch_id)
                    SELECT session_id,steer_id,attempt,?3 FROM session_steers WHERE session_id=?1 AND steer_id=?2",
                    params![adapter.session,row.id,batch])?;
            }
            let remaining = pending_count(&tx, &adapter.session)?;
            tx.commit()?;
            (rows, remaining as usize)
        };
        let payload = match render(batch.as_str(), &steers, remaining) {
            Ok(payload) => payload,
            Err(error) => {
                let mut conn = self.conn.lock().expect("store mutex poisoned");
                let tx = conn.transaction()?;
                tx.execute("DELETE FROM steer_deliveries WHERE batch_id=?1", [&batch])?;
                tx.execute("DELETE FROM steer_batches WHERE id=?1", [&batch])?;
                tx.commit()?;
                return Err(error);
            }
        };
        let hash = hex::encode(Sha256::digest(payload.text.as_bytes()));
        {
            let mut conn = self.conn.lock().expect("store mutex poisoned");
            let tx = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
            reconcile(&tx, &adapter.session)?;
            tx.execute(
                "UPDATE steer_deliveries SET excerpted=0 WHERE batch_id=?1",
                [&batch],
            )?;
            for steer in &payload.excerpts {
                tx.execute(
                    "UPDATE steer_deliveries SET excerpted=1 WHERE batch_id=?1 AND steer_id=?2",
                    params![batch, steer],
                )?;
            }
            let retained = tx.execute(
                "UPDATE steer_batches SET evidence='uncertain',payload_hash=?2 WHERE id=?1 AND evidence='prepared'",
                params![batch,hash])? == 1;
            tx.commit()?;
            if !retained {
                return Ok(None);
            }
        }
        Ok(Some((batch, payload.text)))
    }
}

#[cfg(test)]
#[path = "steers_tests.rs"]
mod tests;
