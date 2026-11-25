use std::fs::File;
use std::path::Path;
use std::time::{Duration, Instant};

use fs2::FileExt;
use rusqlite::{params, OptionalExtension, TransactionBehavior};

use crate::process::{AgentProcess, AttachingLf, LfProcess, ProcessEntry, SessionAttachment};
use crate::{StoreError, StoreResult};
use loopflow_model::id::{AgentProcessId, AgentSessionId, AttachmentToken, LfProcessId};

use super::{SessionCaller, SqliteStore};

/// Saved endpoint and native identity; liveness is observed by the caller.
#[derive(Debug)]
pub struct SavedSessionConnection {
    pub endpoint: String,
    pub thread: AgentSessionId,
    pub completed_at: Option<i64>,
    pub pid: Option<u32>,
    pub os_started_at: Option<i64>,
}

/// What acquisition established about the previous AgentProcess.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PreviousAgentDisposition {
    /// Already ended, or reserved without ever requesting a spawn.
    Inactive,
    /// Positive exit evidence from closing the provider.
    Closed,
}

/// An exclusive attachment fence retained through acquisition and atomic claim.
#[derive(Debug)]
pub struct SessionTransfer<'a> {
    store: &'a SqliteStore,
    session: String,
    expected: Option<SessionAttachment>,
    _dispatch: File,
}

impl SessionTransfer<'_> {
    pub fn attachment(&self) -> Option<&SessionAttachment> {
        self.expected.as_ref()
    }

    pub fn previous_progress(&self) -> StoreResult<(bool, bool)> {
        match &self.expected {
            Some(previous) => {
                let conn = self.store.conn.lock().expect("store mutex poisoned");
                agent_process_progress_in(&conn, &previous.agent_process_id)
            }
            None => Ok((true, false)),
        }
    }

    pub fn claim_attachment(
        self,
        process: &LfProcessId,
        previous: PreviousAgentDisposition,
        start: Option<&AttachingLf>,
        caller: &SessionCaller,
    ) -> StoreResult<SessionAttachment> {
        self.commit(previous, |tx| {
            super::sessions::require_current_actor_in(tx, &self.session, caller)?;
            attach_in(tx, &self.session, self.expected.as_ref(), process, start)
        })
    }

    pub fn claim_input(
        self,
        next: crate::session::LfSession,
        process: &LfProcessId,
        previous: PreviousAgentDisposition,
        start: Option<&AttachingLf>,
        caller: &SessionCaller,
    ) -> StoreResult<(crate::session::LfSession, SessionAttachment)> {
        if loopflow_model::id::canonical_session(&next.id) != self.session {
            return Err(attachment_changed());
        }
        self.commit(previous, |tx| {
            super::sessions::require_current_actor_in(tx, &self.session, caller)?;
            super::sessions::claim_session_input_in(
                tx,
                next,
                self.expected.as_ref(),
                process,
                start,
            )
        })
    }

    pub fn require_owner(&self) -> StoreResult<()> {
        if self
            .expected
            .as_ref()
            .is_some_and(|expected| expected.lf_process_id.is_some())
        {
            Ok(())
        } else {
            Err(attachment_changed())
        }
    }

    pub fn replace_agent(
        self,
        resume_thread: Option<&AgentSessionId>,
        reserved: bool,
        start: Option<&AttachingLf>,
        caller: &SessionCaller,
    ) -> StoreResult<SessionAttachment> {
        let session = self.session.as_str();
        let expected = self.expected.as_ref().ok_or_else(attachment_changed)?;
        let mut conn = self.store.conn.lock().expect("store mutex poisoned");
        let tx = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
        super::sessions::require_current_actor_in(&tx, session, caller)?;
        require_attachment_in(&tx, session, expected)?;
        let now = time::OffsetDateTime::now_utc().unix_timestamp();
        end_agent_in(&tx, session, &expected.agent_process_id, now, !reserved)?;
        let next = attach_in(
            &tx,
            session,
            Some(expected),
            expected
                .lf_process_id
                .as_ref()
                .expect("attachment is owned"),
            start,
        )?;
        // The former thread goes to the log. Never clear a saved thread merely
        // because a new harness omitted it.
        let previous_thread: Option<String> = tx.query_row(
            "SELECT provider_thread FROM agent_sessions WHERE id=?1",
            [session],
            |row| row.get(0),
        )?;
        tx.execute(
            "UPDATE agent_sessions SET provider_thread=?2 WHERE id=?1",
            params![session, resume_thread],
        )?;
        tx.commit()?;
        tracing::info!(
            session,
            agent_process = ?expected.agent_process_id,
            next_agent_process = ?next.agent_process_id,
            previous_thread = previous_thread.as_deref(),
            next_thread = resume_thread.map(AgentSessionId::as_str),
            "agent process replaced"
        );
        Ok(next)
    }

    pub fn finish(
        self,
        outcome: &str,
        departure: Option<&crate::process::Departure>,
        closed: bool,
    ) -> StoreResult<()> {
        let session = self.session.as_str();
        let expected = self.expected.as_ref().ok_or_else(attachment_changed)?;
        let mut conn = self.store.conn.lock().expect("store mutex poisoned");
        let tx = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
        if attachment_in(&tx, session)?.as_ref() != Some(expected) {
            return Err(attachment_changed());
        }
        let now = time::OffsetDateTime::now_utc().unix_timestamp();
        // Only a departure has a reader: the next attachment asks where the
        // conversation went, from the Session. History keeps a copy for
        // people; an exit without one leaves none.
        let exit = departure
            .map(|departure| -> StoreResult<crate::history::Row> {
                let departure = serde_json::to_value(departure)
                    .map_err(|error| StoreError::InvalidData(error.to_string()))?;
                tx.execute(
                    "UPDATE agent_sessions SET departed_attachment=?2,departure=?3 WHERE id=?1",
                    params![session, expected.token.to_string(), departure.to_string()],
                )?;
                let payload = serde_json::json!({
                    "type": "attachment_exit", "outcome": outcome, "attachment_token": expected.token,
                    "departure": departure,
                });
                let captured: Option<i64> = tx.query_row(
                    "SELECT current_capture FROM agent_sessions WHERE id=?1",
                    [session],
                    |row| row.get(0),
                )?;
                let mut row = crate::history::Row::observed(
                    session,
                    format!("attachment:{}:exit", expected.token),
                    captured,
                    now,
                    payload.to_string(),
                );
                row.lf_process_id = expected.lf_process_id.as_ref().map(ToString::to_string);
                Ok(row)
            })
            .transpose()?;
        // A departure continues the Session elsewhere; only an exit without a
        // destination completes it. A headless run is history once it stops
        // working, whatever Work or Graph it served; attaching a later turn
        // reopens it. Interactive primary, Wave and Task conversations, and
        // Graph reviews, stay open.
        tx.execute(
            &format!(
                "UPDATE agent_sessions AS s SET completed_at=?2 WHERE s.id=?1
             AND s.completed_at IS NULL AND ?4
             AND s.attached_lf_process_id IS NOT NULL
             AND CASE WHEN s.interactive=0 THEN ?3 IN ('completed','interrupted','failed')
             ELSE ?3 IN ('completed','interrupted') AND s.primary_scope IS NULL
             AND s.wave_id IS NULL AND {} IS NULL AND s.task_id IS NULL END",
                super::sessions::SESSION_GRAPH
            ),
            params![session, now, outcome, departure.is_none()],
        )?;
        release_attachment_in(&tx, session, &AttachmentToken::new())?;
        if closed {
            end_agent_in(&tx, session, &expected.agent_process_id, now, true)?;
        }
        tx.commit()?;
        drop(conn);
        self.store
            .append_history("attachment exit", &Vec::from_iter(exit));
        Ok(())
    }

    fn commit<T>(
        &self,
        previous: PreviousAgentDisposition,
        claim: impl FnOnce(&rusqlite::Transaction<'_>) -> StoreResult<T>,
    ) -> StoreResult<T> {
        let mut conn = self.store.conn.lock().expect("store mutex poisoned");
        let tx = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
        if attachment_in(&tx, &self.session)?.as_ref() != self.expected.as_ref() {
            return Err(attachment_changed());
        }
        if let Some(attachment) = &self.expected {
            // A never-launched reservation is retired without inventing OS exit.
            end_agent_in(
                &tx,
                &self.session,
                &attachment.agent_process_id,
                time::OffsetDateTime::now_utc().unix_timestamp(),
                previous == PreviousAgentDisposition::Closed,
            )?;
        }
        let result = claim(&tx)?;
        tx.commit()?;
        Ok(result)
    }
}

