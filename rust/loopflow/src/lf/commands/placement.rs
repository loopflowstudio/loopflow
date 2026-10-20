use crate::durable::{MachineId, PlacementProvenance, TaskExecutionSource, WorkMachine, WorkRef};
use crate::id::WaveId;
use crate::lf::WaveCommand;
use crate::store::{open_store, storage_config_from_env, Store};
use anyhow::{anyhow, Context};
use std::path::Path;
use std::sync::Arc;

pub fn wave(repo: &Path, command: &WaveCommand) -> anyhow::Result<()> {
    tokio::runtime::Runtime::new()?.block_on(async {
        let store = open_shared_store().await?;
        let name = match command {
            WaveCommand::Place { name, .. } => name,
            WaveCommand::Rename { wave, .. } => wave,
            _ => unreachable!("Wave placement dispatcher"),
        };
        let wave_id = match WaveId::parse(name) {
            Ok(id) => id,
            Err(_) => crate::work::wave::context::resolve_managed_wave(
                Some(&store),
                Some(repo),
                Some(name),
                None,
            )
            .await?
            .id()
            .clone(),
        };
        let work = WorkRef::Wave(wave_id.clone());
        match command {
            WaveCommand::Place {
                machine_id, json, ..
            } => {
                if let Some(machine_id) = machine_id {
                    require_wave_repository(&store, &wave_id, repo).await?;
                    store.place_work(&work, machine_id).await?;
                }
                print_machine(&store.work_machine(&work).await?, *json)
            }
            WaveCommand::Rename {
                repo: target,
                name,
                json,
                title,
                ..
            } => {
                if let Some(title) = title {
                    let selected = store
                        .get_wave(&wave_id)
                        .await?
                        .ok_or_else(|| anyhow!("Wave {wave_id} not found"))?;
                    crate::ops::linear::rename(
                        repo,
                        &crate::ops::linear::LinearRenameOptions {
                            wave: Some(selected.slug().to_string()),
                            title: title.clone(),
                        },
                        &crate::ops::NullProgress,
                    )
                    .await?;
                }
                print(
                    &crate::work::wave::relocate::relocate_wave(
                        &store,
                        &wave_id,
                        repo,
                        target.as_deref(),
                        name.as_deref(),
                    )
                    .await?,
                    *json,
                    &format!("Wave {wave_id}: renamed"),
                )
            }
            _ => unreachable!("Wave placement dispatcher"),
        }
    })
}

/// Print where Work executes: JSON, or one line naming the Machine and its source.
pub fn print_machine(machine: &WorkMachine, json: bool) -> anyhow::Result<()> {
    let name = |id: &MachineId| {
        machine
            .machines
            .iter()
            .find(|candidate| &candidate.id == id)
            .and_then(|candidate| candidate.label.clone())
            .unwrap_or_else(|| {
                if *id == machine.local_machine_id {
                    "this Machine".into()
                } else {
                    id.to_string()
                }
            })
    };
    let target = machine
        .machine_id
        .as_ref()
        .map_or_else(|| "an unrecorded Machine".into(), name);
    let placement = &machine.placement;
    let why = match machine.execution {
        Some(TaskExecutionSource::RecordedCheckout) => "started there".to_string(),
        Some(TaskExecutionSource::Admission) => "admitted to start there".to_string(),
        _ if placement.provenance == PlacementProvenance::LocalDefault => "default".to_string(),
        _ if placement.source == machine.work => "set here".to_string(),
        _ => format!("from {} {}", placement.source.kind(), placement.source.id()),
    };
    print(
        machine,
        json,
        &format!(
            "{} {}: {target} ({why})",
            machine.work.kind(),
            machine.work.id()
        ),
    )
}

fn print(value: &impl serde::Serialize, json: bool, summary: &str) -> anyhow::Result<()> {
    if json {
        println!("{}", serde_json::to_string_pretty(value)?);
    } else {
        println!("{summary}");
    }
    Ok(())
}

async fn open_shared_store() -> anyhow::Result<Arc<Store>> {
    let config = storage_config_from_env().context("resolve the shared Loopflow store")?;
    open_store(&config)
        .await
        .map(Arc::new)
        .context("open the shared Loopflow store")
}

async fn require_wave_repository(
    store: &Store,
    wave_id: &WaveId,
    repo: &Path,
) -> anyhow::Result<()> {
    let wave = store
        .get_wave(wave_id)
        .await?
        .ok_or_else(|| anyhow!("Wave {wave_id} is not registered"))?;
    let locator = crate::work::wave::WaveLocator::discover(repo, wave.slug())?;
    let local = store.get_wave_at(&locator).await?;
    if local.as_ref().map(crate::work::wave::Wave::id) != Some(wave_id) {
        return Err(anyhow!(
            "Wave {wave_id} belongs to repository {}, not invoking repository {}",
            wave.repo(),
            locator.repo()
        ));
    }
    Ok(())
}
