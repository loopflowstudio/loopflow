use rusqlite::{params, OptionalExtension, TransactionBehavior};

use crate::durable::TaskId;
use crate::id::WaveId;
use crate::store::{StoreError, StoreResult};
use crate::work::chapter::{Chapter, ChapterId, TaskStartEvidence};
use crate::work::project::ProjectId;

use super::SqliteStore;

impl SqliteStore {
    pub fn chapter(&self, wave: &WaveId, id: Option<&ChapterId>) -> StoreResult<Option<Chapter>> {
        let conn = self.conn.lock().expect("store mutex poisoned");
        let sql = if id.is_some() {
            "SELECT receipt FROM wave_chapters WHERE wave_id=?1 AND chapter_id=?2"
        } else {
            "SELECT receipt FROM wave_chapters WHERE wave_id=?1 AND current=1 AND ?2 IS NULL"
        };
        let value: Option<String> = conn
            .query_row(
                sql,
                params![wave.as_str(), id.map(ChapterId::as_str)],
                |row| row.get(0),
            )
            .optional()?;
        value
            .map(|value| serde_json::from_str(&value).map_err(StoreError::from))
            .transpose()
    }

    pub fn chapters(&self, wave: &WaveId) -> StoreResult<Vec<Chapter>> {
        let conn = self.conn.lock().expect("store mutex poisoned");
        let mut query =
            conn.prepare("SELECT receipt FROM wave_chapters WHERE wave_id=?1 ORDER BY rowid")?;
        let rows = query.query_map([wave.as_str()], |row| row.get::<_, String>(0))?;
        rows.map(|row| Ok(serde_json::from_str(&row?)?)).collect()
    }

    pub fn save_chapter(&self, chapter: &Chapter, activate: bool) -> StoreResult<()> {
        let mut conn = self.conn.lock().expect("store mutex poisoned");
        let tx = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
        if activate {
            tx.execute(
                "UPDATE wave_chapters SET current=0 WHERE wave_id=?1",
                [chapter.wave_id.as_str()],
            )?;
        }
        tx.execute(
            "INSERT INTO wave_chapters(wave_id,chapter_id,project_id,current,receipt) VALUES(?1,?2,?3,?4,?5)
             ON CONFLICT(wave_id,chapter_id) DO UPDATE SET receipt=excluded.receipt,
             current=CASE WHEN ?4 THEN 1 ELSE wave_chapters.current END",
            params![chapter.wave_id.as_str(), chapter.id.as_str(), chapter.project_id, activate, serde_json::to_string(chapter)?],
        )?;
        tx.commit()?;
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

    pub fn begin_chapter_task(&self, task: &TaskId) -> StoreResult<()> {
        let mut conn = self.conn.lock().expect("store mutex poisoned");
        let tx = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
        let work = crate::durable::WorkRef::Task(task.clone());
        super::durable::require_ready_work(&tx, &work)?;
        super::durable::require_current_task_chapter(&tx, &work)?;
        tx.execute("INSERT INTO task_events(task_id,kind_json,created_at)
            SELECT ?1,'{\"kind\":\"started\"}',?2 WHERE NOT EXISTS(
              SELECT 1 FROM task_events WHERE task_id=?1 AND json_extract(kind_json,'$.kind')='started')",
            params![task.as_str(), super::super::rows::now_unix()])?;
        tx.commit()?;
        Ok(())
    }

    /// Durable evidence that execution began: a recorded Started, a worker
    /// report or finished Flow, or a claimed worker generation. Preparing a
    /// checkout or an unopened human Run records none of these.
    pub fn task_started(&self, task: &TaskId) -> StoreResult<bool> {
        let conn = self.conn.lock().expect("store mutex poisoned");
        Ok(conn.query_row(
            "SELECT EXISTS(SELECT 1 FROM task_events WHERE task_id=?1 AND json_extract(kind_json,'$.kind')
               IN ('started','progress','body_handed_off','flow_finished'))
             OR EXISTS(SELECT 1 FROM task_flow_positions WHERE task_id=?1 AND worker_generation>0)",
            [task.as_str()],
            |row| row.get(0),
        )?)
    }

