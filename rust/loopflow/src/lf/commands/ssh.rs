//! Run a command on an added machine using credentials resident there.
//! Login transfers are separate foreground commands; this script contains no secrets.

use crate::provider_account::selection::AccountSelection;
use anyhow::{anyhow, Context};
use clap::Parser;
use std::io::Write;
use std::process::{Command, Stdio};

pub const EXPECTED_MACHINE_ID_ENV: &str = "LF_EXPECTED_MACHINE_ID";

pub fn run(
    target: &str,
    repo: Option<&str>,
    forward_agent: bool,
    selection: &AccountSelection,
    lf_args: &[String],
) -> anyhow::Result<()> {
    reject_nested_ssh(lf_args)?;
    let runtime = tokio::runtime::Runtime::new()?;
    let target = runtime.block_on(resolve_target(target))?;
    let selection = runtime.block_on(super::machine_credentials::prepare_launch(
        &target, selection, lf_args,
    ))?;
    let user_name = crate::engine::config::participant_name()?.unwrap_or_default();
    let selection = selection.env_value()?;
    let mut extra_env = vec![
        (EXPECTED_MACHINE_ID_ENV, target.id.as_str()),
        (crate::engine::config::USER_NAME_ENV, user_name.as_str()),
        (
            crate::provider_account::selection::ACCOUNT_SELECTION_ENV,
            selection.as_str(),
        ),
        crate::provider_account::activation::isolation_env(true),
    ];
    let declaration = std::env::var(crate::lf::WORK_DECLARATION_ENV).ok();
    if let Some(value) = declaration.as_deref() {
        extra_env.push((crate::lf::WORK_DECLARATION_ENV, value));
    }
    let cmd = std::iter::once("lf".to_string())
        .chain(resident_args(lf_args)?)
        .collect::<Vec<_>>();
    let preamble = build_preamble(
        &target.route,
        repo.unwrap_or(
            target
                .repo
                .as_deref()
                .expect("added machines have a repository"),
        ),
        &cmd,
        &extra_env,
    );
    match run_ssh(&target.route, forward_agent, &preamble)? {
        SshOutcome::Success => Ok(()),
        SshOutcome::CommandFailure(code) => Err(crate::exec::CommandExit(
            u8::try_from(code).expect("SSH exit status fits a byte"),
        )
        .into()),
        SshOutcome::ConnectionFailure => unreachable!("transport errors are returned by run_ssh"),
    }
}

fn resident_args(args: &[String]) -> anyhow::Result<Vec<String>> {
    let argv = std::iter::once("lf".into())
        .chain(args.iter().cloned())
        .collect();
    let cli = crate::lf::Cli::try_parse_from(crate::lf::navigation::normalize_args(argv)?)?;
    let mut remaining = cli.account.len() + cli.only_account.len();
    let mut result = Vec::new();
    let mut iter = args.iter();
    while let Some(arg) = iter.next() {
        if remaining > 0 && matches!(arg.as_str(), "--account" | "--only-account") {
            iter.next();
            remaining -= 1;
        } else if remaining > 0
            && (arg.starts_with("--account=") || arg.starts_with("--only-account="))
        {
            remaining -= 1;
        } else if cli.shared && arg == "--shared" {
            result.push("--isolate".into());
        } else {
            result.push(arg.clone());
        }
    }
    Ok(result)
}

fn reject_nested_ssh(lf_args: &[String]) -> anyhow::Result<()> {
    if lf_args.first().is_some_and(|arg| arg == "lf") {
        return Err(anyhow!(
            "the remote `lf` is implicit; use `lf machine ssh <target> <args...>` without `-- lf`"
        ));
    }
    let args = std::iter::once("lf".to_string())
        .chain(lf_args.iter().cloned())
        .collect::<Vec<_>>();
    if matches!(
        crate::lf::Cli::try_parse_from(crate::lf::navigation::normalize_args(args)?),
        Ok(crate::lf::Cli {
            command: Some(crate::lf::Commands::Machine {
                cmd: crate::lf::MachineCommand::Ssh { .. }
            }),
            ..
        })
    ) {
        return Err(anyhow!(
            "nested `lf machine ssh` is not supported; connect directly from the origin machine"
        ));
    }
    Ok(())
}

pub(super) async fn resolve_target(target: &str) -> anyhow::Result<crate::durable::Machine> {
    let store = crate::store::open_existing_store()
        .await
        .ok_or_else(|| anyhow!("machine commands need an initialized local store"))?;
    let machine = super::machine::find_machine(&store, target).await?;
    let probe = super::machine::probe(&machine.route).await?;
    super::machine::report_version(&machine.route, &probe.version);
    let reached = probe.id?;
    if reached != machine.id {
        return Err(anyhow!(
            "remote machine identity changed: expected {}, reached {reached}; remove and add the connection again",
            machine.id
        ));
    }
    Ok(machine)
}

