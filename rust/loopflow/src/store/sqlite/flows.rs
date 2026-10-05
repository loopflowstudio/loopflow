//! Every Flow invocation's driver transactions: keyed by invocation id and
//! fenced by `position_version`. Navigation consumes the selected native
//! turn's successful completion; mechanical results belong to Flow history.
//! AgentSession owns launch publication and provider outcomes. A Task's Flows
//! are every invocation naming it; none is selected over another.

use rusqlite::{params, Connection, OptionalExtension, Transaction, TransactionBehavior};
use time::OffsetDateTime;

use crate::durable::{FlowAttempt, FlowSession, FlowTurnSelection, TaskFlowBlocker, TaskId};
use crate::engine::invocation::QueuedInvocation;
use crate::engine::{ConcreteStep, ExecutionCursor};
use crate::id::WaveId;

use crate::store::rows::now_unix;
use crate::store::{StoreError, StoreResult};
use crate::work::task::TaskEventKind;

use super::SqliteStore;

const FLOW_SELECT: &str = "SELECT f.invocation_json, f.review_json, f.step_index, f.iteration,
    f.updated_at, f.position_version, f.task_id, f.wave_id, COALESCE(f.cwd, t.worktree),
    f.message, f.model, f.current_capture,
    (SELECT input_published FROM agent_sessions WHERE current_capture=f.current_capture),
    (SELECT json_extract(done.payload,'$.status') FROM session_events start
        JOIN session_events done ON done.session_id=start.session_id
            AND done.provider_thread=start.provider_thread AND done.provider_turn=start.provider_turn
            AND done.kind='completed' WHERE start.seq=f.selected_start),
    f.pending_session_id,
    f.failure_json, f.state, (SELECT receipt_key FROM session_events WHERE seq=f.current_capture)
    FROM flow_sessions f LEFT JOIN tasks t ON t.id=f.task_id";

// Keep the expression identical to index_session_metadata. SQLite reads the
// indexed scalar; no invocation, cursor or selected-history body is read.
pub(super) const FLOW_METADATA_COLUMNS: &str = "f.id,
    CASE WHEN json_valid(f.invocation_json) THEN CASE WHEN json_type(f.invocation_json,'$.flow')='text' THEN json_extract(f.invocation_json,'$.flow') END END AS name,
    f.state,f.current_capture,f.pending_session_id,f.task_id,f.wave_id,f.updated_at";

pub(super) fn read_flow_summary(
    row: &rusqlite::Row<'_>,
    offset: usize,
) -> StoreResult<Option<crate::session::FlowSummary>> {
    use crate::session::{FlowSummary, FlowSummaryState};
    let Some(id) = row.get::<_, Option<String>>(offset)? else {
        return Ok(None);
    };
    Ok(Some(FlowSummary {
        id,
        name: row.get(offset + 1)?,
        state: match row.get::<_, String>(offset + 2)?.as_str() {
            "current" => FlowSummaryState::Current,
            "completed" => FlowSummaryState::Completed,
            "replaced" => FlowSummaryState::Replaced,
            other => return Err(invalid(format!("unknown Flow state {other}"))),
        },
        current_capture: row.get(offset + 3)?,
        pending_session: row.get(offset + 4)?,
        task_id: row
            .get::<_, Option<String>>(offset + 5)?
            .map(|id| TaskId::parse(&id))
            .transpose()
            .map_err(invalid)?,
        wave_id: row
            .get::<_, Option<String>>(offset + 6)?
            .map(|id| WaveId::parse(&id))
            .transpose()
            .map_err(invalid)?,
        updated_at: row.get(offset + 7)?,
    }))
}

fn invalid(error: impl std::fmt::Display) -> StoreError {
    StoreError::InvalidData(error.to_string())
}

fn operation_in(conn: &Connection, id: &str) -> StoreResult<Option<(i64, Option<String>)>> {
    Ok(conn
        .query_row(
            "SELECT f.operation_start,done.outcome FROM flow_sessions f
         LEFT JOIN flow_events done ON done.operation_start=f.operation_start
         WHERE f.id=?1 AND f.operation_start IS NOT NULL",
            [id],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .optional()?)
}

fn stale(id: &str) -> StoreError {
    StoreError::InvalidAuthority(format!("Flow {id} changed under its driver"))
}

fn read_flow(row: &rusqlite::Row<'_>) -> rusqlite::Result<StoreResult<FlowSession>> {
    let invocation_json: String = row.get(0)?;
    let review_json: Option<String> = row.get(1)?;
    let step_index: i64 = row.get(2)?;
    let iteration: i64 = row.get(3)?;
    let updated_at: i64 = row.get(4)?;
    let version: i64 = row.get(5)?;
    let task_id: Option<String> = row.get(6)?;
    let wave_id: Option<String> = row.get(7)?;
    let cwd: Option<String> = row.get(8)?;
    let message: Option<String> = row.get(9)?;
    let model: Option<String> = row.get(10)?;
    let current_capture: Option<i64> = row.get(11)?;
    let published: Option<bool> = row.get(12)?;
    let outcome: Option<String> = row.get(13)?;
    let pending_session_id: Option<String> = row.get(14)?;
    let failure_json: Option<String> = row.get(15)?;
    let state: String = row.get(16)?;
    let decode = || -> StoreResult<FlowSession> {
        let invocation: QueuedInvocation = serde_json::from_str(&invocation_json)?;
        let updated_at = OffsetDateTime::from_unix_timestamp(updated_at).map_err(invalid)?;
        let failure = failure_json
            .map(|failure| serde_json::from_str::<TaskFlowBlocker>(&failure))
            .transpose()?;
        let cursor = decode_flow_cursor(review_json.as_deref(), step_index, iteration)?;
        let cwd = cwd.ok_or_else(|| {
            invalid(format!(
                "Flow {} has no launch record on its row",
                invocation.id
            ))
        })?;
        Ok(FlowSession {
            cursor,
            version: u64::try_from(version).map_err(invalid)?,
            task_id: task_id
                .map(|id| TaskId::parse(&id))
                .transpose()
                .map_err(invalid)?,
            wave_id: wave_id
                .map(|id| WaveId::parse(&id))
                .transpose()
                .map_err(invalid)?,
            cwd: cwd.into(),
            message,
            model,
            current_attempt: current_capture
                .map(|captured| {
                    Ok::<_, StoreError>(FlowAttempt {
                        captured,
                        run_id: row.get(17)?,
                        published: published.unwrap_or(false),
                        outcome,
                    })
                })
                .transpose()?,
            pending_session_id,
            failure,
            finished: state != "current",
            updated_at,
            invocation,
        })
    };
    Ok(decode().map_err(|error| {
        StoreError::InvalidData(format!(
            "saved Invocation is unreadable; its bytes are unchanged: {error}"
        ))
    }))
}

pub(super) fn flow_in(conn: &Connection, id: &str) -> StoreResult<Option<FlowSession>> {
    conn.query_row(&format!("{FLOW_SELECT} WHERE f.id=?1"), [id], read_flow)
        .optional()?
        .transpose()?
        .map(|flow| project_output(conn, flow))
        .transpose()
}

fn project_output(conn: &Connection, mut flow: FlowSession) -> StoreResult<FlowSession> {
    if !flow.finished
        && flow
            .current_step()
            .and_then(crate::engine::flow_output::FlowOutput::for_step)
            .is_some()
    {
        clear_candidate(&mut flow.cursor);
    }
    if !flow.finished
        && flow
            .current_attempt
            .as_ref()
            .is_some_and(FlowAttempt::completed)
    {
        match selected_output_in(conn, &flow)? {
            Ok(crate::engine::SkillOutcome::Decided(verdict)) => {
                flow.cursor.leaf_mut().progress.verdict = Some(verdict)
            }
            Ok(crate::engine::SkillOutcome::Routed(path)) => {
                flow.cursor.leaf_mut().route = Some(path)
            }
            _ => {}
        }
    }
    Ok(flow)
}

/// The Task's most recently launched Flow: what its status describes. Reading
/// it selects nothing; every invocation naming the Task is equally its work.
pub(super) fn latest_task_flow_in(
    conn: &Connection,
    task_id: &TaskId,
) -> StoreResult<Option<FlowSession>> {
    conn.query_row(
        &format!("{FLOW_SELECT} WHERE f.task_id=?1 ORDER BY f.rowid DESC LIMIT 1"),
        [task_id.as_str()],
        read_flow,
    )
    .optional()?
    .transpose()
    .map_err(|error| match error {
        StoreError::InvalidData(detail) => {
            StoreError::InvalidData(format!("Task {task_id} {detail}"))
        }
        other => other,
    })?
    .map(|flow| project_output(conn, flow))
    .transpose()
}

fn current_flow_in(conn: &Connection, id: &str) -> StoreResult<FlowSession> {
    let flow = flow_in(conn, id)?.ok_or(StoreError::NotFound)?;
    if flow.finished {
        return Err(StoreError::InvalidAuthority(format!(
            "Flow {id} is already finished"
        )));
    }
    Ok(flow)
}

/// A Flow naming a Task reports its progress, failures and completion to it.
fn report_to_task_in(conn: &Connection, id: &str, kind: &TaskEventKind) -> StoreResult<()> {
    let task: Option<String> = conn
        .query_row(
            "SELECT task_id FROM flow_sessions WHERE id=?1",
            [id],
            |row| row.get(0),
        )
        .optional()?
        .flatten();
    if let Some(task) = task {
        super::children::insert_task_event_in(conn, &TaskId::parse(&task).map_err(invalid)?, kind)?;
    }
    Ok(())
}

fn validate_flow(flow: &FlowSession) -> StoreResult<()> {
    crate::engine::flow::validate_repeats(&flow.invocation.steps).map_err(invalid)?;
    let step = flow
        .current_checked()
        .ok_or_else(|| invalid("Flow position has no current step"))?;
    if step.flow.trim().is_empty() || step.step.trim().is_empty() {
        return Err(invalid("flow and step cannot be empty"));
    }
    if step.human && step.id.is_none() {
        return Err(invalid("review flow positions require a stable node id"));
    }
    Ok(())
}

/// Store a launched Flow at its first step. A Task invocation runs in the
/// Task worktree and stores no cwd of its own; its Wave is the Task's. The
/// import re-registers a Flow it already stored; the row it finds wins.
pub(super) fn insert_flow_in(
    conn: &Connection,
    flow: &FlowSession,
    unbound_repo: Option<&str>,
) -> StoreResult<()> {
    validate_flow(flow)?;
    conn.execute(
        "INSERT INTO flow_sessions(id, task_id, wave_id, cwd, message, model, invocation_json,
            step_index, iteration, position_version, failure_json,
            updated_at, review_json, state, unbound_repo)
         VALUES(?1,?2,COALESCE(?3,(SELECT p.wave_id FROM tasks t JOIN projects p ON p.id=t.project_id
            WHERE t.id=?2)),CASE WHEN ?2 IS NULL THEN ?4 END,?5,?6,?7,?8,?9,1,?10,?11,?12,'current',?13)
         ON CONFLICT(id) DO NOTHING",
        params![
            flow.invocation.id,
            flow.task_id.as_ref().map(TaskId::as_str),
            flow.wave_id.as_ref().map(WaveId::as_str),
            flow.cwd.to_string_lossy(),
            flow.message,
            flow.model,
            serde_json::to_string(&flow.invocation)?,
            i64::try_from(flow.cursor.index).map_err(invalid)?,
            i64::from(flow.cursor.iteration),
            flow.failure
                .as_ref()
                .map(serde_json::to_string)
                .transpose()?,
            flow.updated_at.unix_timestamp(),
            serde_json::to_string(&flow.cursor)?,
            unbound_repo,
        ],
    )?;
    Ok(())
}

