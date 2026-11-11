//! A registry hosting terminals that a holder on another thread keeps.

use std::io::{Read, Write};
use std::os::unix::net::UnixStream;
use std::time::{Duration, Instant};

use super::super::wire::{frame, parse_position, Decoder, Incoming, Own, Request, Tag};
use super::{connect, Registry, Start};
use crate::holder::{serve, Place};
use crate::id::{LfSessionId, TerminalId};
use crate::journal::TestLedgerGuard;

/// Stands in for the executable hash both the holder and the server compute.
const BUILD: &str = "feedfacefeedfacefeedfacefeedfacefeedfacefeedfacefeedfacefeedface";

/// This build's holder, serving on a thread under the test's LF home.
fn holder() -> Place {
    let place = Place::of(BUILD).unwrap();
    let (ready, listening) = std::sync::mpsc::channel();
    let served = place.clone();
    std::thread::spawn(move || {
        crate::holder::TEST_IDLE.set(Duration::from_secs(60));
        serve(&served, BUILD, move || ready.send(()).unwrap()).unwrap()
    });
    listening.recv_timeout(Duration::from_secs(10)).unwrap();
    place
}

fn shell(session: Option<&LfSessionId>, request: &str, script: &str) -> Start {
    Start {
        session: session.cloned(),
        request: request.into(),
        argv: vec!["/bin/sh".into(), "-c".into(), script.into()],
        cwd: "/".into(),
        env: vec![("PATH".into(), "/usr/bin:/bin".into())],
        rows: 24,
        cols: 80,
    }
}

/// A controlling view of `terminal`.
struct View {
    socket: UnixStream,
    decoder: Decoder,
    seen: Vec<u8>,
    /// Every frame tag received, in order.
    tags: Vec<Tag>,
    /// The PTY output received in out frames after the last position.
    out: Vec<u8>,
    /// The PTY position the screen shows.
    position: Option<u64>,
}

impl View {
    fn attach(terminal: &TerminalId) -> Self {
        Self::attach_from(terminal, None)
    }

    fn attach_from(terminal: &TerminalId, from: Option<u64>) -> Self {
        let mut socket = connect(terminal).unwrap();
        let attach = Incoming {
            id: 1,
            request: Request::Attach {
                control: true,
                replace: true,
                own: Some(Own::Take),
                rows: 24,
                cols: 80,
                name: None,
                moved: None,
                from,
            },
        };
        socket
            .write_all(&frame(Tag::Request, &serde_json::to_vec(&attach).unwrap()))
            .unwrap();
        socket
            .set_read_timeout(Some(Duration::from_millis(100)))
            .unwrap();
        Self {
            socket,
            decoder: Decoder::default(),
            seen: Vec::new(),
            tags: Vec::new(),
            out: Vec::new(),
            position: from,
        }
    }

    fn type_in(&mut self, bytes: &[u8]) {
        self.socket.write_all(&frame(Tag::Input, bytes)).unwrap();
    }

    /// Read the snapshot and output until they hold `text`.
    fn wait_for(&mut self, text: &str) {
        let deadline = Instant::now() + Duration::from_secs(20);
        while !String::from_utf8_lossy(&self.seen).contains(text) {
            assert!(
                Instant::now() < deadline,
                "view never showed {text:?}: {:?}",
                String::from_utf8_lossy(&self.seen)
            );
            let mut bytes = [0; 8192];
            match self.socket.read(&mut bytes) {
                Ok(0) => panic!(
                    "view closed before {text:?}: {:?}",
                    String::from_utf8_lossy(&self.seen)
                ),
                Ok(n) => self.decoder.extend(&bytes[..n]),
                Err(_) => continue,
            }
            while let Some((tag, payload)) = self.decoder.next().unwrap() {
                self.tags.push(tag);
                match tag {
                    Tag::Position => {
                        self.position = Some(parse_position(&payload).unwrap());
                        self.out.clear();
                    }
                    Tag::Out => {
                        self.out.extend(&payload);
                        *self
                            .position
                            .as_mut()
                            .expect("out frames follow a position") += payload.len() as u64;
                    }
                    _ => {}
                }
                if matches!(tag, Tag::Snapshot | Tag::Out | Tag::Aside) {
                    self.seen.extend(payload);
                }
            }
        }
    }
}

