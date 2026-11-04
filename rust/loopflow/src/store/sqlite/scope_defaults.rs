//! Your defaults by Wave or repository, stored on each of your Machines.
//!
//! One scope chain serves every setting: Wave → parent Waves → repository.
//! Machine is the first setting, in two independent parts: where a scope's new
//! Tasks run and where its primary Session runs. A Task's own Machine is its
//! ownership claim. In place of a Tasks Machine, a scope may name capacity: a
//! pool or on-demand that `lf task run` takes a machine from.
//!
//! Scope settings are your overrides of shared setup (`graph`, `agent`,
//! `skills.<skill>.agent`) for a Wave, a repository or every repository.

use rusqlite::{Connection, OptionalExtension};

use crate::durable::{
    CapacityDefault, DefaultScope, MachineDefault, MachineId, MachineSetting, OwningMachineSource,
    RepositoryId, TaskId,
};
use crate::id::WaveId;
use crate::store::{StoreError, StoreResult};

use super::SqliteStore;

fn field(setting: MachineSetting) -> &'static str {
    match setting {
        MachineSetting::Tasks => "tasks",
        MachineSetting::Session => "session",
    }
}

fn preference_error(error: anyhow::Error) -> StoreError {
    StoreError::InvalidData(error.to_string())
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
    Ok(crate::config::placement::read(conn, Some(scope))
        .map_err(preference_error)?
        .get("machine")
        .cloned()
        .map(serde_yaml_ng::from_value::<crate::config::placement::Placement>)
        .transpose()
        .map_err(|error| preference_error(error.into()))?
        .and_then(|placement| match setting {
            MachineSetting::Tasks => placement.tasks,
            MachineSetting::Session => placement.session,
        })
        .map(MachineId::from_raw))
}

fn capacity_at_in(conn: &Connection, scope: &DefaultScope) -> StoreResult<Option<String>> {
    Ok(crate::config::placement::read(conn, Some(scope))
        .map_err(preference_error)?
        .get("machine")
        .and_then(|value| value.get("capacity"))
        .and_then(serde_yaml_ng::Value::as_str)
        .map(str::to_owned))
}

fn resolved_placement(
    conn: &Connection,
    wave: Option<String>,
    repository: Option<String>,
) -> StoreResult<crate::config::settings::ResolvedSettings> {
    let chain = chain_in(conn, wave, repository)?;
    let mut layers = Vec::new();
    for scope in std::iter::once(None).chain(chain.iter().rev().map(|(scope, _)| Some(scope))) {
        let (path, prefix) =
            crate::config::placement::location(conn, scope).map_err(preference_error)?;
        let document = crate::config::placement::read(conn, scope).map_err(preference_error)?;
        let mut value = serde_yaml_ng::Value::Mapping(Default::default());
        if let Some(machine) = document.get("machine") {
            crate::config::settings::insert(&mut value, &["machine"], machine.clone());
        }
        layers.push(crate::config::ConfigDeclarations {
            source: crate::config::ConfigSource::Personal,
            scope: match scope {
                Some(DefaultScope::Wave(_)) => crate::config::settings::SettingScope::Wave,
                Some(DefaultScope::Repository(_)) => {
                    crate::config::settings::SettingScope::Repository
                }
                None => crate::config::settings::SettingScope::Global,
            },
            path: if prefix.is_empty() {
                path
            } else {
                format!("{}#{}", path.display(), prefix.join(".")).into()
            },
            value,
        });
    }
    crate::config::settings::resolve_layers(&layers, None)
        .map_err(|error| StoreError::InvalidData(error.to_string()))
}

