//! A Task's auto setting, and `lf ship`: auto on, then the run `lf task run`
//! starts. Auto is one mark on `task_graphs`; the run reads it at each
//! interactive step, standing in for the person while it is on, and at each
//! step that merges, waiting there while it is off. `lf task auto off` turns
//! it off; `lf task auto on` and `lf ship` turn it on again.

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
    Run {
        task: Box<Task>,
        from: Option<String>,
    },
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

/// The mark this caller sets now: who is asking, resolved as it is written.
async fn mark(store: &SharedStore, task: &Task, state: AutoState) -> OpsResult<Auto> {
    let repo = super::repository_for_task(store, task)?;
    Ok(Auto {
        state,
        at: loopflow_storage::rows::now_unix(),
        by: starter(
            &store.sqlite,
            &repo,
            crate::journal::caller_session_id().as_ref(),
        )
        .map_err(task_error)?,
    })
}

/// The open Task and its Graph, ready for auto: a Task with no graph takes up
/// its default first; naming `graph` takes that one up in place of the
/// remaining steps.
async fn graph_for_auto(
    store: &SharedStore,
    issue: &str,
    graph: Option<&str>,
) -> OpsResult<(Task, crate::task_graph::TaskGraph)> {
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
    let current =
        crate::store::task_work::task_graph(&store.sqlite, &task.id).map_err(task_error)?;
    if current.is_none() || graph.is_some() {
        let TaskRunSelection {
            graph: selected,
            take_up,
            ..
        } = super::select_task_run(store, &task, task.worktree()?, graph, Some("start"), |_| {
            false
        })?;
        if take_up {
            crate::store::task_work::take_up_task_graph(&store.sqlite, &task.id, &selected)
                .map_err(task_error)?;
        }
    }
    let graph = crate::store::task_work::task_graph(&store.sqlite, &task.id)
        .map_err(task_error)?
        .ok_or_else(|| task_error(format!("Task {issue} has no graph")))?;
    Ok((task, graph))
}

/// Save `state` as this caller's mark. Returns whether auto was on before.
async fn set(
    store: &SharedStore,
    task: &Task,
    state: AutoState,
    note: Option<&str>,
) -> OpsResult<bool> {
    let mark = mark(store, task, state).await?;
    let before = store
        .sqlite
        .set_task_auto(&task.id, &mark, false, note)
        .map_err(task_error)?;
    Ok(before.is_some_and(|before| before.state == AutoState::On))
}

/// `lf task auto on`: turn auto on for the Task's Graph, lifting any stop.
/// Nothing starts; a live run stands in from its next interactive step.
/// Returns whether auto was on already.
pub fn auto_on(issue: &str, note: Option<&str>) -> OpsResult<bool> {
    block_on_task(async {
        let store = task_store().await?;
        let (task, _) = graph_for_auto(&store, issue, None).await?;
        set(&store, &task, AutoState::On, note).await
    })
}