fn wait_until(what: &str, mut done: impl FnMut() -> bool) {
    let deadline = Instant::now() + Duration::from_secs(20);
    while !done() {
        assert!(Instant::now() < deadline, "timed out waiting for {what}");
        std::thread::sleep(Duration::from_millis(50));
    }
}

#[test]
fn a_held_terminal_outlives_the_server_that_started_it() {
    let _ledger = TestLedgerGuard::new();
    let place = holder();
    let session = LfSessionId::from("session_held");
    let first = Registry::open(BUILD).unwrap();
    let terminal = first
        .start(shell(
            Some(&session),
            "request-1",
            "printf 'one\\n'; read line; printf \"two:$line\\n\"; read rest",
        ))
        .unwrap();
    assert_eq!(first.session_terminal(&session), Some(terminal.clone()));
    assert_eq!(
        first
            .start(shell(Some(&session), "request-1", "exit 9"))
            .unwrap(),
        terminal,
        "a repeated request starts nothing"
    );
    let mut view = View::attach(&terminal);
    view.wait_for("one");

    // The server goes; the child and its PTY stay with the holder.
    first.close();
    drop(view);
    let held = crate::holder::client::Client::connect(&place)
        .unwrap()
        .list()
        .unwrap();
    assert!(held
        .iter()
        .any(|held| held.terminal == terminal.as_str() && held.exit.is_none()));

    let second = Registry::open(BUILD).unwrap();
    assert_eq!(second.session_terminal(&session), Some(terminal.clone()));
    let mut view = View::attach(&terminal);
    // The screen was rebuilt from the holder's ring, and the same child answers.
    view.wait_for("one");
    view.type_in(b"again\n");
    view.wait_for("two:again");
    assert_eq!(
        second
            .start(shell(Some(&session), "request-1", "exit 9"))
            .unwrap(),
        terminal,
        "the request still names its terminal after a restart"
    );
    second.close();
    crate::os_process::kill_process_group(
        held.iter()
            .find(|held| held.terminal == terminal.as_str())
            .unwrap()
            .pid,
    );
}

#[test]
fn an_ended_terminal_is_settled_and_never_answers_again() {
    let _ledger = TestLedgerGuard::new();
    let registry = Registry::open(BUILD).unwrap();
    let place = holder();
    let terminal = registry
        .start(shell(None, "request-1", "printf done; exit 3"))
        .unwrap();
    let store = crate::journal::process_ledger().unwrap();
    wait_until("the exit to be recorded", || {
        store.running_terminals().unwrap().is_empty()
    });
    wait_until("the receipt to be removed", || {
        !place.terminal(terminal.as_str()).exists()
    });
    // Attaching never starts anything, and an ID is never reused.
    let refused = connect(&terminal).unwrap_err();
    let refused = refused.downcast_ref::<crate::engine::Error>().unwrap();
    assert_eq!(refused.code, crate::engine::ErrorCode::NotFound);
    let never = connect(&TerminalId::new()).unwrap_err();
    assert!(never.is::<crate::engine::Error>(), "{never:#}");
    registry.close();
}

#[test]
fn an_exit_with_no_server_running_is_settled_by_the_next_one() {
    let _ledger = TestLedgerGuard::new();
    let first = Registry::open(BUILD).unwrap();
    let place = holder();
    let terminal = first
        .start(shell(None, "request-1", "read line; exit 4"))
        .unwrap();
    first.close();
    let mut input = crate::holder::client::Client::connect(&place)
        .unwrap()
        .write(terminal.as_str())
        .unwrap();
    input.write_all(b"go\n").unwrap();
    wait_until("the holder to see the exit", || {
        crate::holder::recorded_exit(&place, terminal.as_str())
            .unwrap()
            .is_some()
    });
    let store = crate::journal::process_ledger().unwrap();
    assert_eq!(store.running_terminals().unwrap().len(), 1);

    let second = Registry::open(BUILD).unwrap();
    wait_until("the exit to be recorded", || {
        store.running_terminals().unwrap().is_empty()
    });
    assert!(connect(&terminal).is_err());
    second.close();
}

