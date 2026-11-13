//! Relay rules: a Desktop pane's relay to a held terminal, run in a child of
//! this test binary on a real PTY. The child plays the server's entrance and
//! Desktop's status socket itself, with the relay's pace shortened; the parent
//! owns the PTY and types when the child asks.

use std::io::{Read, Write};
use std::os::fd::AsRawFd;
use std::os::unix::fs::PermissionsExt;
use std::os::unix::net::{UnixListener, UnixStream};
use std::path::PathBuf;
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

use serde_json::{json, Value};

use super::wire::{self, Decoder, Event, Incoming, Outgoing, Own, Reply, Request, Tag};
use super::{attach_with_terminal, registry, terminal_pair, Host, Pace, TEST_PACE};
use crate::id::{LfSessionId, TerminalId};

const SESSION: &str = "session_5e55105e000040008000000000000002";
const CASE: &str = "LOOPFLOW_RELAY_FIXTURE";
/// How long any one step of a case may take before it fails.
const STEP: Duration = Duration::from_secs(5);
const PACE: Pace = Pace {
    heartbeat: Duration::from_millis(50),
    silence: Duration::from_millis(300),
    attach: Duration::from_millis(300),
    backoff: Duration::from_millis(1),
};

// ---------------------------------------------------------------------------
// Child: the relay, its server and its Desktop pane

#[test]
fn relay_fixture() {
    let Ok(case) = std::env::var(CASE) else {
        return;
    };
    TEST_PACE.set(PACE);
    let home = crate::store::lf_home_dir();
    let entrance = UnixListener::bind(registry::entrance().unwrap()).unwrap();
    entrance.set_nonblocking(true).unwrap();
    let desktop = Desktop::new(std::env::var_os("LF_SERVER_STATUS").unwrap().into());
    let server = Server {
        entrance,
        home: home.clone(),
    };
    let scenario = std::thread::spawn(move || {
        let played = std::panic::catch_unwind(|| match case.as_str() {
            "quiet" => quiet_host_then_silent_bridge(&server, &desktop),
            "unacknowledged" => unacknowledged_attachment(&server, &desktop),
            "outage" => outage_and_retry(&server, &desktop),
            "setup" => failed_setup(&server, &desktop),
            "closure" => pane_closure(&server, &desktop),
            "move" => move_here(&server, &desktop),
            other => panic!("unknown relay case {other}"),
        });
        // The relay may still be waiting; a failed case ends the child now.
        if played.is_err() {
            std::process::exit(101);
        }
    });
    let relayed = attach_with_terminal(
        &Host::Terminal(TerminalId::new()),
        Some(&LfSessionId::from(SESSION)),
        None,
        None,
        false,
    );
    scenario.join().unwrap();
    relayed.unwrap();
    std::fs::write(home.join("relayed"), "").unwrap();
}

/// Heartbeats a quiet host answers keep the relay on its first connection;
/// a bridge that still accepts writes but stops answering is replaced.
fn quiet_host_then_silent_bridge(server: &Server, desktop: &Desktop) {
    let mut first = server.accept();
    first.attached(true);
    desktop.wait("connected", true, 1);
    let quiet = Instant::now() + PACE.silence * 4;
    while Instant::now() < quiet {
        if let Some(Request::Controlled { .. }) = first.next_request(PACE.heartbeat * 2) {
            first.reply(Reply::Accepted);
        }
    }
    assert_eq!(
        desktop.reading()["generation"],
        1,
        "no reconnect while answered"
    );
    // `first` stays open and readable, so only silence can end it.
    let mut second = server.accept();
    second.attached(true);
    desktop.wait("connected", true, 2);
    server.type_keys(b"after recovery\n");
    second.read_input(b"after recovery\n");
    second.exit();
    drop(first);
}

/// An attachment that is never acknowledged gives way to a new connection to
/// the same terminal; nothing about the execution is restarted.
fn unacknowledged_attachment(server: &Server, desktop: &Desktop) {
    let mut first = server.accept();
    assert!(matches!(
        first.next_request(STEP),
        Some(Request::Attach { .. })
    ));
    let mut second = server.accept();
    second.attached(true);
    desktop.wait("connected", true, 2);
    second.exit();
    drop(first);
}

