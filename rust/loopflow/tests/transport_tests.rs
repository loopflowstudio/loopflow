//! Real detached owner and Session attachment; no provider credentials or display.
#![cfg(unix)]

mod support;

use std::io::{BufRead, BufReader, Read, Write};
use std::os::fd::FromRawFd;
use std::os::unix::fs::PermissionsExt;
use std::os::unix::net::{UnixListener, UnixStream};
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Output, Stdio};
use std::time::{Duration, Instant};

use loopflow_test_support::TestRepo;
use serde_json::{json, Value};
use sha2::{Digest, Sha256};

struct Fixture {
    home: tempfile::TempDir,
    repo: TestRepo,
}
impl Fixture {
    fn new() -> Self {
        Self {
            // An OSC opcode in an ordinary path is not terminal status.
            home: tempfile::Builder::new()
                .prefix("lf-7501-")
                .tempdir_in("/tmp")
                .unwrap(),
            repo: TestRepo::new(),
        }
    }
    fn command(&self, args: &[&str]) -> Command {
        self.command_from(Path::new(env!("CARGO_BIN_EXE_lf")), args)
    }
    /// `command` run from another lf executable, such as a removable copy.
    fn command_from(&self, program: &Path, args: &[&str]) -> Command {
        let mut command = Command::new(program);
        for (key, _) in std::env::vars_os() {
            // An inherited cmux workspace would receive the fixture Session's name.
            if ["LF_", "LOOPFLOW_", "CMUX_"]
                .iter()
                .any(|prefix| key.to_string_lossy().starts_with(prefix))
            {
                command.env_remove(key);
            }
        }
        command
            .args(args)
            .current_dir(self.repo.path())
            .env("HOME", self.home.path())
            .env("LF_HOME", self.home.path())
            .env("LF_BIN", env!("CARGO_BIN_EXE_lf"))
            .env("RUST_LOG", "off")
            .stdin(Stdio::null());
        command
    }
    /// Launch a detached owner the way a delegating execution does, returning its Process.
    fn owner(&self, mut command: Command) -> String {
        let logs = self.home.path().join("logs");
        std::fs::create_dir_all(&logs).unwrap();
        let log = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(logs.join("transport.log"))
            .unwrap();
        let mut child = command.stdout(Stdio::piped()).stderr(log).spawn().unwrap();
        let mut line = String::new();
        BufReader::new(child.stdout.take().unwrap())
            .read_line(&mut line)
            .unwrap();
        std::thread::spawn(move || child.wait());
        assert!(!line.trim().is_empty(), "owner exited before acknowledging");
        line.trim().to_owned()
    }
    fn serve(&self, terminal: bool, argv: &[&str]) -> Command {
        let mut command = self.command(&["session", "serve"]);
        if terminal {
            command.arg("--terminal");
        }
        command.arg("--").args(argv);
        command
    }
    fn start(&self, argv: &[&str]) -> String {
        self.owner(self.serve(true, argv))
    }
    /// A Session row naming `owner`, so `lf session resume` attaches to it.
    fn session(&self, owner: &str) -> String {
        let session = "session_5e55105e000040008000000000000001";
        let sql = rusqlite::Connection::open(self.home.path().join("loopflow.db")).unwrap();
        sql.execute(
            "INSERT INTO agent_sessions(id,title,title_source,created_at,input_published,cwd,repo,agent_process_id,transport_lf_process_id)
             VALUES(?1,'fixture','generated',1,0,?2,?3,?4,?4)",
            rusqlite::params![
                session,
                self.repo.path().to_str().unwrap(),
                loopflow::repository::CanonicalRepo::discover(self.repo.path())
                    .unwrap()
                    .to_string(),
                owner
            ],
        )
        .unwrap();
        sql.execute(
            "INSERT INTO session_events(session_id,kind,receipt_key,observed_at,payload)
             VALUES(?1,'captured','run_5e55105e000040008000000000000002',1,json_object('artifact_key','run_5e55105e000040008000000000000002'))",
            [session],
        )
        .unwrap();
        sql.execute(
            "UPDATE agent_sessions SET current_capture=?1 WHERE id=?2",
            rusqlite::params![sql.last_insert_rowid(), session],
        )
        .unwrap();
        session.to_owned()
    }
    fn endpoint(&self, id: &str) -> PathBuf {
        let digest = hex::encode(Sha256::digest(
            self.home
                .path()
                .canonicalize()
                .unwrap()
                .as_os_str()
                .as_encoded_bytes(),
        ));
        // SAFETY: geteuid has no preconditions.
        PathBuf::from(format!(
            "/tmp/lf-{}-{}/{id}.sock",
            unsafe { libc::geteuid() },
            &digest[..16]
        ))
    }
    fn viewer(&self, id: &str, control: bool) -> Viewer {
        self.attach(
            id,
            json!({"kind":"attach","control":control,"replace":true,"rows":24,"cols":80}),
        )
    }
    fn attach(&self, id: &str, request: Value) -> Viewer {
        let socket = UnixStream::connect(self.endpoint(id)).unwrap_or_else(|e| {
            panic!(
                "{}: {e}; home {}",
                self.endpoint(id).display(),
                self.home.path().display()
            )
        });
        socket
            .set_read_timeout(Some(Duration::from_secs(15)))
            .unwrap();
        let mut viewer = Viewer {
            socket: BufReader::new(socket),
            token: None,
        };
        viewer.send(request);
        let attached = viewer.read();
        assert_eq!(attached["kind"], "attached");
        viewer.token = attached["token"].as_str().map(str::to_owned);
        viewer
    }
    fn sql(&self) -> rusqlite::Connection {
        rusqlite::Connection::open(self.home.path().join("loopflow.db")).unwrap()
    }
    /// Put a `claude` stand-in that logs each launch to `launches` and waits on
    /// its terminal first on a PATH, returned.
    fn waiting_provider(&self) -> String {
        let bin = self.home.path().join("bin");
        std::fs::create_dir_all(&bin).unwrap();
        let provider = bin.join("claude");
        std::fs::write(
            &provider,
            r#"#!/bin/sh
if [ "$1" = --version ]; then echo '2.1.302'; exit; fi
stty -echo < /dev/tty
printf '%s:%s:%s\n' "$$" "$LF_CAPTURE_KEY" "$*" >> "$HOME/launches"
read -r answer < /dev/tty
"#,
        )
        .unwrap();
        std::fs::set_permissions(&provider, std::fs::Permissions::from_mode(0o755)).unwrap();
        format!(
            "{}:{}",
            bin.display(),
            std::env::var("PATH").unwrap_or_default()
        )
    }
    /// The first `count` lines `waiting_provider` logged, once they exist.
    fn launches(&self, count: usize) -> Vec<String> {
        let deadline = Instant::now() + Duration::from_secs(30);
        loop {
            let text =
                std::fs::read_to_string(self.home.path().join("launches")).unwrap_or_default();
            let lines: Vec<String> = text.lines().map(str::to_owned).collect();
            if lines.len() >= count {
                return lines;
            }
            assert!(Instant::now() < deadline, "{count} launches; saw {lines:?}");
            std::thread::sleep(Duration::from_millis(20));
        }
    }
    /// Wait until a viewer holds `owner`'s input control.
    fn wait_controlled(&self, owner: &str) {
        let deadline = Instant::now() + Duration::from_secs(30);
        loop {
            let mut socket = UnixStream::connect(self.endpoint(owner)).unwrap();
            writeln!(socket, "{}", json!({"kind":"controlled"})).unwrap();
            let mut line = String::new();
            BufReader::new(socket).read_line(&mut line).unwrap();
            if serde_json::from_str::<Value>(&line).unwrap()["held"] == true {
                return;
            }
            assert!(
                Instant::now() < deadline,
                "no viewer took control of {owner}"
            );
            std::thread::sleep(Duration::from_millis(50));
        }
    }
    fn session_of_owner(&self, owner: &str) -> String {
        self.sql()
            .query_row(
                "SELECT id FROM agent_sessions WHERE transport_lf_process_id=?1",
                [owner],
                |r| r.get(0),
            )
            .unwrap()
    }
    /// Record a turn in progress for `session`.
    fn mark_working(&self, session: &str) {
        self.sql()
            .execute(
                r#"INSERT INTO session_activity(session_id,observed_at,open_tools,pending_input,yielded,agent_process_id,program_status)
                   SELECT id,0,1,0,0,agent_process_id,'{"seen":true,"records":[{"state":"working","id":null,"kind":null,"progress":null,"app":null,"title":null,"msg":null}]}' FROM agent_sessions WHERE id=?1"#,
                [session],
            )
            .unwrap();
    }
    fn running(&self, owner: &str) -> bool {
        self.sql()
            .query_row(
                "SELECT completed_at IS NULL FROM processes WHERE id=?1",
                [owner],
                |r| r.get(0),
            )
            .unwrap()
    }
    fn ends(&self, owner: &str) {
        let deadline = Instant::now() + Duration::from_secs(20);
        while self.running(owner) {
            assert!(
                Instant::now() < deadline,
                "{owner} kept running; logs: {:?}",
                std::fs::read_to_string(self.home.path().join("logs/transport.log"))
            );
            std::thread::sleep(Duration::from_millis(50));
        }
    }
    /// Signal an owner the test leaves running.
    fn stop(&self, owner: &str) {
        let pid: i64 = self
            .sql()
            .query_row("SELECT pid FROM processes WHERE id=?1", [owner], |r| {
                r.get(0)
            })
            .unwrap();
        // SAFETY: test-owned owner PID recorded by this fixture's store.
        unsafe { libc::kill(pid as libc::pid_t, libc::SIGTERM) };
    }
    fn wait_file(&self, name: &str) -> String {
        let deadline = Instant::now() + Duration::from_secs(30);
        loop {
            if let Ok(text) = std::fs::read_to_string(self.home.path().join(name)) {
                if !text.is_empty() {
                    return text;
                }
            }
            assert!(
                Instant::now() < deadline,
                "{name} missing; logs: {:?}",
                std::fs::read_to_string(self.home.path().join("logs/transport.log"))
            );
            std::thread::sleep(Duration::from_millis(20));
        }
    }
}
struct Viewer {
    socket: BufReader<UnixStream>,
    token: Option<String>,
}
impl Viewer {
    fn send(&mut self, value: Value) {
        writeln!(self.socket.get_mut(), "{value}").unwrap();
    }
    fn read(&mut self) -> Value {
        let mut line = String::new();
        self.socket.read_line(&mut line).unwrap();
        serde_json::from_str(&line).unwrap()
    }
    /// Every move notice received until the owner closes this connection,
    /// with the lf entry it names.
    fn moves(&mut self) -> Vec<(Option<String>, Option<String>)> {
        let mut moves = Vec::new();
        let mut line = String::new();
        while matches!(self.socket.read_line(&mut line), Ok(1..)) {
            let value: Value = serde_json::from_str(&line).unwrap();
            if value["kind"] == "moved" {
                moves.push((
                    value["notice"].as_str().map(str::to_owned),
                    value["successor"].as_str().map(str::to_owned),
                ));
            }
            line.clear();
        }
        moves
    }
    fn input(&mut self, text: &str) {
        self.send(json!({"kind":"input","token":self.token,"bytes":text.as_bytes()}));
    }
    fn finish(&mut self) {
        loop {
            let value = self.read();
            if value["kind"] == "exit" {
                assert_eq!(value["code"], 0);
                break;
            }
        }
    }
}

