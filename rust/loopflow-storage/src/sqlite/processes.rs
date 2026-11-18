use std::fs::File;
use std::num::NonZeroU32;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use fs2::FileExt;
use rusqlite::types::Value;
use rusqlite::{params, params_from_iter, OptionalExtension, TransactionBehavior};

use crate::process::{
    AgentProcess, LfProcess, LfProcessStart, ProcessCursor, ProcessEntry, ProcessFilter,
    ProcessOutcomeFilter, ProcessPage, ProcessWorkFilter, SessionAttachment,
};
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
        start: Option<&LfProcessStart>,
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
        start: Option<&LfProcessStart>,
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
        start: Option<&LfProcessStart>,
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
             AND s.wave_id IS NULL AND {} IS NULL AND NOT EXISTS({}) END",
                super::sessions::SESSION_GRAPH,
                super::task_work::session_tasks("s")
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

/// A finished `lf` Process nothing needs any more, over alias `p`: no agent
/// issued it, no running parent may still read its result, and no row
/// references it. Exit and prune both forget exactly these rows. Agent-side
/// references (steers, Session activity, `caller_agent_process_id`) name only
/// `kind='agent'` rows, which are never forgotten. History and turns name a
/// Process without holding it: they keep it for 30 days after it finished,
/// so recent Task history still finds its work, then let it go.
pub(super) const FORGETTABLE: &str = "p.kind='lf' AND p.completed_at IS NOT NULL
    AND p.via_agent IS NOT 1
    AND NOT EXISTS(SELECT 1 FROM processes r WHERE r.id=p.parent_lf_process_id AND r.completed_at IS NULL)
    AND NOT EXISTS(SELECT 1 FROM processes c WHERE c.parent_lf_process_id=p.id)
    AND NOT EXISTS(SELECT 1 FROM processes a WHERE a.kind='agent' AND a.attached_lf_process_id=p.id)
    AND NOT EXISTS(SELECT 1 FROM session_events WHERE lf_process_id=p.id)
    AND (p.completed_at < CAST(strftime('%s','now') AS INTEGER) - 30*86400 OR (
        NOT EXISTS(SELECT 1 FROM history.session_history WHERE lf_process_id=p.id)
        AND NOT EXISTS(SELECT 1 FROM history.turns WHERE lf_process_id=p.id)))
    AND NOT EXISTS(SELECT 1 FROM graph_processes WHERE lf_process_id=p.id)
    AND NOT EXISTS(SELECT 1 FROM graph_process_steps WHERE lf_process_id=p.id)
    AND NOT EXISTS(SELECT 1 FROM graph_process_steps WHERE graph_lf_process_id=p.id)
    AND NOT EXISTS(SELECT 1 FROM task_graphs WHERE lf_process_id=p.id)
    AND NOT EXISTS(SELECT 1 FROM waves WHERE project_activation_lf_process_id=p.id)
    AND NOT EXISTS(SELECT 1 FROM ci_incidents WHERE repair_lf_process_id=p.id)
    AND NOT EXISTS(SELECT 1 FROM task_ship_events WHERE lf_process_id=p.id)
    AND NOT EXISTS(SELECT 1 FROM agent_sessions WHERE transport_lf_process_id=p.id)
    AND NOT EXISTS(SELECT 1 FROM relocated_task_processes WHERE process_id=p.id)
    AND NOT EXISTS(SELECT 1 FROM desktop_shells WHERE owner=p.id)
    AND NOT EXISTS(SELECT 1 FROM desktop_shells WHERE launcher_lf_process_id=p.id)
    AND NOT EXISTS(SELECT 1 FROM tasks WHERE ship_lf_process_id=p.id)";

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

