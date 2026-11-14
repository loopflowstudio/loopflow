//! Record viewing off the byte path; only an explicit shell Close is polled.
//!
//! Nothing here ends a conversation for want of viewers. Task cleanup ends a
//! done or abandoned Task's conversations (`ops::task::retirement`); a Desktop
//! shell ends on its explicit Close.
use std::collections::HashMap;
use std::io::Write;
use std::os::unix::net::UnixStream;
use std::sync::{mpsc, Arc, Mutex};
use std::time::Duration;

use anyhow::Result;

use super::Host;
use crate::harness::native_exit::{NativeExit, Settled};
use crate::process::SessionAttachment;
use crate::store::sqlite::{SqliteStore, ViewerHold};
use crate::store::RecordingGap;

/// How often the recorder retries a failed write and rechecks shell Close and
/// native clients while nothing changes.
#[cfg(not(test))]
const POLL: Duration = Duration::from_secs(2);
#[cfg(test)]
const POLL: Duration = Duration::from_millis(100);

/// The exec'd native client whose attachment the watcher checks. Store reads
/// never run in the terminal's descriptor loop.
#[derive(Debug, Default)]
pub(super) struct Client(Mutex<ClientState>);

#[derive(Debug, Default)]
struct ClientState {
    attached: Option<(String, SessionAttachment)>,
    departure: Option<Settled>,
    /// Wakes the watcher's wait, which blocks while nothing is attached.
    wake: Option<mpsc::Sender<Message>>,
}

impl Client {
    pub fn set(&self, record: Option<&NativeExit>) {
        let mut state = self.0.lock().expect("client attachment lock poisoned");
        state.attached = record
            .filter(|record| record.client)
            .map(|record| (record.session.clone(), record.attachment.clone()));
        state.departure = None;
        if let (Some(_), Some(wake)) = (&state.attached, &state.wake) {
            let _ = wake.send(Message::Wake);
        }
    }

    pub fn take_departure(&self) -> Option<Settled> {
        self.0
            .lock()
            .expect("client attachment lock poisoned")
            .departure
            .take()
    }

