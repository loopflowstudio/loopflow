//! A new Task's conversation finishes the setup its creation admitted: the
//! checkout, then the Graph, one line each in the terminal before the provider
//! starts. Each step reads what is recorded first, so a reopened conversation
//! continues where setup stopped and never launches a second Graph.

use std::time::Instant;

use anyhow::{Context, Result};
use tracing::Instrument;

use super::primary;
use crate::session::LfSession;
use crate::store::SharedStore;

pub(super) async fn finish(
    store: &SharedStore,
    session: LfSession,
    request_id: &str,
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
    let session = step(request_id, "checkout", checkout(store, session))
        .await
        .inspect_err(|error| println!("Checkout failed: {error:#}"))?;
    println!("Checkout ready  {:.1} s", started.elapsed().as_secs_f64());
    // The agent starts once the checkout exists, whatever happens to the Graph.
    match step(
        request_id,
        "graph",
        start_graph(store, &session, request_id, agent),
    )
    .await
    {
        Ok(graph) => println!("Graph {graph} started"),
        Err(error) => {
            tracing::warn!(error = %format!("{error:#}"), "new Task's Graph did not start");
            println!("Graph did not start: {error:#}");
        }
    }
    Ok(session)
}

/// One setup step, timed under its creation's request ID so a Start's report
/// nests it beside the server's creation steps. Never records request text.
async fn step<T>(
    request_id: &str,
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
async fn checkout(store: &SharedStore, session: LfSession) -> Result<LfSession> {
    let task = session
        .task_id
        .as_ref()
        .context("the conversation has no Task")?;
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
    primary::admit_workspace(store, session).await
}

/// Start the Task's run the way a terminal would, detached, and record the
/// launch on the creation so neither a repeat nor a reopened conversation
/// starts another.
async fn start_graph(
    store: &SharedStore,
    session: &LfSession,
    request_id: &str,
    agent: Option<&str>,
) -> Result<String> {
    let task = session
        .task_id
        .as_ref()
        .context("the conversation has no Task")?;
    let task = store
        .get_task(task)
        .await?
        .context("the Task is unavailable")?;
    let row = crate::store::task_work::task_graph_row(&store.sqlite, &task.id)?
        .context("the Task has no Graph")?;
    let launch = match &row.run {
        Some((_, process)) => serde_json::json!({ "lf_process_id": process.id }),
        None => {
            let mut argv = vec![crate::os_process::execution_context()?
                .lf_bin
                .to_string_lossy()
                .into_owned()];
            // The agent this conversation was started with runs its Graph too.
            if let Some(agent) = agent {
                argv.extend(["--agent".into(), agent.to_owned()]);
            }
            argv.extend(["task".into(), "run".into(), task.identifier.clone()]);
            let process = crate::os_process::detached::start_lf(&session.cwd, &argv, &[], &|_| {})?;
            serde_json::json!({ "lf_process_id": process })
        }
    };
    store.sqlite.record_setup_launch(request_id, &launch)?;
    Ok(row.graph.name)
}
