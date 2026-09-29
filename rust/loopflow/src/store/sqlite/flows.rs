//! Every Flow invocation's driver transactions: keyed by invocation id, fenced
//! by `position_version` for the cursor and `claim_json` for the Task worker.
//! Navigation belongs to the selected native turn's caller Exec; mechanical
//! results belong to Flow history. Transitional Run rows still own agent launch
//! publication and outcomes where native selection is absent. A Task's own
//! invocation is the same row, read through `tasks.current_invocation_id`;
//! its writes carry the worker claim as one more predicate.

use rusqlite::{params, Connection, OptionalExtension, Transaction, TransactionBehavior};
use time::OffsetDateTime;

use crate::durable::{
    FlowAttempt, FlowSession, FlowTurnSelection, RunId, TaskFlowBlocker, TaskId, TaskWorkerClaim,
    TaskWorkerClaimOutcome, TaskWorkerOwner, WorkRef,
};
use crate::engine::invocation::QueuedInvocation;
use crate::engine::transitions::FlowVerdict;
use crate::engine::{ConcreteStep, ExecutionCursor};
use crate::id::WaveId;
use crate::session::Run;
use crate::store::rows::now_unix;
use crate::store::{StoreError, StoreResult};
use crate::work::task::{Task, TaskEventKind};

use super::SqliteStore;

/// The invocation a Task points at: the one its worker advances. Every other
/// invocation naming the Task is a Flow about it. `?1` is the Task id.
pub(super) const TASK_INVOCATION: &str = "id=(SELECT current_invocation_id FROM tasks WHERE id=?1)";

const FLOW_SELECT: &str = "SELECT f.invocation_json, f.review_json, f.step_index, f.iteration,
    f.updated_at, f.position_version, f.task_id, f.wave_id, COALESCE(f.cwd, t.worktree),
    f.message, f.model, f.current_run_id,
    (SELECT published FROM runs WHERE id=f.current_run_id),
    CASE WHEN f.selected_start IS NULL THEN (SELECT outcome FROM runs WHERE id=f.current_run_id)
    ELSE (SELECT json_extract(done.payload,'$.status') FROM session_events start
        JOIN session_events done ON done.session_id=start.session_id
            AND done.provider_thread=start.provider_thread AND done.provider_turn=start.provider_turn
            AND done.kind='completed' WHERE start.seq=f.selected_start) END,
    f.pending_session_id,
    (SELECT ready_summary FROM agent_sessions WHERE id=f.pending_session_id),
    f.worker_generation, f.claim_json, f.failure_json, f.state
    FROM flow_sessions f LEFT JOIN tasks t ON t.id=f.task_id";

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
    let current_run_id: Option<String> = row.get(11)?;
    let published: Option<bool> = row.get(12)?;
    let outcome: Option<String> = row.get(13)?;
    let pending_session_id: Option<String> = row.get(14)?;
    let ready_summary: Option<String> = row.get(15)?;
    let worker_generation: i64 = row.get(16)?;
    let claim_json: Option<String> = row.get(17)?;
    let failure_json: Option<String> = row.get(18)?;
    let state: String = row.get(19)?;
    let decode = || -> StoreResult<FlowSession> {
        let invocation: QueuedInvocation = serde_json::from_str(&invocation_json)?;
        let updated_at = OffsetDateTime::from_unix_timestamp(updated_at).map_err(invalid)?;
        let mut failure = failure_json
            .map(|failure| serde_json::from_str::<TaskFlowBlocker>(&failure))
            .transpose()?;
        let cursor = decode_flow_cursor(
            review_json.as_deref(),
            step_index,
            iteration,
            &mut failure,
            updated_at,
        )?;
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
            current_attempt: current_run_id
                .map(|run_id| {
                    Ok::<_, StoreError>(FlowAttempt {
                        run_id: RunId::parse(&run_id).map_err(invalid)?,
                        published: published.unwrap_or(false),
                        outcome,
                    })
                })
                .transpose()?,
            pending_session_id,
            ready_summary,
            worker_generation: u64::try_from(worker_generation).map_err(invalid)?,
            claim: claim_json
                .map(|claim| serde_json::from_str::<TaskWorkerClaim>(&claim))
                .transpose()?,
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
        .transpose()
}

/// The invocation a Task points at, if any.
pub(super) fn task_flow_in(
    conn: &Connection,
    task_id: &TaskId,
) -> StoreResult<Option<FlowSession>> {
    conn.query_row(
        &format!("{FLOW_SELECT} WHERE f.{TASK_INVOCATION}"),
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
    })
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

/// The Task whose own Flow this invocation is, if a Task points at it.
fn pointed_task_in(conn: &Connection, id: &str) -> StoreResult<Option<Task>> {
    let task: Option<String> = conn
        .query_row(
            "SELECT id FROM tasks WHERE current_invocation_id=?1",
            [id],
            |row| row.get(0),
        )
        .optional()?;
    match task {
        Some(task) => super::children::task_on(conn, &TaskId::parse(&task).map_err(invalid)?),
        None => Ok(None),
    }
}

/// A Task's own Flow reports its progress, failures and completion to the Task.
fn report_to_task_in(conn: &Connection, id: &str, kind: &TaskEventKind) -> StoreResult<()> {
    if let Some(task) = pointed_task_in(conn, id)? {
        super::children::insert_task_event_in(conn, &task, kind)?;
    }
    Ok(())
}

/// The captured graph and cursor of any invocation, a Task's included.
pub(super) fn capture_in(
    conn: &Connection,
    id: &str,
) -> StoreResult<(QueuedInvocation, ExecutionCursor)> {
    let (capture, cursor, index, iteration, updated_at): (String, Option<String>, i64, i64, i64) =
        conn.query_row(
            "SELECT invocation_json, review_json, step_index, iteration, updated_at
             FROM flow_sessions WHERE id=?1",
            [id],
            |row| {
                Ok((
                    row.get(0)?,
                    row.get(1)?,
                    row.get(2)?,
                    row.get(3)?,
                    row.get(4)?,
                ))
            },
        )?;
    let capture: QueuedInvocation = serde_json::from_str(&capture)?;
    let cursor = decode_flow_cursor(
        cursor.as_deref(),
        index,
        iteration,
        &mut None,
        OffsetDateTime::from_unix_timestamp(updated_at).map_err(invalid)?,
    )?;
    Ok((capture, cursor))
}

fn validate_flow(flow: &FlowSession) -> StoreResult<()> {
    crate::engine::flow::validate_repeats(&flow.invocation.steps).map_err(invalid)?;
    let step = flow
        .current_checked()
        .ok_or_else(|| invalid("Flow position has no current step"))?;
    if step.flow.trim().is_empty() || step.step.trim().is_empty() {
        return Err(invalid("flow and step cannot be empty"));
    }
    if step.policy.human && step.policy.id.is_none() {
        return Err(invalid("review flow positions require a stable node id"));
    }
    if flow.claim.is_some() {
        return Err(StoreError::InvalidAuthority(
            "a Flow starts without an advancement claim".into(),
        ));
    }
    Ok(())
}

/// Store a launched Flow at its first step. A Task invocation runs in the
/// Task worktree and stores no cwd of its own; its Wave is the Task's. The
/// import re-registers a Flow it already stored; the row it finds wins.
pub(super) fn insert_flow_in(conn: &Connection, flow: &FlowSession) -> StoreResult<()> {
    validate_flow(flow)?;
    conn.execute(
        "INSERT INTO flow_sessions(id, task_id, wave_id, cwd, message, model, invocation_json,
            step_index, iteration, position_version, worker_generation, failure_json,
            updated_at, review_json, state)
         VALUES(?1,?2,COALESCE(?3,(SELECT p.wave_id FROM tasks t JOIN projects p ON p.id=t.project_id
            WHERE t.id=?2)),CASE WHEN ?2 IS NULL THEN ?4 END,?5,?6,?7,?8,?9,1,0,?10,?11,?12,'current')
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
        ],
    )?;
    Ok(())
}

