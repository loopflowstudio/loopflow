//! When a Session owner ends itself.
//!
//! A terminal owner nobody views ends after `session.end_detached_after` of
//! quiet: no viewer, no output from its command, and nothing in the store
//! holding it (a turn in progress, a running Graph, a headless Run). An owner
//! whose lf executable is removed, replaced or superseded (a worktree deleted,
//! a build relinked, Dev.app rebuilt, another installed build selected) ends once
//! its Sessions wait for a person and its command has been quiet for one poll,
//! viewers notwithstanding: it would
//! otherwise run a vanished or outdated build indefinitely. Its attached viewers
//! are told why and follow the resume command through the lf entry. Ending
//! writes the last screen, stops the agent and records the exit, so `lf resume`
//! reopens the conversation from provider history and `lf resume --all` leaves
//! it alone.
//!
//! The clock is wall time: a laptop asleep overnight ends its idle Sessions at
//! the first poll after it wakes. A store that cannot be read is judged again
//! next time; the policy never stops on an error.

use std::io::Write;
use std::os::unix::fs::MetadataExt;
use std::os::unix::net::UnixStream;
use std::path::{Path, PathBuf};
use std::sync::mpsc;
use std::time::{Duration, SystemTime};

use super::Response;
use crate::config::DetachedLimit;
use crate::id::LfProcessId;
use crate::store::sqlite::SqliteStore;

/// How often an owner looks at the clock and its executable.
const POLL: Duration = Duration::from_secs(2);

/// What the serve loop tells the policy.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Activity {
    /// Whether any viewer is attached, sent on each change.
    Viewed(bool),
    /// The command wrote to its terminal: a turn is in progress as far as
    /// the owner can see.
    Output,
}

/// Record activity sent on the first channel. Once the executable is gone or
/// the owner has gone unviewed and quiet past its limit, send the second
/// channel what to tell attached viewers; the serve loop delivers it and ends
/// the owner. Only a terminal owner (`terminal`) ends for want of viewers;
/// a pipe owner is a headless Run.
pub(super) fn spawn(
    owner: LfProcessId,
    terminal: bool,
    waker: UnixStream,
) -> (mpsc::Sender<Activity>, mpsc::Receiver<Response>) {
    let executable = Executable::current();
    let (activity, changes) = mpsc::channel();
    let (end, ending) = mpsc::channel();
    std::thread::spawn(move || {
        watch(&owner, executable.as_ref(), terminal, &changes, &end);
        // Wake the descriptor loop once the policy has spoken.
        let _ = (&waker).write_all(&[1]);
    });
    (activity, ending)
}

/// When an unviewed, quiet owner is due to end, in wall time.
#[derive(Debug)]
struct Grace {
    /// Unknown until the store names the owner's repository; a pipe owner
    /// never learns one and never ends for want of viewers.
    limit: Option<DetachedLimit>,
    viewed: bool,
    quiet_since: SystemTime,
}

impl Grace {
    fn note(&mut self, activity: Activity, now: SystemTime) {
        if let Activity::Viewed(viewed) = activity {
            self.viewed = viewed;
        }
        self.quiet_since = now;
    }

    fn expired(&self, now: SystemTime) -> bool {
        match self.limit {
            Some(DetachedLimit::After(limit)) if !self.viewed => now >= self.quiet_since + limit,
            _ => false,
        }
    }
}

