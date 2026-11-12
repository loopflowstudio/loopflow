//! The viewer protocol travels unchanged over a socket or a command's pipes.
//! The SSH destination only forwards bytes: no PTY, claims or heartbeats.
use std::collections::VecDeque;
use std::fs::File;
use std::io::{self, Read, Write};
use std::os::fd::{AsRawFd, FromRawFd, OwnedFd, RawFd};
use std::os::unix::net::UnixStream;
use std::time::{Duration, Instant};

use anyhow::{Context, Result};

use super::{nonblocking, LIMIT};

#[derive(Debug)]
pub(super) struct Stream {
    pub input: File,
    output: File,
}

impl Stream {
    pub fn new(input: File, output: File) -> io::Result<Self> {
        nonblocking(input.as_raw_fd())?;
        nonblocking(output.as_raw_fd())?;
        Ok(Self { input, output })
    }

    pub fn socket(socket: UnixStream) -> io::Result<Self> {
        let output = socket.try_clone()?;
        Self::new(OwnedFd::from(socket).into(), OwnedFd::from(output).into())
    }

    /// Close a socket for both directions while this viewer holds it. Pipes
    /// are not sockets and stay as they are.
    pub fn shutdown(&self) {
        // SAFETY: shutdown only reads the live descriptor this value owns.
        unsafe { libc::shutdown(self.input.as_raw_fd(), libc::SHUT_RDWR) };
    }
}

impl Read for Stream {
    fn read(&mut self, bytes: &mut [u8]) -> io::Result<usize> {
        self.input.read(bytes)
    }
}

impl Write for Stream {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        let deadline = Instant::now() + Duration::from_secs(2);
        loop {
            match self.output.write(bytes) {
                Err(error) if error.kind() == io::ErrorKind::Interrupted => continue,
                Err(error) if error.kind() == io::ErrorKind::WouldBlock => {
                    if Instant::now() >= deadline {
                        return Err(io::ErrorKind::TimedOut.into());
                    }
                    poll(&mut [interest(self.output.as_raw_fd(), libc::POLLOUT)])?;
                }
                result => return result,
            }
        }
    }

    fn flush(&mut self) -> io::Result<()> {
        self.output.flush()
    }
}

fn interest(fd: RawFd, events: i16) -> libc::pollfd {
    libc::pollfd {
        fd,
        events,
        revents: 0,
    }
}

fn poll(fds: &mut [libc::pollfd]) -> io::Result<()> {
    // SAFETY: poll borrows the live descriptor array for this call only.
    if unsafe { libc::poll(fds.as_mut_ptr(), fds.len() as _, 250) } < 0 {
        let error = io::Error::last_os_error();
        if error.kind() != io::ErrorKind::Interrupted {
            return Err(error);
        }
    }
    Ok(())
}

/// Nonblocking stdin/stdout belong to the caller, including on interrupt.
struct BorrowedFlags {
    fd: RawFd,
    flags: i32,
}

impl BorrowedFlags {
    fn new(fd: RawFd) -> io::Result<Self> {
        // SAFETY: this borrows an open standard descriptor.
        let flags = unsafe { libc::fcntl(fd, libc::F_GETFL) };
        if flags < 0 {
            return Err(io::Error::last_os_error());
        }
        nonblocking(fd)?;
        crate::agent::register_interrupt_cleanup(move || {
            // SAFETY: standard descriptors stay open until process exit.
            unsafe {
                libc::fcntl(fd, libc::F_SETFL, flags);
            }
        });
        Ok(Self { fd, flags })
    }
}

impl Drop for BorrowedFlags {
    fn drop(&mut self) {
        // SAFETY: the bridge only borrows these still-open standard descriptors.
        unsafe {
            libc::fcntl(self.fd, libc::F_SETFL, self.flags);
        }
    }
}

fn duplicate(fd: RawFd) -> io::Result<File> {
    // SAFETY: this creates a fresh owned descriptor without closing the original.
    let duplicate = unsafe { libc::fcntl(fd, libc::F_DUPFD_CLOEXEC, 3) };
    if duplicate < 0 {
        return Err(io::Error::last_os_error());
    }
    // SAFETY: File takes sole ownership of the descriptor just allocated.
    Ok(unsafe { File::from_raw_fd(duplicate) })
}

fn receive(input: &mut impl Read, pending: &mut VecDeque<u8>) -> io::Result<bool> {
    let mut bytes = [0; 16384];
    let limit = bytes.len().min(LIMIT.saturating_sub(pending.len()));
    if limit == 0 {
        return Ok(true);
    }
    match input.read(&mut bytes[..limit]) {
        Ok(0) => Ok(false),
        Ok(n) => {
            pending.extend(&bytes[..n]);
            Ok(true)
        }
        Err(error)
            if matches!(
                error.kind(),
                io::ErrorKind::WouldBlock | io::ErrorKind::Interrupted
            ) =>
        {
            Ok(true)
        }
        Err(error) => Err(error),
    }
}

fn flush(output: &mut impl Write, pending: &mut VecDeque<u8>) -> io::Result<()> {
    if pending.is_empty() {
        return Ok(());
    }
    match output.write(pending.as_slices().0) {
        Ok(0) => Err(io::ErrorKind::WriteZero.into()),
        Ok(n) => {
            pending.drain(..n);
            Ok(())
        }
        Err(error)
            if matches!(
                error.kind(),
                io::ErrorKind::WouldBlock | io::ErrorKind::Interrupted
            ) =>
        {
            Ok(())
        }
        Err(error) => Err(error),
    }
}

pub(super) fn bridge(id: &crate::id::TerminalId) -> Result<()> {
    let mut socket =
        super::connect(id).context("Execution is unavailable; nothing was restarted")?;
    socket.set_nonblocking(true)?;
    let _input_flags = BorrowedFlags::new(0)?;
    let _output_flags = BorrowedFlags::new(1)?;
    let mut input = duplicate(0)?;
    let mut output = duplicate(1)?;
    let mut to_owner = VecDeque::new();
    let mut to_viewer = VecDeque::new();
    let mut input_open = true;
    let mut owner_open = true;
    let mut drain_until = None;
    loop {
        if input_open && owner_open {
            input_open = receive(&mut input, &mut to_owner)?;
        }
        if owner_open {
            flush(&mut socket, &mut to_owner)?;
            owner_open = receive(&mut socket, &mut to_viewer)?;
            if !owner_open {
                drain_until = Some(Instant::now() + Duration::from_secs(2));
            }
        }
        flush(&mut output, &mut to_viewer)?;
        anyhow::ensure!(
            to_owner.len() < LIMIT && to_viewer.len() < LIMIT,
            "Viewer fell behind its stream queue limit"
        );
        if (!input_open && to_owner.is_empty()) || (!owner_open && to_viewer.is_empty()) {
            return Ok(());
        }
        anyhow::ensure!(
            drain_until.is_none_or(|deadline| Instant::now() < deadline),
            "Viewer stopped reading after its held terminal disconnected"
        );
        // Full queues returned above; poll only for open endpoints and pending writes.
        poll(&mut [
            interest(if input_open && owner_open { 0 } else { -1 }, libc::POLLIN),
            interest(
                if owner_open { socket.as_raw_fd() } else { -1 },
                libc::POLLIN
                    | if to_owner.is_empty() {
                        0
                    } else {
                        libc::POLLOUT
                    },
            ),
            interest(if to_viewer.is_empty() { -1 } else { 1 }, libc::POLLOUT),
        ])?;
    }
}
