//! Run creation resolves ancestors inside the caller's write transaction.

use rusqlite::{params, Connection, OptionalExtension, Transaction, TransactionBehavior};

use crate::durable::{RunId, TaskId};
use crate::id::WaveId;
use crate::session::{Run, RunEnd, WorkSource};
use crate::store::{StoreError, StoreResult};

/// Column order read by `read_run`.
pub(super) const RUN_COLUMNS: &str = "id,session_id,invocation_id,task_id,wave_id,work_source,
    created_at,published,cwd,skill,node,iterations,attempt,provider,model,caller_run_id,
    outcome,ended_at";

/// A Run with the names of its Work, as a listing returns it.
#[derive(Debug, Clone)]
pub struct ListedRun {
    pub run: Run,
    pub wave: Option<String>,
    pub project: Option<String>,
    pub task: Option<String>,
}

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
    let outcome: Option<String> = row.get(offset + 16)?;
    let ended_at: Option<i64> = row.get(offset + 17)?;
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
            ended: outcome
                .zip(ended_at)
                .map(|(outcome, at)| RunEnd { outcome, at }),
        })
    })())
}

pub(super) fn inherit_agent_work_in(
    conn: &Connection,
    run: &mut Run,
    caller_exec: Option<&crate::id::ExecId>,
) -> StoreResult<()> {
    if run.invocation_id.is_none()
        && (run.work_source.is_none() || run.work_source == Some(WorkSource::Inherited))
    {
        if let Some(exec) = caller_exec {
            if let Some(work) = super::execs::agent_work_in(conn, exec)? {
                run.task_id = work.task_id;
                run.wave_id = work.wave_id;
                run.work_source = Some(work.source);
            }
        }
    }
    Ok(())
}

/// The caller holds an immediate transaction, including any Session reservation.
pub(super) fn insert_run_in(conn: &Transaction<'_>, mut run: Run) -> StoreResult<Run> {
    // Historical work keeps its own attribution. A new launch takes the
    // conversation's present assignment, including a prospective bind.
    let owner = run
        .session_id
        .as_deref()
        .map(|id| {
            conn.query_row(
                "SELECT current_run_id,task_id,wave_id,flow_session_id,work_source
             FROM agent_sessions WHERE id=?1",
                [id],
                |row| {
                    Ok((
                        row.get::<_, String>(0)?,
                        row.get::<_, Option<String>>(1)?,
                        row.get::<_, Option<String>>(2)?,
                        row.get::<_, Option<String>>(3)?,
                        row.get::<_, Option<String>>(4)?,
                    ))
                },
            )
        })
        .transpose()?;
    if let Some((current, task, wave, flow, source)) = &owner {
        if current != run.id.as_str() {
            if run
                .task_id
                .as_ref()
                .is_some_and(|id| Some(id.as_str()) != task.as_deref())
                || run
                    .wave_id
                    .as_ref()
                    .is_some_and(|id| Some(id.as_str()) != wave.as_deref())
                || run
                    .invocation_id
                    .as_ref()
                    .is_some_and(|id| Some(id) != flow.as_ref())
            {
                return Err(invalid("new work and AgentSession ancestry disagree"));
            }
            run.task_id = task
                .as_deref()
                .map(TaskId::parse)
                .transpose()
                .map_err(invalid)?;
            run.wave_id = wave
                .as_deref()
                .map(WaveId::parse)
                .transpose()
                .map_err(invalid)?;
            run.invocation_id = flow.clone();
            run.work_source = source
                .as_ref()
                .map(|source| serde_json::from_value(serde_json::Value::String(source.clone())))
                .transpose()?;
        }
    }
    if let Some(invocation) = &run.invocation_id {
        let (task, wave): (Option<String>, Option<String>) = conn
            .query_row(
                "SELECT task_id, wave_id FROM flow_sessions WHERE id=?1",
                [invocation],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .optional()?
            .ok_or_else(|| invalid(format!("Invocation {invocation} does not exist")))?;
        let task = task
            .map(|task| TaskId::parse(&task))
            .transpose()
            .map_err(invalid)?;
        if run.task_id.is_some() && run.task_id != task {
            return Err(invalid("Run and Invocation Tasks disagree"));
        }
        run.task_id = task;
        let wave = wave
            .map(|wave| WaveId::parse(&wave))
            .transpose()
            .map_err(invalid)?;
        if wave.is_some() {
            if run.wave_id.is_some() && run.wave_id != wave {
                return Err(invalid("Run and Invocation Waves disagree"));
            }
            run.wave_id = wave;
        }
        let (capture, cursor) = super::flows::capture_in(conn, invocation)?;
        let (node, iterations) = capture.location(&cursor).map_err(invalid)?;
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
        let wave = task_wave_in(conn, task)?;
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
    if let Some((current, task, wave, flow, _)) = &owner {
        if current == run.id.as_str() {
            if task
                .as_deref()
                .is_some_and(|id| Some(id) != run.task_id.as_ref().map(TaskId::as_str))
                || wave
                    .as_deref()
                    .is_some_and(|id| Some(id) != run.wave_id.as_ref().map(WaveId::as_str))
                || flow
                    .as_ref()
                    .is_some_and(|id| Some(id) != run.invocation_id.as_ref())
            {
                return Err(invalid("initial work and AgentSession ancestry disagree"));
            }
            conn.execute(
                "UPDATE agent_sessions SET task_id=?2,wave_id=?3,flow_session_id=?4,work_source=?5 WHERE id=?1",
                params![run.session_id, run.task_id.as_ref().map(TaskId::as_str),
                    run.wave_id.as_ref().map(WaveId::as_str), run.invocation_id, source],
            )?;
        }
    }
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
             VALUES(?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,?12,?13,?14,?15,?16,?17,?18)"
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
            run.caller_run_id.as_ref().map(RunId::as_str),
            run.ended.as_ref().map(|end| &end.outcome),
            run.ended.as_ref().map(|end| end.at)
        ],
    )?;
    Ok(run)
}