/// Exact saved AgentProcess protected against attachment replacement.
#[derive(Debug)]
pub struct AgentProcessSettlement<'a> {
    store: &'a SqliteStore,
    expected: crate::process::AgentProcessState,
    _dispatch: File,
}

impl AgentProcessSettlement<'_> {
    pub fn commit(self) -> StoreResult<()> {
        let mut conn = self.store.conn.lock().expect("store mutex poisoned");
        let tx = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
        let at = time::OffsetDateTime::now_utc().unix_timestamp();
        tx.execute(
            "UPDATE agent_sessions SET agent_ended_at=?2,agent_pid=NULL,agent_os_started_at=NULL,
                agent_spawn_state=CASE WHEN agent_spawn_state='reserved' AND agent_pid IS NULL
                    THEN 'spawn_failed' ELSE 'exited' END,
                agent_endpoint=NULL WHERE agent_process_id=?1",
            params![self.expected.id, at],
        )?;
        complete_dead_headless_run_in(&tx, self.expected.id.as_str(), at)?;
        tx.commit()?;
        Ok(())
    }
}

/// An lf an owner record says runs.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RecordedLf {
    /// What it runs as: `Graph run`, `Session` and the like.
    pub what: String,
    pub id: String,
    pub pid: u32,
    pub os_started_at: Option<i64>,
    pub started_at: Option<i64>,
}

pub(super) const PROCESS_SELECT: &str =
    "SELECT e.id,e.trace_id,e.parent_lf_process_id,e.via_agent,e.caller_session_id,
    e.caller_agent_process_id,e.command,e.repo,e.cwd,
    e.started_at,e.completed_at,e.outcome,e.exit_code,e.signal,e.error,e.pid,e.kind,e.agent_session_id,e.os_started_at FROM processes e";

/// Every Session's agent that has not ended, as `read_agent_state` reads it.
const SESSION_AGENTS: &str = "SELECT s.agent_process_id,s.id,s.agent_started_at,s.agent_ended_at,
    s.agent_pid,s.agent_os_started_at,
    s.agent_launcher_lf_process_id,s.attached_lf_process_id,s.attachment_token,
    s.attached_pid,s.attached_os_started_at,s.provider,COALESCE(s.agent_interactive,s.interactive),
    (s.agent_spawn_state IS 'reserved' AND s.agent_pid IS NULL)
    FROM agent_sessions s WHERE s.agent_process_id IS NOT NULL AND s.agent_ended_at IS NULL
    ORDER BY s.agent_process_id";

fn read_agent_state(
    row: &rusqlite::Row<'_>,
) -> rusqlite::Result<crate::process::AgentProcessState> {
    Ok(crate::process::AgentProcessState {
        id: row.get(0)?,
        session_id: row.get(1)?,
        started_at: row.get(2)?,
        ended_at: row.get(3)?,
        pid: row.get(4)?,
        os_started_at: row.get(5)?,
        launcher_lf_process_id: row.get(6)?,
        attached_lf_process_id: row.get(7)?,
        attachment_token: row.get(8)?,
        holder: row
            .get::<_, Option<String>>(7)?
            .map(|_| -> rusqlite::Result<_> {
                Ok(crate::process::AttachmentHolder {
                    pid: row.get(9)?,
                    os_started_at: row.get(10)?,
                })
            })
            .transpose()?,
        provider: row.get(11)?,
        interactive: row.get(12)?,
        spawn_reserved: row.get(13)?,
    })
}

/// The row's `kind` column is the only place an id becomes either type.
pub(super) fn read_process(row: &rusqlite::Row<'_>) -> rusqlite::Result<ProcessEntry> {
    Ok(match row.get::<_, String>(16)?.as_str() {
        "agent" => ProcessEntry::Agent(AgentProcess {
            id: row.get(0)?,
            pid: row.get(15)?,
            os_started_at: row.get(18)?,
            agent_session_id: row.get(17)?,
            trace_id: row.get(1)?,
            parent_lf_process_id: row.get(2)?,
            command: row.get(6)?,
            repo: row.get(7)?,
            cwd: row.get(8)?,
            started_at: row.get(9)?,
            completed_at: row.get(10)?,
            exit_code: row.get(12)?,
            signal: row.get(13)?,
            error: row.get(14)?,
        }),
        _ => ProcessEntry::Lf(LfProcess {
            id: row.get(0)?,
            pid: row.get(15)?,
            os_started_at: row.get(18)?,
            trace_id: row.get(1)?,
            parent_lf_process_id: row.get(2)?,
            via_agent: row.get(3)?,
            caller_session_id: row.get(4)?,
            caller_agent_process_id: row.get(5)?,
            command: row.get(6)?,
            repo: row.get(7)?,
            cwd: row.get(8)?,
            started_at: row.get(9)?,
            completed_at: row.get(10)?,
            outcome: row.get(11)?,
            exit_code: row.get(12)?,
            signal: row.get(13)?,
            error: row.get(14)?,
        }),
    })
}

/// A row from a query that selects only one kind; the other kind is invalid data.
pub(super) fn read_lf_process(row: &rusqlite::Row<'_>) -> rusqlite::Result<LfProcess> {
    match read_process(row)? {
        ProcessEntry::Lf(process) => Ok(process),
        ProcessEntry::Agent(process) => Err(wrong_kind(process.id.as_str(), "LfProcess")),
    }
}

#[cfg(any(test, feature = "test-support"))]
fn read_agent_process(row: &rusqlite::Row<'_>) -> rusqlite::Result<AgentProcess> {
    match read_process(row)? {
        ProcessEntry::Agent(process) => Ok(process),
        ProcessEntry::Lf(process) => Err(wrong_kind(process.id.as_str(), "AgentProcess")),
    }
}

fn wrong_kind(id: &str, selected: &str) -> rusqlite::Error {
    rusqlite::Error::FromSqlConversionFailure(
        16,
        rusqlite::types::Type::Text,
        format!("process {id} is not the {selected} the query selected").into(),
    )
}

/// The unfinished inventory shared by Task membership and activity.
pub(super) fn unfinished_processes(conn: &rusqlite::Connection) -> StoreResult<Vec<ProcessEntry>> {
    Ok(conn
        .prepare(&format!(
            "{PROCESS_SELECT} INDEXED BY processes_unfinished WHERE e.completed_at IS NULL
             ORDER BY e.started_at DESC,e.id"
        ))?
        .query_map([], read_process)?
        .collect::<rusqlite::Result<Vec<_>>>()?)
}

fn attachment_in(
    conn: &rusqlite::Connection,
    session: &str,
) -> StoreResult<Option<SessionAttachment>> {
    let canonical = loopflow_model::id::canonical_session(session);
    let session = canonical.as_str();
    let row = conn
        .query_row(
            "SELECT attached_lf_process_id,attachment_token,agent_launcher_lf_process_id,
             agent_process_id FROM agent_sessions WHERE id=?1",
            [session],
            |row| {
                Ok((
                    row.get::<_, Option<String>>(0)?,
                    row.get::<_, Option<AttachmentToken>>(1)?,
                    row.get::<_, Option<String>>(2)?,
                    row.get::<_, Option<AgentProcessId>>(3)?,
                ))
            },
        )
        .optional()?
        .ok_or(StoreError::NotFound)?;
    let Some(token) = row.1 else {
        return Ok(None);
    };
    let parse =
        |value: &str| LfProcessId::parse(value).map_err(|e| StoreError::InvalidData(e.to_string()));
    Ok(Some(SessionAttachment {
        lf_process_id: row.0.as_deref().map(parse).transpose()?,
        token,
        agent_process_id: row.3.ok_or(StoreError::NotFound)?,
        launcher_lf_process_id: parse(row.2.as_deref().ok_or_else(|| {
            StoreError::InvalidData("AgentProcess has no parent LfProcess".into())
        })?)?,
    }))
}

fn attachment_changed() -> StoreError {
    StoreError::InvalidAuthority("Session attachment changed".into())
}

fn no_observed_exit() -> StoreError {
    StoreError::InvalidAuthority("Previous AgentProcess has no observed exit".into())
}

