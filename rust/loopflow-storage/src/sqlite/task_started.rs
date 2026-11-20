use std::collections::HashSet;

use crate::StoreResult;
use loopflow_model::durable::TaskId;

use super::SqliteStore;

impl SqliteStore {
    pub fn task_started_or_published(&self, task: &TaskId) -> StoreResult<bool> {
        let mut conn = self.conn.lock().expect("store mutex poisoned");
        let snapshot = conn.savepoint()?;
        task_started_or_published_in(&snapshot, task)
    }

    /// Whether work on the Task began: an owner, a started event, a Session or
    /// Graph step in its checkout, or a Graph past `start`.
    pub fn task_begun(&self, task: &TaskId) -> StoreResult<bool> {
        let mut connection = self.conn.lock().expect("store mutex poisoned");
        let conn = connection.savepoint()?;
        let begun: bool = conn.query_row(
            // Checkout membership is preservation evidence even when a
            // conversation has no explicit Task binding or ownership entry.
            &format!(
                "SELECT EXISTS(SELECT 1 FROM task_ownership WHERE task_id=?1)
                 OR EXISTS(SELECT 1 FROM task_events WHERE task_id=?1
                    AND json_extract(kind_json,'$.kind')='started')
                 OR EXISTS({})
                 OR EXISTS(SELECT 1 FROM graph_processes g JOIN graph_process_steps gs
                    ON gs.graph_lf_process_id=g.lf_process_id WHERE g.task_id=?1)",
                super::task_work::session_ids("?1"),
            ),
            [task.as_str()],
            |row| row.get(0),
        )?;
        if begun {
            return Ok(true);
        }
        conn.query_row(
            "SELECT EXISTS(SELECT 1 FROM task_graphs tg WHERE tg.task_id=t.id AND NOT (tg.step='start' AND tg.lf_process_id IS NULL)) FROM tasks t WHERE t.id=?1",
            [task.as_str()],
            |row| row.get(0),
        )
        .map_err(Into::into)
    }
}

/// The one definition of a started Task that Work surfaces show: a current
/// owner, or a published PR on its current delivery.
pub(super) fn task_started_or_published_in(
    conn: &rusqlite::Connection,
    task: &TaskId,
) -> StoreResult<bool> {
    Ok(super::task_ownership::owned_in(conn, task)?
        || published_in(conn, std::slice::from_ref(task))?.contains(task))
}

/// The listed Tasks whose current delivery has a published PR, in one statement.
pub(super) fn published_in(
    conn: &rusqlite::Connection,
    tasks: &[TaskId],
) -> StoreResult<HashSet<TaskId>> {
    let mut statement = conn.prepare(
        "SELECT DISTINCT task_id FROM task_prs WHERE historical=0
            AND publication_requested_at IS NOT NULL
            AND task_id IN (SELECT value FROM json_each(?1))",
    )?;
    let rows = statement.query_map(
        [super::children::ids_json(tasks.iter().map(TaskId::as_str))],
        |row| Ok(TaskId::from_raw(row.get::<_, String>(0)?)),
    )?;
    rows.collect::<rusqlite::Result<_>>().map_err(Into::into)
}
