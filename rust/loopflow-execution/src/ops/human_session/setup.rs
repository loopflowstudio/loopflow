//! A Task's conversation sets up a Task that took up its Graph and never ran
//! it: the checkout, then the Graph, one line each in the terminal before the
//! provider starts. The Task owes setup until its Graph's run is recorded, so a
//! reopened conversation continues where setup stopped.

use std::time::Instant;

use anyhow::{Context, Result};
use loopflow_model::durable::TaskId;
use loopflow_storage::work::task::{Task, TaskEventKind};
use tracing::Instrument;

use super::primary;
use crate::session::LfSession;
use crate::store::SharedStore;

/// The Task `session` belongs to and the Graph it took up, when it has never
/// run that Graph.
pub(super) fn owed(store: &SharedStore, session: &LfSession) -> Result<Option<(TaskId, String)>> {
    let Some(task) = &session.task_id else {
        return Ok(None);
    };
    Ok(store
        .sqlite
        .unrun_task_graph(task)?
        .map(|unrun| (task.clone(), unrun.graph)))
}

/// Set up `task`, which owes `graph`'s start; `request_id` names the creation
/// whose Start these steps are timed under.
pub(super) async fn finish(
    store: &SharedStore,
    session: LfSession,
    (task, graph): (TaskId, String),
    request_id: Option<&str>,
    agent: Option<&str>,
) -> Result<LfSession> {
    if let Some(request) = session
        .request
        .as_deref()
        .and_then(|text| text.lines().next())
    {
        println!("{request}");
    }
    let started = Instant::now();
    let (session, task) = match step(request_id, "checkout", checkout(store, session, &task)).await
    {
        Ok(checked_out) => checked_out,
        Err(error) => {
            stopped(store, &task, "checkout", &error).await;
            // The launcher prints the error once, as the conversation's end.
            return Err(error.context("Checkout failed"));
        }
    };
    println!("Checkout ready  {:.1} s", started.elapsed().as_secs_f64());
    // The agent starts once the checkout exists, whatever happens to the Graph.
    match step(request_id, "graph", async {
        start_graph(&session, &task, agent)
    })
    .await
    {
        Ok(()) => println!("Graph {graph} started"),
        Err(error) => {
            println!("Graph did not start: {error:#}");
            stopped(store, &task.id, "graph", &error).await;
        }
    }
    Ok(session)
}

/// Record on the Task that setup stopped at `step`, so its position says so
/// until a reopened conversation runs setup again. A resumable failure never
/// refuses a later launch.
async fn stopped(store: &SharedStore, task: &TaskId, step: &str, error: &anyhow::Error) {
    tracing::warn!(error = %format!("{error:#}"), step, "Task setup stopped");
    let event = TaskEventKind::Failed {
        error: format!("Setup stopped: {step} failed: {error:#}"),
        resumable: true,
    };
    if let Err(error) = store.append_task_event(task, &event).await {
        tracing::warn!(%error, "Task setup failure was not recorded");
    }
}

/// One setup step, timed under its creation's request ID so a Start's report
/// nests it beside the server's creation steps. Never records request text.
async fn step<T>(
    request_id: Option<&str>,
    name: &'static str,
    run: impl std::future::Future<Output = T>,
) -> T {
    let span = tracing::info_span!(target: crate::journal::START_TIMING_TARGET, "conversation_setup", request_id, step = name);
    let started = Instant::now();
    let result = run.instrument(span.clone()).await;
    span.in_scope(|| {
        tracing::info!(
            target: crate::journal::START_TIMING_TARGET,
            elapsed_ms = started.elapsed().as_secs_f64() * 1000.0,
            "Setup step finished"
        )
    });
    result
}

/// Check the Task out from local refs, taking the repository's ready checkout
/// when one is waiting, move the conversation into it, and start the next ready
/// checkout.
async fn checkout(
    store: &SharedStore,
    session: LfSession,
    task: &TaskId,
) -> Result<(LfSession, Task)> {
    let task = store
        .get_task(task)
        .await?
        .context("the Task is unavailable")?;
    let repo = session
        .repo
        .as_deref()
        .map(std::path::PathBuf::from)
        .unwrap_or_else(|| session.cwd.clone());
    let placed = task.worktree.as_ref().is_some_and(|cwd| cwd.is_dir());
    let issue = task.id.to_string();
    crate::journal::request::spawn_blocking(move || -> Result<()> {
        if !placed {
            crate::ops::task::admitted_checkout(&repo, &issue)?;
        }
        crate::git::worktrees::prepare_ready_checkout(&crate::git::worktrees::main_repo_root(
            &repo,
        )?);
        Ok(())
    })
    .await??;
    Ok((primary::admit_workspace(store, session).await?, task))
}

/// Start the Task's run the way a terminal would, detached. The run records
/// itself on the Task's Graph, which ends what the Task owes; a second launch
/// racing it loses at that record.
fn start_graph(session: &LfSession, task: &Task, agent: Option<&str>) -> Result<()> {
    let mut argv = vec![crate::os_process::execution_context()?
        .lf_bin
        .to_string_lossy()
        .into_owned()];
    // The agent this conversation was started with runs its Graph too.
    if let Some(agent) = agent {
        argv.extend(["--agent".into(), agent.to_owned()]);
    }
    argv.extend(["task".into(), "run".into(), task.identifier.clone()]);
    crate::os_process::detached::start_lf(&session.cwd, &argv, &[], &|_| {})?;
    Ok(())
}
