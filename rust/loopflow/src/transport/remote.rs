//! A remote viewer's PTY crosses SSH as framed input/output, with client heartbeats.
//! The remote relay ends on lost heartbeats even if sshd has not noticed a broken
//! TCP connection. Its detached Session/shell owner remains independent.
use std::collections::VecDeque;
use std::fs::File;
use std::io::{self, Read, Write};
use std::os::fd::{AsRawFd, FromRawFd, RawFd};
use std::os::unix::process::CommandExt;
use std::process::{Command, Stdio};
use std::time::{Duration, SystemTime};

use anyhow::{bail, Context, Result};
use serde::{Deserialize, Serialize};

use super::wire::{self, Decoder, Tag};
use super::{nonblocking, resize, size, terminal_pair, ViewerTerminal, LIMIT};

const INTERVAL: Duration = Duration::from_secs(10);
const TIMEOUT: Duration = Duration::from_secs(30);

#[derive(Debug, Serialize, Deserialize)]
struct Heartbeat {
    rows: u16,
    cols: u16,
}

fn poll(read: RawFd, second: RawFd, write: RawFd, pending: bool) -> io::Result<()> {
    let mut fds = [
        libc::pollfd {
            fd: read,
            events: libc::POLLIN,
            revents: 0,
        },
        libc::pollfd {
            fd: second,
            events: libc::POLLIN,
            revents: 0,
        },
        libc::pollfd {
            fd: write,
            events: if pending { libc::POLLOUT } else { 0 },
            revents: 0,
        },
    ];
    // SAFETY: poll borrows three live descriptors for the duration of this call.
    if unsafe { libc::poll(fds.as_mut_ptr(), fds.len() as _, 250) } < 0 {
        let error = io::Error::last_os_error();
        if error.kind() != io::ErrorKind::Interrupted {
            return Err(error);
        }
    }
    Ok(())
}

fn read(input: &mut impl Read, bytes: &mut [u8]) -> io::Result<Option<usize>> {
    match input.read(bytes) {
        Err(error)
            if matches!(
                error.kind(),
                io::ErrorKind::WouldBlock | io::ErrorKind::Interrupted
            ) =>
        {
            Ok(None)
        }
        Err(error) if error.raw_os_error() == Some(libc::EIO) => Ok(Some(0)),
        result => result.map(Some),
    }
}

fn flush(output: &mut impl Write, pending: &mut VecDeque<u8>) -> io::Result<()> {
    while !pending.is_empty() {
        match output.write(pending.as_slices().0) {
            Ok(0) => return Err(io::ErrorKind::WriteZero.into()),
            Ok(n) => {
                pending.drain(..n);
            }
            Err(error)
                if matches!(
                    error.kind(),
                    io::ErrorKind::WouldBlock | io::ErrorKind::Interrupted
                ) =>
            {
                break
            }
            Err(error) => return Err(error),
        }
    }
    Ok(())
}

/// Restore flags on borrowed stdin/stdout when the framing relay ends.
struct Nonblocking {
    fd: RawFd,
    flags: i32,
}
impl Nonblocking {
    fn new(fd: RawFd) -> io::Result<Self> {
        // SAFETY: the descriptor remains owned by the caller.
        let flags = unsafe { libc::fcntl(fd, libc::F_GETFL) };
        if flags < 0 {
            return Err(io::Error::last_os_error());
        }
        nonblocking(fd)?;
        crate::agent::register_interrupt_cleanup(move || {
            // SAFETY: this relay only borrows standard descriptors, which stay
            // open until process exit even when an interrupt skips Drop.
            unsafe {
                libc::fcntl(fd, libc::F_SETFL, flags);
            }
        });
        Ok(Self { fd, flags })
    }
}
impl Drop for Nonblocking {
    fn drop(&mut self) {
        // SAFETY: these borrowed standard descriptors live through the relay.
        unsafe {
            libc::fcntl(self.fd, libc::F_SETFL, self.flags);
        }
    }
}

pub(crate) fn client(argv: &[String]) -> Result<()> {
    let mut child = Command::new(&argv[0])
        .args(&argv[1..])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::inherit())
        .process_group(0)
        .spawn()
        .context("start remote viewer transport")?;
    let group = crate::os_process::ProcessGroupGuard::new(child.id());
    let mut input = child.stdout.take().expect("piped SSH stdout");
    let mut output = child.stdin.take().expect("piped SSH stdin");
    nonblocking(input.as_raw_fd())?;
    nonblocking(output.as_raw_fd())?;
    let mut terminal = ViewerTerminal::enter()?;
    terminal.make_raw()?;
    let _stdin_flags = Nonblocking::new(0)?;
    let mut decoder = Decoder::default();
    let mut pending = VecDeque::new();
    let mut next_heartbeat = SystemTime::now();
    let mut previous_size = None;
    let mut bytes = [0; 16384];
    loop {
        let now = SystemTime::now();
        let (rows, cols) = size();
        if now >= next_heartbeat || previous_size != Some((rows, cols)) {
            wire::encode(
                Tag::Request,
                &serde_json::to_vec(&Heartbeat { rows, cols })?,
                &mut pending,
            );
            next_heartbeat = now + INTERVAL;
            previous_size = Some((rows, cols));
        }
        if pending.len() < LIMIT {
            if let Some(n) = read(&mut io::stdin(), &mut bytes)? {
                if n == 0 {
                    bail!("Remote viewer input closed");
                }
                wire::encode(Tag::Input, &bytes[..n], &mut pending);
            }
        }
        flush(&mut output, &mut pending)?;
        if let Some(n) = read(&mut input, &mut bytes)? {
            if n == 0 {
                let status = child.wait()?;
                group.disarm();
                bail!("Remote viewer connection ended without an exit receipt ({status})");
            }
            decoder.extend(&bytes[..n]);
            while let Some((tag, payload)) = decoder.next()? {
                match tag {
                    Tag::Out => terminal.write(1, &payload)?,
                    Tag::Reply => {
                        let code: u8 = serde_json::from_slice(&payload)?;
                        let _ = child.wait();
                        group.disarm();
                        return if code == 0 {
                            Ok(())
                        } else {
                            Err(crate::process::CommandExit(code).into())
                        };
                    }
                    _ => bail!("Unexpected remote viewer frame"),
                }
            }
        }
        poll(
            input.as_raw_fd(),
            if pending.len() < LIMIT { 0 } else { -1 },
            output.as_raw_fd(),
            !pending.is_empty(),
        )?;
    }
}

