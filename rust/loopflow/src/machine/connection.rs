// Public installer, identical to the first-install command in README.md.
pub(crate) const INSTALL_COMMAND: &str = "curl -fsSL https://github.com/loopflowstudio/loopflow/releases/latest/download/install.sh | sh";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum FailureKind {
    NeedsSignIn,
    HostKeyUnknown,
    HostKeyChanged,
    Unreachable,
    MissingLf,
}

#[derive(Debug, thiserror::Error)]
#[error("{message}")]
pub(crate) struct ConnectionFailure {
    pub(crate) kind: FailureKind,
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

pub(crate) fn transport_failure(target: &str, detail: &str) -> anyhow::Error {
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

pub(crate) fn install_command(target: &str) -> String {
    format!(
        "ssh -- {} {}",
        crate::machine::ssh::sh_quote(target),
        crate::machine::ssh::sh_quote(crate::machine::connection::INSTALL_COMMAND)
    )
}
