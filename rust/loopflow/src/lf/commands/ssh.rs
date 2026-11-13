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

/// Whether `lf --machine <target> <args>` runs on this Machine: the target is
/// this Machine, or the command is `task move X --machine <target>` and this
/// Machine began moving X there without it arriving. Run here, a leaving move
/// carries on and a sent move delivers what was sent; forwarded, the target
/// would see a move from a Machine the forwarded personal settings do not name.
pub fn runs_here(target: &str, lf_args: &[String]) -> anyhow::Result<bool> {
    let Some(store) = crate::store::read_existing_registry()? else {
        return Ok(false);
    };
    let store = &store.sqlite;
    let target = crate::ops::task_ownership::resolve_machine(store, target)?;
    if target == store.local_machine()?.id {
        return Ok(true);
    }
    let Ok(cli) = parse_remote_command(lf_args) else {
        return Ok(false);
    };
    let Some(crate::lf::Commands::Task {
        cmd:
            crate::lf::TaskCommand::Move {
                issue,
                machine: Some(machine),
                ..
            },
    }) = &cli.command
    else {
        return Ok(false);
    };
    if crate::ops::task_ownership::resolve_machine(store, machine)? != target {
        return Ok(false);
    }
    let Some(task) = store.task_by_issue(issue)? else {
        return Ok(false);
    };
    Ok(store
        .task_move_in_flight(&task.id)?
        .is_some_and(|moving| moving.to_machine_id == target))
}

pub fn run(
    target: &str,
    forward_agent: bool,
    cli: &Cli,
    lf_args: &[String],
    task_route: Option<TaskExecutionRoute>,
) -> anyhow::Result<()> {
    run_with_io(
        target,
        forward_agent,
        cli,
        lf_args,
        task_route,
        &std::env::current_dir()?,
        None,
    )
}