/// The one cursor write, fenced by the version and the driver's claim. It
/// advances the version, clears the attempt and pending review when the
/// position moved, and releases the claim when the driver stops here.
fn write_cursor_in(
    conn: &Connection,
    (id, version): (&str, u64),
    cursor: &ExecutionCursor,
    failure: Option<&TaskFlowBlocker>,
    clear_attempt: bool,
    claim: Option<&TaskWorkerClaim>,
    release_claim: bool,
) -> StoreResult<u64> {
    let changed = conn.execute(
        "UPDATE flow_sessions SET review_json=?3, step_index=?4, iteration=?5,
            failure_json=?6, updated_at=?7, position_version=position_version+1,
            claim_json=CASE WHEN ?10 THEN NULL ELSE claim_json END,
            current_run_id=CASE WHEN ?8 THEN NULL ELSE current_run_id END,
            pending_session_id=CASE WHEN ?8 THEN NULL ELSE pending_session_id END,
            selected_start=CASE WHEN ?8 THEN NULL ELSE selected_start END,
            operation_start=CASE WHEN ?8 THEN NULL ELSE operation_start END
         WHERE id=?1 AND position_version=?2 AND state='current' AND claim_json IS ?9",
        params![
            id,
            i64::try_from(version).map_err(invalid)?,
            serde_json::to_string(cursor)?,
            i64::try_from(cursor.index).map_err(invalid)?,
            i64::from(cursor.iteration),
            failure.map(serde_json::to_string).transpose()?,
            now_unix(),
            clear_attempt,
            claim.map(serde_json::to_string).transpose()?,
            release_claim,
        ],
    )?;
    if changed != 1 {
        return Err(stale(id));
    }
    Ok(version + 1)
}

/// The cursor without the decision or route recorded on its leaf.
fn position_of(cursor: &ExecutionCursor) -> ExecutionCursor {
    let mut position = cursor.clone();
    clear_candidate(&mut position);
    position
}

fn clear_candidate(cursor: &mut ExecutionCursor) {
    let leaf = cursor.leaf_mut();
    leaf.progress.verdict = None;
    leaf.route = None;
}

/// Block the Flow at its position with `failure`; the failed attempt's Run is
/// named on it. A Task's own Flow records the failure on the Task.
fn fail_flow_in(
    tx: &Transaction<'_>,
    flow: &FlowSession,
    version: u64,
    claim: Option<&TaskWorkerClaim>,
    failure: &TaskFlowBlocker,
) -> StoreResult<()> {
    let mut failure = failure.clone();
    if failure.run_id.is_none() {
        failure.run_id = flow
            .current_attempt
            .as_ref()
            .filter(|attempt| attempt.published)
            .map(|attempt| attempt.run_id.clone());
    }
    let mut cursor = flow.cursor.clone();
    clear_candidate(&mut cursor);
    write_cursor_in(
        tx,
        (flow.id(), version),
        &cursor,
        Some(&failure),
        false,
        claim,
        true,
    )?;
    report_to_task_in(
        tx,
        flow.id(),
        &TaskEventKind::Failed {
            error: failure.reason.clone(),
            resumable: !failure.restart_required,
        },
    )
}

/// Release the position: the attempt and any failure go, the recorded
/// candidate is discarded, and the step runs again as a new attempt.
fn release_in(
    tx: &Transaction<'_>,
    flow: &FlowSession,
    version: u64,
    claim: Option<&TaskWorkerClaim>,
    direction: Option<&str>,
) -> StoreResult<()> {
    let mut cursor = flow.cursor.clone();
    clear_candidate(&mut cursor);
    if let Some(direction) = direction {
        cursor.leaf_mut().progress.direction = Some(direction.to_string());
    }
    write_cursor_in(tx, (flow.id(), version), &cursor, None, true, claim, true)?;
    Ok(())
}

/// End the Flow. A Task's own Flow releases the Task's pointer and records
/// `FlowFinished` with the last step's summary.
fn end_flow_in(
    tx: &Transaction<'_>,
    id: &str,
    version: u64,
    claim: Option<&TaskWorkerClaim>,
    summary: &str,
) -> StoreResult<()> {
    let flow = current_flow_in(tx, id)?;
    if flow.version != version || flow.claim.as_ref() != claim {
        return Err(stale(id));
    }
    consume_selected_in(tx, &flow)?;
    if tx.execute(
        "UPDATE flow_sessions SET state='completed', ended_at=?2, claim_json=NULL
         WHERE id=?1 AND state='current' AND claim_json IS ?3",
        params![
            id,
            now_unix(),
            claim.map(serde_json::to_string).transpose()?
        ],
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
    )?;
    tx.execute(
        "UPDATE tasks SET current_invocation_id=NULL WHERE current_invocation_id=?1",
        [id],
    )?;
    Ok(())
}

/// Store the step's Run before anything launches it. An attempt already at
/// this position is kept: a reservation the launcher has not published, or a
/// completed candidate the driver is about to settle.
fn reserve_attempt_in(tx: &Transaction<'_>, flow: &FlowSession) -> StoreResult<()> {
    if matches!(flow.current_step(), Some(ConcreteStep::Op(_))) {
        return Ok(());
    }
    if flow.current_attempt.is_some() {
        return Ok(());
    }
    let step = flow
        .current_checked()
        .ok_or_else(|| invalid("Flow position has no current step"))?;
    let run_id = RunId::new();
    let session = if !flow.is_human() {
        let (node, iterations) = flow.invocation.location(&flow.cursor).map_err(invalid)?;
        let existing: Option<String> = tx
            .query_row(
                "SELECT s.id FROM runs r JOIN agent_sessions s ON s.id=r.session_id
             WHERE r.invocation_id=?1 AND r.node=?2 AND r.iterations=?3
             AND s.kind='conversation' ORDER BY r.attempt DESC LIMIT 1",
                params![flow.id(), node, serde_json::to_string(&iterations)?],
                |row| row.get(0),
            )
            .optional()?;
        match existing {
            Some(session) => Some(session),
            None => {
                let session = format!("session_{}", uuid::Uuid::new_v4().simple());
                let repo = if flow.cwd.is_dir() {
                    crate::repo::discover_repo_root(&flow.cwd)
                        .map_err(invalid)?
                        .map(|root| {
                            crate::repository::CanonicalRepo::discover(&root)
                                .map(|repo| repo.to_string())
                        })
                        .transpose()
                        .map_err(invalid)?
                } else {
                    None
                };
                tx.execute(
                    "INSERT INTO agent_sessions(id,current_run_id,kind,title,title_source,created_at,interactive,repo)
                     VALUES(?1,?2,'conversation',?3,'generated',?4,0,?5)",
                    params![session, run_id.as_str(), step.step, now_unix(), repo],
                )?;
                Some(session)
            }
        }
    } else {
        None
    };
    let run = super::runs::insert_run_in(
        tx,
        Run {
            id: run_id,
            session_id: session.clone(),
            invocation_id: Some(flow.id().to_owned()),
            node: None,
            iterations: None,
            attempt: None,
            task_id: flow.task_id.clone(),
            wave_id: flow.wave_id.clone(),
            work_source: flow.declared_work().map(|work| work.source),
            created_at: now_unix(),
            published: false,
            cwd: flow.cwd.clone(),
            skill: Some(step.step),
            provider: None,
            model: None,
            caller_run_id: None,
            ended: None,
        },
    )?;
    if let Some(session) = session {
        tx.execute(
            "UPDATE agent_sessions SET current_run_id=?2 WHERE id=?1",
            params![session, run.id.as_str()],
        )?;
    }
    super::runs::select_attempt_in(tx, flow.id(), flow.version, &run.id)
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

/// Recover mechanical results from Flow history and agent results from the
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
        fail_flow_in(tx, &flow, flow.version, flow.claim.as_ref(), &failure)?;
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
    failure.run_id = Some(attempt.run_id.clone());
    fail_flow_in(tx, &flow, flow.version, flow.claim.as_ref(), &failure)?;
    current_flow_in(tx, &id)
}

