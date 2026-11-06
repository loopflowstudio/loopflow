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
            // An inherited host workspace would receive the fixture Session's name.
            if ["LF_", "LOOPFLOW_", "CMUX_", "REX_"]
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
        assert!(
            !line.trim().is_empty(),
            "owner exited before acknowledging: {}",
            std::fs::read_to_string(logs.join("transport.log")).unwrap_or_default()
        );
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

    /// A live two-line provider whose Session is placed at `rex:source`.
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
    /// Retire `session` as its provider's exit does, leaving its owner running.
    fn complete(&self, session: &str) {
        self.sql()
            .execute(
                "UPDATE agent_sessions SET completed_at=1 WHERE id=?1",
                [session],
            )
            .unwrap();
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
    /// Whether the owner ever told this viewer it lost input control.
    fn moved(&mut self) -> bool {
        let mut moved = false;
        while let Some(item) = self.next() {
            if let Item::Event(event) = item {
                moved |= event["type"] == "control" && event["held"] == false;
            }
        }
        moved
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
fn viewers_enter_the_execution_directory_and_later_ones_the_selected_directory() {
    let fixture = Fixture::new();
    let path = fixture.waiting_provider();
    let owner =
        fixture.owner(fixture.native(Path::new(env!("CARGO_BIN_EXE_lf")), &path, "directory"));
    let mut launching = fixture.viewer(&owner, true);
    let items = launching.until_event("directory");
    assert_eq!(
        Path::new(events(&items, "directory")[0]["cwd"].as_str().unwrap())
            .canonicalize()
            .unwrap(),
        fixture.repo.path().canonicalize().unwrap()
    );

    let selected = fixture.home.path().join("selected");
    std::fs::create_dir(&selected).unwrap();
    fixture
        .sql()
        .execute(
            "UPDATE agent_sessions SET cwd=?1 WHERE id=?2",
            rusqlite::params![selected.to_str().unwrap(), fixture.session_of_owner(&owner)],
        )
        .unwrap();
    let mut later = fixture.viewer(&owner, false);
    let items = later.until_event("directory");
    assert_eq!(
        Path::new(events(&items, "directory")[0]["cwd"].as_str().unwrap()),
        selected
    );
    let replay = String::from_utf8_lossy(&out(&items)).into_owned();
    assert!(replay.contains("selected/"), "{replay:?}");
    fixture.stop(&owner);
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

}

/// The owner's recorded checkout hold, as Task cleanup reads it.
fn viewer_hold(fixture: &Fixture, owner: &str) -> Option<String> {
    fixture
        .sql()
        .query_row(
            "SELECT viewer_hold FROM processes WHERE id=?1",
            [owner],
            |r| r.get(0),
        )
        .unwrap()
}

#[track_caller]
fn wait_hold(fixture: &Fixture, owner: &str, expected: Option<&str>) {
    let deadline = Instant::now() + Duration::from_secs(30);
    while viewer_hold(fixture, owner).as_deref() != expected {
        assert!(
            Instant::now() < deadline,
            "viewer hold stayed {:?}, expected {expected:?}",
            viewer_hold(fixture, owner)
        );
        std::thread::sleep(Duration::from_millis(50));
    }
}

#[test]
fn session_viewers_retain_until_the_last_leaves_and_disconnect_starts_absence() {
    let fixture = Fixture::new();
    let owner = fixture.start(&[
        "/bin/sh",
        "-c",
        "stty -echo; read -r answer; printf '%s' \"$answer\" > \"$HOME/answer\"",
    ]);
    assert_eq!(viewer_hold(&fixture, &owner), None);
    // A controlling and a passive (hidden, unfocused) viewer both hold.
    let mut controller = fixture.viewer(&owner, true);
    let passive = fixture.viewer(&owner, false);
    wait_hold(&fixture, &owner, Some("viewed"));
    controller.request(json!({"kind":"leave"}));
    drop(controller);
    std::thread::sleep(Duration::from_secs(3));
    assert_eq!(viewer_hold(&fixture, &owner).as_deref(), Some("viewed"));
    // A dropped connection starts absence immediately; replay grace is independent.
    // Resuming the same stream restores viewing without replaying input.
    passive.socket.shutdown(std::net::Shutdown::Both).unwrap();
    wait_hold(&fixture, &owner, Some("absent"));
    let mut resumed = fixture.connect(&owner);
    resumed.request(json!({"kind":"resume","reconnect":passive.reconnect,
        "received":passive.received,"control":false,"own":null,"rows":24,"cols":80}));
    assert_eq!(resumed.until_reply()["kind"], "attached");
    drop(passive);
    resumed.request(json!({"kind":"leave"}));
    drop(resumed);
    wait_hold(&fixture, &owner, Some("absent"));
    // An SSH viewer whose connection is lost may still be running elsewhere.
    let remote = fixture.attach(
        &owner,
        json!({"kind":"attach","control":false,"replace":true,"rows":24,"cols":80,"ssh":true}),
    );
    wait_hold(&fixture, &owner, Some("viewed"));
    remote.socket.shutdown(std::net::Shutdown::Both).unwrap();
    drop(remote);
    let mut finisher = fixture.viewer(&owner, true);
    finisher.input("done\n");
    finisher.finish();
    assert_eq!(fixture.wait_file("answer"), "done");
    // Ending does not create an indefinite hold for the lost viewer.
    wait_hold(&fixture, &owner, Some("absent"));
