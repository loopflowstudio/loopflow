//! Your defaults by Wave or repository, stored on each of your Machines.
//!
//! One scope chain serves every setting: Wave → parent Waves → repository.
//! Machine is the first setting, in two independent parts: where a scope's new
//! Tasks run and where its primary Session runs. A Task's own Machine is its
//! ownership claim. In place of a Tasks Machine, a scope may name capacity: a
//! pool or on-demand that `lf task run` takes a machine from.
//!
//! Scope settings are your overrides of shared setup (`graph`, `agent`,
//! `skill_agents.<skill>`) for a Wave, a repository or every repository.
use std::collections::BTreeMap;

use rusqlite::{params, Connection, OptionalExtension};

use crate::durable::{
    CapacityDefault, DefaultScope, MachineDefault, MachineId, MachineSetting, OwningMachineSource,
    RepositoryId, TaskId,
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

fn capacity_at_in(conn: &Connection, scope: &DefaultScope) -> StoreResult<Option<String>> {
    let (kind, id) = scope_key(scope);
    Ok(conn
        .query_row(
            "SELECT tasks_capacity FROM scope_defaults WHERE scope_kind=?1 AND scope_id=?2",
            [kind, id],
            |row| row.get::<_, Option<String>>(0),
        )
        .optional()?
        .flatten())
}

/// Your Machine for one setting under a Wave and repository, else this Machine.
/// A scope naming capacity keeps its Tasks here until `lf task run` places them.
pub(super) fn owning_machine_in(
    conn: &Connection,
    wave: Option<String>,
    repository: Option<String>,
    setting: MachineSetting,
) -> StoreResult<(MachineId, OwningMachineSource)> {
    for (scope, name) in chain_in(conn, wave, repository)? {
        let source = || match &scope {
            DefaultScope::Wave(_) => OwningMachineSource::Wave(name.clone()),
            DefaultScope::Repository(_) => OwningMachineSource::Repository,
        };
        if let Some(machine) = machine_at_in(conn, &scope, setting)? {
            return Ok((machine, source()));
        }
        if setting == MachineSetting::Tasks && capacity_at_in(conn, &scope)?.is_some() {
            return Ok((super::task_ownership::local_machine_in(conn)?, source()));
        }
    }
    Ok((
        super::task_ownership::local_machine_in(conn)?,
        OwningMachineSource::ThisMachine,
    ))
}

/// Your overrides of shared setup that apply in one repository and Wave, by
/// key, for each scope.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ScopeSettings {
    pub global: BTreeMap<String, String>,
    pub repository: BTreeMap<String, String>,
    pub wave: BTreeMap<String, String>,
}