#[test]
fn zero_viewers_preserve_shell_jobs_and_fence_replaced_input_claims() {
    let fixture = Fixture::new();
    let owner = fixture.start(&[
        "/bin/sh",
        "-c",
        r#"
        stty -echo
        cd "$HOME"
        retained=value
        (printf detached > job) &
        printf '%s\n' "$$" > pid
        printf 'pending-request-17\n'
        read -r answer
        wait
        printf '%s:%s:%s:%s\n' "$$" "$retained" "$PWD" "$answer" > answer
    "#,
    ]);
    let pid = fixture.wait_file("pid");
    assert_eq!(fixture.wait_file("job"), "detached");
    let mut first = fixture.viewer(&owner, true);
    let second = fixture.attach(
        &owner,
        json!({"kind":"attach","control":true,"replace":true,"rows":24,"cols":80,
            "moved":"This conversation moved to tmux %4: lf resume s1"}),
    );
    // The replaced viewer is told where the conversation went, then let go.
    let moved = read_until(&mut first, "moved");
    assert_eq!(
        moved.last().unwrap()["notice"],
        "This conversation moved to tmux %4: lf resume s1"
    );
    let mut rest = String::new();
    assert_eq!(first.socket.read_line(&mut rest).unwrap(), 0);
    let mut observer = fixture.viewer(&owner, false);
    assert!(observer.token.is_none());
    observer.send(json!({"kind":"input","token":second.token,"bytes":b"passive\n"}));
    drop(observer);
    drop(first);
    drop(second); // zero viewers while the same read remains pending
    let mut replacement = fixture.viewer(&owner, true);
    let mut passive = fixture.viewer(&owner, false);
    passive.send(json!({"kind":"attach","control":false,"replace":true,"rows":24,"cols":80}));
    loop {
        if passive.read()["kind"] == "attached" {
            break;
        }
    }
    drop(passive); // observer disconnect cannot release the controller's claim
    replacement.input("accepted\n");
    replacement.finish();
    assert_eq!(
        fixture.wait_file("answer").trim(),
        format!(
            "{}:value:{}:accepted",
            pid.trim(),
            fixture.home.path().display()
        )
    );
}

#[test]
fn automatic_attachment_preserves_an_existing_viewers_control() {
    let fixture = Fixture::new();
    let owner = fixture.start(&[
        "/bin/sh",
        "-c",
        "stty -echo; read -r answer; printf '%s' \"$answer\" > \"$HOME/answer\"",
    ]);
    let mut controller = fixture.viewer(&owner, true);
    let mut waiting = fixture.viewer(&owner, false);
    waiting.send(json!({"kind":"attach","control":true,"replace":false,"rows":24,"cols":80}));
    loop {
        if waiting.read()["kind"] == "busy" {
            break;
        }
    }
    controller.input("original-controller\n");
    controller.finish();
    assert_eq!(fixture.wait_file("answer"), "original-controller");
}

#[test]
fn automatic_attachment_claims_control_after_the_previous_viewer_disconnects() {
    let fixture = Fixture::new();
    let owner = fixture.start(&[
        "/bin/sh",
        "-c",
        "stty -echo; read -r answer; printf '%s' \"$answer\" > \"$HOME/answer\"",
    ]);
    let controller = fixture.viewer(&owner, true);
    let token = controller.token.clone();
    // SHUT_WR lets the owner observe EOF before it accepts the next viewer.
    controller
        .socket
        .get_ref()
        .shutdown(std::net::Shutdown::Both)
        .unwrap();
    drop(controller);
    let mut waiting = fixture.viewer(&owner, false);
    waiting.send(json!({"kind":"attach","control":true,"replace":false,"rows":24,"cols":80}));
    loop {
        let frame = waiting.read();
        assert_ne!(frame["kind"], "busy");
        if frame["kind"] == "attached" {
            waiting.token = frame["token"].as_str().map(str::to_owned);
            break;
        }
    }
    assert_ne!(waiting.token, token);
    waiting.input("reconnected\n");
    waiting.finish();
    assert_eq!(fixture.wait_file("answer"), "reconnected");
}

/// Stands in for a Task step: registers its live controls with the owner, then
/// handles `detach` like `/detach` and exits on `done`.
const INPUT_HINT_STEP: &str = r#"
import hashlib, json, os, socket, sys
home = os.path.realpath(os.environ["LF_HOME"])
digest = hashlib.sha256(home.encode()).hexdigest()[:16]
path = f"{os.path.realpath('/tmp')}/lf-{os.geteuid()}-{digest}/{os.environ['LF_TRANSPORT_OWNER']}.sock"
key = os.environ["LF_TRANSPORT_CONTROL"]
def request(value):
    s = socket.socket(socket.AF_UNIX)
    s.connect(path)
    s.sendall((json.dumps(dict(value, key=key)) + "\n").encode())
    return s
hint = request({"kind": "input_hint", "hint": "type here"})
print("working", flush=True)
open(os.environ["HOME"] + "/registered", "w").write("1")
for line in sys.stdin:
    if line.strip() == "detach":
        request({"kind": "detach_viewers"}).close()
    if line.strip() == "done":
        break
"#;

fn read_until(viewer: &mut Viewer, kind: &str) -> Vec<Value> {
    let mut frames = Vec::new();
    loop {
        let frame = viewer.read();
        let done = frame["kind"] == kind;
        frames.push(frame);
        if done {
            return frames;
        }
    }
}

