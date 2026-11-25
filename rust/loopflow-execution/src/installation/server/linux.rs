use anyhow::{ensure, Context, Result};
use std::fs;
use std::io;
use std::os::fd::{AsRawFd, FromRawFd, OwnedFd};
use std::os::unix::fs::OpenOptionsExt;
use std::os::unix::net::UnixStream;
use std::os::unix::process::CommandExt;
use std::path::Path;
use std::process::{Command, Stdio};

pub(super) fn user_manager() -> Result<bool> {
    let result = Command::new("systemctl")
        .args(["--user", "show", "--property=Version", "--value"])
        .env("LC_ALL", "C")
        .output();
    let output = match result {
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(false),
        result => result.context("probe systemd user manager")?,
    };
    if output.status.success() {
        return Ok(true);
    }
    let error = String::from_utf8_lossy(&output.stderr);
    if error.contains("Failed to connect to")
        && [
            "No medium found",
            "No such file or directory",
            "Connection refused",
            "Host is down",
            "not set",
            "not defined",
        ]
        .iter()
        .any(|reason| error.contains(reason))
    {
        return Ok(false);
    }
    anyhow::bail!("systemd user manager: {}", error.trim())
}

pub(super) fn start_detached(home: &Path, executable: &Path, replace: bool) -> Result<()> {
    if replace {
        let socket = crate::machine::socket_path()?;
        if let Err(error) = retire_server(&socket) {
            // It may have exited between the readiness probe and replacement.
            match UnixStream::connect(&socket) {
                Err(io)
                    if matches!(
                        io.kind(),
                        io::ErrorKind::ConnectionRefused | io::ErrorKind::NotFound
                    ) => {}
                _ => return Err(error),
            }
        }
    }
    let logs = home.join("logs");
    fs::create_dir_all(&logs)?;
    let log = fs::OpenOptions::new()
        .create(true)
        .append(true)
        .mode(0o600)
        .open(logs.join("server.log"))?;
    let mut command = Command::new(executable);
    for (key, _) in std::env::vars_os() {
        if key.as_encoded_bytes().starts_with(b"LF_") {
            command.env_remove(key);
        }
    }
    command
        .args(["server", "run"])
        .current_dir(home)
        .env("LF_HOME", home)
        .env("LF_BIN", executable)
        .stdin(Stdio::null())
        .stdout(log.try_clone()?)
        .stderr(log);
    // SAFETY: setsid is async-signal-safe and affects only the child.
    unsafe {
        command.pre_exec(|| {
            if libc::setsid() < 0 {
                return Err(io::Error::last_os_error());
            }
            Ok(())
        });
    }
    let mut child = command.spawn().context("start detached lf server run")?;
    std::thread::spawn(move || {
        let _ = child.wait();
    });
    Ok(())
}

/// Only the server accepting connections on this Machine's private socket.
/// Pin the process before checking its arguments and socket again: a reused
/// numeric pid must never receive the signal.
fn retire_server(path: &Path) -> Result<()> {
    let peer = || -> io::Result<i32> {
        let socket = UnixStream::connect(path)?;
        let mut cred = libc::ucred {
            pid: 0,
            uid: 0,
            gid: 0,
        };
        let mut size = std::mem::size_of_val(&cred) as libc::socklen_t;
        // SAFETY: the connected socket and the credential output buffer are live.
        let result = unsafe {
            libc::getsockopt(
                socket.as_raw_fd(),
                libc::SOL_SOCKET,
                libc::SO_PEERCRED,
                (&raw mut cred).cast(),
                &mut size,
            )
        };
        if result < 0 {
            return Err(io::Error::last_os_error());
        }
        Ok(cred.pid)
    };
    let pid = peer()?;
    // SAFETY: pidfd_open returns a new descriptor or an error.
    let fd = unsafe { libc::syscall(libc::SYS_pidfd_open, pid, 0) };
    if fd < 0 {
        return Err(io::Error::last_os_error().into());
    }
    // SAFETY: the descriptor was just created and has no other owner.
    let fd = unsafe { OwnedFd::from_raw_fd(fd as i32) };
    let args = crate::process::OsProcess::arguments(pid as u32)?;
    ensure!(
        args.is_some_and(|args| args.len() == 3 && args[1..] == ["server", "run"])
            && peer()? == pid,
        "another process is listening on {}",
        path.display()
    );
    // SAFETY: the signal addresses only this pinned process, without siginfo.
    let result = unsafe {
        libc::syscall(
            libc::SYS_pidfd_send_signal,
            fd.as_raw_fd(),
            libc::SIGTERM,
            std::ptr::null::<libc::siginfo_t>(),
            0,
        )
    };
    if result < 0 {
        return Err(io::Error::last_os_error().into());
    }
    let mut poll = libc::pollfd {
        fd: fd.as_raw_fd(),
        events: libc::POLLIN,
        revents: 0,
    };
    // SAFETY: poll has one initialized descriptor and a bounded wait.
    let result = unsafe { libc::poll(&raw mut poll, 1, 10_000) };
    if result < 0 {
        return Err(io::Error::last_os_error().into());
    }
    ensure!(
        result > 0,
        "previous Loopflow server did not exit within 10s"
    );
    Ok(())
}
