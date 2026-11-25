//! File observation in a retained Task checkout; never prepares or starts work.
use std::collections::BTreeSet;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

use anyhow::Result;
use notify::{RecursiveMode, Watcher};
use serde::{Deserialize, Serialize};

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

/// One eligible Markdown or HTML page under `scratch/` with a metadata stamp
/// derived from modification time and byte length.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
pub struct TaskScratchPage {
    pub path: String,
    pub stamp: String,
}

/// A Task's file invalidation on this Machine. `checkout` and `revision` are
/// null until the Task has a checkout here; `revision` moves only on working-tree
/// or Git metadata changes, while `scratch_pages` tracks ignored `scratch/`
/// Markdown/HTML writes independently so remote readers skip `git diff`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, schemars::JsonSchema)]
pub struct TaskFilesRevision {
    pub task_id: TaskId,
    pub machine_id: MachineId,
    #[serde(deserialize_with = "Option::deserialize")]
    pub machine_label: Option<String>,
    #[serde(deserialize_with = "Option::deserialize")]
    pub checkout: Option<PathBuf>,
    #[serde(deserialize_with = "Option::deserialize")]
    pub revision: Option<String>,
    pub scratch_pages: Vec<TaskScratchPage>,
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
                scratch_pages: Vec::new(),
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
        let changes = classify_changes(checkout, &batch.paths, &mut watching.ignore_failed);
        if changes.tree {
            self.current.revision = Some(uuid::Uuid::new_v4().to_string());
        }
        if changes.scratch {
            self.current.scratch_pages = scan_scratch_pages(checkout);
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
        self.current.scratch_pages = checkout
            .as_deref()
            .map(scan_scratch_pages)
            .unwrap_or_default();
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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct ChangeKind {
    tree: bool,
    scratch: bool,
}

/// Separate working-tree / Git changes (which bump `revision`) from `scratch/`
/// changes (which refresh `scratch_pages`). Ignored build output bumps neither;
/// an ignored scratch page refreshes `scratch_pages` without bumping `revision`.
fn classify_changes(root: &Path, paths: &BTreeSet<String>, ignore_failed: &mut bool) -> ChangeKind {
    if paths.is_empty() {
        return ChangeKind {
            tree: false,
            scratch: false,
        };
    }
    if paths.contains("") {
        return ChangeKind {
            tree: true,
            scratch: true,
        };
    }
    let scratch = paths
        .iter()
        .any(|p| p == "scratch" || p.starts_with("scratch/"));
    if paths.iter().any(|p| p == ".git" || p.starts_with(".git/")) {
        return ChangeKind {
            tree: true,
            scratch,
        };
    }
    match crate::git::ignored_paths(root, paths.iter().map(String::as_str)) {
        Ok(ignored) => ChangeKind {
            tree: paths.iter().any(|path| !ignored.contains(path)),
            scratch,
        },
        Err(error) => {
            if !std::mem::replace(ignore_failed, true) {
                tracing::warn!("file observation keeps ignored paths: {error}");
            }
            ChangeKind {
                tree: true,
                scratch: true,
            }
        }
    }
}

fn scan_scratch_pages(root: &Path) -> Vec<TaskScratchPage> {
    let Ok((paths, _)) = super::snapshot::scratch_paths(root) else {
        return Vec::new();
    };
    paths
        .into_iter()
        .filter(|path| super::find::is_scratch_page(Path::new(path)))
        .filter_map(|path| {
            let metadata = std::fs::symlink_metadata(root.join(&path)).ok()?;
            if !metadata.is_file() {
                return None;
            }
            let modified = metadata
                .modified()
                .ok()
                .and_then(|time| time.duration_since(std::time::UNIX_EPOCH).ok())
                .map_or((0, 0), |duration| {
                    (duration.as_secs(), duration.subsec_nanos())
                });
            Some(TaskScratchPage {
                path,
                stamp: format!("{}.{}:{}", modified.0, modified.1, metadata.len()),
            })
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeSet;

    use super::{classify_changes, scan_scratch_pages, ChangeKind};

    #[test]
    fn ignored_scratch_pages_refresh_scratch_state_without_bumping_tree_revision() {
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
        std::fs::write(repo.path().join(".gitignore"), "target/\nscratch/\n").unwrap();
        let classify = |paths: &[&str]| {
            let paths: BTreeSet<_> = paths.iter().map(|p| p.to_string()).collect();
            classify_changes(repo.path(), &paths, &mut false)
        };
        assert_eq!(
            classify(&[]),
            ChangeKind {
                tree: false,
                scratch: false
            }
        );
        assert_eq!(
            classify(&["target/out"]),
            ChangeKind {
                tree: false,
                scratch: false
            }
        );
        assert_eq!(
            classify(&["scratch/.!4312!page.tmp"]),
            ChangeKind {
                tree: false,
                scratch: true
            }
        );
        assert_eq!(
            classify(&["scratch/page.html"]),
            ChangeKind {
                tree: false,
                scratch: true
            }
        );
        assert_eq!(
            classify(&["target/out", "note.txt"]),
            ChangeKind {
                tree: true,
                scratch: false
            }
        );
        assert_eq!(
            classify(&["target/out", ".git/index"]),
            ChangeKind {
                tree: true,
                scratch: false
            }
        );
        assert_eq!(
            classify(&["target/out", ""]),
            ChangeKind {
                tree: true,
                scratch: true
            }
        );

        std::fs::create_dir_all(repo.path().join("scratch")).unwrap();
        std::fs::write(repo.path().join("scratch/page.html"), "<p>v1</p>").unwrap();
        std::fs::write(repo.path().join("scratch/notes.tmp"), "ignore").unwrap();
        let first = scan_scratch_pages(repo.path());
        assert_eq!(first.len(), 1);
        assert_eq!(first[0].path, "scratch/page.html");

        std::fs::write(repo.path().join("scratch/page.html"), "<p>v2 longer</p>").unwrap();
        let second = scan_scratch_pages(repo.path());
        assert_eq!(second.len(), 1);
        assert_ne!(first[0].stamp, second[0].stamp);
    }
}
