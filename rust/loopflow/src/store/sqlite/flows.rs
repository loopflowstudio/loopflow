//! A saved Flow's driver transactions: keyed by invocation id, fenced by
//! `position_version` for the cursor and `current_run_id` for the attempt
//! allowed to act. A Task's own invocation is written through its Task
//! transactions in `durable.rs`; both paths share one row and one verdict
//! writer.

use rusqlite::{params, Connection, OptionalExtension, Transaction, TransactionBehavior};

use crate::durable::{FlowAttempt, FlowInvocation, RunId, TaskFlowBlocker, TaskId};
use crate::engine::invocation::QueuedInvocation;
use crate::engine::transitions::FlowVerdict;
use crate::engine::{ConcreteStep, ExecutionCursor};
use crate::id::WaveId;
use crate::store::rows::now_unix;
use crate::store::{StoreError, StoreResult};

use super::SqliteStore;

const FLOW_SELECT: &str = "SELECT invocation_json, review_json, step_index, iteration, updated_at,
    position_version, task_id, wave_id, cwd, message, model, current_run_id,
    (SELECT outcome FROM runs WHERE id=flow_invocations.current_run_id),
    pending_session_id, failure_json, state FROM flow_invocations";

type FlowRow = (
    String,
    Option<String>,
    i64,
    i64,
    i64,
    i64,
    Option<String>,
    Option<String>,
    Option<String>,
    Option<String>,
    Option<String>,
    Option<String>,
    Option<String>,
    Option<String>,
    Option<String>,
    String,
);

fn invalid(error: impl std::fmt::Display) -> StoreError {
    StoreError::InvalidData(error.to_string())
}

fn stale(id: &str) -> StoreError {
    StoreError::InvalidAuthority(format!("Flow {id} changed under its driver"))
}

fn read_flow(row: &rusqlite::Row<'_>) -> rusqlite::Result<StoreResult<FlowInvocation>> {
    let row: FlowRow = (
        row.get(0)?,
        row.get(1)?,
        row.get(2)?,
        row.get(3)?,
        row.get(4)?,
        row.get(5)?,
        row.get(6)?,
        row.get(7)?,
        row.get(8)?,
        row.get(9)?,
        row.get(10)?,
        row.get(11)?,
        row.get(12)?,
        row.get(13)?,
        row.get(14)?,
        row.get(15)?,
    );
    Ok(decode_flow(row))
}

fn decode_flow(
    (
        invocation_json,
        review_json,
        step_index,
        iteration,
        updated_at,
        version,
        task_id,
        wave_id,
        cwd,
        message,
        model,
        current_run_id,
        outcome,
        pending_session_id,
        failure_json,
        state,
    ): FlowRow,
) -> StoreResult<FlowInvocation> {
    let invocation: QueuedInvocation = serde_json::from_str(&invocation_json)?;
    let updated_at = time::OffsetDateTime::from_unix_timestamp(updated_at).map_err(invalid)?;
    let mut failure = failure_json
        .map(|failure| serde_json::from_str::<TaskFlowBlocker>(&failure))
        .transpose()?;
    let cursor = super::durable::decode_flow_cursor(
        review_json.as_deref(),
        step_index,
        iteration,
        &mut failure,
        updated_at,
    )?;
    let cwd = cwd.ok_or_else(|| {
        invalid(format!(
            "Flow {} has no launch record on its row; a Task's own Flow runs through `lf task run`",
            invocation.id
        ))
    })?;
    Ok(FlowInvocation {
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
                    outcome,
                })
            })
            .transpose()?,
        pending_session_id,
        failure,
        finished: state != "current",
        invocation,
    })
}

pub(super) fn flow_in(conn: &Connection, id: &str) -> StoreResult<Option<FlowInvocation>> {
    conn.query_row(&format!("{FLOW_SELECT} WHERE id=?1"), [id], read_flow)
        .optional()?
        .transpose()
}

fn current_flow_in(conn: &Connection, id: &str) -> StoreResult<FlowInvocation> {
    let flow = flow_in(conn, id)?.ok_or(StoreError::NotFound)?;
    if flow.finished {
        return Err(StoreError::InvalidAuthority(format!(
            "Flow {id} is already finished"
        )));
    }
    Ok(flow)
}

