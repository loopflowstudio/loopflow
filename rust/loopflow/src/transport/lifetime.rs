//! When a Session owner ends itself.
//!
//! A terminal owner nobody views ends after `session.end_detached_after` of
//! quiet: no viewer, no output from its command, and nothing in the store
//! holding it (a turn in progress, a running Graph, a headless Run). An owner
//! whose lf executable is removed or replaced (a worktree deleted, a build
//! relinked, Dev.app rebuilt) ends as soon as its Sessions wait for a person,
//! viewers notwithstanding: it would otherwise run a vanished build
//! indefinitely. Its attached viewers are told why and how to return. Ending
//! writes the last screen, stops the agent and records the exit, so `lf resume`
//! reopens the conversation from provider history and `lf resume --all` leaves
//! it alone.
//!
//! The clock is wall time: a laptop asleep overnight ends its idle Sessions at
//! the first poll after it wakes. A store that cannot be read is judged again
//! next time; the policy never stops on an error.

use std::os::unix::fs::MetadataExt;
use std::path::{Path, PathBuf};
use std::sync::mpsc;
use std::time::{Duration, SystemTime};

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
) -> (mpsc::Sender<Activity>, mpsc::Receiver<Option<String>>) {
    let executable = Executable::current();
    let (activity, changes) = mpsc::channel();
    let (end, ending) = mpsc::channel();
    std::thread::spawn(move || {
        watch(&owner, executable.as_ref(), terminal, &changes, &end);
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

    fn due(&self) -> Option<SystemTime> {
        match self.limit {
            Some(DetachedLimit::After(limit)) if !self.viewed => Some(self.quiet_since + limit),
            _ => None,
        }
    }

    fn expired(&self, now: SystemTime) -> bool {
        self.due().is_some_and(|due| due <= now)
    }
}

fn watch(
    owner: &LfProcessId,
    executable: Option<&Executable>,
    terminal: bool,
    changes: &mpsc::Receiver<Activity>,
    end: &mpsc::Sender<Option<String>>,
) {
    let mut store = Store::default();
    let mut grace = Grace {
        limit: None,
        viewed: false,
        quiet_since: SystemTime::now(),
    };
    loop {
        match changes.recv_timeout(POLL) {
            Ok(activity) => {
                grace.note(activity, SystemTime::now());
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
        let gone = executable.and_then(|executable| executable.gone().map(|how| (executable, how)));
        if gone.is_none() && !grace.expired(now) {
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
        let hint = match gone {
            Some((executable, how)) => {
                tracing::warn!(
                    %owner,
                    path = %executable.path.display(),
                    "ending a Session whose lf executable was {how}"
                );
                store
                    .owner_sessions(owner)
                    .ok()
                    .and_then(|sessions| sessions.into_iter().next())
                    .map(|session| {
                        format!(
                            "This Session's lf build was {how}; `lf resume {session}` reopens it."
                        )
                    })
            }
            None => {
                tracing::info!(%owner, "ending a Session nobody has viewed");
                None
            }
        };
        if end.send(hint).is_err() {
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

impl Executable {
    /// `None` when the running executable cannot be read; that owner keeps only
    /// the detached limit.
    fn current() -> Option<Self> {
        let read = || -> std::io::Result<Self> {
            let path = std::fs::canonicalize(std::env::current_exe()?)?;
            let identity = identity(&std::fs::metadata(&path)?);
            Ok(Self { path, identity })
        };
        read()
            .inspect_err(
                |error| tracing::warn!(%error, "Session owner cannot watch its executable"),
            )
            .ok()
    }

    /// `removed` or `replaced` once the file at the path is no longer this one.
    /// Any other read error is transient and judged again next time.
    fn gone(&self) -> Option<&'static str> {
        match std::fs::metadata(&self.path) {
            Ok(metadata) => (identity(&metadata) != self.identity).then_some("replaced"),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => Some("removed"),
            Err(_) => None,
        }
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
        assert_eq!(grace.due(), None);
        grace.note(Activity::Viewed(false), start + Duration::from_secs(400));
        assert_eq!(grace.due(), Some(start + Duration::from_secs(520)));
        grace.limit = Some(DetachedLimit::Off);
        assert_eq!(grace.due(), None);
        grace.limit = None;
        assert_eq!(grace.due(), None);
    }
}
