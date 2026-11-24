//! Name remote machines without transferring credentials or execution authority.

use anyhow::{anyhow, Context};
use serde::{Deserialize, Serialize};
use std::io::{self, IsTerminal, Write};
use std::process::Stdio;

use crate::lf::MachineCommand;
use loopflow_execution::machine::connection::{
    find_machine, install_command, local_machine, probe, report_version, update_command,
    verify_identity, ConnectionFailure, FailureKind, INSTALL_COMMAND,
};
use loopflow_execution::machine::ssh::remote_path;
use loopflow_execution::store::Store;
use loopflow_model::durable::{DefaultScope, Machine, MachineDefault, MachineSetting};

pub fn run(cmd: &MachineCommand, batch: bool) -> anyhow::Result<()> {
    // Rentals own their runtimes, so they dispatch before this one opens.
    match cmd {
        MachineCommand::End { label, force } => return super::rental::end(label, *force),
        MachineCommand::Pool { cmd } => return super::rental::run_pool(cmd),
        MachineCommand::Ondemand { cmd } => return super::rental::run_ondemand(cmd, batch),
        MachineCommand::Prune { json } => return prune(*json),
        _ => {}
    }
    tokio::runtime::Runtime::new()?.block_on(run_async(cmd, batch))
}

/// `lf machine prune`: store retention, as the Machine server runs it hourly.
fn prune(json: bool) -> anyhow::Result<()> {
    let database = loopflow_execution::store::database_path_from_env()?;
    if !database.exists() {
        anyhow::bail!("no store at {}", database.display());
    }
    let now = time::OffsetDateTime::now_utc().unix_timestamp();
    let report = loopflow_execution::store::retention::prune(&database, now)?;
    if json {
        println!("{}", serde_json::to_string_pretty(&report)?);
    } else {
        println!(
            "ended {} runs and agents nobody saw exit · moved {} observations · converted {} turns · forgot {} history rows{}",
            report.ended_owners,
            report.moved_observations,
            report.converted_turns,
            report.forgotten_history,
            if report.vacuumed { " · vacuumed" } else { "" },
        );
    }
    Ok(())
}

async fn existing_store() -> anyhow::Result<Store> {
    loopflow_execution::store::open_existing_store()
        .await?
        .ok_or_else(|| anyhow!("machine commands need an initialized local store"))
}

/// `lf machine list`: your registered Machines with the defaults naming them.
pub async fn machines() -> anyhow::Result<Vec<MachineListing>> {
    listings(&existing_store().await?, None).await
}

async fn listings(store: &Store, label: Option<&str>) -> anyhow::Result<Vec<MachineListing>> {
    let mut machines = match label {
        Some(label) => vec![find_machine(store, label).await?],
        None => store.machines().await?,
    };
    let defaults = loopflow_execution::store::placement::machine_defaults(&store.sqlite)?;
    // This Machine is listed only when one of your defaults names it.
    let local = store.local_machine().await?;
    if label.is_none() && defaults.iter().any(|d| d.machine_id == local.id) {
        machines.insert(0, local);
    }
    let hub = loopflow_execution::ops::plan_hub::hub(&store.sqlite)?.map(|hub| hub.id);
    let mut listed = Vec::new();
    for machine in machines {
        let mut tasks = Vec::new();
        for (task, since) in store.sqlite.tasks_held_by(&machine.id)? {
            if let Some(saved) = store.sqlite.task(&task)? {
                tasks.push(HeldTask {
                    id: task,
                    identifier: saved.identifier,
                    since,
                });
            }
        }
        listed.push(MachineListing {
            hub: hub.as_ref() == Some(&machine.id),
            defaults: defaults
                .iter()
                .filter(|d| d.machine_id == machine.id)
                .cloned()
                .collect(),
            tasks,
            machine,
        });
    }
    Ok(listed)
}

