//! Carrying a Task run again after its carrier (`lf task run`) died. Every
//! Task run is recovered here, whatever its auto setting.

use crate::ops::error::OpsResult;
use crate::ops::human_session::{clock, left_reason, SessionOutlook, SessionOutlookKind};
use crate::ops::recovery::{server_start, DownConversation, ServerStart};
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

/// A Task run whose carrier (`lf task run`) died before the run stopped: it
/// was killed, or ended with no exit record, rather than exiting on its own.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DiedRun {
    /// The step the run reached.
    pub at_step: String,
    /// When the carrier died; `None` until retention records a death nobody saw.
    pub at: Option<i64>,
    /// The carrier that died, which the next carrier takes the run over from.
    pub carrier: LfProcessId,
    /// Starts since the run last finished a step, and when the last one was.
    pub restarts: loopflow_storage::sqlite::task_work::TaskRunRestarts,
}

impl DiedRun {
    /// Whether the server carries the run again, by the rule and schedule
    /// that start a Task's conversation ([`server_start`]). `here`: the Task
    /// is checked out on this Machine.
    pub fn server_start(
        &self,
        here: bool,
        claimed_here: impl FnOnce() -> bool,
        server_runs: impl FnOnce() -> bool,
    ) -> ServerStart {
        let facts = DownConversation {
            here,
            exit_code: None,
            starts_since_turn: self.restarts.count,
            last_started_at: self.restarts.at,
        };
        server_start(&facts, claimed_here, server_runs)
    }

    /// What happens next to the run, and the sentence that says it in this
    /// Machine's local time: "Run died 12:01 at implement · starting again".
    pub fn outlook(&self, issue: &str, now: i64, start: ServerStart) -> (SessionOutlook, String) {
        let died = match self.at {
            Some(at) => format!("Run died {} at {}", clock(at), self.at_step),
            None => format!("Run died at {}", self.at_step),
        };
        match start {
            ServerStart::At(next) => {
                // Nothing started it since it last finished a step: it was
                // due when it died.
                let at = next.or(self.at);
                let again = match at {
                    Some(at) if at > now => format!("starts again {}", clock(at)),
                    _ => "starting again".to_string(),
                };
                let outlook = SessionOutlook {
                    kind: SessionOutlookKind::ComingBack,
                    at: at.and_then(loopflow_harness::transcript::rfc3339),
                    reason: None,
                    command: None,
                };
                (outlook, format!("{died} · {again}"))
            }
            ServerStart::Left(left) => {
                let reason = left_reason(left, self.restarts.count, true);
                let command = format!("lf task run {issue}");
                let why = reason
                    .as_deref()
                    .map_or(String::new(), |reason| format!(" · {reason}"));
                let line = format!("{died}{why}; `{command}` starts it again");
                let outlook = SessionOutlook {
                    kind: SessionOutlookKind::WaitingForYou,
                    at: None,
                    reason,
                    command: Some(command),
                };
                (outlook, line)
            }
        }
    }
}

/// The Task's run, when its carrier died and nothing of the run still runs.
pub fn died_run(
    sqlite: &loopflow_storage::sqlite::SqliteStore,
    task: &loopflow_storage::work::task::TaskId,
) -> crate::store::StoreResult<Option<DiedRun>> {
    let read = || crate::store::task_work::task_graph_row(sqlite, task);
    let died = |carrier: &loopflow_storage::graph_process::TaskCarrier| {
        carrier.died_at.is_some()
            || (carrier.pid.is_some() && !crate::store::task_work::carrier_may_run(carrier))
    };
    let Some(row) = read()? else {
        return Ok(None);
    };
    if !row.run.as_ref().is_some_and(|(_, carrier)| died(carrier)) {
        return Ok(None);
    }
    // A carrier clears its OS process as the last thing it does, so one that
    // exited on its own after this read looks dead with its process still
    // recorded. Read what it left before calling that a death.
    let Some(row) = read()? else {
        return Ok(None);
    };
    let Some((cursor, carrier)) = row.run else {
        return Ok(None);
    };
    // A carrier that exited on its own (failed, interrupted, waiting on a
    // limit) cleared its OS process; one that died left it behind, until
    // retention cleared it and recorded the death.
    if !died(&carrier) || crate::ops::task_execution::open_graph_execution(sqlite, task)?.is_some()
    {
        return Ok(None);
    }
    Ok(Some(DiedRun {
        at_step: row.graph.step_name_of(&cursor).unwrap_or(row.step),
        at: carrier.died_at,
        carrier: carrier.id,
        restarts: sqlite.task_run_restarts(task)?,
    }))
}

