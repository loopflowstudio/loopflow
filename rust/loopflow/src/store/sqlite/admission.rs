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
        if let Some(task) = task {
            if let Some(task) = self.task(task)? {
                if !task.worktree.as_os_str().is_empty() {
                    include(canonical(&task.worktree)?);
                }
            }
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
        std::fs::create_dir_all(&root)
            .map_err(|error| StoreError::InvalidData(error.to_string()))?;
        locks
            .iter()
            .map(|(path, exclusive)| Self::lock_checkout_path(&root, path, *exclusive))
            .collect()
    }

    fn lock_checkout_path(root: &Path, cwd: &Path, exclusive: bool) -> StoreResult<File> {
        let name = hex::encode(Sha256::digest(cwd.as_os_str().as_encoded_bytes()));
        let file = OpenOptions::new()
            .create(true)
            .truncate(false)
            .read(true)
            .write(true)
            .open(root.join(name))
            .map_err(|error| StoreError::InvalidData(error.to_string()))?;
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(2);
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
                        "checkout admission unavailable for {}: {error}",
                        cwd.display()
                    )))
                }
            }
        }
        Ok(file)
    }
}