#[test]
fn only_controlling_viewers_see_live_input_controls() {
    let fixture = Fixture::new();
    let owner = fixture.owner(fixture.serve(false, &["python3", "-c", INPUT_HINT_STEP]));
    fixture.wait_file("registered");

    let mut controller = fixture.viewer(&owner, true);
    let hint = json!({"kind":"input_hint","hint":"type here"});
    assert!(read_until(&mut controller, "input_hint").contains(&hint));

    // Detaching viewers keeps the step's registration for the next one.
    controller.input("detach\n");
    read_until(&mut controller, "detached");
    let mut returning = fixture.viewer(&owner, true);
    read_until(&mut returning, "input_hint");
    let mut observer = fixture.viewer(&owner, false);
    returning.input("done\n");
    returning.finish();

    let frames = read_until(&mut observer, "exit");
    assert!(!frames.contains(&hint), "{frames:?}");
    let tail: String = frames
        .iter()
        .filter(|frame| frame["kind"] == "output")
        .flat_map(|frame| frame["bytes"].as_array().unwrap().clone())
        .map(|byte| byte.as_u64().unwrap() as u8 as char)
        .collect();
    assert_eq!(tail, "working\n");
}

#[test]
fn pipe_eof_delivers_all_input_before_the_command_exits() {
    let fixture = Fixture::new();
    let owner = fixture.owner(fixture.serve(false, &["/bin/sh", "-c", "cat; printf final >&2"]));
    let session = fixture.session(&owner);
    let mut viewer = fixture
        .command(&["session", "resume", &session])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    let input = b"literal ' input\nsecond line\n";
    viewer.stdin.take().unwrap().write_all(input).unwrap();
    let result = wait_for_viewer(viewer, Duration::from_secs(10));
    assert!(result.status.success(), "{result:?}");
    assert_eq!(result.stdout, input);
    assert_eq!(result.stderr, b"final");
}

#[test]
fn slow_observer_does_not_block_output_or_final_exit() {
    let fixture = Fixture::new();
    let owner = fixture.owner(fixture.serve(
        false,
        &[
            "/bin/sh",
            "-c",
            "i=0; while [ \"$i\" -lt 64 ]; do read -r start; dd if=/dev/zero bs=65536 count=1 2>/dev/null; i=$((i+1)); done; printf final >&2",
        ],
    ));
    let mut slow = fixture.viewer(&owner, false);
    let mut active = fixture.viewer(&owner, true);
    active.input("start\n");
    let mut stdout_bytes = 0;
    let mut stderr = Vec::new();
    loop {
        let frame = active.read();
        match frame["kind"].as_str().unwrap() {
            "output" => {
                let bytes: Vec<u8> = serde_json::from_value(frame["bytes"].clone()).unwrap();
                if frame["stream"] == 1 {
                    stdout_bytes += bytes.len();
                    // Pace the producer with the active consumer, while the
                    // observer accumulates enough unread output to disconnect.
                    if stdout_bytes < 4 * 1024 * 1024 && stdout_bytes % 65536 == 0 {
                        active.input("next\n");
                    }
                } else {
                    stderr.extend(bytes);
                }
            }
            "exit" => {
                assert_eq!(frame["code"], 0);
                break;
            }
            kind => panic!("unexpected frame: {kind}"),
        }
    }
    assert_eq!(stdout_bytes, 4 * 1024 * 1024);
    assert_eq!(stderr, b"final");
    let mut retained = Vec::new();
    slow.socket.read_to_end(&mut retained).unwrap();
    assert!(
        retained.len() < stdout_bytes * 2,
        "slow viewer was not disconnected"
    );
}

#[test]
fn native_pending_request_survives_creator_exit_without_settling_capture_or_replay() {
    native_pending_request(None);
}

#[test]
fn attachment_timeout_preserves_pending_execution_without_replay() {
    native_pending_request(Some(SilentOwner::Unacknowledged));
}

#[test]
fn attachment_cancellation_preserves_pending_execution_without_replay() {
    native_pending_request(Some(SilentOwner::Cancelled));
}

#[test]
fn an_owner_that_stops_reading_input_ends_the_viewer_with_a_cause() {
    native_pending_request(Some(SilentOwner::Stalled));
}

#[derive(Clone, Copy, PartialEq)]
enum SilentOwner {
    /// Accepts the attachment request and never answers it.
    Unacknowledged,
    /// As Unacknowledged, and the viewer is interrupted while it waits.
    Cancelled,
    /// Acknowledges the attachment, then never reads the viewer's input.
    Stalled,
}

#[test]
fn acknowledged_viewer_remains_attached_past_handshake_deadline() {
    let fixture = Fixture::new();
    let owner = fixture.owner(fixture.serve(
        false,
        &[
            "/bin/sh",
            "-c",
            "printf ready; read -r answer; printf '%s' \"$answer\"",
        ],
    ));
    let session = fixture.session(&owner);
    let mut viewers = Vec::new();
    {
        let mut viewer = fixture
            .command(&["session", "resume", &session])
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .unwrap();
        let mut ready = [0; 5];
        viewer
            .stdout
            .as_mut()
            .unwrap()
            .read_exact(&mut ready)
            .unwrap();
        assert_eq!(&ready, b"ready");
        viewers.push(viewer);
    }
    // The controller has acknowledged. Only the handshake is bounded, not
    // its connection.
    std::thread::sleep(Duration::from_secs(6));
    for viewer in &mut viewers {
        assert!(viewer.try_wait().unwrap().is_none());
    }
    viewers[0]
        .stdin
        .take()
        .unwrap()
        .write_all(b"accepted\n")
        .unwrap();
    for viewer in viewers {
        let output = wait_for_viewer(viewer, Duration::from_secs(3));
        assert!(output.status.success(), "{output:?}");
        assert_eq!(output.stdout, b"accepted");
    }
}

/// The Session a `waiting_provider` launch line belongs to.
fn session_of(sql: &rusqlite::Connection, line: &str) -> String {
    sql.query_row(
        "SELECT session_id FROM session_events WHERE kind='captured' AND receipt_key=?1",
        [line.split(':').nth(1).unwrap()],
        |r| r.get(0),
    )
    .unwrap()
}

fn wait_for_viewer(mut child: Child, timeout: Duration) -> Output {
    let deadline = Instant::now() + timeout;
    while child.try_wait().unwrap().is_none() {
        if Instant::now() >= deadline {
            child.kill().unwrap();
            let output = child.wait_with_output().unwrap();
            panic!("viewer did not exit within {timeout:?}: {output:?}");
        }
        std::thread::sleep(Duration::from_millis(20));
    }
    child.wait_with_output().unwrap()
}

fn silent_attachment(fixture: &Fixture, owner: &str, session: &str, silent: SilentOwner) {
    // Keep the real owner and provider running while this viewer reaches a
    // socket that accepts its request but never acknowledges it.
    let endpoint = fixture.endpoint(owner);
    let retained = endpoint.with_extension("retained");
    std::fs::rename(&endpoint, &retained).unwrap();
    let listener = UnixListener::bind(&endpoint).unwrap();
    listener.set_nonblocking(true).unwrap();
    let mut viewer = fixture
        .command(&["session", "resume", session])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    let mut input = viewer.stdin.take().unwrap();
    input.write_all(b"must-not-submit\n").unwrap();
    if silent == SilentOwner::Stalled {
        // More input than any socket buffers, so the viewer's sends must stall.
        std::thread::spawn(move || {
            let chunk = [b'x'; 64 * 1024];
            while input.write_all(&chunk).is_ok() {}
        });
    }
    let started = Instant::now();
    // This owner answers whether a viewer holds control, then never
    // acknowledges the attachment itself.
    let mut socket = loop {
        match listener.accept() {
            Ok((socket, _)) => {
                socket.set_nonblocking(false).unwrap();
                socket
                    .set_read_timeout(Some(Duration::from_secs(10)))
                    .unwrap();
                let mut socket = BufReader::new(socket);
                let mut request = String::new();
                socket.read_line(&mut request).unwrap();
                match serde_json::from_str::<Value>(&request).unwrap()["kind"]
                    .as_str()
                    .unwrap()
                {
                    "controlled" => writeln!(
                        socket.get_mut(),
                        "{}",
                        json!({"kind":"controlled","held":false})
                    )
                    .unwrap(),
                    "attach" => break socket,
                    other => panic!("unexpected request {other}"),
                }
            }
            Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {
                assert!(started.elapsed() < Duration::from_secs(10));
                std::thread::sleep(Duration::from_millis(20));
            }
            Err(error) => panic!("accept: {error}"),
        }
    };
    std::fs::remove_file(&endpoint).unwrap();
    std::fs::rename(retained, endpoint).unwrap();
    drop(listener);
    if silent == SilentOwner::Cancelled {
        // SAFETY: signal only this test's live viewer, never the owner/provider.
        assert_eq!(unsafe { libc::kill(viewer.id() as i32, libc::SIGINT) }, 0);
    }
    if silent == SilentOwner::Stalled {
        socket
            .get_mut()
            .write_all(
                b"{\"kind\":\"attached\",\"token\":\"claim\",\"terminal\":false,\"bytes\":[]}\n",
            )
            .unwrap();
        let output = wait_for_viewer(viewer, Duration::from_secs(8));
        assert!(!output.status.success());
        let error = String::from_utf8_lossy(&output.stderr);
        assert!(error.contains("stopped reading"), "{error}");
        assert!(!error.contains("os error"), "{error}");
        return;
    }
    let cancel = silent == SilentOwner::Cancelled;
    let output = wait_for_viewer(viewer, Duration::from_secs(if cancel { 2 } else { 8 }));
    assert!(!output.status.success());
    if cancel {
        assert_eq!(output.status.code(), Some(130));
    } else {
        let error = String::from_utf8_lossy(&output.stderr);
        assert!(error.contains("did not acknowledge attachment"), "{error}");
        assert!(error.contains("outcome is unknown"), "{error}");
    }
    let mut remaining = String::new();
    socket.read_to_string(&mut remaining).unwrap();
    assert!(
        remaining.is_empty(),
        "viewer submitted input before acknowledgement"
    );
}

