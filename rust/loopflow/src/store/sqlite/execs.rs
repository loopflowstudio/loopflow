use std::fs::{File, OpenOptions};
use std::num::NonZeroU32;
use std::path::Path;
use std::time::{Duration, Instant};

use fs2::FileExt;
use rusqlite::types::Value;
use rusqlite::{params, params_from_iter, OptionalExtension, TransactionBehavior};
use sha2::{Digest, Sha256};

use crate::exec::{
    AgentCaller, Exec, ExecCursor, ExecFilter, ExecOutcomeFilter, ExecPage, ExecWorkFilter,
    SessionDriver,
};
use crate::id::ExecId;
use crate::store::{StoreError, StoreResult};

use super::SqliteStore;

pub(super) const EXEC_SELECT: &str =
    "SELECT e.id,e.trace_id,e.parent_exec_id,e.via_agent,e.caller_session_id,
    e.caller_provider_generation,e.command,e.repo,e.cwd,
    e.started_at,e.completed_at,e.outcome,e.exit_code,e.signal,e.error FROM execs e";

pub(super) fn read_exec(row: &rusqlite::Row<'_>) -> rusqlite::Result<Exec> {
    Ok(Exec {
        id: row.get(0)?,
        trace_id: row.get(1)?,
        parent_exec_id: row.get(2)?,
        via_agent: row.get(3)?,
        caller_session_id: row.get(4)?,
        caller_provider_generation: row.get(5)?,
        command: row.get(6)?,
        repo: row.get(7)?,
        cwd: row.get(8)?,
        started_at: row.get(9)?,
        completed_at: row.get(10)?,
        outcome: row.get(11)?,
        exit_code: row.get(12)?,
        signal: row.get(13)?,
        error: row.get(14)?,
    })
}

fn exec_query(
    filter: &ExecFilter,
    after: Option<&ExecCursor>,
    limit: NonZeroU32,
) -> (String, Vec<Value>) {
    let mut sql = String::from("WITH page AS MATERIALIZED (SELECT e.id FROM execs e WHERE 1");
    let mut values = Vec::new();
    let mut bind = |value| {
        values.push(value);
        format!("?{}", values.len())
    };
    for (column, value) in [
        ("id", filter.id.as_ref().map(ExecId::as_str)),
        ("repo", filter.repo.as_deref()),
        (
            "parent_exec_id",
            filter.parent_exec_id.as_ref().map(ExecId::as_str),
        ),
        ("caller_session_id", filter.caller_session_id.as_deref()),
    ] {
        if let Some(value) = value {
            sql.push_str(&format!(
                " AND e.{column}={}",
                bind(Value::Text(value.into()))
            ));
        }
    }
    if let Some(value) = &filter.command_contains {
        sql.push_str(&format!(
            " AND instr(lower(CASE WHEN json_valid(e.command) THEN
                CASE WHEN json_type(e.command)='array'
                    AND NOT EXISTS(SELECT 1 FROM json_each(e.command) WHERE type!='text')
                THEN (SELECT group_concat(value,' ') FROM json_each(e.command))
                ELSE e.command END
             ELSE e.command END),lower({}))>0",
            bind(Value::Text(value.clone()))
        ));
    }
    if let Some(value) = &filter.identity_contains {
        sql.push_str(&format!(
            " AND instr(e.id,{})>0",
            bind(Value::Text(value.clone()))
        ));
    }
    if let Some(outcome) = filter.outcome {
        sql.push_str(match outcome {
            ExecOutcomeFilter::Succeeded => " AND e.outcome='succeeded'",
            ExecOutcomeFilter::Failed => " AND e.outcome='failed'",
            ExecOutcomeFilter::Interrupted => " AND e.outcome='interrupted'",
            ExecOutcomeFilter::Unknown => " AND e.outcome IS NULL",
        });
    }
    if let Some(work) = &filter.performed_work {
        let (column, value) = match work {
            ExecWorkFilter::Task(id) => ("task_id", id.as_str()),
            ExecWorkFilter::Wave(id) => ("wave_id", id.as_str()),
        };
        let value = bind(Value::Text(value.into()));
        // Native starts retain their original assignment. Mechanical starts
        // reference their owning captured Flow. Neither a current Session bind
        // nor an Exec's command context establishes performed work.
        sql.push_str(&format!(
            " AND e.id IN (
            SELECT exec_id FROM session_events
            WHERE kind='started' AND {column}={value} AND exec_id IS NOT NULL
            UNION
            SELECT h.exec_id FROM flow_events h JOIN flow_sessions f ON f.id=h.flow_id
            WHERE h.kind='operation_started' AND f.{column}={value} AND h.exec_id IS NOT NULL
        )"
        ));
    }
    if let Some(after) = after {
        let time = bind(Value::Integer(after.started_at));
        let id = bind(Value::Text(after.id.to_string()));
        sql.push_str(&format!(
            " AND e.started_at<={time} AND (e.started_at<{time} OR e.id>{id})"
        ));
    }
    let limit = bind(Value::Integer(i64::from(limit.get()) + 1));
    sql.push_str(&format!(
        " ORDER BY e.started_at DESC,e.id ASC LIMIT {limit})
         {EXEC_SELECT} JOIN page ON page.id=e.id ORDER BY e.started_at DESC,e.id ASC"
    ));
    (sql, values)
}

