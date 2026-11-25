use anyhow::{ensure, Context, Result};
use std::fs;
use std::io;
use std::os::fd::{AsRawFd, FromRawFd, OwnedFd};
use std::os::unix::fs::OpenOptionsExt;
use std::os::unix::net::UnixStream;
use std::os::unix::process::CommandExt;
use std::path::Path;
use std::process::{Command, Stdio};
#[cfg(target_os = "macos")]
use std::time::{Duration, Instant};

pub(super) fn start(home: &Path, executable: &Path, replace: bool) -> Result<()> {
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
#[cfg(target_os = "macos")]
fn retire_server(path: &Path) -> Result<()> {
    let peer = || -> io::Result<libc::pid_t> {
        let socket = UnixStream::connect(path)?;
        let mut pid: libc::pid_t = 0;
        let mut size = std::mem::size_of_val(&pid) as libc::socklen_t;
        // SAFETY: the connected socket and output buffer are live.
        let result = unsafe {
            libc::getsockopt(
                socket.as_raw_fd(),
                libc::SOL_LOCAL,
                libc::LOCAL_PEERPID,
                (&raw mut pid).cast(),
                &mut size,
            )
        };
        if result < 0 {
            return Err(io::Error::last_os_error());
        }
        Ok(pid)
    };
    let pid = peer()?;
    ensure!(
        pid > 0,
        "another process is listening on {}",
        path.display()
    );
    let Some(before) = crate::process::OsProcess::observe(pid as u32)? else {
        return Ok(());
    };
    // Pin exit notification before re-verifying identity and signaling so we
    // never miss an immediate exit after SIGTERM.
    // SAFETY: kqueue takes no arguments and returns a new descriptor or -1.
    let kq = unsafe { libc::kqueue() };
    if kq < 0 {
        return Err(io::Error::last_os_error().into());
    }
    // SAFETY: kq was just created and has no other owner.
    let kq = unsafe { OwnedFd::from_raw_fd(kq) };
    let register = libc::kevent {
        ident: pid as usize,
        filter: libc::EVFILT_PROC,
        flags: libc::EV_ADD | libc::EV_ONESHOT,
        fflags: libc::NOTE_EXIT,
        data: 0,
        udata: std::ptr::null_mut(),
    };
    let zero = libc::timespec {
        tv_sec: 0,
        tv_nsec: 0,
    };
    // SAFETY: register points to one initialized kevent; zero timeout returns immediately.
    let registered =
        unsafe { libc::kevent(kq.as_raw_fd(), &register, 1, std::ptr::null_mut(), 0, &zero) };
    if registered < 0 {
        let error = io::Error::last_os_error();
        if error.raw_os_error() == Some(libc::ESRCH) {
            return Ok(());
        }
        return Err(error.into());
    }
    let args = crate::process::OsProcess::arguments(pid as u32)?;
    let still_same = crate::process::OsProcess::observe(pid as u32)?
        .is_some_and(|now| now.matches_start(pid as u32, before.started_at));
    if !still_same {
        return Ok(());
    }
    ensure!(
        args.is_some_and(|args| args.len() == 3 && args[1..] == ["server", "run"])
            && peer()? == pid,
        "another process is listening on {}",
        path.display()
    );
    // SAFETY: pid was just verified as this socket's `lf server run` process with matching start time.
    if unsafe { libc::kill(pid, libc::SIGTERM) } < 0 {
        let error = io::Error::last_os_error();
        if error.raw_os_error() == Some(libc::ESRCH) {
            return Ok(());
        }
        return Err(error.into());
    }
    let deadline = Instant::now() + Duration::from_secs(10);
    loop {
        let remaining = deadline.saturating_duration_since(Instant::now());
        if remaining.is_zero() {
            anyhow::bail!("previous Loopflow server did not exit within 10s");
        }
        let timeout = libc::timespec {
            tv_sec: remaining.as_secs() as libc::time_t,
            tv_nsec: remaining.subsec_nanos() as libc::c_long,
        };
        let mut event = std::mem::MaybeUninit::<libc::kevent>::zeroed();
        // SAFETY: event points to valid storage for one kevent; timeout is live.
        let ready = unsafe {
            libc::kevent(
                kq.as_raw_fd(),
                std::ptr::null(),
                0,
                event.as_mut_ptr(),
                1,
                &timeout,
            )
        };
        if ready > 0 {
            return Ok(());
        }
        if ready == 0 {
            anyhow::bail!("previous Loopflow server did not exit within 10s");
        }
        let error = io::Error::last_os_error();
        if error.kind() != io::ErrorKind::Interrupted {
            return Err(error.into());
        }
    }
}

/// Only the server accepting connections on this Machine's private socket.
/// Pin the process before checking its arguments and socket again: a reused
/// numeric pid must never receive the signal.
#[cfg(target_os = "linux")]
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
