use std::fs::{File, OpenOptions};
use std::num::NonZeroU32;
use std::path::Path;
use std::time::{Duration, Instant};

use fs2::FileExt;
use rusqlite::types::Value;
use rusqlite::{params, params_from_iter, OptionalExtension, TransactionBehavior};
use sha2::{Digest, Sha256};

use crate::id::{AttachmentToken, ProcessLfid};
use crate::process::{
    AgentCaller, Process, ProcessCursor, ProcessFilter, ProcessOutcomeFilter, ProcessPage,
    ProcessWorkFilter, SessionAttachment,
};
use crate::store::{StoreError, StoreResult};

use super::SqliteStore;

pub(super) const PROCESS_SELECT: &str =
    "SELECT e.lfid,e.trace_id,e.parent_process_lfid,e.via_agent,e.caller_session_id,
    e.caller_provider_generation,e.command,e.repo,e.cwd,
    e.started_at,e.completed_at,e.outcome,e.exit_code,e.signal,e.error,e.pid,e.kind,e.agent_session_id,e.os_started_at FROM processes e";

pub(super) fn read_process(row: &rusqlite::Row<'_>) -> rusqlite::Result<Process> {
    Ok(Process {
        lfid: row.get(0)?,
        pid: row.get(15)?,
        kind: match row.get::<_, String>(16)?.as_str() {
            "agent" => crate::process::ProcessKind::Agent,
            _ => crate::process::ProcessKind::Lf,
        },
        agent_session_id: row.get(17)?,
        os_started_at: row.get(18)?,
        trace_id: row.get(1)?,
        parent_process_lfid: row.get(2)?,
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

fn process_query(
    filter: &ProcessFilter,
    after: Option<&ProcessCursor>,
    limit: NonZeroU32,
) -> (String, Vec<Value>) {
    let mut sql = String::from("WITH page AS MATERIALIZED (SELECT e.lfid FROM processes e WHERE 1");
    let mut values = Vec::new();
    let mut bind = |value| {
        values.push(value);
        format!("?{}", values.len())
    };
    for (column, value) in [
        ("lfid", filter.lfid.as_ref().map(ProcessLfid::as_str)),
        ("repo", filter.repo.as_deref()),
        (
            "parent_process_lfid",
            filter.parent_process_lfid.as_ref().map(ProcessLfid::as_str),
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
            " AND instr(e.lfid,{})>0",
            bind(Value::Text(value.clone()))
        ));
    }
    if let Some(outcome) = filter.outcome {
        sql.push_str(match outcome {
            ProcessOutcomeFilter::Succeeded => " AND e.outcome='succeeded'",
            ProcessOutcomeFilter::Failed => " AND e.outcome='failed'",
            ProcessOutcomeFilter::Interrupted => " AND e.outcome='interrupted'",
            ProcessOutcomeFilter::Unknown => " AND e.outcome IS NULL",
        });
    }
    if let Some(work) = &filter.performed_work {
        let (column, value, tasks, close) = match work {
            ProcessWorkFilter::Task(id) => ("task_id", id.as_str(), "tw.id=", ""),
            ProcessWorkFilter::Wave(id) => (
                "wave_id",
                id.as_str(),
                "tw.project_id IN (SELECT id FROM projects WHERE wave_id=",
                ")",
            ),
        };
        let value = bind(Value::Text(value.into()));
        // Native starts retain their original assignment. A Flow's steps ran
        // in their Task's checkout. Neither a current Session bind nor
        // another Process's command context establishes performed work.
        sql.push_str(&format!(
            " AND e.lfid IN (
            SELECT process_lfid FROM session_events
            WHERE kind='started' AND {column}={value} AND process_lfid IS NOT NULL
            UNION
            SELECT op.lfid FROM flow_process_steps fs JOIN processes op ON op.lfid=fs.process_lfid
            JOIN tasks tw ON {tasks}{value}{close}
            WHERE tw.worktree!=''
              AND (op.cwd=rtrim(tw.worktree,'/') OR instr(op.cwd,rtrim(tw.worktree,'/')||'/')=1)
        )"
        ));
    }
    if let Some(after) = after {
        let time = bind(Value::Integer(after.started_at));
        let id = bind(Value::Text(after.lfid.to_string()));
        sql.push_str(&format!(
            " AND e.started_at<={time} AND (e.started_at<{time} OR e.lfid>{id})"
        ));
    }
    let limit = bind(Value::Integer(i64::from(limit.get()) + 1));
    sql.push_str(&format!(
        " ORDER BY e.started_at DESC,e.lfid ASC LIMIT {limit})
         {PROCESS_SELECT} JOIN page ON page.lfid=e.lfid ORDER BY e.started_at DESC,e.lfid ASC"
    ));
    (sql, values)
}

fn attachment_in(
    conn: &rusqlite::Connection,
    session: &str,
) -> StoreResult<Option<SessionAttachment>> {
    let row = conn
        .query_row(
            "SELECT p.attached_process_lfid,p.attachment_token,p.provider_generation,p.parent_process_lfid,p.lfid
         FROM agent_sessions s LEFT JOIN processes p ON p.lfid=s.agent_process_lfid WHERE s.id=?1",
            [session],
            |row| {
                Ok((
                    row.get::<_, Option<String>>(0)?,
                    row.get::<_, Option<AttachmentToken>>(1)?,
                    row.get::<_, Option<i64>>(2)?,
                    row.get::<_, Option<String>>(3)?,
                    row.get::<_, Option<ProcessLfid>>(4)?,
                ))
            },
        )
        .optional()?
        .ok_or(StoreError::NotFound)?;
    let Some(token) = row.1 else {
        return Ok(None);
    };
    let parse =
        |value: &str| ProcessLfid::parse(value).map_err(|e| StoreError::InvalidData(e.to_string()));
    Ok(Some(SessionAttachment {
        process_lfid: row.0.as_deref().map(parse).transpose()?,
        token,
        agent_process_lfid: row.4.ok_or(StoreError::NotFound)?,
        provider_generation: row.2.unwrap_or(0),
        provider_process_lfid: parse(row.3.as_deref().ok_or_else(|| {
            StoreError::InvalidData("Session driver has no provider origin".into())
        })?)?,
    }))
}

/// Require the current attached lf invocation, not merely a retained agent row.
fn require_attachment_in(
    conn: &rusqlite::Connection,
    session: &str,
    expected: &SessionAttachment,
) -> StoreResult<()> {
    if attachment_in(conn, session)?.as_ref() != Some(expected) || expected.process_lfid.is_none() {
        return Err(StoreError::InvalidAuthority(
            "Session attachment changed".into(),
        ));
    }
    Ok(())
}

pub(super) fn attach_in(
    tx: &rusqlite::Transaction<'_>,
    session: &str,
    expected: Option<&SessionAttachment>,
    process: &ProcessLfid,
    replace_provider: bool,
) -> StoreResult<SessionAttachment> {
    let current = attachment_in(tx, session)?;
    if current.as_ref() != expected {
        return Err(StoreError::InvalidAuthority(
            "Session attachment changed".into(),
        ));
    }
    let replacing = current.is_none() || replace_provider;
    let driver = SessionAttachment {
        agent_process_lfid: current
            .as_ref()
            .filter(|_| !replace_provider)
            .map_or_else(ProcessLfid::new, |value| value.agent_process_lfid.clone()),
        process_lfid: Some(process.clone()),
        token: AttachmentToken::new(),
        provider_generation: current.as_ref().map_or(1, |value| {
            value.provider_generation + i64::from(replace_provider)
        }),
        provider_process_lfid: current.as_ref().filter(|_| !replace_provider).map_or_else(
            || process.clone(),
            |value| value.provider_process_lfid.clone(),
        ),
    };
    if replacing {
        if let Some(previous) = &current {
            tx.execute(
                "UPDATE processes SET attached_process_lfid=NULL,attachment_token=?2 WHERE lfid=?1",
                params![previous.agent_process_lfid, AttachmentToken::new()],
            )?;
        }
        tx.execute(
            "INSERT INTO processes(lfid,kind,trace_id,parent_process_lfid,command,repo,cwd,
                started_at,agent_session_id,agent_provider,agent_interactive,provider_generation,spawn_state)
             SELECT ?2,'agent',COALESCE((SELECT trace_id FROM processes WHERE lfid=?3),?2),
                ?3,s.provider,s.repo,s.cwd,?4,s.id,s.provider,s.interactive,?5,'reserved' FROM agent_sessions s WHERE s.id=?1",
            params![session,driver.agent_process_lfid,process,
                time::OffsetDateTime::now_utc().unix_timestamp(),driver.provider_generation],
        )?;
        tx.execute(
            "UPDATE agent_sessions SET agent_process_lfid=?2 WHERE id=?1",
            params![session, driver.agent_process_lfid],
        )?;
    }
    tx.execute(
        "UPDATE processes SET attached_process_lfid=?2,attachment_token=?3,attachment_exit_seq=NULL
         WHERE lfid=?1",
        params![driver.agent_process_lfid, driver.process_lfid, driver.token],
    )?;
    Ok(driver)
}

