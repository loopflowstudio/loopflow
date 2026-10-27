//! Selected planning identity, separate from a machine's local repository path.
use rusqlite::{params, Connection, OptionalExtension, TransactionBehavior};
use std::path::{Path, PathBuf};

use crate::durable::{RepositoryId, RepositoryIdentity, TaskExecutionRoute, TaskId};
use crate::store::{StoreError, StoreResult};

use super::SqliteStore;

fn git_locator_in(conn: &Connection, repo: &str) -> StoreResult<Option<PathBuf>> {
    let root = Path::new(repo);
    // Historical symlink spellings never replace the canonical Git locator.
    if root.canonicalize().ok().as_deref() != Some(root) {
        return Ok(None);
    }
    let common = root.join(".git");
    let common = if common.is_dir() {
        common
    } else if common.is_file() {
        // Legacy registrations may name a linked checkout. Its shared Git
        // directory belongs to the main repository, never that local alias.
        let canonical = crate::repository::CanonicalRepo::discover(root)
            .map_err(|error| StoreError::InvalidData(error.to_string()))?;
        if canonical.as_path() != root {
            return Ok(None);
        }
        crate::git::worktrees::git_common_dir(root)
            .map_err(|error| StoreError::InvalidData(error.to_string()))?
    } else {
        return Ok(None);
    };
    Ok(Some(crate::repository::location::locator_directory(
        &common,
        &super::durable::map_local_machine(conn)?.id,
    )))
}

// A retained linked Task checkout witnesses this physical Git directory even
// before locators existed. A remote, basename, or main checkout path cannot.
fn retained_checkout_matches(conn: &Connection, repo: &str) -> StoreResult<bool> {
    let mut statement = conn.prepare(
        "SELECT t.worktree,t.branch FROM tasks t
         JOIN repository_plans w ON w.id=t.repository_id
         WHERE w.repo=?1 AND t.checkout_machine_id=(SELECT id FROM machines WHERE route='local')
           AND t.worktree IS NOT NULL AND t.worktree<>?1 AND t.branch IS NOT NULL",
    )?;
    let retained = statement
        .query_map([repo], |row| {
            Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
        })?
        .collect::<Result<Vec<_>, _>>()?;
    if retained.is_empty() {
        return Ok(false);
    }
    let Ok(observed) = crate::git::worktrees::list_porcelain(Path::new(repo)) else {
        return Ok(false);
    };
    let canonical = crate::repository::CanonicalRepo::discover(Path::new(repo))
        .map_err(|error| StoreError::InvalidData(error.to_string()))?;
    Ok(retained.iter().any(|(path, branch)| {
        let Ok(path) = Path::new(path).canonicalize() else {
            return false;
        };
        path.join(".git").is_file()
            && canonical.contains(&path)
            && observed.iter().any(|(checkout, observed_branch)| {
                checkout.canonicalize().ok().as_ref() == Some(&path)
                    && observed_branch.as_deref() == Some(branch.as_str())
            })
    }))
}