fn native_pending_request(silent_owner: Option<SilentOwner>) {
    let fixture = Fixture::new();
    let bin = fixture.home.path().join("bin");
    std::fs::create_dir(&bin).unwrap();
    let provider = bin.join("claude");
    std::fs::write(
        &provider,
        r#"#!/bin/sh
if [ "$1" = --version ]; then echo '2.1.302'; exit; fi
stty -echo < /dev/tty
printf '%s:%s\n' "$$" "$LF_CAPTURE_KEY" >> "$HOME/launches"
printf 'permission request-17\n'
read -r answer < /dev/tty
printf '%s:%s\n' "$$" "$answer" > "$HOME/answer"
"#,
    )
    .unwrap();
    std::fs::set_permissions(&provider, std::fs::Permissions::from_mode(0o755)).unwrap();
    let mut owner = fixture.serve(
        true,
        &[
            env!("CARGO_BIN_EXE_lf"),
            "--interactive",
            "--agent",
            "claude",
            ":",
            "one original input",
        ],
    );
    owner.env(
        "PATH",
        format!(
            "{}:{}",
            bin.display(),
            std::env::var("PATH").unwrap_or_default()
        ),
    );
    let owner = fixture.owner(owner);
    let launched = fixture.wait_file("launches");
    let pid = launched.trim().split(':').next().unwrap();
    let sql = fixture.sql();
    let session = session_of(&sql, launched.trim());
    let attachment: (String, String) = sql.query_row("SELECT id,attachment_token FROM processes WHERE agent_session_id=?1 AND kind='agent' AND completed_at IS NULL", [&session], |r| Ok((r.get(0)?,r.get(1)?))).unwrap();
    let prepared = fixture
        .command(&["session", "resume", &session, "--prepare", "--json"])
        .output()
        .unwrap();
    assert!(prepared.status.success(), "{prepared:?}");
    let prepared: Value = serde_json::from_slice(&prepared.stdout).unwrap();
    let argv = prepared["open_argv"].as_array().unwrap();
    assert_eq!(
        &argv[argv.len() - 3..],
        &[json!("session"), json!("resume"), json!(session)]
    );
    assert_eq!(fixture.wait_file("launches"), launched);
    let first = fixture.viewer(&owner, true);
    drop(first);
    if let Some(silent) = silent_owner {
        silent_attachment(&fixture, &owner, &session, silent);
    }
    let observed: i64 = sql.query_row("SELECT count(*) FROM session_events WHERE session_id=?1 AND payload LIKE '%terminal.json%'", [&session], |r| r.get(0)).unwrap();
    assert_eq!(
        observed, 0,
        "creator exit or viewer drop settled the pending input"
    );
    let retained: (String, String) = sql.query_row("SELECT id,attachment_token FROM processes WHERE agent_session_id=?1 AND kind='agent' AND completed_at IS NULL", [&session], |r| Ok((r.get(0)?,r.get(1)?))).unwrap();
    assert_eq!(retained, attachment, "viewer changed provider ownership");
    assert!(!fixture.home.path().join("answer").exists());
    // The public Session operation resolves exactly the same retained owner.
    let mut replacement = fixture
        .command(&["session", "resume", &session])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    replacement
        .stdin
        .take()
        .unwrap()
        .write_all(b"approved\n")
        .unwrap();
    let result = replacement.wait_with_output().unwrap();
    assert!(
        result.status.success(),
        "{}",
        String::from_utf8_lossy(&result.stderr)
    );
    let terminal = String::from_utf8_lossy(&result.stdout);
    assert!(terminal.contains("\x1b]7501;id=lf/"), "{terminal}");
    assert!(terminal.contains(":state=done\x1b\\"), "{terminal}");
    assert!(terminal.contains("/step:state=clear\x1b\\"), "{terminal}");
    let injected: i64 = sql
        .query_row(
            "SELECT count(*) FROM session_events WHERE session_id=?1 AND instr(payload, ?2)>0",
            [session.as_str(), r"\u001b]7501;"],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(injected, 0, "terminal status entered Session history");
    assert_eq!(
        fixture.wait_file("answer").trim(),
        format!("{pid}:approved")
    );
    assert_eq!(
        fixture.wait_file("launches"),
        launched,
        "provider was replaced or input replayed"
    );
    let provider_id: String = sql.query_row("SELECT id FROM processes WHERE agent_session_id=?1 AND kind='agent' ORDER BY started_at LIMIT 1", [&session], |r| r.get(0)).unwrap();
    assert_eq!(provider_id, attachment.0);
}

/// A restart kills every owner and provider without recording an exit. `lf resume
/// --all` reopens exactly those conversations from saved history, once; one the
/// person left stays stopped.
#[test]
fn resume_all_restores_interrupted_sessions_and_leaves_left_ones() {
    let fixture = Fixture::new();
    let path = fixture.waiting_provider();
    let mut owners = Vec::new();
    for input in ["first", "second", "left"] {
        let mut owner = fixture.serve(
            true,
            &[
                env!("CARGO_BIN_EXE_lf"),
                "--interactive",
                "--agent",
                "claude",
                ":",
                input,
            ],
        );
        owner.env("PATH", &path);
        owners.push(fixture.owner(owner));
        fixture.launches(owners.len());
    }
    let sql = fixture.sql();
    let started = fixture.launches(3);
    let sessions: Vec<String> = started.iter().map(|line| session_of(&sql, line)).collect();
    // The person leaves the third conversation through the provider's own exit.
    let mut viewer = fixture.viewer(&owners[2], true);
    viewer.input("exit\n");
    viewer.finish();
    let deadline = Instant::now() + Duration::from_secs(10);
    while sql
        .query_row(
            "SELECT completed_at IS NULL FROM processes WHERE id=?1",
            [&owners[2]],
            |r| r.get::<_, bool>(0),
        )
        .unwrap()
    {
        assert!(
            Instant::now() < deadline,
            "left owner never recorded its exit"
        );
        std::thread::sleep(Duration::from_millis(20));
    }
    // A restart: the owner, its lf and the provider die without a word.
    let crash = |owner: &str, line: &str| {
        let receipt: Value = serde_json::from_slice(
            &std::fs::read(
                fixture
                    .home
                    .path()
                    .join(format!("runtime/exec-processes/{owner}.json")),
            )
            .unwrap(),
        )
        .unwrap();
        let provider: i64 = line.split(':').next().unwrap().parse().unwrap();
        let parent = Command::new("ps")
            .args(["-o", "ppid=", "-p", &provider.to_string()])
            .output()
            .unwrap();
        let inner: i64 = String::from_utf8(parent.stdout)
            .unwrap()
            .trim()
            .parse()
            .unwrap();
        let pids = [receipt["pid"].as_i64().unwrap(), inner, provider];
        // Freeze first so no survivor observes another's death and records an exit.
        for signal in [libc::SIGSTOP, libc::SIGKILL] {
            for pid in pids {
                // SAFETY: test-owned PIDs read from this fixture's receipts and launches.
                unsafe { libc::kill(pid as libc::pid_t, signal) };
            }
        }
    };
    for (owner, line) in owners.iter().zip(&started).take(2) {
        crash(owner, line);
    }
    std::thread::sleep(Duration::from_millis(200));
    // The first was mid-turn when the machine went down; the second was waiting.
    sql.execute(
        r#"INSERT INTO session_activity(session_id,observed_at,open_tools,pending_input,yielded,agent_process_id,program_status)
           SELECT id,0,1,0,0,agent_process_id,'{"seen":true,"records":[{"state":"working","id":null,"kind":null,"progress":null,"app":null,"title":null,"msg":null}]}' FROM agent_sessions WHERE id=?1
           ON CONFLICT(session_id) DO UPDATE SET program_status=excluded.program_status,
               agent_process_id=excluded.agent_process_id"#,
        [&sessions[0]],
    )
    .unwrap();
    let resume_all = || -> Value {
        let output = fixture
            .command(&["resume", "--all", "--json"])
            .env("PATH", &path)
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        serde_json::from_slice(&output.stdout).unwrap()
    };
    let resumed = resume_all();
    let ids: Vec<&str> = resumed
        .as_array()
        .unwrap()
        .iter()
        .map(|entry| entry["id"].as_str().unwrap())
        .collect();
    assert_eq!(
        ids.iter()
            .copied()
            .collect::<std::collections::BTreeSet<_>>(),
        sessions[..2]
            .iter()
            .map(String::as_str)
            .collect::<std::collections::BTreeSet<_>>(),
        "{resumed}"
    );
    assert!(
        resumed
            .as_array()
            .unwrap()
            .iter()
            .all(|entry| entry["outcome"]["kind"] == "resumed"),
        "{resumed}"
    );
    for entry in resumed.as_array().unwrap() {
        assert_eq!(
            entry["turn_cut_off"],
            entry["id"] == sessions[0].as_str(),
            "{resumed}"
        );
    }
    let reopened = fixture.launches(5);
    assert_eq!(reopened.len(), 5, "{reopened:?}");
    for line in &reopened[3..] {
        let (_, resumed) = line
            .split_once(" --resume ")
            .unwrap_or_else(|| panic!("reopened without history: {line}"));
        assert!(!resumed.contains(' '), "input replayed: {line}");
    }
    // Rerunning finds nothing to do and starts no provider.
    assert_eq!(resume_all(), json!([]));
    std::thread::sleep(Duration::from_millis(300));
    let reopened = fixture.launches(5);
    assert_eq!(reopened.len(), 5);
    // Resumed owners outlive the command; stop them with the fixture.
    for line in &reopened[3..] {
        let owner: String = sql
            .query_row(
                "SELECT s.transport_lf_process_id FROM agent_sessions s JOIN session_events e
                 ON e.session_id=s.id WHERE e.kind='captured' AND e.receipt_key=?1",
                [line.split(':').nth(1).unwrap()],
                |r| r.get(0),
            )
            .unwrap();
        crash(&owner, line);
    }
}

/// Closing every view of a conversation ends its agent once
/// `session.end_detached_after` passes. A viewer, a turn in progress or a Graph
/// keeps it running. The ended conversation reopens from saved history and
/// stays out of `lf resume --all`.
#[test]
fn detached_sessions_end_after_their_limit() {
    let fixture = Fixture::new();
    std::fs::write(
        fixture.home.path().join("config.yaml"),
        "session:\n  end_detached_after: 2s\n",
    )
    .unwrap();
    let path = fixture.waiting_provider();
    // Each starts viewed, so nothing ends while the fixture is arranged.
    let mut owners = Vec::new();
    let mut viewers = Vec::new();
    for input in ["idle", "viewed", "busy", "graph"] {
        let mut owner = fixture.serve(
            true,
            &[
                env!("CARGO_BIN_EXE_lf"),
                "--interactive",
                "--agent",
                "claude",
                ":",
                input,
            ],
        );
        owner.env("PATH", &path);
        let owner = fixture.owner(owner);
        viewers.push(fixture.viewer(&owner, false));
        owners.push(owner);
    }
    let sql = fixture.sql();
    fixture.launches(4);
    let idle = fixture.session_of_owner(&owners[0]);
    fixture.mark_working(&fixture.session_of_owner(&owners[2]));
    sql.execute(
        "INSERT INTO graph_processes(lf_process_id,graph,compiled)
         SELECT id,'fixture','{}' FROM processes WHERE parent_lf_process_id=?1 LIMIT 1",
        [&owners[3]],
    )
    .unwrap();
    // Closing the window: the idle, busy and Graph owners lose their viewers.
    let viewed = viewers.remove(1);
    drop(viewers);
    fixture.ends(&owners[0]);
    // Several judgments later, everything else still runs.
    std::thread::sleep(Duration::from_secs(4));
    for owner in &owners[1..] {
        assert!(fixture.running(owner), "{owner} ended");
    }
    let resumed = fixture
        .command(&["resume", "--all", "--json"])
        .env("PATH", &path)
        .output()
        .unwrap();
    assert!(
        resumed.status.success(),
        "{}",
        String::from_utf8_lossy(&resumed.stderr)
    );
    assert_eq!(
        serde_json::from_slice::<Value>(&resumed.stdout).unwrap(),
        json!([])
    );
    // Its last viewer leaving lets the viewed one end too.
    drop(viewed);
    fixture.ends(&owners[1]);
    // `lf resume` reopens the ended conversation from its saved history.
    let mut reopen = fixture.serve(
        true,
        &[env!("CARGO_BIN_EXE_lf"), "session", "resume", &idle],
    );
    reopen.env("PATH", &path);
    let reopened = fixture.owner(reopen);
    let _viewer = fixture.viewer(&reopened, false);
    let line = fixture.launches(5).pop().unwrap();
    assert!(
        line.contains(" --resume ") && session_of(&sql, &line) == idle,
        "reopened without history: {line}"
    );
    for owner in [&owners[2], &owners[3], &reopened] {
        fixture.stop(owner);
    }
}

/// A conversation whose lf executable is deleted or rebuilt ends once it waits
/// for a person, even while viewed. One launched through the installed entry
/// ends when the entry's target changes, as `lf install` changes it, and a
/// terminal showing it follows through the entry onto the installed lf with
/// the conversation's history. A turn in progress finishes first: `lf resume`
/// on that owner watches it rather than cutting it, and follows when it ends.
#[test]
fn sessions_end_when_their_lf_executable_goes() {
    let fixture = Fixture::new();
    let path = fixture.waiting_provider();
    let bin = fixture.home.path().join("bin");
    let build = |name: &str| -> PathBuf {
        let copy = bin.join(name);
        std::fs::copy(env!("CARGO_BIN_EXE_lf"), &copy).unwrap();
        copy
    };
    let installed = bin.join("lf");
    std::os::unix::fs::symlink(build("lf-installed"), &installed).unwrap();
    let executables = [
        build("lf-idle"),
        build("lf-replaced"),
        installed.clone(),
        installed.clone(),
    ];
    let mut owners = Vec::new();
    let mut viewers = Vec::new();
    for executable in &executables {
        let lf = executable.to_str().unwrap();
        let mut owner = fixture.command_from(executable, &["session", "serve", "--terminal"]);
        // Each owner's entry is the executable it was launched through.
        owner
            .args(["--", lf, "--interactive", "--agent", "claude", ":", "hello"])
            .env("LF_BIN", lf)
            .env("PATH", &path);
        let owner = fixture.owner(owner);
        viewers.push(fixture.viewer(&owner, false));
        owners.push(owner);
    }
    fixture.launches(owners.len());
    let followed = fixture.session_of_owner(&owners[2]);
    let busy = fixture.session_of_owner(&owners[3]);
    fixture.mark_working(&busy);
    let resume = |session: &str| {
        fixture
            .command_from(&installed, &["session", "resume", session])
            .env("LF_BIN", &installed)
            .env("PATH", &path)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .unwrap()
    };
    let mut terminal = resume(&followed);
    fixture.wait_controlled(&owners[2]);

    // A worktree removed, a build relinked, `lf install` selecting another build.
    std::fs::remove_file(&executables[0]).unwrap();
    let relinked = bin.join("lf-relinked");
    std::fs::write(&relinked, "another build").unwrap();
    std::fs::rename(&relinked, &executables[1]).unwrap();
    let next = bin.join("lf-next");
    other_build(&next);
    let swapped = bin.join("lf-swapped");
    std::os::unix::fs::symlink(&next, &swapped).unwrap();
    std::fs::rename(&swapped, &installed).unwrap();

    fixture.ends(&owners[0]);
    fixture.ends(&owners[1]);
    fixture.ends(&owners[2]);
    // Their viewers learn why; only the superseded one learns which entry
    // reopens the conversation.
    for (index, how) in [(0, "build was removed"), (1, "build was replaced")] {
        let session = fixture.session_of_owner(&owners[index]);
        assert_eq!(
            viewers[index].moves(),
            [(
                Some(format!(
                    "This Session's lf {how}; `lf resume {session}` reopens it."
                )),
                None
            )]
        );
    }
    let superseded_move = (
        Some("This Session's lf was superseded; reopening on the installed lf.".to_owned()),
        Some(installed.to_str().unwrap().to_owned()),
    );
    assert_eq!(viewers[2].moves(), std::slice::from_ref(&superseded_move));
    // The terminal showing the superseded conversation reran its invocation
    // through the entry: the conversation resumed on lf-next with its history.
    let line = fixture.launches(5).pop().unwrap();
    assert!(
        line.contains(" --resume ") && session_of(&fixture.sql(), &line) == followed,
        "reopened without history: {line}"
    );
    let owner_of = |session: &str| -> String {
        fixture
            .sql()
            .query_row(
                "SELECT transport_lf_process_id FROM agent_sessions WHERE id=?1",
                [session],
                |r| r.get(0),
            )
            .unwrap()
    };
    let reopened = owner_of(&followed);
    assert_ne!(reopened, owners[2]);
    let executable: String = fixture
        .sql()
        .query_row(
            "SELECT json_extract(command,'$[0]') FROM processes WHERE id=?1",
            [&reopened],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(
        std::fs::canonicalize(&executable).unwrap(),
        std::fs::canonicalize(bin.join("lf-next")).unwrap(),
        "reopened on {executable}"
    );
    assert!(
        terminal.try_wait().unwrap().is_none(),
        "terminal left the conversation"
    );
    // Opening the busy owner's conversation from the installed lf attaches to
    // it rather than cutting its turn, and several judgments later it still runs.
    let mut watching = resume(&busy);
    fixture.wait_controlled(&owners[3]);
    std::thread::sleep(Duration::from_secs(4));
    assert!(fixture.running(&owners[3]), "busy owner ended mid-turn");
    assert_eq!(
        fixture.launches(5).len(),
        5,
        "the busy conversation relaunched"
    );
    // Its turn settling lets it end; the watching terminal then follows.
    fixture
        .sql()
        .execute("DELETE FROM session_activity WHERE session_id=?1", [&busy])
        .unwrap();
    fixture.ends(&owners[3]);
    assert_eq!(viewers[3].moves(), [superseded_move]);
    let line = fixture.launches(6).pop().unwrap();
    assert!(
        line.contains(" --resume ") && session_of(&fixture.sql(), &line) == busy,
        "reopened without history: {line}"
    );
    let busy_reopened = owner_of(&busy);
    assert_ne!(busy_reopened, owners[3]);
    assert!(
        watching.try_wait().unwrap().is_none(),
        "watching terminal left the conversation"
    );
    drop(viewers);
    for mut terminal in [terminal, watching] {
        terminal.kill().unwrap();
        terminal.wait().unwrap();
    }
    fixture.stop(&reopened);
    fixture.stop(&busy_reopened);
}

/// An lf executable with other bytes than the test binary, still runnable:
/// a copy re-signed under another identifier on macOS, a copy with trailing
/// bytes elsewhere.
fn other_build(path: &Path) {
    std::fs::copy(env!("CARGO_BIN_EXE_lf"), path).unwrap();
    if cfg!(target_os = "macos") {
        let signed = Command::new("codesign")
            .args(["-f", "-s", "-", "-i", "lf-other"])
            .arg(path)
            .output()
            .unwrap();
        assert!(signed.status.success(), "{signed:?}");
    } else {
        let mut file = std::fs::OpenOptions::new().append(true).open(path).unwrap();
        file.write_all(b"\n").unwrap();
    }
    assert_ne!(
        std::fs::read(path).unwrap(),
        std::fs::read(env!("CARGO_BIN_EXE_lf")).unwrap()
    );
}

/// `lf resume` on a conversation whose owner runs another lf build never
/// negotiates with it: the owner ends and the conversation resumes here from
/// its saved history, as it does for the owners `lf install` leaves behind.
#[test]
fn resume_takes_over_an_owner_from_another_build() {
    let fixture = Fixture::new();
    let path = fixture.waiting_provider();
    let other = fixture.home.path().join("bin/lf-other");
    other_build(&other);
    let lf = other.to_str().unwrap();
    let mut owner = fixture.command_from(&other, &["session", "serve", "--terminal"]);
    owner
        .args(["--", lf, "--interactive", "--agent", "claude", ":", "hello"])
        .env("LF_BIN", lf)
        .env("PATH", &path);
    let previous = fixture.owner(owner);
    fixture.launches(1);
    let session = fixture.session_of_owner(&previous);
    let mut viewer = fixture
        .command(&["session", "resume", &session])
        .env("PATH", &path)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    let line = fixture.launches(2).pop().unwrap();
    assert!(
        line.contains(" --resume ") && session_of(&fixture.sql(), &line) == session,
        "reopened without history: {line}"
    );
    fixture.ends(&previous);
    let current: String = fixture
        .sql()
        .query_row(
            "SELECT transport_lf_process_id FROM agent_sessions WHERE id=?1",
            [&session],
            |r| r.get(0),
        )
        .unwrap();
    assert_ne!(current, previous);
    assert!(
        viewer.try_wait().unwrap().is_none(),
        "viewer left the conversation"
    );
    viewer.kill().unwrap();
    let output = viewer.wait_with_output().unwrap();
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("Moved") && stderr.contains("continuing on "),
        "{stderr}"
    );
    assert!(!stderr.contains("cut off"), "{stderr}");
    fixture.stop(&current);
}

/// A superseded owner mid-turn that does not answer its socket cannot be
/// watched to the turn's end, so `lf resume` ends it, reopens the conversation
/// from its saved history and says the turn was cut.
#[test]
fn resume_ends_a_busy_superseded_owner_that_never_answers() {
    let fixture = Fixture::new();
    let path = fixture.waiting_provider();
    let other = fixture.home.path().join("bin/lf-other");
    other_build(&other);
    let lf = other.to_str().unwrap();
    let mut owner = fixture.command_from(&other, &["session", "serve", "--terminal"]);
    owner
        .args(["--", lf, "--interactive", "--agent", "claude", ":", "hello"])
        .env("LF_BIN", lf)
        .env("PATH", &path);
    let previous = fixture.owner(owner);
    fixture.launches(1);
    let session = fixture.session_of_owner(&previous);
    fixture.mark_working(&session);
    let endpoint = fixture.endpoint(&previous);
    std::fs::rename(&endpoint, endpoint.with_extension("retained")).unwrap();
    let listener = UnixListener::bind(&endpoint).unwrap();
    std::thread::spawn(move || {
        let held: Vec<UnixStream> = listener.incoming().map_while(Result::ok).collect();
        drop(held);
    });
    let mut viewer = fixture
        .command(&["session", "resume", &session])
        .env("PATH", &path)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    let line = fixture.launches(2).pop().unwrap();
    assert!(
        line.contains(" --resume ") && session_of(&fixture.sql(), &line) == session,
        "reopened without history: {line}"
    );
    fixture.ends(&previous);
    let current: String = fixture
        .sql()
        .query_row(
            "SELECT transport_lf_process_id FROM agent_sessions WHERE id=?1",
            [&session],
            |r| r.get(0),
        )
        .unwrap();
    assert_ne!(current, previous);
    assert!(
        viewer.try_wait().unwrap().is_none(),
        "viewer left the conversation"
    );
    viewer.kill().unwrap();
    let output = viewer.wait_with_output().unwrap();
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("Moved") && stderr.contains("cut off"),
        "{stderr}"
    );
    fixture.stop(&current);
}

