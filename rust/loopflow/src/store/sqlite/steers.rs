//! Context callbacks carry provider authority, independently of the attached client.
use rusqlite::{params, Connection, OptionalExtension, TransactionBehavior};
use sha2::{Digest, Sha256};

use super::SqliteStore;
use crate::process::SessionAttachment;
use crate::steer::{
    SessionSteerPage, SessionSteerStatus, SteerAdapter, SteerContext, SteerPublication,
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
                "INSERT INTO steer_adapters(agent_process_id,capability,provider_thread,created_at)
                 SELECT ?2,?3,provider_thread,unixepoch() FROM agent_sessions WHERE id=?1
                 ON CONFLICT DO NOTHING",
                params![
                    session,
                    expected.agent_process_id,
                    uuid::Uuid::new_v4().to_string()
                ],
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

    /// Root startup activates native identity once. Other boundaries must match
    /// that binding or identity already recorded by the owned native transport.
    pub(crate) fn with_provider_context<T>(
        &self,
        adapter: &SteerAdapter,
        thread: &str,
        startup: bool,
        write: impl FnOnce() -> StoreResult<T>,
    ) -> StoreResult<T> {
        let _fence = self.lock_session_dispatch(&adapter.session)?;
        {
            let mut conn = self.conn.lock().expect("store mutex poisoned");
            let tx = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
            let binding: Option<(Option<String>, Option<String>)> = tx
                .query_row(
                    "SELECT a.provider_thread,s.provider_thread FROM steer_adapters a
                 JOIN processes p ON p.id=a.agent_process_id
                 JOIN agent_sessions s ON s.agent_process_id=p.id
                 WHERE s.id=?1 AND p.id=?2 AND a.capability=?3 AND p.completed_at IS NULL",
                    params![
                        adapter.session,
                        adapter.agent_process_id,
                        adapter.capability
                    ],
                    |row| Ok((row.get(0)?, row.get(1)?)),
                )
                .optional()?;
            let (bound, observed) = binding.ok_or_else(|| {
                StoreError::InvalidAuthority("Context adapter no longer owns this provider".into())
            })?;
            if thread.is_empty()
                || bound.as_deref().is_some_and(|value| value != thread)
                || observed.as_deref().is_some_and(|value| value != thread)
                || (bound.is_none() && observed.is_none() && !startup)
            {
                return Err(StoreError::InvalidAuthority(
                    "Context callback does not belong to the selected native conversation".into(),
                ));
            }
            tx.execute(
                "UPDATE steer_adapters SET provider_thread=?2,activated_at=COALESCE(activated_at,unixepoch()) WHERE agent_process_id=?1",
                params![adapter.agent_process_id, thread],
            )?;
            tx.commit()?;
        }
        write()
    }
}

fn invalid(message: impl ToString) -> StoreError {
    StoreError::InvalidData(message.to_string())
}

