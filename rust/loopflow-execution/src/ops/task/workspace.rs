//! File observation in a retained Task checkout; never prepares or starts work.
use std::collections::BTreeSet;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

use anyhow::Result;
use notify::{RecursiveMode, Watcher};
use serde::Serialize;

use super::file_store;
use loopflow_model::durable::{MachineId, TaskId};
use loopflow_storage::sqlite::SqliteStore;

fn watch_checkout(
    checkout: &Path,
    mut changed: impl FnMut(notify::Result<Vec<String>>) + Send + 'static,
) -> Result<notify::RecommendedWatcher> {
    let metadata = crate::git::absolute_git_dir(checkout)?;
    // Watchers report either the watched spelling or the resolved one.
    let roots = EventRoots {
        checkout: [checkout.to_path_buf(), checkout.canonicalize()?],
        metadata: [metadata.clone(), metadata.canonicalize()?],
    };
    let mut watcher =
        notify::recommended_watcher(move |event: notify::Result<notify::Event>| match event {
            Ok(event) if event.kind.is_access() => {}
            Ok(event) if event.need_rescan() => changed(Ok(vec![String::new()])),
            Ok(event) => {
                let paths: Vec<_> = event
                    .paths
                    .iter()
                    .filter_map(|path| roots.relative(path))
                    .collect();
                if !paths.is_empty() {
                    changed(Ok(paths));
                }
            }
            Err(error) => changed(Err(error)),
        })?;
    watcher.watch(checkout, RecursiveMode::Recursive)?;
    if !metadata.starts_with(checkout) {
        watcher.watch(&metadata, RecursiveMode::Recursive)?;
    }
    Ok(watcher)
}

/// A Task's file invalidation on this Machine. `checkout` and `revision` are
/// null until the Task has a checkout here; readers rescan on a new revision,
/// including when multiple changes coalesce while their connection is busy.
#[derive(Debug, Clone, PartialEq, Serialize, schemars::JsonSchema)]
pub struct TaskFilesRevision {
    pub task_id: TaskId,
    pub machine_id: MachineId,
    pub machine_label: Option<String>,
    pub checkout: Option<PathBuf>,
    pub revision: Option<String>,
}

/// Follows one Task's recorded checkout: it opens a directory watcher when the
/// checkout appears and replaces it when the record changes.
pub struct TaskFilesWatch {
    store: SqliteStore,
    wake: Arc<dyn Fn() + Send + Sync>,
    watching: Option<CheckoutWatch>,
    current: TaskFilesRevision,
}

impl std::fmt::Debug for TaskFilesWatch {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("TaskFilesWatch")
            .field("current", &self.current)
            .finish_non_exhaustive()
    }
}

#[derive(Debug)]
struct CheckoutWatch {
    _watcher: notify::RecommendedWatcher,
    pending: Arc<Mutex<PendingPaths>>,
    ignore_failed: bool,
}

#[derive(Debug, Default)]
struct PendingPaths {
    paths: BTreeSet<String>,
    error: Option<notify::Error>,
}

impl TaskFilesWatch {
    /// # Errors
    /// The Task does not exist, or the registry cannot be read.
    pub fn open(issue: &str, wake: impl Fn() + Send + Sync + 'static) -> Result<Self> {
        let store = file_store()?;
        let task_id =
            crate::store::children::resolve_task_id(&store, issue, None)?.ok_or_else(|| {
                crate::engine::Error::new(
                    crate::engine::ErrorCode::NotFound,
                    format!("no Task exists for {issue:?}"),
                )
            })?;
        let machine = crate::store::machines::local_machine(&store)?;
        Ok(Self {
            store,
            wake: Arc::new(wake),
            watching: None,
            current: TaskFilesRevision {
                task_id,
                machine_id: machine.id,
                machine_label: machine.label,
                checkout: None,
                revision: None,
            },
        })
    }

    /// The current reading. A changed checkout record replaces the watcher.
    pub fn reading(&mut self) -> Result<&TaskFilesRevision> {
        self.follow_record()?;
        let Some(watching) = &mut self.watching else {
            return Ok(&self.current);
        };
        let batch = std::mem::take(&mut *watching.pending.lock().expect("file watch paths"));
        if let Some(error) = batch.error {
            return Err(error.into());
        }
        let checkout = self.current.checkout.as_deref().expect("watched checkout");
        if invalidates_tree(checkout, &batch.paths, &mut watching.ignore_failed) {
            self.current.revision = Some(uuid::Uuid::new_v4().to_string());
        }
        Ok(&self.current)
    }

