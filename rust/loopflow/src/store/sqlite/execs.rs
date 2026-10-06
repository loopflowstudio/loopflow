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

    /// Record positive pre-spawn evidence under the exact admission fence.
    /// Once spawn is requested, missing process evidence remains unknown.
    pub(crate) fn record_session_provider_launch(
        &self,
        session: &str,
        expected: &SessionDriver,
        spawning: bool,
    ) -> StoreResult<()> {
        let mut conn = self.conn.lock().expect("store mutex poisoned");
        let tx = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
        if driver_in(&tx, session)?.as_ref() != Some(expected) || expected.exec_id.is_none() {
            return Err(StoreError::InvalidAuthority(
                "Session driver changed".into(),
            ));
        }
        let phase = if spawning {
            "spawn_requested"
        } else {
            "reserved"
        };
        tx.execute(
            "INSERT OR IGNORE INTO session_events(session_id,kind,receipt_key,exec_id,observed_at,payload,captured_event)
             SELECT id,'observed',?2,?3,?4,?5,current_capture FROM agent_sessions WHERE id=?1",
            params![session, format!("provider:{}:{phase}", expected.provider_generation),
                expected.exec_id, time::OffsetDateTime::now_utc().unix_timestamp(),
                serde_json::json!({"type":"provider_launch", "phase":phase,
                    "provider_generation":expected.provider_generation}).to_string()],
        )?;
        tx.commit()?;
        Ok(())
    }

    pub(crate) fn session_provider_unstarted(&self, session: &str) -> StoreResult<bool> {
        let conn = self.conn.lock().expect("store mutex poisoned");
        Ok(conn.query_row(
            "SELECT provider_pid IS NULL AND provider_endpoint IS NULL
             AND EXISTS(SELECT 1 FROM session_events e WHERE e.session_id=s.id
                AND e.receipt_key='provider:' || s.provider_generation || ':reserved')
             AND NOT EXISTS(SELECT 1 FROM session_events e WHERE e.session_id=s.id
                AND e.receipt_key='provider:' || s.provider_generation || ':spawn_requested')
             FROM agent_sessions s WHERE id=?1",
            [session],
            |row| row.get(0),
        )?)
    }

    /// Retain a boot witness for this released driver, without settling its
    /// unknown provider outcome. A later boot on the same host excludes all
    /// processes that could have survived that witness, including descendants.
    pub(crate) fn observe_session_recovery_boot(
        &self,
        session: &str,
        expected: &SessionDriver,
        boot: &crate::session_record::recovery::HostBoot,
    ) -> StoreResult<bool> {
        let mut conn = self.conn.lock().expect("store mutex poisoned");
        let tx = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
        if driver_in(&tx, session)?.as_ref() != Some(expected) || expected.exec_id.is_some() {
            return Err(StoreError::InvalidAuthority(
                "Session driver changed".into(),
            ));
        }
        let unidentified: bool = tx.query_row(
            "SELECT provider_pid IS NULL AND provider_endpoint IS NULL FROM agent_sessions WHERE id=?1",
            [session], |row| row.get(0),
        )?;
        if !unidentified {
            return Ok(false);
        }
        // A copied Home cannot witness the death of a provider on its origin
        // host. Establish locality from the provider Exec's retained capture,
        // then bind the witness to the machine's OS identity across restarts.
        let local: bool = tx.query_row(
            "SELECT EXISTS(SELECT 1 FROM session_events c JOIN session_events m ON m.captured_event=c.seq
             WHERE c.session_id=?1 AND c.kind='captured' AND c.exec_id=?2
             AND m.session_id=c.session_id AND m.kind='observed'
             AND m.receipt_key=c.receipt_key || ':manifest.json'
             AND json_extract(m.payload,'$.input_id')=c.receipt_key
             AND json_extract(m.payload,'$.source')='manifest.json'
             AND json_extract(m.payload,'$.evidence.schema_version')=1
             AND json_extract(m.payload,'$.evidence.artifact_key')=c.receipt_key
             AND json_extract(m.payload,'$.evidence.host')=?3)",
            params![session, expected.provider_exec_id, boot.host], |row| row.get(0),
        )?;
        if !local {
            return Ok(false);
        }
        let key = format!("driver:{}:recovery_boot", expected.generation);
        let previous: Option<String> = tx
            .query_row(
                "SELECT payload FROM session_events WHERE session_id=?1 AND receipt_key=?2",
                params![session, key],
                |row| row.get(0),
            )
            .optional()?;
        let previous = previous
            .map(|payload| serde_json::from_str::<serde_json::Value>(&payload))
            .transpose()?;
        let recovered = previous.as_ref().is_some_and(|payload| {
            payload["provider_generation"].as_i64() == Some(expected.provider_generation)
                && payload["provider_exec_id"].as_str() == Some(expected.provider_exec_id.as_str())
                && payload["host"]["machine"].as_str() == Some(boot.machine.as_str())
                && payload["host"]["boot"]
                    .as_str()
                    .is_some_and(|old| old != boot.boot)
        });
        let phase = if recovered {
            "recovered_after_restart"
        } else {
            "recovery_boot"
        };
        tx.execute(
            "INSERT OR IGNORE INTO session_events(session_id,kind,receipt_key,observed_at,payload,captured_event)
             SELECT id,'observed',?2,?3,?4,current_capture FROM agent_sessions WHERE id=?1",
            params![session, format!("driver:{}:{phase}", expected.generation),
                time::OffsetDateTime::now_utc().unix_timestamp(),
                serde_json::json!({"type":phase, "host":boot,
                    "provider_generation":expected.provider_generation,
                    "provider_exec_id":expected.provider_exec_id,
                    "previous":if recovered { previous } else { None }}).to_string()],
        )?;
        tx.commit()?;
        Ok(recovered)
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
        let database = self
            .conn
            .lock()
            .expect("store mutex poisoned")
            .path()
            .map(str::to_owned)
            .ok_or_else(|| {
                StoreError::InvalidData("Session dispatch requires a file-backed store".into())
            })?;
        let root = Path::new(&database)
            .canonicalize()
            .map_err(|error| StoreError::InvalidData(error.to_string()))?
            .with_extension("session-locks");
        std::fs::create_dir_all(&root)
            .map_err(|error| StoreError::InvalidData(error.to_string()))?;
        let file = OpenOptions::new()
            .create(true)
            .truncate(false)
            .read(true)
            .write(true)
            .open(root.join(hex::encode(Sha256::digest(session.as_bytes()))))
            .map_err(|error| StoreError::InvalidData(error.to_string()))?;
        // A synchronous caller must never wait indefinitely on a dispatch whose
        // reactor it might be driving. This bound uses the OS clock, not Tokio.
        let deadline = Instant::now() + Duration::from_secs(15);
        loop {
            match file.try_lock_exclusive() {
                Ok(()) => return Ok(file),
                Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {
                    if Instant::now() >= deadline {
                        return Err(StoreError::InvalidData(format!(
                            "Session {session} dispatch is still busy; retry after the current send finishes"
                        )));
                    }
                    std::thread::sleep(Duration::from_millis(10));
                }
                Err(error) => return Err(StoreError::InvalidData(error.to_string())),
            }
        }
    }

    /// Serialize native dispatch with driver transfer without blocking history
    /// or unrelated database writes while the bounded transport write runs.
    pub(crate) fn with_session_driver<T>(
        &self,
        session: &str,
        expected: &SessionDriver,
        write: impl FnOnce() -> StoreResult<T>,
    ) -> StoreResult<T> {
        let _dispatch = self.lock_session_driver(session)?;
        {
            let conn = self.conn.lock().expect("store mutex poisoned");
            if expected.exec_id.is_none() || driver_in(&conn, session)?.as_ref() != Some(expected) {
                return Err(StoreError::InvalidAuthority(
                    "Session driver changed".into(),
                ));
            }
        }
        write()
    }

    /// Read current conversation attribution and exact retained client membership.
    /// No captured input, history body, endpoint or command outcome establishes liveness.
    pub(crate) fn session_process_ownership(
        &self,
        inputs: &[String],
        execs: &[String],
        pids: &[u32],
    ) -> StoreResult<crate::exec::SessionProcessOwnership> {
        let mut conn = self.conn.lock().expect("store mutex poisoned");
        let tx = conn.transaction()?;
        let input_json = serde_json::to_string(inputs)?;
        let exec_json = serde_json::to_string(execs)?;
        let pid_json = serde_json::to_string(pids)?;
        let sessions = {
            let mut statement = tx.prepare(
                "SELECT s.id,s.title,s.task_id,s.wave_id,s.driver_exec_id,e.trace_id,
                    s.driver_generation,s.provider_pid,s.provider_started_at,s.provider_exec_id
                 FROM agent_sessions s LEFT JOIN execs e ON e.id=s.driver_exec_id
                 WHERE s.id IN (
                    SELECT id FROM agent_sessions WHERE driver_exec_id IN (SELECT value FROM json_each(?2))
                    UNION SELECT id FROM agent_sessions WHERE provider_pid IN (SELECT value FROM json_each(?3))
                    UNION SELECT id FROM agent_sessions WHERE provider_pid IS NULL
                        AND provider_exec_id IN (SELECT value FROM json_each(?2))
                    UNION SELECT session_id FROM session_events
                        WHERE kind='captured' AND receipt_key IN (SELECT value FROM json_each(?1)))
                 ORDER BY s.id",
            )?;
            let mut rows = statement.query(params![input_json, exec_json, pid_json])?;
            let mut sessions = Vec::new();
            while let Some(row) = rows.next()? {
                let task = row
                    .get::<_, Option<String>>(2)?
                    .as_deref()
                    .map(crate::durable::TaskId::parse)
                    .transpose()
                    .map_err(|error| StoreError::InvalidData(error.to_string()))?;
                let wave: Option<crate::id::WaveId> = row.get(3)?;
                sessions.push(crate::exec::SessionProcessObservation {
                    id: row.get(0)?,
                    title: row.get(1)?,
                    work: task
                        .map(crate::durable::WorkRef::Task)
                        .or_else(|| wave.map(crate::durable::WorkRef::Wave)),
                    driver_exec_id: row.get(4)?,
                    driver_trace_id: row.get(5)?,
                    driver_generation: row.get(6)?,
                    provider_pid: row.get(7)?,
                    provider_started_at: row.get(8)?,
                    provider_exec_id: row.get(9)?,
                });
            }
            sessions
        };
        let inputs = {
            let mut statement = tx.prepare(
                "SELECT receipt_key,session_id FROM session_events
                 WHERE kind='captured' AND receipt_key IN (SELECT value FROM json_each(?1))
                    AND session_id IS NOT NULL ORDER BY receipt_key",
            )?;
            let rows = statement
                .query_map([&input_json], |row| Ok((row.get(0)?, row.get(1)?)))?
                .collect::<rusqlite::Result<std::collections::BTreeMap<_, _>>>()?;
            rows
        };
        tx.commit()?;
        Ok(crate::exec::SessionProcessOwnership { sessions, inputs })
    }

    pub fn session_driver(&self, session: &str) -> StoreResult<Option<SessionDriver>> {
        let conn = self.conn.lock().expect("store mutex poisoned");
        driver_in(&conn, session)
    }

    pub fn claim_session_driver(
        &self,
        session: &str,
        expected: Option<&SessionDriver>,
        exec: &ExecId,
        replace_provider: bool,
    ) -> StoreResult<SessionDriver> {
        let _dispatch = self.lock_session_driver(session)?;
        let mut conn = self.conn.lock().expect("store mutex poisoned");
        let tx = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
        let closed_review: bool = tx.query_row(
            "SELECT kind='flow_review' AND completed_at IS NOT NULL FROM agent_sessions WHERE id=?1",
            [session], |row| row.get(0),
        )?;
        if closed_review {
            return Err(StoreError::InvalidAuthority(
                "review has been closed".into(),
            ));
        }
        let current = driver_in(&tx, session)?;
        if current.as_ref() != expected {
            return Err(StoreError::InvalidAuthority(
                "Session driver changed".into(),
            ));
        }
        let driver = SessionDriver {
            exec_id: Some(exec.clone()),
            generation: current.as_ref().map_or(1, |value| value.generation + 1),
            provider_generation: current.as_ref().map_or(1, |value| {
                value.provider_generation + i64::from(replace_provider)
            }),
            provider_exec_id: current
                .as_ref()
                .filter(|_| !replace_provider)
                .map_or_else(|| exec.clone(), |value| value.provider_exec_id.clone()),
        };
        tx.execute(
            "UPDATE agent_sessions SET driver_exec_id=?2,driver_generation=?3,
                provider_generation=?4,provider_exec_id=?5,
                provider_endpoint=CASE WHEN ?6 THEN NULL ELSE provider_endpoint END,
                provider_pid=CASE WHEN ?6 THEN NULL ELSE provider_pid END,
                provider_started_at=CASE WHEN ?6 THEN NULL ELSE provider_started_at END
             WHERE id=?1",
            params![
                session,
                driver.exec_id,
                driver.generation,
                driver.provider_generation,
                driver.provider_exec_id,
                replace_provider
            ],
        )?;
        tx.commit()?;
        Ok(driver)
    }

    pub fn release_session_driver(
        &self,
        session: &str,
        expected: &SessionDriver,
    ) -> StoreResult<SessionDriver> {
        let _dispatch = self.lock_session_driver(session)?;
        let mut conn = self.conn.lock().expect("store mutex poisoned");
        let tx = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
        let mut current = driver_in(&tx, session)?.ok_or(StoreError::NotFound)?;
        if current != *expected {
            return Err(StoreError::InvalidAuthority(
                "Session driver changed".into(),
            ));
        }
        current.exec_id = None;
        current.generation += 1;
        tx.execute(
            "UPDATE agent_sessions SET driver_exec_id=NULL,driver_generation=?2 WHERE id=?1",
            params![session, current.generation],
        )?;
        tx.commit()?;
        Ok(current)
    }

    /// Close the exact driver's runtime and record its exit under the same
    /// transaction as ownership transfer, without settling a Flow review.
    pub(crate) fn finish_session_driver(
        &self,
        session: &str,
        expected: &SessionDriver,
        outcome: &str,
        close_provider: impl FnOnce() -> StoreResult<bool>,
    ) -> StoreResult<()> {
        let _dispatch = self.lock_session_driver(session)?;
        let mut conn = self.conn.lock().expect("store mutex poisoned");
        let tx = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
        if driver_in(&tx, session)?.as_ref() != Some(expected) {
            return Err(StoreError::InvalidAuthority(
                "Session driver changed".into(),
            ));
        }
        let closed = close_provider()?;
        let now = time::OffsetDateTime::now_utc().unix_timestamp();
        let payload = serde_json::json!({
            "type": "driver_exit", "outcome": outcome, "generation": expected.generation
        });
        tx.execute(
            "INSERT INTO session_events(session_id,kind,receipt_key,exec_id,observed_at,payload,captured_event)
             SELECT id,'observed',?2,?3,?4,?5,current_capture FROM agent_sessions WHERE id=?1",
            params![session, format!("driver:{}:exit", expected.generation), expected.exec_id,
                now, payload.to_string()],
        )?;
        // Ordinary disposable conversations retire on an observed exit. Primary
        // conversations, Wave conversations, Task work and Flow reviews stay open.
        tx.execute(
            &format!(
                "UPDATE agent_sessions AS s SET completed_at=?2 WHERE s.id=?1
             AND s.completed_at IS NULL AND s.kind='conversation' AND s.primary_scope IS NULL
             AND s.wave_id IS NULL AND s.flow_session_id IS NULL
             AND s.driver_exec_id=s.provider_exec_id
             AND NOT EXISTS({}) AND ?3 IN ('completed','interrupted')",
                super::task_work::session_tasks("s")
            ),
            params![session, now, outcome],
        )?;
        tx.execute(
            "UPDATE agent_sessions SET driver_exec_id=NULL,driver_generation=driver_generation+1,
                provider_endpoint=CASE WHEN ?2 THEN NULL ELSE provider_endpoint END WHERE id=?1",
            params![session, closed],
        )?;
        tx.commit()?;
        Ok(())
    }

    /// Stale providers retain their historical caller, never the new driver.
    pub fn agent_parent(&self, caller: &AgentCaller) -> StoreResult<Option<(ExecId, String)>> {
        let conn = self.conn.lock().expect("store mutex poisoned");
        let driver = driver_in(&conn, &caller.session_id)?;
        let parent = driver
            .as_ref()
            .filter(|driver| {
                driver.provider_generation == caller.provider_generation
                    && driver.provider_exec_id == caller.origin_exec_id
            })
            .and_then(|driver| driver.exec_id.as_ref())
            .unwrap_or(&caller.origin_exec_id);
        conn.query_row("SELECT trace_id FROM execs WHERE id=?1", [parent], |row| {
            row.get::<_, String>(0)
        })
        .optional()
        .map(|trace| trace.map(|trace| (parent.clone(), trace)))
        .map_err(StoreError::from)
    }
}