fn driver_in(conn: &rusqlite::Connection, session: &str) -> StoreResult<Option<SessionDriver>> {
    let row = conn
        .query_row(
            "SELECT driver_exec_id,driver_generation,provider_generation,provider_exec_id
         FROM agent_sessions WHERE id=?1",
            [session],
            |row| {
                Ok((
                    row.get::<_, Option<String>>(0)?,
                    row.get::<_, i64>(1)?,
                    row.get::<_, i64>(2)?,
                    row.get::<_, Option<String>>(3)?,
                ))
            },
        )
        .optional()?
        .ok_or(StoreError::NotFound)?;
    if row.1 == 0 {
        return Ok(None);
    }
    let parse =
        |value: &str| ExecId::parse(value).map_err(|e| StoreError::InvalidData(e.to_string()));
    Ok(Some(SessionDriver {
        exec_id: row.0.as_deref().map(parse).transpose()?,
        generation: row.1,
        provider_generation: row.2,
        provider_exec_id: parse(row.3.as_deref().ok_or_else(|| {
            StoreError::InvalidData("Session driver has no provider origin".into())
        })?)?,
    }))
}

impl SqliteStore {
    pub fn execs_since(&self, since: i64) -> StoreResult<Vec<Exec>> {
        let conn = self.conn.lock().expect("store mutex poisoned");
        let mut query = conn.prepare(&format!(
            "{EXEC_SELECT} WHERE e.started_at>=?1 ORDER BY e.started_at,e.id"
        ))?;
        let records = query
            .query_map([since], read_exec)?
            .collect::<Result<Vec<_>, _>>()?;
        Ok(records)
    }

