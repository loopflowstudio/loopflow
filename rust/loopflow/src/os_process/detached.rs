//! Headless work that outlives its caller: a plain `lf` process in its own
//! session, with no owner, holder or terminal between it and its launcher.
//!
//! The launcher hands the child one pipe, named by `LF_LAUNCHER_FD`. The child
//! writes its LfProcess ID on that pipe once it is admitted, and `released`
//! when it lets its launcher go (a Task run whose Graph registered, a ship
//! that announced itself). The pipe closes when the child exits. A pipe that
//! closes before the ID arrives leaves delivery unknown; nothing is started
//! again.

use std::fs::File;
use std::io::{self, BufRead, BufReader, Read, Write};
use std::os::fd::{AsRawFd, FromRawFd, RawFd};
use std::os::unix::process::CommandExt;
use std::path::Path;
use std::process::{Child, Command, Stdio};
use std::sync::{mpsc, Mutex, OnceLock};
use std::time::Duration;

use anyhow::{Context, Result};

use crate::id::LfProcessId;

const LAUNCHER_ENV: &str = "LF_LAUNCHER_FD";
const RELEASED: &str = "released";

/// How often a waiting launcher copies new output.
const TAIL_INTERVAL: Duration = Duration::from_millis(50);

/// This process's end of its launcher's pipe, until it lets the launcher go.
static LAUNCHER: OnceLock<Mutex<Option<File>>> = OnceLock::new();

/// Take the launcher's pipe. Nested processes never inherit it.
pub(crate) fn initialize() {
    let Some(fd) = std::env::var(LAUNCHER_ENV)
        .ok()
        .and_then(|fd| fd.parse::<RawFd>().ok())
    else {
        std::env::remove_var(LAUNCHER_ENV);
        return;
    };
    std::env::remove_var(LAUNCHER_ENV);
    // SAFETY: fcntl on a number we were handed; an invalid one fails and is ignored.
    if unsafe { libc::fcntl(fd, libc::F_SETFD, libc::FD_CLOEXEC) } < 0 {
        return;
    }
    // SAFETY: the launcher opened this descriptor for this process alone.
    let file = unsafe { File::from_raw_fd(fd) };
    let _ = LAUNCHER.set(Mutex::new(Some(file)));
}

/// Whether a launcher started this process detached and still waits on it.
pub(crate) fn launched() -> bool {
    LAUNCHER
        .get()
        .is_some_and(|launcher| launcher.lock().expect("launcher lock").is_some())
}

/// Tell the launcher which Process this is. A launcher that stopped listening
/// changes nothing here.
pub(crate) fn acknowledge(id: &LfProcessId) {
    tell(&format!("{id}\n"), false);
}

/// Let the launcher go; it stops waiting and returns success.
pub(crate) fn release() {
    tell(&format!("{RELEASED}\n"), true);
}

fn tell(line: &str, last: bool) {
    let Some(launcher) = LAUNCHER.get() else {
        return;
    };
    let mut launcher = launcher.lock().expect("launcher lock");
    if let Some(pipe) = launcher.as_mut() {
        let _ = pipe.write_all(line.as_bytes());
    }
    if last {
        *launcher = None;
    }
}

/// A started detached process and its pipe.
#[derive(Debug)]
pub(crate) struct Launched {
    child: Child,
    lines: BufReader<io::PipeReader>,
}

/// Start `command` in its own session with stdin from `/dev/null`, dropping
/// every descriptor this process inherited except those `inherit` hands on.
/// The caller sets its output.
pub(crate) fn spawn(mut command: Command, inherit: &dyn Fn(&mut Command)) -> Result<Launched> {
    let (reader, writer) = io::pipe()?;
    let pipe = writer.as_raw_fd();
    command
        .stdin(Stdio::null())
        .env(LAUNCHER_ENV, pipe.to_string());
    let descriptors = inherited_descriptors();
    // SAFETY: only async-signal-safe syscalls are used between fork and exec.
    unsafe {
        command.pre_exec(move || {
            if libc::setsid() < 0 {
                return Err(io::Error::last_os_error());
            }
            for fd in &descriptors {
                libc::fcntl(*fd, libc::F_SETFD, libc::FD_CLOEXEC);
            }
            if libc::fcntl(pipe, libc::F_SETFD, 0) < 0 {
                return Err(io::Error::last_os_error());
            }
            Ok(())
        });
    }
    // After the sweep, so descriptors the caller hands on stay open.
    inherit(&mut command);
    let child = command.spawn()?;
    drop(command);
    drop(writer);
    Ok(Launched {
        child,
        lines: BufReader::new(reader),
    })
}

