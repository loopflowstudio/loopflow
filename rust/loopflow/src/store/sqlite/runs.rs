//! Run creation resolves ancestors inside the caller's write transaction.

use rusqlite::{params, OptionalExtension, Transaction};

use crate::durable::{RunId, TaskId};
use crate::id::WaveId;
use crate::session::{Run, WorkSource};
use crate::store::{StoreError, StoreResult};

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
    Ok((|| {
        Ok(Run {
            id: RunId::parse(&id).map_err(invalid)?,
            session_id,
            invocation_id,
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
    conn.execute(
        "INSERT INTO runs(id,session_id,invocation_id,task_id,wave_id,work_source,created_at,published,cwd,skill)
         VALUES(?1,?2,?3,?4,?5,?6,?7,?8,?9,?10)",
        params![run.id.as_str(), run.session_id, run.invocation_id,
            run.task_id.as_ref().map(TaskId::as_str), run.wave_id.as_ref().map(WaveId::as_str),
            source, run.created_at, run.published, run.cwd.to_string_lossy(), run.skill],
    )?;
    Ok(run)
}
