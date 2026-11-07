//! Desktop shell identity survives the viewer; Processes own execution evidence.
use rusqlite::{params, OptionalExtension};

use crate::durable::TaskId;
use crate::id::LfProcessId;
use crate::store::sqlite::SqliteStore;
use crate::store::{StoreError, StoreResult};

#[derive(Debug, Clone)]
pub(crate) struct DesktopShell {
    pub checkout: String,
    pub owner: Option<LfProcessId>,
}

impl SqliteStore {
    pub(crate) fn desktop_shell(&self, id: &str) -> StoreResult<Option<DesktopShell>> {
        let conn = self.conn.lock().expect("store mutex poisoned");
        Ok(conn
            .query_row(
                "SELECT checkout,owner FROM desktop_shells WHERE id=?1",
                [id],
                |row| {
                    Ok(DesktopShell {
                        checkout: row.get(0)?,
                        owner: row.get(1)?,
                    })
                },
            )
            .optional()?)
    }

    /// Only the caller that inserts the receipt may start execution. An uncertain
    /// launch remains discoverable and is never retried as a fresh shell.
    pub(crate) fn create_desktop_shell(
        &self,
        id: &str,
        task: &TaskId,
        checkout: &str,
    ) -> StoreResult<bool> {
        let conn = self.conn.lock().expect("store mutex poisoned");
        Ok(conn.execute("INSERT INTO desktop_shells(id,task_id,checkout,created_at) VALUES(?1,?2,?3,unixepoch()) ON CONFLICT(id) DO NOTHING", params![id,task.as_str(),checkout])? == 1)
    }

    pub(crate) fn bind_desktop_shell(&self, id: &str, owner: &LfProcessId) -> StoreResult<()> {
        let conn = self.conn.lock().expect("store mutex poisoned");
        if conn.execute(
            "UPDATE desktop_shells SET owner=?2,jobs_settled=0 WHERE id=?1 AND owner IS NULL AND close_requested=0",
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
                "SELECT close_requested FROM desktop_shells WHERE owner=?1",
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
            "UPDATE desktop_shells SET jobs_settled=1 WHERE owner=?1",
            [owner],
        )?;
        Ok(())
    }

    pub(crate) fn close_desktop_shell(&self, id: &str) -> StoreResult<()> {
        self.conn
            .lock()
            .expect("store mutex poisoned")
            .execute("UPDATE desktop_shells SET close_requested=1,jobs_settled=CASE WHEN owner IS NULL THEN 1 ELSE jobs_settled END WHERE id=?1", [id])?;
        Ok(())
    }
}

pub(crate) fn jobs_settled(store: &SqliteStore, task: &TaskId) -> StoreResult<bool> {
    Ok(store.conn.lock().expect("store mutex poisoned").query_row(
        "SELECT NOT EXISTS(SELECT 1 FROM desktop_shells WHERE task_id=?1 AND jobs_settled=0)",
        [task.as_str()],
        |r| r.get(0),
    )?)
}

pub(crate) fn owners(store: &SqliteStore, task: &TaskId) -> StoreResult<Vec<LfProcessId>> {
    let conn = store.conn.lock().expect("store mutex poisoned");
    let mut query =
        conn.prepare("SELECT owner FROM desktop_shells WHERE task_id=?1 AND owner IS NOT NULL")?;
    let rows = query
        .query_map([task.as_str()], |r| r.get(0))?
        .collect::<rusqlite::Result<_>>()?;
    Ok(rows)
}

pub(crate) fn request_retirement(store: &SqliteStore, task: &TaskId) -> StoreResult<()> {
    store.conn.lock().expect("store mutex poisoned").execute(
        "UPDATE desktop_shells SET close_requested=2,jobs_settled=CASE WHEN owner IS NULL THEN 1 ELSE jobs_settled END WHERE task_id=?1 AND close_requested=0",
        [task.as_str()],
    )?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use crate::durable::TaskId;
    use crate::store::sqlite::{SqliteStore, ViewerHold};

    #[test]
    fn desktop_shell_creation_is_once_even_after_lost_acknowledgement_or_exit() {
        let home = tempfile::tempdir().unwrap();
        let store = SqliteStore::open_ephemeral(&home.path().join("loopflow.db")).unwrap();
        let repo = store.ensure_repository("/repo").unwrap();
        let task = TaskId::new();
        store.conn.lock().unwrap().execute(
            "INSERT INTO tasks(id,repository_id,issue_identifier,created_at,worktree,branch) VALUES(?1,?2,'SHELL-1',1,'/repo.task','task')",
            rusqlite::params![task.as_str(),repo.as_str()],
        ).unwrap();
        assert!(store
            .create_desktop_shell("shell", &task, "/repo.task")
            .unwrap());
        assert!(!store
            .create_desktop_shell("shell", &task, "/repo.task")
            .unwrap());
        assert!(store
            .desktop_shell("shell")
            .unwrap()
            .unwrap()
            .owner
            .is_none());
        let owner = store.test_lf_process();
        store.bind_desktop_shell("shell", &owner).unwrap();
        assert!(store.bind_desktop_shell("shell", &owner).is_err());
        assert_eq!(store.shell_owner(&owner).unwrap(), Some(0));
        // Desktop shell ownership adds no checkout hold; the Task Session supplies it.
        store
            .record_owner_viewer_hold(&owner, Some(ViewerHold::Viewed(100)))
            .unwrap();
        assert!(store.task_viewer_holds(&task).unwrap().is_empty());
        store.record_owner_viewer_hold(&owner, None).unwrap();
        store.close_desktop_shell("shell").unwrap();
        assert_eq!(store.shell_owner(&owner).unwrap(), Some(1));
        assert!(!store
            .create_desktop_shell("shell", &task, "/repo.task")
            .unwrap());
        assert_eq!(
            store.desktop_shell("shell").unwrap().unwrap().owner,
            Some(owner.clone())
        );
        store.settle_shell_jobs(&owner).unwrap();
        assert!(store
            .create_desktop_shell("unacknowledged", &task, "/repo.task")
            .unwrap());
        store.close_desktop_shell("unacknowledged").unwrap();
        assert!(store
            .bind_desktop_shell("unacknowledged", &store.test_lf_process())
            .is_err());
        assert!(crate::desktop::shells::jobs_settled(&store, &task).unwrap());
    }
}