pub(crate) fn serve(argv: &[String]) -> Result<()> {
    let (mut master, slave) = terminal_pair()?;
    let mut command = Command::new(&argv[0]);
    command
        .args(&argv[1..])
        .stdin(slave.try_clone()?)
        .stdout(slave.try_clone()?)
        .stderr(slave);
    // SAFETY: only the new child changes its session; stdin is its slave PTY.
    unsafe {
        command.pre_exec(|| {
            if libc::setsid() < 0 || libc::ioctl(0, libc::TIOCSCTTY as _, 0) < 0 {
                return Err(io::Error::last_os_error());
            }
            Ok(())
        });
    }
    let mut child = command.spawn().context("start remote viewer")?;
    drop(command);
    let group = crate::os_process::ProcessGroupGuard::new(child.id());
    nonblocking(master.as_raw_fd())?;
    let _stdin_flags = Nonblocking::new(0)?;
    let _stdout_flags = Nonblocking::new(1)?;
    // SAFETY: dup creates independent owned descriptors; standard streams stay open.
    let mut input = unsafe { File::from_raw_fd(duplicate(0)?) };
    // SAFETY: as above, this File owns only the duplicate.
    let mut output = unsafe { File::from_raw_fd(duplicate(1)?) };
    let result = serve_stream(&mut input, &mut output, &mut master, &mut child);
    group.terminate();
    let _ = child.wait();
    result
}

fn duplicate(fd: RawFd) -> io::Result<RawFd> {
    // SAFETY: dup borrows an open standard descriptor and returns a new one.
    let duplicate = unsafe { libc::dup(fd) };
    if duplicate < 0 {
        Err(io::Error::last_os_error())
    } else {
        Ok(duplicate)
    }
}

fn serve_stream(
    input: &mut File,
    output: &mut File,
    master: &mut File,
    child: &mut std::process::Child,
) -> Result<()> {
    let mut deadline = SystemTime::now() + TIMEOUT;
    let mut decoder = Decoder::default();
    let mut outgoing = VecDeque::new();
    let mut keystrokes = VecDeque::new();
    let mut bytes = [0; 16384];
    let mut ended = false;
    let mut dimensions = None;
    let mut pty_closed = false;
    loop {
        anyhow::ensure!(
            SystemTime::now() < deadline,
            "Remote viewer heartbeat expired; execution remains on its owner"
        );
        if let Some(n) = read(input, &mut bytes)? {
            anyhow::ensure!(n != 0, "Remote viewer disconnected");
            decoder.extend(&bytes[..n]);
            while let Some((tag, payload)) = decoder.next()? {
                match tag {
                    Tag::Request => {
                        let heartbeat: Heartbeat = serde_json::from_slice(&payload)?;
                        deadline = SystemTime::now() + TIMEOUT;
                        if dimensions != Some((heartbeat.rows, heartbeat.cols)) {
                            resize(master.as_raw_fd(), heartbeat.rows, heartbeat.cols)?;
                            dimensions = Some((heartbeat.rows, heartbeat.cols));
                        }
                    }
                    Tag::Input => {
                        anyhow::ensure!(
                            keystrokes.len() + payload.len() <= LIMIT,
                            "Remote viewer input overflow"
                        );
                        keystrokes.extend(payload);
                    }
                    _ => bail!("Unexpected remote viewer request"),
                }
            }
        }
        flush(master, &mut keystrokes)?;
        if outgoing.len() < LIMIT && !pty_closed {
            if let Some(n) = read(master, &mut bytes)? {
                if n == 0 {
                    pty_closed = true;
                } else {
                    wire::encode(Tag::Out, &bytes[..n], &mut outgoing);
                }
            }
        }
        if pty_closed && !ended {
            if let Some(status) = child.try_wait()? {
                let code = u8::try_from(status.code().unwrap_or(1)).unwrap_or(1);
                wire::encode(Tag::Reply, &serde_json::to_vec(&code)?, &mut outgoing);
                ended = true;
            }
        }
        flush(output, &mut outgoing)?;
        if ended && outgoing.is_empty() {
            return Ok(());
        }
        poll(
            input.as_raw_fd(),
            if !pty_closed && outgoing.len() < LIMIT {
                master.as_raw_fd()
            } else {
                -1
            },
            output.as_raw_fd(),
            !outgoing.is_empty(),
        )?;
    }
}