fn publication(row: &rusqlite::Row<'_>) -> rusqlite::Result<SteerPublication> {
    Ok(SteerPublication {
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
const COLUMNS: &str = "p.id,p.seq,p.source,p.revision,p.author,p.text";

/// Callers supply exactly one existing source owner; these columns are private,
/// never a wire target or a second Work hierarchy.
pub(super) fn publish_in(
    conn: &Connection,
    owner: (&str, &str),
    source: &str,
    revision: &str,
    author: &crate::durable::Author,
    text: &str,
) -> StoreResult<SteerPublication> {
    if text.trim().is_empty() {
        return Err(invalid("Steer text cannot be empty"));
    }
    let column = match owner.0 {
        "task" => "task_id",
        "wave" => "wave_id",
        "repository" => "repo",
        "session" => "session_id",
        _ => return Err(invalid("Unsupported publication owner")),
    };
    conn.execute(
        &format!(
            "INSERT INTO steer_publications(id,{column},source,revision,author,text,created_at)
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
    let result = conn.query_row(
        &format!("SELECT {COLUMNS} FROM steer_publications p WHERE source=?1 AND revision=?2"),
        params![source, revision],
        publication,
    )?;
    let same_owner: bool = conn.query_row(
        &format!("SELECT {column} IS ?2 FROM steer_publications WHERE id=?1"),
        params![result.id, owner.1],
        |row| row.get(0),
    )?;
    if !same_owner || result.text != text || result.author != *author {
        return Err(invalid(
            "Publication revision already has different contents or ownership",
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
        SELECT p.id FROM steer_publications p WHERE
        (p.session_id=?1 OR p.task_id IN member_tasks OR p.wave_id IN member_waves
         OR p.repo=(SELECT repo FROM agent_sessions WHERE id=?1))
        AND NOT EXISTS(SELECT 1 FROM steer_publications newer WHERE newer.source=p.source AND newer.seq>p.seq)
        )", super::task_work::session_tasks("s"));
    // A revision or association change cancels the whole not-yet-written batch.
    // Its emitter checks the reservation again before exposing any bytes.
    conn.execute(
        &format!(
            "{scopes} UPDATE steer_batches SET evidence='cancelled'
        WHERE session_id=?1 AND evidence='prepared' AND EXISTS(
            SELECT 1 FROM steer_deliveries d WHERE d.batch_id=steer_batches.id
            AND d.publication_id NOT IN applicable)"
        ),
        [session],
    )?;
    conn.execute(
        "DELETE FROM steer_deliveries WHERE session_id=?1 AND batch_id IN (
        SELECT id FROM steer_batches WHERE evidence='cancelled')",
        [session],
    )?;
    conn.execute(
        &format!(
            "{scopes} INSERT INTO session_steers(session_id,publication_id)
        SELECT ?1,id FROM applicable WHERE true ON CONFLICT DO NOTHING"
        ),
        [session],
    )?;
    conn.execute(
        &format!(
            "{scopes} UPDATE session_steers SET applicable=(publication_id IN applicable)
        WHERE session_id=?1"
        ),
        [session],
    )?;
    // Emitted and uncertain receipts survive scope changes and corrections.
    conn.execute(&format!("{scopes} DELETE FROM session_steers WHERE session_id=?1
        AND NOT EXISTS(SELECT 1 FROM steer_deliveries d WHERE d.session_id=?1 AND d.publication_id=session_steers.publication_id)
        AND publication_id NOT IN applicable"), [session])?;
    Ok(())
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

    pub fn publish_wave_steer(
        &self,
        wave: &crate::id::WaveId,
        author: &str,
        text: &str,
    ) -> StoreResult<SteerPublication> {
        self.publish_scope_steer(("wave", wave.as_str()), author, text)
    }

    pub fn publish_repository_steer(
        &self,
        repo: &crate::repository::CanonicalRepo,
        author: &str,
        text: &str,
    ) -> StoreResult<SteerPublication> {
        self.publish_scope_steer(("repository", &repo.to_string()), author, text)
    }

    fn publish_scope_steer(
        &self,
        owner: (&str, &str),
        author: &str,
        text: &str,
    ) -> StoreResult<SteerPublication> {
        let mut conn = self.conn.lock().expect("store mutex poisoned");
        let tx = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
        let publication = publish_in(
            &tx,
            owner,
            &uuid::Uuid::new_v4().to_string(),
            "1",
            &crate::durable::Author::Named(author.into()),
            text,
        )?;
        // The same association resolver handles immediate fan-out and a later
        // boundary after a new bind; publishing never starts provider work.
        let sessions = tx
            .prepare("SELECT id FROM agent_sessions")?
            .query_map([], |row| row.get::<_, String>(0))?
            .collect::<rusqlite::Result<Vec<_>>>()?;
        for session in sessions {
            reconcile(&tx, &session)?;
        }
        tx.commit()?;
        Ok(publication)
    }

    pub fn publish_session_steer(
        &self,
        session: &str,
        author: &str,
        text: &str,
    ) -> StoreResult<SteerPublication> {
        let mut conn = self.conn.lock().expect("store mutex poisoned");
        let tx = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
        let publication = publish_in(
            &tx,
            ("session", session),
            &uuid::Uuid::new_v4().to_string(),
            "1",
            &crate::durable::Author::Named(author.into()),
            text,
        )?;
        reconcile(&tx, session)?;
        tx.commit()?;
        Ok(publication)
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
            FROM session_steers s JOIN steer_publications p ON p.id=s.publication_id
            LEFT JOIN steer_deliveries d ON d.session_id=s.session_id AND d.publication_id=s.publication_id AND d.attempt=s.attempt
            LEFT JOIN steer_batches b ON b.id=d.batch_id WHERE s.session_id=?1 AND p.seq>?2 ORDER BY p.seq LIMIT ?3"))?
            .query_map(params![session,after,(limit+1) as i64],|r| Ok(SessionSteerStatus { publication: publication(r)?, attempt:r.get(6)?, evidence:r.get(7)?, batch_id:r.get(8)?,payload_hash:r.get(9)?,excerpted:r.get(10)? }))?
            .collect::<rusqlite::Result<Vec<_>>>()?;
        let next_after = if items.len() > limit {
            items.truncate(limit);
            items.last().map(|item| item.publication.sequence)
        } else {
            None
        };
        let pending = tx.query_row(
            "SELECT count(*) FROM session_steers s WHERE s.session_id=?1 AND s.applicable=1
            AND NOT EXISTS(SELECT 1 FROM steer_deliveries d WHERE d.session_id=s.session_id
            AND d.publication_id=s.publication_id AND d.attempt=s.attempt)",
            [session],
            |row| row.get(0),
        )?;
        let capability: String = tx.query_row("SELECT CASE
            WHEN p.id IS NULL OR p.completed_at IS NOT NULL THEN 'idle; pending until next supported provider start'
            WHEN a.agent_process_id IS NULL THEN 'unavailable; provider start required to install adapter'
            WHEN a.activated_at IS NULL THEN 'installed; no authenticated callback observed (hook may be unavailable or untrusted)'
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
        id: &crate::id::SteerPublicationId,
    ) -> StoreResult<SteerPublication> {
        let mut conn = self.conn.lock().expect("store mutex poisoned");
        let tx = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
        reconcile(&tx, session)?;
        let result = tx
            .query_row(
                &format!(
                    "SELECT {COLUMNS} FROM steer_publications p
            JOIN session_steers s ON s.publication_id=p.id WHERE s.session_id=?1 AND p.id=?2"
                ),
                params![session, id],
                publication,
            )
            .optional()?
            .ok_or(StoreError::NotFound)?;
        tx.commit()?;
        Ok(result)
    }

    pub fn retry_session_steer(
        &self,
        session: &str,
        publication_id: &crate::id::SteerPublicationId,
    ) -> StoreResult<()> {
        let _lock = self.lock_session_attachment(session)?;
        let mut conn = self.conn.lock().expect("store mutex poisoned");
        let tx = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
        reconcile(&tx, session)?;
        if tx.execute(
            "UPDATE session_steers SET attempt=attempt+1 WHERE session_id=?1 AND publication_id=?2 AND applicable=1
            AND EXISTS(SELECT 1 FROM steer_deliveries d JOIN steer_batches b ON b.id=d.batch_id
             WHERE d.session_id=?1 AND d.publication_id=?2 AND d.attempt=session_steers.attempt
             AND b.evidence IN ('uncertain','emitted','confirmed'))",
            params![session, publication_id],
        )? == 0
        {
            return Err(invalid("No applicable emitted or uncertain publication to retry"));
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
        render: impl FnOnce(&str, &[SteerPublication], usize) -> StoreResult<SteerContext>,
        write: impl FnOnce(&str) -> StoreResult<()>,
    ) -> StoreResult<bool> {
        let _lock = self.lock_session_dispatch(&adapter.session)?;
        let batch = crate::id::SteerBatchId::new();
        let selected = {
            let mut conn = self.conn.lock().expect("store mutex poisoned");
            let tx = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
            let claim: Option<String> = tx.query_row("SELECT p.attachment_token FROM agent_sessions s
                JOIN processes p ON p.id=s.agent_process_id JOIN steer_adapters a ON a.agent_process_id=p.id
                WHERE s.id=?1 AND p.id=?2 AND a.capability=?3 AND a.provider_thread=?4
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
            let rows = tx.prepare(&format!("SELECT {COLUMNS} FROM session_steers s JOIN steer_publications p ON p.id=s.publication_id
                WHERE s.session_id=?1 AND s.applicable=1 AND NOT EXISTS(SELECT 1 FROM steer_deliveries d
                WHERE d.session_id=s.session_id AND d.publication_id=s.publication_id AND d.attempt=s.attempt)
                ORDER BY p.seq LIMIT 8"))?.query_map([&adapter.session],publication)?.collect::<rusqlite::Result<Vec<_>>>()?;
            if rows.is_empty() {
                return Ok(false);
            }
            tx.execute("INSERT INTO steer_batches(id,session_id,agent_process_id,provider_thread,attachment_token,boundary,evidence,created_at)
                VALUES(?1,?2,?3,?4,?5,?6,'prepared',unixepoch())",params![batch,adapter.session,adapter.agent_process_id,provider_thread,claim,boundary])?;
            for row in &rows {
                tx.execute("INSERT INTO steer_deliveries(session_id,publication_id,attempt,batch_id)
                    SELECT session_id,publication_id,attempt,?3 FROM session_steers WHERE session_id=?1 AND publication_id=?2",
                    params![adapter.session,row.id,batch])?;
            }
            let remaining: i64 = tx.query_row("SELECT count(*) FROM session_steers s WHERE session_id=?1 AND s.applicable=1
                AND NOT EXISTS(SELECT 1 FROM steer_deliveries d WHERE d.session_id=s.session_id AND d.publication_id=s.publication_id AND d.attempt=s.attempt)",[&adapter.session],|r|r.get(0))?;
            tx.commit()?;
            (rows, remaining as usize)
        };
        let payload = match render(batch.as_str(), &selected.0, selected.1) {
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
            for publication in &payload.excerpts {
                tx.execute("UPDATE steer_deliveries SET excerpted=1 WHERE batch_id=?1 AND publication_id=?2",
                    params![batch,publication])?;
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
