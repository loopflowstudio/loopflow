use std::collections::HashSet;

use rusqlite::params;

use crate::durable::TaskId;
use crate::id::WaveId;
use crate::store::StoreResult;

use super::SqliteStore;

impl SqliteStore {
    pub fn projects_pending_adoption(&self, wave: &WaveId) -> StoreResult<Vec<(String, i64)>> {
        let conn = self.conn.lock().expect("store mutex poisoned");
        let mut query = conn.prepare(
            "SELECT external_project_id, legacy_current FROM projects
             WHERE wave_id=?1 AND legacy_current IS NOT NULL ORDER BY external_project_id",
        )?;
        let rows = query.query_map([wave.as_str()], |row| Ok((row.get(0)?, row.get(1)?)))?;
        rows.collect::<Result<Vec<_>, _>>().map_err(Into::into)
    }

    pub fn finish_project_adoption(&self, wave: &WaveId, project: &str) -> StoreResult<()> {
        let conn = self.conn.lock().expect("store mutex poisoned");
        conn.execute(
            "UPDATE projects SET legacy_current=NULL WHERE wave_id=?1 AND external_project_id=?2",
            params![wave.as_str(), project],
        )?;
        Ok(())
    }

    pub fn task_started_or_published(&self, task: &TaskId) -> StoreResult<bool> {
        let mut conn = self.conn.lock().expect("store mutex poisoned");
        let snapshot = conn.savepoint()?;
        task_started_or_published_in(&snapshot, task)
    }

    /// Whether work on the Task began: an owner, a started event, a Session or
    /// Graph step in its checkout, a Graph past `start`, or approval to ship.
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
                 OR EXISTS(SELECT 1 FROM graph_process_steps WHERE graph_lf_process_id IN ({}))",
                super::task_work::session_ids("?1"),
                super::task_work::lf_process_ids(&conn, "?1")?,
            ),
            [task.as_str()],
            |row| row.get(0),
        )?;
        if begun {
            return Ok(true);
        }
        conn.query_row(
            "SELECT t.shipped_at IS NOT NULL OR EXISTS(SELECT 1 FROM task_graphs tg WHERE tg.task_id=t.id AND NOT (tg.step='start' AND tg.lf_process_id IS NULL)) FROM tasks t WHERE t.id=?1",
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
