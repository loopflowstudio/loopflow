//! `lf task create REQUEST --run`: file the Task, start it exactly as
//! `lf task run` does, then open its conversation with the request as its
//! first turn. Desktop's New Task calls this same command.

use std::io::IsTerminal;
use std::os::fd::AsFd;
use std::path::Path;
use std::process::{Command, Stdio};

use anyhow::{bail, Context, Result};
use serde::{Deserialize, Serialize};

use crate::lf::{Cli, TaskCommand};
use crate::ops::human_session::primary;
use crate::store::{open_store, storage_config_from_env};
use crate::work::task::Task;

use super::mutation::{regressed_in_arg, CommandIo};

/// What `lf task create --run --json` prints whenever the Task was filed: the
/// Task, the graph it took up and its admitted conversation. A headless (`-b`)
/// run admits no conversation; a run that failed leaves both absent and the
/// command still exits non-zero, so callers never file the request twice.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TaskCreateReceipt {
    pub task: Task,
    pub graph: Option<String>,
    pub session_id: Option<crate::id::LfSessionId>,
}

/// File or adopt a Task with the caller's streams; execution remains separate.
pub fn create(repo: &Path, command: &TaskCommand, io: &mut CommandIo<'_>) -> Result<Task> {
    let TaskCommand::Create {
        session,
        worktree,
        request,
        no_wave,
        wave,
        title,
        notes,
        regressed_in,
        run,
        graph,
        json,
    } = command
    else {
        bail!("expected task create");
    };
    if let Some(graph) = graph {
        // Validate before creation so a typo leaves no Task behind.
        crate::ops::task::load_graph(graph, "", repo)?;
    }
    let report = match request.as_ref().or(notes.as_ref()) {
        Some(text) => Some(text.clone()),
        None => read_report(io.input)?,
    };
    let task = crate::ops::task::task_create(
        repo,
        if *no_wave { None } else { wave.as_deref() },
        title.clone(),
        report,
        regressed_in_arg(regressed_in),
        crate::ops::task::TaskAdoptionOptions {
            session: session.clone(),
            worktree: worktree.clone(),
        },
    )?;
    match (*json, *run) {
        (false, _) => writeln!(io.output, "{} · {}", task.title, task.identifier)?,
        (true, false) => writeln!(io.output, "{}", serde_json::to_string_pretty(&task)?)?,
        // The launch prints one receipt once the run settles.
        (true, true) => {}
    }
    if let Some(provider) = crate::config::load_config_or_default(Some(repo))
        .planning_transport()
        .remote_name()
        .filter(|_| task.linear_id.is_none())
    {
        writeln!(
            io.error,
            "Saved locally; pending {provider} sync. Retry with `lf task sync {}`.",
            task.identifier
        )?;
    }
    Ok(task)
}

pub fn read_report(input: &mut dyn std::io::Read) -> Result<Option<String>> {
    let mut report = String::new();
    input.read_to_string(&mut report)?;
    Ok((!report.trim().is_empty()).then_some(report))
}

/// Start the created Task and open its conversation.
pub fn start(
    cli: &Cli,
    repo: &Path,
    task: &Task,
    graph: Option<&str>,
    request: Option<&str>,
    json: bool,
) -> Result<()> {
    let issue = &task.identifier;
    // `lf task run` places the Task and returns once its Graph registers;
    // `-b` waits for `end`. Its lines go to stderr when stdout is the receipt.
    let mut run = Command::new(std::env::current_exe()?);
    run.args(cli.step_args());
    if let Some(agent) = &cli.agent {
        run.args(["--agent", agent]);
    }
    run.args(["task", "run", issue])
        .args(graph)
        .current_dir(repo);
    if json {
        run.stdout(Stdio::from(std::io::stderr().as_fd().try_clone_to_owned()?));
    }
    let status = run.status().context("start `lf task run`")?;
    let receipt = |graph, session_id| -> Result<()> {
        let receipt = TaskCreateReceipt {
            task: task.clone(),
            graph,
            session_id,
        };
        println!("{}", serde_json::to_string_pretty(&receipt)?);
        Ok(())
    };
    if !status.success() {
        if json {
            receipt(None, None)?;
        }
        eprintln!("{issue} was created; start it again with `lf task run {issue}`");
        return Err(crate::process::CommandExit(
            status
                .code()
                .and_then(|code| u8::try_from(code).ok())
                .unwrap_or(1),
        )
        .into());
    }
    let (graph, session_id) = admit(cli, repo, task, request)?;
    if json {
        return receipt(Some(graph), session_id);
    }
    // A headless run has no conversation to open.
    if session_id.is_none() {
        return Ok(());
    }
    if !(std::io::stdin().is_terminal() && std::io::stdout().is_terminal()) {
        println!("Open its conversation: lf resume --task {issue}");
        return Ok(());
    }
    let status = Command::new(std::env::current_exe()?)
        .args(["resume", "--task", issue])
        .current_dir(repo)
        .status()
        .context("open the Task's conversation")?;
    if !status.success() {
        bail!("its conversation did not open; open it with `lf resume --task {issue}`");
    }
    Ok(())
}

/// The graph the run took up and, unless headless, the Task's primary
/// conversation with `request` as its first turn.
fn admit(
    cli: &Cli,
    repo: &Path,
    task: &Task,
    request: Option<&str>,
) -> Result<(String, Option<crate::id::LfSessionId>)> {
    tokio::runtime::Runtime::new()?.block_on(async {
        let store = std::sync::Arc::new(open_store(&storage_config_from_env()?).await?);
        let graph = store
            .sqlite
            .task_graph(&task.id)?
            .context("the run did not take up a graph")?
            .name;
        // A headless run has no one to talk to; its conversation is not admitted.
        let session_id = if cli.batch {
            None
        } else {
            let request = request.map(str::to_owned);
            Some(
                primary::ensure_task(&store, repo, &task.identifier, request)
                    .await?
                    .0
                    .id,
            )
        };
        Ok((graph, session_id))
    })
}