/// Placement is resolved once; existing Task claims remain independent of preferences.
pub(super) fn owning_machine_in(
    conn: &Connection,
    wave: Option<String>,
    repository: Option<String>,
    setting: MachineSetting,
) -> StoreResult<(MachineId, OwningMachineSource)> {
    let resolved = resolved_placement(conn, wave.clone(), repository)?;
    let key = format!("machine.{}", field(setting));
    let Some(value) = resolved
        .settings
        .get(&key)
        .filter(|setting| !setting.value.is_null())
    else {
        let source = resolved
            .settings
            .get("machine.capacity")
            .filter(|setting| !setting.value.is_null())
            .and_then(|setting| setting.selected())
            .map(|selected| OwningMachineSource::Preference(Box::new(selected.clone())))
            .unwrap_or(OwningMachineSource::ThisMachine);
        return Ok((super::task_ownership::local_machine_in(conn)?, source));
    };
    let source = OwningMachineSource::Preference(Box::new(
        value
            .selected()
            .expect("resolved placement has a selection")
            .clone(),
    ));
    let id = value.value.as_str().expect("typed Machine reference");
    let aliases = crate::config::placement::machine_declarations(conn).map_err(preference_error)?;
    let machine = aliases
        .get(id)
        .map(|declaration| declaration.id.clone())
        .unwrap_or_else(|| MachineId::from_raw(id));
    Ok((machine, source))
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
        let conn = self.conn.lock().expect("store mutex poisoned");
        edit_placement(&conn, scope, |placement| {
            placement[field(setting)] = machine.to_string().into();
            if setting == MachineSetting::Tasks {
                crate::config::files::remove(placement, &["capacity"]);
            }
        })?;
        Ok(())
    }

    pub fn clear_machine_default(
        &self,
        scope: &DefaultScope,
        setting: MachineSetting,
    ) -> StoreResult<bool> {
        let conn = self.conn.lock().expect("store mutex poisoned");
        edit_placement(&conn, scope, |placement| {
            crate::config::files::remove(placement, &[field(setting)]);
            if setting == MachineSetting::Tasks {
                crate::config::files::remove(placement, &["capacity"]);
            }
        })
    }

    pub fn set_tasks_capacity(&self, scope: &DefaultScope, capacity: &str) -> StoreResult<()> {
        let conn = self.conn.lock().expect("store mutex poisoned");
        edit_placement(&conn, scope, |placement| {
            placement["capacity"] = capacity.into();
            crate::config::files::remove(placement, &["tasks"]);
        })?;
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
        let resolved = resolved_placement(&snapshot, wave, repository)?;
        Ok(resolved.config.machine.capacity)
    }

    /// Every scope whose new Tasks take a machine from a pool or on-demand.
    pub fn capacity_defaults(&self) -> StoreResult<Vec<CapacityDefault>> {
        let conn = self.conn.lock().expect("store mutex poisoned");
        let mut defaults = Vec::new();
        for (scope, name) in crate::config::placement::scopes(&conn).map_err(preference_error)? {
            if let Some(capacity) = capacity_at_in(&conn, &scope)? {
                defaults.push(CapacityDefault {
                    scope,
                    name,
                    capacity,
                });
            }
        }
        Ok(defaults)
    }

    pub fn machine_defaults(&self) -> StoreResult<Vec<MachineDefault>> {
        let conn = self.conn.lock().expect("store mutex poisoned");
        let mut defaults = Vec::new();
        for (scope, name) in crate::config::placement::scopes(&conn).map_err(preference_error)? {
            for setting in [MachineSetting::Tasks, MachineSetting::Session] {
                if let Some(machine_id) = machine_at_in(&conn, &scope, setting)? {
                    defaults.push(MachineDefault {
                        scope: scope.clone(),
                        name: name.clone(),
                        setting,
                        machine_id,
                    });
                }
            }
        }
        Ok(defaults)
    }
}

fn edit_placement(
    conn: &Connection,
    scope: &DefaultScope,
    edit: impl FnOnce(&mut serde_yaml_ng::Value),
) -> StoreResult<bool> {
    let (file, prefix) =
        crate::config::placement::location(conn, Some(scope)).map_err(preference_error)?;
    crate::config::files::update(&file, |document| {
        let path: Vec<_> = prefix
            .iter()
            .map(String::as_str)
            .chain(["machine"])
            .collect();
        let mut placement = path
            .iter()
            .try_fold(&*document, |value, key| value.get(*key))
            .cloned()
            .unwrap_or(serde_yaml_ng::Value::Mapping(Default::default()));
        let previous = placement.clone();
        if placement.is_string() {
            placement = serde_yaml_ng::to_value(serde_yaml_ng::from_value::<
                crate::config::placement::Placement,
            >(placement)?)?;
        }
        edit(&mut placement);
        crate::config::settings::insert(document, &path, placement.clone());
        Ok(previous != placement)
    })
    .map_err(preference_error)
}
