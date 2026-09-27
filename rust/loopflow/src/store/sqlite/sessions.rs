//! Session transactions share the invocation's SQLite transaction and fences.

use rusqlite::{params, Connection, OptionalExtension, Transaction, TransactionBehavior};

use crate::durable::{FlowPosition, RunId};
use crate::session::{Run, Session, TitleSource, WorkSource};
use crate::store::{StoreError, StoreResult};

use super::SqliteStore;

const SESSION_SELECT: &str = "SELECT s.id, s.current_run_id, s.title, s.title_source,
    s.ready_summary, s.completed_at, s.created_at,
    r.id, r.session_id, r.invocation_id, r.task_id, r.wave_id, r.work_source,
    r.created_at, r.published, r.cwd, r.skill
    FROM sessions s JOIN runs r ON r.id=s.current_run_id AND r.session_id=s.id";

fn read_session(row: &rusqlite::Row<'_>) -> rusqlite::Result<StoreResult<(Session, Run)>> {
    let id: String = row.get(0)?;
    let run_id: String = row.get(1)?;
    let title = row.get(2)?;
    let source: String = row.get(3)?;
    let ready_summary = row.get(4)?;
    let completed_at = row.get(5)?;
    let created_at = row.get(6)?;
    let run = super::runs::read_run(row, 7)?;
    Ok((|| {
        let run_id = RunId::parse(&run_id).map_err(invalid)?;
        Ok((
            Session {
                id: id.clone(),
                current_run_id: run_id.clone(),
                title,
                title_source: match source.as_str() {
                    "human" => TitleSource::Human,
                    "generated" => TitleSource::Generated,
                    _ => return Err(invalid("unknown Session title provenance")),
                },
                ready_summary,
                completed_at,
                created_at,
            },
            run?,
        ))
    })())
}

fn invalid(error: impl std::fmt::Display) -> StoreError {
    StoreError::InvalidData(error.to_string())
}

pub(super) fn session_in(conn: &Connection, id: &str) -> StoreResult<Option<(Session, Run)>> {
    conn.query_row(
        &format!("{SESSION_SELECT} WHERE s.id=?1"),
        [id],
        read_session,
    )
    .optional()?
    .transpose()
}

impl SqliteStore {
    pub fn reserve_review_run(&self, expected: &FlowPosition) -> StoreResult<(FlowPosition, Run)> {
        let mut conn = self.conn.lock().expect("store mutex poisoned");
        let tx = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
        let current = super::durable::flow_position_in(&tx, &expected.task_id)?
            .ok_or(StoreError::NotFound)?;
        if current != *expected || current.claim.is_some() || current.failure.is_some() {
            return Err(StoreError::InvalidAuthority(
                "review changed before Run reservation".into(),
            ));
        }
        let id = review_id(expected)?;
        let (session, run) = session_in(&tx, &id)?.ok_or(StoreError::NotFound)?;
        if session.completed_at.is_some() {
            return Err(StoreError::InvalidAuthority("review is complete".into()));
        }
        if run.published {
            let replacement = RunId::new();
            let cwd: String = tx.query_row(
                "SELECT worktree FROM tasks WHERE id=?1",
                [expected.task_id.as_str()],
                |row| row.get(0),
            )?;
            super::runs::insert_run_in(
                &tx,
                Run {
                    id: replacement.clone(),
                    cwd: cwd.into(),
                    published: false,
                    created_at: crate::store::rows::now_unix(),
                    work_source: Some(WorkSource::Inherited),
                    ..run
                },
            )?;
            tx.execute(
                "UPDATE sessions SET current_run_id=?2 WHERE id=?1",
                params![id, replacement.as_str()],
            )?;
            tx.execute(
                "UPDATE flow_invocations SET position_version=position_version+1 WHERE id=?1",
                [&expected.invocation.id],
            )?;
        }
        let position = super::durable::flow_position_in(&tx, &expected.task_id)?
            .ok_or(StoreError::NotFound)?;
        let (_, run) = session_in(&tx, &id)?.ok_or(StoreError::NotFound)?;
        tx.commit()?;
        Ok((position, run))
    }

