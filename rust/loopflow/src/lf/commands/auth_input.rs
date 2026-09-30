//! Cancellable terminal input. No blocking stdin worker survives browser completion.
use std::io::{self, IsTerminal};

use anyhow::{anyhow, Result};
use secrecy::SecretString;

#[cfg(unix)]
use std::{fs::OpenOptions, io::Read, os::fd::AsRawFd, os::unix::fs::OpenOptionsExt};

#[cfg(unix)]
pub(super) struct AuthInput {
    terminal: std::fs::File,
    original: libc::termios,
    active: std::sync::Arc<std::sync::atomic::AtomicBool>,
    bytes: Vec<u8>,
}

#[cfg(unix)]
impl AuthInput {
    pub(super) fn open() -> Result<Option<Self>> {
        if !io::stdin().is_terminal() {
            return Ok(None);
        }
        let terminal = OpenOptions::new()
            .read(true)
            .write(true)
            .custom_flags(libc::O_NONBLOCK)
            .open("/dev/tty")?;
        let mut original = std::mem::MaybeUninit::uninit();
        // SAFETY: original points to writable termios storage and fd is open.
        if unsafe { libc::tcgetattr(terminal.as_raw_fd(), original.as_mut_ptr()) } != 0 {
            return Err(io::Error::last_os_error().into());
        }
        // SAFETY: tcgetattr initialized original on success.
        let original = unsafe { original.assume_init() };
        let cleanup_terminal = terminal.try_clone()?;
        let mut hidden = original;
        hidden.c_lflag &= !libc::ECHO;
        // SAFETY: fd is open and hidden is an initialized termios value.
        if unsafe { libc::tcsetattr(terminal.as_raw_fd(), libc::TCSANOW, &hidden) } != 0 {
            return Err(io::Error::last_os_error().into());
        }
        let active = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(true));
        let cleanup_active = active.clone();
        crate::engine::agent::register_interrupt_cleanup(move || {
            if cleanup_active.swap(false, std::sync::atomic::Ordering::AcqRel) {
                // SAFETY: the retained terminal fd and original attributes are valid.
                unsafe {
                    libc::tcsetattr(cleanup_terminal.as_raw_fd(), libc::TCSAFLUSH, &original);
                }
            }
        });
        Ok(Some(Self {
            terminal,
            original,
            active,
            bytes: Vec::new(),
        }))
    }

    pub(super) async fn line(&mut self) -> Result<SecretString> {
        loop {
            let mut byte = [0];
            match self.terminal.read(&mut byte) {
                Ok(0) => return Err(anyhow!("authorization terminal closed")),
                Ok(_) if byte[0] == b'\n' => {
                    let bytes = std::mem::take(&mut self.bytes);
                    return String::from_utf8(bytes)
                        .map(SecretString::new)
                        .map_err(|_| anyhow!("authorization input must be UTF-8"));
                }
                Ok(_) => {
                    if self.bytes.len() >= 16 * 1024 {
                        return Err(anyhow!("authorization input exceeds limit"));
                    }
                    self.bytes.push(byte[0]);
                }
                Err(error) if error.kind() == io::ErrorKind::WouldBlock => {
                    tokio::time::sleep(std::time::Duration::from_millis(50)).await;
                }
                Err(error) => return Err(error.into()),
            }
        }
    }
}

#[cfg(unix)]
impl Drop for AuthInput {
    fn drop(&mut self) {
        self.active
            .store(false, std::sync::atomic::Ordering::Release);
        // SAFETY: the terminal fd and saved attributes remain valid until drop ends.
        unsafe {
            libc::tcsetattr(self.terminal.as_raw_fd(), libc::TCSAFLUSH, &self.original);
        }
    }
}

#[cfg(not(unix))]
pub(super) struct AuthInput;

#[cfg(not(unix))]
impl AuthInput {
    pub(super) fn open() -> Result<Option<Self>> {
        Ok(None)
    }
    pub(super) async fn line(&mut self) -> Result<SecretString> {
        Err(anyhow!("manual terminal input is unsupported on this host"))
    }
}
