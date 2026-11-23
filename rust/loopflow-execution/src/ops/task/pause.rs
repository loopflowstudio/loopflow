//! Hold execution on its owning Machine while keeping the checkout and cursor.
use std::fs::File;
use std::time::{Duration, Instant};

use fs2::FileExt;
use loopflow_model::durable::TaskId;
use loopflow_storage::process::Departure;
use loopflow_storage::session::PrimaryScope;
use loopflow_storage::sqlite::SqliteStore;
use loopflow_storage::work::task::Task;

use super::{auto, block_on_task, retirement, task_error, task_store, SharedStore};
use crate::harness::agent_process;
use crate::ops::OpsResult;

pub(crate) fn admission(task: &TaskId) -> OpsResult<File> {
    lock(&format!("admit-{task}"))
}

fn lock(name: &str) -> OpsResult<File> {
    let file = retirement::lock_file(name)?;
    file.lock_exclusive()?;
    Ok(file)
}

async fn resolve(issue: &str) -> OpsResult<(SharedStore, Task)> {
    let store = task_store().await?;
    let task = store
        .get_task_by_issue(issue)
        .await
        .map_err(task_error)?
        .ok_or_else(|| task_error(format!("no Task exists for {issue:?}")))?;
    let route = store
        .task_execution_route(&task.id)
        .await
        .map_err(task_error)?;
    if route.machine_id != store.local_machine().await.map_err(task_error)?.id {
        return Err(task_error(format!(
            "Task {issue} runs on Machine {}",
            route.machine_id
        )));
    }
    Ok((store, task))
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
        let (store, task) = resolve(issue).await?;
        let sqlite = &store.sqlite;
        // Serialize cleanup and unpause independently of run admission. Runs
        // see the hold as soon as the interrupt commits, even during a stop.
        let _cleanup = lock(&format!("pause-{}", task.id))?;
        let sessions = {
            let _admission = admission(&task.id)?;
            let by = auto::caller(&store, &task)?;
            let jobs = agent_process::capture_worker_jobs(sqlite, &task.id).map_err(task_error)?;
            sqlite
                .begin_task_pause(&task.id, &by, jobs)
                .map_err(task_error)?
        };
        let stop_primary = || -> anyhow::Result<()> {
            if let Some(primary) = sqlite.primary_session(&PrimaryScope::Task(task.id.clone()))? {
                agent_process::stop_session_agent(
                    sqlite,
                    primary.id.as_str(),
                    Departure::Paused {
                        task: task.id.clone(),
                    },
                )?;
            }
            Ok(())
        };
        let mut failures: Vec<_> = stop_primary()
            .err()
            .into_iter()
            .map(|error| format!("conversation: {error}"))
            .collect();
        // Give the Graph a chance to record the interrupted turn and close
        // its own provider. A retry still owns the saved jobs after a crash.
        let deadline = Instant::now() + Duration::from_secs(2);
        while Instant::now() < deadline && draining(sqlite, &task.id)?.is_some() {
            std::thread::sleep(Duration::from_millis(50));
        }
        for session in &sessions {
            if let Err(error) = crate::os_process::settle_worker_jobs(session) {
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
        let finishing = draining(sqlite, &task.id)?;
        sqlite
            .settle_task_pause(&task.id, finishing.is_some())
            .map_err(task_error)?;
        Ok(match finishing {
            Some(run) => format!("{} pausing ({run} is finishing)", task.identifier),
            None => format!("{} paused", task.identifier),
        })
    })
}

pub fn unpause(issue: &str) -> OpsResult<()> {
    block_on_task(async {
        let (store, task) = resolve(issue).await?;
        let _cleanup = lock(&format!("pause-{}", task.id))?;
        let _admission = admission(&task.id)?;
        if let Some(run) = draining(&store.sqlite, &task.id)? {
            return Err(task_error(format!(
                "{} is still finishing {run}",
                task.identifier
            )));
        }
        store
            .sqlite
            .release_task_pause(&task.id)
            .map_err(task_error)
    })
}