fn process_query(
    filter: &ProcessFilter,
    after: Option<&ProcessCursor>,
    limit: NonZeroU32,
) -> (String, Vec<Value>) {
    let mut sql = String::from("WITH page AS MATERIALIZED (SELECT e.id FROM processes e WHERE 1");
    let mut values = Vec::new();
    let mut bind = |value| {
        values.push(value);
        format!("?{}", values.len())
    };
    for (column, value) in [
        ("id", filter.id.as_deref()),
        ("repo", filter.repo.as_deref()),
        (
            "parent_lf_process_id",
            filter
                .parent_lf_process_id
                .as_ref()
                .map(LfProcessId::as_str),
        ),
        ("caller_session_id", filter.caller_session_id.as_deref()),
        (
            "caller_agent_process_id",
            filter
                .caller_agent_process_id
                .as_ref()
                .map(AgentProcessId::as_str),
        ),
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
            // Only lf processes record an outcome; a provider process has none.
            ProcessOutcomeFilter::Succeeded => " AND e.kind='lf' AND e.outcome='succeeded'",
            ProcessOutcomeFilter::Failed => " AND e.kind='lf' AND e.outcome='failed'",
            ProcessOutcomeFilter::Interrupted => " AND e.kind='lf' AND e.outcome='interrupted'",
            ProcessOutcomeFilter::Unknown => " AND (e.kind='agent' OR e.outcome IS NULL)",
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
        // Native starts retain their original assignment. A Graph's steps ran
        // in their Task's checkout. Neither a current Session bind nor
        // another Process's command context establishes performed work.
        sql.push_str(&format!(
            " AND e.id IN (
            SELECT lf_process_id FROM history.turns
            WHERE {column}={value} AND lf_process_id IS NOT NULL
            UNION
            SELECT op.id FROM graph_process_steps gs JOIN processes op ON op.id=gs.lf_process_id
            JOIN tasks tw ON {tasks}{value}{close}
            JOIN task_checkouts c ON c.task_id=tw.id
            WHERE op.cwd=rtrim(c.worktree,'/') OR instr(op.cwd,rtrim(c.worktree,'/')||'/')=1
        )"
        ));
    }
    if let Some(after) = after {
        let time = bind(Value::Integer(after.started_at));
        let id = bind(Value::Text(after.id.clone()));
        sql.push_str(&format!(
            " AND e.started_at<={time} AND (e.started_at<{time} OR e.id>{id})"
        ));
    }
    let limit = bind(Value::Integer(i64::from(limit.get()) + 1));
    sql.push_str(&format!(
        " ORDER BY e.started_at DESC,e.id ASC LIMIT {limit})
         {PROCESS_SELECT} JOIN page ON page.id=e.id ORDER BY e.started_at DESC,e.id ASC"
    ));
    (sql, values)
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
    start: Option<&LfProcessStart>,
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
    let holder = start.filter(|start| &start.process.id == process);
    let holder_pid = holder.and_then(|start| start.process.pid);
    let holder_started = holder.and_then(|start| start.process.os_started_at);
    let terminal = holder.and_then(|start| start.terminal.as_ref());
    if let Some(start) = start {
        // A best-effort start receipt can lose to a held write lock; the
        // attaching lf's row lands here instead.
        super::record_process_in(tx, &start.process, start.terminal.as_ref())?;
    }
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
            agent_interactive=interactive,agent_endpoint=NULL,agent_inbox=NULL,
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

    /// The unfinished rows not in `skip`, reading only the index for those in
    /// it. `skip` forgets rows that are no longer unfinished.
    pub fn unfinished_processes_except(
        &self,
        skip: &mut std::collections::HashSet<String>,
    ) -> StoreResult<Vec<ProcessEntry>> {
        let conn = self.conn.lock().expect("store mutex poisoned");
        let ids = conn
            .prepare(
                "SELECT id FROM processes INDEXED BY processes_unfinished
                 WHERE completed_at IS NULL ORDER BY started_at DESC,id",
            )?
            .query_map([], |row| row.get::<_, String>(0))?
            .collect::<rusqlite::Result<Vec<_>>>()?;
        let mut read = conn.prepare(&format!("{PROCESS_SELECT} WHERE e.id=?1"))?;
        let mut records = Vec::new();
        let mut skipped = std::collections::HashSet::new();
        for id in ids {
            if skip.contains(&id) {
                skipped.insert(id);
            } else {
                records.extend(read.query_row([&id], read_process).optional()?);
            }
        }
        *skip = skipped;
        Ok(records)
    }

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

    pub fn process_receipt_locator(
        &self,
        id: &LfProcessId,
    ) -> StoreResult<Option<(PathBuf, String)>> {
        let conn = self.conn.lock().expect("store mutex poisoned");
        let locator: Option<String> = conn
            .query_row(
                "SELECT receipt_locator FROM processes WHERE id=?1",
                [id],
                |row| row.get(0),
            )
            .optional()?;
        let home = conn
            .path()
            .filter(|path| !path.is_empty())
            .and_then(|path| Path::new(path).parent());
        Ok(locator
            .zip(home)
            .map(|(locator, home)| (home.to_path_buf(), locator)))
    }

    pub fn process_for_receipt(&self, locator: &str) -> StoreResult<Option<LfProcess>> {
        let conn = self.conn.lock().expect("store mutex poisoned");
        Ok(conn
            .query_row(
                &format!("{PROCESS_SELECT} WHERE e.receipt_locator=?1 AND e.kind='lf'"),
                [locator],
                read_lf_process,
            )
            .optional()?)
    }

    pub fn journal_trace_locator(
        &self,
        trace: &loopflow_model::id::TraceId,
    ) -> StoreResult<Option<String>> {
        let conn = self.conn.lock().expect("store mutex poisoned");
        Ok(conn
            .query_row(
                "SELECT journal_trace_locator FROM processes
                 WHERE trace_id=?1 ORDER BY started_at,id LIMIT 1",
                [trace],
                |row| row.get(0),
            )
            .optional()?)
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

    /// Exact identity wins; otherwise require a unique literal UUID prefix.
    pub fn resolve_process(&self, selector: &str) -> StoreResult<Option<ProcessEntry>> {
        let canonical = selector.to_ascii_lowercase();
        let selector = canonical.as_str();
        let conn = self.conn.lock().expect("store mutex poisoned");
        if let Some(process) = conn
            .query_row(
                &format!("{PROCESS_SELECT} WHERE e.id=?1"),
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
        let mut query =
            conn.prepare("SELECT id FROM processes WHERE id>=?1 AND id<?2 ORDER BY id LIMIT 2")?;
        let ids = query
            .query_map(params![selector, upper], |row| row.get::<_, String>(0))?
            .collect::<rusqlite::Result<Vec<_>>>()?;
        match ids.as_slice() {
            [] => Ok(None),
            [id] => Ok(Some(conn.query_row(
                &format!("{PROCESS_SELECT} WHERE e.id=?1"),
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
                started_at: process.started_at(),
                id: process.id().to_owned(),
            })
        } else {
            None
        };
        Ok(ProcessPage { entries, next })
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
        start: Option<&LfProcessStart>,
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
        start: Option<&LfProcessStart>,
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

    use std::num::NonZeroU32;

    use rusqlite::params;

    use crate::process::{ProcessEntry, ProcessFilter, ProcessOutcomeFilter, ProcessWorkFilter};
    use crate::StoreError;
    use loopflow_model::durable::{ProjectId, TaskId};
    use loopflow_model::id::{LfProcessId, WaveId};

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

    #[test]
    fn process_discovery_retains_command_evidence_without_payloads() {
        let dir = tempfile::tempdir().unwrap();
        let store = crate::test_fixture::open(&dir.path().join("store.db")).unwrap();
        let parent = insert_process(&store, 1, 1);
        let direct = insert_process(&store, 2, 2);
        let agent = insert_process(&store, 3, 3);
        let interrupted = insert_process(&store, 4, 4);
        let issuer = loopflow_model::id::AgentProcessId::new();
        {
            let conn = store.conn.lock().unwrap();
            conn.execute(
                "UPDATE processes SET parent_lf_process_id=?2,via_agent=0,outcome='failed',
                completed_at=5,exit_code=42 WHERE id=?1",
                params![direct, parent],
            )
            .unwrap();
            conn.execute("UPDATE processes SET parent_lf_process_id=?2,via_agent=1,caller_session_id='retained-caller',
                caller_agent_process_id=?3,outcome='succeeded',
                completed_at=6,exit_code=0 WHERE id=?1", params![agent,direct,issuer]).unwrap();
            conn.execute(
                "UPDATE processes SET outcome='interrupted',completed_at=7,exit_code=130
                WHERE id=?1",
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
        let entries = page
            .entries
            .iter()
            .map(|entry| entry.as_lf().unwrap())
            .collect::<Vec<_>>();
        assert_eq!(
            entries.iter().map(|e| &e.id).collect::<Vec<_>>(),
            vec![&interrupted, &agent, &direct, &parent]
        );
        assert_eq!(page.next, None);
        for entry in &entries {
            assert_eq!(store.lf_process(&entry.id).unwrap().as_ref(), Some(*entry));
        }
        let old = entries[3];
        assert_eq!(
            (old.via_agent, old.completed_at, old.exit_code),
            (None, None, None)
        );
        assert_eq!(old.outcome, None);
        assert_eq!(old.signal, None);
        let failed = entries[2];
        assert_eq!(failed.parent_lf_process_id.as_ref(), Some(&parent));
        assert_eq!(failed.via_agent, Some(false));
        assert_eq!(failed.exit_code, Some(42));
        let success = entries[1];
        assert_eq!(
            success.caller_session_id.as_deref(),
            Some("retained-caller")
        );
        assert_eq!(success.caller_agent_process_id.as_ref(), Some(&issuer));
        assert_eq!(success.via_agent, Some(true));
        assert_eq!(entries[0].signal, None);
        let issued = store
            .processes(
                &ProcessFilter {
                    caller_agent_process_id: Some(issuer.clone()),
                    ..Default::default()
                },
                None,
                NonZeroU32::new(10).unwrap(),
            )
            .unwrap();
        assert_eq!(
            issued
                .entries
                .iter()
                .map(ProcessEntry::id)
                .collect::<Vec<_>>(),
            vec![agent.as_str()]
        );
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
            assert_eq!(selected.entries[0].id(), expected.as_str());
            assert_eq!(selected.next, None);
        }
        assert!(!dir.path().join("runs").exists());
    }

    #[test]
    fn process_discovery_separates_literal_search_from_identity_resolution() {
        let dir = tempfile::tempdir().unwrap();
        let store = crate::test_fixture::open(&dir.path().join("store.db")).unwrap();
        let first = insert_process(&store, 1, 1);
        let second = insert_process(&store, 16, 2);
        store
            .conn
            .lock()
            .unwrap()
            .execute(
                "UPDATE processes SET command=?3,parent_lf_process_id=?2,
                caller_session_id='caller' WHERE id=?1",
                params![
                    first,
                    second,
                    serde_json::to_string(&["lf", "pr", "land", "--strict", "%_ bytes"]).unwrap()
                ],
            )
            .unwrap();
        let filter = ProcessFilter {
            repo: Some("/repo".into()),
            parent_lf_process_id: Some(second.clone()),
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
                .id(),
            first.as_str()
        );
        assert_eq!(
            store.resolve_process(first.as_str()).unwrap().unwrap().id(),
            first.as_str()
        );
        assert_eq!(
            store
                .resolve_process(&second.as_str()[..35])
                .unwrap()
                .unwrap()
                .id(),
            second.as_str()
        );
        assert!(matches!(
            store.resolve_process("00000000"),
            Err(StoreError::InvalidData(_))
        ));
        assert_eq!(store.resolve_process("%_").unwrap(), None);
        assert_eq!(store.lf_process(&LfProcessId::new()).unwrap(), None);
        let miss = ProcessFilter {
            command_contains: Some("%_".into()),
            id: Some(second.to_string()),
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
                    "UPDATE processes SET command=?2 WHERE id=?1",
                    params![first, command],
                )
                .unwrap();
            let page = store
                .processes(
                    &ProcessFilter {
                        id: Some(first.to_string()),
                        command_contains: Some("pr land".into()),
                        ..Default::default()
                    },
                    None,
                    NonZeroU32::new(1).unwrap(),
                )
                .unwrap();
            assert_eq!(page.entries.len(), usize::from(command.is_some()));
            assert_eq!(
                store
                    .lf_process(&first)
                    .unwrap()
                    .unwrap()
                    .command
                    .as_deref(),
                command
            );
        }
    }

    #[test]
    fn process_discovery_pages_fixed_data_after_filtering_with_timestamp_ties() {
        let dir = tempfile::tempdir().unwrap();
        let store = crate::test_fixture::open(&dir.path().join("store.db")).unwrap();
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
                    .execute("UPDATE processes SET repo='/other' WHERE id=?1", [id])
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
            found.extend(page.entries.iter().map(|entry| entry.id().to_owned()));
            after = page.next;
            if after.is_none() {
                break;
            }
            assert!(found.len() <= expected.len(), "cursor must make progress");
        }
        assert_eq!(
            found,
            expected
                .into_iter()
                .map(|(_, id)| id.to_string())
                .collect::<Vec<_>>()
        );
    }

    #[test]
    fn process_discovery_deduplicates_performed_work_without_rebinding_history() {
        let dir = tempfile::tempdir().unwrap();
        let store = crate::test_fixture::open(&dir.path().join("store.db")).unwrap();
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
                conn.execute("INSERT INTO repository_plans(id,repo,selected) SELECT 'repo_'||lower(hex(randomblob(16))),w.repo,1 FROM waves w WHERE NOT EXISTS(SELECT 1 FROM repository_plans r WHERE r.repo=w.repo AND r.selected=1) GROUP BY w.repo", []).unwrap();
                conn.execute(
                    "INSERT INTO tasks(repository_id,id,project_id,external_issue_id,issue_identifier,created_at)
                    VALUES((SELECT r.id FROM projects p JOIN waves w ON w.id=p.wave_id JOIN repository_plans r ON r.repo=w.repo AND r.selected=1 WHERE p.id=?2),?1,?2,?3,?3,1)",
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
                store.execute_history("INSERT INTO turns(session_id,provider_thread,provider_turn,lf_process_id,task_id,wave_id,started_at)
                    VALUES(?1,'thread','work',?2,?3,?4,1)",
                    params![format!("session-{index}"),shared,task.as_str(),wave]).unwrap();
            }
            // Same Process drove both Tasks, two turns and a mechanical boundary.
            store.execute_history("INSERT INTO turns(session_id,provider_thread,provider_turn,lf_process_id,task_id,wave_id,started_at)
                VALUES('session-0','thread','retry',?1,?2,?3,2)",params![shared,first.as_str(),wave]).unwrap();
            // Session is bound now; the earlier turn remains unassigned.
            store.execute_history("INSERT INTO turns(session_id,provider_thread,provider_turn,lf_process_id,started_at)
                VALUES('session-0','thread','before-bind',?1,1)",[&unbound]).unwrap();
            // Known Task, unknown original process: neither current attachment nor observer is its owner.
            store.execute_history("INSERT INTO turns(session_id,provider_thread,provider_turn,task_id,wave_id,started_at)
                VALUES('session-0','thread','unmapped',?1,?2,1)",params![first.as_str(),wave]).unwrap();

            conn.execute(
                "UPDATE processes SET caller_session_id='session-0',via_agent=1 WHERE id=?1",
                [&observer],
            )
            .unwrap();
            // A Graph process recorded both Processes as steps in the first Task's checkout.
            conn.execute(
                "INSERT INTO task_checkouts(task_id,worktree) VALUES(?1,'/repo.first') ON CONFLICT(task_id) DO UPDATE SET worktree=excluded.worktree",
                [first.as_str()],
            )
            .unwrap();
            conn.execute(
                "INSERT INTO graph_processes(lf_process_id,graph,compiled) VALUES(?1,'proof','{}')",
                [&observer],
            )
            .unwrap();
            for process in [&shared, &mechanical] {
                conn.execute(
                    "UPDATE processes SET cwd='/repo.first',parent_lf_process_id=?2 WHERE id=?1",
                    params![process, observer],
                )
                .unwrap();
                conn.execute(
                    "INSERT INTO graph_process_steps(graph_lf_process_id,lf_process_id,node,iterations) VALUES(?2,?1,0,'[]')",
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
            page.entries
                .iter()
                .map(ProcessEntry::id)
                .collect::<Vec<_>>(),
            vec![shared.as_str()]
        );
        let page = store
            .processes(&filter, page.next.as_ref(), NonZeroU32::new(1).unwrap())
            .unwrap();
        assert_eq!(page.entries[0].id(), mechanical.as_str());
        assert_eq!(page.next, None);
        let second_filter = ProcessFilter {
            performed_work: Some(ProcessWorkFilter::Task(second)),
            ..Default::default()
        };
        let page = store
            .processes(&second_filter, None, NonZeroU32::new(10).unwrap())
            .unwrap();
        assert_eq!(
            page.entries
                .iter()
                .map(ProcessEntry::id)
                .collect::<Vec<_>>(),
            vec![shared.as_str()]
        );
        let wave_filter = ProcessFilter {
            performed_work: Some(ProcessWorkFilter::Wave(wave)),
            ..Default::default()
        };
        let page = store
            .processes(&wave_filter, None, NonZeroU32::new(10).unwrap())
            .unwrap();
        assert_eq!(
            page.entries
                .iter()
                .map(ProcessEntry::id)
                .collect::<Vec<_>>(),
            vec![shared.as_str(), mechanical.as_str()]
        );
        assert_eq!(store.lf_process(&unbound).unwrap().unwrap().outcome, None);
        assert_eq!(store.lf_process(&observer).unwrap().unwrap().outcome, None);
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
    fn attaching_start_receipt_and_terminal_commit_only_with_the_agent() {
        let home = tempfile::tempdir().unwrap();
        let store = crate::test_fixture::open(&home.path().join("store.db")).unwrap();
        store.test_session("conversation", &crate::session_record::new_artifact_key());
        let seed = insert_process(&store, 1, 1);
        let mut process = store.lf_process(&seed).unwrap().unwrap();
        process.id = LfProcessId::new();
        let terminal = loopflow_model::id::TerminalId::new();
        store.create_terminal(&terminal, "fixture").unwrap();
        let start = crate::process::LfProcessStart {
            process,
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
                &start.process.id,
                Some(&start),
                &SessionCaller {
                    agent: None,
                    capture_key: None
                }
            )
            .is_err());
        assert_eq!(store.lf_process(&start.process.id).unwrap(), None);
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
                &start.process.id,
                Some(&start),
                &SessionCaller {
                    agent: None,
                    capture_key: None,
                },
            )
            .unwrap();
        assert_eq!(
            store.lf_process(&start.process.id).unwrap(),
            Some(start.process.clone())
        );
        let agent = store
            .agent_process(&attached.agent_process_id)
            .unwrap()
            .unwrap();
        assert_eq!(agent.parent_lf_process_id.as_ref(), Some(&start.process.id));
        assert_eq!(agent.trace_id, start.process.trace_id);
        let placement: loopflow_model::id::TerminalId = store
            .conn
            .lock()
            .unwrap()
            .query_row(
                "SELECT held_terminal_id FROM processes WHERE id=?1",
                [&start.process.id],
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

#[cfg(test)]
mod forget_tests {
    use rusqlite::params;

    use loopflow_model::id::{LfProcessId, TraceId};

    /// Forgetting a Process a row still names either fails the delete or, on a
    /// connection that does not enforce foreign keys, leaves a reference no
    /// migration can pass. The schema says which columns can name one.
    #[test]
    fn every_column_that_names_an_lf_process_keeps_it_from_being_forgotten() {
        let home = tempfile::tempdir().unwrap();
        let store = crate::test_fixture::open(&home.path().join("store.db")).unwrap();
        let conn = store.conn.lock().unwrap();
        let mut statement = conn
            .prepare(
                "SELECT m.name, f.\"from\" FROM sqlite_master m, pragma_foreign_key_list(m.name) f
                 WHERE m.type='table' AND f.\"table\"='processes' AND f.on_delete!='CASCADE'",
            )
            .unwrap();
        let references: Vec<(String, String)> = statement
            .query_map([], |row| Ok((row.get(0)?, row.get(1)?)))
            .unwrap()
            .collect::<Result<_, _>>()
            .unwrap();
        assert!(!references.is_empty());
        let unguarded: Vec<_> = references
            .into_iter()
            // Agent rows are never forgotten.
            .filter(|(_, column)| column != "agent_process_id")
            .filter(|(table, column)| {
                !super::FORGETTABLE.contains(&format!("FROM {table} WHERE {column}=p.id"))
            })
            .collect();
        assert_eq!(unguarded, Vec::<(String, String)>::new());
    }

    #[test]
    fn an_exit_forgets_the_process_unless_something_still_needs_it() {
        let home = tempfile::tempdir().unwrap();
        let store = crate::test_fixture::open(&home.path().join("store.db")).unwrap();
        let session = store.test_session("session", &crate::session_record::new_artifact_key());
        let start = |parent: Option<&LfProcessId>, via_agent: bool| {
            let process = crate::process::LfProcess {
                id: LfProcessId::new(),
                pid: None,
                os_started_at: None,
                trace_id: TraceId::new(),
                parent_lf_process_id: parent.cloned(),
                via_agent: Some(via_agent),
                caller_session_id: None,
                caller_agent_process_id: None,
                command: None,
                repo: None,
                cwd: None,
                started_at: 1,
                completed_at: None,
                outcome: None,
                exit_code: None,
                signal: None,
                error: None,
            };
            store.record_process(&process).unwrap();
            process
        };
        let finish = |process: &crate::process::LfProcess| {
            store
                .record_process(&crate::process::LfProcess {
                    completed_at: Some(2),
                    outcome: Some("succeeded".into()),
                    ..process.clone()
                })
                .unwrap();
        };
        let kept = |process: &crate::process::LfProcess| {
            store.process_is_recorded(process.id.as_str()).unwrap()
        };

        let human = start(None, false);
        let agent_issued = start(None, true);
        let graph = start(None, false);
        let step = start(Some(&graph.id), false);
        let narrated = start(None, false);
        let holder = start(None, false);
        let running_child = start(Some(&holder.id), false);
        let shipper = start(None, false);
        let task = loopflow_model::durable::TaskId::new();
        store
            .create_task(&crate::work::task::NewTask {
                repository_id: crate::test_fixture::repository(&store, "/repo"),
                id: task.clone(),
                project_id: None,
                title: "Shipped Task".into(),
                description: "Brief".into(),
                due_date: None,
            })
            .unwrap();
        store
            .ship_task(
                &task,
                (
                    &shipper.id,
                    &crate::graph_process::ExecutionStart::reserved(1),
                ),
                "proof",
            )
            .unwrap();
        {
            let conn = store.conn.lock().unwrap();
            conn.execute(
                "INSERT INTO session_events(session_id,kind,receipt_key,observed_at,payload,lf_process_id)
                 VALUES(?1,'observed','narrated',1,'{}',?2)",
                params![session.id, narrated.id],
            )
            .unwrap();
            conn.execute(
                "INSERT INTO graph_processes(lf_process_id,graph,compiled) VALUES(?1,'g','{}')",
                [&graph.id],
            )
            .unwrap();
        }
        for process in [&human, &agent_issued, &step, &narrated, &holder, &shipper] {
            finish(process);
        }
        assert!(kept(&shipper), "the Task names its ship process");
        assert!(!kept(&human));
        assert!(kept(&agent_issued));
        assert!(kept(&narrated));
        // A running Graph may still read its step's result.
        assert!(kept(&step));
        assert!(kept(&holder), "a running child names it");
        store
            .conn
            .lock()
            .unwrap()
            .execute(
                "INSERT INTO graph_process_steps(graph_lf_process_id,lf_process_id,node,iterations)
                 VALUES(?1,?2,0,'{}')",
                [&graph.id, &step.id],
            )
            .unwrap();

        finish(&running_child);
        assert!(!kept(&running_child), "its parent had already finished");
        finish(&graph);
        assert!(kept(&graph) && kept(&step), "the Graph records its step");
        // Without a step record, finishing the parent forgets its finished child.
        let parent = start(None, false);
        let child = start(Some(&parent.id), false);
        finish(&child);
        assert!(kept(&child));
        finish(&parent);
        assert!(!kept(&child) && !kept(&parent));
    }
}
