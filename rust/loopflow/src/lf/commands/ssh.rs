//! Run a command on an added machine using credentials resident there.
//! Login transfers are separate foreground commands; this script contains no secrets.

use crate::durable::TaskExecutionRoute;
use crate::lf::Cli;
use crate::ops::task::remote::TaskSource;
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
            cmd: crate::lf::SessionCommand::List(_)
                | crate::lf::SessionCommand::Resume { .. }
                | crate::lf::SessionCommand::New { .. }
        }) | Some(crate::lf::Commands::Task {
            cmd: crate::lf::TaskCommand::Location { .. }
                | crate::lf::TaskCommand::Claim { .. }
                | crate::lf::TaskCommand::Unclaim { .. }
                | crate::lf::TaskCommand::Shell { .. }
                | crate::lf::TaskCommand::WatchFiles { .. }
                | crate::lf::TaskCommand::Files { .. }
                | crate::lf::TaskCommand::File { .. }
                | crate::lf::TaskCommand::Diff { .. }
                | crate::lf::TaskCommand::Save { .. }
        }) | Some(crate::lf::Commands::Wave {
            cmd: crate::lf::WaveCommand::Show { .. }
        })
    );
    let runtime = tokio::runtime::Runtime::new()?;
    if let Some(id) = super::work_route::resumed_session(cli).filter(|_| !preview) {
        // Resuming a Session that lives here, or moved elsewhere, moves it there first.
        runtime.block_on(async {
            let Some(store) = crate::store::open_existing_store().await else {
                return anyhow::Ok(());
            };
            let store: crate::store::SharedStore = std::sync::Arc::new(store);
            let Some(session) = crate::ops::human_session::session_by_id(&store, id).await? else {
                return Ok(());
            };
            let machine = resolve_target(target, forward_agent).await?;
            super::session_move::relay(&store, &session, &machine).await
        })?;
    }
    let source = if preview || resident_operation {
        None
    } else {
        runtime.block_on(resolve_task_source(cli.task.as_deref()))?
    };
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
    if let Some(route) = work_route
        .as_ref()
        .filter(|route| route.machine_id != target.id)
    {
        // `resume --task X` continues the Task on M, so the Task moves there first.
        let Some(task) = super::work_route::resumed_task(cli).filter(|_| !preview) else {
            return Err(anyhow!(
                "Task is owned on Machine {}, not {}; `lf task move <task> --machine <machine>` moves it",
                route.machine_id,
                target.id
            ));
        };
        super::session_move::move_task(task, target.id.as_str())?;
    }
    // A Machine names no checkout: the destination resolves the repository's
    // own checkout from its identity. A path/name stays the caller's selection.
    let repository = match cli
        .repository
        .clone()
        .or(work_route.as_ref().map(|route| route.repository_id.clone()))
    {
        Some(repository) => Some(repository),
        None if cli.repo.is_some() => None,
        None => current_repository()?,
    };
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
    let source_json = source.as_ref().map(serde_json::to_string).transpose()?;
    let user_name = crate::config::participant_name()?.unwrap_or_default();
    let mut extra_env = vec![
        (EXPECTED_MACHINE_ID_ENV, target.id.as_str()),
        (crate::config::USER_NAME_ENV, user_name.as_str()),
    ];
    if let Some(selection) = selection.as_deref() {
        extra_env.push((
            crate::provider_account::selection::ACCOUNT_SELECTION_ENV,
            selection,
        ));
        extra_env.push(crate::provider_account::activation::isolation_env(true));
    }
    let agent = crate::config::agent_override();
    if let Some(agent) = agent.as_deref() {
        extra_env.push((crate::config::AGENT_OVERRIDE_ENV, agent));
    }
    if let Some(source) = source_json.as_deref() {
        extra_env.push((crate::lf::TASK_SOURCE_ENV, source));
    }
    let mut cmd = vec!["lf".to_string()];
    if cli.repository.is_none() {
        if let Some(repository) = &repository {
            cmd.extend(["--repository".into(), repository.to_string()]);
        }
    }
    cmd.extend(if preview {
        lf_args.to_vec()
    } else {
        resident_args(lf_args, cli, routed, source.as_ref())
    });
    if matches!(
        &cli.command,
        Some(crate::lf::Commands::Task {
            cmd: crate::lf::TaskCommand::Location { .. }
        })
    ) {
        let bytes = runtime.block_on(request(&target, &cmd[1..], None))?;
        print!("{}", String::from_utf8(bytes)?);
        return Ok(());
    }
    let preparation = match &cli.command {
        Some(crate::lf::Commands::Session {
            cmd:
                crate::lf::SessionCommand::Resume { connection, .. }
                | crate::lf::SessionCommand::New { connection, .. },
        }) if connection.prepare && !preview => Some(connection),
        _ => None,
    };
    if let Some(connection) = preparation {
        if !connection.json {
            cmd.push("--json".into());
        }
        let bytes = runtime.block_on(request(&target, &cmd[1..], None))?;
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
                &record.open_argv,
                &[(EXPECTED_MACHINE_ID_ENV, target.id.as_str())],
            );
            record.open_argv = ssh_argv(&target.route, forward_agent, &script, true)?;
        }
        if connection.json {
            println!("{}", serde_json::to_string(&record)?);
        } else {
            println!(
                "{}",
                shlex::try_join(record.open_argv.iter().map(String::as_str))?
            );
        }
        return Ok(());
    }
    let preamble = build_preamble(&cmd, &extra_env);
    run_ssh(&target.route, forward_agent, &preamble)
}

