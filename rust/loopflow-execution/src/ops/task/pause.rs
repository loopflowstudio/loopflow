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

/// Where a Task that `lf task unpause --run` left in place waits, and the
/// command that moves it: past a human step, past a landing stop with auto
/// off, or to completion at its end.
pub fn parked(issue: &str) -> OpsResult<Option<String>> {
    use crate::task_graph::WorkflowAction;
    block_on_task(async {
        let store = task_store().await?;
        let Some(task) = store.get_task_by_issue(issue).await.map_err(task_error)? else {
            return Ok(None);
        };
        let Some(graph) =
            crate::store::task_work::task_graph(&store.sqlite, &task.id).map_err(task_error)?
        else {
            return Ok(None);
        };
        let id = &task.identifier;
        let next = match graph.control.action {
            WorkflowAction::Run => format!("`lf task run {id}` continues past it."),
            WorkflowAction::Wait => format!("`lf task auto on {id}` lands it."),
            WorkflowAction::Done => format!("`lf task complete {id}` finishes it."),
            WorkflowAction::Pause
            | WorkflowAction::Pausing
            | WorkflowAction::Unpause
            | WorkflowAction::Resume => return Ok(None),
        };
        Ok(Some(format!("{} {next}", graph.sentence)))
    })
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
            && crate::store::task_work::resumes_on_unpause(&store.sqlite, &task.id)
                .map_err(task_error)?;
        store.sqlite.release_task_pause(&task.id).map_err(refused)?;
        Ok(start.then_some((task.id, admission)))
    })
}

#[cfg(test)]
mod tests {
    use super::parked;
    use crate::store::{open_ephemeral_store, StorageConfig};
    use crate::task_graph::CapturedGraph;
    use loopflow_storage::process::Starter;
    use loopflow_storage::sqlite::{Auto, AutoState};
    use loopflow_storage::work::task::{
        LinearWritebackState, Observation, Task, TaskId, TaskPr, TaskPrId,
    };

    #[test]
    fn parked_names_the_command_for_human_stops_landing_waits_and_end() {
        let _lock = crate::journal::test_env_lock();
        let _env = crate::test_ambient::EnvGuard::clear(&["LF_HOME"]);
        let home = tempfile::tempdir().unwrap();
        let repo = tempfile::tempdir().unwrap();
        std::env::set_var("LF_HOME", home.path());

        let runtime = tokio::runtime::Runtime::new().unwrap();
        let store = runtime
            .block_on(open_ephemeral_store(&StorageConfig::sqlite(
                home.path().join("loopflow.db"),
            )))
            .unwrap();
        let now = time::OffsetDateTime::now_utc();
        let task = Task {
            parent_task_id: None,
            repository_id: crate::repository::location::ensure_repository(
                &store.sqlite,
                &repo.path().display().to_string(),
            )
            .unwrap(),
            id: TaskId::new(),
            tracker_id: None,
            revision: 0,
            identifier: "INF-42".into(),
            title: "Parked check".into(),
            description: String::new(),
            tracker_synced_at: None,
            tracker_writeback: LinearWritebackState::Current,
            wave_id: None,
            project_id: None,
            worktree: Some(repo.path().to_path_buf()),
            workspace_slug: "parked".into(),
            branch: "main".into(),
            base_commit: "deadbeef".into(),
            parent_pr_id: None,
            created_at: now,
            updated_at: now,
            rank: 0,
            assignee: None,
            due_date: None,
            state: None,
            completed_at: None,
            observation: Observation::NotRequired,
        };
        let pr = TaskPr {
            id: TaskPrId::new(),
            task_id: task.id.clone(),
            sequence: 1,
            slug: "parked".into(),
            branch: "main".into(),
            base_commit: "deadbeef".into(),
            parent_pr_id: None,
            publication: None,
            merge_commit: None,
            abandoned_at: None,
            ci_observation: None,
            github_observation: None,
            linear_attachment_id: None,
            linear_comment_id: None,
            linear_link_error: None,
            created_at: now,
            updated_at: now,
        };
        runtime.block_on(store.seed_task(&task, &pr)).unwrap();

        let graph = CapturedGraph::load("code", repo.path()).unwrap().unwrap();
        let graph_json = serde_json::to_string(&graph).unwrap();
        store
            .sqlite
            .take_up_task_graph(&task.id, &graph_json)
            .unwrap();

        store
            .sqlite
            .set_task_graph_step(&task.id, "ship-review")
            .unwrap();
        assert_eq!(
            parked("INF-42").unwrap().as_deref(),
            Some("Waiting for you at ship-review. `lf task run INF-42` continues past it.")
        );

        let person = Starter::Person {
            name: Some("Maya".into()),
        };
        store
            .sqlite
            .set_task_auto(
                &task.id,
                &Auto {
                    state: AutoState::Off,
                    at: 0,
                    by: person.clone(),
                },
                false,
            )
            .unwrap();
        store
            .sqlite
            .set_task_graph_step(&task.id, "land --wait-and-fix")
            .unwrap();
        assert_eq!(
            parked("INF-42").unwrap().as_deref(),
            Some("Waiting at land --wait-and-fix: auto is off. `lf task auto on INF-42` lands it.")
        );

        store.sqlite.set_task_graph_step(&task.id, "end").unwrap();
        assert_eq!(
            parked("INF-42").unwrap().as_deref(),
            Some("At its end. `lf task complete INF-42` finishes it.")
        );

        // Still paused -> parked returns None.
        store
            .sqlite
            .begin_task_pause(&task.id, &person, Vec::new())
            .unwrap();
        assert_eq!(parked("INF-42").unwrap(), None);
        store.sqlite.settle_task_pause(&task.id, false).unwrap();
        assert_eq!(parked("INF-42").unwrap(), None);
    }
}
