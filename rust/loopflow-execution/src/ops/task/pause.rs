//! Hold execution on its owning Machine while keeping the checkout and cursor.
use std::fs::File;
use std::time::{Duration, Instant};

use fs2::FileExt;
use loopflow_model::durable::TaskId;
use loopflow_storage::sqlite::{task_pause::StoppingSession, SqliteStore};
use loopflow_storage::work::task::Task;

use super::{auto, block_on_task, retirement, task_error, task_store, SharedStore};
use crate::engine::Error;
use crate::harness::agent_process;
use crate::ops::{OpsError, OpsResult};
use crate::store::ownership::{task_execution_route, task_pause_route, TaskRoute};
use crate::store::StoreError;

pub(super) fn admission(task: &TaskId) -> OpsResult<File> {
    lock(retirement::lock_file(&format!("admit-{task}"))?)
}

/// Hold through placement or provider registration so pause sees the admitted work.
pub(crate) fn admit_launch(store: &SqliteStore, task: &TaskId) -> OpsResult<File> {
    let lock = admission(task)?;
    store.require_task_unpaused(task).map_err(refused)?;
    Ok(lock)
}

/// The hold's own refusals (paused, not paused, closed) change nothing, so
/// they carry `conflict` rather than an unknown outcome.
fn refused(error: StoreError) -> OpsError {
    match error {
        StoreError::InvalidData(message) => Error::conflict(message).into(),
        error => task_error(error),
    }
}

fn lock(file: File) -> OpsResult<File> {
    file.lock_exclusive()?;
    Ok(file)
}

fn cleanup_lock_file(task: &TaskId) -> OpsResult<File> {
    retirement::lock_file(&format!("pause-{task}"))
}

pub(crate) fn cleanup_lock(task: &TaskId) -> OpsResult<File> {
    lock(cleanup_lock_file(task)?)
}

async fn resolve(issue: &str, route: TaskRoute) -> OpsResult<(SharedStore, Task, File)> {
    let store = task_store().await?;
    let task = store
        .get_task_by_issue(issue)
        .await
        .map_err(task_error)?
        .ok_or_else(|| crate::ops::OpsError::TaskNotFound(issue.to_owned()))?;
    let cleanup = cleanup_lock(&task.id)?;
    let route = route(&store.sqlite, &task.id).map_err(task_error)?;
    if route.machine_id != store.local_machine().await.map_err(task_error)?.id {
        return Err(
            Error::conflict(format!("Task {issue} runs on Machine {}", route.machine_id)).into(),
        );
    }
    Ok((store, task, cleanup))
}

fn draining(store: &SqliteStore, task: &TaskId) -> OpsResult<Option<String>> {
    for work in store.task_open_work(task).map_err(task_error)? {
        if let loopflow_storage::sqlite::OpenWork::Graph { run, .. } = work {
            if crate::process::RecordedProcess::from(&run).evidence()
                != crate::store::ProcessIdentityEvidence::Dead
            {
                return Ok(Some(format!("Graph run {}", run.id)));
            }
        }
    }
    if let Some((_, carrier)) = store
        .task_graph_row(task)
        .map_err(task_error)?
        .and_then(|r| r.run)
    {
        if crate::store::task_work::carrier_may_run(&carrier) {
            return Ok(Some(format!("Task run {}", carrier.id)));
        }
    }
    Ok(None)
}

pub fn pause(issue: &str) -> OpsResult<String> {
    block_on_task(async {
        let (store, task, _cleanup) = resolve(issue, task_pause_route).await?;
        let sqlite = &store.sqlite;
        // Serialize cleanup and unpause independently of run admission. Runs
        // see the hold as soon as the interrupt commits, even during a stop.
        let sessions = {
            let _admission = admission(&task.id)?;
            let by = auto::caller(&store, &task)?;
            let sessions =
                agent_process::capture_worker_sessions(sqlite, &task.id).map_err(task_error)?;
            sqlite
                .begin_task_pause(&task.id, &by, sessions)
                .map_err(refused)?
        };
        let finishing = finish(sqlite, &task, &sessions)?;
        Ok(match finishing {
            Some(run) => format!("{} pausing ({run} is finishing)", task.identifier),
            None => format!("{} paused", task.identifier),
        })
    })
}

