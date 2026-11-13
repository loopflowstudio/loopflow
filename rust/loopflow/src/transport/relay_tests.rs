//! Relay rules, run in a child of this test binary on a real PTY: a Desktop
//! pane's relay to a held terminal, and a terminal's relay to its owner. The
//! child plays the server's entrance and Desktop's status socket, or the
//! owner's socket, itself, with the relay's pace shortened; the parent owns
//! the PTY and types when the child asks.

use std::io::{Read, Write};
use std::os::fd::AsRawFd;
use std::os::unix::fs::PermissionsExt;
use std::os::unix::net::{UnixListener, UnixStream};
use std::path::PathBuf;
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

use serde_json::{json, Value};

use super::wire::{self, Decoder, Event, Incoming, Outgoing, Own, Reply, Request, Tag};
use super::{attach_with_terminal, endpoint, registry, terminal_pair, Host, Pace, TEST_PACE};
use crate::id::{LfProcessId, LfSessionId, TerminalId};

const SESSION: &str = "session_5e55105e000040008000000000000002";
const CASE: &str = "LOOPFLOW_RELAY_FIXTURE";
/// How long any one step of a case may take before it fails.
const STEP: Duration = Duration::from_secs(5);
const PACE: Pace = Pace {
    heartbeat: Duration::from_millis(50),
    silence: Duration::from_millis(300),
    attach: Duration::from_millis(300),
    backoff: Duration::from_millis(1),
    stall: Duration::from_millis(200),
};
/// Cases relaying to an owner's socket directly, with no server or pane.
const LOCAL: [&str; 4] = ["handoff", "acknowledged", "unacknowledged-owner", "stalled"];

// ---------------------------------------------------------------------------
// Child: the relay, its server and its Desktop pane

#[test]
fn relay_fixture() {
    let Ok(case) = std::env::var(CASE) else {
        return;
    };
    TEST_PACE.set(PACE);
    let home = crate::store::lf_home_dir();
    let (host, scenario): (Host, Box<dyn FnOnce() + Send>) = if LOCAL.contains(&case.as_str()) {
        let owner = LfProcessId::new();
        let listener = UnixListener::bind(endpoint(&owner).unwrap()).unwrap();
        let owner_socket = Owner {
            listener,
            home: home.clone(),
        };
        let scenario = move || match case.as_str() {
            "handoff" => control_handoff(&owner_socket),
            "acknowledged" => acknowledged_past_the_deadline(&owner_socket),
            "unacknowledged-owner" => unacknowledged_owner(&owner_socket),
            "stalled" => stalled_owner(&owner_socket),
            other => panic!("unknown relay case {other}"),
        };
        (Host::Owner(owner), Box::new(scenario))
    } else {
        let entrance = UnixListener::bind(registry::entrance().unwrap()).unwrap();
        entrance.set_nonblocking(true).unwrap();
        let desktop = Desktop::new(std::env::var_os("LF_SERVER_STATUS").unwrap().into());
        let server = Server {
            entrance,
            home: home.clone(),
        };
        let scenario = move || match case.as_str() {
            "quiet" => quiet_host_then_silent_bridge(&server, &desktop),
            "unacknowledged" => unacknowledged_attachment(&server, &desktop),
            "outage" => outage_and_retry(&server, &desktop),
            "setup" => failed_setup(&server, &desktop),
            "closure" => pane_closure(&server, &desktop),
            "move" => move_here(&server, &desktop),
            other => panic!("unknown relay case {other}"),
        };
        (Host::Terminal(TerminalId::new()), Box::new(scenario))
    };
    let scenario = std::thread::spawn(move || {
        // The relay may still be waiting; a failed case ends the child now.
        if std::panic::catch_unwind(std::panic::AssertUnwindSafe(scenario)).is_err() {
            std::process::exit(101);
        }
    });
    let relayed = attach_with_terminal(&host, Some(&LfSessionId::from(SESSION)), None, None, false);
    scenario.join().unwrap();
    let error = relayed.err().map(|error| format!("{error:#}"));
    std::fs::write(home.join("relayed"), error.unwrap_or_default()).unwrap();
}

/// The pane reads connecting, and takes no keys, until its attachment is
/// acknowledged. Heartbeats a quiet host answers keep the relay on its first
/// connection; a bridge that still accepts writes but stops answering is
/// replaced.
fn quiet_host_then_silent_bridge(server: &Server, desktop: &Desktop) {
    let mut first = server.accept();
    desktop.wait("connecting", false, 1);
    server.type_keys(b"not yet\n");
    first.attached(true, true);
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
    assert!(first.input.is_empty(), "{:?}", first.input);
    // `first` stays open and readable, so only silence can end it.
    let mut second = server.accept();
    second.attached(true, true);
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
    second.attached(true, true);
    desktop.wait("connected", true, 2);
    second.exit();
    drop(first);
}

