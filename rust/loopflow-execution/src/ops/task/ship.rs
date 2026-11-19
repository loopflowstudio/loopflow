//! A Task's auto setting, and `lf ship`: auto on, then the run `lf task run`
//! starts. Auto is one mark on `task_graphs`; the run reads it at each
//! interactive step, standing in for the person while it is on, and at each
//! step that merges, waiting there while it is off. `lf task auto off` turns
//! it off; `lf ship`, spelled `lf task auto on` too, turns it on again.

use crate::durable::{GraphProcessFilter, TaskState};
use crate::engine::task::TaskRun;
use crate::ops::error::OpsResult;
use crate::session::GraphProcessSummaryState;
use crate::task_graph::{TaskGraphPosition, END};
use loopflow_model::id::LfProcessId;
use loopflow_storage::sqlite::{Auto, AutoState};
use loopflow_storage::work::task::Task;

use super::{block_on_task, task_error, task_store, SharedStore, TaskRunSelection};

/// What `lf ship` found before running anything.
#[derive(Debug)]
pub enum ShipStart {
    /// Auto is on; run the Task, standing in from `from` when it waits
    /// at that interactive step.
    Run { issue: String, from: Option<String> },
    /// Nothing new runs: the Task stands at `end`, or its live run carries
    /// on and stands in from its next interactive step.
    Reported(TaskRun),
}

async fn task_by_issue(store: &SharedStore, issue: &str) -> OpsResult<Task> {
    store
        .get_task_by_issue(issue)
        .await
        .map_err(task_error)?
        .ok_or_else(|| task_error(format!("no Task exists for {issue:?}")))
}

/// Who started a run: the Session whose agent started it, with the person
/// named as in `repo`.
pub fn starter(
    sqlite: &loopflow_storage::sqlite::SqliteStore,
    repo: &std::path::Path,
    session: Option<&loopflow_model::id::LfSessionId>,
) -> crate::store::StoreResult<crate::process::Starter> {
    let person = crate::config::participant_name_in(repo).ok().flatten();
    sqlite.starter(session, person.as_deref())
}

/// The Task auto is turned on for: open, and launchable here.
async fn task_for_auto(store: &SharedStore, issue: &str) -> OpsResult<Task> {
    let task = task_by_issue(store, issue).await?;
    let issue = &task.identifier;
    match store.sqlite.task_state(&task.id).map_err(task_error)? {
        TaskState::Done => return Err(task_error(format!("Task {issue} is complete"))),
        TaskState::Abandoned => {
            return Err(task_error(format!(
                "Task {issue} is abandoned; recover it before turning auto on"
            )))
        }
        _ => {}
    }
    store
        .sqlite
        .require_task_launch(&task.id)
        .map_err(task_error)?;
    Ok(task)
}

/// The Graph `lf ship` runs: a Task with none takes up its default; naming
/// `graph` takes that one up in place of the remaining steps. Only a caller
/// that starts the run next takes one up: a Task that took up a Graph and
/// never ran it reads as starting, and its conversation starts the run.
async fn graph_for_ship(
    store: &SharedStore,
    task: &Task,
    graph: Option<&str>,
) -> OpsResult<crate::task_graph::TaskGraph> {
    let current =
        crate::store::task_work::task_graph(&store.sqlite, &task.id).map_err(task_error)?;
    if current.is_none() || graph.is_some() {
        let TaskRunSelection {
            graph: selected,
            take_up,
            ..
        } = super::select_task_run(store, task, task.worktree()?, graph, Some("start"), |_| {
            false
        })?;
        if take_up {
            crate::store::task_work::take_up_task_graph(&store.sqlite, &task.id, &selected)
                .map_err(task_error)?;
        }
    }
    crate::store::task_work::task_graph(&store.sqlite, &task.id)
        .map_err(task_error)?
        .ok_or_else(|| task_error(format!("Task {} has no graph", task.identifier)))
}

/// Save `state` as this caller's mark, naming who is asking as it is written;
/// with `keeps_stop`, a standing stop stays. Returns whether auto was on before.
fn set(
    store: &SharedStore,
    task: &Task,
    state: AutoState,
    keeps_stop: bool,
    note: Option<&str>,
) -> OpsResult<bool> {
    let repo = super::repository_for_task(store, task)?;
    let mark = Auto {
        state,
        at: loopflow_storage::rows::now_unix(),
        by: starter(
            &store.sqlite,
            &repo,
            crate::journal::caller_session_id().as_ref(),
        )
        .map_err(task_error)?,
    };
    let before = store
        .sqlite
        .set_task_auto(&task.id, &mark, keeps_stop, note)
        .map_err(task_error)?;
    Ok(before.is_some_and(|before| before.state == AutoState::On))
}

