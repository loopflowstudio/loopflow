//! Selected planning identity, separate from a machine's local repository path.
use rusqlite::{params, Connection, OptionalExtension, TransactionBehavior};
use std::path::PathBuf;

use crate::store::{StoreError, StoreResult};
use loopflow_model::durable::{RepositoryId, RepositoryIdentity, TaskId};

use super::SqliteStore;

/// Saved execution destination; configuration explains an unclaimed destination above SQL.
#[derive(Debug)]
pub(crate) struct SavedTaskRoute {
    pub(crate) checkout: Option<String>,
    pub(crate) repository_id: RepositoryId,
    pub(crate) machine_id: loopflow_model::durable::MachineId,
    pub(crate) claimed: bool,
}

/// Saved inputs used to discover a repository outside the connection lock.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct RepositoryRegistration {
    pub machine: loopflow_model::durable::MachineId,
    pub selected: Option<RepositoryId>,
    pub identities: Vec<RepositoryId>,
    pub checkouts: Vec<(String, String)>,
}

pub(super) fn registration_in(
    conn: &Connection,
    repo: &str,
) -> StoreResult<RepositoryRegistration> {
    let mut identities =
        conn.prepare("SELECT id FROM repository_plans WHERE repo=?1 ORDER BY id")?;
    let identities = identities
        .query_map([repo], |row| row.get::<_, String>(0))?
        .map(|row| {
            RepositoryId::parse(&row?).map_err(|error| StoreError::InvalidData(error.to_string()))
        })
        .collect::<StoreResult<Vec<_>>>()?;
    // Start from the bounded repository aliases even when measured history
    // makes the planner prefer scanning Tasks or checkouts.
    let mut checkouts = conn.prepare(
        "SELECT c.worktree,c.branch FROM repository_plans w
         CROSS JOIN tasks t ON t.repository_id=w.id
         CROSS JOIN task_checkouts c ON c.task_id=t.id
         WHERE w.repo=?1 AND c.worktree<>?1",
    )?;
    let mut checkouts = checkouts
        .query_map([repo], |row| Ok((row.get(0)?, row.get(1)?)))?
        .collect::<Result<Vec<_>, _>>()?;
    checkouts.sort_unstable();
    Ok(RepositoryRegistration {
        machine: super::task_ownership::local_machine_in(conn)?,
        selected: repository_id_in(conn, repo)?,
        identities,
        checkouts,
    })
}

