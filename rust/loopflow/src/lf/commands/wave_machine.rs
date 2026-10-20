//! Your Machine for a Wave's new Tasks and its Wave Session. The choice is
//! personal: this Machine stores it and copies it to your other Machines.
use crate::durable::{MachineId, OwningMachineSource};
use crate::id::WaveId;
use crate::lf::WaveCommand;
use crate::session::PrimaryScope;
use crate::store::{open_store, storage_config_from_env};
use anyhow::{anyhow, Context};
use std::path::Path;
use std::sync::Arc;

pub fn run(repo: &Path, command: &WaveCommand) -> anyhow::Result<()> {
    let WaveCommand::Machine {
        wave,
        machine,
        unset,
        here,
    } = command
    else {
        unreachable!("Wave Machine dispatcher");
    };
    tokio::runtime::Runtime::new()?.block_on(async {
        let config = storage_config_from_env().context("resolve the shared Loopflow store")?;
        let store = Arc::new(
            open_store(&config)
                .await
                .context("open the shared Loopflow store")?,
        );
        let selected = crate::work::wave::context::resolve_managed_wave(
            Some(&store),
            Some(repo),
            Some(wave),
            None,
        )
        .await?;
        let wave_id = selected.id().clone();
        let name = selected.name().to_string();
        if machine.is_none() && !unset {
            println!("{}", describe(&store.sqlite, &wave_id)?);
            return Ok(());
        }
        let target = machine
            .as_deref()
            .map(|machine| crate::ops::task_ownership::resolve_machine(&store.sqlite, machine))
            .transpose()?;
        store.sqlite.set_wave_machine(&wave_id, target.as_ref())?;
        if let Some(target) = &target {
            move_wave_session(&store, repo, &wave_id, &name, target).await?;
        }
        println!("{}", describe(&store.sqlite, &wave_id)?);
        if !here {
            copy_to_my_machines(&store.sqlite, repo, &name, target.as_ref()).await?;
        }
        Ok(())
    })
}

fn describe(store: &crate::store::sqlite::SqliteStore, wave: &WaveId) -> anyhow::Result<String> {
    let (machine, source) = store.wave_owning_machine(wave)?;
    Ok(match source {
        OwningMachineSource::ThisMachine => {
            "Your Tasks for this Wave start on this Machine".to_string()
        }
        OwningMachineSource::Wave(from) => {
            let label = label(store, &machine)?;
            format!("Your Tasks for Wave {from} start on {label}")
        }
    })
}

fn label(store: &crate::store::sqlite::SqliteStore, machine: &MachineId) -> anyhow::Result<String> {
    if *machine == store.local_machine()?.id {
        return Ok(format!(
            "{} (this Machine)",
            crate::store::sqlite::task_ownership::local_label()
        ));
    }
    Ok(store
        .machine_by_id(machine)?
        .and_then(|machine| machine.label)
        .unwrap_or_else(|| machine.to_string()))
}

/// The Wave Session lives on your Wave Machine: admit it there, and complete
/// it everywhere else. Native history stays where it ran.
async fn move_wave_session(
    store: &crate::store::SharedStore,
    repo: &Path,
    wave: &WaveId,
    name: &str,
    target: &MachineId,
) -> anyhow::Result<()> {
    if *target == store.sqlite.local_machine()?.id {
        crate::ops::human_session::primary::ensure(store, repo, Some(name), false).await?;
    } else if let Some(session) = store
        .sqlite
        .complete_primary_session(&PrimaryScope::Wave(wave.clone()))?
    {
        eprintln!(
            "Wave Session {} completed here; it continues on {target}",
            session.id
        );
    }
    Ok(())
}

/// Each of your registered Machines holds the same choice, so a launch here
/// routes there and the target knows to receive it. An unreachable Machine
/// keeps its old choice until this runs again.
async fn copy_to_my_machines(
    store: &crate::store::sqlite::SqliteStore,
    repo: &Path,
    wave: &str,
    target: Option<&MachineId>,
) -> anyhow::Result<()> {
    let repository = store
        .repository_id(&crate::repository::CanonicalRepo::discover(repo)?.to_string())?
        .ok_or_else(|| anyhow!("repository identity unavailable; run `lf repo identity`"))?;
    let mut args = vec![
        "--repository".to_string(),
        repository.to_string(),
        "wave".into(),
        "machine".into(),
        wave.into(),
    ];
    match target {
        Some(target) => args.push(target.to_string()),
        None => args.push("--unset".into()),
    }
    args.push("--here".into());
    for machine in store.machines()? {
        let name = machine
            .label
            .clone()
            .unwrap_or_else(|| machine.id.to_string());
        match crate::lf::commands::ssh::request(&machine, &args).await {
            Ok(_) => eprintln!("copied to {name}"),
            Err(error) => eprintln!(
                "warning: {name} keeps its previous choice ({error}); rerun `lf wave machine` to copy it"
            ),
        }
    }
    Ok(())
}