/// `lf ship`: auto on, then what `lf task run` would start. An unowned Task
/// is claimed, as `lf task run` would.
pub fn ship_on(issue: &str, graph: Option<&str>, note: Option<&str>) -> OpsResult<ShipStart> {
    block_on_task(async {
        let store = task_store().await?;
        let (task, graph) = graph_for_auto(&store, issue, graph).await?;
        let issue = task.identifier.clone();
        let (from, live) = match &graph.position {
            TaskGraphPosition::At { step } if step == END => {
                return Ok(ShipStart::Reported(TaskRun::Ended { issue }));
            }
            TaskGraphPosition::At { .. } => {
                (graph.waiting_step().map(|step| step.name.clone()), None)
            }
            TaskGraphPosition::Run { .. } => (None, live_graph_run(&store, &task).await?),
        };
        if !store.sqlite.task_owned(&task.id).map_err(task_error)? {
            crate::store::ownership::claim_task(&store.sqlite, &task.id).map_err(task_error)?;
        }
        set(&store, &task, AutoState::On, note).await?;
        if let (Some(graph_process), TaskGraphPosition::Run { at, .. }) = (live, &graph.position) {
            // The run reads auto at each interactive step, so it stands in
            // from here without being started again.
            return Ok(ShipStart::Reported(TaskRun::Running {
                log: Some(crate::process::launch_log::graph_log_path(&graph_process)),
                issue,
                run: format!("{} at {at}", graph.name),
                ships: true,
                graph_process: LfProcessId::parse(&graph_process).map_err(task_error)?,
            }));
        }
        Ok(ShipStart::Run {
            task: Box::new(task),
            from,
        })
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
pub(super) async fn batch_on(store: &SharedStore, task: &Task) -> OpsResult<()> {
    let on = mark(store, task, AutoState::On).await?;
    store
        .sqlite
        .set_task_auto(&task.id, &on, true, None)
        .map_err(task_error)?;
    Ok(())
}

/// `lf task auto off`: turn auto off and take back any merge request
/// Loopflow armed on GitHub. Nothing is killed: the run carries on to its next
/// interactive step or merging step and waits there, until auto is on again.
/// Returns whether auto was on.
pub fn auto_off(issue: &str, note: Option<&str>) -> OpsResult<bool> {
    let (task, was_on) = block_on_task(async {
        let store = task_store().await?;
        let task = task_by_issue(&store, issue).await?;
        let was_on = set(&store, &task, AutoState::Stopped, note).await?;
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

/// Start `lf <args>` again in `worktree`, outliving this process, under `agent`
/// when the run had one. Recovery runs inside any read, so the reader's own
/// agent choice and settings are cleared rather than inherited.
pub(super) fn restart_lf_in(
    worktree: &std::path::Path,
    agent: Option<&str>,
    args: &[&str],
) -> OpsResult<()> {
    let context = crate::os_process::execution_context().map_err(task_error)?;
    let mut argv = vec![context.lf_bin.to_string_lossy().to_string()];
    if let Some(agent) = agent {
        argv.extend(["--agent".to_string(), agent.to_string()]);
    }
    argv.extend(args.iter().map(ToString::to_string));
    let env = [
        (crate::config::AGENT_OVERRIDE_ENV, ""),
        ("LF_SETTINGS", "{}"),
    ];
    crate::os_process::detached::start_lf(worktree, &argv, &env, &|_| {}).map_err(task_error)?;
    Ok(())
}

/// How many times in a row a run whose carrier died is carried again before
/// it waits for a person's `lf task run`.
pub const RUN_RESTARTS: u32 = 5;

/// A Task run whose carrier (`lf task run`) died before the run stopped: it
/// was killed, or ended with no exit record, rather than exiting on its own.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DiedRun {
    /// The step the run reached.
    pub at: String,
    /// The carrier that died, which the next carrier takes the run over from.
    pub carrier: LfProcessId,
    /// Starts since the run last finished a step, and when the last one was.
    pub restarts: loopflow_storage::sqlite::task_work::TaskRunRestarts,
}

impl DiedRun {
    /// When the next start is due (0: at once), or `None` once
    /// [`RUN_RESTARTS`] ran out.
    pub fn next_at(&self) -> Option<i64> {
        (self.restarts.count < RUN_RESTARTS).then(|| {
            crate::ops::recovery::next_start_at(self.restarts.count, self.restarts.at).unwrap_or(0)
        })
    }

    /// One line for the Task's run.
    pub fn reason(&self, issue: &str, now: i64) -> String {
        let at = &self.at;
        match self.next_at() {
            None => format!(
                "Run died {} times at {at}; `lf task run {issue}` starts it again",
                self.restarts.count
            ),
            Some(due) if due <= now => {
                format!("Run died at {at}; the server's next pass carries it again")
            }
            Some(due) => format!(
                "Run died at {at}; it is carried again in {} min (restart {} of {RUN_RESTARTS})",
                (due - now + 59) / 60,
                self.restarts.count + 1
            ),
        }
    }
}

/// The Task's run, when its carrier died and nothing of the run still runs.
pub fn died_run(
    sqlite: &loopflow_storage::sqlite::SqliteStore,
    task: &loopflow_storage::work::task::TaskId,
) -> crate::store::StoreResult<Option<DiedRun>> {
    let Some(row) = crate::store::task_work::task_graph_row(sqlite, task)? else {
        return Ok(None);
    };
    let Some((cursor, carrier)) = row.run else {
        return Ok(None);
    };
    // A carrier that exited on its own (failed, interrupted, waiting on a
    // limit) cleared its OS process; one that died left it behind.
    let died = carrier.pid.is_some() && !crate::store::task_work::carrier_may_run(&carrier);
    if !died || crate::ops::task_execution::open_graph_execution(sqlite, task)?.is_some() {
        return Ok(None);
    }
    Ok(Some(DiedRun {
        at: row.graph.step_name_of(&cursor).unwrap_or(row.step),
        carrier: carrier.id,
        restarts: sqlite.task_run_restarts(task)?,
    }))
}

/// Carry again each run on this Machine whose `lf task run` died, as
/// `lf task run ISSUE`: it stands in at interactive steps when auto is
/// on and stops at the next one otherwise. Unassigned, moved and finished
/// Tasks wait. Returns when the soonest run still waiting its backoff comes due.
pub(crate) fn restart_runs() -> OpsResult<Option<i64>> {
    block_on_task(async {
        let store = task_store().await?;
        let now = loopflow_storage::rows::now_unix();
        let mut soonest: Option<i64> = None;
        for id in store.sqlite.local_task_runs().map_err(task_error)? {
            let Some(died) = died_run(&store.sqlite, &id).map_err(task_error)? else {
                continue;
            };
            let Some(due) = died.next_at() else {
                continue;
            };
            if due > now {
                soonest = Some(soonest.map_or(due, |at: i64| at.min(due)));
                continue;
            }
            let Some((task, worktree)) = continuable(&store, &id, now).await? else {
                continue;
            };
            if !store
                .sqlite
                .count_task_run_restart(&id, &died.carrier, &died.restarts)
                .map_err(task_error)?
            {
                continue;
            }
            restart_lf_in(&worktree, None, &["task", "run", &task.identifier])?;
            tracing::info!(
                "carried {}'s run again at {} after its carrier died",
                task.identifier,
                died.at
            );
        }
        Ok(soonest)
    })
}

/// The Task and its checkout, when a run of it may start here now: open,
/// claimed here, checked out, and not waiting out a limit.
async fn continuable(
    store: &SharedStore,
    id: &loopflow_storage::work::task::TaskId,
    now: i64,
) -> OpsResult<Option<(Task, std::path::PathBuf)>> {
    if store
        .sqlite
        .task_state(id)
        .map_err(task_error)?
        .is_terminal()
        || !store.sqlite.task_claimed_here(id).map_err(task_error)?
    {
        return Ok(None);
    }
    let wait = super::limit_recovery::waiting_on_limit(&store.sqlite, id).map_err(task_error)?;
    if wait.as_ref().is_some_and(|wait| wait.retry_at > now) {
        return Ok(None);
    }
    let Some(task) = store.get_task(id).await.map_err(task_error)? else {
        return Ok(None);
    };
    let Some(worktree) = task.worktree.clone().filter(|path| path.is_dir()) else {
        return Ok(None);
    };
    Ok(Some((task, worktree)))
}

#[cfg(test)]
mod tests {
    use loopflow_model::id::LfProcessId;

    #[test]
    fn a_died_run_is_carried_again_at_once_then_later_then_waits_for_a_person() {
        use loopflow_storage::sqlite::task_work::TaskRunRestarts;
        let died = |count, at| super::DiedRun {
            at: "implement".into(),
            carrier: LfProcessId::new(),
            restarts: TaskRunRestarts { count, at },
        };
        assert_eq!(died(0, None).next_at(), Some(0));
        assert_eq!(
            died(0, None).reason("LOO-1", 100),
            "Run died at implement; the server's next pass carries it again"
        );
        assert_eq!(died(3, Some(100)).next_at(), Some(100 + 2 * 60));
        assert_eq!(
            died(3, Some(100)).reason("LOO-1", 100),
            "Run died at implement; it is carried again in 2 min (restart 4 of 5)"
        );
        assert_eq!(died(5, Some(100)).next_at(), None);
        assert_eq!(
            died(5, Some(100)).reason("LOO-1", 100),
            "Run died 5 times at implement; `lf task run LOO-1` starts it again"
        );
    }
}
