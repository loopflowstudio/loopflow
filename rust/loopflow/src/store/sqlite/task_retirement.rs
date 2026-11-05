//! Session-viewer holds and one cleanup intent pinned to a Task's completion.
use rusqlite::{params, Connection, OptionalExtension};

use super::SqliteStore;
use crate::durable::TaskId;
use crate::id::LfProcessId;
use crate::store::{StoreError, StoreResult};

pub(crate) const WORKSPACE_GRACE: i64 = 15 * 60;

/// What a serving owner knows about its own viewers, recorded on its Process.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ViewerHold {
    /// An attached viewer has not left: open, or inside its reconnect grace.
    Viewed,
    /// An SSH viewer dropped without leaving; its LF process may still be alive.
    Unknown,
}

impl ViewerHold {
    fn as_str(self) -> &'static str {
        match self {
            Self::Viewed => "viewed",
            Self::Unknown => "unknown",
        }
    }
}

/// One owner's recorded hold on a Task. `ended` owners hold only when unknown.
#[derive(Debug)]
pub(crate) struct OwnerHold {
    pub owner: LfProcessId,
    pub hold: ViewerHold,
    pub ended: bool,
}

#[derive(Debug, Clone)]
pub struct TaskRetirement {
    pub task_id: TaskId,
    pub completion: String,
    pub checkout: String,
    pub branch: String,
    pub due_at: Option<i64>,
    pub phase: String,
    pub reason: Option<String>,
}

impl SqliteStore {
    pub(crate) fn record_owner_viewer_hold(
        &self,
        owner: &LfProcessId,
        hold: Option<ViewerHold>,
    ) -> StoreResult<()> {
        self.conn.lock().expect("store mutex poisoned").execute(
            "UPDATE processes SET viewer_hold=?2 WHERE id=?1",
            params![owner.as_str(), hold.map(ViewerHold::as_str)],
        )?;
        Ok(())
    }

    /// Holds from each Task Session's current transport owner and Desktop shell
    /// owners. A replaced owner no longer speaks for its Session.
    pub(crate) fn task_viewer_holds(&self, task: &TaskId) -> StoreResult<Vec<OwnerHold>> {
        let conn = self.conn.lock().expect("store mutex poisoned");
        let mut query = conn.prepare(&format!(
            "SELECT p.id,p.viewer_hold,p.completed_at IS NOT NULL FROM processes p
             WHERE p.viewer_hold IS NOT NULL
               AND (p.id IN (SELECT s.transport_lf_process_id FROM agent_sessions s WHERE s.id IN ({}))
                    OR p.id IN (SELECT owner FROM task_shells WHERE task_id=?1))
               AND (p.completed_at IS NULL OR p.viewer_hold='unknown')
             ORDER BY p.id",
            super::task_work::session_ids("?1")
        ))?;
        let rows = query.query_map([task.as_str()], |r| {
            Ok((
                r.get::<_, LfProcessId>(0)?,
                r.get::<_, String>(1)?,
                r.get::<_, bool>(2)?,
            ))
        })?;
        rows.map(|row| {
            let (owner, hold, ended) = row?;
            let hold = match hold.as_str() {
                "viewed" => ViewerHold::Viewed,
                "unknown" => ViewerHold::Unknown,
                other => {
                    return Err(StoreError::InvalidData(format!(
                        "Unknown viewer hold {other}"
                    )))
                }
            };
            Ok(OwnerHold { owner, hold, ended })
        })
        .collect()
    }