fn finish(
    sqlite: &SqliteStore,
    task: &Task,
    sessions: &[StoppingSession],
) -> OpsResult<Option<String>> {
    let mut failures = Vec::new();
    // Give the Graph a chance to record the interrupted turn and close
    // its own provider. A retry still owns the saved jobs after a crash.
    let deadline = Instant::now() + Duration::from_secs(2);
    while Instant::now() < deadline && draining(sqlite, &task.id)?.is_some() {
        std::thread::sleep(Duration::from_millis(50));
    }
    for session in sessions {
        if let Err(error) = crate::os_process::settle_worker_session(session) {
            failures.push(format!("worker OS session {}: {error}", session.session));
        }
    }
    if !failures.is_empty() {
        return Err(task_error(format!(
            "{} is still stopping: {}. Repeat `lf task pause {}` to finish cleanup.",
            task.identifier,
            failures.join(", "),
            task.identifier
        )));
    }
    settle(sqlite, task)
}

fn settle(sqlite: &SqliteStore, task: &Task) -> OpsResult<Option<String>> {
    let finishing = draining(sqlite, &task.id)?;
    sqlite
        .settle_task_pause(&task.id, finishing.is_some())
        .map_err(task_error)?;
    Ok(finishing)
}

/// A caller may have died after the jobs exited but before marking the hold settled.
fn finish_if_jobs_exited(store: &SqliteStore, task: &Task) -> OpsResult<()> {
    let Some(sessions) = store.stopping_sessions(&task.id).map_err(task_error)? else {
        return Ok(());
    };
    for session in &sessions {
        if let Some(live) = crate::os_process::remaining_session_members(session)
            .map_err(task_error)?
            .first()
        {
            return Err(Error::conflict(format!(
                "{} is still stopping process {}; repeat `lf task pause {}` to finish cleanup",
                task.identifier, live.pid, task.identifier
            ))
            .into());
        }
    }
    settle(store, task)?;
    Ok(())
}

pub(crate) fn before_unassign(store: &SqliteStore, task: &Task) -> OpsResult<()> {
    if store
        .stopping_sessions(&task.id)
        .map_err(task_error)?
        .is_some()
    {
        let lock = cleanup_lock_file(&task.id)?;
        lock.try_lock_exclusive().map_err(|err| -> OpsError {
            if err.kind() == std::io::ErrorKind::WouldBlock {
                Error::conflict(format!("{} is still stopping", task.identifier)).into()
            } else {
                err.into()
            }
        })?;
        finish_if_jobs_exited(store, task)?;
    }
    Ok(())
}

pub fn unpause(issue: &str) -> OpsResult<()> {
    release(issue, false).map(drop)
}

/// Keep admission through placement when the detached unpause carries on as the run.
pub(super) fn release(issue: &str, resume: bool) -> OpsResult<Option<(TaskId, File)>> {
    block_on_task(async {
        let (store, task, _cleanup) = resolve(issue, task_execution_route).await?;
        finish_if_jobs_exited(&store.sqlite, &task)?;
        let admission = admission(&task.id)?;
        if let Some(run) = draining(&store.sqlite, &task.id)? {
            return Err(
                Error::conflict(format!("{} is still finishing {run}", task.identifier)).into(),
            );
        }
        let start = resume
            && crate::store::task_work::task_graph_row(&store.sqlite, &task.id)
                .map_err(task_error)?
                .is_none_or(|row| {
                    row.graph.resumes_on_unpause(
                        &row.step,
                        row.run.as_ref().map(|(cursor, _)| cursor),
                        row.auto,
                    )
                });
        store.sqlite.release_task_pause(&task.id).map_err(refused)?;
        Ok(start.then_some((task.id, admission)))
    })
}