#[cfg(test)]
mod discovery_tests {
    use std::num::NonZeroU32;

    use rusqlite::params;

    use crate::durable::{ProjectId, TaskId};
    use crate::exec::{ExecFilter, ExecOutcomeFilter, ExecWorkFilter};
    use crate::id::{ExecId, TraceId, WaveId};
    use crate::store::sqlite::SqliteStore;
    use crate::store::StoreError;

    fn insert_exec(store: &SqliteStore, number: u32, started_at: i64) -> ExecId {
        let id = ExecId::parse(&format!("00000000-0000-0000-0000-{number:012x}")).unwrap();
        store
            .conn
            .lock()
            .unwrap()
            .execute(
                "INSERT INTO execs(id,trace_id,started_at,command,repo,cwd)
             VALUES(?1,?2,?3,?4,'/repo','/missing-checkout')",
                params![
                    id,
                    TraceId::new(),
                    started_at,
                    serde_json::to_string(&["lf", "inspect"]).unwrap()
                ],
            )
            .unwrap();
        id
    }

    #[test]
    fn active_ownership_filters_dense_retained_history_before_materializing() {
        let dir = tempfile::tempdir().unwrap();
        let store = SqliteStore::open_ephemeral(&dir.path().join("store.db")).unwrap();
        let old = insert_exec(&store, 1, 1);
        let live = insert_exec(&store, 2, 2);
        let input = crate::session_record::new_artifact_key();
        {
            let conn = store.conn.lock().unwrap();
            conn.execute(
                "WITH RECURSIVE n(x) AS (VALUES(1) UNION ALL SELECT x+1 FROM n WHERE x<20000)
                INSERT INTO agent_sessions(id,title,title_source,created_at,completed_at,
                    input_published,cwd,provider_exec_id,provider_pid,provider_started_at)
                SELECT 'retained-'||x,'Retained','human',1,2,1,'/fixture',?1,
                    CASE WHEN x%2=0 THEN x+100000 END,1 FROM n",
                [&old],
            )
            .unwrap();
            conn.execute("UPDATE agent_sessions SET driver_exec_id=?1,completed_at=NULL WHERE id='retained-1'", [&live]).unwrap();
            conn.execute(
                "UPDATE agent_sessions SET provider_exec_id=?1 WHERE id='retained-3'",
                [&live],
            )
            .unwrap();
            super::super::sessions::test_capture(&conn, "retained-5", &input);
        }
        for (execs, pids, inputs, expected) in [
            (vec![], vec![], vec![], vec![]),
            (
                vec![live.to_string()],
                vec![100002],
                vec![input.clone()],
                vec!["retained-1", "retained-2", "retained-3", "retained-5"],
            ),
        ] {
            let started = std::time::Instant::now();
            for _ in 0..5 {
                let result = store
                    .session_process_ownership(&inputs, &execs, &pids)
                    .unwrap();
                assert_eq!(
                    result
                        .sessions
                        .iter()
                        .map(|s| s.id.as_str())
                        .collect::<Vec<_>>(),
                    expected
                );
                assert_eq!(result.inputs.len(), inputs.len());
            }
            eprintln!(
                "active ownership: 20000 retained, {} selected, five bundled-SQLite queries {:?}",
                expected.len(),
                started.elapsed()
            );
        }
    }