/// The captured graph and cursor of any invocation, a Task's included.
pub(super) fn capture_in(
    conn: &Connection,
    id: &str,
) -> StoreResult<(QueuedInvocation, ExecutionCursor)> {
    let (capture, cursor, index, iteration, updated_at): (String, Option<String>, i64, i64, i64) =
        conn.query_row(
            "SELECT invocation_json, review_json, step_index, iteration, updated_at
             FROM flow_invocations WHERE id=?1",
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
    let cursor = super::durable::decode_flow_cursor(
        cursor.as_deref(),
        index,
        iteration,
        &mut None,
        time::OffsetDateTime::from_unix_timestamp(updated_at).map_err(invalid)?,
    )?;
    Ok((capture, cursor))
}

/// Store a launched Flow at its first step. The import re-registers a Flow
/// it already stored; the row it finds wins.
pub(super) fn insert_flow_in(conn: &Connection, flow: &FlowInvocation) -> StoreResult<()> {
    conn.execute(
        "INSERT INTO flow_invocations(id, task_id, wave_id, cwd, message, model, invocation_json,
            step_index, iteration, position_version, worker_generation, failure_json,
            updated_at, review_json, state)
         VALUES(?1,?2,?3,?4,?5,?6,?7,?8,?9,1,0,?10,?11,?12,'current')
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
            now_unix(),
            serde_json::to_string(&flow.cursor)?,
        ],
    )?;
    Ok(())
}

fn write_cursor_in(
    conn: &Connection,
    id: &str,
    version: u64,
    cursor: &ExecutionCursor,
    failure: Option<&TaskFlowBlocker>,
    clear_attempt: bool,
) -> StoreResult<u64> {
    let changed = conn.execute(
        "UPDATE flow_invocations SET review_json=?3, step_index=?4, iteration=?5,
            failure_json=?6, updated_at=?7, position_version=position_version+1,
            current_run_id=CASE WHEN ?8 THEN NULL ELSE current_run_id END,
            pending_session_id=CASE WHEN ?8 THEN NULL ELSE pending_session_id END
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

/// The cursor without the decision or route recorded on its leaf.
fn position_of(cursor: &ExecutionCursor) -> ExecutionCursor {
    let mut position = cursor.clone();
    let leaf = position.leaf_mut();
    leaf.progress.verdict = None;
    leaf.route = None;
    position
}

fn clear_candidate(cursor: &mut ExecutionCursor) {
    let leaf = cursor.leaf_mut();
    leaf.progress.verdict = None;
    leaf.route = None;
}

fn settle_attempt_in(tx: &Transaction<'_>, flow: FlowInvocation) -> StoreResult<FlowInvocation> {
    let id = flow.invocation.id.clone();
    if flow.finished || flow.failure.is_some() || flow.is_human() {
        return Ok(flow);
    }
    let Some(attempt) = &flow.current_attempt else {
        return Ok(flow);
    };
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
    let mut cursor = flow.cursor.clone();
    clear_candidate(&mut cursor);
    let failure = TaskFlowBlocker::now(format!(
        "{} Run {outcome}",
        flow.step_name().unwrap_or_default()
    ));
    write_cursor_in(tx, &id, flow.version, &cursor, Some(&failure), false)?;
    current_flow_in(tx, &id)
}

fn require_attempt_authority(
    flow: &FlowInvocation,
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
        return Err(invalid("decision requires evidence or direction"));
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
        "UPDATE flow_invocations SET review_json=?2 WHERE id=?1",
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
        "UPDATE flow_invocations SET review_json=?2 WHERE id=?1",
        params![id, serde_json::to_string(&cursor)?],
    )?;
    Ok(())
}

impl SqliteStore {
    pub fn create_flow(&self, flow: &FlowInvocation) -> StoreResult<FlowInvocation> {
        let mut conn = self.conn.lock().expect("store mutex poisoned");
        let tx = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
        insert_flow_in(&tx, flow)?;
        let stored = flow_in(&tx, &flow.invocation.id)?.ok_or(StoreError::NotFound)?;
        tx.commit()?;
        Ok(stored)
    }

    pub fn flow(&self, id: &str) -> StoreResult<Option<FlowInvocation>> {
        let conn = self.conn.lock().expect("store mutex poisoned");
        flow_in(&conn, id)
    }

    /// Settle the current attempt from its Run's row: a failed or interrupted
    /// Run blocks the Flow, a live Run keeps it waiting, a completed Run is
    /// the step's completion. The caller holds the Flow's `driver.lock`, so an
    /// operation's Run without an outcome has no process left and settles as
    /// interrupted.
    pub fn recover_flow(&self, id: &str) -> StoreResult<FlowInvocation> {
        let mut conn = self.conn.lock().expect("store mutex poisoned");
        let tx = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
        let flow = flow_in(&tx, id)?.ok_or(StoreError::NotFound)?;
        let flow = settle_attempt_in(&tx, flow)?;
        tx.commit()?;
        Ok(flow)
    }

    /// Clear a recorded failure so the step runs again as a new attempt.
    pub fn retry_flow(&self, id: &str) -> StoreResult<FlowInvocation> {
        let mut conn = self.conn.lock().expect("store mutex poisoned");
        let tx = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
        let flow = settle_attempt_in(&tx, current_flow_in(&tx, id)?)?;
        if flow.failure.is_none() {
            return Err(StoreError::InvalidAuthority(
                "Flow has no recorded failure to retry".into(),
            ));
        }
        if flow.is_human() {
            return Err(StoreError::InvalidAuthority(
                "reopen the human Session to complete this review".into(),
            ));
        }
        let mut cursor = flow.cursor.clone();
        clear_candidate(&mut cursor);
        write_cursor_in(&tx, id, flow.version, &cursor, None, true)?;
        let flow = current_flow_in(&tx, id)?;
        tx.commit()?;
        Ok(flow)
    }

    /// Persist the driver's cursor. A decision or route recorded on the saved
    /// position by its Run outlives a checkpoint of the same position; a
    /// checkpoint that moves the position releases the attempt and the review.
    pub fn checkpoint_flow(
        &self,
        id: &str,
        version: u64,
        cursor: &ExecutionCursor,
    ) -> StoreResult<u64> {
        let mut conn = self.conn.lock().expect("store mutex poisoned");
        let tx = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
        let saved = current_flow_in(&tx, id)?;
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
            return Ok(version);
        }
        let version = write_cursor_in(&tx, id, version, &next, saved.failure.as_ref(), moved)?;
        tx.commit()?;
        Ok(version)
    }

    /// Block the Flow at its position; `lf flow resume --retry` clears it.
    pub fn fail_flow(&self, id: &str, version: u64, reason: &str) -> StoreResult<()> {
        let mut conn = self.conn.lock().expect("store mutex poisoned");
        let tx = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
        let flow = current_flow_in(&tx, id)?;
        let mut cursor = flow.cursor.clone();
        clear_candidate(&mut cursor);
        write_cursor_in(
            &tx,
            id,
            version,
            &cursor,
            Some(&TaskFlowBlocker::now(reason)),
            false,
        )?;
        tx.commit()?;
        Ok(())
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
        Ok(format!(
            "flow:{id}:{}:{}",
            flow.invocation.node_id(&flow.cursor).map_err(invalid)?,
            serde_json::to_string(&crate::engine::flow_graph::flow_iterations(
                &flow.invocation.steps,
                &flow.cursor
            ))?
        ))
    }

    /// The saved Flow waiting on this review Session.
    pub fn waiting_review(&self, session_id: &str) -> StoreResult<FlowInvocation> {
        let conn = self.conn.lock().expect("store mutex poisoned");
        let flow = conn
            .query_row(
                &format!("{FLOW_SELECT} WHERE pending_session_id=?1 AND state='current'"),
                [session_id],
                read_flow,
            )
            .optional()?
            .transpose()?
            .filter(FlowInvocation::is_human)
            .ok_or_else(|| {
                StoreError::InvalidAuthority("Flow Session is stale or already decided".into())
            })?;
        Ok(flow)
    }
}