    /// Resolve retained identity without loading plans, captures or launch eligibility.
    pub(crate) fn resolve_task_id(
        &self,
        selector: &str,
        repo: Option<&str>,
    ) -> StoreResult<Option<crate::durable::TaskId>> {
        self.resolve_work_id("SELECT t.id FROM tasks t JOIN projects p ON p.id=t.project_id JOIN waves w ON w.id=p.wave_id
            WHERE t.id=?1 OR ((t.issue_identifier=?1 OR t.external_issue_id=?1) AND (?2 IS NULL OR w.repo=?2))
            ORDER BY (t.id=?1) DESC,t.id LIMIT 2", selector, repo)
            .map(|id| id.map(crate::durable::TaskId::from_raw))
    }

    pub(crate) fn resolve_wave_id(
        &self,
        selector: &str,
        repo: Option<&str>,
    ) -> StoreResult<Option<crate::id::WaveId>> {
        self.resolve_work_id(
            "SELECT id FROM wave_addresses WHERE id=?1 OR (slug=?1 AND (?2 IS NULL OR repo=?2))
            ORDER BY (id=?1) DESC,id LIMIT 2",
            selector,
            repo,
        )?
        .map(|id| {
            crate::id::WaveId::parse(&id)
                .map_err(|error| StoreError::InvalidData(error.to_string()))
        })
        .transpose()
    }

    fn resolve_work_id(
        &self,
        sql: &str,
        selector: &str,
        repo: Option<&str>,
    ) -> StoreResult<Option<String>> {
        let conn = self.conn.lock().expect("store mutex poisoned");
        let mut query = conn.prepare(sql)?;
        let ids = query
            .query_map(params![selector, repo], |row| row.get::<_, String>(0))?
            .collect::<rusqlite::Result<Vec<_>>>()?;
        if ids.first().is_some_and(|id| id == selector) {
            return Ok(ids.into_iter().next());
        }
        match ids.as_slice() {
            [] => Ok(None),
            [id] => Ok(Some(id.clone())),
            _ => Err(StoreError::InvalidData(format!(
                "Ambiguous Work selector {selector:?}"
            ))),
        }
    }

    /// Read one command without decoding its event history or provider payloads.
    pub fn exec(&self, id: &ExecId) -> StoreResult<Option<Exec>> {
        let conn = self.conn.lock().expect("store mutex poisoned");
        Ok(conn
            .query_row(&format!("{EXEC_SELECT} WHERE e.id=?1"), [id], read_exec)
            .optional()?)
    }

    /// Exact identity wins; otherwise require a unique literal, case-sensitive prefix.
    pub fn resolve_exec(&self, selector: &str) -> StoreResult<Option<Exec>> {
        let conn = self.conn.lock().expect("store mutex poisoned");
        if let Some(exec) = conn
            .query_row(
                &format!("{EXEC_SELECT} WHERE e.id=?1"),
                [selector],
                read_exec,
            )
            .optional()?
        {
            return Ok(Some(exec));
        }
        // Exec IDs are UUID text. This range seeks the existing identity index;
        // '%' and '_' have no wildcard meaning, unlike LIKE/GLOB.
        let upper = format!("{selector}\u{10ffff}");
        let mut query =
            conn.prepare("SELECT id FROM execs WHERE id>=?1 AND id<?2 ORDER BY id LIMIT 2")?;
        let ids = query
            .query_map(params![selector, upper], |row| row.get::<_, ExecId>(0))?
            .collect::<rusqlite::Result<Vec<_>>>()?;
        match ids.as_slice() {
            [] => Ok(None),
            [id] => Ok(Some(conn.query_row(
                &format!("{EXEC_SELECT} WHERE e.id=?1"),
                [id],
                read_exec,
            )?)),
            _ => Err(StoreError::InvalidData(format!(
                "Ambiguous Exec prefix {selector:?}"
            ))),
        }
    }

    /// Filter/deduplicate in SQL; decode only a bounded page plus one lookahead.
    /// Missing payloads do not remove rows. No history body is read for discovery.
    pub fn execs(
        &self,
        filter: &ExecFilter,
        after: Option<&ExecCursor>,
        limit: NonZeroU32,
    ) -> StoreResult<ExecPage> {
        let (sql, values) = exec_query(filter, after, limit);
        let conn = self.conn.lock().expect("store mutex poisoned");
        let mut query = conn.prepare(&sql)?;
        let mut entries = query
            .query_map(params_from_iter(values), read_exec)?
            .collect::<rusqlite::Result<Vec<_>>>()?;
        let next = if entries.len() > limit.get() as usize {
            entries.pop();
            entries.last().map(|exec| ExecCursor {
                started_at: exec.started_at,
                id: exec.id.clone(),
            })
        } else {
            None
        };
        Ok(ExecPage { entries, next })
    }

    pub(crate) fn make_session_interactive(
        &self,
        session: &str,
        expected: &SessionDriver,
    ) -> StoreResult<()> {
        let mut conn = self.conn.lock().expect("store mutex poisoned");
        let tx = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
        if driver_in(&tx, session)?.as_ref() != Some(expected) || expected.exec_id.is_none() {
            return Err(StoreError::InvalidAuthority(
                "Session driver changed".into(),
            ));
        }
        tx.execute(
            "UPDATE agent_sessions SET interactive=1 WHERE id=?1",
            [session],
        )?;
        tx.commit()?;
        Ok(())
    }

    pub fn session_connection(&self, session: &str) -> StoreResult<Option<(String, String)>> {
        let conn = self.conn.lock().expect("store mutex poisoned");
        Ok(conn.query_row(
            "SELECT provider_endpoint,provider_thread FROM agent_sessions WHERE id=?1 AND provider_endpoint IS NOT NULL AND provider_thread IS NOT NULL",
            [session], |row| Ok((row.get(0)?, row.get(1)?)),
        ).optional()?)
    }

    pub(crate) fn session_thread(&self, session: &str) -> StoreResult<Option<String>> {
        let conn = self.conn.lock().expect("store mutex poisoned");
        Ok(conn
            .query_row(
                "SELECT provider_thread FROM agent_sessions WHERE id=?1",
                [session],
                |row| row.get(0),
            )
            .optional()?
            .flatten())
    }

    pub(crate) fn session_provider_process(
        &self,
        session: &str,
    ) -> StoreResult<Option<(u32, i64)>> {
        let conn = self.conn.lock().expect("store mutex poisoned");
        Ok(conn
            .query_row(
                "SELECT provider_pid,provider_started_at FROM agent_sessions WHERE id=?1
             AND provider_pid IS NOT NULL AND provider_started_at IS NOT NULL",
                [session],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .optional()?)
    }

    pub(crate) fn record_session_provider_process(
        &self,
        session: &str,
        expected: &SessionDriver,
        pid: u32,
        started_at: i64,
    ) -> StoreResult<()> {
        let mut conn = self.conn.lock().expect("store mutex poisoned");
        let tx = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
        if driver_in(&tx, session)?.as_ref() != Some(expected) || expected.exec_id.is_none() {
            return Err(StoreError::InvalidAuthority(
                "Session driver changed".into(),
            ));
        }
        tx.execute(
            "UPDATE agent_sessions SET provider_pid=?2,provider_started_at=?3 WHERE id=?1",
            params![session, pid, started_at],
        )?;
        tx.commit()?;
        Ok(())
    }

    pub fn record_session_connection(
        &self,
        session: &str,
        expected: &SessionDriver,
        endpoint: &str,
        thread: &str,
    ) -> StoreResult<()> {
        let mut conn = self.conn.lock().expect("store mutex poisoned");
        let tx = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
        if driver_in(&tx, session)?.as_ref() != Some(expected) || expected.exec_id.is_none() {
            return Err(StoreError::InvalidAuthority(
                "Session driver changed".into(),
            ));
        }
        tx.execute(
            "UPDATE agent_sessions SET provider_endpoint=?2,provider_thread=?3 WHERE id=?1",
            params![session, endpoint, thread],
        )?;
        tx.commit()?;
        Ok(())
    }

    // Dispatch and driver changes share a per-Session OS lock, never a SQLite
    // transaction across provider I/O. Do not unlink lock files: another process
    // may already have the inode open. Process exit releases ownership.
    pub(super) fn lock_session_driver(&self, session: &str) -> StoreResult<File> {