impl SqliteStore {
    pub fn processes_since(&self, since: i64) -> StoreResult<Vec<Process>> {
        let conn = self.conn.lock().expect("store mutex poisoned");
        let mut query = conn.prepare(&format!(
            "{PROCESS_SELECT} WHERE e.started_at>=?1 ORDER BY e.started_at,e.lfid"
        ))?;
        let records = query
            .query_map([since], read_process)?
            .collect::<Result<Vec<_>, _>>()?;
        Ok(records)
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
    pub fn process(&self, id: &ProcessLfid) -> StoreResult<Option<Process>> {
        let conn = self.conn.lock().expect("store mutex poisoned");
        Ok(conn
            .query_row(
                &format!("{PROCESS_SELECT} WHERE e.lfid=?1"),
                [id],
                read_process,
            )
            .optional()?)
    }

    /// Exact identity wins; otherwise require a unique literal, case-sensitive prefix.
    pub fn resolve_process(&self, selector: &str) -> StoreResult<Option<Process>> {
        let conn = self.conn.lock().expect("store mutex poisoned");
        if let Some(process) = conn
            .query_row(
                &format!("{PROCESS_SELECT} WHERE e.lfid=?1"),
                [selector],
                read_process,
            )
            .optional()?
        {
            return Ok(Some(process));
        }
        // Process IDs are UUID text. This range seeks the existing identity index;
        // '%' and '_' have no wildcard meaning, unlike LIKE/GLOB.
        let upper = format!("{selector}\u{10ffff}");
        let mut query = conn.prepare(
            "SELECT lfid FROM processes WHERE lfid>=?1 AND lfid<?2 ORDER BY lfid LIMIT 2",
        )?;
        let ids = query
            .query_map(params![selector, upper], |row| row.get::<_, ProcessLfid>(0))?
            .collect::<rusqlite::Result<Vec<_>>>()?;
        match ids.as_slice() {
            [] => Ok(None),
            [id] => Ok(Some(conn.query_row(
                &format!("{PROCESS_SELECT} WHERE e.lfid=?1"),
                [id],
                read_process,
            )?)),
            _ => Err(StoreError::InvalidData(format!(
                "Ambiguous Process prefix {selector:?}"
            ))),
        }
    }

    /// Filter/deduplicate in SQL; decode only a bounded page plus one lookahead.
    /// Missing payloads do not remove rows. No history body is read for discovery.
    pub fn processes(
        &self,
        filter: &ProcessFilter,
        after: Option<&ProcessCursor>,
        limit: NonZeroU32,
    ) -> StoreResult<ProcessPage> {
        let (sql, values) = process_query(filter, after, limit);
        let conn = self.conn.lock().expect("store mutex poisoned");
        let mut query = conn.prepare(&sql)?;
        let mut entries = query
            .query_map(params_from_iter(values), read_process)?
            .collect::<rusqlite::Result<Vec<_>>>()?;
        let next = if entries.len() > limit.get() as usize {
            entries.pop();
            entries.last().map(|process| ProcessCursor {
                started_at: process.started_at,
                lfid: process.lfid.clone(),
            })
        } else {
            None
        };
        Ok(ProcessPage { entries, next })
    }

    pub(crate) fn make_session_interactive(
        &self,
        session: &str,
        expected: &SessionAttachment,
    ) -> StoreResult<()> {
        let mut conn = self.conn.lock().expect("store mutex poisoned");
        let tx = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
        require_attachment_in(&tx, session, expected)?;
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
            "SELECT p.endpoint,s.provider_thread FROM agent_sessions s JOIN processes p ON p.lfid=s.agent_process_lfid WHERE s.id=?1 AND p.endpoint IS NOT NULL AND s.provider_thread IS NOT NULL",
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
                "SELECT p.pid,p.os_started_at FROM agent_sessions s JOIN processes p ON p.lfid=s.agent_process_lfid WHERE s.id=?1
             AND p.pid IS NOT NULL AND p.os_started_at IS NOT NULL",
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
        expected: &SessionAttachment,
        command: &std::process::Command,
    ) -> StoreResult<()> {
        let mut conn = self.conn.lock().expect("store mutex poisoned");
        let tx = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
        require_attachment_in(&tx, session, expected)?;
        let argv = std::iter::once(command.get_program())
            .chain(command.get_args())
            .map(|arg| arg.to_string_lossy().into_owned())
            .collect::<Vec<_>>();
        tx.execute("UPDATE processes SET spawn_state='spawn_requested',command=?2 WHERE lfid=?1 AND completed_at IS NULL",
            params![expected.agent_process_lfid,serde_json::to_string(&argv)?])?;
        tx.commit()?;
        Ok(())
    }

    /// Only the terminal launcher that owns this generation may report its
    /// spawn failure or the exit it actually waited for.
    pub(crate) fn record_native_provider_exit(
        &self,
        session: &str,
        expected: &SessionAttachment,
        spawned: bool,
    ) -> StoreResult<()> {
        let mut conn = self.conn.lock().expect("store mutex poisoned");
        let tx = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
        require_attachment_in(&tx, session, expected)?;
        tx.execute(
            "UPDATE processes SET spawn_state=?2,completed_at=?3 WHERE lfid=?1",
            params![
                expected.agent_process_lfid,
                if spawned { "exited" } else { "spawn_failed" },
                time::OffsetDateTime::now_utc().unix_timestamp()
            ],
        )?;
        tx.commit()?;
        Ok(())
    }

    pub(crate) fn record_session_provider_process(
        &self,
        session: &str,
        expected: &SessionAttachment,
        pid: u32,
        started_at: i64,
    ) -> StoreResult<()> {
        let mut conn = self.conn.lock().expect("store mutex poisoned");
        let tx = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
        require_attachment_in(&tx, session, expected)?;
        let changed = tx.execute(
            "UPDATE processes SET pid=?2,os_started_at=?3 WHERE lfid=?1
             AND ((pid IS NULL AND os_started_at IS NULL) OR (pid=?2 AND os_started_at=?3))",
            params![expected.agent_process_lfid, pid, started_at],
        )?;
        if changed != 1 {
            return Err(StoreError::InvalidAuthority(
                "AgentProcess already records another OS process; a new launch needs a new record"
                    .into(),
            ));
        }
        tx.commit()?;
        Ok(())
    }

    pub fn record_session_connection(
        &self,
        session: &str,
        expected: &SessionAttachment,
        endpoint: &str,
        thread: &str,
    ) -> StoreResult<()> {
        let mut conn = self.conn.lock().expect("store mutex poisoned");
        let tx = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
        require_attachment_in(&tx, session, expected)?;
        tx.execute(
            "UPDATE processes SET endpoint=?2 WHERE lfid=?1",
            params![expected.agent_process_lfid, endpoint],
        )?;
        tx.execute(
            "UPDATE agent_sessions SET provider_thread=?2 WHERE id=?1",
            params![session, thread],
        )?;
        tx.commit()?;
        Ok(())
    }

    // Dispatch and driver changes share a per-Session OS lock, never a SQLite
    // transaction across provider I/O. Do not unlink lock files: another process
    // may already have the inode open. Process exit releases ownership.
    pub(super) fn lock_session_attachment(&self, session: &str) -> StoreResult<File> {
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
    pub(crate) fn with_session_attachment<T>(
        &self,
        session: &str,
        expected: &SessionAttachment,
        write: impl FnOnce() -> StoreResult<T>,
    ) -> StoreResult<T> {
        let _dispatch = self.lock_session_attachment(session)?;
        {
            let conn = self.conn.lock().expect("store mutex poisoned");
            require_attachment_in(&conn, session, expected)?;
        }
        write()
    }

    /// All unfinished agent rows, including detached and replaced processes.
    pub(crate) fn agent_processes(&self) -> StoreResult<Vec<crate::process::AgentProcess>> {
        let conn = self.conn.lock().expect("store mutex poisoned");
        let mut query = conn.prepare(&format!("{} WHERE e.kind='agent' AND e.completed_at IS NULL ORDER BY e.lfid",
            PROCESS_SELECT.replace(" FROM processes e",",e.attached_process_lfid,e.attachment_token,e.agent_provider,e.agent_interactive FROM processes e")))?;
        let rows = query
            .query_map([], |row| {
                Ok(crate::process::AgentProcess {
                    process: read_process(row)?,
                    attached_process_lfid: row.get(19)?,
                    attachment_token: row.get(20)?,
                    provider: row.get(21)?,
                    interactive: row.get(22)?,
                })
            })?
            .collect::<rusqlite::Result<Vec<_>>>()?;
        Ok(rows)
    }

    /// Recheck the exact record and attachment while excluding native takeover.
    /// Only positive OS death evidence (including successful termination) settles it.
    pub(crate) fn settle_agent_process(
        &self,
        expected: &crate::process::AgentProcess,
        terminate: impl FnOnce() -> StoreResult<()>,
    ) -> StoreResult<bool> {
        let session = expected
            .process
            .agent_session_id
            .as_deref()
            .ok_or(StoreError::NotFound)?;
        let _dispatch = self.lock_session_attachment(session)?;
        let matches: bool = {
            let conn = self.conn.lock().expect("store mutex poisoned");
            conn.query_row(
                "SELECT EXISTS(SELECT 1 FROM processes WHERE lfid=?1 AND kind='agent'
              AND pid IS ?2 AND os_started_at IS ?3 AND attached_process_lfid IS ?4
              AND attachment_token IS ?5 AND completed_at IS NULL)",
                params![
                    expected.process.lfid,
                    expected.process.pid,
                    expected.process.os_started_at,
                    expected.attached_process_lfid,
                    expected.attachment_token
                ],
                |row| row.get(0),
            )?
        };
        if !matches {
            return Ok(false);
        }
        terminate()?;
        let conn = self.conn.lock().expect("store mutex poisoned");
        conn.execute(
            "UPDATE processes SET completed_at=?2,spawn_state='exited',endpoint=NULL WHERE lfid=?1",
            params![
                expected.process.lfid,
                time::OffsetDateTime::now_utc().unix_timestamp()
            ],
        )?;
        Ok(true)
    }

    pub fn session_attachment(&self, session: &str) -> StoreResult<Option<SessionAttachment>> {
        let conn = self.conn.lock().expect("store mutex poisoned");
        attachment_in(&conn, session)
    }

    pub fn claim_session_attachment(
        &self,
        session: &str,
        expected: Option<&SessionAttachment>,
        process: &ProcessLfid,
        replace_provider: bool,
    ) -> StoreResult<SessionAttachment> {
        let _dispatch = self.lock_session_attachment(session)?;
        let mut conn = self.conn.lock().expect("store mutex poisoned");
        let tx = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
        let driver = attach_in(&tx, session, expected, process, replace_provider)?;
        tx.commit()?;
        Ok(driver)
    }

    /// Reserve the next OS process for the same invocation after an observed
    /// exit. An uncertain spawn cannot be retried by overwriting its identity.
    pub(crate) fn prepare_session_agent_process(
        &self,
        session: &str,
        expected: &SessionAttachment,
    ) -> StoreResult<SessionAttachment> {
        let _dispatch = self.lock_session_attachment(session)?;
        let mut conn = self.conn.lock().expect("store mutex poisoned");
        let tx = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
        require_attachment_in(&tx, session, expected)?;
        let (ended, reserved): (bool, bool) = tx.query_row(
            "SELECT completed_at IS NOT NULL,spawn_state='reserved' AND pid IS NULL
             FROM processes WHERE lfid=?1",
            [&expected.agent_process_lfid],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )?;
        let attachment = if ended {
            attach_in(
                &tx,
                session,
                Some(expected),
                expected.process_lfid.as_ref().expect("attachment is owned"),
                true,
            )?
        } else if reserved {
            expected.clone()
        } else {
            return Err(StoreError::InvalidAuthority(
                "Previous AgentProcess has no observed exit".into(),
            ));
        };
        tx.commit()?;
        Ok(attachment)
    }

    pub fn release_session_attachment(
        &self,
        session: &str,
        expected: &SessionAttachment,
    ) -> StoreResult<SessionAttachment> {
        let _dispatch = self.lock_session_attachment(session)?;
        let mut conn = self.conn.lock().expect("store mutex poisoned");
        let tx = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
        let mut current = attachment_in(&tx, session)?.ok_or(StoreError::NotFound)?;
        if current != *expected {
            return Err(StoreError::InvalidAuthority(
                "Session attachment changed".into(),
            ));
        }
        current.process_lfid = None;
        current.token = AttachmentToken::new();
        tx.execute(
            "UPDATE processes SET attached_process_lfid=NULL,attachment_token=?2 WHERE lfid=?1",
            params![current.agent_process_lfid, current.token],
        )?;
        tx.commit()?;
        Ok(current)
    }

    /// Close and settle the exact attachment under the transfer lock. Provider
    /// I/O holds no SQLite lock; the terminal writes commit together afterward.
    pub(crate) fn finish_session_attachment(
        &self,
        session: &str,
        expected: &SessionAttachment,
        outcome: &str,
        close_provider: impl FnOnce() -> StoreResult<bool>,
    ) -> StoreResult<()> {
        let _dispatch = self.lock_session_attachment(session)?;
        {
            let conn = self.conn.lock().expect("store mutex poisoned");
            if attachment_in(&conn, session)?.as_ref() != Some(expected) {
                return Err(StoreError::InvalidAuthority(
                    "Session attachment changed".into(),
                ));
            }
        }
        let closed = close_provider()?;
        let mut conn = self.conn.lock().expect("store mutex poisoned");
        let tx = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
        let now = time::OffsetDateTime::now_utc().unix_timestamp();
        let payload = serde_json::json!({
            "type": "attachment_exit", "outcome": outcome, "attachment_token": expected.token
        });
        let exit_seq: i64 = tx.query_row(
            "INSERT INTO session_events(session_id,kind,receipt_key,process_lfid,observed_at,payload,captured_event)
             SELECT id,'observed',?2,?3,?4,?5,current_capture FROM agent_sessions WHERE id=?1
             RETURNING seq",
            params![session, format!("attachment:{}:exit", expected.token), expected.process_lfid,
                now, payload.to_string()],
            |row| row.get(0),
        )?;
        // Ordinary disposable conversations retire on an observed exit. Primary
        // conversations, Wave conversations, Task work and Flow reviews stay open.
        tx.execute(
            &format!(
                "UPDATE agent_sessions AS s SET completed_at=?2 WHERE s.id=?1
             AND s.completed_at IS NULL AND s.primary_scope IS NULL
             AND s.wave_id IS NULL AND {} IS NULL
             AND EXISTS(SELECT 1 FROM processes p WHERE p.lfid=s.agent_process_lfid AND p.attached_process_lfid=p.parent_process_lfid)
             AND NOT EXISTS({}) AND ?3 IN ('completed','interrupted')",
                super::sessions::SESSION_FLOW,
                super::task_work::session_tasks("s")
            ),
            params![session, now, outcome],
        )?;
        tx.execute(
            "UPDATE processes SET attached_process_lfid=NULL,attachment_token=?3,
                attachment_exit_seq=?4,endpoint=CASE WHEN ?2 THEN NULL ELSE endpoint END,
                completed_at=CASE WHEN ?2 THEN COALESCE(completed_at,?5) ELSE completed_at END,
                spawn_state=CASE WHEN ?2 THEN 'exited' ELSE spawn_state END WHERE lfid=?1",
            params![
                expected.agent_process_lfid,
                closed,
                AttachmentToken::new(),
                exit_seq,
                now
            ],
        )?;
        tx.commit()?;
        Ok(())
    }

    /// Stale providers retain their historical caller, never the new driver.
    pub fn agent_parent(&self, caller: &AgentCaller) -> StoreResult<Option<(ProcessLfid, String)>> {
        let conn = self.conn.lock().expect("store mutex poisoned");
        let driver = attachment_in(&conn, &caller.session_id)?;
        let parent = driver
            .as_ref()
            .filter(|driver| {
                driver.provider_generation == caller.provider_generation
                    && driver.provider_process_lfid == caller.origin_process_lfid
            })
            .and_then(|driver| driver.process_lfid.as_ref())
            .unwrap_or(&caller.origin_process_lfid);
        conn.query_row(
            "SELECT trace_id FROM processes WHERE lfid=?1",
            [parent],
            |row| row.get::<_, String>(0),
        )
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
    use crate::id::{ProcessLfid, TraceId, WaveId};
    use crate::process::{ProcessFilter, ProcessOutcomeFilter, ProcessWorkFilter};
    use crate::store::sqlite::SqliteStore;
    use crate::store::StoreError;

    pub(super) fn insert_process(store: &SqliteStore, number: u32, started_at: i64) -> ProcessLfid {
        let id = ProcessLfid::parse(&format!("00000000-0000-0000-0000-{number:012x}")).unwrap();
        store
            .conn
            .lock()
            .unwrap()
            .execute(
                "INSERT INTO processes(lfid,trace_id,started_at,command,repo,cwd)
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
    fn losing_input_claim_rolls_back_capture_and_provider_reservation() {
        let home = tempfile::tempdir().unwrap();
        let store = SqliteStore::open_ephemeral(&home.path().join("store.db")).unwrap();
        let session =
            store.test_session("conversation", &crate::session_record::new_artifact_key());
        let first = insert_process(&store, 1, 1);
        let second = insert_process(&store, 2, 2);
        let driver = store
            .claim_session_attachment(&session.id, None, &first, true)
            .unwrap();
        let mut next = session.clone();
        next.artifact_key = crate::session_record::new_artifact_key();
        next.input_published = false;
        let before = store.session_history(&session.id, 0, 0).unwrap();
        assert!(store
            .claim_session_input(next.clone(), None, &second, true)
            .is_err());
        assert_eq!(store.session(&session.id).unwrap().unwrap(), session);
        assert_eq!(
            store.session_attachment(&session.id).unwrap(),
            Some(driver.clone())
        );
        assert_eq!(store.session_history(&session.id, 0, 0).unwrap(), before);
        assert!(store
            .session_for_artifact(&next.artifact_key)
            .unwrap()
            .is_none());

        let (admitted, claimed) = store
            .claim_session_input(next.clone(), Some(&driver), &second, true)
            .unwrap();
        assert_eq!(admitted.artifact_key, next.artifact_key);
        assert_ne!(admitted.captured, session.captured);
        assert_eq!(claimed.process_lfid, Some(second.clone()));
        let history = store.session_history(&session.id, 0, 0).unwrap();
        let reserved = store.process(&claimed.agent_process_lfid).unwrap().unwrap();
        assert_eq!(
            reserved.agent_session_id.as_deref(),
            Some(session.id.as_str())
        );
        assert_eq!(reserved.parent_process_lfid.as_ref(), Some(&second));
        assert!(reserved.completed_at.is_none());
        next.artifact_key = crate::session_record::new_artifact_key();
        assert!(store
            .claim_session_input(next, Some(&claimed), &first, true)
            .is_err());
        assert_eq!(store.session_history(&session.id, 0, 0).unwrap(), history);
    }

    #[test]
    fn durable_identity_keeps_reused_and_unknown_pids_distinct() {
        let dir = tempfile::tempdir().unwrap();
        let store = SqliteStore::open_ephemeral(&dir.path().join("store.db")).unwrap();
        let historical = insert_process(&store, 1, 1);
        let retained = store.process(&historical).unwrap().unwrap();
        assert_eq!(retained.pid, None);
        let mut first = retained.clone();
        first.lfid = ProcessLfid::new();
        first.pid = Some(4242);
        store.record_process(&first).unwrap();
        let mut second = first.clone();
        second.lfid = ProcessLfid::new();
        second.parent_process_lfid = Some(first.lfid.clone());
        store.record_process(&second).unwrap();
        first.completed_at = Some(3);
        first.outcome = Some("failed".into());
        first.exit_code = Some(42);
        store.record_process(&first).unwrap();
        assert_eq!(store.process(&first.lfid).unwrap(), Some(first));
        assert_eq!(store.process(&second.lfid).unwrap(), Some(second));
        assert_eq!(store.process(&historical).unwrap(), Some(retained));
    }

    #[test]
    fn process_discovery_retains_command_evidence_without_payloads() {
        let dir = tempfile::tempdir().unwrap();
        let store = SqliteStore::open_ephemeral(&dir.path().join("store.db")).unwrap();
        let parent = insert_process(&store, 1, 1);
        let direct = insert_process(&store, 2, 2);
        let agent = insert_process(&store, 3, 3);
        let interrupted = insert_process(&store, 4, 4);
        {
            let conn = store.conn.lock().unwrap();
            conn.execute(
                "UPDATE processes SET parent_process_lfid=?2,via_agent=0,outcome='failed',
                completed_at=5,exit_code=42 WHERE lfid=?1",
                params![direct, parent],
            )
            .unwrap();
            conn.execute("UPDATE processes SET parent_process_lfid=?2,via_agent=1,caller_session_id='retained-caller',
                caller_provider_generation=7,outcome='succeeded',
                completed_at=6,exit_code=0 WHERE lfid=?1", params![agent,direct]).unwrap();
            conn.execute(
                "UPDATE processes SET outcome='interrupted',completed_at=7,exit_code=130
                WHERE lfid=?1",
                [&interrupted],
            )
            .unwrap();
        }
        let page = store
            .processes(
                &ProcessFilter::default(),
                None,
                NonZeroU32::new(10).unwrap(),
            )
            .unwrap();
        assert_eq!(
            page.entries.iter().map(|e| &e.lfid).collect::<Vec<_>>(),
            vec![&interrupted, &agent, &direct, &parent]
        );
        assert_eq!(page.next, None);
        for entry in &page.entries {
            assert_eq!(store.process(&entry.lfid).unwrap().as_ref(), Some(entry));
        }
        let old = &page.entries[3];
        assert_eq!(
            (old.via_agent, old.completed_at, old.exit_code),
            (None, None, None)
        );
        assert_eq!(old.outcome, None);
        assert_eq!(old.signal, None);
        let failed = &page.entries[2];
        assert_eq!(failed.parent_process_lfid.as_ref(), Some(&parent));
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
            (ProcessOutcomeFilter::Unknown, &parent),
            (ProcessOutcomeFilter::Failed, &direct),
            (ProcessOutcomeFilter::Succeeded, &agent),
            (ProcessOutcomeFilter::Interrupted, &interrupted),
        ] {
            let filter = ProcessFilter {
                outcome: Some(outcome),
                ..Default::default()
            };
            let selected = store
                .processes(&filter, None, NonZeroU32::new(1).unwrap())
                .unwrap();
            assert_eq!(&selected.entries[0].lfid, expected);
            assert_eq!(selected.next, None);
        }
        assert!(!dir.path().join("runs").exists());
    }

    #[test]
    fn process_discovery_separates_literal_search_from_identity_resolution() {
        let dir = tempfile::tempdir().unwrap();
        let store = SqliteStore::open_ephemeral(&dir.path().join("store.db")).unwrap();
        let first = insert_process(&store, 1, 1);
        let second = insert_process(&store, 16, 2);
        store
            .conn
            .lock()
            .unwrap()
            .execute(
                "UPDATE processes SET command=?3,parent_process_lfid=?2,
                caller_session_id='caller' WHERE lfid=?1",
                params![
                    first,
                    second,
                    serde_json::to_string(&["lf", "pr", "land", "--strict", "%_ bytes"]).unwrap()
                ],
            )
            .unwrap();
        let filter = ProcessFilter {
            repo: Some("/repo".into()),
            parent_process_lfid: Some(second.clone()),
            caller_session_id: Some("caller".into()),
            command_contains: Some("PR LAND --strict %_".into()),
            identity_contains: Some("000000000001".into()),
            ..Default::default()
        };
        assert_eq!(
            store
                .processes(&filter, None, NonZeroU32::new(1).unwrap())
                .unwrap()
                .entries[0]
                .lfid,
            first
        );
        assert_eq!(
            store.resolve_process(first.as_str()).unwrap().unwrap().lfid,
            first
        );
        assert_eq!(
            store
                .resolve_process(&second.as_str()[..35])
                .unwrap()
                .unwrap()
                .lfid,
            second
        );
        assert!(matches!(
            store.resolve_process("00000000"),
            Err(StoreError::InvalidData(_))
        ));
        assert_eq!(store.resolve_process("%_").unwrap(), None);
        assert_eq!(store.process(&ProcessLfid::new()).unwrap(), None);
        let miss = ProcessFilter {
            command_contains: Some("%_".into()),
            lfid: Some(second),
            ..Default::default()
        };
        assert!(store
            .processes(&miss, None, NonZeroU32::new(1).unwrap())
            .unwrap()
            .entries
            .is_empty());
        for command in [Some("[old pr land"), Some("{\"note\":\"pr land\"}"), None] {
            store
                .conn
                .lock()
                .unwrap()
                .execute(
                    "UPDATE processes SET command=?2 WHERE lfid=?1",
                    params![first, command],
                )
                .unwrap();
            let page = store
                .processes(
                    &ProcessFilter {
                        lfid: Some(first.clone()),
                        command_contains: Some("pr land".into()),
                        ..Default::default()
                    },
                    None,
                    NonZeroU32::new(1).unwrap(),
                )
                .unwrap();
            assert_eq!(page.entries.len(), usize::from(command.is_some()));
            assert_eq!(
                store.process(&first).unwrap().unwrap().command.as_deref(),
                command
            );
        }
    }

    #[test]
    fn process_discovery_pages_fixed_data_after_filtering_with_timestamp_ties() {
        let dir = tempfile::tempdir().unwrap();
        let store = SqliteStore::open_ephemeral(&dir.path().join("store.db")).unwrap();
        let mut expected = Vec::new();
        for number in 1..=12 {
            let id = insert_process(&store, number, i64::from(number / 5));
            if number % 2 == 0 {
                expected.push((i64::from(number / 5), id));
            } else {
                store
                    .conn
                    .lock()
                    .unwrap()
                    .execute("UPDATE processes SET repo='/other' WHERE lfid=?1", [id])
                    .unwrap();
            }
        }
        expected.sort_by(|a, b| b.0.cmp(&a.0).then_with(|| a.1.as_str().cmp(b.1.as_str())));
        let filter = ProcessFilter {
            repo: Some("/repo".into()),
            ..Default::default()
        };
        let mut after = None;
        let mut found = Vec::new();
        loop {
            let page = store
                .processes(&filter, after.as_ref(), NonZeroU32::new(2).unwrap())
                .unwrap();
            assert!(page.entries.len() <= 2);
            found.extend(page.entries.into_iter().map(|entry| entry.lfid));
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
    fn process_discovery_deduplicates_performed_work_without_rebinding_history() {
        let dir = tempfile::tempdir().unwrap();
        let store = SqliteStore::open_ephemeral(&dir.path().join("store.db")).unwrap();
        let wave = WaveId::new();
        let project = ProjectId::new();
        let first = TaskId::new();
        let second = TaskId::new();
        let shared = insert_process(&store, 1, 3);
        let mechanical = insert_process(&store, 2, 2);
        let unbound = insert_process(&store, 3, 1);
        let observer = insert_process(&store, 4, 4);
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
                conn.execute("INSERT INTO session_events(session_id,provider_thread,provider_turn,kind,receipt_key,process_lfid,task_id,wave_id,observed_at,payload)
                    VALUES(?1,'thread','work','started','',?2,?3,?4,1,'unreadable payload')",
                    params![format!("session-{index}"),shared,task.as_str(),wave]).unwrap();
            }
            // Same Process drove both Tasks, two turns and a mechanical boundary.
            conn.execute("INSERT INTO session_events(session_id,provider_thread,provider_turn,kind,receipt_key,process_lfid,task_id,wave_id,observed_at,payload)
                VALUES('session-0','thread','retry','started','',?1,?2,?3,2,'{}')",params![shared,first.as_str(),wave]).unwrap();
            // Session is bound now; the earlier turn remains unassigned.
            conn.execute("INSERT INTO session_events(session_id,provider_thread,provider_turn,kind,receipt_key,process_lfid,observed_at,payload)
                VALUES('session-0','thread','before-bind','started','',?1,1,'{}')",[&unbound]).unwrap();
            // Known Task, unknown original process: neither current driver nor observer is its owner.
            conn.execute("INSERT INTO session_events(session_id,provider_thread,provider_turn,kind,receipt_key,task_id,wave_id,observed_at,payload)
                VALUES('session-0','thread','unmapped','started','',?1,?2,1,'{}')",params![first.as_str(),wave]).unwrap();

            conn.execute(
                "UPDATE processes SET caller_session_id='session-0',via_agent=1 WHERE lfid=?1",
                [&observer],
            )
            .unwrap();
            // A Flow's driver recorded both Processes as steps in the first Task's checkout.
            conn.execute(
                "UPDATE tasks SET worktree='/repo.first' WHERE id=?1",
                [first.as_str()],
            )
            .unwrap();
            conn.execute(
                "INSERT INTO flow_processes(process_lfid,flow,graph) VALUES(?1,'proof','{}')",
                [&observer],
            )
            .unwrap();
            for process in [&shared, &mechanical] {
                conn.execute(
                    "UPDATE processes SET cwd='/repo.first',parent_process_lfid=?2 WHERE lfid=?1",
                    params![process, observer],
                )
                .unwrap();
                conn.execute(
                    "INSERT INTO flow_process_steps(flow_process_lfid,process_lfid,node,iterations) VALUES(?2,?1,0,'[]')",
                    params![process, observer],
                )
                .unwrap();
            }
        }
        let filter = ProcessFilter {
            performed_work: Some(ProcessWorkFilter::Task(first.clone())),
            ..Default::default()
        };
        let page = store
            .processes(&filter, None, NonZeroU32::new(1).unwrap())
            .unwrap();
        assert_eq!(
            page.entries.iter().map(|e| &e.lfid).collect::<Vec<_>>(),
            vec![&shared]
        );
        let page = store
            .processes(&filter, page.next.as_ref(), NonZeroU32::new(1).unwrap())
            .unwrap();
        assert_eq!(page.entries[0].lfid, mechanical);
        assert_eq!(page.next, None);
        let second_filter = ProcessFilter {
            performed_work: Some(ProcessWorkFilter::Task(second)),
            ..Default::default()
        };
        let page = store
            .processes(&second_filter, None, NonZeroU32::new(10).unwrap())
            .unwrap();
        assert_eq!(
            page.entries.iter().map(|e| &e.lfid).collect::<Vec<_>>(),
            vec![&shared]
        );
        let wave_filter = ProcessFilter {
            performed_work: Some(ProcessWorkFilter::Wave(wave)),
            ..Default::default()
        };
        let page = store
            .processes(&wave_filter, None, NonZeroU32::new(10).unwrap())
            .unwrap();
        assert_eq!(
            page.entries.iter().map(|e| &e.lfid).collect::<Vec<_>>(),
            vec![&shared, &mechanical]
        );
        assert_eq!(store.process(&unbound).unwrap().unwrap().outcome, None);
        assert_eq!(store.process(&observer).unwrap().unwrap().outcome, None);
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

#[cfg(test)]
mod attachment_tests {
    use super::discovery_tests::insert_process;
    use crate::id::ProcessLfid;
    use crate::session::SessionActivity;
    use crate::store::sqlite::SqliteStore;
    use crate::store::StoreError;

    #[test]
    fn attachment_close_keeps_transfer_fenced_without_blocking_store_writes() {
        let home = tempfile::tempdir().unwrap();
        let path = home.path().join("store.db");
        let store = SqliteStore::open_ephemeral(&path).unwrap();
        store.test_session("conversation", &crate::session_record::new_artifact_key());
        store.test_session("unrelated", &crate::session_record::new_artifact_key());
        let parent = insert_process(&store, 1, 1);
        let attachment = store
            .claim_session_attachment("conversation", None, &parent, true)
            .unwrap();
        let history = store.session_history("conversation", 0, 0).unwrap();
        let lock_path = std::fs::read_dir(path.with_extension("session-locks"))
            .unwrap()
            .next()
            .unwrap()
            .unwrap()
            .path();
        let lock = std::fs::File::open(lock_path).unwrap();

        let close = || -> crate::store::StoreResult<()> {
            // Fail immediately on the old nested-lock shape instead of hanging.
            drop(store.conn.try_lock().expect("close must release SQLite"));
            assert_eq!(
                fs2::FileExt::try_lock_exclusive(&lock).unwrap_err().kind(),
                std::io::ErrorKind::WouldBlock
            );
            store.rename_session(
                "unrelated",
                "Saved during close",
                crate::session::TitleSource::Human,
            )?;
            assert_eq!(
                store.session_attachment("conversation")?,
                Some(attachment.clone())
            );
            Ok(())
        };
        assert!(store
            .finish_session_attachment("conversation", &attachment, "completed", || {
                close()?;
                Err(StoreError::InvalidData("provider close failed".into()))
            })
            .is_err());
        assert_eq!(
            store.session_history("conversation", 0, 0).unwrap(),
            history
        );
        assert_eq!(
            store.session_attachment("conversation").unwrap(),
            Some(attachment.clone())
        );
        assert!(store
            .process(&attachment.agent_process_lfid)
            .unwrap()
            .unwrap()
            .completed_at
            .is_none());

        store
            .finish_session_attachment("conversation", &attachment, "completed", || {
                close()?;
                Ok(true)
            })
            .unwrap();
        assert!(store
            .session_attachment("conversation")
            .unwrap()
            .unwrap()
            .process_lfid
            .is_none());
        assert!(store
            .process(&attachment.agent_process_lfid)
            .unwrap()
            .unwrap()
            .completed_at
            .is_some());
        assert_eq!(
            store.session("unrelated").unwrap().unwrap().title,
            "Saved during close"
        );
        fs2::FileExt::try_lock_exclusive(&lock).unwrap();
    }

    #[test]
    fn respawn_requires_positive_exit_and_preserves_uncertain_attempts() {
        let home = tempfile::tempdir().unwrap();
        let store = SqliteStore::open_ephemeral(&home.path().join("store.db")).unwrap();
        store.test_session("conversation", "run_00000000000000000000000000000001");
        let parent = insert_process(&store, 1, 1);
        let first = store
            .claim_session_attachment("conversation", None, &parent, true)
            .unwrap();
        assert_eq!(
            store
                .prepare_session_agent_process("conversation", &first)
                .unwrap(),
            first
        );
        store
            .record_session_provider_launch(
                "conversation",
                &first,
                &std::process::Command::new("fixture"),
            )
            .unwrap();
        assert!(store
            .prepare_session_agent_process("conversation", &first)
            .is_err());
        assert_eq!(
            store.session_attachment("conversation").unwrap(),
            Some(first.clone())
        );
        store
            .record_native_provider_exit("conversation", &first, false)
            .unwrap();
        let failed = store.process(&first.agent_process_lfid).unwrap().unwrap();
        let next = store
            .prepare_session_agent_process("conversation", &first)
            .unwrap();
        assert_ne!(next.agent_process_lfid, first.agent_process_lfid);
        assert_eq!(next.process_lfid, first.process_lfid);
        assert_eq!(
            store.process(&first.agent_process_lfid).unwrap(),
            Some(failed)
        );
        assert!(store
            .prepare_session_agent_process("conversation", &first)
            .is_err());
        assert!(store
            .with_session_attachment("conversation", &first, || Ok(()))
            .is_err());
        assert_eq!(
            store.session_attachment("conversation").unwrap(),
            Some(next)
        );
    }

    #[test]
    fn returning_to_the_same_lf_process_does_not_revive_its_previous_attachment() {
        let home = tempfile::tempdir().unwrap();
        let store = SqliteStore::open_ephemeral(&home.path().join("store.db")).unwrap();
        store.test_session("conversation", "run_00000000000000000000000000000001");
        let a = ProcessLfid::new();
        let b = ProcessLfid::new();
        for id in [&a, &b] {
            store
                .conn
                .lock()
                .unwrap()
                .execute(
                    "INSERT INTO processes(lfid,trace_id,started_at) VALUES(?1,'fixture',1)",
                    [id],
                )
                .unwrap();
        }
        let first = store
            .claim_session_attachment("conversation", None, &a, true)
            .unwrap();
        store
            .record_session_connection("conversation", &first, "endpoint", "native-history")
            .unwrap();
        let second = store
            .claim_session_attachment("conversation", Some(&first), &b, false)
            .unwrap();
        let third = store
            .claim_session_attachment("conversation", Some(&second), &a, false)
            .unwrap();
        assert_eq!(first.process_lfid, third.process_lfid);
        assert_eq!(first.provider_process_lfid, third.provider_process_lfid);
        assert_eq!(first.provider_generation, third.provider_generation);
        assert_ne!(first.token, third.token);
        assert_eq!(
            store.session_connection("conversation").unwrap(),
            Some(("endpoint".into(), "native-history".into()))
        );

        let current = SessionActivity {
            observed_at: 100,
            open_tools: 1,
            pending_input: 0,
            yielded: false,
        };
        store
            .record_session_activity("conversation", &third, &current)
            .unwrap();
        for stale in [&first, &second] {
            // Queued sends and approval replies share this exact dispatch boundary.
            assert!(matches!(
                store.with_session_attachment::<()>("conversation", stale, || panic!(
                    "stale native write"
                )),
                Err(StoreError::InvalidAuthority(_))
            ));
            assert!(matches!(
                store.finish_session_attachment("conversation", stale, "interrupted", || panic!(
                    "stale stop"
                )),
                Err(StoreError::InvalidAuthority(_))
            ));
            assert!(matches!(
                store.release_session_attachment("conversation", stale),
                Err(StoreError::InvalidAuthority(_))
            ));
            store
                .record_session_activity(
                    "conversation",
                    stale,
                    &SessionActivity {
                        observed_at: 200,
                        pending_input: 1,
                        ..current.clone()
                    },
                )
                .unwrap();
        }
        let reading: (i64, i64) = store.conn.lock().unwrap().query_row(
            "SELECT observed_at,pending_input FROM session_activity WHERE session_id='conversation'", [], |row| Ok((row.get(0)?,row.get(1)?)),
        ).unwrap();
        assert_eq!(reading, (100, 0));
        store
            .with_session_attachment("conversation", &third, || Ok(()))
            .unwrap();
        let released = store
            .release_session_attachment("conversation", &third)
            .unwrap();
        let fourth = store
            .claim_session_attachment("conversation", Some(&released), &a, false)
            .unwrap();
        assert_ne!(third.token, fourth.token);
        assert!(matches!(
            store.with_session_attachment::<()>("conversation", &third, || panic!(
                "released native write"
            )),
            Err(StoreError::InvalidAuthority(_))
        ));
    }

    #[test]
    fn attachment_migration_preserves_released_history_and_waiting_evidence() {
        use crate::store::migrations::{apply_before_current_draft, current_draft_sql};
        let conn = rusqlite::Connection::open_in_memory().unwrap();
        apply_before_current_draft(&conn, "agent_process");
        conn.execute_batch(include_str!(
            "../../../tests/fixtures/agent_process_released.sql"
        ))
        .unwrap();
        let history = || {
            conn.prepare(
                "SELECT seq,session_id,receipt_key,payload FROM session_events ORDER BY seq",
            )
            .unwrap()
            .query_map([], |row| {
                Ok((
                    row.get::<_, i64>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, String>(2)?,
                    row.get::<_, String>(3)?,
                ))
            })
            .unwrap()
            .collect::<rusqlite::Result<Vec<_>>>()
            .unwrap()
        };
        let before = history();
        conn.execute_batch(&current_draft_sql("agent_process"))
            .unwrap();
        assert_eq!(history(), before);
        let live: (String, String, String, u32, i64, i64) = conn.query_row(
            "SELECT p.attached_process_lfid,p.parent_process_lfid,s.provider_thread,p.pid,p.os_started_at,s.current_capture FROM agent_sessions s JOIN processes p ON p.lfid=s.agent_process_lfid WHERE s.id='live'",
            [], |row| Ok((row.get(0)?,row.get(1)?,row.get(2)?,row.get(3)?,row.get(4)?,row.get(5)?)),
        ).unwrap();
        assert_eq!(
            live,
            (
                "parent".into(),
                "parent".into(),
                "native".into(),
                42,
                100,
                before[1].0
            )
        );
        let matched: i64 = conn.query_row("SELECT COUNT(*) FROM session_activity a JOIN agent_sessions s ON s.id=a.session_id JOIN processes p ON p.lfid=s.agent_process_lfid WHERE a.attachment_token=p.attachment_token", [], |row| row.get(0)).unwrap();
        assert_eq!(matched, 1);
        let exit: i64 = conn
            .query_row(
                "SELECT p.attachment_exit_seq FROM agent_sessions s JOIN processes p ON p.lfid=s.agent_process_lfid WHERE s.id='released'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(exit, before[0].0);
        let violations: i64 = conn
            .query_row("SELECT COUNT(*) FROM pragma_foreign_key_check", [], |row| {
                row.get(0)
            })
            .unwrap();
        assert_eq!(violations, 0);
        for table in ["agent_sessions", "session_activity"] {
            let columns = conn
                .prepare(&format!("PRAGMA table_info({table})"))
                .unwrap()
                .query_map([], |row| row.get::<_, String>(1))
                .unwrap()
                .collect::<rusqlite::Result<Vec<_>>>()
                .unwrap();
            assert!(!columns.iter().any(|column| column.starts_with("driver_")));
        }
    }
    #[test]
    fn detached_and_replaced_agents_remain_in_the_process_inventory() {
        let home = tempfile::tempdir().unwrap();
        let store = SqliteStore::open_ephemeral(&home.path().join("db")).unwrap();
        let session =
            store.test_session("conversation", &crate::session_record::new_artifact_key());
        let a = insert_process(&store, 1, 1);
        let b = insert_process(&store, 2, 2);
        let first = store
            .claim_session_attachment(&session.id, None, &a, true)
            .unwrap();
        store
            .record_session_provider_process(&session.id, &first, 4242, 123)
            .unwrap();
        let second = store
            .claim_session_attachment(&session.id, Some(&first), &b, false)
            .unwrap();
        assert_eq!(first.agent_process_lfid, second.agent_process_lfid);
        assert_ne!(first.token, second.token);
        let released = store
            .release_session_attachment(&session.id, &second)
            .unwrap();
        let rows = store.agent_processes().unwrap();
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].process.parent_process_lfid, Some(a.clone()));
        assert_eq!(rows[0].attached_process_lfid, None);
        assert_eq!(rows[0].process.pid, Some(4242));
        assert!(matches!(
            store.record_session_provider_process(&session.id, &first, 5555, 123),
            Err(StoreError::InvalidAuthority(_))
        ));
        let replacement = store
            .claim_session_attachment(&session.id, Some(&released), &b, true)
            .unwrap();
        assert_ne!(replacement.agent_process_lfid, first.agent_process_lfid);
        assert_eq!(store.agent_processes().unwrap().len(), 2);
        let old = store.process(&first.agent_process_lfid).unwrap().unwrap();
        assert_eq!(old.pid, Some(4242));
        assert_eq!(old.parent_process_lfid, Some(a));
        assert!(old.completed_at.is_none());
    }

    #[test]
    fn agent_spawn_lifecycle_uses_the_record_and_retains_failed_identity() {
        let home = tempfile::tempdir().unwrap();
        let store = SqliteStore::open_ephemeral(&home.path().join("db")).unwrap();
        let session =
            store.test_session("conversation", &crate::session_record::new_artifact_key());
        let parent = insert_process(&store, 1, 1);
        let attachment = store
            .claim_session_attachment(&session.id, None, &parent, true)
            .unwrap();
        let history = store.session_history(&session.id, 0, 0).unwrap();
        let mut command = std::process::Command::new("codex");
        command.args(["app-server", "--listen", "unix:///fixture"]);
        store
            .record_session_provider_launch(&session.id, &attachment, &command)
            .unwrap();
        store
            .record_session_provider_process(&session.id, &attachment, 4242, 123)
            .unwrap();
        store
            .record_native_provider_exit(&session.id, &attachment, false)
            .unwrap();
        let row = store
            .process(&attachment.agent_process_lfid)
            .unwrap()
            .unwrap();
        assert_eq!(row.kind, crate::process::ProcessKind::Agent);
        assert_eq!(row.pid, Some(4242));
        assert_eq!(row.os_started_at, Some(123));
        assert!(store
            .record_session_provider_process(&session.id, &attachment, 5555, 124)
            .is_err());
        assert_eq!(
            store
                .process(&attachment.agent_process_lfid)
                .unwrap()
                .unwrap()
                .pid,
            Some(4242)
        );
        assert_eq!(
            row.command.as_deref(),
            Some(r#"["codex","app-server","--listen","unix:///fixture"]"#)
        );
        assert!(row.completed_at.is_some());
        assert!(
            row.outcome.is_none(),
            "failed exec is not an observed provider exit"
        );
        assert_eq!(store.session_history(&session.id, 0, 0).unwrap(), history);
        assert!(store.agent_processes().unwrap().is_empty());
    }

    #[test]
    fn stale_orphan_observation_cannot_settle_a_transferred_agent() {
        let home = tempfile::tempdir().unwrap();
        let store = SqliteStore::open_ephemeral(&home.path().join("db")).unwrap();
        let session =
            store.test_session("conversation", &crate::session_record::new_artifact_key());
        let a = insert_process(&store, 1, 1);
        let b = insert_process(&store, 2, 2);
        let first = store
            .claim_session_attachment(&session.id, None, &a, true)
            .unwrap();
        store
            .record_session_provider_process(&session.id, &first, 4242, 123)
            .unwrap();
        let observed = store.agent_processes().unwrap().remove(0);
        store
            .claim_session_attachment(&session.id, Some(&first), &b, false)
            .unwrap();
        assert!(!store
            .settle_agent_process(&observed, || panic!("stale signal"))
            .unwrap());
        let current = store.agent_processes().unwrap().remove(0);
        assert!(store
            .settle_agent_process(&current, || {
                // Settlement holds the attachment fence, not the SQLite mutex.
                assert_eq!(
                    store.process(&current.process.lfid)?.as_ref(),
                    Some(&current.process)
                );
                Ok(())
            })
            .unwrap());
        let retained = store.process(&first.agent_process_lfid).unwrap().unwrap();
        assert_eq!(retained.pid, Some(4242));
        assert_eq!(retained.parent_process_lfid, Some(a));
        assert!(retained.completed_at.is_some());
    }
}