    #[test]
    fn exec_discovery_retains_command_evidence_without_payloads() {
        let dir = tempfile::tempdir().unwrap();
        let store = SqliteStore::open_ephemeral(&dir.path().join("store.db")).unwrap();
        let parent = insert_exec(&store, 1, 1);
        let direct = insert_exec(&store, 2, 2);
        let agent = insert_exec(&store, 3, 3);
        let interrupted = insert_exec(&store, 4, 4);
        {
            let conn = store.conn.lock().unwrap();
            conn.execute(
                "UPDATE execs SET parent_exec_id=?2,via_agent=0,outcome='failed',
                completed_at=5,exit_code=42 WHERE id=?1",
                params![direct, parent],
            )
            .unwrap();
            conn.execute("UPDATE execs SET parent_exec_id=?2,via_agent=1,caller_session_id='retained-caller',
                caller_provider_generation=7,outcome='succeeded',
                completed_at=6,exit_code=0 WHERE id=?1", params![agent,direct]).unwrap();
            conn.execute(
                "UPDATE execs SET outcome='interrupted',completed_at=7,exit_code=130
                WHERE id=?1",
                [&interrupted],
            )
            .unwrap();
        }
        let page = store
            .execs(&ExecFilter::default(), None, NonZeroU32::new(10).unwrap())
            .unwrap();
        assert_eq!(
            page.entries.iter().map(|e| &e.id).collect::<Vec<_>>(),
            vec![&interrupted, &agent, &direct, &parent]
        );
        assert_eq!(page.next, None);
        for entry in &page.entries {
            assert_eq!(store.exec(&entry.id).unwrap().as_ref(), Some(entry));
        }
        let old = &page.entries[3];
        assert_eq!(
            (old.via_agent, old.completed_at, old.exit_code),
            (None, None, None)
        );
        assert_eq!(old.outcome, None);
        assert_eq!(old.signal, None);
        let failed = &page.entries[2];
        assert_eq!(failed.parent_exec_id.as_ref(), Some(&parent));
        assert_eq!(failed.via_agent, Some(false));
        assert_eq!(failed.exit_code, Some(42));
        let success = &page.entries[1];
        assert_eq!(
            success.caller_session_id.as_deref(),
            Some("retained-caller")
        );
        assert_eq!(success.caller_provider_generation, Some(7));
        assert_eq!(success.via_agent, Some(true));
        assert_eq!(page.entries[0].signal, None);
        for (outcome, expected) in [
            (ExecOutcomeFilter::Unknown, &parent),
            (ExecOutcomeFilter::Failed, &direct),
            (ExecOutcomeFilter::Succeeded, &agent),
            (ExecOutcomeFilter::Interrupted, &interrupted),
        ] {
            let filter = ExecFilter {
                outcome: Some(outcome),
                ..Default::default()
            };
            let selected = store
                .execs(&filter, None, NonZeroU32::new(1).unwrap())
                .unwrap();
            assert_eq!(&selected.entries[0].id, expected);
            assert_eq!(selected.next, None);
        }
        assert!(!dir.path().join("runs").exists());
    }

    #[test]
    fn exec_discovery_separates_literal_search_from_identity_resolution() {
        let dir = tempfile::tempdir().unwrap();
        let store = SqliteStore::open_ephemeral(&dir.path().join("store.db")).unwrap();
        let first = insert_exec(&store, 1, 1);
        let second = insert_exec(&store, 16, 2);
        store
            .conn
            .lock()
            .unwrap()
            .execute(
                "UPDATE execs SET command=?3,parent_exec_id=?2,
                caller_session_id='caller' WHERE id=?1",
                params![
                    first,
                    second,
                    serde_json::to_string(&["lf", "pr", "land", "--strict", "%_ bytes"]).unwrap()
                ],
            )
            .unwrap();
        let filter = ExecFilter {
            repo: Some("/repo".into()),
            parent_exec_id: Some(second.clone()),
            caller_session_id: Some("caller".into()),
            command_contains: Some("PR LAND --strict %_".into()),
            identity_contains: Some("000000000001".into()),
            ..Default::default()
        };
        assert_eq!(
            store
                .execs(&filter, None, NonZeroU32::new(1).unwrap())
                .unwrap()
                .entries[0]
                .id,
            first
        );
        assert_eq!(
            store.resolve_exec(first.as_str()).unwrap().unwrap().id,
            first
        );
        assert_eq!(
            store
                .resolve_exec(&second.as_str()[..35])
                .unwrap()
                .unwrap()
                .id,
            second
        );
        assert!(matches!(
            store.resolve_exec("00000000"),
            Err(StoreError::InvalidData(_))
        ));
        assert_eq!(store.resolve_exec("%_").unwrap(), None);
        assert_eq!(store.exec(&ExecId::new()).unwrap(), None);
        let miss = ExecFilter {
            command_contains: Some("%_".into()),
            id: Some(second),
            ..Default::default()
        };
        assert!(store
            .execs(&miss, None, NonZeroU32::new(1).unwrap())
            .unwrap()
            .entries
            .is_empty());
        for command in [Some("[old pr land"), Some("{\"note\":\"pr land\"}"), None] {
            store
                .conn
                .lock()
                .unwrap()
                .execute(
                    "UPDATE execs SET command=?2 WHERE id=?1",
                    params![first, command],
                )
                .unwrap();
            let page = store
                .execs(
                    &ExecFilter {
                        id: Some(first.clone()),
                        command_contains: Some("pr land".into()),
                        ..Default::default()
                    },
                    None,
                    NonZeroU32::new(1).unwrap(),
                )
                .unwrap();
            assert_eq!(page.entries.len(), usize::from(command.is_some()));
            assert_eq!(
                store.exec(&first).unwrap().unwrap().command.as_deref(),
                command
            );
        }
    }

    #[test]
    fn exec_discovery_pages_fixed_data_after_filtering_with_timestamp_ties() {
        let dir = tempfile::tempdir().unwrap();
        let store = SqliteStore::open_ephemeral(&dir.path().join("store.db")).unwrap();
        let mut expected = Vec::new();
        for number in 1..=12 {
            let id = insert_exec(&store, number, i64::from(number / 5));
            if number % 2 == 0 {
                expected.push((i64::from(number / 5), id));
            } else {
                store
                    .conn
                    .lock()
                    .unwrap()
                    .execute("UPDATE execs SET repo='/other' WHERE id=?1", [id])
                    .unwrap();
            }
        }
        expected.sort_by(|a, b| b.0.cmp(&a.0).then_with(|| a.1.as_str().cmp(b.1.as_str())));
        let filter = ExecFilter {
            repo: Some("/repo".into()),
            ..Default::default()
        };
        let mut after = None;
        let mut found = Vec::new();
        loop {
            let page = store
                .execs(&filter, after.as_ref(), NonZeroU32::new(2).unwrap())
                .unwrap();
            assert!(page.entries.len() <= 2);
            found.extend(page.entries.into_iter().map(|entry| entry.id));
            after = page.next;
            if after.is_none() {
                break;
            }
            assert!(found.len() <= expected.len(), "cursor must make progress");
        }
        assert_eq!(
            found,
            expected.into_iter().map(|(_, id)| id).collect::<Vec<_>>()
        );
    }

    #[test]
    fn exec_discovery_deduplicates_performed_work_without_rebinding_history() {
        let dir = tempfile::tempdir().unwrap();
        let store = SqliteStore::open_ephemeral(&dir.path().join("store.db")).unwrap();
        let wave = WaveId::new();
        let project = ProjectId::new();
        let first = TaskId::new();
        let second = TaskId::new();
        let shared = insert_exec(&store, 1, 3);
        let mechanical = insert_exec(&store, 2, 2);
        let unbound = insert_exec(&store, 3, 1);
        let observer = insert_exec(&store, 4, 4);
        {
            let conn = store.conn.lock().unwrap();
            conn.execute(
                "INSERT INTO waves(id,name,repo,created_at) VALUES(?1,'proof','/repo',1)",
                [&wave],
            )
            .unwrap();
            conn.execute(
                "INSERT INTO projects(id,wave_id,external_project_id,created_at)
                VALUES(?1,?2,'project-proof',1)",
                params![project.as_str(), wave],
            )
            .unwrap();
            for (index, task) in [&first, &second].into_iter().enumerate() {
                conn.execute(
                    "INSERT INTO tasks(id,project_id,external_issue_id,issue_identifier,created_at)
                    VALUES(?1,?2,?3,?3,1)",
                    params![task.as_str(), project.as_str(), format!("PROOF-{index}")],
                )
                .unwrap();
                conn.execute("INSERT INTO agent_sessions(id,title,title_source,created_at,input_published,cwd,task_id,wave_id)
                    VALUES(?1,'Now bound','human',1,1,'/missing',?2,?3)",
                    params![format!("session-{index}"),task.as_str(),wave]).unwrap();
                super::super::sessions::test_capture(
                    &conn,
                    &format!("session-{index}"),
                    &format!("run_{index:032x}"),
                );
                conn.execute("INSERT INTO session_events(session_id,provider_thread,provider_turn,kind,receipt_key,exec_id,task_id,wave_id,observed_at,payload)
                    VALUES(?1,'thread','work','started','',?2,?3,?4,1,'unreadable payload')",
                    params![format!("session-{index}"),shared,task.as_str(),wave]).unwrap();
            }
            // Same Exec drove both Tasks, two turns and a mechanical boundary.
            conn.execute("INSERT INTO session_events(session_id,provider_thread,provider_turn,kind,receipt_key,exec_id,task_id,wave_id,observed_at,payload)
                VALUES('session-0','thread','retry','started','',?1,?2,?3,2,'{}')",params![shared,first.as_str(),wave]).unwrap();
            // Session is bound now; the earlier turn remains unassigned.
            conn.execute("INSERT INTO session_events(session_id,provider_thread,provider_turn,kind,receipt_key,exec_id,observed_at,payload)
                VALUES('session-0','thread','before-bind','started','',?1,1,'{}')",[&unbound]).unwrap();
            // Known Task, unknown original process: neither current driver nor observer is its owner.
            conn.execute("INSERT INTO session_events(session_id,provider_thread,provider_turn,kind,receipt_key,task_id,wave_id,observed_at,payload)
                VALUES('session-0','thread','unmapped','started','',?1,?2,1,'{}')",params![first.as_str(),wave]).unwrap();
            conn.execute("UPDATE agent_sessions SET driver_exec_id=?1,provider_exec_id=?1 WHERE id='session-0'",[&observer]).unwrap();
            conn.execute(
                "UPDATE execs SET caller_session_id='session-0',via_agent=1 WHERE id=?1",
                [&observer],
            )
            .unwrap();
            conn.execute("INSERT INTO flow_sessions(id,task_id,wave_id,invocation_json,step_index,iteration,position_version,
                worker_generation,updated_at,state,ended_at) VALUES('mechanical',?1,?2,'{\"id\":\"mechanical\"}',0,0,1,0,1,'completed',2)",params![first.as_str(),wave]).unwrap();
            for exec in [&shared, &mechanical] {
                conn.execute(
                    "INSERT INTO flow_events(flow_id,node,iterations,kind,exec_id,payload)
                    VALUES('mechanical',0,'[]','operation_started',?1,'unreadable payload')",
                    [exec],
                )
                .unwrap();
            }
        }
        let filter = ExecFilter {
            performed_work: Some(ExecWorkFilter::Task(first.clone())),
            ..Default::default()
        };
        let page = store
            .execs(&filter, None, NonZeroU32::new(1).unwrap())
            .unwrap();
        assert_eq!(
            page.entries.iter().map(|e| &e.id).collect::<Vec<_>>(),
            vec![&shared]
        );
        let page = store
            .execs(&filter, page.next.as_ref(), NonZeroU32::new(1).unwrap())
            .unwrap();
        assert_eq!(page.entries[0].id, mechanical);
        assert_eq!(page.next, None);
        let second_filter = ExecFilter {
            performed_work: Some(ExecWorkFilter::Task(second)),
            ..Default::default()
        };
        let page = store
            .execs(&second_filter, None, NonZeroU32::new(10).unwrap())
            .unwrap();
        assert_eq!(
            page.entries.iter().map(|e| &e.id).collect::<Vec<_>>(),
            vec![&shared]
        );
        let wave_filter = ExecFilter {
            performed_work: Some(ExecWorkFilter::Wave(wave)),
            ..Default::default()
        };
        let page = store
            .execs(&wave_filter, None, NonZeroU32::new(10).unwrap())
            .unwrap();
        assert_eq!(
            page.entries.iter().map(|e| &e.id).collect::<Vec<_>>(),
            vec![&shared, &mechanical]
        );
        assert_eq!(store.exec(&unbound).unwrap().unwrap().outcome, None);
        assert_eq!(store.exec(&observer).unwrap().unwrap().outcome, None);
        assert!(store
            .conn
            .lock()
            .unwrap()
            .prepare("PRAGMA foreign_key_check")
            .unwrap()
            .query([])
            .unwrap()
            .next()
            .unwrap()
            .is_none());
        assert!(!dir.path().join("runs").exists());
    }
}
