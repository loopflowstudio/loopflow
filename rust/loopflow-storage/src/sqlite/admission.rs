//! Checkout admission serializes membership changes and execution before SQLite writes.

use std::collections::BTreeMap;
use std::fs::File;
use std::path::{Path, PathBuf};

use sha2::{Digest, Sha256};

use crate::{StoreError, StoreResult};

use super::SqliteStore;

impl SqliteStore {
    pub fn lock_checkout_roots(&self, roots: &[PathBuf]) -> StoreResult<Vec<File>> {
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
            include(root.clone());
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
        crate::lock_file::lock_with_timeout_verified(
            &root.join(name),
            exclusive,
            std::time::Duration::from_secs(2),
            std::time::Duration::from_millis(10),
        )
        .map_err(|error| {
            StoreError::InvalidData(format!(
                "checkout admission unavailable for {}: {error}",
                cwd.display()
            ))
        })
    }
}