fn resident_args(
    args: &[String],
    cli: &Cli,
    routed: bool,
    source: Option<&TaskSource>,
) -> Vec<String> {
    // Automatic Work routing carries the resolved plan, not a path on this Machine.
    let task_flag = cli.task.as_ref().map(|task| format!("--task={task}"));
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
        } else if let Some(source) = source.filter(|_| {
            (arg == "--task" && iter.as_slice().first() == cli.task.as_ref())
                || task_flag.as_ref() == Some(arg)
        }) {
            // Linear mappings resolve independently retained Task IDs on the target.
            if arg == "--task" {
                iter.next();
            }
            result.push(format!(
                "--task={}",
                source
                    .issue_id
                    .as_deref()
                    .unwrap_or(source.task_id.as_str())
            ));
        } else {
            result.push(arg.clone());
        }
    }
    result
}

async fn resolve_task_source(selector: Option<&str>) -> anyhow::Result<Option<TaskSource>> {
    let Some(selector) = selector else {
        return Ok(None);
    };
    let Some(store) = crate::store::open_existing_store().await else {
        return Ok(None);
    };
    // Validate pushed code before any planning I/O can hide missing source work.
    let source = TaskSource::resolve(&store, selector).await?;
    let repo = match store.get_task_by_issue(selector).await? {
        Some(task) => Some(crate::ops::task::repository_for_task(&store, &task)?),
        None => crate::repo::discover_repo_root(&crate::repo::working_directory()?)
            .ok()
            .flatten(),
    };
    if let Some(repo) = repo {
        if crate::ops::planning_client::connected(&repo.to_string_lossy()) {
            if let Some(task) = store.get_task_by_issue(selector).await? {
                if let Err(error) = crate::ops::planning_export::sync_export(
                    &store,
                    &repo,
                    &crate::durable::WorkRef::Task(task.id),
                )
                .await
                {
                    tracing::warn!(%error,"Linear publication pending; remote resolution uses retained planning");
                }
            }
            let source = TaskSource::resolve(&store, selector).await?;
            if source
                .as_ref()
                .is_some_and(|source| source.issue_id.is_none())
            {
                return Err(anyhow!("Task {selector} is saved locally but has not reached Linear; retry after planning synchronization"));
            }
            return Ok(source);
        }
        if let Err(error) =
            crate::ops::planning_sync::publish_repository(&store, &repo.to_string_lossy()).await
        {
            tracing::warn!(%error,"Git planning publication pending; remote resolution uses retained planning");
        }
    }

    Ok(source)
}

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

