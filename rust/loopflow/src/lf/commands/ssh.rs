//! Run a command on an added machine using credentials resident there.
//! Login transfers are separate foreground commands; this script contains no secrets.

use crate::durable::TaskExecutionRoute;
use crate::lf::Cli;
use crate::provider_account::selection::AccountSelection;
use anyhow::{anyhow, Context};
use clap::Parser;
use std::io::IsTerminal;
use std::process::{Command, Stdio};

pub const EXPECTED_MACHINE_ID_ENV: &str = "LF_EXPECTED_MACHINE_ID";

pub fn run(
    target: &str,
    forward_agent: bool,
    cli: &Cli,
    lf_args: &[String],
    task_route: Option<TaskExecutionRoute>,
) -> anyhow::Result<()> {
    let preview = cli.context
        || cli.explain
        || matches!(
            cli.command,
            Some(crate::lf::Commands::Task {
                cmd: crate::lf::TaskCommand::Location { .. }
            })
        );
    // These operations use the addressed Machine's own records and accounts.
    // Inspection and file editing must never prepare or transfer a provider login.
    let resident_operation = matches!(
        &cli.command,
        Some(crate::lf::Commands::Session {
            cmd: crate::lf::SessionCommand::List { .. }
                | crate::lf::SessionCommand::Open { .. }
                | crate::lf::SessionCommand::Ensure { .. }
        }) | Some(crate::lf::Commands::Task {
            cmd: crate::lf::TaskCommand::Location { .. }
                | crate::lf::TaskCommand::Files { .. }
                | crate::lf::TaskCommand::File { .. }
                | crate::lf::TaskCommand::Diff { .. }
                | crate::lf::TaskCommand::Save { .. }
        }) | Some(crate::lf::Commands::Wave {
            cmd: crate::lf::WaveCommand::Show { .. }
        })
    );
    if !preview && matches!(cli.command, Some(crate::lf::Commands::Desktop { .. })) {
        super::desktop::require_supported()?;
    }
    let runtime = tokio::runtime::Runtime::new()?;
    let target = if preview || resident_operation {
        let store = crate::store::read_existing_registry()?
            .ok_or_else(|| anyhow!("Machine unavailable: local registry is absent"))?;
        runtime.block_on(super::machine::find_machine(&store, target))?
    } else {
        runtime.block_on(resolve_target(target, forward_agent))?
    };
    // Automatic routing already chose both Machine and plan. Keep that reading
    // together rather than combining its Machine with a second plan reading.
    let routed = task_route.is_some();
    let work_route = match task_route {
        Some(route) => Some(route),
        // A path/name addresses the destination's filesystem, not the caller's.
        None if cli.repo.is_some() => None,
        None => super::work_route::resolve(cli)?,
    };
    if let Some(route) = &work_route {
        if route.machine_id != target.id {
            return Err(anyhow!(
                "Task executes on Machine {}, not {}; delegation does not move existing work",
                route.machine_id,
                target.id
            ));
        }
    }
    let repository = cli
        .repository
        .as_ref()
        .or(work_route.as_ref().map(|route| &route.repository_id));
    // Preview forwards only authored selectors. Account lookup/connection and
    // isolation preparation belong to launch, on neither side of this read.
    let selection = if preview || resident_operation {
        None
    } else {
        Some(
            runtime
                .block_on(super::machine_credentials::prepare_launch(
                    &target,
                    &AccountSelection::from_env()?,
                    cli,
                ))?
                .env_value()?,
        )
    };
    let user_name = crate::engine::config::participant_name()?.unwrap_or_default();
    let mut extra_env = vec![
        (EXPECTED_MACHINE_ID_ENV, target.id.as_str()),
        (crate::engine::config::USER_NAME_ENV, user_name.as_str()),
    ];
    if let Some(selection) = selection.as_deref() {
        extra_env.push((
            crate::provider_account::selection::ACCOUNT_SELECTION_ENV,
            selection,
        ));
        extra_env.push(crate::provider_account::activation::isolation_env(true));
    }
    let declaration = std::env::var(crate::lf::WORK_DECLARATION_ENV).ok();
    if let Some(value) = declaration.as_deref() {
        extra_env.push((crate::lf::WORK_DECLARATION_ENV, value));
    }
    let mut cmd = vec!["lf".to_string()];
    if cli.repository.is_none() {
        if let Some(repository) = repository {
            cmd.extend(["--repository".into(), repository.to_string()]);
        }
    }
    cmd.extend(if preview {
        lf_args.to_vec()
    } else {
        resident_args(lf_args, cli, routed)
    });
    if matches!(
        &cli.command,
        Some(crate::lf::Commands::Task {
            cmd: crate::lf::TaskCommand::Location { .. }
        })
    ) {
        let bytes = runtime.block_on(read(&target, &cmd[1..]))?;
        print!("{}", String::from_utf8(bytes)?);
        return Ok(());
    }
    if matches!(
        &cli.command,
        Some(crate::lf::Commands::Session {
            cmd: crate::lf::SessionCommand::Open { json: true, .. }
        })
    ) && !preview
    {
        let bytes = runtime.block_on(read(&target, &cmd[1..]))?;
        let mut record: crate::ops::human_session::SessionRecord = serde_json::from_slice(&bytes)?;
        anyhow::ensure!(
            record
                .workspace
                .as_ref()
                .is_some_and(|workspace| workspace.machine_id == target.id),
            "Session reply does not belong to the addressed Machine"
        );
        if !record.open_argv.is_empty() {
            let script = build_preamble(
                &target.route,
                None,
                &record.open_argv,
                &[(EXPECTED_MACHINE_ID_ENV, target.id.as_str())],
            );
            record.open_argv = ssh_argv(&target.route, forward_agent, &script, true)?;
        }
        println!("{}", serde_json::to_string(&record)?);
        return Ok(());
    }
    let preamble = build_preamble(
        &target.route,
        if repository.is_some() || cli.repo.is_some() {
            None
        } else {
            target.repo.as_deref()
        },
        &cmd,
        &extra_env,
    );
    run_ssh(&target.route, forward_agent, &preamble)
}

