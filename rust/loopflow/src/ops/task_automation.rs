//! Whether a Task's work is executing, and which Tasks a repository holds.
use std::collections::HashSet;

use crate::journal::{process_evidence, ProcessIdentityEvidence};
use crate::process::LfProcess;
use crate::store::sqlite::SqliteStore;
use crate::store::SharedStore;
use crate::work::task::Task;

use super::error::{OpsError, OpsResult};

fn error(error: impl std::fmt::Display) -> OpsError {
    OpsError::Message(error.to_string())
}

/// Whether anything is executing is asked only of the OS, about recorded
/// Processes. Unknown evidence blocks. The caller's lineage waits on the caller,
/// so it never does; that exempts waiting and grants no authority.
pub(crate) fn execution_blockers(
    store: &SqliteStore,
    processes: &[LfProcess],
) -> OpsResult<Vec<String>> {
    let mut lineage = HashSet::new();
    let mut next = crate::journal::current_lf_process_id();
    while let Some(id) = next.filter(|id| lineage.insert(id.clone())) {
        next = store
            .process(&id)
            .map_err(error)?
            .and_then(|process| process.parent_lf_process_id);
    }
    Ok(processes
        .iter()
        .filter(|process| {
            process.completed_at.is_none()
                && !lineage.contains(&process.id)
                && process_evidence(store, &process.id) != ProcessIdentityEvidence::Dead
        })
        .map(|process| {
            format!(
                "Process {} has live or unresolved execution; inspect `lf history show {}`",
                process.id, process.id
            )
        })
        .collect())
}

/// A Task's work is every Process in its checkout or bound through a Session.
pub(crate) fn task_execution_blockers(
    store: &SqliteStore,
    task: &crate::durable::TaskId,
) -> OpsResult<Vec<String>> {
    let processes = store.task_open_processes(task).map_err(error)?;
    execution_blockers(store, &processes)
}

pub(crate) async fn repository_tasks(
    store: &SharedStore,
    repo: &crate::repository::RepoId,
) -> OpsResult<Vec<Task>> {
    let mut tasks = Vec::new();
    for task in store.list_tasks(None).await.map_err(error)? {
        let task_repo = super::task::repository_for_task(store, &task)?;
        if crate::repository::RepoId::discover(&task_repo)
            .ok()
            .as_ref()
            == Some(repo)
        {
            tasks.push(task);
        }
    }
    Ok(tasks)
}