fn require_turn_authority(
    conn: &Connection,
    flow: &FlowSession,
    version: u64,
    actor: &crate::id::ExecId,
) -> StoreResult<()> {
    let valid: bool = conn.query_row(
        "SELECT EXISTS(SELECT 1 FROM flow_sessions f
         JOIN session_events start ON start.seq=f.selected_start
         JOIN flow_events selected ON selected.flow_id=f.id AND selected.kind='selected'
             AND selected.session_event=start.seq
         JOIN execs caller ON caller.id=?2 AND caller.via_agent=1
             AND caller.caller_session_id=start.session_id
             AND caller.caller_provider_generation=start.provider_generation
             AND caller.caller_flow_turn=json_extract(selected.payload,'$.caller_token')
         JOIN agent_sessions s ON s.id=start.session_id
             AND s.provider_generation=start.provider_generation
         WHERE f.id=?1 AND NOT EXISTS(SELECT 1 FROM session_events done
             WHERE done.session_id=start.session_id AND done.provider_thread=start.provider_thread
                 AND done.provider_turn=start.provider_turn AND done.kind='completed'))",
        params![flow.id(), actor],
        |row| row.get(0),
    )?;
    if flow.version != version || flow.failure.is_some() || !valid {
        return Err(StoreError::InvalidAuthority(
            "navigation requires the selected native turn's original caller".into(),
        ));
    }
    Ok(())
}

fn stale_task_worker(task_id: &TaskId) -> StoreError {
    StoreError::InvalidAuthority(format!("Task worker for {task_id} is stale"))
}

/// Take the Task's position for one worker, which holds it until it parks,
/// finishes, fails or releases. `replacing` names the claim of a worker
/// proven dead; its unfinished attempt ends as interrupted, a completed
/// candidate is kept for the new worker to settle.
fn claim_task_worker_in(
    tx: &Transaction<'_>,
    task_id: &TaskId,
    expected_invocation: &str,
    expected_version: u64,
    owner: &TaskWorkerOwner,
    claimed_at: OffsetDateTime,
    replacing: Option<&TaskWorkerClaim>,
) -> StoreResult<TaskWorkerClaimOutcome> {
    let work = WorkRef::Task(task_id.clone());
    super::durable::require_task_worker_eligible(tx, &work)?;
    let flow = task_flow_in(tx, task_id)?.ok_or(StoreError::NotFound)?;
    if flow.is_human() {
        return Err(StoreError::InvalidAuthority(
            "Review Flow positions wait for their Session to complete".to_string(),
        ));
    }
    if flow.invocation.id != expected_invocation
        || (replacing.is_none() && flow.version != expected_version)
    {
        return Ok(TaskWorkerClaimOutcome::Stale {
            actual_version: flow.version,
        });
    }
    if flow.claim.as_ref() != replacing {
        return match (flow.claim, replacing) {
            (Some(claim), None) => Ok(TaskWorkerClaimOutcome::Busy(claim)),
            _ => Err(stale_task_worker(task_id)),
        };
    }
    let generation = flow
        .worker_generation
        .checked_add(1)
        .ok_or_else(|| StoreError::InvalidData("Task worker generation overflow".to_string()))?;
    let claim = TaskWorkerClaim {
        invocation_id: flow.invocation.id.clone(),
        generation,
        position_version: flow.version,
        owner: owner.clone(),
        claimed_at,
    };
    // The claim also clears a blocker the cursor's legacy verdict shape
    // carried, by writing the cursor as it decoded.
    let changed = tx.execute(
        "UPDATE flow_sessions SET worker_generation=?2, claim_json=?3, failure_json=NULL,
            updated_at=?4, review_json=?7
         WHERE id=?1 AND position_version=?5 AND claim_json IS ?6",
        params![
            flow.invocation.id,
            i64::try_from(generation).map_err(invalid)?,
            serde_json::to_string(&claim)?,
            claimed_at.unix_timestamp(),
            i64::try_from(flow.version).map_err(invalid)?,
            replacing.map(serde_json::to_string).transpose()?,
            serde_json::to_string(&flow.cursor)?,
        ],
    )?;
    if changed != 1 {
        return Err(stale_task_worker(task_id));
    }
    if replacing.is_some() {
        match &flow.current_attempt {
            // A reservation the dead worker never launched is nobody's Run.
            Some(attempt) if !attempt.published => {
                tx.execute(
                    "UPDATE flow_sessions SET current_run_id=NULL WHERE id=?1",
                    [&flow.invocation.id],
                )?;
            }
            // Its launched Run has no process left; the new worker settles it.
            Some(attempt) if attempt.outcome.is_none() => {
                tx.execute(
                    "UPDATE runs SET outcome='interrupted', ended_at=?2 WHERE id=?1
                     AND NOT EXISTS(SELECT 1 FROM flow_sessions WHERE current_run_id=?1 AND selected_start IS NOT NULL)",
                    params![attempt.run_id.as_str(), now_unix()],
                )?;
            }
            _ => {}
        }
    }
    let flow = task_flow_in(tx, task_id)?.ok_or(StoreError::NotFound)?;
    reserve_attempt_in(tx, &flow)?;
    Ok(TaskWorkerClaimOutcome::Claimed(claim))
}

impl SqliteStore {
    pub fn create_flow(&self, flow: &FlowSession) -> StoreResult<FlowSession> {
        let mut conn = self.conn.lock().expect("store mutex poisoned");
        let tx = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
        insert_flow_in(&tx, flow)?;
        let stored = flow_in(&tx, &flow.invocation.id)?.ok_or(StoreError::NotFound)?;
        tx.commit()?;
        Ok(stored)
    }

    /// Make `flow` the Task's Flow. A current Flow the Task points at closes
    /// as `replaced` unless a worker holds it.
    pub fn start_task_flow(
        &self,
        task_id: &TaskId,
        flow: &FlowSession,
    ) -> StoreResult<FlowSession> {
        if flow.task_id.as_ref() != Some(task_id) {
            return Err(StoreError::InvalidAuthority(
                "Flow position does not belong to this Task".to_string(),
            ));
        }
        let mut conn = self.conn.lock().expect("store mutex poisoned");
        let tx = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
        let work = WorkRef::Task(task_id.clone());
        super::durable::require_ready_work(&tx, &work)?;
        super::durable::require_current_task_chapter(&tx, &work)?;
        if flow_in(&tx, flow.id())?.is_some() {
            return Err(StoreError::InvalidAuthority(format!(
                "Flow invocation {} already exists",
                flow.id()
            )));
        }
        if let Some(current) = task_flow_in(&tx, task_id)? {
            if current.claim.is_some() {
                return Err(StoreError::InvalidAuthority(format!(
                    "Task {task_id} worker holds its current Flow; stop it before selecting another"
                )));
            }
            tx.execute(
                "UPDATE flow_sessions SET state='replaced', ended_at=?2 WHERE id=?1",
                params![current.invocation.id, now_unix()],
            )?;
        }
        insert_flow_in(&tx, flow)?;
        tx.execute(
            "UPDATE tasks SET current_invocation_id=?2 WHERE id=?1",
            params![task_id.as_str(), flow.invocation.id],
        )?;
        let stored = task_flow_in(&tx, task_id)?.ok_or(StoreError::NotFound)?;
        tx.commit()?;
        Ok(stored)
    }

    pub fn flow(&self, id: &str) -> StoreResult<Option<FlowSession>> {
        let conn = self.conn.lock().expect("store mutex poisoned");
        flow_in(&conn, id)
    }

    /// The Flow a Task points at.
    pub fn task_flow(&self, task_id: &TaskId) -> StoreResult<Option<FlowSession>> {
        let conn = self.conn.lock().expect("store mutex poisoned");
        task_flow_in(&conn, task_id)
    }