pub(super) fn run_with_io(
    target: &str,
    forward_agent: bool,
    cli: &Cli,
    lf_args: &[String],
    task_route: Option<TaskExecutionRoute>,
    directory: &std::path::Path,
    mut io: Option<&mut super::mutation::CommandIo<'_>>,
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
            cmd: crate::lf::SessionCommand::Card { .. }
                | crate::lf::SessionCommand::List(_)
                | crate::lf::SessionCommand::Resume { .. }
                | crate::lf::SessionCommand::New { .. }
                | crate::lf::SessionCommand::Bind { .. }
        }) | Some(crate::lf::Commands::Task {
            cmd: crate::lf::TaskCommand::Location { .. }
                | crate::lf::TaskCommand::Claim { .. }
                | crate::lf::TaskCommand::Unassign { .. }
                | crate::lf::TaskCommand::WatchFiles { .. }
                | crate::lf::TaskCommand::Files { .. }
                | crate::lf::TaskCommand::File { .. }
                | crate::lf::TaskCommand::Diff { .. }
                | crate::lf::TaskCommand::Changes { .. }
                | crate::lf::TaskCommand::Show { .. }
                | crate::lf::TaskCommand::Detail { .. }
                | crate::lf::TaskCommand::Save { .. }
                | crate::lf::TaskCommand::Create { run: false, .. }
        }) | Some(crate::lf::Commands::Plan {
            cmd: crate::lf::PlanCommand::Board { .. }
        }) | Some(crate::lf::Commands::Desktop {
            cmd: crate::lf::DesktopCommand::Shell { .. }
        }) | Some(crate::lf::Commands::Wave {
            cmd: crate::lf::WaveCommand::Show { .. }
        })
    );
    let runtime = tokio::runtime::Runtime::new()?;
    if let Some(id) = super::work_route::resumed_session(cli).filter(|_| !preview) {
        // Resuming a Session that lives here, or moved elsewhere, moves it (or its Task) there first.
        super::session_move::move_session(id, target, forward_agent)?;
    }
    let source = if preview || resident_operation {
        None
    } else {
        runtime.block_on(resolve_task_source(cli.task.as_deref(), directory))?
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
        super::session_move::move_task(task, target.id.as_str(), None)?;
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
        None => current_repository(directory)?,
    };
    // Preview forwards only authored selectors. Account lookup and connection
    // belong to launch, on neither side of this read. The target launches in
    // its own shared home.
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
    let user_name = crate::config::participant_name_in(directory)?.unwrap_or_default();
    let origin = runtime.block_on(super::machine::local_machine())?;
    let profile = crate::config::remote::outgoing(Some(directory), origin.id.as_str())?;
    let profile = serde_json::to_string(&profile)?;
    let mut extra_env = vec![
        (crate::config::remote::PROFILE_ENV, profile.as_str()),
        (EXPECTED_MACHINE_ID_ENV, target.id.as_str()),
        (crate::config::USER_NAME_ENV, user_name.as_str()),
    ];
    if let Some(selection) = selection.as_deref() {
        extra_env.push((
            crate::provider_account::selection::ACCOUNT_SELECTION_ENV,
            selection,
        ));
    }
    let agent = std::env::var(crate::config::AGENT_OVERRIDE_ENV).ok();
    if let Some(agent) = agent.as_deref() {
        extra_env.push((crate::config::AGENT_OVERRIDE_ENV, agent));
    }
    let settings = std::env::var("LF_SETTINGS").ok();
    if let Some(settings) = settings.as_deref() {
        extra_env.push(("LF_SETTINGS", settings));
    }
    if let Some(source) = source_json.as_deref() {
        extra_env.push((crate::lf::TASK_SOURCE_ENV, source));
    }
    let mut cmd = vec!["lf".to_string()];
    if cli.repository.is_none() {
        if let Some(repository) = &repository {
            cmd.extend(["--repo-id".into(), repository.to_string()]);
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
        output(&mut io, &String::from_utf8(bytes)?)?;
        return Ok(());
    }
    let connection = match &cli.command {
        Some(crate::lf::Commands::Session {
            cmd:
                crate::lf::SessionCommand::Resume { connection, .. }
                | crate::lf::SessionCommand::New { connection, .. },
        }) if !preview => Some(connection),
        _ => None,
    };
    let replace = connection.is_none_or(|connection| {
        connection.intent() == crate::lf::commands::write::SessionOpenIntent::Resume
    });
    // Only a person at a terminal views the remote Session; scripts and pipes
    // keep the plain SSH command, whose stdin reaches the provider directly.
    let attended = !cli.batch && std::io::stdin().is_terminal();
    if let Some(connection) = connection.filter(|connection| {
        connection.prepare
            || ((connection.json || cli.json) && attended && connection.message.is_none())
    }) {
        if !connection.prepare {
            cmd.push("--prepare".into());
        }
        if !connection.json && !cli.json {
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
                &[
                    (EXPECTED_MACHINE_ID_ENV, target.id.as_str()),
                    (crate::transport::STREAM_ENV, "1"),
                ],
            );
            record.open_argv = ssh_argv(&target.route, forward_agent, &script)?;
        }
        if !connection.prepare {
            anyhow::ensure!(
                !record.open_argv.is_empty(),
                "Session has no viewer launch command"
            );
            crate::transport::relay_over(&record.open_argv, replace)?;
            // Read the returned Session without repeating New/fork or starting
            // another provider. JSON belongs after the terminal has returned.
            let bytes = runtime.block_on(request(
                &target,
                &[
                    "session".into(),
                    "resume".into(),
                    record.id.to_string(),
                    "--prepare".into(),
                    "--json".into(),
                ],
                None,
            ))?;
            output(&mut io, &String::from_utf8(bytes)?)?;
            return Ok(());
        }
        if !record.open_argv.is_empty() {
            record.open_argv.splice(
                0..0,
                [
                    std::env::current_exe()?.to_string_lossy().into_owned(),
                    "session".into(),
                    "attach".into(),
                    "--".into(),
                ],
            );
        }
        if !replace && !record.open_argv.is_empty() {
            record
                .open_argv
                .splice(0..0, ["/usr/bin/env".into(), "LF_RELAY_IF_FREE=1".into()]);
        }
        if connection.json || cli.json {
            output(&mut io, &format!("{}\n", serde_json::to_string(&record)?))?;
        } else {
            output(
                &mut io,
                &format!(
                    "{}\n",
                    shlex::try_join(record.open_argv.iter().map(String::as_str))?
                ),
            )?;
        }
        return Ok(());
    }
    let viewer = connection
        .is_some_and(|connection| !connection.prepare && connection.message.is_none() && attended)
        || (!preview
            && matches!(
                &cli.command,
                Some(crate::lf::Commands::Desktop {
                    cmd: crate::lf::DesktopCommand::Shell { args },
                }) if !args.close
            ));
    if viewer {
        extra_env.push((crate::transport::STREAM_ENV, "1"));
    }
    let preamble = build_preamble(&cmd, &extra_env);
    match io {
        Some(io) => run_ssh_io(&target.route, forward_agent, &preamble, cli, io),
        None if viewer => crate::transport::relay_over(
            &ssh_argv(&target.route, forward_agent, &preamble)?,
            replace,
        ),
        None => run_ssh(&target.route, forward_agent, &preamble),
    }
}