#[cfg(test)]
mod tests {
    use crate::durable::{FlowInvocation, RunId};
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
            .create_run(Run {
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
            })
            .unwrap()
    }

    fn launched(store: &SqliteStore, steps: Vec<ConcreteStep>, index: usize) -> FlowInvocation {
        store
            .create_flow(&FlowInvocation {
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
            })
            .unwrap()
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
            .create_flow(&FlowInvocation {
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
            })
            .unwrap();
        let id = flow.id().to_string();
        assert_eq!(flow.version, 1);
        assert!(store.recover_flow(&id).unwrap().current_attempt.is_none());

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
            store.recover_flow(&id).is_err(),
            "an unsettled Run keeps the Flow waiting"
        );
        // A checkpoint of the same position keeps the recorded decision.
        let version = store.checkpoint_flow(&id, 1, &flow.cursor).unwrap();
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
        let blocked = store.recover_flow(&id).unwrap();
        assert!(blocked
            .failure
            .unwrap()
            .reason
            .contains("loop-decide Run failed"));
        assert!(blocked.cursor.progress.verdict.is_none());
        assert!(store
            .record_flow_decision(&id, blocked.version, &run.id, &iterate)
            .is_err());
        let retried = store.retry_flow(&id).unwrap();
        assert!(retried.failure.is_none() && retried.current_attempt.is_none());
        assert!(store.retry_flow(&id).is_err(), "nothing left to retry");

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
        let settled = store.recover_flow(&id).unwrap();
        assert_eq!(settled.cursor.progress.verdict, Some(iterate));
        assert_eq!(
            settled.current_attempt.unwrap().outcome.as_deref(),
            Some("completed")
        );
        let mut next = settled.cursor.clone();
        assert!(!next.finish(&steps).unwrap());
        assert_eq!((next.index, next.iteration), (0, 1));
        let version = store.checkpoint_flow(&id, settled.version, &next).unwrap();
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
            .checkpoint_flow(&id, version - 1, &moved.cursor)
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

        let blocked = store.recover_flow(&id).unwrap();
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

        let retried = store.retry_flow(&id).unwrap();
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
        let settled = store.recover_flow(&id).unwrap();
        assert!(settled.failure.is_none());
        assert_eq!(
            settled.current_attempt.unwrap().outcome.as_deref(),
            Some("completed")
        );
    }
}