async fn run_async(cmd: &MachineCommand, batch: bool) -> anyhow::Result<()> {
    match cmd {
        MachineCommand::Connect {
            target,
            provider,
            email,
            chrome_profile,
        } => {
            let machine =
                loopflow_execution::machine::connection::resolve_target(target, false).await?;
            return loopflow_execution::machine::credentials::connect(
                &machine,
                *provider,
                email.as_deref(),
                chrome_profile.as_deref(),
                batch,
            )
            .await;
        }
        MachineCommand::Credentials { cmd } => {
            return super::machine_credentials::receive(cmd).await
        }
        MachineCommand::Self_ { json } => {
            let machine = local_machine().await?;
            if *json {
                println!("{}", serde_json::to_string(&machine)?);
            } else {
                println!("{}", machine.id);
            }
            return Ok(());
        }
        _ => {}
    }
    // Changes to the registry start it on a fresh home.
    let store = loopflow_execution::store::open_store(
        &loopflow_execution::store::storage_config_from_env()?,
    )
    .await?;
    match cmd {
        MachineCommand::Public { name } => {
            let name = (name != "off").then_some(name.as_str());
            store.sqlite.set_local_public_name(name)?;
            match name {
                Some(name) => println!("This Machine is public as {name}"),
                None => println!("This Machine is private"),
            }
        }
        MachineCommand::Add {
            target,
            label,
            pool,
            cost_usd,
            expires_at,
            json,
        } => {
            let label = label.clone().unwrap_or_else(|| default_label(target));
            if label.trim().is_empty() {
                return Err(anyhow!("label must be nonempty"));
            }
            let ownership = loopflow_storage::rental::Ownership {
                pool: pool.clone(),
                cost_usd: cost_usd.unwrap_or_default(),
                expires_at: expires_at
                    .as_deref()
                    .map(|at| {
                        time::OffsetDateTime::parse(
                            at,
                            &time::format_description::well_known::Rfc3339,
                        )
                        .map_err(|error| anyhow!("--expires-at {at:?}: {error}"))
                    })
                    .transpose()?,
            };
            if let Some(pool) = pool {
                if loopflow_execution::store::rentals::pool(&store.sqlite, pool)?.is_none() {
                    return Err(anyhow!(
                        "no pool {pool}; `lf machine pool add {pool}` creates it"
                    ));
                }
            }
            let probe = match probe(target, false).await {
                Err(error)
                    if error
                        .downcast_ref::<ConnectionFailure>()
                        .is_some_and(|failure| failure.kind == FailureKind::MissingLf)
                        && !batch
                        && !*json
                        && io::stdin().is_terminal()
                        && io::stderr().is_terminal() =>
                {
                    if !confirm_install(target)? {
                        return Err(error);
                    }
                    install_remote(target).await?;
                    probe(target, false).await?
                }
                result => result?,
            };
            report_version(target, &probe.version);
            let route = if target == "local" {
                "ssh://local"
            } else {
                target
            };
            let machine = store.add_machine(&probe.id?, route, &label).await?;
            if pool.is_some() || cost_usd.is_some() || expires_at.is_some() {
                loopflow_execution::store::rentals::set_machine_ownership(
                    &store.sqlite,
                    &machine.id,
                    &ownership,
                )?;
            }
            print_machine(&machine, *json)?;
        }
        MachineCommand::Hub { machine, clear } => {
            let selected = match machine {
                Some(machine) => Some(find_machine(&store, machine).await?),
                None if *clear => None,
                None => {
                    match loopflow_execution::ops::plan_hub::hub(&store.sqlite)? {
                        Some(hub) => println!("Hub {}.", hub.name()),
                        None => println!("No hub."),
                    }
                    return Ok(());
                }
            };
            loopflow_execution::ops::plan_hub::set_hub(
                &store.sqlite,
                selected.as_ref().map(|machine| &machine.id),
            )?;
            match selected {
                Some(hub) => println!("This Machine exchanges its plan with {}.", hub.name()),
                None => println!("No hub."),
            }
        }
        MachineCommand::List { label, json } => {
            let listings = listings(&store, label.as_deref()).await?;
            if *json {
                println!("{}", serde_json::to_string(&listings)?);
            } else {
                for listing in &listings {
                    print_machine(&listing.machine, false)?;
                    if listing.hub {
                        println!("  hub: this Machine exchanges its plan with it");
                    }
                    for default in &listing.defaults {
                        let scope = match &default.scope {
                            DefaultScope::Wave(_) => "Wave",
                            DefaultScope::Repository(_) => "repository",
                        };
                        let part = match default.setting {
                            MachineSetting::Tasks => "Tasks",
                            MachineSetting::Session => "Session",
                        };
                        println!("  {scope} {} {part}", default.name);
                    }
                    for task in &listing.tasks {
                        // A move that has not arrived reads as that, as in `lf task status`.
                        let place = loopflow_execution::ops::task_ownership::move_line(
                            &store.sqlite,
                            &task.id,
                        )?
                        .unwrap_or_else(|| {
                            format!(
                                "since {}",
                                loopflow_execution::output::format_time(task.since)
                            )
                        });
                        println!("  {} {place}", task.identifier);
                    }
                }
            }
        }
        MachineCommand::Status { label, json } => {
            let machines = match label {
                Some(label) => vec![find_machine(&store, label).await?],
                None => store.machines().await?,
            };
            let mut statuses = Vec::new();
            for machine in machines {
                let status = connection_status(&store, machine).await?;
                if !*json {
                    println!(
                        "{}  {}  remote lf {} / local lf {}",
                        status.machine.label.as_deref().unwrap_or(""),
                        status.error.as_deref().unwrap_or("reachable"),
                        status.remote_version.as_deref().unwrap_or("unknown"),
                        status.local_version
                    );
                    if let Some(version) = &status.remote_version {
                        report_version(&status.machine.route, version);
                    }
                }
                statuses.push(status);
            }
            if *json {
                println!("{}", serde_json::to_string(&statuses)?);
            }
        }
        MachineCommand::Rename { label, name } => {
            store.rename_machine(label, name).await?;
            println!("Renamed {label} to {name}");
        }
        MachineCommand::Remove { label } => {
            store.remove_machine(label).await?;
            println!("Removed {label}");
        }
        MachineCommand::Connect { .. }
        | MachineCommand::Credentials { .. }
        | MachineCommand::Self_ { .. }
        | MachineCommand::Doctor { .. } => {
            unreachable!(
                "credential, identity and doctor commands dispatch before opening the store"
            )
        }
        MachineCommand::End { .. }
        | MachineCommand::Prune { .. }
        | MachineCommand::Pool { .. }
        | MachineCommand::Ondemand { .. } => {
            unreachable!("machine end, pool and ondemand dispatch before the store opens")
        }
    }
    Ok(())
}

