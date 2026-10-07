//! Inspect durable Machine identity and routes.

use crate::lf::MachineCommand;
use anyhow::anyhow;

pub fn run(cmd: &MachineCommand) -> anyhow::Result<()> {
    match cmd {
        MachineCommand::Desktop
        | MachineCommand::Screenshot { .. }
        | MachineCommand::Install { .. }
        | MachineCommand::SyncSkills { .. }
        | MachineCommand::Doctor { .. }
        | MachineCommand::Ssh { .. } => {
            unreachable!("startup and operation commands dispatch separately")
        }
        MachineCommand::User { json } => {
            let name = crate::engine::config::load_user_name()?;
            if *json {
                println!("{}", serde_json::to_string(&name)?);
            } else if let Some(name) = name {
                println!("{name}");
            }
            Ok(())
        }
        MachineCommand::Id { json } => id_cmd(*json),
        MachineCommand::Observe {
            machine_id,
            route,
            json,
        } => observe_cmd(machine_id, route, *json),
    }
}

fn id_cmd(json: bool) -> anyhow::Result<()> {
    let runtime = tokio::runtime::Runtime::new()?;
    let machine = runtime.block_on(async {
        crate::store::open_existing_store()
            .await
            .ok_or_else(|| anyhow!("lf machine id needs an initialized local store"))?
            .local_machine()
            .await
            .map_err(anyhow::Error::from)
    })?;
    if json {
        println!("{}", serde_json::to_string(&machine)?);
    } else {
        println!("{}", machine.id);
    }
    Ok(())
}

fn observe_cmd(
    machine_id: &crate::durable::MachineId,
    route: &str,
    json: bool,
) -> anyhow::Result<()> {
    let parsed = crate::engine::machine_route::MachineRoute::parse(route)
        .ok_or_else(|| anyhow!("invalid Machine route: {route:?}"))?;
    if !parsed.is_remote() {
        return Err(anyhow!(
            "the local Machine route is managed by `lf machine id`; observe only remote SSH routes"
        ));
    }
    let route = parsed.to_string();
    let runtime = tokio::runtime::Runtime::new()?;
    let machine = runtime.block_on(async {
        crate::store::open_existing_store()
            .await
            .ok_or_else(|| anyhow!("lf machine observe needs an initialized local store"))?
            .observe_machine(machine_id, &route)
            .await
            .map_err(anyhow::Error::from)
    })?;
    if json {
        println!("{}", serde_json::to_string(&machine)?);
    } else {
        println!("{}  {}", machine.id, machine.route);
    }
    Ok(())
}

fn validate_expected_machine(local: &crate::durable::MachineId) -> anyhow::Result<()> {
    let Ok(raw) = std::env::var(crate::lf::commands::ssh::EXPECTED_MACHINE_ID_ENV) else {
        return Ok(());
    };
    let expected = crate::durable::MachineId::parse(&raw)
        .map_err(|error| anyhow!("invalid expected Machine id: {error}"))?;
    if expected != *local {
        return Err(anyhow!(
            "refusing Machine-addressed command: expected Machine {expected}, local Machine is {local}"
        ));
    }
    Ok(())
}

pub fn validate_expected_machine_process() -> anyhow::Result<()> {
    if std::env::var_os(crate::lf::commands::ssh::EXPECTED_MACHINE_ID_ENV).is_none() {
        return Ok(());
    }
    let runtime = tokio::runtime::Runtime::new()?;
    let local = runtime.block_on(async {
        crate::store::open_existing_store()
            .await
            .ok_or_else(|| anyhow!("Machine-addressed command needs an initialized local store"))?
            .local_machine()
            .await
            .map_err(anyhow::Error::from)
    })?;
    validate_expected_machine(&local.id)
}