fn build_preamble(cmd: &[String], extra_env: &[(&str, &str)]) -> String {
    let mut lines = vec![super::machine::REMOTE_PATH.to_string()];
    for (name, value) in extra_env {
        lines.push(format!("export {name}={}", sh_quote(value)));
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

/// The caller's repository by identity; `None` outside a repository or before
/// `lf repository`, when the destination runs in its home directory.
fn current_repository() -> anyhow::Result<Option<crate::durable::RepositoryId>> {
    let Some(repo) = crate::repository::CanonicalRepo::current()? else {
        return Ok(None);
    };
    let Some(store) = crate::store::read_existing_registry()? else {
        return Ok(None);
    };
    Ok(store.sqlite.repository_id(&repo.to_string())?)
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
    let mut args = crate::machine_route::bounded_ssh_args(dest, forward_agent)?;
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

/// One bounded request through the ordinary SSH envelope, with `input` on its
/// stdin. No probes, credentials, provider launch or local fallback; callers
/// validate the request-bound reply. A request carrying input (a moved
/// conversation) may check out its branch there, so it waits longer.
pub(crate) async fn request(
    machine: &crate::durable::Machine,
    args: &[String],
    input: Option<&[u8]>,
) -> anyhow::Result<Vec<u8>> {
    use tokio::io::AsyncWriteExt;
    let cmd = std::iter::once("lf".to_string())
        .chain(args.iter().cloned())
        .collect::<Vec<_>>();
    let preamble = build_preamble(&cmd, &[(EXPECTED_MACHINE_ID_ENV, machine.id.as_str())]);
    let mut command = tokio::process::Command::new("ssh");
    command
        .args(crate::machine_route::bounded_ssh_args(
            &machine.route,
            false,
        )?)
        .arg(preamble)
        .stdin(if input.is_some() {
            Stdio::piped()
        } else {
            Stdio::null()
        })
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .kill_on_drop(true);
    let bound = std::time::Duration::from_secs(if input.is_some() { 300 } else { 20 });
    let exchange = async {
        let mut child = command.spawn()?;
        if let (Some(input), Some(mut stdin)) = (input, child.stdin.take()) {
            stdin.write_all(input).await?;
        }
        child.wait_with_output().await
    };
    let output = tokio::time::timeout(bound, exchange)
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
    fn repository_addressed_transport_preserves_arguments_in_the_home_directory() {
        use std::os::unix::fs::PermissionsExt;
        let dir = tempfile::tempdir().unwrap();
        let bin = dir.path().join(".local/bin");
        std::fs::create_dir_all(&bin).unwrap();
        let lf = bin.join("lf");
        std::fs::write(&lf, "#!/bin/sh\nprintf '%s\\n' \"$PWD\" \"$@\"\n").unwrap();
        std::fs::set_permissions(&lf, std::fs::Permissions::from_mode(0o755)).unwrap();
        let script = build_preamble(
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
            resident_args(&args, &parse_remote_command(&args).unwrap(), false, None),
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
            resident_args(&args, &parse_remote_command(&args).unwrap(), false, None),
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
            assert_eq!(resident_args(&args, &cli, false, None), args);
            assert_eq!(
                resident_args(&args, &cli, true, None),
                ["--task", "LOO-427", "skill", "debug", "--", "--repo", "literal"]
            );
        }
    }

    #[test]
    fn ssh_args_bound_the_connection() {
        let args = crate::machine_route::bounded_ssh_args("jack@mini-heart", false).unwrap();
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
        let args = crate::machine_route::bounded_ssh_args("host", true).unwrap();
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
