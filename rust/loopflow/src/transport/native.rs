//! A native client's exit text names its own resume commands, which bypass lf
//! and, for a remote Codex client, a relay that no longer exists. While a client
//! runs, the owner withholds what it prints after restoring the terminal and
//! shows lf's return line instead.

use std::os::fd::RawFd;
use std::time::{Duration, Instant};

use super::LIMIT;

/// Output after a client restores its terminal is exit text only if the client
/// exits within this window; otherwise it was ordinary output.
const EXIT_WINDOW: Duration = Duration::from_secs(5);

#[derive(Debug)]
pub(super) struct NativeClient {
    exit: String,
    pub detach: String,
    raw: bool,
    held: Vec<u8>,
    since: Option<Instant>,
}

impl NativeClient {
    pub fn new(exit: String, detach: String) -> Self {
        Self {
            exit,
            detach,
            raw: false,
            held: Vec::new(),
            since: None,
        }
    }

    /// Bytes to deliver now from output read while the terminal at `fd` is in
    /// its current mode.
    pub fn accept(&mut self, fd: RawFd, bytes: &[u8]) -> Vec<u8> {
        if is_raw(fd) {
            self.raw = true;
            self.since = None;
            let mut ready = std::mem::take(&mut self.held);
            ready.extend_from_slice(bytes);
            return ready;
        }
        if !self.raw {
            return bytes.to_vec();
        }
        self.held.extend_from_slice(bytes);
        self.since.get_or_insert_with(Instant::now);
        if self.held.len() > LIMIT / 16 {
            self.since = None;
            return std::mem::take(&mut self.held);
        }
        Vec::new()
    }

    /// Held output that outlived the exit window.
    pub fn expire(&mut self) -> Vec<u8> {
        if self
            .since
            .is_some_and(|since| since.elapsed() > EXIT_WINDOW)
        {
            self.since = None;
            return std::mem::take(&mut self.held);
        }
        Vec::new()
    }

    /// Output withheld from a client whose end was never reported.
    pub fn release(self) -> Vec<u8> {
        self.held
    }

    /// The client exited: keep its terminal restoration, replace its text.
    pub fn finish(self) -> Vec<u8> {
        let mut bytes = controls(&self.held);
        bytes.extend_from_slice(format!("\r\n{}\r\n", self.exit).as_bytes());
        bytes
    }
}

fn is_raw(fd: RawFd) -> bool {
    // SAFETY: tcgetattr writes into a live termios; the descriptor is borrowed.
    unsafe {
        let mut termios = std::mem::zeroed::<libc::termios>();
        libc::tcgetattr(fd, &mut termios) == 0 && termios.c_lflag & libc::ICANON == 0
    }
}

/// Mode and cursor sequences restore the viewer's terminal. String sequences
/// (titles, queries, graphics) and text are the client's exit message.
fn controls(bytes: &[u8]) -> Vec<u8> {
    let mut kept = Vec::new();
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] != 0x1b {
            i += 1;
            continue;
        }
        let start = i;
        i += 1;
        match bytes.get(i) {
            Some(b'[') => {
                i += 1;
                while i < bytes.len() && !(0x40..=0x7e).contains(&bytes[i]) {
                    i += 1;
                }
                i = (i + 1).min(bytes.len());
                kept.extend_from_slice(&bytes[start..i]);
            }
            Some(b']' | b'P' | b'_' | b'^' | b'X') => {
                while i < bytes.len() {
                    if bytes[i] == 0x07 {
                        i += 1;
                        break;
                    }
                    if bytes[i] == 0x1b && bytes.get(i + 1) == Some(&b'\\') {
                        i += 2;
                        break;
                    }
                    i += 1;
                }
            }
            Some(b'(' | b')') => {
                i = (i + 2).min(bytes.len());
                kept.extend_from_slice(&bytes[start..i]);
            }
            Some(_) => {
                i += 1;
                kept.extend_from_slice(&bytes[start..i]);
            }
            None => {}
        }
    }
    kept
}

#[cfg(test)]
mod tests {
    use std::os::fd::AsRawFd;

    use super::{controls, NativeClient};

    fn terminal() -> (std::fs::File, std::fs::File) {
        let (mut master, mut slave) = (-1, -1);
        // SAFETY: openpty initializes both descriptors on success.
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
        // SAFETY: openpty returned two newly owned descriptors.
        unsafe {
            use std::os::fd::FromRawFd;
            (
                std::fs::File::from_raw_fd(master),
                std::fs::File::from_raw_fd(slave),
            )
        }
    }

    fn set_raw(fd: i32, raw: bool) {
        // SAFETY: termios is read from a live descriptor before modification.
        unsafe {
            let mut termios = std::mem::zeroed::<libc::termios>();
            libc::tcgetattr(fd, &mut termios);
            if raw {
                libc::cfmakeraw(&mut termios);
            } else {
                termios.c_lflag |= libc::ICANON | libc::ECHO;
            }
            libc::tcsetattr(fd, libc::TCSANOW, &termios);
        }
    }

    #[test]
    fn exit_text_after_terminal_restoration_becomes_the_lf_line() {
        let (master, slave) = terminal();
        let mut client = NativeClient::new("Left it.".into(), "Detached.".into());
        assert_eq!(
            client.accept(master.as_raw_fd(), b"Starting\r\n"),
            b"Starting\r\n"
        );
        set_raw(slave.as_raw_fd(), true);
        assert_eq!(client.accept(master.as_raw_fd(), b"frame"), b"frame");
        set_raw(slave.as_raw_fd(), false);
        let exit = b"\x1b[?1049l\x1b[?25h\x1b]0;title\x07To continue, run:\r\n  codex resume 1\r\n";
        assert!(client.accept(master.as_raw_fd(), exit).is_empty());
        assert_eq!(client.finish(), b"\x1b[?1049l\x1b[?25h\r\nLeft it.\r\n");
    }

    #[test]
    fn output_after_a_client_returns_to_its_terminal_is_delivered() {
        let (master, slave) = terminal();
        let mut client = NativeClient::new("Left it.".into(), "Detached.".into());
        set_raw(slave.as_raw_fd(), true);
        client.accept(master.as_raw_fd(), b"frame");
        set_raw(slave.as_raw_fd(), false);
        assert!(client
            .accept(master.as_raw_fd(), b"shell output\r\n")
            .is_empty());
        set_raw(slave.as_raw_fd(), true);
        assert_eq!(
            client.accept(master.as_raw_fd(), b"next frame"),
            b"shell output\r\nnext frame"
        );
    }

    #[test]
    fn controls_keep_terminal_modes_and_drop_text_and_strings() {
        assert_eq!(
            controls(b"\x1b[0mhi\x1bP+q\x1b\\\x1b_Ga=q\x1b\\\x1b(B\x1b7done\x1b[?2004l"),
            b"\x1b[0m\x1b(B\x1b7\x1b[?2004l"
        );
    }
}