fn build_preamble(host: &str, repo: &str, cmd: &[String], extra_env: &[(&str, &str)]) -> String {
    let mut lines = vec![super::machine::REMOTE_PATH.to_string()];
    for (name, value) in extra_env {
        lines.push(format!("export {name}={}", sh_quote(value)));
    }
    lines.push("LF_REACHED_MACHINE_ID=$(lf machine id) || exit 1".to_string());
    lines.push(r#"[ "$LF_REACHED_MACHINE_ID" = "$LF_EXPECTED_MACHINE_ID" ] || { echo 'remote machine identity changed' >&2; exit 1; }"#.to_string());
    let path = if repo.starts_with('/') {
        sh_quote(repo)
    } else {
        format!(
            "\"$HOME\"/{}",
            sh_quote(repo.strip_prefix("~/").unwrap_or(repo))
        )
    };
    lines.push(format!(
        "cd -- {path} || {{ echo {} >&2; exit 1; }}",
        sh_quote(&format!("no repo {repo} on {host}"))
    ));
    lines.push(format!(
        "exec {}",
        cmd.iter()
            .map(|arg| sh_quote(arg))
            .collect::<Vec<_>>()
            .join(" ")
    ));
    lines.join("\n") + "\n"
}

/// POSIX single-quote escaping: wrap in `'…'`, and render any embedded single
/// quote as `'\''`. Safe for arbitrary bytes including secrets.
pub(super) fn sh_quote(value: &str) -> String {
    let mut quoted = String::with_capacity(value.len() + 2);
    quoted.push('\'');
    for ch in value.chars() {
        if ch == '\'' {
            quoted.push_str("'\\''");
        } else {
            quoted.push(ch);
        }
    }
    quoted.push('\'');
    quoted
}

/// What the remote process's exit status means for the caller.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum SshOutcome {
    /// Remote command succeeded.
    Success,
    /// SSH transport/connection failure (its reserved code `255`, or death by
    /// signal): unreachable host, unknown key, auth refusal, or a bounded
    /// timeout firing. Actionable and host-named; never a real remote code.
    ConnectionFailure,
    /// The remote command itself exited nonzero — propagate its code verbatim.
    CommandFailure(i32),
}

fn ssh_args(dest: &str, forward_agent: bool) -> Vec<String> {
    let mut args = Vec::new();
    if forward_agent {
        args.push("-A".to_string());
    }
    args.extend(crate::engine::machine_route::bounded_ssh_args(dest));
    args.push("bash -s".to_string());
    args
}

/// Classify an ssh exit code. `255` is ssh's reserved transport-error code;
/// `None` means death by signal — both are connection-phase failures, distinct
/// from a real remote command code we must propagate.
fn classify_exit(code: Option<i32>) -> SshOutcome {
    match code {
        Some(0) => SshOutcome::Success,
        Some(255) | None => SshOutcome::ConnectionFailure,
        Some(other) => SshOutcome::CommandFailure(other),
    }
}

/// A sanitized, host-named error for a transport-phase failure. Carries no
/// credential value; ssh's own reason is already on the inherited stderr.
fn connection_error(host: &str) -> anyhow::Error {
    anyhow!(
        "lf machine ssh could not reach '{host}': ssh failed during connection/transport \
         (bounded by BatchMode + ConnectTimeout={}s). See the ssh error above; check \
         the host is reachable, its key is known, and key auth works.",
        crate::engine::machine_route::SSH_CONNECT_TIMEOUT_SECS
    )
}

/// Pipe the preamble into `ssh [-A] <host> bash -s`, streaming stdout/stderr and
/// classifying the remote exit code. Agent forwarding (`-A`) is opt-in. Bounded
/// so an unreachable or misconfigured host fails fast instead of hanging.
fn run_ssh(dest: &str, forward_agent: bool, preamble: &str) -> anyhow::Result<SshOutcome> {
    let mut child = Command::new("ssh")
        .args(ssh_args(dest, forward_agent))
        .stdin(Stdio::piped())
        .stdout(Stdio::inherit())
        .stderr(Stdio::inherit())
        .spawn()
        .context("failed to spawn ssh")?;

    child
        .stdin
        .take()
        .ok_or_else(|| anyhow!("ssh stdin unavailable"))?
        .write_all(preamble.as_bytes())
        .context("failed to write preamble to ssh")?;

    let status = child.wait().context("ssh did not complete")?;
    match classify_exit(status.code()) {
        outcome @ (SshOutcome::Success | SshOutcome::CommandFailure(_)) => Ok(outcome),
        SshOutcome::ConnectionFailure => Err(connection_error(dest)),
    }
}

#[cfg(test)]
mod tests {
    use super::{
        build_preamble, classify_exit, reject_nested_ssh, resident_args, sh_quote, ssh_args,
        SshOutcome,
    };

    #[test]
    fn quotes_shell_data_and_preserves_remote_exit() {
        assert_eq!(sh_quote("it's here"), "'it'\\''s here'");
        assert_eq!(classify_exit(Some(42)), SshOutcome::CommandFailure(42));
        assert_eq!(classify_exit(Some(255)), SshOutcome::ConnectionFailure);
        let script = build_preamble(
            "mini",
            "~/project's checkout",
            &["lf".into(), "session".into(), "list".into()],
            &[("LF_EXPECTED_MACHINE_ID", "home_test")],
        );
        assert!(script.contains("machine identity changed"));
        assert!(!script.contains("TOKEN"));
        assert!(!script.contains("LEASE"));
        assert!(ssh_args("mini", false).contains(&"StrictHostKeyChecking=yes".into()));
    }

    #[test]
    fn nested_ssh_is_rejected_before_transport() {
        assert!(reject_nested_ssh(&["ssh".into(), "other".into(), "list".into()]).is_err());
    }
    #[test]
    fn remote_preferences_cannot_relax_the_resident_account_restriction() {
        let args = ["--account", "person@", "--shared", "skill", "implement"].map(str::to_string);
        assert_eq!(
            resident_args(&args).unwrap(),
            ["--isolate", "skill", "implement"]
        );
    }
}
