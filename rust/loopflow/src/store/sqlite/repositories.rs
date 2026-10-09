//! Selected planning identity, separate from a machine's local repository path.
use rusqlite::{params, Connection, OptionalExtension, TransactionBehavior};

use crate::durable::{
    MachineId, RepositoryId, TaskExecutionRoute, TaskExecutionSource, TaskId, WorkRef,
};
use crate::store::{StoreError, StoreResult};

use super::SqliteStore;

pub(super) fn ensure_repository_in(conn: &Connection, repo: &str) -> StoreResult<RepositoryId> {
    conn.execute(
        "INSERT INTO repository_plans(repo,id,selected)
         SELECT ?1,?2,1 WHERE NOT EXISTS(SELECT 1 FROM repository_plans WHERE repo=?1)",
        params![repo, RepositoryId::new().as_str()],
    )?;
    repository_id_in(conn, repo)?.ok_or(StoreError::NotFound)
}

pub(super) fn repository_id_in(conn: &Connection, repo: &str) -> StoreResult<Option<RepositoryId>> {
    let id: Option<String> = conn
        .query_row(
            "SELECT id FROM repository_plans WHERE repo=?1 AND selected=1",
            [repo],
            |row| row.get(0),
        )
        .optional()?;
    id.map(|id| {
        RepositoryId::parse(&id).map_err(|error| StoreError::InvalidData(error.to_string()))
    })
    .transpose()
}

