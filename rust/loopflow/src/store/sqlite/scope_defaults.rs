//! Your defaults by Wave or repository, stored on each of your Machines.
//!
//! One scope chain serves every setting: Wave → parent Waves → repository.
//! Machine is the first setting, in two independent parts: where a scope's new
//! Tasks run and where its primary Session runs. A Task's own Machine is its
//! ownership claim.
use rusqlite::{params, Connection, OptionalExtension};

use crate::durable::{
    DefaultScope, MachineDefault, MachineId, MachineSetting, OwningMachineSource, RepositoryId,
};
use crate::id::WaveId;
use crate::store::{StoreError, StoreResult};

use super::SqliteStore;

fn scope_key(scope: &DefaultScope) -> (&'static str, &str) {
    match scope {
        DefaultScope::Wave(wave) => ("wave", wave.as_str()),
        DefaultScope::Repository(repository) => ("repository", repository.as_str()),
    }
}

fn column(setting: MachineSetting) -> &'static str {
    match setting {
        MachineSetting::Tasks => "tasks_machine_id",
        MachineSetting::Session => "session_machine_id",
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

fn machine_at_in(
    conn: &Connection,
    scope: &DefaultScope,
    setting: MachineSetting,
) -> StoreResult<Option<MachineId>> {
    let (kind, id) = scope_key(scope);
    Ok(conn
        .query_row(
            &format!(
                "SELECT {} FROM scope_defaults WHERE scope_kind=?1 AND scope_id=?2",
                column(setting)
            ),
            [kind, id],
            |row| row.get::<_, Option<String>>(0),
        )
        .optional()?
        .flatten()
        .map(MachineId::from_raw))
}

/// Your Machine for one setting under a Wave and repository, else this Machine.
pub(super) fn owning_machine_in(
    conn: &Connection,
    wave: Option<String>,
    repository: Option<String>,
    setting: MachineSetting,
) -> StoreResult<(MachineId, OwningMachineSource)> {
    for (scope, name) in chain_in(conn, wave, repository)? {
        if let Some(machine) = machine_at_in(conn, &scope, setting)? {
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
    /// Your Machine for a Wave's new Tasks or its Wave Session.
    pub fn wave_owning_machine(
        &self,
        wave: &WaveId,
        setting: MachineSetting,
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
        owning_machine_in(&snapshot, Some(wave.to_string()), repository, setting)
    }

    /// Your Machine for a repository's Session or its Tasks with no Wave default.
    pub fn repository_owning_machine(
        &self,
        repository: &RepositoryId,
        setting: MachineSetting,
    ) -> StoreResult<(MachineId, OwningMachineSource)> {
        let mut conn = self.conn.lock().expect("store mutex poisoned");
        let snapshot = conn.savepoint()?;
        owning_machine_in(&snapshot, None, Some(repository.to_string()), setting)
    }

    /// Set one Machine setting at a scope. Existing Tasks keep their claims.
    pub fn set_machine_default(
        &self,
        scope: &DefaultScope,
        setting: MachineSetting,
        machine: &MachineId,
    ) -> StoreResult<()> {
        let (kind, id) = scope_key(scope);
        let column = column(setting);
        let conn = self.conn.lock().expect("store mutex poisoned");
        conn.execute(
            &format!(
                "INSERT INTO scope_defaults(scope_kind,scope_id,{column},set_at) VALUES(?1,?2,?3,?4)
                 ON CONFLICT(scope_kind,scope_id) DO UPDATE SET {column}=excluded.{column},set_at=excluded.set_at"
            ),
            params![kind, id, machine.as_str(), crate::store::rows::now_unix()],
        )?;
        Ok(())
    }

    /// Every Machine default on this Machine, Waves first, Tasks before Session.
    pub fn machine_defaults(&self) -> StoreResult<Vec<MachineDefault>> {
        let conn = self.conn.lock().expect("store mutex poisoned");
        let mut statement = conn.prepare(
            "SELECT d.scope_kind,d.scope_id,d.tasks_machine_id,d.session_machine_id,
                    coalesce(w.name,r.repo,d.scope_id)
             FROM scope_defaults d
             LEFT JOIN waves w ON d.scope_kind='wave' AND w.id=d.scope_id
             LEFT JOIN repository_plans r ON d.scope_kind='repository' AND r.id=d.scope_id AND r.selected=1
             ORDER BY d.scope_kind DESC,5",
        )?;
        let rows = statement.query_map([], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, Option<String>>(2)?,
                row.get::<_, Option<String>>(3)?,
                row.get::<_, String>(4)?,
            ))
        })?;
        let mut defaults = Vec::new();
        for row in rows {
            let (kind, id, tasks, session, name) = row?;
            let scope = if kind == "wave" {
                DefaultScope::Wave(parse_wave(&id)?)
            } else {
                DefaultScope::Repository(RepositoryId::from_raw(id))
            };
            for (setting, machine) in [
                (MachineSetting::Tasks, tasks),
                (MachineSetting::Session, session),
            ] {
                if let Some(machine) = machine {
                    defaults.push(MachineDefault {
                        scope: scope.clone(),
                        name: name.clone(),
                        setting,
                        machine_id: MachineId::from_raw(machine),
                    });
                }
            }
        }
        Ok(defaults)
    }
}
