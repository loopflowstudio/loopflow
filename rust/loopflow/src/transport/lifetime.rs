//! Record viewing off the byte path; only an explicit shell Close and an
//! exec'd native client's attachment are polled.
use std::io::Write;
use std::os::unix::net::UnixStream;
use std::sync::{mpsc, Arc, Mutex};
use std::time::Duration;

use anyhow::Result;

use crate::harness::native_exit::{NativeExit, Settled};
use crate::id::LfProcessId;
use crate::process::SessionAttachment;
use crate::store::sqlite::{SqliteStore, ViewerHold};
use crate::store::RecordingGap;

const POLL: Duration = Duration::from_secs(2);

/// The exec'd native client whose attachment the existing lifecycle worker
/// checks. Store reads never run in the terminal's descriptor loop.
#[derive(Debug, Default)]
pub(super) struct Client(Mutex<ClientState>);

#[derive(Debug, Default)]
struct ClientState {
    attached: Option<(String, SessionAttachment)>,
    departure: Option<Settled>,
}

impl Client {
    pub fn set(&self, record: Option<&NativeExit>) {
        let mut state = self.0.lock().expect("client attachment lock poisoned");
        state.attached = record
            .filter(|record| record.client)
            .map(|record| (record.session.clone(), record.attachment.clone()));
        state.departure = None;
    }

    pub fn take_departure(&self) -> Option<Settled> {
        self.0
            .lock()
            .expect("client attachment lock poisoned")
            .departure
            .take()
    }

    fn watching(&self) -> bool {
        self.0
            .lock()
            .expect("client attachment lock poisoned")
            .attached
            .is_some()
    }

    fn check(&self, store: &SqliteStore) -> Result<bool> {
        let attached = self
            .0
            .lock()
            .expect("client attachment lock poisoned")
            .attached
            .clone();
        let Some((session, expected)) = &attached else {
            return Ok(false);
        };
        if store.session_attachment(session)?.as_ref() == Some(expected) {
            return Ok(false);
        }
        let notice = store
            .attachment_departure(session, expected)?
            .map(|departure| crate::lf::commands::util::departure_notice(store, session, departure))
            .transpose()?;
        let mut state = self.0.lock().expect("client attachment lock poisoned");
        if state.attached != attached {
            return Ok(false);
        }
        state.attached = None;
        // Keep the known move even if recording the exit later takes too long.
        state.departure = Some(Settled {
            code: 0,
            lines: notice.into_iter().collect(),
        });
        Ok(true)
    }
}

pub(super) fn spawn(
    owner: LfProcessId,
    shell: bool,
    waker: UnixStream,
    client: Arc<Client>,
) -> (mpsc::Sender<Option<ViewerHold>>, mpsc::Receiver<()>) {
    let (viewing, changes) = mpsc::channel();
    let (close, closing) = mpsc::channel();
    std::thread::spawn(move || watch(&owner, shell, &changes, &close, &waker, &client));
    (viewing, closing)
}

fn watch(
    owner: &LfProcessId,
    shell: bool,
    changes: &mpsc::Receiver<Option<ViewerHold>>,
    close: &mpsc::Sender<()>,
    mut waker: &UnixStream,
    client: &Client,
) {
    let mut store = None;
    let mut store_failure = RecordingGap::default();
    let mut recorded_hold = None;
    let mut hold_failure = RecordingGap::default();
    let mut close_failure = RecordingGap::default();
    let mut client_failure = RecordingGap::default();
    let mut hold = None;
    loop {
        // Once viewing is recorded, a conversation needs no timer. Only failed
        // writes, explicit shell Close and an attached native client need
        // another look without a change.
        let change = if shell || recorded_hold != Some(hold) || client.watching() {
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
        if report_result(
            &mut client_failure,
            "native client attachment",
            client.check(store),
        ) == Some(true)
        {
            let _ = waker.write_all(&[1]);
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
    use super::{Client, ClientState};
    use crate::store::sqlite::ViewerHold;
    use std::time::{Duration, Instant};

    #[test]
    fn client_retirement_waits_for_a_readable_store_and_keeps_its_move_notice() {
        let home = tempfile::tempdir().unwrap();
        let store =
            crate::store::sqlite::SqliteStore::open_ephemeral(&home.path().join("db")).unwrap();
        let session = store.test_session("moving", &crate::session_record::new_artifact_key());
        let attachment = store
            .claim_session_attachment(&session.id, None, &store.test_lf_process(), true)
            .unwrap();
        let client = Client(std::sync::Mutex::new(ClientState {
            attached: Some((session.id.to_string(), attachment.clone())),
            departure: None,
        }));
        assert!(!client.check(&store).unwrap());
        store
            .conn
            .lock()
            .unwrap()
            .execute_batch("ALTER TABLE agent_sessions RENAME TO agent_sessions_unavailable")
            .unwrap();
        assert!(client.check(&store).is_err());
        assert!(client.take_departure().is_none());
        store
            .conn
            .lock()
            .unwrap()
            .execute_batch("ALTER TABLE agent_sessions_unavailable RENAME TO agent_sessions")
            .unwrap();
        store
            .finish_session_attachment(
                &session.id,
                &attachment,
                "completed",
                Some(&crate::process::Departure::Terminal {
                    to: Some("tmux %9".into()),
                }),
                || Ok(true),
            )
            .unwrap();
        assert!(client.check(&store).unwrap());
        let moved = client.take_departure().unwrap();
        assert_eq!(moved.code, 0);
        assert!(moved.lines[0].contains("moved to tmux %9"));
        assert!(!client.check(&store).unwrap());
        assert!(client.take_departure().is_none());
    }

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
                super::watch(
                    &watched,
                    false,
                    &changes,
                    &close,
                    &waker,
                    &super::Client::default(),
                );
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
