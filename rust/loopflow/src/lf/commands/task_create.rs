//! `lf task create REQUEST --run`: file the Task, start it exactly as
//! `lf task run` does, then open its conversation with the request as its
//! first turn. Desktop's New Task calls this same command.

use std::fs::OpenOptions;
use std::io::IsTerminal;
use std::os::fd::AsFd;
use std::os::unix::fs::OpenOptionsExt;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

use anyhow::{bail, Context, Result};
use fs2::FileExt;
use serde::{Deserialize, Serialize};

use crate::lf::{Cli, Commands, TaskCommand};
use crate::ops::human_session::primary;
use crate::store::{open_store, storage_config_from_env};
use crate::work::task::Task;

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

/// Refuse an unknown graph before any Task exists, so a typo leaves nothing behind.
pub fn check_graph(repo: &Path, name: &str) -> Result<()> {
    crate::ops::task::load_graph(name, "", repo)?;
    Ok(())
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
    let result = launch(cli, repo, task, graph, request, json);
    if json {
        let receipt = result.as_ref().cloned().unwrap_or_else(|_| filed(task));
        println!("{}", serde_json::to_string_pretty(&receipt)?);
    }
    let receipt = result?;
    if json {
        return Ok(());
    }
    // A headless run has no conversation to open.
    if receipt.session_id.is_none() {
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

fn filed(task: &Task) -> TaskCreateReceipt {
    TaskCreateReceipt {
        task: task.clone(),
        graph: None,
        session_id: None,
    }
}

// Both ordinary CLI creation and recoverable admission start through this path.
fn launch(
    cli: &Cli,
    repo: &Path,
    task: &Task,
    graph: Option<&str>,
    request: Option<&str>,
    json: bool,
) -> Result<TaskCreateReceipt> {
    let issue = &task.identifier;
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
    if !status.success() {
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
    Ok(TaskCreateReceipt {
        task: task.clone(),
        graph: Some(graph),
        session_id,
    })
}

/// A recoverable create/start result. In-progress and uncertain outcomes never
/// authorize another Graph run; clients keep the same request ID and Task ID.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TaskCreateOutcome {
    pub task_id: crate::durable::TaskId,
    pub state: TaskCreateState,
    pub receipt: Option<TaskCreateReceipt>,
    pub message: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
#[non_exhaustive]
pub enum TaskCreateState {
    InProgress,
    Started,
    Failed,
    Unknown,
}

#[derive(Serialize)]
struct Intent<'a> {
    repository: PathBuf,
    wave: Option<&'a str>,
    title: Option<&'a str>,
    report: Option<&'a str>,
    request: Option<&'a str>,
    regressed_in: Option<&'a str>,
    graph: Option<&'a str>,
    agent: Option<&'a str>,
    step_args: Vec<String>,
}

/// Admit the whole CLI operation before effects. Only the request lock holder
/// may file/start it. A crash before start is recoverable with the reserved Task
/// ID; a crash across Graph launch is reported, never automatically replayed.
pub fn recoverable(
    cli: &Cli,
    repo: &Path,
    id: &crate::id::TaskCreateRequestId,
    report: Option<String>,
) -> Result<TaskCreateOutcome> {
    let Some(Commands::Task {
        cmd:
            TaskCommand::Create {
                request,
                wave,
                title,
                regressed_in,
                graph,
                ..
            },
    }) = &cli.command
    else {
        bail!("creation admission requires task create");
    };
    let repo = crate::git::worktrees::main_repo_root(repo)?;
    let intent = serde_json::to_string(&Intent {
        repository: repo.clone(),
        wave: wave.as_deref(),
        title: title.as_deref(),
        report: report.as_deref(),
        request: request.as_deref(),
        regressed_in: regressed_in.as_deref(),
        graph: graph.as_deref(),
        agent: cli.agent.as_deref(),
        step_args: cli.step_args(),
    })?;
    let store =
        tokio::runtime::Runtime::new()?.block_on(open_store(&storage_config_from_env()?))?;
    let id = uuid::Uuid::parse_str(id.as_str())?.to_string();
    let reservation = store.sqlite.reserve_task_create(&id, &intent)?;
    let locks = crate::store::lf_home_dir().join("task-create-locks");
    std::fs::create_dir_all(&locks)?;
    let lock = OpenOptions::new()
        .read(true)
        .write(true)
        .create(true)
        .truncate(false)
        .mode(0o600)
        .open(locks.join(&id))?;
    match lock.try_lock_exclusive() {
        Ok(()) => {}
        Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {
            return Ok(TaskCreateOutcome {
                task_id: reservation.task_id,
                state: TaskCreateState::InProgress,
                receipt: None,
                message: None,
            });
        }
        Err(error) => return Err(error.into()),
    }
    // The previous holder may have completed between reservation and lock.
    let reservation = store.sqlite.reserve_task_create(&id, &intent)?;
    if let Some(result) = reservation.result {
        return Ok(serde_json::from_str(&result)?);
    }
    if reservation.start_attempted {
        return Ok(TaskCreateOutcome {
            receipt: store.sqlite.task(&reservation.task_id)?.as_ref().map(filed),
            task_id: reservation.task_id,
            state: TaskCreateState::Unknown,
            message: Some(
                "The start result was interrupted. Inspect this Task before starting it again."
                    .into(),
            ),
        });
    }
    if let Some(graph) = graph {
        check_graph(&repo, graph)?;
    }
    let regression = regressed_in.as_deref().map(|version| {
        crate::work::task::release_version(version).unwrap_or_else(|| version.to_owned())
    });
    let task = crate::ops::task::task_create_with_id(
        &repo,
        wave.as_deref(),
        title.clone(),
        report,
        regression,
        reservation.task_id.clone(),
    )?;
    store.sqlite.begin_task_create_start(&id)?;
    let result = launch(
        cli,
        &repo,
        &task,
        graph.as_deref(),
        request.as_deref(),
        true,
    );
    let outcome = match result {
        Ok(receipt) => TaskCreateOutcome {
            task_id: task.id.clone(),
            state: TaskCreateState::Started,
            receipt: Some(receipt),
            message: None,
        },
        Err(error) => TaskCreateOutcome {
            task_id: task.id.clone(),
            state: TaskCreateState::Failed,
            receipt: Some(filed(&task)),
            message: Some(format!(
                "{error:#}; {} was created. Inspect it before trying another start.",
                task.identifier
            )),
        },
    };
    store
        .sqlite
        .finish_task_create(&id, &serde_json::to_string(&outcome)?)?;
    Ok(outcome)
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
