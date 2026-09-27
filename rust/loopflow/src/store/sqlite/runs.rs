//! Run creation resolves ancestors inside the caller's write transaction.

use rusqlite::{params, Connection, OptionalExtension, Transaction};

use crate::durable::{RunId, TaskId};
use crate::id::WaveId;
use crate::session::{Run, WorkSource};
use crate::store::{StoreError, StoreResult};

/// Column order read by `read_run`.
pub(super) const RUN_COLUMNS: &str = "id,session_id,invocation_id,task_id,wave_id,work_source,
    created_at,published,cwd,skill,node,iterations,attempt,provider,model,caller_run_id";

fn invalid(error: impl std::fmt::Display) -> StoreError {
    StoreError::InvalidData(error.to_string())
}

pub(super) fn read_run(
    row: &rusqlite::Row<'_>,
    offset: usize,
) -> rusqlite::Result<StoreResult<Run>> {
    let id: String = row.get(offset)?;
    let session_id = row.get(offset + 1)?;
    let invocation_id = row.get(offset + 2)?;
    let task_id: Option<String> = row.get(offset + 3)?;
    let wave_id: Option<String> = row.get(offset + 4)?;
    let source: Option<String> = row.get(offset + 5)?;
    let created_at = row.get(offset + 6)?;
    let published = row.get(offset + 7)?;
    let cwd: String = row.get(offset + 8)?;
    let skill = row.get(offset + 9)?;
    let node = row.get(offset + 10)?;
    let iterations: Option<String> = row.get(offset + 11)?;
    let attempt = row.get(offset + 12)?;
    let provider = row.get(offset + 13)?;
    let model = row.get(offset + 14)?;
    let caller: Option<String> = row.get(offset + 15)?;
    Ok((|| {
        Ok(Run {
            id: RunId::parse(&id).map_err(invalid)?,
            session_id,
            invocation_id,
            node,
            iterations: iterations
                .map(|value| serde_json::from_str(&value))
                .transpose()?,
            attempt,
            task_id: task_id
                .map(|id| TaskId::parse(&id))
                .transpose()
                .map_err(invalid)?,
            wave_id: wave_id
                .map(|id| WaveId::parse(&id))
                .transpose()
                .map_err(invalid)?,
            work_source: source
                .as_deref()
                .map(|source| match source {
                    "declared" => Ok(WorkSource::Declared),
                    "checkout" => Ok(WorkSource::Checkout),
                    "inherited" => Ok(WorkSource::Inherited),
                    "bound" => Ok(WorkSource::Bound),
                    _ => Err(invalid("unknown Run work provenance")),
                })
                .transpose()?,
            created_at,
            published,
            cwd: cwd.into(),
            skill,
            provider,
            model,
            caller_run_id: caller
                .map(|id| RunId::parse(&id))
                .transpose()
                .map_err(invalid)?,
        })
    })())
}

/// The caller holds an immediate transaction, including any Session reservation.
pub(super) fn insert_run_in(conn: &Transaction<'_>, mut run: Run) -> StoreResult<Run> {
    if let Some(invocation) = &run.invocation_id {
        let task: Option<String> = conn
            .query_row(
                "SELECT task_id FROM flow_invocations WHERE id=?1",
                [invocation],
                |row| row.get(0),
            )
            .optional()?
            .ok_or_else(|| invalid(format!("Invocation {invocation} does not exist")))?;
        let task = task
            .map(|id| TaskId::parse(&id))
            .transpose()
            .map_err(invalid)?;
        if run.task_id.is_some() && run.task_id != task {
            return Err(invalid("Run and Invocation nullable Tasks disagree"));
        }
        run.task_id = task;
        let (node, iterations) = location_in(conn, invocation)?;
        if run.node.is_some_and(|supplied| supplied != node)
            || run
                .iterations
                .as_ref()
                .is_some_and(|supplied| supplied != &iterations)
        {
            return Err(invalid(
                "Run location differs from the captured Invocation cursor",
            ));
        }
        run.node = Some(node);
        run.iterations = Some(iterations);
    }
    if let Some(task) = &run.task_id {
        let wave: String = conn
            .query_row(
                "SELECT p.wave_id FROM tasks t JOIN projects p ON p.id=t.project_id WHERE t.id=?1",
                [task.as_str()],
                |row| row.get(0),
            )
            .optional()?
            .ok_or_else(|| invalid(format!("Task {task} does not exist")))?;
        let wave = WaveId::parse(&wave).map_err(invalid)?;
        if run
            .wave_id
            .as_ref()
            .is_some_and(|supplied| *supplied != wave)
        {
            return Err(invalid("Run Task and Wave disagree"));
        }
        run.wave_id = Some(wave);
    }
    let source = run.work_source.map(|source| match source {
        WorkSource::Declared => "declared",
        WorkSource::Checkout => "checkout",
        WorkSource::Inherited => "inherited",
        WorkSource::Bound => "bound",
    });
    let iterations = run
        .iterations
        .as_ref()
        .map(serde_json::to_string)
        .transpose()?;
    if run.attempt.is_some() {
        return Err(invalid("new Run attempt ordinal is assigned by the writer"));
    }
    if let (Some(invocation), Some(node), Some(iterations)) =
        (&run.invocation_id, run.node, &iterations)
    {
        run.attempt = Some(conn.query_row(
            "SELECT coalesce(max(attempt),0)+1 FROM runs WHERE invocation_id=?1 AND node=?2 AND iterations=?3",
            params![invocation, node, iterations], |row| row.get(0),
        )?);
    }
    conn.execute(
        &format!(
            "INSERT INTO runs({RUN_COLUMNS})
             VALUES(?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,?12,?13,?14,?15,?16)"
        ),
        params![
            run.id.as_str(),
            run.session_id,
            run.invocation_id,
            run.task_id.as_ref().map(TaskId::as_str),
            run.wave_id.as_ref().map(WaveId::as_str),
            source,
            run.created_at,
            run.published,
            run.cwd.to_string_lossy(),
            run.skill,
            run.node,
            iterations,
            run.attempt,
            run.provider,
            run.model,
            run.caller_run_id.as_ref().map(RunId::as_str)
        ],
    )?;
    Ok(run)
}