    /// Done Tasks whose waiting cleanup this owner's release may unblock.
    pub(crate) fn owner_waiting_retirements(
        &self,
        owner: &LfProcessId,
    ) -> StoreResult<Vec<TaskId>> {
        let conn = self.conn.lock().expect("store mutex poisoned");
        let mut query = conn.prepare(&format!(
            "SELECT r.task_id FROM task_retirements r WHERE r.phase!='retired' AND (
                EXISTS(SELECT 1 FROM agent_sessions s WHERE s.transport_lf_process_id=?1 AND s.id IN ({}))
                OR EXISTS(SELECT 1 FROM task_shells t WHERE t.owner=?1 AND t.task_id=r.task_id))",
            super::task_work::session_ids("r.task_id")
        ))?;
        let rows = query
            .query_map([owner.as_str()], |r| {
                Ok(TaskId::from_raw(r.get::<_, String>(0)?))
            })?
            .collect::<rusqlite::Result<_>>()?;
        Ok(rows)
    }

    /// `hold` names why a viewer still retains the checkout. Holds clear any
    /// deadline; release starts one fifteen-minute interval that later work
    /// completion does not restart.
    pub(crate) fn reconcile_task_retirement(
        &self,
        task: &TaskId,
        now: i64,
        hold: Option<&str>,
    ) -> StoreResult<Option<TaskRetirement>> {
        let mut conn = self.conn.lock().expect("store mutex poisoned");
        let tx = conn.transaction()?;
        tx.execute("INSERT INTO task_retirements(task_id,completion,checkout,branch,due_at,phase)
            SELECT id,planning_completed_at,worktree,branch,NULL,'waiting' FROM tasks
            WHERE id=?1 AND planning_completed=1 AND planning_completed_at IS NOT NULL AND worktree IS NOT NULL
            ON CONFLICT(task_id) DO NOTHING", [task.as_str()])?;
        match hold {
            Some(reason) => hold_retirement(&tx, task, reason)?,
            None => {
                tx.execute(
                    "UPDATE task_retirements SET due_at=COALESCE(due_at,?2+?3),reason=NULL
                     WHERE task_id=?1 AND phase='waiting'",
                    params![task.as_str(), now, WORKSPACE_GRACE],
                )?;
            }
        }
        tx.commit()?;
        drop(conn);
        self.task_retirement(task)
    }

    pub fn task_retirement(&self, task: &TaskId) -> StoreResult<Option<TaskRetirement>> {
        let conn = self.conn.lock().expect("store mutex poisoned");
        Ok(conn
            .query_row(
                "SELECT task_id,completion,checkout,branch,due_at,phase,reason
                 FROM task_retirements WHERE task_id=?1",
                [task.as_str()],
                |r| {
                    Ok(TaskRetirement {
                        task_id: TaskId::from_raw(r.get::<_, String>(0)?),
                        completion: r.get(1)?,
                        checkout: r.get(2)?,
                        branch: r.get(3)?,
                        due_at: r.get(4)?,
                        phase: r.get(5)?,
                        reason: r.get(6)?,
                    })
                },
            )
            .optional()?)
    }

    pub(crate) fn retirement_progress(
        &self,
        intent: &TaskRetirement,
        phase: &str,
        reason: Option<&str>,
    ) -> StoreResult<bool> {
        Ok(self.conn.lock().expect("store mutex poisoned").execute(
            "UPDATE task_retirements SET phase=?5,reason=?6 WHERE task_id=?1 AND completion=?2 AND checkout=?3 AND branch=?4 AND due_at IS NOT NULL",
            params![intent.task_id.as_str(),intent.completion,intent.checkout,intent.branch,phase,reason])? == 1)
    }

    pub(crate) fn owner_retiring(&self, owner: &LfProcessId) -> StoreResult<bool> {
        let conn = self.conn.lock().expect("store mutex poisoned");
        Ok(conn.query_row(&format!("SELECT
            EXISTS(SELECT 1 FROM task_retirements h JOIN agent_sessions s ON s.id IN ({}) WHERE s.transport_lf_process_id=?1 AND h.phase='closing')
            OR EXISTS(SELECT 1 FROM task_retirements h JOIN task_shells s ON s.task_id=h.task_id WHERE s.owner=?1 AND h.phase='closing')", super::task_work::session_ids("h.task_id")), [owner], |r| r.get(0))?)
    }
}

// A returning viewer cancels only retirement's queued shell closes, never
// explicit Close. Keep both writes in the caller's retirement transaction.
fn hold_retirement(conn: &Connection, task: &TaskId, reason: &str) -> StoreResult<()> {
    conn.execute(
        "UPDATE task_shells SET close_requested=0 WHERE task_id=?1 AND close_requested=2",
        [task.as_str()],
    )?;
    conn.execute(
        "UPDATE task_retirements SET due_at=NULL,phase='waiting',reason=?2
         WHERE task_id=?1 AND phase!='retired'",
        params![task.as_str(), reason],
    )?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::{SqliteStore, TaskId, ViewerHold, WORKSPACE_GRACE};
    use rusqlite::params;

    fn fixture() -> (tempfile::TempDir, SqliteStore, TaskId) {
        let home = tempfile::tempdir().unwrap();
        let store = SqliteStore::open_ephemeral(&home.path().join("loopflow.db")).unwrap();
        let repo = store.ensure_repository("/repo").unwrap();
        let task = TaskId::new();
        store.conn.lock().unwrap().execute("INSERT INTO tasks(id,repository_id,issue_identifier,created_at,worktree,branch,planning_completed,planning_completed_at) VALUES(?1,?2,'SHELL-1',1,'/repo.task','task',1,'completion-1')", params![task.as_str(),repo.as_str()]).unwrap();
        (home, store, task)
    }

    fn session_owner(store: &SqliteStore, task: &TaskId, session: &str) -> crate::id::LfProcessId {
        let owner = store.test_lf_process();
        store.test_session(session, &crate::session_record::new_artifact_key());
        store.conn.lock().unwrap().execute("UPDATE agent_sessions SET task_id=?1,interactive=1,transport_lf_process_id=?2 WHERE id=?3", params![task.as_str(),owner,session]).unwrap();
        owner
    }

    fn held(store: &SqliteStore, task: &TaskId) -> Vec<(ViewerHold, bool)> {
        store
            .task_viewer_holds(task)
            .unwrap()
            .into_iter()
            .map(|h| (h.hold, h.ended))
            .collect()
    }

    #[test]
    fn viewer_holds_read_current_session_owners_and_survive_only_as_unknown() {
        let (_home, store, task) = fixture();
        let first = session_owner(&store, &task, "first");
        let second = session_owner(&store, &task, "second");
        let unrelated = store.test_lf_process();
        store
            .record_owner_viewer_hold(&unrelated, Some(ViewerHold::Viewed))
            .unwrap();
        assert!(held(&store, &task).is_empty());
        store
            .record_owner_viewer_hold(&first, Some(ViewerHold::Viewed))
            .unwrap();
        store
            .record_owner_viewer_hold(&second, Some(ViewerHold::Viewed))
            .unwrap();
        assert_eq!(held(&store, &task).len(), 2);
        // One viewer's exit leaves the other Session's viewer holding.
        store.record_owner_viewer_hold(&first, None).unwrap();
        assert_eq!(held(&store, &task), vec![(ViewerHold::Viewed, false)]);
        // A cleanly ended owner told its viewers; only a lost SSH viewer survives it.
        let conn = store.conn.lock().unwrap();
        conn.execute("UPDATE processes SET completed_at=1 WHERE id=?1", [&second])
            .unwrap();
        drop(conn);
        assert!(held(&store, &task).is_empty());
        store
            .record_owner_viewer_hold(&second, Some(ViewerHold::Unknown))
            .unwrap();
        assert_eq!(held(&store, &task), vec![(ViewerHold::Unknown, true)]);
        // A replacement owner speaks for the Session; the old one no longer does.
        let replacement = store.test_lf_process();
        store
            .conn
            .lock()
            .unwrap()
            .execute(
                "UPDATE agent_sessions SET transport_lf_process_id=?1 WHERE id='second'",
                [&replacement],
            )
            .unwrap();
        assert!(held(&store, &task).is_empty());
        assert_eq!(
            store.owner_waiting_retirements(&replacement).unwrap(),
            vec![]
        );
        store.reconcile_task_retirement(&task, 100, None).unwrap();
        assert_eq!(
            store.owner_waiting_retirements(&replacement).unwrap(),
            vec![task.clone()]
        );
        assert!(store
            .owner_waiting_retirements(&unrelated)
            .unwrap()
            .is_empty());
    }

    #[test]
    fn workspace_retirement_holds_cancel_deadlines_and_work_does_not_restart_them() {
        let (_home, store, task) = fixture();
        let held = store
            .reconcile_task_retirement(&task, 100, Some("Retained by an open Task Session viewer"))
            .unwrap()
            .unwrap();
        assert_eq!(held.due_at, None);
        let left = store
            .reconcile_task_retirement(&task, 110, None)
            .unwrap()
            .unwrap();
        assert_eq!(left.due_at, Some(110 + WORKSPACE_GRACE));
        // The two-minute agent policy does not add two minutes to this deadline.
        assert_eq!(
            store
                .reconcile_task_retirement(&task, 230, None)
                .unwrap()
                .unwrap()
                .due_at,
            left.due_at
        );
        store
            .create_task_shell("shell", &task, "/repo.task")
            .unwrap();
        store.close_task_shells(&task).unwrap();
        let returned = store
            .reconcile_task_retirement(&task, 900, Some("Viewer liveness unknown; cleanup pending"))
            .unwrap()
            .unwrap();
        assert_eq!(returned.due_at, None);
        assert!(returned.reason.unwrap().contains("unknown"));
        assert!(!store.task_shell("shell").unwrap().unwrap().close_requested);
        assert_eq!(
            store
                .reconcile_task_retirement(&task, 920, None)
                .unwrap()
                .unwrap()
                .due_at,
            Some(920 + WORKSPACE_GRACE)
        );
    }

    #[test]
    fn workspace_retirement_reopen_and_checkout_replacement_revoke_queued_work() {
        let (_home, store, task) = fixture();
        let old = store
            .reconcile_task_retirement(&task, 100, None)
            .unwrap()
            .unwrap();
        store
            .conn
            .lock()
            .unwrap()
            .execute(
                "UPDATE tasks SET planning_completed=0 WHERE id=?1",
                [task.as_str()],
            )
            .unwrap();
        assert!(!store.retirement_progress(&old, "closing", None).unwrap());
        assert!(store
            .reconcile_task_retirement(&task, 200, None)
            .unwrap()
            .is_none());
        store.conn.lock().unwrap().execute("UPDATE tasks SET planning_completed=1,planning_completed_at='completion-2' WHERE id=?1", [task.as_str()]).unwrap();
        let next = store
            .reconcile_task_retirement(&task, 300, None)
            .unwrap()
            .unwrap();
        assert_eq!(next.due_at, Some(1200));
        assert!(!store.retirement_progress(&old, "retired", None).unwrap());
        store
            .conn
            .lock()
            .unwrap()
            .execute(
                "UPDATE tasks SET worktree='/repo.new' WHERE id=?1",
                [task.as_str()],
            )
            .unwrap();
        assert!(!store.retirement_progress(&next, "closing", None).unwrap());
    }

    #[test]
    fn workspace_shell_creation_is_once_even_after_lost_acknowledgement_or_exit() {
        let (_home, store, task) = fixture();
        assert!(store
            .create_task_shell("shell", &task, "/repo.task")
            .unwrap());
        assert!(!store
            .create_task_shell("shell", &task, "/repo.task")
            .unwrap());
        assert!(store.task_shell("shell").unwrap().unwrap().owner.is_none());
        let owner = store.test_lf_process();
        store.bind_task_shell("shell", &owner).unwrap();
        assert!(store.bind_task_shell("shell", &owner).is_err());
        assert_eq!(store.shell_owner(&owner).unwrap(), Some(0));
        // A viewed Desktop shell is an LF-owned hold; an ordinary shell never is.
        store
            .record_owner_viewer_hold(&owner, Some(ViewerHold::Viewed))
            .unwrap();
        assert_eq!(held(&store, &task), vec![(ViewerHold::Viewed, false)]);
        store.record_owner_viewer_hold(&owner, None).unwrap();
        store.close_task_shell("shell").unwrap();
        assert_eq!(store.shell_owner(&owner).unwrap(), Some(1));
        assert!(!store
            .create_task_shell("shell", &task, "/repo.task")
            .unwrap());
        assert_eq!(
            store.task_shell("shell").unwrap().unwrap().owner,
            Some(owner.clone())
        );
        store.settle_shell_jobs(&owner).unwrap();
        assert!(store
            .create_task_shell("unacknowledged", &task, "/repo.task")
            .unwrap());
        store.close_task_shell("unacknowledged").unwrap();
        assert!(store
            .bind_task_shell("unacknowledged", &store.test_lf_process())
            .is_err());
        assert!(store.task_shell_jobs_settled(&task).unwrap());
    }
}