pub(super) fn register_in(
    conn: &Connection,
    repo: &str,
    inputs: &RepositoryRegistration,
    id: &RepositoryId,
    bind: bool,
) -> StoreResult<bool> {
    if registration_in(conn, repo)? != *inputs {
        return Ok(false);
    }
    let bound: Option<String> = conn
        .query_row(
            "SELECT repo FROM repository_plans WHERE id=?1",
            [id.as_str()],
            |row| row.get(0),
        )
        .optional()?;
    if bound.as_deref().is_some_and(|other| other != repo) {
        return Err(StoreError::InvalidData(format!(
                "repository identity {id} already locates {}; cannot associate another local checkout {repo}",
                bound.expect("conflicting path is present")
            )));
    }
    if bind {
        conn.execute(
            "UPDATE repository_plans SET selected=0 WHERE repo=?1 AND selected=1 AND id!=?2",
            params![repo, id.as_str()],
        )?;
    }
    conn.execute(
        "INSERT INTO repository_plans(repo,id,selected) VALUES(?1,?2,1)
             ON CONFLICT(id) DO UPDATE SET selected=1 WHERE selected=0",
        params![repo, id.as_str()],
    )?;
    Ok(true)
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
             SELECT t.id,e.id FROM task_checkouts c JOIN tasks t ON t.id=c.task_id
               JOIN repository_plans w ON w.id=t.repository_id JOIN processes e
               ON e.cwd=rtrim(c.worktree,'/') OR instr(e.cwd,rtrim(c.worktree,'/')||'/')=1
             WHERE w.repo=?1 AND (c.worktree=?1 OR instr(c.worktree,?1||'/')=1)",
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
            ("task_checkouts", "worktree", "task_id IN (SELECT t.id FROM tasks t JOIN repository_plans w ON w.id=t.repository_id WHERE w.repo=?2)"),
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
        // One snapshot that only reads: a savepoint, not the write lock.
        let tx = conn.savepoint()?;
        let machine = super::durable::map_local_machine(&tx)?.id;
        let location = (|| -> StoreResult<TaskLocation> {
            let repo: String = tx.query_row(
                "SELECT repo FROM repository_plans WHERE id=?1",
                [repository.as_str()],
                |row| row.get(0),
            )?;
            let id = super::children::resolve_task_id_in(&tx, task.as_str(), Some(&repo))?
                .ok_or(StoreError::NotFound)?;
            let checkout: Option<String> = tx
                .query_row(
                    "SELECT worktree FROM task_checkouts WHERE task_id=?1",
                    [id.as_str()],
                    |row| row.get(0),
                )
                .optional()?;
            if checkout.is_none() {
                return Ok(if super::task_ownership::owned_in(&tx, &id)? {
                    TaskLocation::Unavailable {
                        reason: "not checked out on this Machine; read its owner".into(),
                    }
                } else {
                    TaskLocation::Unrecorded
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

    pub(crate) fn task_execution_route_prepared(
        &self,
        task: &TaskId,
        me: &super::task_ownership::OwnershipFacts,
    ) -> StoreResult<Option<SavedTaskRoute>> {
        let mut conn = self.conn.lock().expect("store mutex poisoned");
        let tx = conn.savepoint()?;
        if !me.matches_task_in(&tx, task)? {
            return Ok(None);
        }
        let (repo, checkout): (String, Option<String>) = tx.query_row(
            "SELECT w.repo,c.worktree FROM tasks t JOIN repository_plans w ON w.id=t.repository_id
             LEFT JOIN task_checkouts c ON c.task_id=t.id WHERE t.id=?1",
            [task.as_str()],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )?;
        let repository_id = repository_id_in(&tx, &repo)?.ok_or_else(|| {
            StoreError::InvalidData(format!("repository {repo} has no selected plan identity"))
        })?;
        let (machine_id, claimed) = super::task_ownership::writer_in(&tx, task, me)?;
        Ok(Some(SavedTaskRoute {
            checkout,
            repository_id,
            machine_id,
            claimed,
        }))
    }

    pub(crate) fn repository_registration(
        &self,
        repo: &str,
    ) -> StoreResult<RepositoryRegistration> {
        let mut conn = self.conn.lock().expect("store mutex poisoned");
        let snapshot = conn.savepoint()?;
        registration_in(&snapshot, repo)
    }

    /// Recheck saved acquisition inputs and select the supplied identity atomically.
    pub(crate) fn register_repository(
        &self,
        repo: &str,
        inputs: &RepositoryRegistration,
        id: &RepositoryId,
        bind: bool,
    ) -> StoreResult<bool> {
        let mut conn = self.conn.lock().expect("store mutex poisoned");
        if inputs.selected.as_ref() == Some(id) {
            let snapshot = conn.savepoint()?;
            return Ok(registration_in(&snapshot, repo)? == *inputs);
        }
        let tx = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
        if !register_in(&tx, repo, inputs, id, bind)? {
            return Ok(false);
        }
        tx.commit()?;
        Ok(true)
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
}

#[cfg(test)]
mod tests {
    use super::SqliteStore;
    use loopflow_model::durable::{MachineId, TaskId};

    #[test]
    fn relocation_rolls_back_and_retains_nested_task_processes() {
        use loopflow_model::durable::{ProjectId, TaskId};
        use loopflow_model::id::{LfProcessId, TraceId, WaveId};
        use rusqlite::params;
        let dir = tempfile::tempdir().unwrap();
        let store = SqliteStore::open_ephemeral(&dir.path().join("db")).unwrap();
        let id = crate::repository::location::ensure_repository(&store, "/before").unwrap();
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
            conn.execute("INSERT INTO tasks(repository_id,id,project_id,external_issue_id,issue_identifier,created_at) VALUES((SELECT r.id FROM projects p JOIN waves w ON w.id=p.wave_id JOIN repository_plans r ON r.repo=w.repo AND r.selected=1 WHERE p.id=?2),?1,?2,'issue','PROOF-1',1)",params![task.as_str(),project.as_str()]).unwrap();
            conn.execute(
                "INSERT INTO task_checkouts(task_id,worktree) VALUES(?1,'/before/task')",
                [task.as_str()],
            )
            .unwrap();
            conn.execute("INSERT INTO processes(id,trace_id,cwd,started_at) VALUES(?1,?2,'/before/task/src',1)",params![process,TraceId::new()]).unwrap();
            conn.execute_batch("CREATE TRIGGER interrupt_move BEFORE UPDATE OF worktree ON task_checkouts BEGIN SELECT RAISE(ABORT,'interrupted'); END;").unwrap();
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
        assert_eq!(
            store.task_work(&task).unwrap().processes[0].id(),
            process.as_str()
        );
        assert_eq!(
            store.task_open_processes(&task).unwrap()[0].id(),
            process.as_str()
        );
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
        use crate::durable::{OwningMachineSource, TaskExecutionSource};
        use loopflow_model::durable::{DefaultScope, MachineSetting};
        let _env = crate::journal::test_env_lock();
        let home = tempfile::tempdir().unwrap();
        let _home =
            crate::lf::commands::graph::EnvVarGuard::set("LF_HOME", home.path().to_str().unwrap());
        let store = SqliteStore::open_ephemeral(&home.path().join("db")).unwrap();
        let repo = home.path().canonicalize().unwrap();
        let repo = repo.to_str().unwrap();
        let (mini, worker) = (MachineId::new(), MachineId::new());
        let project =
            crate::work::wave::bootstrap::ensure_wave_project(&store, repo, "work").unwrap();
        let wave = project.wave_id.clone();
        let repository = crate::repository::location::ensure_repository(&store, repo).unwrap();
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
        let local = crate::store::machines::local_machine(&store).unwrap().id;
        let claimed = new_task("Claimed here");
        crate::store::ownership::claim_task(&store, &claimed.id)
            .unwrap()
            .unwrap();

        crate::store::placement::set_machine_default(
            &store,
            &DefaultScope::Repository(repository.clone()),
            MachineSetting::Tasks,
            &worker,
        )
        .unwrap();
        let task = new_task("Routed");
        let route = crate::store::ownership::task_execution_route(&store, &task.id).unwrap();
        assert_eq!(route.machine_id, worker);
        assert!(
            matches!(route.source, TaskExecutionSource::OwningMachine(OwningMachineSource::Preference(selected)) if selected.scope == crate::config::settings::SettingScope::Repository)
        );
        // The Wave's Session Machine places no Task.
        crate::store::placement::set_machine_default(
            &store,
            &DefaultScope::Wave(wave.clone()),
            MachineSetting::Session,
            &local,
        )
        .unwrap();
        assert_eq!(
            crate::store::ownership::task_execution_route(&store, &task.id)
                .unwrap()
                .machine_id,
            worker
        );
        assert_eq!(
            crate::store::placement::wave_owning_machine(&store, &wave, MachineSetting::Session)
                .unwrap()
                .0,
            local.clone()
        );
        // The Wave's default outranks the repository's.
        crate::store::placement::set_machine_default(
            &store,
            &DefaultScope::Wave(wave.clone()),
            MachineSetting::Tasks,
            &mini,
        )
        .unwrap();
        let route = crate::store::ownership::task_execution_route(&store, &task.id).unwrap();
        assert_eq!(route.machine_id, mini);
        assert!(
            matches!(route.source, TaskExecutionSource::OwningMachine(OwningMachineSource::Preference(selected)) if selected.scope == crate::config::settings::SettingScope::Wave)
        );
        // Only your Machine for the Wave claims; this one refuses without writing.
        assert!(crate::store::ownership::claim_task(&store, &task.id).is_err());
        assert!(crate::store::ownership::task_ownership(&store, &task.id)
            .unwrap()
            .log
            .is_empty());
        // Captured, not inherited: a later default moves no claimed Task.
        let route = crate::store::ownership::task_execution_route(&store, &claimed.id).unwrap();
        assert_eq!(
            (route.machine_id, route.source),
            (local, TaskExecutionSource::Owner)
        );
        assert_eq!(
            crate::store::placement::machine_defaults(&store)
                .unwrap()
                .len(),
            3
        );
    }

    #[test]
    fn a_waves_capacity_takes_its_new_tasks_until_a_nearer_machine_does() {
        use loopflow_model::durable::{DefaultScope, MachineSetting};
        let home = tempfile::tempdir().unwrap();
        let store = SqliteStore::open_ephemeral(&home.path().join("db")).unwrap();
        let repo = home.path().canonicalize().unwrap();
        let repo = repo.to_str().unwrap();
        let project =
            crate::work::wave::bootstrap::ensure_wave_project(&store, repo, "infrastructure")
                .unwrap();
        let wave = DefaultScope::Wave(project.wave_id.clone());
        let repository = crate::repository::location::ensure_repository(&store, repo).unwrap();
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
        crate::store::placement::set_machine_default(
            &store,
            &DefaultScope::Repository(repository.clone()),
            MachineSetting::Tasks,
            &worker,
        )
        .unwrap();
        assert_eq!(
            crate::store::placement::task_capacity(&store, &task.id).unwrap(),
            None
        );

        // The Wave is nearer than the repository: its Tasks rent, and until
        // `lf task run` places one it stays on this Machine.
        crate::store::placement::set_tasks_capacity(&store, &wave, "code").unwrap();
        assert_eq!(
            crate::store::placement::task_capacity(&store, &task.id)
                .unwrap()
                .as_deref(),
            Some("code")
        );
        assert_eq!(
            crate::store::placement::wave_owning_machine(
                &store,
                &project.wave_id,
                MachineSetting::Tasks
            )
            .unwrap()
            .0,
            crate::store::machines::local_machine(&store).unwrap().id
        );
        assert_eq!(
            crate::store::placement::capacity_defaults(&store)
                .unwrap()
                .len(),
            1
        );

        // A Machine at the same scope replaces it.
        crate::store::placement::set_machine_default(&store, &wave, MachineSetting::Tasks, &worker)
            .unwrap();
        assert_eq!(
            crate::store::placement::task_capacity(&store, &task.id).unwrap(),
            None
        );
        assert!(crate::store::placement::capacity_defaults(&store)
            .unwrap()
            .is_empty());
    }

    #[test]
    fn repository_plans_are_explicit_and_do_not_depend_on_clone_names() {
        let a = tempfile::tempdir().unwrap();
        let b = tempfile::tempdir().unwrap();
        let first = SqliteStore::open_ephemeral(&a.path().join("db")).unwrap();
        let second = SqliteStore::open_ephemeral(&b.path().join("db")).unwrap();
        let id = crate::repository::location::ensure_repository(&first, "/src/same").unwrap();
        assert_eq!(
            crate::repository::location::ensure_repository(&first, "/src/same").unwrap(),
            id
        );
        let other = crate::repository::location::ensure_repository(&second, "/src/same").unwrap();
        assert_ne!(id, other);
        crate::repository::location::bind_repository(&second, "/src/same", &id).unwrap();
        assert_eq!(second.repository_id("/src/same").unwrap(), Some(id.clone()));
        assert_eq!(
            second.repository_path(&other).unwrap().as_deref(),
            Some("/src/same")
        );
        assert_eq!(
            crate::repository::location::ensure_repository(&second, "/src/same").unwrap(),
            id
        );
        crate::repository::location::bind_repository(&second, "/src/same", &id).unwrap();
        assert_eq!(
            second.repository_path(&id).unwrap().as_deref(),
            Some("/src/same")
        );
        assert_eq!(
            first.repository_path(&id).unwrap().as_deref(),
            Some("/src/same")
        );
        let unrelated =
            crate::repository::location::ensure_repository(&second, "/another/clone").unwrap();
        assert!(
            crate::repository::location::bind_repository(&second, "/another/clone", &id).is_err()
        );
        assert_eq!(
            second.repository_id("/another/clone").unwrap(),
            Some(unrelated)
        );
        crate::repository::location::bind_repository(&second, "/src/same", &other).unwrap();
        assert_eq!(second.repository_id("/src/same").unwrap(), Some(other));
        assert_eq!(
            second.repository_path(&id).unwrap().as_deref(),
            Some("/src/same")
        );
        assert!(first.repository_id("/unknown").unwrap().is_none());
    }
}
