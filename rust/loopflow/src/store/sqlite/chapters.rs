use rusqlite::{params, TransactionBehavior};

use crate::durable::TaskId;
use crate::id::WaveId;
use crate::ops::chapter::TaskStartEvidence;
use crate::store::{StoreError, StoreResult};
use crate::work::project::ProjectId;

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

    pub fn move_chapter_task(&self, task: &TaskId, project: &ProjectId) -> StoreResult<()> {
        let mut conn = self.conn.lock().expect("store mutex poisoned");
        let tx = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
        let changed = tx.execute(
            "UPDATE tasks SET project_id=?2,updated_at=?3 WHERE id=?1 AND
             (SELECT wave_id FROM projects WHERE id=tasks.project_id) =
             (SELECT wave_id FROM projects WHERE id=?2)",
            params![
                task.as_str(),
                project.as_str(),
                super::super::rows::now_unix()
            ],
        )?;
        if changed != 1 {
            return Err(StoreError::InvalidData(
                "chapter transfer requires a Task and successor in the same Wave".into(),
            ));
        }
        tx.commit()?;
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
            // Legacy Starts have not all been imported as Runs. They remain
            // retirement evidence, never a second definition of current Started.
            "SELECT EXISTS(SELECT 1 FROM tasks WHERE id=?1 AND started_at IS NOT NULL)
                 OR EXISTS(SELECT 1 FROM task_events WHERE task_id=?1
                    AND json_extract(kind_json,'$.kind')='started')",
            [task.as_str()],
            |row| row.get(0),
        )?;
        let (abandoned, completed): (bool, bool) = conn.query_row(
            &format!(
                "SELECT state='abandoned',state='done' FROM (SELECT {} AS state FROM tasks t WHERE t.id=?1)",
                super::durable::task_state_sql("t")
            ),
            [task.as_str()],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )?;
        Ok(TaskStartEvidence {
            begun,
            authored: None,
            published: false,
            abandoned,
            completed,
        })
    }

    /// Retire backlog under the same write lock as Flow launch. Return whether
    /// it is retired, preserving the original terminal time on retries. If
    /// execution won the race, the caller reclassifies it and transfers it.
    pub fn retire_chapter_backlog(&self, task: &TaskId) -> StoreResult<bool> {
        let mut conn = self.conn.lock().expect("store mutex poisoned");
        let tx = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
        tx.execute(
            &format!(
                "UPDATE tasks AS t SET abandoned_at=?2 WHERE t.id=?1 AND {}
                 AND t.started_at IS NULL
                 AND NOT EXISTS(SELECT 1 FROM task_prs WHERE task_id=?1
                    AND (publication_requested_at IS NOT NULL OR merge_commit IS NOT NULL))
                 AND NOT EXISTS(SELECT 1 FROM task_events WHERE task_id=?1
                    AND json_extract(kind_json,'$.kind')='started')",
                super::durable::task_open_sql("t")
            ),
            params![task.as_str(), super::super::rows::now_unix()],
        )?;
        let retired = tx.query_row(
            "SELECT EXISTS(SELECT 1 FROM tasks WHERE id=?1 AND abandoned_at IS NOT NULL)",
            [task.as_str()],
            |row| row.get(0),
        )?;
        tx.commit()?;
        Ok(retired)
    }
}