/// The one cursor write, fenced by the version. It advances the version and
/// clears the attempt and pending review when the position moved.
fn write_cursor_in(
    conn: &Connection,
    (id, version): (&str, u64),
    cursor: &ExecutionCursor,
    failure: Option<&TaskFlowBlocker>,
    clear_attempt: bool,
) -> StoreResult<u64> {
    let changed = conn.execute(
        "UPDATE flow_sessions SET review_json=?3, step_index=?4, iteration=?5,
            failure_json=?6, updated_at=?7, position_version=position_version+1,
            current_capture=CASE WHEN ?8 THEN NULL ELSE current_capture END,
            pending_session_id=CASE WHEN ?8 THEN NULL ELSE pending_session_id END,
            selected_start=CASE WHEN ?8 THEN NULL ELSE selected_start END,
            operation_start=CASE WHEN ?8 THEN NULL ELSE operation_start END
         WHERE id=?1 AND position_version=?2 AND state='current'",
        params![
            id,
            i64::try_from(version).map_err(invalid)?,
            serde_json::to_string(cursor)?,
            i64::try_from(cursor.index).map_err(invalid)?,
            i64::from(cursor.iteration),
            failure.map(serde_json::to_string).transpose()?,
            now_unix(),
            clear_attempt,
        ],
    )?;
    if changed != 1 {
        return Err(stale(id));
    }
    Ok(version + 1)
}

/// Feedback and a candidate result do not move the selected boundary.
fn position_of(cursor: &ExecutionCursor) -> ExecutionCursor {
    let mut position = cursor.clone();
    clear_candidate(&mut position);
    position.leaf_mut().progress.direction = None;
    position
}

fn clear_candidate(cursor: &mut ExecutionCursor) {
    let leaf = cursor.leaf_mut();
    leaf.progress.verdict = None;
    leaf.route = None;
}

/// Block the Flow at its position with `failure`; the failed attempt's Run is
/// named on it. A Task's Flow records the failure on the Task.
fn fail_flow_in(
    tx: &Transaction<'_>,
    flow: &FlowSession,
    version: u64,
    failure: &TaskFlowBlocker,
) -> StoreResult<()> {
    let mut failure = failure.clone();
    if failure.captured.is_none() {
        failure.captured = flow
            .current_attempt
            .as_ref()
            .filter(|attempt| attempt.published)
            .map(|attempt| attempt.captured);
    }
    let mut cursor = flow.cursor.clone();
    clear_candidate(&mut cursor);
    write_cursor_in(tx, (flow.id(), version), &cursor, Some(&failure), false)?;
    report_to_task_in(
        tx,
        flow.id(),
        &TaskEventKind::Failed {
            error: failure.reason.clone(),
            resumable: true,
        },
    )
}

/// End the Flow. A Task's Flow records `FlowFinished` with the last step's
/// summary.
fn end_flow_in(tx: &Transaction<'_>, id: &str, version: u64, summary: &str) -> StoreResult<()> {
    let flow = current_flow_in(tx, id)?;
    if flow.version != version {
        return Err(stale(id));
    }
    consume_selected_in(tx, &flow)?;
    if tx.execute(
        "UPDATE flow_sessions SET state='completed', ended_at=?2
         WHERE id=?1 AND state='current'",
        params![id, now_unix()],
    )? != 1
    {
        return Err(stale(id));
    }
    report_to_task_in(
        tx,
        id,
        &TaskEventKind::FlowFinished {
            invocation_id: flow.invocation.id.clone(),
            flow: flow.invocation.flow.clone(),
            summary: summary.to_string(),
        },
    )
}

/// Store the step's Run before anything launches it. An attempt already at
/// this position is kept: a reservation the launcher has not published, or a
/// completed candidate the driver is about to settle.
fn reserve_attempt_in(
    tx: &Transaction<'_>,
    flow: &FlowSession,
    exec: Option<&crate::id::ExecId>,
) -> StoreResult<()> {
    if let (Some(attempt), Some(exec)) = (&flow.current_attempt, exec) {
        let owner: Option<String> = tx.query_row(
            "SELECT exec_id FROM session_events WHERE seq=?1",
            [attempt.captured],
            |row| row.get(0),
        )?;
        if owner.as_deref() != Some(exec.as_str()) {
            return Err(invalid(
                "Flow capture belongs to another or unknown step Exec",
            ));
        }
    }
    if flow.current_attempt.is_some()
        || matches!(flow.current_step(), Some(ConcreteStep::Command(_)))
    {
        return Ok(());
    }
    if flow.is_human() {
        return Err(invalid("Human review belongs in the Task conversation"));
    }
    let (node, iterations) = flow.invocation.location(&flow.cursor).map_err(invalid)?;
    let existing: Option<String> = tx.query_row(
        "SELECT id FROM agent_sessions WHERE flow_session_id=?1 AND node=?2 AND iterations=?3 AND kind='conversation'",
        params![flow.id(),node,serde_json::to_string(&iterations)?],|row| row.get(0)).optional()?;
    let session = if let Some(id) = existing {
        let mut session = super::sessions::session_in(tx, &id)?.ok_or(StoreError::NotFound)?;
        session.artifact_key = crate::session_record::new_artifact_key();
        session.input_published = false;
        let exec = exec.cloned().or_else(crate::journal::current_exec_id);
        super::sessions::replace_input_in(tx, &mut session, exec.as_ref())?;
        session
    } else {
        super::sessions::reserve_flow_conversation_in(
            tx,
            flow,
            format!("session_{}", uuid::Uuid::new_v4().simple()),
            crate::session::SessionKind::Conversation,
            flow.current().step,
            exec,
        )?
    };
    super::sessions::select_input_in(tx, flow, &session)
}

