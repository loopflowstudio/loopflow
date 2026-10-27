//! Context callbacks carry provider authority, independently of the attached client.
use std::path::Path;

use rusqlite::{params, Connection, OptionalExtension, TransactionBehavior};
use sha2::{Digest, Sha256};

use super::SqliteStore;
use crate::process::SessionAttachment;
use crate::steer::{
    NativeBoundary, SessionSteerPage, SessionSteerStatus, Steer, SteerAdapter, SteerContext,
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
                (NativeBoundary::Select { source }, previous)
                | (NativeBoundary::Activate { source }, previous @ None) => {
                    tx.execute(
                        "INSERT INTO session_events(session_id,kind,receipt_key,lf_process_id,observed_at,payload,captured_event)
                         SELECT s.id,'observed',?2,p.attached_lf_process_id,unixepoch(),json_object('type','agent_session_selected',
                            'agent_process_id',p.id,'provider_thread',?3,'next_provider_thread',?4,'source',?5),s.current_capture
                         FROM agent_sessions s JOIN processes p ON p.id=s.agent_process_id WHERE s.id=?1",
                        params![
                            adapter.session,
                            format!("agent-session:{}:selected:{}", adapter.agent_process_id, uuid::Uuid::new_v4()),
                            previous,
                            thread,
                            source
                        ],
                    )?;
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

fn reconcile(conn: &Connection, session: &str) -> StoreResult<()> {
    let exists: bool = conn.query_row(
        "SELECT EXISTS(SELECT 1 FROM agent_sessions WHERE id=?1)",
        [session],
        |r| r.get(0),
    )?;
    if !exists {
        return Err(StoreError::NotFound);
    }
    // Membership is derived here; receipts alone retain delivery state.
    // Reuse Task inventory's association rule, including explicit binds and
    // checkout membership. UNION terminates even in malformed cyclic ancestry.
    let scopes = format!("WITH RECURSIVE member_tasks(id) AS (
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
        )", super::task_work::session_tasks("s"));
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
        SELECT id FROM steer_batches WHERE evidence='cancelled')",
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
            WHEN a.activated_at IS NULL THEN 'capability reserved; no authenticated callback observed (hook may be unavailable or untrusted)'
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

    /// Returns whether a payload was written, not whether it was accepted. Only
    /// owned provider history containing the batch and hash could confirm it.
    pub(crate) fn emit_session_steers(
        &self,
        adapter: &SteerAdapter,
        provider_thread: &str,
        boundary: &str,
        render: impl FnOnce(&str, &[Steer], usize) -> StoreResult<SteerContext>,
        write: impl FnOnce(&str) -> StoreResult<()>,
    ) -> StoreResult<bool> {
        let _lock = self.lock_session_dispatch(&adapter.session)?;
        let batch = crate::id::SteerBatchId::new();
        let (steers, remaining) = {
            let mut conn = self.conn.lock().expect("store mutex poisoned");
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
                return Ok(false);
            }
            let rows = tx.prepare(&format!("SELECT {COLUMNS} FROM session_steers s JOIN steers p ON p.id=s.steer_id
                WHERE s.session_id=?1 AND s.applicable=1 AND NOT EXISTS(SELECT 1 FROM steer_deliveries d
                WHERE d.session_id=s.session_id AND d.steer_id=s.steer_id AND d.attempt=s.attempt)
                ORDER BY p.seq LIMIT 8"))?.query_map([&adapter.session],read_steer)?.collect::<rusqlite::Result<Vec<_>>>()?;
            if rows.is_empty() {
                return Ok(false);
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
                return Ok(false);
            }
        }
        write(&payload.text)?;
        self.conn.lock().expect("store mutex poisoned").execute(
            "UPDATE steer_batches SET evidence='emitted' WHERE id=?1",
            [batch],
        )?;
        Ok(true)
    }
}

#[cfg(test)]
#[path = "steers_tests.rs"]
mod tests;
