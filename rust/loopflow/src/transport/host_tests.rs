//! Host rules, driven through `Hosted::pass` in this process: the same code an
//! owner and the Machine server's held terminals run. A test plays the program
//! on the far side of a PTY or pipes and its viewers on socket pairs; time
//! moves by backdating the host's own timestamps.

use std::fs::File;
use std::io::{self, Read, Write};
use std::ops::{Index, IndexMut};
use std::os::fd::{AsRawFd, FromRawFd};
use std::os::unix::net::UnixStream;
use std::os::unix::process::ExitStatusExt;
use std::process::ExitStatus;
use std::sync::mpsc;
use std::time::{Duration, Instant};

use serde_json::{json, Value};

use super::{Child, Hosted, Local, Pass, Watchers};
use crate::id::LfProcessId;
use crate::journal::TestLedgerGuard;
use crate::store::sqlite::ViewerHold;
use crate::transport::{
    nonblocking, terminal_pair, Connection, Host, GRACE, HEARTBEAT_TIMEOUT, LIMIT, SPOOL,
};

/// What the execution's own control connections present.
const KEY: &str = "fixture";
/// How long a test waits for something the host should do at once.
const PATIENCE: Duration = Duration::from_secs(5);

/// What a viewer receives: replies, stream bytes and the events riding in
/// stream 1 under its nonce.
#[derive(Debug)]
enum Item {
    Reply(Value),
    Snapshot(Vec<u8>),
    Out(Vec<u8>),
    Err(Vec<u8>),
    Event(Value),
}

/// One test viewer, by its place in the bench.
#[derive(Debug, Clone, Copy)]
struct V(usize);

struct Peer {
    socket: Option<UnixStream>,
    /// Read from the socket, not yet decoded.
    buffer: Vec<u8>,
    /// The host closed this connection.
    closed: bool,
    /// Whether the bench drains this socket; a slow viewer reads nothing.
    reading: bool,
    token: Option<String>,
    id: String,
    reconnect: String,
    nonce: String,
    /// Stream bytes received, as a resume reports them.
    received: u64,
    /// Every byte read, headers included.
    wire: u64,
    ids: u64,
}

impl Peer {
    fn accept(&mut self, attached: &Value) {
        self.token = attached["token"].as_str().map(str::to_owned);
        self.id = attached["viewer"].as_str().unwrap().to_owned();
        self.reconnect = attached["reconnect"].as_str().unwrap().to_owned();
        self.nonce = attached["nonce"].as_str().unwrap().to_owned();
    }

    fn drain(&mut self) {
        let Some(socket) = self.socket.as_mut().filter(|_| self.reading) else {
            return;
        };
        let mut bytes = [0; 65536];
        loop {
            match socket.read(&mut bytes) {
                Ok(0) => {
                    self.closed = true;
                    return;
                }
                Ok(n) => self.buffer.extend(&bytes[..n]),
                Err(error) if error.kind() == io::ErrorKind::WouldBlock => return,
                Err(_) => {
                    self.closed = true;
                    return;
                }
            }
        }
    }