/// A launcher's handed-off exit record for a fresh Session's provider, kept
/// beside `terminal` as the server that accepted it writes it.
fn kept_exit_record(place: &Place, terminal: &TerminalId) -> crate::process::SessionAttachment {
    let store = crate::journal::process_ledger().unwrap();
    let session = store.test_session("held", &crate::session_record::new_artifact_key());
    let launcher = store.test_lf_process();
    let attachment = store
        .claim_session_attachment(&session.id, None, &launcher, true)
        .unwrap();
    let record = crate::harness::native_exit::NativeExit {
        session: session.id.to_string(),
        attachment: attachment.clone(),
        provider: "claude".into(),
        client: false,
        resuming: false,
        cwd: "/".into(),
        capture: None,
        capture_key: None,
        account: None,
        diagnostics: ("/absent.log".into(), 0),
    };
    std::fs::write(
        super::exit_record_file(place, terminal),
        serde_json::to_vec(&record).unwrap(),
    )
    .unwrap();
    attachment
}

fn provider_ended(attachment: &crate::process::SessionAttachment) -> bool {
    crate::journal::process_ledger()
        .unwrap()
        .agent_process(&attachment.agent_process_id)
        .unwrap()
        .is_some_and(|process| process.completed_at.is_some())
}

#[test]
fn an_exit_record_kept_while_no_server_ran_is_settled_by_the_next_one() {
    let _ledger = TestLedgerGuard::new();
    let first = Registry::open(BUILD).unwrap();
    let place = holder();
    let terminal = first
        .start(shell(None, "request-1", "read line; exit 4"))
        .unwrap();
    let attachment = kept_exit_record(&place, &terminal);
    first.close();
    let mut input = crate::holder::client::Client::connect(&place)
        .unwrap()
        .write(terminal.as_str())
        .unwrap();
    input.write_all(b"go\n").unwrap();
    wait_until("the holder to see the exit", || {
        crate::holder::recorded_exit(&place, terminal.as_str())
            .unwrap()
            .is_some()
    });
    assert!(!provider_ended(&attachment));

    let second = Registry::open(BUILD).unwrap();
    let store = crate::journal::process_ledger().unwrap();
    wait_until("the exit to be recorded", || {
        store.running_terminals().unwrap().is_empty()
    });
    assert!(provider_ended(&attachment));
    assert!(!place.terminal(terminal.as_str()).exists());
    second.close();
}

#[test]
fn a_server_taking_up_a_terminal_settles_the_exit_record_kept_beside_it() {
    let _ledger = TestLedgerGuard::new();
    let first = Registry::open(BUILD).unwrap();
    let place = holder();
    let terminal = first
        .start(shell(None, "request-1", "read line; exit 4"))
        .unwrap();
    let attachment = kept_exit_record(&place, &terminal);
    first.close();

    let second = Registry::open(BUILD).unwrap();
    let mut view = View::attach(&terminal);
    view.type_in(b"go\n");
    let store = crate::journal::process_ledger().unwrap();
    wait_until("the exit to be recorded", || {
        store.running_terminals().unwrap().is_empty()
    });
    assert!(provider_ended(&attachment));
    second.close();
}

/// A shell printing a counter line every 50 ms until it is killed.
const COUNTER: &str = "i=0; while :; do i=$((i+1)); printf 'tick:%d\\n' $i; sleep 0.05; done";

/// The PTY bytes the holder keeps for `terminal` from `from` up to `until`.
fn ring(place: &Place, terminal: &TerminalId, from: u64, until: u64) -> Vec<u8> {
    let mut output = crate::holder::client::Client::connect(place)
        .unwrap()
        .read(terminal.as_str(), from)
        .unwrap();
    let mut bytes = Vec::new();
    while from + (bytes.len() as u64) < until {
        match output.frame().unwrap() {
            crate::holder::wire::Frame::Out(out) => bytes.extend(out),
            crate::holder::wire::Frame::Mode(_) => {}
            other => panic!("unexpected holder frame {other:?}"),
        }
    }
    bytes.truncate((until - from) as usize);
    bytes
}

fn kill(place: &Place, terminal: &TerminalId) {
    let held = crate::holder::client::Client::connect(place)
        .unwrap()
        .list()
        .unwrap();
    crate::os_process::kill_process_group(
        held.iter()
            .find(|held| held.terminal == terminal.as_str())
            .unwrap()
            .pid,
    );
}

const LIVE: &str = "\"type\":\"live\"";

