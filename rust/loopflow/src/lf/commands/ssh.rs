//! Run a command on an added machine using credentials resident there.
//! Login transfers are separate foreground commands; this script contains no secrets.

use crate::lf::Cli;
use crate::provider_account::selection::AccountSelection;
use anyhow::{anyhow, Context};
use clap::Parser;
use std::io::Write;
use std::process::{Command, Stdio};

pub const EXPECTED_MACHINE_ID_ENV: &str = "LF_EXPECTED_MACHINE_ID";

pub fn run(target: &str, forward_agent: bool, lf_args: &[String]) -> anyhow::Result<()> {
    let cli = parse_remote_command(lf_args)?;
    let inherited_selection = AccountSelection::from_env()?;
    let runtime = tokio::runtime::Runtime::new()?;
    let target = runtime.block_on(resolve_target(target, forward_agent))?;
    let selection = runtime.block_on(super::machine_credentials::prepare_launch(
        &target,
        &inherited_selection,
        &cli,
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
        .chain(resident_args(lf_args, &cli))
        .collect::<Vec<_>>();
    let preamble = build_preamble(
        &target.route,
        target
            .repo
            .as_deref()
            .expect("added machines have a repository"),
        &cmd,
        &extra_env,
    );
    run_ssh(&target.route, forward_agent, &preamble)
}

fn resident_args(args: &[String], cli: &Cli) -> Vec<String> {
    let mut remaining = cli.account.len() + cli.only_account.len();
    let mut shared = cli.shared;
    let mut result = Vec::new();
    let mut iter = args.iter();
    while let Some(arg) = iter.next() {
        if arg == "--" {
            result.push(arg.clone());
            result.extend(iter.cloned());
            break;
        }
        if remaining > 0 && matches!(arg.as_str(), "--account" | "--only-account") {
            iter.next();
            remaining -= 1;
        } else if remaining > 0
            && (arg.starts_with("--account=") || arg.starts_with("--only-account="))
        {
            remaining -= 1;
        } else if shared && arg == "--shared" {
            result.push("--isolate".into());
            shared = false;
        } else {
            result.push(arg.clone());
        }
    }
    result
}

fn parse_remote_command(lf_args: &[String]) -> anyhow::Result<Cli> {
    if lf_args.first().is_some_and(|arg| arg == "lf") {
        return Err(anyhow!(
            "the remote `lf` is implicit; use `lf --machine <target> <args...>` without `-- lf`"
        ));
    }
    let args = std::iter::once("lf".to_string())
        .chain(lf_args.iter().cloned())
        .collect::<Vec<_>>();
    match Cli::try_parse_from(crate::lf::navigation::normalize_args(args)?) {
        Ok(cli) => Ok(cli),
        Err(error)
            if matches!(
                error.kind(),
                clap::error::ErrorKind::DisplayHelp | clap::error::ErrorKind::DisplayVersion
            ) =>
        {
            // The target prints its own help/version without connecting a login.
            Ok(Cli::try_parse_from(["lf", "help"])?)
        }
        Err(error) => Err(error.into()),
    }
}

pub(super) async fn resolve_target(
    target: &str,
    forward_agent: bool,
) -> anyhow::Result<crate::durable::Machine> {
    let store = crate::store::open_existing_store()
        .await
        .ok_or_else(|| anyhow!("machine commands need an initialized local store"))?;
    let machine = super::machine::find_machine(&store, target).await?;
    let probe = super::machine::probe(&machine.route, forward_agent).await?;
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

/// Classify an ssh exit code. `255` is ssh's reserved transport-error code;
/// `None` means death by signal — both are connection-phase failures, distinct
/// from a real remote command code we must propagate.
fn command_result(dest: &str, code: Option<i32>) -> anyhow::Result<()> {
    match code {
        Some(0) => Ok(()),
        Some(255) | None => Err(super::machine::transport_failure(
            dest,
            "SSH transport closed; see the SSH error above",
        )),
        Some(code) => Err(crate::process::CommandExit(
            u8::try_from(code).expect("SSH command exit status fits a byte"),
        )
        .into()),
    }
}

/// Pipe the preamble into `ssh [-A] <host> bash -s`, streaming stdout/stderr and
/// classifying the remote exit code. Agent forwarding (`-A`) is opt-in. Bounded
/// so an unreachable or misconfigured host fails fast instead of hanging.
fn run_ssh(dest: &str, forward_agent: bool, preamble: &str) -> anyhow::Result<()> {
    let mut child = Command::new("ssh")
        .args(crate::engine::machine_route::bounded_ssh_args(
            dest,
            forward_agent,
        )?)
        .arg("bash -s")
        .stdin(Stdio::piped())
        .stdout(Stdio::inherit())
        .stderr(Stdio::inherit())
        .spawn()
        .context("failed to spawn ssh")?;

    let written = child
        .stdin
        .take()
        .ok_or_else(|| anyhow!("ssh stdin unavailable"))?
        .write_all(preamble.as_bytes());
    if written.is_err() {
        let _ = child.kill();
    }
    let status = child.wait().context("ssh did not complete");
    written.context("failed to write preamble to ssh")?;
    command_result(dest, status?.code())
}

#[cfg(test)]
mod tests {
    use super::{build_preamble, command_result, parse_remote_command, resident_args, sh_quote};

    #[test]
    fn quotes_shell_data_without_forwarding_credentials() {
        assert_eq!(sh_quote("it's here"), "'it'\\''s here'");
        let script = build_preamble(
            "mini",
            "~/project's checkout",
            &["lf".into(), "session".into(), "list".into()],
            &[("LF_EXPECTED_MACHINE_ID", "home_test")],
        );
        assert!(script.contains("machine identity changed"));
        assert!(!script.contains("TOKEN"));
        assert!(!script.contains("LEASE"));
    }

    #[test]
    fn remote_preferences_cannot_relax_the_resident_account_restriction() {
        let args = ["--account", "person@", "--shared", "skill", "implement"].map(str::to_string);
        assert_eq!(
            resident_args(&args, &parse_remote_command(&args).unwrap()),
            ["--isolate", "skill", "implement"]
        );
    }
    #[test]
    fn remote_prompt_preserves_literal_account_and_home_flags() {
        let args = [
            "--only-account=codex=person@",
            "--shared",
            "skill",
            "implement",
            "--",
            "--shared",
            "--account=other@",
        ]
        .map(str::to_string);
        assert_eq!(
            resident_args(&args, &parse_remote_command(&args).unwrap()),
            [
                "--isolate",
                "skill",
                "implement",
                "--",
                "--shared",
                "--account=other@"
            ]
        );
    }
    #[test]
    fn ssh_args_bound_the_connection() {
        let args =
            crate::engine::machine_route::bounded_ssh_args("jack@mini-heart", false).unwrap();
        // Primary hang killer: never block on an interactive prompt.
        assert!(args.iter().any(|a| a == "BatchMode=yes"));
        // Connect handshake and stalled-session bounds.
        assert!(args.iter().any(|a| a == "ConnectTimeout=10"));
        assert!(args.iter().any(|a| a == "ServerAliveInterval=10"));
        assert!(args.iter().any(|a| a == "ServerAliveCountMax=3"));
        assert_eq!(args.last().unwrap(), "jack@mini-heart");
        // Agent forwarding stays opt-in; OpenSSH parses destination ports.
        assert!(!args.iter().any(|a| a == "-A"));
        assert!(!args.iter().any(|a| a == "-p"));
    }

    #[test]
    fn ssh_args_opt_in_agent_forwarding() {
        let args = crate::engine::machine_route::bounded_ssh_args("host", true).unwrap();
        assert_eq!(args.first().unwrap(), "-A");
    }

    #[test]
    fn command_result_preserves_remote_exit_codes_and_reports_transport_failure() {
        assert!(command_result("mini", Some(0)).is_ok());
        for code in [Some(255), None] {
            let error = command_result("mini", code).unwrap_err();
            assert!(error
                .downcast_ref::<crate::process::CommandExit>()
                .is_none());
            assert!(error.to_string().contains("unreachable"));
            assert!(error.to_string().contains("mini"));
        }
        for code in [1, 42, 254] {
            let error = command_result("mini", Some(code)).unwrap_err();
            assert_eq!(
                error
                    .downcast_ref::<crate::process::CommandExit>()
                    .unwrap()
                    .0,
                code as u8
            );
        }
    }
}
