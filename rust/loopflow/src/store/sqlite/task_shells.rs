//! Desktop shell identity survives the viewer; Processes own execution evidence.
use rusqlite::{params, OptionalExtension};

use crate::durable::TaskId;
use crate::id::LfProcessId;
use crate::store::{StoreError, StoreResult};

use super::SqliteStore;

#[derive(Debug, Clone)]
pub struct TaskShell {
    pub id: String,
    pub task_id: TaskId,
    pub checkout: String,
    pub owner: Option<LfProcessId>,
    pub close_requested: bool,
}

impl SqliteStore {
    pub fn task_shell(&self, id: &str) -> StoreResult<Option<TaskShell>> {
        let conn = self.conn.lock().expect("store mutex poisoned");
        Ok(conn
            .query_row(
                "SELECT id,task_id,checkout,owner,close_requested FROM task_shells WHERE id=?1",
                [id],
                read_shell,
            )
            .optional()?)
    }

    pub fn task_shells(&self, task: &TaskId) -> StoreResult<Vec<TaskShell>> {
        let conn = self.conn.lock().expect("store mutex poisoned");
        let mut query =
            conn.prepare("SELECT id,task_id,checkout,owner,close_requested FROM task_shells WHERE task_id=?1 ORDER BY created_at,id")?;
        let rows = query
            .query_map([task.as_str()], read_shell)?
            .collect::<rusqlite::Result<_>>()?;
        Ok(rows)
    }

    /// Only the caller that inserts the receipt may start execution. An uncertain
    /// launch remains discoverable and is never retried as a fresh shell.
    pub(crate) fn create_task_shell(
        &self,
        id: &str,
        task: &TaskId,
        checkout: &str,
    ) -> StoreResult<bool> {
        let conn = self.conn.lock().expect("store mutex poisoned");
        Ok(conn.execute("INSERT INTO task_shells(id,task_id,checkout,created_at) VALUES(?1,?2,?3,unixepoch()) ON CONFLICT(id) DO NOTHING", params![id,task.as_str(),checkout])? == 1)
    }

    pub(crate) fn bind_task_shell(&self, id: &str, owner: &LfProcessId) -> StoreResult<()> {
        let conn = self.conn.lock().expect("store mutex poisoned");
        if conn.execute(
            "UPDATE task_shells SET owner=?2,jobs_settled=0 WHERE id=?1 AND owner IS NULL AND close_requested=0",
            params![id, owner],
        )? != 1
        {
            return Err(StoreError::InvalidData(
                "Shell creation was already consumed or closed".into(),
            ));
        }
        Ok(())
    }

    pub(crate) fn shell_owner(&self, owner: &LfProcessId) -> StoreResult<Option<i64>> {
        let conn = self.conn.lock().expect("store mutex poisoned");
        Ok(conn
            .query_row(
                "SELECT close_requested FROM task_shells WHERE owner=?1",
                [owner],
                |r| r.get(0),
            )
            .optional()?)
    }

    /// A shell may have hosted earlier conversations. Only their still-live
    /// providers, or a Graph running underneath it, hold current requested work.
    pub(crate) fn shell_holds_work(&self, owner: &LfProcessId) -> StoreResult<bool> {
        let conn = self.conn.lock().expect("store mutex poisoned");
        Ok(conn.query_row("WITH RECURSIVE ancestors(id) AS (
            SELECT p.parent_lf_process_id FROM graph_processes g JOIN processes p ON p.id=g.lf_process_id WHERE p.completed_at IS NULL
            UNION SELECT p.parent_lf_process_id FROM processes p JOIN ancestors a ON p.id=a.id)
            SELECT ?1 IN (SELECT id FROM ancestors WHERE id IS NOT NULL)
            OR EXISTS(SELECT 1 FROM agent_sessions s JOIN processes p ON p.id=s.agent_process_id
                WHERE s.transport_lf_process_id=?1 AND s.completed_at IS NULL AND p.completed_at IS NULL)", [owner], |r| r.get(0))?)
    }

    pub(crate) fn settle_shell_jobs(&self, owner: &LfProcessId) -> StoreResult<()> {
        self.conn.lock().expect("store mutex poisoned").execute(
            "UPDATE task_shells SET jobs_settled=1 WHERE owner=?1",
            [owner],
        )?;
        Ok(())
    }

    pub(crate) fn task_shell_jobs_settled(&self, task: &TaskId) -> StoreResult<bool> {
        Ok(self.conn.lock().expect("store mutex poisoned").query_row(
            "SELECT NOT EXISTS(SELECT 1 FROM task_shells WHERE task_id=?1 AND jobs_settled=0)",
            [task.as_str()],
            |r| r.get(0),
        )?)
    }

    pub(crate) fn close_task_shell(&self, id: &str) -> StoreResult<()> {
        self.conn
            .lock()
            .expect("store mutex poisoned")
            .execute("UPDATE task_shells SET close_requested=1,jobs_settled=CASE WHEN owner IS NULL THEN 1 ELSE jobs_settled END WHERE id=?1", [id])?;
        Ok(())
    }

    pub(crate) fn task_shell_owners(&self, task: &TaskId) -> StoreResult<Vec<LfProcessId>> {
        let conn = self.conn.lock().expect("store mutex poisoned");
        let mut query =
            conn.prepare("SELECT owner FROM task_shells WHERE task_id=?1 AND owner IS NOT NULL")?;
        let rows = query
            .query_map([task.as_str()], |r| r.get(0))?
            .collect::<rusqlite::Result<_>>()?;
        Ok(rows)
    }

    pub(crate) fn close_task_shells(&self, task: &TaskId) -> StoreResult<()> {
        self.conn.lock().expect("store mutex poisoned").execute(
            "UPDATE task_shells SET close_requested=2,jobs_settled=CASE WHEN owner IS NULL THEN 1 ELSE jobs_settled END WHERE task_id=?1 AND close_requested=0",
            [task.as_str()],
        )?;
        Ok(())
    }
}

fn read_shell(row: &rusqlite::Row<'_>) -> rusqlite::Result<TaskShell> {
    Ok(TaskShell {
        id: row.get(0)?,
        task_id: TaskId::from_raw(row.get::<_, String>(1)?),
        checkout: row.get(2)?,
        owner: row.get(3)?,
        close_requested: row.get(4)?,
    })
}
