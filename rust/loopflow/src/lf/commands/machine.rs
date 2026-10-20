//! Name remote machines without transferring credentials or execution authority.

use anyhow::{anyhow, Context};
use serde::Serialize;
use std::io::{self, IsTerminal, Write};
use std::process::Stdio;
use std::time::Duration;

use crate::durable::{DefaultScope, Machine, MachineDefault, MachineId};
use crate::lf::MachineCommand;
use crate::store::Store;

pub fn run(cmd: &MachineCommand, batch: bool) -> anyhow::Result<()> {
    let runtime = tokio::runtime::Runtime::new()?;
    runtime.block_on(run_async(cmd, batch))
}

async fn run_async(cmd: &MachineCommand, batch: bool) -> anyhow::Result<()> {
    match cmd {
        MachineCommand::Connect {
            target,
            provider,
            email,
            chrome_profile,
        } => {
            let machine = super::ssh::resolve_target(target, false).await?;
            return super::machine_credentials::connect(
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
        _ => {}
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
            json,
        } => {
            let label = label.clone().unwrap_or_else(|| default_label(target));
            if label.trim().is_empty() {
                return Err(anyhow!("label must be nonempty"));
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
            print_machine(&machine, *json)?;
        }
        MachineCommand::List { label, json } => {
            let mut machines = match label {
                Some(label) => vec![find_machine(&store, label).await?],
                None => store.machines().await?,
            };
            let defaults = store.sqlite.machine_defaults()?;
            // This Machine is listed only when one of your defaults names it.
            let local = store.local_machine().await?;
            if label.is_none() && defaults.iter().any(|d| d.machine_id == local.id) {
                machines.insert(0, local);
            }
            let listings = machines
                .into_iter()
                .map(|machine| MachineListing {
                    defaults: defaults
                        .iter()
                        .filter(|d| d.machine_id == machine.id)
                        .cloned()
                        .collect(),
                    machine,
                })
                .collect::<Vec<_>>();
            if *json {
                println!("{}", serde_json::to_string(&listings)?);
            } else {
                for listing in &listings {
                    print_machine(&listing.machine, false)?;
                    for default in &listing.defaults {
                        match &default.scope {
                            DefaultScope::Wave(_) => println!("  Wave {}", default.name),
                            DefaultScope::Repository(_) => {
                                println!("  repository {}", default.name)
                            }
                        }
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
                let result = probe(&machine.route, false).await;
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
                    local_version: crate::build_info::BUILD_VERSION,
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
        MachineCommand::Connect { .. } | MachineCommand::Credentials { .. } => {
            unreachable!("credential commands dispatch before opening the store")
        }
        MachineCommand::Use { .. } => unreachable!("machine use dispatches with its repository"),
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
                crate::store::sqlite::task_ownership::local_label()
            ),
            (None, _) => String::new(),
        };
        println!("{label}  {}  {}", machine.route, machine.id);
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

/// A Machine with your Wave and repository defaults that name it.
#[derive(Debug, Serialize)]
struct MachineListing {
    #[serde(flatten)]
    machine: Machine,
    defaults: Vec<MachineDefault>,
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

// Public installer, identical to the first-install command in README.md.
const INSTALL_COMMAND: &str = "curl -fsSL https://github.com/loopflowstudio/loopflow/releases/latest/download/install.sh | sh";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum FailureKind {
    NeedsSignIn,
    HostKeyUnknown,
    HostKeyChanged,
    Unreachable,
    MissingLf,
}

#[derive(Debug, thiserror::Error)]
#[error("{message}")]
struct ConnectionFailure {
    kind: FailureKind,
    message: String,
}

fn connection_failure(target: &str, kind: FailureKind, detail: &str) -> anyhow::Error {
    let ssh = format!(
        "ssh -o ControlPath=none -- {}",
        super::ssh::sh_quote(target)
    );
    let guidance = match kind {
        FailureKind::NeedsSignIn => format!("needs-sign-in: run `{ssh}` to set up SSH key access, then retry"),
        FailureKind::HostKeyUnknown => format!("host-key unknown: run `{ssh}` and verify the host fingerprint before accepting it"),
        FailureKind::HostKeyChanged => format!("host-key changed: run `{ssh}` to inspect the conflict; verify the new fingerprint with the machine owner before repairing the known_hosts entry SSH identifies"),
        FailureKind::Unreachable => format!("unreachable: run `{ssh}` to check the address, network and SSH service"),
        FailureKind::MissingLf => format!("no lf on the remote: run `{}`", install_command(target)),
    };
    ConnectionFailure {
        kind,
        message: format!(
            "machine {target:?}: {guidance}{}",
            if detail.is_empty() {
                String::new()
            } else {
                format!("; {detail}")
            }
        ),
    }
    .into()
}

pub(super) fn transport_failure(target: &str, detail: &str) -> anyhow::Error {
    let lower = detail.to_ascii_lowercase();
    let kind = if lower.contains("remote host identification has changed")
        || lower.contains("offending")
        || lower.contains("host key has changed")
    {
        FailureKind::HostKeyChanged
    } else if lower.contains("host key verification failed")
        || lower.contains("no matching host key is known")
        || lower.contains("no ") && lower.contains("host key is known")
    {
        FailureKind::HostKeyUnknown
    } else if lower.contains("permission denied")
        || lower.contains("authentication failed")
        || lower.contains("too many authentication failures")
        || lower.contains("sign_and_send_pubkey")
    {
        FailureKind::NeedsSignIn
    } else {
        FailureKind::Unreachable
    };
    connection_failure(target, kind, detail)
}

pub(super) async fn probe(target: &str, forward_agent: bool) -> anyhow::Result<Probe> {
    if target.is_empty() || target.contains('\0') {
        return Err(anyhow!("SSH destination cannot be empty or contain NUL"));
    }
    let command = format!(
        "{REMOTE_PATH}; command -v lf >/dev/null 2>&1 || {{ echo lf-not-installed; exit 127; }}; lf --version && lf machine id"
    );
    let mut child = tokio::process::Command::new("ssh");
    child
        .env("LC_ALL", "C")
        .args(crate::machine_route::bounded_ssh_args(
            target,
            forward_agent,
        )?)
        .arg(command)
        .stdin(Stdio::null())
        .kill_on_drop(true);
    let output = tokio::time::timeout(Duration::from_secs(20), child.output())
        .await
        .map_err(|_| {
            connection_failure(
                target,
                FailureKind::Unreachable,
                "did not answer within 20 seconds",
            )
        })?
        .context("could not run ssh")?;
    let detail = String::from_utf8_lossy(&output.stderr);
    if output.status.code() == Some(255) || output.status.code().is_none() {
        return Err(transport_failure(target, detail.trim()));
    }
    let text = String::from_utf8(output.stdout)?;
    let mut lines = text.lines();
    let version = lines.next().and_then(|line| line.strip_prefix("lf "));
    if text.trim() == "lf-not-installed" && output.status.code() == Some(127) {
        return Err(connection_failure(
            target,
            FailureKind::MissingLf,
            detail.trim(),
        ));
    }
    let version = version
        .ok_or_else(|| {
            anyhow!(
                "machine {target:?} did not return its lf version: {}; update with `{}`",
                detail.trim(),
                update_command(target)
            )
        })?
        .to_string();
    let id = if output.status.success() {
        MachineId::parse(lines.next().unwrap_or_default())
            .map_err(|error| anyhow!("machine {target:?} did not return its identity: {error}"))
    } else {
        Err(anyhow!(
            "machine {target:?} identity probe failed: {}; update with `{}`",
            detail.trim(),
            update_command(target)
        ))
    };
    Ok(Probe { id, version })
}

fn install_command(target: &str) -> String {
    format!(
        "ssh -- {} {}",
        super::ssh::sh_quote(target),
        super::ssh::sh_quote(INSTALL_COMMAND)
    )
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
    let command =
        format!("{REMOTE_PATH}; if ! command -v lf >/dev/null 2>&1; then {INSTALL_COMMAND}; fi");
    let status = tokio::process::Command::new("ssh")
        .args(crate::machine_route::bounded_ssh_args(target, false)?)
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

pub(super) fn report_version(target: &str, remote: &str) {
    let local = crate::build_info::BUILD_VERSION;
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
    let local = crate::store::read_existing_registry()
        .context("Machine identity unavailable")?
        .ok_or_else(|| anyhow!("Machine identity unavailable: local registry is absent"))?
        .sqlite
        .local_machine()?;
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
