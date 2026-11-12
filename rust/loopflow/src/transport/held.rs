//! The server's end of one terminal a holder keeps: its output from a PTY
//! position, its input, its size and its exit. A lost holder connection says
//! nothing about the agent; this reads again from the position it reached.

use std::collections::VecDeque;
use std::io::{self, Read, Write};
use std::os::fd::{AsRawFd, RawFd};
use std::os::unix::net::UnixStream;
use std::os::unix::process::ExitStatusExt;
use std::process::ExitStatus;
use std::time::{Duration, Instant};

use crate::id::TerminalId;
use crate::server::holder::client::Client;
use crate::server::holder::wire::{self, Decoder, Exit, Frame, Request};
use crate::server::holder::Place;

use super::hosted::Chunk;
use super::input::PendingInput;

/// The first wait before asking a holder again; it doubles to `RETRY_LIMIT`.
const RETRY: Duration = Duration::from_millis(100);
const RETRY_LIMIT: Duration = Duration::from_secs(2);
/// How long the registry's loop may wait on one holder to open a connection.
const CONNECT: Duration = Duration::from_millis(500);

/// How a held terminal's child ended.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) enum Ended {
    Exited(Exit),
    /// The holder is gone, or no longer holds the terminal, and left no
    /// `exit.json`: the child is not running and its status was never seen.
    Unknown,
}

impl Ended {
    /// The wait status settlement reads; an unknown end reads as SIGKILL,
    /// which is what a holder's death delivers.
    pub fn status(&self) -> ExitStatus {
        match self {
            Self::Exited(exit) => match (exit.code, exit.signal) {
                (Some(code), _) => ExitStatus::from_raw((code & 0xff) << 8),
                (None, signal) => ExitStatus::from_raw(signal.unwrap_or(libc::SIGKILL)),
            },
            Self::Unknown => ExitStatus::from_raw(libc::SIGKILL),
        }
    }
}

pub(super) struct Source {
    place: Place,
    terminal: TerminalId,
    pid: u32,
    /// The next PTY byte not yet read.
    at: u64,
    output: Option<(UnixStream, Decoder)>,
    input: Option<UnixStream>,
    /// Requests that need no answer read: resize and restore.
    requests: Option<UnixStream>,
    unsent: VecDeque<u8>,
    raw: bool,
    /// Where the stream resumed after the ring passed `at`.
    gap: Option<u64>,
    ended: Option<Ended>,
    reported: bool,
    retry_at: Option<Instant>,
    retry: Duration,
}

impl Source {
    /// Read `terminal`, whose child is `pid`, from byte `at`.
    pub fn new(place: Place, terminal: TerminalId, pid: u32, at: u64) -> Self {
        Self {
            place,
            terminal,
            pid,
            at,
            output: None,
            input: None,
            requests: None,
            unsent: VecDeque::new(),
            raw: false,
            gap: None,
            ended: None,
            reported: false,
            retry_at: None,
            retry: RETRY,
        }
    }

    pub fn pid(&self) -> u32 {
        self.pid
    }

    /// The PTY position everything returned by `read` so far ends at.
    pub fn position(&self) -> u64 {
        self.at
    }

    pub fn is_raw(&self) -> bool {
        self.raw
    }

    /// Whether the child has ended and all its output was read.
    pub fn drained(&self) -> bool {
        self.ended.is_some()
    }

    /// Whether the child ended without anyone seeing its status.
    pub fn outcome_unknown(&self) -> bool {
        self.ended == Some(Ended::Unknown)
    }

    /// The child's end, reported once.
    pub fn exited(&mut self) -> Option<Ended> {
        if self.reported {
            return None;
        }
        let ended = self.ended.clone()?;
        self.reported = true;
        Some(ended)
    }

    /// Where output resumed after bytes this server never read were dropped
    /// from the ring; the screen built so far is missing them.
    pub fn take_gap(&mut self) -> Option<u64> {
        self.gap.take()
    }

    /// When a lost connection is next tried.
    pub fn retry_at(&self) -> Option<Instant> {
        self.retry_at.filter(|_| self.ended.is_none())
    }

    fn may_connect(&self) -> bool {
        self.ended.is_none() && self.retry_at.is_none_or(|at| at <= Instant::now())
    }

    fn lost(&mut self) {
        self.retry_at = Some(Instant::now() + self.retry);
        self.retry = (self.retry * 2).min(RETRY_LIMIT);
    }

