//! Record viewing off the byte path; only an explicit shell Close is polled.
use std::io::Write;
use std::os::unix::net::UnixStream;
use std::sync::mpsc;
use std::time::Duration;

use crate::id::LfProcessId;
use crate::store::sqlite::ViewerHold;
use crate::store::RecordingGap;

const POLL: Duration = Duration::from_secs(2);

pub(super) fn spawn(
    owner: LfProcessId,
    shell: bool,
    waker: UnixStream,
) -> (mpsc::Sender<Option<ViewerHold>>, mpsc::Receiver<()>) {
    let (viewing, changes) = mpsc::channel();
    let (close, closing) = mpsc::channel();
    std::thread::spawn(move || watch(&owner, shell, &changes, &close, &waker));
    (viewing, closing)
}

fn watch(
    owner: &LfProcessId,
    shell: bool,
    changes: &mpsc::Receiver<Option<ViewerHold>>,
    close: &mpsc::Sender<()>,
    mut waker: &UnixStream,
) {
    let mut store = None;
    let mut store_failure = RecordingGap::default();
    let mut recorded_hold = None;
    let mut hold_failure = RecordingGap::default();
    let mut close_failure = RecordingGap::default();
    let mut hold = None;
    loop {
        // Once viewing is recorded, a conversation needs no timer. Only failed
        // writes and explicit shell Close need another attempt without a change.
        let change = if shell || recorded_hold != Some(hold) {
            changes.recv_timeout(POLL)
        } else {
            changes
                .recv()
                .map_err(|_| mpsc::RecvTimeoutError::Disconnected)
        };
        match change {
            Ok(changed) => hold = changed,
            Err(mpsc::RecvTimeoutError::Timeout) => {}
            Err(mpsc::RecvTimeoutError::Disconnected) => return,
        }
        for changed in changes.try_iter() {
            hold = changed;
        }
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
        if recorded_hold != Some(hold)
            && report_result(
                &mut hold_failure,
                "Session viewer hold",
                store.record_owner_viewer_hold(owner, hold),
            )
            .is_some()
        {
            recorded_hold = Some(hold);
        }
        if shell
            && report_result(
                &mut close_failure,
                "Shell Close",
                store.shell_should_close(owner),
            ) == Some(true)
        {
            let _ = close.send(());
            let _ = waker.write_all(&[1]);
            return;
        }
    }
}

fn report_result<T>(
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
    use crate::store::sqlite::ViewerHold;
    use std::time::{Duration, Instant};

    #[test]
    fn viewer_state_retries_after_a_store_failure_without_another_viewer_change() {
        let guard = crate::journal::TestLedgerGuard::new();
        let store = crate::journal::process_ledger().unwrap();
        let owner = store.test_lf_process();
        let db = guard.home().join("loopflow.db");
        let conn = rusqlite::Connection::open(&db).unwrap();
        conn.execute_batch(
            "CREATE TRIGGER reject_viewer_change BEFORE UPDATE OF viewed ON processes
            BEGIN SELECT RAISE(FAIL, 'temporary viewer write failure'); END;",
        )
        .unwrap();
        let (activity, changes) = std::sync::mpsc::channel();
        let (close, closing) = std::sync::mpsc::channel();
        let (_wake, waker) = std::os::unix::net::UnixStream::pair().unwrap();
        let watched = owner.clone();
        let recorder = std::thread::spawn(move || {
            crate::journal::with_test_ledger(db, || {
                super::watch(&watched, false, &changes, &close, &waker);
            })
        });
        activity.send(Some(ViewerHold::Viewed(123))).unwrap();
        std::thread::sleep(super::POLL * 2);
        let read = || {
            conn.query_row(
                "SELECT viewer_hold_at FROM processes WHERE id=?1",
                [&owner],
                |r| r.get::<_, Option<i64>>(0),
            )
            .unwrap()
        };
        assert_eq!(read(), None);
        conn.execute_batch("DROP TRIGGER reject_viewer_change")
            .unwrap();
        let deadline = Instant::now() + super::POLL * 3;
        while read().is_none() && Instant::now() < deadline {
            std::thread::sleep(Duration::from_millis(20));
        }
        assert_eq!(read(), Some(123));
        // After recovery the recorder waits for changes; departure must wake it.
        activity.send(Some(ViewerHold::Absent(124))).unwrap();
        let deadline = Instant::now() + super::POLL * 3;
        while read() != Some(124) && Instant::now() < deadline {
            std::thread::sleep(Duration::from_millis(20));
        }
        drop(activity);
        recorder.join().unwrap();
        assert_eq!(read(), Some(124));
        assert!(closing.try_recv().is_err());
    }
}