    pub(crate) fn flow_turn_selection(
        &self,
        run: &RunId,
    ) -> StoreResult<Option<FlowTurnSelection>> {
        let conn = self.conn.lock().expect("store mutex poisoned");
        let launch: Option<(String, String)> = conn
            .query_row(
                "SELECT f.id,s.id FROM flow_sessions f JOIN runs r ON r.id=f.current_run_id
             JOIN agent_sessions s ON s.id=r.session_id
             WHERE r.id=?1 AND r.published=1 AND f.state='current' AND s.kind='conversation'",
                [run.as_str()],
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
            flow_id: id,
            version: flow.version,
            claim: flow.claim,
            session_id,
            after,
            caller_token: None,
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

    /// Select a native turn only for the launch authorized at this exact boundary.
    /// A Session observer cannot select a turn; the launching Flow driver carries
    /// its saved version/claim and the reserved Session identity.
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
            claim,
            session_id,
            after,
            caller_token,
        } = selection;
        let caller_token = caller_token.as_deref().ok_or_else(|| stale(id))?;
        let mut conn = self.conn.lock().expect("store mutex poisoned");
        let tx = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
        let flow = current_flow_in(&tx, id)?;
        if session != session_id
            || flow.version != *version
            || flow.claim.as_ref() != claim.as_ref()
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
             VALUES(?1,?2,?3,?4,'selected',?5,?6,json_object('caller_token',?7))
             ON CONFLICT(flow_id,kind,session_event) DO NOTHING",
            params![id, i64::try_from(*version).map_err(invalid)?, node, serde_json::to_string(&iterations)?, start, now_unix(),caller_token],
        )?;
        tx.commit()?;
        Ok(())
    }

    /// Recover the selected boundary while the caller holds its driver lock.
    /// An operation without a completion receipt blocks for inspection; native
    /// recovery retains the selected turn's outcome separately from its driver.
    pub fn recover_flow(
        &self,
        id: &str,
        claim: Option<&TaskWorkerClaim>,
    ) -> StoreResult<FlowSession> {
        let mut conn = self.conn.lock().expect("store mutex poisoned");
        let tx = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
        let flow = flow_in(&tx, id)?.ok_or(StoreError::NotFound)?;
        if flow.claim.as_ref() != claim {
            return Err(stale(id));
        }
        let flow = settle_attempt_in(&tx, flow)?;
        tx.commit()?;
        Ok(flow)
    }

    /// Store the step's Run for the driver about to launch it.
    pub fn reserve_attempt(
        &self,
        id: &str,
        version: u64,
        claim: Option<&TaskWorkerClaim>,
    ) -> StoreResult<FlowSession> {
        let mut conn = self.conn.lock().expect("store mutex poisoned");
        let tx = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
        let flow = current_flow_in(&tx, id)?;
        if flow.version != version || flow.claim.as_ref() != claim {
            return Err(stale(id));
        }
        reserve_attempt_in(&tx, &flow)?;
        let flow = current_flow_in(&tx, id)?;
        tx.commit()?;
        Ok(flow)
    }

    /// Begin a mechanical effect, or return None for its retained success.
    pub(crate) fn begin_flow_operation(
        &self,
        id: &str,
        version: u64,
        claim: Option<&TaskWorkerClaim>,
        exec: Option<&crate::id::ExecId>,
    ) -> StoreResult<Option<i64>> {
        let mut conn = self.conn.lock().expect("store mutex poisoned");
        let tx = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
        let flow = current_flow_in(&tx, id)?;
        if flow.version != version
            || flow.claim.as_ref() != claim
            || flow.failure.is_some()
            || !matches!(flow.current_step(), Some(ConcreteStep::Op(_)))
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
            "UPDATE flow_sessions SET operation_start=?2,current_run_id=NULL WHERE id=?1",
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
        claim: Option<&TaskWorkerClaim>,
        start: i64,
        exec: Option<&crate::id::ExecId>,
        succeeded: bool,
    ) -> StoreResult<()> {
        let mut conn = self.conn.lock().expect("store mutex poisoned");
        let tx = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
        let flow = current_flow_in(&tx, id)?;
        if flow.version != version || flow.claim.as_ref() != claim || flow.failure.is_some() {
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

    /// The launch publishes the reserved attempt with the provider it starts.
    pub fn publish_attempt(
        &self,
        id: &str,
        version: u64,
        run: &RunId,
        claim: Option<&TaskWorkerClaim>,
        provider: &str,
        model: Option<&str>,
    ) -> StoreResult<()> {
        let conn = self.conn.lock().expect("store mutex poisoned");
        if conn.execute(
            "UPDATE runs SET published=1, provider=?5, model=?6 WHERE id=?2 AND published=0
             AND EXISTS(SELECT 1 FROM flow_sessions WHERE id=?1 AND current_run_id=?2
                AND position_version=?3 AND state='current' AND claim_json IS ?4)",
            params![
                id,
                run.as_str(),
                i64::try_from(version).map_err(invalid)?,
                claim.map(serde_json::to_string).transpose()?,
                provider,
                model,
            ],
        )? != 1
        {
            return Err(StoreError::InvalidAuthority(format!(
                "Flow {id} step launch is stale"
            )));
        }
        Ok(())
    }

    /// Clear a recorded failure so the step runs again as a new attempt, with
    /// `direction` for the next attempt when the human supplied one.
    pub fn retry_flow(&self, id: &str, direction: Option<&str>) -> StoreResult<FlowSession> {
        let mut conn = self.conn.lock().expect("store mutex poisoned");
        let tx = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
        let flow = settle_attempt_in(&tx, current_flow_in(&tx, id)?)?;
        let Some(failure) = &flow.failure else {
            return Err(StoreError::InvalidAuthority(
                "Flow has no recorded failure to retry".into(),
            ));
        };
        if failure.restart_required {
            return Err(StoreError::InvalidAuthority(
                "Task failure requires an explicit Flow restart".into(),
            ));
        }
        if flow.is_human() {
            return Err(StoreError::InvalidAuthority(
                "reopen the human Session to complete this review".into(),
            ));
        }
        release_in(&tx, &flow, flow.version, flow.claim.as_ref(), direction)?;
        let flow = current_flow_in(&tx, id)?;
        tx.commit()?;
        Ok(flow)
    }

    /// Give the position back after a step ended without a result.
    pub fn release_flow(
        &self,
        id: &str,
        version: u64,
        claim: Option<&TaskWorkerClaim>,
    ) -> StoreResult<FlowSession> {
        let mut conn = self.conn.lock().expect("store mutex poisoned");
        let tx = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
        let flow = current_flow_in(&tx, id)?;
        release_in(&tx, &flow, version, claim, None)?;
        let flow = current_flow_in(&tx, id)?;
        tx.commit()?;
        Ok(flow)
    }

    /// Persist the driver's cursor. A decision or route recorded on the saved
    /// position by its Run outlives a checkpoint of the same position; a
    /// checkpoint that moves the position releases the attempt and the review.
    /// `progress` is the finished step's summary, reported to the Task.
    pub fn checkpoint_flow(
        &self,
        id: &str,
        version: u64,
        cursor: &ExecutionCursor,
        claim: Option<&TaskWorkerClaim>,
        progress: Option<&str>,
    ) -> StoreResult<FlowSession> {
        let mut conn = self.conn.lock().expect("store mutex poisoned");
        let tx = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
        let saved = current_flow_in(&tx, id)?;
        if saved.version != version || saved.claim.as_ref() != claim {
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
                    "checkpoint conflicts with the saved Flow decision".into(),
                ));
            }
            if saved_leaf.route.is_some() && leaf.route.is_some() && saved_leaf.route != leaf.route
            {
                return Err(StoreError::InvalidAuthority(
                    "checkpoint conflicts with the saved route".into(),
                ));
            }
            leaf.progress.verdict = saved_leaf
                .progress
                .verdict
                .clone()
                .or(leaf.progress.verdict.take());
            leaf.route = saved_leaf.route.clone().or(leaf.route.take());
        }
        if next == saved.cursor {
            return Ok(saved);
        }
        if moved {
            consume_selected_in(&tx, &saved)?;
        }
        // A driver parks at a review; the claim goes with the same write.
        let parks = FlowSession {
            cursor: next.clone(),
            ..saved.clone()
        }
        .is_human();
        write_cursor_in(
            &tx,
            (id, version),
            &next,
            saved.failure.as_ref(),
            moved,
            claim,
            parks,
        )?;
        if let Some(summary) = progress.filter(|summary| !summary.trim().is_empty()) {
            report_to_task_in(
                &tx,
                id,
                &TaskEventKind::Progress {
                    summary: summary.to_string(),
                },
            )?;
        }
        let saved = current_flow_in(&tx, id)?;
        tx.commit()?;
        Ok(saved)
    }

    /// Block the Flow at its position; `lf flow resume --retry` or
    /// `lf task resume` clears it.
    pub fn fail_flow(
        &self,
        id: &str,
        version: u64,
        claim: Option<&TaskWorkerClaim>,
        failure: &TaskFlowBlocker,
    ) -> StoreResult<FlowSession> {
        let mut conn = self.conn.lock().expect("store mutex poisoned");
        let tx = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
        let flow = current_flow_in(&tx, id)?;
        fail_flow_in(&tx, &flow, version, claim, failure)?;
        let flow = current_flow_in(&tx, id)?;
        tx.commit()?;
        Ok(flow)
    }

    /// The Flow ran its last step.
    pub fn end_flow(
        &self,
        id: &str,
        version: u64,
        claim: Option<&TaskWorkerClaim>,
        summary: &str,
    ) -> StoreResult<()> {
        let mut conn = self.conn.lock().expect("store mutex poisoned");
        let tx = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
        end_flow_in(&tx, id, version, claim, summary)?;
        tx.commit()?;
        Ok(())
    }

    /// Park a Task's Flow at its review: the review Session exists and the
    /// row waits on it.
    pub fn reserve_task_review(&self, id: &str, version: u64) -> StoreResult<FlowSession> {
        let mut conn = self.conn.lock().expect("store mutex poisoned");
        let tx = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
        let flow = current_flow_in(&tx, id)?;
        if flow.version != version {
            return Err(stale(id));
        }
        if flow.pending_session_id.is_none() {
            super::sessions::reserve_task_review_in(&tx, &flow)?;
        }
        let flow = current_flow_in(&tx, id)?;
        tx.commit()?;
        Ok(flow)
    }

    /// The human completed the Task's review: its feedback becomes the next
    /// step's direction, or the Flow's closing summary when the review was last.
    pub fn complete_task_review(
        &self,
        task_id: &TaskId,
        expected: &FlowSession,
        summary: &str,
    ) -> StoreResult<()> {
        if expected.task_id.as_ref() != Some(task_id)
            || !expected.is_human()
            || expected.claim.is_some()
            || expected.failure.is_some()
        {
            return Err(StoreError::InvalidAuthority(
                "Task review completion requires its exact unclaimed position".to_string(),
            ));
        }
        let mut conn = self.conn.lock().expect("store mutex poisoned");
        let tx = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
        let current = task_flow_in(&tx, task_id)?.ok_or(StoreError::NotFound)?;
        if current != *expected {
            return Err(StoreError::InvalidAuthority(
                "Task review position changed before completion".to_string(),
            ));
        }
        super::sessions::complete_review_in(&tx, expected)?;
        let mut next = expected.cursor.clone();
        next.leaf_mut().progress.direction = Some(summary.to_string());
        let finished = next
            .finish(&expected.invocation.steps)
            .map_err(|error| StoreError::InvalidData(error.to_string()))?;
        if finished {
            end_flow_in(&tx, expected.id(), expected.version, None, summary)?;
        } else {
            write_cursor_in(
                &tx,
                (expected.id(), expected.version),
                &next,
                None,
                true,
                None,
                true,
            )?;
            report_to_task_in(
                &tx,
                expected.id(),
                &TaskEventKind::Progress {
                    summary: summary.to_string(),
                },
            )?;
        }
        tx.commit()?;
        Ok(())
    }

    pub fn claim_task_worker(
        &self,
        task_id: &TaskId,
        expected_invocation: &str,
        expected_version: u64,
        owner: &TaskWorkerOwner,
        claimed_at: OffsetDateTime,
    ) -> StoreResult<TaskWorkerClaimOutcome> {
        let mut conn = self.conn.lock().expect("store mutex poisoned");
        let tx = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
        let outcome = claim_task_worker_in(
            &tx,
            task_id,
            expected_invocation,
            expected_version,
            owner,
            claimed_at,
            None,
        )?;
        tx.commit()?;
        Ok(outcome)
    }

    /// Replace the claim of a worker proven dead at the same position.
    pub fn reclaim_task_worker(
        &self,
        task_id: &TaskId,
        expected: &TaskWorkerClaim,
        owner: &TaskWorkerOwner,
        claimed_at: OffsetDateTime,
    ) -> StoreResult<TaskWorkerClaim> {
        let mut conn = self.conn.lock().expect("store mutex poisoned");
        let tx = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
        let outcome = claim_task_worker_in(
            &tx,
            task_id,
            &expected.invocation_id,
            expected.position_version,
            owner,
            claimed_at,
            Some(expected),
        )?;
        let TaskWorkerClaimOutcome::Claimed(claim) = outcome else {
            return Err(stale_task_worker(task_id));
        };
        tx.commit()?;
        Ok(claim)
    }

    /// `lf flow decide` from the selected native turn's tool Exec.
    pub fn record_flow_decision(
        &self,
        id: &str,
        version: u64,
        actor: &crate::id::ExecId,
        verdict: &FlowVerdict,
    ) -> StoreResult<()> {
        let mut conn = self.conn.lock().expect("store mutex poisoned");
        let tx = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
        let mut flow = current_flow_in(&tx, id)?;
        require_turn_authority(&tx, &flow, version, actor)?;
        if verdict.summary.trim().is_empty() {
            return Err(invalid("review evidence cannot be empty"));
        }
        match flow.current_step() {
            Some(ConcreteStep::Skill(skill)) if skill.policy.human => {
                return Err(StoreError::InvalidAuthority(
                    "an interactive review returns feedback; its following decision step owns navigation"
                        .into(),
                ))
            }
            Some(ConcreteStep::Skill(skill)) if skill.policy.repeat.is_some() => {}
            _ => {
                return Err(StoreError::InvalidAuthority(
                    "this Flow step does not own a decision".into(),
                ))
            }
        }
        let progress = &mut flow.cursor.leaf_mut().progress;
        if progress
            .verdict
            .as_ref()
            .is_some_and(|saved| saved != verdict)
        {
            return Err(StoreError::InvalidAuthority(
                "Flow step already has a different decision".into(),
            ));
        }
        progress.verdict = Some(verdict.clone());
        tx.execute(
            "UPDATE flow_sessions SET review_json=?2 WHERE id=?1",
            params![id, serde_json::to_string(&flow.cursor)?],
        )?;
        tx.commit()?;
        Ok(())
    }

    /// `lf flow route` from the selected native router turn's tool Exec.
    pub fn record_flow_path(
        &self,
        id: &str,
        version: u64,
        actor: &crate::id::ExecId,
        path: &str,
    ) -> StoreResult<()> {
        let mut conn = self.conn.lock().expect("store mutex poisoned");
        let tx = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
        let mut flow = current_flow_in(&tx, id)?;
        require_turn_authority(&tx, &flow, version, actor)?;
        let Some(ConcreteStep::Xor(branch)) = flow.current_step() else {
            return Err(StoreError::InvalidAuthority(
                "this Flow step is not a router".into(),
            ));
        };
        if !branch.paths.contains_key(path) {
            return Err(invalid(format!("unknown Flow path {path:?}")));
        }
        let route = &mut flow.cursor.leaf_mut().route;
        if route.as_deref().is_some_and(|saved| saved != path) {
            return Err(StoreError::InvalidAuthority(
                "router already selected a different path".into(),
            ));
        }
        *route = Some(path.to_owned());
        tx.execute(
            "UPDATE flow_sessions SET review_json=?2 WHERE id=?1",
            params![id, serde_json::to_string(&flow.cursor)?],
        )?;
        tx.commit()?;
        Ok(())
    }

    /// The Ask key of a loop blocker raised by the selected deciding turn:
    /// `flow:<invocation>:<node>:<iterations>`.
    pub fn flow_blocker_key(
        &self,
        id: &str,
        version: u64,
        actor: &crate::id::ExecId,
    ) -> StoreResult<String> {
        let conn = self.conn.lock().expect("store mutex poisoned");
        let flow = current_flow_in(&conn, id)?;
        require_turn_authority(&conn, &flow, version, actor)?;
        let deciding = matches!(flow.current_step(), Some(ConcreteStep::Skill(skill))
            if skill.policy.repeat.is_some() && !skill.policy.human);
        if !deciding {
            return Err(StoreError::InvalidAuthority(
                "this step cannot report a loop blocker".into(),
            ));
        }
        flow.invocation.blocker_key(&flow.cursor).map_err(invalid)
    }

    /// The saved Flow waiting on this review Session.
    pub fn waiting_review(&self, session_id: &str) -> StoreResult<FlowSession> {
        let conn = self.conn.lock().expect("store mutex poisoned");
        let flow = conn
            .query_row(
                &format!("{FLOW_SELECT} WHERE f.pending_session_id=?1 AND f.state='current'"),
                [session_id],
                read_flow,
            )
            .optional()?
            .transpose()?
            .filter(FlowSession::is_human)
            .ok_or_else(|| {
                StoreError::InvalidAuthority("Flow Session is stale or already decided".into())
            })?;
        Ok(flow)
    }
}

