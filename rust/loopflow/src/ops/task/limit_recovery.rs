//! A step waiting out a usage limit does so in its own Process. When that
//! Process ends first (a restart, a reboot), the wait's record says which step
//! the run was waiting at. `lf task run` starts the run again at that step, so
//! the steps before it stay done; the recovery sweep does so by itself once the
//! wait is over, for a Task that `restarts_by_itself`. A restart runs under the
//! agent the waiting step was explicitly given, not the one of whoever's read
//! happened to trigger it.

use crate::durable::TaskId;
use crate::journal::ProcessIdentityEvidence;
use crate::ops::task_execution::task_execution_blockers;
use crate::store::sqlite::{AgentLimitRecovery, SqliteStore};
use crate::store::StoreResult;

use super::super::error::OpsResult;
use super::{block_on_task, task_error, task_store};

/// A step of the Task's latest Graph whose Process ended waiting on a limit. It
/// is where `lf task run` picks up.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct LimitWait {
    pub step: String,
    pub retry_at: i64,
    /// The explicit agent choice the step ran under.
    pub agent: Option<String>,
}

pub(crate) fn waiting_on_limit(
    store: &SqliteStore,
    task: &TaskId,
) -> StoreResult<Option<LimitWait>> {
    let Some((step, recovery)) = ended_step_recovery(store, task)? else {
        return Ok(None);
    };
    Ok(recovery.retry_at.map(|retry_at| LimitWait {
        step,
        retry_at,
        agent: recovery.agent,
    }))
}

/// The latest step of the Task's latest Graph, once its Process has ended,
/// and how it was getting past a limit.
pub(crate) fn ended_step_recovery(
    store: &SqliteStore,
    task: &TaskId,
) -> StoreResult<Option<(String, AgentLimitRecovery)>> {
    let Some(graph) = store.task_graphs(task)?.pop() else {
        return Ok(None);
    };
    let Some(step) = graph.latest() else {
        return Ok(None);
    };
    if crate::journal::lf_evidence(store, &step.process.id) != ProcessIdentityEvidence::Dead {
        return Ok(None);
    }
    Ok(store
        .agent_limit_recovery(&step.process.id)?
        .map(|recovery| (graph.label(step), recovery)))
}

/// Whether the sweep starts a waiting run again without anyone asking: the
/// Task is owned by this Machine, unfinished, and nothing else still executes for it.
/// Status promises an automatic restart only when this holds.
pub(crate) fn restarts_by_itself(store: &SqliteStore, task: &TaskId) -> OpsResult<bool> {
    Ok(store.task_claimed_here(task).map_err(task_error)?
        && !store.task_state(task).map_err(task_error)?.is_terminal()
        && task_execution_blockers(store, task)?.is_empty())
}

/// Start again every waiting run whose wait is over and that `restarts_by_itself`.
/// A shipping Task's own restart does it, through the same `waiting_on_limit`.
pub(crate) fn restart_waiting_runs() -> OpsResult<()> {
    block_on_task(async {
        let store = task_store().await?;
        let now = crate::store::rows::now_unix();
        let waited = store
            .sqlite
            .tasks_past_a_limit_wait(now)
            .map_err(task_error)?;
        let shipping = store.sqlite.local_shipping_tasks().map_err(task_error)?;
        for id in waited {
            if shipping.iter().any(|(shipped, _)| *shipped == id)
                || !restarts_by_itself(&store.sqlite, &id)?
            {
                continue;
            }
            let Some(wait) = waiting_on_limit(&store.sqlite, &id).map_err(task_error)? else {
                continue;
            };
            if wait.retry_at > now {
                continue;
            }
            let Some(task) = store.get_task(&id).await.map_err(task_error)? else {
                continue;
            };
            let Some(worktree) = task.worktree.as_ref().filter(|path| path.is_dir()) else {
                continue;
            };
            super::ship::restart_lf_in(
                worktree,
                wait.agent.as_deref(),
                &["task", "run", &task.identifier],
            )?;
            tracing::info!(
                "restarted the run waiting on a limit for {}",
                task.identifier
            );
        }
        Ok(())
    })
}
