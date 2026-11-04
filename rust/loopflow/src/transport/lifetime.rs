//! When a Session owner ends itself.
//!
//! A terminal owner nobody views ends after `session.end_detached_after` of
//! quiet: no viewer, no output from its command, and nothing in the store
//! holding it (a turn, pending request, running Graph or headless Run). An owner
//! whose lf executable is removed, replaced or superseded (a worktree deleted,
//! a build relinked, Dev.app rebuilt, another installed build selected) ends once
//! its Sessions wait for a person and its command has been quiet for one poll,
//! even while viewed. Its attached viewers
//! are told why and follow the resume command through the lf entry. Ending
//! writes the last screen, stops the agent and records the exit, so `lf resume`
//! reopens the conversation from provider history.
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

use anyhow::Result;

use super::wire::Event;
use crate::config::DetachedLimit;
use crate::id::LfProcessId;
use crate::store::sqlite::SqliteStore;

/// How often an owner looks at the clock and its executable.
const POLL: Duration = Duration::from_secs(2);

/// Activity accepted by the serving owner. Revisions include changes whose
/// notification is still queued or throttled on its way to the policy.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub(super) struct Activity {
    pub viewed: bool,
    pub viewers: u64,
    pub output: u64,
}

impl Activity {
    pub fn view(&mut self, viewed: bool) -> bool {
        if self.viewed == viewed {
            return false;
        }
        self.viewed = viewed;
        self.viewers += 1;
        true
    }
}

/// An ending candidate carries the activity the policy actually judged.
#[derive(Debug)]
pub(super) struct Ending {
    /// None means the detached grace expired.
    pub change: Option<(Change, PathBuf)>,
    pub activity: Activity,
}

impl Ending {
    pub fn may_end(&self, activity: Activity, input_queued: bool) -> bool {
        !input_queued
            && activity.output == self.activity.output
            && (self.change.is_some()
                || (!activity.viewed && activity.viewers == self.activity.viewers))
    }

    /// Recheck the serving owner's current evidence before preparing its exit.
    pub fn confirm(
        &self,
        owner: &LfProcessId,
        activity: Activity,
        input_queued: bool,
    ) -> Result<Option<Event>> {
        if !self.may_end(activity, input_queued) {
            return Ok(None);
        }
        let store = crate::journal::process_ledger()?;
        if !store.owner_waits_for_person(owner)? {
            return Ok(None);
        }
        let (notice, successor) = match &self.change {
            Some((change, path)) => {
                let Some(session) = store.owner_sessions(owner)?.into_iter().next() else {
                    return Ok(None);
                };
                let successor = crate::os_process::resolve_lf_binary().ok().map(|entry| {
                    vec![
                        entry.to_string_lossy().into_owned(),
                        "session".into(),
                        "resume".into(),
                        session.clone(),
                    ]
                });
                tracing::warn!(%owner, ?path, "ending a Session whose lf was {change}");
                (
                    Some(format!(
                        "This Session's lf was {change}; reopening it as `lf resume {session}`"
                    )),
                    successor,
                )
            }
            None => {
                tracing::info!(%owner, "ending a Session nobody has viewed");
                (None, None)
            }
        };
        Ok(Some(Event::Control {
            held: false,
            notice,
            successor,
        }))
    }
}

/// Record activity sent on the first channel. Once the executable is gone or
/// the owner has gone unviewed and quiet past its limit, send the second
/// channel a candidate for the serve loop to confirm before ending the owner.
/// Only a terminal owner (`terminal`) ends for want of viewers;
/// a pipe owner is a headless Run.
pub(super) fn spawn(
    owner: LfProcessId,
    terminal: bool,
    waker: UnixStream,
) -> (mpsc::Sender<Activity>, mpsc::Receiver<Ending>) {
    #[cfg(test)]
    if let Some(channels) = super::lifetime_tests::channels() {
        return channels;
    }
    let executable = Executable::current();
    let (activity, changes) = mpsc::channel();
    let (end, ending) = mpsc::channel();
    std::thread::spawn(move || {
        watch(
            &owner,
            executable.as_ref(),
            terminal,
            &changes,
            &end,
            &waker,
        );
    });
    (activity, ending)
}

/// When an unviewed, quiet owner is due to end, in wall time.
#[derive(Debug)]
struct Grace {
    /// Unknown until the store names the owner's repository; a pipe owner
    /// never learns one and never ends for want of viewers.
    limit: Option<DetachedLimit>,
    activity: Activity,
    quiet_since: SystemTime,
}

impl Grace {
    fn note(&mut self, activity: Activity, now: SystemTime) {
        if activity != self.activity {
            self.quiet_since = now;
            self.activity = activity;
        }
    }