    fn follow_record(&mut self) -> Result<()> {
        // The record names a checkout before Git makes it; its CheckoutReady
        // event is the next planning write, which reads it again.
        let checkout = self
            .store
            .task_checkout(&self.current.task_id)?
            .map(|checkout| checkout.worktree)
            .filter(|worktree| worktree.join(".git").exists());
        if checkout == self.current.checkout {
            return Ok(());
        }
        self.watching = checkout
            .as_deref()
            .map(|path| self.watch(path))
            .transpose()?;
        self.current.revision = checkout.as_ref().map(|_| uuid::Uuid::new_v4().to_string());
        self.current.checkout = checkout;
        Ok(())
    }

    fn watch(&self, checkout: &Path) -> Result<CheckoutWatch> {
        let pending = Arc::new(Mutex::new(PendingPaths::default()));
        let changes = pending.clone();
        let wake = self.wake.clone();
        let watcher = watch_checkout(checkout, move |event| {
            let mut batch = changes.lock().expect("file watch paths");
            match event {
                Err(error) => batch.error = Some(error),
                Ok(paths) if !batch.paths.contains("") => {
                    batch.paths.extend(paths);
                    if batch.paths.len() > 1024 || batch.paths.contains("") {
                        batch.paths.clear();
                        batch.paths.insert(String::new());
                    }
                }
                Ok(_) => {}
            }
            drop(batch);
            wake();
        })?;
        Ok(CheckoutWatch {
            _watcher: watcher,
            pending,
            ignore_failed: false,
        })
    }
}

struct EventRoots {
    checkout: [PathBuf; 2],
    metadata: [PathBuf; 2],
}

impl EventRoots {
    /// Report metadata as if it sat inside the checkout, as a main checkout's does.
    fn relative(&self, path: &Path) -> Option<String> {
        if let Some(inside) = self
            .metadata
            .iter()
            .find_map(|root| path.strip_prefix(root).ok())
        {
            return Some(
                Path::new(".git")
                    .join(inside)
                    .to_string_lossy()
                    .into_owned(),
            );
        }
        let inside = self
            .checkout
            .iter()
            .find_map(|root| path.strip_prefix(root).ok())?;
        Some(inside.to_string_lossy().into_owned())
    }
}

/// Rescans and Git metadata always invalidate the tree. Other changes matter
/// only if at least one path is visible, or Git cannot tell.
fn invalidates_tree(root: &Path, paths: &BTreeSet<String>, ignore_failed: &mut bool) -> bool {
    if paths.is_empty() {
        return false;
    }
    if paths
        .iter()
        .any(|p| p.is_empty() || p == ".git" || p.starts_with(".git/"))
    {
        return true;
    }
    match crate::git::ignored_paths(root, paths.iter().map(String::as_str)) {
        Ok(ignored) => paths.iter().any(|path| !ignored.contains(path)),
        Err(error) => {
            if !std::mem::replace(ignore_failed, true) {
                tracing::warn!("file observation keeps ignored paths: {error}");
            }
            true
        }
    }
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeSet;

    use super::invalidates_tree;

    #[test]
    fn ignored_build_output_never_invalidates_the_tree() {
        let repo = tempfile::tempdir().unwrap();
        let git = |args: &[&str]| {
            assert!(std::process::Command::new("git")
                .args(args)
                .current_dir(repo.path())
                .status()
                .unwrap()
                .success())
        };
        git(&["init", "-q"]);
        std::fs::write(repo.path().join(".gitignore"), "target/\n").unwrap();
        let changed = |paths: &[&str]| {
            let paths: BTreeSet<_> = paths.iter().map(|p| p.to_string()).collect();
            invalidates_tree(repo.path(), &paths, &mut false)
        };
        assert!(!changed(&[]));
        assert!(!changed(&["target/out"]));
        assert!(changed(&["target/out", "note.txt"]));
        assert!(changed(&["target/out", ".git/index"]));
        assert!(changed(&["target/out", ""]));
    }
}
