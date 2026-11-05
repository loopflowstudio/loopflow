//! A step waiting out a usage limit does so in its own Process. When that
//! Process ends first (a restart, a reboot), the wait's record says which step
//! the run stood at. The run starts again there once an account can take it,
//! so the steps before it stay done.

use crate::durable::TaskId;
use crate::journal::ProcessIdentityEvidence;
use crate::store::sqlite::SqliteStore;
use crate::store::StoreResult;

use super::super::error::OpsResult;
use super::{block_on_task, task_error, task_store};

/// The step of the Task's latest Graph that ended waiting on a limit, and when
/// that wait ends. It is where `lf task run` picks up.
pub(crate) fn held(store: &SqliteStore, task: &TaskId) -> StoreResult<Option<(String, i64)>> {
    let Some(graph) = store.task_graphs(task)?.pop() else {
        return Ok(None);
    };
    let Some(step) = graph.latest() else {
        return Ok(None);
    };
    if crate::journal::process_evidence(store, &step.process.id) != ProcessIdentityEvidence::Dead {
        return Ok(None);
    }
    Ok(store
        .agent_limit_recovery(&step.process.id)?
        .and_then(|recovery| recovery.retry_at)
        .map(|retry_at| (graph.label(step), retry_at)))
}

/// Start again every owned Task's run that a limit held and whose wait is over.
/// A shipping Task's own restart does it, through the same `held`.
pub(crate) fn restart_held_runs() -> OpsResult<()> {
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
                || store
                    .sqlite
                    .task_state(&id)
                    .map_err(task_error)?
                    .is_terminal()
                || !store.sqlite.task_owned(&id).map_err(task_error)?
            {
                continue;
            }
            let Some((_, retry_at)) = held(&store.sqlite, &id).map_err(task_error)? else {
                continue;
            };
            if retry_at > now
                || !crate::ops::task_execution::task_execution_blockers(&store.sqlite, &id)?
                    .is_empty()
            {
                continue;
            }
            let Some(task) = store.get_task(&id).await.map_err(task_error)? else {
                continue;
            };
            let Some(worktree) = task.worktree.as_ref().filter(|path| path.is_dir()) else {
                continue;
            };
            super::ship::start_lf_in(worktree, &["task", "run", &task.identifier])?;
            tracing::info!("restarted the run a limit held for {}", task.identifier);
        }
        Ok(())
    })
}
