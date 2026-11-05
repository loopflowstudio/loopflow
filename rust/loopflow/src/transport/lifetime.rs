//! When a Session owner ends itself.
//!
//! A terminal owner nobody views ends after `session.end_detached_after` of
//! quiet: no viewer, no output from its command, and nothing in the store
//! holding it (a turn, pending request, running Graph or headless Run). Nothing
//! else ends an owner on its own: a removed, rebuilt or superseded lf executable
//! leaves it serving (Jack Heart, 2026-11-05), and `lf resume` or
//! `lf monitor rollover` move it when a person or operator decides to. Ending
//! writes the last screen, stops the agent and records the exit, so `lf resume`
//! reopens the conversation from provider history.
//!
//! The clock is wall time: a laptop asleep overnight ends its idle Sessions at
//! the first poll after it wakes. A store that cannot be read is judged again
//! next time; the policy never stops on an error.

use std::io::Write;
use std::os::unix::net::UnixStream;
use std::path::Path;
use std::sync::mpsc;
use std::time::{Duration, Instant, SystemTime};

use anyhow::Result;

use crate::config::DetachedLimit;
use crate::id::LfProcessId;
use crate::store::sqlite::sessions::OwnerRetention;
use crate::store::RecordingGap;

/// How often an owner looks at the clock.
pub(super) const POLL: Duration = Duration::from_secs(2);

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

/// An ending candidate carries the activity the policy actually judged when
/// the detached grace expired.
#[derive(Debug)]
pub(super) struct Ending {
    pub activity: Activity,
}

impl Ending {
    pub fn may_end(&self, activity: Activity, input_queued: bool) -> bool {
        !input_queued
            && activity.output == self.activity.output
            && !activity.viewed
            && activity.viewers == self.activity.viewers
    }

    /// Recheck the serving owner's current evidence before preparing its exit.
    pub fn confirm(
        &self,
        owner: &LfProcessId,
        activity: Activity,
        input_queued: bool,
    ) -> Result<bool> {
        if !self.may_end(activity, input_queued) {
            return Ok(false);
        }
        let store = crate::journal::process_ledger()?;
        if store.owner_retention(owner)?.is_some() {
            return Ok(false);
        }
        tracing::info!(%owner, "ending a Session nobody has viewed");
        Ok(true)
    }
}