fn decode_flow_progress(
    json: Option<&str>,
    failure: &mut Option<TaskFlowBlocker>,
    observed_at: OffsetDateTime,
) -> StoreResult<crate::engine::transitions::FlowProgress> {
    let Some(json) = json else {
        return Ok(Default::default());
    };
    let mut value: serde_json::Value = serde_json::from_str(json)?;
    if let Some(node) = value
        .get("node_id")
        .and_then(|v| v.as_str())
        .map(str::to_string)
    {
        let count = value
            .get("completed_passes")
            .and_then(|v| v.as_u64())
            .unwrap_or(0);
        value = serde_json::json!({
            "repeats": {node: count}, "direction": value.get("direction"),
            "verdict": value.get("verdict"),
        });
    }
    // Blocked was historically serialized as a navigation verdict. Preserve
    // its evidence as a retryable stop without inventing a forward decision.
    if value.pointer("/verdict/decision").and_then(|v| v.as_str()) == Some("blocked") {
        let reason = value
            .pointer("/verdict/summary")
            .and_then(|v| v.as_str())
            .ok_or_else(|| StoreError::InvalidData("blocked Flow verdict has no summary".into()))?;
        let reason = if reason.trim().is_empty() {
            "legacy Flow verdict is blocked without evidence"
        } else {
            reason
        };
        match failure {
            Some(existing) if existing.reason != reason => {
                existing
                    .reason
                    .push_str(&format!("\nLegacy Flow blocker: {reason}"));
            }
            Some(_) => {}
            None => {
                *failure = Some(TaskFlowBlocker {
                    run_id: None,
                    reason: reason.into(),
                    restart_required: false,
                    observed_at,
                })
            }
        }
        value["verdict"] = serde_json::Value::Null;
    }
    Ok(serde_json::from_value(value)?)
}