#[test]
fn a_view_continues_from_its_position_after_the_server_restarts() {
    let _ledger = TestLedgerGuard::new();
    let place = holder();
    let first = Registry::open(BUILD).unwrap();
    let terminal = first.start(shell(None, "request-1", COUNTER)).unwrap();
    let mut view = View::attach(&terminal);
    view.wait_for("tick:3");
    assert_eq!(
        view.tags
            .iter()
            .filter(|tag| **tag == Tag::Snapshot)
            .count(),
        1
    );
    let left = view.position.expect("a held view is told its position");

    // The server goes; the agent keeps printing into the holder's ring.
    first.close();
    drop(view);
    std::thread::sleep(Duration::from_millis(500));

    let second = Registry::open(BUILD).unwrap();
    let mut view = View::attach_from(&terminal, Some(left));
    view.wait_for(LIVE);
    assert!(
        !view.tags.contains(&Tag::Snapshot) && !view.tags.contains(&Tag::Position),
        "a continued view is neither repainted nor moved: {:?}",
        view.tags
    );
    // Its stream is exactly the PTY output since it left, including what
    // was printed while no server ran.
    let reached = view.position.unwrap();
    assert_eq!(view.out, ring(&place, &terminal, left, reached));
    let printed = String::from_utf8_lossy(&view.out).matches("tick:").count();
    assert!(printed >= 5, "{:?}", String::from_utf8_lossy(&view.out));
    // Then it follows the live stream.
    let ticks = String::from_utf8_lossy(&ring(&place, &terminal, 0, reached))
        .matches("tick:")
        .count();
    view.wait_for(&format!("tick:{}", ticks + 2));
    assert_eq!(
        view.out,
        ring(&place, &terminal, left, view.position.unwrap())
    );
    second.close();
    kill(&place, &terminal);
}

#[test]
fn a_view_the_ring_no_longer_reaches_is_painted_afresh() {
    let _ledger = TestLedgerGuard::new();
    let place = holder();
    let registry = Registry::open(BUILD).unwrap();
    let terminal = registry
        .start(shell(
            None,
            "request-1",
            "head -c 3000000 /dev/zero | tr '\\0' x; printf 'end\\n'; read line",
        ))
        .unwrap();
    wait_until("the ring to pass its start", || {
        crate::holder::client::Client::connect(&place)
            .unwrap()
            .list()
            .unwrap()
            .iter()
            .any(|held| held.ring_start > 0 && held.written > 3_000_000)
    });
    let mut view = View::attach_from(&terminal, Some(1));
    view.wait_for(LIVE);
    assert!(view.tags.contains(&Tag::Snapshot), "{:?}", view.tags);
    let at = view.position.unwrap();
    let ring_start = crate::holder::client::Client::connect(&place)
        .unwrap()
        .list()
        .unwrap()[0]
        .ring_start;
    assert!(
        at >= ring_start,
        "the snapshot's position {at} is inside the ring"
    );
    // A position past anything the PTY printed is painted afresh too.
    let mut view = View::attach_from(&terminal, Some(u64::MAX / 2));
    view.wait_for(LIVE);
    assert!(view.tags.contains(&Tag::Snapshot), "{:?}", view.tags);
    registry.close();
    kill(&place, &terminal);
}

#[test]
fn queries_replayed_from_the_ring_are_not_answered_again() {
    let _ledger = TestLedgerGuard::new();
    let place = holder();
    let answers = tempfile::tempdir().unwrap();
    let file = answers.path().join("answers");
    let first = Registry::open(BUILD).unwrap();
    // A program asks for the cursor once and keeps everything it is sent.
    let script = format!(
        "stty raw -echo; printf 'asked\\033[6n'; exec cat > {}",
        file.display()
    );
    let terminal = first.start(shell(None, "request-1", &script)).unwrap();
    let read = || std::fs::read(&file).unwrap_or_default();
    // No view is attached: the retained screen answers.
    wait_until("the screen to answer", || read().ends_with(b"R"));
    let answered = read();
    first.close();

    let second = Registry::open(BUILD).unwrap();
    // The rebuilt screen has replayed the question and holds it.
    let mut view = View::attach(&terminal);
    view.wait_for("asked");
    std::thread::sleep(Duration::from_millis(500));
    assert_eq!(read(), answered, "a replayed query was answered again");
    second.close();
    kill(&place, &terminal);
}
