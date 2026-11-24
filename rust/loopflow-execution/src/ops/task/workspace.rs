//! File observation in a retained Task checkout; never prepares or starts work.
use std::collections::{BTreeSet, HashSet};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

use anyhow::Result;
use notify::{RecursiveMode, Watcher};
use serde::Serialize;

use super::{file_context, file_store};
use loopflow_model::durable::{MachineId, TaskId};

fn watch_checkout(
    expected: &Path,
    mut changed: impl FnMut(notify::Result<Vec<String>>) + Send + 'static,
) -> Result<notify::RecommendedWatcher> {
    let metadata = crate::git::absolute_git_dir(expected)?;
    // Watchers report either the watched spelling or the resolved one.
    let roots = EventRoots {
        checkout: vec![expected.to_path_buf(), expected.canonicalize()?],
        metadata: vec![metadata.clone(), metadata.canonicalize()?],
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
    watcher.watch(expected, RecursiveMode::Recursive)?;
    if !metadata.starts_with(expected) {
        watcher.watch(&metadata, RecursiveMode::Recursive)?;
    }
    Ok(watcher)
}

/// One checkout's file invalidation. Readers rescan on a new revision, including
/// when multiple changes coalesce while their connection is busy.
#[derive(Debug, Clone, Serialize, schemars::JsonSchema)]
pub struct TaskFilesRevision {
    pub task_id: TaskId,
    pub machine_id: MachineId,
    pub checkout: PathBuf,
    pub revision: String,
}

#[derive(Debug)]
pub struct TaskFilesWatch {
    _watcher: notify::RecommendedWatcher,
    pending: Arc<Mutex<PendingPaths>>,
    current: TaskFilesRevision,
    ignore_failed: bool,
}

#[derive(Debug, Default)]
struct PendingPaths {
    paths: BTreeSet<String>,
    error: Option<notify::Error>,
}

impl TaskFilesWatch {
    pub fn open(issue: &str, wake: impl Fn() + Send + 'static) -> Result<Self> {
        let store = file_store()?;
        let checkout = file_context(&store, issue)?;
        let current = TaskFilesRevision {
            task_id: checkout.task_id,
            machine_id: crate::store::machines::local_machine(&store)?.id,
            checkout: checkout.worktree,
            revision: uuid::Uuid::new_v4().to_string(),
        };
        let pending = Arc::new(Mutex::new(PendingPaths::default()));
        let changes = pending.clone();
        let watcher = watch_checkout(&current.checkout, move |event| {
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
        Ok(Self {
            _watcher: watcher,
            pending,
            current,
            ignore_failed: false,
        })
    }

    pub fn reading(&mut self) -> Result<&TaskFilesRevision> {
        let batch = std::mem::take(&mut *self.pending.lock().expect("file watch paths"));
        if let Some(error) = batch.error {
            return Err(error.into());
        }
        if !batch.paths.is_empty()
            && !relevant_paths(&self.current.checkout, batch.paths, &mut self.ignore_failed)
                .is_empty()
        {
            self.current.revision = uuid::Uuid::new_v4().to_string();
        }
        Ok(&self.current)
    }
}

struct EventRoots {
    checkout: Vec<PathBuf>,
    metadata: Vec<PathBuf>,
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

/// Drop git-ignored paths. Metadata and rescans pass; the reader judges them.
/// When git cannot judge, every path stays relevant.
fn relevant_paths(root: &Path, batch: BTreeSet<String>, ignore_failed: &mut bool) -> Vec<String> {
    if batch.contains("") {
        return vec![String::new()];
    }
    let candidates = batch
        .iter()
        .filter(|path| *path != ".git" && !path.starts_with(".git/"))
        .map(String::as_str);
    let ignored = match crate::git::ignored_paths(root, candidates) {
        Ok(ignored) => ignored,
        Err(error) => {
            if !std::mem::replace(ignore_failed, true) {
                tracing::warn!("file observation keeps ignored paths: {error}");
            }
            HashSet::new()
        }
    };
    batch
        .into_iter()
        .filter(|path| !ignored.contains(path))
        .collect()
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeSet;

    use super::relevant_paths;

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
        let batch: BTreeSet<String> = ["target/out", "note.txt", ".git/index"]
            .map(String::from)
            .into();
        let mut ignore_failed = false;
        assert_eq!(
            relevant_paths(repo.path(), batch, &mut ignore_failed),
            [".git/index", "note.txt"]
        );
        assert!(!ignore_failed);
        // A rescan is never filtered.
        let rescan: BTreeSet<String> = ["", "target/out"].map(String::from).into();
        assert_eq!(
            relevant_paths(repo.path(), rescan, &mut ignore_failed),
            [""]
        );
    }
}