pub(super) fn ensure_repository_in(conn: &Connection, repo: &str) -> StoreResult<RepositoryId> {
    let common = git_locator_in(conn, repo)?;
    let locator = if let Some(common) = &common {
        crate::repository::location::read_locator(common)
            .map_err(|error| StoreError::InvalidData(error.to_string()))?
    } else {
        None
    };
    if let Some(id) = repository_id_in(conn, repo)? {
        if let Some(common) = common.as_ref().filter(|_| locator.is_none()) {
            if !retained_checkout_matches(conn, repo)? {
                return Err(StoreError::InvalidData(format!(
                "{repo} retains repository {id} but has no Git locator; establish the original checkout with `lf repository --bind {id}`, or recover the moved checkout with `lf repository --relocate {id}`"
                )));
            }
            crate::repository::location::write_locator(common, &id)
                .map_err(|error| StoreError::InvalidData(error.to_string()))?;
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
                    "Git locator conflicts with retained Work; reconcile with `lf repository`"
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
        // Planning routes reference destination addresses. Move both sides
        // before checking the retained relationship at commit.
        tx.pragma_update(None, "defer_foreign_keys", true)?;
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
             SELECT t.id,e.id FROM tasks t JOIN repository_plans w ON w.id=t.repository_id JOIN processes e
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
            ("tasks", "worktree", "checkout_machine_id=(SELECT id FROM machines WHERE route='local') AND repository_id IN (SELECT id FROM repository_plans WHERE repo=?2)"),
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
            let (checkout, owner): (Option<String>, Option<String>) = tx.query_row(
                "SELECT worktree,checkout_machine_id FROM tasks WHERE id=?1",
                [id.as_str()],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )?;
            if checkout.is_none() && !super::task_ownership::owned_in(&tx, &id)? {
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

    pub fn task_execution_route(&self, task: &TaskId) -> StoreResult<TaskExecutionRoute> {
        let mut conn = self.conn.lock().expect("store mutex poisoned");
        let tx = conn.savepoint()?;
        let (repo, checkout): (String, Option<String>) = tx.query_row(
            "SELECT w.repo,t.worktree FROM tasks t JOIN repository_plans w ON w.id=t.repository_id
             WHERE t.id=?1",
            [task.as_str()],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )?;
        let repository_id = repository_id_in(&tx, &repo)?.ok_or_else(|| {
            StoreError::InvalidData(format!("repository {repo} has no selected plan identity"))
        })?;
        let (machine_id, source) = super::task_ownership::writer_in(&tx, task)?;
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

    pub(crate) fn repository_paths(&self) -> StoreResult<Vec<PathBuf>> {
        let conn = self.conn.lock().expect("store mutex poisoned");
        let mut query =
            conn.prepare("SELECT DISTINCT repo FROM repository_plans WHERE selected=1")?;
        let paths = query
            .query_map([], |row| row.get::<_, String>(0).map(PathBuf::from))?
            .collect::<Result<Vec<_>, _>>()?;
        Ok(paths)
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
        if let Some(common) = git_locator_in(&tx, repo)? {
            crate::repository::location::write_locator(&common, id)
                .map_err(|error| StoreError::InvalidData(error.to_string()))?;
        }
        tx.commit()?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::SqliteStore;
    use crate::durable::{MachineId, TaskId};

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
            conn.execute("INSERT INTO repository_plans(id,repo,selected) SELECT 'repo_'||lower(hex(randomblob(16))),w.repo,1 FROM waves w WHERE NOT EXISTS(SELECT 1 FROM repository_plans r WHERE r.repo=w.repo AND r.selected=1) GROUP BY w.repo", []).unwrap();
            conn.execute("INSERT INTO tasks(repository_id,id,project_id,external_issue_id,issue_identifier,worktree,created_at,checkout_machine_id) VALUES((SELECT r.id FROM projects p JOIN waves w ON w.id=p.wave_id JOIN repository_plans r ON r.repo=w.repo AND r.selected=1 WHERE p.id=?2),?1,?2,'issue','PROOF-1','/before/task',1,(SELECT id FROM machines WHERE route='local'))",params![task.as_str(),project.as_str()]).unwrap();
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
    fn machine_defaults_route_unowned_tasks_until_one_is_claimed() {
        use crate::durable::{
            DefaultScope, MachineSetting, OwningMachineSource, TaskExecutionSource,
        };
        let home = tempfile::tempdir().unwrap();
        let store = SqliteStore::open_ephemeral(&home.path().join("db")).unwrap();
        let repo = home.path().canonicalize().unwrap();
        let repo = repo.to_str().unwrap();
        let (mini, worker) = (MachineId::new(), MachineId::new());
        let project = store.ensure_wave_project(repo, "work").unwrap();
        let wave = project.wave_id.clone();
        let repository = store.ensure_repository(repo).unwrap();
        let new_task = |title: &str| {
            store
                .create_task(&crate::work::task::NewTask {
                    repository_id: repository.clone(),
                    id: TaskId::new(),
                    project_id: Some(project.id.clone()),
                    title: title.into(),
                    description: String::new(),
                    due_date: None,
                })
                .unwrap()
        };
        // Unset: this Machine takes the Task, and the claim is captured.
        let local = store.local_machine().unwrap().id;
        let claimed = new_task("Claimed here");
        store.claim_task(&claimed.id).unwrap().unwrap();

        store
            .set_machine_default(
                &DefaultScope::Repository(repository.clone()),
                MachineSetting::Tasks,
                &worker,
            )
            .unwrap();
        let task = new_task("Routed");
        let route = store.task_execution_route(&task.id).unwrap();
        assert_eq!(
            (route.machine_id, route.source),
            (
                worker.clone(),
                TaskExecutionSource::OwningMachine(OwningMachineSource::Repository)
            )
        );
        // The Wave's Session Machine places no Task.
        store
            .set_machine_default(
                &DefaultScope::Wave(wave.clone()),
                MachineSetting::Session,
                &local,
            )
            .unwrap();
        assert_eq!(
            store.task_execution_route(&task.id).unwrap().machine_id,
            worker
        );
        assert_eq!(
            store
                .wave_owning_machine(&wave, MachineSetting::Session)
                .unwrap(),
            (local.clone(), OwningMachineSource::Wave("work".into()))
        );
        // The Wave's default outranks the repository's.
        store
            .set_machine_default(
                &DefaultScope::Wave(wave.clone()),
                MachineSetting::Tasks,
                &mini,
            )
            .unwrap();
        let route = store.task_execution_route(&task.id).unwrap();
        assert_eq!(
            (route.machine_id, route.source),
            (
                mini,
                TaskExecutionSource::OwningMachine(OwningMachineSource::Wave("work".into()))
            )
        );
        // Only your Machine for the Wave claims; this one refuses without writing.
        assert!(store.claim_task(&task.id).is_err());
        assert!(store.task_ownership(&task.id).unwrap().log.is_empty());
        // Captured, not inherited: a later default moves no claimed Task.
        let route = store.task_execution_route(&claimed.id).unwrap();
        assert_eq!(
            (route.machine_id, route.source),
            (local, TaskExecutionSource::Owner)
        );
        assert_eq!(store.machine_defaults().unwrap().len(), 3);
    }

    #[test]
    fn a_waves_capacity_takes_its_new_tasks_until_a_nearer_machine_does() {
        use crate::durable::{DefaultScope, MachineSetting, OwningMachineSource};
        let home = tempfile::tempdir().unwrap();
        let store = SqliteStore::open_ephemeral(&home.path().join("db")).unwrap();
        let repo = home.path().canonicalize().unwrap();
        let repo = repo.to_str().unwrap();
        let project = store.ensure_wave_project(repo, "infrastructure").unwrap();
        let wave = DefaultScope::Wave(project.wave_id.clone());
        let repository = store.ensure_repository(repo).unwrap();
        let task = store
            .create_task(&crate::work::task::NewTask {
                repository_id: repository.clone(),
                id: TaskId::new(),
                project_id: Some(project.id.clone()),
                title: "Rented".into(),
                description: String::new(),
                due_date: None,
            })
            .unwrap();
        let worker = MachineId::new();
        store
            .set_machine_default(
                &DefaultScope::Repository(repository.clone()),
                MachineSetting::Tasks,
                &worker,
            )
            .unwrap();
        assert_eq!(store.task_capacity(&task.id).unwrap(), None);

        // The Wave is nearer than the repository: its Tasks rent, and until
        // `lf task run` places one it stays on this Machine.
        store.set_tasks_capacity(&wave, "code").unwrap();
        assert_eq!(
            store.task_capacity(&task.id).unwrap().as_deref(),
            Some("code")
        );
        assert_eq!(
            store
                .wave_owning_machine(&project.wave_id, MachineSetting::Tasks)
                .unwrap(),
            (
                store.local_machine().unwrap().id,
                OwningMachineSource::Wave("infrastructure".into())
            )
        );
        assert_eq!(store.capacity_defaults().unwrap().len(), 1);

        // A Machine at the same scope replaces it.
        store
            .set_machine_default(&wave, MachineSetting::Tasks, &worker)
            .unwrap();
        assert_eq!(store.task_capacity(&task.id).unwrap(), None);
        assert!(store.capacity_defaults().unwrap().is_empty());
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