    fn attached(&self) -> bool {
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

/// One thread recording viewing for every execution a host serves: the
/// Machine server's held terminals share one, a Session owner process has its
/// own. It ends once its handle and every watched execution are gone.
#[derive(Debug)]
pub(super) struct Watcher {
    messages: mpsc::Sender<Message>,
    next: std::sync::atomic::AtomicU64,
}

/// Where one execution sends its viewing; dropping it stops the watch.
#[derive(Debug)]
pub(super) struct Viewing {
    key: u64,
    messages: mpsc::Sender<Message>,
}

impl Viewing {
    pub fn send(&self, hold: Option<ViewerHold>) {
        let _ = self.messages.send(Message::Hold(self.key, hold));
    }
}

impl Drop for Viewing {
    fn drop(&mut self) {
        let _ = self.messages.send(Message::Gone(self.key));
    }
}

#[derive(Debug)]
enum Message {
    Watch(u64, Box<Watch>),
    Hold(u64, Option<ViewerHold>),
    Gone(u64),
    /// A native client attached: look again without a viewing change.
    Wake,
}

impl Watcher {
    pub fn spawn() -> Self {
        let (messages, received) = mpsc::channel();
        std::thread::Builder::new()
            .name("lifetime".into())
            .spawn(move || record(&received))
            .expect("spawn the lifetime thread");
        Self {
            messages,
            next: std::sync::atomic::AtomicU64::new(0),
        }
    }

    /// Record `host`'s viewing sent through the returned handle. A Desktop
    /// shell's owner (`shell`) also learns of its explicit Close on the
    /// receiver, after `waker` wakes its wait; so does any host whose
    /// `client` moved to another attachment, through `client` itself.
    pub fn watch(
        &self,
        host: Host,
        shell: bool,
        waker: UnixStream,
        client: Arc<Client>,
    ) -> (Viewing, mpsc::Receiver<()>) {
        let key = self.next.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        let (close, closing) = mpsc::channel();
        client
            .0
            .lock()
            .expect("client attachment lock poisoned")
            .wake = Some(self.messages.clone());
        let watch = Watch {
            host,
            shell,
            close,
            waker,
            client,
            hold: None,
            recorded: None,
            hold_failure: RecordingGap::default(),
            close_failure: RecordingGap::default(),
            client_failure: RecordingGap::default(),
        };
        let _ = self.messages.send(Message::Watch(key, Box::new(watch)));
        let viewing = Viewing {
            key,
            messages: self.messages.clone(),
        };
        (viewing, closing)
    }
}

#[derive(Debug)]
struct Watch {
    host: Host,
    shell: bool,
    close: mpsc::Sender<()>,
    waker: UnixStream,
    client: Arc<Client>,
    hold: Option<ViewerHold>,
    /// None means no observation has been committed, including initial absence.
    recorded: Option<Option<ViewerHold>>,
    hold_failure: RecordingGap,
    close_failure: RecordingGap,
    client_failure: RecordingGap,
}

impl Watch {
    /// Only a failed write, an explicit shell Close or an attached native
    /// client needs another attempt without a change.
    fn waiting(&self) -> bool {
        self.shell || self.recorded != Some(self.hold) || self.client.attached()
    }

    /// Record what changed; false once a shell's Close has been delivered.
    fn record(&mut self, store: &crate::store::sqlite::SqliteStore) -> bool {
        if self.recorded != Some(self.hold)
            && report_result(
                &mut self.hold_failure,
                "Session viewer hold",
                store.record_owner_viewer_hold(self.host.transport(), self.hold),
            )
            .is_some()
        {
            self.recorded = Some(self.hold);
        }
        if report_result(
            &mut self.client_failure,
            "native client attachment",
            self.client.check(store),
        ) == Some(true)
        {
            let _ = (&self.waker).write_all(&[1]);
        }
        let Some(owner) = self.host.owner().filter(|_| self.shell) else {
            return true;
        };
        if report_result(
            &mut self.close_failure,
            "Shell Close",
            store.shell_should_close(owner),
        ) == Some(true)
        {
            let _ = self.close.send(());
            let _ = (&self.waker).write_all(&[1]);
            return false;
        }
        true
    }
}

fn record(received: &mpsc::Receiver<Message>) {
    let mut watches: HashMap<u64, Watch> = HashMap::new();
    let mut store = None;
    let mut store_failure = RecordingGap::default();
    loop {
        let message = if watches.values().any(Watch::waiting) {
            received.recv_timeout(POLL)
        } else {
            received
                .recv()
                .map_err(|_| mpsc::RecvTimeoutError::Disconnected)
        };
        let mut apply = |message| match message {
            Message::Watch(key, watch) => {
                watches.insert(key, *watch);
            }
            Message::Hold(key, hold) => {
                if let Some(watch) = watches.get_mut(&key) {
                    watch.hold = hold;
                }
            }
            Message::Gone(key) => {
                watches.remove(&key);
            }
            Message::Wake => {}
        };
        match message {
            Ok(message) => apply(message),
            Err(mpsc::RecvTimeoutError::Timeout) => {}
            Err(mpsc::RecvTimeoutError::Disconnected) => return,
        }
        for message in received.try_iter() {
            apply(message);
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
        watches.retain(|_, watch| watch.record(store));
    }
}

/// Each operation keeps its own gap until that operation succeeds.
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
    use super::Host;
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
        let (messages, received) = std::sync::mpsc::channel();
        let watcher = super::Watcher {
            messages,
            next: std::sync::atomic::AtomicU64::new(0),
        };
        let recorder = std::thread::spawn(move || {
            crate::journal::with_test_ledger(db, || super::record(&received))
        });
        let (_wake, waker) = std::os::unix::net::UnixStream::pair().unwrap();
        let (viewing, closing) = watcher.watch(
            Host::Owner(owner.clone()),
            false,
            waker,
            std::sync::Arc::default(),
        );
        viewing.send(Some(ViewerHold::Viewed(123)));
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
        // Retries come every POLL; the deadline only bounds a broken retry.
        let deadline = Instant::now() + Duration::from_secs(5);
        while read().is_none() && Instant::now() < deadline {
            std::thread::sleep(Duration::from_millis(20));
        }
        assert_eq!(read(), Some(123));
        // After recovery the recorder waits for changes; departure must wake it.
        viewing.send(Some(ViewerHold::Absent(124)));
        let deadline = Instant::now() + Duration::from_secs(5);
        while read() != Some(124) && Instant::now() < deadline {
            std::thread::sleep(Duration::from_millis(20));
        }
        drop(viewing);
        drop(watcher);
        recorder.join().unwrap();
        assert_eq!(read(), Some(124));
        assert!(closing.try_recv().is_err());
    }

    #[test]
    fn client_retirement_waits_for_a_readable_store_and_keeps_its_move_notice() {
        use super::{Client, ClientState};
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
            wake: None,
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
}