/// Desktop restarting after an lf upgrade leaves the conversation in an owner
/// from the earlier build, which cannot write the advanced store, so its record
/// still says the departed terminal shows it. Opening the Session asks the owner, reads
/// `detached` and reattaches to it without starting anything new.
#[test]
fn opening_reattaches_an_owner_whose_viewer_record_went_stale() {
    let fixture = Fixture::new();
    let owner = fixture.owner(fixture.serve(
        false,
        &["/bin/sh", "-c", "read -r answer; printf '%s' \"$answer\""],
    ));
    let session = fixture.session(&owner);
    fixture
        .sql()
        .execute(
            "UPDATE agent_sessions SET provider='claude' WHERE id=?1",
            [&session],
        )
        .unwrap();
    let viewed = || -> bool {
        fixture
            .sql()
            .query_row("SELECT viewed FROM processes WHERE id=?1", [&owner], |r| {
                r.get(0)
            })
            .unwrap()
    };
    let desktop = fixture.viewer(&owner, true);
    let deadline = Instant::now() + Duration::from_secs(10);
    while !viewed() {
        assert!(Instant::now() < deadline, "owner never recorded its viewer");
        std::thread::sleep(Duration::from_millis(20));
    }
    // Desktop quits.
    desktop
        .socket
        .get_ref()
        .shutdown(std::net::Shutdown::Both)
        .unwrap();
    drop(desktop);
    let deadline = Instant::now() + Duration::from_secs(10);
    while viewed() {
        assert!(Instant::now() < deadline, "the viewer never left");
        std::thread::sleep(Duration::from_millis(20));
    }
    // An earlier build's owner cannot write the store a newer lf advanced, so
    // its record keeps the viewer that left.
    fixture
        .sql()
        .execute("UPDATE processes SET viewed=1 WHERE id=?1", [&owner])
        .unwrap();

    let prepared = fixture
        .command(&["session", "resume", &session, "--prepare", "--json"])
        .output()
        .unwrap();
    assert!(prepared.status.success(), "{prepared:?}");
    let prepared: Value = serde_json::from_slice(&prepared.stdout).unwrap();
    assert_eq!(prepared["state"], "detached");
    assert_eq!(
        prepared["actions"],
        json!([{"kind":"open","label":"Reconnect","help":"Show the running Session in this terminal","unavailable_reason":null}])
    );
    assert_eq!(prepared["terminal_ids"], json!([]));

    let mut reopened = fixture
        .command(&["session", "resume", &session])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    reopened
        .stdin
        .take()
        .unwrap()
        .write_all(b"same conversation\n")
        .unwrap();
    let output = wait_for_viewer(reopened, Duration::from_secs(10));
    assert!(output.status.success(), "{output:?}");
    assert_eq!(output.stdout, b"same conversation");
}

