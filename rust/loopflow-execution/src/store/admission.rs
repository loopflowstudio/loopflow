//! Observe workspace roots before taking storage-owned admission leases.
use super::{sqlite::SqliteStore, StoreError, StoreResult};
use loopflow_model::durable::TaskId;
use std::fs::File;
use std::path::{Path, PathBuf};

pub(crate) fn lock_checkout(store: &SqliteStore, cwd: &Path) -> StoreResult<Vec<File>> {
    lock_task_checkouts(store, &[cwd], None)
}

pub(crate) fn lock_session_checkouts(
    store: &SqliteStore,
    session: &crate::session::LfSession,
) -> StoreResult<Vec<File>> {
    lock_task_checkouts(store, &[&session.cwd], session.task_id.as_ref())
}

pub fn lock_task_checkouts(
    store: &SqliteStore,
    workspaces: &[&Path],
    task: Option<&TaskId>,
) -> StoreResult<Vec<File>> {
    let mut roots = task
        .map(|id| store.task(id))
        .transpose()?
        .flatten()
        .and_then(|task| task.worktree)
        .into_iter()
        .collect::<Vec<_>>();
    for cwd in workspaces {
        let cwd = canonical(cwd)?;
        roots.push(canonical(&crate::git::worktree_root(&cwd).unwrap_or(cwd))?);
    }
    lock_checkout_roots(store, &roots)
}

pub(crate) fn lock_checkout_roots(
    store: &SqliteStore,
    roots: &[PathBuf],
) -> StoreResult<Vec<File>> {
    let roots = roots
        .iter()
        .map(|root| canonical(root))
        .collect::<StoreResult<Vec<_>>>()?;
    store.lock_checkout_roots(&roots)
}
fn canonical(path: &Path) -> StoreResult<PathBuf> {
    super::canonicalize_with_missing_tail(path)
        .map_err(|error| StoreError::InvalidData(error.to_string()))
}