/// Carry again each run on this Machine whose `lf task run` died, as
/// `lf task run ISSUE`: it passes breakpoints when auto is
/// on and parks at the next one otherwise. Unassigned, moved and finished
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
            // This is the server asking: one runs. `continuable` reads the
            // claim and the checkout once the run is due.
            let ServerStart::At(due) = died.server_start(true, || true, || true) else {
                continue;
            };
            let due = due.unwrap_or(0);
            if due > now {
                soonest = Some(soonest.map_or(due, |at: i64| at.min(due)));
                continue;
            }
            // Pause and recovery share admission through the restart count and spawn.
            let _admission = super::pause::admission(&id)?;
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
                died.at_step
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
    use loopflow_storage::execution::TaskState;
    if !matches!(
        store.sqlite.task_state(id).map_err(task_error)?,
        TaskState::Idle | TaskState::Active | TaskState::Moving
    ) || !store.sqlite.task_claimed_here(id).map_err(task_error)?
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
    use crate::ops::human_session::{clock, SessionOutlookKind};
    use crate::ops::recovery::ServerStart;
    use loopflow_model::id::LfProcessId;
    use loopflow_storage::sqlite::task_work::TaskRunRestarts;

    fn died(count: u32, restarted: Option<i64>, at: Option<i64>) -> super::DiedRun {
        super::DiedRun {
            at_step: "implement".into(),
            at,
            carrier: LfProcessId::new(),
            restarts: TaskRunRestarts {
                count,
                at: restarted,
            },
        }
    }

    fn read(run: &super::DiedRun, now: i64, server: bool) -> (SessionOutlookKind, String) {
        let start = run.server_start(true, || true, || server);
        let (outlook, line) = run.outlook("LOO-1", now, start);
        (outlook.kind, line)
    }

    #[test]
    fn a_died_run_starts_again_at_once_then_later_then_waits_for_a_person() {
        use SessionOutlookKind::{ComingBack, WaitingForYou};
        let first = died(0, None, Some(100));
        assert_eq!(
            first.server_start(true, || true, || true),
            ServerStart::At(None)
        );
        assert_eq!(
            read(&first, 100, true),
            (
                ComingBack,
                format!("Run died {} at implement · starting again", clock(100))
            )
        );
        let later = died(3, Some(100), None);
        assert_eq!(
            later.server_start(true, || true, || true),
            ServerStart::At(Some(100 + 2 * 60))
        );
        assert_eq!(
            read(&later, 100, true),
            (
                ComingBack,
                format!("Run died at implement · starts again {}", clock(220))
            )
        );
        assert_eq!(
            read(&died(5, Some(100), None), 100, true),
            (
                WaitingForYou,
                "Run died at implement · started 5 times; `lf task run LOO-1` starts it again"
                    .to_string()
            )
        );
    }

    #[test]
    fn a_died_run_promises_no_start_this_machine_will_not_make() {
        use SessionOutlookKind::WaitingForYou;
        let run = died(0, None, None);
        assert_eq!(
            read(&run, 100, false),
            (
                WaitingForYou,
                "Run died at implement · no server runs here to start it; `lf task run LOO-1` starts it again"
                    .to_string()
            )
        );
        let unclaimed = run.server_start(true, || false, || true);
        assert_eq!(
            run.outlook("LOO-1", 100, unclaimed).0.reason.as_deref(),
            Some("its Task is not claimed on this Machine")
        );
        let elsewhere = run.server_start(false, || true, || true);
        assert_eq!(
            run.outlook("LOO-1", 100, elsewhere).0.reason.as_deref(),
            Some("its Task is not checked out here")
        );
    }
}
