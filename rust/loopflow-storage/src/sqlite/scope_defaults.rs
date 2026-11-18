//! Saved configuration addresses and nearest-first scope chains.
use super::SqliteStore;
use crate::{StoreError, StoreResult};
use loopflow_model::durable::{DefaultScope, MachineId, ProjectId, RepositoryId, TaskId};
use loopflow_model::id::WaveId;
use rusqlite::{Connection, OptionalExtension};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ScopeAddress {
    pub scope: DefaultScope,
    pub name: String,
    pub repo: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PlacementScope {
    pub local_machine: MachineId,
    pub person_id: Option<String>,
    pub chain: Vec<ScopeAddress>,
}

pub(crate) fn address_in(conn: &Connection, scope: &DefaultScope) -> StoreResult<ScopeAddress> {
    let (repo, name) = match scope {
        DefaultScope::Repository(id) => conn.query_row(
            "SELECT repo,'' FROM repository_plans WHERE id=?1",
            [id.as_str()],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )?,
        DefaultScope::Wave(id) => conn.query_row(
            "SELECT repo,name FROM waves WHERE id=?1",
            [id.as_str()],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )?,
    };
    Ok(ScopeAddress {
        scope: scope.clone(),
        name,
        repo,
    })
}

pub(crate) fn scope_in(
    conn: &Connection,
    mut wave: Option<String>,
    repository: Option<String>,
) -> StoreResult<PlacementScope> {
    let (local, person_id): (String, Option<String>) = conn.query_row(
        "SELECT id,person_id FROM machines WHERE is_local=1",
        [],
        |row| Ok((row.get(0)?, row.get(1)?)),
    )?;
    let mut chain = Vec::new();
    let mut visited = std::collections::HashSet::new();
    while let Some(id) = wave.filter(|id| visited.insert(id.clone())) {
        let id = WaveId::parse(&id).map_err(|error| StoreError::InvalidData(error.to_string()))?;
        chain.push(address_in(conn, &DefaultScope::Wave(id.clone()))?);
        wave = conn.query_row(
            "SELECT parent_wave_id FROM waves WHERE id=?1",
            [id.as_str()],
            |row| row.get(0),
        )?;
    }
    if let Some(repository) = repository {
        chain.push(address_in(
            conn,
            &DefaultScope::Repository(RepositoryId::from_raw(repository)),
        )?);
    }
    Ok(PlacementScope {
        local_machine: MachineId::from_raw(local),
        person_id,
        chain,
    })
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TaskScope {
    pub repository: RepositoryId,
    pub project: Option<ProjectId>,
    pub placement: PlacementScope,
}

pub(crate) fn new_task_scope_in(
    conn: &Connection,
    repository: &RepositoryId,
    project: Option<&ProjectId>,
) -> StoreResult<TaskScope> {
    let wave = project
        .map(|project| {
            conn.query_row(
                "SELECT wave_id FROM projects WHERE id=?1",
                [project.as_str()],
                |row| row.get::<_, String>(0),
            )
        })
        .transpose()?;
    Ok(TaskScope {
        repository: repository.clone(),
        project: project.cloned(),
        placement: scope_in(conn, wave, Some(repository.to_string()))?,
    })
}

pub(crate) fn task_scope_in(conn: &Connection, task: &TaskId) -> StoreResult<TaskScope> {
    let (repository, project): (String, Option<String>) = conn
        .query_row(
            "SELECT repository_id,project_id FROM tasks WHERE id=?1",
            [task.as_str()],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .optional()?
        .ok_or(StoreError::NotFound)?;
    new_task_scope_in(
        conn,
        &RepositoryId::from_raw(repository),
        project.as_deref().map(ProjectId::from_raw).as_ref(),
    )
}

impl SqliteStore {
    pub fn task_scope(&self, task: &TaskId) -> StoreResult<TaskScope> {
        let mut conn = self.conn.lock().expect("store mutex poisoned");
        let snapshot = conn.savepoint()?;
        task_scope_in(&snapshot, task)
    }

    pub fn new_task_scope(
        &self,
        repository: &RepositoryId,
        project: Option<&ProjectId>,
    ) -> StoreResult<TaskScope> {
        let mut conn = self.conn.lock().expect("store mutex poisoned");
        let snapshot = conn.savepoint()?;
        new_task_scope_in(&snapshot, repository, project)
    }

    pub fn placement_scope(
        &self,
        wave: Option<&WaveId>,
        repository: Option<&RepositoryId>,
    ) -> StoreResult<PlacementScope> {
        let mut conn = self.conn.lock().expect("store mutex poisoned");
        let snapshot = conn.savepoint()?;
        let repository = match (wave, repository) {
            (_, Some(repository)) => Some(repository.to_string()),
            (Some(wave), None) => snapshot.query_row(
                "SELECT r.id FROM waves w JOIN repository_plans r ON r.repo=w.repo AND r.selected=1 WHERE w.id=?1",
                [wave.as_str()], |row| row.get(0),
            ).optional()?,
            _ => None,
        };
        scope_in(&snapshot, wave.map(ToString::to_string), repository)
    }

    pub fn scope_address(&self, scope: &DefaultScope) -> StoreResult<ScopeAddress> {
        let conn = self.conn.lock().expect("store mutex poisoned");
        address_in(&conn, scope)
    }

    pub fn scope_addresses(&self) -> StoreResult<Vec<ScopeAddress>> {
        let mut conn = self.conn.lock().expect("store mutex poisoned");
        let snapshot = conn.savepoint()?;
        let mut query =
            snapshot.prepare("SELECT id,repo FROM repository_plans WHERE selected=1")?;
        let mut out = query
            .query_map([], |row| {
                Ok(ScopeAddress {
                    scope: DefaultScope::Repository(RepositoryId::from_raw(
                        row.get::<_, String>(0)?,
                    )),
                    repo: row.get(1)?,
                    name: row.get(1)?,
                })
            })?
            .collect::<Result<Vec<_>, _>>()?;
        let mut query =
            snapshot.prepare("SELECT id,repo,slug FROM wave_addresses WHERE retired_at IS NULL")?;
        for row in query.query_map([], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, String>(2)?,
            ))
        })? {
            let (id, repo, name) = row?;
            out.push(ScopeAddress {
                scope: DefaultScope::Wave(
                    WaveId::parse(&id)
                        .map_err(|error| StoreError::InvalidData(error.to_string()))?,
                ),
                repo,
                name,
            });
        }
        Ok(out)
    }
}