fn settings_key(scope: Option<&DefaultScope>) -> (&'static str, &str) {
    scope.map_or(("global", ""), scope_key)
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

    /// Set one Machine setting at a scope; a Tasks Machine replaces the
    /// scope's capacity. Existing Tasks keep their claims.
    pub fn set_machine_default(
        &self,
        scope: &DefaultScope,
        setting: MachineSetting,
        machine: &MachineId,
    ) -> StoreResult<()> {
        let (kind, id) = scope_key(scope);
        let column = column(setting);
        let capacity = if setting == MachineSetting::Tasks {
            ",tasks_capacity=NULL"
        } else {
            ""
        };
        let conn = self.conn.lock().expect("store mutex poisoned");
        conn.execute(
            &format!(
                "INSERT INTO scope_defaults(scope_kind,scope_id,{column},set_at) VALUES(?1,?2,?3,?4)
                 ON CONFLICT(scope_kind,scope_id) DO UPDATE SET {column}=excluded.{column},set_at=excluded.set_at{capacity}"
            ),
            params![kind, id, machine.as_str(), crate::store::rows::now_unix()],
        )?;
        Ok(())
    }

    /// Clear one Machine setting at a scope, with the Tasks capacity it may
    /// name; false when it held none.
    pub fn clear_machine_default(
        &self,
        scope: &DefaultScope,
        setting: MachineSetting,
    ) -> StoreResult<bool> {
        let (kind, id) = scope_key(scope);
        let (set, held) = match setting {
            MachineSetting::Tasks => (
                "tasks_machine_id=NULL,tasks_capacity=NULL",
                "(tasks_machine_id IS NOT NULL OR tasks_capacity IS NOT NULL)",
            ),
            MachineSetting::Session => {
                ("session_machine_id=NULL", "session_machine_id IS NOT NULL")
            }
        };
        let mut conn = self.conn.lock().expect("store mutex poisoned");
        let transaction = conn.transaction()?;
        let cleared = transaction.execute(
            &format!(
                "UPDATE scope_defaults SET {set} WHERE scope_kind=?1 AND scope_id=?2 AND {held}"
            ),
            [kind, id],
        )? > 0;
        transaction.execute(
            "DELETE FROM scope_defaults WHERE scope_kind=?1 AND scope_id=?2
             AND tasks_machine_id IS NULL AND session_machine_id IS NULL AND tasks_capacity IS NULL",
            [kind, id],
        )?;
        transaction.commit()?;
        Ok(cleared)
    }

    /// Set your override of a shared setting; `None` is every repository.
    pub fn set_scope_setting(
        &self,
        scope: Option<&DefaultScope>,
        key: &str,
        value: &str,
    ) -> StoreResult<()> {
        let (kind, id) = settings_key(scope);
        let conn = self.conn.lock().expect("store mutex poisoned");
        conn.execute(
            "INSERT INTO scope_settings(scope_kind,scope_id,key,value,set_at) VALUES(?1,?2,?3,?4,?5)
             ON CONFLICT(scope_kind,scope_id,key) DO UPDATE SET value=excluded.value,set_at=excluded.set_at",
            params![kind, id, key, value, crate::store::rows::now_unix()],
        )?;
        Ok(())
    }

    /// Clear your override of a shared setting; false when there was none.
    pub fn clear_scope_setting(
        &self,
        scope: Option<&DefaultScope>,
        key: &str,
    ) -> StoreResult<bool> {
        let (kind, id) = settings_key(scope);
        let conn = self.conn.lock().expect("store mutex poisoned");
        Ok(conn.execute(
            "DELETE FROM scope_settings WHERE scope_kind=?1 AND scope_id=?2 AND key=?3",
            [kind, id, key],
        )? > 0)
    }

    /// Whether you have set any override, so readers skip finding the repository.
    pub fn has_scope_settings(&self) -> StoreResult<bool> {
        let conn = self.conn.lock().expect("store mutex poisoned");
        // A store older than this build has no table, and so no values.
        Ok(conn.query_row(
            "SELECT EXISTS(SELECT 1 FROM sqlite_master WHERE type = 'table' AND name = 'scope_settings')",
            [],
            |row| row.get::<_, bool>(0),
        )? && conn.query_row("SELECT EXISTS(SELECT 1 FROM scope_settings)", [], |row| {
            row.get(0)
        })?)
    }

    /// Your overrides for work in `repo` (its canonical main checkout) and,
    /// when named, the Wave at `wave_slug` there.
    pub fn scope_settings(
        &self,
        repo: Option<&str>,
        wave_slug: Option<&str>,
    ) -> StoreResult<ScopeSettings> {
        let conn = self.conn.lock().expect("store mutex poisoned");
        let mut statement = conn.prepare(
            "SELECT scope_kind,key,value FROM scope_settings
             WHERE scope_kind='global'
                OR (scope_kind='repository' AND scope_id IN
                    (SELECT id FROM repository_plans WHERE repo=?1 AND selected=1))
                OR (scope_kind='wave' AND scope_id IN
                    (SELECT id FROM wave_addresses WHERE repo=?1 AND slug=?2 AND retired_at IS NULL))",
        )?;
        let rows = statement.query_map(params![repo, wave_slug], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, String>(2)?,
            ))
        })?;
        let mut settings = ScopeSettings::default();
        for row in rows {
            let (kind, key, value) = row?;
            let scope = match kind.as_str() {
                "global" => &mut settings.global,
                "repository" => &mut settings.repository,
                _ => &mut settings.wave,
            };
            scope.insert(key, value);
        }
        Ok(settings)
    }

    /// Send a scope's new Tasks to a pool or on-demand, replacing its Tasks
    /// Machine. Its Session stays where it runs.
    pub fn set_tasks_capacity(&self, scope: &DefaultScope, capacity: &str) -> StoreResult<()> {
        let (kind, id) = scope_key(scope);
        let conn = self.conn.lock().expect("store mutex poisoned");
        conn.execute(
            "INSERT INTO scope_defaults(scope_kind,scope_id,tasks_capacity,set_at) VALUES(?1,?2,?3,?4)
             ON CONFLICT(scope_kind,scope_id) DO UPDATE SET tasks_capacity=excluded.tasks_capacity,
                 tasks_machine_id=NULL,set_at=excluded.set_at",
            params![kind, id, capacity, crate::store::rows::now_unix()],
        )?;
        Ok(())
    }

    /// The pool or on-demand this Task's nearest scope sends it to; `None`
    /// when a nearer scope names a Machine or none names either.
    pub fn task_capacity(&self, task: &TaskId) -> StoreResult<Option<String>> {
        let mut conn = self.conn.lock().expect("store mutex poisoned");
        let snapshot = conn.savepoint()?;
        let (wave, repository): (Option<String>, Option<String>) = snapshot.query_row(
            "SELECT p.wave_id,t.repository_id FROM tasks t LEFT JOIN projects p ON p.id=t.project_id WHERE t.id=?1",
            [task.as_str()],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )?;
        for (scope, _) in chain_in(&snapshot, wave, repository)? {
            if machine_at_in(&snapshot, &scope, MachineSetting::Tasks)?.is_some() {
                return Ok(None);
            }
            if let Some(capacity) = capacity_at_in(&snapshot, &scope)? {
                return Ok(Some(capacity));
            }
        }
        Ok(None)
    }

    /// Every scope whose new Tasks take a machine from a pool or on-demand.
    pub fn capacity_defaults(&self) -> StoreResult<Vec<CapacityDefault>> {
        let conn = self.conn.lock().expect("store mutex poisoned");
        let mut statement = conn.prepare(
            "SELECT d.scope_kind,d.scope_id,d.tasks_capacity,coalesce(w.name,r.repo,d.scope_id)
             FROM scope_defaults d
             LEFT JOIN waves w ON d.scope_kind='wave' AND w.id=d.scope_id
             LEFT JOIN repository_plans r ON d.scope_kind='repository' AND r.id=d.scope_id AND r.selected=1
             WHERE d.tasks_capacity IS NOT NULL
             ORDER BY d.scope_kind DESC,4",
        )?;
        let rows = statement.query_map([], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, String>(2)?,
                row.get::<_, String>(3)?,
            ))
        })?;
        let mut defaults = Vec::new();
        for row in rows {
            let (kind, id, capacity, name) = row?;
            let scope = if kind == "wave" {
                DefaultScope::Wave(parse_wave(&id)?)
            } else {
                DefaultScope::Repository(RepositoryId::from_raw(id))
            };
            defaults.push(CapacityDefault {
                scope,
                name,
                capacity,
            });
        }
        Ok(defaults)
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