fn selected_output_in(
    conn: &Connection,
    flow: &FlowSession,
) -> StoreResult<Result<crate::engine::SkillOutcome, String>> {
    let Some(output) = flow
        .current_step()
        .and_then(crate::engine::flow_output::FlowOutput::for_step)
    else {
        return Ok(Ok(crate::engine::SkillOutcome::Completed));
    };
    let receipt: Option<(String, Option<String>)> = conn.query_row(
        "SELECT done.payload,output.payload FROM flow_sessions f
         JOIN session_events start ON start.seq=f.selected_start
         JOIN session_events done ON done.session_id=start.session_id
            AND done.provider_thread=start.provider_thread AND done.provider_turn=start.provider_turn
            AND done.kind='completed'
         LEFT JOIN session_events output ON output.session_id=start.session_id
            AND output.provider_thread=start.provider_thread AND output.provider_turn=start.provider_turn
            AND output.kind='output'
         WHERE f.id=?1", [flow.id()], |row| Ok((row.get(0)?,row.get(1)?))).optional()?;
    let Some((completion, payload)) = receipt else {
        return Err(StoreError::InvalidAuthority(
            "selected native completion is absent".into(),
        ));
    };
    if serde_json::from_str::<serde_json::Value>(&completion)?["status"] != "completed" {
        return Err(StoreError::InvalidAuthority(
            "selected native turn did not succeed".into(),
        ));
    }
    Ok(match payload {
        Some(payload) => output.decode_receipt(&serde_json::from_str(&payload)?),
        None => Err("final output is absent".into()),
    })
}

/// The Flow retains the exact successful native receipt in the same transaction
/// that moves its cursor. A different turn cannot satisfy this selection.
fn consume_selected_in(tx: &Transaction<'_>, flow: &FlowSession) -> StoreResult<()> {
    if let Some((_, outcome)) = operation_in(tx, flow.id())? {
        if outcome.as_deref() != Some("completed") {
            return Err(StoreError::InvalidAuthority(
                "operation has no successful completion; inspect its effect before retrying".into(),
            ));
        }
    }
    match selected_output_in(tx, flow)?.map_err(invalid)? {
        crate::engine::SkillOutcome::Decided(verdict)
            if flow.cursor.leaf().progress.verdict.as_ref() != Some(&verdict) =>
        {
            return Err(stale(flow.id()))
        }
        crate::engine::SkillOutcome::Routed(path)
            if flow.cursor.leaf().route.as_ref() != Some(&path) =>
        {
            return Err(stale(flow.id()))
        }
        _ => {}
    }
    let selected: Option<i64> = tx.query_row(
        "SELECT selected_start FROM flow_sessions WHERE id=?1",
        [flow.id()],
        |row| row.get(0),
    )?;
    let Some(start) = selected else { return Ok(()) };
    let completion: Option<i64> = tx
        .query_row(
            "SELECT done.seq FROM session_events origin JOIN session_events done
         ON done.session_id=origin.session_id AND done.provider_thread=origin.provider_thread
            AND done.provider_turn=origin.provider_turn AND done.kind='completed'
         WHERE origin.seq=?1 AND json_extract(done.payload,'$.status')='completed'",
            [start],
            |row| row.get(0),
        )
        .optional()?;
    let completion = completion.ok_or_else(|| {
        StoreError::InvalidAuthority(format!(
            "Flow {} is waiting for its selected native completion",
            flow.id()
        ))
    })?;
    tx.execute(
        "INSERT INTO flow_events(flow_id,version,node,iterations,kind,session_event,observed_at)
         SELECT flow_id,?3,node,iterations,'consumed',?2,?4 FROM flow_events
         WHERE flow_id=?1 AND kind='selected' AND session_event=?5
         ON CONFLICT(flow_id,kind,session_event) DO NOTHING",
        params![
            flow.id(),
            completion,
            i64::try_from(flow.version).map_err(invalid)?,
            now_unix(),
            start
        ],
    )?;
    Ok(())
}

/// Read mechanical results from Flow history and agent results from the
/// selected native turn (or the transitional Run when no turn was selected).
/// Missing mechanical completion is unknown, never an invented interruption.
fn settle_attempt_in(tx: &Transaction<'_>, flow: FlowSession) -> StoreResult<FlowSession> {
    let id = flow.invocation.id.clone();
    if flow.finished || flow.failure.is_some() || flow.is_human() {
        return Ok(flow);
    }
    if let Some((_, outcome)) = operation_in(tx, flow.id())? {
        if outcome.as_deref() == Some("completed") {
            return Ok(flow);
        }
        let reason = outcome.unwrap_or_else(|| {
            "has no completion receipt; inspect its effect before retrying".into()
        });
        let failure =
            TaskFlowBlocker::now(format!("{} {reason}", flow.step_name().unwrap_or_default()));
        fail_flow_in(tx, &flow, flow.version, &failure)?;
        return current_flow_in(tx, &id);
    }
    let Some(attempt) = &flow.current_attempt else {
        return Ok(flow);
    };
    if !attempt.published {
        return Ok(flow);
    }
    let outcome = match attempt.outcome.as_deref() {
        Some(outcome) => outcome.to_owned(),
        None => {
            return Err(StoreError::InvalidAuthority(format!(
                "Flow {id} is waiting for Run {}; its completion is not recorded",
                attempt.run_id
            )))
        }
    };
    if outcome == "completed" {
        return Ok(flow);
    }
    let mut failure = TaskFlowBlocker::now(format!(
        "{} Run {outcome}",
        flow.step_name().unwrap_or_default()
    ));
    failure.captured = Some(attempt.captured);
    fail_flow_in(tx, &flow, flow.version, &failure)?;
    current_flow_in(tx, &id)
}

impl SqliteStore {
    pub fn create_flow(&self, flow: &FlowSession) -> StoreResult<FlowSession> {
        // Observe new placement before taking SQLite's writer lock.
        let repo = if flow.task_id.is_none() && flow.wave_id.is_none() {
            match crate::repo::discover_repo_root(&flow.cwd).and_then(|root| {
                root.map(|root| {
                    crate::repository::CanonicalRepo::discover(&root)
                        .map(|repo| repo.to_string())
                        .map_err(anyhow::Error::from)
                })
                .transpose()
            }) {
                Ok(repo) => repo,
                Err(error) => {
                    tracing::debug!(%error, "Flow repository observation unavailable");
                    None
                }
            }
        } else {
            None
        };
        let mut conn = self.conn.lock().expect("store mutex poisoned");
        let tx = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
        insert_flow_in(&tx, flow, repo.as_deref())?;
        let stored = flow_in(&tx, &flow.invocation.id)?.ok_or(StoreError::NotFound)?;
        tx.commit()?;
        Ok(stored)
    }

    pub fn flow(&self, id: &str) -> StoreResult<Option<FlowSession>> {
        let conn = self.conn.lock().expect("store mutex poisoned");
        flow_in(&conn, id)
    }

    pub fn latest_task_flow(&self, task_id: &TaskId) -> StoreResult<Option<FlowSession>> {
        let conn = self.conn.lock().expect("store mutex poisoned");
        latest_task_flow_in(&conn, task_id)
    }

    pub(crate) fn flow_turn_selection(&self, run: &str) -> StoreResult<Option<FlowTurnSelection>> {
        let conn = self.conn.lock().expect("store mutex poisoned");
        let launch: Option<(String, String)> = conn
            .query_row(
                "SELECT f.id,s.id FROM flow_sessions f JOIN agent_sessions s ON s.current_capture=f.current_capture
             WHERE s.current_capture=(SELECT seq FROM session_events WHERE kind='captured' AND receipt_key=?1) AND s.input_published=1 AND f.state='current' AND s.kind='conversation'",
                [run],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .optional()?;
        let Some((id, session_id)) = launch else {
            return Ok(None);
        };
        let flow = current_flow_in(&conn, &id)?;
        let after = conn.query_row(
            "SELECT COALESCE(MAX(seq),0) FROM session_events",
            [],
            |row| row.get(0),
        )?;
        Ok(Some(FlowTurnSelection {
            output: flow
                .current_step()
                .and_then(crate::engine::flow_output::FlowOutput::for_step),
            flow_id: id,
            version: flow.version,
            session_id,
            after,
        }))
    }

    pub(crate) fn pending_flow_conversation(&self, id: &str) -> StoreResult<Option<String>> {
        let conn = self.conn.lock().expect("store mutex poisoned");
        Ok(conn.query_row(
            "SELECT start.session_id FROM flow_sessions f JOIN session_events start ON start.seq=f.selected_start
             WHERE f.id=?1 AND f.state='current' AND f.failure_json IS NULL
               AND NOT EXISTS(SELECT 1 FROM session_events done WHERE done.session_id=start.session_id
                   AND done.provider_thread=start.provider_thread AND done.provider_turn=start.provider_turn
                   AND done.kind='completed')",
            [id], |row| row.get(0),
        ).optional()?)
    }

    /// A selected process remains owned until it exits, even after its provider
    /// or mechanical effect has returned.
    pub(crate) fn pending_flow_step_exec(
        &self,
        id: &str,
    ) -> StoreResult<Option<crate::id::ExecId>> {
        let conn = self.conn.lock().expect("store mutex poisoned");
        let exec: Option<String> = conn
            .query_row(
                "SELECT COALESCE(operation.exec_id,captured.exec_id) FROM flow_sessions f
             LEFT JOIN flow_events operation ON operation.seq=f.operation_start
             LEFT JOIN session_events captured ON captured.seq=f.current_capture
             WHERE f.id=?1 AND f.state='current'",
                [id],
                |row| row.get(0),
            )
            .optional()?
            .flatten();
        exec.map(|id| crate::id::ExecId::parse(&id).map_err(invalid))
            .transpose()
    }

    pub(crate) fn flow_operation_completed(&self, id: &str) -> StoreResult<bool> {
        let conn = self.conn.lock().expect("store mutex poisoned");
        Ok(operation_in(&conn, id)?
            .is_some_and(|(_, outcome)| outcome.as_deref() == Some("completed")))
    }

    /// Select a native turn only for the launch authorized at this exact boundary.
    /// A Session observer cannot select a turn; the launching Flow driver carries
    /// its saved version and the reserved Session identity.
    pub(crate) fn select_flow_turn(
        &self,
        selection: &FlowTurnSelection,
        session: &str,
        driver: &crate::exec::SessionDriver,
        start: i64,
    ) -> StoreResult<()> {
        let FlowTurnSelection {
            flow_id: id,
            version,
            session_id,
            after,
            ..
        } = selection;
        let mut conn = self.conn.lock().expect("store mutex poisoned");
        let tx = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
        let flow = current_flow_in(&tx, id)?;
        if session != session_id
            || flow.version != *version
            || flow.failure.is_some()
            || start <= *after
        {
            return Err(stale(id));
        }
        let selected: Option<i64> = tx.query_row(
            "SELECT selected_start FROM flow_sessions WHERE id=?1",
            [id],
            |row| row.get(0),
        )?;
        if let Some(old) = selected.filter(|old| *old != start) {
            let retryable: bool = tx.query_row(
                "SELECT EXISTS(SELECT 1 FROM session_events prior
                 JOIN session_events done ON done.session_id=prior.session_id
                   AND done.provider_thread=prior.provider_thread AND done.provider_turn=prior.provider_turn
                 WHERE prior.seq=?1 AND prior.seq<?2 AND done.kind='completed'
                   AND json_extract(done.payload,'$.status') IN ('failed','interrupted'))",
                params![old, start], |row| row.get(0),
            )?;
            if !retryable {
                return Err(stale(id));
            }
        }
        let valid: bool = tx.query_row(
            "SELECT EXISTS(SELECT 1 FROM session_events e JOIN agent_sessions s ON s.id=e.session_id
             JOIN flow_sessions f ON f.id=s.flow_session_id
             WHERE f.id=?1 AND s.id=?2 AND e.seq=?3 AND e.kind='started'
               AND e.exec_id=?4 AND e.exec_id=s.driver_exec_id
               AND s.driver_generation=?5 AND e.provider_generation=?6 AND s.provider_generation=?6
               AND NOT EXISTS(SELECT 1 FROM flow_events old WHERE old.flow_id=f.id
                   AND old.kind='selected' AND old.session_event=e.seq AND old.version<>?7))",
            params![id,session,start,driver.exec_id,driver.generation,driver.provider_generation,i64::try_from(*version).map_err(invalid)?], |row| row.get(0),
        )?;
        if !valid {
            return Err(stale(id));
        }
        if selected.is_some_and(|old| old != start) {
            let mut cursor = flow.cursor.clone();
            clear_candidate(&mut cursor);
            tx.execute(
                "UPDATE flow_sessions SET review_json=?2 WHERE id=?1",
                params![id, serde_json::to_string(&cursor)?],
            )?;
        }
        let (node, iterations) = flow.invocation.location(&flow.cursor).map_err(invalid)?;
        tx.execute(
            "UPDATE flow_sessions SET selected_start=?2 WHERE id=?1",
            params![id, start],
        )?;
        tx.execute(
            "INSERT INTO flow_events(flow_id,version,node,iterations,kind,session_event,observed_at,payload)
             VALUES(?1,?2,?3,?4,'selected',?5,?6,'{}')
             ON CONFLICT(flow_id,kind,session_event) DO NOTHING",
            params![id, i64::try_from(*version).map_err(invalid)?, node, serde_json::to_string(&iterations)?, start, now_unix()],
        )?;
        tx.commit()?;
        Ok(())
    }

    /// Record the selected step's observed result while the caller holds its
    /// driver lock. An operation without a completion receipt blocks for inspection.
    pub fn settle_flow_step(&self, id: &str) -> StoreResult<FlowSession> {
        let mut conn = self.conn.lock().expect("store mutex poisoned");
        let tx = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
        let flow = flow_in(&tx, id)?.ok_or(StoreError::NotFound)?;
        let flow = settle_attempt_in(&tx, flow)?;
        tx.commit()?;
        Ok(flow)
    }

    /// Store the step's Run for the driver about to launch it.
    pub fn reserve_attempt(
        &self,
        id: &str,
        version: u64,
        exec: Option<&crate::id::ExecId>,
    ) -> StoreResult<FlowSession> {
        let cwd = self.flow(id)?.ok_or(StoreError::NotFound)?.cwd;
        let _admission = self.lock_checkout(&cwd)?;
        let mut conn = self.conn.lock().expect("store mutex poisoned");
        let tx = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
        let flow = current_flow_in(&tx, id)?;
        if flow.version != version {
            return Err(stale(id));
        }
        reserve_attempt_in(&tx, &flow, exec)?;
        let flow = current_flow_in(&tx, id)?;
        tx.commit()?;
        Ok(flow)
    }

    /// Begin a mechanical effect, or return None for its retained success.
    pub(crate) fn begin_flow_operation(
        &self,
        id: &str,
        version: u64,
        exec: Option<&crate::id::ExecId>,
    ) -> StoreResult<Option<i64>> {
        let mut conn = self.conn.lock().expect("store mutex poisoned");
        let tx = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
        let flow = current_flow_in(&tx, id)?;
        if flow.version != version
            || flow.failure.is_some()
            || !matches!(flow.current_step(), Some(ConcreteStep::Command(_)))
        {
            return Err(stale(id));
        }
        if let Some((_, outcome)) = operation_in(&tx, id)? {
            return if outcome.as_deref() == Some("completed") {
                Ok(None)
            } else {
                Err(stale(id))
            };
        }
        let (node, iterations) = flow.invocation.location(&flow.cursor).map_err(invalid)?;
        tx.execute(
            "INSERT INTO flow_events(flow_id,version,node,iterations,kind,exec_id,observed_at)
            VALUES(?1,?2,?3,?4,'operation_started',?5,?6)",
            params![
                id,
                i64::try_from(version).map_err(invalid)?,
                node,
                serde_json::to_string(&iterations)?,
                exec.map(crate::id::ExecId::as_str),
                now_unix()
            ],
        )?;
        let start = tx.last_insert_rowid();
        tx.execute(
            "UPDATE flow_sessions SET operation_start=?2,current_capture=NULL WHERE id=?1",
            params![id, start],
        )?;
        tx.execute(
            "UPDATE tasks SET started_at=?2 WHERE id=?1 AND started_at IS NULL",
            params![flow.task_id.as_ref().map(TaskId::as_str), now_unix()],
        )?;
        tx.commit()?;
        Ok(Some(start))
    }

    /// The same Flow driver records only an observed operation result.
    pub(crate) fn finish_flow_operation(
        &self,
        id: &str,
        version: u64,
        start: i64,
        exec: Option<&crate::id::ExecId>,
        succeeded: bool,
    ) -> StoreResult<()> {
        let mut conn = self.conn.lock().expect("store mutex poisoned");
        let tx = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
        let flow = current_flow_in(&tx, id)?;
        if flow.version != version || flow.failure.is_some() {
            return Err(stale(id));
        }
        let selected = operation_in(&tx, id)?;
        if selected
            .as_ref()
            .is_none_or(|(selected, _)| *selected != start)
        {
            return Err(stale(id));
        }
        let owner: Option<String> = tx.query_row(
            "SELECT exec_id FROM flow_events WHERE seq=?1",
            [start],
            |row| row.get(0),
        )?;
        if owner.as_deref() != exec.map(crate::id::ExecId::as_str) {
            return Err(stale(id));
        }
        let outcome = if succeeded { "completed" } else { "failed" };
        if let Some((_, Some(previous))) = selected {
            return if previous == outcome {
                Ok(())
            } else {
                Err(stale(id))
            };
        }
        tx.execute("INSERT INTO flow_events(flow_id,version,node,iterations,kind,exec_id,observed_at,operation_start,outcome)
            SELECT flow_id,version,node,iterations,'operation_completed',exec_id,?2,seq,?3 FROM flow_events WHERE seq=?1",
            params![start,now_unix(),outcome])?;
        tx.commit()?;
        Ok(())
    }

    pub(crate) fn flow_output(
        &self,
        id: &str,
    ) -> StoreResult<Result<crate::engine::SkillOutcome, String>> {
        let conn = self.conn.lock().expect("store mutex poisoned");
        selected_output_in(&conn, &current_flow_in(&conn, id)?)
    }

    /// Correct invalid output in the same conversation, with two corrective turns
    /// at most.
    pub(crate) fn correct_flow_output(&self, id: &str, version: u64) -> StoreResult<FlowSession> {
        let mut conn = self.conn.lock().expect("store mutex poisoned");
        let tx = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
        let flow = current_flow_in(&tx, id)?;
        if flow.version != version {
            return Err(stale(id));
        }
        let Err(error) = selected_output_in(&tx, &flow)? else {
            return Err(stale(id));
        };
        let (node, iterations) = flow.invocation.location(&flow.cursor).map_err(invalid)?;
        let count: i64 = tx.query_row(
            "SELECT COUNT(*) FROM flow_events selected
             JOIN session_events start ON start.seq=selected.session_event
             JOIN session_events done ON done.session_id=start.session_id
               AND done.provider_thread=start.provider_thread AND done.provider_turn=start.provider_turn
               AND done.kind='completed' AND json_extract(done.payload,'$.status')='completed'
             WHERE selected.flow_id=?1 AND selected.node=?2 AND selected.iterations=?3 AND selected.kind='selected'
               AND selected.seq>COALESCE((SELECT MAX(consumed.seq) FROM flow_events consumed
                 WHERE consumed.flow_id=?1 AND consumed.node=?2 AND consumed.iterations=?3 AND consumed.kind='consumed'),0)",
             params![id,node,serde_json::to_string(&iterations)?], |row| row.get(0))?;
        if count >= 3 {
            return Err(invalid(format!(
                "structured output validation exhausted after {count} successful turns: {error}"
            )));
        }
        let mut cursor = flow.cursor.clone();
        clear_candidate(&mut cursor);
        cursor.leaf_mut().progress.direction = Some(format!("The previous final output was invalid: {error}. Return a corrected value using the declared schema; preserve the preceding work."));
        write_cursor_in(&tx, (id, version), &cursor, None, true)?;
        let flow = current_flow_in(&tx, id)?;
        tx.commit()?;
        Ok(flow)
    }

    /// The launch publishes the reserved attempt with the provider it starts.
    pub fn publish_attempt(
        &self,
        id: &str,
        version: u64,
        captured: i64,
        provider: &str,
        model: Option<&str>,
    ) -> StoreResult<()> {
        let conn = self.conn.lock().expect("store mutex poisoned");
        if conn.execute(
            "UPDATE agent_sessions SET input_published=1, provider=?4, model=?5 WHERE current_capture=?2 AND input_published=0
             AND EXISTS(SELECT 1 FROM flow_sessions WHERE id=?1 AND current_capture=?2
                AND position_version=?3 AND state='current')
             AND EXISTS(SELECT 1 FROM session_events capture WHERE capture.seq=?2 AND (capture.exec_id IS NULL OR capture.exec_id=?6))",
            params![
                id,
                captured,
                i64::try_from(version).map_err(invalid)?,
                provider,
                model,
                crate::journal::current_exec_id(),
            ],
        )? != 1
        {
            return Err(StoreError::InvalidAuthority(format!(
                "Flow {id} step launch is stale"
            )));
        }
        Ok(())
    }

    /// Persist the driver's cursor, validating candidates against the selected
    /// native output. Moving the position consumes its exact successful receipt
    /// and releases the attempt. The row is evidence of where the driver stands.
    /// `progress` is the finished step's summary, reported to the Task.
    pub fn record_flow_cursor(
        &self,
        id: &str,
        version: u64,
        cursor: &ExecutionCursor,
        progress: Option<&str>,
    ) -> StoreResult<FlowSession> {
        let mut conn = self.conn.lock().expect("store mutex poisoned");
        let tx = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
        let mut saved = current_flow_in(&tx, id)?;
        if saved.version != version {
            return Err(stale(id));
        }
        let mut next = cursor.clone();
        let moved = position_of(&saved.cursor) != position_of(&next);
        if !moved {
            let saved_leaf = saved.cursor.leaf();
            let leaf = next.leaf_mut();
            if saved_leaf.progress.verdict.is_some()
                && leaf.progress.verdict.is_some()
                && saved_leaf.progress.verdict != leaf.progress.verdict
            {
                return Err(StoreError::InvalidAuthority(
                    "cursor conflicts with the recorded Flow decision".into(),
                ));
            }
            if saved_leaf.route.is_some() && leaf.route.is_some() && saved_leaf.route != leaf.route
            {
                return Err(StoreError::InvalidAuthority(
                    "cursor conflicts with the recorded route".into(),
                ));
            }
            leaf.progress.verdict = saved_leaf
                .progress
                .verdict
                .clone()
                .or(leaf.progress.verdict.take());
            leaf.route = saved_leaf.route.clone().or(leaf.route.take());
        }
        if !moved
            && next != saved.cursor
            && (next.leaf().progress.verdict.is_some() || next.leaf().route.is_some())
        {
            match selected_output_in(&tx, &saved)?.map_err(invalid)? {
                crate::engine::SkillOutcome::Decided(verdict)
                    if next.leaf().progress.verdict.as_ref() == Some(&verdict) => {}
                crate::engine::SkillOutcome::Routed(path)
                    if next.leaf().route.as_ref() == Some(&path) => {}
                _ => return Err(stale(id)),
            }
        }
        if next == saved.cursor {
            return Ok(saved);
        }
        if moved {
            consume_selected_in(&tx, &saved)?;
        }
        if let Some(summary) = progress.filter(|summary| !summary.trim().is_empty()) {
            report_to_task_in(
                &tx,
                id,
                &TaskEventKind::Progress {
                    summary: summary.into(),
                },
            )?;
        }
        saved.cursor = next;
        write_cursor_in(
            &tx,
            (id, version),
            &saved.cursor,
            saved.failure.as_ref(),
            moved,
        )?;
        let saved = current_flow_in(&tx, id)?;
        tx.commit()?;
        Ok(saved)
    }

    /// Retain the failed position and selected effect for caller inspection.
    pub fn fail_flow(
        &self,
        id: &str,
        version: u64,
        failure: &TaskFlowBlocker,
    ) -> StoreResult<FlowSession> {
        let mut conn = self.conn.lock().expect("store mutex poisoned");
        let tx = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
        let flow = current_flow_in(&tx, id)?;
        fail_flow_in(&tx, &flow, version, failure)?;
        let flow = current_flow_in(&tx, id)?;
        tx.commit()?;
        Ok(flow)
    }

    /// The Flow ran its last step.
    pub fn end_flow(&self, id: &str, version: u64, summary: &str) -> StoreResult<()> {
        let mut conn = self.conn.lock().expect("store mutex poisoned");
        let tx = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
        end_flow_in(&tx, id, version, summary)?;
        tx.commit()?;
        Ok(())
    }
}