/// An owner from an earlier lf can stay alive after an upgrade yet never answer
/// its socket. Opening its Session neither waits on it nor gives up: the stale
/// viewer record reads `detached`, and reconnecting stops its provider and
/// continues the same provider thread in a new owner, without replaying input.
#[test]
fn opening_takes_over_from_an_owner_that_never_answers() {
    let fixture = Fixture::new();
    let path = fixture.waiting_provider();
    let mut owner = fixture.serve(
        true,
        &[
            env!("CARGO_BIN_EXE_lf"),
            "--interactive",
            "--agent",
            "claude",
            ":",
            "one original input",
        ],
    );
    owner.env("PATH", &path);
    let owner = fixture.owner(owner);
    let launched = fixture.launches(1).remove(0);
    let provider: libc::pid_t = launched.split(':').next().unwrap().parse().unwrap();
    let session = session_of(&fixture.sql(), &launched);
    // The owner keeps running, but its socket accepts and never replies, and
    // its record still names the Desktop that quit.
    let endpoint = fixture.endpoint(&owner);
    std::fs::rename(&endpoint, endpoint.with_extension("retained")).unwrap();
    let listener = UnixListener::bind(&endpoint).unwrap();
    std::thread::spawn(move || {
        let held: Vec<UnixStream> = listener.incoming().map_while(Result::ok).collect();
        drop(held);
    });
    fixture
        .sql()
        .execute("UPDATE processes SET viewed=1 WHERE id=?1", [&owner])
        .unwrap();

    let prepared = fixture
        .command(&["session", "resume", &session, "--prepare", "--json"])
        .output()
        .unwrap();
    assert!(prepared.status.success(), "{prepared:?}");
    let prepared: Value = serde_json::from_slice(&prepared.stdout).unwrap();
    assert_eq!(prepared["state"], "detached", "{prepared}");
    assert_eq!(prepared["actions"][0]["label"], "Reconnect", "{prepared}");

    let reopened = fixture
        .command(&["session", "resume", &session])
        .env("PATH", &path)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    let launches = fixture.launches(2);
    let (_, thread) = launches[1]
        .split_once(" --resume ")
        .unwrap_or_else(|| panic!("reopened without history: {launches:?}"));
    assert!(!thread.contains(' '), "input replayed: {launches:?}");
    assert!(launched.contains(thread), "another thread: {launches:?}");
    let deadline = Instant::now() + Duration::from_secs(10);
    // SAFETY: signal 0 only probes the fixture provider's PID.
    while unsafe { libc::kill(provider, 0) } == 0 {
        assert!(Instant::now() < deadline, "the old provider kept running");
        std::thread::sleep(Duration::from_millis(20));
    }
    let current: String = fixture
        .sql()
        .query_row(
            "SELECT transport_lf_process_id FROM agent_sessions WHERE id=?1",
            [&session],
            |r| r.get(0),
        )
        .unwrap();
    assert_ne!(current, owner);
    drop(wait_for_viewer(reopened, Duration::from_secs(10)));
    for owner in [owner, current] {
        if fixture.running(&owner) {
            fixture.stop(&owner);
        }
    }
}