/// Exhaustion reads unavailable; keys typed meanwhile are discarded, and Retry
/// reconnects without running setup again.
fn outage_and_retry(server: &Server, desktop: &Desktop) {
    let mut first = server.accept();
    first.attached(true);
    desktop.wait("connected", true, 1);
    // A lost status channel is presentation; the relay keeps relaying.
    desktop.disconnect();
    server.type_keys(b"before outage\n");
    first.read_input(b"before outage\n");
    drop(first);
    let refused = server.refuse_until(|| desktop.reading()["state"] == "unavailable");
    assert_eq!(refused, 8, "one bounded cycle");
    let message = desktop.reading()["message"].as_str().unwrap().to_owned();
    assert!(message.contains("Retry only reconnects"), "{message}");
    server.type_keys(b"uncertain outage input\n");
    desktop.send(json!({"kind": "retry"}));
    let mut second = server.accept();
    second.attached(true);
    desktop.wait("connected", true, 2);
    server.type_keys(b"after retry\n");
    second.read_input(b"after retry\n");
    second.exit();
    server.assert_setup_ran_once();
}

/// Setup that fails reads unavailable with its diagnostic. Retry reconnects
/// without running setup again, asking only for free claims: a pane that
/// never connected cannot take input from whoever has it now.
fn failed_setup(server: &Server, desktop: &Desktop) {
    let refused = server.refuse_until(|| desktop.reading()["state"] == "unavailable");
    assert_eq!(refused, 0, "failed setup spends the first cycle");
    let message = desktop.reading()["message"].as_str().unwrap().to_owned();
    assert!(
        message.contains("fixture Background Items disabled"),
        "{message}"
    );
    server.type_keys(b"typed during failed setup\n");
    desktop.send(json!({"kind": "retry"}));
    let mut first = server.accept();
    let Some(Request::Attach { replace, own, .. }) = first.next_request(STEP) else {
        panic!("Retry attaches");
    };
    assert!(
        !replace && own != Some(Own::Take),
        "recovery takes no claim"
    );
    first.attached_reply(false);
    desktop.wait("connected", false, 1);
    first.exit();
    assert!(first.input.is_empty(), "{:?}", first.input);
    server.assert_setup_ran_once();
}

/// Closing the pane ends a relay waiting to reconnect, and nothing else.
fn pane_closure(server: &Server, desktop: &Desktop) {
    let mut first = server.accept();
    first.attached(true);
    desktop.wait("connected", true, 1);
    drop(first);
    desktop.wait("reconnecting", false, 2);
    std::fs::remove_dir_all(desktop.path.parent().unwrap()).unwrap();
}

/// Move here is consumed before its takeover is sent: an acknowledgement lost
/// with its bridge is never retried, recovery asks only for free claims, and
/// keys typed meanwhile never reach the program.
fn move_here(server: &Server, desktop: &Desktop) {
    let mut first = server.accept();
    first.attached(false);
    desktop.wait("connected", false, 1);
    desktop.send(json!({"kind": "move", "generation": 1}));
    let Some(Request::Attach {
        replace: true,
        own: Some(Own::Take),
        ..
    }) = first.next_request(STEP)
    else {
        panic!("Move here takes control");
    };
    desktop.wait("moving", false, 2);
    server.type_keys(b"must not be replayed\n");
    // The takeover applied, but its acknowledgement is lost with the bridge.
    drop(first);
    desktop.wait("reconnecting", false, 3);
    desktop.send(json!({"kind": "move", "generation": 2}));
    let mut second = server.accept();
    let Some(Request::Attach { replace, own, .. }) = second.next_request(STEP) else {
        panic!("recovery attaches");
    };
    assert!(
        !replace && own != Some(Own::Take),
        "recovery takes no claim"
    );
    second.reply(Reply::Busy);
    let Some(Request::Attach { control: false, .. }) = second.next_request(STEP) else {
        panic!("a busy recovery attaches passively");
    };
    second.attached_reply(false);
    desktop.wait("connected", false, 3);
    std::thread::sleep(PACE.heartbeat * 4);
    assert_eq!(
        desktop.reading()["generation"],
        3,
        "the stale Move stays consumed"
    );
    second.exit();
    assert!(second.input.is_empty(), "{:?}", second.input);
}