pub(super) fn task_wave_in(conn: &Connection, task: &TaskId) -> StoreResult<WaveId> {
    let wave: String = conn
        .query_row(
            "SELECT p.wave_id FROM tasks t JOIN projects p ON p.id=t.project_id WHERE t.id=?1",
            [task.as_str()],
            |row| row.get(0),
        )
        .optional()?
        .ok_or_else(|| invalid(format!("Task {task} does not exist")))?;
    WaveId::parse(&wave).map_err(invalid)
}

/// Human and headless reservations select the attempt under the same version fence.
pub(super) fn select_attempt_in(
    conn: &Connection,
    invocation: &str,
    version: u64,
    run: &RunId,
) -> StoreResult<()> {
    let (capture, cursor) = super::flows::capture_in(conn, invocation)?;
    let (node, iterations) = capture.location(&cursor).map_err(invalid)?;
    let iterations = serde_json::to_string(&iterations)?;
    if conn.execute(
        "UPDATE flow_sessions SET current_run_id=?3 WHERE id=?1 AND state='current' AND position_version=?2
         AND EXISTS(SELECT 1 FROM runs r WHERE r.id=?3 AND r.invocation_id=?1 AND ((r.node=?4 AND r.iterations=?5) OR (r.node IS NULL AND r.id=current_run_id)))
         AND (pending_session_id IS NULL OR EXISTS(SELECT 1 FROM agent_sessions s
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
        "SELECT EXISTS(SELECT 1 FROM flow_sessions WHERE id=?1 AND current_run_id=?2 AND state='current')",
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

    /// A Run that belongs to no Session. One that names no Work takes its
    /// caller's; one that runs a Flow step is the step's current attempt.
    pub fn create_run(
        &self,
        mut run: Run,
        caller_exec: Option<&crate::id::ExecId>,
    ) -> StoreResult<Run> {
        let mut conn = self.conn.lock().expect("store mutex poisoned");
        let tx = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
        if let (None, None, Some(caller)) = (&run.task_id, &run.wave_id, &run.caller_run_id) {
            let inherited: Option<(Option<String>, Option<String>)> = tx
                .query_row(
                    "SELECT task_id, wave_id FROM runs WHERE id=?1",
                    [caller.as_str()],
                    |row| Ok((row.get(0)?, row.get(1)?)),
                )
                .optional()?;
            if let Some((task, Some(wave))) = inherited {
                run.task_id = task
                    .map(|id| TaskId::parse(&id))
                    .transpose()
                    .map_err(invalid)?;
                run.wave_id = Some(WaveId::parse(&wave).map_err(invalid)?);
                run.work_source = Some(WorkSource::Inherited);
            }
        }
        inherit_agent_work_in(&tx, &mut run, caller_exec)?;
        let run = insert_run_in(&tx, run)?;
        tx.execute(
            "UPDATE flow_sessions SET current_run_id=?2
             WHERE id=?1 AND state='current' AND pending_session_id IS NULL",
            params![run.invocation_id, run.id.as_str()],
        )?;
        tx.commit()?;
        Ok(run)
    }

    /// A Run settles once.
    pub fn end_run(&self, id: &RunId, end: &RunEnd) -> StoreResult<()> {
        let conn = self.conn.lock().expect("store mutex poisoned");
        conn.execute(
            "UPDATE runs SET outcome=?2, ended_at=?3 WHERE id=?1 AND outcome IS NULL",
            params![id.as_str(), end.outcome, end.at],
        )?;
        Ok(())
    }

    /// Launched Runs that started or ended since `since`, newest first. Work
    /// is named by id, name or provider id; `caller` is a Run id.
    pub fn runs(
        &self,
        wave: Option<&str>,
        project: Option<&str>,
        task: Option<&str>,
        caller: Option<&str>,
        since: i64,
    ) -> StoreResult<Vec<ListedRun>> {
        let conn = self.conn.lock().expect("store mutex poisoned");
        let mut query = conn.prepare(&format!(
            "SELECT {RUN_COLUMNS},
                (SELECT name FROM waves WHERE id=runs.wave_id),
                (SELECT p.project_slug FROM tasks t JOIN projects p ON p.id=t.project_id
                    WHERE t.id=runs.task_id),
                (SELECT issue_identifier FROM tasks WHERE id=runs.task_id)
             FROM runs WHERE published=1
             AND (?1 IS NULL OR wave_id IN (SELECT id FROM waves WHERE id=?1 OR name=?1))
             AND (?2 IS NULL OR task_id IN (SELECT t.id FROM tasks t
                JOIN projects p ON p.id=t.project_id
                WHERE p.id=?2 OR p.project_slug=?2 OR p.external_project_id=?2))
             AND (?3 IS NULL OR task_id IN (SELECT id FROM tasks
                WHERE id=?3 OR issue_identifier=?3 OR external_issue_id=?3))
             AND (?4 IS NULL OR caller_run_id=?4)
             AND (created_at>=?5 OR ended_at>=?5)
             ORDER BY created_at DESC, id DESC"
        ))?;
        let rows = query.query_map(params![wave, project, task, caller, since], |row| {
            Ok(read_run(row, 0)?.map(|run| (run, row.get(18), row.get(19), row.get(20))))
        })?;
        rows.map(|row| {
            let (run, wave, project, task) = row??;
            Ok(ListedRun {
                run,
                wave: wave?,
                project: project?,
                task: task?,
            })
        })
        .collect()
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