#[test]
fn nested_cli_execution_survives_its_calling_provider_and_viewer() {
    let fixture = Fixture::new();
    let bin = fixture.home.path().join("bin");
    std::fs::create_dir(&bin).unwrap();
    let provider = bin.join("claude");
    std::fs::write(
        &provider,
        r#"#!/bin/sh
if [ "$1" = --version ]; then echo '2.1.302'; exit; fi
if mkdir "$HOME/outer" 2>/dev/null; then
    printf '%s:%s\n' "$$" "$LF_CAPTURE_KEY" > "$HOME/caller"
    "$LF_BIN" --interactive --agent claude : 'nested original input' < /dev/tty &
    printf '%s\n' "$!" > "$HOME/nested-viewer"
    wait
else
    stty -echo < /dev/tty
    printf '%s:%s\n' "$$" "$LF_CAPTURE_KEY" >> "$HOME/nested-launches"
    read -r answer < /dev/tty
    printf '%s:%s\n' "$$" "$answer" > "$HOME/nested-answer"
fi
"#,
    )
    .unwrap();
    std::fs::set_permissions(&provider, std::fs::Permissions::from_mode(0o755)).unwrap();
    let mut owner = fixture.serve(
        true,
        &[
            env!("CARGO_BIN_EXE_lf"),
            "--interactive",
            "--agent",
            "claude",
            ":",
            "call the nested CLI",
        ],
    );
    owner.env("PATH", format!("{}:/usr/bin:/bin", bin.display()));
    fixture.owner(owner);
    let caller = fixture.wait_file("caller");
    let nested = fixture.wait_file("nested-launches");
    let viewer: i32 = fixture.wait_file("nested-viewer").trim().parse().unwrap();
    let (caller_pid, caller_capture) = caller.trim().split_once(':').unwrap();
    let (nested_pid, nested_capture) = nested.trim().split_once(':').unwrap();
    let sql = rusqlite::Connection::open(fixture.home.path().join("loopflow.db")).unwrap();
    let identity = |capture: &str| -> (String, String, String) {
        sql.query_row(
            "SELECT s.id,s.agent_process_id,s.transport_lf_process_id
             FROM agent_sessions s JOIN session_events e ON e.session_id=s.id
             WHERE e.kind='captured' AND e.receipt_key=?1",
            [capture],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
        )
        .unwrap()
    };
    let parent = identity(caller_capture);
    let child = identity(nested_capture);
    assert_ne!(parent.0, child.0);
    assert_ne!(
        parent.2, child.2,
        "nested execution inherited its caller's owner"
    );
    let descendants: i64 = sql
        .query_row(
            "SELECT count(*) FROM processes WHERE caller_session_id=?1",
            [&parent.0],
            |row| row.get(0),
        )
        .unwrap();
    assert!(descendants > 0, "nested CLI lost its caller provenance");
    // SAFETY: both PIDs are reported by this fixture's live processes. Kill
    // precisely the calling provider and nested viewer, never either owner.
    unsafe {
        assert_eq!(libc::kill(caller_pid.parse().unwrap(), libc::SIGKILL), 0);
        assert_eq!(libc::kill(viewer, libc::SIGKILL), 0);
    }
    let mut attached = fixture.viewer(&child.2, true);
    assert_eq!(identity(nested_capture), child);
    let settled: i64 = sql.query_row(
        "SELECT count(*) FROM session_events WHERE session_id=?1 AND payload LIKE '%terminal.json%'",
        [&child.0], |row| row.get(0),
    ).unwrap();
    assert_eq!(settled, 0, "caller death settled the nested request");
    attached.input("accepted-once\n");
    attached.finish();
    assert_eq!(
        fixture.wait_file("nested-answer").trim(),
        format!("{nested_pid}:accepted-once")
    );
    assert_eq!(fixture.wait_file("nested-launches"), nested);
}