struct Server {
    entrance: UnixListener,
    home: PathBuf,
}

impl Server {
    /// The next connection the relay opens, answered as hosted.
    fn accept(&self) -> Peer {
        let deadline = Instant::now() + STEP;
        loop {
            match self.entrance.accept() {
                Ok((mut socket, _)) => {
                    socket.set_nonblocking(false).unwrap();
                    // Read the relay's name line, then answer it as hosted.
                    let mut byte = [0];
                    while byte[0] != b'\n' {
                        socket.read_exact(&mut byte).unwrap();
                    }
                    socket
                        .write_all(b"{\"answer\":\"hosted\",\"revision\":1}\n")
                        .unwrap();
                    return Peer {
                        socket,
                        decoder: Decoder::default(),
                        input: Vec::new(),
                    };
                }
                Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {
                    assert!(Instant::now() < deadline, "the relay did not connect");
                    std::thread::sleep(Duration::from_millis(2));
                }
                Err(error) => panic!("{error}"),
            }
        }
    }

    /// Closes every connection unanswered, as a stopped server's entrance
    /// would, until `done`; returns how many it closed.
    fn refuse_until(&self, done: impl Fn() -> bool) -> usize {
        let deadline = Instant::now() + STEP;
        let mut refused = 0;
        while !done() {
            assert!(Instant::now() < deadline, "the relay kept reconnecting");
            if let Ok((socket, _)) = self.entrance.accept() {
                drop(socket);
                refused += 1;
            }
            std::thread::sleep(Duration::from_millis(2));
        }
        refused
    }

    /// Retry only reconnects: setup ran for the first connection alone.
    fn assert_setup_ran_once(&self) {
        let attempts = std::fs::read_to_string(self.home.join("setup-attempts")).unwrap();
        assert_eq!(attempts, "install server\n", "Retry only reconnects");
    }

    /// Has the parent type `keys` at the pane's terminal.
    fn type_keys(&self, keys: &[u8]) {
        let typed = self.home.join("typed");
        std::fs::write(self.home.join("type.pending"), keys).unwrap();
        std::fs::rename(self.home.join("type.pending"), self.home.join("type")).unwrap();
        wait_until(|| typed.exists(), "the parent typed");
        std::fs::remove_file(typed).unwrap();
    }
}

/// One connection from the relay, as its host sees it.
struct Peer {
    socket: UnixStream,
    decoder: Decoder,
    input: Vec<u8>,
}

impl Peer {
    /// The next request within `wait`, collecting input frames on the way.
    fn next_request(&mut self, wait: Duration) -> Option<Request> {
        let deadline = Instant::now() + wait;
        loop {
            while let Some((tag, payload)) = self.decoder.next().unwrap() {
                match tag {
                    Tag::Input => self.input.extend(payload),
                    Tag::Request => {
                        let incoming: Incoming = serde_json::from_slice(&payload).unwrap();
                        return Some(incoming.request);
                    }
                    other => panic!("unexpected {other:?} from the relay"),
                }
            }
            let left = deadline.saturating_duration_since(Instant::now());
            if left.is_zero() {
                return None;
            }
            self.socket.set_read_timeout(Some(left)).unwrap();
            let mut bytes = [0; 4096];
            match self.socket.read(&mut bytes) {
                Ok(0) => return None,
                Ok(n) => self.decoder.extend(&bytes[..n]),
                Err(error)
                    if matches!(
                        error.kind(),
                        std::io::ErrorKind::WouldBlock | std::io::ErrorKind::TimedOut
                    ) =>
                {
                    return None
                }
                Err(error) => panic!("{error}"),
            }
        }
    }

    fn reply(&mut self, reply: Reply) {
        let reply = Outgoing {
            answers: 0,
            seq: 0,
            reply,
        };
        self.socket
            .write_all(&wire::frame(
                Tag::Reply,
                &serde_json::to_vec(&reply).unwrap(),
            ))
            .unwrap();
    }

