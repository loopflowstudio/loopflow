//! `lf task create REQUEST --run`: file the Task, start it exactly as
//! `lf task run` does, then open its conversation with the request as its
//! first turn. Desktop's New Task calls this same command.

use std::io::IsTerminal;
use std::os::fd::AsFd;
use std::path::Path;
use std::process::{Command, Stdio};

use anyhow::{bail, Context, Result};
use serde::{Deserialize, Serialize};

use crate::lf::Cli;
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
    if crate::task_graph::CapturedGraph::load(name, repo)?.is_none() {
        let available = crate::graph::available_graph_names(repo)
            .map(|names| names.join(", "))
            .unwrap_or_default();
        bail!(
            "{}. Available: {available}",
            crate::task_graph::missing_graph(name)
        );
    }
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
        let session_id = match cli.batch {
            true => None,
            false => Some(
                primary::ensure_task(
                    &store,
                    repo,
                    &task.identifier,
                    false,
                    request.map(str::to_owned),
                )
                .await?
                .id,
            ),
        };
        Ok((graph, session_id))
    })
}
