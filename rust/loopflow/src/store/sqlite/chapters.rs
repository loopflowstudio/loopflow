use rusqlite::params;

use crate::durable::TaskId;
use crate::id::WaveId;
use crate::ops::chapter::TaskStartEvidence;
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

    pub fn chapter_task_evidence(&self, task: &TaskId) -> StoreResult<TaskStartEvidence> {
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
        // Begun follows the Task's Graph: past `start`, or approved to ship.
        let (abandoned, completed, active): (bool, bool, bool) = conn.query_row(
            &format!(
                "SELECT state='abandoned',state='done',t.shipped_at IS NOT NULL OR EXISTS(SELECT 1 FROM task_graphs tg WHERE tg.task_id=t.id AND NOT (tg.step='start' AND tg.lf_process_id IS NULL)) FROM (SELECT {} AS state, t.id, t.shipped_at FROM tasks t WHERE t.id=?1) t",
                super::durable::task_state_sql("t")
            ),
            [task.as_str()],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
        )?;
        Ok(TaskStartEvidence {
            begun: begun || active,
            authored: None,
            published: false,
            abandoned,
            completed,
        })
    }
}

/// The one definition of a started Task that Work surfaces show: a current
/// owner, or a published PR on its current delivery.
pub(super) fn task_started_or_published_in(
    conn: &rusqlite::Connection,
    task: &TaskId,
) -> StoreResult<bool> {
    Ok(super::task_ownership::owned_in(conn, task)?
        || conn.query_row(
            "SELECT EXISTS(SELECT 1 FROM task_prs WHERE task_id=?1 AND historical=0
                AND publication_requested_at IS NOT NULL)",
            [task.as_str()],
            |row| row.get(0),
        )?)
}
