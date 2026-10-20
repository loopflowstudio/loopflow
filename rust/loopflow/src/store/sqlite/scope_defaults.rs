//! Your defaults by Wave or repository, stored on each of your Machines.
//!
//! One scope chain serves every setting: Wave → parent Waves → repository.
//! Machine is the first setting; a Task's own Machine is its ownership claim.
use rusqlite::{params, Connection, OptionalExtension};

use crate::durable::{DefaultScope, MachineDefault, MachineId, OwningMachineSource, RepositoryId};
use crate::id::WaveId;
use crate::store::{StoreError, StoreResult};

use super::SqliteStore;

fn scope_key(scope: &DefaultScope) -> (&'static str, &str) {
    match scope {
        DefaultScope::Wave(wave) => ("wave", wave.as_str()),
        DefaultScope::Repository(repository) => ("repository", repository.as_str()),
    }
}

fn parse_wave(id: &str) -> StoreResult<WaveId> {
    WaveId::parse(id).map_err(|error| StoreError::InvalidData(error.to_string()))
}

/// The scopes above a Task, nearest first, each with its display name.
fn chain_in(
    conn: &Connection,
    mut wave: Option<String>,
    repository: Option<String>,
) -> StoreResult<Vec<(DefaultScope, String)>> {
    let mut chain = Vec::new();
    let mut visited = std::collections::HashSet::new();
    while let Some(id) = wave.filter(|id| visited.insert(id.clone())) {
        let (name, parent): (String, Option<String>) = conn.query_row(
            "SELECT name,parent_wave_id FROM waves WHERE id=?1",
            [&id],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )?;
        chain.push((DefaultScope::Wave(parse_wave(&id)?), name));
        wave = parent;
    }
    if let Some(repository) = repository {
        chain.push((
            DefaultScope::Repository(RepositoryId::from_raw(repository)),
            String::new(),
        ));
    }
    Ok(chain)
}

fn machine_at_in(conn: &Connection, scope: &DefaultScope) -> StoreResult<Option<MachineId>> {
    let (kind, id) = scope_key(scope);
    Ok(conn
        .query_row(
            "SELECT machine_id FROM scope_defaults WHERE scope_kind=?1 AND scope_id=?2",
            [kind, id],
            |row| row.get::<_, Option<String>>(0),
        )
        .optional()?
        .flatten()
        .map(MachineId::from_raw))
}

/// Your Machine for new work under a Wave and repository, else this Machine.
pub(super) fn owning_machine_in(
    conn: &Connection,
    wave: Option<String>,
    repository: Option<String>,
) -> StoreResult<(MachineId, OwningMachineSource)> {
    for (scope, name) in chain_in(conn, wave, repository)? {
        if let Some(machine) = machine_at_in(conn, &scope)? {
            let source = match scope {
                DefaultScope::Wave(_) => OwningMachineSource::Wave(name),
                DefaultScope::Repository(_) => OwningMachineSource::Repository,
            };
            return Ok((machine, source));
        }
    }
    Ok((
        super::task_ownership::local_machine_in(conn)?,
        OwningMachineSource::ThisMachine,
    ))
}

impl SqliteStore {
    /// Your Machine for a Wave's new Tasks and its Wave Session.
    pub fn wave_owning_machine(
        &self,
        wave: &WaveId,
    ) -> StoreResult<(MachineId, OwningMachineSource)> {
        let mut conn = self.conn.lock().expect("store mutex poisoned");
        let snapshot = conn.savepoint()?;
        let repository: Option<String> = snapshot
            .query_row(
                "SELECT r.id FROM waves w JOIN repository_plans r ON r.repo=w.repo AND r.selected=1
                 WHERE w.id=?1",
                [wave.as_str()],
                |row| row.get(0),
            )
            .optional()?;
        owning_machine_in(&snapshot, Some(wave.to_string()), repository)
    }

    /// Your Machine for a repository's Session and its Tasks with no Wave default.
    pub fn repository_owning_machine(
        &self,
        repository: &RepositoryId,
    ) -> StoreResult<(MachineId, OwningMachineSource)> {
        let mut conn = self.conn.lock().expect("store mutex poisoned");
        let snapshot = conn.savepoint()?;
        owning_machine_in(&snapshot, None, Some(repository.to_string()))
    }

    /// Set your Machine at a scope. Existing Tasks keep their claims.
    pub fn set_machine_default(
        &self,
        scope: &DefaultScope,
        machine: &MachineId,
    ) -> StoreResult<()> {
        let (kind, id) = scope_key(scope);
        let conn = self.conn.lock().expect("store mutex poisoned");
        conn.execute(
            "INSERT INTO scope_defaults(scope_kind,scope_id,machine_id,set_at) VALUES(?1,?2,?3,?4)
             ON CONFLICT(scope_kind,scope_id) DO UPDATE SET machine_id=excluded.machine_id,set_at=excluded.set_at",
            params![kind, id, machine.as_str(), crate::store::rows::now_unix()],
        )?;
        Ok(())
    }

    /// Every Machine default on this Machine, Waves first.
    pub fn machine_defaults(&self) -> StoreResult<Vec<MachineDefault>> {
        let conn = self.conn.lock().expect("store mutex poisoned");
        let mut statement = conn.prepare(
            "SELECT d.scope_kind,d.scope_id,d.machine_id,coalesce(w.name,r.repo,d.scope_id)
             FROM scope_defaults d
             LEFT JOIN waves w ON d.scope_kind='wave' AND w.id=d.scope_id
             LEFT JOIN repository_plans r ON d.scope_kind='repository' AND r.id=d.scope_id AND r.selected=1
             WHERE d.machine_id IS NOT NULL ORDER BY d.scope_kind DESC,4",
        )?;
        let rows = statement.query_map([], |row| {
            let id: String = row.get(1)?;
            Ok(MachineDefault {
                scope: if row.get::<_, String>(0)? == "wave" {
                    DefaultScope::Wave(parse_wave(&id).map_err(|error| {
                        rusqlite::Error::FromSqlConversionFailure(
                            1,
                            rusqlite::types::Type::Text,
                            Box::new(error),
                        )
                    })?)
                } else {
                    DefaultScope::Repository(RepositoryId::from_raw(id))
                },
                machine_id: MachineId::from_raw(row.get::<_, String>(2)?),
                name: row.get(3)?,
            })
        })?;
        Ok(rows.collect::<Result<_, _>>()?)
    }
}
