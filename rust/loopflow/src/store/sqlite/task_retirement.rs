//! Host observations and one cleanup intent pinned to a Task's completion.
use rusqlite::{params, OptionalExtension};
use serde::{Deserialize, Serialize};

use super::SqliteStore;
use crate::durable::TaskId;
use crate::id::LfProcessId;
use crate::store::{StoreError, StoreResult};

pub(crate) const WORKSPACE_GRACE: i64 = 15 * 60;
pub(crate) const HOST_LEASE: i64 = 30;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, clap::ValueEnum)]
#[serde(rename_all = "snake_case")]
pub enum HostUse {
    Retained,
    Away,
    Closed,
    Unknown,
}

impl HostUse {
    fn as_str(self) -> &'static str {
        match self {
            Self::Retained => "retained",
            Self::Away => "away",
            Self::Closed => "closed",
            Self::Unknown => "unknown",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
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
    pub(crate) fn observe_task_host(
        &self,
        task: &TaskId,
        occurrence: &str,
        state: HostUse,
        now: i64,
        revision: i64,
    ) -> StoreResult<()> {
        if occurrence.is_empty() {
            return Err(StoreError::InvalidData(
                "Host occurrence identity is empty".into(),
            ));
        }
        let mut conn = self.conn.lock().expect("store mutex poisoned");
        let tx = conn.transaction()?;
        let changed = tx.execute("INSERT INTO task_host_occurrences(task_id,occurrence,state,lease_until,revision) VALUES(?1,?2,?3,?4,?5)
            ON CONFLICT(task_id,occurrence) DO UPDATE SET state=excluded.state,lease_until=excluded.lease_until,revision=excluded.revision
            WHERE excluded.revision>task_host_occurrences.revision", params![task.as_str(),occurrence,state.as_str(),now+HOST_LEASE,revision])?;
        if changed == 0 && tx.query_row("SELECT state=?3 AND revision=?4 FROM task_host_occurrences WHERE task_id=?1 AND occurrence=?2", params![task.as_str(),occurrence,state.as_str(),revision], |r| r.get::<_,bool>(0)).optional()?.unwrap_or(false) {
            return Ok(());
        }
        if changed == 0 {
            return Err(StoreError::InvalidData(
                "Host observation was superseded".into(),
            ));
        }
        if matches!(state, HostUse::Retained | HostUse::Unknown) {
            tx.execute(
                "UPDATE task_shells SET close_requested=0 WHERE task_id=?1 AND close_requested=2",
                [task.as_str()],
            )?;
            tx.execute("UPDATE task_retirements SET due_at=NULL,phase='waiting',reason=NULL WHERE task_id=?1 AND phase!='retired'", [task.as_str()])?;
        }
        tx.commit()?;
        Ok(())
    }

    pub(crate) fn reconcile_task_retirement(
        &self,
        task: &TaskId,
        now: i64,
    ) -> StoreResult<Option<TaskRetirement>> {
        let mut conn = self.conn.lock().expect("store mutex poisoned");
        let tx = conn.transaction()?;
        tx.execute("INSERT INTO task_retirements(task_id,completion,checkout,branch,due_at,phase)
            SELECT id,planning_completed_at,worktree,branch,NULL,'waiting' FROM tasks
            WHERE id=?1 AND planning_completed=1 AND planning_completed_at IS NOT NULL AND worktree IS NOT NULL
            ON CONFLICT(task_id) DO NOTHING", [task.as_str()])?;
        let (retained, unknown, all_closed): (bool,bool,bool) = tx.query_row(
            "SELECT EXISTS(SELECT 1 FROM task_host_occurrences WHERE task_id=?1 AND state='retained' AND lease_until>=?2),
                    EXISTS(SELECT 1 FROM task_host_occurrences WHERE task_id=?1 AND (state='unknown' OR (state='retained' AND lease_until<?2))),
                    EXISTS(SELECT 1 FROM task_host_occurrences WHERE task_id=?1) AND NOT EXISTS(SELECT 1 FROM task_host_occurrences WHERE task_id=?1 AND state!='closed')",
            params![task.as_str(),now], |r| Ok((r.get(0)?,r.get(1)?,r.get(2)?)))?;
        if retained || unknown {
            tx.execute(
                "UPDATE task_shells SET close_requested=0 WHERE task_id=?1 AND close_requested=2",
                [task.as_str()],
            )?;
            tx.execute("UPDATE task_retirements SET due_at=NULL,phase='waiting',reason=?2 WHERE task_id=?1 AND phase!='retired'", params![task.as_str(),if retained { "Retained by a host" } else { "Host observation unavailable; cleanup pending" }])?;
        } else {
            tx.execute("UPDATE task_retirements SET due_at=CASE WHEN ?2 THEN ?3 ELSE COALESCE(due_at,?3+?4) END,reason=NULL WHERE task_id=?1 AND phase='waiting'", params![task.as_str(),all_closed,now,WORKSPACE_GRACE])?;
        }
        tx.commit()?;
        drop(conn);
        self.task_retirement(task)
    }

    pub fn task_retirement(&self, task: &TaskId) -> StoreResult<Option<TaskRetirement>> {
        let conn = self.conn.lock().expect("store mutex poisoned");
        Ok(conn.query_row("SELECT task_id,completion,checkout,branch,due_at,phase,reason FROM task_retirements WHERE task_id=?1", [task.as_str()], |r| Ok(TaskRetirement {
            task_id:TaskId::from_raw(r.get::<_, String>(0)?),completion:r.get(1)?,checkout:r.get(2)?,branch:r.get(3)?,due_at:r.get(4)?,phase:r.get(5)?,reason:r.get(6)?,
        })).optional()?)
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

    pub(crate) fn owner_host_hold(&self, owner: &LfProcessId) -> StoreResult<bool> {
        let conn = self.conn.lock().expect("store mutex poisoned");
        Ok(conn.query_row(
            &format!(
                "SELECT EXISTS(SELECT 1 FROM task_host_occurrences h
            JOIN agent_sessions s ON s.id IN ({})
            WHERE s.transport_lf_process_id=?1 AND h.state='retained' AND h.lease_until>=unixepoch())",
                super::task_work::session_ids("h.task_id")
            ),
            [owner],
            |r| r.get(0),
        )?)
    }

    pub(crate) fn owner_host_closed(&self, owner: &LfProcessId) -> StoreResult<bool> {
        let conn = self.conn.lock().expect("store mutex poisoned");
        Ok(conn.query_row(&format!("SELECT
            EXISTS(SELECT 1 FROM task_host_occurrences h JOIN agent_sessions s ON s.id IN ({}) WHERE s.transport_lf_process_id=?1)
            AND NOT EXISTS(SELECT 1 FROM task_host_occurrences h JOIN agent_sessions s ON s.id IN ({})
                WHERE s.transport_lf_process_id=?1 AND h.state!='closed')", super::task_work::session_ids("h.task_id"),super::task_work::session_ids("h.task_id")), [owner], |r| r.get(0))?)
    }

    pub(crate) fn owner_retiring(&self, owner: &LfProcessId) -> StoreResult<bool> {
        let conn = self.conn.lock().expect("store mutex poisoned");
        Ok(conn.query_row(&format!("SELECT
            EXISTS(SELECT 1 FROM task_retirements h JOIN agent_sessions s ON s.id IN ({}) WHERE s.transport_lf_process_id=?1 AND h.phase='closing')
            OR EXISTS(SELECT 1 FROM task_retirements h JOIN task_shells s ON s.task_id=h.task_id WHERE s.owner=?1 AND h.phase='closing')", super::task_work::session_ids("h.task_id")), [owner], |r| r.get(0))?)
    }

    pub(crate) fn retirement_hosts_closed(&self, task: &TaskId) -> StoreResult<bool> {
        let conn = self.conn.lock().expect("store mutex poisoned");
        Ok(conn.query_row("SELECT NOT EXISTS(SELECT 1 FROM task_host_occurrences WHERE task_id=?1 AND state!='closed')", [task.as_str()], |r| r.get(0))?)
    }
}

#[cfg(test)]
mod tests {
    use super::{HostUse, SqliteStore, TaskId, WORKSPACE_GRACE};
    use rusqlite::params;

    fn fixture() -> (tempfile::TempDir, SqliteStore, TaskId) {
        let home = tempfile::tempdir().unwrap();
        let store = SqliteStore::open_ephemeral(&home.path().join("loopflow.db")).unwrap();
        let repo = store.ensure_repository("/repo").unwrap();
        let task = TaskId::new();
        store.conn.lock().unwrap().execute("INSERT INTO tasks(id,repository_id,issue_identifier,created_at,worktree,branch,planning_completed,planning_completed_at) VALUES(?1,?2,'SHELL-1',1,'/repo.task','task',1,'completion-1')", params![task.as_str(),repo.as_str()]).unwrap();
        (home, store, task)
    }

    #[test]
    fn workspace_retirement_returns_cancel_deadlines_and_close_skips_grace() {
        let (_home, store, task) = fixture();
        store
            .observe_task_host(&task, "desktop", HostUse::Retained, 100, 1)
            .unwrap();
        assert_eq!(
            store
                .reconcile_task_retirement(&task, 100)
                .unwrap()
                .unwrap()
                .due_at,
            None
        );
        store
            .observe_task_host(&task, "desktop", HostUse::Away, 110, 2)
            .unwrap();
        let left = store
            .reconcile_task_retirement(&task, 110)
            .unwrap()
            .unwrap();
        assert_eq!(left.due_at, Some(110 + WORKSPACE_GRACE));
        // The two-minute agent policy does not add two minutes to this deadline.
        assert_eq!(
            store
                .reconcile_task_retirement(&task, 230)
                .unwrap()
                .unwrap()
                .due_at,
            left.due_at
        );
        store
            .observe_task_host(&task, "desktop", HostUse::Retained, 900, 3)
            .unwrap();
        assert_eq!(
            store
                .reconcile_task_retirement(&task, 900)
                .unwrap()
                .unwrap()
                .due_at,
            None
        );
        store
            .observe_task_host(&task, "desktop", HostUse::Away, 920, 4)
            .unwrap();
        assert_eq!(
            store
                .reconcile_task_retirement(&task, 920)
                .unwrap()
                .unwrap()
                .due_at,
            Some(1820)
        );
        store
            .observe_task_host(&task, "desktop", HostUse::Closed, 930, 5)
            .unwrap();
        assert_eq!(
            store
                .reconcile_task_retirement(&task, 930)
                .unwrap()
                .unwrap()
                .due_at,
            Some(930)
        );
        assert!(store
            .observe_task_host(&task, "desktop", HostUse::Away, 940, 4)
            .is_err());
    }

    #[test]
    fn workspace_retirement_multiple_hosts_and_lost_observation_preserve_resources() {
        let (_home, store, task) = fixture();
        store
            .observe_task_host(&task, "rex:block-a", HostUse::Retained, 100, 1)
            .unwrap();
        store
            .observe_task_host(&task, "desktop", HostUse::Closed, 100, 1)
            .unwrap();
        assert_eq!(
            store
                .reconcile_task_retirement(&task, 100)
                .unwrap()
                .unwrap()
                .due_at,
            None
        );
        let unknown = store
            .reconcile_task_retirement(&task, 200)
            .unwrap()
            .unwrap();
        assert_eq!(unknown.due_at, None);
        assert!(unknown.reason.unwrap().contains("unavailable"));
        store
            .observe_task_host(&task, "rex:block-a", HostUse::Away, 210, 2)
            .unwrap();
        assert_eq!(
            store
                .reconcile_task_retirement(&task, 210)
                .unwrap()
                .unwrap()
                .due_at,
            Some(1110)
        );
        assert!(!store.retirement_hosts_closed(&task).unwrap());
        store
            .observe_task_host(&task, "rex:block-a", HostUse::Closed, 220, 3)
            .unwrap();
        assert!(store.retirement_hosts_closed(&task).unwrap());
    }

    #[test]
    fn workspace_retirement_reopen_and_checkout_replacement_revoke_queued_work() {
        let (_home, store, task) = fixture();
        let old = store
            .reconcile_task_retirement(&task, 100)
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
            .reconcile_task_retirement(&task, 200)
            .unwrap()
            .is_none());
        store.conn.lock().unwrap().execute("UPDATE tasks SET planning_completed=1,planning_completed_at='completion-2' WHERE id=?1", [task.as_str()]).unwrap();
        let next = store
            .reconcile_task_retirement(&task, 300)
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
    fn workspace_retirement_host_retention_applies_to_the_same_session_in_any_host() {
        let (_home, store, task) = fixture();
        let now = time::OffsetDateTime::now_utc().unix_timestamp();
        let owner = store.test_lf_process();
        store.test_session("session", &crate::session_record::new_artifact_key());
        store.conn.lock().unwrap().execute("UPDATE agent_sessions SET task_id=?1,interactive=1,transport_lf_process_id=?2 WHERE id='session'", params![task.as_str(),owner]).unwrap();
        assert!(!store.owner_host_hold(&owner).unwrap());
        for host in ["rex", "desktop"] {
            store
                .observe_task_host(&task, host, HostUse::Retained, now, 1)
                .unwrap();
            assert!(store.owner_host_hold(&owner).unwrap());
            store
                .observe_task_host(&task, host, HostUse::Away, now, 2)
                .unwrap();
            assert!(!store.owner_host_hold(&owner).unwrap());
            store
                .observe_task_host(&task, host, HostUse::Closed, now, 3)
                .unwrap();
            assert!(store.owner_host_closed(&owner).unwrap());
        }
        let intent = store
            .reconcile_task_retirement(&task, now)
            .unwrap()
            .unwrap();
        assert!(store.retirement_progress(&intent, "closing", None).unwrap());
        assert!(store.owner_host_closed(&owner).unwrap());
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
