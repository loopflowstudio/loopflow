//! File observation in a retained Task checkout; never prepares or starts work.
use std::collections::{BTreeSet, HashSet};
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::sync::{
    atomic::{AtomicBool, Ordering},
    mpsc, Arc, Mutex,
};
use std::time::{Duration, Instant};

use anyhow::{anyhow, ensure, Result};
use notify::{RecursiveMode, Watcher};
use serde::{Deserialize, Serialize};

use super::{file_context, file_store};
use loopflow_model::durable::{MachineId, TaskId};
use loopflow_storage::sqlite::TaskCheckout;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TaskFilesFrame {
    pub request: String,
    pub task_id: TaskId,
    pub machine_id: MachineId,
    pub checkout: String,
    /// Checkout-relative paths that may change what the view shows: `.git/...`
    /// for repository metadata, `""` for a rescan. Empty is a heartbeat.
    pub paths: Vec<String>,
}

pub(crate) fn pinned_checkout(issue: &str, expected: &Path) -> Result<TaskCheckout> {
    let checkout = file_context(&file_store()?, issue)?;
    ensure!(
        checkout.worktree == expected,
        "The Task checkout changed; reopen the Task. No companion was retargeted."
    );
    ensure!(
        expected.is_dir(),
        "The recorded Task checkout is unavailable"
    );
    Ok(checkout)
}

/// The existing owned-line transport keeps stdin open and closes it on cancellation.
/// Heartbeats recheck recorded ownership; filesystem events, not a read timer,
/// invalidate contents. Access events from our own reads never cause another read,
/// and neither do git-ignored writes such as `lf`'s own journal traces.
pub fn watch_task_files(issue: &str, expected: &Path, request: &str) -> Result<()> {
    let checkout = pinned_checkout(issue, expected)?;
    let machine_id = crate::store::machines::local_machine(&file_store()?)?.id;
    let (send, events) = mpsc::channel::<Vec<String>>();
    let error = Arc::new(Mutex::new(None));
    let failure = error.clone();
    let observed = send.clone();
    let _watcher = watch_checkout(expected, move |event| {
        let paths = event.unwrap_or_else(|error| {
            *failure.lock().expect("file observation error lock") = Some(error.to_string());
            vec![String::new()]
        });
        let _ = observed.send(paths);
    })?;
    let closed = Arc::new(AtomicBool::new(false));
    let input_closed = closed.clone();
    std::thread::spawn(move || {
        let mut byte = [0];
        while std::io::stdin().read(&mut byte).is_ok_and(|n| n != 0) {}
        input_closed.store(true, Ordering::Release);
        let _ = send.send(Vec::new());
    });
    let mut paths = vec![String::new()];
    let mut ignore_failed = false;
    loop {
        if closed.load(Ordering::Acquire) {
            return Ok(());
        }
        if let Some(error) = error.lock().expect("file observation error lock").take() {
            return Err(anyhow!("Live file observation failed: {error}"));
        }
        let current = pinned_checkout(issue, expected)?;
        ensure!(
            current.task_id == checkout.task_id,
            "Task execution ownership changed; reopen the Task"
        );
        let frame = TaskFilesFrame {
            request: request.into(),
            task_id: current.task_id,
            machine_id: machine_id.clone(),
            checkout: expected.to_string_lossy().into_owned(),
            paths,
        };
        println!("{}", serde_json::to_string(&frame)?);
        std::io::stdout().flush()?;
        let heartbeat = Instant::now() + Duration::from_secs(2);
        paths = loop {
            // Continuous ignored churn must not starve the heartbeat's ownership recheck.
            let wait = heartbeat.saturating_duration_since(Instant::now());
            if wait.is_zero() {
                break Vec::new();
            }
            let Ok(first) = events.recv_timeout(wait) else {
                break Vec::new();
            };
            if closed.load(Ordering::Acquire) {
                break Vec::new();
            }
            std::thread::sleep(Duration::from_millis(150));
            let batch = first
                .into_iter()
                .chain(events.try_iter().flatten())
                .collect();
            let relevant = relevant_paths(expected, batch, &mut ignore_failed);
            if !relevant.is_empty() {
                break relevant;
            }
        };
    }
}

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