pub(super) fn decode_flow_cursor(
    review_json: Option<&str>,
    step_index: i64,
    iteration: i64,
) -> StoreResult<ExecutionCursor> {
    let root_index = usize::try_from(step_index).map_err(invalid)?;
    let root_iteration = u32::try_from(iteration).map_err(invalid)?;
    let cursor = review_json
        .map(serde_json::from_str::<ExecutionCursor>)
        .transpose()?
        .unwrap_or_else(|| ExecutionCursor {
            index: root_index,
            iteration: root_iteration,
            ..Default::default()
        });
    if cursor.index != root_index || cursor.iteration != root_iteration {
        return Err(StoreError::InvalidData(
            "stored Flow cursor does not match its root projection".into(),
        ));
    }
    Ok(cursor)
}

#[cfg(test)]
impl SqliteStore {
    pub(crate) fn test_flow_turn(&self, run: &str) -> i64 {
        let session = self.session_for_artifact(run).unwrap().unwrap();
        let driver = self
            .session_driver(&session.id)
            .unwrap()
            .unwrap_or_else(|| {
                let exec = crate::id::ExecId::new();
                self.conn
                    .lock()
                    .unwrap()
                    .execute(
                        "INSERT INTO execs(id,trace_id,started_at) VALUES(?1,'fixture',1)",
                        [&exec],
                    )
                    .unwrap();
                self.claim_session_driver(&session.id, None, &exec, false)
                    .unwrap()
            });
        let selection = self.flow_turn_selection(run).unwrap().unwrap();
        let turn = uuid::Uuid::new_v4().to_string();
        let start = self
            .record_session_turn_origin(
                &session.id,
                "fixture-thread",
                &turn,
                driver.provider_generation,
                driver.exec_id.as_ref().unwrap(),
            )
            .unwrap();
        self.select_flow_turn(&selection, &session.id, &driver, start)
            .unwrap();
        start
    }