fn watch(
    owner: &LfProcessId,
    executable: Option<&Executable>,
    terminal: bool,
    changes: &mpsc::Receiver<Activity>,
    end: &mpsc::Sender<Response>,
) {
    let mut store = Store::default();
    let mut last_output = SystemTime::now();
    let mut grace = Grace {
        limit: None,
        viewed: false,
        quiet_since: last_output,
    };
    loop {
        match changes.recv_timeout(POLL) {
            Ok(activity) => {
                let now = SystemTime::now();
                grace.note(activity, now);
                if activity == Activity::Output {
                    last_output = now;
                }
                if let Activity::Viewed(viewed) = activity {
                    // A store a newer lf has advanced refuses this write. The
                    // policy keeps running; readers ask this owner directly.
                    if let Some(store) = store.open(owner) {
                        if let Err(error) = store.record_owner_viewers(owner, viewed) {
                            tracing::warn!(%error, %owner, viewed, "cannot record Session viewers");
                        }
                    }
                }
            }
            Err(mpsc::RecvTimeoutError::Timeout) => {}
            Err(mpsc::RecvTimeoutError::Disconnected) => return,
        }
        if terminal && grace.limit.is_none() {
            if let Some(store) = store.open(owner) {
                let repo = store
                    .process(owner)
                    .ok()
                    .flatten()
                    .and_then(|process| process.repo);
                grace.limit = Some(limit(repo.as_deref()));
            }
        }
        let now = SystemTime::now();
        let change = executable.and_then(Executable::change);
        // Native turns may leave only terminal output. Viewers never prolong a
        // vanished build, so their activity cannot reset this quiet interval.
        let quiet = match change {
            Some(_) => now.duration_since(last_output).unwrap_or_default() >= POLL,
            None => grace.expired(now),
        };
        if !quiet {
            continue;
        }
        let Some(store) = store.open(owner) else {
            continue;
        };
        match store.owner_waits_for_person(owner) {
            Ok(true) => {}
            // A turn or a running Graph holds it: activity, and the grace
            // runs again from here.
            Ok(false) => {
                grace.quiet_since = now;
                continue;
            }
            Err(error) => {
                tracing::warn!(%error, %owner, "cannot judge whether the Session waits for a person");
                continue;
            }
        }
        let moved = match change {
            Some(change) => {
                tracing::warn!(
                    %owner,
                    path = ?executable.map(|executable| &executable.path),
                    "ending a Session whose lf was {change}"
                );
                let session = match store.owner_sessions(owner) {
                    Ok(sessions) => sessions.into_iter().next(),
                    Err(error) => {
                        tracing::warn!(%error, %owner, "cannot find the Session to reopen; judging again later");
                        continue;
                    }
                };
                let Some(session) = session else {
                    continue;
                };
                let successor = crate::os_process::resolve_lf_binary().ok().map(|entry| {
                    vec![
                        entry.to_string_lossy().into_owned(),
                        "session".into(),
                        "resume".into(),
                        session.clone(),
                    ]
                });
                Response::Moved {
                    notice: Some(format!(
                        "This Session's lf was {change}; reopening it as `lf resume {session}`"
                    )),
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
        return;
    }
}

/// The process ledger, opened when first needed and again after a failure.
#[derive(Default)]
struct Store {
    open: Option<SqliteStore>,
    failed: Option<String>,
}

impl Store {
    fn open(&mut self, owner: &LfProcessId) -> Option<&SqliteStore> {
        if self.open.is_none() {
            match crate::journal::process_ledger() {
                Ok(store) => {
                    self.open = Some(store);
                    self.failed = None;
                }
                Err(error) => {
                    let text = error.to_string();
                    // The same failure every poll is one line, not a flood.
                    if self.failed.as_ref() != Some(&text) {
                        tracing::warn!(%error, %owner, "Session owner cannot read the store; judging again later");
                        self.failed = Some(text);
                    }
                }
            }
        }
        self.open.as_ref()
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

/// The detached limit configured for `repo`. An unreadable configuration
/// keeps the default rather than keeping the Session.
fn limit(repo: Option<&str>) -> DetachedLimit {
    match crate::config::load_config(repo.map(Path::new)) {
        Ok(config) => config.unwrap_or_default().session.end_detached_after,
        Err(error) => {
            tracing::warn!(%error, ?repo, "configuration unreadable; using the default detached limit");
            DetachedLimit::default()
        }
    }
}

#[cfg(test)]
mod tests {
    use std::time::{Duration, SystemTime};

    use super::{Activity, DetachedLimit, Grace};

    #[test]
    fn the_grace_runs_from_the_last_viewer_or_output_in_wall_time() {
        let start = SystemTime::UNIX_EPOCH + Duration::from_secs(1_000);
        let mut grace = Grace {
            limit: Some(DetachedLimit::After(Duration::from_secs(120))),
            viewed: false,
            quiet_since: start,
        };
        assert!(!grace.expired(start + Duration::from_secs(119)));
        // A night asleep counts: the clock, not uptime, decides.
        assert!(grace.expired(start + Duration::from_secs(8 * 3_600)));
        grace.note(Activity::Output, start + Duration::from_secs(100));
        assert!(!grace.expired(start + Duration::from_secs(200)));
        assert!(grace.expired(start + Duration::from_secs(220)));
        grace.note(Activity::Viewed(true), start + Duration::from_secs(300));
        assert!(!grace.expired(start + Duration::from_secs(1_000)));
        grace.note(Activity::Viewed(false), start + Duration::from_secs(400));
        assert!(!grace.expired(start + Duration::from_secs(519)));
        assert!(grace.expired(start + Duration::from_secs(520)));
        grace.limit = Some(DetachedLimit::Off);
        assert!(!grace.expired(start + Duration::from_secs(1_000)));
        grace.limit = None;
        assert!(!grace.expired(start + Duration::from_secs(1_000)));
    }
}
