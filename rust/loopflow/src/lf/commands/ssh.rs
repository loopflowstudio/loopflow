//! Run a command on an added machine using credentials resident there.
//! Login transfers are separate foreground commands; this script contains no secrets.

use crate::lf::Cli;
use anyhow::anyhow;
use clap::Parser;
use loopflow_execution::durable::TaskExecutionRoute;
use loopflow_execution::machine::connection::resolve_target;
use loopflow_execution::machine::ssh::{build_preamble, request, EXPECTED_MACHINE_ID_ENV};
use loopflow_execution::machine::ssh::{run_ssh, ssh_argv};
use loopflow_execution::ops::human_session::SessionRecord;
use loopflow_model::durable::Machine;
use std::io::IsTerminal;

/// Whether `lf --machine <target> <args>` runs on this Machine: the target is
/// this Machine, or the command is `task move X --machine <target>` and this
/// Machine began moving X there without it arriving. Run here, a leaving move
/// carries on and a sent move delivers what was sent; forwarded, the target
/// would see a move from a Machine the forwarded personal settings do not name.
pub fn runs_here(target: &str, lf_args: &[String]) -> anyhow::Result<bool> {
    let Some(store) = loopflow_execution::store::read_existing_registry()? else {
        return Ok(false);
    };
    let store = &store.sqlite;
    let target = loopflow_execution::ops::task_ownership::resolve_machine(store, target)?;
    if target == loopflow_execution::store::machines::local_machine(store)?.id {
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
    if loopflow_execution::ops::task_ownership::resolve_machine(store, machine)? != target {
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
    mut io: Option<&mut loopflow_execution::command::CommandIo<'_>>,
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
    // Only a command that may start an agent carries `forward`; a named value
    // this Machine cannot supply stops it before any SSH. Session viewers are
    // started by the destination's server, which carries nothing.
    let launching = cli.may_launch_agent() && !resident_operation;
    let forwarded = (!preview && launching)
        .then(|| loopflow_execution::machine::forward::read(directory))
        .transpose()?;
    let runtime = tokio::runtime::Runtime::new()?;
    if let Some(id) = super::work_route::resumed_session(cli).filter(|_| !preview) {
        // Resuming a Session that lives here, or moved elsewhere, moves it (or its Task) there first.
        loopflow_execution::ops::session_move::move_session(
            id,
            target,
            forward_agent,
            super::write::run_task,
        )?;
    }
    let source = if preview || resident_operation {
        None
    } else {
        runtime.block_on(loopflow_execution::machine::task::resolve_task_source(
            cli.task.as_deref(),
            directory,
        ))?
    };
    let target = if preview || resident_operation {
        runtime
            .block_on(loopflow_execution::machine::connection::find_registered_machine(target))?
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
        loopflow_execution::ops::session_move::move_task(
            task,
            target.id.as_str(),
            None,
            super::write::run_task,
        )?;
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
        None => loopflow_execution::machine::task::current_repository(directory)?,
    };
    // A plain command reads what it runs with from its environment.
    let mut environment =
        loopflow_execution::machine::wire::Selection::capture(directory, source.clone())?
            .environment(&loopflow_execution::machine::wire::Secrets::default())?;
    environment.push((
        loopflow_execution::machine::ssh::EXPECTED_MACHINE_ID_ENV.into(),
        target.id.to_string(),
    ));
    environment.push((
        loopflow_execution::machine::login::MACHINE_NAME_ENV.into(),
        target.name(),
    ));
    let mut extra_env: Vec<(&str, &str)> = environment
        .iter()
        .map(|(name, value)| (name.as_str(), value.as_str()))
        .collect();
    let mut cmd = vec!["lf".to_string()];
    if cli.repository.is_none() {
        if let Some(repository) = &repository {
            cmd.extend(["--repo-id".into(), repository.to_string()]);
        }
    }
    cmd.extend(if preview {
        lf_args.to_vec()
    } else {
        loopflow_execution::machine::task::resident_args(
            lf_args,
            cli.task.as_ref(),
            cli.repo.is_some(),
            routed,
            source.as_ref(),
        )
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
    let unfocused = connection.is_some_and(|connection| connection.unfocused);
    if unfocused {
        // The destination bridges bytes. The local relay owns this option.
        cmd.retain(|arg| arg != "--unfocused");
    }
    // Only a person at a terminal views the remote Session; scripts and pipes
    // keep the plain SSH command, whose stdin reaches the provider directly.
    let attended = !cli.batch && std::io::stdin().is_terminal();
    if let Some(connection) = connection
        .filter(|connection| connection.prepare || ((connection.json || cli.json) && attended))
    {
        if !connection.prepare {
            cmd.push("--prepare".into());
        }
        if !connection.json && !cli.json {
            cmd.push("--json".into());
        }
        let bytes = runtime.block_on(request(&target, &cmd[1..], None))?;
        let record = serde_json::from_slice(&bytes)?;
        if !connection.prepare {
            let record = viewed_remotely(record, &target, forward_agent)?;
            anyhow::ensure!(
                !record.open_argv.is_empty(),
                "Session has no viewer launch command"
            );
            loopflow_execution::transport::relay_over(&record.open_argv, unfocused)?;
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
        let record = prepared_remotely(record, &target, forward_agent)?;
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
    let viewer = connection.is_some_and(|connection| !connection.prepare && attended)
        || (!preview
            && matches!(
                &cli.command,
                Some(crate::lf::Commands::Desktop {
                    cmd: crate::lf::DesktopCommand::Shell { args },
                }) if !args.close
            ));
    if viewer {
        extra_env.push((loopflow_execution::transport::STREAM_ENV, "1"));
    }
    let mut preamble = build_preamble(&cmd, &extra_env);
    if let Some(forwarded) = &forwarded {
        let mut stderr = std::io::stderr();
        let notices: &mut dyn std::io::Write = match io.as_mut() {
            Some(io) => &mut *io.error,
            None => &mut stderr,
        };
        if let Some(file) =
            loopflow_execution::machine::forward::deliver(&target, forwarded, notices)?
        {
            preamble = loopflow_execution::machine::forward::sourced(&file, &preamble);
        }
    }
    match io {
        Some(io) => {
            let reads_input = matches!(
                &cli.command,
                Some(crate::lf::Commands::Task {
                    cmd: crate::lf::TaskCommand::Save { .. }
                        | crate::lf::TaskCommand::Create {
                            request: None,
                            notes: None,
                            ..
                        },
                })
            );
            loopflow_execution::machine::ssh::run_ssh_io(
                &target.route,
                forward_agent,
                &preamble,
                reads_input,
                io,
            )
        }
        None if viewer => loopflow_execution::transport::relay_over(
            &ssh_argv(&target.route, forward_agent, &preamble)?,
            unfocused,
        ),
        None => run_ssh(&target.route, forward_agent, &preamble),
    }
}

/// A Session `target` prepared, its viewer command reaching it over SSH.
fn viewed_remotely(
    mut record: SessionRecord,
    target: &Machine,
    forward_agent: bool,
) -> anyhow::Result<SessionRecord> {
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
                (loopflow_execution::transport::STREAM_ENV, "1"),
            ],
        );
        record.open_argv = ssh_argv(&target.route, forward_agent, &script)?;
    }
    Ok(record)
}

/// A Session `target` prepared, opened from here through `lf session attach`.
pub(super) fn prepared_remotely(
    record: SessionRecord,
    target: &Machine,
    forward_agent: bool,
) -> anyhow::Result<SessionRecord> {
    let mut record = viewed_remotely(record, target, forward_agent)?;
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
    Ok(record)
}

fn output(
    io: &mut Option<&mut loopflow_execution::command::CommandIo<'_>>,
    text: &str,
) -> anyhow::Result<()> {
    match io {
        Some(io) => io.output.write_all(text.as_bytes())?,
        None => {
            use std::io::Write;
            std::io::stdout().write_all(text.as_bytes())?;
        }
    }
    Ok(())
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

#[cfg(test)]
mod dispatch_tests;

#[cfg(test)]
mod tests {
    use super::parse_remote_command;
    use loopflow_execution::machine::task::resident_args;

    #[test]
    fn repository_paths_stay_on_explicit_machine_but_not_automatic_work_routes() {
        for selection in [vec!["--repo", "~/src/remote"], vec!["--repo=~/src/remote"]] {
            let mut args = selection;
            args.extend([
                "--task", "LOO-427", "skill", "debug", "--", "--repo", "literal",
            ]);
            let args = args.into_iter().map(str::to_owned).collect::<Vec<_>>();
            let cli = parse_remote_command(&args).unwrap();
            assert_eq!(
                resident_args(&args, cli.task.as_ref(), cli.repo.is_some(), false, None),
                args
            );
            assert_eq!(
                resident_args(&args, cli.task.as_ref(), cli.repo.is_some(), true, None),
                ["--task", "LOO-427", "skill", "debug", "--", "--repo", "literal"]
            );
        }
    }
}