    fn test_turn_event(
        &self,
        start: &i64,
        kind: crate::session::SessionEventKind,
        payload: &serde_json::Value,
    ) -> StoreResult<()> {
        let (session, thread, turn): (String, String, String) =
            self.conn.lock().unwrap().query_row(
                "SELECT session_id,provider_thread,provider_turn FROM session_events WHERE seq=?1",
                [start],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
            )?;
        self.record_session_event(&session, &thread, &turn, kind, payload)?;
        Ok(())
    }

    pub(crate) fn test_output(&self, start: &i64, value: &serde_json::Value) -> StoreResult<()> {
        self.test_turn_event(
            start,
            crate::session::SessionEventKind::Output,
            &serde_json::json!({"value":value}),
        )
    }

    pub(crate) fn test_decision_output(
        &self,
        start: &i64,
        verdict: &crate::engine::transitions::FlowVerdict,
    ) -> StoreResult<()> {
        self.test_output(start, &serde_json::to_value(verdict)?)
    }

    pub(crate) fn test_finish_flow_turn(&self, start: &i64, status: &str) {
        self.test_turn_event(
            start,
            crate::session::SessionEventKind::Completed,
            &serde_json::json!({"status":status}),
        )
        .unwrap();
    }
}

#[cfg(test)]
mod tests {
    use crate::durable::FlowSession;
    use crate::engine::flow::{Command, RepeatPolicy};
    use crate::engine::invocation::QueuedInvocation;
    use crate::engine::transitions::{FlowDecision, FlowVerdict};
    use crate::engine::{ConcreteCommand, ConcreteSkill, ConcreteStep, ExecutionCursor, Skill};
    use crate::store::sqlite::SqliteStore;