    pub(crate) fn publish_review_run(
        &self,
        session_id: &str,
        run_id: &RunId,
        version: u64,
    ) -> StoreResult<()> {
        let mut conn = self.conn.lock().expect("store mutex poisoned");
        let tx = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
        if tx.execute("UPDATE flow_invocations SET position_version=position_version+1
            WHERE state='current' AND pending_session_id=?1 AND position_version=?3
            AND claim_json IS NULL AND EXISTS(SELECT 1 FROM sessions s JOIN runs r ON r.id=s.current_run_id
                WHERE s.id=?1 AND r.id=?2 AND r.published=0 AND s.completed_at IS NULL)",
            params![session_id, run_id.as_str(), i64::try_from(version).map_err(invalid)?])? != 1 {
            return Err(StoreError::InvalidAuthority("review Run reservation is stale".into()));
        }
        tx.execute("UPDATE runs SET published=1 WHERE id=?1", [run_id.as_str()])?;
        tx.commit()?;
        Ok(())
    }

    pub fn session(&self, id: &str) -> StoreResult<Option<(Session, Run)>> {
        let conn = self.conn.lock().expect("store mutex poisoned");
        session_in(&conn, id)
    }

    pub fn session_for_run(&self, run_id: &RunId) -> StoreResult<Option<(Session, Run)>> {
        let conn = self.conn.lock().expect("store mutex poisoned");
        conn.query_row(
            &format!("{SESSION_SELECT} WHERE s.id=(SELECT session_id FROM runs WHERE id=?1)"),
            [run_id.as_str()],
            read_session,
        )
        .optional()?
        .transpose()
    }

    pub fn open_review_sessions(&self) -> StoreResult<Vec<(Session, Run)>> {
        let conn = self.conn.lock().expect("store mutex poisoned");
        let mut query = conn.prepare(&format!(
            "{SESSION_SELECT} WHERE s.completed_at IS NULL AND EXISTS(
                SELECT 1 FROM flow_invocations f WHERE f.pending_session_id=s.id AND f.state='current')
             ORDER BY s.created_at, s.id"
        ))?;
        let rows = query.query_map([], read_session)?;
        rows.map(|row| row?).collect()
    }

    pub fn session_run_ids(&self) -> StoreResult<Vec<RunId>> {
        let conn = self.conn.lock().expect("store mutex poisoned");
        let mut query = conn.prepare("SELECT id FROM runs WHERE session_id IS NOT NULL")?;
        let rows = query.query_map([], |row| row.get::<_, String>(0))?;
        rows.map(|row| RunId::parse(&row?).map_err(invalid))
            .collect()
    }

    pub fn session_runs(&self, id: &str) -> StoreResult<Vec<Run>> {
        let conn = self.conn.lock().expect("store mutex poisoned");
        let mut query = conn.prepare(
            "SELECT id, session_id, invocation_id, task_id, wave_id, work_source,
             created_at, published, cwd, skill
             FROM runs WHERE session_id=?1 ORDER BY created_at, id",
        )?;
        let rows = query.query_map([id], |row| super::runs::read_run(row, 0))?;
        rows.map(|row| row?).collect()
    }

    pub fn rename_session(
        &self,
        id: &str,
        expected_run: Option<&RunId>,
        title: &str,
        source: TitleSource,
    ) -> StoreResult<()> {
        if title.trim().is_empty() {
            return Err(invalid("Session title cannot be empty"));
        }
        let mut conn = self.conn.lock().expect("store mutex poisoned");
        let tx = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
        let (session, _) = session_in(&tx, id)?.ok_or(StoreError::NotFound)?;
        if expected_run.is_some_and(|run| *run != session.current_run_id) {
            return Err(StoreError::InvalidAuthority(
                "Session changed before rename".into(),
            ));
        }
        let source = match source {
            TitleSource::Human => "human",
            TitleSource::Generated => "generated",
        };
        tx.execute(
            "UPDATE sessions SET title=?2, title_source=?3
             WHERE id=?1 AND (title_source='generated' OR ?3='human')",
            params![id, title.trim(), source],
        )?;
        tx.commit()?;
        Ok(())
    }

    pub fn ready_session(&self, id: &str, expected_run: &RunId, summary: &str) -> StoreResult<()> {
        if summary.trim().is_empty() {
            return Err(invalid("ready summary cannot be empty"));
        }
        let mut conn = self.conn.lock().expect("store mutex poisoned");
        let tx = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
        if tx.execute(
            "UPDATE sessions SET ready_summary=?3 WHERE id=?1 AND current_run_id=?2
             AND completed_at IS NULL AND EXISTS(SELECT 1 FROM runs r
                 JOIN flow_invocations f ON f.id=r.invocation_id
                 WHERE r.id=?2 AND r.published=1 AND f.pending_session_id=?1 AND f.state='current'
                 AND f.claim_json IS NULL)",
            params![id, expected_run.as_str(), summary.trim()],
        )? != 1
        {
            return Err(StoreError::InvalidAuthority(
                "review Session is stale".into(),
            ));
        }
        // Readiness invalidates an in-flight completion's snapshot as well.
        tx.execute(
            "UPDATE flow_invocations SET position_version=position_version+1
            WHERE pending_session_id=?1 AND state='current'",
            [id],
        )?;
        tx.commit()?;
        Ok(())
    }
}