    /// Reads the attachment and acknowledges it, with input when `control`.
    fn attached(&mut self, control: bool) {
        assert!(matches!(
            self.next_request(STEP),
            Some(Request::Attach { .. })
        ));
        self.attached_reply(control);
    }

    fn attached_reply(&mut self, control: bool) {
        self.reply(Reply::Attached {
            viewer: "viewer".into(),
            reconnect: "reconnect".into(),
            nonce: "nonce".into(),
            token: control.then(|| "claim".into()),
            terminal: true,
            grace_ms: 60_000,
            owner: None,
        });
    }

    fn read_input(&mut self, expected: &[u8]) {
        let deadline = Instant::now() + STEP;
        while !self.input.ends_with(expected) {
            assert!(Instant::now() < deadline, "input {:?}", self.input);
            self.next_request(PACE.heartbeat);
        }
        assert_eq!(self.input, expected, "only what was typed while connected");
    }

    /// The program exits, which ends the relay.
    fn exit(&mut self) {
        let exited = wire::event("nonce", 1, Event::ChildExited { code: 0 });
        self.socket
            .write_all(&wire::frame(Tag::Out, &exited))
            .unwrap();
    }
}

/// Desktop's end of a pane's status socket.
struct Desktop {
    path: PathBuf,
    listener: UnixListener,
    state: std::sync::Mutex<(Option<UnixStream>, Vec<u8>, Value)>,
}

impl Desktop {
    fn new(path: PathBuf) -> Self {
        let listener = UnixListener::bind(&path).unwrap();
        listener.set_nonblocking(true).unwrap();
        Self {
            path,
            listener,
            state: std::sync::Mutex::new((None, Vec::new(), Value::Null)),
        }
    }

    fn reading(&self) -> Value {
        let mut state = self.state.lock().unwrap();
        let (socket, received, reading) = &mut *state;
        if let Ok((stream, _)) = self.listener.accept() {
            stream.set_nonblocking(true).unwrap();
            *socket = Some(stream);
            received.clear();
        }
        let mut bytes = [0; 8192];
        while let Some(stream) = socket {
            match stream.read(&mut bytes) {
                Ok(0) => *socket = None,
                Ok(n) => received.extend_from_slice(&bytes[..n]),
                Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => break,
                Err(error) => panic!("status: {error}"),
            }
        }
        while let Some(end) = received.iter().position(|byte| *byte == b'\n') {
            let line: Vec<_> = received.drain(..=end).collect();
            *reading = serde_json::from_slice(&line).unwrap();
        }
        reading.clone()
    }

    fn wait(&self, state: &str, input: bool, generation: u64) {
        let deadline = Instant::now() + STEP;
        loop {
            let reading = self.reading();
            if reading["state"] == state
                && reading["input"] == input
                && reading["generation"] == generation
            {
                return;
            }
            assert!(
                Instant::now() < deadline,
                "expected {state}/{input}/{generation}, read {reading}"
            );
            std::thread::sleep(Duration::from_millis(2));
        }
    }

    /// Drops the relay's status connection, as a busy or restarting Desktop can.
    fn disconnect(&self) {
        *self.state.lock().unwrap() = (None, Vec::new(), Value::Null);
    }

    fn send(&self, action: Value) {
        self.reading();
        let mut state = self.state.lock().unwrap();
        let stream = state.0.as_mut().expect("the relay reports status");
        writeln!(stream, "{action}").unwrap();
    }
}

fn wait_until(done: impl Fn() -> bool, what: &str) {
    let deadline = Instant::now() + STEP;
    while !done() {
        assert!(Instant::now() < deadline, "timed out waiting until {what}");
        std::thread::sleep(Duration::from_millis(2));
    }
}

// ---------------------------------------------------------------------------
// Parent: the pane's terminal

struct RelayProcess(std::process::Child);

impl Drop for RelayProcess {
    fn drop(&mut self) {
        let _ = self.0.kill();
        let _ = self.0.wait();
    }
}

