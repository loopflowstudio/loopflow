//! Carrying a Task run again after its carrier (`lf task run`) died. Every
//! Task run is recovered here, whatever its auto setting.

use crate::ops::error::OpsResult;
use loopflow_model::id::LfProcessId;
use loopflow_storage::work::task::Task;

use super::{block_on_task, task_error, task_store, SharedStore};

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
    // limit) cleared its OS process; one that died left it behind, until
    // retention cleared it and recorded the death.
    let died = carrier.died_at.is_some()
        || (carrier.pid.is_some() && !crate::store::task_work::carrier_may_run(&carrier));
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
                .count_task_run_restart(&id, &died.carrier, died.restarts.count)
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
