//! Real detached owner and Session attachment. Ordinary tests need no provider
//! account or display; ignored native acceptance explicitly opts into them.
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
    /// A holder outlives every server, so a test that fails before its agent
    /// ends would leave the holder and that agent running. Stop each holder
    /// this fixture's store recorded; its agents lose their PTY with it.
    fn drop(&mut self) {
        loopflow_test_support::end_recorded_processes(
            self.home.path(),
            Path::new(env!("CARGO_BIN_EXE_lf")),
        );
        let Ok(sql) = rusqlite::Connection::open(self.home.path().join("loopflow.db")) else {
            return;
        };
        let holders: Vec<libc::pid_t> = sql
            .prepare(
                r#"SELECT pid FROM processes WHERE command='["lf","holder","run"]'
                   AND completed_at IS NULL AND pid IS NOT NULL"#,
            )
            .and_then(|mut query| {
                query
                    .query_map([], |row| row.get(0))?
                    .collect::<rusqlite::Result<_>>()
            })
            .unwrap_or_default();
        for pid in holders {
            if command_line(pid).1.contains("holder run") {
                // SAFETY: a holder this fixture's servers started and recorded.
                unsafe { libc::kill(pid, libc::SIGKILL) };
            }
        }
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
        let fixture = Self::uninitialized();
        support::seed_store(fixture.home.path());
        fixture
    }

    fn uninitialized() -> Self {
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
        self.named_provider("claude", script)
    }
    fn named_provider(&self, name: &str, script: &str) -> String {
        let bin = self.home.path().join("bin");
        std::fs::create_dir_all(&bin).unwrap();
        let provider = bin.join(name);
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
    /// The held terminal that hosts `session`.
    fn terminal_of(&self, session: &str) -> String {
        self.sql()
            .query_row(
                "SELECT held_terminal_id FROM agent_sessions WHERE id=?1",
                [session],
                |r| r.get(0),
            )
            .unwrap()
    }
    /// Name `terminal` at the server's entrance, as every view does: the
    /// connection and the entrance's answer, from a server of this build.
    fn held_entrance(&self, terminal: &str) -> (UnixStream, Value) {
        let mut socket = UnixStream::connect(self.endpoint("held")).unwrap();
        socket
            .write_all(
                format!(
                    "{}\n",
                    json!({ "terminal": terminal, "revision": HELD_REVISION })
                )
                .as_bytes(),
            )
            .unwrap();
        let mut answer = Vec::new();
        let mut byte = [0];
        while socket.read(&mut byte).unwrap() == 1 && byte[0] != b'\n' {
            answer.push(byte[0]);
        }
        let answer: Value = serde_json::from_slice(&answer).unwrap();
        assert_eq!(answer["revision"], HELD_REVISION, "{answer}");
        (socket, answer["answer"].clone())
    }
    /// Wait until a viewer holds input control of a held terminal.
    fn wait_held_controlled(&self, terminal: &str) {
        let deadline = Instant::now() + Duration::from_secs(30);
        loop {
            let (mut socket, answer) = self.held_entrance(terminal);
            assert_eq!(answer, "hosted", "{terminal} is not hosted");
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
                "{terminal} never had a controlling viewer"
            );
            std::thread::sleep(Duration::from_millis(50));
        }
    }
    /// A held terminal the server still hosts, unended.
    fn assert_held_hosted(&self, terminal: &str) {
        let (_, answer) = self.held_entrance(terminal);
        assert_eq!(answer, "hosted", "{terminal} is not hosted");
        let ended: bool = self
            .sql()
            .query_row(
                "SELECT ended_at IS NOT NULL FROM held_terminals WHERE id=?1",
                [terminal],
                |r| r.get(0),
            )
            .unwrap();
        assert!(!ended, "{terminal} ended");
    }
    /// Wait until a held terminal's exit is recorded.
    fn held_ends(&self, terminal: &str) {
        let deadline = Instant::now() + Duration::from_secs(20);
        while !self
            .sql()
            .query_row(
                "SELECT ended_at IS NOT NULL FROM held_terminals WHERE id=?1",
                [terminal],
                |r| r.get::<_, bool>(0),
            )
            .unwrap()
        {
            assert!(Instant::now() < deadline, "{terminal} never ended");
            std::thread::sleep(Duration::from_millis(50));
        }
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
/// The revision of what follows the held-terminal entrance's answer.
const HELD_REVISION: u32 = 1;
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
fn prepared_start_preserves_a_claim_time_winner() {
    let intent = "--start-if-free";
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
    let mut winner = fixture.viewer(&owner, false);
    let endpoint = fixture.endpoint(&owner);
    let retained = endpoint.with_extension("retained");
    // Interpose only at the socket boundary: all ownership reads see the
    // free real owner, then the competing viewer claims before Attach arrives.
    std::fs::rename(&endpoint, &retained).unwrap();
    let listener = UnixListener::bind(&endpoint).unwrap();
    let backend = retained.clone();
    let race = std::thread::spawn(move || loop {
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
    });
    let opened = fixture.prepared_command(&prepared).output().unwrap();
    assert!(
        !opened.status.success(),
        "opening stole a competing claim: {opened:?}"
    );
    let mut controller = race.join().unwrap();
    std::fs::remove_file(&endpoint).unwrap();
    std::fs::rename(retained, endpoint).unwrap();
    assert_eq!(fixture.wait_file("launches"), launched);
    assert!(!fixture.home.path().join("answer").exists());
    controller.input("original-controller\n");
    controller.finish();
    let pid = launched.trim().split(':').next().unwrap();
    assert_eq!(
        fixture.wait_file("answer").trim(),
        format!("{pid}:original-controller")
    );
}

/// Viewers on pipes: end of input still delivers everything typed before the
/// command exits, and a viewer stream keeps input, size and exit status.
#[test]
fn pipe_viewers_deliver_input_size_and_exit() {
    pipe_eof_delivers_all_input_before_the_command_exits();
    viewer_stream_over_pipes_preserves_terminal_input_size_and_exit();
}

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

#[test]
fn first_native_launch_keeps_only_valid_desktop_terminal_placement() {
    for valid in [true, false] {
        let fixture = Fixture::new();
        let path = fixture.waiting_provider();
        // A bare launch runs directly, from a Desktop pane too.
        let mut command = fixture.command(&["-i", "-a", "claude", ":", "terminal placement"]);
        command.env("PATH", path).env("LF_SERVER", "1");
        let terminal = Terminal::spawn_with_placement(command, Some(valid));
        let launch = fixture.launches(1).pop().unwrap();
        let session = session_of(&fixture.sql(), &launch);
        // The launching lf, which stays recorded after the provider exits and
        // its attachment is released.
        let placement: Option<String> = fixture.sql().query_row(
            "SELECT p.terminal_id FROM agent_sessions s JOIN processes a ON a.id=s.agent_process_id JOIN processes p ON p.id=a.parent_lf_process_id WHERE s.id=?1",
            [&session], |row| row.get(0),
        ).unwrap();
        // A bare launch runs in its terminal: no owner serves it.
        let owner: Option<String> = fixture
            .sql()
            .query_row(
                "SELECT transport_lf_process_id FROM agent_sessions WHERE id=?1",
                [&session],
                |row| row.get(0),
            )
            .unwrap();
        terminal.stop();
        assert_eq!(owner, None);
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

/// Unviewed and quiet, a held terminal runs until its agent exits or Task
/// cleanup ends it. The lf process that takes up its handoff names it, since
/// a terminal is no process, and records the Desktop pane it launched from.
/// A later view shows the conversation's saved screen, carried across the
/// relay's exec, until the resumed provider's own output replaces it.
#[test]
fn a_held_conversation_outlives_its_view() {
    let fixture = Fixture::new();
    let _service = ServerProof::start(&fixture, false);
    let path = fixture.waiting_provider();
    // The repository's conversation is managed, so a terminal viewing it holds it.
    let mut command = fixture.command(&["-a", "claude", "resume"]);
    command.env("PATH", &path);
    let view = Terminal::spawn_with_placement(command, Some(true));
    let launch = fixture.launches(1).pop().unwrap();
    let provider: libc::pid_t = launch.split(':').next().unwrap().parse().unwrap();
    let _provider = KillOnDrop(provider);
    let session = session_of(&fixture.sql(), &launch);
    let held = fixture.terminal_of(&session);
    let (named, placement): (Option<String>, Option<String>) = fixture
        .sql()
        .query_row(
            "SELECT p.held_terminal_id, p.terminal_id FROM agent_sessions s
             JOIN processes a ON a.id=s.agent_process_id
             JOIN processes p ON p.id=a.attached_lf_process_id WHERE s.id=?1",
            [&session],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )
        .unwrap();
    assert_eq!(named.as_deref(), Some(held.as_str()));
    assert_eq!(placement.as_deref(), Some("desktop-shell"));
    view.stop();
    // The view's process is gone; the terminal's host still answers for it.
    fixture.assert_held_hosted(&held);
    assert!(alive(provider), "the held agent ended with its view");
    // SAFETY: the pid the fixture's provider recorded for itself.
    unsafe { libc::kill(provider, libc::SIGKILL) };
    fixture.held_ends(&held);

    let mut saved = ghostty_vt::Terminal::new(80, 24).unwrap();
    saved.write(b"\x1b[?1049hsaved preview");
    let mut bytes =
        format!("lf-terminal-snapshot/1 ghostty={}\n", ghostty_vt::REVISION).into_bytes();
    bytes.extend(saved.encode().unwrap());
    let screens = fixture.home.path().join("screens");
    std::fs::create_dir_all(&screens).unwrap();
    std::fs::write(screens.join(format!("{session}.ghostsnp")), bytes).unwrap();
    let mut command = fixture.command(&["-a", "claude", "resume", &session]);
    command.env("PATH", &path);
    let mut view = Terminal::spawn_with_placement(command, Some(true));
    let resumed = fixture.launches(2).pop().unwrap();
    let provider: libc::pid_t = resumed.split(':').next().unwrap().parse().unwrap();
    let _provider = KillOnDrop(provider);
    assert!(
        resumed.contains(" --resume ") && session_of(&fixture.sql(), &resumed) == session,
        "{resumed}"
    );
    view.wait_text("ready for");
    let screen = view.screen.lock().unwrap().clone();
    let text = String::from_utf8_lossy(&screen);
    let preview = text
        .find("saved preview")
        .unwrap_or_else(|| panic!("no saved screen: {text}"));
    assert!(
        preview < text.rfind("ready for").unwrap(),
        "the saved screen came after live output: {text}"
    );
    let mut display = ghostty_vt::Terminal::new(80, 24).unwrap();
    display.write(&screen);
    let shown = display.text().unwrap();
    assert!(!display.alternate_screen(), "{shown}");
    assert!(!shown.contains("saved preview"), "{shown}");
    view.stop();
    // SAFETY: the pid the fixture's provider recorded for itself.
    unsafe { libc::kill(provider, libc::SIGKILL) };
    fixture.held_ends(&fixture.terminal_of(&session));
}

#[test]
fn native_pending_request_survives_creator_exit_without_settling_capture_or_replay() {
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
/// when the provider exits. A failed provider settles once with its own code;
/// a failing Graph step returns 1.
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
    let owner_pid = pid_of_process(&fixture, &owner);
    assert_eq!(
        parent,
        owner_pid,
        "a launcher still waits between the owner and its provider: {}",
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
    failed_native_provider_settles_once_with_its_own_code();
    a_failing_graph_step_returns_one();
}

/// A Graph's failing step returns 1, never the provider's code, and records its
/// launcher and capture as failed. `native_exit`'s tests own a handed-off
/// provider's codes.
fn a_failing_graph_step_returns_one() {
    let fixture = Fixture::new();
    let path = parented_provider(&fixture);
    let provider = fixture.home.path().join("bin/claude");
    let mut script = std::fs::read_to_string(&provider).unwrap();
    script.push_str("exit \"$answer\"\n");
    std::fs::write(provider, script).unwrap();
    fixture
        .repo
        .create_file(".lf/skills/probe.md", "Exit proof.");
    fixture
        .repo
        .create_file(".lf/graphs/return.yaml", "- probe\n");
    let mut command = fixture.serve(
        true,
        &[
            env!("CARGO_BIN_EXE_lf"),
            "-i",
            "-a",
            "claude",
            "graph",
            "return",
        ],
    );
    command.env("PATH", path);
    let owner = fixture.owner(command);
    let launched = fixture.launches(1)[0].clone();
    let capture = launched.split(':').nth(2).unwrap();
    let session: String = fixture
        .sql()
        .query_row(
            "SELECT session_id FROM session_events WHERE kind='captured' AND receipt_key=?1",
            [capture],
            |r| r.get(0),
        )
        .unwrap();
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
    viewer.stdin.take().unwrap().write_all(b"7\n").unwrap();
    let result = wait_for_viewer(viewer, Duration::from_secs(30));
    fixture.ends(&owner);
    assert_eq!(result.status.code(), Some(1), "{result:?}");
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
    assert_eq!(capture, "failed");
    assert_eq!(fixture.launches(1).len(), 1);
}

fn failed_native_provider_settles_once_with_its_own_code() {
    let fixture = Fixture::new();
    let path = fixture.provider("#!/bin/sh\nexit 99\n");
    let owner = fixture.owner(fixture.native(
        Path::new(env!("CARGO_BIN_EXE_lf")),
        &path,
        "failed provider",
    ));
    let mut viewer = fixture.viewer(&owner, true);
    let frames = viewer.until_event("child_exited");
    assert_eq!(events(&frames, "child_exited").last().unwrap()["code"], 99);
    let reports = events(&frames, "program_status");
    assert!(reports.last().unwrap()["status"]["records"]
        .as_array()
        .unwrap()
        .iter()
        .any(|report| report["state"] == "error"));
    fixture.ends(&owner);
    assert!(!fixture.home.path().join("launches").exists());
    let sql = fixture.sql();
    let (code, outcome, provider_outcome): (i32, String, Option<String>) = sql.query_row(
        "SELECT launcher.exit_code, launcher.outcome, agent.outcome FROM processes agent JOIN processes launcher ON launcher.id=agent.parent_lf_process_id WHERE agent.kind='agent'",
        [], |r| Ok((r.get(0)?,r.get(1)?,r.get(2)?)),
    ).unwrap();
    assert_eq!(provider_outcome, None);
    assert_eq!(code, 99);
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

/// Each JSON launch starts its provider once and prints one record: a resume
/// that starts the owner, a fork and a new thread from a prepared Session.
#[test]
fn json_launches_start_once_and_return_one_record() {
    resume_json_returns_once_when_it_starts_the_owner();
    prepared_thread_json("resume", true);
    prepared_thread_json("new", false);
}

fn resume_json_returns_once_when_it_starts_the_owner() {
    let fixture = Fixture::new();
    let _server = fixture.machine_server();
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
    let completed: Option<i64> = fixture
        .sql()
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

fn prepared_thread_json(command: &str, fork: bool) {
    let fixture = Fixture::new();
    let _server = fixture.machine_server();
    let bin = fixture.home.path().join("bin");
    // The only executable provider is this stand-in; no native turn is sent.
    fixture.provider(
        "#!/bin/sh\nif [ \"$1\" = --version ]; then echo '2.1.302'; exit; fi\nprintf '%s\\n' \"$LF_CAPTURE_KEY\" >> \"$HOME/launches\"\n",
    );
    let path = format!("{}:/usr/bin:/bin", bin.display());
    let prepared = fixture
        .command(&["-a", "claude", "resume", "--prepare", "--json"])
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
    let transcript = fixture
        .home
        .path()
        .join(format!("claude/projects/fixture/{native}.jsonl"));
    std::fs::create_dir_all(transcript.parent().unwrap()).unwrap();
    let request = "Keep the original request";
    let turn = json!({"type":"user","userType":"external","message":{"content":request}});
    std::fs::write(transcript, turn.to_string()).unwrap();
    let mut args = vec!["-a", "claude", "session", command, source_id, "--json"];
    if fork {
        args.push("--fork");
    }
    let viewer = fixture
        .command(&args)
        .env("PATH", &path)
        .env("CLAUDE_CONFIG_DIR", fixture.home.path().join("claude"))
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
    let agent = || -> Option<String> {
        fixture
            .sql()
            .query_row(
                "SELECT agent_process_id FROM agent_sessions WHERE id=?1",
                [&session],
                |r| r.get(0),
            )
            .unwrap()
    };
    let first_agent = agent();
    assert!(first_agent.is_some());
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
    // The installed lf runs its own provider on the same thread.
    let deadline = Instant::now() + Duration::from_secs(20);
    while agent() == first_agent {
        assert!(Instant::now() < deadline, "the provider was not replaced");
        std::thread::sleep(Duration::from_millis(50));
    }
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

/// Resume takes over an owner only when it cannot answer: a superseded owner
/// of a completed Session that is silent is ended, one that answers is
/// attached, and a silent owner of this build gives way to the same thread.
#[test]
fn resume_takes_over_only_an_owner_that_does_not_answer() {
    resume_ends_an_unanswering_superseded_owner_of_a_completed_session();
    resume_attaches_an_answering_superseded_owner_of_a_completed_session();
    opening_takes_over_from_an_owner_that_never_answers();
}

/// A completed Session names no known work, so `lf resume` still gets you back
/// from a superseded owner that cannot answer.
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

/// An idle owner from an earlier lf can stay alive after an upgrade yet never
/// answer its socket. Reconnecting neither waits on it nor gives up: it stops
/// its provider and continues the same provider thread in a new owner, without
/// replaying input. `human_session`'s tests own how its stale record reads.
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
fn headless_runs_settle_their_original_step_after_creator_teardown() {
    headless_run(&["graph", "proof"]);
    headless_run(&["skill", "probe", "--message", "go"]);
}

fn headless_run(command: &[&str]) {
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
    // Nothing ends a Run for want of viewers: it settles as succeeded only
    // once its held turn is released.
    assert_eq!(outcome(&step), None, "viewer death settled the execution");
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

/// Every viewer of an owner starts with the snapshot at its size and shares
/// live output; viewers enter the execution's directory, later ones the
/// Session's selected directory.
#[test]
fn every_viewer_starts_with_the_snapshot_and_shares_live_output() {
    viewers_enter_the_execution_directory_and_later_ones_the_selected_directory();
    let fixture = Fixture::new();
    let owner = fixture.start(&["/bin/sh", "-c", ECHO_LINES]);
    // Once an observer has seen `drawn`, the owner's screen holds it.
    until_out(&mut fixture.viewer(&owner, false), "drawn");
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
    viewer.input("unfinished draft");
    until_out(&mut viewer, "unfinished draft");
    fixture.sql().execute("UPDATE tasks SET issue_title='Completed shell',issue_description='',workspace_slug='task',updated_at=1,planning_completed=1,planning_completed_at='2000-01-01T00:00:00Z' WHERE id=?1", [task]).unwrap();
    fixture
        .sql()
        .execute(
            "UPDATE task_checkouts SET base_commit='head' WHERE task_id=?1",
            [task],
        )
        .unwrap();
    fixture
        .sql()
        .execute(
            "INSERT INTO task_retirements(task_id,completion,checkout,branch,due_at,phase)
        SELECT tasks.id,planning_completed_at,c.worktree,c.branch,0,'waiting'
        FROM tasks JOIN task_checkouts c ON c.task_id=tasks.id WHERE tasks.id=?1",
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
    // The ended shell's last screen, kept under its owner, holds input typed
    // but never submitted.
    assert!(fixture
        .last_screen(&owner)
        .text()
        .unwrap()
        .contains("unfinished draft"));
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
        self.sql()
            .execute(
                "INSERT INTO tasks(id,repository_id,issue_identifier,created_at)
             VALUES(?1,?2,'SHELL-1',1)",
                rusqlite::params![task, repo.as_str()],
            )
            .unwrap();
        self.sql()
            .execute(
                "INSERT INTO task_checkouts(task_id,worktree,branch) VALUES(?1,?2,'task')",
                rusqlite::params![task, self.repo.path().to_str().unwrap()],
            )
            .unwrap();
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
    viewer.input("export RETAINED=still-here; sleep 30 & printf 'job:%s\\n' $! > \"$HOME/job\"; printf '%s\\n' $$ > \"$HOME/shell-pid\"; printf 'state-ready\\n'\r");
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
    let shell = fixture
        .wait_file("shell-pid")
        .trim()
        .parse::<i32>()
        .unwrap();
    drop(viewer);
    // A completed nested conversation leaves the shell to its explicit Close
    // (`shell_should_close` reads only `close_requested`).
    let session = fixture.session(&owner);
    fixture
        .sql()
        .execute(
            "UPDATE agent_sessions SET interactive=1,completed_at=unixepoch() WHERE id=?1",
            [&session],
        )
        .unwrap();
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
    // Nothing else from the shell's PTY session outlives it.
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
            session != shell || state.starts_with('Z'),
            "retained PTY process {process}"
        );
    }
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
    // A new shell is never created in a checkout that is gone.
    let stale = fixture
        .command(&base[..6])
        .args(["stale-shell", "--create"])
        .current_dir(fixture.home.path())
        .output()
        .unwrap();
    std::fs::rename(&retired, &checkout).unwrap();
    assert!(!ended.status.success());
    assert!(String::from_utf8_lossy(&ended.stderr).contains("Shell ended"));
    assert!(!stale.status.success());
    let refusal = String::from_utf8_lossy(&stale.stderr);
    assert!(refusal.contains("checkout is unavailable"), "{refusal}");
    assert_eq!(fixture.wait_file("launches").lines().count(), 1);
    assert!(!fixture.endpoint(&owner).exists());
}

#[path = "transport/ssh_shells.rs"]
mod ssh_shells;

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

// Server adapter proofs keep the existing owner/provider fixture and its
// protocol. The service must never become another execution authority.
struct ServerProof<'a> {
    remote_address: Option<std::net::SocketAddr>,
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
            remote_address: None,
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
        let mut command = self.command(&["server", "run"]);
        let tailscale = self.fixture.home.path().join("bin/tailscale");
        if tailscale.exists() {
            command.env(
                "PATH",
                format!("{}:/usr/bin:/bin", tailscale.parent().unwrap().display()),
            );
        }
        command
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
                if self.remote_address.is_none_or(|address| {
                    std::net::TcpStream::connect_timeout(&address, Duration::from_millis(100))
                        .is_ok()
                }) {
                    return connection.welcome().instance.clone();
                }
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

    /// Starts `script` in a held terminal at the fixture's home and returns
    /// its terminal id.
    fn start_held(
        &self,
        session: Option<loopflow::id::LfSessionId>,
        request: &str,
        script: String,
    ) -> String {
        let home = self.fixture.home.path();
        let started = self
            .connect()
            .request(loopflow::server::wire::Operation::Server(
                loopflow::server::wire::ServerOperation::SessionStart {
                    session,
                    request: request.into(),
                    argv: vec!["/bin/sh".into(), "-c".into(), script],
                    cwd: home.to_path_buf(),
                    env: vec![
                        ("PATH".into(), "/usr/bin:/bin".into()),
                        ("HOME".into(), home.display().to_string()),
                    ],
                    rows: 24,
                    cols: 80,
                    placement: None,
                },
            ))
            .unwrap();
        started.result["terminal_id"].as_str().unwrap().to_owned()
    }

    fn request(&self, operation: Value) -> Value {
        self.connect()
            .request(serde_json::from_value(operation).unwrap())
            .unwrap()
            .result
    }

    fn mobile_status(&self) -> Value {
        self.request(json!({"kind":"mobile.status"}))
    }

    fn configure_remote(&self, configuration: Value) -> Value {
        self.request(json!({"kind":"mobile.configure","configuration":configuration}))
            ["configuration"]
            .clone()
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

/// Cross-language acceptance, explicitly opted into an already configured private
/// Tailscale host and installed Simulator. Ordinary CI never needs either.
#[cfg(target_os = "macos")]
#[test]
#[ignore = "requires LOOPFLOW_MOBILE_SIMULATOR, ENDPOINT, STATUS_FILE and CERTIFICATE_FILE; host identity is a fixture"]
fn mobile_machine_native_acceptance() {
    mobile_native_acceptance(None);
}

/// Uses the installed Claude account for a new, tool-free test conversation.
/// No existing conversation is resumed and the Machine registry stays isolated.
#[cfg(target_os = "macos")]
#[test]
#[ignore = "requires mobile host fixtures plus LOOPFLOW_MOBILE_CLAUDE pointing to the signed-in Claude executable"]
fn mobile_machine_claude_acceptance() {
    let claude = PathBuf::from(std::env::var("LOOPFLOW_MOBILE_CLAUDE").unwrap());
    assert!(claude.is_absolute() && claude.is_file());
    mobile_native_acceptance(Some(("claude", &claude)));
}

#[cfg(target_os = "macos")]
#[test]
#[ignore = "requires mobile host fixtures plus LOOPFLOW_MOBILE_CODEX pointing to the signed-in Codex executable"]
fn mobile_machine_codex_acceptance() {
    let codex = PathBuf::from(std::env::var("LOOPFLOW_MOBILE_CODEX").unwrap());
    assert!(codex.is_absolute() && codex.is_file());
    mobile_native_acceptance(Some(("codex", &codex)));
}

#[cfg(target_os = "macos")]
fn mobile_native_acceptance(provider: Option<(&str, &Path)>) {
    // The test binary's constructor deliberately replaces HOME and installs a
    // Keychain stand-in. Real-provider checks must opt into the caller's account
    // explicitly; never undo isolation for ordinary fixtures.
    let account = provider.map(|_| {
        let home = std::env::var("LOOPFLOW_MOBILE_ACCOUNT_HOME")
            .expect("real-provider acceptance requires LOOPFLOW_MOBILE_ACCOUNT_HOME");
        let path = std::env::var("LOOPFLOW_MOBILE_ACCOUNT_PATH")
            .expect("real-provider acceptance requires LOOPFLOW_MOBILE_ACCOUNT_PATH");
        assert!(Path::new(&home).is_absolute() && Path::new(&home).is_dir());
        (home, path)
    });
    let simulator = std::env::var("LOOPFLOW_MOBILE_SIMULATOR").unwrap();
    let endpoint = std::env::var("LOOPFLOW_MOBILE_ENDPOINT").unwrap();
    let host_status: Value = serde_json::from_slice(
        &std::fs::read(std::env::var("LOOPFLOW_MOBILE_STATUS_FILE").unwrap()).unwrap(),
    )
    .unwrap();
    let certificate =
        std::fs::read_to_string(std::env::var("LOOPFLOW_MOBILE_CERTIFICATE_FILE").unwrap())
            .unwrap();
    let root = Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .parent()
        .unwrap();
    let fixture = Fixture::new();
    fixture
        .repo
        .create_file(".lf/config.yaml", "agent: claude\ngraph: mobile\n");
    fixture
        .repo
        .create_file(".lf/graphs/mobile.yaml", "- interactive: demo\n");
    fixture.repo.stage_all();
    fixture.repo.commit("Mobile acceptance defaults");
    fixture.repo.push();
    let path = fixture.provider(
        r#"#!/bin/sh
if [ "$1" = --version ]; then echo '2.1.302'; exit; fi
stty echo icanon < /dev/tty
printf '%s\n' "$$" >> "$HOME/launches"
printf '\033[?2004hmobile-owner-ready\r\n> '
printf '\033]7501;id=proof:state=blocked:kind=question:app=fixture\033\\'
read -r answer < /dev/tty
printf '%s\n' "$answer" >> "$HOME/answers"
printf '\r\naccepted: %s\r\n' "$answer"
"#,
    );
    let path = if let Some((name, executable)) = provider {
        let options = match name {
            "claude" => "--safe-mode --strict-mcp-config --tools ''",
            // Both the app-server and its native TUI use this wrapper. Each
            // remains fixture-owned; neither attaches to the user's daemon.
            "codex" => "--disable daemon_auto_start --disable hooks --disable plugins --disable apps --disable multi_agent --disable shell_tool --disable browser_use --disable computer_use",
            _ => unreachable!("the opt-in tests name their provider"),
        };
        fixture.named_provider(name, &format!(
            "#!/bin/sh\nif [ \"$1\" != --version ]; then printf '%s\\n' \"$$\" >> \"$LF_HOME/launches\"; fi\nexport PATH={}\nexec {} {options} \"$@\"\n",
            shlex::try_quote(&account.as_ref().unwrap().1).unwrap(),
            shlex::try_quote(executable.to_str().unwrap()).unwrap()
        ))
    } else {
        path
    };
    let gh = fixture.home.path().join("bin/gh");
    std::fs::write(&gh, "#!/bin/sh\nexit 1\n").unwrap();
    std::fs::set_permissions(gh, std::fs::Permissions::from_mode(0o755)).unwrap();
    let mode = fixture
        .command(&["desktop", "portfolio", "manual"])
        .output()
        .unwrap();
    assert!(mode.status.success());
    let portfolio = fixture
        .command(&[
            "desktop",
            "portfolio",
            "add",
            fixture.repo.path().to_str().unwrap(),
        ])
        .output()
        .unwrap();
    assert!(
        portfolio.status.success(),
        "{}",
        String::from_utf8_lossy(&portfolio.stderr)
    );
    let mut command = fixture.serve(
        true,
        &[
            env!("CARGO_BIN_EXE_lf"),
            "-i",
            "-a",
            provider.map_or("claude", |(name, _)| name),
            ":",
            if provider.is_some() {
                "Do not use any tools. Reply only with the concatenation of MOBILE and _READY."
            } else {
                "Mobile acceptance"
            },
        ],
    );
    command.env("PATH", &path);
    if let Some((home, _)) = &account {
        // The CLI uses its normal account without copying or inspecting secrets.
        // LF_HOME still isolates every Loopflow record and process.
        command.env("HOME", home);
        command.env_remove("CLAUDECODE");
    }
    let owner = fixture.owner(command);
    let expected_launches = if provider.is_some_and(|(name, _)| name == "codex") {
        // Config discovery uses one short-lived stdio app-server before the
        // retained app-server and native TUI. Wait for all three before taking
        // the baseline; otherwise the TUI looks like a reconnect-time restart.
        3
    } else {
        1
    };
    let first_launch = fixture.launches(expected_launches);
    assert_eq!(first_launch.len(), expected_launches);
    let session = fixture.session_of_owner(&owner);
    let task = support::register_task_without_pr(
        fixture.home.path(),
        &fixture.repo.path().canonicalize().unwrap(),
        "mobile-proof",
        &fixture.repo.head_sha(),
    );
    fixture
        .sql()
        .execute(
            "INSERT OR REPLACE INTO task_primary_sessions(task_id,session_id) VALUES(?1,?2)",
            rusqlite::params![task.task.id.as_str(), session],
        )
        .unwrap();

    let mut desktop = fixture.viewer(&owner, true);
    if provider.is_none() {
        desktop.input("desktop draft");
        // Wait for echo before the phone takes its snapshot.
        until_out(&mut desktop, "desktop draft");
    }

    // Pace 32 MiB with the active phone while a second phone stops reading.
    // The retained owner, not an unbounded phone queue, owns slow-viewer policy.
    std::fs::write(fixture.home.path().join("output-chunk"), vec![b'x'; 65536]).unwrap();
    let output_owner = fixture.start(&[
        "/bin/sh",
        "-c",
        r#"stty -echo < /dev/tty
printf 'output-ready\r\n'
read -r start < /dev/tty
printf '\033]7501;id=proof:state=working:app=proof\033\\'
i=0
while [ "$i" -lt 512 ]; do
  cat "$HOME/output-chunk"
  read -r next < /dev/tty
  printf '%s\n' "$next" >> "$HOME/output-acks"
  [ "$next" = next ] || exit 1
  i=$((i+1))
done
printf '\r\noutput-complete\r\n'
printf '\033]7501;id=proof:state=done:app=proof\033\\'
read -r finish < /dev/tty
printf 'output-final\r\n'
"#,
    ]);
    let output_session = fixture.session(&output_owner);
    let ip: std::net::IpAddr = host_status["Self"]["TailscaleIPs"]
        .as_array()
        .unwrap()
        .iter()
        .filter_map(Value::as_str)
        .find(|ip| !ip.contains(':'))
        .unwrap()
        .parse()
        .unwrap();
    let remote_address = std::net::SocketAddr::new(
        ip,
        reqwest::Url::parse(&endpoint)
            .unwrap()
            .port_or_known_default()
            .unwrap(),
    );
    mobile_host(&fixture, &host_status, &certificate);
    let config = json!({"enabled":true,"endpoint":endpoint,"owner":{
        "node":host_status["Self"]["ID"],"user":host_status["Self"]["UserID"]}});
    let config_path = fixture.home.path().join("mobile.yaml");
    std::fs::write(config_path, serde_yaml_ng::to_string(&config).unwrap()).unwrap();
    let mut service = ServerProof {
        fixture: &fixture,
        child: None,
        supervised: false,
        remote_address: Some(remote_address),
        executable: env!("CARGO_BIN_EXE_lf").into(),
    };
    let service_log_path = root.join("swift/.build/mobile-service.log");
    let service_log = std::fs::File::create(&service_log_path).unwrap();
    let launch = || {
        fixture
            .command(&["server", "run"])
            .env("PATH", &path)
            .env("HTTPS_PROXY", "http://127.0.0.1:1")
            .env("HTTP_PROXY", "http://127.0.0.1:1")
            .env("RUST_LOG", "loopflow::server::remote=debug")
            .stdout(Stdio::null())
            .stderr(service_log.try_clone().unwrap())
            .spawn()
            .unwrap()
    };
    service.child = Some(launch());
    let first_instance = service.ready();
    let machine = service.request(json!({"kind":"machine.self"}));
    assert!(machine["id"].is_string(), "{machine}");

    let integration_directory = || {
        let container = Command::new("xcrun")
            .args([
                "simctl",
                "get_app_container",
                &simulator,
                "com.loopflow.ios.dev",
                "data",
            ])
            .output()
            .unwrap();
        assert!(container.status.success());
        PathBuf::from(String::from_utf8(container.stdout).unwrap().trim())
            .join("Documents/LoopflowIntegration")
    };
    let mut setup_dir = integration_directory();
    std::fs::create_dir_all(&setup_dir).unwrap();
    let setup = setup_dir.join("address.txt");
    std::fs::write(&setup, &endpoint).unwrap();
    let control_name = format!("service-{}", uuid::Uuid::new_v4());
    let mut control = setup_dir.join(&control_name);
    std::fs::create_dir(&control).unwrap();
    std::fs::write(
        setup_dir.join("session.json"),
        json!({"session":session,"outputSession":provider.is_none().then_some(output_session),"creationRepository":provider.is_none().then(|| fixture.repo.path().canonicalize().unwrap()),"serviceControl":control_name,"realProvider":provider.is_some()})
            .to_string(),
    )
    .unwrap();

    let log_path = root.join("swift/.build/mobile-acceptance.log");
    let log = std::fs::File::create(&log_path).unwrap();
    let mut test = Command::new("xcodebuild");
    for (key, _) in std::env::vars_os() {
        if key.to_string_lossy().starts_with("LF_") {
            test.env_remove(key);
        }
    }
    let mut native = test
        .current_dir(root)
        .args([
            "test",
            "-project",
            "swift/LoopflowMobile.xcodeproj",
            "-scheme",
            "LoopflowiOS",
            "-destination",
            &format!("platform=iOS Simulator,id={simulator}"),
            "-derivedDataPath",
            "swift/.build/ios",
            "-skipPackagePluginValidation",
            "-parallel-testing-enabled",
            "NO",
            "-only-testing:LoopflowiOSTests/MachineConnectionIntegrationTests",
            "CODE_SIGNING_ALLOWED=YES",
            "CODE_SIGN_IDENTITY=-",
        ])
        .stdin(Stdio::null())
        .stdout(log.try_clone().unwrap())
        .stderr(log)
        .spawn()
        .unwrap();
    let deadline = Instant::now() + Duration::from_secs(300);
    let mut stopped = false;
    let mut restarted = false;
    let mut locate_after = Instant::now();
    let result = loop {
        if let Some(result) = native.try_wait().unwrap() {
            break result;
        }
        // Xcode may reinstall the app into a new data container while preserving
        // its Documents. Follow that move instead of retaining the pre-build path.
        if !control.exists() && Instant::now() >= locate_after {
            setup_dir = integration_directory();
            control = setup_dir.join(&control_name);
            locate_after = Instant::now() + Duration::from_secs(1);
        }
        if control.join("stop").exists() && !stopped {
            let mut child = service.child.take().unwrap();
            child.kill().unwrap();
            child.wait().unwrap();
            stopped = true;
            std::fs::write(control.join("stopped"), "").unwrap();
        }
        if control.join("start").exists() && !restarted {
            assert!(stopped);
            service.child = Some(launch());
            assert_ne!(service.ready(), first_instance);
            restarted = true;
            std::fs::write(control.join("started"), "").unwrap();
        }
        if Instant::now() >= deadline {
            native.kill().unwrap();
            break native.wait().unwrap();
        }
        std::thread::sleep(Duration::from_millis(20));
    };
    // Do not leave fixture setup/control state for another test.
    let _ = std::fs::remove_file(setup);
    let _ = std::fs::remove_file(setup_dir.join("address.txt"));
    let _ = std::fs::remove_file(setup_dir.join("session.json"));
    let _ = std::fs::remove_dir_all(control);
    assert!(
        result.success(),
        "Native acceptance failed; inspect {} and {}",
        log_path.display(),
        service_log_path.display()
    );
    assert!(
        stopped && restarted,
        "Native proof must cross a real service interruption"
    );
    if provider.is_some() {
        assert!(fixture.running(&owner));
        assert_eq!(fixture.session_of_owner(&owner), session);
        assert_eq!(
            fixture.launches(expected_launches),
            first_launch,
            "Recovery must keep the same actual provider"
        );
        return;
    }
    fixture.ends(&owner);
    assert_eq!(
        fixture.launches(3).len(),
        first_launch.len() + 2,
        "Only explicit Session opening and new Task creation may launch another provider"
    );
    assert_eq!(
        std::fs::read_to_string(fixture.home.path().join("answers")).unwrap(),
        "desktop draft + phone + desktop\ncontinued from phone\ncreated from phone\n",
        "Input must reach the child exactly once"
    );
    let acknowledgements =
        std::fs::read_to_string(fixture.home.path().join("output-acks")).unwrap();
    assert_eq!(
        acknowledgements,
        "next\n".repeat(512),
        "Native edits must reach the busy terminal exactly once"
    );
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
    let legacy_plist = home.join("server.plist");
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
    let fixture = Fixture::uninitialized();
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

// The host CLI is the network-identity boundary. Fixtures supply its responses;
// TLS, HTTP/WebSocket, service dispatch and owner lifetimes remain real.
fn mobile_host(fixture: &Fixture, status: &Value, pem: &str) {
    let bin = fixture.home.path().join("bin");
    std::fs::create_dir_all(&bin).unwrap();
    let cli = bin.join("tailscale");
    std::fs::write(
        &cli,
        r#"#!/bin/sh
case "$1" in
status) cat "$HOME/tailscale-status.json" ;;
cert)
  if [ -f "$HOME/hold-certificate" ]; then
    printf ready > "$HOME/certificate-requested"
    while [ -f "$HOME/hold-certificate" ]; do sleep 0.02; done
  fi
  cat "$HOME/tailscale-certificate.pem" ;;

whois) cat "$HOME/tailscale-peer.json" ;;
*) exit 1 ;;
esac
"#,
    )
    .unwrap();
    std::fs::set_permissions(cli, std::fs::Permissions::from_mode(0o755)).unwrap();
    std::fs::write(
        fixture.home.path().join("tailscale-status.json"),
        status.to_string(),
    )
    .unwrap();
    let certificate = fixture.home.path().join("tailscale-certificate.pem");
    std::fs::write(&certificate, pem).unwrap();
    std::fs::set_permissions(certificate, std::fs::Permissions::from_mode(0o600)).unwrap();
    std::fs::write(
        fixture.home.path().join("tailscale-peer.json"),
        json!({
            "Node":{"StableID":"fixture-phone", "User":status["Self"]["UserID"]},
            "UserProfile":{"ID":status["Self"]["UserID"]}
        })
        .to_string(),
    )
    .unwrap();
}

#[test]
fn server_mobile_changes_preserve_local_connections() {
    use tokio_rustls::rustls::{
        self,
        pki_types::{pem::PemObject, CertificateDer, ServerName},
    };
    use tokio_tungstenite::tungstenite::Message;
    let fixture = Fixture::new();
    let certificate =
        rcgen::generate_simple_self_signed(vec!["fixture.tail.ts.net".into()]).unwrap();
    let pem = certificate.cert.pem();
    let status = json!({"BackendState":"Running", "Self":{"ID":"node1", "DNSName":"fixture.tail.ts.net.", "UserID":1, "TailscaleIPs":["127.0.0.1", "2001:db8::1"]}, "User":{"1":{"LoginName":"owner@example.com"}}});
    mobile_host(
        &fixture,
        &status,
        &format!("{pem}{}", certificate.signing_key.serialize_pem()),
    );
    let mut service = ServerProof::start(&fixture, false);
    let instance = service.ready();
    let roadmap = json!({"kind":"plan.list","directory":"/","wave":null,"task":null,"all":true});
    let mut watch = service.connect();
    let watched = watch
        .subscribe(serde_json::from_value(roadmap.clone()).unwrap())
        .unwrap();
    let initial = watch.reading(&watched).unwrap().expect("a reading");
    assert!(initial["unavailable"].is_null(), "{initial}");
    let proposed = json!({"enabled":true,"endpoint":"https://fixture.tail.ts.net:0","owner":{"node":"node1","user":1}});
    // Delay the external certificate response so the caller can disconnect
    // after admission, before setup finishes. Closing before admission would
    // merely drop an unauthenticated request, not lose an admitted result.
    let hold = fixture.home.path().join("hold-certificate");
    std::fs::write(&hold, "").unwrap();
    let mut abandoned = service.connect();
    abandoned
        .send(
            serde_json::from_value(json!({"kind":"mobile.configure","configuration":proposed}))
                .unwrap(),
        )
        .unwrap();
    assert_eq!(fixture.wait_file("certificate-requested"), "ready");
    drop(abandoned);
    std::fs::remove_file(hold).unwrap();
    let deadline = Instant::now() + Duration::from_secs(15);
    let config = loop {
        let status = service.mobile_status();
        if status["ready"] == true {
            assert!(status["error"].is_null());
            break status["configuration"].clone();
        }
        assert!(
            Instant::now() < deadline,
            "Lost reply left setup unavailable: {status}"
        );
        std::thread::sleep(Duration::from_millis(20));
    };
    let saved = fixture.home.path().join("mobile.yaml");
    let persisted: Value =
        serde_yaml_ng::from_str(&std::fs::read_to_string(&saved).unwrap()).unwrap();
    assert_eq!(persisted, config);
    let port = reqwest::Url::parse(config["endpoint"].as_str().expect("enabled endpoint"))
        .unwrap()
        .port()
        .unwrap();
    let connect = |path: &str| {
        let mut roots = rustls::RootCertStore::empty();
        roots
            .add(CertificateDer::from_pem_slice(pem.as_bytes()).unwrap())
            .unwrap();
        let config = rustls::ClientConfig::builder()
            .with_root_certificates(roots)
            .with_no_client_auth();
        let client = rustls::ClientConnection::new(
            std::sync::Arc::new(config),
            ServerName::try_from("fixture.tail.ts.net").unwrap(),
        )
        .unwrap();
        let socket = std::net::TcpStream::connect(("127.0.0.1", port)).unwrap();
        socket
            .set_read_timeout(Some(Duration::from_secs(15)))
            .unwrap();
        let tls = rustls::StreamOwned::new(client, socket);
        tokio_tungstenite::tungstenite::client(format!("wss://fixture.tail.ts.net{path}"), tls)
            .map_err(|error| error.to_string())
    };
    // One text frame per message: hello, then a subscription on the same connection.
    let text = |reader: &mut tokio_tungstenite::tungstenite::WebSocket<_>| -> Value {
        match reader.read().unwrap() {
            Message::Text(text) => serde_json::from_str(text.as_str()).unwrap(),
            other => panic!("expected a text frame: {other:?}"),
        }
    };
    let remote_watch = |reader: &mut tokio_tungstenite::tungstenite::WebSocket<_>| {
        reader
            .send(Message::Text(
                json!({"type":"hello","revision":loopflow::server::wire::REVISION,"min_revision":loopflow::server::wire::MIN_REVISION,"client":{
                    "kind":"test","platform":"test","name":"proof","version":"0"}})
                .to_string()
                .into(),
            ))
            .unwrap();
        let welcome = text(reader);
        assert_eq!(welcome["type"], "welcome");
        assert!(welcome["machine"]["id"].is_string(), "{welcome}");
        reader
            .send(Message::Text(
                json!({"type":"subscribe","id":"remote-watch","operation":roadmap})
                    .to_string()
                    .into(),
            ))
            .unwrap();
        assert_eq!(text(reader)["result"], Value::Null);
        assert_eq!(text(reader)["type"], "reading");
    };
    let closed = |reader: &mut tokio_tungstenite::tungstenite::WebSocket<_>| loop {
        match reader.read() {
            Ok(Message::Close(_)) | Err(_) => break,
            Ok(_) => {}
        }
    };
    let (mut reader, _) = connect("/v1/machine").unwrap();
    remote_watch(&mut reader);
    // The same authenticated remote client resolves a Session, then redeems its
    // ticket on a second WebSocket carrying the unchanged owner protocol.
    let owner = fixture.owner(fixture.serve(true, &["/bin/cat"]));
    let session = fixture.session(&owner);
    reader
        .send(Message::Text(
            json!({
                "type":"request", "id":"terminal", "select":null,
                "operation":{"kind":"session.attach","session":session,"owner_lf_process_id":null}
            })
            .to_string()
            .into(),
        ))
        .unwrap();
    let issued = loop {
        let reply = text(&mut reader);
        if reply["id"] == "terminal" {
            break reply;
        }
    };
    assert_eq!(issued["type"], "response", "{issued}");
    assert_eq!(issued["result"]["owner_lf_process_id"], owner);
    let (mut terminal, _) = connect("/v1/ticket").unwrap();
    terminal
        .send(Message::Text(
            json!({"type":"attach", "ticket":issued["result"]["ticket"]})
                .to_string()
                .into(),
        ))
        .unwrap();
    let bridged = text(&mut terminal);
    assert_eq!(bridged["type"], "bridged");
    assert_eq!(bridged["owner_lf_process_id"], owner);
    terminal.send(Message::Binary(frame(REQUEST, &serde_json::to_vec(&json!({
        "id":1,"kind":"attach","control":false,"replace":false,"own":null,"rows":24,"cols":80
    })).unwrap()).into())).unwrap();
    let mut bytes = Vec::new();
    while bytes.len() < 5
        || bytes.len() < 5 + u32::from_be_bytes(bytes[1..5].try_into().unwrap()) as usize
    {
        match terminal.read().unwrap() {
            Message::Binary(chunk) => bytes.extend_from_slice(&chunk),
            Message::Ping(_) | Message::Pong(_) => {}
            other => panic!("expected owner bytes: {other:?}"),
        }
    }
    assert_eq!(bytes[0], REPLY);
    let size = u32::from_be_bytes(bytes[1..5].try_into().unwrap()) as usize;
    let reply: Value = serde_json::from_slice(&bytes[5..5 + size]).unwrap();
    assert_eq!(reply["kind"], "attached", "{reply}");
    terminal.close(None).unwrap();
    assert_eq!(service.configure_remote(config.clone()), config);
    service.configure_remote(Value::Null);
    closed(&mut reader);
    let off = service.mobile_status();
    assert_eq!(off["ready"], false);
    assert_eq!(off["configuration"]["enabled"], false);
    watch.checkpoint(&watched, 9).unwrap();
    while watch.reading(&watched).unwrap().expect("a reading")["answers"] != 9 {}

    assert_eq!(
        service.ready(),
        instance,
        "Setup must not restart the local service"
    );
    service.configure_remote(config.clone());
    let (mut reader, _) = connect("/v1/machine").unwrap();
    remote_watch(&mut reader);
    let status_path = fixture.home.path().join("tailscale-status.json");
    std::fs::write(&status_path, r#"{"BackendState":"NeedsLogin"}"#).unwrap();
    // Logout must close existing remote connections.
    closed(&mut reader);
    watch.checkpoint(&watched, 10).unwrap();
    while watch.reading(&watched).unwrap().expect("a reading")["answers"] != 10 {}

    assert_eq!(service.ready(), instance);
    let unavailable = service.mobile_status();
    assert_eq!(unavailable["ready"], false);
    assert_eq!(unavailable["configuration"]["enabled"], true);
    assert!(unavailable["error"].is_string());
    std::fs::write(&status_path, status.to_string()).unwrap();
    // The original saved approval recovers automatically; the caller does not
    // need to turn access on again after a transient Tailscale outage.
    let deadline = Instant::now() + Duration::from_secs(15);
    loop {
        if service.mobile_status()["ready"] == true {
            break;
        }
        assert!(
            Instant::now() < deadline,
            "Same-account connection did not recover"
        );
        std::thread::sleep(Duration::from_millis(50));
    }
    service.restart();
    service.ready();
    let deadline = Instant::now() + Duration::from_secs(15);
    loop {
        if std::net::TcpStream::connect(("127.0.0.1", port)).is_ok() {
            break;
        }
        assert!(
            Instant::now() < deadline,
            "Saved mobile listener was not restored"
        );
        std::thread::sleep(Duration::from_millis(20));
    }
    assert!(connect("/v1/machine").is_ok());
    // Foreign device identity is rejected even with a valid HTTPS connection.
    std::fs::write(
        fixture.home.path().join("tailscale-peer.json"),
        r#"{"Node":{"StableID":"foreign","User":2},"UserProfile":{"ID":2}}"#,
    )
    .unwrap();
    assert!(connect("/v1/machine").is_err());
    // Saved disable, offline status and a new Tailscale owner are
    // `server::mobile`'s tests.
    drop(service);
    assert!(!fixture.home.path().join("server.plist").exists());
}

/// A server restart, and a Move here from another pane, both keep the same
/// owner, draft, provider and Graph.
#[test]
fn server_preserves_owner_and_graph_across_restart() {
    server_continuity(false, false);
    server_continuity(false, true);
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

#[cfg(target_os = "macos")]
#[test]
#[ignore = "registers and removes an isolated launchd job; run explicitly outside network sandbox"]
fn installed_server_preserves_owner_and_graph_across_replacement() {
    server_continuity(true, false);
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

/// Ends a process the test cannot otherwise reach, including on panic.
struct KillOnDrop(libc::pid_t);

impl Drop for KillOnDrop {
    fn drop(&mut self) {
        // SAFETY: the pid was read from this test's own agent or holder.
        unsafe { libc::kill(self.0, libc::SIGKILL) };
    }
}

fn alive(pid: libc::pid_t) -> bool {
    // SAFETY: signal 0 only checks that the process exists.
    unsafe { libc::kill(pid, 0) == 0 }
}

fn command_line(pid: libc::pid_t) -> (libc::pid_t, String) {
    let output = Command::new("ps")
        .args(["-o", "ppid=,command=", "-p", &pid.to_string()])
        .output()
        .unwrap();
    let line = String::from_utf8_lossy(&output.stdout).trim().to_owned();
    let (parent, command) = line.split_once(' ').unwrap_or((&line, ""));
    (
        parent.trim().parse().unwrap_or(0),
        command.trim().to_owned(),
    )
}

#[test]
fn a_held_pane_reads_reconnecting_until_the_next_server_answers() {
    let fixture = Fixture::new();
    let mut service = ServerProof::start(&fixture, false);
    let path = fixture.waiting_provider();
    let mut launch = fixture.command(&["-a", "claude", "resume"]);
    launch.env("PATH", &path);
    let first = Terminal::spawn_with_placement(launch, Some(true));
    let launched = fixture.launches(1).pop().unwrap();
    let provider: libc::pid_t = launched.split(':').next().unwrap().parse().unwrap();
    let _provider = KillOnDrop(provider);
    let session = session_of(&fixture.sql(), &launched);
    let held = fixture.terminal_of(&session);
    first.stop();
    // Desktop's pane: the Session's held terminal, with a native status row.
    let status = ConnectionStatus::new(fixture.home.path().join("connection.sock"));
    let mut command = service.command(&[
        "session",
        "attach",
        "--terminal",
        &held,
        "--session",
        &session,
    ]);
    command
        .env("PATH", &path)
        .env("LF_SERVER", "1")
        .env("LF_SERVER_STATUS", &status.path);
    let mut view = Terminal::spawn(command);
    let ready = |after: u64| {
        let deadline = Instant::now() + Duration::from_secs(30);
        loop {
            let reading = status.reading();
            if reading.as_ref().is_some_and(|reading| {
                reading["state"] == "connected"
                    && reading["input"] == true
                    && reading["native_ready"] == true
                    && reading["generation"].as_u64().unwrap() > after
            }) {
                return reading.unwrap()["generation"].as_u64().unwrap();
            }
            assert!(Instant::now() < deadline, "not ready: {reading:?}");
            std::thread::sleep(Duration::from_millis(20));
        }
    };
    let generation = ready(0);
    service.stop();
    wait_server_status(&status, "reconnecting", false, generation + 1);
    service.child = Some(service.launch());
    service.ready();
    ready(generation);
    view.assert_viewing("the held pane");
    view.stop();
    // SAFETY: the pid the fixture's provider recorded for itself.
    unsafe { libc::kill(provider, libc::SIGKILL) };
    fixture.held_ends(&held);
}

/// A managed launch refused for a stopped server started no provider, so the
/// next launch starts that conversation fresh instead of resuming one that
/// never ran (real Claude answers `--resume` with "No conversation found").
/// The refused launch is piped, so it runs unheld and its owner checks the server.
#[test]
fn a_launch_refused_for_a_stopped_server_leaves_nothing_to_resume() {
    let fixture = Fixture::new();
    let path = fixture.waiting_provider();
    // The selected installation's `lf install server` reaches no supervisor.
    let refusing = fixture.home.path().join("lf-refusing-install");
    std::fs::write(
        &refusing,
        format!(
            "#!/bin/sh\nif [ \"$1 $2\" = \"install server\" ]; then exit 1; fi\nexec {} \"$@\"\n",
            env!("CARGO_BIN_EXE_lf")
        ),
    )
    .unwrap();
    std::fs::set_permissions(&refusing, std::fs::Permissions::from_mode(0o755)).unwrap();
    let refused = fixture
        .command(&["-a", "claude", "resume"])
        .env("PATH", &path)
        .env("LF_BIN", &refusing)
        .output()
        .unwrap();
    let stderr = String::from_utf8_lossy(&refused.stderr);
    assert!(
        stderr.contains("Loopflow server is not running"),
        "{stderr}"
    );

    let _service = ServerProof::start(&fixture, false);
    let mut launch = fixture.command(&["-a", "claude", "resume"]);
    launch.env("PATH", &path);
    let view = Terminal::spawn(launch);
    let launched = fixture.launches(1).pop().unwrap();
    let provider: libc::pid_t = launched.split(':').next().unwrap().parse().unwrap();
    let _provider = KillOnDrop(provider);
    assert!(
        launched.contains("--session-id ") && !launched.contains("--resume "),
        "{launched}"
    );
    let held = fixture.terminal_of(&session_of(&fixture.sql(), &launched));
    view.stop();
    // SAFETY: the pid the fixture's provider recorded for itself.
    unsafe { libc::kill(provider, libc::SIGKILL) };
    fixture.held_ends(&held);
}

#[test]
fn held_session_survives_server_restart() {
    let fixture = Fixture::new();
    let mut service = ServerProof::start(&fixture, false);
    let home = fixture.home.path();
    let script = format!(
        "echo $$ > {home}/agent.pid; i=0; while [ ! -e {home}/stop ]; do i=$((i+1)); printf 'tick:%d\\n' $i; sleep 0.1; done; exit 3",
        home = home.display()
    );
    let terminal = service.start_held(None, "held-restart", script);
    let agent: libc::pid_t = {
        let deadline = Instant::now() + Duration::from_secs(20);
        loop {
            if let Some(pid) = std::fs::read_to_string(home.join("agent.pid"))
                .ok()
                .and_then(|pid| pid.trim().parse().ok())
            {
                break pid;
            }
            assert!(Instant::now() < deadline, "the agent never started");
            std::thread::sleep(Duration::from_millis(20));
        }
    };
    let _agent = KillOnDrop(agent);
    // The agent's parent is the holder, not the server or an owner process.
    let (holder, _) = command_line(agent);
    let (_, holder_command) = command_line(holder);
    assert!(holder_command.contains("holder run"), "{holder_command}");
    assert!(
        !holder_command.contains("session serve"),
        "{holder_command}"
    );

    let mut view =
        Terminal::spawn(service.command(&["session", "attach", "--terminal", &terminal]));
    view.wait_text("tick:3");

    // SIGTERM the server: the agent keeps printing into the holder's ring.
    let server = service.child.take().unwrap();
    // SAFETY: the fixture's own server process.
    unsafe { libc::kill(server.id() as libc::pid_t, libc::SIGTERM) };
    let mut server = server;
    server.wait().unwrap();
    let stopped = view.screen.lock().unwrap().len();
    let last_seen = |screen: &[u8]| {
        String::from_utf8_lossy(screen)
            .split("tick:")
            .filter_map(|tick| {
                tick.split(|c: char| !c.is_ascii_digit())
                    .next()?
                    .parse::<u32>()
                    .ok()
            })
            .max()
            .unwrap_or(0)
    };
    let before = last_seen(&view.screen.lock().unwrap());
    std::thread::sleep(Duration::from_millis(800));
    assert!(alive(agent), "the agent ended with its server");
    view.assert_viewing("the view of a stopped server");

    // A new server: the view continues from its position, with no repaint,
    // and shows what was printed while no server ran.
    service.child = Some(service.launch());
    service.ready();
    view.wait_text(&format!("tick:{}", before + 12));
    let screen = view.screen.lock().unwrap().clone();
    let after = &screen[stopped..];
    let text = String::from_utf8_lossy(after);
    assert!(
        !text.contains("\x1b[?1047l\x1b[?47l") && !text.contains("\x1b[2J"),
        "the view was repainted: {text:?}"
    );
    assert!(
        text.contains("\x1b[5n"),
        "the view did not continue: {text:?}"
    );
    let ticks: Vec<u32> = text
        .split("tick:")
        .skip(1)
        .filter_map(|tick| {
            tick.split(|c: char| !c.is_ascii_digit())
                .next()?
                .parse()
                .ok()
        })
        .collect();
    assert_eq!(ticks.first(), Some(&(before + 1)), "{text:?}");
    assert!(
        ticks.windows(2).all(|pair| pair[1] == pair[0] + 1),
        "the stream skipped or repeated output: {ticks:?}"
    );

    // The terminal's Session names its provider, as a launch records it.
    fixture
        .sql()
        .execute(
            "INSERT INTO agent_sessions(id,title,title_source,created_at,provider,cwd,input_published,held_terminal_id)
             VALUES('session_held_restart','Held','generated',1,'claude',?2,1,?1)",
            rusqlite::params![terminal, home.to_str().unwrap()],
        )
        .unwrap();
    // SIGKILL the server; the agent exits 3 while none runs.
    service.stop();
    std::fs::write(home.join("stop"), "").unwrap();
    let deadline = Instant::now() + Duration::from_secs(20);
    while alive(agent) {
        assert!(Instant::now() < deadline, "the agent did not exit");
        std::thread::sleep(Duration::from_millis(20));
    }
    service.child = Some(service.launch());
    service.ready();
    let ended = || {
        fixture
            .sql()
            .query_row(
                "SELECT ended_at IS NOT NULL, exit_code FROM held_terminals WHERE id=?1",
                [&terminal],
                |r| Ok((r.get::<_, bool>(0)?, r.get::<_, Option<i64>>(1)?)),
            )
            .unwrap()
    };
    let deadline = Instant::now() + Duration::from_secs(20);
    while !ended().0 {
        assert!(Instant::now() < deadline, "the exit was never recorded");
        std::thread::sleep(Duration::from_millis(50));
    }
    assert_eq!(ended(), (true, Some(3)));
    let holders = home.join("holders");
    let receipts: Vec<_> = std::fs::read_dir(&holders)
        .unwrap()
        .flatten()
        .flat_map(|build| std::fs::read_dir(build.path()).unwrap().flatten())
        .map(|entry| entry.file_name())
        .collect();
    assert!(
        !receipts
            .iter()
            .any(|name| name.to_string_lossy() == terminal),
        "the terminal's directory outlived its settlement: {receipts:?}"
    );
    // The view ends as the agent did, not as a refused attach.
    assert_eq!(view.wait_exit().code(), Some(3));
    // The same line a view connected at the exit prints.
    view.wait_text("claude exited with 3.");
    let screen = String::from_utf8_lossy(&view.screen.lock().unwrap()).into_owned();
    assert!(!screen.contains("is not running"), "{screen:?}");
}

/// A view whose server turns out to be another build becomes the installed
/// lf as a fresh view of the same terminal: it repaints from the server's
/// screen and reads live output, and the agent ticks on across the switch.
#[test]
fn a_held_view_meeting_another_build_reopens_as_the_installed_lf() {
    let fixture = Fixture::new();
    let mut service = ServerProof::start(&fixture, false);
    let home = fixture.home.path();
    let script = format!(
        "echo $$ > {home}/agent.pid; i=0; while :; do i=$((i+1)); printf 'tick:%d\\n' $i; sleep 0.1; done",
        home = home.display()
    );
    let terminal = service.start_held(None, "held-other-build", script);
    let mut view =
        Terminal::spawn(service.command(&["session", "attach", "--terminal", &terminal]));
    view.wait_text("tick:3");
    let agent: libc::pid_t = std::fs::read_to_string(home.join("agent.pid"))
        .unwrap()
        .trim()
        .parse()
        .unwrap();
    let _agent = KillOnDrop(agent);
    let relay = view.child.id();

    // While no server runs, a server of another build takes the entrance.
    service.stop();
    let entrance = fixture.endpoint("held");
    let _ = std::fs::remove_file(&entrance);
    let other = UnixListener::bind(&entrance).unwrap();
    other.set_nonblocking(true).unwrap();
    let deadline = Instant::now() + Duration::from_secs(20);
    let mut reconnect = loop {
        match other.accept() {
            Ok((socket, _)) => break socket,
            Err(_) => {
                assert!(Instant::now() < deadline, "the view never reconnected");
                std::thread::sleep(Duration::from_millis(20));
            }
        }
    };
    reconnect.set_nonblocking(false).unwrap();
    let mut named = Vec::new();
    let mut byte = [0];
    while reconnect.read(&mut byte).unwrap() == 1 && byte[0] != b'\n' {
        named.push(byte[0]);
    }
    let named: Value = serde_json::from_slice(&named).unwrap();
    assert_eq!(
        named,
        json!({ "terminal": terminal, "revision": HELD_REVISION })
    );
    drop(other);

    // This build's server is up by the time the other build answers.
    service.child = Some(service.launch());
    service.ready();
    let switched = view.screen.lock().unwrap().len();
    reconnect
        .write_all(
            format!(
                "{}\n",
                json!({ "answer": "hosted", "revision": HELD_REVISION + 1 })
            )
            .as_bytes(),
        )
        .unwrap();
    drop(reconnect);
    let ticks = |screen: &[u8]| -> Vec<u32> {
        String::from_utf8_lossy(screen)
            .split("tick:")
            .skip(1)
            .filter_map(|tick| {
                tick.split(|c: char| !c.is_ascii_digit())
                    .next()?
                    .parse()
                    .ok()
            })
            .collect()
    };
    let last = *ticks(&view.screen.lock().unwrap()).iter().max().unwrap();
    view.wait_text(&format!("tick:{}", last + 10));
    view.assert_viewing("the replaced view");
    assert_eq!(view.child.id(), relay, "the view became a new process");
    assert!(alive(agent), "the agent ended across the switch");
    let screen = view.screen.lock().unwrap().clone();
    let text = String::from_utf8_lossy(&screen[switched..]).into_owned();
    assert!(
        text.contains("\x1b[?1047l\x1b[?47l"),
        "the replacement did not repaint: {text:?}"
    );
    let after: Vec<u32> = ticks(text.as_bytes());
    assert!(
        after.windows(2).all(|pair| pair[1] == pair[0] + 1),
        "live output skipped or repeated: {after:?}"
    );
    view.stop();
}

/// A holder that does not answer while a server starts keeps its terminals:
/// their views wait and continue once it answers, and `session.attach` is
/// told to ask again, never that the terminal ended. Gone instead, its
/// terminals and its row settle with an unknown outcome.
#[test]
fn a_held_view_waits_for_a_holder_stopped_at_server_startup() {
    use loopflow::engine::{Error, ErrorCode};
    use loopflow::server::wire::{Operation, ServerOperation};

    let fixture = Fixture::new();
    let mut service = ServerProof::start(&fixture, false);
    let home = fixture.home.path();
    let session: loopflow::id::LfSessionId = "session_held_attach".into();
    let script = format!(
        "echo $$ > {home}/agent.pid; i=0; while :; do i=$((i+1)); printf 'tick:%d\\n' $i; sleep 0.1; done",
        home = home.display()
    );
    let terminal = service.start_held(Some(session.clone()), "held-stopped", script);
    let deadline = Instant::now() + Duration::from_secs(20);
    let agent: libc::pid_t = loop {
        if let Some(pid) = std::fs::read_to_string(home.join("agent.pid"))
            .ok()
            .and_then(|pid| pid.trim().parse().ok())
        {
            break pid;
        }
        assert!(Instant::now() < deadline, "the agent never started");
        std::thread::sleep(Duration::from_millis(20));
    };
    let _agent = KillOnDrop(agent);
    let holder = command_line(agent).0;
    let signal = |signal| {
        // SAFETY: the holder this fixture's server started.
        assert_eq!(unsafe { libc::kill(holder, signal) }, 0);
    };
    let ticket = |service: &ServerProof, terminal_id: Option<&str>| {
        service
            .connect()
            .request(Operation::Server(ServerOperation::SessionAttach {
                session: session.clone(),
                owner_lf_process_id: None,
                terminal_id: terminal_id.map(|id| serde_json::from_value(json!(id)).unwrap()),
            }))
    };
    let mut view =
        Terminal::spawn(service.command(&["session", "attach", "--terminal", &terminal]));
    view.wait_text("tick:3");
    let ticks = |screen: &[u8]| -> Vec<u32> {
        String::from_utf8_lossy(screen)
            .split("tick:")
            .skip(1)
            .filter_map(|tick| {
                tick.split(|c: char| !c.is_ascii_digit())
                    .next()?
                    .parse()
                    .ok()
            })
            .collect()
    };

    // The holder is stopped while a new server starts.
    service.stop();
    signal(libc::SIGSTOP);
    let stopped = view.screen.lock().unwrap().len();
    let before = ticks(&view.screen.lock().unwrap())
        .into_iter()
        .max()
        .unwrap();
    service.child = Some(service.launch());
    service.ready();
    assert_eq!(fixture.held_entrance(&terminal).1, "waiting");
    for named in [None, Some(terminal.as_str())] {
        let waiting = ticket(&service, named).unwrap_err();
        let waiting = waiting.downcast::<Error>().unwrap();
        assert_eq!(waiting.code, ErrorCode::Unavailable, "{waiting:?}");
        let data = waiting.data.unwrap();
        assert_eq!(data["terminal_id"], terminal.as_str(), "{data}");
        assert_eq!(data["retry"], true, "{data}");
    }
    std::thread::sleep(Duration::from_millis(800));
    view.assert_viewing("the view of a stopped holder");

    // It answers: the view continues from its position.
    signal(libc::SIGCONT);
    view.wait_text(&format!("tick:{}", before + 5));
    let after = ticks(&view.screen.lock().unwrap()[stopped..]);
    assert_eq!(after.first(), Some(&(before + 1)), "{after:?}");
    assert!(
        after.windows(2).all(|pair| pair[1] == pair[0] + 1),
        "the stream skipped or repeated output: {after:?}"
    );
    // Asking again now attaches to the same terminal.
    let attached = ticket(&service, None).unwrap();
    assert!(attached.result["ticket"].is_string(), "{}", attached.result);

    // Stopped again at the next server's startup, then killed: gone.
    service.stop();
    signal(libc::SIGSTOP);
    service.child = Some(service.launch());
    service.ready();
    assert_eq!(fixture.held_entrance(&terminal).1, "waiting");
    signal(libc::SIGKILL);
    fixture.held_ends(&terminal);
    let code: Option<i64> = fixture
        .sql()
        .query_row(
            "SELECT exit_code FROM held_terminals WHERE id=?1",
            [&terminal],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(code, None, "nobody saw the agent exit");
    let unfinished = || {
        fixture
            .sql()
            .query_row(
                r#"SELECT COUNT(*) FROM processes WHERE command='["lf","holder","run"]'
                   AND pid=?1 AND completed_at IS NULL"#,
                [holder],
                |r| r.get::<_, i64>(0),
            )
            .unwrap()
    };
    let deadline = Instant::now() + Duration::from_secs(20);
    while unfinished() > 0 {
        assert!(
            Instant::now() < deadline,
            "the gone holder's row never ended"
        );
        std::thread::sleep(Duration::from_millis(50));
    }
    assert_eq!(fixture.held_entrance(&terminal).1, "ended");
    assert!(!view.wait_exit().success());
}

/// How many threads `pid` runs.
fn thread_count(pid: u32) -> usize {
    if let Ok(tasks) = std::fs::read_dir(format!("/proc/{pid}/task")) {
        return tasks.count();
    }
    let output = Command::new("ps")
        .args(["-M", "-p", &pid.to_string()])
        .output()
        .unwrap();
    assert!(output.status.success(), "{output:?}");
    String::from_utf8_lossy(&output.stdout).lines().count() - 1
}

/// Task cleanup ends an abandoned Task's unviewed held conversation: a held
/// terminal has no process to signal, so the server ends it on the notice.
#[test]
fn task_cleanup_ends_an_abandoned_tasks_held_terminal() {
    let fixture = Fixture::new();
    let service = ServerProof::start(&fixture, false);
    let home = fixture.home.path();
    let task = "task_5e55105e000040008000000000000008";
    fixture.shell_task(task);
    fixture.sql().execute("UPDATE tasks SET issue_title='Abandoned held',issue_description='',workspace_slug='task',updated_at=1 WHERE id=?1", [task]).unwrap();
    fixture
        .sql()
        .execute(
            "UPDATE task_checkouts SET base_commit='head' WHERE task_id=?1",
            [task],
        )
        .unwrap();
    let terminal = service.start_held(None, "abandoned", "sleep 600".to_owned());
    fixture
        .sql()
        .execute(
            "INSERT INTO agent_sessions(id,title,title_source,created_at,provider,cwd,input_published,held_terminal_id,task_id,interactive)
             VALUES('session_held_abandoned','Held','generated',1,'claude',?2,1,?1,?3,1)",
            rusqlite::params![terminal, home.to_str().unwrap(), task],
        )
        .unwrap();
    // Open, the conversation stays however long nobody views it.
    let _ = fixture.command(&["task", "retire", task]).output().unwrap();
    fixture.assert_held_hosted(&terminal);
    fixture
        .sql()
        .execute("UPDATE tasks SET abandoned_at=1 WHERE id=?1", [task])
        .unwrap();
    let _ = fixture.command(&["task", "retire", task]).output().unwrap();
    fixture.held_ends(&terminal);
}

/// Every held terminal shares its build's one holder, which `lf ps` lists
/// once across a server restart; no owner process runs per terminal. One
/// server thread hosts every held terminal and its views: starting more
/// terminals and viewing them adds no thread each.
#[test]
fn held_terminals_share_one_holder_and_add_no_server_threads() {
    let fixture = Fixture::new();
    let mut service = ServerProof::start(&fixture, false);
    let home = fixture.home.path();
    let server = service.child.as_ref().unwrap().id();
    let start = |service: &ServerProof, request: String| {
        let script = format!(
            "while [ ! -e {}/stop ]; do echo tick; sleep 0.1; done",
            home.display()
        );
        let terminal = service.start_held(None, &request, script);
        let (view, answer) = fixture.held_entrance(&terminal);
        assert_eq!(answer, "hosted");
        (terminal, view)
    };
    let mut held = vec![start(&service, "first".into())];
    let one = thread_count(server);
    const MORE: usize = 8;
    held.extend((0..MORE).map(|more| start(&service, format!("more-{more}"))));
    // The runtime's own pool may add a worker; a thread per terminal or per
    // view would add at least MORE.
    let deadline = Instant::now() + Duration::from_secs(20);
    loop {
        let many = thread_count(server);
        if many <= one + 2 {
            break;
        }
        assert!(
            Instant::now() < deadline,
            "{many} server threads with {} held terminals, {one} with one",
            MORE + 1
        );
        std::thread::sleep(Duration::from_millis(200));
    }
    let terminals: Vec<String> = held.into_iter().map(|(terminal, _)| terminal).collect();
    let listed = |service: &ServerProof| {
        let output = service
            .command(&["process", "list", "--json"])
            .stdin(Stdio::null())
            .output()
            .unwrap();
        assert!(output.status.success(), "{output:?}");
        let snapshot: Value = serde_json::from_slice(&output.stdout).unwrap();
        snapshot["nodes"].as_array().unwrap().clone()
    };
    let holders = |nodes: &[Value]| -> Vec<u64> {
        nodes
            .iter()
            .filter(|node| node["label"] == "lf holder")
            .map(|node| node["pid"].as_u64().unwrap())
            .collect()
    };
    let nodes = listed(&service);
    let first = holders(&nodes);
    assert_eq!(first.len(), 1, "{nodes:#?}");
    assert!(command_line(first[0] as libc::pid_t)
        .1
        .contains("holder run"));
    for node in &nodes {
        if let Some(pid) = node["pid"].as_u64() {
            let (_, command) = command_line(pid as libc::pid_t);
            assert!(!command.contains("session serve"), "{nodes:#?}");
        }
    }
    // A new server takes up the same holder and records nothing new.
    service.stop();
    service.child = Some(service.launch());
    service.ready();
    assert_eq!(holders(&listed(&service)), first);

    std::fs::write(home.join("stop"), "").unwrap();
    for terminal in &terminals {
        fixture.held_ends(terminal);
    }
    // The holder leaves, as it would after idling: the server that adopted it
    // settles its row, though only the server that started it saw the exit.
    let holder = first[0] as libc::pid_t;
    // SAFETY: the holder this fixture's servers recorded.
    unsafe { libc::kill(holder, libc::SIGKILL) };
    // A settled holder row is completed, then forgotten: none stays unfinished.
    let unfinished = || {
        fixture
            .sql()
            .query_row(
                r#"SELECT COUNT(*) FROM processes WHERE command='["lf","holder","run"]'
                   AND pid=?1 AND completed_at IS NULL"#,
                [holder],
                |r| r.get::<_, i64>(0),
            )
            .unwrap()
    };
    let deadline = Instant::now() + Duration::from_secs(20);
    while unfinished() > 0 {
        assert!(
            Instant::now() < deadline,
            "the adopted holder's row never ended"
        );
        std::thread::sleep(Duration::from_millis(50));
    }
}