/// Runs `case` in a relay child on a PTY, typing what it asks for.
fn relay_case(case: &str) {
    let home = tempfile::Builder::new().tempdir_in("/tmp").unwrap();
    let pane = home.path().join("pane");
    std::fs::create_dir(&pane).unwrap();
    let installer = home.path().join("installer");
    // The test constructor gives the child its own HOME; log beside LF_HOME.
    let fails = if case == "setup" {
        "echo 'fixture Background Items disabled' >&2; exit 1"
    } else {
        ""
    };
    std::fs::write(
        &installer,
        format!(
            "#!/bin/sh\necho \"$*\" >> {}\n{fails}\n",
            home.path().join("setup-attempts").display()
        ),
    )
    .unwrap();
    std::fs::set_permissions(&installer, std::fs::Permissions::from_mode(0o755)).unwrap();
    let (mut master, slave) = terminal_pair().unwrap();
    let mut child = RelayProcess(
        Command::new(std::env::current_exe().unwrap())
            .env_clear()
            .env("HOME", home.path())
            .env("PATH", "/usr/bin:/bin")
            .env("LF_HOME", home.path())
            .env("LF_BIN", &installer)
            .env("LF_SERVER_STATUS", pane.join("status.sock"))
            .env(CASE, case)
            .args([
                "--exact",
                "transport::relay_tests::relay_fixture",
                "--nocapture",
            ])
            .stdin(slave.try_clone().unwrap())
            .stdout(slave.try_clone().unwrap())
            .stderr(Stdio::piped())
            .spawn()
            .unwrap(),
    );
    drop(slave);
    let deadline = Instant::now() + Duration::from_secs(30);
    let mut output = Vec::new();
    let status = loop {
        assert!(
            Instant::now() < deadline,
            "relay case {case} timed out: {}",
            String::from_utf8_lossy(&output)
        );
        drain(&mut master, &mut output, 20);
        let request = home.path().join("type");
        if let Ok(keys) = std::fs::read(&request) {
            std::fs::remove_file(&request).unwrap();
            master.write_all(&keys).unwrap();
            std::fs::write(home.path().join("typed"), "").unwrap();
        }
        if let Some(status) = child.0.try_wait().unwrap() {
            break status;
        }
    };
    let mut error = String::new();
    child
        .0
        .stderr
        .take()
        .unwrap()
        .read_to_string(&mut error)
        .unwrap();
    // A terminal relay records its diagnostics, panics included, in logs/.
    for log in std::fs::read_dir(home.path().join("logs"))
        .into_iter()
        .flatten()
    {
        error += &std::fs::read_to_string(log.unwrap().path()).unwrap_or_default();
    }
    assert!(
        status.success(),
        "relay case {case}: {error}\n{}",
        String::from_utf8_lossy(&output)
    );
    assert!(
        home.path().join("relayed").exists(),
        "relay case {case} did not run: {error}"
    );
}

fn drain(master: &mut std::fs::File, output: &mut Vec<u8>, timeout_ms: i32) {
    let mut fd = libc::pollfd {
        fd: master.as_raw_fd(),
        events: libc::POLLIN,
        revents: 0,
    };
    // SAFETY: poll receives one live descriptor.
    if unsafe { libc::poll(&mut fd, 1, timeout_ms) } > 0 {
        let mut bytes = [0; 16384];
        if let Ok(n) = master.read(&mut bytes) {
            output.extend_from_slice(&bytes[..n]);
        }
    }
}

#[test]
fn a_quiet_host_stays_attached_and_a_silent_bridge_reconnects() {
    relay_case("quiet");
}

#[test]
fn an_unacknowledged_attachment_reconnects_to_the_same_terminal() {
    relay_case("unacknowledged");
}

#[test]
fn outage_input_is_discarded_and_retry_reconnects_after_exhaustion() {
    relay_case("outage");
}

#[test]
fn failed_setup_retry_only_reconnects_without_taking_claims() {
    relay_case("setup");
}

#[test]
fn closing_the_pane_cancels_a_pending_reconnect() {
    relay_case("closure");
}

#[test]
fn move_here_is_consumed_before_its_takeover_is_sent() {
    relay_case("move");
}