/// Whether the record has ended, and whether it is a reservation that never
/// requested a spawn. Anything else may still be running.
fn agent_process_progress_in(
    conn: &rusqlite::Connection,
    agent: &AgentProcessId,
) -> StoreResult<(bool, bool)> {
    Ok(conn.query_row(
        "SELECT agent_ended_at IS NOT NULL,agent_spawn_state IS 'reserved' AND agent_pid IS NULL
         FROM agent_sessions WHERE agent_process_id=?1",
        [agent],
        |row| Ok((row.get(0)?, row.get(1)?)),
    )?)
}

/// Require the current attached LfProcess, not merely a retained agent row.
fn require_attachment_in(
    conn: &rusqlite::Connection,
    session: &str,
    expected: &SessionAttachment,
) -> StoreResult<()> {
    if attachment_in(conn, session)?.as_ref() != Some(expected) || expected.lf_process_id.is_none()
    {
        return Err(attachment_changed());
    }
    Ok(())
}

/// A headless run whose AgentProcess died without a recorded exit, or whose
/// exit an earlier lf recorded and kept open, is history.
pub(super) fn complete_dead_headless_run_in(
    conn: &rusqlite::Connection,
    agent_process: &str,
    at: i64,
) -> StoreResult<()> {
    conn.execute(
        "UPDATE agent_sessions SET completed_at=?2 WHERE agent_process_id=?1
         AND interactive=0 AND completed_at IS NULL AND moved_to_machine_id IS NULL",
        params![agent_process, at],
    )?;
    Ok(())
}

/// End the Session's agent `agent` unless it already ended, clearing its OS
/// process and endpoint. `exited` records an observed exit; a reservation that never
/// spawned keeps its spawn state.
fn end_agent_in(
    conn: &rusqlite::Connection,
    session: &str,
    agent: &AgentProcessId,
    at: i64,
    exited: bool,
) -> StoreResult<()> {
    conn.execute(
        "UPDATE agent_sessions SET agent_ended_at=?3,agent_pid=NULL,agent_os_started_at=NULL,
            agent_endpoint=NULL,agent_spawn_state=CASE WHEN ?4 THEN 'exited' ELSE agent_spawn_state END
         WHERE id=?1 AND agent_process_id=?2 AND agent_ended_at IS NULL",
        params![
            loopflow_model::id::canonical_session(session),
            agent,
            at,
            exited
        ],
    )?;
    Ok(())
}

/// Release the Session's attachment: no lf holds it, and `token` fences out
/// the writers of the one that did.
fn release_attachment_in(
    conn: &rusqlite::Connection,
    session: &str,
    token: &AttachmentToken,
) -> StoreResult<()> {
    conn.execute(
        "UPDATE agent_sessions SET attached_lf_process_id=NULL,attachment_token=?2,
            attached_pid=NULL,attached_os_started_at=NULL,attached_terminal_id=NULL WHERE id=?1",
        params![loopflow_model::id::canonical_session(session), token],
    )?;
    Ok(())
}

pub(super) fn attach_in(
    tx: &rusqlite::Transaction<'_>,
    session: &str,
    expected: Option<&SessionAttachment>,
    process: &LfProcessId,
    start: Option<&AttachingLf>,
) -> StoreResult<SessionAttachment> {
    let current = attachment_in(tx, session)?;
    if current.as_ref() != expected {
        return Err(attachment_changed());
    }
    super::sessions::repair_session_title_in(tx, session, None)?;
    let attached = SessionAttachment {
        agent_process_id: AgentProcessId::new(),
        lf_process_id: Some(process.clone()),
        token: AttachmentToken::new(),
        launcher_lf_process_id: process.clone(),
    };
    // The holder's OS process and terminal, when the attaching lf is this one.
    let holder = start.filter(|start| &start.lf_process_id == process);
    let holder_pid = holder.and_then(|start| start.holder.pid);
    let holder_started = holder.and_then(|start| start.holder.os_started_at);
    let terminal = holder.and_then(|start| start.terminal.as_ref());
    if let Some(previous) = &current {
        // A Session runs one agent: a replaced one has ended, or never spawned.
        let (ended, reserved) = agent_process_progress_in(tx, &previous.agent_process_id)?;
        if !ended && !reserved {
            return Err(no_observed_exit());
        }
        end_agent_in(
            tx,
            session,
            &previous.agent_process_id,
            time::OffsetDateTime::now_utc().unix_timestamp(),
            false,
        )?;
    }
    // A new agent and its token land in one write, so the bridge to a
    // released reader's agent row sees them together. A new agent on a
    // finished headless run is working again; its exit completes the run
    // once more. A moved conversation stays moved.
    tx.execute(
        "UPDATE agent_sessions SET agent_process_id=?2,agent_launcher_lf_process_id=?3,
            agent_started_at=?4,agent_pid=NULL,agent_os_started_at=NULL,agent_ended_at=NULL,
            agent_exit_code=NULL,agent_spawn_state='reserved',agent_command=NULL,
            agent_interactive=interactive,agent_endpoint=NULL,agent_inbox=NULL,agent_socket=NULL,
            attachment_token=?5,attached_lf_process_id=?3,attached_pid=?6,
            attached_os_started_at=?7,attached_terminal_id=?8,
            completed_at=CASE WHEN interactive=0 AND moved_to_machine_id IS NULL
                THEN NULL ELSE completed_at END WHERE id=?1",
        params![
            session,
            attached.agent_process_id,
            process,
            time::OffsetDateTime::now_utc().unix_timestamp(),
            attached.token,
            holder_pid,
            holder_started,
            terminal,
        ],
    )?;
    Ok(attached)
}

impl SqliteStore {
    pub fn unfinished_processes(&self) -> StoreResult<Vec<ProcessEntry>> {
        unfinished_processes(&self.conn.lock().expect("store mutex poisoned"))
    }

    /// Every lf an owner record says runs, with the OS process it recorded:
    /// unended runs, carriers and CI repairs, and the lf attached to each
    /// Session.
    pub fn recorded_lfs(&self) -> StoreResult<Vec<RecordedLf>> {
        let mut connection = self.conn.lock().expect("store mutex poisoned");
        let conn = connection.savepoint()?;
        // Before promotion a released store may still record every live lf in
        // processes. Only the migrated schema also records them on their owners.
        if !crate::migration_runner::migration_is_applied(&conn, "process_rows_on_owners")? {
            return Ok(Vec::new());
        }
        let mut query = conn.prepare_cached(
            "SELECT 'Graph run',lf_process_id,pid,os_started_at,started_at FROM graph_processes
                 WHERE ended_at IS NULL AND pid IS NOT NULL
             UNION ALL SELECT 'Graph step',lf_process_id,pid,os_started_at,started_at
                 FROM graph_process_steps WHERE ended_at IS NULL AND pid IS NOT NULL
             UNION ALL SELECT 'Task run',lf_process_id,run_pid,run_os_started_at,NULL
                 FROM task_graphs WHERE lf_process_id IS NOT NULL AND run_pid IS NOT NULL
             UNION ALL SELECT 'CI repair',repair_lf_process_id,repair_pid,repair_os_started_at,
                 repair_started_at FROM ci_incidents
                 WHERE repair_lf_process_id IS NOT NULL AND repair_finished_at IS NULL AND repair_pid IS NOT NULL
             UNION ALL SELECT 'Session',attached_lf_process_id,attached_pid,attached_os_started_at,
                 NULL FROM agent_sessions INDEXED BY agent_sessions_attached
                 WHERE attached_lf_process_id IS NOT NULL AND attached_pid IS NOT NULL",
        )?;
        let rows = query.query_map([], |row| {
            Ok(RecordedLf {
                what: row.get(0)?,
                id: row.get(1)?,
                pid: row.get(2)?,
                os_started_at: row.get(3)?,
                started_at: row.get(4)?,
            })
        })?;
        Ok(rows.collect::<Result<Vec<_>, _>>()?)
    }

    /// Fixtures read what a released lf wrote; this lf writes no Process row.
    #[cfg(any(test, feature = "test-support"))]
    pub fn processes_since(&self, since: i64) -> StoreResult<Vec<ProcessEntry>> {
        let conn = self.conn.lock().expect("store mutex poisoned");
        let mut query = conn.prepare(&format!(
            "{PROCESS_SELECT} WHERE e.started_at>=?1 ORDER BY e.started_at,e.id"
        ))?;
        let records = query
            .query_map([since], read_process)?
            .collect::<Result<Vec<_>, _>>()?;
        Ok(records)
    }