fn print_machine(machine: &Machine, json: bool) -> anyhow::Result<()> {
    if json {
        println!("{}", serde_json::to_string(machine)?);
    } else {
        let label = match (&machine.label, machine.route.as_str()) {
            (Some(label), _) => label.clone(),
            (None, "local") => format!(
                "{} (this Machine)",
                loopflow_execution::store::ownership::local_label()
            ),
            (None, _) => String::new(),
        };
        println!("{label}  {}  {}", machine.route, machine.id);
    }
    Ok(())
}

/// A Machine with your Wave and repository defaults that name it.
#[derive(Debug, Serialize, schemars::JsonSchema)]
pub struct MachineListing {
    #[serde(flatten)]
    machine: Machine,
    defaults: Vec<MachineDefault>,
    /// This Machine exchanges its plan with it (`lf machine hub`).
    hub: bool,
    /// Unfinished Tasks it owns, as this Machine reads their ownership.
    tasks: Vec<HeldTask>,
}

#[derive(Debug, Serialize, schemars::JsonSchema)]
struct HeldTask {
    id: loopflow_model::durable::TaskId,
    identifier: String,
    /// Unix seconds when the Task's claim reached the Machine.
    since: i64,
}

/// Health of a registered connection, using the CLI's identity check and deadline.
pub(super) async fn status_by_id(
    id: &loopflow_model::durable::MachineId,
) -> anyhow::Result<Vec<MachineStatus>> {
    static PROBES: tokio::sync::Semaphore = tokio::sync::Semaphore::const_new(4);
    let _permit = PROBES.acquire().await?;
    let store = existing_store().await?;
    let machine = loopflow_execution::store::machines::machine_by_id(&store.sqlite, id)?
        .filter(|machine| machine.label.is_some())
        .ok_or_else(|| anyhow!("Machine {id} is no longer registered"))?;
    Ok(vec![connection_status(&store, machine).await?])
}

async fn connection_status(store: &Store, machine: Machine) -> anyhow::Result<MachineStatus> {
    let ended = store
        .sqlite
        .rental_for_machine(&machine.id)?
        .filter(|rental| !rental.state.is_live());
    let (state, remote_version, error) = if let Some(rental) = ended {
        let when = rental
            .ended_at
            .map_or_else(String::new, |at| format!(" {at}"));
        (
            ConnectionState::EndedRental,
            None,
            Some(format!("rental ended{when}")),
        )
    } else {
        match probe(&machine.route, false).await {
            Ok(probe) => {
                let version = Some(probe.version);
                match probe.id.and_then(|id| verify_identity(&machine.id, id)) {
                    Ok(()) => (ConnectionState::Reachable, version, None),
                    Err(error) => (ConnectionState::Error, version, Some(error.to_string())),
                }
            }
            Err(error) => (ConnectionState::Error, None, Some(error.to_string())),
        }
    };
    Ok(MachineStatus {
        state,
        remote_version,
        error,
        update_command: update_command(&machine.route),
        local_version: crate::build_info::BUILD_VERSION.to_owned(),
        machine,
    })
}

