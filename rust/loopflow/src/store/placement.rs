//! Resolve personal placement from saved addresses after releasing SQL.
use crate::durable::OwningMachineSource;
use crate::store::{StoreError, StoreResult};
use loopflow_model::durable::{
    CapacityDefault, DefaultScope, MachineDefault, MachineId, MachineSetting, RepositoryId, TaskId,
};
use loopflow_model::id::WaveId;
use loopflow_storage::sqlite::scope_defaults::{PlacementScope, ScopeAddress};
use loopflow_storage::sqlite::SqliteStore;

fn field(setting: MachineSetting) -> &'static str {
    match setting {
        MachineSetting::Tasks => "tasks",
        MachineSetting::Session => "session",
    }
}

fn preference_error(error: anyhow::Error) -> StoreError {
    StoreError::InvalidData(error.to_string())
}

fn machine_at_in(
    home: &std::path::Path,
    scope: &ScopeAddress,
    setting: MachineSetting,
) -> StoreResult<Option<MachineId>> {
    Ok(crate::config::placement::read_at(home, Some(scope))
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

fn capacity_at_in(home: &std::path::Path, scope: &ScopeAddress) -> StoreResult<Option<String>> {
    Ok(crate::config::placement::read_at(home, Some(scope))
        .map_err(preference_error)?
        .get("machine")
        .and_then(|value| value.get("capacity"))
        .and_then(serde_yaml_ng::Value::as_str)
        .map(str::to_owned))
}

fn resolved_placement(
    home: &std::path::Path,
    saved: &PlacementScope,
) -> StoreResult<crate::config::settings::ResolvedSettings> {
    let chain = &saved.chain;
    let mut layers = Vec::new();
    for scope in std::iter::once(None).chain(chain.iter().rev().map(Some)) {
        let (path, prefix) =
            crate::config::placement::location_at(home, scope).map_err(preference_error)?;
        let document = crate::config::placement::read_at(home, scope).map_err(preference_error)?;
        let mut value = serde_yaml_ng::Value::Mapping(Default::default());
        if let Some(machine) = document.get("machine") {
            crate::config::settings::insert(&mut value, &["machine"], machine.clone());
        }
        layers.push(crate::config::ConfigDeclarations {
            source: crate::config::ConfigSource::Personal,
            scope: match scope.map(|scope| &scope.scope) {
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
pub(crate) fn owning_machine(
    home: &std::path::Path,
    saved: &PlacementScope,
    setting: MachineSetting,
) -> StoreResult<(MachineId, OwningMachineSource)> {
    let resolved = resolved_placement(home, saved)?;
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
        return Ok((saved.local_machine.clone(), source));
    };
    let source = OwningMachineSource::Preference(Box::new(
        value
            .selected()
            .expect("resolved placement has a selection")
            .clone(),
    ));
    let id = value.value.as_str().expect("typed Machine reference");
    let aliases = crate::config::placement::global_settings_at(home)
        .map(|config| config.machines)
        .map_err(preference_error)?;
    let machine = aliases
        .get(id)
        .map(|declaration| declaration.id.clone())
        .unwrap_or_else(|| MachineId::from_raw(id));
    Ok((machine, source))
}

/// Your Machine for a Wave's new Tasks or its Wave Session.
pub fn wave_owning_machine(
    store: &SqliteStore,
    wave: &WaveId,
    setting: MachineSetting,
) -> StoreResult<(MachineId, OwningMachineSource)> {
    let saved = store.placement_scope(Some(wave), None)?;
    owning_machine(&store.home_dir()?, &saved, setting)
}

/// Your Machine for a repository's Session or its Tasks with no Wave default.
pub fn repository_owning_machine(
    store: &SqliteStore,
    repository: &RepositoryId,
    setting: MachineSetting,
) -> StoreResult<(MachineId, OwningMachineSource)> {
    let saved = store.placement_scope(None, Some(repository))?;
    owning_machine(&store.home_dir()?, &saved, setting)
}

/// Set one Machine setting at a scope; a Tasks Machine replaces the
/// scope's capacity. Existing Tasks keep their claims.
pub fn set_machine_default(
    store: &SqliteStore,
    scope: &DefaultScope,
    setting: MachineSetting,
    machine: &MachineId,
) -> StoreResult<()> {
    let saved = store.scope_address(scope)?;
    edit_placement(&store.home_dir()?, &saved, |placement| {
        placement[field(setting)] = machine.to_string().into();
        if setting == MachineSetting::Tasks {
            crate::config::files::remove(placement, &["capacity"]);
        }
    })?;
    Ok(())
}

pub fn clear_machine_default(
    store: &SqliteStore,
    scope: &DefaultScope,
    setting: MachineSetting,
) -> StoreResult<bool> {
    let saved = store.scope_address(scope)?;
    edit_placement(&store.home_dir()?, &saved, |placement| {
        crate::config::files::remove(placement, &[field(setting)]);
        if setting == MachineSetting::Tasks {
            crate::config::files::remove(placement, &["capacity"]);
        }
    })
}

pub fn set_tasks_capacity(
    store: &SqliteStore,
    scope: &DefaultScope,
    capacity: &str,
) -> StoreResult<()> {
    let saved = store.scope_address(scope)?;
    edit_placement(&store.home_dir()?, &saved, |placement| {
        placement["capacity"] = capacity.into();
        crate::config::files::remove(placement, &["tasks"]);
    })?;
    Ok(())
}

/// The pool or on-demand this Task's nearest scope sends it to; `None`
/// when a nearer scope names a Machine or none names either.
pub fn task_capacity(store: &SqliteStore, task: &TaskId) -> StoreResult<Option<String>> {
    let saved = store.task_scope(task)?;
    Ok(resolved_placement(&store.home_dir()?, &saved.placement)?
        .config
        .machine
        .capacity)
}

/// Every scope whose new Tasks take a machine from a pool or on-demand.
pub fn capacity_defaults(store: &SqliteStore) -> StoreResult<Vec<CapacityDefault>> {
    let scopes = store.scope_addresses()?;
    let home = store.home_dir()?;
    let mut defaults = Vec::new();
    for saved in scopes {
        let scope = saved.scope.clone();
        let name = saved.name.clone();
        if let Some(capacity) = capacity_at_in(&home, &saved)? {
            defaults.push(CapacityDefault {
                scope,
                name,
                capacity,
            });
        }
    }
    Ok(defaults)
}

pub fn machine_defaults(store: &SqliteStore) -> StoreResult<Vec<MachineDefault>> {
    let scopes = store.scope_addresses()?;
    let home = store.home_dir()?;
    let mut defaults = Vec::new();
    for saved in scopes {
        let scope = saved.scope.clone();
        let name = saved.name.clone();
        for setting in [MachineSetting::Tasks, MachineSetting::Session] {
            if let Some(machine_id) = machine_at_in(&home, &saved, setting)? {
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

fn edit_placement(
    home: &std::path::Path,
    scope: &ScopeAddress,
    edit: impl FnOnce(&mut serde_yaml_ng::Value),
) -> StoreResult<bool> {
    let (file, prefix) =
        crate::config::placement::location_at(home, Some(scope)).map_err(preference_error)?;
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
        crate::config::placement::expand_shorthand(&mut placement)?;
        edit(&mut placement);
        crate::config::settings::insert(document, &path, placement.clone());
        Ok(previous != placement)
    })
    .map_err(preference_error)
}
