//! Held terminals and Session attachment through the Machine server.
//! Ordinary tests need no provider account or display; ignored native
//! acceptance explicitly opts into them.
#![cfg(unix)]

mod support;

// Decode a terminal's persisted screen with the same pinned Ghostty library.
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
    fn sql(&self) -> rusqlite::Connection {
        rusqlite::Connection::open(self.home.path().join("loopflow.db")).unwrap()
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
    /// A view of a held terminal, through the server's entrance.
    #[track_caller]
    fn held_viewer(&self, terminal: &str, control: bool) -> Viewer {
        self.held_attach(
            terminal,
            json!({"kind":"attach","control":control,"replace":true,
                "own":control.then_some("take"),"rows":24,"cols":80}),
        )
    }
    /// Start `argv` in a held terminal, as `session.start` does for a launch.
    fn start_held(&self, argv: &[&str]) -> String {
        let home = self.home.path().to_str().unwrap();
        let env = [
            ("HOME", home),
            ("LF_HOME", home),
            ("PATH", "/usr/bin:/bin"),
            ("RUST_LOG", "off"),
        ];
        let started = loopflow::server::client::Connection::open_at(&self.endpoint("machine"))
            .unwrap()
            .request(loopflow::server::wire::Operation::Server(
                loopflow::server::wire::ServerOperation::SessionStart {
                    session: None,
                    request: uuid::Uuid::new_v4().to_string(),
                    argv: argv.iter().map(|arg| arg.to_string()).collect(),
                    cwd: self.repo.path().to_path_buf(),
                    env: env
                        .iter()
                        .map(|(name, value)| (name.to_string(), value.to_string()))
                        .collect(),
                    rows: 24,
                    cols: 80,
                    placement: None,
                },
            ))
            .unwrap();
        started.result["terminal_id"].as_str().unwrap().to_owned()
    }
    /// A view of a held terminal attached with `request`.
    #[track_caller]
    fn held_attach(&self, terminal: &str, request: Value) -> Viewer {
        let (socket, answer) = self.held_entrance(terminal);
        assert_eq!(answer, "hosted", "{terminal} is not hosted");
        Viewer::new(socket).attach(request)
    }
    /// Whether a holder still runs `terminal`, as recorded.
    fn held_running(&self, terminal: &str) -> bool {
        self.sql()
            .query_row(
                "SELECT ended_at IS NULL FROM held_terminals WHERE id=?1",
                [terminal],
                |r| r.get(0),
            )
            .unwrap()
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
                std::fs::read_to_string(self.home.path().join("logs/lf.log"))
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
/// A held terminal's PTY byte position, sent beside its output.
const POSITION: u8 = 0x05;
/// lf's own events and reports to a held terminal's viewer.
const ASIDE: u8 = 0x06;
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
    nonce: String,
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
            nonce: String::new(),
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
        self.nonce = attached["nonce"].as_str().unwrap().to_owned();
    }
    /// The next item, or `None` once the server closes this connection.
    #[track_caller]
    fn next(&mut self) -> Option<Item> {
        let (mut tag, mut payload) = read_frame(&mut self.socket)?;
        while tag == POSITION {
            (tag, payload) = read_frame(&mut self.socket)?;
        }
        Some(match tag {
            REPLY => Item::Reply(serde_json::from_slice(&payload).unwrap()),
            SNAPSHOT => Item::Snapshot(payload),
            ERR => Item::Err(payload),
            OUT | ASIDE => {
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
            if let Item::Reply(reply) = self.next().expect("server closed before replying") {
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
        terminal.stop();
        assert_eq!(placement.as_deref(), valid.then_some("desktop-shell"));
    }
}

/// Bare `lf` behaves as its provider would started from the shell: the
/// provider leads the terminal's foreground group, no held terminal sits between, and
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
fn held_desktop_launch_links_its_session_to_the_pane() {
    let fixture = Fixture::new();
    let _service = ServerProof::start(&fixture, false);
    let path = fixture.waiting_provider();
    // The repository's conversation is managed, so a terminal viewing it holds it.
    let mut command = fixture.command(&["-a", "claude", "resume"]);
    command.env("PATH", path);
    let terminal = Terminal::spawn_with_placement(command, Some(true));
    let launch = fixture.launches(1).pop().unwrap();
    let session = session_of(&fixture.sql(), &launch);
    let held = fixture.terminal_of(&session);
    let placement: Option<String> = fixture.sql().query_row(
        "SELECT p.terminal_id FROM agent_sessions s JOIN processes a ON a.id=s.agent_process_id JOIN processes p ON p.id=a.attached_lf_process_id WHERE s.id=?1",
        [&session], |row| row.get(0),
    ).unwrap();
    terminal.stop();
    let provider: libc::pid_t = launch.split(':').next().unwrap().parse().unwrap();
    // SAFETY: the pid the fixture's provider recorded for itself.
    unsafe { libc::kill(provider, libc::SIGKILL) };
    fixture.held_ends(&held);
    assert_eq!(placement.as_deref(), Some("desktop-shell"));
}

/// Unviewed and quiet, a held terminal runs until its agent exits or Task
/// cleanup ends it. The lf process that takes up its handoff names it, since
/// a terminal is no process.
#[test]
fn a_held_conversation_outlives_its_view() {
    let fixture = Fixture::new();
    let _service = ServerProof::start(&fixture, false);
    let path = fixture.waiting_provider();
    let mut command = fixture.command(&["-a", "claude", "resume"]);
    command.env("PATH", path);
    let view = Terminal::spawn_with_placement(command, Some(true));
    let launch = fixture.launches(1).pop().unwrap();
    let provider: libc::pid_t = launch.split(':').next().unwrap().parse().unwrap();
    let _provider = KillOnDrop(provider);
    let session = session_of(&fixture.sql(), &launch);
    let held = fixture.terminal_of(&session);
    let named: Option<String> = fixture
        .sql()
        .query_row(
            "SELECT p.held_terminal_id FROM agent_sessions s
             JOIN processes a ON a.id=s.agent_process_id
             JOIN processes p ON p.id=a.attached_lf_process_id WHERE s.id=?1",
            [&session],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(named.as_deref(), Some(held.as_str()));
    view.stop();
    std::thread::sleep(Duration::from_secs(5));
    let ended: bool = fixture
        .sql()
        .query_row(
            "SELECT ended_at IS NOT NULL FROM held_terminals WHERE id=?1",
            [&held],
            |r| r.get(0),
        )
        .unwrap();
    assert!(!ended, "an unviewed held conversation ended for being idle");
    assert!(alive(provider), "the held agent ended for being idle");
    // SAFETY: the pid the fixture's provider recorded for itself.
    unsafe { libc::kill(provider, libc::SIGKILL) };
    fixture.held_ends(&held);
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

#[test]
fn piped_resume_json_runs_here_and_returns_once() {
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
}

fn prepared_thread_json(command: &str, fork: bool, source_provider: &str) {
    let fixture = Fixture::new();
    let _server = fixture.machine_server();
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
    let (next_input, request): (String, String) = sql
        .query_row(
            "SELECT c.receipt_key,s.request FROM agent_sessions s JOIN session_events c ON c.seq=s.current_capture WHERE s.id=?1",
            [id],
            |r| Ok((r.get(0)?, r.get(1)?)),
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

#[test]
fn headless_graph_settles_its_original_step() {
    headless_run(&["graph", "proof"], false);
}

#[test]
fn resume_refuses_a_graph_step_and_leaves_it_running() {
    headless_run(&["graph", "proof"], true);
}

#[test]
fn a_headless_run_settles_its_step() {
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
    assert_eq!(
        outcome(&step),
        None,
        "the step settled before its turn ended"
    );
    std::fs::remove_file(fixture.home.path().join("hold-turn")).unwrap();
    assert!(creator.wait().unwrap().success());
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
            .unwrap_or_else(|| panic!("server closed first: {items:?}"));
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
    let _server = fixture.machine_server();
    let held = fixture.start_held(&["/bin/sh", "-c", ECHO_LINES]);
    fixture.wait_file("drawn");
    std::thread::sleep(Duration::from_millis(100));
    let mut first = fixture.held_viewer(&held, true);
    let mut second = fixture.held_viewer(&held, false);
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
    let _server = fixture.machine_server();
    let held = fixture.start_held(&[
        "/bin/sh", "-c",
        "i=1; while [ $i -le 20 ]; do printf 'line %s: abcdefghijklmnopqrstuvwxyz0123456789\\r\\n' \"$i\"; i=$((i+1)); done; printf 'ready> '; read -r answer",
    ]);
    let mut first = fixture.held_viewer(&held, true);
    let mut expected = ghostty_vt::Terminal::new(80, 24).unwrap();
    for item in until_out(&mut first, "ready> ") {
        if let Item::Snapshot(bytes) | Item::Out(bytes) = item {
            expected.write(&bytes);
        }
    }
    expected.resize(40, 12).unwrap();
    let mut second = fixture.held_attach(
        &held,
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
fn resize_reaches_every_viewer_as_its_own_event_and_program_bytes_do_not() {
    let fixture = Fixture::new();
    let _server = fixture.machine_server();
    let held = fixture.start_held(&["/bin/sh", "-c", ECHO_LINES]);
    fixture.wait_file("drawn");
    let mut sizer = fixture.held_viewer(&held, true);
    let mut observer = fixture.held_viewer(&held, false);
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
    let _server = fixture.machine_server();
    let held = fixture.start_held(&["/bin/sh", "-c", ECHO_LINES]);
    fixture.wait_file("drawn");
    let mut typist = fixture.held_viewer(&held, true);
    let dark = json!({"background":"#111111","foreground":"#eeeeee"});
    let light = json!({"background":"#eeeeee","foreground":"#111111"});
    typist.request(json!({"kind":"theme","theme":dark}));
    typist.until_event("theme");
    let mut waiting = fixture.held_attach(
        &held,
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
    let mut taker = fixture.held_attach(
        &held,
        json!({"kind":"attach","control":false,"replace":true,"own":"take","rows":40,"cols":120,"name":"tmux %4"}),
    );
    let items = taker.until_event("owner");
    assert!(events(&items, "theme").is_empty(), "{items:?}");
    // An owner without a color report supplies no override, even to new observers.
    let mut observer = fixture.held_viewer(&held, false);
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
    let mut fresh = fixture.held_viewer(&held, false);
    let items = fresh.until_event("owner");
    assert_eq!(events(&items, "theme")[0]["theme"], light);
    typist.input("size\n");
    until_out(&mut typist, "30 90");
    typist.input("done\n");
    typist.finish();
}

#[test]
fn completed_task_cleanup_ends_its_held_shell_and_settles_jobs() {
    let fixture = Fixture::new();
    let _server = fixture.machine_server();
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
    let owner = fixture.wait_shell_terminal("cleanup-shell");
    // The launch attaches after the shell starts; take over only from it.
    fixture.wait_held_controlled(&owner);
    let mut viewer = fixture.held_viewer(&owner, true);
    launch.wait().unwrap();
    viewer.input("sleep 60 & printf '%s\\n' $! > \"$HOME/job\"\r");
    let job = fixture.wait_file("job").trim().parse::<u32>().unwrap();
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
    fixture.held_ends(&owner);
    fixture.shell_settles(&owner);
    let (settled, requested): (bool, i64) = fixture
        .sql()
        .query_row(
            "SELECT jobs_settled,close_requested FROM desktop_shells WHERE terminal_id=?1",
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

    fn wait_shell_terminal(&self, id: &str) -> String {
        // The shell can write its ready file before the launcher binds its terminal.
        let deadline = Instant::now() + Duration::from_secs(30);
        loop {
            let terminal: Option<String> = self
                .sql()
                .query_row(
                    "SELECT terminal_id FROM desktop_shells WHERE id=?1",
                    [id],
                    |r| r.get(0),
                )
                .unwrap();
            if let Some(terminal) = terminal {
                return terminal;
            }
            assert!(Instant::now() < deadline, "{id} terminal was never bound");
            std::thread::sleep(Duration::from_millis(20));
        }
    }

    /// Wait until the server confirms an ended shell left no jobs.
    fn shell_settles(&self, terminal: &str) {
        let deadline = Instant::now() + Duration::from_secs(20);
        while !self
            .sql()
            .query_row(
                "SELECT jobs_settled FROM desktop_shells WHERE terminal_id=?1",
                [terminal],
                |r| r.get::<_, bool>(0),
            )
            .unwrap()
        {
            assert!(Instant::now() < deadline, "{terminal} jobs never settled");
            std::thread::sleep(Duration::from_millis(50));
        }
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
    let _server = fixture.machine_server();
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
    let owner = fixture.wait_shell_terminal("desktop-shell");
    fixture.wait_held_controlled(&owner);
    let mut viewer = fixture.held_viewer(&owner, true);
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
    // Nobody viewing it ends nothing.
    std::thread::sleep(Duration::from_secs(4));
    assert!(fixture.held_running(&owner));
    let mut retry = fixture
        .command(&base)
        .arg("--create")
        .env("SHELL", "/bin/bash")
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .unwrap();
    fixture.wait_held_controlled(&owner);
    let mut returning = fixture.held_viewer(&owner, true);
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
    fixture.held_ends(&owner);
    fixture.shell_settles(&owner);
    let settled: bool = fixture
        .sql()
        .query_row(
            "SELECT jobs_settled FROM desktop_shells WHERE terminal_id=?1",
            [&owner],
            |r| r.get(0),
        )
        .unwrap();
    assert!(
        settled,
        "shell job settlement was not recorded: {:?}",
        std::fs::read_to_string(fixture.home.path().join("logs/lf.log"))
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
}

#[test]
fn desktop_shell_repeated_lifecycles_leave_no_jobs() {
    let fixture = Fixture::new();
    let _server = fixture.machine_server();
    let task = "task_5e55105e000040008000000000000006";
    fixture.shell_task(task);
    for cycle in 0..3 {
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
        let owner = fixture.wait_shell_terminal(&id);
        fixture.wait_held_controlled(&owner);
        let mut viewer = fixture.held_viewer(&owner, true);
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
        fixture.held_ends(&owner);
        fixture.shell_settles(&owner);
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
    assert_eq!((total, settled), (3, 3));
}

#[path = "transport/ssh_shells.rs"]
mod ssh_shells;

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
}

#[test]
fn server_task_watch_replaces_readings_across_repositories() {
    let fixture = Fixture::new();
    let portfolio = |args: &[&str]| {
        let output = fixture
            .command(&[&["desktop", "portfolio"], args].concat())
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
    };
    portfolio(&["manual"]);
    portfolio(&["add", fixture.repo.path().to_str().unwrap()]);
    let task = support::register_task_without_pr(
        fixture.home.path(),
        &fixture.repo.path().canonicalize().unwrap(),
        "mobile-watch",
        &fixture.repo.head_sha(),
    );
    let service = ServerProof::start(&fixture, false);
    let query = json!({"kind":"view.select","expression":"kind:task state:open"});
    let mut watch = service.connect();
    let tasks = watch
        .subscribe(serde_json::from_value(query.clone()).unwrap())
        .unwrap();
    let first = watch.reading(&tasks).unwrap().expect("a reading");
    let contents = |frame: &Value| -> Vec<Value> {
        frame["body"]
            .as_array()
            .unwrap_or_else(|| panic!("Expected a complete Task reading: {frame}"))
            .iter()
            .flat_map(|plan| plan["contents"].as_array().unwrap().clone())
            .collect()
    };
    assert_eq!(contents(&first).len(), 1);
    fixture
        .sql()
        .execute(
            "UPDATE tasks SET issue_title='Renamed on Desktop' WHERE id=?1",
            [task.task.id.as_str()],
        )
        .unwrap();
    let updated = watch.reading(&tasks).unwrap().expect("a reading");
    assert_eq!(contents(&updated)[0]["title"], "Renamed on Desktop");

    // A Task outside the portfolio is absent, as from Desktop's sidebar.
    let second = TestRepo::new();
    support::register_task_without_pr(
        fixture.home.path(),
        &second.path().canonicalize().unwrap(),
        "other-repository",
        &second.head_sha(),
    );
    watch.checkpoint(&tasks, 11).unwrap();
    loop {
        let reading = watch.reading(&tasks).unwrap().expect("a reading");
        if reading["answers"] == 11 {
            assert_eq!(contents(&reading).len(), 1);
            break;
        }
    }
    portfolio(&["add", second.path().to_str().unwrap()]);
    loop {
        if contents(&watch.reading(&tasks).unwrap().expect("a reading")).len() == 2 {
            break;
        }
    }
    watch.checkpoint(&tasks, 12).unwrap();
    loop {
        let reading = watch.reading(&tasks).unwrap().expect("a reading");
        if reading["answers"] == 12 {
            assert_eq!(contents(&reading).len(), 2);
            break;
        }
    }
    drop(watch);
    // The one-shot read shares the subscription's query and projection.
    let plans = service.request(query);
    assert_eq!(plans.as_array().unwrap().len(), 2);
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
fn server_mobile_changes_preserve_local_connections_and_saved_disable() {
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
    service.configure_remote(Value::Null);
    service.restart();
    service.ready();
    assert!(std::net::TcpStream::connect(("127.0.0.1", port)).is_err());
    // A saved approval does not migrate to a new Tailscale owner after restart.
    service.configure_remote(config.clone());
    let mut changed = status;
    changed["Self"]["UserID"] = json!(2);
    changed["User"]["2"] = json!({"LoginName":"other@example.com"});
    std::fs::write(status_path, changed.to_string()).unwrap();
    service.restart();
    service.ready();
    assert!(std::net::TcpStream::connect(("127.0.0.1", port)).is_err());
    let deadline = Instant::now() + Duration::from_secs(15);
    loop {
        let mobile = service.mobile_status();
        if let Some(error) = mobile["error"].as_str() {
            assert!(error.contains("Tailscale account changed"), "{error}");
            assert_eq!(mobile["ready"], false);
            assert_eq!(mobile["configuration"]["owner"]["user"], 1);
            break;
        }
        assert!(
            Instant::now() < deadline,
            "Changed account was not reported"
        );
        std::thread::sleep(Duration::from_millis(20));
    }
    drop(service);
    let offline = fixture
        .command(&["server", "mobile", "--status"])
        .output()
        .unwrap();
    assert!(offline.status.success());
    let offline: Value = serde_json::from_slice(&offline.stdout).unwrap();
    assert_eq!(offline["configuration"]["enabled"], true);
    assert_eq!(offline["ready"], false);
    assert!(offline["error"].is_string());
    assert!(fixture
        .command(&["server", "mobile", "--off"])
        .output()
        .unwrap()
        .status
        .success());
    let persisted: Value =
        serde_yaml_ng::from_str(&std::fs::read_to_string(saved).unwrap()).unwrap();
    assert_eq!(persisted["enabled"], false);
    assert!(!fixture.home.path().join("server.plist").exists());
}

#[test]
fn server_mobile_setup_repairs_obsolete_settings_and_occupied_ports() {
    let fixture = Fixture::new();
    let certificate =
        rcgen::generate_simple_self_signed(vec!["fixture.tail.ts.net".into()]).unwrap();
    let status = json!({"BackendState":"Running", "Self":{"ID":"node1", "DNSName":"fixture.tail.ts.net.", "UserID":1, "TailscaleIPs":["127.0.0.1"]}, "User":{"1":{"LoginName":"owner@example.com"}}});
    mobile_host(
        &fixture,
        &status,
        &format!(
            "{}{}",
            certificate.cert.pem(),
            certificate.signing_key.serialize_pem()
        ),
    );
    let saved = fixture.home.path().join("mobile.yaml");
    let preferences = fixture.home.path().join("config.yaml");
    let unrelated = "agent: claude\n";
    std::fs::write(&preferences, unrelated).unwrap();
    let settings = json!({"enabled":true,"endpoint":"https://obsolete.tail.ts.net:8443"});
    std::fs::write(&saved, serde_yaml_ng::to_string(&settings).unwrap()).unwrap();
    let service = ServerProof::start(&fixture, false);
    let instance = service.ready();
    let initial = service.mobile_status();
    assert_eq!(initial["ready"], false);
    assert!(initial["error"].is_string());
    let setup = || {
        let output = fixture
            .command(&["server", "mobile"])
            .env(
                "PATH",
                format!(
                    "{}:/usr/bin:/bin",
                    fixture.home.path().join("bin").display()
                ),
            )
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        serde_json::from_slice::<Value>(&output.stdout).unwrap()
    };
    let first = setup();
    assert_eq!(service.ready(), instance);
    assert_eq!(first["owner"], json!({"node":"node1","user":1}));
    let persisted: Value =
        serde_yaml_ng::from_str(&std::fs::read_to_string(&saved).unwrap()).unwrap();
    assert_eq!(persisted, first);
    assert_eq!(std::fs::read_to_string(&preferences).unwrap(), unrelated);
    let machine = service.request(json!({"kind":"machine.self"}));
    assert!(machine["id"].is_string(), "{machine}");
    assert_eq!(service.connect().welcome().machine.as_ref(), Some(&machine));

    let port = reqwest::Url::parse(first["endpoint"].as_str().unwrap())
        .unwrap()
        .port()
        .unwrap();
    drop(service);
    let occupied = std::net::TcpListener::bind(("127.0.0.1", port)).unwrap();
    let service = ServerProof::start(&fixture, false);
    let instance = service.ready();
    let deadline = Instant::now() + Duration::from_secs(15);
    loop {
        let mobile = service.mobile_status();
        if mobile["error"].is_string() {
            assert_eq!(mobile["ready"], false);
            assert_eq!(
                mobile["configuration"], first,
                "Restoration must keep the saved address"
            );
            break;
        }
        assert!(
            Instant::now() < deadline,
            "Occupied port was not reported: {mobile}"
        );
        std::thread::sleep(Duration::from_millis(20));
    }
    let repaired = setup();
    assert_ne!(repaired["endpoint"], first["endpoint"]);
    assert_eq!(repaired["owner"], first["owner"]);
    assert_eq!(service.ready(), instance);
    let ready = service.mobile_status();
    assert_eq!(ready["ready"], true);
    assert_eq!(ready["configuration"], repaired);
    let persisted: Value =
        serde_yaml_ng::from_str(&std::fs::read_to_string(&saved).unwrap()).unwrap();
    assert_eq!(persisted, repaired);
    assert_eq!(occupied.local_addr().unwrap().port(), port);

    drop(service);
    std::fs::write(&saved, serde_yaml_ng::to_string(&settings).unwrap()).unwrap();
    let off = fixture
        .command(&["server", "mobile", "--off"])
        .output()
        .unwrap();
    assert!(
        off.status.success(),
        "{}",
        String::from_utf8_lossy(&off.stderr)
    );
    let persisted: Value =
        serde_yaml_ng::from_str(&std::fs::read_to_string(&saved).unwrap()).unwrap();
    assert_eq!(
        persisted,
        json!({}),
        "Offline disable clears obsolete mobile settings"
    );
    assert!(!fixture.home.path().join("server.plist").exists());

    assert_eq!(std::fs::read_to_string(preferences).unwrap(), unrelated);
    // A damaged file stays available for inspection.
    let invalid = "enabled: [unfinished";
    std::fs::write(&saved, invalid).unwrap();
    let off = fixture
        .command(&["server", "mobile", "--off"])
        .output()
        .unwrap();
    assert!(!off.status.success());
    assert_eq!(std::fs::read_to_string(saved).unwrap(), invalid);
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
    let terminal = uuid::Uuid::new_v4().to_string();
    let mut command = fixture.command(&[
        "session",
        "attach",
        "--terminal",
        &terminal,
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

/// The server reads every held terminal's Program Status, viewed or not:
/// an agent asking a question with no view open is waiting on its person.
#[test]
fn a_held_terminal_with_no_view_reports_that_it_waits() {
    let fixture = Fixture::new();
    let _service = ServerProof::start(&fixture, false);
    let path = fixture.provider(
        r#"#!/bin/sh
if [ "$1" = --version ]; then echo '2.1.302'; exit; fi
printf '%s:%s:%s\n' "$$" "$LF_CAPTURE_KEY" "$*" >> "$HOME/launches"
printf '\033]7501;state=working:app=claude-code\033\\'
while [ ! -e "$HOME/ask" ]; do sleep 0.05; done
printf 'May I?\r\n\033]7501;state=blocked:app=claude-code\033\\'
read -r answer < /dev/tty
"#,
    );
    let mut launch = fixture.command(&["-a", "claude", "resume"]);
    launch.env("PATH", &path);
    let first = Terminal::spawn_with_placement(launch, Some(true));
    let launched = fixture.launches(1).pop().unwrap();
    let provider: libc::pid_t = launched.split(':').next().unwrap().parse().unwrap();
    let _provider = KillOnDrop(provider);
    let session = session_of(&fixture.sql(), &launched);
    let held = fixture.terminal_of(&session);
    first.stop();
    let available = || {
        let listed = fixture
            .command(&["session", "list", "--json", "--all"])
            .output()
            .unwrap();
        assert!(listed.status.success(), "{listed:?}");
        let listed: Value = serde_json::from_slice(&listed.stdout).unwrap();
        listed
            .as_array()
            .into_iter()
            .flatten()
            .chain(listed["sessions"].as_array().into_iter().flatten())
            .find(|row| row["id"] == session.as_str())
            .map(|row| row["availability"] == "available")
    };
    assert_eq!(available(), Some(false), "a working agent waits on nobody");
    std::fs::write(fixture.home.path().join("ask"), "").unwrap();
    let deadline = Instant::now() + Duration::from_secs(20);
    while available() != Some(true) {
        assert!(
            Instant::now() < deadline,
            "the question with no view open never read as waiting"
        );
        std::thread::sleep(Duration::from_millis(100));
    }
    // SAFETY: the pid the fixture's provider recorded for itself.
    unsafe { libc::kill(provider, libc::SIGKILL) };
    fixture.held_ends(&held);
}

/// A Session an earlier build's `lf session serve` owner still runs restarts
/// held when resumed: that owner, found by its recorded pid and birth, ends
/// first, so no second agent runs on its thread.
#[test]
fn resume_ends_an_earlier_builds_owner_and_continues_held() {
    use std::os::unix::process::ExitStatusExt;
    let fixture = Fixture::new();
    let _server = fixture.machine_server();
    let path = fixture.waiting_provider();
    let prepared = fixture
        .command(&["-a", "claude", "resume", "--prepare", "--json"])
        .env("PATH", &path)
        .output()
        .unwrap();
    assert!(prepared.status.success(), "{prepared:?}");
    let prepared: Value = serde_json::from_slice(&prepared.stdout).unwrap();
    let session = prepared["id"].as_str().unwrap().to_owned();
    // The earlier build's owner: a plain process its row names by pid and birth.
    let mut stand_in = Command::new("/bin/sleep").arg("600").spawn().unwrap();
    let born = time::OffsetDateTime::now_utc().unix_timestamp();
    let pid = stand_in.id();
    let (exited, reaped) = std::sync::mpsc::channel();
    std::thread::spawn(move || exited.send(stand_in.wait().unwrap()));
    let owner = "00000000-0000-4000-8000-0000000008a1";
    let sql = fixture.sql();
    sql.execute(
        r#"INSERT INTO processes(id,trace_id,command,started_at,pid,os_started_at)
           VALUES(?1,?1,'["lf","session","serve","--terminal","--","lf"]',?2,?3,?2)"#,
        rusqlite::params![owner, born, pid],
    )
    .unwrap();
    sql.execute(
        "UPDATE agent_sessions SET transport_lf_process_id=?1,held_terminal_id=NULL WHERE id=?2",
        rusqlite::params![owner, session],
    )
    .unwrap();
    let mut command = fixture.command(&["resume", &session]);
    command.env("PATH", path);
    let view = Terminal::spawn_with_placement(command, Some(true));
    let launch = fixture.launches(1).pop().unwrap();
    let provider: libc::pid_t = launch.split(':').next().unwrap().parse().unwrap();
    let _provider = KillOnDrop(provider);
    let status = reaped
        .recv_timeout(Duration::from_secs(10))
        .expect("the earlier owner outlived the resume");
    assert_eq!(status.signal(), Some(libc::SIGTERM));
    let held = fixture.terminal_of(&session);
    assert!(fixture.held_running(&held));
    let owner_after: Option<String> = sql
        .query_row(
            "SELECT transport_lf_process_id FROM agent_sessions WHERE id=?1",
            [&session],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(owner_after, None);
    assert_eq!(fixture.launches(1).len(), 1);
    view.stop();
    // SAFETY: the pid the fixture's provider recorded for itself.
    unsafe { libc::kill(provider, libc::SIGKILL) };
    fixture.held_ends(&held);
}

#[test]
fn continuing_a_session_starts_one_held_terminal() {
    let fixture = Fixture::new();
    let service = ServerProof::start(&fixture, false);
    let prepared = service
        .command(&["-a", "claude", "resume", "--prepare", "--json"])
        .output()
        .unwrap();
    assert!(
        prepared.status.success(),
        "{}",
        String::from_utf8_lossy(&prepared.stderr)
    );
    let prepared: Value = serde_json::from_slice(&prepared.stdout).unwrap();
    let session = prepared["id"].as_str().unwrap();
    let continuing = |session: &str| {
        service
            .connect()
            .request(loopflow::server::wire::Operation::Server(
                loopflow::server::wire::ServerOperation::SessionContinue {
                    session: session.into(),
                    request: "phone-continue".into(),
                    rows: 40,
                    cols: 60,
                },
            ))
    };
    let started = continuing(session).unwrap();
    let terminal = started.result["terminal_id"].as_str().unwrap().to_owned();
    // A repeat after a lost answer starts nothing.
    let repeated = continuing(session).unwrap();
    assert_eq!(repeated.result["terminal_id"], terminal.as_str());
    let missing = continuing("session_00000000000040008000000000000099").unwrap_err();
    assert!(
        format!("{missing:#}").contains("Session is unavailable"),
        "{missing:#}"
    );
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
    let started = service
        .connect()
        .request(loopflow::server::wire::Operation::Server(
            loopflow::server::wire::ServerOperation::SessionStart {
                session: None,
                request: "held-restart".into(),
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
    let terminal = started.result["terminal_id"].as_str().unwrap().to_owned();
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
    // The agent's parent is the holder, not the server.
    let (holder, _) = command_line(agent);
    let (_, holder_command) = command_line(holder);
    assert!(holder_command.contains("holder run"), "{holder_command}");

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
    let started = service
        .connect()
        .request(loopflow::server::wire::Operation::Server(
            loopflow::server::wire::ServerOperation::SessionStart {
                session: None,
                request: "held-other-build".into(),
                argv: vec!["/bin/sh".into(), "-c".into(), script],
                cwd: home.to_path_buf(),
                env: vec![("PATH".into(), "/usr/bin:/bin".into())],
                rows: 24,
                cols: 80,
                placement: None,
            },
        ))
        .unwrap();
    let terminal = started.result["terminal_id"].as_str().unwrap().to_owned();
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
/// their views wait and continue once it answers. Gone instead, its terminals
/// and its row settle with an unknown outcome.
#[test]
fn a_held_view_waits_for_a_holder_stopped_at_server_startup() {
    let fixture = Fixture::new();
    let mut service = ServerProof::start(&fixture, false);
    let home = fixture.home.path();
    let script = format!(
        "echo $$ > {home}/agent.pid; i=0; while :; do i=$((i+1)); printf 'tick:%d\\n' $i; sleep 0.1; done",
        home = home.display()
    );
    let started = service
        .connect()
        .request(loopflow::server::wire::Operation::Server(
            loopflow::server::wire::ServerOperation::SessionStart {
                session: None,
                request: "held-stopped".into(),
                argv: vec!["/bin/sh".into(), "-c".into(), script],
                cwd: home.to_path_buf(),
                env: vec![("PATH".into(), "/usr/bin:/bin".into())],
                rows: 24,
                cols: 80,
                placement: None,
            },
        ))
        .unwrap();
    let terminal = started.result["terminal_id"].as_str().unwrap().to_owned();
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

/// `session.attach` for a terminal whose holder has not answered this server
/// is told to ask again, never that the terminal ended; once the holder
/// answers, the same request attaches. A ticket for a terminal that ended
/// before it was presented is refused by name.
#[test]
fn session_attach_waits_for_a_holder_stopped_at_server_startup() {
    use loopflow::engine::{Error, ErrorCode};
    use loopflow::server::wire::{Operation, ServerOperation};

    let fixture = Fixture::new();
    let mut service = ServerProof::start(&fixture, false);
    let home = fixture.home.path();
    let session: loopflow::id::LfSessionId = "session_held_attach".into();
    let script = format!(
        "echo $$ > {home}/agent.pid; i=0; while [ ! -e {home}/stop ]; do i=$((i+1)); printf 'tick:%d\\n' $i; sleep 0.1; done",
        home = home.display()
    );
    let started = service
        .connect()
        .request(Operation::Server(ServerOperation::SessionStart {
            session: Some(session.clone()),
            request: "held-attach".into(),
            argv: vec!["/bin/sh".into(), "-c".into(), script],
            cwd: home.to_path_buf(),
            env: vec![("PATH".into(), "/usr/bin:/bin".into())],
            rows: 24,
            cols: 80,
            placement: None,
        }))
        .unwrap();
    let terminal = started.result["terminal_id"].as_str().unwrap().to_owned();
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
                terminal_id: terminal_id.map(|id| serde_json::from_value(json!(id)).unwrap()),
            }))
    };
    let refused = |failure: anyhow::Error| failure.downcast::<Error>().unwrap();

    // The holder is stopped while a new server starts.
    service.stop();
    signal(libc::SIGSTOP);
    service.child = Some(service.launch());
    service.ready();
    for named in [None, Some(terminal.as_str())] {
        let waiting = refused(ticket(&service, named).unwrap_err());
        assert_eq!(waiting.code, ErrorCode::Unavailable, "{waiting:?}");
        let data = waiting.data.unwrap();
        assert_eq!(data["terminal_id"], terminal.as_str(), "{data}");
        assert_eq!(data["retry"], true, "{data}");
    }

    // It answers: asking again attaches to the same agent.
    signal(libc::SIGCONT);
    let deadline = Instant::now() + Duration::from_secs(30);
    let mut socket = loop {
        match loopflow::server::client::attach(&fixture.endpoint("machine"), &session) {
            Ok(socket) => break socket,
            Err(failure) => {
                let waiting = refused(failure);
                assert_eq!(waiting.code, ErrorCode::Unavailable, "{waiting:?}");
                assert!(Instant::now() < deadline, "the holder was never taken up");
                std::thread::sleep(Duration::from_millis(100));
            }
        }
    };
    socket
        .set_read_timeout(Some(Duration::from_secs(15)))
        .unwrap();
    socket
        .write_all(&frame(
            REQUEST,
            json!({"id":1,"kind":"attach","control":true,"replace":true,"own":"take","rows":24,"cols":80})
                .to_string()
                .as_bytes(),
        ))
        .unwrap();
    let mut seen = Vec::new();
    while !String::from_utf8_lossy(&seen).contains("tick:") {
        let (tag, payload) = read_frame(&mut socket).expect("the bridge closed before output");
        if tag == SNAPSHOT || tag == OUT {
            seen.extend(payload);
        }
    }
    drop(socket);

    // A ticket issued while the terminal ran, presented after it ended.
    let issued = ticket(&service, Some(&terminal)).unwrap();
    std::fs::write(home.join("stop"), "").unwrap();
    fixture.held_ends(&terminal);
    let mut bridge = UnixStream::connect(fixture.endpoint("machine")).unwrap();
    bridge
        .set_read_timeout(Some(Duration::from_secs(15)))
        .unwrap();
    bridge
        .write_all(
            format!(
                "{}\n",
                json!({"type":"attach","ticket":issued.result["ticket"]})
            )
            .as_bytes(),
        )
        .unwrap();
    let mut answer = String::new();
    BufReader::new(bridge).read_line(&mut answer).unwrap();
    let answer: Value = serde_json::from_str(&answer).unwrap();
    assert_eq!(answer["type"], "error", "{answer}");
    assert_eq!(answer["code"], "not_found", "{answer}");
    assert_eq!(answer["data"]["terminal_id"], terminal.as_str(), "{answer}");
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

/// One server thread hosts every held terminal and its views: starting more
/// terminals and viewing them adds no thread each.
#[test]
fn server_threads_do_not_grow_with_held_terminals() {
    let fixture = Fixture::new();
    let service = ServerProof::start(&fixture, false);
    let home = fixture.home.path();
    let server = service.child.as_ref().unwrap().id();
    let start = |request: String| {
        let script = format!(
            "while [ ! -e {}/stop ]; do echo tick; sleep 0.1; done",
            home.display()
        );
        let started = service
            .connect()
            .request(loopflow::server::wire::Operation::Server(
                loopflow::server::wire::ServerOperation::SessionStart {
                    session: None,
                    request,
                    argv: vec!["/bin/sh".into(), "-c".into(), script],
                    cwd: home.to_path_buf(),
                    env: vec![("PATH".into(), "/usr/bin:/bin".into())],
                    rows: 24,
                    cols: 80,
                    placement: None,
                },
            ))
            .unwrap();
        let terminal = started.result["terminal_id"].as_str().unwrap().to_owned();
        let (view, answer) = fixture.held_entrance(&terminal);
        assert_eq!(answer, "hosted");
        (terminal, view)
    };
    let mut held = vec![start("first".into())];
    let one = thread_count(server);
    const MORE: usize = 8;
    held.extend((0..MORE).map(|more| start(format!("more-{more}"))));
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
    std::fs::write(home.join("stop"), "").unwrap();
    for (terminal, _view) in &held {
        fixture.held_ends(terminal);
    }
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
    let started = service
        .connect()
        .request(loopflow::server::wire::Operation::Server(
            loopflow::server::wire::ServerOperation::SessionStart {
                session: None,
                request: "abandoned".into(),
                argv: vec!["/bin/sh".into(), "-c".into(), "sleep 600".into()],
                cwd: home.to_path_buf(),
                env: vec![("PATH".into(), "/usr/bin:/bin".into())],
                rows: 24,
                cols: 80,
                placement: None,
            },
        ))
        .unwrap();
    let terminal = started.result["terminal_id"].as_str().unwrap().to_owned();
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
/// once across a server restart; no owner process runs per terminal.
#[test]
fn held_terminals_list_one_holder_process() {
    let fixture = Fixture::new();
    let mut service = ServerProof::start(&fixture, false);
    let home = fixture.home.path();
    let start = |service: &ServerProof, request: &str| {
        let script = format!("while [ ! -e {}/stop ]; do sleep 0.1; done", home.display());
        let started = service
            .connect()
            .request(loopflow::server::wire::Operation::Server(
                loopflow::server::wire::ServerOperation::SessionStart {
                    session: None,
                    request: request.into(),
                    argv: vec!["/bin/sh".into(), "-c".into(), script],
                    cwd: home.to_path_buf(),
                    env: vec![("PATH".into(), "/usr/bin:/bin".into())],
                    rows: 24,
                    cols: 80,
                    placement: None,
                },
            ))
            .unwrap();
        started.result["terminal_id"].as_str().unwrap().to_owned()
    };
    let terminals = [start(&service, "first"), start(&service, "second")];
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
