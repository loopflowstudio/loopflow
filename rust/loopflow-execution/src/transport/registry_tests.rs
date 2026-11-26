//! A registry hosting terminals that a holder on another thread keeps.

use crate::process::attachment;
use crate::store::session_projection;
use std::io::{Read, Write};
use std::os::unix::net::UnixStream;
use std::time::{Duration, Instant};

use super::super::wire::{frame, parse_position, Decoder, Incoming, Request, Tag};
use super::{connect, Registry, Start};
use crate::journal::TestLedgerGuard;
use crate::terminal_holder::{serve, Place};
use loopflow_model::id::{LfSessionId, TerminalId};

/// Stands in for the executable hash both the holder and the server compute.
const BUILD: &str = "feedfacefeedfacefeedfacefeedfacefeedfacefeedfacefeedfacefeedface";

/// This build's holder, serving on a thread under the test's LF home.
fn holder() -> Place {
    let place = Place::of(BUILD).unwrap();
    let (ready, listening) = std::sync::mpsc::channel();
    let served = place.clone();
    std::thread::spawn(move || {
        crate::terminal_holder::TEST_IDLE.set(Duration::from_secs(60));
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
        placement: None,
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
        Self::attach_sized(terminal, from, 80)
    }

    fn attach_sized(terminal: &TerminalId, from: Option<u64>, cols: u16) -> Self {
        let mut socket = connect(terminal).unwrap();
        let attach = Incoming {
            id: 1,
            request: Request::Attach {
                focused: None,
                rows: 24,
                cols,
                name: None,
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
fn held_children_get_terminal_colors_when_the_caller_leaves_them_unset() {
    let _ledger = TestLedgerGuard::new();
    let place = holder();
    let registry = Registry::open(BUILD).unwrap();
    for (request, caller, expected) in [
        ("missing", vec![], "xterm-256color:truecolor"),
        (
            "empty",
            vec![("TERM", ""), ("COLORTERM", "")],
            "xterm-256color:truecolor",
        ),
        (
            "custom",
            vec![("TERM", "screen-256color"), ("COLORTERM", "24bit")],
            "screen-256color:24bit",
        ),
        ("partial", vec![("TERM", "dumb")], "dumb:truecolor"),
    ] {
        let mut start = shell(
            None,
            request,
            r#"printf 'colors=%s:%s\n' "$TERM" "$COLORTERM"; read line"#,
        );
        start
            .env
            .extend(caller.into_iter().map(|(k, v)| (k.into(), v.into())));
        let terminal = registry.start(start).unwrap();
        let mut view = View::attach(&terminal);
        view.wait_for(&format!("colors={expected}"));
        view.type_in(b"\n");
        drop(view);
        wait_until("color probe settled", || {
            !place.terminal(terminal.as_str()).exists()
        });
    }
    registry.close();
}

#[test]
fn an_interrupted_paste_ends_before_the_next_view_types() {
    let ledger = TestLedgerGuard::new();
    let place = holder();
    let mut registry = Registry::open(BUILD).unwrap();
    for interruption in ["disconnect", "another view", "restart"] {
        let end = ledger.home().join(interruption.replace(' ', "-"));
        let script = r#"stty raw -echo
printf 'ready\r\n'
dd bs=1 count=9 of=/dev/null 2>/dev/null
printf 'prefix\r\n'
dd bs=1 count=6 of="$PASTE_END" 2>/dev/null
read line
printf 'reply:%s\r\n' "$line""#;
        let mut start = shell(None, &interruption.replace(' ', "-"), script);
        start
            .env
            .push(("PASTE_END".into(), end.display().to_string()));
        let terminal = registry.start(start).unwrap();
        let mut first = Some(View::attach(&terminal));
        let pasting = first.as_mut().unwrap();
        pasting.wait_for("ready");
        pasting.type_in(b"\x1b[200~abc");
        pasting.wait_for("prefix");
        let wait_for_end = || {
            wait_until("child to receive the paste end", || {
                end.metadata().is_ok_and(|metadata| metadata.len() == 6)
            });
            assert_eq!(std::fs::read(&end).unwrap(), b"\x1b[201~", "{interruption}");
        };
        let mut next = if interruption == "another view" {
            // The first view stays; the second one's keys end its paste.
            let mut next = View::attach(&terminal);
            next.wait_for("prefix");
            next.type_in(b"next\n");
            wait_for_end();
            next
        } else {
            if interruption == "restart" {
                registry.close();
            }
            first = None;
            wait_for_end();
            if interruption == "restart" {
                registry = Registry::open(BUILD).unwrap();
            }
            let mut next = View::attach(&terminal);
            next.type_in(b"next\n");
            next
        };
        next.wait_for("reply:next");
        drop(first);
        wait_until("paste child exit receipt", || {
            crate::terminal_holder::recorded_exit(&place, terminal.as_str())
                .unwrap()
                .is_some()
        });
    }
    registry.close();
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
    let held = crate::terminal_holder::client::Client::connect(&place)
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
fn a_resumed_session_gets_a_new_terminal_and_the_old_position_is_refused() {
    let _ledger = TestLedgerGuard::new();
    let registry = Registry::open(BUILD).unwrap();
    let place = holder();
    let session = LfSessionId::from("session_resumed");
    let ended = registry
        .start(shell(Some(&session), "request-1", "printf first; exit 0"))
        .unwrap();
    let mut view = View::attach(&ended);
    view.wait_for("first");
    let left = view.position.expect("a held view is told its position");
    drop(view);
    wait_until("the receipt to be removed", || {
        !place.terminal(ended.as_str()).exists()
    });

    let resumed = registry
        .start(shell(
            Some(&session),
            "request-2",
            "printf second; read line",
        ))
        .unwrap();
    assert_ne!(resumed, ended, "a terminal ID is never reused");
    assert_eq!(registry.session_terminal(&session), Some(resumed.clone()));
    // A view continuing the ended terminal from its position finds nothing
    // to attach to; the holder answers neither from byte 0.
    let refused = connect(&ended).unwrap_err();
    let refused = refused.downcast_ref::<crate::engine::Error>().unwrap();
    assert_eq!(refused.code, crate::engine::ErrorCode::NotFound);
    let unheld = crate::terminal_holder::client::Client::connect(&place)
        .unwrap()
        .read(ended.as_str(), left)
        .unwrap_err();
    assert!(
        unheld.to_string().contains("does not hold terminal"),
        "{unheld:#}"
    );
    registry.close();
    kill(&place, &resumed);
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
    let mut input = crate::terminal_holder::client::Client::connect(&place)
        .unwrap()
        .write(terminal.as_str())
        .unwrap();
    input.write_all(b"go\n").unwrap();
    wait_until("the holder to see the exit", || {
        crate::terminal_holder::recorded_exit(&place, terminal.as_str())
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
    let attachment =
        attachment::claim_session_attachment(&store, &session.id, None, &launcher).unwrap();
    let record = crate::harness::native_exit::NativeExit {
        session: session.id.to_string(),
        attachment: attachment.clone(),
        provider: "claude".into(),
        client: false,
        cwd: "/".into(),
        capture: None,

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
    let mut input = crate::terminal_holder::client::Client::connect(&place)
        .unwrap()
        .write(terminal.as_str())
        .unwrap();
    input.write_all(b"go\n").unwrap();
    wait_until("the holder to see the exit", || {
        crate::terminal_holder::recorded_exit(&place, terminal.as_str())
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
    let mut output = crate::terminal_holder::client::Client::connect(place)
        .unwrap()
        .read(terminal.as_str(), from)
        .unwrap();
    let mut bytes = Vec::new();
    while from + (bytes.len() as u64) < until {
        match output.frame().unwrap() {
            crate::terminal_holder::wire::Frame::Out(out) => bytes.extend(out),
            crate::terminal_holder::wire::Frame::Mode(_) => {}
            other => panic!("unexpected holder frame {other:?}"),
        }
    }
    bytes.truncate((until - from) as usize);
    bytes
}

fn kill(place: &Place, terminal: &TerminalId) {
    let held = crate::terminal_holder::client::Client::connect(place)
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
    // Wait for output, not time: at least five full counter lines.
    ring(&place, &terminal, left, left + 60);

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

/// A server taking up a terminal narrows it by a column and widens it back to
/// ask for a repaint. A view that takes the size meanwhile keeps it.
#[test]
fn a_view_sizing_the_terminal_during_the_repaint_nudge_keeps_its_size() {
    let _ledger = TestLedgerGuard::new();
    let place = holder();
    let first = Registry::open(BUILD).unwrap();
    let script = "trap 'echo size:$(stty size)' WINCH; echo up; \
                  while :; do read -r line && echo now:$(stty size); done";
    let terminal = first.start(shell(None, "request-1", script)).unwrap();
    let mut view = View::attach(&terminal);
    view.wait_for("up");
    first.close();
    drop(view);

    let second = Registry::open(BUILD).unwrap();
    let mut wide = View::attach_sized(&terminal, None, 120);
    wide.wait_for("size:24 120");
    std::thread::sleep(super::NUDGE * 3);
    wide.type_in(b"\n");
    wide.wait_for("now:24 120");
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
        crate::terminal_holder::client::Client::connect(&place)
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
    let ring_start = crate::terminal_holder::client::Client::connect(&place)
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

#[test]
fn a_holder_that_refuses_connections_and_keeps_its_lock_is_waited_for() {
    let _ledger = TestLedgerGuard::new();
    let place = Place::of(BUILD).unwrap();
    // A stopped holder whose queue is full refuses like this dead socket.
    drop(std::os::unix::net::UnixListener::bind(&place.socket).unwrap());
    let lock = std::fs::File::create(&place.lock).unwrap();
    fs2::FileExt::try_lock_exclusive(&lock).unwrap();
    let terminal = TerminalId::new();
    std::fs::create_dir_all(place.terminal(terminal.as_str())).unwrap();

    let registry = Registry::open(BUILD).unwrap();
    std::thread::sleep(Duration::from_millis(700));
    let waiting = connect(&terminal).unwrap_err();
    assert!(!waiting.is::<crate::engine::Error>(), "{waiting:#}");

    // Its lock free, it is gone: the terminal ends with an unknown outcome.
    drop(lock);
    wait_until("the terminal to end", || {
        connect(&terminal).is_err_and(|refused| refused.is::<crate::engine::Error>())
    });
    registry.close();
}

/// The server reads every held terminal's Program Status, viewed or not: an
/// agent asking a question with no view open is waiting on its person.
#[test]
fn a_held_terminal_with_no_view_reports_that_it_waits() {
    let ledger = TestLedgerGuard::new();
    let registry = Registry::open(BUILD).unwrap();
    let place = holder();
    let store = crate::journal::process_ledger().unwrap();
    let session = store.test_session("asking", &crate::session_record::new_artifact_key());
    let launcher = store.test_lf_process();
    let attachment =
        attachment::claim_session_attachment(&store, session.id.as_str(), None, &launcher).unwrap();
    let ask = ledger.home().join("ask");
    let script = format!(
        r"printf '\033]7501;state=working:app=claude-code\033\\'
while [ ! -e '{ask}' ]; do sleep 0.02; done
printf 'May I?\r\n\033]7501;state=blocked:app=claude-code\033\\'
read answer
printf '\033]7501;state=done:app=claude-code\033\\\033]7501;id=child:state=error\033\\'
read next",
        ask = ask.display()
    );
    let terminal = registry
        .start(shell(Some(&session.id), "request-1", &script))
        .unwrap();
    store
        .bind_session_terminal(session.id.as_str(), &attachment, &terminal)
        .unwrap();
    let pid = crate::terminal_holder::client::Client::connect(&place)
        .unwrap()
        .list()
        .unwrap()
        .into_iter()
        .find(|held| held.terminal == terminal.as_str())
        .unwrap()
        .pid;
    let started = crate::process::os_process::process_started_at(pid)
        .unwrap()
        .unwrap();
    store
        .record_agent_process_identity(session.id.as_str(), &attachment, pid, started)
        .unwrap();
    let summary = || {
        session_projection::session_summary(
            &store,
            session.id.as_str(),
            time::OffsetDateTime::now_utc().unix_timestamp(),
        )
        .unwrap()
        .unwrap()
    };
    assert!(!summary().available, "a working agent waits on nobody");
    std::fs::write(&ask, "").unwrap();
    wait_until("the unviewed question to read as waiting", || {
        summary().available
    });
    let mut view = View::attach(&terminal);
    view.wait_for("May I?");
    view.type_in(b"yes\n");
    wait_until("the finished turn to read as available", || {
        let row = summary();
        row.available
            && row.program_status.is_some_and(|status| {
                status
                    .records
                    .iter()
                    .any(|r| r.state == loopflow_model::program_status::State::Done)
            })
    });
    view.type_in(b"x");
    wait_until(
        "typing to dismiss the stored done and error reports",
        || {
            let row = summary();
            !row.available
                && row
                    .program_status
                    .is_some_and(|status| status.seen && status.records.is_empty())
        },
    );
    registry.close();
    kill(&place, &terminal);
}

#[test]
fn wake_idle_prompt_guards_against_drafts_typing_and_open_pastes_but_ignores_mouse_and_focus() {
    let ledger = TestLedgerGuard::new();
    let place = holder();
    let registry = Registry::open(BUILD).unwrap();
    let pasted = ledger.home().join("pasted");
    let rule = "─".repeat(80);
    let script = format!(
        r#"stty -echo
printf '\033[?2004h\033[2J\033[H{rule}\r\n\033[94m>\033[m\r\n{rule}\r\n? for shortcuts   auto · Gemini 4 Neon · high\r\n'
IFS= read -r line
printf '%s' "$line" > "{}"
"#,
        pasted.display()
    );
    let terminal = registry.start(shell(None, "wake-guard", &script)).unwrap();
    let mut view = View::attach(&terminal);
    view.wait_for("? for shortcuts");

    // Unknown terminal reports Absent.
    assert_eq!(
        registry
            .wake_idle_prompt(
                &TerminalId::new(),
                "ignored",
                crate::harness::antigravity::prompt_ready
            )
            .unwrap(),
        super::WakeTerminal::Absent
    );

    // Focus reports and mouse wheel events do not count as user keystrokes.
    view.type_in(b"\x1b[I\x1b[<64;10;20M\x1b[O");
    std::thread::sleep(Duration::from_millis(50));

    // Caller-supplied screen predicate controls readiness (transport stays provider-agnostic).
    assert_eq!(
        registry
            .wake_idle_prompt(&terminal, "refused-by-predicate", |_| false)
            .unwrap(),
        super::WakeTerminal::NotReady
    );

    // An open bracketed paste blocks injection.
    let paste_seen = ledger.home().join("paste_seen");
    let paste_script = format!(
        r#"stty raw -echo
printf '\033[?2004h\033[2J\033[H{rule}\r\n\033[94m>\033[m\r\n{rule}\r\n? for shortcuts   auto · Gemini 4 Neon · high\r\n'
dd bs=1 count=13 of="{}" 2>/dev/null
sleep 30
"#,
        paste_seen.display()
    );
    let draft_terminal = registry
        .start(shell(None, "wake-paste", &paste_script))
        .unwrap();
    let mut paste_view = View::attach(&draft_terminal);
    paste_view.wait_for("? for shortcuts");
    paste_view.type_in(b"\x1b[200~partial");
    wait_until("paste bytes forwarded through hosted terminal", || {
        std::fs::read(&paste_seen).is_ok_and(|bytes| bytes == b"\x1b[200~partial")
    });
    assert_eq!(
        registry
            .wake_idle_prompt(
                &draft_terminal,
                "blocked",
                crate::harness::antigravity::prompt_ready
            )
            .unwrap(),
        super::WakeTerminal::NotReady
    );
    drop(paste_view);

    // The clean terminal (which only saw focus + mouse events) accepts the wake.
    assert_eq!(
        registry
            .wake_idle_prompt(
                &terminal,
                crate::steer::WAKE_PROMPT,
                crate::harness::antigravity::prompt_ready
            )
            .unwrap(),
        super::WakeTerminal::Injected
    );
    wait_until("pasted wake prompt written", || pasted.exists());
    assert_eq!(
        std::fs::read_to_string(&pasted).unwrap(),
        format!(
            "\x1b[I\x1b[<64;10;20M\x1b[O\x1b[200~{}\x1b[201~",
            crate::steer::WAKE_PROMPT
        )
    );
    drop(view);
    registry.close();
    kill(&place, &draft_terminal);
}

#[test]
fn session_terminal_for_prefers_the_requested_live_terminal() {
    let _ledger = TestLedgerGuard::new();
    let registry = Registry::open(BUILD).unwrap();
    let place = holder();
    let session = LfSessionId::from("session_overlap");
    let first = registry
        .start(shell(Some(&session), "request-1", "read line"))
        .unwrap();
    let second = registry
        .start(shell(Some(&session), "request-2", "read line"))
        .unwrap();
    assert_eq!(
        registry.session_terminal_for(&session, Some(&first)),
        Some(first.clone())
    );
    assert_eq!(
        registry.session_terminal_for(&session, Some(&second)),
        Some(second.clone())
    );
    kill(&place, &first);
    wait_until("the first terminal to end", || {
        registry.session_terminal_for(&session, Some(&first)) == Some(second.clone())
    });
    registry.close();
    kill(&place, &second);
}