fn resident_args(args: &[String], cli: &Cli, routed: bool) -> Vec<String> {
    // Automatic Work routing carries the resolved plan, not a path on this Machine.
    let mut remove_repo = routed && cli.repo.is_some();
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
        if remove_repo && arg == "--repo" {
            iter.next();
            remove_repo = false;
        } else if remove_repo && arg.starts_with("--repo=") {
            remove_repo = false;
        } else if remaining > 0 && matches!(arg.as_str(), "--account" | "--only-account") {
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

/// Parse transport policy once; help/version are forwarded for the peer to print.
pub fn parse_remote_command(lf_args: &[String]) -> anyhow::Result<Cli> {
    if lf_args.first().is_some_and(|arg| arg == "lf") {
        return Err(anyhow!(
            "the remote `lf` is implicit; use `lf --machine <target> <args...>` without `-- lf`"
        ));
    }
    let args = std::iter::once("lf".to_string())
        .chain(lf_args.iter().cloned())
        .collect::<Vec<_>>();
    match Cli::try_parse_from(args) {
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

fn build_preamble(
    host: &str,
    repo: Option<&str>,
    cmd: &[String],
    extra_env: &[(&str, &str)],
) -> String {
    let mut lines = vec![super::machine::REMOTE_PATH.to_string()];
    for (name, value) in extra_env {
        lines.push(format!("export {name}={}", sh_quote(value)));
    }
    if let Some(repo) = repo {
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
    }
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

/// Carry the script as an argument: stdin belongs to file drafts or terminal input.
fn ssh_argv(
    dest: &str,
    forward_agent: bool,
    preamble: &str,
    terminal: bool,
) -> anyhow::Result<Vec<String>> {
    let mut args = crate::engine::machine_route::bounded_ssh_args(dest, forward_agent)?;
    if terminal {
        for arg in &mut args {
            if arg == "-T" {
                *arg = "-tt".into();
            }
        }
    }
    let mut argv = vec!["ssh".into()];
    argv.extend(args);
    argv.push(preamble.into());
    Ok(argv)
}

fn run_ssh(dest: &str, forward_agent: bool, preamble: &str) -> anyhow::Result<()> {
    let argv = ssh_argv(
        dest,
        forward_agent,
        preamble,
        std::io::stdin().is_terminal(),
    )?;
    let status = Command::new(&argv[0])
        .args(&argv[1..])
        .stdin(Stdio::inherit())
        .stdout(Stdio::inherit())
        .stderr(Stdio::inherit())
        .status()
        .context("ssh did not complete")?;
    command_result(dest, status.code())
}

/// One bounded read through the ordinary SSH envelope. No probes, credentials,
/// provider launch or local fallback; callers validate the request-bound reply.
pub(crate) async fn read(
    machine: &crate::durable::Machine,
    args: &[String],
) -> anyhow::Result<Vec<u8>> {
    let cmd = std::iter::once("lf".to_string())
        .chain(args.iter().cloned())
        .collect::<Vec<_>>();
    let preamble = build_preamble(
        &machine.route,
        None,
        &cmd,
        &[(EXPECTED_MACHINE_ID_ENV, machine.id.as_str())],
    );
    let mut child = tokio::process::Command::new("ssh");
    child
        .args(crate::engine::machine_route::bounded_ssh_args(
            &machine.route,
            false,
        )?)
        .arg(preamble)
        .stdin(Stdio::null())
        .kill_on_drop(true);
    let output = tokio::time::timeout(std::time::Duration::from_secs(20), child.output())
        .await
        .context("Machine command unavailable: peer timed out")??;
    anyhow::ensure!(
        output.status.success(),
        "Machine command unavailable: {}",
        String::from_utf8_lossy(&output.stderr).trim()
    );
    Ok(output.stdout)
}

#[cfg(test)]
mod tests {
    use super::{build_preamble, command_result, parse_remote_command, resident_args};

    #[test]
    fn repository_addressed_transport_preserves_arguments_without_entering_machine_default() {
        use std::os::unix::fs::PermissionsExt;
        let dir = tempfile::tempdir().unwrap();
        let bin = dir.path().join(".local/bin");
        std::fs::create_dir_all(&bin).unwrap();
        let lf = bin.join("lf");
        std::fs::write(&lf, "#!/bin/sh\nprintf '%s\\n' \"$PWD\" \"$@\"\n").unwrap();
        std::fs::set_permissions(&lf, std::fs::Permissions::from_mode(0o755)).unwrap();
        let script = build_preamble(
            "fixture",
            None,
            &[
                "lf".into(),
                "--repository".into(),
                "repo_exact".into(),
                "--task".into(),
                "task_exact".into(),
                ":".into(),
                "literal 'draft'; $(false)".into(),
            ],
            &[("LF_EXPECTED_MACHINE_ID", "home_fixture")],
        );
        let output = std::process::Command::new("/bin/bash")
            .args(["-c", &script])
            .env_clear()
            .env("HOME", dir.path())
            .env("PATH", "/usr/bin:/bin")
            .current_dir(dir.path())
            .output()
            .unwrap();
        assert!(output.status.success());
        let text = String::from_utf8(output.stdout).unwrap();
        let lines = text.lines().collect::<Vec<_>>();
        assert_eq!(
            lines[0],
            dir.path().canonicalize().unwrap().to_str().unwrap()
        );
        assert_eq!(
            &lines[1..],
            &[
                "--repository",
                "repo_exact",
                "--task",
                "task_exact",
                ":",
                "literal 'draft'; $(false)"
            ]
        );
    }

    #[test]
    fn remote_preferences_cannot_relax_the_resident_account_restriction() {
        let args = ["--account", "person@", "--shared", "skill", "implement"].map(str::to_string);
        assert_eq!(
            resident_args(&args, &parse_remote_command(&args).unwrap(), false),
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
            resident_args(&args, &parse_remote_command(&args).unwrap(), false),
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
    fn repository_paths_stay_on_explicit_machine_but_not_automatic_work_routes() {
        for selection in [vec!["--repo", "~/src/remote"], vec!["--repo=~/src/remote"]] {
            let mut args = selection;
            args.extend([
                "--task", "LOO-427", "skill", "debug", "--", "--repo", "literal",
            ]);
            let args = args.into_iter().map(str::to_owned).collect::<Vec<_>>();
            let cli = parse_remote_command(&args).unwrap();
            assert_eq!(resident_args(&args, &cli, false), args);
            assert_eq!(
                resident_args(&args, &cli, true),
                ["--task", "LOO-427", "skill", "debug", "--", "--repo", "literal"]
            );
        }
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