/// Record activity sent on the first channel. Once the owner has gone unviewed
/// and quiet past its limit, send the second channel a candidate for the serve
/// loop to confirm before ending the owner. Only a terminal owner (`terminal`)
/// ends for want of viewers; a pipe owner is a headless Run.
pub(super) fn spawn(
    owner: LfProcessId,
    terminal: bool,
    waker: UnixStream,
) -> (mpsc::Sender<Activity>, mpsc::Receiver<Ending>) {
    #[cfg(test)]
    if let Some(channels) = super::lifetime_tests::channels() {
        return channels;
    }
    let (activity, changes) = mpsc::channel();
    let (end, ending) = mpsc::channel();
    std::thread::spawn(move || {
        watch(&owner, terminal, &changes, &end, &waker);
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
    terminal: bool,
    changes: &mpsc::Receiver<Activity>,
    end: &mpsc::Sender<Ending>,
    mut waker: &UnixStream,
) {
    let mut store = None;
    let mut store_failure = RecordingGap::default();
    let mut recorded_viewed = None;
    let mut viewer_failure = RecordingGap::default();
    let mut repository_failure = RecordingGap::default();
    let mut retention_failure = RecordingGap::default();
    let mut retained: Option<OwnerRetention> = None;
    let mut next_poll = Instant::now() + POLL;
    let mut grace = Grace {
        limit: None,
        activity: Activity::default(),
        quiet_since: SystemTime::now(),
    };
    loop {
        match changes.recv_timeout(next_poll.saturating_duration_since(Instant::now())) {
            Ok(activity) => grace.note(activity, SystemTime::now()),
            Err(mpsc::RecvTimeoutError::Timeout) => {}
            Err(mpsc::RecvTimeoutError::Disconnected) => return,
        }
        // Activity updates the clock immediately but never accelerates file or
        // store work. Output can arrive much faster than the policy's poll.
        if Instant::now() < next_poll {
            continue;
        }
        next_poll = Instant::now() + POLL;
        if store.is_none() {
            store = report_result(
                &mut store_failure,
                "Session store",
                crate::journal::process_ledger(),
            );
        }
        let Some(store) = store.as_ref() else {
            continue;
        };
        let viewed = grace.activity.viewed;
        if recorded_viewed != Some(viewed)
            && report_result(
                &mut viewer_failure,
                "Session viewers",
                store.record_owner_viewers(owner, viewed),
            )
            .is_some()
        {
            recorded_viewed = Some(viewed);
        }
        if terminal && grace.limit.is_none() {
            grace.limit = report_result(
                &mut repository_failure,
                "Session repository",
                store.process(owner),
            )
            .flatten()
            .map(|process| limit(process.repo.as_deref()));
        }
        let now = SystemTime::now();
        if !grace.expired(now) {
            continue;
        }
        let Some(retention) = report_result(
            &mut retention_failure,
            "Session retention, retrying",
            store.owner_retention(owner),
        ) else {
            continue;
        };
        if retained != retention {
            if let Some(reason) = retention {
                tracing::info!(%owner, "retaining detached Session past its limit: {reason}");
            }
            retained = retention;
        }
        // Retained work counts as activity: the grace runs again from here.
        if retention.is_some() {
            grace.quiet_since = now;
            continue;
        }
        if end
            .send(Ending {
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

/// Each operation keeps its own gap until that operation succeeds. Other policy
/// work can succeed during an outage without closing or reopening it.
pub(super) fn report_result<T>(
    gap: &mut RecordingGap,
    what: &str,
    result: Result<T, impl std::fmt::Display>,
) -> Option<T> {
    result
        .inspect(|_| gap.recovered())
        .inspect_err(|error| gap.note(what, error))
        .ok()
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

    use super::{Activity, DetachedLimit, Ending, Grace};

    #[test]
    fn queued_endings_require_current_output_and_idle_viewer_evidence() {
        let mut activity = Activity::default();
        let candidate = Ending { activity };
        assert!(candidate.may_end(activity, false));
        activity.view(true);
        assert!(!candidate.may_end(activity, false));
        activity.view(false);
        // Attaching and leaving again still restarts the idle grace.
        assert!(!candidate.may_end(activity, false));
        activity.output += 1;
        assert!(!candidate.may_end(activity, false));
        let fresh = Ending { activity };
        assert!(fresh.may_end(activity, false));
        assert!(!fresh.may_end(activity, true));
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
                super::watch(&watched, false, &changes, &end, &waker);
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
    fn output_restarts_the_grace_then_quiet_allows_ending() {
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
        // An unreadable Process must not latch the global grace in place of
        // the repository's setting. Repair it without activity.
        conn.execute(
            "UPDATE processes SET trace_id='unreadable' WHERE id=?1",
            [&owner],
        )
        .unwrap();
        let (activity, changes) = std::sync::mpsc::channel();
        let (end, ending) = std::sync::mpsc::channel();
        let (_wake, waker) = std::os::unix::net::UnixStream::pair().unwrap();
        let watched = owner.clone();
        let policy = std::thread::spawn(move || {
            crate::journal::with_test_ledger(db, || {
                super::watch(&watched, true, &changes, &end, &waker);
            });
        });
        assert!(matches!(
            ending.recv_timeout(super::POLL * 2),
            Err(std::sync::mpsc::RecvTimeoutError::Timeout)
        ));
        conn.execute("UPDATE processes SET trace_id=?1 WHERE id=?1", [&owner])
            .unwrap();
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
