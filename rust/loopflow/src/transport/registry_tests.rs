//! A registry hosting terminals that a holder on another thread keeps.

use std::io::{Read, Write};
use std::os::unix::net::UnixStream;
use std::time::{Duration, Instant};

use super::super::wire::{frame, Decoder, Incoming, Own, Request, Tag};
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
}

impl View {
    fn attach(terminal: &TerminalId) -> Self {
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
                if matches!(tag, Tag::Snapshot | Tag::Out) {
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