pub(super) fn decode_flow_cursor(
    review_json: Option<&str>,
    step_index: i64,
    iteration: i64,
    failure: &mut Option<TaskFlowBlocker>,
    updated_at: OffsetDateTime,
) -> StoreResult<ExecutionCursor> {
    let root_index = usize::try_from(step_index).map_err(invalid)?;
    let root_iteration = u32::try_from(iteration).map_err(invalid)?;
    let saved = review_json
        .map(serde_json::from_str::<serde_json::Value>)
        .transpose()?;
    let cursor = match saved {
        Some(value) if value.get("index").is_some() => serde_json::from_value(value)?,
        _ => ExecutionCursor {
            index: root_index,
            iteration: root_iteration,
            progress: decode_flow_progress(review_json, failure, updated_at)?,
            ..Default::default()
        },
    };
    if cursor.index != root_index || cursor.iteration != root_iteration {
        return Err(StoreError::InvalidData(
            "stored Flow cursor does not match its root projection".into(),
        ));
    }
    Ok(cursor)
}

#[cfg(test)]
impl SqliteStore {
    /// A fixture tool command from one selected native turn; no Run grants navigation.
    pub(crate) fn test_turn_caller(
        &self,
        session: &str,
        generation: i64,
        token: &str,
    ) -> crate::id::ExecId {
        let actor = crate::id::ExecId::new();
        self.conn.lock().unwrap().execute(
            "INSERT INTO execs(id,trace_id,started_at,via_agent,caller_session_id,caller_provider_generation,caller_flow_turn)
             VALUES(?1,'fixture',1,1,?2,?3,?4)",
            params![actor,session,generation,token],
        ).unwrap();
        actor
    }

    pub(crate) fn test_flow_turn(&self, run: &RunId) -> crate::id::ExecId {
        let (session, _) = self.session_for_run(run).unwrap().unwrap();
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
        let mut selection = self.flow_turn_selection(run).unwrap().unwrap();
        let turn = uuid::Uuid::new_v4().to_string();
        selection.caller_token = Some(turn.clone());
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
        self.test_turn_caller(&session.id, driver.provider_generation, &turn)
    }

    pub(crate) fn test_finish_flow_turn(&self, actor: &crate::id::ExecId, status: &str) {
        let (session,thread,turn): (String,String,String) = self.conn.lock().unwrap().query_row(
            "SELECT start.session_id,start.provider_thread,start.provider_turn FROM execs e
             JOIN flow_events selected ON json_extract(selected.payload,'$.caller_token')=e.caller_flow_turn
             JOIN session_events start ON start.seq=selected.session_event WHERE e.id=?1", [actor],
            |row| Ok((row.get(0)?,row.get(1)?,row.get(2)?)),
        ).unwrap();
        self.record_session_event(
            &session,
            &thread,
            &turn,
            crate::session::SessionEventKind::Completed,
            &serde_json::json!({"status":status}),
        )
        .unwrap();
    }
}

#[cfg(test)]
mod tests {
    use crate::durable::FlowSession;
    use crate::engine::flow::{Op, RepeatPolicy};
    use crate::engine::invocation::QueuedInvocation;
    use crate::engine::transitions::{FlowDecision, FlowVerdict};
    use crate::engine::{
        ConcreteOp, ConcreteSkill, ConcreteStep, ExecutionCursor, OccurrencePolicy, Skill,
    };
    use crate::session::{Run, RunEnd};
    use crate::store::sqlite::SqliteStore;

    fn step(id: &str, from: Option<&str>) -> ConcreteStep {
        ConcreteStep::Skill(ConcreteSkill {
            skill: Skill::named(id),
            flow_parents: vec![],
            policy: OccurrencePolicy {
                id: Some(id.into()),
                human: false,
                repeat: from.map(|from| RepeatPolicy { from: from.into() }),
            },
        })
    }

    fn verdict(decision: FlowDecision) -> FlowVerdict {
        FlowVerdict {
            decision,
            summary: "Observed progress; next proof is specific".into(),
        }
    }