/// Exhaustion reads unavailable; keys typed meanwhile are discarded, and Retry
/// reconnects without running setup again.
fn outage_and_retry(server: &Server, desktop: &Desktop) {
    let mut first = server.accept();
    first.attached(true, true);
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
    second.attached(true, true);
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
    first.attached_reply(false, true);
    desktop.wait("connected", false, 1);
    first.exit();
    assert!(first.input.is_empty(), "{:?}", first.input);
    server.assert_setup_ran_once();
}

/// Closing the pane ends a relay waiting to reconnect, and nothing else.
fn pane_closure(server: &Server, desktop: &Desktop) {
    let mut first = server.accept();
    first.attached(true, true);
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
    first.attached(false, true);
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
    second.attached_reply(false, true);
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

/// Losing control restores the terminal, prints the owner's notice and
/// leaves; nothing runs after it.
fn control_handoff(owner: &Owner) {
    let mut viewer = owner.accept();
    viewer.attached(true, true);
    viewer.send(Tag::Snapshot, b"\x1b[?1049h");
    let notice = Event::Control {
        held: false,
        notice: Some("owner retired".into()),
    };
    viewer.send(Tag::Out, &wire::event("nonce", 1, notice));
    assert!(matches!(viewer.next_request(STEP), Some(Request::Leave)));
}

/// Only the handshake is bounded: an acknowledged viewer stays attached past
/// the attach deadline and its input still arrives.
fn acknowledged_past_the_deadline(owner: &Owner) {
    let mut viewer = owner.accept();
    viewer.attached(true, false);
    let past = Instant::now() + PACE.attach * 3;
    while Instant::now() < past {
        if let Some(Request::Controlled { .. }) = viewer.next_request(PACE.heartbeat * 2) {
            viewer.reply(Reply::Accepted);
        }
    }
    owner.type_keys(b"accepted\n");
    viewer.read_input(b"accepted\n");
    viewer.exit();
}

/// An owner that never acknowledges ends the viewer at the deadline, and
/// nothing typed meanwhile reaches it.
fn unacknowledged_owner(owner: &Owner) {
    let mut viewer = owner.accept();
    assert!(matches!(
        viewer.next_request(STEP),
        Some(Request::Attach { .. })
    ));
    owner.type_keys(b"must-not-submit\n");
    viewer.socket.set_read_timeout(Some(STEP)).unwrap();
    let mut rest = Vec::new();
    viewer.socket.read_to_end(&mut rest).unwrap();
    viewer.decoder.extend(&rest);
    while viewer.next_request(Duration::ZERO).is_some() {}
    assert!(viewer.input.is_empty(), "{:?}", viewer.input);
}

/// An owner that answers but stops reading ends the viewer once a write
/// stalls, saying why.
fn stalled_owner(owner: &Owner) {
    let mut viewer = owner.accept();
    viewer.attached(true, false);
    let mut answers = viewer.socket.try_clone().unwrap();
    // Answering without reading keeps the viewer from judging silence.
    std::thread::spawn(move || {
        let accepted = Outgoing {
            answers: 0,
            seq: 0,
            reply: Reply::Accepted,
        };
        let accepted = wire::frame(Tag::Reply, &serde_json::to_vec(&accepted).unwrap());
        while answers.write_all(&accepted).is_ok() {
            std::thread::sleep(PACE.heartbeat);
        }
    });
    std::fs::write(owner.home.join("flood"), "").unwrap();
    // The socket stays open and unread until the child exits.
    std::mem::forget(viewer);
}

/// An owner's socket, as its viewer reaches it.
struct Owner {
    listener: UnixListener,
    home: PathBuf,
}

impl Owner {
    fn accept(&self) -> Peer {
        let (socket, _) = self.listener.accept().unwrap();
        Peer {
            socket,
            decoder: Decoder::default(),
            input: Vec::new(),
        }
    }

    fn type_keys(&self, keys: &[u8]) {
        type_keys(&self.home, keys);
    }
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

    fn type_keys(&self, keys: &[u8]) {
        type_keys(&self.home, keys);
    }
}

/// Has the parent type `keys` at the relay's terminal.
fn type_keys(home: &std::path::Path, keys: &[u8]) {
    let typed = home.join("typed");
    std::fs::write(home.join("type.pending"), keys).unwrap();
    std::fs::rename(home.join("type.pending"), home.join("type")).unwrap();
    wait_until(|| typed.exists(), "the parent typed");
    std::fs::remove_file(typed).unwrap();
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

    fn send(&mut self, tag: Tag, payload: &[u8]) {
        self.socket.write_all(&wire::frame(tag, payload)).unwrap();
    }

    fn reply(&mut self, reply: Reply) {
        let reply = Outgoing {
            answers: 0,
            seq: 0,
            reply,
        };
        self.send(Tag::Reply, &serde_json::to_vec(&reply).unwrap());
    }

    /// Reads the attachment and acknowledges it, with input when `control`,
    /// for a program with or without a terminal.
    fn attached(&mut self, control: bool, terminal: bool) {
        assert!(matches!(
            self.next_request(STEP),
            Some(Request::Attach { .. })
        ));
        self.attached_reply(control, terminal);
    }

    fn attached_reply(&mut self, control: bool, terminal: bool) {
        self.reply(Reply::Attached {
            viewer: "viewer".into(),
            reconnect: "reconnect".into(),
            nonce: "nonce".into(),
            token: control.then(|| "claim".into()),
            terminal,
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
        self.send(Tag::Out, &exited);
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
// Parent: the relay's terminal

struct RelayProcess(std::process::Child);

impl Drop for RelayProcess {
    fn drop(&mut self) {
        let _ = self.0.kill();
        let _ = self.0.wait();
    }
}

/// What a relay case left behind.
struct Relayed {
    /// The relay's error, empty when it returned normally.
    error: String,
    screen: String,
    stderr: String,
}

/// Runs `case` in a relay child on a PTY, typing what it asks for.
fn relay_case(case: &str) -> Relayed {
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
    let mut command = Command::new(std::env::current_exe().unwrap());
    command
        .env_clear()
        .env("HOME", home.path())
        .env("PATH", "/usr/bin:/bin")
        .env("LF_HOME", home.path())
        .env("LF_BIN", &installer)
        .env(CASE, case)
        .args([
            "--exact",
            "transport::relay_tests::relay_fixture",
            "--nocapture",
        ])
        .stdin(slave.try_clone().unwrap())
        .stdout(slave.try_clone().unwrap())
        .stderr(Stdio::piped());
    if !LOCAL.contains(&case) {
        command.env("LF_SERVER_STATUS", pane.join("status.sock"));
    }
    let mut child = RelayProcess(command.spawn().unwrap());
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
        if std::fs::remove_file(home.path().join("flood")).is_ok() {
            // More input than any socket buffers, typed until the relay's
            // terminal goes away.
            let mut flood = master.try_clone().unwrap();
            std::thread::spawn(move || {
                let line = [&[b'x'; 99][..], b"\n"].concat();
                while flood.write_all(&line).is_ok() {}
            });
        }
        if let Some(status) = child.0.try_wait().unwrap() {
            break status;
        }
    };
    let mut stderr = String::new();
    child
        .0
        .stderr
        .take()
        .unwrap()
        .read_to_string(&mut stderr)
        .unwrap();
    drain(&mut master, &mut output, 0);
    let mut diagnostics = stderr.clone();
    // A terminal relay records its diagnostics, panics included, in logs/.
    for log in std::fs::read_dir(home.path().join("logs"))
        .into_iter()
        .flatten()
    {
        diagnostics += &std::fs::read_to_string(log.unwrap().path()).unwrap_or_default();
    }
    let screen = String::from_utf8_lossy(&output).into_owned();
    assert!(
        status.success(),
        "relay case {case}: {diagnostics}\n{screen}"
    );
    let error = std::fs::read_to_string(home.path().join("relayed"))
        .unwrap_or_else(|_| panic!("relay case {case} did not run: {diagnostics}"));
    Relayed {
        error,
        screen,
        stderr,
    }
}

/// Runs `case`, which the relay ends normally.
fn relays(case: &str) -> Relayed {
    let relayed = relay_case(case);
    assert!(
        relayed.error.is_empty(),
        "relay case {case}: {}",
        relayed.error
    );
    relayed
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
    relays("quiet");
}

#[test]
fn an_unacknowledged_attachment_reconnects_to_the_same_terminal() {
    relays("unacknowledged");
}

#[test]
fn outage_input_is_discarded_and_retry_reconnects_after_exhaustion() {
    relays("outage");
}

#[test]
fn failed_setup_retry_only_reconnects_without_taking_claims() {
    relays("setup");
}

#[test]
fn closing_the_pane_cancels_a_pending_reconnect() {
    relays("closure");
}

#[test]
fn move_here_is_consumed_before_its_takeover_is_sent() {
    relays("move");
}

#[test]
fn control_handoff_restores_the_terminal_and_prints_the_notice() {
    let relayed = relays("handoff");
    assert!(
        relayed.screen.contains("\x1b[?1049l"),
        "{:?}",
        relayed.screen
    );
    assert!(
        relayed.stderr.contains("owner retired"),
        "{}",
        relayed.stderr
    );
}

#[test]
fn an_acknowledged_viewer_stays_attached_past_the_attach_deadline() {
    relays("acknowledged");
}

#[test]
fn an_owner_that_never_acknowledges_ends_the_viewer_without_its_input() {
    let error = relay_case("unacknowledged-owner").error;
    assert!(error.contains("did not acknowledge attachment"), "{error}");
    assert!(error.contains("outcome is unknown"), "{error}");
}

#[test]
fn an_owner_that_stops_reading_input_ends_the_viewer_with_a_cause() {
    let error = relay_case("stalled").error;
    assert!(error.contains("stopped reading requests"), "{error}");
    assert!(error.contains("later input was not delivered"), "{error}");
    assert!(!error.contains("os error"), "{error}");
}
