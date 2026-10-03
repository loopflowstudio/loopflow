//! Private, nonblocking transport for the provider's native browser request.
use std::fs::{self, File, OpenOptions};
use std::io::{self, Read};
use std::os::unix::fs::{OpenOptionsExt, PermissionsExt};

use tokio::process::Command;

const MAX_URL_BYTES: usize = 16 * 1024;

pub(super) struct BrowserHandoff {
    _directory: tempfile::TempDir,
    pipe: File,
    bytes: Vec<u8>,
}

impl BrowserHandoff {
    pub(super) fn new(command: &mut Command) -> io::Result<Self> {
        let directory = tempfile::Builder::new().prefix("lf-browser-").tempdir()?;
        fs::set_permissions(directory.path(), fs::Permissions::from_mode(0o700))?;
        let fifo = directory.path().join("url");
        let path = std::ffi::CString::new(fifo.as_os_str().as_encoded_bytes())?;
        // SAFETY: path is a NUL-terminated C string, valid for this call.
        if unsafe { libc::mkfifo(path.as_ptr(), 0o600) } != 0 {
            return Err(io::Error::last_os_error());
        }
        // Keep both ends open, without a blocking I/O task. Dropping the handle
        // closes the pipe even when no browser request ever arrives.
        let pipe = OpenOptions::new()
            .read(true)
            .write(true)
            .custom_flags(libc::O_NONBLOCK)
            .open(&fifo)?;
        let helper = directory.path().join("browser");
        fs::write(
            &helper,
            b"#!/bin/sh\nprintf '%s\\n' \"$1\" > \"$LF_AUTH_BROWSER_FIFO\"\n",
        )?;
        fs::set_permissions(&helper, fs::Permissions::from_mode(0o700))?;
        command
            .env("BROWSER", &helper)
            .env("CLAUDE_BROWSER", &helper)
            .env("LF_AUTH_BROWSER_FIFO", &fifo);
        Ok(Self {
            _directory: directory,
            pipe,
            bytes: Vec::new(),
        })
    }

    pub(super) fn read_url(&mut self) -> io::Result<Option<String>> {
        let mut chunk = [0; 1024];
        loop {
            match self.pipe.read(&mut chunk) {
                Ok(0) => break,
                Ok(count) => {
                    if self.bytes.len() + count > MAX_URL_BYTES {
                        return Err(io::Error::new(
                            io::ErrorKind::InvalidData,
                            "browser handoff exceeds limit",
                        ));
                    }
                    self.bytes.extend_from_slice(&chunk[..count]);
                    if let Some(end) = self.bytes.iter().position(|byte| *byte == b'\n') {
                        let value =
                            std::str::from_utf8(&self.bytes[..end]).map_err(|_| invalid_url())?;
                        let url = reqwest::Url::parse(value).map_err(|_| invalid_url())?;
                        if url.scheme() != "https" || url.host_str().is_none() {
                            return Err(invalid_url());
                        }
                        return Ok(Some(value.to_string()));
                    }
                }
                Err(error) if error.kind() == io::ErrorKind::WouldBlock => break,
                Err(error) => return Err(error),
            }
        }
        Ok(None)
    }
}

fn invalid_url() -> io::Error {
    io::Error::new(
        io::ErrorKind::InvalidData,
        "invalid browser authorization URL",
    )
}