    /// Output that is ready, at most `passes` reads of it.
    pub fn read(&mut self, passes: usize) -> Vec<Chunk> {
        let mut chunks = Vec::new();
        if self.output.is_none() && self.may_connect() {
            self.open_output();
        }
        self.drain_answers();
        self.flush_requests();
        // Frames that arrived with the holder's answer are already buffered.
        self.decode(&mut chunks);
        for _ in 0..passes {
            let Some((socket, decoder)) = self.output.as_mut() else {
                break;
            };
            let mut bytes = [0; 16384];
            match socket.read(&mut bytes) {
                Ok(0) => {
                    // No exit frame: the connection ended, not the child.
                    self.output = None;
                    self.lost();
                    break;
                }
                Ok(n) => decoder.extend(&bytes[..n]),
                Err(e) if e.kind() == io::ErrorKind::WouldBlock => break,
                Err(e) if e.kind() == io::ErrorKind::Interrupted => continue,
                Err(_) => {
                    self.output = None;
                    self.lost();
                    break;
                }
            }
            self.decode(&mut chunks);
        }
        chunks
    }

    /// Take every whole frame received so far.
    fn decode(&mut self, chunks: &mut Vec<Chunk>) {
        loop {
            let frame = match self.output.as_mut().map(|(_, decoder)| decoder.frame()) {
                Some(Ok(Some(frame))) => frame,
                Some(Ok(None)) | None => return,
                Some(Err(error)) => {
                    tracing::warn!(%error, terminal = %self.terminal, "holder stream unreadable; reading again");
                    self.output = None;
                    self.lost();
                    return;
                }
            };
            self.retry = RETRY;
            match frame {
                Frame::Out(bytes) => {
                    self.at += bytes.len() as u64;
                    chunks.push(Chunk {
                        stream: 1,
                        bytes,
                        raw: self.raw,
                        end: Some(self.at),
                    });
                }
                Frame::Mode(mode) => self.raw = mode.raw,
                Frame::Gap(gap) => {
                    self.at = gap.ring_start;
                    self.gap = Some(gap.ring_start);
                }
                Frame::Exit(exit) => {
                    self.ended = Some(Ended::Exited(exit));
                    self.output = None;
                }
            }
        }
    }

    fn open_output(&mut self) {
        let client = match Client::connect_within(&self.place, CONNECT) {
            Ok(client) => client,
            Err(_) => return self.unreachable(),
        };
        match client.read(self.terminal.as_str(), self.at) {
            Ok(output) => {
                if output.socket.set_nonblocking(true).is_ok() {
                    self.output = Some((output.socket, output.decoder));
                    self.retry_at = None;
                } else {
                    self.lost();
                }
            }
            // The holder answered and does not hold this terminal.
            Err(error) if error.downcast_ref::<io::Error>().is_none() => {
                tracing::warn!(%error, terminal = %self.terminal, "holder no longer holds the terminal");
                self.settle_without_holder();
            }
            Err(_) => self.lost(),
        }
    }

    /// No holder is listening. One that is alive is waited for; otherwise the
    /// holder is gone, and its children with it.
    fn unreachable(&mut self) {
        if self.place.claimed() {
            self.lost();
        } else {
            self.settle_without_holder();
        }
    }

    fn settle_without_holder(&mut self) {
        self.ended = Some(
            match crate::server::holder::recorded_exit(&self.place, self.terminal.as_str()) {
                Ok(Some(exit)) => Ended::Exited(exit),
                Ok(None) => Ended::Unknown,
                Err(error) => {
                    tracing::warn!(%error, terminal = %self.terminal, "exit receipt unreadable");
                    Ended::Unknown
                }
            },
        );
        self.output = None;
    }

    /// Move queued input toward the child. A lost connection keeps the bytes.
    pub fn forward(&mut self, pending: &mut PendingInput) {
        if self.input.is_none() && self.may_connect() && !pending.is_empty() {
            let opened = Client::connect_within(&self.place, CONNECT)
                .map_err(anyhow::Error::from)
                .and_then(|client| client.write(self.terminal.as_str()))
                .and_then(|socket| Ok(socket.set_nonblocking(true).map(|()| socket)?));
            match opened {
                Ok(socket) => self.input = Some(socket),
                Err(_) => self.lost(),
            }
        }
        pending.forward(&mut Input(self));
    }

    pub fn close_input(&mut self) {
        self.input = None;
    }

    pub fn resize(&mut self, rows: u16, cols: u16) {
        self.request(&Request::Resize {
            terminal: self.terminal.to_string(),
            rows,
            cols,
        });
    }

    /// Return a PTY a program left raw to the modes it started with.
    pub fn restore_modes(&mut self) {
        self.request(&Request::Restore {
            terminal: self.terminal.to_string(),
        });
    }

    fn request(&mut self, request: &Request) {
        self.unsent.extend(wire::line(request));
        self.flush_requests();
    }

    fn flush_requests(&mut self) {
        if self.unsent.is_empty() {
            return;
        }
        if self.requests.is_none() && self.may_connect() {
            match UnixStream::connect(&self.place.socket) {
                Ok(socket) if socket.set_nonblocking(true).is_ok() => self.requests = Some(socket),
                _ => self.lost(),
            }
        }
        let Some(socket) = self.requests.as_mut() else {
            return;
        };
        while !self.unsent.is_empty() {
            match socket.write(self.unsent.as_slices().0) {
                Ok(n) if n > 0 => {
                    self.unsent.drain(..n);
                }
                Err(e) if e.kind() == io::ErrorKind::WouldBlock => return,
                Err(e) if e.kind() == io::ErrorKind::Interrupted => {}
                _ => {
                    // A size or mode request the holder never read is stale by
                    // the time it could be sent again.
                    self.requests = None;
                    self.unsent.clear();
                    return;
                }
            }
        }
    }

