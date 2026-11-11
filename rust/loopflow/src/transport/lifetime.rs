//! Owners record viewing and obey close requests. Task cleanup owns eligibility.
//! Store failures never authorize ending; byte forwarding remains independent.

use std::io::Write;
use std::os::unix::net::UnixStream;
use std::sync::mpsc;
use std::time::{Duration, Instant};

use anyhow::Result;

use super::wire::Event;
use crate::id::LfProcessId;
use crate::store::sqlite::owner_close::CloseRequest;
use crate::store::sqlite::ViewerHold;
use crate::store::RecordingGap;

pub(super) const POLL: Duration = Duration::from_secs(2);

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub(super) struct Activity {
    /// Output can reveal a turn whose best-effort hook never reached the store.
    pub output: u64,
    pub hold: Option<ViewerHold>,
}

#[derive(Debug)]
pub(super) struct Ending {
    pub activity: Activity,
    pub request: CloseRequest,
}

impl Ending {
    pub fn consume(
        &self,
        owner: &LfProcessId,
        activity: Activity,
        input_queued: bool,
    ) -> Result<Option<Event>> {
        let store = crate::journal::process_ledger()?;
        if input_queued
            || (!self.request.is_shell() && matches!(activity.hold, Some(ViewerHold::Viewed(_))))
        {
            return Ok(None);
        }
        if self.request == CloseRequest::Completed && activity != self.activity {
            return Ok(None);
        }
        // Read only the request, so a return, reopen or new work can revoke a
        // queued close without another Task judgment in the serving owner.
        if store.owner_close_request(owner)? != Some(self.request) {
            return Ok(None);
        }
        Ok(Some(Event::Control {
            held: false,
            notice: Some(self.request.notice().into()),
        }))
    }
}

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
    std::thread::spawn(move || watch(&owner, terminal, &changes, &end, &waker));
    (activity, ending)
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
    let mut recorded_hold = None;
    let mut hold_failure = RecordingGap::default();
    let mut close_failure = RecordingGap::default();
    let mut cancel_failure = RecordingGap::default();
    let mut next_poll = Instant::now() + POLL;
    let mut activity = Activity::default();
    // Output after the last observed absence of a request may have arrived
    // after cleanup's decision, before this poll noticed the request.
    let mut before_request = Activity::default();
    loop {
        match changes.recv_timeout(next_poll.saturating_duration_since(Instant::now())) {
            Ok(changed) => activity = changed,
            Err(mpsc::RecvTimeoutError::Timeout) => {}
            Err(mpsc::RecvTimeoutError::Disconnected) => return,
        }
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
        if recorded_hold != Some(activity.hold)
            && report_result(
                &mut hold_failure,
                "Session viewer hold",
                store.record_owner_viewer_hold(owner, activity.hold),
            )
            .is_some()
        {
            recorded_hold = Some(activity.hold);
        }
        if recorded_hold != Some(activity.hold) {
            continue;
        }
        match report_result(
            &mut close_failure,
            "Owner close request",
            store.owner_close_request(owner),
        ) {
            Some(None) => before_request = activity,
            Some(Some(request)) => {
                if request == CloseRequest::Completed && activity != before_request {
                    // Cancellation can wait on a writer; keep it off the byte path.
                    if report_result(
                        &mut cancel_failure,
                        "Cancel owner close request",
                        store.cancel_owner_close(owner, request),
                    )
                    .is_some()
                    {
                        before_request = activity;
                    }
                    continue;
                }
                if request == CloseRequest::Abandoned && !terminal {
                    continue;
                }
                if end
                    .send(Ending {
                        activity: before_request,
                        request,
                    })
                    .is_err()
                {
                    return;
                }
                let _ = waker.write_all(&[1]);
            }
            None => {}
        }
    }
}

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

#[cfg(test)]
mod tests {
    use super::{Activity, Ending};
    use crate::store::sqlite::owner_close::CloseRequest;
    use std::time::Duration;

    #[test]
    fn output_before_the_owner_reads_a_close_request_still_revokes_it() {
        let guard = crate::journal::TestLedgerGuard::new();
        let store = crate::journal::process_ledger().unwrap();
        let owner = store.test_lf_process();
        let db = guard.home().join("loopflow.db");
        let conn = rusqlite::Connection::open(&db).unwrap();
        conn.execute(
            "UPDATE processes SET close_requested='completed' WHERE id=?1",
            [&owner],
        )
        .unwrap();
        let (activity, changes) = std::sync::mpsc::channel();
        let (end, ending) = std::sync::mpsc::channel();
        let (_wake, waker) = std::os::unix::net::UnixStream::pair().unwrap();
        let watched = owner.clone();
        let watcher = std::thread::spawn(move || {
            crate::journal::with_test_ledger(db, || {
                super::watch(&watched, true, &changes, &end, &waker);
            })
        });
        let output = Activity {
            output: 1,
            ..Activity::default()
        };
        activity.send(output).unwrap();
        let deadline = std::time::Instant::now() + super::POLL * 3;
        while store.owner_close_request(&owner).unwrap().is_some()
            && std::time::Instant::now() < deadline
        {
            std::thread::sleep(Duration::from_millis(20));
        }
        drop(activity);
        watcher.join().unwrap();
        assert!(ending.try_recv().is_err());
        assert_eq!(store.owner_close_request(&owner).unwrap(), None);
    }

