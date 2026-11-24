//! Machine declarations and presentation compose above saved Machine rows.
use crate::store::{StoreError, StoreResult};
use loopflow_model::durable::{Machine, MachineId, MachineRef};
use loopflow_storage::sqlite::{durable::SavedMachine, SqliteStore};
use time::OffsetDateTime;
fn invalid(error: impl std::fmt::Display) -> StoreError {
    StoreError::InvalidData(error.to_string())
}

fn config(store: &SqliteStore) -> StoreResult<crate::config::Config> {
    crate::config::placement::global_settings_at(&store.home_dir()?).map_err(invalid)
}

pub fn present(
    saved: Option<SavedMachine>,
    id: &MachineId,
    config: &crate::config::Config,
) -> StoreResult<Option<Machine>> {
    let label = config
        .machines
        .iter()
        .find(|(_, declaration)| declaration.id == *id)
        .map(|(name, _)| name.clone());
    if saved.is_none() && label.is_none() {
        return Ok(None);
    }
    let (local, created, observed) = saved
        .map(|row| (row.local, row.created_at, row.observed_at))
        .unwrap_or((false, 0, 0));
    Ok(Some(Machine {
        id: id.clone(),
        label,
        route: if local {
            "local".into()
        } else {
            config.connections.get(id).cloned().unwrap_or_default()
        },
        created_at: OffsetDateTime::from_unix_timestamp(created).map_err(invalid)?,
        observed_at: OffsetDateTime::from_unix_timestamp(observed).map_err(invalid)?,
    }))
}

pub fn machine_by_id(store: &SqliteStore, id: &MachineId) -> StoreResult<Option<Machine>> {
    let saved = store.saved_machine(id)?;
    present(saved, id, &config(store)?)
}

/// A saved Machine can outlive its declaration in preferences.
pub fn registered_machine(
    store: &SqliteStore,
    machine: &MachineRef,
) -> StoreResult<Option<Machine>> {
    let Some(id) = &machine.id else {
        return Ok(None);
    };
    Ok(machine_by_id(store, id)?.filter(|machine| machine.label.is_some()))
}

pub fn local_machine(store: &SqliteStore) -> StoreResult<Machine> {
    let saved = store.saved_local_machine()?;
    let id = saved.id.clone();
    present(Some(saved), &id, &config(store)?)?.ok_or(StoreError::NotFound)
}

pub fn machines(store: &SqliteStore) -> StoreResult<Vec<Machine>> {
    let saved: std::collections::HashMap<_, _> = store
        .saved_machines()?
        .into_iter()
        .map(|row| (row.id.clone(), row))
        .collect();
    let config = config(store)?;
    config
        .machines
        .values()
        .map(|declaration| {
            present(
                saved.get(&declaration.id).cloned(),
                &declaration.id,
                &config,
            )?
            .ok_or(StoreError::NotFound)
        })
        .collect()
}

pub fn add_machine(
    store: &SqliteStore,
    machine_id: &MachineId,
    target: &str,
    label: &str,
) -> StoreResult<Machine> {
    if target.is_empty() || target == "local" || label.trim().is_empty() {
        return Err(invalid(
            "machine target and label must be nonempty; local is reserved",
        ));
    }
    if store.saved_local_machine()?.id == *machine_id {
        return Err(invalid("cannot add the local machine as remote"));
    }
    let declaration = crate::config::placement::MachineDeclaration {
        id: machine_id.clone(),
    };
    crate::config::files::update(&store.home_dir()?.join("config.yaml"), |document| {
        if let Some(existing) = document
            .get("machines")
            .and_then(|machines| machines.get(label))
        {
            let existing: crate::config::placement::MachineDeclaration =
                serde_yaml_ng::from_value(existing.clone())?;
            anyhow::ensure!(existing.id == *machine_id, "machine name already exists");
        }
        crate::config::settings::insert(
            document,
            &["machines", label],
            serde_yaml_ng::to_value(declaration)?,
        );
        crate::config::settings::insert(
            document,
            &["connections", machine_id.as_str()],
            target.into(),
        );
        Ok(())
    })
    .map_err(invalid)?;
    store.register_machine(machine_id)?;
    machine_by_id(store, machine_id)?.ok_or(StoreError::NotFound)
}

pub fn rename_machine(store: &SqliteStore, label: &str, name: &str) -> StoreResult<()> {
    crate::config::files::update(&store.home_dir()?.join("config.yaml"), |document| {
        anyhow::ensure!(!name.trim().is_empty(), "machine label cannot be empty");
        let machines = document
            .get_mut("machines")
            .and_then(serde_yaml_ng::Value::as_mapping_mut)
            .ok_or(StoreError::NotFound)?;
        anyhow::ensure!(!machines.contains_key(name), "machine name already exists");
        let declaration = machines.remove(label).ok_or(StoreError::NotFound)?;
        machines.insert(name.into(), declaration);
        Ok(())
    })
    .map_err(invalid)
}

pub fn remove_machine(store: &SqliteStore, label: &str) -> StoreResult<()> {
    if !crate::config::placement::write_global(&store.home_dir()?, &["machines", label], None)
        .map_err(invalid)?
    {
        return Err(StoreError::NotFound);
    }
    Ok(())
}

pub fn pool_machines(
    store: &SqliteStore,
    name: &str,
) -> StoreResult<Vec<loopflow_storage::rental::PoolMachine>> {
    let saved = store.saved_pool_machines(name)?;
    let config = config(store)?;
    saved
        .into_iter()
        .map(|(machine, ownership, tasks)| {
            let id = machine.id.clone();
            Ok(loopflow_storage::rental::PoolMachine {
                machine: present(Some(machine), &id, &config)?.ok_or(StoreError::NotFound)?,
                ownership,
                tasks,
            })
        })
        .collect()
}

pub fn copied_sessions(
    store: &SqliteStore,
    task: Option<&loopflow_model::durable::TaskId>,
    repository: Option<&loopflow_model::durable::RepositoryId>,
) -> StoreResult<Vec<crate::machine_records::CopiedSession>> {
    let mut saved = store.saved_copied_sessions(task, repository)?;
    let config = config(store)?;
    for session in &mut saved {
        session.origin_label = config
            .machines
            .iter()
            .find(|(_, declaration)| declaration.id == session.origin)
            .map(|(name, _)| name.clone());
    }
    Ok(saved)
}

pub fn copied_history(
    store: &SqliteStore,
    session: &str,
) -> StoreResult<Option<crate::machine_records::CopiedHistory>> {
    let Some(mut saved) = store.saved_copied_history(session)? else {
        return Ok(None);
    };
    let config = config(store)?;
    saved.session.origin_label = config
        .machines
        .iter()
        .find(|(_, declaration)| declaration.id == saved.session.origin)
        .map(|(name, _)| name.clone());
    Ok(Some(saved))
}

#[cfg(test)]
mod tests {
    use super::{add_machine, machines};

    use loopflow_model::durable::MachineId;

    #[test]
    fn aliases_of_one_machine_keep_the_same_saved_observation() {
        let home = tempfile::tempdir().unwrap();
        let store = crate::store::opening::open_ephemeral(&home.path().join("db")).unwrap();
        let id = MachineId::new();
        let first = add_machine(&store, &id, "mini", "builder").unwrap();
        add_machine(&store, &id, "mini", "alias").unwrap();
        let listed = machines(&store).unwrap();
        assert_eq!(listed.len(), 2);
        assert!(listed
            .iter()
            .all(|machine| machine.id == id && machine.created_at == first.created_at));
    }
}
