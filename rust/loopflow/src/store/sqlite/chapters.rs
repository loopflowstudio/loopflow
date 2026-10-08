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

    /// A Task is started by its first Run, an unpublished reservation included.
    pub fn task_started(&self, task: &TaskId) -> StoreResult<bool> {
        let conn = self.conn.lock().expect("store mutex poisoned");
        Ok(conn.query_row(
            "SELECT EXISTS(SELECT 1 FROM tasks WHERE id=?1 AND started_at IS NOT NULL)",
            [task.as_str()],
            |row| row.get(0),
        )?)
    }

    pub fn chapter_task_evidence(&self, task: &TaskId) -> StoreResult<TaskStartEvidence> {
        let conn = self.conn.lock().expect("store mutex poisoned");
        let begun: bool = conn.query_row(
            // Checkout membership is preservation evidence even when a
            // conversation has no explicit Task binding or Started timestamp.
            &format!(
                "SELECT EXISTS(SELECT 1 FROM tasks WHERE id=?1 AND started_at IS NOT NULL)
                 OR EXISTS(SELECT 1 FROM task_events WHERE task_id=?1
                    AND json_extract(kind_json,'$.kind')='started')
                 OR EXISTS({})
                 OR EXISTS(SELECT 1 FROM flow_process_steps WHERE flow_process_lfid IN ({}))",
                super::task_work::session_ids("?1"),
                super::task_work::process_lfids("?1"),
            ),
            [task.as_str()],
            |row| row.get(0),
        )?;
        let (abandoned, completed, active): (bool, bool, bool) = conn.query_row(
            &format!(
                "SELECT state='abandoned',state='done',state='active' FROM (SELECT {} AS state FROM tasks t WHERE t.id=?1)",
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
