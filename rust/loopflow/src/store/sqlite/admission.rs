//! Checkout admission serializes membership changes and execution before SQLite writes.

use std::collections::BTreeMap;
use std::fs::{File, OpenOptions};
use std::path::{Path, PathBuf};

use fs2::FileExt;
use sha2::{Digest, Sha256};

use crate::durable::TaskId;
use crate::store::{StoreError, StoreResult};

use super::SqliteStore;

impl SqliteStore {
    pub(crate) fn lock_checkout(&self, cwd: &Path) -> StoreResult<Vec<File>> {
        self.lock_task_checkouts(&[cwd], None)
    }

    pub(crate) fn lock_task_checkouts(
        &self,
        workspaces: &[&Path],
        task: Option<&TaskId>,
    ) -> StoreResult<Vec<File>> {
        let roots = task
            .map(|id| self.task(id))
            .transpose()?
            .flatten()
            .and_then(|task| task.worktree)
            .into_iter()
            .collect::<Vec<_>>();
        self.lock_checkout_roots(workspaces, &roots)
    }

    pub(crate) fn lock_checkout_roots(
        &self,
        workspaces: &[&Path],
        roots: &[PathBuf],
    ) -> StoreResult<Vec<File>> {
        let paths = self.checkout_lock_paths(workspaces, roots)?;
        paths
            .iter()
            .map(|(path, exclusive)| {
                let file = Self::open_checkout_lock(path)?;
                Self::acquire_checkout_lock(file, *exclusive, std::time::Duration::from_secs(2))
            })
            .collect()
    }

    /// Lock discovery and file preparation are separate from parent-owned admission.
    pub(crate) fn checkout_lock_paths(
        &self,
        workspaces: &[&Path],
        roots: &[PathBuf],
    ) -> StoreResult<Vec<(PathBuf, bool)>> {
        let canonical = |path: &Path| {
            crate::store::canonicalize_with_missing_tail(path)
                .map_err(|error| StoreError::InvalidData(error.to_string()))
        };
        // Ancestor intent survives registration of a previously unknown Task root.
        // Siblings share ancestors; an exclusive root excludes every descendant.
        // Merge modes before locking so overlapping workspaces never upgrade a lock.
        let mut locks = BTreeMap::new();
        let mut include = |path: PathBuf| {
            for ancestor in path.ancestors().skip(1) {
                locks.entry(ancestor.to_path_buf()).or_insert(false);
            }
            locks.insert(path, true);
        };
        for root in roots {
            include(canonical(root)?);
        }
        for cwd in workspaces {
            let cwd = canonical(cwd)?;
            include(canonical(
                &crate::engine::git::worktree_root(&cwd).unwrap_or(cwd),
            )?);
        }
        let database = self
            .conn
            .lock()
            .expect("store mutex poisoned")
            .path()
            .map(str::to_string);
        let root = Path::new(database.as_deref().unwrap_or("/tmp/loopflow.db"))
            .with_extension("admission");
        Ok(locks
            .into_iter()
            .map(|(path, exclusive)| {
                let name = hex::encode(Sha256::digest(path.as_os_str().as_encoded_bytes()));
                (root.join(name), exclusive)
            })
            .collect())
    }

    pub(crate) fn open_checkout_lock(path: &Path) -> StoreResult<File> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)
                .map_err(|error| StoreError::InvalidData(error.to_string()))?;
        }
        OpenOptions::new()
            .create(true)
            .truncate(false)
            .read(true)
            .write(true)
            .open(path)
            .map_err(|error| StoreError::InvalidData(error.to_string()))
    }

    pub(crate) fn acquire_checkout_lock(
        file: File,
        exclusive: bool,
        wait: std::time::Duration,
    ) -> StoreResult<File> {
        let deadline = std::time::Instant::now() + wait;
        loop {
            let acquired = if exclusive {
                FileExt::try_lock_exclusive(&file)
            } else {
                FileExt::try_lock_shared(&file)
            };
            match acquired {
                Ok(()) => break,
                Err(error)
                    if error.kind() == std::io::ErrorKind::WouldBlock
                        && std::time::Instant::now() < deadline =>
                {
                    std::thread::sleep(std::time::Duration::from_millis(10));
                }
                Err(error) => {
                    return Err(StoreError::InvalidData(format!(
                        "checkout admission unavailable: {error}"
                    )))
                }
            }
        }
        Ok(file)
    }
}