impl SqliteStore {
    pub fn task_execution_route(&self, task: &TaskId) -> StoreResult<TaskExecutionRoute> {
        let mut conn = self.conn.lock().expect("store mutex poisoned");
        let tx = conn.transaction()?;
        let (repo, checkout, machine, started): (
            String,
            Option<String>,
            Option<String>,
            Option<i64>,
        ) = tx.query_row(
            "SELECT w.repo,t.worktree,t.checkout_machine_id,t.started_at
             FROM tasks t JOIN projects p ON p.id=t.project_id JOIN waves w ON w.id=p.wave_id
             WHERE t.id=?1",
            [task.as_str()],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?)),
        )?;
        let repository_id = repository_id_in(&tx, &repo)?.ok_or_else(|| {
            StoreError::InvalidData(format!("repository {repo} has no selected plan identity"))
        })?;
        let (machine_id, source) = if checkout.as_deref().is_some_and(|path| !path.is_empty())
            || started.is_some()
        {
            let machine = machine.ok_or_else(|| StoreError::InvalidData(format!(
                "Task {task} has unknown execution Machine; delegation cannot relocate existing work"
            )))?;
            (
                MachineId::parse(&machine)
                    .map_err(|error| StoreError::InvalidData(error.to_string()))?,
                TaskExecutionSource::RecordedCheckout,
            )
        } else {
            (
                super::durable::placement_in(&tx, &WorkRef::Task(task.clone()))?.machine_id,
                TaskExecutionSource::EffectiveDelegation,
            )
        };
        Ok(TaskExecutionRoute {
            repository_id,
            machine_id,
            source,
        })
    }

    pub fn ensure_repository(&self, repo: &str) -> StoreResult<RepositoryId> {
        let mut conn = self.conn.lock().expect("store mutex poisoned");
        let tx = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
        let id = ensure_repository_in(&tx, repo)?;
        tx.commit()?;
        Ok(id)
    }

    pub fn repository_id(&self, repo: &str) -> StoreResult<Option<RepositoryId>> {
        let conn = self.conn.lock().expect("store mutex poisoned");
        repository_id_in(&conn, repo)
    }

    pub fn repository_path(&self, id: &RepositoryId) -> StoreResult<Option<String>> {
        let conn = self.conn.lock().expect("store mutex poisoned");
        conn.query_row(
            "SELECT repo FROM repository_plans WHERE id=?1",
            [id.as_str()],
            |row| row.get(0),
        )
        .optional()
        .map_err(Into::into)
    }

    /// Explicitly associate this checkout with a selected repository identity.
    /// Retain prior IDs as local locators; Work, provider mappings, journals and
    /// execution belong to the repository path and are never rewritten here.
    pub fn bind_repository(&self, repo: &str, id: &RepositoryId) -> StoreResult<()> {
        let mut conn = self.conn.lock().expect("store mutex poisoned");
        let tx = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
        let bound: Option<String> = tx
            .query_row(
                "SELECT repo FROM repository_plans WHERE id=?1",
                [id.as_str()],
                |row| row.get(0),
            )
            .optional()?;
        if let Some(other) = bound.as_deref() {
            if other != repo {
                return Err(StoreError::InvalidData(format!(
                    "repository identity {id} already locates {other}; cannot associate another local checkout {repo}"
                )));
            }
        }
        tx.execute(
            "UPDATE repository_plans SET selected=0 WHERE repo=?1 AND selected=1 AND id!=?2",
            params![repo, id.as_str()],
        )?;
        tx.execute(
            "INSERT INTO repository_plans(repo,id,selected) VALUES(?1,?2,1)
             ON CONFLICT(id) DO UPDATE SET selected=1 WHERE selected=0",
            params![repo, id.as_str()],
        )?;
        tx.commit()?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::SqliteStore;

    #[test]
    fn two_machine_routing_uses_one_plan_and_keeps_execution_local() {
        use crate::durable::{ProjectId, TaskId, WorkRef};
        use crate::id::WaveId;
        let a = tempfile::tempdir().unwrap();
        let b = tempfile::tempdir().unwrap();
        let first = SqliteStore::open_ephemeral(&a.path().join("db")).unwrap();
        let second = SqliteStore::open_ephemeral(&b.path().join("db")).unwrap();
        let repository = first.ensure_repository("/first/repo").unwrap();
        second.bind_repository("/second/repo", &repository).unwrap();
        let destination = second.local_machine().unwrap().id;
        let connection = first
            .add_machine(&destination, "second", "second", "/unrelated/default")
            .unwrap();
        let wave = WaveId::new();
        let project = ProjectId::new();
        let task = TaskId::new();
        for (store, repo) in [(&first, "/first/repo"), (&second, "/second/repo")] {
            let conn = store.conn.lock().unwrap();
            conn.execute(
                "INSERT INTO waves(id,name,repo,created_at) VALUES(?1,'work',?2,1)",
                rusqlite::params![wave.as_str(), repo],
            )
            .unwrap();
            conn.execute(
                "INSERT INTO projects(id,wave_id,created_at) VALUES(?1,?2,1)",
                rusqlite::params![project.as_str(), wave.as_str()],
            )
            .unwrap();
            conn.execute("INSERT INTO tasks(id,project_id,issue_identifier,issue_title,issue_description,workspace_slug,created_at,updated_at) VALUES(?1,?2,'WORK-1','Shared Task','','',1,1)", rusqlite::params![task.as_str(),project.as_str()]).unwrap();
        }
        first
            .place_work(&WorkRef::Wave(wave.clone()), &destination)
            .unwrap();
        // Routing uses the read-only owner, not an initializing Store or a
        // second connection to establish the observing Machine.
        let observer = SqliteStore::open_read_only(&a.path().join("db")).unwrap();
        assert_eq!(
            observer.task_execution_route(&task).unwrap(),
            second.task_execution_route(&task).unwrap()
        );
        assert_ne!(observer.local_machine().unwrap().id, destination);
        // Only the destination records its checkout; planning identity is shared,
        // not the filesystem path or runtime state.
        second
            .conn
            .lock()
            .unwrap()
            .execute(
                "UPDATE tasks SET worktree='/second/repo.task',checkout_machine_id=?2 WHERE id=?1",
                rusqlite::params![task.as_str(), destination.as_str()],
            )
            .unwrap();
        second
            .place_work(&WorkRef::Wave(wave), &destination)
            .unwrap();
        assert_eq!(
            second.task_execution_route(&task).unwrap().machine_id,
            destination
        );
        assert!(first.task(&task).unwrap().unwrap().worktree.is_none());
        assert_eq!(first.machine_by_id(&destination).unwrap(), Some(connection));
    }

    #[test]
    fn repository_plans_are_explicit_and_do_not_depend_on_clone_names() {
        let a = tempfile::tempdir().unwrap();
        let b = tempfile::tempdir().unwrap();
        let first = SqliteStore::open_ephemeral(&a.path().join("db")).unwrap();
        let second = SqliteStore::open_ephemeral(&b.path().join("db")).unwrap();
        let id = first.ensure_repository("/src/same").unwrap();
        assert_eq!(first.ensure_repository("/src/same").unwrap(), id);
        let other = second.ensure_repository("/src/same").unwrap();
        assert_ne!(id, other);
        second.bind_repository("/src/same", &id).unwrap();
        assert_eq!(second.repository_id("/src/same").unwrap(), Some(id.clone()));
        assert_eq!(
            second.repository_path(&other).unwrap().as_deref(),
            Some("/src/same")
        );
        assert_eq!(second.ensure_repository("/src/same").unwrap(), id);
        second.bind_repository("/src/same", &id).unwrap();
        assert_eq!(
            second.repository_path(&id).unwrap().as_deref(),
            Some("/src/same")
        );
        assert_eq!(
            first.repository_path(&id).unwrap().as_deref(),
            Some("/src/same")
        );
        let unrelated = second.ensure_repository("/another/clone").unwrap();
        assert!(second.bind_repository("/another/clone", &id).is_err());
        assert_eq!(
            second.repository_id("/another/clone").unwrap(),
            Some(unrelated)
        );
        second.bind_repository("/src/same", &other).unwrap();
        assert_eq!(second.repository_id("/src/same").unwrap(), Some(other));
        assert_eq!(
            second.repository_path(&id).unwrap().as_deref(),
            Some("/src/same")
        );
        assert!(first.repository_id("/unknown").unwrap().is_none());
    }
}