pub(super) fn review_id(position: &FlowPosition) -> StoreResult<String> {
    let step = position
        .current_checked()
        .ok_or_else(|| invalid("review has no captured step"))?;
    let node = step
        .policy
        .id
        .ok_or_else(|| invalid("review has no captured node"))?;
    Ok(format!(
        "{}:{}:{}:{}:{}",
        position.task_id, position.invocation.id, step.flow, node, position.cursor.iteration
    ))
}

/// Called only inside the already fenced invocation mutation transaction.
pub(super) fn save_review_in(conn: &Transaction<'_>, position: &FlowPosition) -> StoreResult<()> {
    if !position.is_human() {
        conn.execute(
            "UPDATE flow_invocations SET pending_session_id=NULL WHERE id=?1",
            [&position.invocation.id],
        )?;
        return Ok(());
    }
    let id = review_id(position)?;
    if let Some((session, _)) = session_in(conn, &id)? {
        if session.completed_at.is_some() {
            return Err(StoreError::InvalidAuthority(
                "completed review cannot be reopened by a cursor write".into(),
            ));
        }
    } else {
        let run_id = position.session_run_id.clone().unwrap_or_default();
        let title: String = conn.query_row(
            "SELECT issue_title FROM tasks WHERE id=?1",
            [position.task_id.as_str()],
            |row| row.get(0),
        )?;
        conn.execute(
            "INSERT INTO sessions(id,current_run_id,title,title_source,ready_summary,created_at)
            VALUES(?1,?2,?3,'generated',?4,?5)",
            params![
                id,
                run_id.as_str(),
                title,
                position.ready_summary,
                position.updated_at.unix_timestamp()
            ],
        )?;
        insert_review_run_in(
            conn,
            &run_id,
            &id,
            position,
            position.session_run_id.is_some(),
        )?;
    }
    conn.execute(
        "UPDATE flow_invocations SET pending_session_id=?2 WHERE id=?1",
        params![position.invocation.id, id],
    )?;
    Ok(())
}

fn insert_review_run_in(
    conn: &Transaction<'_>,
    id: &RunId,
    session_id: &str,
    position: &FlowPosition,
    published: bool,
) -> StoreResult<()> {
    let cwd: String = conn.query_row(
        "SELECT worktree FROM tasks WHERE id=?1",
        [position.task_id.as_str()],
        |row| row.get(0),
    )?;
    super::runs::insert_run_in(
        conn,
        Run {
            id: id.clone(),
            session_id: Some(session_id.to_owned()),
            invocation_id: Some(position.invocation.id.clone()),
            task_id: Some(position.task_id.clone()),
            wave_id: None,
            work_source: Some(WorkSource::Inherited),
            created_at: position.updated_at.unix_timestamp(),
            published,
            cwd: cwd.into(),
            skill: Some(position.current().step),
        },
    )?;
    Ok(())
}

pub(super) fn complete_review_in(conn: &Connection, expected: &FlowPosition) -> StoreResult<()> {
    let id = review_id(expected)?;
    let summary = expected
        .ready_summary
        .as_deref()
        .filter(|summary| !summary.trim().is_empty())
        .ok_or_else(|| StoreError::InvalidAuthority("review is not ready".into()))?;
    let run_id = expected
        .session_run_id
        .as_ref()
        .ok_or_else(|| StoreError::InvalidAuthority("review has no published Run".into()))?;
    if conn.execute(
        "UPDATE sessions SET completed_at=?3 WHERE id=?1 AND current_run_id=?2
        AND completed_at IS NULL AND ready_summary IS ?4",
        params![id, run_id.as_str(), crate::store::rows::now_unix(), summary],
    )? != 1
    {
        return Err(StoreError::InvalidAuthority(
            "review changed before completion".into(),
        ));
    }
    Ok(())
}
