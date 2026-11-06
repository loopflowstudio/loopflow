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
    /// A connected viewer, observed at this wall-clock second.
    Viewed(i64),
    /// The last connected viewer left or disconnected at this wall-clock second.
    Absent(i64),
}

impl ViewerHold {
    fn as_str(self) -> &'static str {
        match self {
            Self::Viewed(_) => "viewed",
            Self::Absent(_) => "absent",
        }
    }
}

/// One Session owner's dated viewing evidence, retained after owner exit.
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
    pub retry_at: Option<i64>,
    pub failures: u32,
}

impl SqliteStore {
    pub(crate) fn record_owner_viewer_hold(
        &self,
        owner: &LfProcessId,
        hold: Option<ViewerHold>,
    ) -> StoreResult<()> {
        self.conn.lock().expect("store mutex poisoned").execute(
            "UPDATE processes SET viewer_hold=?2,viewer_hold_at=?3 WHERE id=?1",
            params![
                owner.as_str(),
                hold.map(ViewerHold::as_str),
                hold.map(|h| match h {
                    ViewerHold::Viewed(at) | ViewerHold::Absent(at) => at,
                })
            ],
        )?;
        Ok(())
    }

    /// Only Session viewers retain a checkout. Shell receipts own cleanup, not retention.
    pub(crate) fn task_viewer_holds(&self, task: &TaskId) -> StoreResult<Vec<OwnerHold>> {
        let conn = self.conn.lock().expect("store mutex poisoned");
        let mut query = conn.prepare(&format!(
            "SELECT p.id,p.viewer_hold,p.completed_at IS NOT NULL,p.viewer_hold_at FROM processes p
             WHERE p.viewer_hold IS NOT NULL
               AND p.id IN (SELECT s.transport_lf_process_id FROM agent_sessions s WHERE s.id IN ({}))
             ORDER BY p.id",
            super::task_work::session_ids("?1")
        ))?;
        let rows = query.query_map([task.as_str()], |r| {
            Ok((
                r.get::<_, LfProcessId>(0)?,
                r.get::<_, String>(1)?,
                r.get::<_, bool>(2)?,
                r.get::<_, i64>(3)?,
            ))
        })?;
        rows.map(|row| {
            let (owner, hold, ended, at) = row?;
            let hold = match hold.as_str() {
                "viewed" => ViewerHold::Viewed(at),
                "absent" => ViewerHold::Absent(at),
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

    pub(crate) fn pending_retirements(&self) -> StoreResult<Vec<TaskId>> {
        let conn = self.conn.lock().expect("store mutex poisoned");
        let mut query = conn.prepare(
            "SELECT task_id FROM task_retirements WHERE phase!='retired' ORDER BY task_id",
        )?;
        let tasks = query
            .query_map([], |r| Ok(TaskId::from_raw(r.get::<_, String>(0)?)))?
            .collect::<rusqlite::Result<_>>()?;
        Ok(tasks)
    }

    pub(crate) fn fail_retirement(&self, task: &TaskId, now: i64, reason: &str) -> StoreResult<()> {
        self.conn.lock().expect("store mutex poisoned").execute(
            "UPDATE task_retirements SET reason=?3,retry_at=?2+MIN(3600,60*(1 << MIN(failures,6))),failures=failures+1
             WHERE task_id=?1 AND phase!='retired' AND due_at IS NOT NULL",
            params![task.as_str(), now, reason])?;
        Ok(())
    }

    /// `hold` names why a viewer still retains the checkout. Holds clear any
    /// deadline; release starts one fifteen-minute interval that later work
    /// completion does not restart.
    pub(crate) fn reconcile_task_retirement(
        &self,
        task: &TaskId,
        now: i64,
        hold: Option<&str>,
        absent_since: Option<i64>,
    ) -> StoreResult<Option<TaskRetirement>> {
        let mut conn = self.conn.lock().expect("store mutex poisoned");
        let tx = conn.transaction()?;
        tx.execute("INSERT INTO task_retirements(task_id,completion,checkout,branch,due_at,phase)
            SELECT id,planning_completed_at,worktree,branch,?2+?3,'waiting' FROM tasks
            WHERE id=?1 AND planning_completed=1 AND planning_completed_at IS NOT NULL AND worktree IS NOT NULL
            ON CONFLICT(task_id) DO NOTHING", params![task.as_str(),now,WORKSPACE_GRACE])?;
        match hold {
            Some(reason) => hold_retirement(&tx, task, Some(reason))?,
            None => {
                // A viewer can reconnect and leave between cleanup passes,
                // including after shell closure was queued. Revoke that work too.
                let renewed: bool = tx.query_row(
                    "SELECT EXISTS(SELECT 1 FROM task_retirements WHERE task_id=?1
                     AND phase!='retired' AND due_at<?2+?3)",
                    params![task.as_str(), absent_since, WORKSPACE_GRACE],
                    |row| row.get(0),
                )?;
                if renewed {
                    hold_retirement(&tx, task, None)?;
                }
                tx.execute(
                    "UPDATE task_retirements SET due_at=?2+?3,reason=NULL
                     WHERE task_id=?1 AND phase='waiting' AND due_at IS NULL",
                    params![task.as_str(), absent_since.unwrap_or(now), WORKSPACE_GRACE],
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
                "SELECT task_id,completion,checkout,branch,due_at,phase,reason,retry_at,failures
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
                        retry_at: r.get(7)?,
                        failures: r.get(8)?,
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
fn hold_retirement(conn: &Connection, task: &TaskId, reason: Option<&str>) -> StoreResult<()> {
    conn.execute(
        "UPDATE task_shells SET close_requested=0 WHERE task_id=?1 AND close_requested=2",
        [task.as_str()],
    )?;
    conn.execute(
        "UPDATE task_retirements SET due_at=NULL,phase='waiting',reason=?2,retry_at=NULL,failures=0
         WHERE task_id=?1 AND phase!='retired'
           AND (due_at IS NOT NULL OR phase!='waiting' OR reason IS NOT ?2)",
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
    fn viewer_evidence_records_absence_and_follows_current_session_owners() {
        let (_home, store, task) = fixture();
        let first = session_owner(&store, &task, "first");
        let second = session_owner(&store, &task, "second");
        let unrelated = store.test_lf_process();
        store
            .record_owner_viewer_hold(&unrelated, Some(ViewerHold::Viewed(100)))
            .unwrap();
        assert!(held(&store, &task).is_empty());
        store
            .record_owner_viewer_hold(&first, Some(ViewerHold::Viewed(100)))
            .unwrap();
        store
            .record_owner_viewer_hold(&second, Some(ViewerHold::Viewed(100)))
            .unwrap();
        assert_eq!(held(&store, &task).len(), 2);
        // One viewer's exit leaves the other Session's viewer holding.
        store.record_owner_viewer_hold(&first, None).unwrap();
        assert_eq!(held(&store, &task), vec![(ViewerHold::Viewed(100), false)]);
        // An ended owner retains its dated evidence; it cannot hold indefinitely.
        let conn = store.conn.lock().unwrap();
        conn.execute("UPDATE processes SET completed_at=1 WHERE id=?1", [&second])
            .unwrap();
        drop(conn);
        assert_eq!(held(&store, &task), vec![(ViewerHold::Viewed(100), true)]);
        store
            .record_owner_viewer_hold(&second, Some(ViewerHold::Absent(110)))
            .unwrap();
        assert_eq!(held(&store, &task), vec![(ViewerHold::Absent(110), true)]);
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
    }

    #[test]
    fn workspace_retirement_holds_cancel_deadlines_and_work_does_not_restart_them() {
        let (_home, store, task) = fixture();
        let held = store
            .reconcile_task_retirement(
                &task,
                100,
                Some("Retained by an open Task Session viewer"),
                None,
            )
            .unwrap()
            .unwrap();
        assert_eq!(held.due_at, None);
        let left = store
            .reconcile_task_retirement(&task, 110, None, None)
            .unwrap()
            .unwrap();
        assert_eq!(left.due_at, Some(110 + WORKSPACE_GRACE));
        // The two-minute agent policy does not add two minutes to this deadline.
        assert_eq!(
            store
                .reconcile_task_retirement(&task, 230, None, None)
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
            .reconcile_task_retirement(
                &task,
                900,
                Some("Retained by an open Task Session viewer"),
                None,
            )
            .unwrap()
            .unwrap();
        assert_eq!(returned.due_at, None);
        assert!(returned
            .reason
            .unwrap()
            .contains("open Task Session viewer"));
        assert!(!store.task_shell("shell").unwrap().unwrap().close_requested);
        assert_eq!(
            store
                .reconcile_task_retirement(&task, 920, None, None)
                .unwrap()
                .unwrap()
                .due_at,
            Some(920 + WORKSPACE_GRACE)
        );
    }

    #[test]
    fn disconnect_deadline_uses_observed_absence_not_later_cleanup_or_agent_exit() {
        let (_home, store, task) = fixture();
        store
            .reconcile_task_retirement(&task, 100, Some("viewed"), None)
            .unwrap();
        let lost = store
            .reconcile_task_retirement(&task, 400, None, Some(110))
            .unwrap()
            .unwrap();
        assert_eq!(lost.due_at, Some(1010));
        let ended = store
            .reconcile_task_retirement(&task, 900, None, Some(110))
            .unwrap()
            .unwrap();
        assert_eq!(ended.due_at, lost.due_at);
        // A short reconnect may begin and end between cleanup passes,
        // after retirement has already queued shell closes.
        store
            .create_task_shell("shell", &task, "/repo.task")
            .unwrap();
        store.retirement_progress(&lost, "closing", None).unwrap();
        store.close_task_shells(&task).unwrap();
        let returned = store
            .reconcile_task_retirement(&task, 1310, None, Some(1300))
            .unwrap()
            .unwrap();
        assert_eq!(returned.due_at, Some(2200));
        assert_eq!(returned.phase, "waiting");
        assert!(!store.task_shell("shell").unwrap().unwrap().close_requested);
        store
            .reconcile_task_retirement(&task, 1950, Some("reconnected"), None)
            .unwrap();
        assert_eq!(
            store
                .reconcile_task_retirement(&task, 2050, None, Some(2000))
                .unwrap()
                .unwrap()
                .due_at,
            Some(2900)
        );
    }

    #[test]
    fn retry_backoff_is_bounded_and_returning_viewer_revokes_it() {
        let (_home, store, task) = fixture();
        let initial = store
            .reconcile_task_retirement(&task, 100, None, None)
            .unwrap()
            .unwrap();
        for (attempt, seconds) in [60, 120, 240, 480, 960, 1920, 3600, 3600]
            .into_iter()
            .enumerate()
        {
            store.fail_retirement(&task, 2000, "offline").unwrap();
            let failed = store
                .reconcile_task_retirement(&task, 2000, None, None)
                .unwrap()
                .unwrap();
            assert_eq!(failed.retry_at, Some(2000 + seconds));
            assert_eq!(failed.failures as usize, attempt + 1);
            assert_eq!(failed.due_at, initial.due_at);
            assert_eq!(failed.reason.as_deref(), Some("offline"));
        }
        let returned = store
            .reconcile_task_retirement(&task, 2100, Some("viewed"), None)
            .unwrap()
            .unwrap();
        assert_eq!(returned.retry_at, None);
        assert_eq!(returned.failures, 0);
        assert_eq!(returned.due_at, None);
    }

    #[test]
    fn workspace_retirement_reopen_and_checkout_replacement_revoke_queued_work() {
        let (_home, store, task) = fixture();
        let old = store
            .reconcile_task_retirement(&task, 100, None, None)
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
            .reconcile_task_retirement(&task, 200, None, None)
            .unwrap()
            .is_none());
        store.conn.lock().unwrap().execute("UPDATE tasks SET planning_completed=1,planning_completed_at='completion-2' WHERE id=?1", [task.as_str()]).unwrap();
        let next = store
            .reconcile_task_retirement(&task, 300, None, None)
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
        // Desktop shell ownership adds no checkout hold; the Task Session supplies it.
        store
            .record_owner_viewer_hold(&owner, Some(ViewerHold::Viewed(100)))
            .unwrap();
        assert!(held(&store, &task).is_empty());
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