    fn drain_answers(&mut self) {
        let Some(socket) = self.requests.as_mut() else {
            return;
        };
        let mut bytes = [0; 1024];
        loop {
            match socket.read(&mut bytes) {
                Ok(0) => {
                    self.requests = None;
                    return;
                }
                Ok(_) => {}
                Err(e) if e.kind() == io::ErrorKind::Interrupted => {}
                Err(e) if e.kind() == io::ErrorKind::WouldBlock => return,
                Err(_) => {
                    self.requests = None;
                    return;
                }
            }
        }
    }

    /// A second read of this terminal from `from`, for a view that already
    /// shows the screen up to there. `None` when the holder cannot serve it:
    /// the view is painted afresh instead.
    pub fn catch_up(&self, from: u64) -> Option<CatchUp> {
        let output = Client::connect_within(&self.place, CONNECT)
            .ok()?
            .read(self.terminal.as_str(), from)
            .ok()?;
        output.socket.set_nonblocking(true).ok()?;
        Some(CatchUp {
            socket: output.socket,
            decoder: output.decoder,
            at: from,
        })
    }

    /// End the child and everything in its process group.
    pub fn retire(&self) {
        if self.ended.is_none() {
            crate::os_process::kill_process_group(self.pid);
        }
    }

    pub fn poll(&self, input: bool, fds: &mut Vec<libc::pollfd>) {
        let pollfd = |fd: RawFd, events: libc::c_short| libc::pollfd {
            fd,
            events,
            revents: 0,
        };
        if let Some((socket, _)) = &self.output {
            fds.push(pollfd(socket.as_raw_fd(), libc::POLLIN));
        }
        if let Some(socket) = self.input.as_ref().filter(|_| input) {
            fds.push(pollfd(socket.as_raw_fd(), libc::POLLOUT));
        }
        if let Some(socket) = &self.requests {
            let events = if self.unsent.is_empty() {
                libc::POLLIN
            } else {
                libc::POLLIN | libc::POLLOUT
            };
            fds.push(pollfd(socket.as_raw_fd(), events));
        }
    }
}

/// The PTY output a view missed, read from the holder's ring on a connection
/// of its own, so the registry keeps no second ring.
#[derive(Debug)]
pub(super) struct CatchUp {
    socket: UnixStream,
    decoder: Decoder,
    /// The next PTY byte this read returns.
    at: u64,
}

impl CatchUp {
    /// The bytes from where the view stands up to `until`, the position the
    /// live stream has reached; what lies beyond comes live. An error means
    /// the view cannot continue: the ring no longer holds its position or
    /// the holder went away.
    pub fn read(&mut self, until: u64) -> io::Result<Vec<u8>> {
        let mut caught = Vec::new();
        loop {
            while let Some(frame) = self.decoder.frame()? {
                match frame {
                    Frame::Out(bytes) => {
                        let wanted = until.saturating_sub(self.at).min(bytes.len() as u64);
                        caught.extend_from_slice(&bytes[..wanted as usize]);
                        self.at += bytes.len() as u64;
                    }
                    Frame::Mode(_) => {}
                    Frame::Gap(_) => {
                        return Err(io::Error::other(
                            "the ring no longer holds this view's position",
                        ))
                    }
                    Frame::Exit(_) => return Ok(caught),
                }
                if self.at >= until {
                    return Ok(caught);
                }
            }
            let mut bytes = [0; 16384];
            match self.socket.read(&mut bytes) {
                Ok(0) => return Err(io::ErrorKind::UnexpectedEof.into()),
                Ok(n) => self.decoder.extend(&bytes[..n]),
                Err(e) if e.kind() == io::ErrorKind::WouldBlock => return Ok(caught),
                Err(e) if e.kind() == io::ErrorKind::Interrupted => {}
                Err(e) => return Err(e),
            }
        }
    }

    pub fn fd(&self) -> RawFd {
        self.socket.as_raw_fd()
    }
}

/// The holder's input connection as a writer that never loses typed bytes: a
/// missing or failed connection reads as not ready.
struct Input<'a>(&'a mut Source);

impl Write for Input<'_> {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        let Some(socket) = self.0.input.as_mut() else {
            return Err(io::ErrorKind::WouldBlock.into());
        };
        match socket.write(bytes) {
            Ok(n) if n > 0 => Ok(n),
            Err(e) if e.kind() == io::ErrorKind::WouldBlock => Err(e),
            _ => {
                self.0.input = None;
                self.0.lost();
                Err(io::ErrorKind::WouldBlock.into())
            }
        }
    }

    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}
