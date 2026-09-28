//! Every Flow invocation's driver transactions: keyed by invocation id, fenced
//! by `position_version` for the cursor, `claim_json` for the one live Task
//! worker, and `current_run_id` for the attempt allowed to act. A Task's own
//! invocation is the same row, read through `tasks.current_invocation_id`;
//! its writes carry the worker claim as one more predicate.

use rusqlite::{params, Connection, OptionalExtension, Transaction, TransactionBehavior};
use time::OffsetDateTime;

use crate::durable::{
    FlowAttempt, FlowSession, RunId, TaskFlowBlocker, TaskId, TaskWorkerClaim,
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
    (SELECT outcome FROM runs WHERE id=f.current_run_id),
    f.pending_session_id,
    (SELECT ready_summary FROM agent_sessions WHERE id=f.pending_session_id),
    f.worker_generation, f.claim_json, f.failure_json, f.state
    FROM flow_sessions f LEFT JOIN tasks t ON t.id=f.task_id";

fn invalid(error: impl std::fmt::Display) -> StoreError {
    StoreError::InvalidData(error.to_string())
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
            pending_session_id=CASE WHEN ?8 THEN NULL ELSE pending_session_id END
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
    claim: Option<&TaskWorkerClaim>,
    summary: &str,
) -> StoreResult<()> {
    let flow = current_flow_in(tx, id)?;
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
    if flow.current_attempt.is_some() {
        return Ok(());
    }
    let step = flow
        .current_checked()
        .ok_or_else(|| invalid("Flow position has no current step"))?;
    let run_id = RunId::new();
    let session = if step.kind != crate::engine::invocation::StepKind::Op && !flow.is_human() {
        let (node, iterations) = super::runs::location_in(tx, flow.id())?;
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
            skill: (step.kind != crate::engine::invocation::StepKind::Op).then_some(step.step),
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

/// Settle the current attempt from its Run's row: a failed or interrupted
/// Run blocks the Flow, a live Run keeps it waiting, a completed Run is the
/// step's completion, and a reservation nothing launched is the next launch.
fn settle_attempt_in(tx: &Transaction<'_>, flow: FlowSession) -> StoreResult<FlowSession> {
    let id = flow.invocation.id.clone();
    if flow.finished || flow.failure.is_some() || flow.is_human() {
        return Ok(flow);
    }
    let Some(attempt) = &flow.current_attempt else {
        return Ok(flow);
    };
    if !attempt.published {
        return Ok(flow);
    }
    let outcome = match attempt.outcome.as_deref() {
        Some(outcome) => outcome.to_owned(),
        // An operation runs inside the driver that recorded it, and the caller
        // is the only live driver: a receipt that never came is an
        // interruption whose side effect may still have happened.
        None if matches!(flow.current_step(), Some(ConcreteStep::Op(_))) => {
            tx.execute(
                "UPDATE runs SET outcome='interrupted', ended_at=?2 WHERE id=?1 AND outcome IS NULL",
                params![attempt.run_id.as_str(), now_unix()],
            )?;
            "interrupted before its completion receipt; inspect its effect before retrying".into()
        }
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

fn require_attempt_authority(
    flow: &FlowSession,
    version: u64,
    run: &RunId,
    what: &str,
) -> StoreResult<()> {
    if flow.version != version
        || flow.failure.is_some()
        || flow
            .current_attempt
            .as_ref()
            .is_none_or(|attempt| attempt.run_id != *run || attempt.outcome.is_some())
    {
        return Err(StoreError::InvalidAuthority(format!(
            "{what} belongs to a stale or different Flow Run"
        )));
    }
    Ok(())
}

/// Record the current attempt's decision on its position. The caller has
/// checked the Run's authority over the invocation and holds the transaction.
pub(super) fn record_verdict_in(
    tx: &Transaction<'_>,
    id: &str,
    run: &RunId,
    verdict: &FlowVerdict,
) -> StoreResult<()> {
    if verdict.summary.trim().is_empty() {
        return Err(invalid("review evidence cannot be empty"));
    }
    super::runs::require_attempt_in(tx, id, run)?;
    let (invocation, mut cursor) = capture_in(tx, id)?;
    let (steps, leaf) = cursor.current_body(&invocation.steps);
    match steps.get(leaf.index) {
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
    let progress = &mut cursor.leaf_mut().progress;
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
        params![id, serde_json::to_string(&cursor)?],
    )?;
    Ok(())
}

/// Record the current attempt's route on its position; same contract as
/// [`record_verdict_in`].
pub(super) fn record_route_in(
    tx: &Transaction<'_>,
    id: &str,
    run: &RunId,
    path: &str,
) -> StoreResult<()> {
    super::runs::require_attempt_in(tx, id, run)?;
    let (invocation, mut cursor) = capture_in(tx, id)?;
    let (steps, leaf) = cursor.current_body(&invocation.steps);
    let Some(ConcreteStep::Xor(branch)) = steps.get(leaf.index) else {
        return Err(StoreError::InvalidAuthority(
            "this Flow step is not a router".into(),
        ));
    };
    if !branch.paths.contains_key(path) {
        return Err(invalid(format!("unknown Flow path {path:?}")));
    }
    let route = &mut cursor.leaf_mut().route;
    if route.as_deref().is_some_and(|saved| saved != path) {
        return Err(StoreError::InvalidAuthority(
            "router already selected a different path".into(),
        ));
    }
    *route = Some(path.to_owned());
    tx.execute(
        "UPDATE flow_sessions SET review_json=?2 WHERE id=?1",
        params![id, serde_json::to_string(&cursor)?],
    )?;
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
                    "UPDATE runs SET outcome='interrupted', ended_at=?2 WHERE id=?1",
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

    /// Settle the current attempt from its Run's row: a failed or interrupted
    /// Run blocks the Flow, a live Run keeps it waiting, a completed Run is
    /// the step's completion. The caller holds the Flow's `driver.lock`, so an
    /// operation's Run without an outcome has no process left and settles as
    /// interrupted.
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
        claim: Option<&TaskWorkerClaim>,
        summary: &str,
    ) -> StoreResult<()> {
        let mut conn = self.conn.lock().expect("store mutex poisoned");
        let tx = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
        end_flow_in(&tx, id, claim, summary)?;
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
            end_flow_in(&tx, expected.id(), None, summary)?;
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

    /// `lf flow decide` from inside the step's Run.
    pub fn record_flow_decision(
        &self,
        id: &str,
        version: u64,
        run: &RunId,
        verdict: &FlowVerdict,
    ) -> StoreResult<()> {
        let mut conn = self.conn.lock().expect("store mutex poisoned");
        let tx = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
        let flow = current_flow_in(&tx, id)?;
        require_attempt_authority(&flow, version, run, "decision")?;
        record_verdict_in(&tx, id, run, verdict)?;
        tx.commit()?;
        Ok(())
    }

    /// `lf flow route` from inside the router's Run.
    pub fn record_flow_path(
        &self,
        id: &str,
        version: u64,
        run: &RunId,
        path: &str,
    ) -> StoreResult<()> {
        let mut conn = self.conn.lock().expect("store mutex poisoned");
        let tx = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
        let flow = current_flow_in(&tx, id)?;
        require_attempt_authority(&flow, version, run, "route")?;
        record_route_in(&tx, id, run, path)?;
        tx.commit()?;
        Ok(())
    }

    /// The Ask key of a loop blocker raised from inside the deciding Run:
    /// `flow:<invocation>:<node>:<iterations>`.
    pub fn flow_blocker_key(&self, id: &str, version: u64, run: &RunId) -> StoreResult<String> {
        let conn = self.conn.lock().expect("store mutex poisoned");
        let flow = current_flow_in(&conn, id)?;
        require_attempt_authority(&flow, version, run, "loop blocker")?;
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
mod tests {
    use crate::durable::{FlowSession, RunId};
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

    fn attempt(store: &SqliteStore, invocation: &str, skill: Option<&str>, provider: &str) -> Run {
        store
            .create_run(
                Run {
                    id: RunId::new(),
                    session_id: None,
                    invocation_id: Some(invocation.to_string()),
                    node: None,
                    iterations: None,
                    attempt: None,
                    task_id: None,
                    wave_id: None,
                    work_source: None,
                    created_at: 1,
                    published: true,
                    cwd: "/repo".into(),
                    skill: skill.map(str::to_owned),
                    provider: Some(provider.into()),
                    model: None,
                    caller_run_id: None,
                    ended: None,
                },
                None,
            )
            .unwrap()
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
        assert!(store
            .session_for_run(&reserved.current_attempt.unwrap().run_id)
            .unwrap()
            .is_none());
    }

    #[test]
    fn a_decision_belongs_to_the_current_attempt_and_recovery_reads_its_outcome() {
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

        // The step's Run is the attempt allowed to decide, at the version it saw.
        let run = attempt(&store, &id, Some("loop-decide"), "codex");
        assert_eq!(run.node, Some(1));
        let iterate = verdict(FlowDecision::Iterate);
        assert!(store
            .record_flow_decision(&id, 1, &RunId::new(), &iterate)
            .is_err());
        assert!(store
            .record_flow_decision(&id, 2, &run.id, &iterate)
            .is_err());
        store
            .record_flow_decision(&id, 1, &run.id, &iterate)
            .unwrap();
        store
            .record_flow_decision(&id, 1, &run.id, &iterate)
            .unwrap();
        assert!(store
            .record_flow_decision(&id, 1, &run.id, &verdict(FlowDecision::Advance))
            .is_err());
        assert!(
            store.recover_flow(&id, None).is_err(),
            "an unsettled Run keeps the Flow waiting"
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

        // A failed Run blocks the Flow with its candidate cleared; retry opens
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
        let blocked = store.recover_flow(&id, None).unwrap();
        assert!(blocked
            .failure
            .unwrap()
            .reason
            .contains("loop-decide Run failed"));
        assert!(blocked.cursor.progress.verdict.is_none());
        assert!(store
            .record_flow_decision(&id, blocked.version, &run.id, &iterate)
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
        store
            .record_flow_decision(&id, retried.version, &second.id, &iterate)
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
            .record_flow_decision(&id, version, &second.id, &verdict(FlowDecision::Advance))
            .is_err());
        assert!(store
            .checkpoint_flow(&id, version - 1, &moved.cursor, None, None)
            .is_err());
    }

    /// The driver died while running an operation: its Run has no receipt and
    /// its side effect may have happened. Recovery blocks for inspection;
    /// `--retry` is the explicit replay.
    #[test]
    fn an_interrupted_operation_blocks_for_inspection_instead_of_replaying() {
        let dir = tempfile::tempdir().unwrap();
        let store = SqliteStore::open_ephemeral(&dir.path().join("loopflow.db")).unwrap();
        let steps = vec![
            ConcreteStep::Op(ConcreteOp {
                item: Op {
                    command: "pr".into(),
                    args: vec!["land".into()],
                },
                flow_parents: vec![],
            }),
            step("finish", None),
        ];
        let flow = launched(&store, steps, 0);
        let id = flow.id().to_string();
        let run = attempt(&store, &id, None, "loopflow");

        let blocked = store.recover_flow(&id, None).unwrap();
        let failure = blocked
            .failure
            .expect("an operation without a receipt blocks");
        assert!(
            failure
                .reason
                .starts_with("op: pr land Run interrupted before its completion receipt"),
            "{}",
            failure.reason
        );
        assert_eq!(
            store
                .run(&run.id)
                .unwrap()
                .unwrap()
                .ended
                .map(|end| end.outcome),
            Some("interrupted".into())
        );

        let retried = store.retry_flow(&id, None).unwrap();
        assert!(retried.failure.is_none() && retried.current_attempt.is_none());
        let second = attempt(&store, &id, None, "loopflow");
        assert_eq!(second.attempt, Some(2));
        store
            .end_run(
                &second.id,
                &RunEnd {
                    outcome: "completed".into(),
                    at: 2,
                },
            )
            .unwrap();
        let settled = store.recover_flow(&id, None).unwrap();
        assert!(settled.failure.is_none());
        assert_eq!(
            settled.current_attempt.unwrap().outcome.as_deref(),
            Some("completed")
        );
    }
}