    #[test]
    fn requests_wait_for_viewers_and_queued_input_and_can_be_revoked() {
        let _guard = crate::journal::TestLedgerGuard::new();
        let store = crate::journal::process_ledger().unwrap();
        let owner = store.test_lf_process();
        let conn = rusqlite::Connection::open(_guard.home().join("loopflow.db")).unwrap();
        for request in [
            CloseRequest::Completed,
            CloseRequest::Abandoned,
            CloseRequest::Shell,
            CloseRequest::RetiredShell,
        ] {
            conn.execute(
                "UPDATE processes SET close_requested=?2 WHERE id=?1",
                rusqlite::params![owner, request.as_str()],
            )
            .unwrap();
            let candidate = Ending {
                activity: Activity::default(),
                request,
            };
            assert!(candidate
                .consume(&owner, Activity::default(), true)
                .unwrap()
                .is_none());
            let viewed = Activity {
                hold: Some(crate::store::sqlite::ViewerHold::Viewed(1)),
                ..Activity::default()
            };
            assert_eq!(
                candidate.consume(&owner, viewed, false).unwrap().is_some(),
                request.is_shell()
            );
            assert!(candidate
                .consume(&owner, Activity::default(), false)
                .unwrap()
                .is_some());
            store.cancel_owner_close(&owner, request).unwrap();
            assert!(candidate
                .consume(&owner, Activity::default(), false)
                .unwrap()
                .is_none());
        }
        conn.execute(
            "UPDATE processes SET close_requested='completed' WHERE id=?1",
            [&owner],
        )
        .unwrap();
        conn.execute_batch("BEGIN IMMEDIATE").unwrap();
        let candidate = Ending {
            activity: Activity::default(),
            request: CloseRequest::Completed,
        };
        let output = Activity {
            output: 1,
            ..Activity::default()
        };
        assert!(candidate.consume(&owner, output, false).unwrap().is_none());
        conn.execute_batch("ROLLBACK").unwrap();
        conn.execute_batch("DROP TABLE processes").unwrap();
        let candidate = Ending {
            activity: Activity::default(),
            request: CloseRequest::Completed,
        };
        assert!(candidate
            .consume(&owner, Activity::default(), false)
            .is_err());
    }

    #[test]
    fn viewer_state_retries_after_a_store_failure_without_another_viewer_change() {
        let guard = crate::journal::TestLedgerGuard::new();
        let store = crate::journal::process_ledger().unwrap();
        let owner = store.test_lf_process();
        let db = guard.home().join("loopflow.db");
        let conn = rusqlite::Connection::open(&db).unwrap();
        let repo = store.ensure_repository("/repo").unwrap();
        let task = crate::durable::TaskId::new();
        conn.execute("INSERT INTO tasks(id,repository_id,issue_identifier,created_at,planning_completed,planning_completed_at)
            VALUES(?1,?2,'RETRY-1',1,1,'2000-01-01T00:00:00Z')", rusqlite::params![task.as_str(),repo.as_str()]).unwrap();
        store.test_session("session", &crate::session_record::new_artifact_key());
        conn.execute("UPDATE agent_sessions SET task_id=?1,interactive=1,transport_lf_process_id=?2 WHERE id='session'",
            rusqlite::params![task.as_str(),owner]).unwrap();
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
        conn.execute(
            "UPDATE processes SET close_requested='completed' WHERE id=?1",
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
        activity
            .send(Activity {
                output: 0,
                hold: Some(crate::store::sqlite::ViewerHold::Absent(0)),
            })
            .unwrap();
        std::thread::sleep(super::POLL * 2);
        assert!(
            ending.try_recv().is_err(),
            "failed viewing writes cannot authorize closure"
        );
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
        // Recovery records the previously missed viewing change and revokes
        // the stale close. Cleanup, not the owner, requests closure again.
        let deadline = std::time::Instant::now() + super::POLL * 3;
        while store.owner_close_request(&owner).unwrap().is_some()
            && std::time::Instant::now() < deadline
        {
            std::thread::sleep(Duration::from_millis(20));
        }
        assert_eq!(store.owner_close_request(&owner).unwrap(), None);
        crate::ops::task::retirement::request_conversation_closes(&store, &task).unwrap();
        let recovered = ending.recv_timeout(super::POLL * 2);
        drop(activity);
        policy.join().unwrap();
        assert!(
            recovered.is_ok(),
            "cleanup can request closure again after recording recovers"
        );
        assert!(
            !viewed,
            "a recovered store must not retain stale viewer state"
        );
    }
}
