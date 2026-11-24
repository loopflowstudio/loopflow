use crate::machine::ssh::remote_path;
use crate::store::Store;
use anyhow::{anyhow, Context};
use loopflow_model::durable::{Machine, MachineId};
use std::process::Stdio;
use std::time::Duration;
// Public installer, identical to the first-install command in README.md.
pub const INSTALL_COMMAND: &str = "curl -fsSL https://github.com/loopflowstudio/loopflow/releases/latest/download/install.sh | sh";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FailureKind {
    NeedsSignIn,
    HostKeyUnknown,
    HostKeyChanged,
    Unreachable,
    MissingLf,
}

#[derive(Debug, thiserror::Error)]
#[error("{message}")]
pub struct ConnectionFailure {
    pub kind: FailureKind,
    message: String,
}

pub(crate) fn connection_failure(target: &str, kind: FailureKind, detail: &str) -> anyhow::Error {
    let ssh = format!(
        "ssh -o ControlPath=none -- {}",
        crate::machine::ssh::sh_quote(target)
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

pub fn transport_failure(target: &str, detail: &str) -> anyhow::Error {
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

pub fn install_command(target: &str) -> String {
    format!(
        "ssh -- {} {}",
        crate::machine::ssh::sh_quote(target),
        crate::machine::ssh::sh_quote(crate::machine::connection::INSTALL_COMMAND)
    )
}
pub async fn find_machine(store: &Store, selector: &str) -> anyhow::Result<Machine> {
    store
        .machines()
        .await?
        .into_iter()
        .find(|machine| {
            machine.label.as_deref() == Some(selector) || machine.id.as_str() == selector
        })
        .ok_or_else(|| not_added(selector))
}

/// `selector` in this Machine's registry; a home with no registry has added none.
pub async fn find_registered_machine(selector: &str) -> anyhow::Result<Machine> {
    match crate::store::read_existing_registry()? {
        Some(store) => find_machine(&store, selector).await,
        None => Err(not_added(selector)),
    }
}

fn not_added(selector: &str) -> anyhow::Error {
    anyhow!("machine {selector:?} is not added; run `lf machine add {selector}` first")
}

#[derive(Debug)]
pub struct Probe {
    pub id: anyhow::Result<MachineId>,
    pub version: String,
}

pub async fn probe(target: &str, forward_agent: bool) -> anyhow::Result<Probe> {
    if target.is_empty() || target.contains('\0') {
        return Err(anyhow!("SSH destination cannot be empty or contain NUL"));
    }
    let command = format!(
        "{}; command -v lf >/dev/null 2>&1 || {{ echo lf-not-installed; exit 127; }}; lf --version && lf machine id",
        remote_path()
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
    // `lf --version` prints the build version, then a label such as
    // `(build 3f3153d95 on v0.15.3)`; only the version compares.
    let version = lines
        .next()
        .and_then(|line| line.strip_prefix("lf "))
        .and_then(|line| line.split_whitespace().next());
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

pub fn report_version(target: &str, remote: &str) {
    let local = crate::build_identity::version();
    if remote == local {
        return;
    }
    let update = if release_number(remote) > release_number(local) {
        "Update this one with `lf install`".to_string()
    } else {
        format!("Update the remote with `{}`", update_command(target))
    };
    eprintln!("lf versions differ: remote {remote}, local {local}. {update}");
}

/// Numeric release components; build metadata and suffixes do not order.
fn release_number(version: &str) -> Vec<u64> {
    version
        .split(['-', '+'])
        .next()
        .unwrap_or_default()
        .split('.')
        .map(|part| part.parse().unwrap_or(0))
        .collect()
}

pub fn update_command(target: &str) -> String {
    format!(
        "ssh -- {} 'lf install'",
        crate::machine::ssh::sh_quote(target)
    )
}

pub async fn resolve_target(
    target: &str,
    forward_agent: bool,
) -> anyhow::Result<loopflow_model::durable::Machine> {
    let store = crate::store::open_existing_store()
        .await?
        .ok_or_else(|| anyhow!("machine commands need an initialized local store"))?;
    let machine = find_machine(&store, target).await?;
    if machine.id == crate::store::machines::local_machine(&store.sqlite)?.id {
        return Ok(machine);
    }
    let probe = probe(&machine.route, forward_agent).await?;
    report_version(&machine.route, &probe.version);
    verify_identity(&machine.id, probe.id?)?;
    Ok(machine)
}

pub fn verify_identity(expected: &MachineId, reached: MachineId) -> anyhow::Result<()> {
    anyhow::ensure!(
        reached == *expected,
        "remote machine identity changed: expected {expected}, reached {reached}; remove and add the connection again"
    );
    Ok(())
}

/// This Machine's durable identity; a fresh home gets its store here.
pub async fn local_machine() -> anyhow::Result<Machine> {
    let store = crate::store::open_store(&crate::store::storage_config_from_env()?).await?;
    Ok(store.local_machine().await?)
}

#[cfg(test)]
mod tests {
    use super::release_number;
    #[test]
    fn release_numbers_order_by_component() {
        assert!(release_number("0.14.14") > release_number("0.14.12"));
        assert!(release_number("0.14.10") > release_number("0.14.9"));
        assert_eq!(release_number("0.14.14-dev+abc"), release_number("0.14.14"));
    }
}