    pub fn resolve_wave_id(
        &self,
        selector: &str,
        repo: Option<&str>,
    ) -> StoreResult<Option<loopflow_model::id::WaveId>> {
        let canonical = loopflow_model::id::canonical_uuid(selector);
        self.resolve_work_id(
            "SELECT id FROM wave_addresses WHERE id=?1 OR (slug=?1 AND (?2 IS NULL OR repo=?2))
            ORDER BY (id=?1) DESC,id LIMIT 2",
            &canonical,
            repo,
        )?
        .map(|id| {
            loopflow_model::id::WaveId::parse(&id)
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
    pub fn lf_process(&self, id: &LfProcessId) -> StoreResult<Option<LfProcess>> {
        let conn = self.conn.lock().expect("store mutex poisoned");
        Ok(conn
            .query_row(
                &format!("{PROCESS_SELECT} WHERE e.id=?1 AND e.kind='lf'"),
                [id],
                read_lf_process,
            )
            .optional()?)
    }

    /// The agent row the bridge keeps for a released reader, by its id.
    /// Fixtures inspect it; this lf reads its agents from their Sessions.
    #[cfg(any(test, feature = "test-support"))]
    pub fn agent_process(&self, id: &AgentProcessId) -> StoreResult<Option<AgentProcess>> {
        let conn = self.conn.lock().expect("store mutex poisoned");
        Ok(conn
            .query_row(
                &format!("{PROCESS_SELECT} WHERE e.id=?1 AND e.kind='agent'"),
                [id],
                read_agent_process,
            )
            .optional()?)
    }

    /// Who started a run: the Session whose agent started it, else `person`.
    pub fn starter(
        &self,
        session: Option<&loopflow_model::id::LfSessionId>,
        person: Option<&str>,
    ) -> StoreResult<crate::process::Starter> {
        use crate::process::Starter;
        let Some(session) = session else {
            return Ok(Starter::Person {
                name: person.map(str::to_owned),
            });
        };
        let conn = self.conn.lock().expect("store mutex poisoned");
        let name = conn
            .query_row(
                "SELECT title FROM agent_sessions WHERE id=?1",
                [session.as_str()],
                |row| row.get::<_, Option<String>>(0),
            )
            .optional()?
            .flatten()
            .filter(|title| !title.is_empty());
        Ok(Starter::Session {
            id: session.clone(),
            name,
            requester: person.map(str::to_owned),
        })
    }

    pub fn make_session_interactive(
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

    pub fn saved_session_connection(
        &self,
        session: &str,
    ) -> StoreResult<Option<SavedSessionConnection>> {
        let conn = self.conn.lock().expect("store mutex poisoned");
        Ok(conn
            .query_row(
                "SELECT agent_endpoint,provider_thread,agent_ended_at,agent_pid,agent_os_started_at
             FROM agent_sessions
             WHERE id=?1 AND agent_endpoint IS NOT NULL AND provider_thread IS NOT NULL",
                [session],
                |row| {
                    Ok(SavedSessionConnection {
                        endpoint: row.get(0)?,
                        thread: row.get(1)?,
                        completed_at: row.get(2)?,
                        pid: row.get(3)?,
                        os_started_at: row.get(4)?,
                    })
                },
            )
            .optional()?)
    }

    pub fn session_thread(&self, session: &str) -> StoreResult<Option<AgentSessionId>> {
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

    /// A recorded lf Process for fixtures that attach as someone other than
    /// the test's own journal Process.
    #[cfg(any(test, feature = "test-support"))]
    pub fn test_lf_process(&self) -> LfProcessId {
        let id = LfProcessId::new();
        self.conn
            .lock()
            .expect("store mutex poisoned")
            .execute(
                "INSERT INTO processes(id,trace_id,started_at) VALUES(?1,?1,1)",
                [&id],
            )
            .expect("record fixture lf Process");
        id
    }

    /// Record `pid` started at `os_started_at` as the OS process of the lf
    /// holding `session`'s attachment, for fixtures whose holder is not the
    /// test's own journal Process.
    #[cfg(any(test, feature = "test-support"))]
    pub fn test_set_holder(&self, session: &str, pid: u32, os_started_at: i64) {
        self.conn
            .lock()
            .expect("store mutex poisoned")
            .execute(
                "UPDATE agent_sessions SET attached_pid=?2,attached_os_started_at=?3
                 WHERE id=?1 AND attached_lf_process_id IS NOT NULL",
                params![
                    loopflow_model::id::canonical_session(session),
                    pid,
                    os_started_at
                ],
            )
            .expect("record fixture holder");
    }

    #[cfg(any(test, feature = "test-support"))]
    pub fn agent_process_identity(&self, session: &str) -> StoreResult<Option<(u32, i64)>> {
        let conn = self.conn.lock().expect("store mutex poisoned");
        Ok(conn
            .query_row(
                "SELECT agent_pid,agent_os_started_at FROM agent_sessions WHERE id=?1
             AND agent_pid IS NOT NULL AND agent_os_started_at IS NOT NULL",
                [session],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .optional()?)
    }

    /// Record positive pre-spawn evidence under the exact admission fence.
    /// Once spawn is requested, missing process evidence remains unknown.
    pub fn record_agent_process_launch(
        &self,
        session: &str,
        expected: &SessionAttachment,
        command: &std::process::Command,
        interactive: bool,
    ) -> StoreResult<()> {
        let mut conn = self.conn.lock().expect("store mutex poisoned");
        let tx = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
        require_attachment_in(&tx, session, expected)?;
        let argv = std::iter::once(command.get_program())
            .chain(command.get_args())
            .map(|arg| arg.to_string_lossy().into_owned())
            .collect::<Vec<_>>();
        let changed = tx.execute(
            "UPDATE agent_sessions SET agent_spawn_state='spawn_requested',agent_command=?2,
                agent_interactive=?3
            WHERE agent_process_id=?1 AND agent_ended_at IS NULL
                AND agent_spawn_state='reserved' AND agent_pid IS NULL",
            params![
                expected.agent_process_id,
                serde_json::to_string(&argv)?,
                interactive
            ],
        )?;
        if changed != 1 {
            return Err(StoreError::InvalidAuthority(
                "AgentProcess already has a launch attempt; a new launch needs a new record".into(),
            ));
        }
        tx.commit()?;
        Ok(())
    }

    /// Only the launcher holding this attachment may report its spawn failure
    /// or the exit it actually waited for.
    pub fn record_agent_process_exit(
        &self,
        session: &str,
        expected: &SessionAttachment,
        spawned: bool,
    ) -> StoreResult<()> {
        let mut conn = self.conn.lock().expect("store mutex poisoned");
        let tx = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
        require_attachment_in(&tx, session, expected)?;
        tx.execute(
            "UPDATE agent_sessions SET agent_spawn_state=?2,agent_ended_at=?3,agent_pid=NULL,
                agent_os_started_at=NULL WHERE agent_process_id=?1",
            params![
                expected.agent_process_id,
                if spawned { "exited" } else { "spawn_failed" },
                time::OffsetDateTime::now_utc().unix_timestamp()
            ],
        )?;
        tx.commit()?;
        Ok(())
    }

    /// The provider's own exit code, as the launcher that became it saw it.
    pub fn record_agent_exit_code(&self, agent: &AgentProcessId, code: i32) -> StoreResult<()> {
        self.conn.lock().expect("store mutex poisoned").execute(
            "UPDATE agent_sessions SET agent_exit_code=?2 WHERE agent_process_id=?1",
            params![agent, code],
        )?;
        Ok(())
    }

    pub fn record_agent_process_identity(
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
            "UPDATE agent_sessions SET agent_pid=?2,agent_os_started_at=?3 WHERE agent_process_id=?1
             AND agent_ended_at IS NULL AND ((agent_pid IS NULL AND agent_os_started_at IS NULL)
                 OR (agent_pid=?2 AND agent_os_started_at=?3))",
            params![expected.agent_process_id, pid, started_at],
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

    /// Select the native conversation observed by the currently attached provider.
    pub fn record_session_agent_session(
        &self,
        session: &str,
        expected: &SessionAttachment,
        thread: &AgentSessionId,
    ) -> StoreResult<()> {
        let mut conn = self.conn.lock().expect("store mutex poisoned");
        let tx = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
        require_attachment_in(&tx, session, expected)?;
        let transition =
            super::provider_transitions::observe_in(&tx, session, thread.as_str(), "observed")?;
        tx.execute(
            "UPDATE agent_sessions SET provider_thread=?2 WHERE id=?1",
            params![session, thread],
        )?;
        tx.commit()?;
        drop(conn);
        self.append_history("provider transition", &Vec::from_iter(transition));
        Ok(())
    }

    pub fn agent_endpoint(&self, process: &AgentProcessId) -> StoreResult<Option<String>> {
        let conn = self.conn.lock().expect("store mutex poisoned");
        conn.query_row(
            "SELECT agent_endpoint FROM agent_sessions WHERE agent_process_id=?1",
            [process],
            |row| row.get(0),
        )
        .optional()
        .map(Option::flatten)
        .map_err(Into::into)
    }

    pub fn record_agent_endpoint(
        &self,
        session: &str,
        expected: &SessionAttachment,
        endpoint: &str,
    ) -> StoreResult<()> {
        let mut conn = self.conn.lock().expect("store mutex poisoned");
        let tx = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
        require_attachment_in(&tx, session, expected)?;
        tx.execute(
            "UPDATE agent_sessions SET agent_endpoint=?2 WHERE agent_process_id=?1",
            params![expected.agent_process_id, endpoint],
        )?;
        tx.commit()?;
        Ok(())
    }

    /// The agent the Session's current AgentProcess runs on; a retry may move
    /// the conversation to another provider or model. Closing the process and
    /// routing a steer both go by the recorded provider.
    pub fn record_session_agent(
        &self,
        session: &str,
        expected: &SessionAttachment,
        provider: &str,
        model: Option<&str>,
    ) -> StoreResult<()> {
        let mut conn = self.conn.lock().expect("store mutex poisoned");
        let tx = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
        require_attachment_in(&tx, session, expected)?;
        tx.execute(
            "UPDATE agent_sessions SET provider=?2,model=?3 WHERE agent_process_id=?1",
            params![expected.agent_process_id, provider, model],
        )?;
        tx.commit()?;
        Ok(())
    }

    pub fn record_session_connection(
        &self,
        session: &str,
        expected: &SessionAttachment,
        endpoint: &str,
        thread: &AgentSessionId,
    ) -> StoreResult<()> {
        let mut conn = self.conn.lock().expect("store mutex poisoned");
        let tx = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
        require_attachment_in(&tx, session, expected)?;
        tx.execute(
            "UPDATE agent_sessions SET agent_endpoint=?2 WHERE agent_process_id=?1",
            params![expected.agent_process_id, endpoint],
        )?;
        let transition =
            super::provider_transitions::observe_in(&tx, session, thread.as_str(), "observed")?;
        tx.execute(
            "UPDATE agent_sessions SET provider_thread=?2 WHERE id=?1",
            params![session, thread],
        )?;
        tx.commit()?;
        drop(conn);
        self.append_history("provider transition", &Vec::from_iter(transition));
        Ok(())
    }

    // Dispatch and attachment changes share a per-Session OS lock, never a SQLite
    // transaction across provider I/O. Do not unlink lock files: another process
    // may already have the inode open. Process exit releases ownership.
    pub(super) fn lock_session_attachment(&self, session: &str) -> StoreResult<File> {
        self.lock_session_fence(session, false)
    }

    /// Independent transport writers may progress together, but neither may
    /// cross a provider stop or replacement.
    pub(super) fn lock_session_dispatch(&self, session: &str) -> StoreResult<File> {
        self.lock_session_fence(session, true)
    }

    fn session_fence_file(&self, session: &str) -> StoreResult<File> {
        let conn = self.conn.lock().expect("store mutex poisoned");
        let database = conn.path().map(str::to_owned).ok_or_else(|| {
            StoreError::InvalidData("Session dispatch requires a file-backed store".into())
        })?;
        let locator: String = conn
            .query_row(
                "SELECT fence_locator FROM agent_sessions WHERE id=?1",
                [loopflow_model::id::canonical_session(session)],
                |row| row.get(0),
            )
            .optional()?
            .ok_or(StoreError::NotFound)?;
        drop(conn);
        crate::canonical_ids::session_fence_file(Path::new(&database), &locator)
    }

    #[cfg(any(test, feature = "test-support"))]
    pub fn try_lock_session_attachment(&self, session: &str) -> StoreResult<Option<File>> {
        let file = self.session_fence_file(session)?;
        match file.try_lock_exclusive() {
            Ok(()) => Ok(Some(file)),
            Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => Ok(None),
            Err(error) => Err(StoreError::InvalidData(error.to_string())),
        }
    }

    fn lock_session_fence(&self, session: &str, shared: bool) -> StoreResult<File> {
        let file = self.session_fence_file(session)?;
        // A synchronous caller must never wait indefinitely on a dispatch whose
        // reactor it might be driving. This bound uses the OS clock, not Tokio.
        let deadline = Instant::now() + Duration::from_secs(15);
        loop {
            let locked = if shared {
                FileExt::try_lock_shared(&file)
            } else {
                file.try_lock_exclusive()
            };
            match locked {
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

    /// Serialize native dispatch with attachment transfer without blocking history
    /// or unrelated database writes while the bounded transport write runs.
    pub fn with_session_attachment<T>(
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

    /// The transport owns serialization of its writes. Hold shared exclusion
    /// through the bounded write so synchronous provider hooks can also emit
    /// before the provider drains its input.
    pub fn with_session_dispatch<T>(
        &self,
        session: &str,
        expected: &SessionAttachment,
        write: impl FnOnce() -> StoreResult<T>,
    ) -> StoreResult<T> {
        let _dispatch = self.lock_session_dispatch(session)?;
        {
            let conn = self.conn.lock().expect("store mutex poisoned");
            require_attachment_in(&conn, session, expected)?;
        }
        write()
    }

    /// Every Session's agent that has not ended, attached or released.
    pub fn agent_processes(&self) -> StoreResult<Vec<crate::process::AgentProcessState>> {
        let conn = self.conn.lock().expect("store mutex poisoned");
        let mut query = conn.prepare(SESSION_AGENTS)?;
        let rows = query
            .query_map([], read_agent_state)?
            .collect::<rusqlite::Result<Vec<_>>>()?;
        Ok(rows)
    }

    /// The agent `id`, ended or not, while it is its Session's current one.
    pub fn agent_state(
        &self,
        id: &AgentProcessId,
    ) -> StoreResult<Option<crate::process::AgentProcessState>> {
        let conn = self.conn.lock().expect("store mutex poisoned");
        Ok(conn
            .query_row(
                &SESSION_AGENTS
                    .replace("s.agent_ended_at IS NULL", "s.agent_process_id=?1")
                    .replace(" ORDER BY s.agent_process_id", ""),
                [id],
                read_agent_state,
            )
            .optional()?)
    }

    /// Acquire the Session fence and compare the complete saved process image.
    /// The returned claim keeps takeover excluded through external termination.
    pub fn begin_agent_settlement(
        &self,
        expected: &crate::process::AgentProcessState,
    ) -> StoreResult<Option<AgentProcessSettlement<'_>>> {
        let dispatch = self.lock_session_attachment(&expected.session_id)?;
        let matches: bool = {
            let conn = self.conn.lock().expect("store mutex poisoned");
            conn.query_row(
                "SELECT EXISTS(SELECT 1 FROM agent_sessions WHERE agent_process_id=?1
              AND agent_pid IS ?2 AND agent_os_started_at IS ?3 AND attached_lf_process_id IS ?4
              AND attachment_token IS ?5 AND agent_ended_at IS NULL
              AND (agent_spawn_state IS 'reserved' AND agent_pid IS NULL)=?6)",
                params![
                    expected.id,
                    expected.pid,
                    expected.os_started_at,
                    expected.attached_lf_process_id,
                    expected.attachment_token,
                    expected.spawn_reserved
                ],
                |row| row.get(0),
            )?
        };
        Ok(matches.then(|| AgentProcessSettlement {
            store: self,
            expected: expected.clone(),
            _dispatch: dispatch,
        }))
    }

    /// Terminal placement is owned by the current attached lf, not by provider receipts.
    pub fn record_session_terminal(
        &self,
        session: &str,
        expected: &SessionAttachment,
        terminal: Option<&str>,
    ) -> StoreResult<()> {
        self.with_session_attachment(session, expected, || {
            let conn = self.conn.lock().expect("store mutex poisoned");
            conn.execute(
                "UPDATE agent_sessions SET attached_terminal_id=?2 WHERE id=?1",
                params![loopflow_model::id::canonical_session(session), terminal],
            )?;
            Ok(())
        })
    }

    pub fn session_attachment(&self, session: &str) -> StoreResult<Option<SessionAttachment>> {
        let conn = self.conn.lock().expect("store mutex poisoned");
        attachment_in(&conn, session)
    }

    pub fn attach_session(
        &self,
        session: &str,
        expected: Option<&SessionAttachment>,
        process: &LfProcessId,
        start: Option<&AttachingLf>,
        caller: &SessionCaller,
    ) -> StoreResult<SessionAttachment> {
        let canonical = loopflow_model::id::canonical_session(session);
        let session = canonical.as_str();
        let _dispatch = self.lock_session_attachment(session)?;
        let mut conn = self.conn.lock().expect("store mutex poisoned");
        let tx = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
        super::sessions::require_current_actor_in(&tx, session, caller)?;
        let attachment = attach_in(&tx, session, expected, process, start)?;
        tx.commit()?;
        Ok(attachment)
    }

    /// A bound or primary conversation reopens managed; attribution alone
    /// leaves a Session bare.
    pub fn is_bare(&self, session: &crate::session::LfSession) -> StoreResult<bool> {
        Ok(session.bound_at.is_none() && self.primary_scope(&session.id)?.is_none())
    }

    /// Hold attachment transfer across acquisition and the eventual SQL claim.
    pub fn begin_session_transfer(
        &self,
        session: &str,
        expected: Option<&SessionAttachment>,
    ) -> StoreResult<SessionTransfer<'_>> {
        let session = loopflow_model::id::canonical_session(session);
        let dispatch = self.lock_session_attachment(&session)?;
        if self.session_attachment(&session)?.as_ref() != expected {
            return Err(attachment_changed());
        }
        Ok(SessionTransfer {
            store: self,
            session,
            expected: expected.cloned(),
            _dispatch: dispatch,
        })
    }

    /// Take over whatever attachment the Session has once its fence is held. A
    /// previous provider's exit may still be settling; reading the attachment
    /// before the fence would race that settlement.
    pub fn begin_session_resume(&self, session: &str) -> StoreResult<SessionTransfer<'_>> {
        let session = loopflow_model::id::canonical_session(session);
        let dispatch = self.lock_session_attachment(&session)?;
        let expected = self.session_attachment(&session)?;
        Ok(SessionTransfer {
            store: self,
            session,
            expected,
            _dispatch: dispatch,
        })
    }

    /// Reserve the next OS process for the same invocation after an observed
    /// exit. An uncertain spawn cannot be retried by overwriting its identity.
    pub fn prepare_agent_process(
        &self,
        session: &str,
        expected: &SessionAttachment,
        start: Option<&AttachingLf>,
    ) -> StoreResult<SessionAttachment> {
        let _dispatch = self.lock_session_attachment(session)?;
        let mut conn = self.conn.lock().expect("store mutex poisoned");
        let tx = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
        require_attachment_in(&tx, session, expected)?;
        let (ended, reserved) = agent_process_progress_in(&tx, &expected.agent_process_id)?;
        let attachment = if ended {
            attach_in(
                &tx,
                session,
                Some(expected),
                expected
                    .lf_process_id
                    .as_ref()
                    .expect("attachment is owned"),
                start,
            )?
        } else if reserved {
            expected.clone()
        } else {
            return Err(no_observed_exit());
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
            return Err(attachment_changed());
        }
        current.lf_process_id = None;
        current.token = AttachmentToken::new();
        release_attachment_in(&tx, session, &current.token)?;
        tx.commit()?;
        Ok(current)
    }

    /// Where the conversation went when `attachment` ended, if a mover said.
    /// The Session keeps it under the ended token, so a later attachment
    /// never answers for an earlier one.
    pub fn attachment_departure(
        &self,
        session: &str,
        attachment: &SessionAttachment,
    ) -> StoreResult<Option<crate::process::Departure>> {
        let canonical = loopflow_model::id::canonical_session(session);
        let conn = self.conn.lock().expect("store mutex poisoned");
        let departure = conn
            .query_row(
                "SELECT departure FROM agent_sessions WHERE id=?1 AND departed_attachment=?2",
                params![canonical.as_str(), attachment.token.to_string()],
                |row| row.get::<_, Option<String>>(0),
            )
            .optional()?
            .flatten();
        departure
            .map(|departure| serde_json::from_str(&departure))
            .transpose()
            .map_err(|error| StoreError::InvalidData(error.to_string()))
    }
}

#[cfg(test)]
mod discovery_tests {
    use loopflow_model::id::LfProcessId;

    use crate::test_fixture::process as insert_process;

    #[test]
    fn legacy_unknown_spawn_state_is_not_a_reservation_or_an_exit() {
        let home = tempfile::tempdir().unwrap();
        let store = crate::test_fixture::open(&home.path().join("store.db")).unwrap();
        store.test_session("session", &crate::session_record::new_artifact_key());
        let attachment = store
            .attach_session(
                "session",
                None,
                &store.test_lf_process(),
                None,
                &crate::sqlite::SessionCaller {
                    agent: None,
                    capture_key: None,
                },
            )
            .unwrap();
        let conn = store.conn.lock().unwrap();
        conn.execute(
            "UPDATE processes SET spawn_state=NULL WHERE id=?1",
            [&attachment.agent_process_id],
        )
        .unwrap();
        assert_eq!(
            super::agent_process_progress_in(&conn, &attachment.agent_process_id).unwrap(),
            (false, false)
        );
    }

    #[test]
    fn durable_identity_keeps_reused_and_unknown_pids_distinct() {
        let dir = tempfile::tempdir().unwrap();
        let store = crate::test_fixture::open(&dir.path().join("store.db")).unwrap();
        let historical = insert_process(&store, 1, 1);
        let retained = store.lf_process(&historical).unwrap().unwrap();
        assert_eq!(retained.pid, None);
        let mut first = retained.clone();
        first.id = LfProcessId::new();
        first.pid = Some(4242);
        store.record_process(&first).unwrap();
        let mut second = first.clone();
        second.id = LfProcessId::new();
        second.parent_lf_process_id = Some(first.id.clone());
        store.record_process(&second).unwrap();
        first.completed_at = Some(3);
        first.outcome = Some("failed".into());
        first.exit_code = Some(42);
        store.record_process(&first).unwrap();
        assert_eq!(store.lf_process(&first.id).unwrap(), Some(first));
        assert_eq!(store.lf_process(&second.id).unwrap(), Some(second));
        assert_eq!(store.lf_process(&historical).unwrap(), Some(retained));
    }
}

#[cfg(test)]
mod attachment_tests {

    use super::SessionCaller;
    use crate::test_fixture::process as insert_process;

    use crate::StoreError;
    use loopflow_model::id::LfProcessId;

    #[test]
    fn replaced_callers_cannot_rename_bind_or_attach_either_title_source() {
        use crate::process::AgentCaller;
        use crate::session::TitleSource;
        use loopflow_model::durable::TaskId;

        let home = tempfile::tempdir().unwrap();
        let store = crate::test_fixture::open(&home.path().join("store.db")).unwrap();
        let process = insert_process(&store, 1, 1);
        let replacement = insert_process(&store, 2, 2);
        for source in [TitleSource::Generated, TitleSource::Human] {
            let id = format!("conversation-{source:?}");
            let session = store.test_session(&id, &crate::session_record::new_artifact_key());
            store
                .conn
                .lock()
                .unwrap()
                .execute(
                    "UPDATE agent_sessions SET title_source=?2 WHERE id=?1",
                    rusqlite::params![
                        id,
                        if source == TitleSource::Generated {
                            "generated"
                        } else {
                            "human"
                        }
                    ],
                )
                .unwrap();
            let absent = SessionCaller {
                agent: None,
                capture_key: None,
            };
            store
                .rename_session_prepared(&id, "Original title", source, &absent)
                .unwrap();
            let first = store
                .attach_session(&id, None, &process, None, &absent)
                .unwrap();
            let caller = SessionCaller {
                agent: Some(AgentCaller {
                    session_id: id.clone().into(),
                    agent_process_id: Some(first.agent_process_id.clone()),
                    origin_lf_process_id: process.clone(),
                    trace_id: None,
                }),
                capture_key: session.artifact_key.clone(),
            };
            // A current provider can act; the same observation loses authority
            // when another provider takes over, even when it knows the new token.
            store
                .rename_session_prepared(&id, "Original title", source, &caller)
                .unwrap();
            let current = store
                .attach_session(&id, Some(&first), &replacement, None, &absent)
                .unwrap();
            let before = store.session(&id).unwrap();
            assert_eq!(before.as_ref().unwrap().title_source, source);
            let history = store.session_history(&id, 0, 0).unwrap();
            for result in [
                store.rename_session_prepared(&id, "Changed", TitleSource::Human, &caller),
                store
                    .bind_session_prepared(
                        &id,
                        session.captured,
                        &TaskId::new(),
                        &session.cwd,
                        &caller,
                    )
                    .map(|_| ()),
                store
                    .attach_session(&id, Some(&current), &process, None, &caller)
                    .map(|_| ()),
            ] {
                assert!(
                    matches!(result, Err(StoreError::InvalidAuthority(ref message)) if message == "Session provider was replaced"),
                    "{result:?}"
                );
            }
            assert_eq!(store.session(&id).unwrap(), before);
            assert_eq!(store.session_history(&id, 0, 0).unwrap(), history);
            assert_eq!(store.session_attachment(&id).unwrap(), Some(current));
        }
    }

    #[test]
    fn rename_refuses_a_capture_owned_by_another_callers_session() {
        use crate::process::AgentCaller;
        let home = tempfile::tempdir().unwrap();
        let store = crate::test_fixture::open(&home.path().join("store.db")).unwrap();
        let session =
            store.test_session("conversation", &crate::session_record::new_artifact_key());
        let other = store.test_session("other", &crate::session_record::new_artifact_key());
        let process = insert_process(&store, 1, 1);
        let first = store
            .attach_session(
                "conversation",
                None,
                &process,
                None,
                &SessionCaller {
                    agent: None,
                    capture_key: None,
                },
            )
            .unwrap();
        let caller = SessionCaller {
            agent: Some(AgentCaller {
                session_id: session.id.clone(),
                agent_process_id: Some(first.agent_process_id),
                origin_lf_process_id: process,
                trace_id: None,
            }),
            capture_key: other.artifact_key,
        };
        let result = store.rename_session_prepared(
            "conversation",
            "Changed",
            crate::session::TitleSource::Human,
            &caller,
        );
        assert!(
            matches!(result, Err(StoreError::InvalidData(ref message)) if message == "capture belongs to another Session")
        );
        assert_eq!(store.session("conversation").unwrap(), Some(session));
    }

    #[test]
    fn attaching_and_its_terminal_commit_only_with_the_agent() {
        let home = tempfile::tempdir().unwrap();
        let store = crate::test_fixture::open(&home.path().join("store.db")).unwrap();
        store.test_session("conversation", &crate::session_record::new_artifact_key());
        let terminal = loopflow_model::id::TerminalId::new();
        store.create_terminal(&terminal, "fixture").unwrap();
        let start = crate::process::AttachingLf {
            lf_process_id: LfProcessId::new(),
            holder: crate::process::AttachmentHolder {
                pid: Some(1),
                os_started_at: Some(1),
            },
            terminal: Some(terminal.clone()),
        };
        store
            .conn
            .lock()
            .unwrap()
            .execute_batch(
                "CREATE TEMP TRIGGER reject_agent BEFORE INSERT ON processes
             WHEN NEW.kind='agent' BEGIN SELECT RAISE(ABORT,'agent insert failed'); END;",
            )
            .unwrap();
        assert!(store
            .attach_session(
                "conversation",
                None,
                &start.lf_process_id,
                Some(&start),
                &SessionCaller {
                    agent: None,
                    capture_key: None
                }
            )
            .is_err());
        assert_eq!(store.lf_process(&start.lf_process_id).unwrap(), None);
        assert_eq!(store.session_attachment("conversation").unwrap(), None);
        store
            .conn
            .lock()
            .unwrap()
            .execute_batch("DROP TRIGGER reject_agent")
            .unwrap();
        let attached = store
            .attach_session(
                "conversation",
                None,
                &start.lf_process_id,
                Some(&start),
                &SessionCaller {
                    agent: None,
                    capture_key: None,
                },
            )
            .unwrap();
        // No lf writes its own Process row: the attachment and its terminal
        // are on the Session, and the agent row a released reader joins
        // names its launcher.
        assert_eq!(store.lf_process(&start.lf_process_id).unwrap(), None);
        let agent = store
            .agent_process(&attached.agent_process_id)
            .unwrap()
            .unwrap();
        assert_eq!(
            agent.parent_lf_process_id.as_ref(),
            Some(&start.lf_process_id)
        );
        let placement: loopflow_model::id::TerminalId = store
            .conn
            .lock()
            .unwrap()
            .query_row(
                "SELECT attached_terminal_id FROM agent_sessions WHERE id='conversation'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(placement, terminal);
    }

    #[test]
    fn attachment_migration_preserves_released_history_and_waiting_evidence() {
        use crate::migration_runner::{apply_before_migration, migration_section_sql};
        let conn = rusqlite::Connection::open_in_memory().unwrap();
        apply_before_migration(
            &crate::test_fixture::observers(),
            &crate::test_fixture::build(),
            &conn,
            "agent_process",
        );
        conn.execute_batch(include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/tests/fixtures/migrations/agent_process_released.sql"
        )))
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
        conn.execute_batch(&migration_section_sql("agent_process"))
            .unwrap();
        assert_eq!(history(), before);
        conn.execute_batch(&migration_section_sql("lf_process_ids"))
            .unwrap();
        conn.execute_batch(&migration_section_sql("process_terminal"))
            .unwrap();
        let terminal: Option<String> = conn
            .query_row(
                "SELECT terminal_id FROM processes WHERE id='parent'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert!(
            terminal.is_none(),
            "migration does not invent terminal ownership"
        );
        let renamed = history();
        assert_eq!(renamed[0].2, "attachment:7:exit");
        assert_eq!(
            (renamed[0].0, &renamed[0].1, &renamed[0].3),
            (before[0].0, &before[0].1, &before[0].3)
        );
        assert_eq!(renamed[1..], before[1..]);
        let live: (String, String, String, u32, i64, i64) = conn.query_row(
            "SELECT p.attached_lf_process_id,p.parent_lf_process_id,s.provider_thread,p.pid,p.os_started_at,s.current_capture FROM agent_sessions s JOIN processes p ON p.id=s.agent_process_id WHERE s.id='live'",
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
        let matched: i64 = conn.query_row("SELECT COUNT(*) FROM session_activity a JOIN agent_sessions s ON s.id=a.session_id JOIN processes p ON p.id=s.agent_process_id WHERE a.attachment_token=p.attachment_token", [], |row| row.get(0)).unwrap();
        assert_eq!(matched, 1);
        let exit: i64 = conn
            .query_row(
                "SELECT p.attachment_exit_seq FROM agent_sessions s JOIN processes p ON p.id=s.agent_process_id WHERE s.id='released'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(exit, before[0].0);
        // Only the current generation has a record. Earlier generations stay
        // unknown, and the released counters remain beside them as history.
        let agent: String = conn
            .query_row(
                "SELECT agent_process_id FROM agent_sessions WHERE id='live'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        let identities = |sql: &str| {
            conn.prepare(sql)
                .unwrap()
                .query_map([], |row| {
                    Ok((row.get::<_, Option<String>>(0)?, row.get::<_, i64>(1)?))
                })
                .unwrap()
                .collect::<rusqlite::Result<Vec<_>>>()
                .unwrap()
        };
        let expected = vec![(Some(agent.clone()), 3), (None, 2)];
        assert_eq!(
            identities(
                "SELECT agent_process_id,provider_generation FROM session_events
                 WHERE kind='started' ORDER BY seq"
            ),
            expected
        );
        assert_eq!(
            identities(
                "SELECT caller_agent_process_id,caller_provider_generation FROM processes
                 WHERE caller_session_id='live' ORDER BY started_at DESC"
            ),
            expected
        );
        let reading: Option<String> = conn
            .query_row(
                "SELECT agent_process_id FROM session_activity WHERE session_id='live'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(reading, Some(agent));
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
            assert!(!columns.iter().any(|column| column == "provider_generation"));
        }
    }
    #[test]
    fn a_released_agent_stays_in_the_inventory_and_blocks_replacement_until_it_exits() {
        let home = tempfile::tempdir().unwrap();
        let store = crate::test_fixture::open(&home.path().join("db")).unwrap();
        let session =
            store.test_session("conversation", &crate::session_record::new_artifact_key());
        let a = insert_process(&store, 1, 1);
        let b = insert_process(&store, 2, 2);
        let first = store
            .attach_session(
                &session.id,
                None,
                &a,
                None,
                &crate::sqlite::SessionCaller {
                    agent: None,
                    capture_key: None,
                },
            )
            .unwrap();
        store
            .record_agent_process_identity(&session.id, &first, 4242, 123)
            .unwrap();
        let released = store
            .release_session_attachment(&session.id, &first)
            .unwrap();
        let rows = store.agent_processes().unwrap();
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].launcher_lf_process_id, Some(a.clone()));
        assert_eq!(rows[0].attached_lf_process_id, None);
        assert_eq!(rows[0].pid, Some(4242));
        assert!(matches!(
            store.record_agent_process_identity(&session.id, &first, 5555, 123),
            Err(StoreError::InvalidAuthority(_))
        ));
        // A Session runs one agent: the released one still runs, so nothing
        // replaces it until it exits.
        assert!(matches!(
            store.attach_session(
                &session.id,
                Some(&released),
                &b,
                None,
                &crate::sqlite::SessionCaller {
                    agent: None,
                    capture_key: None,
                },
            ),
            Err(StoreError::InvalidAuthority(_))
        ));
        assert_eq!(store.agent_processes().unwrap().len(), 1);
    }

    #[test]
    fn agent_spawn_lifecycle_uses_the_record_and_retains_failed_identity() {
        let home = tempfile::tempdir().unwrap();
        let store = crate::test_fixture::open(&home.path().join("db")).unwrap();
        let session =
            store.test_session("conversation", &crate::session_record::new_artifact_key());
        let parent = insert_process(&store, 1, 1);
        let attachment = store
            .attach_session(
                &session.id,
                None,
                &parent,
                None,
                &crate::sqlite::SessionCaller {
                    agent: None,
                    capture_key: None,
                },
            )
            .unwrap();
        let history = store.session_history(&session.id, 0, 0).unwrap();
        let mut command = std::process::Command::new("codex");
        command.args(["app-server", "--listen", "unix:///fixture"]);
        store
            .record_agent_process_launch(&session.id, &attachment, &command, false)
            .unwrap();
        store
            .record_agent_process_identity(&session.id, &attachment, 4242, 123)
            .unwrap();
        store
            .record_agent_process_exit(&session.id, &attachment, false)
            .unwrap();
        let row = store
            .agent_process(&attachment.agent_process_id)
            .unwrap()
            .unwrap();
        assert_eq!(row.pid, Some(4242));
        assert_eq!(row.os_started_at, Some(123));
        assert!(store
            .record_agent_process_identity(&session.id, &attachment, 5555, 124)
            .is_err());
        assert_eq!(
            store
                .agent_process(&attachment.agent_process_id)
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
        assert_eq!(store.session_history(&session.id, 0, 0).unwrap(), history);
        assert!(store.agent_processes().unwrap().is_empty());
    }
}

/// A released lf still running after install writes only `processes`; the
/// expand migration's bridge copies what it writes onto the owner rows this
/// lf reads. The statements below are 0.16.0's, verbatim.
#[cfg(test)]
mod bridge_tests {
    use rusqlite::params;

    use super::SessionCaller;
    use loopflow_model::id::{LfProcessId, TraceId};

    /// 0.16.0 `record_process_in`: a command's start, and later its end.
    const RELEASED_RECORD_PROCESS: &str =
        "INSERT INTO processes(id,trace_id,parent_lf_process_id,command,repo,cwd,started_at,
            via_agent,caller_session_id,caller_agent_process_id,completed_at,outcome,exit_code,signal,error,pid,os_started_at,
            held_terminal_id)
         VALUES(?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,?12,?13,?14,?15,?16,?17,?18)
         ON CONFLICT(id) DO UPDATE SET completed_at=excluded.completed_at,
            outcome=excluded.outcome,exit_code=excluded.exit_code,signal=excluded.signal,error=excluded.error
         WHERE processes.completed_at IS NULL AND excluded.completed_at IS NOT NULL";
    /// 0.16.0 `record_graph`.
    const RELEASED_RECORD_GRAPH: &str =
        "INSERT INTO graph_processes(lf_process_id,graph,compiled,task_id) VALUES(?1,?2,?3,?4)";
    /// 0.16.0 `record_agent_endpoint`.
    const RELEASED_RECORD_ENDPOINT: &str = "UPDATE processes SET endpoint=?2 WHERE id=?1";

    #[allow(clippy::too_many_arguments)]
    fn record_process(
        conn: &rusqlite::Connection,
        id: &LfProcessId,
        trace: &TraceId,
        started_at: i64,
        completed_at: Option<i64>,
        outcome: Option<&str>,
        exit_code: Option<i32>,
        error: Option<&str>,
    ) {
        conn.execute(
            RELEASED_RECORD_PROCESS,
            params![
                id,
                trace,
                None::<String>,
                r#"["lf","graph","code"]"#,
                "/repo",
                "/repo",
                started_at,
                false,
                None::<String>,
                None::<String>,
                completed_at,
                outcome,
                exit_code,
                None::<String>,
                error,
                4242,
                started_at,
                None::<String>,
            ],
        )
        .unwrap();
    }

    #[test]
    fn a_released_graph_runners_end_ends_its_graph_run() {
        let home = tempfile::tempdir().unwrap();
        let store = crate::test_fixture::open(&home.path().join("store.db")).unwrap();
        let conn = store.conn.lock().unwrap();
        let run = LfProcessId::new();
        let trace = TraceId::new();
        record_process(&conn, &run, &trace, 100, None, None, None, None);
        conn.execute(
            RELEASED_RECORD_GRAPH,
            params![run, "code", "{}", None::<String>],
        )
        .unwrap();
        let row = |conn: &rusqlite::Connection| {
            conn.query_row(
                "SELECT started_at,ended_at,pid,exit_code,error FROM graph_processes
                 WHERE lf_process_id=?1",
                [&run],
                |row| {
                    Ok((
                        row.get::<_, Option<i64>>(0)?,
                        row.get::<_, Option<i64>>(1)?,
                        row.get::<_, Option<u32>>(2)?,
                        row.get::<_, Option<i32>>(3)?,
                        row.get::<_, Option<String>>(4)?,
                    ))
                },
            )
            .unwrap()
        };
        assert_eq!(row(&conn), (Some(100), None, Some(4242), None, None));
        record_process(
            &conn,
            &run,
            &trace,
            100,
            Some(160),
            Some("failed"),
            Some(1),
            Some("gate failed"),
        );
        assert_eq!(
            row(&conn),
            (
                Some(100),
                Some(160),
                None,
                Some(1),
                Some("gate failed".into())
            )
        );
    }

    #[test]
    fn a_released_launchers_endpoint_reaches_its_session() {
        let home = tempfile::tempdir().unwrap();
        let store = crate::test_fixture::open(&home.path().join("store.db")).unwrap();
        store.test_session("conversation", &crate::session_record::new_artifact_key());
        let attachment = store
            .attach_session(
                "conversation",
                None,
                &store.test_lf_process(),
                None,
                &SessionCaller {
                    agent: None,
                    capture_key: None,
                },
            )
            .unwrap();
        store
            .conn
            .lock()
            .unwrap()
            .execute(
                RELEASED_RECORD_ENDPOINT,
                params![attachment.agent_process_id, "ws://127.0.0.1:4500"],
            )
            .unwrap();
        assert_eq!(
            store.agent_endpoint(&attachment.agent_process_id).unwrap(),
            Some("ws://127.0.0.1:4500".into())
        );
    }
}
