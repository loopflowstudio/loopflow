//! Desktop shell identity survives the viewer; its held terminal owns execution.
//! A shell an earlier build started names its `lf session serve` owner instead;
//! that owner ends itself on Close and is never viewed again.
use rusqlite::{params, OptionalExtension};

use crate::durable::TaskId;
use crate::id::{LfProcessId, TerminalId};
use crate::store::sqlite::SqliteStore;
use crate::store::{StoreError, StoreResult};

#[derive(Debug, Clone)]
pub(crate) struct DesktopShell {
    pub checkout: String,
    /// The terminal that runs the shell; none until its start is acknowledged.
    pub terminal: Option<TerminalId>,
    /// Its start was acknowledged, by a terminal or an earlier build's owner.
    pub started: bool,
}

impl SqliteStore {
    pub(crate) fn desktop_shell(&self, id: &str) -> StoreResult<Option<DesktopShell>> {
        let conn = self.conn.lock().expect("store mutex poisoned");
        Ok(conn
            .query_row(
                "SELECT checkout,terminal_id,terminal_id IS NOT NULL OR owner IS NOT NULL
                 FROM desktop_shells WHERE id=?1",
                [id],
                |row| {
                    Ok(DesktopShell {
                        checkout: row.get(0)?,
                        terminal: row.get(1)?,
                        started: row.get(2)?,
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
        launcher: Option<&LfProcessId>,
    ) -> StoreResult<bool> {
        let conn = self.conn.lock().expect("store mutex poisoned");
        Ok(conn.execute(
            "INSERT INTO desktop_shells(id,task_id,checkout,launcher_lf_process_id,created_at)
             VALUES(?1,?2,?3,?4,unixepoch()) ON CONFLICT(id) DO NOTHING",
            params![id, task.as_str(), checkout, launcher],
        )? == 1)
    }

    pub(crate) fn bind_desktop_shell(&self, id: &str, terminal: &TerminalId) -> StoreResult<()> {
        let conn = self.conn.lock().expect("store mutex poisoned");
        if conn.execute(
            "UPDATE desktop_shells SET terminal_id=?2 WHERE id=?1
             AND terminal_id IS NULL AND owner IS NULL AND jobs_settled=0",
            params![id, terminal],
        )? != 1
        {
            return Err(StoreError::InvalidData(
                "Shell creation was already consumed or closed".into(),
            ));
        }
        Ok(())
    }

    /// Whether `terminal` runs a Desktop shell.
    pub(crate) fn is_shell_terminal(&self, terminal: &TerminalId) -> StoreResult<bool> {
        Ok(self.conn.lock().expect("store mutex poisoned").query_row(
            "SELECT EXISTS(SELECT 1 FROM desktop_shells WHERE terminal_id=?1)",
            [terminal],
            |r| r.get(0),
        )?)
    }

    /// The shell's terminal ended and nothing it started is left running.
    pub(crate) fn settle_shell_jobs(&self, terminal: &TerminalId) -> StoreResult<()> {
        self.conn.lock().expect("store mutex poisoned").execute(
            "UPDATE desktop_shells SET jobs_settled=1 WHERE terminal_id=?1",
            [terminal],
        )?;
        Ok(())
    }

    pub(crate) fn settle_unstarted_task_shells(&self, task: &TaskId) -> StoreResult<()> {
        self.conn.lock().expect("store mutex poisoned").execute(
            "UPDATE desktop_shells SET jobs_settled=1
             WHERE task_id=?1 AND owner IS NULL AND terminal_id IS NULL",
            [task.as_str()],
        )?;
        Ok(())
    }

    /// Record Close. An earlier build's owner polls this; a held shell is
    /// ended by its caller.
    pub(crate) fn close_desktop_shell(&self, id: &str) -> StoreResult<()> {
        self.conn.lock().expect("store mutex poisoned").execute(
            "UPDATE desktop_shells SET close_requested=1,
            jobs_settled=CASE WHEN owner IS NULL AND terminal_id IS NULL THEN 1 ELSE jobs_settled END
            WHERE id=?1",
            [id],
        )?;
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

/// The terminal of each of the Task's started shells, with the `lf` that
/// created it.
pub(crate) fn hosts(
    store: &SqliteStore,
    task: &TaskId,
) -> StoreResult<Vec<(TerminalId, Option<LfProcessId>)>> {
    let conn = store.conn.lock().expect("store mutex poisoned");
    let mut query = conn.prepare(
        "SELECT terminal_id,launcher_lf_process_id FROM desktop_shells
         WHERE task_id=?1 AND terminal_id IS NOT NULL",
    )?;
    let rows = query
        .query_map([task.as_str()], |r| Ok((r.get(0)?, r.get(1)?)))?
        .collect::<rusqlite::Result<_>>()?;
    Ok(rows)
}

#[cfg(test)]
mod tests {
    use crate::durable::TaskId;
    use crate::id::TerminalId;
    use crate::store::sqlite::SqliteStore;

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
            .create_desktop_shell("shell", &task, "/repo.task", None)
            .unwrap());
        assert!(!store
            .create_desktop_shell("shell", &task, "/repo.task", None)
            .unwrap());
        assert!(!store.desktop_shell("shell").unwrap().unwrap().started);
        let terminal = TerminalId::new();
        store.create_terminal(&terminal, "build").unwrap();
        store.bind_desktop_shell("shell", &terminal).unwrap();
        assert!(store.bind_desktop_shell("shell", &terminal).is_err());
        assert!(store.is_shell_terminal(&terminal).unwrap());
        store.close_desktop_shell("shell").unwrap();
        assert_eq!(
            store.desktop_shell("shell").unwrap().unwrap().terminal,
            Some(terminal.clone())
        );
        // Closing a running shell settles nothing until its terminal ends.
        assert!(!crate::desktop::shells::jobs_settled(&store, &task).unwrap());
        store.settle_shell_jobs(&terminal).unwrap();
        assert!(store
            .create_desktop_shell("unacknowledged", &task, "/repo.task", None)
            .unwrap());
        store.close_desktop_shell("unacknowledged").unwrap();
        assert!(store
            .bind_desktop_shell("unacknowledged", &TerminalId::new())
            .is_err());
        assert!(crate::desktop::shells::jobs_settled(&store, &task).unwrap());
    }
}