fn location_in(conn: &Connection, invocation: &str) -> StoreResult<(u32, Vec<Vec<u32>>)> {
    let (capture, cursor, index, iteration, updated_at): (String, Option<String>, i64, i64, i64) = conn.query_row(
        "SELECT invocation_json,review_json,step_index,iteration,updated_at FROM flow_invocations WHERE id=?1",
        [invocation],
        |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?, row.get(4)?)),
    )?;
    let capture: crate::engine::invocation::QueuedInvocation = serde_json::from_str(&capture)?;
    let cursor = super::durable::decode_flow_cursor(
        cursor.as_deref(),
        index,
        iteration,
        &mut None,
        time::OffsetDateTime::from_unix_timestamp(updated_at).map_err(invalid)?,
    )?;
    Ok((
        capture.node_id(&cursor).map_err(invalid)?,
        crate::engine::flow_graph::flow_iterations(&capture.steps, &cursor),
    ))
}

/// Human and headless reservations select the attempt under the same version fence.
pub(super) fn select_attempt_in(
    conn: &Connection,
    invocation: &str,
    version: u64,
    run: &RunId,
) -> StoreResult<()> {
    let (node, iterations) = location_in(conn, invocation)?;
    let iterations = serde_json::to_string(&iterations)?;
    if conn.execute(
        "UPDATE flow_invocations SET current_run_id=?3 WHERE id=?1 AND state='current' AND position_version=?2
         AND EXISTS(SELECT 1 FROM runs r WHERE r.id=?3 AND r.invocation_id=?1 AND ((r.node=?4 AND r.iterations=?5) OR (r.node IS NULL AND r.id=current_run_id)))
         AND (pending_session_id IS NULL OR EXISTS(SELECT 1 FROM sessions s
             WHERE s.id=pending_session_id AND s.current_run_id=?3))",
        params![invocation, i64::try_from(version).map_err(invalid)?, run.as_str(), node, iterations],
    )? != 1 {
        return Err(StoreError::InvalidAuthority("Invocation changed before attempt selection".into()));
    }
    Ok(())
}

pub(super) fn require_attempt_in(
    conn: &Connection,
    invocation: &str,
    run: &RunId,
) -> StoreResult<()> {
    let current: bool = conn.query_row(
        "SELECT EXISTS(SELECT 1 FROM flow_invocations WHERE id=?1 AND current_run_id=?2 AND state='current')",
        params![invocation, run.as_str()], |row| row.get(0),
    )?;
    if !current {
        return Err(StoreError::InvalidAuthority(
            "Run is not the current Invocation attempt".into(),
        ));
    }
    Ok(())
}

impl super::SqliteStore {
    pub fn run(&self, id: &RunId) -> StoreResult<Option<Run>> {
        let conn = self.conn.lock().expect("store mutex poisoned");
        conn.query_row(
            &format!("SELECT {RUN_COLUMNS} FROM runs WHERE id=?1"),
            [id.as_str()],
            |row| read_run(row, 0),
        )
        .optional()?
        .transpose()
    }

    pub fn position_runs(
        &self,
        invocation: &str,
        node: u32,
        iterations: &[Vec<u32>],
    ) -> StoreResult<Vec<Run>> {
        let conn = self.conn.lock().expect("store mutex poisoned");
        let mut query = conn.prepare(&format!(
            "SELECT {RUN_COLUMNS} FROM runs
            WHERE invocation_id=?1 AND node=?2 AND iterations=?3 ORDER BY attempt"
        ))?;
        let rows = query.query_map(
            params![invocation, node, serde_json::to_string(iterations)?],
            |row| read_run(row, 0),
        )?;
        rows.map(|row| row?).collect()
    }
}