    fn step(id: &str, from: Option<&str>) -> ConcreteStep {
        ConcreteStep::Skill(ConcreteSkill {
            skill: Skill::named(id),
            sources: vec![],
            id: Some(id.into()),
            human: false,
            repeat: from.map(|from| RepeatPolicy { from: from.into() }),
        })
    }

    fn verdict(decision: FlowDecision) -> FlowVerdict {
        FlowVerdict {
            decision,
            summary: "Observed progress; next proof is specific".into(),
        }
    }

    fn attempt(
        store: &SqliteStore,
        invocation: &str,
        _skill: Option<&str>,
        provider: &str,
    ) -> crate::session::AgentSession {
        let flow = store.flow(invocation).unwrap().unwrap();
        let flow = store
            .reserve_attempt(invocation, flow.version, None)
            .unwrap();
        let run = flow.current_attempt.unwrap().run_id;
        store
            .publish_attempt(
                invocation,
                flow.version,
                store.captured_sequence(&run).unwrap().unwrap(),
                provider,
                None,
            )
            .unwrap();
        store.session_for_artifact(&run).unwrap().unwrap()
    }

    fn launched(store: &SqliteStore, steps: Vec<ConcreteStep>, index: usize) -> FlowSession {
        store
            .create_flow(&FlowSession {
                invocation: QueuedInvocation::new("proof", steps).unwrap(),
                cursor: ExecutionCursor {
                    index,
                    ..ExecutionCursor::default()
                },
                version: 0,
                task_id: None,
                wave_id: None,
                cwd: "/repo".into(),
                message: None,
                model: None,
                current_attempt: None,
                pending_session_id: None,
                failure: None,
                finished: false,
                updated_at: time::OffsetDateTime::now_utc(),
            })
            .unwrap()
    }

    #[test]
    fn saved_flow_commands_migrate_without_changing_the_cursor_or_identity() {
        use clap::Parser;

        let dir = tempfile::tempdir().unwrap();
        let store = SqliteStore::open_ephemeral(&dir.path().join("loopflow.db")).unwrap();
        let commands = [
            ("task", vec!["sync", "--plan"], "sync --plan"),
            ("task", vec!["pr", "land", "-c"], "pr land -c"),
            ("task", vec!["wt", "list"], "wt list"),
            (
                "task",
                vec!["commit", "-m", "captured"],
                "commit -m captured",
            ),
            ("rebase", vec![], "sync"),
            ("install", vec![], "home install"),
        ];
        let steps = commands
            .iter()
            .map(|(command, args, _)| {
                ConcreteStep::Command(ConcreteCommand {
                    item: Command {
                        command: (*command).into(),
                        args: args.iter().map(|arg| (*arg).into()).collect(),
                    },
                    sources: vec!["captured-custom-flow".into()],
                })
            })
            .collect();
        let flow = launched(&store, steps, 1);
        let restored = store.flow(flow.id()).unwrap().unwrap();
        assert_eq!(restored.id(), flow.id());
        assert_eq!(restored.cursor, flow.cursor);
        assert_eq!(restored.version, flow.version);
        for (step, (_, _, expected)) in restored.invocation.steps.iter().zip(commands) {
            let ConcreteStep::Command(command) = step else {
                panic!("saved command")
            };
            assert_eq!(command.item.display_name(), expected);
            assert_eq!(command.sources, ["captured-custom-flow"]);
            crate::lf::Cli::try_parse_from(command.item.argv()).unwrap();
        }
        let again: QueuedInvocation =
            serde_json::from_str(&serde_json::to_string(&restored.invocation).unwrap()).unwrap();
        assert_eq!(again, restored.invocation);
    }

    #[test]
    fn repeated_node_acknowledges_only_consumed_successful_steers() {
        let dir = tempfile::tempdir().unwrap();
        let store = SqliteStore::open_ephemeral(&dir.path().join("loopflow.db")).unwrap();
        for status in ["failed", "interrupted", "completed"] {
            let flow = launched(&store, vec![step("work", None), step("next", None)], 0);
            let session = attempt(&store, flow.id(), None, "codex");
            for (seq, evidence) in [
                serde_json::json!({"type":"user_input","op":"steer_seed_through","text":"10"}),
                serde_json::json!({"type":"user_input","op":"steer_transport_accepted:11","text":"live direction"}),
            ].into_iter().enumerate() {
                let source = format!("events.jsonl:{seq}");
                store.retain_session_observation(&session, &crate::session::SessionObservation {
                    artifact_key: session.artifact_key.clone(), source: source.clone(),
                    observed_at: 1, task_id: None, wave_id: None,
                    payload: serde_json::json!({"input_id":session.artifact_key,"source":source,"evidence":evidence}),
                }).unwrap();
            }
            let turn = store.test_flow_turn(&session.artifact_key);
            store.test_finish_flow_turn(&turn, status);
            assert_eq!(store.completed_step_steer_id(&flow).unwrap(), 0);
            let recovered = store.settle_flow_step(flow.id()).unwrap();
            if status != "completed" {
                assert!(recovered.failure.is_some());
                assert_eq!(store.completed_step_steer_id(&flow).unwrap(), 0);
                continue;
            }
            let mut cursor = recovered.cursor.clone();
            cursor.finish(&recovered.invocation.steps).unwrap();
            let next = store
                .record_flow_cursor(flow.id(), recovered.version, &cursor, None)
                .unwrap();
            assert_eq!(store.completed_step_steer_id(&next).unwrap(), 0);
            let mut repeated = flow.clone();
            repeated.cursor.iteration = 1;
            assert_eq!(store.completed_step_steer_id(&repeated).unwrap(), 11);
            let other = launched(&store, vec![step("work", None)], 0);
            assert_eq!(store.completed_step_steer_id(&other).unwrap(), 0);
            store
                .conn
                .lock()
                .unwrap()
                .execute(
                    "DELETE FROM session_events WHERE kind='observed' AND captured_event=?1",
                    [session.captured],
                )
                .unwrap();
            assert_eq!(store.completed_step_steer_id(&repeated).unwrap(), 0);
        }
    }

