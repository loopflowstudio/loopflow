//! Which Task checkouts changed on disk. A dirty file or a new commit is not
//! a store commit, so the reader watches each existing checkout and its Git
//! metadata and asks Git again only about the ones that changed.

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

/// A checkout an agent is building in changes continuously. It is asked about
/// at most this often.
const REST: Duration = Duration::from_secs(3);

#[derive(Debug, Default)]
pub(super) struct Checkouts {
    watched: Vec<PathBuf>,
    changed: Arc<Mutex<BTreeSet<PathBuf>>>,
    asked_on: Option<Instant>,
    #[cfg(target_os = "macos")]
    stream: Option<crate::platform::fs_events::Stream>,
}

/// A linked worktree keeps its index and HEAD outside the checkout.
fn metadata(worktree: &Path) -> Option<PathBuf> {
    let pointer = std::fs::read_to_string(worktree.join(".git")).ok()?;
    Path::new(pointer.strip_prefix("gitdir:")?.trim())
        .canonicalize()
        .ok()
}

impl Checkouts {
    /// Watch the checkouts that exist now; `wake` runs when one changes.
    pub(super) fn watch(
        &mut self,
        mut worktrees: Vec<PathBuf>,
        wake: impl Fn() + Send + Sync + 'static,
    ) {
        worktrees.retain(|worktree| worktree.is_dir());
        worktrees.sort();
        worktrees.dedup();
        if worktrees == self.watched {
            return;
        }
        self.watched = worktrees;
        #[cfg(target_os = "macos")]
        {
            // Events name resolved paths; Git was asked with the stored ones.
            let roots = self
                .watched
                .iter()
                .flat_map(|worktree| {
                    [worktree.canonicalize().ok(), metadata(worktree)]
                        .into_iter()
                        .flatten()
                        .map(|root| (root, worktree.clone()))
                })
                .collect::<Vec<_>>();
            let (changed, all) = (self.changed.clone(), self.watched.clone());
            let watch = roots.clone();
            self.stream = None;
            self.stream = crate::platform::fs_events::Stream::start(
                &roots
                    .iter()
                    .map(|(root, _)| root.as_path())
                    .collect::<Vec<_>>(),
                0.5,
                false,
                move |path| {
                    let mut changed = changed.lock().unwrap_or_else(|error| error.into_inner());
                    match path {
                        None => changed.extend(all.iter().cloned()),
                        Some(path) => changed.extend(
                            watch
                                .iter()
                                .filter(|(root, _)| path.starts_with(root))
                                .map(|(_, worktree)| worktree.clone()),
                        ),
                    }
                    drop(changed);
                    wake();
                },
            )
            .inspect_err(|error| tracing::warn!(%error, "cannot watch Task checkouts"))
            .ok();
        }
        #[cfg(not(target_os = "macos"))]
        let _ = (wake, metadata);
    }

    /// The checkouts that changed since they were last taken, once rested.
    pub(super) fn take(&mut self, now: Instant) -> BTreeSet<PathBuf> {
        if self.asked_on.is_some_and(|asked| now - asked < REST) {
            return BTreeSet::new();
        }
        let changed = std::mem::take(
            &mut *self
                .changed
                .lock()
                .unwrap_or_else(|error| error.into_inner()),
        );
        if !changed.is_empty() {
            self.asked_on = Some(now);
        }
        changed
    }
}
