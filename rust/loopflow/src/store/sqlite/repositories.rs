//! Selected planning identity, separate from a machine's local repository path.
use rusqlite::{params, Connection, OptionalExtension, TransactionBehavior};

use crate::durable::{
    MachineId, RepositoryId, RepositoryIdentity, TaskExecutionRoute, TaskExecutionSource, TaskId,
    WorkRef,
};
use crate::store::{StoreError, StoreResult};

use super::SqliteStore;

pub(super) fn ensure_repository_in(conn: &Connection, repo: &str) -> StoreResult<RepositoryId> {
    let common = std::path::Path::new(repo).join(".git");
    // Historical rows can retain a symlink spelling. Only the canonical entry
    // enrolls Git; reading/importing legacy aliases must not replace its locator.
    let common = if common.is_dir()
        && std::path::Path::new(repo).canonicalize().ok().as_deref()
            == Some(std::path::Path::new(repo))
    {
        Some(crate::repository::location::locator_directory(
            &common,
            &super::durable::map_local_machine(conn)?.id,
        ))
    } else {
        None
    };
    let locator = if let Some(common) = &common {
        crate::repository::location::read_locator(common)
            .map_err(|error| StoreError::InvalidData(error.to_string()))?
    } else {
        None
    };
    if let Some(id) = repository_id_in(conn, repo)? {
        if common.is_some() && locator.is_none() {
            return Err(StoreError::InvalidData(format!(
                "{repo} retains repository {id} but has no Git locator; establish the original checkout with `lf repo identity --bind {id}`, or recover the moved checkout with `lf repo identity --relocate {id}`"
            )));
        }
        if let Some(locator) = &locator {
            let path: Option<String> = conn
                .query_row(
                    "SELECT repo FROM repository_plans WHERE id=?1",
                    [locator.as_str()],
                    |row| row.get(0),
                )
                .optional()?;
            if path.as_deref() != Some(repo) {
                return Err(StoreError::InvalidData(
                    "Git locator conflicts with retained Work; reconcile with `lf repo identity`"
                        .into(),
                ));
            }
        }
        return Ok(id);
    }
    let id = locator.unwrap_or_else(RepositoryId::new);
    // The Git locator may precede a rolled-back transaction. Reuse it on retry;
    // an identity already registered elsewhere must be reconciled, not copied.
    conn.execute(
        "INSERT INTO repository_plans(repo,id,selected)
         SELECT ?1,?2,1 WHERE NOT EXISTS(SELECT 1 FROM repository_plans WHERE repo=?1)",
        params![repo, id.as_str()],
    )?;
    if let Some(common) = &common {
        crate::repository::location::write_locator(common, &id)
            .map_err(|error| StoreError::InvalidData(error.to_string()))?;
    }
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

/// Routing and transactional preparation use the same sharing boundary.
/// Shared mutation routes are planning evidence, never an execution reservation.
pub(super) fn task_is_shared_in(conn: &Connection, task: &TaskId) -> StoreResult<bool> {
    Ok(conn.query_row(
        "SELECT EXISTS(SELECT 1 FROM planning_peer_changes c JOIN planning_peer_routes r ON r.mutation_id=c.id WHERE c.record_table='tasks' AND c.object_id=?1)",
        [task.as_str()],
        |row| row.get(0),
    )?)
}

impl SqliteStore {
    /// Move current addresses together. Captures, Process rows and provider
    /// history retain the paths at which they were recorded.
    pub(crate) fn relocate_repository(
        &self,
        id: &RepositoryId,
        destination: &str,
    ) -> StoreResult<()> {
        let mut conn = self.conn.lock().expect("store mutex poisoned");
        let tx = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
        let source: String = tx.query_row(
            "SELECT repo FROM repository_plans WHERE id=?1",
            [id.as_str()],
            |row| row.get(0),
        )?;
        if source == destination {
            return Ok(());
        }
        if repository_id_in(&tx, destination)?.is_some() {
            return Err(StoreError::InvalidData(format!(
                "{destination} already retains another repository; no Work was moved"
            )));
        }
        // Freeze only observed checkout membership before changing its address.
        // A future clone reusing the old path cannot acquire this history.
        tx.execute(
            "INSERT OR IGNORE INTO relocated_task_processes(task_id,process_id)
             SELECT t.id,e.id FROM tasks t JOIN projects p ON p.id=t.project_id
             JOIN waves w ON w.id=p.wave_id JOIN processes e
               ON e.cwd=rtrim(t.worktree,'/') OR instr(e.cwd,rtrim(t.worktree,'/')||'/')=1
             WHERE w.repo=?1 AND t.checkout_machine_id=(SELECT id FROM machines WHERE route='local')
               AND (t.worktree=?1 OR instr(t.worktree,?1||'/')=1)",
            [&source],
        )?;
        for table in [
            "repository_plans",
            "waves",
            "agent_sessions",
            "planning_destinations",
            "planning_peer_routes",
            "planning_peer_imports",
            "planning_peer_conflicts",
            "pm_projects",
            "pm_items",
        ] {
            tx.execute(
                &format!("UPDATE {table} SET repo=?2 WHERE repo=?1"),
                params![source, destination],
            )?;
        }
        // Only descendants moved with the directory. Sibling worktrees keep
        // their actual paths, even when their names retain the old prefix.
        for (table, column, scope) in [
            ("agent_sessions", "cwd", "repo=?2"),
            ("tasks", "worktree", "checkout_machine_id=(SELECT id FROM machines WHERE route='local') AND project_id IN (SELECT p.id FROM projects p JOIN waves w ON w.id=p.wave_id WHERE w.repo=?2)"),
        ] {
            tx.execute(
                &format!("UPDATE {table} SET {column}=?2 || substr({column},length(?1)+1)
                    WHERE ({scope}) AND ({column}=?1 OR instr({column},?1||'/')=1)"),
                params![source, destination],
            )?;
        }
        tx.commit()?;
        Ok(())
    }

    /// Read execution only on its owning Machine. Foreign cached placements do
    /// not become another authority; peers must read that Machine themselves.
    pub fn observe_task_location(
        &self,
        repository: &RepositoryId,
        task: &TaskId,
        request: &str,
    ) -> StoreResult<crate::durable::TaskLocationObservation> {
        use crate::durable::{TaskLocation, TaskLocationObservation};
        let mut conn = self.conn.lock().expect("store mutex poisoned");
        let tx = conn.transaction()?;
        let machine = super::durable::map_local_machine(&tx)?.id;
        let location = (|| -> StoreResult<TaskLocation> {
            let repo: String = tx.query_row(
                "SELECT repo FROM repository_plans WHERE id=?1",
                [repository.as_str()],
                |row| row.get(0),
            )?;
            let id = super::children::resolve_task_id_in(&tx, task.as_str(), Some(&repo))?
                .ok_or(StoreError::NotFound)?;
            let (checkout, owner, started): (Option<String>, Option<String>, Option<i64>) = tx
                .query_row(
                    "SELECT worktree,checkout_machine_id,started_at FROM tasks WHERE id=?1",
                    [id.as_str()],
                    |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
                )?;
            if checkout.is_none() && started.is_none() {
                return Ok(TaskLocation::Unrecorded);
            }
            if owner.as_deref() != Some(machine.as_str()) {
                return Ok(TaskLocation::Unavailable {
                    reason: "execution Machine is unknown or not this observer; read its owner"
                        .into(),
                });
            }
            Ok(TaskLocation::Recorded {
                task_id: id,
                checkout,
            })
        })()
        .unwrap_or_else(|error| TaskLocation::Unavailable {
            reason: error.to_string(),
        });
        Ok(TaskLocationObservation {
            request: request.into(),
            repository_id: repository.clone(),
            task_id: task.clone(),
            machine_id: machine,
            observed_at: time::OffsetDateTime::now_utc().unix_timestamp(),
            location,
        })
    }

    pub fn task_is_shared(&self, task: &TaskId) -> StoreResult<bool> {
        let conn = self.conn.lock().expect("store mutex poisoned");
        task_is_shared_in(&conn, task)
    }

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
        } else if let Some(owner) = super::task_admission::owner_in(&tx, task)? {
            (owner, TaskExecutionSource::Admission)
        } else {
            (
                super::durable::placement_in(&tx, &WorkRef::Task(task.clone()))?.machine_id,
                TaskExecutionSource::EffectiveDelegation,
            )
        };
        Ok(TaskExecutionRoute {
            task_id: task.clone(),
            checkout,
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

    pub fn repository_identity(&self, repo: &str) -> StoreResult<Option<RepositoryIdentity>> {
        let conn = self.conn.lock().expect("store mutex poisoned");
        // A single statement prevents a concurrent bind from mixing generations.
        let mut statement = conn.prepare(
            "SELECT selected.id, locator.id FROM repository_plans selected
             JOIN repository_plans locator ON locator.repo=selected.repo
             WHERE selected.repo=?1 AND selected.selected=1 ORDER BY locator.id",
        )?;
        let rows = statement.query_map([repo], |row| {
            Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
        })?;
        let mut identity: Option<RepositoryIdentity> = None;
        for row in rows {
            let (id, locator) = row?;
            let parse = |value: &str| {
                RepositoryId::parse(value)
                    .map_err(|error| StoreError::InvalidData(error.to_string()))
            };
            let selected = parse(&id)?;
            identity
                .get_or_insert_with(|| RepositoryIdentity {
                    id: selected,
                    locators: Vec::new(),
                })
                .locators
                .push(parse(&locator)?);
        }
        Ok(identity)
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
    fn relocation_rolls_back_and_retains_nested_task_processes() {
        use crate::durable::{ProjectId, TaskId};
        use crate::id::{LfProcessId, TraceId, WaveId};
        use rusqlite::params;
        let dir = tempfile::tempdir().unwrap();
        let store = SqliteStore::open_ephemeral(&dir.path().join("db")).unwrap();
        let id = store.ensure_repository("/before").unwrap();
        let wave = WaveId::new();
        let project = ProjectId::new();
        let task = TaskId::new();
        let process = LfProcessId::new();
        {
            let conn = store.conn.lock().unwrap();
            conn.execute(
                "INSERT INTO waves(id,name,repo,created_at) VALUES(?1,'proof','/before',1)",
                [&wave],
            )
            .unwrap();
            conn.execute("INSERT INTO projects(id,wave_id,external_project_id,created_at) VALUES(?1,?2,'project',1)", params![project.as_str(),wave]).unwrap();
            conn.execute("INSERT INTO tasks(id,project_id,external_issue_id,issue_identifier,worktree,created_at,checkout_machine_id) VALUES(?1,?2,'issue','PROOF-1','/before/task',1,(SELECT id FROM machines WHERE route='local'))",params![task.as_str(),project.as_str()]).unwrap();
            conn.execute("INSERT INTO processes(id,trace_id,cwd,started_at) VALUES(?1,?2,'/before/task/src',1)",params![process,TraceId::new()]).unwrap();
            conn.execute_batch("CREATE TRIGGER interrupt_move BEFORE UPDATE OF worktree ON tasks BEGIN SELECT RAISE(ABORT,'interrupted'); END;").unwrap();
        }
        assert!(store.relocate_repository(&id, "/after").is_err());
        assert_eq!(
            store.repository_path(&id).unwrap().as_deref(),
            Some("/before")
        );
        assert_eq!(store.get_wave(&wave).unwrap().unwrap().repo(), "/before");
        store
            .conn
            .lock()
            .unwrap()
            .execute_batch("DROP TRIGGER interrupt_move;")
            .unwrap();
        store.relocate_repository(&id, "/after").unwrap();
        store.relocate_repository(&id, "/after").unwrap();
        assert_eq!(store.task_work(&task).unwrap().processes[0].id, process);
        assert_eq!(store.task_open_processes(&task).unwrap()[0].id, process);
        let conn = store.conn.lock().unwrap();
        assert_eq!(
            super::super::task_work::task_of_process(&conn, &process)
                .unwrap()
                .as_deref(),
            Some(task.as_str())
        );
        let cwd: String = conn
            .query_row("SELECT cwd FROM processes WHERE id=?1", [&process], |row| {
                row.get(0)
            })
            .unwrap();
        assert_eq!(cwd, "/before/task/src");
        assert_eq!(
            conn.query_row("SELECT count(*) FROM relocated_task_processes", [], |row| {
                row.get::<_, i64>(0)
            })
            .unwrap(),
            1
        );
        let unrelated = LfProcessId::new();
        conn.execute(
            "INSERT INTO processes(id,trace_id,cwd,started_at) VALUES(?1,?2,'/before/task/src',2)",
            params![unrelated, TraceId::new()],
        )
        .unwrap();
        assert!(super::super::task_work::task_of_process(&conn, &unrelated)
            .unwrap()
            .is_none());
    }

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