/// `lf ship`: auto on, then what `lf task run` would start. An unowned Task
/// is claimed, as `lf task run` would.
pub fn ship_on(issue: &str, graph: Option<&str>, note: Option<&str>) -> OpsResult<ShipStart> {
    block_on_task(async {
        let store = task_store().await?;
        let task = task_for_auto(&store, issue).await?;
        let graph = graph_for_ship(&store, &task, graph).await?;
        let issue = task.identifier.clone();
        if matches!(&graph.position, TaskGraphPosition::At { step } if step == END) {
            return Ok(ShipStart::Reported(TaskRun::Ended { issue }));
        }
        if !store.sqlite.task_owned(&task.id).map_err(task_error)? {
            crate::store::ownership::claim_task(&store.sqlite, &task.id).map_err(task_error)?;
        }
        set(&store, &task, AutoState::On, false, note)?;
        let from = match &graph.position {
            TaskGraphPosition::At { .. } => graph.waiting_step().map(|step| step.name.clone()),
            TaskGraphPosition::Run { at, .. } => {
                // A live run reads auto at each interactive step, so it stands
                // in from here without being started again.
                if let Some(graph_process) = live_graph_run(&store, &task).await? {
                    return Ok(ShipStart::Reported(TaskRun::Running {
                        log: Some(crate::process::launch_log::graph_log_path(&graph_process)),
                        issue,
                        run: format!("{} at {at}", graph.name),
                        merges: true,
                        graph_process: LfProcessId::parse(&graph_process).map_err(task_error)?,
                    }));
                }
                None
            }
        };
        Ok(ShipStart::Run { issue, from })
    })
}

/// The Task's Graph process that is running now, if one is.
async fn live_graph_run(store: &SharedStore, task: &Task) -> OpsResult<Option<String>> {
    let filter = GraphProcessFilter {
        task_id: Some(task.id.clone()),
        state: Some(GraphProcessSummaryState::Current),
        ..GraphProcessFilter::default()
    };
    let page = store
        .graph_inventory(&filter, None, std::num::NonZeroU32::MIN)
        .await
        .map_err(task_error)?;
    Ok(page
        .entries
        .into_iter()
        .next()
        .map(|entry| entry.summary.id))
}

/// `lf -b task run`: auto on unless someone turned it off.
pub(super) fn batch_on(store: &SharedStore, task: &Task) -> OpsResult<()> {
    set(store, task, AutoState::On, true, None).map(drop)
}

/// `lf task auto off`: turn auto off and take back any merge request
/// Loopflow armed on GitHub. Nothing is killed: the run carries on to its next
/// interactive step or merging step and waits there, until auto is on again.
/// Returns whether auto was on.
pub fn auto_off(issue: &str, note: Option<&str>) -> OpsResult<bool> {
    let (task, was_on) = block_on_task(async {
        let store = task_store().await?;
        let task = task_by_issue(&store, issue).await?;
        let was_on = set(&store, &task, AutoState::Stopped, false, note)?;
        Ok((task, was_on))
    })?;
    if was_on {
        if let Ok(worktree) = task.worktree() {
            super::clear_task_pr_merge_before_head_mutation(worktree, true, &|_| {})?;
        }
    }
    Ok(was_on)
}

/// Create a Task for the checkout's branch so it can ship: it takes the branch,
/// its checkout and any PR as its own, and is titled from the branch.
pub fn ship_branch_task(repo: &std::path::Path, wave: Option<&str>) -> OpsResult<Task> {
    let branch = crate::git::current_branch(repo)
        .map_err(task_error)?
        .ok_or_else(|| task_error("lf ship needs a branch; this checkout is detached"))?;
    if crate::git::get_default_branch(repo).map_err(task_error)? == branch {
        return Err(task_error(format!(
            "{branch} is the default branch; ship from a Task or a branch of its own"
        )));
    }
    let title = branch
        .rsplit('/')
        .next()
        .unwrap_or(&branch)
        .replace(['-', '_'], " ");
    let task = super::task_create(repo, wave, Some(title), None, Default::default(), None)?;
    super::prepare_task(
        repo,
        &task.identifier,
        super::TaskProcessOptions {
            wave: wave.map(str::to_owned),
            branch: Some(branch),
            ..Default::default()
        },
        None,
        super::remote::source_for_task(&task.identifier)?,
    )
}