    #[test]
    fn a_native_completion_settles_only_its_selected_flow_turn_without_a_run_outcome() {
        let dir = tempfile::tempdir().unwrap();
        let store = SqliteStore::open_ephemeral(&dir.path().join("loopflow.db")).unwrap();
        let flow = launched(
            &store,
            vec![step("work", None), step("decide", Some("work"))],
            1,
        );
        let flow = store
            .reserve_attempt(flow.id(), flow.version, None)
            .unwrap();
        let run = &flow.current_attempt.as_ref().unwrap().run_id;
        let session = store.session_for_artifact(run).unwrap().unwrap();
        store
            .publish_attempt(
                flow.id(),
                flow.version,
                store.captured_sequence(run).unwrap().unwrap(),
                "codex",
                None,
            )
            .unwrap();
        let exec = crate::id::ExecId::new();
        store
            .conn
            .lock()
            .unwrap()
            .execute(
                "INSERT INTO execs(id,trace_id,started_at) VALUES(?1,'fixture',1)",
                [&exec],
            )
            .unwrap();
        let driver = store
            .claim_session_driver(&session.id, None, &exec, true)
            .unwrap();
        let earlier = store
            .record_session_turn_origin(
                &session.id,
                "thread",
                "earlier-success",
                driver.provider_generation,
                &exec,
            )
            .unwrap();
        store
            .record_session_event(
                &session.id,
                "thread",
                "earlier-success",
                crate::session::SessionEventKind::Completed,
                &serde_json::json!({"status":"completed"}),
            )
            .unwrap();
        let selection = store.flow_turn_selection(run).unwrap().unwrap();
        assert!(store
            .select_flow_turn(&selection, &session.id, &driver, earlier)
            .is_err());
        let start = store
            .record_session_turn_origin(
                &session.id,
                "thread",
                "selected",
                driver.provider_generation,
                &exec,
            )
            .unwrap();
        let mut stale = selection.clone();
        stale.version += 1;
        assert!(store
            .select_flow_turn(&stale, &session.id, &driver, start)
            .is_err());
        store
            .select_flow_turn(&selection, &session.id, &driver, start)
            .unwrap();
        store
            .record_session_event(
                &session.id,
                "thread",
                "unrelated-success",
                crate::session::SessionEventKind::Completed,
                &serde_json::json!({"status":"completed"}),
            )
            .unwrap();
        assert!(
            store.settle_flow_step(flow.id()).is_err(),
            "unrelated successes cannot settle selected work"
        );
        assert!(store
            .end_flow(flow.id(), flow.version, "premature")
            .is_err());
        assert!(!store.flow(flow.id()).unwrap().unwrap().finished);
        store
            .test_decision_output(&start, &verdict(FlowDecision::Advance))
            .unwrap();
        assert!(store
            .test_decision_output(&start, &verdict(FlowDecision::Iterate))
            .is_err());
        let automatic_start = store
            .record_session_turn_origin(
                &session.id,
                "thread",
                "automatic-retry",
                driver.provider_generation,
                &exec,
            )
            .unwrap();
        assert!(
            store
                .select_flow_turn(&selection, &session.id, &driver, automatic_start)
                .is_err(),
            "a live selected turn cannot be replaced"
        );
        store
            .record_session_event(
                &session.id,
                "thread",
                "selected",
                crate::session::SessionEventKind::Completed,
                &serde_json::json!({"status":"failed"}),
            )
            .unwrap();
        store
            .select_flow_turn(&selection, &session.id, &driver, automatic_start)
            .unwrap();
        store
            .test_decision_output(&start, &verdict(FlowDecision::Advance))
            .unwrap();
        assert!(
            store.flow_output(flow.id()).is_err(),
            "a failed turn's retained result cannot navigate the retry"
        );
        assert!(
            store
                .flow(flow.id())
                .unwrap()
                .unwrap()
                .cursor
                .leaf()
                .progress
                .verdict
                .is_none(),
            "a failed native turn cannot supply the retry's navigation"
        );
        store
            .test_decision_output(&automatic_start, &verdict(FlowDecision::Iterate))
            .unwrap();
        assert!(store
            .test_decision_output(&automatic_start, &verdict(FlowDecision::Advance))
            .is_err());
        assert!(
            store.settle_flow_step(flow.id()).is_err(),
            "automatic retry awaits its own completion"
        );
        store
            .record_session_event(
                &session.id,
                "thread",
                "automatic-retry",
                crate::session::SessionEventKind::Completed,
                &serde_json::json!({"status":"failed"}),
            )
            .unwrap();
        assert!(store.settle_flow_step(flow.id()).unwrap().failure.is_some());
    }

    #[test]
    fn agent_steps_reserve_a_conversation_before_launch() {
        let dir = tempfile::tempdir().unwrap();
        let store = SqliteStore::open_ephemeral(&dir.path().join("loopflow.db")).unwrap();
        let flow = launched(&store, vec![step("work", None)], 0);
        let reserved = store
            .reserve_attempt(flow.id(), flow.version, None)
            .unwrap();
        let first = reserved.current_attempt.as_ref().unwrap();
        let session = store
            .session_for_artifact(&first.run_id)
            .unwrap()
            .expect("agent admission reserves its conversation before provider launch");
        let run = session.clone();
        assert!(!session.interactive);
        assert_eq!(session.flow_session_id.as_deref(), Some(flow.id()));
        assert!(!run.input_published);
        assert!(
            reserved.pending_session_id.is_none(),
            "headless work is not a review"
        );
        store
            .rename_session(
                &session.id,
                run.captured,
                "Investigation",
                crate::session::TitleSource::Human,
            )
            .unwrap();
        store
            .publish_attempt(
                flow.id(),
                reserved.version,
                run.captured.unwrap(),
                "codex",
                None,
            )
            .unwrap();
        let actor = store.test_flow_turn(&run.artifact_key);
        store.test_finish_flow_turn(&actor, "failed");
        store.settle_flow_step(flow.id()).unwrap();
        assert!(store.flow(flow.id()).unwrap().unwrap().failure.is_some());
        store.assert_no_historical_runs();
        let history = store.session_history(&session.id, 0, 0).unwrap();
        assert!(history
            .iter()
            .any(|event| event.payload["status"] == "failed"));

        let op = launched(
            &store,
            vec![ConcreteStep::Command(ConcreteCommand {
                item: Command {
                    command: "status".into(),
                    args: vec![],
                },
                sources: vec![],
            })],
            0,
        );
        let reserved = store.reserve_attempt(op.id(), op.version, None).unwrap();
        assert!(reserved.current_attempt.is_none());
        assert_eq!(
            store
                .conn
                .lock()
                .unwrap()
                .query_row(
                    "SELECT COUNT(*) FROM sqlite_master WHERE type='table' AND name='runs'",
                    [],
                    |row| row.get::<_, i64>(0)
                )
                .unwrap(),
            0
        );
    }

    #[test]
    fn a_decision_belongs_to_the_selected_turn() {
        let dir = tempfile::tempdir().unwrap();
        let store = SqliteStore::open_ephemeral(&dir.path().join("loopflow.db")).unwrap();
        let steps = vec![
            step("work", None),
            step("loop-decide", Some("work")),
            step("finish", None),
        ];
        let flow = store
            .create_flow(&FlowSession {
                invocation: QueuedInvocation::new("proof", steps.clone()).unwrap(),
                cursor: ExecutionCursor {
                    index: 1,
                    ..ExecutionCursor::default()
                },
                version: 0,
                task_id: None,
                wave_id: None,
                cwd: "/repo".into(),
                message: None,
                model: None,
                current_attempt: None,
                pending_session_id: None,
                failure: None,
                finished: false,
                updated_at: time::OffsetDateTime::now_utc(),
            })
            .unwrap();
        let id = flow.id().to_string();
        assert_eq!(flow.version, 1);
        assert!(store
            .settle_flow_step(&id)
            .unwrap()
            .current_attempt
            .is_none());

        // Navigation consumes the selected native turn's recorded output.
        let run = attempt(&store, &id, Some("loop-decide"), "codex");
        assert_eq!(run.node, Some(1));
        let actor = store.test_flow_turn(&run.artifact_key);
        let iterate = verdict(FlowDecision::Iterate);
        assert!(store.flow_output(&id).is_err());
        store.test_decision_output(&actor, &iterate).unwrap();
        store.test_decision_output(&actor, &iterate).unwrap();
        assert!(store
            .test_decision_output(&actor, &verdict(FlowDecision::Advance))
            .is_err());
        assert!(
            store.settle_flow_step(&id).is_err(),
            "an unsettled native turn keeps the Flow waiting"
        );
        // A checkpoint of the same position keeps the recorded decision.
        let version = store
            .record_flow_cursor(&id, 1, &flow.cursor, None)
            .unwrap()
            .version;
        assert_eq!(version, 1, "nothing to write");
        assert_eq!(
            store.flow(&id).unwrap().unwrap().cursor.progress.verdict,
            None
        );

        // A failed native turn blocks the Flow with its candidate cleared.
        store.test_finish_flow_turn(&actor, "failed");
        let blocked = store.settle_flow_step(&id).unwrap();
        assert!(blocked
            .failure
            .unwrap()
            .reason
            .contains("loop-decide Run failed"));
        assert!(blocked.cursor.progress.verdict.is_none());
        assert!(store.flow_output(&id).is_err());
    }

    fn successful_boundary(
        store: &SqliteStore,
        flow: &FlowSession,
        decision: Option<FlowDecision>,
    ) -> FlowSession {
        let session = attempt(store, flow.id(), None, "proof");
        let actor = store.test_flow_turn(&session.artifact_key);
        if let Some(decision) = decision {
            store
                .test_decision_output(&actor, &verdict(decision))
                .unwrap();
        }
        store.test_finish_flow_turn(&actor, "completed");
        let saved = store.settle_flow_step(flow.id()).unwrap();
        let mut next = saved.cursor.clone();
        next.finish(&saved.invocation.steps).unwrap();
        store
            .record_flow_cursor(flow.id(), saved.version, &next, None)
            .unwrap()
    }