    /// The next whole frame read so far.
    fn decode(&mut self) -> Option<Item> {
        if self.buffer.len() < 5 {
            return None;
        }
        let length = u32::from_be_bytes(self.buffer[1..5].try_into().unwrap()) as usize;
        if self.buffer.len() < 5 + length {
            return None;
        }
        let tag = self.buffer[0];
        let payload: Vec<u8> = self.buffer.drain(..5 + length).skip(5).collect();
        self.wire += 5 + length as u64;
        if tag != 0x01 {
            self.received += length as u64;
        }
        Some(match tag {
            0x01 => Item::Reply(serde_json::from_slice(&payload).unwrap()),
            0x02 => Item::Snapshot(payload),
            0x04 => Item::Err(payload),
            0x03 => {
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
}

/// The program's side of the execution.
struct Program {
    out: Option<File>,
    err: Option<File>,
    input: Option<File>,
    /// Input the program has read.
    typed: Vec<u8>,
}

struct Bench {
    host: Hosted,
    child: Child,
    program: Program,
    exits: mpsc::Sender<io::Result<ExitStatus>>,
    peers: Vec<Peer>,
    ended: Option<u8>,
    _wake: UnixStream,
    _watchers: Watchers,
    ledger: TestLedgerGuard,
}

impl Index<V> for Bench {
    type Output = Peer;
    fn index(&self, viewer: V) -> &Peer {
        &self.peers[viewer.0]
    }
}

impl IndexMut<V> for Bench {
    fn index_mut(&mut self, viewer: V) -> &mut Peer {
        &mut self.peers[viewer.0]
    }
}

fn pipe() -> (File, File) {
    let mut fds = [0; 2];
    // SAFETY: pipe writes two new descriptors into a live two-element array.
    assert_eq!(unsafe { libc::pipe(fds.as_mut_ptr()) }, 0);
    // SAFETY: pipe returned two newly owned descriptors.
    unsafe { (File::from_raw_fd(fds[0]), File::from_raw_fd(fds[1])) }
}

impl Bench {
    /// A program on a PTY, as `lf session serve --terminal` runs one.
    fn terminal() -> Self {
        let (master, slave) = terminal_pair().unwrap();
        // SAFETY: tcgetattr and tcsetattr read and write a live termios on the slave.
        unsafe {
            let mut termios = std::mem::zeroed::<libc::termios>();
            assert_eq!(libc::tcgetattr(slave.as_raw_fd(), &mut termios), 0);
            libc::cfmakeraw(&mut termios);
            assert_eq!(
                libc::tcsetattr(slave.as_raw_fd(), libc::TCSANOW, &termios),
                0
            );
        }
        nonblocking(master.as_raw_fd()).unwrap();
        nonblocking(slave.as_raw_fd()).unwrap();
        let input = master.try_clone().unwrap();
        let program = Program {
            input: Some(slave.try_clone().unwrap()),
            out: Some(slave),
            err: None,
            typed: Vec::new(),
        };
        Self::new(true, vec![(1, master)], input, program)
    }

    /// A program on pipes, as a piped or `-b` execution runs one.
    fn pipes() -> Self {
        let (out_reader, out) = pipe();
        let (err_reader, err) = pipe();
        let (program_input, input) = pipe();
        for file in [&out_reader, &err_reader, &input, &out, &err, &program_input] {
            nonblocking(file.as_raw_fd()).unwrap();
        }
        let program = Program {
            out: Some(out),
            err: Some(err),
            input: Some(program_input),
            typed: Vec::new(),
        };
        Self::new(
            false,
            vec![(1, out_reader), (2, err_reader)],
            input,
            program,
        )
    }

    fn new(terminal: bool, readers: Vec<(u8, File)>, input: File, program: Program) -> Self {
        let ledger = TestLedgerGuard::new();
        let watchers = Watchers::spawn();
        let (waker, wake) = UnixStream::pair().unwrap();
        waker.set_nonblocking(true).unwrap();
        wake.set_nonblocking(true).unwrap();
        let host = Hosted::new(
            Host::Owner(LfProcessId::new()),
            terminal,
            None,
            KEY.into(),
            waker,
            &watchers,
        )
        .unwrap();
        let (exits, exit) = mpsc::channel();
        Self {
            host,
            child: Child::Local(Local {
                readers,
                input: Some(input),
                cooked: None,
                exit,
                pid: std::process::id(),
                shell_jobs: None,
            }),
            program,
            exits,
            peers: Vec::new(),
            ended: None,
            _wake: wake,
            _watchers: watchers,
            ledger,
        }
    }

    /// One host pass, then every reading viewer and the program read what
    /// is ready for them.
    fn pass(&mut self) {
        if self.ended.is_none() {
            if let Pass::Ended(code, _) = self.host.pass(&mut self.child).unwrap() {
                self.ended = Some(code);
                // The owner process exits here, closing every connection.
                self.host.viewers.clear();
            }
        }
        for peer in &mut self.peers {
            peer.drain();
        }
        if let Some(input) = self.program.input.as_mut() {
            let mut bytes = [0; 65536];
            while let Ok(n @ 1..) = input.read(&mut bytes) {
                self.program.typed.extend(&bytes[..n]);
            }
        }
    }

    /// Pass until `done` holds, failing after `PATIENCE`.
    #[track_caller]
    fn pass_until(&mut self, what: &str, mut done: impl FnMut(&mut Self) -> bool) {
        let deadline = Instant::now() + PATIENCE;
        loop {
            self.pass();
            if done(self) {
                return;
            }
            assert!(Instant::now() < deadline, "never saw {what}");
            std::thread::sleep(Duration::from_micros(200));
        }
    }

    // -- program ------------------------------------------------------------

    /// The program writes `bytes` to stream 1, while the host keeps relaying.
    fn print(&mut self, bytes: &[u8]) {
        self.write(1, bytes);
    }

    fn write(&mut self, stream: u8, bytes: &[u8]) {
        let deadline = Instant::now() + PATIENCE;
        let mut rest = bytes;
        while !rest.is_empty() {
            let file = match stream {
                2 => self.program.err.as_mut(),
                _ => self.program.out.as_mut(),
            }
            .expect("the program's output is open");
            match file.write(rest) {
                Ok(n) => rest = &rest[n..],
                Err(error) if error.kind() == io::ErrorKind::WouldBlock => {
                    assert!(Instant::now() < deadline, "the host stopped reading output");
                    self.pass();
                }
                Err(error) => panic!("program output: {error}"),
            }
        }
        self.pass();
    }

    /// Pass until the program has read `text`; returns all it has read.
    #[track_caller]
    fn typed(&mut self, text: &str) -> String {
        self.pass_until(&format!("program input {text:?}"), |bench| {
            String::from_utf8_lossy(&bench.program.typed).contains(text)
        });
        String::from_utf8_lossy(&self.program.typed).into_owned()
    }

    /// The PTY's size, as the program reads it.
    fn size(&self) -> (u16, u16) {
        let mut size = libc::winsize {
            ws_row: 0,
            ws_col: 0,
            ws_xpixel: 0,
            ws_ypixel: 0,
        };
        let fd = self.program.out.as_ref().unwrap().as_raw_fd();
        // SAFETY: ioctl writes a winsize into live storage of the right size.
        assert_eq!(unsafe { libc::ioctl(fd, libc::TIOCGWINSZ, &mut size) }, 0);
        (size.ws_row, size.ws_col)
    }

    /// The program exits with `code`; the host announces it and ends.
    fn exit(&mut self, code: i32) {
        self.program.out = None;
        self.program.err = None;
        self.program.input = None;
        self.exits
            .send(Ok(ExitStatus::from_raw(code << 8)))
            .unwrap();
        self.pass_until("the exit", |bench| bench.host.exit.is_some());
        // Skip the short wait for output still in flight.
        let (code, at) = self.host.exit.take().unwrap();
        self.host.exit = Some((code, at - Duration::from_millis(100)));
        self.pass_until("the host to end", |bench| bench.ended.is_some());
    }

    // -- viewers --------------------------------------------------------------

    fn connect(&mut self) -> V {
        let (socket, peer) = UnixStream::pair().unwrap();
        socket.set_nonblocking(true).unwrap();
        peer.set_nonblocking(true).unwrap();
        self.host.admit(socket);
        self.peers.push(Peer {
            socket: Some(peer),
            buffer: Vec::new(),
            closed: false,
            reading: true,
            token: None,
            id: String::new(),
            reconnect: String::new(),
            nonce: String::new(),
            received: 0,
            wire: 0,
            ids: 0,
        });
        V(self.peers.len() - 1)
    }

    #[track_caller]
    fn attach(&mut self, request: Value) -> V {
        let viewer = self.connect();
        self.request(viewer, request);
        self.attached(viewer);
        viewer
    }

    /// The reply to an attach or resume, which this peer now speaks as.
    #[track_caller]
    fn attached(&mut self, viewer: V) -> Value {
        let reply = self.until_reply(viewer);
        assert_eq!(reply["kind"], "attached", "{reply}");
        self[viewer].accept(&reply);
        reply
    }

    /// A resume of `from`'s stream at everything it received; taking
    /// ownership also claims input.
    fn resume(&self, from: V, own: Option<&str>, rows: u16, cols: u16) -> Value {
        json!({"kind":"resume","reconnect":self[from].reconnect,"received":self[from].received,
            "control":own.is_some(),"own":own,"rows":rows,"cols":cols})
    }

    #[track_caller]
    fn viewer(&mut self, control: bool) -> V {
        self.attach(json!({"kind":"attach","control":control,"replace":true,
            "own":control.then_some("take"),"rows":24,"cols":80}))
    }

    fn send(&mut self, viewer: V, tag: u8, payload: &[u8]) {
        let mut bytes = vec![tag];
        bytes.extend((payload.len() as u32).to_be_bytes());
        bytes.extend(payload);
        let deadline = Instant::now() + PATIENCE;
        let mut rest = bytes.as_slice();
        while !rest.is_empty() {
            let socket = self[viewer].socket.as_mut().expect("viewer is connected");
            match socket.write(rest) {
                Ok(n) => rest = &rest[n..],
                Err(error) if error.kind() == io::ErrorKind::WouldBlock => {
                    assert!(
                        Instant::now() < deadline,
                        "the host stopped reading {viewer:?}"
                    );
                    self.pass();
                }
                Err(error) => panic!("viewer {viewer:?}: {error}"),
            }
        }
    }

    fn request(&mut self, viewer: V, mut value: Value) -> u64 {
        self[viewer].ids += 1;
        let id = self[viewer].ids;
        value["id"] = json!(id);
        self.send(viewer, 0x11, value.to_string().as_bytes());
        id
    }

    /// A request from the execution itself, which presents its control key.
    fn control(&mut self, viewer: V, mut value: Value) {
        value["key"] = json!(KEY);
        self.request(viewer, value);
    }

    fn input(&mut self, viewer: V, text: &str) {
        self.send(viewer, 0x12, text.as_bytes());
    }

    /// The connection is lost mid-stream: no leave.
    fn lose(&mut self, viewer: V) {
        if let Some(socket) = self[viewer].socket.take() {
            // A host that already closed its end leaves nothing to shut down.
            let _ = socket.shutdown(std::net::Shutdown::Both);
        }
    }

    /// The next item, or none once the host closed the connection.
    #[track_caller]
    fn next(&mut self, viewer: V) -> Option<Item> {
        let deadline = Instant::now() + PATIENCE;
        loop {
            if let Some(item) = self[viewer].decode() {
                return Some(item);
            }
            if self[viewer].closed || self[viewer].socket.is_none() {
                return None;
            }
            assert!(Instant::now() < deadline, "viewer {viewer:?} waited");
            self.pass();
            if self[viewer].buffer.len() < 5 {
                std::thread::sleep(Duration::from_micros(200));
            }
        }
    }

    /// Items up to the first that satisfies `done` over everything so far.
    #[track_caller]
    fn until(&mut self, viewer: V, done: impl Fn(&[Item]) -> bool) -> Vec<Item> {
        let mut items = Vec::new();
        loop {
            let item = self
                .next(viewer)
                .unwrap_or_else(|| panic!("host closed first: {items:?}"));
            items.push(item);
            if done(&items) {
                return items;
            }
        }
    }

    #[track_caller]
    fn until_reply(&mut self, viewer: V) -> Value {
        let items = self.until(viewer, |items| matches!(items.last(), Some(Item::Reply(_))));
        let Some(Item::Reply(reply)) = items.last() else {
            unreachable!()
        };
        reply.clone()
    }

    #[track_caller]
    fn until_event(&mut self, viewer: V, kind: &str) -> Vec<Item> {
        self.until(
            viewer,
            |items| matches!(items.last(), Some(Item::Event(event)) if event["type"] == kind),
        )
    }

    #[track_caller]
    fn until_out(&mut self, viewer: V, text: &str) -> Vec<Item> {
        self.until(viewer, |items| {
            String::from_utf8_lossy(&out(items)).contains(text)
        })
    }

    /// Items up to the owner event naming `id`.
    #[track_caller]
    fn until_owner(&mut self, viewer: V, id: &str) -> Vec<Item> {
        self.until(viewer, |items| {
            matches!(items.last(), Some(Item::Event(event))
                if event["type"] == "owner" && event["viewer"] == id)
        })
    }

    /// Everything this viewer receives until the host closes it.
    #[track_caller]
    fn rest(&mut self, viewer: V) -> Vec<Item> {
        std::iter::from_fn(|| self.next(viewer)).collect()
    }

    /// The host's record for this viewer, by its public id.
    fn hosted(&mut self, viewer: V) -> &mut super::Viewer {
        let id = self[viewer].id.clone();
        self.host
            .viewers
            .iter_mut()
            .find(|hosted| hosted.id == id)
            .expect("the host keeps this viewer")
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

fn dark() -> Value {
    json!({"background":"#111111","foreground":"#eeeeee"})
}

fn light() -> Value {
    json!({"background":"#eeeeee","foreground":"#111111"})
}

// ---------------------------------------------------------------------------
// Attachment and screens

#[test]
fn a_fresh_viewer_starts_with_the_screen_at_its_own_size() {
    let mut bench = Bench::terminal();
    let first = bench.viewer(true);
    for line in 1..=20 {
        bench.print(format!("line {line}: abcdefghijklmnopqrstuvwxyz0123456789\r\n").as_bytes());
    }
    bench.print(b"ready> ");
    let mut expected = crate::ghostty_vt::Terminal::new(80, 24).unwrap();
    for item in bench.until_out(first, "ready> ") {
        if let Item::Snapshot(bytes) | Item::Out(bytes) = item {
            expected.write(&bytes);
        }
    }
    expected.resize(40, 12).unwrap();
    let second = bench.attach(json!({
        "kind":"attach", "control":true, "replace":true, "own":"take", "rows":12, "cols":40,
    }));
    let Some(Item::Snapshot(snapshot)) = bench.next(second) else {
        panic!("a fresh viewer starts with its screen");
    };
    let mut display = crate::ghostty_vt::Terminal::new(40, 12).unwrap();
    display.write(&snapshot);
    assert_eq!(display.text().unwrap(), expected.text().unwrap());
    assert_eq!(display.cursor(), expected.cursor());
    assert_eq!(bench.size(), (12, 40));
}

#[test]
fn a_busy_claim_leaves_the_controller_typing() {
    let mut bench = Bench::terminal();
    let controller = bench.viewer(true);
    let waiting = bench.viewer(false);
    bench.request(
        waiting,
        json!({"kind":"attach","control":true,"replace":false,"rows":24,"cols":80}),
    );
    assert_eq!(bench.until_reply(waiting)["kind"], "busy");
    bench.input(waiting, "passive\n");
    bench.input(controller, "original-controller\n");
    assert_eq!(
        bench.typed("original-controller\n"),
        "original-controller\n"
    );
}

/// The waiting viewer precedes the controller in the host's list, and the
/// controller's end of stream arrives in the same pass as the claim.
#[test]
fn a_claim_after_the_controller_disconnects_takes_control() {
    let mut bench = Bench::terminal();
    let waiting = bench.viewer(false);
    let controller = bench.viewer(true);
    let token = bench[controller].token.clone();
    bench.lose(controller);
    bench.request(
        waiting,
        json!({"kind":"attach","control":true,"replace":false,"rows":24,"cols":80}),
    );
    bench.attached(waiting);
    assert!(bench[waiting].token.is_some());
    assert_ne!(bench[waiting].token, token);
    bench.input(waiting, "reconnected\n");
    assert_eq!(bench.typed("reconnected\n"), "reconnected\n");
}

#[test]
fn every_replaced_controller_gets_a_notice_during_concurrent_takeovers() {
    let mut bench = Bench::terminal();
    let first = bench.viewer(true);
    let second = bench.viewer(false);
    let third = bench.viewer(false);
    // Both takeovers reach the host in one pass.
    for viewer in [second, third] {
        bench.request(
            viewer,
            json!({"kind":"attach","control":true,"replace":true,
                "rows":24,"cols":80,"moved":"moved elsewhere"}),
        );
    }
    bench.pass();
    assert_eq!(bench.until_reply(third)["kind"], "attached");
    bench.request(first, json!({"kind":"controlled"}));
    bench.until_reply(second);
    bench.request(second, json!({"kind":"controlled"}));
    for viewer in [first, second] {
        let items = bench.until(viewer, |items| matches!(items.last(), Some(Item::Reply(_))));
        let notices = events(&items, "control");
        assert_eq!(notices.len(), 1, "{items:?}");
        assert_eq!(notices[0]["held"], false);
        assert_eq!(notices[0]["notice"], "moved elsewhere");
    }
}

/// A replaced controller learns where the conversation went and who sizes the
/// terminal; passive viewers cannot type; zero viewers end nothing.
#[test]
fn replaced_and_passive_viewers_cannot_type_and_zero_viewers_end_nothing() {
    let mut bench = Bench::terminal();
    bench.print(b"pending-request-17\r\n");
    let first = bench.viewer(true);
    let second = bench.attach(json!({"kind":"attach","control":true,"replace":true,
        "own":"take","rows":24,"cols":80,
        "moved":"This conversation moved to tmux %4: lf resume s1"}));
    let moved = bench.until_event(first, "control");
    let Some(Item::Event(control)) = moved.last() else {
        unreachable!()
    };
    assert_eq!(control["held"], false);
    assert_eq!(
        control["notice"],
        "This conversation moved to tmux %4: lf resume s1"
    );
    let second_id = bench[second].id.clone();
    assert!(
        events(&moved, "owner")
            .iter()
            .any(|event| event["viewer"] == second_id && event["held"] == false),
        "{moved:?}"
    );
    bench.input(first, "replaced\n");
    let observer = bench.viewer(false);
    assert!(bench[observer].token.is_none());
    bench.input(observer, "passive\n");
    for viewer in [observer, first, second] {
        bench.request(viewer, json!({"kind":"leave"}));
    }
    bench.pass_until("zero viewers", |bench| bench.host.viewers.is_empty());
    let replacement = bench.viewer(true);
    let passive = bench.viewer(false);
    bench.request(
        passive,
        json!({"kind":"attach","control":false,"replace":true,"rows":24,"cols":80}),
    );
    assert_eq!(bench.until_reply(passive)["kind"], "attached");
    // An observer's disconnect cannot release the controller's claim.
    bench.lose(passive);
    bench.input(replacement, "accepted\n");
    assert_eq!(bench.typed("accepted\n"), "accepted\n");
    assert!(bench.ended.is_none());
}

// ---------------------------------------------------------------------------
// Ownership: size and colors

#[test]
fn resize_reaches_every_viewer_as_its_own_event_and_program_bytes_do_not() {
    let mut bench = Bench::terminal();
    let sizer = bench.viewer(true);
    let observer = bench.viewer(false);
    bench.until_event(observer, "owner");
    bench.request(sizer, json!({"kind":"resize","rows":30,"cols":100}));
    let items = bench.until_event(observer, "resize");
    assert_eq!(events(&items, "resize").last().unwrap()["cols"], 100);
    assert_eq!(bench.size(), (30, 100));
    // An event under another nonce is program output.
    bench.print(b"\x1b_lf1;deadbeef;{\"seq\":9,\"type\":\"resize\",\"rows\":1,\"cols\":1}\x1b\\");
    let items = bench.until_out(observer, "deadbeef");
    assert!(events(&items, "resize").is_empty(), "{items:?}");
    // Event sequence numbers are gapless per viewer and replies carry the latest.
    bench.print(b"mark\r\n");
    let items = bench.until_out(sizer, "mark");
    let all: Vec<u64> = items
        .iter()
        .filter_map(|item| match item {
            Item::Event(event) => event["seq"].as_u64(),
            _ => None,
        })
        .collect();
    assert!(all.windows(2).all(|pair| pair[1] == pair[0] + 1), "{all:?}");
    bench.request(sizer, json!({"kind":"controlled"}));
    let reply = bench.until_reply(sizer);
    assert_eq!(reply["kind"], "controlled");
    assert!(reply["seq"].as_u64().unwrap() >= *all.last().unwrap());
}

#[test]
fn ownership_queues_behind_a_taker_and_tells_the_loser_who_took_it() {
    let mut bench = Bench::terminal();
    let typist = bench.viewer(true);
    bench.request(typist, json!({"kind":"theme","theme":dark()}));
    bench.until_event(typist, "theme");
    let waiting = bench.attach(json!({"kind":"attach","control":false,"replace":true,
        "own":"request","rows":30,"cols":90,"name":"pane 2"}));
    let items = bench.until_event(waiting, "owner");
    assert_eq!(events(&items, "theme")[0]["theme"], dark());
    bench.request(waiting, json!({"kind":"theme","theme":light()}));
    bench.request(waiting, json!({"kind":"controlled"}));
    bench.until_reply(waiting);
    assert_eq!(bench.size(), (24, 80));
    bench.print(b"mark\r\n");
    for viewer in [waiting, typist] {
        let items = bench.until_out(viewer, "mark");
        assert!(events(&items, "theme").is_empty(), "{items:?}");
    }
    let taker = bench.attach(json!({"kind":"attach","control":false,"replace":true,
        "own":"take","rows":40,"cols":120,"name":"tmux %4"}));
    let items = bench.until_event(taker, "owner");
    assert!(events(&items, "theme").is_empty(), "{items:?}");
    // An owner without a color report supplies no override, even to new observers.
    let observer = bench.viewer(false);
    let items = bench.until_event(observer, "owner");
    assert!(events(&items, "theme").is_empty(), "{items:?}");
    let taker_id = bench[taker].id.clone();
    let items = bench.until_owner(typist, &taker_id);
    assert!(events(&items, "theme").is_empty(), "{items:?}");
    assert!(
        events(&items, "owner")
            .iter()
            .any(|event| event["viewer"] == taker_id
                && event["name"] == "tmux %4"
                && event["held"] == false),
        "{items:?}"
    );
    assert_eq!(bench.size(), (40, 120));
    // The taker leaves: the queued request supplies size and colors to everyone.
    bench.request(taker, json!({"kind":"leave"}));
    let waiting_id = bench[waiting].id.clone();
    let items = bench.until_owner(waiting, &waiting_id);
    assert_eq!(events(&items, "owner").last().unwrap()["held"], true);
    assert_eq!(events(&items, "theme")[0]["theme"], light());
    let seq = events(&items, "owner").last().unwrap()["seq"].clone();
    bench.request(waiting, json!({"kind":"controlled"}));
    assert_eq!(bench.until_reply(waiting)["seq"], seq);
    for viewer in [typist, observer] {
        let items = bench.until_owner(viewer, &waiting_id);
        assert_eq!(events(&items, "theme")[0]["theme"], light());
    }
    let fresh = bench.viewer(false);
    let items = bench.until_event(fresh, "owner");
    assert_eq!(events(&items, "theme")[0]["theme"], light());
    assert_eq!(bench.size(), (30, 90));
}

// ---------------------------------------------------------------------------
// Resume

#[test]
fn a_dropped_viewer_resumes_from_its_byte_count_without_a_snapshot() {
    let mut bench = Bench::terminal();
    bench.print(b"drawn\r\n");
    let first = bench.viewer(true);
    bench.until_event(first, "owner");
    bench.request(first, json!({"kind":"theme","theme":dark()}));
    let initial = bench.until_event(first, "theme");
    let last_seq = events(&initial, "theme")[0]["seq"].as_u64().unwrap();
    bench.print(b"line one\r\n");
    bench.until_out(first, "line one\r\n");
    bench.lose(first);
    let second = bench.viewer(true);
    bench.request(second, json!({"kind":"theme","theme":light()}));
    bench.until_event(second, "theme");
    bench.print(b"line two\r\n");
    bench.until_out(second, "line two\r\n");
    // Three bytes short of what it had: the stream resumes with exactly them.
    let resumed = bench.connect();
    let mut resume = bench.resume(first, None, 24, 80);
    resume["received"] = json!(bench[first].received - 3);
    bench.request(resumed, resume);
    let reply = bench.attached(resumed);
    let items = bench.until_out(resumed, "line two\r\n");
    assert!(
        items.iter().all(|item| !matches!(item, Item::Snapshot(_))),
        "{items:?}"
    );
    assert_eq!(out(&items), b"e\r\nline two\r\n", "{items:?}");
    assert_eq!(events(&items, "theme").len(), 1, "{items:?}");
    assert_eq!(events(&items, "theme")[0]["theme"], light());
    let mut seq = last_seq;
    for item in &items {
        if let Item::Event(event) = item {
            seq += 1;
            assert_eq!(event["seq"], seq);
        }
    }
    assert_eq!(reply["seq"], seq);
    bench.print(b"line three\r\n");
    for viewer in [resumed, second] {
        let items = bench.until_out(viewer, "line three\r\n");
        assert!(events(&items, "theme").is_empty(), "{items:?}");
    }
    // Resuming did not restore the old colors; explicitly taking ownership does.
    bench.request(resumed, json!({"kind":"own","mode":"take"}));
    let resumed_id = bench[resumed].id.clone();
    for viewer in [resumed, second] {
        let items = bench.until_owner(viewer, &resumed_id);
        assert_eq!(events(&items, "theme").last().unwrap()["theme"], dark());
    }
    bench.request(resumed, json!({"kind":"controlled"}));
    assert_eq!(bench.until_reply(resumed)["seq"], seq + 3); // resize, theme, owner
}

#[test]
fn a_resume_in_the_same_pass_as_its_loss_reclaims_input_and_size() {
    let mut bench = Bench::terminal();
    let first = bench.viewer(true);
    bench.until_event(first, "owner");
    let resumed = bench.connect();
    bench.lose(first);
    bench.request(resumed, bench.resume(first, Some("take"), 35, 110));
    let reply = bench.attached(resumed);
    assert!(
        !reply["token"].is_null(),
        "resume did not regain input: {reply}"
    );
    let items = bench.until_event(resumed, "owner");
    assert_eq!(events(&items, "resize")[0]["rows"], 35);
    assert_eq!(bench.size(), (35, 110));
    bench.input(resumed, "typed\n");
    bench.typed("typed\n");
}

#[test]
fn resume_supersedes_its_open_socket_without_taking_a_newer_viewers_claims() {
    for own in ["request", "take"] {
        let mut bench = Bench::terminal();
        let first = bench.viewer(true);
        bench.until_event(first, "owner");
        let back = bench.connect();
        bench.request(back, bench.resume(first, Some(own), 35, 110));
        let reply = bench.attached(back);
        assert_eq!(reply["viewer"], bench[first].id);
        assert!(!reply["token"].is_null());
        assert_eq!(bench.size(), (35, 110), "{own}");
        // Closing the superseded socket cannot release the replacement's claims.
        bench.lose(first);
        bench.input(back, "one\n");
        bench.typed("one\n");
        let newer = bench.viewer(true);
        bench.until_event(newer, "owner");
        bench.request(newer, json!({"kind":"resize","rows":40,"cols":120}));
        bench.until_event(newer, "resize");
        assert_eq!(bench.size(), (40, 120), "{own}");
        let again = bench.connect();
        bench.request(again, bench.resume(back, Some(own), 20, 70));
        let reply = bench.attached(again);
        assert!(reply["token"].is_null(), "{reply}");
        assert_eq!(reply["owner"], bench[newer].id);
        bench.lose(back);
        bench.input(newer, "two\n");
        bench.typed("two\n");
        assert_eq!(bench.size(), (40, 120), "{own}");
    }
}

#[test]
fn a_resume_past_the_spool_is_refused_and_a_fresh_attach_repaints() {
    let mut bench = Bench::terminal();
    let first = bench.viewer(true);
    bench.until_event(first, "owner");
    bench.lose(first);
    let second = bench.viewer(true);
    let line = [b'x'; 1023];
    for _ in 0..=SPOOL / 1024 {
        bench.print(&line);
        bench.print(b"\n");
    }
    bench.until_out(second, "x\n");
    let back = bench.connect();
    bench.request(back, bench.resume(first, None, 24, 80));
    assert_eq!(bench.until_reply(back)["kind"], "reconnect_required");
    bench.request(
        back,
        json!({"kind":"attach","control":false,"replace":true,"own":null,"rows":24,"cols":80}),
    );
    bench.attached(back);
    assert!(matches!(bench.next(back), Some(Item::Snapshot(_))));
}

// ---------------------------------------------------------------------------
// Liveness: heartbeats and viewer holds

/// Connected viewers hold the execution; a lost connection starts absence at
/// once, while its stream waits for a resume.
#[test]
fn viewer_holds_follow_connected_viewers_and_a_lost_connection_starts_absence() {
    let mut bench = Bench::terminal();
    bench.pass();
    assert_eq!(bench.host.observed_hold, None);
    let controller = bench.viewer(true);
    let passive = bench.viewer(false);
    assert!(matches!(
        bench.host.observed_hold,
        Some(ViewerHold::Viewed(_))
    ));
    bench.request(controller, json!({"kind":"leave"}));
    bench.pass();
    assert!(matches!(
        bench.host.observed_hold,
        Some(ViewerHold::Viewed(_))
    ));
    bench.lose(passive);
    bench.pass_until("absence", |bench| {
        matches!(bench.host.observed_hold, Some(ViewerHold::Absent(_)))
    });
    let resumed = bench.connect();
    bench.request(resumed, bench.resume(passive, None, 24, 80));
    bench.attached(resumed);
    assert!(matches!(
        bench.host.observed_hold,
        Some(ViewerHold::Viewed(_))
    ));
    bench.request(resumed, json!({"kind":"leave"}));
    bench.pass_until("absence", |bench| {
        matches!(bench.host.observed_hold, Some(ViewerHold::Absent(_)))
    });
}

#[test]
fn heartbeat_expiry_drops_only_the_silent_viewer_while_output_continues() {
    let mut bench = Bench::terminal();
    let lost = bench.viewer(true);
    let passive = bench.viewer(false);
    bench.print(b"tick\r\n");
    bench.until_out(lost, "tick");
    // The passive viewer's heartbeat refreshes it; the controller stays silent.
    bench.hosted(lost).heartbeat_at -= HEARTBEAT_TIMEOUT;
    bench.hosted(passive).heartbeat_at -= HEARTBEAT_TIMEOUT - Duration::from_secs(1);
    bench.request(passive, json!({"kind":"controlled"}));
    bench.print(b"tock\r\n");
    bench.until_out(passive, "tock");
    let rest = bench.rest(lost);
    assert!(
        !String::from_utf8_lossy(&out(&rest)).contains("tock"),
        "{rest:?}"
    );
    bench.print(b"after\r\n");
    bench.until_out(passive, "after");
    assert!(bench.ended.is_none());
    assert!(matches!(
        bench.host.observed_hold,
        Some(ViewerHold::Viewed(_))
    ));
    // The expired controller cannot prevent a new connection claiming input.
    let replacement = bench.attach(json!({"kind":"attach","control":true,
        "replace":false,"own":"take","rows":31,"cols":97}));
    assert!(bench[replacement].token.is_some());
    bench.request(replacement, json!({"kind":"leave"}));
    bench.request(passive, json!({"kind":"leave"}));
    bench.pass_until("absence", |bench| {
        matches!(bench.host.observed_hold, Some(ViewerHold::Absent(_)))
    });
    // Replay grace and the unchanged execution still permit a reconnect.
    let resumed = bench.connect();
    bench.request(resumed, bench.resume(lost, Some("take"), 24, 80));
    bench.attached(resumed);
    let items = bench.until_out(resumed, "after");
    assert!(
        String::from_utf8_lossy(&out(&items)).contains("tock"),
        "{items:?}"
    );
    assert!(matches!(
        bench.host.observed_hold,
        Some(ViewerHold::Viewed(_))
    ));
}

// ---------------------------------------------------------------------------
// Input and output flow

/// Keystrokes sent just before a viewer closes reach the program, even when
/// the host writes to the closed viewer before reading them.
#[test]
fn input_sent_before_a_viewer_closes_reaches_the_program() {
    let mut bench = Bench::pipes();
    let attach = json!({"kind":"attach","control":true,"replace":true,
        "own":"take","rows":24,"cols":80});
    let viewer = bench.connect();
    bench.request(viewer, attach.clone());
    let input: Vec<u8> = (0..20000).map(|i| b'a' + (i % 26) as u8).collect();
    for chunk in input.chunks(1000) {
        bench.input(viewer, std::str::from_utf8(chunk).unwrap());
    }
    bench[viewer].socket = None;
    bench.pass_until("all input", |bench| {
        bench.program.typed.len() >= input.len()
    });
    assert_eq!(bench.program.typed, input);

    drop(bench);

    // Attaching, typing and closing at once, while the program keeps
    // printing, loses nothing.
    let mut bench = Bench::pipes();
    let rounds: Vec<String> = (0..50).map(|round| format!("round {round:02}\n")).collect();
    for round in &rounds {
        let viewer = bench.connect();
        bench.request(viewer, attach.clone());
        bench.input(viewer, round);
        bench[viewer].socket = None;
        bench.print(b"tick\n");
    }
    let expected = rounds.concat();
    bench.pass_until("every round", |bench| {
        bench.program.typed.len() >= expected.len()
    });
    assert_eq!(String::from_utf8_lossy(&bench.program.typed), expected);
}

/// Input and control replies never wait on SQLite: a host holding the
/// store's write lock elsewhere relays at once and replays nothing.
#[test]
fn a_locked_store_neither_delays_nor_replays_input() {
    let mut bench = Bench::terminal();
    let viewer = bench.viewer(true);
    let database = bench.ledger.home().join("loopflow.db");
    crate::journal::process_ledger().unwrap();
    let sql = rusqlite::Connection::open(database).unwrap();
    sql.execute_batch("BEGIN IMMEDIATE").unwrap();
    let started = Instant::now();
    bench.input(viewer, "retained-draft\n");
    bench.request(viewer, json!({"kind":"controlled"}));
    assert_eq!(bench.until_reply(viewer)["held"], true);
    bench.typed("retained-draft\n");
    bench.request(viewer, json!({"kind":"leave"}));
    let returned = bench.viewer(true);
    bench.request(returned, json!({"kind":"controlled"}));
    assert_eq!(bench.until_reply(returned)["held"], true);
    assert!(
        started.elapsed() < Duration::from_secs(1),
        "{:?}",
        started.elapsed()
    );
    sql.execute_batch("ROLLBACK").unwrap();
    bench.input(returned, "continued\n");
    assert_eq!(bench.typed("continued\n"), "retained-draft\ncontinued\n");
}

#[test]
fn a_slow_observer_is_cut_at_the_limit_while_output_and_exit_continue() {
    let mut bench = Bench::pipes();
    let slow = bench.viewer(false);
    bench[slow].reading = false;
    let active = bench.viewer(true);
    let block = vec![0; 65536];
    let total = LIMIT + 4 * block.len();
    let mut seen = 0;
    while seen < total {
        // Paced by the active viewer, as a program waiting on its reader is.
        bench.print(&block);
        let items = bench.until(active, |items| out(items).len() >= block.len());
        seen += out(&items).len();
    }
    bench.write(2, b"final");
    bench.exit(0);
    let items = bench.rest(active);
    assert_eq!(events(&items, "child_exited")[0]["code"], 0, "{items:?}");
    let stderr: Vec<u8> = items
        .iter()
        .filter_map(|item| match item {
            Item::Err(bytes) => Some(bytes.clone()),
            _ => None,
        })
        .flatten()
        .collect();
    assert_eq!(stderr, b"final");
    bench[slow].reading = true;
    let retained = bench.rest(slow);
    assert!(
        out(&retained).len() < total,
        "the slow viewer was not disconnected"
    );
}

#[test]
fn a_megabyte_burst_costs_within_ten_percent_on_the_wire() {
    let mut bench = Bench::pipes();
    let viewer = bench.viewer(true);
    let before = bench[viewer].wire;
    bench.print(&vec![0; 1024 * 1024]);
    bench.exit(0);
    let items = bench.rest(viewer);
    assert_eq!(out(&items).len(), 1024 * 1024);
    let wire = bench[viewer].wire - before;
    assert!(wire < 1024 * 1024 * 11 / 10, "{wire} bytes on the wire");
}

// ---------------------------------------------------------------------------
// The execution's own requests

#[test]
fn input_hints_reach_only_controlling_viewers_and_outlive_a_detach() {
    let mut bench = Bench::pipes();
    // The step registers its controls on a connection it keeps open.
    let step = bench.connect();
    bench.control(step, json!({"kind":"input_hint","hint":"type here"}));
    bench.print(b"working\n");
    let controller = bench.viewer(true);
    let items = bench.until_event(controller, "hint");
    assert_eq!(events(&items, "hint")[0]["text"], "type here");
    // Detaching viewers keeps the step's registration for the next one.
    let detaching = bench.connect();
    bench.control(detaching, json!({"kind":"detach_viewers"}));
    bench.until_event(controller, "detached");
    let returning = bench.viewer(true);
    bench.until_event(returning, "hint");
    let observer = bench.viewer(false);
    bench.exit(0);
    let items = bench.rest(observer);
    assert!(events(&items, "hint").is_empty(), "{items:?}");
    assert_eq!(events(&items, "child_exited")[0]["code"], 0);
    assert_eq!(out(&items), b"working\n");
}

#[test]
fn reported_directories_reach_current_and_later_viewers() {
    let mut bench = Bench::terminal();
    let early = bench.viewer(true);
    let reporter = bench.connect();
    bench.control(reporter, json!({"kind":"directory","cwd":"/tmp/reported"}));
    let items = bench.until_event(early, "directory");
    assert_eq!(events(&items, "directory")[0]["cwd"], "/tmp/reported");
    let later = bench.viewer(false);
    let items = bench.until_event(later, "directory");
    assert_eq!(events(&items, "directory")[0]["cwd"], "/tmp/reported");
}

/// A controller that acknowledged the native client's Return line is ready;
/// a stale or passive acknowledgement is not.
#[test]
fn readiness_follows_the_controller_acknowledging_return() {
    let mut bench = Bench::terminal();
    let controller = bench.viewer(true);
    let observer = bench.viewer(false);
    bench.request(observer, json!({"kind":"controlled"}));
    assert_eq!(bench.until_reply(observer)["ready"], false);
    let launcher = bench.connect();
    bench.control(
        launcher,
        json!({"kind":"native_started","exit":"exited","detach":"lf resume s1"}),
    );
    let items = bench.until_event(controller, "return");
    assert_eq!(events(&items, "return")[0]["detach"], "lf resume s1");
    bench.request(
        observer,
        json!({"kind":"controlled","ready":"lf resume s1"}),
    );
    bench.request(
        controller,
        json!({"kind":"controlled","ready":"lf resume s0"}),
    );
    bench.until_reply(controller);
    bench.until_reply(observer);
    bench.request(observer, json!({"kind":"controlled"}));
    assert_eq!(bench.until_reply(observer)["ready"], false);
    bench.request(
        controller,
        json!({"kind":"controlled","ready":"lf resume s1"}),
    );
    bench.until_reply(controller);
    bench.request(observer, json!({"kind":"controlled"}));
    let reply = bench.until_reply(observer);
    assert_eq!(reply["held"], true);
    assert_eq!(reply["ready"], true);
}

#[test]
fn grace_expiry_refuses_bytes_before_repainting_the_terminal_screen() {
    let mut bench = Bench::terminal();
    let expired = bench.viewer(false);
    bench.print(b"stale output");
    bench.until_out(expired, "stale output");
    bench.lose(expired);
    bench.pass_until("the lost connection", |bench| {
        bench.hosted(expired).connection.is_dropped()
    });
    let newer = bench.viewer(true);
    bench.print(b"after outage");
    bench.until_out(newer, "after outage");
    bench.hosted(expired).connection = Connection::Dropped(Instant::now() - GRACE);

    let returning = bench.connect();
    let mut resume = bench.resume(expired, None, 24, 80);
    resume["received"] = json!(0);
    bench.request(returning, resume);
    let Some(Item::Reply(refused)) = bench.next(returning) else {
        panic!("refusal must precede output");
    };
    assert_eq!(refused["kind"], "reconnect_required");
    bench.request(
        returning,
        json!({"kind":"attach","control":false,"replace":false,"own":null,"rows":24,"cols":80}),
    );
    let Some(Item::Reply(attached)) = bench.next(returning) else {
        panic!("attachment answers first");
    };
    assert_eq!(attached["owner"], bench[newer].id);
    let Some(Item::Snapshot(snapshot)) = bench.next(returning) else {
        panic!("a fresh attachment repaints");
    };
    let mut screen = crate::ghostty_vt::Terminal::new(80, 24).unwrap();
    screen.write(&snapshot);
    assert!(screen.text().unwrap().contains("after outage"));
    assert_eq!(bench.host.control, bench[newer].token);
}