fn output(io: &mut Option<&mut super::mutation::CommandIo<'_>>, text: &str) -> anyhow::Result<()> {
    match io {
        Some(io) => io.output.write_all(text.as_bytes())?,
        None => {
            use std::io::Write;
            std::io::stdout().write_all(text.as_bytes())?;
        }
    }
    Ok(())
}

fn run_ssh_io(
    dest: &str,
    forward_agent: bool,
    preamble: &str,
    cli: &Cli,
    io: &mut super::mutation::CommandIo<'_>,
) -> anyhow::Result<()> {
    use std::io::Write;
    let args = ssh_argv(dest, forward_agent, preamble)?;
    let mut input = Vec::new();
    let reads_input = matches!(
        &cli.command,
        Some(crate::lf::Commands::Task {
            cmd: crate::lf::TaskCommand::Save { .. }
                | crate::lf::TaskCommand::Create {
                    request: None,
                    notes: None,
                    ..
                },
        }) | Some(crate::lf::Commands::Desktop {
            cmd: crate::lf::DesktopCommand::Layout { save: true, .. },
        })
    );
    if reads_input {
        io.input.read_to_end(&mut input)?;
    }
    let mut child = Command::new(&args[0])
        .args(&args[1..])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()?;
    let mut stdin = child.stdin.take().expect("piped SSH stdin");
    // Drain output while sending large drafts: either pipe can fill first.
    let output = std::thread::scope(|scope| {
        let sending = scope.spawn(move || stdin.write_all(&input));
        let output = child.wait_with_output()?;
        let sent = sending.join().expect("SSH input writer panicked");
        if output.status.success() {
            sent?;
        }
        Ok::<_, std::io::Error>(output)
    })?;
    io.output.write_all(&output.stdout)?;
    io.error.write_all(&output.stderr)?;
    command_result(dest, output.status.code())
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

async fn resolve_task_source(
    selector: Option<&str>,
    directory: &std::path::Path,
) -> anyhow::Result<Option<TaskSource>> {
    let Some(selector) = selector else {
        return Ok(None);
    };
    let Some(store) = crate::store::open_existing_store().await? else {
        return Ok(None);
    };
    // Validate pushed code before any planning I/O can hide missing source work.
    let source = TaskSource::resolve(&store, selector).await?;
    let repo = match store.get_task_by_issue(selector).await? {
        Some(task) => Some(crate::ops::task::repository_for_task(&store, &task)?),
        None => crate::repo::discover_repo_root(directory).ok().flatten(),
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
        .await?
        .ok_or_else(|| anyhow!("machine commands need an initialized local store"))?;
    let machine = super::machine::find_machine(&store, target).await?;
    if machine.id == store.sqlite.local_machine()?.id {
        return Ok(machine);
    }
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
/// `lf repo`, when the destination runs in its home directory.
fn current_repository(
    directory: &std::path::Path,
) -> anyhow::Result<Option<crate::durable::RepositoryId>> {
    let Some(repo) = crate::repository::CanonicalRepo::containing(directory)? else {
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
fn ssh_argv(dest: &str, forward_agent: bool, preamble: &str) -> anyhow::Result<Vec<String>> {
    let args = crate::machine_route::bounded_ssh_args(dest, forward_agent)?;
    let mut argv = vec!["ssh".into()];
    argv.extend(args);
    argv.push(preamble.into());
    Ok(argv)
}

fn run_ssh(dest: &str, forward_agent: bool, preamble: &str) -> anyhow::Result<()> {
    let mut argv = ssh_argv(dest, forward_agent, preamble)?;
    if std::io::stdin().is_terminal() {
        for arg in &mut argv {
            if arg == "-T" {
                *arg = "-t".into();
            }
        }
    }
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
                "--repo-id".into(),
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
                "--repo-id",
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
        let args = ["--account", "person@", "skill", "implement"].map(str::to_string);
        assert_eq!(
            resident_args(&args, &parse_remote_command(&args).unwrap(), false, None),
            ["skill", "implement"]
        );
    }
    #[test]
    fn remote_prompt_preserves_literal_account_flags() {
        let args = [
            "--only-account=codex=person@",
            "skill",
            "implement",
            "--",
            "--shared",
            "--account=other@",
        ]
        .map(str::to_string);
        assert_eq!(
            resident_args(&args, &parse_remote_command(&args).unwrap(), false, None),
            ["skill", "implement", "--", "--shared", "--account=other@"]
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
