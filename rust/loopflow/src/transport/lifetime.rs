//! When a Session owner ends itself.
//!
//! An owner whose lf executable is removed, replaced or superseded (a worktree
//! deleted, a build relinked, Dev.app rebuilt, `lf install` selecting another
//! build) ends as soon as its Sessions wait for a person, viewers
//! notwithstanding: it would otherwise run a vanished or outdated build
//! indefinitely. Its attached viewers are told why and sent the `lf resume`
//! that reopens the conversation through the entry, and they run it. An owner nobody has viewed for
//! `session.end_detached_after` ends the same way. Ending stops its agent and
//! records its exit, so `lf resume` reopens the conversation from provider
//! history and `lf resume --all` leaves it alone.

use std::os::unix::fs::MetadataExt;
use std::path::{Path, PathBuf};
use std::sync::mpsc;
use std::time::{Duration, Instant};

use anyhow::Result;

use super::Response;
use crate::config::DetachedLimit;
use crate::id::LfProcessId;

/// The longest `recheck` pause.
const LONGEST_PAUSE: Duration = Duration::from_secs(5 * 60);
/// How often an owner looks at its executable and the entry.
const EXECUTABLE_PAUSE: Duration = Duration::from_secs(2);

/// Record viewer changes sent on the first channel. Once the executable is gone
/// or superseded, or the owner has gone unviewed past its limit, send the second
/// channel the `Response::Moved` for attached viewers; the serve loop delivers
/// it and ends the owner.
pub(super) fn spawn(owner: LfProcessId) -> (mpsc::Sender<bool>, mpsc::Receiver<Response>) {
    let executable = Executable::current();
    let (viewers, changes) = mpsc::channel();
    let (end, ending) = mpsc::channel();
    std::thread::spawn(move || {
        if let Err(error) = watch(&owner, executable.as_ref(), &changes, &end) {
            tracing::warn!(%error, %owner, "Session owner policy stopped");
        }
    });
    (viewers, ending)
}

fn watch(
    owner: &LfProcessId,
    executable: Option<&Executable>,
    changes: &mpsc::Receiver<bool>,
    end: &mpsc::Sender<Response>,
) -> Result<()> {
    let store = crate::journal::process_ledger()?;
    let repo = store.process(owner)?.and_then(|process| process.repo);
    let limit = limit(repo.as_deref());
    let mut due: Option<Instant> = None;
    loop {
        let wake = [due, executable.map(|_| Instant::now() + EXECUTABLE_PAUSE)]
            .into_iter()
            .flatten()
            .min();
        let change = match wake {
            Some(wake) => changes.recv_timeout(wake.saturating_duration_since(Instant::now())),
            None => changes
                .recv()
                .map_err(|_| mpsc::RecvTimeoutError::Disconnected),
        };
        match change {
            Ok(viewed) => {
                // A store a newer lf has advanced refuses this write. The
                // policy keeps running; readers ask this owner directly.
                if let Err(error) = store.record_owner_viewers(owner, viewed) {
                    tracing::warn!(%error, %owner, viewed, "cannot record Session viewers");
                }
                due = limit
                    .filter(|_| !viewed)
                    .map(|limit| Instant::now() + limit);
            }
            Err(mpsc::RecvTimeoutError::Timeout) => {
                let now = Instant::now();
                let expired = due.is_some_and(|due| due <= now);
                let change = executable.and_then(Executable::change);
                if change.is_none() && !expired {
                    continue;
                }
                // A store a newer lf has advanced may refuse this read too;
                // the owner judges again next time rather than running on.
                let waits = match store.owner_waits_for_person(owner) {
                    Ok(waits) => waits,
                    Err(error) => {
                        tracing::warn!(%error, %owner, "cannot tell whether the Session waits for a person");
                        false
                    }
                };
                if waits {
                    let moved = match change {
                        Some(change) => {
                            tracing::warn!(%owner, "ending a Session whose lf was {change}");
                            // The notice names the command viewers run through
                            // the entry, so it survives a failed follow.
                            let session = store
                                .owner_sessions(owner)?
                                .into_iter()
                                .next()
                                .expect("an owner waiting for a person has a Session");
                            let successor =
                                crate::os_process::resolve_lf_binary().ok().map(|entry| {
                                    vec![
                                        entry.to_string_lossy().into_owned(),
                                        "session".into(),
                                        "resume".into(),
                                        session.clone(),
                                    ]
                                });
                            let notice = format!(
                                "This Session's lf was {change}; reopening it as `lf resume {session}`"
                            );
                            Response::Moved {
                                notice: Some(notice),
                                successor,
                            }
                        }
                        None => {
                            tracing::info!(%owner, "ending a Session nobody has viewed");
                            Response::Moved {
                                notice: None,
                                successor: None,
                            }
                        }
                    };
                    if end.send(moved).is_err() {
                        crate::platform::kill_process(std::process::id());
                    }
                    return Ok(());
                }
                if expired {
                    due = limit.map(|limit| now + recheck(limit));
                }
            }
            Err(mpsc::RecvTimeoutError::Disconnected) => return Ok(()),
        }
    }
}

/// The file this owner runs from, as it was when the owner started.
#[derive(Debug)]
struct Executable {
    path: PathBuf,
    identity: (u64, u64, u64, i64, i64),
}

/// What became of the owner's executable.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Change {
    Removed,
    Replaced,
    /// The entry now dispatches to another build.
    Superseded,
}

impl std::fmt::Display for Change {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            Change::Removed => "removed",
            Change::Replaced => "replaced",
            Change::Superseded => "superseded",
        })
    }
}

impl Executable {
    /// `None` when the running executable cannot be read; that owner keeps only
    /// the detached limit.
    fn current() -> Option<Self> {
        let read = || -> std::io::Result<Self> {
            let path = crate::journal::current_executable()?;
            let identity = identity(&std::fs::metadata(&path)?);
            Ok(Self { path, identity })
        };
        read()
            .inspect_err(
                |error| tracing::warn!(%error, "Session owner cannot watch its executable"),
            )
            .ok()
    }

    /// Removed or replaced once the file at the path is no longer this one;
    /// superseded once the entry dispatches to another build. Any other read
    /// error is transient and judged again next time.
    fn change(&self) -> Option<Change> {
        match std::fs::metadata(&self.path) {
            Ok(metadata) if identity(&metadata) != self.identity => return Some(Change::Replaced),
            Ok(_) => {}
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                return Some(Change::Removed)
            }
            Err(_) => return None,
        }
        crate::installation::superseded(&self.path).then_some(Change::Superseded)
    }
}

fn identity(metadata: &std::fs::Metadata) -> (u64, u64, u64, i64, i64) {
    (
        metadata.dev(),
        metadata.ino(),
        metadata.len(),
        metadata.mtime(),
        metadata.mtime_nsec(),
    )
}

/// The detached limit configured for `repo`; `None` when it is off or the
/// configuration is unreadable, so the owner never ends for want of viewers.
fn limit(repo: Option<&str>) -> Option<Duration> {
    match crate::config::load_config(repo.map(Path::new)) {
        Ok(config) => match config.unwrap_or_default().session.end_detached_after {
            DetachedLimit::After(limit) => Some(limit),
            DetachedLimit::Off => None,
        },
        Err(error) => {
            tracing::warn!(%error, ?repo, "keep detached Sessions: configuration unreadable");
            None
        }
    }
}

/// How long an owner past `limit` waits before looking again while a turn or
/// Graph holds it.
fn recheck(limit: Duration) -> Duration {
    (limit / 12).clamp(Duration::from_secs(1), LONGEST_PAUSE)
}
