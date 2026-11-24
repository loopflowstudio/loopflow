//! Selected planning identity, separate from a machine's local repository path.
use rusqlite::{params, Connection, OptionalExtension, TransactionBehavior};
use std::path::PathBuf;

use crate::{StoreError, StoreResult};
use loopflow_model::durable::{RepositoryId, RepositoryIdentity, TaskId};

use super::SqliteStore;

/// Saved execution destination; configuration explains an unclaimed destination above SQL.
#[derive(Debug)]
pub struct SavedTaskRoute {
    pub checkout: Option<String>,
    pub repository_id: RepositoryId,
    pub machine_id: loopflow_model::durable::MachineId,
    pub claimed: bool,
}

/// Saved inputs used to discover a repository outside the connection lock.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RepositoryRegistration {
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
    /// Move current addresses together. Captures and provider history retain
    /// the paths at which they were recorded.
    pub fn relocate_repository(&self, id: &RepositoryId, destination: &str) -> StoreResult<()> {
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
        for table in [
            "repository_plans",
            "waves",
            "agent_sessions",
            "tracker_projects",
            "tracker_items",
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
    ) -> StoreResult<crate::execution::TaskLocationObservation> {
        use crate::execution::{TaskLocation, TaskLocationObservation};
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
            let id = super::children::resolve_task_id_in(
                &tx,
                task.as_str(),
                Some(std::slice::from_ref(&repo)),
                None,
            )?
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

    pub fn task_execution_route_prepared(
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

    pub fn repository_registration(&self, repo: &str) -> StoreResult<RepositoryRegistration> {
        let mut conn = self.conn.lock().expect("store mutex poisoned");
        let snapshot = conn.savepoint()?;
        registration_in(&snapshot, repo)
    }

    /// Recheck saved acquisition inputs and select the supplied identity atomically.
    pub fn register_repository(
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

    /// Let `id`, the name another Machine gives this repository, find the
    /// checkout at `repo` too. The selected identity and every saved row stay.
    /// `false` when `id` already names another checkout on this Machine.
    pub fn locate_repository(&self, repo: &str, id: &RepositoryId) -> StoreResult<bool> {
        let conn = self.conn.lock().expect("store mutex poisoned");
        conn.execute(
            "INSERT INTO repository_plans(repo,id,selected) VALUES(?1,?2,0)
             ON CONFLICT(id) DO NOTHING",
            params![repo, id.as_str()],
        )?;
        let located: String = conn.query_row(
            "SELECT repo FROM repository_plans WHERE id=?1",
            [id.as_str()],
            |row| row.get(0),
        )?;
        Ok(located == repo)
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

    pub fn repository_paths(&self) -> StoreResult<Vec<PathBuf>> {
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

    /// This Machine's path for the repository a Task or Wave belongs to.
    pub fn work_repository_path(
        &self,
        task: Option<&TaskId>,
        wave: Option<&loopflow_model::id::WaveId>,
    ) -> StoreResult<Option<String>> {
        let conn = self.conn.lock().expect("store mutex poisoned");
        conn.query_row(
            "SELECT coalesce(
                (SELECT r.repo FROM tasks t JOIN repository_plans r ON r.id=t.repository_id
                 WHERE t.id=?1),
                (SELECT repo FROM waves WHERE id=?2))",
            params![task.map(TaskId::as_str), wave.map(|wave| wave.as_str())],
            |row| row.get(0),
        )
        .map_err(Into::into)
    }
}

#[cfg(test)]
mod tests {

    #[test]
    fn relocation_rolls_back_and_moves_nested_checkouts() {
        use loopflow_model::durable::{ProjectId, TaskId};
        use loopflow_model::id::WaveId;
        use rusqlite::params;
        let dir = tempfile::tempdir().unwrap();
        let store = crate::test_fixture::open(&dir.path().join("db")).unwrap();
        let id = crate::test_fixture::repository(&store, "/before");
        let wave = WaveId::new();
        let project = ProjectId::new();
        let task = TaskId::new();
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
        let conn = store.conn.lock().unwrap();
        let worktree: String = conn
            .query_row(
                "SELECT worktree FROM task_checkouts WHERE task_id=?1",
                [task.as_str()],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(worktree, "/after/task");
    }
}