impl Launched {
    /// The child's LfProcess, once it is admitted. Its child is reaped in the
    /// background; this process no longer waits on it.
    pub(crate) fn acknowledged(mut self) -> Result<LfProcessId> {
        let mut line = String::new();
        let read = self.lines.read_line(&mut line);
        let mut child = self.child;
        std::thread::spawn(move || {
            let _ = child.wait();
        });
        if read.unwrap_or(0) == 0 {
            anyhow::bail!(
                "The process exited before acknowledging execution; delivery is unknown, do not replay"
            );
        }
        LfProcessId::parse(line.trim())
            .with_context(|| format!("The process acknowledged {:?}, not a Process", line.trim()))
    }
}

/// Start `lf` again with `argv` in `cwd`, detached, and print its output here
/// until it exits or lets this process go. Killing this process leaves the
/// work running. Returns the exit code to report.
pub(crate) fn run(argv: &[String], cwd: &Path, inherit: &dyn Fn(&mut Command)) -> Result<u8> {
    let directory = crate::transport::private_directory()?;
    let stdout = tempfile::NamedTempFile::new_in(&directory)?;
    let stderr = tempfile::NamedTempFile::new_in(&directory)?;
    let mut tails = [
        (stdout.reopen()?, Output::Stdout),
        (stderr.reopen()?, Output::Stderr),
    ];
    let mut command = Command::new(&argv[0]);
    command
        .args(&argv[1..])
        .current_dir(cwd)
        .stdout(stdout.as_file().try_clone()?)
        .stderr(stderr.as_file().try_clone()?);
    let launched = spawn(command, inherit)?;
    // The open descriptors keep the output; no file is left behind.
    drop((stdout, stderr));
    let Launched { mut child, lines } = launched;
    let (signal, signals) = mpsc::channel();
    std::thread::spawn(move || {
        for line in lines.lines() {
            let Ok(line) = line else { break };
            if line == RELEASED {
                let _ = signal.send(());
            }
        }
    });
    loop {
        let released = signals.try_recv().is_ok();
        let exited = child.try_wait()?;
        for (file, output) in &mut tails {
            output.copy(file)?;
        }
        if released {
            return Ok(0);
        }
        if let Some(status) = exited {
            return Ok(crate::transport::provider_exit_code(status));
        }
        std::thread::sleep(TAIL_INTERVAL);
    }
}

#[derive(Debug, Clone, Copy)]
enum Output {
    Stdout,
    Stderr,
}

impl Output {
    /// Copy what the child wrote since the last copy.
    fn copy(self, file: &mut File) -> io::Result<()> {
        let mut bytes = Vec::new();
        file.read_to_end(&mut bytes)?;
        if bytes.is_empty() {
            return Ok(());
        }
        match self {
            Self::Stdout => {
                let mut out = io::stdout().lock();
                out.write_all(&bytes)?;
                out.flush()
            }
            Self::Stderr => io::stderr().lock().write_all(&bytes),
        }
    }
}

/// Descriptors above stderr, such as a terminal host's pipes to its other
/// children that this process inherited. Detached work outlives that host, so
/// it must not hold them: a reader waiting for the host's end to close would
/// wait for as long as the work runs. Release locks and worktree leases this
/// process inherited stay: a release's surviving child keeps its target and
/// checkout exclusive after the release controller dies. Marking an already
/// close-on-exec or closed number (the listing's own) again is harmless.
pub(crate) fn inherited_descriptors() -> Vec<RawFd> {
    let Ok(entries) = std::fs::read_dir("/dev/fd") else {
        return Vec::new();
    };
    let held: Vec<RawFd> = ["LF_RELEASE_LOCK_FD", "LF_WORKTREE_LEASE_FD"]
        .iter()
        .filter_map(|name| std::env::var(name).ok()?.parse().ok())
        .collect();
    entries
        .filter_map(|entry| entry.ok()?.file_name().to_str()?.parse().ok())
        .filter(|fd| *fd > libc::STDERR_FILENO && !held.contains(fd))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn shell(script: &str) -> Command {
        let mut command = Command::new("/bin/sh");
        command.args(["-c", script]);
        command
    }

    #[test]
    fn acknowledgement_names_the_process_the_child_reports() {
        let id = LfProcessId::new();
        let launched = spawn(
            shell(&format!("echo {id} >&$LF_LAUNCHER_FD; sleep 0.1")),
            &|_| {},
        )
        .unwrap();
        assert_eq!(launched.acknowledged().unwrap(), id);
    }

    #[test]
    fn a_child_that_exits_unacknowledged_leaves_delivery_unknown() {
        let error = spawn(shell("exit 3"), &|_| {})
            .unwrap()
            .acknowledged()
            .unwrap_err();
        assert!(format!("{error:#}").contains("do not replay"), "{error:#}");
    }

    #[test]
    fn the_child_leads_its_own_process_group_without_stdin() {
        let id = LfProcessId::new();
        let launched = spawn(
            shell(&format!(
                "[ \"$(ps -o pgid= -p $$ | tr -d ' ')\" = \"$$\" ] || exit 1; \
             read line && exit 1; echo {id} >&$LF_LAUNCHER_FD"
            )),
            &|_| {},
        )
        .unwrap();
        assert_eq!(launched.acknowledged().unwrap(), id);
    }
}