    fn attempt(store: &SqliteStore, invocation: &str, _skill: Option<&str>, provider: &str) -> Run {
        let flow = store.flow(invocation).unwrap().unwrap();
        let flow = store
            .reserve_attempt(invocation, flow.version, None)
            .unwrap();
        let run = flow.current_attempt.unwrap().run_id;
        store
            .publish_attempt(invocation, flow.version, &run, None, provider, None)
            .unwrap();
        store.session_for_run(&run).unwrap().unwrap().1
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
                ready_summary: None,
                worker_generation: 0,
                claim: None,
                failure: None,
                finished: false,
                updated_at: time::OffsetDateTime::now_utc(),
            })
            .unwrap()
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
        let (session, _) = store.session_for_run(run).unwrap().unwrap();
        store
            .publish_attempt(flow.id(), flow.version, run, None, "codex", None)
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
        let mut selection = store.flow_turn_selection(run).unwrap().unwrap();
        selection.caller_token = Some("selected-token".into());
        let actor =
            store.test_turn_caller(&session.id, driver.provider_generation, "selected-token");
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
            store.recover_flow(flow.id(), None).is_err(),
            "unrelated successes cannot settle selected work"
        );
        assert!(store
            .end_flow(flow.id(), flow.version, None, "premature")
            .is_err());
        assert!(!store.flow(flow.id()).unwrap().unwrap().finished);
        store
            .record_flow_decision(
                flow.id(),
                flow.version,
                &actor,
                &verdict(FlowDecision::Advance),
            )
            .unwrap();
        assert!(store
            .record_flow_decision(
                flow.id(),
                flow.version,
                &actor,
                &verdict(FlowDecision::Iterate)
            )
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
        selection.caller_token = Some("retry-token".into());
        store
            .select_flow_turn(&selection, &session.id, &driver, automatic_start)
            .unwrap();
        assert!(
            store
                .record_flow_decision(
                    flow.id(),
                    flow.version,
                    &actor,
                    &verdict(FlowDecision::Advance)
                )
                .is_err(),
            "a failed turn's delayed child cannot navigate after retry starts"
        );
        let actor = store.test_turn_caller(&session.id, driver.provider_generation, "retry-token");
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
            .record_flow_decision(
                flow.id(),
                flow.version,
                &actor,
                &verdict(FlowDecision::Iterate),
            )
            .unwrap();
        assert!(store
            .record_flow_decision(
                flow.id(),
                flow.version,
                &actor,
                &verdict(FlowDecision::Advance)
            )
            .is_err());
        assert!(
            store.recover_flow(flow.id(), None).is_err(),
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
        assert!(store
            .recover_flow(flow.id(), None)
            .unwrap()
            .failure
            .is_some());
        let retry = store.retry_flow(flow.id(), None).unwrap();
        let retry = store
            .reserve_attempt(flow.id(), retry.version, None)
            .unwrap();
        let retry_run = &retry.current_attempt.as_ref().unwrap().run_id;
        assert_eq!(
            store.session_for_run(retry_run).unwrap().unwrap().0.id,
            session.id
        );
        store
            .publish_attempt(flow.id(), retry.version, retry_run, None, "codex", None)
            .unwrap();
        let mut retry_selection = store.flow_turn_selection(retry_run).unwrap().unwrap();
        retry_selection.caller_token = Some("explicit-retry-token".into());
        assert!(store
            .select_flow_turn(&retry_selection, &session.id, &driver, earlier)
            .is_err());
        let retry_start = store
            .record_session_turn_origin(
                &session.id,
                "thread",
                "retry",
                driver.provider_generation,
                &exec,
            )
            .unwrap();
        assert!(store
            .select_flow_turn(&selection, &session.id, &driver, retry_start)
            .is_err());
        let wrong_claim = crate::durable::TaskWorkerClaim {
            invocation_id: flow.id().to_owned(),
            generation: 99,
            position_version: retry.version,
            owner: crate::durable::TaskWorkerOwner {
                trace_id: crate::id::TraceId::new(),
                exec_id: exec.clone(),
                pid: 1,
                started_at: 1,
            },
            claimed_at: time::OffsetDateTime::now_utc(),
        };
        let mut stale = retry_selection.clone();
        stale.claim = Some(wrong_claim.clone());
        assert!(store
            .select_flow_turn(&stale, &session.id, &driver, retry_start)
            .is_err());
        store
            .select_flow_turn(&retry_selection, &session.id, &driver, retry_start)
            .unwrap();
        assert!(
            store.recover_flow(flow.id(), None).is_err(),
            "retry cannot consume an older successful turn"
        );
        let completion = store
            .record_session_event(
                &session.id,
                "thread",
                "retry",
                crate::session::SessionEventKind::Completed,
                &serde_json::json!({"status":"completed"}),
            )
            .unwrap();
        let recovered = store
            .recover_flow(flow.id(), None)
            .expect("native completion survives a missing driver outcome");
        let later = store
            .record_session_turn_origin(
                &session.id,
                "thread",
                "later",
                driver.provider_generation,
                &exec,
            )
            .unwrap();
        assert!(
            store
                .select_flow_turn(&retry_selection, &session.id, &driver, later)
                .is_err(),
            "successful selection cannot be replaced"
        );
        assert!(recovered.current_attempt.as_ref().unwrap().completed());
        let mut next = recovered.cursor.clone();
        next.index += 1;
        assert!(store
            .checkpoint_flow(flow.id(), recovered.version + 1, &next, None, None)
            .is_err());
        assert!(store
            .end_flow(
                flow.id(),
                recovered.version,
                Some(&wrong_claim),
                "stale claim"
            )
            .is_err());
        store
            .end_flow(flow.id(), recovered.version, None, "native success")
            .unwrap();
        assert!(store
            .end_flow(flow.id(), recovered.version, None, "duplicate")
            .is_err());
        assert!(store.flow(flow.id()).unwrap().unwrap().finished);
        let conn = store.conn.lock().unwrap();
        let consumed: Vec<i64> = conn
            .prepare("SELECT session_event FROM flow_events WHERE flow_id=?1 AND kind='consumed'")
            .unwrap()
            .query_map([flow.id()], |row| row.get(0))
            .unwrap()
            .collect::<Result<_, _>>()
            .unwrap();
        assert_eq!(consumed, vec![completion]);
        let unknown: (Option<String>, Option<i64>) = conn
            .query_row(
                "SELECT outcome,exit_code FROM execs WHERE id=?1",
                [&exec],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .unwrap();
        assert_eq!(unknown, (None, None));
        drop(conn);
        assert!(
            store.run(run).unwrap().unwrap().ended.is_none(),
            "native success cannot invent a Run outcome"
        );
    }

    #[test]
    fn agent_steps_reserve_a_conversation_and_retries_retain_it() {
        let dir = tempfile::tempdir().unwrap();
        let store = SqliteStore::open_ephemeral(&dir.path().join("loopflow.db")).unwrap();
        let flow = launched(&store, vec![step("work", None)], 0);
        let reserved = store
            .reserve_attempt(flow.id(), flow.version, None)
            .unwrap();
        let first = reserved.current_attempt.as_ref().unwrap();
        let (session, run) = store
            .session_for_run(&first.run_id)
            .unwrap()
            .expect("agent admission reserves its conversation before provider launch");
        assert!(!session.interactive);
        assert_eq!(session.flow_session_id.as_deref(), Some(flow.id()));
        assert!(!run.published);
        assert!(
            reserved.pending_session_id.is_none(),
            "headless work is not a review"
        );
        store
            .rename_session(
                &session.id,
                Some(&run.id),
                "Investigation",
                crate::session::TitleSource::Human,
            )
            .unwrap();
        store
            .publish_attempt(flow.id(), reserved.version, &run.id, None, "codex", None)
            .unwrap();
        store
            .end_run(
                &run.id,
                &RunEnd {
                    outcome: "failed".into(),
                    at: 2,
                },
            )
            .unwrap();
        store.recover_flow(flow.id(), None).unwrap();
        let retry = store.retry_flow(flow.id(), None).unwrap();
        let retry = store
            .reserve_attempt(flow.id(), retry.version, None)
            .unwrap();
        let second = retry.current_attempt.unwrap();
        let (same, replacement) = store.session_for_run(&second.run_id).unwrap().unwrap();
        assert_eq!(same.id, session.id);
        assert_eq!(same.title, "Investigation");
        assert_eq!(same.current_run_id, replacement.id);
        assert_ne!(run.id, replacement.id);
        assert_eq!(
            store.run(&run.id).unwrap().unwrap().ended.unwrap().outcome,
            "failed"
        );
        assert!(store
            .publish_attempt(flow.id(), reserved.version, &run.id, None, "codex", None)
            .is_err());

        let op = launched(
            &store,
            vec![ConcreteStep::Op(ConcreteOp {
                item: Op {
                    command: "status".into(),
                    args: vec![],
                },
                flow_parents: vec![],
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
                    "SELECT COUNT(*) FROM runs WHERE invocation_id=?1",
                    [op.id()],
                    |row| row.get::<_, i64>(0)
                )
                .unwrap(),
            0
        );
    }

    #[test]
    fn a_decision_belongs_to_the_selected_turn_and_recovery_reads_its_outcome() {
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
                ready_summary: None,
                worker_generation: 0,
                claim: None,
                failure: None,
                finished: false,
                updated_at: time::OffsetDateTime::now_utc(),
            })
            .unwrap();
        let id = flow.id().to_string();
        assert_eq!(flow.version, 1);
        assert!(store
            .recover_flow(&id, None)
            .unwrap()
            .current_attempt
            .is_none());

        // Navigation names the selected native turn through the tool's Exec.
        let run = attempt(&store, &id, Some("loop-decide"), "codex");
        assert_eq!(run.node, Some(1));
        let actor = store.test_flow_turn(&run.id);
        let iterate = verdict(FlowDecision::Iterate);
        assert!(store
            .record_flow_decision(&id, 1, &crate::id::ExecId::new(), &iterate)
            .is_err());
        assert!(store
            .record_flow_decision(&id, 2, &actor, &iterate)
            .is_err());
        store
            .record_flow_decision(&id, 1, &actor, &iterate)
            .unwrap();
        store
            .record_flow_decision(&id, 1, &actor, &iterate)
            .unwrap();
        assert!(store
            .record_flow_decision(&id, 1, &actor, &verdict(FlowDecision::Advance))
            .is_err());
        assert!(
            store.recover_flow(&id, None).is_err(),
            "an unsettled native turn keeps the Flow waiting"
        );
        // A checkpoint of the same position keeps the recorded decision.
        let version = store
            .checkpoint_flow(&id, 1, &flow.cursor, None, None)
            .unwrap()
            .version;
        assert_eq!(version, 1, "nothing to write");
        assert_eq!(
            store.flow(&id).unwrap().unwrap().cursor.progress.verdict,
            Some(iterate.clone())
        );

        // A failed native turn blocks the Flow with its candidate cleared; retry opens
        // the position for the next attempt.
        store
            .end_run(
                &run.id,
                &RunEnd {
                    outcome: "failed".into(),
                    at: 2,
                },
            )
            .unwrap();
        store.test_finish_flow_turn(&actor, "failed");
        let blocked = store.recover_flow(&id, None).unwrap();
        assert!(blocked
            .failure
            .unwrap()
            .reason
            .contains("loop-decide Run failed"));
        assert!(blocked.cursor.progress.verdict.is_none());
        assert!(store
            .record_flow_decision(&id, blocked.version, &actor, &iterate)
            .is_err());
        let retried = store.retry_flow(&id, None).unwrap();
        assert!(retried.failure.is_none() && retried.current_attempt.is_none());
        assert!(
            store.retry_flow(&id, None).is_err(),
            "nothing left to retry"
        );

        // The second attempt completes; its decision survives to the settled edge.
        let second = attempt(&store, &id, Some("loop-decide"), "codex");
        assert_eq!(second.attempt, Some(2));
        let successor = store.test_flow_turn(&second.id);
        store
            .record_flow_decision(&id, retried.version, &successor, &iterate)
            .unwrap();
        store
            .end_run(
                &second.id,
                &RunEnd {
                    outcome: "completed".into(),
                    at: 3,
                },
            )
            .unwrap();
        store.test_finish_flow_turn(&successor, "completed");
        let settled = store.recover_flow(&id, None).unwrap();
        assert_eq!(settled.cursor.progress.verdict, Some(iterate));
        assert_eq!(
            settled.current_attempt.unwrap().outcome.as_deref(),
            Some("completed")
        );
        let mut next = settled.cursor.clone();
        assert!(!next.finish(&steps).unwrap());
        assert_eq!((next.index, next.iteration), (0, 1));
        let version = store
            .checkpoint_flow(&id, settled.version, &next, None, None)
            .unwrap()
            .version;
        let moved = store.flow(&id).unwrap().unwrap();
        assert_eq!((moved.version, &moved.cursor), (version, &next));
        assert!(
            moved.current_attempt.is_none(),
            "a new position has no attempt"
        );
        assert!(store
            .record_flow_decision(&id, version, &successor, &verdict(FlowDecision::Advance))
            .is_err());
        assert!(store
            .checkpoint_flow(&id, version - 1, &moved.cursor, None, None)
            .is_err());
    }

    /// Recovery retains uncertainty, and only an explicit retry can repeat an effect.
    #[test]
    fn an_interrupted_operation_blocks_for_inspection_instead_of_replaying() {
        let dir = tempfile::tempdir().unwrap();
        let store = SqliteStore::open_ephemeral(&dir.path().join("loopflow.db")).unwrap();
        let flow = launched(
            &store,
            vec![
                ConcreteStep::Op(ConcreteOp {
                    item: Op {
                        command: "rebase".into(),
                        args: vec!["--plan".into()],
                    },
                    flow_parents: vec![],
                }),
                step("finish", None),
            ],
            0,
        );
        let start = store
            .begin_flow_operation(flow.id(), flow.version, None, None)
            .unwrap()
            .unwrap();
        let mut next = flow.cursor.clone();
        next.finish(&flow.invocation.steps).unwrap();
        assert!(store
            .checkpoint_flow(flow.id(), flow.version, &next, None, None)
            .is_err());
        let blocked = store.recover_flow(flow.id(), None).unwrap();
        assert!(blocked
            .failure
            .as_ref()
            .unwrap()
            .reason
            .contains("inspect its effect"));
        assert!(store
            .begin_flow_operation(flow.id(), blocked.version, None, None)
            .is_err());
        let retried = store.retry_flow(flow.id(), None).unwrap();
        assert!(store
            .finish_flow_operation(flow.id(), flow.version, None, start, None, true)
            .is_err());
        let second = store
            .begin_flow_operation(flow.id(), retried.version, None, None)
            .unwrap()
            .unwrap();
        assert_ne!(start, second);
        store
            .finish_flow_operation(flow.id(), retried.version, None, second, None, true)
            .unwrap();
        store
            .finish_flow_operation(flow.id(), retried.version, None, second, None, true)
            .unwrap();
        assert!(store
            .finish_flow_operation(flow.id(), retried.version, None, second, None, false)
            .is_err());
        assert!(store
            .recover_flow(flow.id(), None)
            .unwrap()
            .failure
            .is_none());
        assert!(store
            .begin_flow_operation(flow.id(), retried.version, None, None)
            .unwrap()
            .is_none());
        store
            .checkpoint_flow(flow.id(), retried.version, &next, None, None)
            .unwrap();
        assert!(store
            .finish_flow_operation(flow.id(), retried.version, None, second, None, true)
            .is_err());
        let conn = store.conn.lock().unwrap();
        let counts: (i64, i64, i64) = conn
            .query_row(
                "SELECT (SELECT COUNT(*) FROM runs),
            (SELECT COUNT(*) FROM flow_events WHERE kind='operation_started'),
            (SELECT COUNT(*) FROM flow_events WHERE kind='operation_completed')",
                [],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
            )
            .unwrap();
        assert_eq!(counts, (0, 2, 1));
        assert_eq!(super::operation_in(&conn, flow.id()).unwrap(), None);
        let earlier: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM flow_events WHERE operation_start=?1",
                [start],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(earlier, 0, "unknown earlier outcome stays absent");
    }
}