    fn expired(&self, now: SystemTime) -> bool {
        match self.limit {
            Some(DetachedLimit::After(limit)) if !self.activity.viewed => {
                now >= self.quiet_since + limit
            }
            _ => false,
        }
    }
}

fn watch(
    owner: &LfProcessId,
    executable: Option<&Executable>,
    terminal: bool,
    changes: &mpsc::Receiver<Activity>,
    end: &mpsc::Sender<Ending>,
    mut waker: &UnixStream,
) {
    let mut store = Store::default();
    let mut recorded_viewed = None;
    let mut last_output = SystemTime::now();
    let mut grace = Grace {
        limit: None,
        activity: Activity::default(),
        quiet_since: last_output,
    };
    loop {
        match changes.recv_timeout(POLL) {
            Ok(activity) => {
                let now = SystemTime::now();
                if activity.output != grace.activity.output {
                    last_output = now;
                }
                grace.note(activity, now);
            }
            Err(mpsc::RecvTimeoutError::Timeout) => {}
            Err(mpsc::RecvTimeoutError::Disconnected) => return,
        }
        let viewed = grace.activity.viewed;
        if recorded_viewed != Some(viewed) {
            if let Some(store) = store.open(owner) {
                match store.record_owner_viewers(owner, viewed) {
                    Ok(()) => recorded_viewed = Some(viewed),
                    Err(error) => {
                        tracing::warn!(%error, %owner, viewed, "cannot record Session viewers; retrying");
                    }
                }
            }
        }
        if terminal && grace.limit.is_none() {
            if let Some(store) = store.open(owner) {
                match store.process(owner) {
                    Ok(Some(process)) => grace.limit = Some(limit(process.repo.as_deref())),
                    Ok(None) => {}
                    Err(error) => {
                        tracing::warn!(%error, %owner, "cannot load Session repository for detached limit; retrying");
                    }
                }
            }
        }
        let now = SystemTime::now();
        let change = executable.and_then(|executable| {
            executable
                .change()
                .map(|change| (change, executable.path.clone()))
        });
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
        if end
            .send(Ending {
                change,
                activity: grace.activity,
            })
            .is_err()
        {
            return;
        }
        let _ = waker.write_all(&[1]);
        // Input accepted since the judgment can veto it in the serving owner.
        // Keep judging until that owner exits or drops this channel.
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
pub(super) enum Change {
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

    use super::{Activity, Change, DetachedLimit, Ending, Grace};

    fn ending(idle: bool, activity: Activity) -> Ending {
        Ending {
            change: (!idle).then(|| (Change::Replaced, "replaced".into())),
            activity,
        }
    }

    #[test]
    fn queued_endings_require_current_output_and_idle_viewer_evidence() {
        for idle in [true, false] {
            let mut activity = Activity::default();
            let candidate = ending(idle, activity);
            assert!(candidate.may_end(activity, false));
            activity.view(true);
            assert_eq!(candidate.may_end(activity, false), !idle);
            activity.view(false);
            // Attaching and leaving again still restarts the idle grace.
            assert_eq!(candidate.may_end(activity, false), !idle);
            activity.output += 1;
            assert!(!candidate.may_end(activity, false));
            let fresh = ending(idle, activity);
            assert!(fresh.may_end(activity, false));
            assert!(!fresh.may_end(activity, true));
        }
    }

    #[test]
    fn viewer_state_retries_after_a_store_failure_without_another_viewer_change() {
        let guard = crate::journal::TestLedgerGuard::new();
        let store = crate::journal::process_ledger().unwrap();
        let owner = store.test_lf_process();
        let db = guard.home().join("loopflow.db");
        let conn = rusqlite::Connection::open(&db).unwrap();
        conn.execute(
            "UPDATE processes SET viewed=1 WHERE id=?1",
            [owner.as_str()],
        )
        .unwrap();
        conn.execute_batch(
            "CREATE TRIGGER reject_viewer_change BEFORE UPDATE OF viewed ON processes
             BEGIN SELECT RAISE(FAIL, 'temporary viewer write failure'); END;",
        )
        .unwrap();
        let (activity, changes) = std::sync::mpsc::channel();
        let (end, _ending) = std::sync::mpsc::channel();
        let (_wake, waker) = std::os::unix::net::UnixStream::pair().unwrap();
        let watched = owner.clone();
        let policy = std::thread::spawn(move || {
            crate::journal::with_test_ledger(db, || {
                super::watch(&watched, None, false, &changes, &end, &waker);
            });
        });
        activity
            .send(Activity {
                viewed: false,
                viewers: 2,
                output: 0,
            })
            .unwrap();
        std::thread::sleep(super::POLL * 2);
        conn.execute_batch("DROP TRIGGER reject_viewer_change")
            .unwrap();
        let deadline = std::time::Instant::now() + super::POLL * 3;
        let mut viewed = true;
        while viewed && std::time::Instant::now() < deadline {
            viewed = conn
                .query_row("SELECT viewed FROM processes WHERE id=?1", [&owner], |r| {
                    r.get(0)
                })
                .unwrap();
            std::thread::sleep(Duration::from_millis(20));
        }
        drop(activity);
        policy.join().unwrap();
        assert!(
            !viewed,
            "a recovered store must not retain stale viewer state"
        );
    }

    #[test]
    fn output_restarts_both_policies_then_quiet_allows_ending() {
        let guard = crate::journal::TestLedgerGuard::new();
        let store = crate::journal::process_ledger().unwrap();
        let owner = store.test_lf_process();
        let db = guard.home().join("loopflow.db");
        let conn = rusqlite::Connection::open(&db).unwrap();
        store.test_session("session", &crate::session_record::new_artifact_key());
        conn.execute(
            "UPDATE agent_sessions SET interactive=1, transport_lf_process_id=?1 WHERE id='session'",
            [owner.as_str()],
        ).unwrap();
        std::fs::write(
            guard.home().join("config.yaml"),
            "session:\n  end_detached_after: 2s\n",
        )
        .unwrap();
        for removed in [false, true] {
            if !removed {
                // An unreadable Process must not latch the global grace in
                // place of the repository's setting. Repair it without activity.
                conn.execute(
                    "UPDATE processes SET trace_id='unreadable' WHERE id=?1",
                    [&owner],
                )
                .unwrap();
            }
            let executable = removed.then(|| super::Executable {
                path: guard.home().join("removed"),
                identity: (0, 0, 0, 0, 0),
            });
            let (activity, changes) = std::sync::mpsc::channel();
            let (end, ending) = std::sync::mpsc::channel();
            let (_wake, waker) = std::os::unix::net::UnixStream::pair().unwrap();
            let watched = owner.clone();
            let db = db.clone();
            let policy = std::thread::spawn(move || {
                crate::journal::with_test_ledger(db, || {
                    super::watch(&watched, executable.as_ref(), true, &changes, &end, &waker);
                });
            });
            if !removed {
                assert!(matches!(
                    ending.recv_timeout(super::POLL * 2),
                    Err(std::sync::mpsc::RecvTimeoutError::Timeout)
                ));
                conn.execute("UPDATE processes SET trace_id=?1 WHERE id=?1", [&owner])
                    .unwrap();
            }
            let candidate = ending.recv_timeout(Duration::from_secs(8)).unwrap();
            let observed = Activity {
                output: 1,
                ..Activity::default()
            };
            assert!(!candidate.may_end(observed, false));
            activity.send(observed).unwrap();
            assert!(matches!(
                ending.recv_timeout(Duration::from_secs(1)),
                Err(std::sync::mpsc::RecvTimeoutError::Timeout)
            ));
            let fresh = ending.recv_timeout(Duration::from_secs(8)).unwrap();
            assert!(fresh.may_end(observed, false));
            drop(activity);
            policy.join().unwrap();
        }
    }

    #[test]
    fn the_grace_runs_from_the_last_viewer_or_output_in_wall_time() {
        let start = SystemTime::UNIX_EPOCH + Duration::from_secs(1_000);
        let mut grace = Grace {
            limit: Some(DetachedLimit::After(Duration::from_secs(120))),
            activity: Activity::default(),
            quiet_since: start,
        };
        assert!(!grace.expired(start + Duration::from_secs(119)));
        // A night asleep counts: the clock, not uptime, decides.
        assert!(grace.expired(start + Duration::from_secs(8 * 3_600)));
        let mut activity = Activity {
            output: 1,
            ..Activity::default()
        };
        grace.note(activity, start + Duration::from_secs(100));
        assert!(!grace.expired(start + Duration::from_secs(200)));
        assert!(grace.expired(start + Duration::from_secs(220)));
        activity.view(true);
        grace.note(activity, start + Duration::from_secs(300));
        assert!(!grace.expired(start + Duration::from_secs(1_000)));
        activity.view(false);
        grace.note(activity, start + Duration::from_secs(400));
        assert!(!grace.expired(start + Duration::from_secs(519)));
        assert!(grace.expired(start + Duration::from_secs(520)));
        // A veto can repeat an already reported snapshot without extending quiet.
        grace.note(activity, start + Duration::from_secs(519));
        assert!(grace.expired(start + Duration::from_secs(520)));
        grace.limit = Some(DetachedLimit::Off);
        assert!(!grace.expired(start + Duration::from_secs(1_000)));
        grace.limit = None;
        assert!(!grace.expired(start + Duration::from_secs(1_000)));
    }
}