#[test]
fn codex_original_request_channel_and_turn_survive_all_viewers() {
    let fixture = Fixture::new();
    let bin = fixture.home.path().join("bin");
    std::fs::create_dir(&bin).unwrap();
    support::install_native_codex(&bin);
    let path = format!("{}:/usr/bin:/bin", bin.display());
    std::fs::write(fixture.home.path().join("wait-approval"), "").unwrap();
    // The ordinary interactive CLI is the creator/viewer. Give it a real tty,
    // then kill it only after the original protocol request is pending.
    let mut master = -1;
    let mut slave = -1;
    // SAFETY: openpty initializes both descriptors; no optional settings are supplied.
    assert_eq!(
        unsafe {
            libc::openpty(
                &mut master,
                &mut slave,
                std::ptr::null_mut(),
                std::ptr::null_mut(),
                std::ptr::null_mut(),
            )
        },
        0
    );
    // SAFETY: each newly opened descriptor is transferred exactly once.
    let _terminal = unsafe { std::fs::File::from_raw_fd(master) };
    // SAFETY: the slave is a distinct, newly opened descriptor.
    let terminal_input = unsafe { std::fs::File::from_raw_fd(slave) };
    let mut creator = fixture
        .command(&["-i", "--agent", "codex", ":", "Original input"])
        .env("PATH", &path)
        .stdin(terminal_input.try_clone().unwrap())
        .stdout(terminal_input.try_clone().unwrap())
        .stderr(terminal_input)
        .spawn()
        .unwrap();
    let pending: Value = serde_json::from_str(&fixture.wait_file("pending-request.json")).unwrap();
    let launched = fixture.wait_file("launched");
    let sql = rusqlite::Connection::open(fixture.home.path().join("loopflow.db")).unwrap();
    let session: String = sql
        .query_row(
            "SELECT session_id FROM session_events WHERE kind='captured' AND receipt_key=?1",
            [launched.trim()],
            |r| r.get(0),
        )
        .unwrap();
    let identity: (String,i64,String) = sql.query_row("SELECT id,pid,attachment_token FROM processes WHERE agent_session_id=?1 AND kind='agent' AND completed_at IS NULL", [&session], |r| Ok((r.get(0)?,r.get(1)?,r.get(2)?))).unwrap();
    let owner: String = sql
        .query_row(
            "SELECT transport_lf_process_id FROM agent_sessions WHERE id=?1",
            [&session],
            |r| r.get(0),
        )
        .unwrap();
    creator.kill().unwrap();
    creator.wait().unwrap();
    let first = fixture.viewer(&owner, true);
    drop(first);
    let observed: i64 = sql.query_row("SELECT count(*) FROM session_events WHERE session_id=?1 AND payload LIKE '%terminal.json%'", [&session], |r| r.get(0)).unwrap();
    assert_eq!(observed, 0);
    let replacement = fixture.viewer(&owner, true);
    let retained: (String,i64,String) = sql.query_row("SELECT id,pid,attachment_token FROM processes WHERE agent_session_id=?1 AND kind='agent' AND completed_at IS NULL", [&session], |r| Ok((r.get(0)?,r.get(1)?,r.get(2)?))).unwrap();
    assert_eq!(retained, identity, "viewers transferred provider ownership");
    drop(replacement);
    let mut resumed = fixture
        .command(&["resume", &session])
        .env("PATH", &path)
        .stdin(Stdio::piped())
        .stdout(Stdio::null())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    resumed
        .stdin
        .take()
        .unwrap()
        .write_all(b"decline\n")
        .unwrap();
    let outcome = resumed.wait_with_output().unwrap();
    assert!(
        outcome.status.success(),
        "{}",
        String::from_utf8_lossy(&outcome.stderr)
    );
    let answer: Value = serde_json::from_str(&fixture.wait_file("native-answer.json")).unwrap();
    assert_eq!(answer["id"], pending["id"]);
    let turns: Value = serde_json::from_str(&fixture.wait_file("native-turns.json")).unwrap();
    assert_eq!(turns.as_array().unwrap().len(), 1, "input replayed");
    assert_eq!(turns[0]["id"], pending["params"]["turnId"]);
    assert_eq!(turns[0]["status"], "completed");
    assert_eq!(
        fixture.wait_file("native-thread"),
        pending["params"]["threadId"].as_str().unwrap()
    );
    assert_eq!(fixture.wait_file("launched"), launched);
}

#[test]
fn headless_graph_settles_its_original_step_after_creator_teardown() {
    headless_graph(false);
}

#[test]
fn resume_refuses_a_graph_step_and_leaves_it_running() {
    headless_graph(true);
}

fn headless_graph(resume: bool) {
    let fixture = Fixture::new();
    let bin = fixture.home.path().join("bin");
    std::fs::create_dir(&bin).unwrap();
    let provider = bin.join("opencode");
    std::fs::write(
        &provider,
        include_str!("support/opencode_server.py").replace("__WAIT__", "False"),
    )
    .unwrap();
    std::fs::set_permissions(&provider, std::fs::Permissions::from_mode(0o755)).unwrap();
    fixture
        .repo
        .create_file(".lf/skills/probe.md", "Do the fixture work.");
    fixture
        .repo
        .create_file(".lf/graphs/proof.yaml", "- probe\n");
    std::fs::write(fixture.home.path().join("hold-turn"), "").unwrap();
    let mut creator = fixture
        .command(&["-b", "--agent", "opencode", "graph", "proof"])
        .env(
            "PATH",
            format!(
                "{}:{}",
                bin.display(),
                std::env::var("PATH").unwrap_or_default()
            ),
        )
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .unwrap();
    let launched = fixture.wait_file("launched");
    creator.kill().unwrap();
    creator.wait().unwrap();
    let sql = rusqlite::Connection::open(fixture.home.path().join("loopflow.db")).unwrap();
    let (session, step): (String,String) = sql.query_row("SELECT session_id,lf_process_id FROM session_events WHERE kind='captured' AND receipt_key=?1", [launched.trim()], |r| Ok((r.get(0)?,r.get(1)?))).unwrap();
    let outcome: Option<String> = sql
        .query_row("SELECT outcome FROM processes WHERE id=?1", [&step], |r| {
            r.get(0)
        })
        .unwrap();
    assert!(outcome.is_none(), "viewer death settled the execution");
    if resume {
        // A Run is never resumed or taken over: the step keeps its turn.
        let output = fixture.command(&["resume", &session]).output().unwrap();
        assert!(!output.status.success());
        let error = String::from_utf8_lossy(&output.stderr);
        assert!(
            error.contains("is a Run (proof · probe · running)"),
            "{error}"
        );
    }
    std::fs::remove_file(fixture.home.path().join("hold-turn")).unwrap();
    let deadline = Instant::now() + Duration::from_secs(30);
    loop {
        let outcome: Option<String> = sql
            .query_row("SELECT outcome FROM processes WHERE id=?1", [&step], |r| {
                r.get(0)
            })
            .unwrap();
        if let Some(outcome) = outcome {
            assert_eq!(outcome, "succeeded");
            break;
        }
        assert!(Instant::now() < deadline, "step did not settle");
        std::thread::sleep(Duration::from_millis(20));
    }
    let graph: String = sql
        .query_row(
            "SELECT graph_lf_process_id FROM graph_process_steps WHERE lf_process_id=?1",
            [&step],
            |r| r.get(0),
        )
        .unwrap();
    loop {
        let outcome: Option<String> = sql
            .query_row("SELECT outcome FROM processes WHERE id=?1", [&graph], |r| {
                r.get(0)
            })
            .unwrap();
        if let Some(outcome) = outcome {
            assert_eq!(outcome, "succeeded");
            break;
        }
        assert!(Instant::now() < deadline, "Graph did not settle");
        std::thread::sleep(Duration::from_millis(20));
    }
    let captures: i64 = sql
        .query_row(
            "SELECT count(*) FROM session_events WHERE session_id=?1 AND kind='captured'",
            [&session],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(captures, 1);
    assert_eq!(fixture.wait_file("launched"), launched);
}