    pub fn chapter_task_evidence(&self, task: &TaskId) -> StoreResult<TaskStartEvidence> {
        let conn = self.conn.lock().expect("store mutex poisoned");
        let begun: bool = conn.query_row(
            "SELECT EXISTS(SELECT 1 FROM task_events WHERE task_id=?1 AND json_extract(kind_json,'$.kind')='started')
             OR EXISTS(SELECT 1 FROM task_flow_positions WHERE task_id=?1 AND (worker_generation>0 OR session_run_id IS NOT NULL))",
            [task.as_str()], |row| row.get(0),
        )?;
        let claimed: bool = conn.query_row("SELECT EXISTS(SELECT 1 FROM task_flow_positions WHERE task_id=?1 AND claim_json IS NOT NULL)", [task.as_str()], |row| row.get(0))?;
        let (abandoned, completed): (bool, bool) = conn.query_row(
            "SELECT work_state='abandoned',work_state='done' FROM tasks WHERE id=?1",
            [task.as_str()],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )?;
        Ok(TaskStartEvidence {
            begun,
            worker_claimed: claimed,
            authored: None,
            published: false,
            abandoned,
            completed,
        })
    }

    /// Retire backlog under the same write lock as worker claims. If execution
    /// won the race, the caller reclassifies it as started and transfers it.
    pub fn retire_chapter_backlog(&self, task: &TaskId) -> StoreResult<bool> {
        let mut conn = self.conn.lock().expect("store mutex poisoned");
        let tx = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
        let changed = tx.execute(
            "UPDATE tasks SET work_state='abandoned',work_terminal_at=?2 WHERE id=?1 AND work_state='ready'
             AND NOT EXISTS(SELECT 1 FROM task_events WHERE task_id=?1 AND json_extract(kind_json,'$.kind')='started')
             AND NOT EXISTS(SELECT 1 FROM task_flow_positions WHERE task_id=?1 AND (worker_generation>0 OR claim_json IS NOT NULL OR session_run_id IS NOT NULL))",
            params![task.as_str(), super::super::rows::now_unix()],
        )?;
        tx.commit()?;
        Ok(changed == 1)
    }
}

#[cfg(test)]
mod tests {
    use crate::id::WaveId;
    use crate::ops::chapter::empty_plan;
    use crate::store::sqlite::SqliteStore;
    use crate::work::chapter::{Chapter, ChapterId, ChapterPhase};
    use crate::work::wave::Wave;

    #[test]
    fn chapter_cutover_keeps_one_current_plan_and_preserves_history() {
        let directory = tempfile::tempdir().unwrap();
        let store = SqliteStore::new(&directory.path().join("chapters.db")).unwrap();
        let wave = Wave::new(
            WaveId::new(),
            "product".into(),
            directory.path().display().to_string(),
        );
        store.create_wave(&wave).unwrap();
        let mut first = Chapter {
            predecessor_metrics: Vec::new(),
            id: ChapterId::parse("one").unwrap(),
            wave_id: wave.id().clone(),
            wave: "product".into(),
            project_id: "external-one".into(),
            content: empty_plan(),
            predecessors: vec![],
            tasks: vec![],
            phase: ChapterPhase::Preparing,
            created_at: 1,
            activated_at: None,
            completed_at: None,
            error: None,
        };
        store.save_chapter(&first, false).unwrap();
        assert!(store.chapter(wave.id(), None).unwrap().is_none());
        first.activated_at = Some(2);
        first.phase = ChapterPhase::Complete;
        first.completed_at = Some(2);
        store.save_chapter(&first, true).unwrap();
        let mut second = first.clone();
        second.id = ChapterId::parse("two").unwrap();
        second.project_id = "external-two".into();
        second.phase = ChapterPhase::Transferring;
        second.completed_at = None;
        store.save_chapter(&second, true).unwrap();
        store.save_chapter(&second, false).unwrap();
        assert_eq!(store.chapter(wave.id(), None).unwrap().unwrap(), second);
        assert_eq!(
            store.chapter(wave.id(), Some(&first.id)).unwrap().unwrap(),
            first
        );
        assert_eq!(store.chapters(wave.id()).unwrap().len(), 2);
        let mut competing = second.clone();
        competing.id = ChapterId::parse("three").unwrap();
        competing.project_id = "external-three".into();
        assert!(store.save_chapter(&competing, true).is_err());
        assert_eq!(store.chapter(wave.id(), None).unwrap().unwrap(), second);
    }
}