#[derive(Debug, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(rename_all = "snake_case")]
enum ConnectionState {
    Reachable,
    Error,
    EndedRental,
}

#[derive(Debug, Serialize, Deserialize, schemars::JsonSchema)]
pub(super) struct MachineStatus {
    state: ConnectionState,
    machine: Machine,
    local_version: String,
    #[serde(deserialize_with = "Option::deserialize")]
    remote_version: Option<String>,
    #[serde(deserialize_with = "Option::deserialize")]
    error: Option<String>,
    update_command: String,
}

fn confirm_install(target: &str) -> anyhow::Result<bool> {
    eprint!("No lf on {target}. Install it with the published installer? [Y/n]: ");
    io::stderr().flush()?;
    let mut answer = String::new();
    if io::stdin().read_line(&mut answer)? == 0 {
        return Ok(false);
    }
    Ok(matches!(
        answer.trim().to_ascii_lowercase().as_str(),
        "" | "y" | "yes"
    ))
}

async fn install_remote(target: &str) -> anyhow::Result<()> {
    // Recheck on the target: another add may have installed it while we asked.
    let command = format!(
        "{}; if ! command -v lf >/dev/null 2>&1; then {INSTALL_COMMAND}; fi",
        remote_path()
    );
    let status = tokio::process::Command::new("ssh")
        .args(loopflow_execution::machine_route::bounded_ssh_args(
            target, false,
        )?)
        .arg(command)
        .stdin(Stdio::null())
        .stdout(Stdio::inherit())
        .stderr(Stdio::inherit())
        .kill_on_drop(true)
        .status()
        .await
        .context("could not run remote installer")?;
    if !status.success() {
        return Err(anyhow!(
            "remote installer failed on {target:?}; retry with `{}`",
            install_command(target)
        ));
    }
    Ok(())
}

fn default_label(target: &str) -> String {
    let host = target
        .strip_prefix("ssh://")
        .unwrap_or(target)
        .rsplit('@')
        .next()
        .unwrap_or(target);
    if let Some(host) = host.strip_prefix('[') {
        return host.split(']').next().unwrap_or(host).to_string();
    }
    if host.matches(':').count() == 1 {
        return host.split(':').next().unwrap_or(host).to_string();
    }
    host.to_string()
}

fn validate_expected_machine(local: &loopflow_model::durable::MachineId) -> anyhow::Result<()> {
    let Ok(raw) = std::env::var(loopflow_execution::machine::ssh::EXPECTED_MACHINE_ID_ENV) else {
        return Ok(());
    };
    let expected = loopflow_model::durable::MachineId::parse(&raw)
        .map_err(|error| anyhow!("invalid expected Machine id: {error}"))?;
    if expected != *local {
        return Err(anyhow!(
            "refusing Machine-addressed command: expected Machine {expected}, local Machine is {local}"
        ));
    }
    Ok(())
}

pub fn validate_expected_machine_process() -> anyhow::Result<()> {
    if std::env::var_os(loopflow_execution::machine::ssh::EXPECTED_MACHINE_ID_ENV).is_none() {
        return Ok(());
    }
    let store = loopflow_execution::store::read_existing_registry()
        .context("Machine identity unavailable")?
        .ok_or_else(|| anyhow!("Machine identity unavailable: local registry is absent"))?;
    let local = loopflow_execution::store::machines::local_machine(&store.sqlite)?;
    validate_expected_machine(&local.id)
}

#[cfg(test)]
mod tests {
    use super::default_label;

    #[test]
    fn labels_follow_ssh_hosts_without_restricting_destinations() {
        for (target, label) in [
            ("mini", "mini"),
            ("jack@mini", "mini"),
            ("ssh://jack@mini:2222", "mini"),
            ("ssh://jack@[::1]:2222", "::1"),
            ("::1", "::1"),
        ] {
            assert_eq!(default_label(target), label);
        }
    }
}
