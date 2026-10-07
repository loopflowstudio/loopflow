//! Name remote machines without transferring credentials or execution authority.

use anyhow::{anyhow, Context};
use serde::Serialize;
use std::process::Stdio;
use std::time::Duration;

use crate::durable::{Machine, MachineId};
use crate::lf::MachineCommand;
use crate::store::Store;

pub fn run(cmd: &MachineCommand) -> anyhow::Result<()> {
    let runtime = tokio::runtime::Runtime::new()?;
    runtime.block_on(run_async(cmd))
}

async fn run_async(cmd: &MachineCommand) -> anyhow::Result<()> {
    if let MachineCommand::User { json } = cmd {
        let name = crate::engine::config::load_user_name()?;
        if *json {
            println!("{}", serde_json::to_string(&name)?);
        } else if let Some(name) = name {
            println!("{name}");
        }
        return Ok(());
    }
    let store = crate::store::open_existing_store()
        .await
        .ok_or_else(|| anyhow!("machine commands need an initialized local store"))?;
    match cmd {
        MachineCommand::Id { json } => {
            let machine = store.local_machine().await?;
            if *json {
                println!("{}", serde_json::to_string(&machine)?);
            } else {
                println!("{}", machine.id);
            }
        }
        MachineCommand::Add {
            target,
            label,
            repo,
            json,
        } => {
            let label = label.clone().unwrap_or_else(|| default_label(target));
            let repo = match repo {
                Some(repo) => repo.clone(),
                None => crate::engine::machine_route::resolve_home_relative_repo(
                    &crate::lf::commands::util::find_repo_root().context(
                        "use --repo to name the remote repository outside a local checkout",
                    )?,
                )
                .map_err(anyhow::Error::msg)?,
            };
            if label.trim().is_empty() || repo.trim().is_empty() {
                return Err(anyhow!("label and repository must be nonempty"));
            }
            let probe = probe(target).await?;
            report_version(target, &probe.version);
            let route = if target == "local" {
                "ssh://local"
            } else {
                target
            };
            let machine = store.add_machine(&probe.id?, route, &label, &repo).await?;
            print_machine(&machine, *json)?;
        }
        MachineCommand::List { label, json } => {
            let machines = match label {
                Some(label) => vec![find_machine(&store, label).await?],
                None => store.machines().await?,
            };
            if *json {
                println!("{}", serde_json::to_string(&machines)?);
            } else {
                for machine in &machines {
                    print_machine(machine, false)?;
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
                let result = probe(&machine.route).await;
                let (remote_version, error) = match result {
                    Ok(probe) => {
                        let error = match probe.id {
                            Ok(id) if id == machine.id => None,
                            Ok(id) => Some(format!("identity changed: expected {}, reached {id}; remove and add the connection again", machine.id)),
                            Err(error) => Some(error.to_string()),
                        };
                        (Some(probe.version), error)
                    }
                    Err(error) => (None, Some(error.to_string())),
                };
                let status = MachineStatus {
                    update_command: update_command(&machine.route),
                    machine,
                    local_version: env!("CARGO_PKG_VERSION"),
                    remote_version,
                    error,
                };
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
        _ => unreachable!("startup and operation commands dispatch separately"),
    }
    Ok(())
}

fn print_machine(machine: &Machine, json: bool) -> anyhow::Result<()> {
    if json {
        println!("{}", serde_json::to_string(machine)?);
    } else {
        println!(
            "{}  {}  {}  {}",
            machine.label.as_deref().unwrap_or(""),
            machine.route,
            machine.repo.as_deref().unwrap_or(""),
            machine.id
        );
    }
    Ok(())
}

pub(super) async fn find_machine(store: &Store, selector: &str) -> anyhow::Result<Machine> {
    store
        .machines()
        .await?
        .into_iter()
        .find(|machine| {
            machine.label.as_deref() == Some(selector) || machine.id.as_str() == selector
        })
        .ok_or_else(|| {
            anyhow!("machine {selector:?} is not added; run `lf machine add {selector}` first")
        })
}

#[derive(Debug, Serialize)]
struct MachineStatus {
    machine: Machine,
    local_version: &'static str,
    remote_version: Option<String>,
    error: Option<String>,
    update_command: String,
}

#[derive(Debug)]
pub(super) struct Probe {
    pub id: anyhow::Result<MachineId>,
    pub version: String,
}

pub(super) const REMOTE_PATH: &str =
    "export PATH=\"$HOME/.cargo/bin:$HOME/.local/bin:/opt/homebrew/bin:/usr/local/bin:$PATH\"";

pub(super) async fn probe(target: &str) -> anyhow::Result<Probe> {
    if target.is_empty() || target.contains('\0') {
        return Err(anyhow!("SSH destination cannot be empty or contain NUL"));
    }
    let command = format!("{REMOTE_PATH}; lf --version && lf machine id");
    let mut child = tokio::process::Command::new("ssh");
    child
        .args(crate::engine::machine_route::bounded_ssh_args(target))
        .arg(command)
        .stdin(Stdio::null())
        .kill_on_drop(true);
    let output = tokio::time::timeout(Duration::from_secs(20), child.output())
        .await
        .map_err(|_| anyhow!("machine {target:?} did not answer within 20 seconds"))?
        .context("could not run ssh")?;
    let text = String::from_utf8(output.stdout)?;
    let mut lines = text.lines();
    let version = lines.next().and_then(|line| line.strip_prefix("lf "))
        .ok_or_else(|| anyhow!("machine {target:?} did not return its lf version: {}; check SSH key access and update with `{}`", String::from_utf8_lossy(&output.stderr).trim(), update_command(target)))?.to_string();
    let id = if output.status.success() {
        MachineId::parse(lines.next().unwrap_or_default())
            .map_err(|error| anyhow!("machine {target:?} did not return its identity: {error}"))
    } else {
        Err(anyhow!(
            "machine {target:?} identity probe failed: {}; update with `{}`",
            String::from_utf8_lossy(&output.stderr).trim(),
            update_command(target)
        ))
    };
    Ok(Probe { id, version })
}

pub(super) fn report_version(target: &str, remote: &str) {
    let local = env!("CARGO_PKG_VERSION");
    if remote != local {
        eprintln!(
            "lf versions differ: remote {remote}, local {local}. Update the remote with `{}`",
            update_command(target)
        );
    }
}

fn update_command(target: &str) -> String {
    format!("ssh -- {} 'lf install'", super::ssh::sh_quote(target))
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
