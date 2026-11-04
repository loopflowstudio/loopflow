//! Real detached owner and Session attachment; no provider credentials or display.
#![cfg(unix)]

mod support;

// Decode the owner's persisted screen with the same pinned Ghostty library.
#[allow(dead_code)]
#[path = "../src/ghostty_vt.rs"]
mod ghostty_vt;

use std::io::{BufRead, BufReader, Read, Write};
use std::os::fd::{AsRawFd, FromRawFd};
use std::os::unix::fs::PermissionsExt;
use std::os::unix::net::{UnixListener, UnixStream};
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Output, Stdio};
use std::time::{Duration, Instant};

use loopflow::program_status;
use loopflow_test_support::TestRepo;
use rusqlite::OptionalExtension;
use serde_json::{json, Value};
use sha2::{Digest, Sha256};

struct Fixture {
    home: tempfile::TempDir,
    repo: TestRepo,
}

struct PausedOwner(libc::pid_t);

impl Drop for PausedOwner {
    fn drop(&mut self) {
        // SAFETY: only the fixture's paused owner is resumed, including on panic.
        unsafe { libc::kill(self.0, libc::SIGCONT) };
    }
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
    /// Launch a native Claude fixture under the selected owner executable.
    fn native(&self, binary: &Path, path: &str, prompt: &str) -> Command {
        let mut command = self.command_from(binary, &["session", "serve", "--terminal"]);
        command
            .arg("--")
            .arg(binary)
            .args(["-i", "-a", "claude", ":", prompt])
            .env("PATH", path);
        command
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
    #[track_caller]
    fn viewer(&self, id: &str, control: bool) -> Viewer {
        self.attach(
            id,
            json!({"kind":"attach","control":control,"replace":true,
                "own":control.then_some("take"),"rows":24,"cols":80}),
        )
    }
    #[track_caller]
    fn attach(&self, id: &str, request: Value) -> Viewer {
        let mut viewer = self.connect(id);
        viewer.request(request);
        let attached = viewer.until_reply();
        assert_eq!(attached["kind"], "attached", "{attached}");
        viewer.accept(&attached);
        viewer
    }
    fn connect(&self, id: &str) -> Viewer {
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
        Viewer {
            socket,
            token: None,
            id: String::new(),
            reconnect: String::new(),
            nonce: String::new(),
            received: 0,
            wire: 0,
            ids: 0,
        }
    }
    fn sql(&self) -> rusqlite::Connection {
        rusqlite::Connection::open(self.home.path().join("loopflow.db")).unwrap()
    }

    fn pause(&self, owner: &str) -> PausedOwner {
        let pid = self
            .sql()
            .query_row("SELECT pid FROM processes WHERE id=?1", [owner], |r| {
                r.get(0)
            })
            .unwrap();
        // SAFETY: this process owns the child; waitpid observes only its stopped state.
        assert_eq!(unsafe { libc::kill(pid, libc::SIGSTOP) }, 0);
        let paused = PausedOwner(pid);
        let mut status = 0;
        // SAFETY: status is writable and pid names this fixture's child.
        assert_eq!(
            unsafe { libc::waitpid(pid, &mut status, libc::WUNTRACED) },
            pid
        );
        assert!(libc::WIFSTOPPED(status));
        paused
    }
    /// Install a stand-in first on PATH without invoking a Machine provider.
    fn provider(&self, script: &str) -> String {
        let bin = self.home.path().join("bin");
        std::fs::create_dir_all(&bin).unwrap();
        let provider = bin.join("claude");
        std::fs::write(&provider, script).unwrap();
        std::fs::set_permissions(&provider, std::fs::Permissions::from_mode(0o755)).unwrap();
        format!(
            "{}:{}",
            bin.display(),
            std::env::var("PATH").unwrap_or_default()
        )
    }
    /// Log each launch to `launches` and wait on its terminal.
    fn waiting_provider(&self) -> String {
        self.provider(
            r#"#!/bin/sh
if [ "$1" = --version ]; then echo '2.1.302'; exit; fi
stty -echo < /dev/tty
printf '%s:%s:%s\n' "$$" "$LF_CAPTURE_KEY" "$*" >> "$HOME/launches"
case "$*" in *talking*) while [ ! -e "$HOME/hush" ]; do printf 'tick\r\n'; sleep 0.5; done;; esac
printf 'ready for %s\r\n' "$*"
read -r answer < /dev/tty
"#,
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
            socket
                .write_all(&frame(
                    REQUEST,
                    json!({"id":1,"kind":"controlled"}).to_string().as_bytes(),
                ))
                .unwrap();
            let (tag, payload) = read_frame(&mut socket).unwrap();
            assert_eq!(tag, REPLY);
            if serde_json::from_slice::<Value>(&payload).unwrap()["held"] == true {
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
    fn owner_of(&self, session: &str) -> String {
        self.sql()
            .query_row(
                "SELECT transport_lf_process_id FROM agent_sessions WHERE id=?1",
                [session],
                |r| r.get(0),
            )
            .unwrap()
    }
    /// Record a turn in progress for `session`.
    fn mark_working(&self, session: &str) {
        self.sql()
            .execute(
                "INSERT INTO session_activity(session_id,observed_at,open_tools,pending_input,yielded,agent_process_id,attachment_token)
                 SELECT s.id,0,1,0,0,s.agent_process_id,p.attachment_token
                 FROM agent_sessions s JOIN processes p ON p.id=s.agent_process_id WHERE s.id=?1",
                [session],
            )
            .unwrap();
    }
    fn mark_stale_working(&self, session: &str) {
        self.sql()
            .execute(
                r#"INSERT INTO session_activity(session_id,observed_at,open_tools,pending_input,yielded,agent_process_id,program_status)
                   SELECT id,0,0,0,0,agent_process_id,'{"seen":true,"records":[{"state":"working","id":null,"kind":null,"progress":null,"app":null,"title":null,"msg":null}]}' FROM agent_sessions WHERE id=?1"#,
                [session],
            )
            .unwrap();
    }
    /// Decode the last screen left by an ending owner.
    fn last_screen(&self, session: &str) -> ghostty_vt::Terminal {
        let bytes = std::fs::read(self.home.path().join(format!("screens/{session}.ghostsnp")))
            .unwrap_or_else(|error| panic!("no last screen for {session}: {error}"));
        let newline = bytes.iter().position(|byte| *byte == b'\n').unwrap();
        assert_eq!(
            std::str::from_utf8(&bytes[..newline]).unwrap(),
            format!("lf-terminal-snapshot/1 ghostty={}", ghostty_vt::REVISION)
        );
        ghostty_vt::Terminal::decode(&bytes[newline + 1..]).unwrap()
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
const REPLY: u8 = 0x01;
const SNAPSHOT: u8 = 0x02;
const OUT: u8 = 0x03;
const ERR: u8 = 0x04;
const REQUEST: u8 = 0x11;
const INPUT: u8 = 0x12;

fn frame(tag: u8, payload: &[u8]) -> Vec<u8> {
    let mut bytes = vec![tag];
    bytes.extend((payload.len() as u32).to_be_bytes());
    bytes.extend(payload);
    bytes
}

/// The next frame from `reader`, or `None` at EOF.
#[track_caller]
fn read_frame(reader: &mut impl Read) -> Option<(u8, Vec<u8>)> {
    let mut header = [0; 5];
    if reader.read(&mut header[..1]).unwrap() == 0 {
        return None;
    }
    reader.read_exact(&mut header[1..]).unwrap();
    let length = u32::from_be_bytes(header[1..].try_into().unwrap()) as usize;
    let mut payload = vec![0; length];
    reader.read_exact(&mut payload).unwrap();
    Some((header[0], payload))
}

/// What a viewer receives: replies to its requests, stream bytes, and the
/// events riding in stream 1 under its nonce.
#[derive(Debug, Clone, PartialEq)]
enum Item {
    Reply(Value),
    Snapshot(Vec<u8>),
    Out(Vec<u8>),
    Err(Vec<u8>),
    Event(Value),
}

struct Viewer {
    socket: UnixStream,
    token: Option<String>,
    id: String,
    reconnect: String,
    nonce: String,
    /// Stream bytes received, as a resume reports them.
    received: u64,
    /// Every byte read from the socket, headers included.
    wire: u64,
    ids: u64,
}
impl Viewer {
    fn request(&mut self, mut value: Value) -> u64 {
        self.ids += 1;
        value["id"] = json!(self.ids);
        self.socket
            .write_all(&frame(REQUEST, value.to_string().as_bytes()))
            .unwrap();
        self.ids
    }
    fn accept(&mut self, attached: &Value) {
        self.token = attached["token"].as_str().map(str::to_owned);
        self.id = attached["viewer"].as_str().unwrap().to_owned();
        self.reconnect = attached["reconnect"].as_str().unwrap().to_owned();
        self.nonce = attached["nonce"].as_str().unwrap().to_owned();
    }
    /// The next item, or `None` once the owner closes this connection.
    #[track_caller]
    fn next(&mut self) -> Option<Item> {
        let (tag, payload) = read_frame(&mut self.socket)?;
        self.wire += 5 + payload.len() as u64;
        if tag != REPLY {
            self.received += payload.len() as u64;
        }
        Some(match tag {
            REPLY => Item::Reply(serde_json::from_slice(&payload).unwrap()),
            SNAPSHOT => Item::Snapshot(payload),
            ERR => Item::Err(payload),
            OUT => {
                let prefix = format!("\x1b_lf1;{};", self.nonce).into_bytes();
                match payload.strip_prefix(prefix.as_slice()) {
                    Some(body) if body.ends_with(b"\x1b\\") => {
                        Item::Event(serde_json::from_slice(&body[..body.len() - 2]).unwrap())
                    }
                    _ => Item::Out(payload),
                }
            }
            other => panic!("unexpected frame tag {other:#x}"),
        })
    }
    fn moves(&mut self) -> Vec<(Option<String>, Option<Vec<String>>)> {
        let mut moves = Vec::new();
        while let Some(item) = self.next() {
            if let Item::Event(event) = item {
                if event["type"] == "control" && event["held"] == false {
                    moves.push((
                        serde_json::from_value(event["notice"].clone()).unwrap(),
                        serde_json::from_value(event["successor"].clone()).unwrap(),
                    ));
                }
            }
        }
        moves
    }
    #[track_caller]
    fn until_reply(&mut self) -> Value {
        loop {
            if let Item::Reply(reply) = self.next().expect("owner closed before replying") {
                return reply;
            }
        }
    }
    /// Everything up to and including the first event of `kind`.
    #[track_caller]
    fn until_event(&mut self, kind: &str) -> Vec<Item> {
        until(
            self,
            |items| matches!(items.last(), Some(Item::Event(event)) if event["type"] == kind),
        )
    }
    fn input(&mut self, text: &str) {
        self.socket
            .write_all(&frame(INPUT, text.as_bytes()))
            .unwrap();
    }
    #[track_caller]
    fn finish(&mut self) {
        let items = self.until_event("child_exited");
        let Some(Item::Event(exit)) = items.last() else {
            unreachable!()
        };
        assert_eq!(exit["code"], 0, "{items:?}");
    }
}

/// Stream 1 bytes among `items`, concatenated.
fn out(items: &[Item]) -> Vec<u8> {
    items
        .iter()
        .filter_map(|item| match item {
            Item::Out(bytes) | Item::Snapshot(bytes) => Some(bytes.as_slice()),
            _ => None,
        })
        .flatten()
        .copied()
        .collect()
}

fn events<'a>(items: &'a [Item], kind: &str) -> Vec<&'a Value> {
    items
        .iter()
        .filter_map(|item| match item {
            Item::Event(event) if event["type"] == kind => Some(event),
            _ => None,
        })
        .collect()
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
        json!({"kind":"attach","control":true,"replace":true,"own":"take","rows":24,"cols":80,
            "moved":"This conversation moved to tmux %4: lf resume s1"}),
    );
    // The replaced viewer is told where the conversation went and who sizes
    // the terminal now; it stays attached until it leaves.
    let moved = first.until_event("control");
    let Some(Item::Event(control)) = moved.last() else {
        unreachable!()
    };
    assert_eq!(control["held"], false);
    assert_eq!(
        control["notice"],
        "This conversation moved to tmux %4: lf resume s1"
    );
    assert!(
        events(&moved, "owner")
            .iter()
            .any(|event| event["viewer"] == second.id && event["held"] == false),
        "{moved:?}"
    );
    let mut observer = fixture.viewer(&owner, false);
    assert!(observer.token.is_none());
    observer.input("passive\n");
    drop(observer);
    drop(first);
    drop(second); // zero viewers while the same read remains pending
    let mut replacement = fixture.viewer(&owner, true);
    let mut passive = fixture.viewer(&owner, false);
    passive.request(json!({"kind":"attach","control":false,"replace":true,"rows":24,"cols":80}));
    assert_eq!(passive.until_reply()["kind"], "attached");
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
    waiting.request(json!({"kind":"attach","control":true,"replace":false,"rows":24,"cols":80}));
    assert_eq!(waiting.until_reply()["kind"], "busy");
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
    // The waiting viewer precedes the controller in the owner's connection list.
    let mut waiting = fixture.viewer(&owner, false);
    let controller = fixture.viewer(&owner, true);
    let token = controller.token.clone();
    let paused = fixture.pause(&owner);
    controller
        .socket
        .shutdown(std::net::Shutdown::Both)
        .unwrap();
    drop(controller);
    waiting.request(json!({"kind":"attach","control":true,"replace":false,"rows":24,"cols":80}));
    drop(paused);
    let attached = waiting.until_reply();
    if attached["kind"] != "attached" {
        fixture.stop(&owner);
    }
    assert_eq!(attached["kind"], "attached");
    waiting.accept(&attached);
    assert_ne!(waiting.token, token);
    waiting.input("reconnected\n");
    waiting.finish();
    assert_eq!(fixture.wait_file("answer"), "reconnected");
}

#[test]
fn every_replaced_controller_gets_a_notice_during_concurrent_takeovers() {
    let fixture = Fixture::new();
    let owner = fixture.start(&["/bin/sh", "-c", ECHO_LINES]);
    fixture.wait_file("drawn");
    let mut first = fixture.viewer(&owner, true);
    let mut second = fixture.viewer(&owner, false);
    let mut third = fixture.viewer(&owner, false);
    let paused = fixture.pause(&owner);
    for viewer in [&mut second, &mut third] {
        viewer.request(json!({"kind":"attach","control":true,"replace":true,
            "rows":24,"cols":80,"moved":"moved elsewhere"}));
    }
    drop(paused);
    assert_eq!(third.until_reply()["kind"], "attached");
    // Both takeovers have been processed before these barriers are answered.
    first.request(json!({"kind":"controlled"}));
    second.until_reply();
    second.request(json!({"kind":"controlled"}));
    let notices: Vec<_> = [&mut first, &mut second]
        .into_iter()
        .map(|viewer| {
            let items = until(viewer, |items| matches!(items.last(), Some(Item::Reply(_))));
            events(&items, "control")
                .into_iter()
                .cloned()
                .collect::<Vec<_>>()
        })
        .collect();
    third.input("done\n");
    third.finish();
    for notice in notices {
        assert_eq!(notice.len(), 1, "{notice:?}");
        assert_eq!(notice[0]["held"], false);
        assert_eq!(notice[0]["notice"], "moved elsewhere");
    }
}

/// Stands in for a Task step: registers its live controls with the owner, then
/// handles `detach` like `/detach` and exits on `done`.
const INPUT_HINT_STEP: &str = r#"
import hashlib, json, os, socket, struct, sys
home = os.path.realpath(os.environ["LF_HOME"])
digest = hashlib.sha256(home.encode()).hexdigest()[:16]
path = f"{os.path.realpath('/tmp')}/lf-{os.geteuid()}-{digest}/{os.environ['LF_TRANSPORT_OWNER']}.sock"
key = os.environ["LF_TRANSPORT_CONTROL"]
def request(value):
    s = socket.socket(socket.AF_UNIX)
    s.connect(path)
    body = json.dumps(dict(value, key=key, id=1)).encode()
    s.sendall(struct.pack(">BI", 0x11, len(body)) + body)
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

#[test]
fn only_controlling_viewers_see_live_input_controls() {
    let fixture = Fixture::new();
    let owner = fixture.owner(fixture.serve(false, &["python3", "-c", INPUT_HINT_STEP]));
    fixture.wait_file("registered");

    let mut controller = fixture.viewer(&owner, true);
    let items = controller.until_event("hint");
    assert_eq!(events(&items, "hint")[0]["text"], "type here");

    // Detaching viewers keeps the step's registration for the next one.
    controller.input("detach\n");
    controller.until_event("detached");
    let mut returning = fixture.viewer(&owner, true);
    returning.until_event("hint");
    let mut observer = fixture.viewer(&owner, false);
    returning.input("done\n");
    returning.finish();

    let items = observer.until_event("child_exited");
    assert!(events(&items, "hint").is_empty(), "{items:?}");
    assert_eq!(out(&items), b"working\n");
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
        match active.next().unwrap() {
            Item::Out(bytes) => {
                stdout_bytes += bytes.len();
                // Pace the producer with the active consumer, while the
                // observer accumulates enough unread output to disconnect.
                if stdout_bytes < 4 * 1024 * 1024 && stdout_bytes % 65536 == 0 {
                    active.input("next\n");
                }
            }
            Item::Err(bytes) => stderr.extend(bytes),
            Item::Event(event) if event["type"] == "child_exited" => {
                assert_eq!(event["code"], 0);
                break;
            }
            Item::Event(_) => {}
            other => panic!("unexpected item: {other:?}"),
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

/// Answer the Session's control probe, then accept its viewer connection.
fn accept_attachment(listener: &UnixListener) -> UnixStream {
    let started = Instant::now();
    loop {
        match listener.accept() {
            Ok((mut socket, _)) => {
                socket.set_nonblocking(false).unwrap();
                socket
                    .set_read_timeout(Some(Duration::from_secs(10)))
                    .unwrap();
                let (tag, payload) = read_frame(&mut socket).unwrap();
                assert_eq!(tag, REQUEST);
                let request: Value = serde_json::from_slice(&payload).unwrap();
                match request["kind"].as_str().unwrap() {
                    "controlled" => socket
                        .write_all(&frame(
                            REPLY,
                            json!({"answers":request["id"],"seq":0,"kind":"controlled","held":false})
                                .to_string()
                                .as_bytes(),
                        ))
                        .unwrap(),
                    "attach" => return socket,
                    other => panic!("unexpected request {other}"),
                }
            }
            Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {
                assert!(started.elapsed() < Duration::from_secs(10));
                std::thread::sleep(Duration::from_millis(20));
            }
            Err(error) => panic!("accept: {error}"),
        }
    }
}

#[test]
fn successor_handoff_restores_the_terminal_and_preserves_command_results() {
    let fixture = Fixture::new();
    let owner = fixture.start(&["/bin/sh", "-c", "sleep 60"]);
    let session = fixture.session(&owner);
    let endpoint = fixture.endpoint(&owner);
    let retained = endpoint.with_extension("retained");
    std::fs::rename(&endpoint, &retained).unwrap();
    let listener = UnixListener::bind(&endpoint).unwrap();
    listener.set_nonblocking(true).unwrap();

    let entry = fixture.home.path().join("successor entry");
    std::fs::write(
        &entry,
        r#"#!/bin/sh
[ "$#" = 3 ] && [ "$1" = session ] && [ "$2" = resume ] && [ "$3" = "$EXPECTED_SESSION" ] || exit 91
[ "$(stty -g)" = "$EXPECTED_TERMINAL" ] || exit 92
printf 'successor saw restored terminal\n'
if [ "$SUCCESSOR_RESULT" = signal ]; then kill -TERM $$; fi
exit "$SUCCESSOR_RESULT"
"#,
    )
    .unwrap();
    std::fs::set_permissions(&entry, std::fs::Permissions::from_mode(0o755)).unwrap();

    let mut master = -1;
    let mut slave = -1;
    // SAFETY: openpty initializes both descriptors; optional settings are null.
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
    let mut terminal = unsafe { std::fs::File::from_raw_fd(master) };
    // SAFETY: master is our live PTY; nonblocking reads drain the completed viewer.
    assert_eq!(
        unsafe { libc::fcntl(master, libc::F_SETFL, libc::O_NONBLOCK) },
        0
    );
    // SAFETY: the slave is a distinct, newly opened descriptor.
    let input = unsafe { std::fs::File::from_raw_fd(slave) };
    let original = Command::new("stty")
        .arg("-g")
        .stdin(input.try_clone().unwrap())
        .output()
        .unwrap();
    assert!(original.status.success());
    let original = String::from_utf8(original.stdout).unwrap();

    for (result, exit, error) in [
        ("0", 0, None),
        ("23", 23, None),
        ("signal", 1, Some("ended by a signal")),
        ("missing", 1, Some("run /")),
        ("empty", 1, Some("empty successor command")),
        ("none", 0, None),
    ] {
        let successor = match result {
            "none" => Value::Null,
            "empty" => json!([]),
            "missing" => json!([
                fixture.home.path().join("missing"),
                "session",
                "resume",
                session
            ]),
            _ => json!([entry, "session", "resume", session]),
        };
        let mut viewer = fixture
            .command(&["session", "resume", &session])
            .env("EXPECTED_SESSION", &session)
            .env("EXPECTED_TERMINAL", original.trim())
            .env("SUCCESSOR_RESULT", result)
            .stdin(input.try_clone().unwrap())
            .stdout(input.try_clone().unwrap())
            .stderr(Stdio::piped())
            .spawn()
            .unwrap();
        let mut socket = accept_attachment(&listener);
        socket
            .write_all(&frame(
                REPLY,
                json!({
                    "answers":1,"seq":0,"kind":"attached","viewer":"v","reconnect":"r",
                    "nonce":"00","token":"claim","terminal":true,"grace_ms":60000,"owner":"v"
                })
                .to_string()
                .as_bytes(),
            ))
            .unwrap();
        socket.write_all(&frame(SNAPSHOT, b"\x1b[?1049h")).unwrap();
        let event = json!({"seq":1,"type":"control","held":false,
            "notice":"owner retired","successor":successor});
        socket
            .write_all(&frame(OUT, format!("\x1b_lf1;00;{event}\x1b\\").as_bytes()))
            .unwrap();
        let deadline = Instant::now() + Duration::from_secs(10);
        let mut screen = Vec::new();
        loop {
            let exited = viewer.try_wait().unwrap().is_some();
            // Drain while the viewer runs: a PTY can hold less than its reset sequence.
            match terminal.read_to_end(&mut screen) {
                Ok(_) => {}
                Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {}
                Err(error) => panic!("read terminal: {error}"),
            }
            if exited {
                break;
            }
            if Instant::now() >= deadline {
                viewer.kill().unwrap();
                viewer.wait().unwrap();
                panic!(
                    "{result}: viewer did not exit: {}",
                    String::from_utf8_lossy(&screen)
                );
            }
            std::thread::sleep(Duration::from_millis(20));
        }
        let output = viewer.wait_with_output().unwrap();
        let stdout = String::from_utf8_lossy(&screen);
        let stderr = String::from_utf8_lossy(&output.stderr);
        assert_eq!(
            output.status.code(),
            Some(exit),
            "{result}: {stdout} {stderr}"
        );
        assert!(stderr.contains("owner retired"), "{result}: {stderr}");
        if let Some(error) = error {
            assert!(stderr.contains(error), "{result}: {stderr}");
        }
        assert!(stdout.contains("\x1b[?1049l"), "{result}: {stdout}");
        if matches!(result, "0" | "23" | "signal") {
            let restored = stdout.find("\x1b[?1049l").unwrap();
            let followed = stdout.find("successor saw restored terminal").unwrap();
            assert!(restored < followed, "{stdout}");
        }
    }
    drop(listener);
    std::fs::remove_file(&endpoint).unwrap();
    std::fs::rename(retained, endpoint).unwrap();
    fixture.stop(&owner);
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
    let mut socket = accept_attachment(&listener);
    std::fs::remove_file(&endpoint).unwrap();
    std::fs::rename(retained, endpoint).unwrap();
    drop(listener);
    if silent == SilentOwner::Cancelled {
        // SAFETY: signal only this test's live viewer, never the owner/provider.
        assert_eq!(unsafe { libc::kill(viewer.id() as i32, libc::SIGINT) }, 0);
    }
    if silent == SilentOwner::Stalled {
        socket
            .write_all(&frame(
                REPLY,
                json!({"answers":1,"seq":0,"kind":"attached","viewer":"v","reconnect":"r",
                    "nonce":"00","token":"claim","terminal":false,"grace_ms":60000,
                    "owner":null})
                .to_string()
                .as_bytes(),
            ))
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
    let mut remaining = Vec::new();
    socket.read_to_end(&mut remaining).unwrap();
    assert!(
        remaining.is_empty(),
        "viewer submitted input before acknowledgement"
    );
}

#[test]
fn first_native_launch_keeps_only_valid_desktop_terminal_placement() {
    for valid in [true, false] {
        let fixture = Fixture::new();
        let path = fixture.waiting_provider();
        let mut command = fixture.command(&["-i", "-a", "claude", ":", "terminal placement"]);
        command.env("PATH", path);
        let terminal = Terminal::spawn_with_placement(command, Some(valid));
        let launch = fixture.launches(1).pop().unwrap();
        let session = session_of(&fixture.sql(), &launch);
        let placement: Option<String> = fixture.sql().query_row(
            "SELECT p.terminal_id FROM agent_sessions s JOIN processes a ON a.id=s.agent_process_id JOIN processes p ON p.id=a.attached_lf_process_id WHERE s.id=?1",
            [&session], |row| row.get(0),
        ).unwrap();
        let owner = fixture.owner_of(&session);
        terminal.stop();
        fixture.stop(&owner);
        assert_eq!(placement.as_deref(), valid.then_some("desktop-shell"));
    }
}

#[test]
fn native_program_status_has_no_stale_launcher_working_record() {
    let fixture = Fixture::new();
    let path = fixture.provider(
        r#"#!/bin/sh
if [ "$1" = --version ]; then echo '2.1.302'; exit; fi
stty -echo < /dev/tty
printf 'native ready\n'
while read -r state < /dev/tty; do
    [ "$state" = exit ] && exit 0
    # Claude's title-disable flag also disables its Program Status reports.
    if [ -z "${CLAUDE_CODE_DISABLE_TERMINAL_TITLE-}" ]; then
        printf '\033]7501;state=%s:app=claude-code\033\\' "$state"
    fi
    printf 'reported %s\n' "$state"
done
"#,
    );
    let owner =
        fixture.owner(fixture.native(Path::new(env!("CARGO_BIN_EXE_lf")), &path, "native status"));
    let mut viewer = fixture.viewer(&owner, true);
    let mut received = out(&until_out(&mut viewer, "native ready"));
    let states = [
        "working", "idle", "blocked", "working", "done", "idle", "clear",
    ];
    for state in states {
        viewer.input(&format!("{state}\n"));
        received.extend(out(&until_out(&mut viewer, &format!("reported {state}"))));
        if state == "working" {
            // No provider output or turn settlement while this read completes.
            // A quiet stream must not synthesize idle.
            viewer.request(json!({"kind":"controlled"}));
            let quiet = until(
                &mut viewer,
                |items| matches!(items.last(), Some(Item::Reply(reply)) if reply["kind"] == "controlled"),
            );
            received.extend(out(&quiet));
        }
        drop(viewer);
        viewer = fixture.viewer(&owner, true);
        let snapshot = match viewer.next().unwrap() {
            Item::Snapshot(bytes) => bytes,
            other => panic!("expected retained snapshot, got {other:?}"),
        };
        let mut terminal = ghostty_vt::Terminal::new(80, 24).unwrap();
        terminal.write(b"\x1b]7501;id=previous:state=working\x1b\\");
        terminal.write(&snapshot);
        let status = terminal.program_status();
        assert!(status.seen);
        if state == "clear" {
            assert!(status.records.is_empty());
        } else {
            assert_eq!(status.records.len(), 1);
            assert_eq!(
                serde_json::to_value(status.records[0].state).unwrap(),
                state
            );
            assert_eq!(status.records[0].id, None);
            assert_eq!(status.records[0].app.as_deref(), Some("claude-code"));
        }
    }
    viewer.input("exit\n");
    viewer.finish();

    let received = String::from_utf8(received).unwrap();
    let reports: Vec<_> = received
        .split("\x1b]7501;")
        .skip(1)
        .map(|report| report.split("\x1b\\").next().unwrap())
        .collect();
    assert_eq!(
        reports,
        states.map(|state| format!("state={state}:app=claude-code")),
        "native reports must pass unchanged without a competing lf record"
    );
}

#[test]
fn native_program_status_detects_support_before_a_viewer_attaches() {
    let fixture = Fixture::new();
    let owner = fixture.start(&[
        "python3",
        "-c",
        r#"
import os, select, tty
from pathlib import Path
tty.setraw(0)
os.write(1, b"\x1b]7501;?\x07")
reply = b""
while select.select([0], [], [], 3)[0]:
    reply += os.read(0, 1024)
    if reply.endswith(b"\x07"):
        break
path = Path(os.environ["HOME"]) / "detection"
path.with_suffix(".tmp").write_text(reply.hex() or "missing")
path.with_suffix(".tmp").rename(path)
os.write(1, b"\x1b]7501;state=blocked:kind=question\x1b\\ready")
os.read(0, 1)
"#,
    ]);
    let reply = fixture.wait_file("detection");
    let mut viewer = fixture.viewer(&owner, true);
    let items = until_out(&mut viewer, "ready");
    let mut terminal = ghostty_vt::Terminal::new(80, 24).unwrap();
    for item in items {
        match item {
            Item::Snapshot(bytes) | Item::Out(bytes) => terminal.write(&bytes),
            _ => {}
        }
    }
    viewer.input("x");
    viewer.finish();
    assert_eq!(reply, hex::encode(b"\x1b]7501;?\x07"));
    assert_eq!(
        terminal.program_status().records[0].state,
        program_status::State::Blocked
    );
    assert_eq!(
        terminal.program_status().records[0].kind,
        Some(program_status::Kind::Question)
    );
}

fn native_pending_request(silent_owner: Option<SilentOwner>) {
    let fixture = Fixture::new();
    let path = fixture.provider(
        r#"#!/bin/sh
if [ "$1" = --version ]; then echo '2.1.302'; exit; fi
stty -echo < /dev/tty
printf '%s:%s\n' "$$" "$LF_CAPTURE_KEY" >> "$HOME/launches"
printf 'permission request-17\n'
read -r answer < /dev/tty
printf '%s:%s\n' "$$" "$answer" > "$HOME/answer"
"#,
    );
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
    owner.env("PATH", path);
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

/// A `claude` stand-in that logs its PID and parent, then waits on its terminal.
fn parented_provider(fixture: &Fixture) -> String {
    fixture.provider(
        r#"#!/bin/sh
if [ "$1" = --version ]; then echo '2.1.302'; exit; fi
stty -echo < /dev/tty
printf '%s:%s:%s\n' "$$" "$PPID" "$LF_CAPTURE_KEY" >> "$HOME/launches"
read -r answer < /dev/tty
"#,
    )
}

fn pid_of_process(fixture: &Fixture, id: &str) -> i64 {
    fixture
        .sql()
        .query_row("SELECT pid FROM processes WHERE id=?1", [id], |r| r.get(0))
        .unwrap()
}

fn command_of(pid: i64) -> String {
    let output = Command::new("ps")
        .args(["-o", "command=", "-p", &pid.to_string()])
        .output()
        .unwrap();
    String::from_utf8_lossy(&output.stdout).trim().to_owned()
}

/// The launcher becomes its provider: Claude's parent is the owner itself, no
/// `lf -i` waits beside it, and the owner records what the launcher used to
/// when the provider exits.
#[test]
fn a_terminal_launcher_becomes_its_provider_and_the_owner_records_its_exit() {
    let fixture = Fixture::new();
    let path = parented_provider(&fixture);
    let mut owner = fixture.serve(
        true,
        &[
            env!("CARGO_BIN_EXE_lf"),
            "--interactive",
            "--agent",
            "claude",
            ":",
            "stay small",
        ],
    );
    owner.env("PATH", &path);
    let owner = fixture.owner(owner);
    let launched = fixture.launches(1)[0].clone();
    let mut fields = launched.split(':');
    let provider: i64 = fields.next().unwrap().parse().unwrap();
    let parent: i64 = fields.next().unwrap().parse().unwrap();
    let capture = fields.next().unwrap().to_owned();
    assert_eq!(
        parent,
        pid_of_process(&fixture, &owner),
        "a launcher still waits between the owner and its provider: {}",
        command_of(parent)
    );
    let sql = fixture.sql();
    let session: String = sql
        .query_row(
            "SELECT session_id FROM session_events WHERE kind='captured' AND receipt_key=?1",
            [&capture],
            |r| r.get(0),
        )
        .unwrap();
    let (agent, launcher): (i64, String) = sql
        .query_row(
            "SELECT p.pid, p.attached_lf_process_id FROM processes p
             JOIN agent_sessions s ON s.agent_process_id=p.id WHERE s.id=?1",
            [&session],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )
        .unwrap();
    assert_eq!(
        agent, provider,
        "the AgentProcess names the exec'd launcher's PID"
    );
    assert!(
        fixture.running(&launcher),
        "the launcher's Process completed before its provider exited"
    );
    let mut viewer = fixture.viewer(&owner, true);
    viewer.input("bye\n");
    viewer.finish();
    fixture.ends(&owner);
    let exit: (String, Option<String>) = sql
        .query_row(
            "SELECT json_extract(payload,'$.outcome'), lf_process_id FROM session_events
             WHERE session_id=?1 AND json_extract(payload,'$.type')='attachment_exit'",
            [&session],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )
        .unwrap();
    assert_eq!(exit, ("completed".to_owned(), Some(launcher.clone())));
    let settled: i64 = sql
        .query_row(
            "SELECT count(*) FROM session_events WHERE session_id=?1 AND payload LIKE '%terminal.json%'",
            [&session],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(settled, 1, "the owner did not settle the capture");
    let agent_done: bool = sql
        .query_row(
            "SELECT p.completed_at IS NOT NULL FROM processes p
             JOIN agent_sessions s ON s.agent_process_id=p.id WHERE s.id=?1",
            [&session],
            |r| r.get(0),
        )
        .unwrap();
    assert!(agent_done, "the AgentProcess never recorded its exit");
    let launcher_outcome: Option<String> = sql
        .query_row(
            "SELECT outcome FROM processes WHERE id=?1",
            [&launcher],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(launcher_outcome.as_deref(), Some("succeeded"));
}

#[test]
fn native_exit_preserves_returning_launcher_results() {
    for code in [7, 130] {
        for handoff in [false, true] {
            let fixture = Fixture::new();
            let path = parented_provider(&fixture);
            let provider = fixture.home.path().join("bin/claude");
            let mut script = std::fs::read_to_string(&provider).unwrap();
            script.push_str("exit \"$answer\"\n");
            std::fs::write(provider, script).unwrap();
            let argv = [
                env!("CARGO_BIN_EXE_lf"),
                "-i",
                "-a",
                "claude",
                ":",
                "exit proof",
            ];
            fixture
                .repo
                .create_file(".lf/skills/probe.md", "Exit proof.");
            fixture
                .repo
                .create_file(".lf/graphs/return.yaml", "- probe\n");
            let mut command = if handoff {
                fixture.serve(true, &argv)
            } else {
                fixture.serve(
                    true,
                    &[
                        env!("CARGO_BIN_EXE_lf"),
                        "-i",
                        "-a",
                        "claude",
                        "graph",
                        "return",
                    ],
                )
            };
            command.env("PATH", path);
            let owner = fixture.owner(command);
            let launched = fixture.launches(1)[0].clone();
            let capture = launched.split(':').nth(2).unwrap();
            let session: String = fixture.sql().query_row(
                "SELECT session_id FROM session_events WHERE kind='captured' AND receipt_key=?1",
                [capture], |r| r.get(0),
            ).unwrap();
            let launcher: String = fixture.sql().query_row(
                "SELECT attached_lf_process_id FROM processes WHERE id=(SELECT agent_process_id FROM agent_sessions WHERE id=?1)",
                [&session], |r| r.get(0),
            ).unwrap();
            let mut viewer = fixture
                .command(&["session", "attach", &owner, "--session-json", &session])
                .stdin(Stdio::piped())
                .stdout(Stdio::piped())
                .stderr(Stdio::piped())
                .spawn()
                .unwrap();
            viewer
                .stdin
                .take()
                .unwrap()
                .write_all(format!("{code}\n").as_bytes())
                .unwrap();
            let result = wait_for_viewer(viewer, Duration::from_secs(30));
            fixture.ends(&owner);
            assert_eq!(
                result.status.code(),
                Some(1),
                "handoff={handoff}, provider={code}: {result:?}"
            );
            assert!(
                !String::from_utf8_lossy(&result.stdout).contains("\"open_argv\""),
                "a failed resume printed success JSON"
            );
            let recorded: (i32, String) = fixture
                .sql()
                .query_row(
                    "SELECT exit_code,outcome FROM processes WHERE id=?1",
                    [&launcher],
                    |r| Ok((r.get(0)?, r.get(1)?)),
                )
                .unwrap();
            assert_eq!(recorded, (1, "failed".into()));
            let capture: String = fixture.sql().query_row(
                "SELECT json_extract(payload,'$.evidence.outcome') FROM session_events WHERE session_id=?1 AND json_extract(payload,'$.source')='terminal.json'",
                [&session], |r| r.get(0),
            ).unwrap();
            assert_eq!(capture, "failed", "handoff={handoff}, provider={code}");
            assert_eq!(fixture.launches(1).len(), 1);
        }
    }
}

#[test]
fn a_delayed_owner_keeps_exclusive_ownership_of_the_provider_wait() {
    let fixture = Fixture::new();
    let path = parented_provider(&fixture);
    let mut command = fixture.serve(
        true,
        &[
            "/bin/sh",
            "-c",
            "while [ ! -f \"$HOME/start-launch\" ]; do sleep 0.01; done; kill -STOP \"$PPID\"; exec \"$@\"",
            "launcher",
            env!("CARGO_BIN_EXE_lf"),
            "-i",
            "-a",
            "claude",
            ":",
            "delayed handoff",
        ],
    );
    command.env("PATH", path);
    let owner = fixture.owner(command);
    let owner_pid = pid_of_process(&fixture, &owner);
    std::fs::write(fixture.home.path().join("start-launch"), "").unwrap();
    let deadline = Instant::now() + Duration::from_secs(30);
    let opened = loop {
        let opened: bool = fixture.sql().query_row(
            "SELECT EXISTS(SELECT 1 FROM session_events WHERE json_extract(payload,'$.type')='interactive_opened')",
            [], |row| row.get(0),
        ).unwrap();
        if opened || Instant::now() >= deadline {
            break opened;
        }
        std::thread::sleep(Duration::from_millis(20));
    };
    // Exceed the former acknowledgement timeout while the owner is stopped.
    std::thread::sleep(Duration::from_secs(3));
    // SAFETY: this fixture owns the stopped process and resumes it before assertions.
    unsafe { libc::kill(owner_pid as libc::pid_t, libc::SIGCONT) };
    assert!(opened, "launcher never reached its exit handoff");
    let launch = fixture.launches(1)[0].clone();
    let parent: i64 = launch.split(':').nth(1).unwrap().parse().unwrap();
    let mut viewer = fixture.viewer(&owner, true);
    viewer.input("done\n");
    let frames = viewer.until_event("child_exited");
    fixture.ends(&owner);
    assert_eq!(
        parent, owner_pid,
        "a delayed acknowledgement restored a waiting launcher"
    );
    assert_eq!(events(&frames, "child_exited").last().unwrap()["code"], 0);
    let settlements: i64 = fixture.sql().query_row(
        "SELECT count(*) FROM session_events WHERE json_extract(payload,'$.source')='terminal.json'",
        [], |row| row.get(0),
    ).unwrap();
    assert_eq!(settlements, 1, "the capture must settle exactly once");
}

#[test]
fn failed_native_exec_settles_once_without_launching_a_provider() {
    let fixture = Fixture::new();
    // Discovery succeeds, then exec meets an absent interpreter. No real
    // provider or substitute from the Machine can run.
    let path = fixture.provider(
        r#"#!/bin/sh
if [ "$1" = --version ]; then
    printf '#!/absent/fixture-interpreter\n' > "$0"
    echo '2.1.302'
    exit
fi
exit 99
"#,
    );
    let mut command = fixture.serve(
        true,
        &[
            env!("CARGO_BIN_EXE_lf"),
            "-i",
            "-a",
            "claude",
            ":",
            "exec failure",
        ],
    );
    command.env("PATH", path);
    let owner = fixture.owner(command);
    let mut viewer = fixture.viewer(&owner, true);
    let frames = viewer.until_event("child_exited");
    assert_eq!(events(&frames, "child_exited").last().unwrap()["code"], 1);
    fixture.ends(&owner);
    assert!(!fixture.home.path().join("launches").exists());
    let sql = fixture.sql();
    let (code, outcome, provider_outcome): (i32, String, Option<String>) = sql.query_row(
        "SELECT launcher.exit_code, launcher.outcome, agent.outcome FROM processes agent JOIN processes launcher ON launcher.id=agent.parent_lf_process_id WHERE agent.kind='agent'",
        [], |r| Ok((r.get(0)?,r.get(1)?,r.get(2)?)),
    ).unwrap();
    assert_eq!(provider_outcome, None);
    assert_eq!(code, 1);
    assert_eq!(outcome, "failed");
    let captures: i64 = sql.query_row(
        "SELECT count(*) FROM session_events WHERE json_extract(payload,'$.source')='terminal.json' AND json_extract(payload,'$.evidence.outcome')='failed'",
        [], |r| r.get(0),
    ).unwrap();
    assert_eq!(captures, 1);
}

fn returned_session(output: &Output) -> Value {
    let text = String::from_utf8_lossy(&output.stdout);
    let start = text
        .find("{\n")
        .unwrap_or_else(|| panic!("Session JSON missing: {output:?}"));
    serde_json::from_str(text[start..].trim()).unwrap()
}

#[test]
fn resume_json_returns_once_when_it_starts_the_owner() {
    let fixture = Fixture::new();
    let path = parented_provider(&fixture);
    let provider = fixture.home.path().join("bin/claude");
    let script = std::fs::read_to_string(&provider)
        .unwrap()
        .replace("stty -echo < /dev/tty\n", "")
        .replace(" < /dev/tty", "");
    std::fs::write(provider, script).unwrap();
    let prepared = fixture
        .command(&["-a", "claude", "resume", "--prepare", "--json"])
        .env("PATH", &path)
        .output()
        .unwrap();
    assert!(prepared.status.success(), "{prepared:?}");
    let record: Value = serde_json::from_slice(&prepared.stdout).unwrap();
    let session = record["id"].as_str().unwrap();
    let mut viewer = fixture
        .command(&["-a", "claude", "resume", session, "--json"])
        .env("PATH", path)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    fixture.launches(1);
    viewer.stdin.take().unwrap().write_all(b"done\n").unwrap();
    let result = wait_for_viewer(viewer, Duration::from_secs(30));
    assert!(result.status.success(), "{result:?}");
    let returned: Value = serde_json::from_slice(&result.stdout).unwrap();
    assert_eq!(returned["id"], session);
    assert_eq!(fixture.launches(1).len(), 1);
    let owner: String = fixture
        .sql()
        .query_row(
            "SELECT transport_lf_process_id FROM agent_sessions WHERE id=?1",
            [session],
            |r| r.get(0),
        )
        .unwrap();
    fixture.ends(&owner);
}

#[test]
fn resume_json_returns_after_relay_and_viewer_takeover() {
    let fixture = Fixture::new();
    let owner = fixture.start(&["/bin/sh", "-c", "stty -echo; read answer"]);
    let session = fixture.session(&owner);
    let args = ["resume", &session, "--json"];
    let mut viewer = fixture
        .command(&args)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    let deadline = Instant::now() + Duration::from_secs(10);
    while !command_of(i64::from(viewer.id())).contains("session attach") {
        if viewer.try_wait().unwrap().is_some() || Instant::now() >= deadline {
            let _ = viewer.kill();
            let result = viewer.wait_with_output().unwrap();
            fixture.stop(&owner);
            panic!("viewer did not become a relay: {result:?}");
        }
        std::thread::sleep(Duration::from_millis(20));
    }
    let mut observer = fixture.viewer(&owner, false);
    loop {
        observer.request(json!({"kind":"controlled"}));
        if observer.until_reply()["held"] == true {
            break;
        }
        assert!(Instant::now() < deadline, "viewer never attached");
        std::thread::sleep(Duration::from_millis(20));
    }
    // The departing viewer still owes its JSON; the provider stays live.
    let mut next = fixture.viewer(&owner, true);
    let result = wait_for_viewer(viewer, Duration::from_secs(10));
    assert!(result.status.success(), "{result:?}");
    let record = returned_session(&result);
    assert_eq!(record["id"], session);
    assert!(fixture.running(&owner));
    next.input("done\n");
    next.finish();
    fixture.ends(&owner);
}

fn prepared_thread_json(command: &str, fork: bool, source_provider: &str) {
    let fixture = Fixture::new();
    let bin = fixture.home.path().join("bin");
    // The only executable provider is this stand-in; no native turn is sent.
    fixture.provider(
        "#!/bin/sh\nif [ \"$1\" = --version ]; then echo '2.1.302'; exit; fi\nprintf '%s\\n' \"$LF_CAPTURE_KEY\" >> \"$HOME/launches\"\n",
    );
    let path = format!("{}:/usr/bin:/bin", bin.display());
    let prepared = fixture
        .command(&["-a", source_provider, "resume", "--prepare", "--json"])
        .env("PATH", &path)
        .output()
        .unwrap();
    assert!(prepared.status.success(), "{prepared:?}");
    let source: Value = serde_json::from_slice(&prepared.stdout).unwrap();
    let source_id = source["id"].as_str().unwrap();
    let sql = fixture.sql();
    let input = format!("run_{}", uuid::Uuid::new_v4().simple());
    sql.execute(
        "INSERT INTO session_events(session_id,kind,receipt_key,observed_at,payload)
         VALUES(?1,'captured',?2,1,json_object('artifact_key',?2))",
        [source_id, &input],
    )
    .unwrap();
    sql.execute(
        "UPDATE agent_sessions SET current_capture=?2,input_published=1 WHERE id=?1",
        rusqlite::params![source_id, sql.last_insert_rowid()],
    )
    .unwrap();
    let native = uuid::Uuid::new_v4().to_string();
    sql.execute(
        "UPDATE agent_sessions SET provider_thread=?2 WHERE id=?1",
        [source_id, &native],
    )
    .unwrap();
    sql.execute(
        "INSERT INTO session_events(session_id,kind,receipt_key,observed_at,payload,captured_event)
         SELECT id,'observed',?2||':fixture-native',created_at,json_object('input_id',?2,'source','provider-session:fixture','evidence',json_object('schema_version',1,'provider_session_id',?3,'account_id',NULL)),current_capture FROM agent_sessions WHERE id=?1",
        [source_id, &input, &native],
    ).unwrap();
    let transcript = if source_provider == "codex" {
        fixture.home.path().join(format!(
            "codex/sessions/2026/11/02/rollout-2026-11-02T00-00-00-{native}.jsonl"
        ))
    } else {
        fixture
            .home
            .path()
            .join(format!("claude/projects/fixture/{native}.jsonl"))
    };
    std::fs::create_dir_all(transcript.parent().unwrap()).unwrap();
    let request = "Keep the original request";
    let turn = if source_provider == "codex" {
        json!({"type":"response_item","payload":{"type":"message","role":"user","content":[{"type":"input_text","text":request}]}})
    } else {
        json!({"type":"user","userType":"external","message":{"content":request}})
    };
    std::fs::write(transcript, turn.to_string()).unwrap();
    let mut args = vec!["-a", "claude", "session", command, source_id, "--json"];
    if fork {
        args.push("--fork");
    }
    let viewer = fixture
        .command(&args)
        .env("PATH", &path)
        .env("CLAUDE_CONFIG_DIR", fixture.home.path().join("claude"))
        .env("CODEX_HOME", fixture.home.path().join("codex"))
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    let result = wait_for_viewer(viewer, Duration::from_secs(30));
    assert!(result.status.success(), "{result:?}");
    // Exactly one JSON record, for the resulting conversation.
    let returned: Value = serde_json::from_slice(&result.stdout).unwrap();
    let id = returned["id"].as_str().unwrap();
    assert_eq!(id != source_id, fork);
    assert_eq!(returned["provider"], "claude");
    let (next_input, request, owner): (String, String, String) = sql
        .query_row(
            "SELECT c.receipt_key,s.request,s.transport_lf_process_id FROM agent_sessions s JOIN session_events c ON c.seq=s.current_capture WHERE s.id=?1",
            [id],
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
        )
        .unwrap();
    assert_ne!(next_input, input);
    assert!(request.contains("Keep the original request"));
    assert_eq!(fixture.launches(1), vec![next_input]);
    let sessions: i64 = sql
        .query_row("SELECT count(*) FROM agent_sessions", [], |r| r.get(0))
        .unwrap();
    assert_eq!(sessions, if fork { 2 } else { 1 });
    let captures: i64 = sql
        .query_row(
            "SELECT count(*) FROM session_events WHERE kind='captured'",
            [],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(captures, 2, "thread preparation was replayed");
    if fork {
        let retained: String = sql
            .query_row(
                "SELECT c.receipt_key FROM agent_sessions s JOIN session_events c ON c.seq=s.current_capture WHERE s.id=?1",
                [source_id],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(retained, input);
    }
    fixture.ends(&owner);
}

#[test]
fn resume_fork_json_launches_once_and_returns_the_new_session() {
    prepared_thread_json("resume", true, "claude");
}

#[test]
fn session_new_json_launches_once_and_keeps_the_session() {
    prepared_thread_json("new", false, "claude");
}

#[test]
fn provider_changing_resume_json_launches_once_and_keeps_the_session() {
    prepared_thread_json("resume", false, "codex");
}

/// Opening a prepared conversation leaves one owner and one relay: the connect
/// becomes the launcher, the launcher becomes Claude, and a viewer that
/// resolved the Session relays bytes with nothing else loaded.
#[test]
fn opening_a_prepared_conversation_runs_one_owner_and_one_relay() {
    let fixture = Fixture::new();
    let path = parented_provider(&fixture);
    let prepared = fixture
        .command(&["-a", "claude", "session", "resume", "--prepare", "--json"])
        .env("PATH", &path)
        .output()
        .unwrap();
    assert!(
        prepared.status.success(),
        "{}",
        String::from_utf8_lossy(&prepared.stderr)
    );
    let prepared: Value = serde_json::from_slice(&prepared.stdout).unwrap();
    let session = prepared["id"].as_str().unwrap().to_owned();
    // What Desktop's pane runs, under the terminal owner it delegates to.
    let mut owner = fixture.serve(
        true,
        &[
            env!("CARGO_BIN_EXE_lf"),
            "-a",
            "claude",
            "session",
            "resume",
            &session,
        ],
    );
    owner.env("PATH", &path);
    let owner = fixture.owner(owner);
    let launched = fixture.launches(1)[0].clone();
    let mut fields = launched.split(':');
    let provider: i64 = fields.next().unwrap().parse().unwrap();
    let parent: i64 = fields.next().unwrap().parse().unwrap();
    let owner_pid = pid_of_process(&fixture, &owner);
    assert_eq!(
        parent,
        owner_pid,
        "a process still waits between the owner and its provider: {}",
        command_of(parent)
    );
    // The owner's planning sync runs short `git` commands; no `lf` stays.
    let children = Command::new("pgrep")
        .args(["-P", &owner_pid.to_string()])
        .output()
        .unwrap();
    let lf_children: Vec<String> = std::str::from_utf8(&children.stdout)
        .unwrap()
        .split_whitespace()
        .filter(|pid| *pid != provider.to_string())
        .map(|pid| command_of(pid.parse().unwrap()))
        .filter(|command| command.contains(env!("CARGO_BIN_EXE_lf")))
        .collect();
    assert!(
        lf_children.is_empty(),
        "lf processes beside the provider: {lf_children:?}"
    );
    let mut viewer = fixture
        .command(&["-a", "claude", "session", "resume", &session, "--json"])
        .env("PATH", &path)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    let deadline = Instant::now() + Duration::from_secs(30);
    let relay = loop {
        let command = command_of(i64::from(viewer.id()));
        if command.contains("session attach") {
            break command;
        }
        assert!(
            Instant::now() < deadline && viewer.try_wait().unwrap().is_none(),
            "the viewer kept the process that resolved the Session: {command}"
        );
        std::thread::sleep(Duration::from_millis(50));
    };
    assert!(relay.contains(&owner), "{relay}");
    let db = rusqlite::Connection::open(fixture.home.path().join("loopflow.db")).unwrap();
    let completed: Option<i64> = db
        .query_row(
            "SELECT completed_at FROM processes WHERE pid=?1",
            [viewer.id()],
            |row| row.get(0),
        )
        .unwrap();
    assert!(
        completed.is_none(),
        "exec completed the live viewer's command"
    );
    viewer.stdin.take().unwrap().write_all(b"bye\n").unwrap();
    let result = wait_for_viewer(viewer, Duration::from_secs(30));
    assert!(
        result.status.success(),
        "{}",
        String::from_utf8_lossy(&result.stderr)
    );
    let record = returned_session(&result);
    assert_eq!(record["id"], session);
    assert_eq!(
        fixture.launches(1).len(),
        1,
        "the provider was launched twice"
    );
    fixture.ends(&owner);
}

#[test]
fn interactive_graph_settles_each_step_before_starting_the_next() {
    let fixture = Fixture::new();
    let path = parented_provider(&fixture);
    fixture
        .repo
        .create_file(".lf/skills/probe.md", "Do the fixture work.");
    fixture
        .repo
        .create_file(".lf/graphs/proof.yaml", "- probe\n- probe\n");
    let mut command = fixture.serve(
        true,
        &[
            env!("CARGO_BIN_EXE_lf"),
            "-i",
            "-a",
            "claude",
            "graph",
            "proof",
        ],
    );
    command.env("PATH", path);
    let owner = fixture.owner(command);
    let first = fixture.launches(1)[0].clone();
    let capture = first.split(':').nth(2).unwrap();
    let mut viewer = fixture.viewer(&owner, true);
    viewer.input("next\n");
    fixture.launches(2);
    let sql = fixture.sql();
    let (completed, outcome): (bool, String) = sql
        .query_row(
            "SELECT agent.completed_at IS NOT NULL, launcher.outcome
         FROM session_events e
         JOIN agent_sessions s ON s.id=e.session_id
         JOIN processes agent ON agent.id=s.agent_process_id
         JOIN processes launcher ON launcher.id=e.lf_process_id
         WHERE e.kind='captured' AND e.receipt_key=?1",
            [capture],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .unwrap();
    assert!(
        completed,
        "the first provider's exit waited for the Graph to end"
    );
    assert_eq!(outcome, "succeeded");
    viewer.input("done\n");
    viewer.finish();
    fixture.ends(&owner);
}

/// Closing every view of a conversation ends its agent once
/// `session.end_detached_after` of quiet passes. A viewer, live turn, output or
/// running Graph keeps it; stale status and finished Graphs do not. The ended
/// conversation reopens from saved history without replaying its input.
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
    for input in [
        "idle", "viewed", "busy", "graph", "step", "stale", "talking",
    ] {
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
    fixture.launches(7);
    let [idle_owner, viewed_owner, busy_owner, graph_owner, step_owner, stale_owner, talking_owner] =
        <[String; 7]>::try_from(owners).unwrap();
    let idle = fixture.session_of_owner(&idle_owner);
    fixture.mark_working(&fixture.session_of_owner(&busy_owner));
    let graph: String = sql
        .query_row(
            "INSERT INTO graph_processes(lf_process_id,graph,compiled)
             SELECT id,'fixture','{}' FROM processes WHERE parent_lf_process_id=?1 LIMIT 1
             RETURNING lf_process_id",
            [&graph_owner],
            |r| r.get(0),
        )
        .unwrap();
    // This owner is outside the Graph's ancestry: only its Session's current
    // capture makes it a step, so membership must hold it independently.
    let step: String = sql
        .query_row(
            "UPDATE processes SET parent_lf_process_id=?1 WHERE id=(
             SELECT captured.lf_process_id FROM agent_sessions s
             JOIN session_events captured ON captured.seq=s.current_capture
             WHERE s.transport_lf_process_id=?2) RETURNING id",
            [&graph, &step_owner],
            |r| r.get(0),
        )
        .unwrap();
    sql.execute(
        "INSERT INTO graph_process_steps(graph_lf_process_id,lf_process_id,node,iterations)
         VALUES(?1,?2,0,'[]')",
        [&graph, &step],
    )
    .unwrap();
    fixture.mark_stale_working(&fixture.session_of_owner(&stale_owner));
    // Closing the window: every owner but the viewed one loses its viewers.
    let viewed = viewers.remove(1);
    drop(viewers);
    fixture.ends(&idle_owner);
    fixture.ends(&stale_owner);
    // Several judgments later, everything else still runs.
    std::thread::sleep(Duration::from_secs(4));
    for owner in [
        &viewed_owner,
        &busy_owner,
        &graph_owner,
        &step_owner,
        &talking_owner,
    ] {
        assert!(fixture.running(owner), "{owner} ended");
    }
    // The idle owner left what its terminal showed when it ended.
    let screen = fixture.last_screen(&idle);
    assert!(
        screen.text().unwrap().contains("ready for"),
        "{:?}",
        screen.text().unwrap()
    );
    // Its last viewer leaving lets the viewed one end too; its Graph
    // finishing releases both its ancestor owner and its step's owner;
    // falling quiet lets the talking one end.
    drop(viewed);
    sql.execute("UPDATE processes SET completed_at=1 WHERE id=?1", [&graph])
        .unwrap();
    std::fs::write(fixture.home.path().join("hush"), "").unwrap();
    fixture.ends(&viewed_owner);
    fixture.ends(&graph_owner);
    fixture.ends(&step_owner);
    fixture.ends(&talking_owner);
    assert!(fixture.running(&busy_owner), "busy owner ended mid-turn");
    // `lf resume` reopens the ended conversation from its saved history.
    let mut reopen = fixture.serve(
        true,
        &[env!("CARGO_BIN_EXE_lf"), "session", "resume", &idle],
    );
    reopen.env("PATH", &path);
    let reopened = fixture.owner(reopen);
    let _viewer = fixture.viewer(&reopened, false);
    let line = fixture.launches(8).pop().unwrap();
    assert!(
        line.contains(" --resume ") && session_of(&sql, &line) == idle,
        "reopened without history: {line}"
    );
    for owner in [&busy_owner, &reopened] {
        fixture.stop(owner);
    }
}

/// A retained editor accepts corrections after both deadlines; plain submissions settle.
#[test]
fn native_drafts_remain_editable_and_acknowledged_prompts_can_end() {
    let fixture = Fixture::new();
    std::fs::write(
        fixture.home.path().join("config.yaml"),
        "session:\n  end_detached_after: 2s\n",
    )
    .unwrap();
    let path = fixture.provider(
        r#"#!/usr/bin/env python3
import json, os, pathlib, subprocess, sys, tty
if sys.argv[1:] == ["--version"]:
    print("2.1.317")
    sys.exit(0)
home = pathlib.Path(os.environ["HOME"])
name = os.environ["EDITOR_CASE"]
settings = json.loads(pathlib.Path(sys.argv[sys.argv.index("--settings") + 1]).read_text())
def hook(event, **extra):
    command = settings["hooks"][event][0]["hooks"][0]["command"]
    payload = dict(session_id=name, hook_event_name=event, **extra)
    subprocess.run(command, shell=True, input=json.dumps(payload).encode(), stdout=subprocess.DEVNULL, check=True)
hook("SessionStart", source="startup")
hook("UserPromptSubmit", prompt_id="seed", prompt="initial")
hook("Stop", prompt_id="seed", background_tasks=[], session_crons=[])
terminal = os.open("/dev/tty", os.O_RDWR)
tty.setraw(terminal)
(home / (name + "-ready")).write_text("ready")
os.write(1, b"EDITOR_READY\r\n")
draft = bytearray()
while True:
    key = os.read(terminal, 1)
    if key == b'\x04': break
    if key == b'\x7f':
        draft = draft[:-1]
    elif key == b'\r':
        prompt = draft.decode()
        hook("UserPromptSubmit", prompt_id="typed", prompt=prompt)
        hook("Stop", prompt_id="typed", background_tasks=[], session_crons=[])
        (home / (name + "-submitted")).write_text(prompt)
        draft.clear()
    else:
        draft.extend(key)
    os.write(1, b'\r\nEDITOR:' + draft + b'\r\n')
"#,
    );
    let removable = fixture.home.path().join("bin/lf-editor");
    std::fs::copy(env!("CARGO_BIN_EXE_lf"), &removable).unwrap();
    let mut owners = Vec::new();
    for (name, binary) in [
        ("draft", removable.as_path()),
        ("submitted", Path::new(env!("CARGO_BIN_EXE_lf"))),
    ] {
        let mut command = fixture.native(binary, &path, "initial");
        command.env("EDITOR_CASE", name);
        let owner = fixture.owner(command);
        let mut viewer = fixture.viewer(&owner, true);
        until_out(&mut viewer, "EDITOR_READY");
        viewer.input(if name == "draft" {
            "draft worlX"
        } else {
            "simple\r"
        });
        if name == "submitted" {
            fixture.wait_file("submitted-submitted");
        } else {
            until_out(&mut viewer, "EDITOR:draft worlX");
        }
        drop(viewer);
        owners.push(owner);
    }
    fixture.ends(&owners[1]);
    assert!(
        fixture.running(&owners[0]),
        "an unsent draft ended after its grace"
    );
    std::fs::remove_file(&removable).unwrap();
    std::thread::sleep(Duration::from_secs(4));
    assert!(
        fixture.running(&owners[0]),
        "an unsent draft ended with its executable"
    );
    let mut viewer = fixture.viewer(&owners[0], true);
    viewer.input("\x7fd\r");
    fixture.wait_file("draft-submitted");
    assert_eq!(
        std::fs::read_to_string(fixture.home.path().join("draft-submitted")).unwrap(),
        "draft world"
    );
    // An edit is deliberately not reconstructed: its editor remains owned.
    std::thread::sleep(Duration::from_secs(4));
    assert!(fixture.running(&owners[0]));
    viewer.input("\x04");
    viewer.finish();
    fixture.ends(&owners[0]);
}

/// Native start/stop callbacks preserve silent work across both deadlines.
#[test]
fn silent_native_turns_survive_until_settlement() {
    native_work_survives_until_settlement(false);
}

#[test]
fn native_background_watches_survive_until_settlement() {
    native_work_survives_until_settlement(true);
}

fn native_work_survives_until_settlement(background: bool) {
    let fixture = Fixture::new();
    std::fs::write(
        fixture.home.path().join("config.yaml"),
        "session:\n  end_detached_after: 2s\n",
    )
    .unwrap();
    let path = fixture.provider(
        r#"#!/usr/bin/env python3
import json, os, pathlib, subprocess, sys, time
if sys.argv[1:] == ["--version"]:
    print("2.1.302")
    sys.exit(0)
home = pathlib.Path(os.environ["HOME"])
name = os.environ["SILENT_CASE"]
background = os.environ["BACKGROUND_WATCH"] == "true"
settings = json.loads(pathlib.Path(sys.argv[sys.argv.index("--settings") + 1]).read_text())
def hook(event, **extra):
    command = settings["hooks"][event][0]["hooks"][0]["command"]
    payload = dict(session_id=name, hook_event_name=event, **extra)
    subprocess.run(command, shell=True, input=json.dumps(payload).encode(), stdout=subprocess.DEVNULL, check=True)
hook("SessionStart", source="startup")
# A second prompt must report its start even with no new steering to deliver.
hook("UserPromptSubmit")
hook("Stop")
hook("UserPromptSubmit")
# Child-agent completion must not settle the main conversation's turn.
hook("Stop", agent_id="child")
if background:
    hook("Stop", background_tasks=[dict(id="watch", type="shell", status="running")], session_crons=[])
    # Missing registry evidence and child completion cannot settle that watch.
    hook("Stop")
    hook("Stop", agent_id="child", background_tasks=[], session_crons=[])
(home / (name + "-started")).write_text("started")
while not (home / (name + "-settle")).exists():
    time.sleep(0.05)
if background:
    # The shell watch finished, but a scheduled wakeup still owns work.
    hook("Stop", background_tasks=[], session_crons=[dict(id="poll", schedule="* * * * *", recurring=True)])
    (home / (name + "-scheduled")).write_text("scheduled")
    while not (home / (name + "-unschedule")).exists():
        time.sleep(0.05)
hook("Stop", background_tasks=[], session_crons=[])
(home / (name + "-settled")).write_text("settled")
while True:
    time.sleep(1)
"#,
    );
    let removable = fixture.home.path().join("bin/lf-removable");
    std::fs::copy(env!("CARGO_BIN_EXE_lf"), &removable).unwrap();
    let mut owners = Vec::new();
    for (name, executable) in [
        ("idle", Path::new(env!("CARGO_BIN_EXE_lf"))),
        ("gone", removable.as_path()),
    ] {
        let mut command = fixture.native(executable, &path, "silent turn");
        command
            .env("BACKGROUND_WATCH", background.to_string())
            .env("SILENT_CASE", name);
        let owner = fixture.owner(command);
        let viewer = fixture.viewer(&owner, false);
        fixture.wait_file(&format!("{name}-started"));
        drop(viewer);
        owners.push(owner);
    }
    std::thread::sleep(Duration::from_secs(6));
    for owner in &owners {
        assert!(
            fixture.running(owner),
            "silent native turn ended after its grace"
        );
    }
    // A viewed idle terminal cannot settle a quiet supervisor's native turn.
    let _viewed = fixture.viewer(&owners[1], false);
    fixture
        .sql()
        .execute(
            r#"UPDATE session_activity SET program_status='{"records":[{"state":"idle"}]}'
           WHERE session_id=(SELECT id FROM agent_sessions WHERE transport_lf_process_id=?1)"#,
            [&owners[1]],
        )
        .unwrap();
    std::fs::remove_file(&removable).unwrap();
    std::thread::sleep(Duration::from_secs(4));
    for owner in &owners {
        assert!(
            fixture.running(owner),
            "silent native turn ended with its executable"
        );
    }
    // A live Graph continues to hold a supervisor after its native turn yields.
    let sql = fixture.sql();
    let graphs = if background {
        Vec::new()
    } else {
        owners
            .iter()
            .map(|owner| {
                sql.query_row(
                    "INSERT INTO graph_processes(lf_process_id,graph,compiled)
                 SELECT id,'supervised','{}' FROM processes WHERE parent_lf_process_id=?1 LIMIT 1
                 RETURNING lf_process_id",
                    [owner],
                    |row| row.get::<_, String>(0),
                )
                .unwrap()
            })
            .collect::<Vec<_>>()
    };
    for (name, owner) in ["idle", "gone"].iter().zip(&owners) {
        std::fs::write(fixture.home.path().join(format!("{name}-settle")), "").unwrap();
        if background {
            fixture.wait_file(&format!("{name}-scheduled"));
            std::thread::sleep(Duration::from_secs(4));
            assert!(fixture.running(owner), "scheduled native work ended");
            std::fs::write(fixture.home.path().join(format!("{name}-unschedule")), "").unwrap();
        }
        fixture.wait_file(&format!("{name}-settled"));
    }
    if !background {
        std::thread::sleep(Duration::from_secs(6));
        for owner in &owners {
            assert!(fixture.running(owner), "quiet Graph supervisor ended");
        }
        for graph in graphs {
            sql.execute("UPDATE processes SET completed_at=1 WHERE id=?1", [graph])
                .unwrap();
        }
    }
    for owner in &owners {
        fixture.ends(owner);
    }
}

/// A conversation whose lf executable is deleted, rebuilt or superseded ends
/// once it waits for a person, even while viewed, and a terminal showing it
/// follows through its lf entry into the reopened conversation with its
/// history: a deleted build through the entry that still dispatches, a rebuilt
/// build through its own path, and one launched through the installed entry
/// onto the build `lf install` selected. A turn in progress finishes first:
/// `lf resume` on that owner watches it rather than cutting it, and follows
/// when it ends.
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
    let talking = build("lf-talking");
    let entry = bin.join("lf-entry");
    std::os::unix::fs::symlink(bin.join("lf-installed"), &entry).unwrap();
    let removed = build("lf-idle");
    let replaced = build("lf-replaced");
    // (executable, the entry it was launched through)
    let launched = [
        (removed.clone(), entry.clone()),
        (replaced.clone(), replaced.clone()),
        (installed.clone(), installed.clone()),
        (installed.clone(), installed.clone()),
    ];
    let mut owners = Vec::new();
    let mut viewers = Vec::new();
    for (executable, entry) in &launched {
        let entry = entry.to_str().unwrap();
        let mut owner = fixture.command_from(executable, &["session", "serve", "--terminal"]);
        owner
            .args([
                "--",
                entry,
                "--interactive",
                "--agent",
                "claude",
                ":",
                "hello",
            ])
            .env("LF_BIN", entry)
            .env("PATH", &path);
        let owner = fixture.owner(owner);
        viewers.push(fixture.viewer(&owner, false));
        owners.push(owner);
    }
    let mut talking_command = fixture.command_from(&talking, &["session", "serve", "--terminal"]);
    talking_command
        .args([
            "--",
            entry.to_str().unwrap(),
            "--interactive",
            "--agent",
            "claude",
            ":",
            "talking",
        ])
        .env("LF_BIN", &entry)
        .env("PATH", &path);
    let talking_owner = fixture.owner(talking_command);
    fixture.launches(owners.len() + 1);
    let sessions: Vec<String> = owners
        .iter()
        .map(|owner| fixture.session_of_owner(owner))
        .collect();
    let busy = sessions[3].clone();
    fixture.mark_working(&busy);
    let resume = |entry: &Path, session: &str| {
        let mut command = fixture.command_from(entry, &["session", "resume", session]);
        command.env("LF_BIN", entry).env("PATH", &path);
        Terminal::spawn(command)
    };
    let mut terminals: Vec<Terminal> = (0..3)
        .map(|index| resume(&launched[index].1, &sessions[index]))
        .collect();
    for owner in &owners[..3] {
        fixture.wait_controlled(owner);
    }

    // A worktree removed, a build rebuilt in place, `lf install` selecting
    // another build.
    std::fs::remove_file(&removed).unwrap();
    std::fs::remove_file(&talking).unwrap();
    let rebuilt = bin.join("lf-rebuilt");
    other_build(&rebuilt);
    std::fs::rename(&rebuilt, &replaced).unwrap();
    let next = bin.join("lf-next");
    other_build(&next);
    let swapped = bin.join("lf-swapped");
    std::os::unix::fs::symlink(&next, &swapped).unwrap();
    std::fs::rename(&swapped, &installed).unwrap();

    for owner in &owners[..3] {
        fixture.ends(owner);
    }
    // Every viewer learns why and the command that reopens the conversation
    // through its entry.
    let moved = |index: usize, change: &str| {
        (
            Some(format!(
                "This Session's lf was {change}; reopening it as `lf resume {}`",
                sessions[index]
            )),
            Some(vec![
                launched[index].1.to_str().unwrap().to_owned(),
                "session".to_owned(),
                "resume".to_owned(),
                sessions[index].clone(),
            ]),
        )
    };
    for (index, change) in [(0, "removed"), (1, "replaced"), (2, "superseded")] {
        assert_eq!(viewers[index].moves(), [moved(index, change)], "{change}");
    }
    // Each terminal ran that command: the conversation resumed with its
    // history on the build its entry runs now.
    let lines = fixture.launches(8);
    let mut reopened_sessions: Vec<String> = lines[5..]
        .iter()
        .map(|line| {
            assert!(
                line.contains(" --resume "),
                "reopened without history: {line}"
            );
            session_of(&fixture.sql(), line)
        })
        .collect();
    reopened_sessions.sort();
    let mut expected = sessions[..3].to_vec();
    expected.sort();
    assert_eq!(reopened_sessions, expected);
    let mut reopened = Vec::new();
    for (index, build) in [(0, "lf-installed"), (1, "lf-replaced"), (2, "lf-next")] {
        let owner = fixture.owner_of(&sessions[index]);
        assert_ne!(owner, owners[index]);
        let executable: String = fixture
            .sql()
            .query_row(
                "SELECT json_extract(command,'$[0]') FROM processes WHERE id=?1",
                [&owner],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(
            std::fs::canonicalize(&executable).unwrap(),
            std::fs::canonicalize(bin.join(build)).unwrap(),
            "reopened on {executable}"
        );
        terminals[index].assert_viewing(&format!("terminal {index}"));
        reopened.push(owner);
    }
    // Opening the busy owner's conversation from the installed lf attaches to
    // it rather than cutting its turn, and several judgments later it still runs.
    let watching = resume(&installed, &busy);
    fixture.wait_controlled(&owners[3]);
    std::thread::sleep(Duration::from_secs(4));
    assert!(fixture.running(&owners[3]), "busy owner ended mid-turn");
    assert_eq!(
        fixture.launches(8).len(),
        8,
        "the busy conversation relaunched"
    );
    assert!(
        fixture.running(&talking_owner),
        "native turn ended while drawing"
    );
    // Once output stops, a viewer cannot keep a vanished build.
    let _talking_viewer = fixture.viewer(&talking_owner, false);
    std::fs::write(fixture.home.path().join("hush"), "").unwrap();
    fixture.ends(&talking_owner);
    // Its turn settling lets it end; the watching terminal then follows.
    fixture
        .sql()
        .execute("DELETE FROM session_activity WHERE session_id=?1", [&busy])
        .unwrap();
    fixture.ends(&owners[3]);
    assert_eq!(viewers[3].moves(), [moved(3, "superseded")]);
    let line = fixture.launches(9).pop().unwrap();
    assert!(
        line.contains(" --resume ") && session_of(&fixture.sql(), &line) == busy,
        "reopened without history: {line}"
    );
    let busy_reopened = fixture.owner_of(&busy);
    assert_ne!(busy_reopened, owners[3]);
    terminals.push(watching);
    // Every terminal is still in its reopened conversation.
    std::thread::sleep(Duration::from_secs(1));
    for (index, terminal) in terminals.iter_mut().enumerate() {
        terminal.assert_viewing(&format!("terminal {index}"));
    }
    drop(viewers);
    for terminal in terminals {
        terminal.stop();
    }
    for owner in reopened.iter().chain([&busy_reopened]) {
        fixture.stop(owner);
    }
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

/// An `lf resume` typed at a terminal: its screen is a pseudo-terminal, read
/// in the background so the viewer never blocks on it; its stdin stays open.
struct Terminal {
    child: Child,
    screen: std::sync::Arc<std::sync::Mutex<Vec<u8>>>,
}
impl Terminal {
    fn spawn(command: Command) -> Self {
        Self::spawn_with_placement(command, None)
    }

    fn spawn_with_placement(mut command: Command, placement: Option<bool>) -> Self {
        let mut master = -1;
        let mut slave = -1;
        // SAFETY: valid output pointers; null selects default PTY settings.
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
        // SAFETY: openpty returned two independently owned descriptors.
        let mut master = unsafe { std::fs::File::from_raw_fd(master) };
        // SAFETY: the fresh slave descriptor is owned only by this File.
        let slave = unsafe { std::fs::File::from_raw_fd(slave) };
        if let Some(valid) = placement {
            // SAFETY: slave is a live PTY; ttyname returns a terminated name.
            let tty = unsafe { std::ffi::CStr::from_ptr(libc::ttyname(slave.as_raw_fd())) }
                .to_str()
                .unwrap()
                .to_owned();
            command
                .env("LF_TERMINAL_ID", "desktop-shell")
                .env("LF_TERMINAL_TTY", if valid { &tty } else { "/dev/stale" })
                .stdin(slave.try_clone().unwrap());
        } else {
            command.stdin(Stdio::piped());
        }
        command
            .stdout(Stdio::from(slave.try_clone().unwrap()))
            .stderr(Stdio::from(slave));
        let child = command.spawn().unwrap();
        drop(command);
        let screen = std::sync::Arc::new(std::sync::Mutex::new(Vec::new()));
        let written = screen.clone();
        std::thread::spawn(move || {
            let mut buffer = [0u8; 4096];
            // Linux reports EIO once the last slave closes; keep what came.
            while let Ok(read @ 1..) = master.read(&mut buffer) {
                written.lock().unwrap().extend_from_slice(&buffer[..read]);
            }
        });
        Self { child, screen }
    }
    /// Panics with the screen when the viewer has left the conversation.
    fn assert_viewing(&mut self, what: &str) {
        if let Some(status) = self.child.try_wait().unwrap() {
            panic!(
                "{what} left the conversation ({status}):\n{}",
                String::from_utf8_lossy(&self.screen.lock().unwrap())
            );
        }
    }
    fn stop(mut self) {
        self.child.kill().unwrap();
        self.child.wait().unwrap();
    }
}

/// A Session whose owner runs another lf build than this test binary, with
/// that build as its entry: `(owner, session)`.
fn owner_on_another_build(fixture: &Fixture, path: &str) -> (String, String) {
    let other = fixture.home.path().join("bin/lf-other");
    other_build(&other);
    let lf = other.to_str().unwrap();
    let mut owner = fixture.command_from(&other, &["session", "serve", "--terminal"]);
    owner
        .args(["--", lf, "--interactive", "--agent", "claude", ":", "hello"])
        .env("LF_BIN", lf)
        .env("PATH", path);
    let owner = fixture.owner(owner);
    fixture.launches(1);
    let session = fixture.session_of_owner(&owner);
    (owner, session)
}

/// `lf resume` on `session`, held by `previous` on another build: the owner
/// ends and the conversation resumes here from its saved history. Returns
/// what the viewer told the person.
fn takes_over(fixture: &Fixture, path: &str, previous: &str, session: &str) -> String {
    let mut viewer = fixture
        .command(&["session", "resume", session])
        .env("PATH", path)
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
    fixture.ends(previous);
    let current = fixture.owner_of(session);
    assert_ne!(current, previous);
    assert!(
        viewer.try_wait().unwrap().is_none(),
        "viewer left the conversation"
    );
    viewer.kill().unwrap();
    let output = viewer.wait_with_output().unwrap();
    fixture.stop(&current);
    String::from_utf8_lossy(&output.stderr).into_owned()
}

/// `lf resume` on a conversation whose owner runs another lf build never
/// negotiates with it: the owner ends and the conversation resumes here from
/// its saved history, as it does for the owners `lf install` leaves behind.
#[test]
fn resume_takes_over_an_owner_from_another_build() {
    let fixture = Fixture::new();
    let path = fixture.waiting_provider();
    let (previous, session) = owner_on_another_build(&fixture, &path);
    let stderr = takes_over(&fixture, &path, &previous, &session);
    assert!(
        stderr.contains("Moved") && stderr.contains("continuing on "),
        "{stderr}"
    );
    assert!(!stderr.contains("cut off"), "{stderr}");
}

/// A superseded owner mid-turn that does not answer its socket cannot be
/// watched to the turn's end, so `lf resume` ends it, reopens the conversation
/// from its saved history and says the turn was cut.
#[test]
fn resume_ends_a_busy_superseded_owner_that_never_answers() {
    let fixture = Fixture::new();
    let path = fixture.waiting_provider();
    let (previous, session) = owner_on_another_build(&fixture, &path);
    fixture.mark_working(&session);
    let endpoint = fixture.endpoint(&previous);
    std::fs::rename(&endpoint, endpoint.with_extension("retained")).unwrap();
    let listener = UnixListener::bind(&endpoint).unwrap();
    std::thread::spawn(move || {
        let held: Vec<UnixStream> = listener.incoming().map_while(Result::ok).collect();
        drop(held);
    });
    let stderr = takes_over(&fixture, &path, &previous, &session);
    assert!(
        stderr.contains("Moved") && stderr.contains("cut off"),
        "{stderr}"
    );
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
    desktop.socket.shutdown(std::net::Shutdown::Both).unwrap();
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
    headless_run(&["graph", "proof"], false);
}

#[test]
fn resume_refuses_a_graph_step_and_leaves_it_running() {
    headless_run(&["graph", "proof"], true);
}

#[test]
fn a_headless_run_outlives_the_detached_limit() {
    headless_run(&["skill", "probe", "--message", "go"], false);
}

fn headless_run(command: &[&str], resume: bool) {
    let fixture = Fixture::new();
    std::fs::write(
        fixture.home.path().join("config.yaml"),
        "session:\n  end_detached_after: 2s\n",
    )
    .unwrap();
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
        .command(&[&["-b", "--agent", "opencode"], command].concat())
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
    let outcome = |id: &str| -> Option<String> {
        sql.query_row("SELECT outcome FROM processes WHERE id=?1", [id], |r| {
            r.get(0)
        })
        .unwrap()
    };
    let settles = |id: &str, what: &str| {
        let deadline = Instant::now() + Duration::from_secs(30);
        loop {
            if let Some(outcome) = outcome(id) {
                assert_eq!(outcome, "succeeded");
                return;
            }
            assert!(Instant::now() < deadline, "{what} did not settle");
            std::thread::sleep(Duration::from_millis(20));
        }
    };
    assert_eq!(outcome(&step), None, "viewer death settled the execution");
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
    // Unviewed well past the limit, the Run keeps its turn.
    std::thread::sleep(Duration::from_secs(4));
    assert_eq!(outcome(&step), None, "the detached limit ended a Run");
    std::fs::remove_file(fixture.home.path().join("hold-turn")).unwrap();
    settles(&step, "step");
    let graph: Option<String> = sql
        .query_row(
            "SELECT graph_lf_process_id FROM graph_process_steps WHERE lf_process_id=?1",
            [&step],
            |r| r.get(0),
        )
        .optional()
        .unwrap();
    if let Some(graph) = &graph {
        settles(graph, "Graph");
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

/// A shell that echoes each line it reads, so viewers can make it print on demand.
const ECHO_LINES: &str = r#"
stty -echo -onlcr
printf 'drawn\r\n'
printf 1 > "$HOME/drawn"
while read -r line; do
    case "$line" in
        done) exit 0 ;;
        size) stty size ;;
        burst) dd if=/dev/zero bs=65536 count=32 2>/dev/null ;;
        fake) printf '\033_lf1;deadbeef;{"seq":9,"type":"resize","rows":1,"cols":1}\033\\' ;;
        *) printf 'line %s\r\n' "$line" ;;
    esac
done
"#;

#[track_caller]
fn until_out(viewer: &mut Viewer, text: &str) -> Vec<Item> {
    until(viewer, |items| {
        String::from_utf8_lossy(&out(items)).contains(text)
    })
}

/// Items up to the first that satisfies `done` over everything so far.
#[track_caller]
fn until(viewer: &mut Viewer, done: impl Fn(&[Item]) -> bool) -> Vec<Item> {
    let mut items = Vec::new();
    loop {
        let item = viewer
            .next()
            .unwrap_or_else(|| panic!("owner closed first: {items:?}"));
        items.push(item);
        if done(&items) {
            return items;
        }
    }
}

/// Items up to the owner event naming `id`.
#[track_caller]
fn until_owner(viewer: &mut Viewer, id: &str) -> Vec<Item> {
    until(viewer, |items| {
        matches!(items.last(), Some(Item::Event(event))
            if event["type"] == "owner" && event["viewer"] == id)
    })
}

#[test]
fn every_viewer_starts_with_the_snapshot_and_shares_live_output() {
    let fixture = Fixture::new();
    let owner = fixture.start(&["/bin/sh", "-c", ECHO_LINES]);
    fixture.wait_file("drawn");
    std::thread::sleep(Duration::from_millis(100));
    let mut first = fixture.viewer(&owner, true);
    let mut second = fixture.viewer(&owner, false);
    for viewer in [&mut first, &mut second] {
        let items = viewer.until_event("owner");
        let Item::Snapshot(snapshot) = &items[0] else {
            panic!("stream does not begin with the snapshot: {items:?}");
        };
        assert!(String::from_utf8_lossy(snapshot).contains("drawn"));
        assert_eq!(events(&items, "resize")[0]["rows"], 24);
    }
    first.input("hello\n");
    for viewer in [&mut first, &mut second] {
        let items = until_out(viewer, "line hello");
        assert!(items.iter().all(|item| !matches!(item, Item::Snapshot(_))));
    }
    first.input("done\n");
    first.finish();
    second.finish();
    assert!(first.next().is_none());
    assert!(second.next().is_none());
}

#[test]
fn a_dropped_viewer_resumes_from_its_byte_count_without_a_snapshot() {
    let fixture = Fixture::new();
    let owner = fixture.start(&["/bin/sh", "-c", ECHO_LINES]);
    fixture.wait_file("drawn");
    let mut first = fixture.viewer(&owner, true);
    let dark = json!({"background":"#111111","foreground":"#eeeeee"});
    let light = json!({"background":"#eeeeee","foreground":"#111111"});
    first.until_event("owner");
    first.request(json!({"kind":"theme","theme":dark}));
    let initial = first.until_event("theme");
    let last_seq = events(&initial, "theme")[0]["seq"].as_u64().unwrap();
    first.input("one\n");
    until_out(&mut first, "line one\r\n");
    let received = first.received;
    let reconnect = first.reconnect.clone();
    // The connection is lost mid-stream: no leave, no close from the owner.
    first.socket.shutdown(std::net::Shutdown::Both).unwrap();
    drop(first);
    let mut second = fixture.viewer(&owner, true);
    second.request(json!({"kind":"theme","theme":light}));
    second.until_event("theme");
    second.input("two\n");
    until_out(&mut second, "line two\r\n");
    // Three bytes short of what it had: the stream resumes with exactly them.
    let mut resumed = fixture.connect(&owner);
    resumed.request(
        json!({"kind":"resume","reconnect":reconnect,"received":received - 3,
        "control":false,"own":null,"rows":24,"cols":80}),
    );
    let reply = resumed.until_reply();
    assert_eq!(reply["kind"], "attached", "{reply}");
    resumed.accept(&reply);
    let items = until_out(&mut resumed, "line two\r\n");
    assert!(
        items.iter().all(|item| !matches!(item, Item::Snapshot(_))),
        "{items:?}"
    );
    assert_eq!(out(&items), b"e\r\nline two\r\n", "{items:?}");
    assert_eq!(events(&items, "theme").len(), 1, "{items:?}");
    assert_eq!(events(&items, "theme")[0]["theme"], light);
    let mut seq = last_seq;
    for item in &items {
        if let Item::Event(event) = item {
            seq += 1;
            assert_eq!(event["seq"], seq);
        }
    }
    assert_eq!(reply["seq"], seq);
    second.input("three\n");
    for viewer in [&mut resumed, &mut second] {
        let items = until_out(viewer, "line three\r\n");
        assert!(events(&items, "theme").is_empty(), "{items:?}");
    }
    // Resuming did not restore the old colors; explicitly taking ownership does.
    resumed.request(json!({"kind":"own","mode":"take"}));
    let resumed_id = resumed.id.clone();
    for viewer in [&mut resumed, &mut second] {
        let items = until_owner(viewer, &resumed_id);
        assert_eq!(events(&items, "theme").last().unwrap()["theme"], dark);
    }
    resumed.request(json!({"kind":"controlled"}));
    let reply = resumed.until_reply();
    assert_eq!(reply["seq"], seq + 3); // resize, theme, owner
    second.input("done\n");
    second.finish();
    resumed.finish();
}

#[test]
fn an_immediate_resume_can_reclaim_input_and_resize() {
    let fixture = Fixture::new();
    let owner = fixture.start(&["/bin/sh", "-c", ECHO_LINES]);
    fixture.wait_file("drawn");
    let mut first = fixture.viewer(&owner, true);
    first.until_event("owner");
    let mut resumed = fixture.connect(&owner);
    // Deliver EOF and resume in one owner pass, without a scheduling sleep.
    let paused = fixture.pause(&owner);
    first.socket.shutdown(std::net::Shutdown::Both).unwrap();
    resumed.request(json!({"kind":"resume","reconnect":first.reconnect,
        "received":first.received,"control":true,"own":"take","rows":35,"cols":110}));
    drop(paused);
    let reply = resumed.until_reply();
    if reply["token"].is_null() {
        fixture.stop(&owner);
        let _ = resumed.socket.read_to_end(&mut Vec::new());
        panic!("resume did not regain input: {reply}");
    }
    resumed.accept(&reply);
    let items = resumed.until_event("owner");
    assert_eq!(events(&items, "resize")[0]["rows"], 35);
    resumed.input("size\n");
    until_out(&mut resumed, "35 110");
    resumed.input("done\n");
    resumed.finish();
    assert!(resumed.next().is_none());
}

#[test]
fn a_resume_past_the_spool_is_refused_and_a_fresh_attach_repaints() {
    let fixture = Fixture::new();
    let owner = fixture.start(&["/bin/sh", "-c", ECHO_LINES]);
    fixture.wait_file("drawn");
    let mut first = fixture.viewer(&owner, true);
    first.until_event("owner");
    let received = first.received;
    let reconnect = first.reconnect.clone();
    first.socket.shutdown(std::net::Shutdown::Both).unwrap();
    drop(first);
    let mut second = fixture.viewer(&owner, true);
    second.input("burst\n");
    let mut seen = 0;
    while seen < 2 * 1024 * 1024 {
        if let Item::Out(bytes) = second.next().unwrap() {
            seen += bytes.len();
        }
    }
    let mut back = fixture.connect(&owner);
    back.request(
        json!({"kind":"resume","reconnect":reconnect,"received":received,
        "control":false,"own":null,"rows":24,"cols":80}),
    );
    assert_eq!(back.until_reply()["kind"], "reconnect_required");
    back.request(
        json!({"kind":"attach","control":false,"replace":true,"own":null,"rows":24,"cols":80}),
    );
    let attached = back.until_reply();
    assert_eq!(attached["kind"], "attached");
    back.accept(&attached);
    assert!(matches!(back.next(), Some(Item::Snapshot(_))));
    second.input("done\n");
    second.finish();
}

#[test]
fn resize_reaches_every_viewer_as_its_own_event_and_program_bytes_do_not() {
    let fixture = Fixture::new();
    let owner = fixture.start(&["/bin/sh", "-c", ECHO_LINES]);
    fixture.wait_file("drawn");
    let mut sizer = fixture.viewer(&owner, true);
    let mut observer = fixture.viewer(&owner, false);
    observer.until_event("owner");
    sizer.request(json!({"kind":"resize","rows":30,"cols":100}));
    let items = observer.until_event("resize");
    let resize = events(&items, "resize");
    assert_eq!(resize.last().unwrap()["cols"], 100);
    sizer.input("size\n");
    until_out(&mut observer, "30 100");
    sizer.input("fake\n");
    let items = until_out(&mut observer, "deadbeef");
    assert!(events(&items, "resize").is_empty(), "{items:?}");
    // Event sequence numbers are gapless per viewer and replies carry the latest.
    sizer.input("size\n");
    let items = until_out(&mut sizer, "30 100");
    let all: Vec<u64> = items
        .iter()
        .filter_map(|item| match item {
            Item::Event(event) => event["seq"].as_u64(),
            _ => None,
        })
        .collect();
    assert!(all.windows(2).all(|pair| pair[1] == pair[0] + 1), "{all:?}");
    sizer.request(json!({"kind":"controlled"}));
    let reply = sizer.until_reply();
    assert_eq!(reply["kind"], "controlled");
    assert!(reply["seq"].as_u64().unwrap() >= *all.last().unwrap());
    sizer.input("done\n");
    sizer.finish();
}

#[test]
fn ownership_queues_behind_a_taker_and_tells_the_loser_who_took_it() {
    let fixture = Fixture::new();
    let owner = fixture.start(&["/bin/sh", "-c", ECHO_LINES]);
    fixture.wait_file("drawn");
    let mut typist = fixture.viewer(&owner, true);
    let dark = json!({"background":"#111111","foreground":"#eeeeee"});
    let light = json!({"background":"#eeeeee","foreground":"#111111"});
    typist.request(json!({"kind":"theme","theme":dark}));
    typist.until_event("theme");
    let mut waiting = fixture.attach(
        &owner,
        json!({"kind":"attach","control":false,"replace":true,"own":"request","rows":30,"cols":90,"name":"pane 2"}),
    );
    let items = waiting.until_event("owner");
    assert_eq!(events(&items, "theme")[0]["theme"], dark);
    waiting.request(json!({"kind":"theme","theme":light}));
    waiting.request(json!({"kind":"controlled"}));
    waiting.until_reply();
    typist.input("size\n");
    for viewer in [&mut waiting, &mut typist] {
        let items = until_out(viewer, "24 80");
        assert!(events(&items, "theme").is_empty(), "{items:?}");
    }
    let mut taker = fixture.attach(
        &owner,
        json!({"kind":"attach","control":false,"replace":true,"own":"take","rows":40,"cols":120,"name":"tmux %4"}),
    );
    let items = taker.until_event("owner");
    assert!(events(&items, "theme").is_empty(), "{items:?}");
    // An owner without a color report supplies no override, even to new observers.
    let mut observer = fixture.viewer(&owner, false);
    let items = observer.until_event("owner");
    assert!(events(&items, "theme").is_empty(), "{items:?}");
    let items = until_owner(&mut typist, &taker.id);
    assert!(events(&items, "theme").is_empty(), "{items:?}");
    let lost = events(&items, "owner");
    assert!(
        lost.iter().any(|event| event["viewer"] == taker.id
            && event["name"] == "tmux %4"
            && event["held"] == false),
        "{items:?}"
    );
    typist.input("size\n");
    until_out(&mut typist, "40 120");
    // The taker leaves: the queued request supplies size and colors to everyone.
    taker.request(json!({"kind":"leave"}));
    drop(taker);
    let waiting_id = waiting.id.clone();
    let items = until_owner(&mut waiting, &waiting_id);
    assert_eq!(events(&items, "owner").last().unwrap()["held"], true);
    assert_eq!(events(&items, "theme")[0]["theme"], light);
    let seq = events(&items, "owner").last().unwrap()["seq"].clone();
    waiting.request(json!({"kind":"controlled"}));
    assert_eq!(waiting.until_reply()["seq"], seq);
    for viewer in [&mut typist, &mut observer] {
        let items = until_owner(viewer, &waiting_id);
        assert_eq!(events(&items, "theme")[0]["theme"], light);
    }
    let mut fresh = fixture.viewer(&owner, false);
    let items = fresh.until_event("owner");
    assert_eq!(events(&items, "theme")[0]["theme"], light);
    typist.input("size\n");
    until_out(&mut typist, "30 90");
    typist.input("done\n");
    typist.finish();
}

#[test]
fn a_megabyte_burst_costs_within_ten_percent_on_the_wire() {
    let fixture = Fixture::new();
    let owner = fixture.owner(fixture.serve(
        false,
        &[
            "/bin/sh",
            "-c",
            "read -r go; dd if=/dev/zero bs=65536 count=16 2>/dev/null",
        ],
    ));
    let mut viewer = fixture.viewer(&owner, true);
    viewer.input("go\n");
    let items = viewer.until_event("child_exited");
    assert_eq!(out(&items).len(), 1024 * 1024);
    assert!(
        viewer.wire < 1024 * 1024 * 11 / 10,
        "{} bytes on the wire",
        viewer.wire
    );
}
