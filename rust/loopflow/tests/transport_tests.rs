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
use loopflow_test_support::{MachineServer, TestRepo};
use rusqlite::OptionalExtension;
use serde_json::{json, Value};
use sha2::{Digest, Sha256};

struct Fixture {
    home: tempfile::TempDir,
    repo: TestRepo,
}

impl Drop for Fixture {
    fn drop(&mut self) {
        loopflow_test_support::end_recorded_processes(
            self.home.path(),
            Path::new(env!("CARGO_BIN_EXE_lf")),
        );
    }
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
        // An OSC opcode in an ordinary path is not terminal status.
        let home = tempfile::Builder::new()
            .prefix("lf-7501-")
            .tempdir_in("/tmp")
            .unwrap();
        // Stand-in providers never ask Claude's folder-trust question, so lf must
        // not report one and pre-empt the activity these tests arrange.
        std::fs::write(
            home.path().join(".claude.json"),
            json!({"projects": {"/": {"hasTrustDialogAccepted": true}}}).to_string(),
        )
        .unwrap();
        Self {
            home,
            repo: TestRepo::new(),
        }
    }
    fn command(&self, args: &[&str]) -> Command {
        self.command_from(Path::new(env!("CARGO_BIN_EXE_lf")), args)
    }
    /// Execute the returned program too: preparation may wrap lf in `env`.
    fn prepared_command(&self, record: &Value) -> Command {
        let argv: Vec<&str> = record["open_argv"]
            .as_array()
            .unwrap()
            .iter()
            .map(|arg| arg.as_str().unwrap())
            .collect();
        let (program, args) = argv.split_first().expect("preparation returns a command");
        self.command_from(Path::new(program), args)
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
    /// The Machine server under this home, stopped when dropped.
    fn machine_server(&self) -> MachineServer {
        MachineServer::start(env!("CARGO_BIN_EXE_lf"), self.home.path())
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
        self.connect(id).attach(request)
    }
    fn connect(&self, id: &str) -> Viewer {
        let socket = UnixStream::connect(self.endpoint(id)).unwrap_or_else(|e| {
            panic!(
                "{}: {e}; home {}",
                self.endpoint(id).display(),
                self.home.path().display()
            )
        });
        Viewer::new(socket)
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
    /// The Session `owner` runs, binding a fixture Session to it when the
    /// test started a bare owner: Desktop's panes always name one.
    fn bound_session(&self, owner: &str) -> String {
        let sql = self.sql();
        if let Ok(session) = sql.query_row(
            "SELECT id FROM agent_sessions WHERE transport_lf_process_id=?1",
            [owner],
            |r| r.get(0),
        ) {
            return session;
        }
        let session = format!("session_{}", owner.replace('-', ""));
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
        session
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
    /// Wait until `owner` has recorded whether a viewer is attached.
    fn wait_viewed(&self, owner: &str, viewed: bool) {
        let deadline = Instant::now() + Duration::from_secs(20);
        while self
            .sql()
            .query_row("SELECT viewed FROM processes WHERE id=?1", [owner], |r| {
                r.get::<_, Option<bool>>(0)
            })
            .unwrap()
            != Some(viewed)
        {
            assert!(
                Instant::now() < deadline,
                "{owner} never recorded viewed={viewed}"
            );
            std::thread::sleep(Duration::from_millis(50));
        }
    }
    fn ends(&self, owner: &str) {
        let deadline = Instant::now() + Duration::from_secs(20);
        // The completion receipt can precede the owner's socket cleanup.
        while self.running(owner) || self.endpoint(owner).exists() {
            assert!(
                Instant::now() < deadline,
                "{owner} did not finish cleanup; logs: {:?}",
                std::fs::read_to_string(self.home.path().join("logs/transport.log"))
            );
            std::thread::sleep(Duration::from_millis(50));
        }
    }
    /// Signal an owner the test leaves running.
    fn stop(&self, owner: &str) {
        let pid: Option<i64> = self
            .sql()
            .query_row("SELECT pid FROM processes WHERE id=?1", [owner], |r| {
                r.get(0)
            })
            .optional()
            .unwrap();
        if let Some(pid) = pid {
            // SAFETY: test-owned owner PID recorded by this fixture's store.
            unsafe { libc::kill(pid as libc::pid_t, libc::SIGTERM) };
        }
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
    fn new(socket: UnixStream) -> Self {
        socket
            .set_read_timeout(Some(Duration::from_secs(15)))
            .unwrap();
        Self {
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

    #[track_caller]
    fn attach(mut self, request: Value) -> Self {
        self.request(request);
        let attached = self.until_reply();
        assert_eq!(attached["kind"], "attached", "{attached}");
        self.accept(&attached);
        self
    }

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

/// A Rex server of its own, under a short path: Unix socket paths are limited.
fn isolated_rex() -> (tempfile::TempDir, PathBuf, support::rex::IsolatedRex) {
    let root = tempfile::Builder::new()
        .prefix("rex498-")
        .tempdir_in("/tmp")
        .unwrap();
    let bin = root.path().join("bin");
    std::fs::create_dir(&bin).unwrap();
    let rex =
        support::rex::IsolatedRex::start(root.path(), &bin, Path::new(env!("CARGO_BIN_EXE_lf")));
    (root, bin, rex)
}

#[test]
fn conversation_open_here_confirms_attachment_and_does_not_take_existing_input() {
    let fixture = Fixture::new();
    let path = fixture.provider(
        r#"#!/bin/sh
if [ "$1" = --version ]; then echo '2.1.302'; exit; fi
stty -echo < /dev/tty
printf '%s\n' "$LF_CAPTURE_KEY" >> "$HOME/launches"
printf 'open-ready\r\n'
read -r answer < /dev/tty
printf '%s' "$answer" > "$HOME/answer"
"#,
    );
    let owner =
        fixture.owner(fixture.native(Path::new(env!("CARGO_BIN_EXE_lf")), &path, "original"));
    fixture.launches(1);
    let session = fixture.session_of_owner(&owner);
    let mut command = fixture.command(&["open", "--session", &session]);
    // Direct CLI still attaches here inside Rex; no host request is needed.
    command
        .env("REX_SESSION", "isolated-caller")
        .env("REX_BLOCK", "isolated-block")
        .env("LF_OPEN_RECEIPT", fixture.home.path().join("attached"))
        .env("PATH", &path)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    let mut viewer = command.spawn().unwrap();
    assert_eq!(fixture.wait_file("attached"), "attached");
    // The owner's asynchronous store update can lag its attachment handshake.
    fixture
        .sql()
        .execute("UPDATE processes SET viewed=0 WHERE id=?1", [&owner])
        .unwrap();
    let report = fixture
        .command(&["open", "--session", &session, "--json"])
        .env("PATH", &path)
        .output()
        .unwrap();
    assert!(report.status.success(), "{report:?}");
    let report: Value = serde_json::from_slice(&report.stdout).unwrap();
    assert_eq!(report["receipts"][0]["status"], "elsewhere");
    viewer
        .stdin
        .as_mut()
        .unwrap()
        .write_all(b"still-here\n")
        .unwrap();
    assert_eq!(fixture.wait_file("answer"), "still-here");
    let result = wait_for_viewer(viewer, Duration::from_secs(20));
    assert!(result.status.success(), "{result:?}");
    assert_eq!(fixture.launches(1).len(), 1);
}

#[test]
fn conversation_open_pending_retry_waits_for_the_controlling_viewers_readiness() {
    let fixture = Fixture::new();
    let path = fixture.waiting_provider();
    let owner =
        fixture.owner(fixture.native(Path::new(env!("CARGO_BIN_EXE_lf")), &path, "original"));
    fixture.launches(1);
    let session = fixture.session_of_owner(&owner);
    let mut viewer = fixture.viewer(&owner, true);
    let returned = viewer.until_event("return");
    let ready = events(&returned, "return")[0]["detach"].clone();
    // The opening caller can time out while its viewer has yet to accept Return.
    // Repeating open must neither claim readiness nor create another attachment.
    for _ in 0..2 {
        let report = fixture
            .command(&["open", "--session", &session, "--json"])
            .env("PATH", &path)
            .output()
            .unwrap();
        assert!(report.status.success(), "{report:?}");
        let report: Value = serde_json::from_slice(&report.stdout).unwrap();
        assert_eq!(report["receipts"][0]["status"], "opening");
        assert!(report["receipts"][0]["reason"]
            .as_str()
            .unwrap()
            .contains("Still opening"));
    }
    viewer.request(json!({"kind":"controlled", "ready":ready}));
    viewer.until_reply();
    // Wait for the owner to observe the acknowledgement, independently of caller timing.
    let deadline = Instant::now() + Duration::from_secs(5);
    loop {
        viewer.request(json!({"kind":"controlled"}));
        if viewer.until_reply()["ready"] == true {
            break;
        }
        assert!(Instant::now() < deadline);
    }
    let report = fixture
        .command(&["open", "--session", &session, "--json"])
        .env("PATH", &path)
        .output()
        .unwrap();
    assert!(report.status.success(), "{report:?}");
    let report: Value = serde_json::from_slice(&report.stdout).unwrap();
    assert_eq!(report["receipts"][0]["status"], "elsewhere");
    assert_eq!(fixture.session_of_owner(&owner), session);
    assert_eq!(fixture.launches(1).len(), 1);
    viewer.input("done\n");
    viewer.finish();
}

#[test]
fn conversation_open_continues_closed_history_without_replaying_the_request() {
    reopen_conversation(false);
}

#[test]
fn completed_task_closes_a_conversation_resumed_after_checkout_retirement() {
    reopen_conversation(true);
}

fn reopen_conversation(retired: bool) {
    let fixture = Fixture::new();
    let path = fixture.provider(
        r#"#!/bin/sh
if [ "$1" = --version ]; then echo '2.1.302'; exit; fi
stty -echo < /dev/tty
printf '%s:%s\n' "$LF_CAPTURE_KEY" "$*" >> "$HOME/launches"
read -r answer < /dev/tty
"#,
    );
    let first = fixture.owner(fixture.native(
        Path::new(env!("CARGO_BIN_EXE_lf")),
        &path,
        "original-request",
    ));
    fixture.launches(1);
    let session = fixture.session_of_owner(&first);
    let mut source = fixture.viewer(&first, true);
    source.input("done\n");
    source.finish();
    fixture.ends(&first);
    // Stand in for the provider's saved thread after its first terminal exits.
    let sql = fixture.sql();
    let input: String = sql.query_row("SELECT json_extract(e.payload,'$.artifact_key') FROM agent_sessions s JOIN session_events e ON e.seq=s.current_capture WHERE s.id=?1", [&session], |row| row.get(0)).unwrap();
    let native = uuid::Uuid::new_v4().to_string();
    sql.execute(
        "UPDATE agent_sessions SET provider_thread=?2 WHERE id=?1",
        [&session, &native],
    )
    .unwrap();
    sql.execute(
        "INSERT INTO session_events(session_id,kind,receipt_key,observed_at,payload,captured_event)
         SELECT id,'observed',?2||':fixture-native',created_at,json_object('input_id',?2,'source','provider-session:fixture','evidence',json_object('schema_version',1,'provider_session_id',?3,'account_id',NULL)),current_capture FROM agent_sessions WHERE id=?1",
        [&session, &input, &native],
    ).unwrap();
    let transcript = fixture
        .home
        .path()
        .join(format!("claude/projects/fixture/{native}.jsonl"));
    std::fs::create_dir_all(transcript.parent().unwrap()).unwrap();
    std::fs::write(
        transcript,
        json!({"type":"user","userType":"external","message":{"content":"original-request"}})
            .to_string(),
    )
    .unwrap();
    if retired {
        let task = complete_conversation(&fixture, &first);
        sql.execute(
            "UPDATE tasks SET worktree=?2,branch='retired' WHERE id=?1",
            rusqlite::params![task, fixture.repo.path().join("removed").to_str().unwrap()],
        )
        .unwrap();
        sql.execute(
            "INSERT INTO task_retirements(task_id,completion,checkout,branch,due_at,phase)
            SELECT id,planning_completed_at,worktree,branch,0,'retired' FROM tasks WHERE id=?1",
            [&task],
        )
        .unwrap();
    }
    let output = fixture
        .command(&["open", "--session", &session, "--focus", "--json"])
        .env("PATH", &path)
        .output()
        .unwrap();
    assert!(!output.status.success());
    assert!(String::from_utf8_lossy(&output.stdout).contains("Not open"));
    // A fresh native client needs the terminal a person opens from.
    let mut command = fixture.serve(
        true,
        &[
            env!("CARGO_BIN_EXE_lf"),
            "-a",
            "codex",
            "open",
            "--session",
            &session,
        ],
    );
    command
        .env("PATH", &path)
        .env("LF_SETTINGS", "agent: codex");
    let owner = fixture.owner(command);
    let mut viewer = fixture.viewer(&owner, true);
    viewer.until_event("return");
    let launches = fixture.launches(2);
    let (_, command) = launches[1].split_once(':').unwrap();
    let args = shlex::split(command).unwrap();
    let resume = args.iter().position(|arg| arg == "--resume").unwrap();
    assert_eq!(args[resume + 1], native);
    assert!(
        !args.iter().any(|arg| arg == "--"),
        "no new prompt: {args:?}"
    );
    if retired {
        drop(viewer);
        fixture.wait_viewed(&owner, false);
        // Resumption keeps the Task done and the retirement finished. Advance
        // only the new owner's absence, then let its real policy close it.
        sql.execute(
            "UPDATE processes SET viewer_hold_at=0 WHERE id=?1",
            [&owner],
        )
        .unwrap();
        retire_conversations(&fixture);
        fixture.ends(&owner);
        let (done, phase): (bool, String) = sql.query_row(
            "SELECT t.planning_completed,h.phase FROM tasks t JOIN task_retirements h ON h.task_id=t.id",
            [], |r| Ok((r.get(0)?,r.get(1)?))).unwrap();
        assert!(done);
        assert_eq!(phase, "retired");
        assert_eq!(
            sql.query_row(
                "SELECT provider_thread FROM agent_sessions WHERE id=?1",
                [&session],
                |r| r.get::<_, String>(0)
            )
            .unwrap(),
            native
        );
    } else {
        viewer.input("done\n");
        viewer.finish();
        fixture.ends(&owner);
    }
    assert_eq!(fixture.launches(2).len(), 2);
}

/// Workspace opening uses the same resume command as a person in a native tab.
#[test]
#[ignore = "requires the installed Rex CLI; creates and destroys an isolated local server"]
fn rex_workspace_resumes_the_existing_provider_in_the_task_worktree() {
    let fixture = Fixture::new();
    let (_root, bin, rex) = isolated_rex();
    support::register_task_without_pr(
        fixture.home.path(),
        fixture.repo.path(),
        "main",
        &fixture.repo.head_sha(),
    );
    let provider = fixture.provider(
        r#"#!/bin/sh
if [ "$1" = --version ]; then echo '2.1.302'; exit; fi
stty -echo < /dev/tty
printf '%s:%s:%s\n' "$$" "$LF_CAPTURE_KEY" "$*" >> "$HOME/launches"
printf 'workspace-ready\r\n'
read -r answer < /dev/tty
printf '%s' "$answer" > "$HOME/answer"
"#,
    );
    let owner =
        fixture.owner(fixture.native(Path::new(env!("CARGO_BIN_EXE_lf")), &provider, "workspace"));
    fixture.launches(1);
    let session = fixture.session_of_owner(&owner);
    let bound = fixture
        .command(&["session", "bind", &session, "--task", "INF-123", "--json"])
        .output()
        .unwrap();
    assert!(bound.status.success(), "{bound:?}");
    let mut source = fixture.viewer(&owner, true);
    source.input("unfinished-");
    drop(source);
    let path = format!("{}:{provider}", bin.display());
    let opened = fixture
        .command(&[
            "workspace",
            "--task",
            "INF-123",
            "--session",
            &session,
            "--host",
            "rex",
            "--server",
            &rex.endpoint,
            "--no-focus",
            "--json",
        ])
        .env("PATH", &path)
        .output()
        .unwrap();
    assert!(opened.status.success(), "{opened:?}");
    let report: Value = serde_json::from_slice(&opened.stdout).unwrap();
    let container = report["receipts"][0]["container"].as_str().unwrap();
    let block = rex.block_ids(container).into_iter().next().unwrap();

    let deadline = Instant::now() + Duration::from_secs(10);
    loop {
        let pwd = rex.block_call(container, &block, "pwd");
        if rex.screen(container, &block).contains("workspace-ready")
            && pwd
                .to_string()
                .contains(fixture.repo.path().to_str().unwrap())
        {
            break;
        }
        assert!(
            Instant::now() < deadline,
            "wrong workspace directory: {pwd}"
        );
        std::thread::sleep(Duration::from_millis(50));
    }
    rex.write(container, &block, "continued\n");
    assert_eq!(fixture.wait_file("answer"), "unfinished-continued");
    assert_eq!(
        fixture.launches(1).len(),
        1,
        "resume started another provider"
    );
}

/// A Task with no conversation yet: opening it starts one provider, in the Task's
/// linked worktree rather than the repository, ready for what a person types.
#[test]
#[ignore = "requires the installed Rex CLI; creates and destroys an isolated local server"]
fn rex_open_starts_a_fresh_conversation_in_the_task_worktree() {
    let fixture = Fixture::new();
    let (_root, bin, rex) = isolated_rex();
    let registered = support::register_task_without_pr(
        fixture.home.path(),
        fixture.repo.path(),
        "main",
        &fixture.repo.head_sha(),
    );
    let worktree = fixture.repo.create_named_worktree("fresh-task");
    support::register_sibling_task(&registered, "INF-124", "fresh-task", &worktree);
    // Rex children find the provider on the server's PATH, the guarded `bin`,
    // and their $HOME is the server's, so the script names the fixture's.
    let provider = bin.join("claude");
    std::fs::write(
        &provider,
        format!(
            r#"#!/bin/sh
if [ "$1" = --version ]; then echo '2.1.302'; exit; fi
stty -echo < /dev/tty
pwd -P >> '{home}/launches'
printf 'fresh-ready\r\n'
read -r answer < /dev/tty
printf '%s' "$answer" > '{home}/answer'
"#,
            home = fixture.home.path().display()
        ),
    )
    .unwrap();
    std::fs::set_permissions(&provider, std::fs::Permissions::from_mode(0o755)).unwrap();
    let agent = fixture
        .command(&["set", "--global", "default_agent", "claude"])
        .output()
        .unwrap();
    assert!(agent.status.success(), "{agent:?}");
    let path = format!("{}:{}", bin.display(), std::env::var("PATH").unwrap());
    let opened = fixture
        .command(&[
            "open",
            "--task",
            "INF-124",
            "--host",
            "rex",
            "--server",
            &rex.endpoint,
            "--tab",
            "--json",
        ])
        .env("PATH", &path)
        .output()
        .unwrap();
    assert!(opened.status.success(), "{opened:?}");
    let report: Value = serde_json::from_slice(&opened.stdout).unwrap();
    let container = report["receipts"][0]["container"].as_str().unwrap();
    let blocks = rex.call("session.list_blocks", json!({"session_id":container}));
    let block = blocks["blocks"]
        .as_array()
        .unwrap()
        .iter()
        .find(|block| block["label"] == "Conversation")
        .unwrap_or_else(|| panic!("no Conversation pane: {blocks}"))["block_id"]
        .as_str()
        .unwrap()
        .to_owned();
    let deadline = Instant::now() + Duration::from_secs(20);
    while !rex.screen(container, &block).contains("fresh-ready") {
        assert!(
            Instant::now() < deadline,
            "provider never drew: {}\n{:?}",
            rex.screen(container, &block),
            std::fs::read_to_string(fixture.home.path().join("logs/lf.log"))
        );
        std::thread::sleep(Duration::from_millis(50));
    }
    let checkout = worktree.canonicalize().unwrap();
    assert_eq!(fixture.launches(1), [checkout.to_str().unwrap()]);
    assert!(rex
        .pwd(container, &block)
        .is_some_and(|url| url.ends_with(&format!("{}/", checkout.display()))));
    rex.write(container, &block, "typed-here\n");
    assert_eq!(fixture.wait_file("answer"), "typed-here");
}

/// Someone in an ordinary Rex terminal elsewhere resumes a live Session by hand.
/// While attached, the terminal's process and its report both name the Task
/// directory, so a native Split starts there; leaving withdraws the report.
#[test]
#[ignore = "requires the installed Rex CLI; creates and destroys an isolated local server"]
fn rex_manual_resume_elsewhere_reports_the_session_directory_until_it_leaves() {
    let fixture = Fixture::new();
    let (root, _bin, rex) = isolated_rex();
    let provider = fixture.waiting_provider();
    let lf = env!("CARGO_BIN_EXE_lf");
    let owner = fixture.owner(fixture.native(Path::new(lf), &provider, "manual"));
    fixture.launches(1);
    let session = fixture.session_of_owner(&owner);
    let elsewhere = root.path().join("elsewhere");
    std::fs::create_dir(&elsewhere).unwrap();
    let (pid, left) = (root.path().join("viewer.pid"), root.path().join("left"));
    let script = r#"cd "$1" && /bin/sh -c 'printf %s "$$" > "$0"; exec "$@"' "$2" env HOME="$3" LF_HOME="$3" LF_BIN="$4" RUST_LOG=off "$4" session resume "$5"; printf left > "$6"; exec /bin/cat"#;
    let created = rex.call(
        "session.create",
        json!({"label":"manual","initial_windows":[{"window_label":"manual","layout":{"block":{
            "flavor":"com.superlogical.terminal.shell","options":{"shell":"none","cwd":elsewhere,
            "command":["/bin/sh","-c",script,"manual",elsewhere,pid,fixture.home.path(),lf,session,left]}}}}]}),
    );
    let container = created["session_id"].as_str().unwrap().to_owned();
    let block = rex.block_ids(&container).remove(0);
    let checkout = fixture.repo.path().canonicalize().unwrap();
    let reported = || rex.pwd(&container, &block);

    let deadline = Instant::now() + Duration::from_secs(15);
    let task_url = format!("{}/", checkout.display());
    while !reported().is_some_and(|url| url.ends_with(&task_url)) {
        assert!(
            Instant::now() < deadline,
            "no Task report: {:?}\n{}",
            reported(),
            rex.screen(&container, &block)
        );
        std::thread::sleep(Duration::from_millis(50));
    }
    // Rex's login wrapper can hide the viewer's cwd from the host; read the
    // launcher-written viewer process directly.
    let viewer = std::fs::read_to_string(&pid).unwrap();
    let lsof = Command::new("lsof")
        .args(["-a", "-d", "cwd", "-Fn", "-p", viewer.trim()])
        .output()
        .unwrap();
    let cwd = String::from_utf8_lossy(&lsof.stdout)
        .lines()
        .find_map(|line| line.strip_prefix('n').map(PathBuf::from))
        .expect("viewer cwd");
    assert_eq!(cwd.canonicalize().unwrap(), checkout);

    rex.write(&container, &block, "\u{1d}");
    let deadline = Instant::now() + Duration::from_secs(10);
    while !left.exists() || reported().is_some() {
        assert!(
            Instant::now() < deadline,
            "report outlived the viewer: {:?}",
            reported()
        );
        std::thread::sleep(Duration::from_millis(50));
    }
    // Resuming attached to the running provider; leaving never ended it.
    let launches = std::fs::read_to_string(fixture.home.path().join("launches")).unwrap();
    assert_eq!(launches.lines().count(), 1, "{launches}");
    fixture.viewer(&owner, true).input("done\n");
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
    // The Session retains its owner after exit so the final hold remains readable.
    fixture.session(&owner);
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
    // A viewer whose connection is lost may still be running elsewhere.
    let remote = fixture.viewer(&owner, false);
    wait_hold(&fixture, &owner, Some("viewed"));
    remote.socket.shutdown(std::net::Shutdown::Both).unwrap();
    drop(remote);
    let mut finisher = fixture.viewer(&owner, true);
    finisher.input("done\n");
    finisher.finish();
    assert_eq!(fixture.wait_file("answer"), "done");
    // Ending does not create an indefinite hold for the lost viewer.
    wait_hold(&fixture, &owner, Some("absent"));
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
fn automatic_command_attachment_preserves_an_existing_viewers_control() {
    let fixture = Fixture::new();
    let owner = fixture.start(&[
        "/bin/sh",
        "-c",
        "stty -echo; read -r answer; printf '%s' \"$answer\" > \"$HOME/answer\"",
    ]);
    let mut controller = fixture.viewer(&owner, true);
    let output = fixture
        .command(&[
            "session",
            "attach",
            "--",
            "/usr/bin/env",
            "LF_VIEWER_STREAM=1",
            env!("CARGO_BIN_EXE_lf"),
            "session",
            "attach",
            &owner,
        ])
        .env("LF_RELAY_IF_FREE", "1")
        .output()
        .unwrap();
    assert!(!output.status.success());
    assert!(
        String::from_utf8_lossy(&output.stderr).contains("Session is attached in another viewer"),
        "{output:?}"
    );
    controller.input("original-controller\n");
    controller.finish();
    assert_eq!(fixture.wait_file("answer"), "original-controller");
}

#[test]
fn prepared_reveal_preserves_a_later_controlling_viewer() {
    prepared_open_preserves_controller("--reveal-existing", false);
}

#[test]
fn prepared_start_preserves_a_later_controlling_viewer() {
    prepared_open_preserves_controller("--start-if-free", false);
}

#[test]
fn prepared_start_preserves_a_claim_time_winner() {
    prepared_open_preserves_controller("--start-if-free", true);
}

fn prepared_open_preserves_controller(intent: &str, claim_race: bool) {
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
    let mut command = fixture.serve(
        true,
        &[
            env!("CARGO_BIN_EXE_lf"),
            "--interactive",
            "--agent",
            "claude",
            ":",
            "original input",
        ],
    );
    command.env("PATH", path);
    let owner = fixture.owner(command);
    let launched = fixture.wait_file("launches");
    let session = session_of(&fixture.sql(), launched.trim());
    let prepared = fixture
        .command(&["session", "resume", &session, intent, "--prepare", "--json"])
        .output()
        .unwrap();
    assert!(prepared.status.success(), "{prepared:?}");
    let prepared: Value = serde_json::from_slice(&prepared.stdout).unwrap();
    assert!(prepared["open_argv"]
        .as_array()
        .unwrap()
        .contains(&json!(intent)));
    let mut controller = Some(fixture.viewer(&owner, !claim_race));
    let endpoint = fixture.endpoint(&owner);
    let retained = endpoint.with_extension("retained");
    let race = if claim_race {
        // Interpose only at the socket boundary: all ownership reads see the
        // free real owner, then the competing viewer claims before Attach arrives.
        std::fs::rename(&endpoint, &retained).unwrap();
        let listener = UnixListener::bind(&endpoint).unwrap();
        let backend = retained.clone();
        let mut winner = controller.take().unwrap();
        Some(std::thread::spawn(move || loop {
            let (mut socket, _) = listener.accept().unwrap();
            let (tag, payload) = read_frame(&mut socket).unwrap();
            assert_eq!(tag, REQUEST);
            let request: Value = serde_json::from_slice(&payload).unwrap();
            let attaching = request["kind"] == "attach";
            if attaching {
                winner.request(json!({"kind":"attach","control":true,"replace":true,
                        "own":"take","rows":24,"cols":80}));
                let attached = winner.until_reply();
                winner.accept(&attached);
            } else {
                assert_eq!(request["kind"], "controlled");
            }
            let mut real = UnixStream::connect(&backend).unwrap();
            real.write_all(&frame(tag, &payload)).unwrap();
            let (reply_tag, reply) = read_frame(&mut real).unwrap();
            socket.write_all(&frame(reply_tag, &reply)).unwrap();
            if attaching {
                assert_eq!(
                    serde_json::from_slice::<Value>(&reply).unwrap()["kind"],
                    "busy"
                );
                return winner;
            }
        }))
    } else {
        None
    };
    let opened = fixture.prepared_command(&prepared).output().unwrap();
    assert!(
        !opened.status.success(),
        "opening stole a competing claim: {opened:?}"
    );
    if let Some(race) = race {
        controller = Some(race.join().unwrap());
        std::fs::remove_file(&endpoint).unwrap();
        std::fs::rename(retained, endpoint).unwrap();
    }
    assert_eq!(fixture.wait_file("launches"), launched);
    assert!(!fixture.home.path().join("answer").exists());
    let mut controller = controller.unwrap();
    controller.input("original-controller\n");
    controller.finish();
    let pid = launched.trim().split(':').next().unwrap();
    assert_eq!(
        fixture.wait_file("answer").trim(),
        format!("{pid}:original-controller")
    );
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

#[test]
fn a_locked_store_does_not_delay_input_or_replay_it_on_return() {
    let fixture = Fixture::new();
    let owner = fixture.start(&[
        "/bin/sh",
        "-c",
        "stty -echo; printf ready; read -r first; printf '%s' \"$first\" > \"$HOME/received\"; printf 'first:%s\\n' \"$first\"; read -r second; printf 'second:%s\\n' \"$second\"",
    ]);
    let mut viewer = fixture.viewer(&owner, true);
    until_out(&mut viewer, "ready");
    let sql = fixture.sql();
    sql.execute_batch("BEGIN IMMEDIATE").unwrap();
    viewer.input("retained-draft\n");
    // Input and control still flow while every SQLite writer is blocked.
    std::thread::sleep(Duration::from_millis(100));
    viewer
        .socket
        .set_read_timeout(Some(Duration::from_secs(2)))
        .unwrap();
    viewer.request(json!({"kind":"controlled"}));
    assert_eq!(viewer.until_reply()["held"], true);
    assert_eq!(fixture.wait_file("received"), "retained-draft");
    drop(viewer);
    let mut returned = fixture.viewer(&owner, true);
    returned.request(json!({"kind":"controlled"}));
    assert_eq!(returned.until_reply()["held"], true);
    sql.execute_batch("ROLLBACK").unwrap();
    // A replay would consume the second read, so continued input proves
    // both delivery order and that the queued request arrived only once.
    returned.input("continued\n");
    until_out(&mut returned, "second:continued\r\n");
    returned.finish();
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
                            json!({"answers":request["id"],"seq":0,"kind":"controlled","held":false,"ready":false})
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

/// A viewer told it no longer holds input restores its terminal, prints the
/// owner's notice and leaves with exit 0. Nothing runs after it: the owner
/// judged itself, and the person decides what to open next.
#[test]
fn control_handoff_restores_the_terminal_and_prints_the_notice() {
    let fixture = Fixture::new();
    let owner = fixture.start(&["/bin/sh", "-c", "sleep 60"]);
    let session = fixture.session(&owner);
    let endpoint = fixture.endpoint(&owner);
    let retained = endpoint.with_extension("retained");
    std::fs::rename(&endpoint, &retained).unwrap();
    let listener = UnixListener::bind(&endpoint).unwrap();
    listener.set_nonblocking(true).unwrap();

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

    let mut viewer = fixture
        .command(&["session", "resume", &session])
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
    let event = json!({"seq":1,"type":"control","held":false,"notice":"owner retired"});
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
            panic!("viewer did not exit: {}", String::from_utf8_lossy(&screen));
        }
        std::thread::sleep(Duration::from_millis(20));
    }
    let output = viewer.wait_with_output().unwrap();
    let stdout = String::from_utf8_lossy(&screen);
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert_eq!(output.status.code(), Some(0), "{stdout} {stderr}");
    assert!(stderr.contains("owner retired"), "{stderr}");
    assert!(stdout.contains("\x1b[?1049l"), "{stdout}");
    drop(listener);
    std::fs::remove_file(&endpoint).unwrap();
    std::fs::rename(retained, endpoint).unwrap();
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
        assert!(error.contains("stopped reading requests"), "{error}");
        assert!(error.contains("later input was not delivered"), "{error}");
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
        // The launching lf, which stays recorded after the provider exits and
        // its attachment is released.
        let placement: Option<String> = fixture.sql().query_row(
            "SELECT p.terminal_id FROM agent_sessions s JOIN processes a ON a.id=s.agent_process_id JOIN processes p ON p.id=a.parent_lf_process_id WHERE s.id=?1",
            [&session], |row| row.get(0),
        ).unwrap();
        terminal.stop();
        assert_eq!(placement.as_deref(), valid.then_some("desktop-shell"));
    }
}

/// A terminal host's pipe reaches the lf it starts. The detached owner outlives
/// that terminal and must not keep the pipe, or the host's other reader (Desktop's
/// status observer) never sees end of input and runs for as long as the owner.
#[test]
fn detached_owner_keeps_no_descriptor_its_terminal_inherited() {
    let fixture = Fixture::new();
    let path = fixture.waiting_provider();
    let (mut reader, writer) = std::io::pipe().unwrap();
    let inherited = writer.as_raw_fd();
    let mut command = fixture.command(&["-i", "-a", "claude", ":", "inherited"]);
    command.env("PATH", path);
    // SAFETY: fcntl is async-signal-safe; only this child inherits the writer.
    unsafe {
        std::os::unix::process::CommandExt::pre_exec(&mut command, move || {
            libc::fcntl(inherited, libc::F_SETFD, 0);
            Ok(())
        });
    }
    let terminal = Terminal::spawn(command);
    let launch = fixture.launches(1).pop().unwrap();
    let owner = fixture.owner_of(&session_of(&fixture.sql(), &launch));
    drop(writer);
    terminal.stop();
    let (done, closed) = std::sync::mpsc::channel();
    std::thread::spawn(move || done.send(reader.read(&mut [0; 1]).unwrap()));
    assert_eq!(closed.recv_timeout(Duration::from_secs(10)), Ok(0));
    assert!(fixture.running(&owner), "the owner outlives its terminal");
}

/// Bare `lf` behaves as its provider would started from the shell: the
/// provider leads the terminal's foreground group, no owner holds it, and
/// closing the terminal ends it.
#[test]
fn bare_lf_runs_its_provider_in_the_foreground_and_ends_with_its_terminal() {
    use std::os::unix::process::CommandExt;
    let fixture = Fixture::new();
    let path = fixture.provider(
        r#"#!/bin/sh
if [ "$1" = --version ]; then echo '2.1.302'; exit; fi
printf '%s %s\n' "$$" "$(ps -o tpgid= -p $$ | tr -d ' ')" > "$HOME/foreground.tmp"
mv "$HOME/foreground.tmp" "$HOME/foreground"
read -r answer < /dev/tty
"#,
    );
    let (mut master, mut slave) = (-1, -1);
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
    let master = unsafe { std::fs::File::from_raw_fd(master) };
    let slave = unsafe { std::fs::File::from_raw_fd(slave) };
    // SAFETY: flags on a descriptor this test owns. A terminal emulator's
    // master is never inherited by the programs it runs.
    unsafe {
        libc::fcntl(master.as_raw_fd(), libc::F_SETFL, libc::O_NONBLOCK);
        libc::fcntl(master.as_raw_fd(), libc::F_SETFD, libc::FD_CLOEXEC);
    }
    let mut command = fixture.command(&["-i", "-a", "claude"]);
    command
        .env("PATH", path)
        .stdin(slave.try_clone().unwrap())
        .stdout(slave.try_clone().unwrap())
        .stderr(slave);
    // SAFETY: setsid and ioctl are async-signal-safe. The PTY becomes lf's
    // controlling terminal, as a shell's is.
    unsafe {
        command.pre_exec(|| {
            if libc::setsid() < 0 || libc::ioctl(0, libc::TIOCSCTTY as _, 0) < 0 {
                return Err(std::io::Error::last_os_error());
            }
            Ok(())
        });
    }
    let mut lf = command.spawn().unwrap();
    drop(command);
    // The reader holds the only master descriptor; closing the terminal is
    // that thread dropping it.
    let screen = std::sync::Arc::new(std::sync::Mutex::new(Vec::new()));
    let written = screen.clone();
    let (close, closing) = std::sync::mpsc::channel::<()>();
    let reader = std::thread::spawn(move || {
        let mut master = master;
        let mut buffer = [0; 4096];
        while closing.try_recv().is_err() {
            match master.read(&mut buffer) {
                Ok(read @ 1..) => written.lock().unwrap().extend_from_slice(&buffer[..read]),
                _ => std::thread::sleep(Duration::from_millis(20)),
            }
        }
    });
    let deadline = Instant::now() + Duration::from_secs(30);
    let recorded = loop {
        if let Ok(recorded) = std::fs::read_to_string(fixture.home.path().join("foreground")) {
            break recorded;
        }
        assert!(
            Instant::now() < deadline && lf.try_wait().unwrap().is_none(),
            "the provider did not start:\n{}",
            String::from_utf8_lossy(&screen.lock().unwrap())
        );
        std::thread::sleep(Duration::from_millis(20));
    };
    let (provider, foreground) = recorded.trim().split_once(' ').unwrap();
    assert_eq!(
        provider, foreground,
        "the provider leads the foreground group"
    );
    let owners: i64 = fixture
        .sql()
        .query_row(
            "SELECT COUNT(*) FROM agent_sessions WHERE transport_lf_process_id IS NOT NULL",
            [],
            |row| row.get(0),
        )
        .unwrap();
    assert_eq!(owners, 0, "no owner holds a bare conversation");
    close.send(()).unwrap();
    reader.join().unwrap();
    let deadline = Instant::now() + Duration::from_secs(20);
    while lf.try_wait().unwrap().is_none() {
        assert!(Instant::now() < deadline, "lf outlived its terminal");
        std::thread::sleep(Duration::from_millis(20));
    }
    let provider: libc::pid_t = provider.parse().unwrap();
    let deadline = Instant::now() + Duration::from_secs(20);
    // SAFETY: signal 0 only checks whether the stand-in still exists.
    while unsafe { libc::kill(provider, 0) } == 0 {
        assert!(
            Instant::now() < deadline,
            "the provider outlived its terminal"
        );
        std::thread::sleep(Duration::from_millis(20));
    }
}

#[test]
fn taskless_session_renames_reach_plain_terminals_and_reattached_viewers() {
    let fixture = Fixture::new();
    let path = fixture.provider(
        r#"#!/bin/sh
if [ "$1" = --version ]; then echo '2.1.302'; exit; fi
stty -echo < /dev/tty
printf '%s\n' "$*" > "$HOME/launches"
while read -r answer < /dev/tty; do
    [ "$answer" = stale ] || exit 0
    printf '\033]2;default\007\033]7501;state=idle:app=claude-code\033\\'
done
"#,
    );
    let mut command = fixture.serve(true, &[env!("CARGO_BIN_EXE_lf"), "-i", "-a", "claude"]);
    command
        .env("PATH", path)
        .env_remove("TMUX")
        .env_remove("TMUX_PANE");
    let owner = fixture.owner(command);
    fixture.launches(1);
    let session = fixture.session_of_owner(&owner);
    let initial: String = fixture
        .sql()
        .query_row(
            "SELECT title FROM agent_sessions WHERE id=?1",
            [&session],
            |row| row.get(0),
        )
        .unwrap();
    let mut viewer = fixture.viewer(&owner, true);
    until_out(&mut viewer, &format!("\x1b]0;{initial}\x07"));
    let renamed = fixture
        .command(&["session", "rename", &session, "Resume repair"])
        .output()
        .unwrap();
    assert!(
        renamed.status.success(),
        "{}",
        String::from_utf8_lossy(&renamed.stderr)
    );
    until_out(&mut viewer, "\x1b]0;Resume repair\x07");
    viewer.input("stale\n");
    let overwritten = out(&until_out(&mut viewer, "\x1b]0;Resume repair\x07"));
    assert!(overwritten
        .windows(b"\x1b]2;default\x07".len())
        .any(|bytes| bytes == b"\x1b]2;default\x07"));
    assert!(overwritten
        .windows(b"7501;state=idle".len())
        .any(|bytes| bytes == b"7501;state=idle"));
    drop(viewer);
    let mut viewer = fixture.viewer(&owner, true);
    let snapshot = match viewer.next().unwrap() {
        Item::Snapshot(bytes) => bytes,
        other => panic!("expected snapshot, got {other:?}"),
    };
    viewer.input("bye\n");
    viewer.finish();
    fixture.ends(&owner);
    assert_ne!(initial, "default");
    let title = b"\x1b]0;Resume repair\x07";
    assert!(snapshot.windows(title.len()).any(|bytes| bytes == title));
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
    let owner = fixture.owner(fixture.native(
        Path::new(env!("CARGO_BIN_EXE_lf")),
        &path,
        "one original input",
    ));
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
    let owner =
        fixture.owner(fixture.native(Path::new(env!("CARGO_BIN_EXE_lf")), &path, "stay small"));
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
    let owner =
        fixture.owner(fixture.native(Path::new(env!("CARGO_BIN_EXE_lf")), &path, "exec failure"));
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
fn prepared_start_and_take_back_keep_one_thread_with_or_without_a_session() {
    for existing in [false, true] {
        let fixture = Fixture::new();
        let path = fixture.provider(
            r#"#!/bin/sh
if [ "$1" = --version ]; then echo '2.1.302'; exit; fi
printf '%s:%s:%s\n' "$$" "$LF_CAPTURE_KEY" "$*" >> "$HOME/launches"
read -r answer < /dev/tty
printf '%s' "$answer" > "$HOME/answer"
"#,
        );
        let prepare = |args: &[&str]| -> Value {
            let result = fixture.command(args).env("PATH", &path).output().unwrap();
            assert!(result.status.success(), "{result:?}");
            serde_json::from_slice(&result.stdout).unwrap()
        };
        let before = existing.then(|| prepare(&["-a", "claude", "resume", "--prepare", "--json"]));
        let mut args = vec![
            "-a",
            "claude",
            "resume",
            "--start-if-free",
            "--prepare",
            "--json",
        ];
        if let Some(before) = &before {
            args.push(before["id"].as_str().unwrap());
        }
        let prepared = prepare(&args);
        let session = prepared["id"].as_str().unwrap();
        if let Some(before) = &before {
            assert_eq!(before["id"], session);
        }
        assert!(!fixture.home.path().join("launches").exists());
        assert!(prepared["open_argv"]
            .as_array()
            .unwrap()
            .contains(&json!("--start-if-free")));

        // Execute exactly what preparation returned, through delegation and relay.
        let mut command = fixture.prepared_command(&prepared);
        command.env("PATH", &path);
        let mut first = Terminal::spawn(command);
        let launched = fixture.wait_file("launches");
        let owner = fixture.owner_of(session);
        fixture.wait_controlled(&owner);
        let capture: i64 = fixture
            .sql()
            .query_row(
                "SELECT current_capture FROM agent_sessions WHERE id=?1",
                [session],
                |r| r.get(0),
            )
            .unwrap();

        // This prepared Start predates the holder. It may not replay or replace it.
        let refused = fixture
            .prepared_command(&prepared)
            .env("PATH", &path)
            .output()
            .unwrap();
        assert!(!refused.status.success(), "{refused:?}");
        assert!(String::from_utf8_lossy(&refused.stderr).contains("Take back"));
        assert_eq!(fixture.wait_file("launches"), launched);
        assert!(!fixture.home.path().join("answer").exists());

        // Explicit Take back alone carries ordinary CLI resume authority.
        let takeover = prepare(&["session", "resume", session, "--prepare", "--json"]);
        assert!(!takeover["open_argv"]
            .as_array()
            .unwrap()
            .contains(&json!("--start-if-free")));
        let mut next = fixture
            .prepared_command(&takeover)
            .env("PATH", &path)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .unwrap();
        let deadline = Instant::now() + Duration::from_secs(30);
        let departed = loop {
            if let Some(status) = first.child.try_wait().unwrap() {
                break status;
            }
            assert!(Instant::now() < deadline, "viewer did not depart");
            std::thread::sleep(Duration::from_millis(20));
        };
        assert!(departed.success(), "{departed:?}");
        next.stdin
            .take()
            .unwrap()
            .write_all(b"taken-back\n")
            .unwrap();
        let result = wait_for_viewer(next, Duration::from_secs(30));
        assert!(result.status.success(), "{result:?}");
        assert_eq!(fixture.wait_file("answer"), "taken-back");
        assert_eq!(fixture.wait_file("launches"), launched);
        assert_eq!(
            fixture
                .sql()
                .query_row(
                    "SELECT current_capture FROM agent_sessions WHERE id=?1",
                    [session],
                    |r| r.get::<_, i64>(0),
                )
                .unwrap(),
            capture
        );
        assert_eq!(
            fixture
                .sql()
                .query_row("SELECT count(*) FROM agent_sessions", [], |r| r
                    .get::<_, i64>(0))
                .unwrap(),
            1
        );
        fixture.ends(&owner);
    }
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

fn retire_conversations(fixture: &Fixture) {
    let output = fixture.command(&["task", "retire"]).output().unwrap();
    assert!(output.status.success(), "{output:?}");
}

/// Complete a Task without a checkout and advance its viewing grace.
fn complete_conversation(fixture: &Fixture, owner: &str) -> String {
    fixture.wait_viewed(owner, false);
    let sql = fixture.sql();
    let session = fixture.bound_session(owner);
    let task: Option<String> = sql
        .query_row(
            "SELECT task_id FROM agent_sessions WHERE id=?1",
            [&session],
            |r| r.get(0),
        )
        .unwrap();
    let task = task.unwrap_or_else(|| {
        let store =
            loopflow::store::sqlite::SqliteStore::new(&fixture.home.path().join("loopflow.db"))
                .unwrap();
        let task = store
            .create_task(&loopflow::work::task::NewTask {
                id: loopflow::durable::TaskId::new(),
                repository_id: store
                    .ensure_repository(fixture.repo.path().to_str().unwrap())
                    .unwrap(),
                project_id: None,
                title: "Retire conversation".into(),
                description: String::new(),
                due_date: None,
            })
            .unwrap();
        task.id.to_string()
    });
    sql.execute(
        "UPDATE agent_sessions SET task_id=?1,interactive=1 WHERE id=?2",
        rusqlite::params![task, session],
    )
    .unwrap();
    // The planning journal owns Task state, and the Machine server projects
    // it back over the row, so complete as a journal edit over the observed one.
    let disposition = json!({"planning_state": "completed", "planning_completed": 1,
        "planning_completed_at": "2000-01-01T00:00:00Z"});
    sql.execute_batch("BEGIN IMMEDIATE").unwrap();
    let parents: Vec<String> = sql
        .prepare(
            "SELECT id FROM planning_peer_changes WHERE object_id=?1 AND field='disposition'
             AND id IN (SELECT id FROM planning_peer_observed)",
        )
        .unwrap()
        .query_map([&task], |r| r.get(0))
        .unwrap()
        .map(Result::unwrap)
        .collect();
    let clock: i64 = sql
        .query_row("SELECT max(clock)+1 FROM planning_peer_changes", [], |r| {
            r.get(0)
        })
        .unwrap();
    let change = uuid::Uuid::new_v4().to_string();
    let mutation = json!({"edit": {"target": task, "change": {"field": "disposition",
        "value": disposition}}, "clock": clock, "parents": parents});
    sql.execute(
        "INSERT INTO planning_peer_changes(id,record_table,mutation) VALUES(?1,'tasks',?2)",
        [&change, &mutation.to_string()],
    )
    .unwrap();
    sql.execute(
        "DELETE FROM planning_peer_observed WHERE id IN
         (SELECT id FROM planning_peer_changes WHERE object_id=?1 AND field='disposition')",
        [&task],
    )
    .unwrap();
    sql.execute(
        "INSERT INTO planning_peer_observed(id) VALUES(?1)",
        [&change],
    )
    .unwrap();
    sql.execute("UPDATE tasks SET planning_state='completed',planning_completed=1,planning_completed_at='2000-01-01T00:00:00Z' WHERE id=?1", [&task]).unwrap();
    sql.execute_batch("COMMIT").unwrap();
    // Advance the existing absence grace without waiting fifteen minutes.
    sql.execute(
        "UPDATE processes SET viewer_hold='absent',viewer_hold_at=0 WHERE id=?1",
        [owner],
    )
    .unwrap();
    task
}

#[test]
fn completed_task_without_checkout_ends_a_conversation_with_typed_input() {
    let fixture = Fixture::new();
    let owner = fixture.start(&["/bin/sh", "-c", "printf ready; read -r answer"]);
    let session = fixture.bound_session(&owner);
    let mut viewer = fixture.viewer(&owner, true);
    until_out(&mut viewer, "ready");
    viewer.input("unfinished draft");
    until_out(&mut viewer, "unfinished draft");
    drop(viewer);
    let task = complete_conversation(&fixture, &owner);
    retire_conversations(&fixture);
    fixture.ends(&owner);
    let (no_checkout, no_retirement): (bool, bool) = fixture.sql().query_row(
        "SELECT worktree IS NULL,NOT EXISTS(SELECT 1 FROM task_retirements WHERE task_id=tasks.id) FROM tasks WHERE id=?1",
        [&task], |r| Ok((r.get(0)?,r.get(1)?))).unwrap();
    assert!(no_checkout && no_retirement);
    assert!(fixture
        .last_screen(&session)
        .text()
        .unwrap()
        .contains("unfinished draft"));
}

#[test]
fn quiet_unviewed_conversations_ignore_the_obsolete_detached_limit() {
    let fixture = Fixture::new();
    std::fs::write(
        fixture.home.path().join("config.yaml"),
        "session: {end_detached_after: 1s}\n",
    )
    .unwrap();
    let owner = fixture.start(&["/bin/sh", "-c", "printf ready; read -r answer"]);
    let session = fixture.bound_session(&owner);
    fixture
        .sql()
        .execute(
            "UPDATE agent_sessions SET interactive=1 WHERE id=?1",
            [&session],
        )
        .unwrap();
    let mut viewer = fixture.viewer(&owner, false);
    until_out(&mut viewer, "ready");
    drop(viewer);
    fixture.wait_viewed(&owner, false);
    std::thread::sleep(Duration::from_secs(5));
    assert!(fixture.running(&owner));
    let mut returned = fixture.viewer(&owner, true);
    returned.input("done\n");
    returned.finish();
    fixture.ends(&owner);
}

/// An upgrade preserves a running provider's saved callback contract and identity.
#[test]
fn retained_hooks_survive_upgrade() {
    let fixture = Fixture::new();
    // Hooks reach the Machine server, which records their turns.
    let _server = fixture.machine_server();
    let path = fixture.provider(
        r#"#!/usr/bin/env python3
import json, os, pathlib, subprocess, sys, time
if sys.argv[1:] == ["--version"]:
    print("2.1.302")
    sys.exit(0)
home = pathlib.Path(os.environ["HOME"])
settings = json.loads(pathlib.Path(sys.argv[sys.argv.index("--settings") + 1]).read_text())
# Keep the declaration an older lf installed, including its executable address.
for groups in settings["hooks"].values():
    for group in groups:
        for handler in group["hooks"]:
            handler["command"] = handler["command"].replace(os.environ["LF_BIN"], os.environ["HOOK_ENTRY"])
stop = settings["hooks"]["Stop"][0]["hooks"][0]
stop["command"] = stop["command"].replace("__context-block --moment stop", "__planning-check")
def mark(name, text="ready"):
    pending = home / (name + ".tmp")
    pending.write_text(text)
    pending.replace(home / name)
def hook(event, group=0, **extra):
    command = settings["hooks"][event][group]["hooks"][0]["command"]
    payload = dict(session_id="retained-native-thread", hook_event_name=event, **extra)
    result = subprocess.run(command, shell=True, input=json.dumps(payload).encode(), capture_output=True)
    if result.returncode:
        raise RuntimeError(result.stderr.decode())
    return result
def wait(name):
    deadline = time.monotonic() + 25
    while not (home / name).exists():
        if time.monotonic() > deadline: raise RuntimeError("missing " + name)
        time.sleep(0.02)
hook("SessionStart", source="startup")
hook("UserPromptSubmit")
hook("Stop", background_tasks=[], session_crons=[])
hook("UserPromptSubmit")
mark("before-upgrade", str(os.getpid()))
wait("upgrade-ready")
# These are the same in-memory declarations and inherited environment.
hook("SessionStart", group=1, source="compact")
hook("Stop", agent_id="child", background_tasks=[], session_crons=[])
mark("child-stopped")
wait("stop-parent")
result = hook("Stop", background_tasks=[], session_crons=[])
assert result.stderr == b"", result.stderr
# A real delivery failure must not be turned into a successful retired hook.
missing = home / "missing-delivery.json"
env = dict(os.environ, LF_CONTEXT_DELIVERY=str(missing))
failed = subprocess.run(stop["command"], shell=True, env=env,
    input=json.dumps(dict(session_id="retained-native-thread", hook_event_name="Stop")).encode(), capture_output=True)
assert failed.returncode != 0, failed
assert str(missing).encode() in failed.stderr, failed.stderr
mark("after-upgrade", str(os.getpid()))
wait("finish-upgrade")
"#,
    );
    let entry = fixture.home.path().join("bin/lf-entry");
    // Model the old command spelling at the installed address, then replace
    // that address while its provider stays alive. No installed Home is touched.
    let candidate = env!("CARGO_BIN_EXE_lf");
    std::fs::write(
        &entry,
        format!(
            "#!/bin/sh\nif [ \"$1\" = __planning-check ]; then shift; exec '{candidate}' __context-block --moment stop \"$@\"; fi\nexec '{candidate}' \"$@\"\n"
        ),
    )
    .unwrap();
    std::fs::set_permissions(&entry, std::fs::Permissions::from_mode(0o755)).unwrap();
    let mut command = fixture.native(Path::new(candidate), &path, "retained request");
    command.env("HOOK_ENTRY", &entry);
    let owner = fixture.owner(command);
    let mut viewer = fixture.viewer(&owner, false);
    let before_pid = fixture.wait_file("before-upgrade");
    let sql = fixture.sql();
    let identity = || {
        sql.query_row(
            "SELECT s.id,s.provider_thread,p.id,p.pid,p.attachment_token,s.current_capture
             FROM agent_sessions s JOIN processes p ON p.id=s.agent_process_id
             WHERE s.transport_lf_process_id=?1",
            [&owner],
            |r| {
                Ok((
                    r.get::<_, String>(0)?,
                    r.get::<_, String>(1)?,
                    r.get::<_, String>(2)?,
                    r.get::<_, i64>(3)?,
                    r.get::<_, String>(4)?,
                    r.get::<_, i64>(5)?,
                ))
            },
        )
        .unwrap()
    };
    let before = identity();
    let yielded = || {
        sql.query_row(
            "SELECT yielded FROM session_activity WHERE session_id=?1",
            [&before.0],
            |r| r.get::<_, bool>(0),
        )
        .unwrap()
    };
    assert!(!yielded());
    let replacement = entry.with_extension("next");
    std::os::unix::fs::symlink(candidate, &replacement).unwrap();
    std::fs::rename(replacement, &entry).unwrap();
    std::fs::write(fixture.home.path().join("upgrade-ready"), "ready").unwrap();
    fixture.wait_file("child-stopped");
    assert!(!yielded(), "a subagent must not settle the retained turn");
    std::fs::write(fixture.home.path().join("stop-parent"), "ready").unwrap();
    assert_eq!(fixture.wait_file("after-upgrade"), before_pid);
    assert!(yielded(), "the retained Stop hook must run, not be skipped");
    assert_eq!(
        identity(),
        before,
        "upgrade must retain the conversation and fence"
    );
    std::fs::write(fixture.home.path().join("finish-upgrade"), "ready").unwrap();
    viewer.finish();
    fixture.ends(&owner);
}

/// Reconnect keeps a live turn in its old provider; after settlement the
/// viewer follows the new build and completes more turns in the same history.
#[test]
fn resumed_session_completes_turns_after_build_replacement_and_viewer_return() {
    continue_session_after_upgrade(false);
}

#[test]
fn unanswering_superseded_owner_preserves_pending_turn_and_queued_input() {
    continue_session_after_upgrade(true);
}

fn continue_session_after_upgrade(unanswering: bool) {
    let fixture = Fixture::new();
    // Hooks reach the Machine server, which records their turns.
    let _server = fixture.machine_server();
    let path = fixture.provider(
        r#"#!/usr/bin/env python3
import json, os, pathlib, subprocess, sys, time, tty
if sys.argv[1:] == ["--version"]:
    print("2.1.317")
    sys.exit(0)
home = pathlib.Path(os.environ["HOME"])
thread = "upgrade-native-thread"
history_file = home / "native-history.json"
resumed = "--resume" in sys.argv
if resumed:
    assert sys.argv[sys.argv.index("--resume") + 1] == thread, sys.argv
    assert "original request" not in sys.argv, sys.argv
    history = json.loads(history_file.read_text())
    assert history == ["original request", "retained draft", "pending draft"], history
else:
    assert not history_file.exists(), "conversation was recreated"
    history = []
settings = json.loads(pathlib.Path(sys.argv[sys.argv.index("--settings") + 1]).read_text())
def mark(name, value):
    pending = home / (name + ".tmp")
    pending.write_text(value)
    pending.replace(home / name)
def hook(event, **extra):
    command = settings["hooks"][event][0]["hooks"][0]["command"]
    payload = dict(session_id=thread, hook_event_name=event, **extra)
    subprocess.run(command, shell=True, input=json.dumps(payload).encode(),
                   stdout=subprocess.DEVNULL, check=True)
def turn(prompt):
    prompt_id = "turn-" + str(len(history))
    hook("UserPromptSubmit", prompt_id=prompt_id, prompt=prompt)
    if prompt == "retained draft":
        mark("pending-turn", prompt)
        deadline = time.monotonic() + 60
        while not (home / "release-turn").exists():
            assert time.monotonic() < deadline, "pending turn was never released"
            time.sleep(0.02)
    history.append(prompt)
    mark("native-history.json", json.dumps(history))
    hook("Stop", prompt_id=prompt_id, background_tasks=[], session_crons=[])
    mark("completed-" + str(len(history)), prompt)
    os.write(1, ("\r\nCOMPLETED:" + prompt + "\r\n").encode())
hook("SessionStart", source="resume" if resumed else "startup")
if not resumed:
    turn("original request")
terminal = os.open("/dev/tty", os.O_RDWR)
tty.setraw(terminal)
mark("ready-" + ("2" if resumed else "1"), str(os.getpid()))
draft = bytearray()
while True:
    key = os.read(terminal, 1)
    if key == b'\x04': break
    if key == b'\r':
        turn(draft.decode())
        draft.clear()
    elif key == b'\x7f':
        del draft[-1:]
    else:
        draft.extend(key)
    mark("draft", draft.decode())
    os.write(1, b'\r\nEDITOR:' + draft + b'\r\n')
"#,
    );
    let bin = fixture.home.path().join("bin");
    let previous = bin.join("lf-before");
    std::fs::copy(env!("CARGO_BIN_EXE_lf"), &previous).unwrap();
    let entry = bin.join("lf");
    std::os::unix::fs::symlink(&previous, &entry).unwrap();
    let mut command = fixture.native(&entry, &path, "original request");
    command.env("LF_BIN", &entry);
    let owner = fixture.owner(command);
    // Native launch and its hooks finish before starting the viewer's I/O deadline.
    fixture.wait_file("ready-1");
    let mut first = fixture.viewer(&owner, true);
    let session = fixture.session_of_owner(&owner);
    let store = loopflow::store::sqlite::SqliteStore::new(&fixture.home.path().join("loopflow.db"))
        .unwrap();
    let task = store
        .create_task(&loopflow::work::task::NewTask {
            id: loopflow::durable::TaskId::new(),
            repository_id: store
                .ensure_repository(
                    &loopflow::repository::CanonicalRepo::discover(fixture.repo.path())
                        .unwrap()
                        .to_string(),
                )
                .unwrap(),
            project_id: None,
            title: "Continue the existing conversation".into(),
            description: "Preserve recorded work across upgrade".into(),
            due_date: None,
        })
        .unwrap();
    let capture = store.session(&session).unwrap().unwrap().captured;
    store.bind_session(&session, capture, &task.id).unwrap();
    let continuity = || {
        fixture
            .sql()
            .query_row(
                "SELECT current_capture,provider_thread,task_id FROM agent_sessions WHERE id=?1",
                [&session],
                |r| {
                    Ok((
                        r.get::<_, i64>(0)?,
                        r.get::<_, String>(1)?,
                        r.get::<_, String>(2)?,
                    ))
                },
            )
            .unwrap()
    };
    let attachment = || store.session_attachment(&session).unwrap().unwrap();
    let original = continuity();
    let before = attachment();
    // A native turn holds the provider while later input waits in its terminal.
    first.input("retained drafx\x7ft\rpending draft");
    assert_eq!(fixture.wait_file("pending-turn"), "retained draft");
    fixture.wait_viewed(&owner, true);
    first.socket.shutdown(std::net::Shutdown::Both).unwrap();
    drop(first);
    // Pausing during that record's write would leave the store locked.
    fixture.wait_viewed(&owner, false);

    let next = bin.join("lf-after");
    other_build(&next);
    let swapped = bin.join("lf-swap");
    std::os::unix::fs::symlink(&next, &swapped).unwrap();
    std::fs::rename(&swapped, &entry).unwrap();
    let resume = || {
        let mut command = fixture.command_from(&entry, &["session", "resume", &session]);
        command.env("LF_BIN", &entry).env("PATH", &path);
        Terminal::spawn(command)
    };
    if unanswering {
        let paused = fixture.pause(&owner);
        let mut failed = resume();
        let status = failed.wait_exit();
        drop(paused);
        assert!(!status.success(), "{status:?}");
        assert_eq!(
            attachment(),
            before,
            "failed attachment replaced the provider"
        );
        assert_eq!(continuity(), original);
        assert!(fixture.running(&owner), "failed attachment ended the owner");
        assert!(!fixture.home.path().join("completed-2").exists());
    }
    let mut terminal = resume();
    fixture.wait_controlled(&owner);
    assert_eq!(
        attachment(),
        before,
        "reconnect must retain the live turn's provider"
    );
    assert_eq!(continuity(), original);
    std::fs::write(fixture.home.path().join("release-turn"), "").unwrap();
    assert_eq!(fixture.wait_file("completed-2"), "retained draft");
    // The bytes queued behind the pending request become the next draft.
    let deadline = Instant::now() + Duration::from_secs(10);
    while fixture.wait_file("draft") != "pending draft" {
        assert!(Instant::now() < deadline, "queued draft was lost");
        std::thread::sleep(Duration::from_millis(20));
    }
    terminal.input(b"\r");
    assert_eq!(fixture.wait_file("completed-3"), "pending draft");
    assert_eq!(attachment(), before);
    assert_eq!(continuity(), original);
    // Settlement leaves the old owner running until the next explicit resume.
    assert!(fixture.running(&owner), "a superseded owner ended itself");
    terminal.input(b"\x1d");
    let status = terminal.wait_exit();
    assert!(status.success(), "{status:?}");
    terminal = resume();
    fixture.wait_file("ready-2");
    fixture.ends(&owner);
    let replacement = fixture.owner_of(&session);
    assert_ne!(replacement, owner);
    fixture.wait_controlled(&replacement);
    let after = attachment();
    assert_ne!(after.agent_process_id, before.agent_process_id);
    assert_ne!(after.token, before.token);
    terminal.input(b"after upgrade\r");
    assert_eq!(fixture.wait_file("completed-4"), "after upgrade");
    terminal.input(b"\x1d");
    let status = terminal.wait_exit();
    assert!(status.success(), "{status:?}");
    let mut returned = resume();
    fixture.wait_controlled(&replacement);
    returned.input(b"after return\r");
    assert_eq!(fixture.wait_file("completed-5"), "after return");
    assert_eq!(
        attachment(),
        after,
        "viewer return must not replace the provider"
    );
    assert_eq!(continuity(), original);
    let history: Value = serde_json::from_str(&fixture.wait_file("native-history.json")).unwrap();
    assert_eq!(
        history,
        json!([
            "original request",
            "retained draft",
            "pending draft",
            "after upgrade",
            "after return"
        ])
    );
    returned.input(b"\x04");
    let status = returned.wait_exit();
    assert!(status.success(), "{status:?}");
    fixture.ends(&replacement);
}

/// Native start/stop callbacks protect live work during completed-Task cleanup.
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
    // Hooks reach the Machine server, which records their turns.
    let _server = fixture.machine_server();
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
    # Missing registry evidence and child completion cannot settle that subscription.
    hook("Stop")
    hook("Stop", agent_id="child", background_tasks=[], session_crons=[])
(home / (name + "-started")).write_text("started")
while not (home / (name + "-settle")).exists():
    time.sleep(0.05)
if background:
    # The shell subscription finished, but a scheduled wakeup still owns work.
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
        complete_conversation(&fixture, &owner);
        owners.push(owner);
    }
    // Cleanup signals synchronously; its return is the lifetime decision.
    retire_conversations(&fixture);
    for owner in &owners {
        assert!(
            fixture.running(owner),
            "silent native turn ended while unviewed"
        );
    }
    // A viewed idle terminal cannot settle a quiet supervisor's native turn.
    let viewed = fixture.viewer(&owners[1], false);
    fixture
        .sql()
        .execute(
            r#"UPDATE session_activity SET program_status='{"records":[{"state":"idle"}]}'
           WHERE session_id=(SELECT id FROM agent_sessions WHERE transport_lf_process_id=?1)"#,
            [&owners[1]],
        )
        .unwrap();
    std::fs::remove_file(&removable).unwrap();
    retire_conversations(&fixture);
    for owner in &owners {
        assert!(
            fixture.running(owner),
            "silent native turn ended with its executable"
        );
    }
    drop(viewed);
    complete_conversation(&fixture, &owners[1]);
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
                 SELECT id,'supervised','{\"name\":\"supervised\",\"steps\":[]}' FROM processes WHERE parent_lf_process_id=?1 LIMIT 1
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
            retire_conversations(&fixture);
            assert!(fixture.running(owner), "scheduled native work ended");
            std::fs::write(fixture.home.path().join(format!("{name}-unschedule")), "").unwrap();
        }
        fixture.wait_file(&format!("{name}-settled"));
    }
    if !background {
        retire_conversations(&fixture);
        for owner in &owners {
            assert!(fixture.running(owner), "quiet Graph supervisor ended");
        }
        for graph in graphs {
            sql.execute("UPDATE processes SET completed_at=1 WHERE id=?1", [graph])
                .unwrap();
        }
    }
    // Completed Tasks end once work settles and viewing has expired.
    retire_conversations(&fixture);
    for owner in &owners {
        fixture.ends(owner);
    }
}

/// A conversation whose lf executable is deleted, rebuilt or superseded keeps
/// serving on the build it started with: nothing but a person quitting, `lf
/// resume`, rollover, Task closure or an explicit end stops an owner, and
/// its viewers are never told to move. `lf resume` through the installed entry
/// is the person's own decision: it ends the superseded idle owner and reopens
/// the conversation with its history on the installed lf.
#[test]
fn sessions_keep_serving_when_their_lf_executable_goes() {
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
    let entry = bin.join("lf-entry");
    std::os::unix::fs::symlink(bin.join("lf-installed"), &entry).unwrap();
    let removed = build("lf-idle");
    let replaced = build("lf-replaced");
    // (executable, the entry it was launched through)
    let launched = [
        (removed.clone(), entry.clone()),
        (replaced.clone(), replaced.clone()),
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
    fixture.launches(owners.len());
    let sessions: Vec<String> = owners
        .iter()
        .map(|owner| fixture.session_of_owner(owner))
        .collect();

    // A worktree removed, a build rebuilt in place, `lf install` selecting
    // another build.
    std::fs::remove_file(&removed).unwrap();
    let rebuilt = bin.join("lf-rebuilt");
    other_build(&rebuilt);
    std::fs::rename(&rebuilt, &replaced).unwrap();
    let next = bin.join("lf-next");
    other_build(&next);
    let swapped = bin.join("lf-swapped");
    std::os::unix::fs::symlink(&next, &swapped).unwrap();
    std::fs::rename(&swapped, &installed).unwrap();

    // Several policy polls later every owner is still serving.
    std::thread::sleep(Duration::from_secs(8));
    for (index, owner) in owners.iter().enumerate() {
        assert!(
            fixture.running(owner),
            "owner {index} ended after its executable went"
        );
    }
    // The person resumes the superseded conversation through the installed
    // entry: that ends its owner and reopens it with history on the installed lf.
    let mut command = fixture.command_from(&installed, &["session", "resume", &sessions[2]]);
    command.env("LF_BIN", &installed).env("PATH", &path);
    let mut terminal = Terminal::spawn(command);
    let line = fixture.launches(owners.len() + 1).pop().unwrap();
    assert!(
        line.contains(" --resume ") && session_of(&fixture.sql(), &line) == sessions[2],
        "reopened without history: {line}"
    );
    let reopened = fixture.owner_of(&sessions[2]);
    assert_ne!(reopened, owners[2]);
    assert!(
        !fixture.running(&owners[2]),
        "resume left the superseded owner"
    );
    terminal.assert_viewing("terminal 2");
    for owner in &owners[..2] {
        assert!(fixture.running(owner), "resume of another Session ended it");
    }
    terminal.stop();
    for owner in owners[..2].iter().chain([&reopened]) {
        fixture.stop(owner);
    }
    // No viewer of an untouched owner was ever told to move.
    for (index, viewer) in viewers[..2].iter_mut().enumerate() {
        assert!(!viewer.moved(), "viewer {index} was moved");
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
    /// A byte-stream relay, with neither an owner PTY nor a local terminal.
    fn stream(mut command: Command) -> Self {
        let (mut reader, writer) = UnixStream::pair().unwrap();
        command
            .stdin(Stdio::piped())
            .stdout(std::os::fd::OwnedFd::from(writer.try_clone().unwrap()))
            .stderr(std::os::fd::OwnedFd::from(writer));
        let child = command.spawn().unwrap();
        drop(command);
        let screen = std::sync::Arc::new(std::sync::Mutex::new(Vec::new()));
        let written = screen.clone();
        std::thread::spawn(move || {
            let mut buffer = [0; 4096];
            while let Ok(read @ 1..) = reader.read(&mut buffer) {
                written.lock().unwrap().extend_from_slice(&buffer[..read]);
            }
        });
        Self { child, screen }
    }

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

    fn input(&mut self, bytes: &[u8]) {
        self.child.stdin.as_mut().unwrap().write_all(bytes).unwrap();
    }

    fn wait_text(&mut self, text: &str) {
        let deadline = Instant::now() + Duration::from_secs(20);
        loop {
            let screen = self.screen.lock().unwrap().clone();
            if String::from_utf8_lossy(&screen).contains(text) {
                return;
            }
            assert!(
                Instant::now() < deadline,
                "missing {text:?}: {}",
                String::from_utf8_lossy(&screen)
            );
            std::thread::sleep(Duration::from_millis(20));
        }
    }

    fn wait_exit(&mut self) -> std::process::ExitStatus {
        let deadline = Instant::now() + Duration::from_secs(20);
        loop {
            if let Some(status) = self.child.try_wait().unwrap() {
                return status;
            }
            assert!(Instant::now() < deadline, "relay did not exit");
            std::thread::sleep(Duration::from_millis(20));
        }
    }
}

impl Drop for Terminal {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

/// A Session whose owner runs another lf build than this test binary, with
/// that build as its entry: `(owner, session)`.
fn owner_on_another_build(fixture: &Fixture, path: &str) -> (String, String) {
    let other = fixture.home.path().join("bin/lf-other");
    other_build(&other);
    let mut command = fixture.native(&other, path, "hello");
    command.env("LF_BIN", &other);
    let owner = fixture.owner(command);
    fixture.launches(1);
    let session = fixture.session_of_owner(&owner);
    (owner, session)
}

/// An idle owner on another build ends; resume continues its saved conversation.
#[test]
fn resume_takes_over_an_owner_from_another_build() {
    let fixture = Fixture::new();
    let path = fixture.waiting_provider();
    let (previous, session) = owner_on_another_build(&fixture, &path);
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
    let current = fixture.owner_of(&session);
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
}

/// Socket silence cannot turn retained work into permission to replace a provider.
fn unanswering_superseded_owner_keeps(protection: &str) {
    let fixture = Fixture::new();
    let path = fixture.waiting_provider();
    let (owner, session) = owner_on_another_build(&fixture, &path);
    let viewer = fixture.viewer(&owner, true);
    let sql = fixture.sql();
    match protection {
        "request" | "turn" => {
            fixture.mark_working(&session);
            if protection == "request" {
                sql.execute(
                    "UPDATE session_activity SET open_tools=0,pending_input=1,yielded=1 WHERE session_id=?1",
                    [&session],
                ).unwrap();
            }
        }
        "graph" | "step" => {
            let graph = loopflow::id::LfProcessId::new();
            sql.execute(
                "INSERT INTO processes(id,trace_id,started_at,parent_lf_process_id) VALUES(?1,'fixture',1,?2)",
                rusqlite::params![graph, (protection == "graph").then_some(&owner)],
            ).unwrap();
            sql.execute(
                "INSERT INTO graph_processes(lf_process_id,graph,compiled) VALUES(?1,'fixture','{}')",
                [&graph],
            ).unwrap();
            if protection == "step" {
                sql.execute(
                    "UPDATE processes SET parent_lf_process_id=?1 WHERE id=(
                     SELECT e.lf_process_id FROM agent_sessions s
                     JOIN session_events e ON e.seq=s.current_capture WHERE s.id=?2)",
                    rusqlite::params![graph, session],
                )
                .unwrap();
                sql.execute(
                    "INSERT INTO graph_process_steps(graph_lf_process_id,lf_process_id,node,iterations)
                     SELECT ?1,e.lf_process_id,0,'[]' FROM agent_sessions s
                     JOIN session_events e ON e.seq=s.current_capture WHERE s.id=?2",
                    rusqlite::params![graph, session],
                ).unwrap();
            }
        }
        "run" => {
            sql.execute(
                "UPDATE agent_sessions SET interactive=0 WHERE id=?1",
                [&session],
            )
            .unwrap();
        }
        _ => unreachable!(),
    }
    let identity = || {
        sql.query_row(
            "SELECT s.agent_process_id,p.attachment_token,s.current_capture,s.provider_thread
             FROM agent_sessions s JOIN processes p ON p.id=s.agent_process_id
             WHERE s.id=?1",
            [&session],
            |r| {
                Ok((
                    r.get::<_, String>(0)?,
                    r.get::<_, String>(1)?,
                    r.get::<_, i64>(2)?,
                    r.get::<_, Option<String>>(3)?,
                ))
            },
        )
        .unwrap()
    };
    let before = identity();
    drop(viewer);
    let paused = fixture.pause(&owner);
    let resumed = fixture
        .command(&["session", "resume", &session])
        .env("PATH", &path)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    let output = wait_for_viewer(resumed, Duration::from_secs(20));
    let after = identity();
    let running = fixture.running(&owner);
    let launches = fixture.launches(1);
    drop(paused);
    // Finish only this fixture after observing the refused replacement.
    fixture.stop(&owner);
    fixture.ends(&owner);
    assert!(!output.status.success(), "{protection}: {output:?}");
    assert!(running, "{protection}: resume ended the protected owner");
    assert_eq!(after, before, "{protection}: resume changed retained state");
    assert_eq!(
        launches.len(),
        1,
        "{protection}: resume launched another provider"
    );
}

#[test]
fn unanswering_superseded_owners_preserve_a_request() {
    unanswering_superseded_owner_keeps("request");
}

#[test]
fn unanswering_superseded_owners_preserve_a_turn() {
    unanswering_superseded_owner_keeps("turn");
}

#[test]
fn unanswering_superseded_owners_preserve_a_graph() {
    unanswering_superseded_owner_keeps("graph");
}

#[test]
fn unanswering_superseded_owners_preserve_a_graph_step() {
    unanswering_superseded_owner_keeps("step");
}

#[test]
fn unanswering_superseded_owners_preserve_a_run() {
    unanswering_superseded_owner_keeps("run");
}

/// A completed Session names no known work, so `lf resume` still gets you back
/// from a superseded owner that cannot answer.
#[test]
fn resume_ends_an_unanswering_superseded_owner_of_a_completed_session() {
    let fixture = Fixture::new();
    let path = fixture.waiting_provider();
    let (previous, session) = owner_on_another_build(&fixture, &path);
    fixture.complete(&session);
    let paused = fixture.pause(&previous);
    let mut viewer = fixture
        .command(&["session", "resume", &session])
        .env("PATH", &path)
        .env_remove("RUST_LOG")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    // A stopped owner holds SIGTERM until it continues, so it continues once resume
    // has seen the silence; the signal then ends it whichever arrives first.
    let log = fixture.home.path().join("logs/lf.log");
    let deadline = Instant::now() + Duration::from_secs(20);
    while !std::fs::read_to_string(&log)
        .is_ok_and(|text| text.contains("Session owner does not answer"))
    {
        assert!(Instant::now() < deadline, "resume never saw the silence");
        std::thread::sleep(Duration::from_millis(20));
    }
    drop(paused);
    let launches = fixture.launches(2);
    fixture.ends(&previous);
    let current = fixture.owner_of(&session);
    viewer.kill().unwrap();
    let output = viewer.wait_with_output().unwrap();
    assert_ne!(current, previous);
    assert_eq!(launches.len(), 2, "resume launched a second replacement");
    assert!(launches[1].contains(" --resume "), "{launches:?}");
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("Moved"), "{stderr}");
}

/// An answering owner may hold a turn or draft nothing records: a viewer reaches
/// it, so resume attaches and ends nothing.
#[test]
fn resume_attaches_an_answering_superseded_owner_of_a_completed_session() {
    let fixture = Fixture::new();
    let path = fixture.waiting_provider();
    let (owner, session) = owner_on_another_build(&fixture, &path);
    fixture.complete(&session);
    let mut viewer = fixture
        .command(&["session", "resume", &session])
        .env("PATH", &path)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    fixture.wait_controlled(&owner);
    let running = fixture.running(&owner);
    let launches = fixture.launches(1);
    viewer.kill().unwrap();
    viewer.wait().unwrap();
    fixture.stop(&owner);
    fixture.ends(&owner);
    assert!(running, "resume ended an answering owner");
    assert_eq!(launches.len(), 1, "resume launched another provider");
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
    let viewed = || -> Option<bool> {
        fixture
            .sql()
            .query_row("SELECT viewed FROM processes WHERE id=?1", [&owner], |r| {
                r.get(0)
            })
            .unwrap()
    };
    let desktop = fixture.viewer(&owner, true);
    let deadline = Instant::now() + Duration::from_secs(10);
    while viewed() != Some(true) {
        assert!(Instant::now() < deadline, "owner never recorded its viewer");
        std::thread::sleep(Duration::from_millis(20));
    }
    // Desktop quits.
    desktop.socket.shutdown(std::net::Shutdown::Both).unwrap();
    drop(desktop);
    let deadline = Instant::now() + Duration::from_secs(10);
    while viewed() != Some(false) {
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
        json!([
            {"kind":"open","label":"Reconnect","help":"Show the running Session in this terminal","unavailable_reason":null},
            {"kind":"reveal","label":"Show conversation","help":"Continue existing history without taking another terminal's input","unavailable_reason":null}
        ])
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

/// An idle owner from an earlier lf can stay alive after an upgrade yet never
/// answer its socket. Opening its Session neither waits on it nor gives up: the stale
/// viewer record reads `detached`, and reconnecting stops its provider and
/// continues the same provider thread in a new owner, without replaying input.
#[test]
fn opening_takes_over_from_an_owner_that_never_answers() {
    let fixture = Fixture::new();
    let path = fixture.waiting_provider();
    let owner = fixture.owner(fixture.native(
        Path::new(env!("CARGO_BIN_EXE_lf")),
        &path,
        "one original input",
    ));
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
}

#[test]
fn codex_original_request_channel_and_turn_survive_all_viewers() {
    let fixture = Fixture::new();
    let bin = fixture.home.path().join("bin");
    std::fs::create_dir(&bin).unwrap();
    support::install_native_codex(&bin);
    let path = format!("{}:/usr/bin:/bin", bin.display());
    std::fs::write(fixture.home.path().join("wait-approval"), "").unwrap();
    // A scope's conversation, held by its owner from the start; no viewer.
    let mut owner = fixture.serve(
        true,
        &[
            env!("CARGO_BIN_EXE_lf"),
            "-i",
            "--agent",
            "codex",
            ":",
            "Original input",
        ],
    );
    owner.env("PATH", &path);
    let owner = fixture.owner(owner);
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
fn a_headless_run_survives_without_viewers() {
    headless_run(&["skill", "probe", "--message", "go"], false);
}

fn headless_run(command: &[&str], resume: bool) {
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
    // Several lifetime polls without viewers still leave the Run's turn alive.
    std::thread::sleep(Duration::from_secs(4));
    assert_eq!(
        outcome(&step),
        None,
        "an unviewed Run ended before its work settled"
    );
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
fn first_snapshot_uses_the_new_viewers_dimensions() {
    let fixture = Fixture::new();
    let owner = fixture.start(&[
        "/bin/sh", "-c",
        "i=1; while [ $i -le 20 ]; do printf 'line %s: abcdefghijklmnopqrstuvwxyz0123456789\\r\\n' \"$i\"; i=$((i+1)); done; printf 'ready> '; read -r answer",
    ]);
    let mut first = fixture.viewer(&owner, true);
    let mut expected = ghostty_vt::Terminal::new(80, 24).unwrap();
    for item in until_out(&mut first, "ready> ") {
        if let Item::Snapshot(bytes) | Item::Out(bytes) = item {
            expected.write(&bytes);
        }
    }
    expected.resize(40, 12).unwrap();
    let mut second = fixture.attach(
        &owner,
        json!({
            "kind":"attach", "control":true, "replace":true, "own":"take", "rows":12, "cols":40,
        }),
    );
    let Some(Item::Snapshot(snapshot)) = second.next() else {
        panic!("fresh viewer must start with its screen");
    };
    let mut display = ghostty_vt::Terminal::new(40, 12).unwrap();
    display.write(&snapshot);
    assert_eq!(display.text().unwrap(), expected.text().unwrap());
    assert_eq!(display.cursor(), expected.cursor());
    second.input("done\n");
    second.finish();
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
fn resume_supersedes_its_open_socket_without_taking_a_newer_viewers_claims() {
    for own in ["request", "take"] {
        let fixture = Fixture::new();
        let owner = fixture.start(&["/bin/sh", "-c", ECHO_LINES]);
        fixture.wait_file("drawn");
        let mut first = fixture.viewer(&owner, true);
        first.until_event("owner");
        let mut back = fixture.connect(&owner);
        back.request(json!({"kind":"resume","reconnect":first.reconnect,
            "received":first.received,"control":true,"own":own,"rows":35,"cols":110}));
        let reply = back.until_reply();
        assert_eq!(reply["kind"], "attached");
        assert_eq!(reply["viewer"], first.id);
        assert!(!reply["token"].is_null());
        back.accept(&reply);
        back.input("size\n");
        until_out(&mut back, "35 110");
        // Closing the superseded socket cannot release the replacement's claims.
        drop(first);
        back.input("one\n");
        until_out(&mut back, "line one");
        let mut newer = fixture.viewer(&owner, true);
        newer.until_event("owner");
        newer.request(json!({"kind":"resize","rows":40,"cols":120}));
        newer.input("size\n");
        until_out(&mut newer, "40 120");
        let mut again = fixture.connect(&owner);
        again.request(json!({"kind":"resume","reconnect":back.reconnect,
            "received":back.received,"control":true,"own":own,"rows":20,"cols":70}));
        let reply = again.until_reply();
        assert_eq!(reply["kind"], "attached");
        assert!(reply["token"].is_null(), "{reply}");
        assert_eq!(reply["owner"], newer.id);
        again.accept(&reply);
        drop(back);
        newer.input("size\n");
        until_out(&mut newer, "40 120");
        newer.input("done\n");
        newer.finish();
        again.finish();
    }
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

/// Wait until `path` holds `len` bytes, the program having written them all.
fn wait_for_answer(fixture: &Fixture, len: usize) -> Vec<u8> {
    let answer = fixture.home.path().join("answer");
    let deadline = Instant::now() + Duration::from_secs(20);
    while !answer.exists() {
        if Instant::now() > deadline {
            let partial = std::fs::read(fixture.home.path().join("answer.tmp")).unwrap_or_default();
            panic!(
                "input lost; received {} of {len} bytes: {:?}",
                partial.len(),
                String::from_utf8_lossy(&partial[partial.len().saturating_sub(200)..])
            );
        }
        std::thread::sleep(Duration::from_millis(20));
    }
    std::fs::read(answer).unwrap()
}

/// A program that keeps the owner writing to its viewers while it collects
/// exactly `len` bytes of input into `$HOME/answer`.
fn collecting(len: usize) -> String {
    format!(
        "(while :; do echo tick; sleep 0.01; done) & \
         head -c {len} > \"$HOME/answer.tmp\"; mv \"$HOME/answer.tmp\" \"$HOME/answer\"; kill $!"
    )
}

/// Keystrokes sent just before a viewer closes reach the program, even when
/// the owner writes to the closed viewer before reading them.
#[test]
fn viewer_input_survives_its_disconnect() {
    let fixture = Fixture::new();
    const N: usize = 20000;
    let program = collecting(N);
    let owner = fixture.owner(fixture.serve(false, &["/bin/sh", "-c", &program]));
    let paused = fixture.pause(&owner);
    let mut socket = UnixStream::connect(fixture.endpoint(&owner)).unwrap();
    // The paused owner accepts nothing; room for every byte before it runs.
    let room: libc::c_int = 1 << 20;
    // SAFETY: the option value is a live c_int of the size given.
    assert_eq!(
        unsafe {
            libc::setsockopt(
                socket.as_raw_fd(),
                libc::SOL_SOCKET,
                libc::SO_SNDBUF,
                (&room as *const libc::c_int).cast(),
                std::mem::size_of::<libc::c_int>() as libc::socklen_t,
            )
        },
        0
    );
    let attach = json!({"id":1,"kind":"attach","control":true,"replace":true,
        "own":"take","rows":24,"cols":80});
    let mut bytes = frame(REQUEST, attach.to_string().as_bytes());
    let input: Vec<u8> = (0..N).map(|i| b'a' + (i % 26) as u8).collect();
    for chunk in input.chunks(1000) {
        bytes.extend(frame(INPUT, chunk));
    }
    socket.write_all(&bytes).unwrap();
    drop(socket);
    drop(paused);
    assert_eq!(wait_for_answer(&fixture, N), input);

    // Attaching, typing and closing at once, against a program that keeps the
    // owner writing, loses nothing.
    let fixture = Fixture::new();
    let rounds: Vec<String> = (0..50).map(|round| format!("round {round:02}\n")).collect();
    let program = collecting(rounds.iter().map(String::len).sum());
    let owner = fixture.owner(fixture.serve(false, &["/bin/sh", "-c", &program]));
    for round in &rounds {
        let mut socket = UnixStream::connect(fixture.endpoint(&owner)).unwrap();
        // Separate writes, so the owner can read the attach alone and reply
        // after the close, while the keystrokes still wait unread.
        socket
            .write_all(&frame(REQUEST, attach.to_string().as_bytes()))
            .unwrap();
        socket.write_all(&frame(INPUT, round.as_bytes())).unwrap();
    }
    let answer = wait_for_answer(&fixture, rounds.iter().map(String::len).sum());
    assert_eq!(String::from_utf8(answer).unwrap(), rounds.concat());
}

#[test]
fn completed_task_cleanup_signals_shell_owner_and_settles_jobs() {
    let fixture = Fixture::new();
    let task = "task_5e55105e000040008000000000000007";
    fixture.shell_task(task);
    let mut launch = fixture
        .command(&[
            "desktop",
            "shell",
            task,
            "--checkout",
            fixture.repo.path().to_str().unwrap(),
            "--id",
            "cleanup-shell",
            "--create",
            "--",
            "/bin/sh",
            "-c",
            "printf ready > \"$HOME/ready\"",
        ])
        .env("SHELL", "/bin/bash")
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .unwrap();
    fixture.wait_file("ready");
    let owner = fixture.shell_owner("cleanup-shell");
    // The launch attaches after the shell starts; take over only from it.
    fixture.wait_controlled(&owner);
    let mut viewer = fixture.viewer(&owner, true);
    launch.wait().unwrap();
    viewer.input("sleep 60 & printf '%s\\n' $! > \"$HOME/job\"\r");
    let job = fixture.wait_file("job").trim().parse::<u32>().unwrap();
    fixture.sql().execute("UPDATE tasks SET issue_title='Completed shell',issue_description='',workspace_slug='task',base_commit='head',updated_at=1,planning_completed=1,planning_completed_at='2000-01-01T00:00:00Z' WHERE id=?1", [task]).unwrap();
    fixture
        .sql()
        .execute(
            "INSERT INTO task_retirements(task_id,completion,checkout,branch,due_at,phase)
        SELECT id,planning_completed_at,worktree,branch,0,'waiting' FROM tasks WHERE id=?1",
            [task],
        )
        .unwrap();
    // The shell's own viewer holds neither its Task nor ordinary shell jobs.
    let cleanup = fixture.command(&["task", "retire", task]).output().unwrap();
    assert!(!cleanup.status.success(), "a PR-less checkout must remain");
    assert!(
        String::from_utf8_lossy(&cleanup.stderr).contains("Retained PR-less checkout"),
        "{cleanup:?}"
    );
    let ended = viewer.until_event("control");
    assert!(events(&ended, "control")
        .iter()
        .any(|event| event["notice"] == "Task completed"));
    fixture.ends(&owner);
    let (settled, requested): (bool, i64) = fixture
        .sql()
        .query_row(
            "SELECT jobs_settled,close_requested FROM desktop_shells WHERE owner=?1",
            [&owner],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )
        .unwrap();
    assert!(settled);
    assert_eq!(
        requested, 0,
        "automatic cleanup must not store a shell Close"
    );
    assert!(fixture.repo.path().is_dir());
    let status = Command::new("ps")
        .args(["-p", &job.to_string(), "-o", "stat="])
        .output()
        .unwrap();
    assert!(
        !status.status.success()
            || String::from_utf8_lossy(&status.stdout)
                .trim()
                .starts_with('Z')
    );
}

#[test]
fn returning_task_viewer_resets_cleanup_without_another_sweep() {
    let fixture = Fixture::new();
    let task = "task_5e55105e000040008000000000000006";
    fixture.shell_task(task);
    let base = [
        "desktop",
        "shell",
        task,
        "--checkout",
        fixture.repo.path().to_str().unwrap(),
        "--id",
        "desktop-shell",
    ];
    let mut launch = fixture
        .command(&base)
        .args([
            "--create",
            "--",
            "/bin/sh",
            "-c",
            "printf ready > \"$HOME/ready\"",
        ])
        .env("SHELL", "/bin/bash")
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .unwrap();
    fixture.wait_file("ready");
    let shell = fixture.shell_owner("desktop-shell");
    fixture.wait_controlled(&shell);
    let mut terminal = fixture.viewer(&shell, true);
    launch.wait().unwrap();
    terminal.input("export RETAINED=still-here; sleep 60 & printf '%s\\n' $! > \"$HOME/job\"\r");
    let job = fixture.wait_file("job").trim().parse::<u32>().unwrap();

    // This independent Session can be viewed passively, without controlling
    // either execution. Its viewers, rather than shell viewers, retain the Task.
    let session_owner = fixture.start(&["/bin/sleep", "60"]);
    let session = fixture.session(&session_owner);
    fixture
        .sql()
        .execute(
            "UPDATE agent_sessions SET task_id=?1,interactive=1 WHERE id=?2",
            rusqlite::params![task, session],
        )
        .unwrap();
    fixture
        .sql()
        .execute(
            "UPDATE tasks SET planning_completed=1,planning_completed_at='completion' WHERE id=?1",
            [task],
        )
        .unwrap();
    fixture
        .sql()
        .execute(
            "INSERT INTO task_retirements(task_id,completion,checkout,branch,due_at,phase)
        SELECT id,planning_completed_at,worktree,branch,0,'waiting' FROM tasks WHERE id=?1",
            [task],
        )
        .unwrap();

    for brief_return in [false, true] {
        fixture.sql().execute_batch("UPDATE task_retirements SET due_at=0,phase='closing',retry_at=unixepoch()+3600,failures=3;").unwrap();
        let mut passive = fixture.viewer(&session_owner, false);
        wait_hold(&fixture, &session_owner, Some("viewed"));
        let state: (String, Option<i64>, Option<i64>, i64, i64) = fixture
            .sql()
            .query_row(
                "SELECT h.phase,h.due_at,h.retry_at,h.failures,s.close_requested
             FROM task_retirements h JOIN desktop_shells s ON s.task_id=h.task_id",
                [],
                |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?, r.get(4)?)),
            )
            .unwrap();
        assert_eq!(state, ("waiting".into(), None, None, 0, 0));
        if brief_return {
            passive.request(json!({"kind":"leave"}));
            wait_hold(&fixture, &session_owner, Some("absent"));
        }
        // No sweep occurs between return and these assertions.
        assert!(fixture.running(&shell));
        terminal.input("printf 'value:%s\\n' \"$RETAINED\"\r");
        until_out(&mut terminal, "value:still-here");
        let job_state = Command::new("ps")
            .args(["-p", &job.to_string(), "-o", "stat="])
            .output()
            .unwrap();
        assert!(job_state.status.success());
        assert!(!String::from_utf8_lossy(&job_state.stdout)
            .trim()
            .starts_with('Z'));
        if !brief_return {
            passive.request(json!({"kind":"leave"}));
            wait_hold(&fixture, &session_owner, Some("absent"));
        }
    }
    // An explicit Close still ends this shell with a passive Task viewer present.
    let _passive = fixture.viewer(&session_owner, false);
    wait_hold(&fixture, &session_owner, Some("viewed"));
    assert!(fixture
        .command(&base)
        .arg("--close")
        .output()
        .unwrap()
        .status
        .success());
    fixture.ends(&shell);
    let settled: bool = fixture
        .sql()
        .query_row(
            "SELECT jobs_settled FROM desktop_shells WHERE owner=?1",
            [&shell],
            |r| r.get(0),
        )
        .unwrap();
    assert!(settled);
}

impl Fixture {
    fn shell_task(&self, task: &str) -> loopflow::durable::MachineId {
        let output = self.command(&["machine", "id"]).output().unwrap();
        assert!(output.status.success(), "{output:?}");
        let store =
            loopflow::store::sqlite::SqliteStore::new(&self.home.path().join("loopflow.db"))
                .unwrap();
        let repo = store
            .ensure_repository(self.repo.path().to_str().unwrap())
            .unwrap();
        let machine = store.local_machine().unwrap().id;
        self.sql().execute(
            "INSERT INTO tasks(id,repository_id,issue_identifier,created_at,worktree,checkout_machine_id,branch)
             VALUES(?1,?2,'SHELL-1',1,?3,?4,'task')",
            rusqlite::params![task,repo.as_str(),self.repo.path().to_str().unwrap(),machine.as_str()],
        ).unwrap();
        machine
    }

    fn shell_owner(&self, id: &str) -> String {
        self.sql()
            .query_row("SELECT owner FROM desktop_shells WHERE id=?1", [id], |r| {
                r.get(0)
            })
            .unwrap()
    }

    fn shell_count(&self) -> u32 {
        self.sql()
            .query_row("SELECT count(*) FROM desktop_shells", [], |r| r.get(0))
            .unwrap()
    }
}

fn desktop_shell_bootstrap() -> String {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../swift/LoopflowMac/GhosttyResources/share/ghostty");
    let mut files = std::collections::BTreeMap::new();
    for (destination, source) in [
        (".zshenv", "task-zshenv"),
        ("bash-env", "task-bash-env"),
        (
            "ghostty/shell-integration/zsh/.zshenv",
            "shell-integration/zsh/.zshenv",
        ),
        (
            "ghostty/shell-integration/zsh/ghostty-integration",
            "shell-integration/zsh/ghostty-integration",
        ),
        (
            "ghostty/shell-integration/bash/ghostty.bash",
            "shell-integration/bash/ghostty.bash",
        ),
        (
            "ghostty/shell-integration/bash/bash-preexec.sh",
            "shell-integration/bash/bash-preexec.sh",
        ),
        (
            "ghostty/shell-integration/fish/vendor_conf.d/ghostty-shell-integration.fish",
            "shell-integration/fish/vendor_conf.d/ghostty-shell-integration.fish",
        ),
    ] {
        files.insert(
            destination.to_owned(),
            std::fs::read_to_string(root.join(source)).unwrap(),
        );
    }
    serde_json::to_string(&json!({
        "script": std::fs::read_to_string(root.join("task-shell.sh")).unwrap(),
        "files": files,
    }))
    .unwrap()
}

#[test]
fn desktop_shell_reconnects_without_replay_and_close_settles_its_jobs() {
    shell_reconnects_and_settles(false);
    shell_reconnects_and_settles(true);
}

fn shell_reconnects_and_settles(integrated: bool) {
    let fixture = Fixture::new();
    let task = "task_5e55105e000040008000000000000005";
    fixture.shell_task(task);
    let base = [
        "desktop",
        "shell",
        task,
        "--checkout",
        fixture.repo.path().to_str().unwrap(),
        "--id",
        "desktop-shell",
    ];
    let mut command = fixture.command(&base);
    command.arg("--create");
    if integrated {
        command.args(["--bootstrap", &desktop_shell_bootstrap()]);
    }
    let mut launch = command
        .args([
            "--",
            "/bin/sh",
            "-c",
            "printf '%s\\n' \"$1\" >> \"$HOME/launches\"",
            "initial",
            "$(literal); spaced argument",
        ])
        .env_remove("GHOSTTY_RESOURCES_DIR")
        .env("SHELL", "/bin/bash")
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .unwrap();
    assert_eq!(
        fixture.wait_file("launches").trim(),
        "$(literal); spaced argument"
    );
    let owner = fixture.shell_owner("desktop-shell");
    fixture.wait_controlled(&owner);
    let mut viewer = fixture.viewer(&owner, true);
    launch.wait().unwrap();
    viewer.input("export RETAINED=still-here; sleep 30 & printf 'job:%s\\n' $! > \"$HOME/job\"; printf 'state-ready\\n'\r");
    until_out(&mut viewer, "state-ready");
    viewer.input(
        "printf 'integration:%s\\n' \"${GHOSTTY_RESOURCES_DIR-unset}\" > \"$HOME/integration\"\r",
    );
    let integration = fixture.wait_file("integration");
    viewer.input("command -v lf > \"$HOME/lf-path\"; printf '%s\\n' \"$LF_TERMINAL_ID\" > \"$HOME/terminal-id\"\r");
    assert_eq!(
        fixture.wait_file("lf-path").trim(),
        env!("CARGO_BIN_EXE_lf")
    );
    assert_eq!(fixture.wait_file("terminal-id").trim(), "desktop-shell");
    if integrated {
        assert!(integration.contains("shell-resources"), "{integration}");
    } else {
        assert_eq!(integration.trim(), "integration:unset");
        assert!(!fixture.home.path().join("shell-resources").exists());
    }
    let job = fixture
        .wait_file("job")
        .trim()
        .strip_prefix("job:")
        .unwrap()
        .parse::<u32>()
        .unwrap();
    drop(viewer);
    // Simulate a completed nested conversation; shell purpose stays immutable.
    let session = fixture.session(&owner);
    fixture
        .sql()
        .execute(
            "UPDATE agent_sessions SET interactive=1,completed_at=unixepoch() WHERE id=?1",
            [&session],
        )
        .unwrap();
    std::thread::sleep(Duration::from_secs(4));
    assert!(fixture.running(&owner));
    let mut retry = fixture
        .command(&base)
        .arg("--create")
        .env("SHELL", "/bin/bash")
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .unwrap();
    fixture.wait_controlled(&owner);
    let mut returning = fixture.viewer(&owner, true);
    retry.wait().unwrap();
    returning.input("printf 'value:%s\\n' \"$RETAINED\"\r");
    until_out(&mut returning, "value:still-here");
    assert_eq!(fixture.wait_file("launches").lines().count(), 1);
    let close = fixture.command(&base).arg("--close").output().unwrap();
    assert!(
        close.status.success(),
        "{}",
        String::from_utf8_lossy(&close.stderr)
    );
    fixture.ends(&owner);
    let settled: bool = fixture
        .sql()
        .query_row(
            "SELECT jobs_settled FROM desktop_shells WHERE owner=?1",
            [&owner],
            |r| r.get(0),
        )
        .unwrap();
    assert!(
        settled,
        "shell job settlement was not recorded: {:?}",
        std::fs::read_to_string(fixture.home.path().join("logs/transport.log"))
    );
    // Zombies cannot execute or retain terminal descriptors.
    let output = Command::new("ps")
        .args(["-p", &job.to_string(), "-o", "stat="])
        .output()
        .unwrap();
    assert!(
        output.stdout.is_empty()
            || String::from_utf8_lossy(&output.stdout)
                .trim()
                .starts_with('Z')
    );
    // History remains readable after retirement removed the recorded directory.
    let checkout = fixture.repo.path().to_path_buf();
    let retired = checkout.with_extension("retired");
    std::fs::rename(&checkout, &retired).unwrap();
    let ended = fixture
        .command(&base)
        .current_dir(fixture.home.path())
        .arg("--create")
        .output()
        .unwrap();
    std::fs::rename(&retired, &checkout).unwrap();
    assert!(!ended.status.success());
    assert!(String::from_utf8_lossy(&ended.stderr).contains("Shell ended"));
    assert_eq!(fixture.wait_file("launches").lines().count(), 1);
    assert!(!fixture.endpoint(&owner).exists());
}

#[test]
fn desktop_shell_twenty_lifecycles_leave_no_owned_jobs_or_sockets() {
    let fixture = Fixture::new();
    let task = "task_5e55105e000040008000000000000006";
    fixture.shell_task(task);
    for cycle in 0..20 {
        let id = format!("cycle-{cycle}");
        let base = [
            "desktop",
            "shell",
            task,
            "--checkout",
            fixture.repo.path().to_str().unwrap(),
            "--id",
            &id,
        ];
        let ready = fixture.home.path().join("cycle-ready");
        let _ = std::fs::remove_file(&ready);
        let _ = std::fs::remove_file(fixture.home.path().join("shell-pid"));
        let mut launch = fixture
            .command(&base)
            .args([
                "--create",
                "--",
                "/bin/sh",
                "-c",
                "printf ready > \"$HOME/cycle-ready\"",
            ])
            .env("SHELL", "/bin/bash")
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .unwrap();
        fixture.wait_file("cycle-ready");
        let owner = fixture.shell_owner(&id);
        fixture.wait_controlled(&owner);
        let mut viewer = fixture.viewer(&owner, true);
        launch.wait().unwrap();
        viewer
            .input("sleep 30 & printf '%s\\n' $$ > \"$HOME/shell-pid\"; printf 'cycle-ready\\n'\r");
        until_out(&mut viewer, "cycle-ready");
        let pid = fixture
            .wait_file("shell-pid")
            .trim()
            .parse::<u32>()
            .unwrap();
        drop(viewer);
        assert!(fixture
            .command(&base)
            .arg("--close")
            .output()
            .unwrap()
            .status
            .success());
        fixture.ends(&owner);
        assert!(!fixture.endpoint(&owner).exists(), "cycle {cycle}");
        let inventory = Command::new("ps")
            .args(["-axo", "pid=,stat="])
            .output()
            .unwrap();
        assert!(inventory.status.success());
        for row in String::from_utf8_lossy(&inventory.stdout).lines() {
            let mut fields = row.split_whitespace();
            let process = fields.next().unwrap().parse::<i32>().unwrap();
            let state = fields.next().unwrap();
            // SAFETY: getsid only inspects the enumerated PID; a raced exit is absent.
            let session = unsafe { libc::getsid(process) };
            assert!(
                session != pid as i32 || state.starts_with('Z'),
                "cycle {cycle} retained PTY process {process}"
            );
        }
        let settled: bool = fixture
            .sql()
            .query_row(
                "SELECT jobs_settled FROM desktop_shells WHERE id=?1",
                [&id],
                |r| r.get(0),
            )
            .unwrap();
        assert!(settled, "cycle {cycle}");
    }
    let (total, settled): (u32, u32) = fixture
        .sql()
        .query_row(
            "SELECT count(*),sum(jobs_settled) FROM desktop_shells WHERE task_id=?1",
            [task],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )
        .unwrap();
    assert_eq!((total, settled), (20, 20));
}

#[path = "transport/ssh_shells.rs"]
mod ssh_shells;

#[test]
fn viewer_stream_over_pipes_preserves_terminal_input_size_and_exit() {
    let fixture = Fixture::new();
    let owner = fixture.start(&[
        "/bin/sh",
        "-c",
        "stty -echo; read -r line; stty size; printf 'received:<%s>\\n' \"$line\"; exit 7",
    ]);
    let _session = fixture.session(&owner);
    let mut child = fixture
        .command(&[
            "session",
            "attach",
            "--",
            "/usr/bin/env",
            "LF_VIEWER_STREAM=1",
            env!("CARGO_BIN_EXE_lf"),
            "session",
            "attach",
            &owner,
        ])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    let mut input = child.stdin.take().unwrap();
    input.write_all(b"typed-value\n").unwrap();
    let output = child.wait_with_output().unwrap();
    assert_eq!(output.status.code(), Some(7), "{output:?}");
    let text = String::from_utf8_lossy(&output.stdout);
    assert!(text.contains("received:<typed-value>"), "{text}");
    assert!(text.contains("24 80"), "initial terminal size: {text}");
}

#[test]
fn local_terminal_reconnects_without_replaying_input_or_taking_newer_claims() {
    for (takeover, retry) in [(false, false), (true, false), (false, true)] {
        let fixture = Fixture::new();
        let owner = fixture.owner(fixture.serve(true, &[
            "/bin/sh", "-c",
            "stty -echo; printf '%s\\n' $$ >> \"$HOME/launches\"; printf 'ready-for-input\\n'; while IFS= read -r line; do printf '%s\\n' \"$line\" >> \"$HOME/input\"; printf '%s\\n' \"$line\"; done",
        ]));
        let owner_pid = pid_of_process(&fixture, &owner);
        let endpoint = fixture.endpoint(&owner);
        let retained = endpoint.with_extension("retained");
        std::fs::rename(&endpoint, &retained).unwrap();
        let listener = UnixListener::bind(&endpoint).unwrap();
        let (connections, connected) = std::sync::mpsc::channel();
        std::thread::scope(|scope| {
            let proxy = scope.spawn(|| {
                let (mut downstream, _) = listener.accept().unwrap();
                let mut upstream = UnixStream::connect(&retained).unwrap();
                for socket in [&downstream, &upstream] {
                    socket
                        .set_read_timeout(Some(Duration::from_secs(15)))
                        .unwrap();
                    socket
                        .set_write_timeout(Some(Duration::from_secs(5)))
                        .unwrap();
                }
                let mut requests = downstream.try_clone().unwrap();
                let mut destination = upstream.try_clone().unwrap();
                connections
                    .send((
                        downstream.try_clone().unwrap(),
                        upstream.try_clone().unwrap(),
                    ))
                    .unwrap();
                let forward = scope.spawn(move || {
                    let _ = std::io::copy(&mut requests, &mut destination);
                });
                let _ = std::io::copy(&mut upstream, &mut downstream);
                forward.join().unwrap();
            });
            let mut relay = Terminal::spawn(fixture.command(&["session", "attach", &owner]));
            let (downstream, upstream) = connected.recv_timeout(Duration::from_secs(10)).unwrap();
            relay.wait_text("ready-for-input");
            relay.input(b"before loss\n");
            relay.wait_text("before loss");
            // Pause only this fixture's viewer, then lose its old connection.
            // This models an owner that expired the connection during sleep.
            let paused = PausedOwner(relay.child.id() as libc::pid_t);
            // SAFETY: the PID is this test's live relay child.
            assert_eq!(unsafe { libc::kill(paused.0, libc::SIGSTOP) }, 0);
            relay.input(b"typing during outage\n");
            std::fs::remove_file(&endpoint).unwrap();
            if !retry {
                std::fs::rename(&retained, &endpoint).unwrap();
            }
            downstream.shutdown(std::net::Shutdown::Both).unwrap();
            upstream.shutdown(std::net::Shutdown::Both).unwrap();
            proxy.join().unwrap();
            wait_hold(&fixture, &owner, Some("absent"));
            let mut newer = takeover.then(|| {
                fixture.attach(
                    &owner,
                    json!({
                        "kind":"attach", "control":true, "replace":true,
                        "own":"take", "rows":45, "cols":115,
                    }),
                )
            });
            drop(paused);
            if retry {
                relay.wait_text("Ctrl+R to reconnect");
                relay.assert_viewing("Unavailable owner leaves retry in this terminal");
                std::fs::rename(&retained, &endpoint).unwrap();
                relay.input(b"more discarded typing\x12");
            }
            relay.wait_text(if takeover {
                "Reconnected as a viewer"
            } else {
                "Reconnected to the same Session"
            });
            if let Some(newer) = &mut newer {
                relay.input(b"must not reach provider\n");
                let mut observer = fixture.connect(&owner);
                observer.request(json!({"kind":"attach", "control":false, "replace":false, "rows":24, "cols":80}));
                assert_eq!(observer.until_reply()["owner"], newer.id);
                newer.input("after recovery\n");
            } else {
                relay.input(b"after recovery\n");
            }
            relay.wait_text("after recovery");
            assert_eq!(fixture.wait_file("input"), "before loss\nafter recovery\n");
            assert_eq!(fixture.wait_file("launches").lines().count(), 1);
            assert_eq!(pid_of_process(&fixture, &owner), owner_pid);
            relay.assert_viewing("Recovered local terminal");
            relay.input(&[0x1d]);
            assert!(relay.wait_exit().success());
        });
    }
}

#[test]
fn viewer_stream_heartbeat_expires_only_the_silent_viewer_while_output_continues() {
    let fixture = Fixture::new();
    let owner = fixture.start(&[
        "/bin/sh",
        "-c",
        "while :; do printf 'tick\\n'; sleep 0.1; done",
    ]);
    let mut lost = fixture.viewer(&owner, true);
    let mut passive = fixture.viewer(&owner, false);
    let started = Instant::now();
    let mut next_heartbeat = started;
    // Keep reading the silent endpoint: only missing heartbeats can end it.
    while lost.next().is_some() {
        assert!(started.elapsed() < Duration::from_secs(35));
        if Instant::now() >= next_heartbeat {
            passive.request(json!({"kind":"controlled"}));
            next_heartbeat = Instant::now() + Duration::from_secs(5);
        }
    }
    assert!(started.elapsed() >= Duration::from_secs(29));
    assert!(fixture.running(&owner));
    assert_eq!(viewer_hold(&fixture, &owner).as_deref(), Some("viewed"));
    // The expired controller cannot prevent a new connection claiming input.
    let mut replacement = fixture.attach(
        &owner,
        json!({"kind":"attach","control":true,
        "replace":false,"own":"take","rows":31,"cols":97}),
    );
    assert!(replacement.token.is_some());
    replacement.request(json!({"kind":"leave"}));
    passive.request(json!({"kind":"leave"}));
    wait_hold(&fixture, &owner, Some("absent"));
    // Replay grace and the unchanged execution still permit a reconnect.
    let mut resumed = fixture.connect(&owner);
    resumed.request(json!({"kind":"resume","reconnect":lost.reconnect,
        "received":lost.received,"control":true,"own":"take","rows":24,"cols":80}));
    assert_eq!(resumed.until_reply()["kind"], "attached");
    wait_hold(&fixture, &owner, Some("viewed"));
    resumed.request(json!({"kind":"leave"}));
    wait_hold(&fixture, &owner, Some("absent"));
}

#[test]
fn viewer_stream_backpressure_releases_bridge_and_owner_keeps_draining() {
    let fixture = Fixture::new();
    let owner = fixture.owner(fixture.serve(
        false,
        &["/bin/sh", "-c", "read -r start; head -c 16777216 /dev/zero"],
    ));
    let _session = fixture.session(&owner);
    let mut child = fixture
        .command(&["session", "attach", &owner])
        .env("LF_VIEWER_STREAM", "1")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    let mut input = child.stdin.take().unwrap();
    input.write_all(&frame(REQUEST, br#"{"id":1,"kind":"attach","control":true,"replace":false,"own":null,"rows":24,"cols":80}"#)).unwrap();
    input.write_all(&frame(INPUT, b"go\n")).unwrap();
    // Keep stdin open and leave stdout completely unread until the bridge exits.
    // The owner must keep draining execution, then release this stalled viewer.
    let output = wait_for_viewer(child, Duration::from_secs(15));
    assert!(!output.status.success());
    let (tag, payload) = read_frame(&mut std::io::Cursor::new(&output.stdout)).unwrap();
    assert_eq!(tag, REPLY);
    assert_eq!(
        serde_json::from_slice::<Value>(&payload).unwrap()["kind"],
        "attached"
    );
    fixture.ends(&owner);
}

#[test]
fn viewer_stream_waits_for_destination_resolution_before_attachment() {
    let fixture = Fixture::new();
    let owner = fixture.start(&[
        "/bin/sh",
        "-c",
        "read -r line; printf 'received:%s' \"$line\"",
    ]);
    let _session = fixture.session(&owner);
    let mut child = fixture
        .command(&[
            "session",
            "attach",
            "--",
            "/bin/sh",
            "-c",
            "sleep 6; exec \"$@\"",
            "fixture",
            "/usr/bin/env",
            "LF_VIEWER_STREAM=1",
            env!("CARGO_BIN_EXE_lf"),
            "session",
            "attach",
            &owner,
        ])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    let mut input = child.stdin.take().unwrap();
    input.write_all(b"ready\n").unwrap();
    let output = wait_for_viewer(child, Duration::from_secs(15));
    assert!(output.status.success(), "{output:?}");
    assert!(String::from_utf8_lossy(&output.stdout).contains("received:ready"));
}

// Server adapter proofs keep the existing owner/provider fixture and its
// protocol. The service must never become another execution authority.
struct ServerProof<'a> {
    fixture: &'a Fixture,
    child: Option<Child>,
    supervised: bool,
    executable: PathBuf,
}

impl<'a> ServerProof<'a> {
    fn start(fixture: &'a Fixture, supervised: bool) -> Self {
        // The server observes an installed Machine; startup creates no Work.
        tokio::runtime::Runtime::new()
            .unwrap()
            .block_on(loopflow::store::open_ephemeral_store(
                &loopflow::store::StorageConfig::sqlite(fixture.home.path().join("loopflow.db")),
            ))
            .unwrap();
        let mut service = Self {
            fixture,
            child: None,
            supervised,
            executable: env!("CARGO_BIN_EXE_lf").into(),
        };
        if supervised {
            let mut child = Command::new("uv")
                .args([
                    "run",
                    "python",
                    "scripts/test_server_registration.py",
                    "--lf",
                    env!("CARGO_BIN_EXE_lf"),
                    "--home",
                ])
                .arg(fixture.home.path())
                .current_dir(Path::new(env!("CARGO_MANIFEST_DIR")).join("../.."))
                .stdin(Stdio::piped())
                .stdout(Stdio::piped())
                .stderr(Stdio::inherit())
                .spawn()
                .unwrap();
            let mut ready = String::new();
            BufReader::new(child.stdout.as_mut().unwrap())
                .read_line(&mut ready)
                .unwrap();
            service.executable = ready.trim().into();
            assert!(
                service.executable.is_file(),
                "Signed helper not ready: {ready}"
            );
            service.child = Some(child);
        } else {
            service.child = Some(service.launch());
        }
        let instance = service.ready();
        if supervised {
            // Concurrent Desktop/CLI setup must obtain the same installed helper.
            let results = std::thread::scope(|scope| {
                let ensure = || service.command(&["install", "server"]).output().unwrap();
                let first = scope.spawn(ensure);
                let second = scope.spawn(ensure);
                [first.join().unwrap(), second.join().unwrap()]
            });
            for result in results {
                assert!(
                    result.status.success(),
                    "{}",
                    String::from_utf8_lossy(&result.stderr)
                );
                let reading: Value = serde_json::from_slice(&result.stdout).unwrap();
                assert_eq!(
                    reading["instance"], instance,
                    "Setup must not replace a ready helper"
                );
            }
        }
        service
    }

    // Every part of the installed proof uses the executable being replaced,
    // including subprocesses that resolve the public lf entry through LF_BIN.
    fn command(&self, args: &[&str]) -> Command {
        let mut command = self.fixture.command_from(&self.executable, args);
        command.env("LF_BIN", &self.executable);
        command
    }

    fn relay_command(&self, owner: &str) -> (Command, ConnectionStatus) {
        let status = ConnectionStatus::new(self.fixture.home.path().join("connection.sock"));
        let session = self.fixture.bound_session(owner);
        let mut command = self.command(&["session", "attach", owner, "--session", &session]);
        command
            .env("LF_SERVER", "1")
            .env("LF_SERVER_STATUS", &status.path);
        (command, status)
    }

    fn launch(&self) -> Child {
        self.command(&["server", "run"])
            .env_remove("RUST_LOG")
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .unwrap()
    }

    fn ready(&self) -> String {
        let deadline = Instant::now() + Duration::from_secs(20);
        loop {
            if let Ok(connection) =
                loopflow::server::client::Connection::open_at(&self.fixture.endpoint("machine"))
            {
                return connection.welcome().instance.clone();
            }
            assert!(
                Instant::now() < deadline,
                "Server service did not become ready: {:?}",
                std::fs::read_to_string(self.fixture.home.path().join("installed-helper.log"))
            );
            std::thread::sleep(Duration::from_millis(20));
        }
    }

    fn restart(&mut self) {
        if self.supervised {
            let child = self.child.as_mut().unwrap();
            child
                .stdin
                .as_mut()
                .unwrap()
                .write_all(b"replace\n")
                .unwrap();
            let mut ready = String::new();
            BufReader::new(child.stdout.as_mut().unwrap())
                .read_line(&mut ready)
                .unwrap();
            assert_eq!(ready.trim(), "replaced");
        } else {
            self.stop();
            self.child = Some(self.launch());
        }
    }

    fn stop(&mut self) {
        if let Some(mut child) = self.child.take() {
            if !self.supervised {
                let _ = child.kill();
            }
            drop(child.stdin.take());
            let _ = child.wait();
        }
    }

    fn connect(&self) -> loopflow::server::client::Connection {
        let connection =
            loopflow::server::client::Connection::open_at(&self.fixture.endpoint("machine"))
                .unwrap();
        connection
            .set_read_timeout(Some(Duration::from_secs(15)))
            .unwrap();
        connection
    }

    fn viewer(&self, owner: &str) -> Viewer {
        let session = self.fixture.bound_session(owner);
        let socket = loopflow::server::client::attach(
            &self.fixture.endpoint("machine"),
            &session.as_str().into(),
            Some(&serde_json::from_value(json!(owner)).unwrap()),
        )
        .unwrap();
        Viewer::new(socket).attach(
            json!({"kind":"attach","control":true,"replace":true,"own":"take","rows":24,"cols":80}),
        )
    }
}

impl Drop for ServerProof<'_> {
    fn drop(&mut self) {
        self.stop();
    }
}

struct ConnectionStatus {
    path: PathBuf,
    listener: UnixListener,
    state: std::cell::RefCell<ConnectionStatusState>,
}

#[derive(Default)]
struct ConnectionStatusState {
    socket: Option<UnixStream>,
    received: Vec<u8>,
    reading: Option<Value>,
}

impl ConnectionStatus {
    fn new(path: PathBuf) -> Self {
        let listener = UnixListener::bind(&path).unwrap();
        listener.set_nonblocking(true).unwrap();
        Self {
            path,
            listener,
            state: Default::default(),
        }
    }

    fn reading(&self) -> Option<Value> {
        let mut state = self.state.borrow_mut();
        if state.socket.is_none() {
            if let Ok((stream, _)) = self.listener.accept() {
                stream.set_nonblocking(true).unwrap();
                state.socket = Some(stream);
            }
        }
        let mut bytes = [0; 8192];
        while let Some(socket) = &mut state.socket {
            match socket.read(&mut bytes) {
                Ok(0) => {
                    state.socket = None;
                    break;
                }
                Ok(n) => state.received.extend_from_slice(&bytes[..n]),
                Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => break,
                Err(error) => panic!("status: {error}"),
            }
        }
        while let Some(end) = state.received.iter().position(|b| *b == b'\n') {
            let line: Vec<_> = state.received.drain(..=end).collect();
            state.reading = Some(serde_json::from_slice(&line).unwrap());
        }
        state.reading.clone()
    }

    fn send(&self, action: Value) {
        self.reading();
        let mut state = self.state.borrow_mut();
        let stream = state.socket.as_mut().expect("relay connected");
        writeln!(stream, "{action}").unwrap();
    }

    fn disconnect(&self) {
        *self.state.borrow_mut() = ConnectionStatusState::default();
    }
}

fn wait_server_status(status: &ConnectionStatus, state: &str, input: bool, generation: u64) {
    let deadline = Instant::now() + Duration::from_secs(30);
    loop {
        let reading = status.reading();
        if reading.as_ref().is_some_and(|reading| {
            reading["state"] == state
                && reading["input"] == input
                && reading["generation"].as_u64().unwrap() >= generation
        }) {
            return;
        }
        assert!(
            Instant::now() < deadline,
            "Expected {state}/{input}, got {reading:?}"
        );
        std::thread::sleep(Duration::from_millis(20));
    }
}

fn request_server_move(status: &ConnectionStatus) {
    let reading = status.reading().unwrap();
    assert_eq!(reading["state"], "connected");
    assert_eq!(reading["input"], false);
    status.send(json!({"kind":"move", "generation":reading["generation"]}));
}

#[cfg(target_os = "macos")]
#[test]
#[ignore = "uses the login launchd domain with isolated development jobs"]
fn server_registration_migrates_old_job_and_respects_disabled_choice() {
    struct Jobs(Vec<String>);
    impl Drop for Jobs {
        fn drop(&mut self) {
            for target in &self.0 {
                for action in ["bootout", "enable"] {
                    let _ = Command::new("/bin/launchctl")
                        .args([action, target])
                        .output();
                }
            }
        }
    }
    let fixture = Fixture::new();
    let home = fixture.home.path().canonicalize().unwrap();
    let digest = hex::encode(Sha256::digest(home.as_os_str().as_encoded_bytes()));
    // SAFETY: geteuid has no preconditions.
    let domain = format!("gui/{}", unsafe { libc::geteuid() });
    let old_label = format!("com.loopflow.machine.{}", &digest[..16]);
    let new_label = format!("com.loopflow.server.{}", &digest[..16]);
    let old = format!("{domain}/{old_label}");
    let new = format!("{domain}/{new_label}");
    let _jobs = Jobs(vec![old.clone(), new.clone()]);
    let launchctl = |args: &[&str]| {
        let output = Command::new("/bin/launchctl").args(args).output().unwrap();
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
    };
    launchctl(&["disable", &old]);
    let refused = fixture.command(&["install", "server"]).output().unwrap();
    assert!(!refused.status.success());
    assert!(
        String::from_utf8_lossy(&refused.stderr).contains("disabled"),
        "{}",
        String::from_utf8_lossy(&refused.stderr)
    );
    assert!(!fixture.endpoint("machine").exists());
    launchctl(&["enable", &old]);
    let owner = fixture.owner(fixture.command(&["session", "serve", "--", "/bin/cat"]));
    let owner_pid = pid_of_process(&fixture, &owner);
    let legacy_plist = home.join("machine-service.plist");
    std::fs::write(
        &legacy_plist,
        format!(
            r#"<?xml version="1.0"?>
<plist version="1.0"><dict><key>Label</key><string>{old_label}</string>
<key>ProgramArguments</key><array><string>/bin/sleep</string><string>120</string></array>
<key>RunAtLoad</key><true/><key>KeepAlive</key><true/></dict></plist>"#
        ),
    )
    .unwrap();
    launchctl(&["bootstrap", &domain, legacy_plist.to_str().unwrap()]);
    std::fs::write(fixture.endpoint("machine"), "retired endpoint").unwrap();
    let results = std::thread::scope(|scope| {
        let ensure = || fixture.command(&["install", "server"]).output().unwrap();
        let first = scope.spawn(ensure);
        let second = scope.spawn(ensure);
        [first.join().unwrap(), second.join().unwrap()]
    });
    let readings = results.map(|result| {
        assert!(
            result.status.success(),
            "{}",
            String::from_utf8_lossy(&result.stderr)
        );
        serde_json::from_slice::<Value>(&result.stdout).unwrap()
    });
    assert_eq!(readings[0]["ready"], true);
    assert_eq!(readings[0]["instance"], readings[1]["instance"]);
    assert!(!Command::new("/bin/launchctl")
        .args(["print", &old])
        .output()
        .unwrap()
        .status
        .success());
    launchctl(&["print", &new]);
    assert_eq!(pid_of_process(&fixture, &owner), owner_pid);
    assert!(!legacy_plist.exists());
    assert!(fixture.endpoint("machine").exists());
    // The retired executable's lock cannot admit a second listener.
    let lock = std::fs::File::open(fixture.endpoint("machine").with_extension("lock")).unwrap();
    assert!(fs2::FileExt::try_lock_exclusive(&lock).is_err());
}

#[test]
fn server_status_does_not_install_or_create_work() {
    let fixture = Fixture::new();
    let result = fixture.command(&["server", "status"]).output().unwrap();
    assert!(
        result.status.success(),
        "{}",
        String::from_utf8_lossy(&result.stderr)
    );
    let status: Value = serde_json::from_slice(&result.stdout).unwrap();
    assert_eq!(status["ready"], false);
    assert!(status["instance"].is_null());
    assert!(!fixture.endpoint("machine").exists());
    assert!(!fixture.home.path().join("server.plist").exists());
    assert!(!fixture.home.path().join("loopflow.db").exists());
}

#[test]
fn terminal_bridges_leave_the_client_connections_free() {
    let fixture = Fixture::new();
    let owner = fixture.owner(fixture.serve(false, &["/bin/cat"]));
    let session = loopflow::id::LfSessionId::from(fixture.session(&owner).as_str());
    let service = ServerProof::start(&fixture, false);
    // More bridges than the server admits clients: each takes a ticket on a
    // client connection it closes, then opens the bridge on its own.
    let bridges: Vec<_> = (0..loopflow::server::wire::MAX_CONNECTIONS + 8)
        .map(|_| {
            loopflow::server::client::attach(&fixture.endpoint("machine"), &session, None).unwrap()
        })
        .collect();
    // Desktop's own connection still gets in.
    let welcome = service.connect();
    assert_eq!(
        welcome.welcome().limits.max_connections,
        loopflow::server::wire::MAX_CONNECTIONS
    );
    drop(bridges);
}

/// One line to a fresh connection and the server's one-line answer.
fn first_exchange(socket: &Path, line: &str) -> Value {
    let mut stream = UnixStream::connect(socket).unwrap();
    stream
        .set_read_timeout(Some(Duration::from_secs(10)))
        .unwrap();
    writeln!(stream, "{line}").unwrap();
    let mut answer = String::new();
    BufReader::new(&stream).read_line(&mut answer).unwrap();
    serde_json::from_str(&answer).unwrap()
}

#[test]
fn an_attach_ticket_opens_one_bridge_from_any_connection_of_its_user() {
    let fixture = Fixture::new();
    let owner = fixture.owner(fixture.serve(false, &["/bin/cat"]));
    let session = fixture.session(&owner);
    let service = ServerProof::start(&fixture, false);
    let socket = fixture.endpoint("machine");
    let mut client = service.connect();
    let issued = client
        .request(
            serde_json::from_value(
                json!({"kind":"session.attach","session":session,"owner_lf_process_id":null}),
            )
            .unwrap(),
        )
        .unwrap()
        .result;
    assert_eq!(issued["owner_lf_process_id"], owner);
    assert_eq!(
        issued["expires_in_ms"],
        loopflow::server::wire::TICKET_LIFETIME.as_millis() as u64
    );
    let ticket = issued["ticket"].as_str().unwrap();
    assert_eq!(ticket.len(), 22, "{ticket}");
    // The client connection that asked stays a client.
    assert!(client
        .request(serde_json::from_value(json!({"kind":"config.catalog"})).unwrap())
        .is_ok());
    // The ticket works on a connection the requester did not open, and once.
    let bridged = first_exchange(
        &socket,
        &json!({"type":"attach","ticket":ticket}).to_string(),
    );
    assert_eq!(bridged["type"], "bridged", "{bridged}");
    assert_eq!(bridged["owner_lf_process_id"], owner);
    let spent = first_exchange(
        &socket,
        &json!({"type":"attach","ticket":ticket}).to_string(),
    );
    assert_eq!(spent["type"], "error", "{spent}");
    assert_eq!(spent["code"], "not_found");
    assert!(spent["id"].is_null());
    // Unknown and malformed first lines are refused before any bridge.
    let unknown = first_exchange(
        &socket,
        r#"{"type":"attach","ticket":"AAAAAAAAAAAAAAAAAAAAAA"}"#,
    );
    assert_eq!(unknown["code"], "not_found", "{unknown}");
    let malformed = first_exchange(&socket, r#"{"type":"attach"}"#);
    assert_eq!(malformed["code"], "invalid", "{malformed}");
    // After hello, attach is no longer a first message.
    let mut late = service.connect();
    let error = late
        .request(
            serde_json::from_value(json!({"kind":"session.attach","session":"session_missing","owner_lf_process_id":null}))
                .unwrap(),
        )
        .unwrap_err();
    assert_eq!(
        error
            .downcast_ref::<loopflow::engine::Error>()
            .unwrap()
            .code,
        loopflow::engine::ErrorCode::NotFound
    );
}

#[test]
fn server_subscriptions_refresh_work_and_resolve_primary_session() {
    let fixture = Fixture::new();
    let owner = fixture.owner(fixture.serve(false, &["/bin/cat"]));
    let session = fixture.session(&owner);
    let service = ServerProof::start(&fixture, false);
    let query = json!({"kind":"plan.list","directory":"/","wave":null,"task":null,"all":true});
    let mut subscription = service.connect();
    let tasks = subscription
        .subscribe(serde_json::from_value(query).unwrap())
        .unwrap();
    let initial = subscription.reading(&tasks).unwrap().expect("a reading");
    assert!(initial["unavailable"].is_null(), "{initial}");
    subscription.checkpoint(&tasks, 17).unwrap();
    let confirmed = subscription.reading(&tasks).unwrap().expect("a reading");
    assert_eq!(confirmed["answers"], 17);

    // Write parity, concurrent traces and no request Process rows are covered
    // by served_reads. This proof owns live subscription invalidation.
    // A changed Task reading uses the shared invalidation path, with no polling
    // projection or shadow Task store in the service.
    let registered = support::register_task_with_pr(
        fixture.home.path(),
        &fixture.repo.path().canonicalize().unwrap(),
        "server-proof",
        &fixture.repo.head_sha(),
    );
    let db = rusqlite::Connection::open(fixture.home.path().join("loopflow.db")).unwrap();
    db.execute(
        "UPDATE tasks SET primary_session_id=?2 WHERE id=?1",
        rusqlite::params![registered.task.id.as_str(), session],
    )
    .unwrap();
    // The read shares the subscription's connection.
    let socket_plan = subscription
        .request(
            serde_json::from_value(json!({"kind":"desktop.view.get","directory":fixture.repo.path(),"expression":"task:INF-123"}))
                .unwrap(),
        )
        .unwrap()
        .result;
    let cli = service
        .command(&["desktop", "view", "task:INF-123", "--plan", "--json"])
        .output()
        .unwrap();
    assert!(
        cli.status.success(),
        "{}",
        String::from_utf8_lossy(&cli.stderr)
    );
    let cli_plan: Value = serde_json::from_slice(&cli.stdout).unwrap();
    assert_eq!(socket_plan, cli_plan);
    assert_eq!(socket_plan["contents"][0]["primary_session"], session);
    subscription.checkpoint(&tasks, 18).unwrap();
    loop {
        let changed = subscription.reading(&tasks).unwrap().expect("a reading");
        if changed["answers"] == 18 {
            assert!(changed["body"].to_string().contains("INF-123"), "{changed}");
            break;
        }
    }
    drop(subscription);
}

#[test]
fn server_initial_connection_reports_opening_until_input_is_acknowledged() {
    let fixture = Fixture::new();
    let owner = fixture.owner(fixture.serve(false, &["/bin/cat"]));
    let service = ServerProof::start(&fixture, false);
    let installer = fixture.home.path().join("installer");
    std::fs::write(
        &installer,
        "#!/bin/sh\nwhile [ ! -f \"$HOME/setup-allowed\" ]; do sleep 0.02; done\n",
    )
    .unwrap();
    std::fs::set_permissions(&installer, std::fs::Permissions::from_mode(0o755)).unwrap();
    let (mut command, status) = service.relay_command(&owner);
    command.env("LF_BIN", installer);
    let mut relay = Terminal::stream(command);
    wait_server_status(&status, "connecting", false, 1);
    relay.input(b"not yet\n");
    std::fs::write(fixture.home.path().join("setup-allowed"), "").unwrap();
    wait_server_status(&status, "connected", true, 1);
    relay.input(b"after acknowledgement\n");
    relay.wait_text("after acknowledgement");
    assert!(!String::from_utf8_lossy(&relay.screen.lock().unwrap()).contains("not yet"));
    drop(relay.child.stdin.take());
    assert!(relay.wait_exit().success());
}

#[test]
fn status_channel_failure_keeps_relay_alive_and_never_replays_outage_input() {
    let fixture = Fixture::new();
    let owner = fixture.owner(fixture.serve(false, &["/bin/cat"]));
    let owner_pid = pid_of_process(&fixture, &owner);
    let mut service = ServerProof::start(&fixture, false);
    let (command, status) = service.relay_command(&owner);
    let mut relay = Terminal::stream(command);
    wait_server_status(&status, "connected", true, 1);
    status.disconnect();
    std::fs::remove_file(&status.path).unwrap();
    relay.input(b"status unavailable but provider connected\n");
    relay.wait_text("status unavailable but provider connected");
    relay.assert_viewing("Presentation cannot end the terminal relay");
    service.stop();
    // The broken status channel supplies no readout. Observe claim release on
    // the real owner before injecting uncertain input into the relay.
    let deadline = Instant::now() + Duration::from_secs(10);
    loop {
        let mut probe = fixture.connect(&owner);
        probe.request(json!({"kind":"controlled"}));
        if probe.until_reply()["held"] == false {
            break;
        }
        assert!(Instant::now() < deadline);
        std::thread::sleep(Duration::from_millis(20));
    }
    relay.input(b"uncertain-outage-input\n");
    let repaired = ConnectionStatus::new(status.path.clone());
    wait_server_status(&repaired, "reconnecting", false, 2);
    service.child = Some(service.launch());
    service.ready();
    wait_server_status(&repaired, "connected", true, 2);
    relay.input(b"after status repair\n");
    relay.wait_text("after status repair");
    assert!(
        !String::from_utf8_lossy(&relay.screen.lock().unwrap()).contains("uncertain-outage-input")
    );
    assert_eq!(pid_of_process(&fixture, &owner), owner_pid);
    drop(relay.child.stdin.take());
    assert!(relay.wait_exit().success());
}

#[test]
fn server_reconnects_a_writable_bridge_that_stops_answering() {
    let fixture = Fixture::new();
    let owner = fixture.owner(fixture.serve(false, &[
        "/bin/sh", "-c",
        "while IFS= read -r line; do printf '%s\\n' \"$line\" >> \"$HOME/input\"; printf '%s\\n' \"$line\"; done",
    ]));
    let owner_pid = pid_of_process(&fixture, &owner);
    let service = ServerProof::start(&fixture, false);
    let endpoint = fixture.endpoint(&owner);
    let retained = endpoint.with_extension("retained");
    std::fs::rename(&endpoint, &retained).unwrap();
    let listener = UnixListener::bind(&endpoint).unwrap();
    let stalled = std::sync::atomic::AtomicBool::new(false);
    std::thread::scope(|scope| {
        let proxy = scope.spawn(|| {
            let (mut downstream, _) = listener.accept().unwrap();
            let mut upstream = UnixStream::connect(&retained).unwrap();
            for socket in [&downstream, &upstream] {
                socket
                    .set_read_timeout(Some(Duration::from_secs(45)))
                    .unwrap();
                socket
                    .set_write_timeout(Some(Duration::from_secs(5)))
                    .unwrap();
            }
            let mut requests = downstream.try_clone().unwrap();
            let mut destination = upstream.try_clone().unwrap();
            let forward = scope.spawn(move || {
                let _ = std::io::copy(&mut requests, &mut destination);
                let _ = destination.shutdown(std::net::Shutdown::Both);
            });
            while let Some((tag, payload)) = read_frame(&mut upstream) {
                // Requests still reach the real owner, including heartbeats.
                // Only the return path is lost: no EOF or failed write can help.
                if !stalled.load(std::sync::atomic::Ordering::SeqCst) {
                    downstream.write_all(&frame(tag, &payload)).unwrap();
                }
            }
            forward.join().unwrap();
        });
        let (command, status) = service.relay_command(&owner);
        let mut relay = Terminal::stream(command);
        wait_server_status(&status, "connected", true, 1);
        relay.input(b"before stall\n");
        relay.wait_text("before stall");
        stalled.store(true, std::sync::atomic::Ordering::SeqCst);
        // Fresh connections can reach the same owner while this bridge stays open.
        std::fs::remove_file(&endpoint).unwrap();
        std::fs::rename(&retained, &endpoint).unwrap();
        let deadline = Instant::now() + Duration::from_secs(40);
        loop {
            let reading = status.reading();
            if reading.as_ref().is_some_and(|reading| {
                reading["state"] == "connected" && reading["generation"] == 2
            }) {
                break;
            }
            assert!(
                Instant::now() < deadline,
                "Silent bridge never recovered: {reading:?}"
            );
            std::thread::sleep(Duration::from_millis(20));
        }
        relay.input(b"after recovery\n");
        relay.wait_text("after recovery");
        assert_eq!(fixture.wait_file("input"), "before stall\nafter recovery\n");
        assert_eq!(pid_of_process(&fixture, &owner), owner_pid);
        drop(relay.child.stdin.take());
        assert!(relay.wait_exit().success());
        proxy.join().unwrap();
    });
}

#[test]
fn server_heartbeats_keep_a_quiet_provider_attached() {
    let fixture = Fixture::new();
    let owner = fixture.owner(fixture.serve(false, &["/bin/cat"]));
    let service = ServerProof::start(&fixture, false);
    let (command, status) = service.relay_command(&owner);
    let mut relay = Terminal::stream(command);
    wait_server_status(&status, "connected", true, 1);
    std::thread::sleep(Duration::from_secs(32));
    relay.assert_viewing("Quiet provider still answers heartbeats");
    let reading = status.reading().unwrap();
    assert_eq!(reading["state"], "connected");
    assert_eq!(reading["generation"], 1, "No unnecessary reconnect");
    relay.input(b"still attached\n");
    relay.wait_text("still attached");
    drop(relay.child.stdin.take());
    assert!(relay.wait_exit().success());
}

#[test]
fn server_preserves_owner_and_graph_across_restart() {
    server_continuity(false, false);
}

#[test]
fn server_reconnect_preserves_newer_viewers_input_and_geometry() {
    // Exercise both byte-counted Resume and fresh terminal-screen recovery.
    for terminal in [false, true] {
        let fixture = Fixture::new();
        let owner = fixture.owner(fixture.serve(terminal, &[
            "/bin/sh", "-c",
            "printf 'ready-for-input\\n'; read -r answer; printf '%s' \"$answer\" > \"$HOME/answer\"; if [ -t 0 ]; then stty size > \"$HOME/size\"; fi",
        ]));
        let mut service = ServerProof::start(&fixture, false);
        let (command, status) = service.relay_command(&owner);
        let mut relay = if terminal {
            Terminal::spawn(command)
        } else {
            Terminal::stream(command)
        };
        relay.wait_text("ready-for-input");
        service.stop();
        wait_server_status(&status, "reconnecting", false, 2);

        let mut newer = fixture.attach(
            &owner,
            json!({
                "kind":"attach", "control":true, "replace":true, "own":"take", "rows":31, "cols":99,
            }),
        );
        newer.request(json!({"kind":"resize","rows":45,"cols":115}));
        relay.input(b"uncertain typing");
        service.child = Some(service.launch());
        service.ready();
        wait_server_status(&status, "connected", false, 2);
        relay.assert_viewing("Recovered observer");
        relay.input(b"must not reach the provider\n");
        let mut observer = fixture.connect(&owner);
        observer.request(
            json!({"kind":"attach", "control":false, "replace":false, "rows":24, "cols":80}),
        );
        let reply = observer.until_reply();
        assert_eq!(reply["owner"], newer.id, "reconnect cannot steal geometry");
        assert!(reply["token"].is_null());
        newer.input("newer viewer\n");
        newer.finish();
        assert_eq!(fixture.wait_file("answer"), "newer viewer");
        assert!(relay.wait_exit().success());
        if terminal {
            assert_eq!(fixture.wait_file("size").trim(), "45 115");
            assert!(String::from_utf8_lossy(&relay.screen.lock().unwrap()).contains("newer viewer"));
        }
        if !terminal {
            let screen = relay.screen.lock().unwrap();
            assert_eq!(
                String::from_utf8_lossy(&screen)
                    .matches("ready-for-input")
                    .count(),
                1,
                "Resume must continue at the received byte, not replay the initial output"
            );
        }
    }
}

#[test]
fn server_move_here_lost_ack_preserves_newer_claim_and_discards_input() {
    for terminal in [false, true] {
        server_move_lost_ack(terminal);
    }
}

fn server_move_lost_ack(terminal: bool) {
    let fixture = Fixture::new();
    let owner = fixture.owner(fixture.serve(
        terminal,
        &[
            "/bin/sh",
            "-c",
            "printf 'ready\\n'; read -r answer; printf '%s' \"$answer\" > \"$HOME/answer\"",
        ],
    ));
    let owner_pid = pid_of_process(&fixture, &owner);
    let mut service = ServerProof::start(&fixture, false);
    let (command, status) = service.relay_command(&owner);
    let mut relay = if terminal {
        Terminal::spawn(command)
    } else {
        Terminal::stream(command)
    };
    wait_server_status(&status, "connected", true, 1);
    service.stop();
    wait_server_status(&status, "reconnecting", false, 2);
    let _other = fixture.viewer(&owner, true);

    // Intercept the real owner's takeover acknowledgement, after it has applied
    // the claim. Recovery will go directly to that same owner again.
    let endpoint = fixture.endpoint(&owner);
    let retained = endpoint.with_extension("retained");
    std::fs::rename(&endpoint, &retained).unwrap();
    let listener = UnixListener::bind(&endpoint).unwrap();
    let (ack_send, ack_receive) = std::sync::mpsc::sync_channel(1);
    let (close_send, close_receive) = std::sync::mpsc::sync_channel(1);
    std::thread::scope(|scope| {
        let proxy_retained = retained.clone();
        let proxy = scope.spawn(move || {
            let (mut downstream, _) = listener.accept().unwrap();
            let mut upstream = UnixStream::connect(&proxy_retained).unwrap();
            for socket in [&downstream, &upstream] {
                socket
                    .set_read_timeout(Some(Duration::from_secs(15)))
                    .unwrap();
                socket
                    .set_write_timeout(Some(Duration::from_secs(15)))
                    .unwrap();
            }
            let mut requests = downstream.try_clone().unwrap();
            let mut destination = upstream.try_clone().unwrap();
            let forward = scope.spawn(move || {
                let _ = std::io::copy(&mut requests, &mut destination);
            });
            while let Some((tag, payload)) = read_frame(&mut upstream) {
                if tag == REPLY {
                    let reply: Value = serde_json::from_slice(&payload).unwrap();
                    if reply["kind"] == "attached" && reply["token"].is_string() {
                        ack_send.send(()).unwrap();
                        close_receive.recv_timeout(Duration::from_secs(15)).unwrap();
                        break;
                    }
                }
                downstream.write_all(&frame(tag, &payload)).unwrap();
            }
            downstream.shutdown(std::net::Shutdown::Both).unwrap();
            upstream.shutdown(std::net::Shutdown::Both).unwrap();
            forward.join().unwrap();
        });
        service.child = Some(service.launch());
        service.ready();
        wait_server_status(&status, "connected", false, 2);
        request_server_move(&status);
        ack_receive.recv_timeout(Duration::from_secs(15)).unwrap();
        wait_server_status(&status, "moving", false, 3);
        relay.input(b"must not be replayed");
        let mut newer = Viewer::new(UnixStream::connect(&retained).unwrap()).attach(json!({
            "kind":"attach", "control":true, "replace":true, "own":"take", "rows":31, "cols":99,
        }));
        std::fs::remove_file(&endpoint).unwrap();
        std::fs::rename(&retained, &endpoint).unwrap();
        // Even a delayed duplicate of the consumed action cannot cross recovery.
        status.send(json!({"kind":"move", "generation":2}));
        close_send.send(()).unwrap();
        proxy.join().unwrap();
        wait_server_status(&status, "connected", false, 4);
        let mut observer = fixture.connect(&owner);
        observer.request(
            json!({"kind":"attach", "control":false, "replace":false, "rows":24, "cols":80}),
        );
        assert_eq!(observer.until_reply()["owner"], newer.id);
        assert_eq!(pid_of_process(&fixture, &owner), owner_pid);
        newer.input("newer viewer\n");
        newer.finish();
        assert_eq!(fixture.wait_file("answer"), "newer viewer");
        assert!(relay.wait_exit().success());
    });
}

#[test]
fn server_move_here_pending_action_is_cancelled_by_pane_closure() {
    let fixture = Fixture::new();
    let owner = fixture.owner(fixture.serve(false, &["/bin/sh", "-c", "read -r answer"]));
    let mut service = ServerProof::start(&fixture, false);
    let directory = fixture.home.path().join("pane");
    std::fs::create_dir(&directory).unwrap();
    let status = ConnectionStatus::new(directory.join("status.sock"));
    let (mut command, _) = service.relay_command(&owner);
    command.env("LF_SERVER_STATUS", &status.path);
    let mut relay = Terminal::stream(command);
    wait_server_status(&status, "connected", true, 1);
    service.stop();
    wait_server_status(&status, "reconnecting", false, 2);
    let mut newer = fixture.attach(
        &owner,
        json!({
            "kind":"attach", "control":true, "replace":true, "own":"take", "rows":31, "cols":99,
        }),
    );
    service.child = Some(service.launch());
    service.ready();
    wait_server_status(&status, "connected", false, 2);
    let pid = relay.child.id() as libc::pid_t;
    // SAFETY: stop only this fixture's relay so closure precedes consumption.
    assert_eq!(unsafe { libc::kill(pid, libc::SIGSTOP) }, 0);
    let paused = PausedOwner(pid);
    let mut stopped = 0;
    // SAFETY: waitpid observes our child's stop; stopped is writable.
    assert_eq!(
        unsafe { libc::waitpid(pid, &mut stopped, libc::WUNTRACED) },
        pid
    );
    request_server_move(&status);
    let closed = directory.with_extension("closed");
    std::fs::rename(&directory, &closed).unwrap();
    std::fs::remove_dir_all(closed).unwrap();
    drop(paused);
    assert!(relay.wait_exit().success());
    let mut observer = fixture.connect(&owner);
    observer
        .request(json!({"kind":"attach", "control":false, "replace":false, "rows":24, "cols":80}));
    assert_eq!(observer.until_reply()["owner"], newer.id);
    newer.input("still controlled\n");
    newer.finish();
}

#[test]
fn server_reconnect_refreshes_expired_output_without_replaying_input() {
    let fixture = Fixture::new();
    let owner = fixture.owner(fixture.serve(
        false,
        &[
            "/bin/sh",
            "-c",
            r#"
printf 'ready-for-outage\n'
while [ ! -f "$HOME/release" ]; do sleep 0.02; done
dd if=/dev/zero bs=65536 count=20 2>/dev/null | tr '\000' x
printf '\ncurrent-output\n'
printf done > "$HOME/filled"
read -r answer
printf '%s' "$answer" > "$HOME/answer"
"#,
        ],
    ));
    let mut service = ServerProof::start(&fixture, false);
    let (command, status) = service.relay_command(&owner);
    let mut relay = Terminal::stream(command);
    relay.wait_text("ready-for-outage");
    service.stop();
    wait_server_status(&status, "reconnecting", false, 2);
    std::fs::write(fixture.home.path().join("release"), "go").unwrap();
    fixture.wait_file("filled");
    relay.input(b"discarded");
    service.child = Some(service.launch());
    service.ready();
    wait_server_status(&status, "connected", true, 2);
    relay.wait_text("current-output");
    relay.input(b"fresh input\n");
    assert_eq!(fixture.wait_file("answer"), "fresh input");
    assert!(relay.wait_exit().success());
}

#[test]
fn server_reconnect_reports_unavailable_without_starting_another() {
    let fixture = Fixture::new();
    let path = fixture.provider(
        r#"#!/bin/sh
if [ "$1" = --version ]; then echo '2.1.302'; exit; fi
printf '%s\n' "$$" >> "$HOME/launches"
printf 'provider-ready\n'
read -r answer < /dev/tty
"#,
    );
    let owner = fixture.owner(fixture.native(Path::new(env!("CARGO_BIN_EXE_lf")), &path, "hello"));
    let first = fixture.launches(1);
    let mut service = ServerProof::start(&fixture, false);
    let (mut command, status) = service.relay_command(&owner);
    command.env("PATH", &path);
    let mut relay = Terminal::spawn(command);
    relay.wait_text("provider-ready");
    service.stop();
    wait_server_status(&status, "reconnecting", false, 2);
    fixture.stop(&owner);
    fixture.ends(&owner);
    service.child = Some(service.launch());
    service.ready();
    wait_server_status(&status, "unavailable", false, 2);
    relay.assert_viewing("Unavailable keeps the last screen");
    relay.input(&[0x1d]);
    assert!(relay.wait_exit().success());
    assert_eq!(
        fixture.launches(1),
        first,
        "connection recovery cannot launch or resume a provider"
    );
}

#[test]
fn server_reconnect_retries_after_exhaustion_without_sending_retry_to_provider() {
    let fixture = Fixture::new();
    let owner = fixture.owner(fixture.serve(
        false,
        &[
            "/bin/sh",
            "-c",
            "printf 'ready\\n'; read -r answer; printf '%s' \"$answer\" > \"$HOME/answer\"",
        ],
    ));
    let mut service = ServerProof::start(&fixture, false);
    let (command, status) = service.relay_command(&owner);
    let mut relay = Terminal::stream(command);
    relay.wait_text("ready");
    service.stop();
    wait_server_status(&status, "unavailable", false, 2);
    relay.input(b"discarded");
    service.child = Some(service.launch());
    service.ready();
    status.send(json!({"kind":"retry"}));
    wait_server_status(&status, "connected", true, 2);
    relay.input(b"submitted\n");
    assert_eq!(fixture.wait_file("answer"), "submitted");
    assert!(relay.wait_exit().success());
}

#[test]
fn server_failed_setup_retry_only_reconnects_and_preserves_newer_claims() {
    for terminal in [false, true] {
        let fixture = Fixture::new();
        let owner = fixture.owner(fixture.serve(terminal, &[
            "/bin/sh", "-c",
            "printf '%s\\n' \"$$\" >> \"$HOME/launches\"; printf 'retained-owner-screen\\n'; read -r answer; printf '%s' \"$answer\" > \"$HOME/answer\"",
        ]));
        let provider = fixture.launches(1);
        let owner_pid = pid_of_process(&fixture, &owner);
        let mut service = ServerProof::start(&fixture, false);
        service.stop();
        let installer = fixture.home.path().join("installer");
        std::fs::write(
            &installer,
            r#"#!/bin/sh
printf 'attempt\n' >> "$HOME/setup-attempts"
if [ -f "$HOME/setup-allowed" ]; then
    touch "$HOME/registered-again"
    exit 0
fi
echo 'fixture Background Items disabled' >&2
exit 1
"#,
        )
        .unwrap();
        std::fs::set_permissions(&installer, std::fs::Permissions::from_mode(0o755)).unwrap();
        let (mut command, status) = service.relay_command(&owner);
        command.env("LF_BIN", installer);
        let mut relay = if terminal {
            Terminal::spawn(command)
        } else {
            Terminal::stream(command)
        };
        wait_server_status(&status, "unavailable", false, 1);
        assert!(status.reading().unwrap()["message"]
            .as_str()
            .unwrap()
            .contains("fixture Background Items disabled"));

        // Setup would now succeed, but Retry must not invoke it. Another viewer
        // takes both claims before this pane has ever connected successfully.
        std::fs::write(fixture.home.path().join("setup-allowed"), "").unwrap();
        let mut newer = fixture.attach(
            &owner,
            json!({
                "kind":"attach", "control":true, "replace":true, "own":"take", "rows":31, "cols":99,
            }),
        );
        relay.input(b"discarded during failed setup");
        status.send(json!({"kind":"retry"}));
        wait_server_status(&status, "connecting", false, 1);
        wait_server_status(&status, "unavailable", false, 1);
        assert!(status.reading().unwrap()["message"]
            .as_str()
            .unwrap()
            .contains("fixture Background Items disabled"));
        assert_eq!(fixture.wait_file("setup-attempts"), "attempt\n");
        assert!(!fixture.home.path().join("registered-again").exists());

        // An independent installation repair starts the service. The same pane
        // recovers the exact owner, without repeating setup or taking control.
        service.child = Some(service.launch());
        service.ready();
        status.send(json!({"kind":"retry"}));
        wait_server_status(&status, "connected", false, 1);
        relay.wait_text("retained-owner-screen");
        relay.assert_viewing("Failed initial setup recovers read-only");
        relay.input(b"discarded after recovery\n");
        let mut observer = fixture.connect(&owner);
        observer.request(
            json!({"kind":"attach", "control":false, "replace":false, "rows":24, "cols":80}),
        );
        let reply = observer.until_reply();
        assert_eq!(reply["owner"], newer.id);
        assert!(reply["token"].is_null());
        assert_eq!(pid_of_process(&fixture, &owner), owner_pid);
        newer.input("newer viewer\n");
        newer.finish();
        assert_eq!(fixture.wait_file("answer"), "newer viewer");
        assert!(relay.wait_exit().success());
        assert_eq!(fixture.launches(1), provider);
        assert_eq!(fixture.wait_file("setup-attempts"), "attempt\n");
        assert!(!fixture.home.path().join("registered-again").exists());
    }
}

#[test]
fn server_startup_cancels_its_pending_installer_when_pane_closes() {
    let fixture = Fixture::new();
    let directory = fixture.home.path().join("native-pane");
    std::fs::create_dir(&directory).unwrap();
    let installer = fixture.home.path().join("installer");
    std::fs::write(
        &installer,
        "#!/bin/sh\nprintf '%s' \"$$\" > \"$HOME/installer-pid\"\nexec sleep 60\n",
    )
    .unwrap();
    std::fs::set_permissions(&installer, std::fs::Permissions::from_mode(0o755)).unwrap();
    let owner = uuid::Uuid::new_v4().to_string();
    let mut command = fixture.command(&[
        "session",
        "attach",
        &owner,
        "--session",
        "session_5e55105e000040008000000000000009",
    ]);
    command
        .env("LF_BIN", &installer)
        .env("LF_SERVER", "1")
        .env("LF_SERVER_STATUS", directory.join("status.json"));
    let mut relay = Terminal::stream(command);
    let pid = fixture.wait_file("installer-pid").parse::<i32>().unwrap();
    std::fs::remove_dir_all(directory).unwrap();
    assert!(relay.wait_exit().success());
    // SAFETY: signal zero only inspects the fixture's recorded installer PID.
    assert_eq!(
        unsafe { libc::kill(pid, 0) },
        -1,
        "cancelled setup must be reaped"
    );
}

#[test]
fn owned_program_does_not_inherit_the_desktop_pane_relay() {
    let fixture = Fixture::new();
    let mut serve = fixture.serve(
        false,
        &[
            "/bin/sh",
            "-c",
            "printf '%s %s' \"${LF_SERVER-unset}\" \"${LF_SERVER_STATUS-unset}\" > \"$HOME/relay-env\"; read -r answer",
        ],
    );
    serve
        .env("LF_SERVER", "1")
        .env("LF_SERVER_STATUS", fixture.home.path().join("status.json"));
    fixture.owner(serve);
    assert_eq!(fixture.wait_file("relay-env"), "unset unset");
}

#[test]
fn server_reconnect_cancels_when_native_pane_closes() {
    let fixture = Fixture::new();
    let owner = fixture.owner(fixture.serve(
        false,
        &["/bin/sh", "-c", "printf 'ready\\n'; read -r answer"],
    ));
    let mut service = ServerProof::start(&fixture, false);
    let directory = fixture.home.path().join("native-pane");
    std::fs::create_dir(&directory).unwrap();
    let status = ConnectionStatus::new(directory.join("status.sock"));
    let mut command = fixture.command(&[
        "session",
        "attach",
        &owner,
        "--session",
        &fixture.bound_session(&owner),
    ]);
    command
        .env("LF_SERVER", "1")
        .env("LF_SERVER_STATUS", &status.path);
    let mut relay = Terminal::stream(command);
    wait_server_status(&status, "connected", true, 1);
    service.stop();
    wait_server_status(&status, "reconnecting", false, 2);
    let closed = directory.with_extension("closed");
    std::fs::rename(&directory, &closed).unwrap();
    std::fs::remove_dir_all(closed).unwrap();
    assert!(relay.wait_exit().success());
    // The pane owns cancellation, never the provider's lifetime.
    assert!(pid_of_process(&fixture, &owner) > 0);
}

#[cfg(target_os = "macos")]
#[test]
#[ignore = "registers and removes an isolated launchd job; run explicitly outside network sandbox"]
fn installed_server_preserves_owner_and_graph_across_replacement() {
    server_continuity(true, false);
}

#[test]
fn server_move_here_preserves_draft_provider_and_graph() {
    server_continuity(false, true);
}

fn server_continuity(supervised: bool, takeover: bool) {
    let fixture = Fixture::new();
    let path = fixture.provider(
        r#"#!/bin/sh
if [ "$1" = --version ]; then echo '2.1.302'; exit; fi
stty echo < /dev/tty
printf '%s:%s:%s\n' "$$" "$PPID" "$LF_CAPTURE_KEY" >> "$HOME/launches"
printf 'server-proof-provider-ready\n'
read -r answer < /dev/tty
printf '%s\n' "$answer" >> "$HOME/answers"
"#,
    );
    fixture
        .repo
        .create_file(".lf/skills/probe.md", "Do the fixture work.");
    fixture
        .repo
        .create_file(".lf/graphs/server-proof.yaml", "- probe\n- probe\n");
    let mut service = ServerProof::start(&fixture, supervised);
    let mut command = service.command(&["session", "serve", "--terminal", "--"]);
    command
        .arg(&service.executable)
        .args(["-i", "-a", "claude", "graph", "server-proof"])
        .env("PATH", path);
    let owner = fixture.owner(command);
    let first = fixture.launches(1);
    let owner_pid = pid_of_process(&fixture, &owner);
    let first_instance = service.ready();
    let mut viewer = service.viewer(&owner);
    viewer.input("draft");
    // Observe delivery before disconnecting: bytes still in the bridge are
    // uncertain input, not a draft the provider has received.
    until_out(&mut viewer, "draft");
    drop(viewer); // Desktop quit: the owner keeps its native editor.
    let (command, status) = service.relay_command(&owner);
    let mut relay = Terminal::spawn(command);
    relay.wait_text("server-proof-provider-ready");
    fixture.wait_controlled(&owner);
    let newer =
        if takeover {
            service.stop();
            wait_server_status(&status, "reconnecting", false, 2);
            Some(fixture.attach(&owner, json!({
            "kind":"attach", "control":true, "replace":true, "own":"take", "rows":31, "cols":99,
        })))
        } else {
            None
        };
    service.restart();

    let deadline = Instant::now() + Duration::from_secs(20);
    while service.ready() == first_instance {
        assert!(Instant::now() < deadline);
        std::thread::sleep(Duration::from_millis(20));
    }
    assert_eq!(pid_of_process(&fixture, &owner), owner_pid);
    assert_eq!(
        fixture.launches(1),
        first,
        "reconnect must not launch another provider"
    );
    wait_server_status(&status, "connected", !takeover, 2);
    let deadline = Instant::now() + Duration::from_secs(5);
    while status.reading().unwrap()["native_ready"] != true {
        assert!(
            Instant::now() < deadline,
            "Native readiness must follow the live attachment"
        );
        std::thread::sleep(Duration::from_millis(20));
    }
    if takeover {
        relay.input(b"discard this uncertain input");
        request_server_move(&status);
        wait_server_status(&status, "connected", true, 3);
        let mut observer = fixture.connect(&owner);
        observer.request(json!({
            "kind":"attach", "control":false, "replace":false, "rows":24, "cols":80,
        }));
        let reply = observer.until_reply();
        assert!(reply["owner"].is_string());
        assert_ne!(reply["owner"], newer.as_ref().unwrap().id);
        assert_eq!(pid_of_process(&fixture, &owner), owner_pid);
        assert_eq!(fixture.launches(1), first);
    }
    let screen = relay.screen.lock().unwrap().clone();
    assert!(!String::from_utf8_lossy(&screen).contains("Session connection lost"));
    relay.input(b" kept\n");
    fixture.launches(2); // The existing Graph advanced exactly once.
    relay.input(b"done\n");
    fixture.ends(&owner);
    assert!(relay.wait_exit().success());
    assert_eq!(fixture.launches(2).len(), 2);
    assert_eq!(
        std::fs::read_to_string(fixture.home.path().join("answers")).unwrap(),
        "draft kept\ndone\n"
    );
}

#[test]
fn exact_owner_refuses_a_replacement_while_session_attach_finds_it() {
    let fixture = Fixture::new();
    let old = fixture.start(&["/bin/cat"]);
    let session = fixture.bound_session(&old);
    fixture.stop(&old);
    let current = fixture.start(&["/bin/cat"]);
    fixture
        .sql()
        .execute(
            "UPDATE agent_sessions SET agent_process_id=?2,transport_lf_process_id=?2 WHERE id=?1",
            rusqlite::params![session, current],
        )
        .unwrap();
    let service = ServerProof::start(&fixture, false);
    let mut client = service.connect();
    let before: i64 = fixture
        .sql()
        .query_row("SELECT count(*) FROM processes", [], |r| r.get(0))
        .unwrap();
    let refused = client
        .request(
            serde_json::from_value(json!({
                "kind":"session.attach","session":session,"owner_lf_process_id":old
            }))
            .unwrap(),
        )
        .unwrap_err();
    let error = refused.downcast_ref::<loopflow::engine::Error>().unwrap();
    assert_eq!(error.code, loopflow::engine::ErrorCode::NotFound);
    assert_eq!(error.data.as_ref().unwrap()["owner_lf_process_id"], old);
    assert_eq!(
        error.data.as_ref().unwrap()["current_lf_process_id"],
        current
    );
    let attached = client
        .request(
            serde_json::from_value(json!({
                "kind":"session.attach","session":session,"owner_lf_process_id":null
            }))
            .unwrap(),
        )
        .unwrap()
        .result;
    assert_eq!(attached["owner_lf_process_id"], current);
    assert_eq!(
        fixture
            .sql()
            .query_row("SELECT count(*) FROM processes", [], |r| r.get::<_, i64>(0))
            .unwrap(),
        before
    );
}

#[test]
fn a_stalled_bridge_leaves_the_healthy_bridge_and_control_checkpoint_live() {
    let fixture = Fixture::new();
    let owner = fixture.owner(fixture.serve(false, &["/bin/sh", "-c",
        "i=0; while [ $i -lt 64 ]; do read -r line; dd if=/dev/zero bs=65536 count=1 2>/dev/null; i=$((i+1)); done"]));
    let service = ServerProof::start(&fixture, false);
    let _stalled = service.viewer(&owner);
    let mut active = service.viewer(&owner);
    let mut client = service.connect();
    let subscription = client
        .subscribe(loopflow::lf::commands::read::Read::ProcessList)
        .unwrap();
    active.input("next\n");
    let mut bytes = 0;
    while bytes < 4 * 1024 * 1024 {
        if let Item::Out(part) = active.next().unwrap() {
            bytes += part.len();
            if bytes % 65536 == 0 && bytes < 4 * 1024 * 1024 {
                active.input("next\n");
            }
            if bytes == 2 * 1024 * 1024 {
                assert!(client
                    .request(serde_json::from_value(json!({"kind":"config.catalog"})).unwrap())
                    .is_ok());
                client.checkpoint(&subscription, 77).unwrap();
                loop {
                    if client.reading(&subscription).unwrap().unwrap()["answers"] == 77 {
                        break;
                    }
                }
            }
        }
    }
    assert_eq!(bytes, 4 * 1024 * 1024);
}

#[test]
fn grace_expiry_refuses_bytes_before_repainting_the_terminal_screen() {
    let fixture = Fixture::new();
    let owner = fixture.start(&["/bin/sh", "-c", ECHO_LINES]);
    fixture.wait_file("drawn");
    let service = ServerProof::start(&fixture, false);
    let first = service.viewer(&owner);
    let reconnect = first.reconnect.clone();
    let received = first.received;
    drop(first);
    let mut newer = service.viewer(&owner);
    newer.input("after outage\n");
    until_out(&mut newer, "line after outage");
    // Exercise the production grace without adding a runtime test setting.
    let deadline = Instant::now() + Duration::from_secs(61);
    while Instant::now() < deadline {
        newer.request(json!({"kind":"controlled"}));
        newer.until_reply();
        std::thread::sleep(Duration::from_secs(1));
    }
    let session = fixture.bound_session(&owner);
    let socket = loopflow::server::client::attach(
        &fixture.endpoint("machine"),
        &session.as_str().into(),
        None,
    )
    .unwrap();
    let mut back = Viewer::new(socket);
    back.request(json!({"kind":"resume","reconnect":reconnect,"received":received,"control":false,"own":null,"rows":24,"cols":80}));
    match back.next().unwrap() {
        Item::Reply(reply) => assert_eq!(reply["kind"], "reconnect_required"),
        other => panic!("output arrived before refusal: {other:?}"),
    }
    back.request(
        json!({"kind":"attach","control":false,"replace":false,"own":null,"rows":24,"cols":80}),
    );
    let reply = back.until_reply();
    assert_eq!(reply["owner"], newer.id);
    back.accept(&reply);
    let Some(Item::Snapshot(bytes)) = back.next() else {
        panic!("fresh attach needs a snapshot");
    };
    let mut screen = ghostty_vt::Terminal::new(80, 24).unwrap();
    screen.write(&bytes);
    assert!(screen.text().unwrap().contains("after outage"));
    newer.input("done\n");
    newer.finish();
}
