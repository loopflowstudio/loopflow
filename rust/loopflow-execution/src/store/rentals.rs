//! Personal capacity declarations acquired before synchronous rental commands.
use crate::store::{StoreError, StoreResult};
use loopflow_model::durable::MachineId;
use loopflow_storage::rental::{OnDemand, Ownership, Pool, Rental, Usd};
use loopflow_storage::sqlite::SqliteStore;

fn invalid(error: impl std::fmt::Display) -> StoreError {
    StoreError::InvalidData(error.to_string())
}
fn require_name(name: &str) -> StoreResult<()> {
    if name.trim().is_empty() || name.trim() != name {
        return Err(invalid("name must be nonempty without surrounding spaces"));
    }
    Ok(())
}
fn lock(store: &SqliteStore) -> StoreResult<crate::config::files::LockedFile> {
    crate::config::files::LockedFile::open(&store.home_dir()?.join("config.yaml")).map_err(invalid)
}
fn config(store: &SqliteStore) -> StoreResult<crate::config::Config> {
    crate::config::placement::global_settings_at(&store.home_dir()?).map_err(invalid)
}
fn require_free_name(store: &SqliteStore, name: &str) -> StoreResult<()> {
    require_name(name)?;
    let config = config(store)?;
    if config.pools.contains_key(name) || config.ondemands.contains_key(name) {
        return Err(invalid(format!("{name} already names a pool or on-demand")));
    }
    Ok(())
}
fn edit(
    file: &crate::config::files::LockedFile,
    keys: &[&str],
    value: Option<serde_yaml_ng::Value>,
) -> StoreResult<bool> {
    file.update(crate::config::keys::Root::General, |document| {
        if let Some(value) = value {
            crate::config::settings::insert(document, keys, value);
            Ok(true)
        } else {
            Ok(crate::config::files::remove(document, keys))
        }
    })
    .map_err(invalid)
}

pub fn pools(store: &SqliteStore) -> StoreResult<Vec<Pool>> {
    Ok(config(store)?
        .pools
        .into_iter()
        .map(|(name, declaration)| Pool {
            name,
            off: declaration.off,
        })
        .collect())
}
pub fn pool(store: &SqliteStore, name: &str) -> StoreResult<Option<Pool>> {
    Ok(pools(store)?.into_iter().find(|pool| pool.name == name))
}
pub fn add_pool(store: &SqliteStore, name: &str) -> StoreResult<Pool> {
    let file = lock(store)?;
    require_free_name(store, name)?;
    edit(
        &file,
        &["pools", name],
        Some(
            serde_yaml_ng::to_value(crate::config::placement::PoolDeclaration { off: false })
                .map_err(invalid)?,
        ),
    )?;
    pool(store, name)?.ok_or(StoreError::NotFound)
}
pub fn set_pool_off(store: &SqliteStore, name: &str, off: bool) -> StoreResult<()> {
    let file = lock(store)?;
    if pool(store, name)?.is_none() {
        return Err(StoreError::NotFound);
    }
    edit(&file, &["pools", name, "off"], Some(off.into()))?;
    Ok(())
}
pub fn remove_pool(store: &SqliteStore, name: &str) -> StoreResult<()> {
    let file = lock(store)?;
    if !edit(&file, &["pools", name], None)? {
        return Err(StoreError::NotFound);
    }
    store.clear_pool_machines(name)
}
pub fn set_machine_ownership(
    store: &SqliteStore,
    machine: &MachineId,
    ownership: &Ownership,
) -> StoreResult<()> {
    let _file = lock(store)?;
    if let Some(name) = &ownership.pool {
        if pool(store, name)?.is_none() {
            return Err(StoreError::NotFound);
        }
    }
    store.set_machine_ownership_prepared(machine, ownership)
}

pub fn ondemands(store: &SqliteStore) -> StoreResult<Vec<OnDemand>> {
    Ok(config(store)?
        .ondemands
        .into_iter()
        .map(|(name, declaration)| OnDemand {
            name,
            provider: declaration.provider,
            hardware: declaration.hardware,
            max_machines: declaration.max_machines,
            max_usd_per_hour: declaration.max_usd_per_hour,
            hours: declaration.hours,
        })
        .collect())
}
pub fn ondemand(store: &SqliteStore, name: &str) -> StoreResult<Option<OnDemand>> {
    Ok(ondemands(store)?
        .into_iter()
        .find(|ondemand| ondemand.name == name))
}
pub fn add_ondemand(store: &SqliteStore, ondemand: &OnDemand) -> StoreResult<()> {
    let file = lock(store)?;
    require_free_name(store, &ondemand.name)?;
    let declaration = crate::config::placement::OnDemandDeclaration {
        provider: ondemand.provider,
        hardware: ondemand.rental_hardware(),
        max_machines: ondemand.max_machines,
        max_usd_per_hour: ondemand.max_usd_per_hour,
        hours: ondemand.hours,
    };
    edit(
        &file,
        &["ondemands", &ondemand.name],
        Some(serde_yaml_ng::to_value(declaration).map_err(invalid)?),
    )?;
    Ok(())
}
pub fn set_ondemand_max(store: &SqliteStore, name: &str, max: u32) -> StoreResult<()> {
    let file = lock(store)?;
    if ondemand(store, name)?.is_none() {
        return Err(StoreError::NotFound);
    }
    edit(
        &file,
        &["ondemands", name, "max_machines"],
        Some(max.into()),
    )?;
    Ok(())
}
pub fn remove_ondemand(store: &SqliteStore, name: &str) -> StoreResult<()> {
    let file = lock(store)?;
    let live = store.live_rentals(name)?;
    if live > 0 {
        return Err(invalid(format!(
            "{name} still has {live} live machine(s); `lf machine end` gives them back"
        )));
    }
    if !edit(&file, &["ondemands", name], None)? {
        return Err(StoreError::NotFound);
    }
    Ok(())
}
pub fn admit_rental(
    store: &SqliteStore,
    name: &str,
    hours: Option<f64>,
    max_usd: Option<Usd>,
    by_hand: bool,
) -> StoreResult<Rental> {
    let _file = lock(store)?;
    let declaration = ondemand(store, name)?.ok_or(StoreError::NotFound)?;
    store.admit_rental_prepared(&declaration, hours, max_usd, by_hand)
}
