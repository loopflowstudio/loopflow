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
                | crate::lf::TaskCommand::Find { .. }
                | crate::lf::TaskCommand::Grep { .. }
                | crate::lf::TaskCommand::Symbols { .. }
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
        loopflow_execution::ops::session_move::move_session(id, target, super::write::run_task)?;
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
    let search = match &cli.command {
        Some(crate::lf::Commands::Task { cmd }) => checkout_search(cmd, task_route.as_ref()),
        _ => None,
    };
    let work_route = match task_route {
        Some(route) => Some(route),
        // A path/name addresses the destination's filesystem, not the caller's.
        None if cli.repo.is_some() || search.is_some() => None,
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
    // The checkout's server shares one inventory across a peer picker's keystrokes.
    if let (Some((read, true)), Some(repository), false) = (search, &repository, preview) {
        use loopflow_execution::machine::wire::{Secrets, Selection, Sender};
        let sender = Sender {
            repository: repository.clone(),
            selection: Selection::capture(directory, None)?,
            secrets: Secrets::default(),
        };
        let mut connection =
            loopflow_execution::machine::client::Connection::open_on(&target, sender, None)?;
        let answer = connection.request(crate::server::wire::Operation::Read(read))?;
        return output(
            &mut io,
            &format!("{}\n", serde_json::to_string_pretty(&answer.result)?),
        );
    }
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
        let record = prepared_remotely(record, &target, None)?;
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

/// A Session `target` prepared, opened here by `lf open --here`, which this
/// Machine's server attaches where it runs. `route` is the Task this Machine
/// routed the resume for.
pub(super) fn prepared_remotely(
    record: SessionRecord,
    target: &Machine,
    route: Option<&TaskExecutionRoute>,
) -> anyhow::Result<SessionRecord> {
    anyhow::ensure!(
        record
            .workspace
            .as_ref()
            .is_some_and(|workspace| workspace.machine_id == target.id),
        "Session reply does not belong to the addressed Machine"
    );
    let mut value = serde_json::to_value(record)?;
    localize_remote_session(&mut value, target, route)?;
    Ok(serde_json::from_value(value)?)
}

/// A Session record `machine` sent, as this Machine opens it: every command it
/// names is this Machine's `lf open`, which attaches where the Session runs,
/// and a record read for a Task carries this Machine's ID for that Task.
/// `record` may be a selected reading; fields it lacks stay absent.
pub(crate) fn localize_remote_session(
    record: &mut serde_json::Value,
    machine: &Machine,
    route: Option<&TaskExecutionRoute>,
) -> anyhow::Result<()> {
    static TASK_RESUME: std::sync::LazyLock<regex::Regex> = std::sync::LazyLock::new(|| {
        regex::Regex::new(r"lf resume --task [^\s`]+").expect("the pattern is valid")
    });
    let Some(id) = record
        .get("id")
        .and_then(serde_json::Value::as_str)
        .map(str::to_owned)
    else {
        return Ok(());
    };
    let [flag, selector] = match route {
        Some(route) => ["--task".to_owned(), route.peer_selector.clone()],
        None => ["--machine".to_owned(), machine.id.to_string()],
    };
    let session = format!("lf open {flag} {selector} --session {id}");
    let primary = match route {
        Some(_) => format!("lf open {flag} {selector}"),
        None => session.clone(),
    };
    let session_resume = format!("lf resume {id}");
    for pointer in ["/action/help", "/outlook/command", "/status_line"] {
        if let Some(text) = record.pointer_mut(pointer) {
            if let Some(line) = text.as_str() {
                let line = TASK_RESUME.replace_all(line, primary.as_str());
                *text = line.replace(&session_resume, &session).into();
            }
        }
    }
    if let Some(argv) = record.get_mut("open_argv") {
        if argv.as_array().is_some_and(|argv| !argv.is_empty()) {
            let exe = std::env::current_exe()?.to_string_lossy().into_owned();
            *argv = vec![
                exe,
                "open".into(),
                flag,
                selector,
                "--session".into(),
                id,
                "--here".into(),
            ]
            .into();
        }
    }
    let Some(route) = route else {
        return Ok(());
    };
    // Readers take a record's Task from its first ID, its Work and its
    // workspace; each names the Task as this Machine knows it.
    let local = serde_json::Value::from(route.task_id.to_string());
    if let Some(task_ids) = record
        .get_mut("task_ids")
        .and_then(serde_json::Value::as_array_mut)
    {
        task_ids.retain(|id| id != &local);
        task_ids.insert(0, local.clone());
    }
    if let Some(work) = record.get_mut("work").filter(|work| work["kind"] == "task") {
        work["id"] = local.clone();
    }
    if let Some(task_id) = record
        .pointer_mut("/workspace/task_id")
        .filter(|task_id| !task_id.is_null())
    {
        *task_id = local;
    }
    Ok(())
}

/// A Session list `machine` answered for a Task: an array of records, or a
/// page of them.
pub(crate) fn localize_remote_sessions(
    list: &mut serde_json::Value,
    machine: &Machine,
    route: &TaskExecutionRoute,
) -> anyhow::Result<()> {
    let records = match list.get_mut("entries") {
        Some(entries) => entries,
        None => list,
    };
    if let Some(records) = records.as_array_mut() {
        for record in records {
            localize_remote_session(record, machine, Some(route))?;
        }
    }
    Ok(())
}

fn checkout_search(
    cmd: &crate::lf::TaskCommand,
    route: Option<&TaskExecutionRoute>,
) -> Option<(super::read::Read, bool)> {
    use super::read::Read;
    use crate::lf::TaskCommand;

    let owner_issue =
        |issue: &str| route.map_or_else(|| issue.to_owned(), |route| route.peer_selector.clone());
    match cmd {
        TaskCommand::Find {
            issue,
            query,
            limit,
            json,
        } => Some((
            Read::TaskFind {
                issue: owner_issue(issue),
                query: query.clone(),
                limit: *limit,
            },
            *json,
        )),
        TaskCommand::Symbols {
            issue,
            name,
            path,
            json,
        } => Some((
            Read::TaskSymbols {
                issue: owner_issue(issue),
                name: name.clone(),
                path: path.clone(),
            },
            *json,
        )),
        TaskCommand::Grep {
            issue,
            query,
            whole_word,
            json,
        } => Some((
            Read::TaskGrep {
                issue: owner_issue(issue),
                query: query.clone(),
                whole_word: *whole_word,
            },
            *json,
        )),
        _ => None,
    }
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
    use super::{localize_remote_session, localize_remote_sessions, parse_remote_command};
    use loopflow_execution::durable::{TaskExecutionRoute, TaskExecutionSource};
    use loopflow_execution::machine::task::resident_args;
    use loopflow_model::durable::{Machine, MachineId, RepositoryId, TaskId};
    use serde_json::json;

    fn mini() -> Machine {
        Machine {
            label: Some("mini".into()),
            id: MachineId::new(),
            route: "mini.local".into(),
            created_at: time::OffsetDateTime::UNIX_EPOCH,
            observed_at: time::OffsetDateTime::UNIX_EPOCH,
        }
    }

    fn route_on(machine: &Machine) -> TaskExecutionRoute {
        TaskExecutionRoute {
            task_id: TaskId::new(),
            peer_selector: "LOO-7".into(),
            checkout: None,
            repository_id: RepositoryId::from_raw("github.com/acme/app"),
            machine_id: machine.id.clone(),
            source: TaskExecutionSource::Owner,
        }
    }

    fn remote_record(peer_task: &str) -> serde_json::Value {
        json!({
            "id": "session_1",
            "task_ids": [peer_task],
            "work": { "kind": "task", "id": peer_task },
            "workspace": { "machine_id": "MACHINE_PEER", "worktree": "/src/app", "task_id": peer_task, "unavailable": null },
            "open_argv": ["/opt/lf", "resume", "session_1"],
            "action": {
                "label": "Open here",
                "help": "View the running conversation here: lf resume session_1",
            },
            "outlook": { "kind": "waiting", "at": null, "reason": null, "command": "lf resume --task LOO-7" },
            "status_line": "died 12:01 (exit 137); `lf resume --task LOO-7` opens it",
        })
    }

    #[test]
    fn a_routed_task_session_opens_with_this_machines_lf_open() {
        let machine = mini();
        let route = route_on(&machine);
        let mut record = remote_record("TASK_PEER");
        localize_remote_session(&mut record, &machine, Some(&route)).unwrap();
        assert_eq!(
            record["action"]["help"],
            "View the running conversation here: lf open --task LOO-7 --session session_1"
        );
        assert_eq!(record["outlook"]["command"], "lf open --task LOO-7");
        assert_eq!(
            record["status_line"],
            "died 12:01 (exit 137); `lf open --task LOO-7` opens it"
        );
        let argv = record["open_argv"].as_array().unwrap();
        assert_eq!(
            argv[1..],
            [
                "open",
                "--task",
                "LOO-7",
                "--session",
                "session_1",
                "--here"
            ]
        );
        let local = route.task_id.to_string();
        assert_eq!(record["task_ids"], json!([local, "TASK_PEER"]));
        assert_eq!(record["work"], json!({ "kind": "task", "id": local }));
        assert_eq!(record["workspace"]["task_id"], local.as_str());
    }

    #[test]
    fn a_routed_record_keeps_other_work_and_an_unowned_workspace() {
        let machine = mini();
        let route = route_on(&machine);
        let mut record = remote_record("TASK_PEER");
        record["work"] = json!({ "kind": "wave", "id": "WAVE_PEER" });
        record["workspace"]["task_id"] = serde_json::Value::Null;
        record["task_ids"] = json!(["TASK_OTHER", route.task_id.to_string()]);
        localize_remote_session(&mut record, &machine, Some(&route)).unwrap();
        assert_eq!(record["work"], json!({ "kind": "wave", "id": "WAVE_PEER" }));
        assert!(record["workspace"]["task_id"].is_null());
        assert_eq!(
            record["task_ids"],
            json!([route.task_id.to_string(), "TASK_OTHER"])
        );
    }

    #[test]
    fn a_machine_only_session_opens_by_machine_and_keeps_its_task_ids() {
        let machine = mini();
        let mut record = remote_record("TASK_PEER");
        localize_remote_session(&mut record, &machine, None).unwrap();
        let session = format!("lf open --machine {} --session session_1", machine.id);
        assert_eq!(record["outlook"]["command"], session.as_str());
        assert_eq!(
            record["action"]["help"],
            format!("View the running conversation here: {session}")
        );
        assert_eq!(record["task_ids"], json!(["TASK_PEER"]));
    }

    #[test]
    fn a_routed_page_and_a_selected_reading_localize_what_they_carry() {
        let machine = mini();
        let route = route_on(&machine);
        let mut page = json!({
            "entries": [remote_record("TASK_PEER"), { "id": "session_2", "status_line": "closed; `lf resume session_2` opens it" }],
            "next": null,
        });
        localize_remote_sessions(&mut page, &machine, &route).unwrap();
        assert_eq!(
            page["entries"][0]["outlook"]["command"],
            "lf open --task LOO-7"
        );
        assert_eq!(
            page["entries"][1],
            json!({ "id": "session_2", "status_line": "closed; `lf open --task LOO-7 --session session_2` opens it" })
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