    #[test]
    fn invalid_output_retries_the_same_conversation_and_exhausts_durably() {
        for valid_retry in [false, true] {
            let dir = tempfile::tempdir().unwrap();
            let path = dir.path().join("loopflow.db");
            let store = SqliteStore::open_ephemeral(&path).unwrap();
            let flow = launched(
                &store,
                vec![
                    step("work", None),
                    step("decide", Some("work")),
                    step("after", None),
                ],
                1,
            );
            let original = attempt(&store, flow.id(), None, "proof");
            let actor = store.test_flow_turn(&original.artifact_key);
            store
                .test_output(&actor, &serde_json::json!({"decision":"advance"}))
                .unwrap();
            store.test_finish_flow_turn(&actor, "completed");
            assert!(store.flow_output(flow.id()).unwrap().is_err());
            let corrected = store.correct_flow_output(flow.id(), flow.version).unwrap();
            assert!(
                corrected.current_attempt.is_none(),
                "correction does not capture in the driver"
            );
            assert_eq!(corrected.invocation, flow.invocation);
            assert_eq!(corrected.cursor.index, flow.cursor.index);
            assert!(store.correct_flow_output(flow.id(), flow.version).is_err());
            drop(store);
            let store = SqliteStore::open_ephemeral(&path).unwrap();
            let second = attempt(&store, flow.id(), None, "proof");
            assert_eq!(second.id, original.id);
            assert_ne!(second.artifact_key, original.artifact_key);
            let actor = store.test_flow_turn(&second.artifact_key);
            if valid_retry {
                store
                    .test_decision_output(&actor, &verdict(FlowDecision::Advance))
                    .unwrap();
                assert!(
                    store.flow_output(flow.id()).is_err(),
                    "output alone cannot settle"
                );
                store.test_finish_flow_turn(&actor, "completed");
                let saved = store.flow(flow.id()).unwrap().unwrap();
                assert_eq!(
                    saved.cursor.progress.verdict,
                    Some(verdict(FlowDecision::Advance))
                );
                let mut next = saved.cursor.clone();
                next.finish(&saved.invocation.steps).unwrap();
                let advanced = store
                    .record_flow_cursor(saved.id(), saved.version, &next, None)
                    .unwrap();
                assert_eq!(advanced.cursor.index, 2);
                assert!(store
                    .record_flow_cursor(saved.id(), saved.version, &next, None)
                    .is_err());
                let conn = rusqlite::Connection::open(&path).unwrap();
                let consumed: i64 = conn
                    .query_row(
                        "SELECT COUNT(*) FROM flow_events WHERE kind='consumed'",
                        [],
                        |row| row.get(0),
                    )
                    .unwrap();
                assert_eq!(consumed, 1);
            } else {
                store
                    .test_output(&actor, &serde_json::json!({"decision":"unknown"}))
                    .unwrap();
                store.test_finish_flow_turn(&actor, "completed");
                let next = store
                    .correct_flow_output(flow.id(), corrected.version)
                    .unwrap();
                let third = attempt(&store, flow.id(), None, "proof");
                assert_eq!(third.id, original.id);
                let actor = store.test_flow_turn(&third.artifact_key);
                store.test_finish_flow_turn(&actor, "completed");
                assert!(store
                    .correct_flow_output(flow.id(), next.version)
                    .unwrap_err()
                    .to_string()
                    .contains("exhausted after 3"));
                assert_eq!(store.session_inputs(&original.id).unwrap().len(), 3);
            }
        }
    }

    #[test]
    fn loop_positions_settle_the_exact_pass_once() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("loopflow.db");
        let store = SqliteStore::open_ephemeral(&path).unwrap();
        let initial = launched(
            &store,
            vec![
                step("work", None),
                step("decide", Some("work")),
                step("after", None),
            ],
            1,
        );
        let first = successful_boundary(&store, &initial, Some(FlowDecision::Iterate));
        assert_eq!(first.id(), initial.id());
        assert_eq!((first.cursor.index, first.cursor.iteration), (0, 1));
        assert_eq!((first.task_id.clone(), first.wave_id.clone()), (None, None));
        assert!(store
            .reserve_attempt(initial.id(), initial.version, None)
            .is_err());
        assert!(store
            .record_flow_cursor(initial.id(), initial.version, &first.cursor, None)
            .is_err());
        let decision = successful_boundary(&store, &first, None);
        let second = successful_boundary(&store, &decision, Some(FlowDecision::Iterate));
        assert_eq!(second.id(), first.id());
        assert_eq!(second.cursor.iteration, 2);
        assert!(!store.flow(initial.id()).unwrap().unwrap().finished);
        assert!(store
            .record_flow_cursor(decision.id(), decision.version, &second.cursor, None)
            .is_err());

        let decision = successful_boundary(&store, &second, None);
        let selected = attempt(&store, decision.id(), None, "proof");
        let actor = store.test_flow_turn(&selected.artifact_key);
        store
            .test_decision_output(&actor, &verdict(FlowDecision::Advance))
            .unwrap();
        let saved = store.flow(decision.id()).unwrap().unwrap();
        let mut next = saved.cursor.clone();
        next.leaf_mut().progress.verdict = Some(verdict(FlowDecision::Advance));
        next.finish(&saved.invocation.steps).unwrap();
        assert!(
            store
                .record_flow_cursor(saved.id(), saved.version, &next, None)
                .is_err(),
            "no continuation before exact success"
        );
        store.test_finish_flow_turn(&actor, "completed");
        let advanced = store
            .record_flow_cursor(saved.id(), saved.version, &next, None)
            .unwrap();
        assert_eq!(advanced.id(), initial.id());
        assert_eq!((advanced.cursor.index, advanced.cursor.iteration), (2, 2));
        assert!(store
            .record_flow_cursor(saved.id(), saved.version, &next, None)
            .is_err());
        let consumed: i64 = store
            .conn
            .lock()
            .unwrap()
            .query_row(
                "SELECT count(*) FROM flow_events WHERE kind='consumed'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(consumed, 5);
        let finished = successful_boundary(&store, &advanced, None);
        store
            .end_flow(finished.id(), finished.version, "done")
            .unwrap();
        assert!(store.flow(initial.id()).unwrap().unwrap().finished);
    }

    #[test]
    fn overlapping_loops_retain_each_return_counter() {
        let dir = tempfile::tempdir().unwrap();
        let store = SqliteStore::open_ephemeral(&dir.path().join("loopflow.db")).unwrap();
        let root = launched(
            &store,
            vec![
                step("start", None),
                step("middle", None),
                step("inner", Some("start")),
                step("outer", Some("middle")),
                step("after", None),
            ],
            3,
        );
        let outer = successful_boundary(&store, &root, Some(FlowDecision::Iterate));
        let at_inner = successful_boundary(&store, &outer, None);
        let inner = successful_boundary(&store, &at_inner, Some(FlowDecision::Iterate));
        assert_eq!(inner.id(), root.id());
        let inner = successful_boundary(&store, &inner, None);
        let inner = successful_boundary(&store, &inner, None);
        let outer_return = successful_boundary(&store, &inner, Some(FlowDecision::Advance));
        assert_eq!(outer_return.id(), outer.id());
        assert_eq!(outer_return.cursor.index, 3);
        let root_return = successful_boundary(&store, &outer_return, Some(FlowDecision::Advance));
        assert_eq!(root_return.id(), root.id());
        assert_eq!(root_return.cursor.index, 4);
        assert_eq!(
            root_return.cursor.progress.repeats,
            [("inner".to_owned(), 1), ("outer".to_owned(), 1)].into()
        );
    }

    /// Recovery retains uncertainty, and only an explicit retry can repeat an effect.
    #[test]
    fn an_interrupted_operation_blocks_for_inspection_instead_of_replaying() {
        let dir = tempfile::tempdir().unwrap();
        let store = SqliteStore::open_ephemeral(&dir.path().join("loopflow.db")).unwrap();
        let flow = launched(
            &store,
            vec![
                ConcreteStep::Command(ConcreteCommand {
                    item: Command {
                        command: "sync".into(),
                        args: vec!["--plan".into()],
                    },
                    sources: vec![],
                }),
                step("finish", None),
            ],
            0,
        );
        let start = store
            .begin_flow_operation(flow.id(), flow.version, None)
            .unwrap()
            .unwrap();
        let mut next = flow.cursor.clone();
        next.finish(&flow.invocation.steps).unwrap();
        assert!(store
            .record_flow_cursor(flow.id(), flow.version, &next, None)
            .is_err());
        let blocked = store.settle_flow_step(flow.id()).unwrap();
        assert!(blocked
            .failure
            .as_ref()
            .unwrap()
            .reason
            .contains("inspect its effect"));
        assert!(store
            .begin_flow_operation(flow.id(), blocked.version, None)
            .is_err());
        assert!(store
            .finish_flow_operation(flow.id(), flow.version, start, None, true)
            .is_err());
    }

    #[test]
    fn a_completed_operation_is_recorded_once_and_never_replayed() {
        let dir = tempfile::tempdir().unwrap();
        let store = SqliteStore::open_ephemeral(&dir.path().join("loopflow.db")).unwrap();
        let flow = launched(
            &store,
            vec![
                ConcreteStep::Command(ConcreteCommand {
                    item: Command {
                        command: "pr".into(),
                        args: vec!["publish".into()],
                    },
                    sources: vec![],
                }),
                step("finish", None),
            ],
            0,
        );
        let start = store
            .begin_flow_operation(flow.id(), flow.version, None)
            .unwrap()
            .unwrap();
        for _ in 0..2 {
            store
                .finish_flow_operation(flow.id(), flow.version, start, None, true)
                .unwrap();
        }
        assert!(store
            .finish_flow_operation(flow.id(), flow.version, start, None, false)
            .is_err());
        assert!(store.settle_flow_step(flow.id()).unwrap().failure.is_none());
        assert!(store
            .begin_flow_operation(flow.id(), flow.version, None)
            .unwrap()
            .is_none());
        let mut next = flow.cursor.clone();
        next.finish(&flow.invocation.steps).unwrap();
        store
            .record_flow_cursor(flow.id(), flow.version, &next, None)
            .unwrap();
        assert!(store
            .finish_flow_operation(flow.id(), flow.version, start, None, true)
            .is_err());
    }
}
