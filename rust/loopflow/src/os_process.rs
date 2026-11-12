use std::io::Read;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, ExitStatus, Output, Stdio};
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::{mpsc, Arc};
use std::thread;
use std::time::{Duration, Instant};

use anyhow::{anyhow, Context, Result};

pub(crate) const DISCORD_TOKEN_ENV: &str = "LF_DISCORD_TOKEN";

/// Bound both execution and inherited output pipes, then reap the owned group.
pub(crate) fn bounded_output(command: &mut Command, timeout: Duration) -> std::io::Result<Output> {
    #[cfg(unix)]
    {
        use std::os::unix::process::CommandExt;
        command.process_group(0);
    }
    let mut child = command
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()?;
    let group = ProcessGroupGuard::new(child.id());
    let deadline = Instant::now() + timeout;
    let (sender, receiver) = mpsc::channel();
    let stdout = child.stdout.take().expect("stdout was piped");
    let stderr = child.stderr.take().expect("stderr was piped");
    for (index, mut pipe) in [
        (0, Box::new(stdout) as Box<dyn Read + Send>),
        (1, Box::new(stderr)),
    ] {
        let sender = sender.clone();
        thread::spawn(move || {
            let mut bytes = Vec::new();
            let result = pipe.read_to_end(&mut bytes).map(|_| bytes);
            let _ = sender.send((index, result));
        });
    }
    drop(sender);
    let result = (|| {
        let mut streams = [None, None];
        let mut status = None;
        loop {
            while let Ok((index, bytes)) = receiver.try_recv() {
                streams[index] = Some(bytes?);
            }
            if status.is_none() {
                status = child.try_wait()?;
            }
            if let Some(status) = status {
                if streams.iter().all(Option::is_some) {
                    return Ok(Output {
                        status,
                        stdout: streams[0].take().expect("stdout completed"),
                        stderr: streams[1].take().expect("stderr completed"),
                    });
                }
            }
            if Instant::now() >= deadline {
                return Err(std::io::Error::new(
                    std::io::ErrorKind::TimedOut,
                    "command inspection deadline exceeded",
                ));
            }
            thread::sleep(Duration::from_millis(10));
        }
    })();
    // Kill the owned group even if its leader has exited but descendants retain pipes.
    group.terminate();
    let cleanup_deadline = Instant::now() + Duration::from_secs(1);
    while child.try_wait()?.is_none() {
        if Instant::now() >= cleanup_deadline {
            // Retain a reaper if the OS cannot settle the killed child promptly.
            thread::spawn(move || {
                let _ = child.wait();
            });
            return Err(std::io::Error::new(
                std::io::ErrorKind::TimedOut,
                "command child cleanup deadline exceeded",
            ));
        }
        thread::sleep(Duration::from_millis(10));
    }
    result
}

/// Wait for an owned child, returning its exit status and whether it timed out.
pub(crate) fn wait_for_exit(
    child: &mut Child,
    timeout: Option<Duration>,
    mut on_tick: impl FnMut(),
) -> std::io::Result<(ExitStatus, bool)> {
    let timeout_at = timeout.map(|value| Instant::now() + value);
    loop {
        if let Some(status) = child.try_wait()? {
            return Ok((status, false));
        }
        if timeout_at.is_some_and(|value| Instant::now() >= value) {
            let _ = child.kill();
            return Ok((child.wait()?, true));
        }
        on_tick();
        std::thread::sleep(Duration::from_millis(50));
    }
}

/// Owns a child process group until its work is known to be complete.
///
/// The child must be spawned into a fresh process group whose id is its pid.
/// Interrupt cleanup is registered because the CLI signal handler exits the
/// process without running Rust destructors.
#[derive(Debug)]
pub(crate) struct ProcessGroupGuard {
    pid: Arc<AtomicU32>,
}

impl ProcessGroupGuard {
    pub(crate) fn new(pid: u32) -> Self {
        assert!(pid > 1, "owned process group must have a child pid");
        let pid = Arc::new(AtomicU32::new(pid));
        let interrupt_pid = Arc::clone(&pid);
        crate::agent::register_interrupt_cleanup(move || {
            kill_process_group(interrupt_pid.swap(0, Ordering::AcqRel));
        });
        Self { pid }
    }

    pub(crate) fn terminate(&self) {
        kill_process_group(self.pid.swap(0, Ordering::AcqRel));
    }

    pub(crate) fn disarm(&self) {
        self.pid.store(0, Ordering::Release);
    }
}

impl Drop for ProcessGroupGuard {
    fn drop(&mut self) {
        self.terminate();
    }
}

/// SIGKILL every member of a process group the caller created. A provider
/// entry point is often a shim whose real server is a grandchild in the same
/// group; killing only the direct child leaves that server running.
pub(crate) fn kill_process_group(pid: u32) {
    if pid <= 1 {
        return;
    }

    #[cfg(unix)]
    if let Ok(pid) = i32::try_from(pid) {
        // SAFETY: callers spawn the child into a fresh process group whose id
        // is its pid; a negative pid targets only that owned group.
        unsafe {
            libc::kill(-pid, libc::SIGKILL);
        }
    }
    #[cfg(not(unix))]
    crate::platform::kill_process(pid);
}

/// SIGTERM gets a short grace; SIGKILL waits for the kernel to finish tearing
/// the group down, which takes seconds for large address spaces under load.
const TERMINATE_GRACE: Duration = Duration::from_secs(2);
const KILL_GRACE: Duration = Duration::from_secs(10);

/// SIGTERM a process group, wait, then SIGKILL the stragglers. Succeeds when
/// every member has exited: zombies await their own parent's wait and cannot
/// keep an AgentProcess alive. The caller proves this is an owned group.
#[cfg(unix)]
pub(crate) fn terminate_process_group(pgid: u32) -> Result<()> {
    let group = i32::try_from(pgid)
        .ok()
        .filter(|_| pgid > 1 && current_process_group_id() != Some(pgid))
        .ok_or_else(|| anyhow!("process group {pgid} is not an owned group"))?;
    let mut survivors = Vec::new();
    for (signal, grace) in [
        (libc::SIGTERM, TERMINATE_GRACE),
        (libc::SIGKILL, KILL_GRACE),
    ] {
        // SAFETY: a negative pid signals only the named process group.
        if unsafe { libc::kill(-group, signal) } != 0 {
            let error = std::io::Error::last_os_error();
            if error.raw_os_error() == Some(libc::ESRCH) {
                return Ok(());
            }
            return Err(anyhow!("cannot signal process group {pgid}: {error}"));
        }
        let deadline = Instant::now() + grace;
        // Observe before checking the deadline so the last look follows it.
        loop {
            // SAFETY: WNOHANG collects the leader only if it is this
            // process's exited child; otherwise it fails without effect.
            unsafe {
                libc::waitpid(group, std::ptr::null_mut(), libc::WNOHANG);
            }
            survivors = crate::journal::OsProcess::group(pgid)
                .map_err(|error| anyhow!("cannot observe process group {pgid}: {error}"))?;
            if survivors.is_empty() {
                return Ok(());
            }
            if Instant::now() >= deadline {
                break;
            }
            std::thread::sleep(Duration::from_millis(25));
        }
    }
    Err(anyhow!(
        "process group {pgid} survives SIGKILL: {}",
        crate::journal::OsProcess::describe(&survivors)
    ))
}

#[cfg(not(unix))]
pub(crate) fn terminate_process_group(pgid: u32) -> Result<()> {
    Err(anyhow!(
        "process group {pgid} cannot be signaled on this platform"
    ))
}

pub(crate) mod detached;
pub(crate) mod foreground;
#[cfg(test)]
mod installation_tests;
mod lifeline;
pub(crate) use lifeline::{
    agent_process_lifeline_path, hold_agent_process_lifeline, inherit_lifelines,
    release_agent_lifeline, spawn_agent_process, spawn_native_agent_process, WATCHDOG,
};

pub(crate) fn current_process_group_id() -> Option<u32> {
    // SAFETY: getpgrp has no preconditions and does not dereference memory.
    let process_group = unsafe { libc::getpgrp() };
    u32::try_from(process_group).ok().filter(|id| *id > 1)
}

/// `lf __context-block --moment <moment>` in its own OS session, so it outlives
/// the provider hook that starts it and that hook's timeout.
pub(crate) fn detached_context_block(moment: &str) -> Result<Command> {
    use std::os::unix::process::CommandExt;

    let mut command = Command::new(resolve_lf_binary()?);
    command
        .args(["__context-block", "--moment", moment])
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null());
    // SAFETY: only an async-signal-safe syscall runs between fork and exec.
    unsafe {
        command.pre_exec(|| {
            if libc::setsid() < 0 {
                return Err(std::io::Error::last_os_error());
            }
            Ok(())
        });
    }
    Ok(command)
}

/// Resolve a stable installed entry, the Desktop helper running this process,
/// or an explicit private-Home executable.
/// Keep symlinks intact so later launches pick up installation changes.
pub(crate) fn resolve_lf_binary() -> Result<PathBuf> {
    let candidate = if crate::store::custom_home_selected() {
        std::env::var_os("LF_BIN")
            .filter(|value| !value.is_empty())
            .or_else(|| std::env::var_os("CARGO_BIN_EXE_lf"))
            .map(PathBuf::from)
    } else if let Some(helper) = crate::installation::desktop_helper() {
        Some(helper)
    } else {
        // The gate selects and verifies the installed artifact when executed.
        let gate = crate::installation::entry_gate_path(
            &crate::installation::root()?,
            &crate::installation::ArtifactRole::Cli,
        )?;
        let installed = crate::installation::account_home()?.join(".local/bin/lf");
        [gate, installed].into_iter().find(|path| path.is_file())
    };
    let candidate = candidate
        .or_else(|| {
            let current = std::env::current_exe().ok()?;
            if current.file_name().is_some_and(|name| name == "lf") {
                return Some(current);
            }
            let sibling = current.parent()?.join("lf");
            sibling.is_file().then_some(sibling)
        })
        .unwrap_or_else(|| PathBuf::from("lf"));
    if candidate.is_absolute() {
        anyhow::ensure!(
            candidate.is_file(),
            "lf binary {} does not exist",
            candidate.display()
        );
        return Ok(candidate);
    }
    which_on_path(&candidate).ok_or_else(|| {
        anyhow!(
            "cannot resolve an executable path for `{}`",
            candidate.display()
        )
    })
}

/// Capture the resolved CLI and Machine for a provider child.
pub(crate) fn execution_context() -> Result<crate::child::ChildExecutionContext> {
    Ok(crate::child::ChildExecutionContext {
        lf_bin: resolve_lf_binary()?,
        lf_home: crate::store::lf_home_dir(),
    })
}

pub(crate) fn which_on_path(name: &Path) -> Option<PathBuf> {
    if name.components().count() > 1 {
        return executable_file(name).then(|| name.to_path_buf());
    }
    let path = std::env::var_os("PATH")?;
    std::env::split_paths(&path)
        .map(|dir| dir.join(name))
        .find(|candidate| executable_file(candidate))
}

fn executable_file(path: &Path) -> bool {
    let Ok(metadata) = path.metadata() else {
        return false;
    };
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        if metadata.permissions().mode() & 0o111 == 0 {
            return false;
        }
    }
    metadata.is_file()
}

/// A failed process's own last word, formatted to follow its exit status.
pub(crate) fn exit_reason(stderr: &str) -> String {
    stderr
        .lines()
        .rev()
        .map(str::trim)
        .find(|line| !line.is_empty())
        .map(|line| format!(": {line}"))
        .unwrap_or_default()
}

pub(crate) fn shell_escape(value: &str) -> String {
    let escaped = value.replace('\'', "'\\''");
    format!("'{escaped}'")
}

#[cfg(test)]
mod tests {

    use std::os::unix::process::CommandExt;
    use std::process::Command;
    use std::time::Duration;

    use super::terminate_process_group;

    #[test]
    fn private_home_keeps_explicit_binary_selection() {
        let _lock = loopflow_test_support::env_lock().lock().unwrap();
        let _env = crate::test_ambient::EnvGuard::clear(&["LF_HOME", "LF_BIN"]);
        let home = tempfile::tempdir().unwrap();
        std::env::set_var("LF_HOME", home.path());
        let binary = std::env::current_exe().unwrap();
        std::env::set_var("LF_BIN", &binary);
        assert_eq!(super::resolve_lf_binary().unwrap(), binary);
        crate::installation::canonicalize_selected_home().unwrap();
        std::env::set_var("LF_BIN", "/missing/selected/lf");
        assert!(super::resolve_lf_binary().is_err());
    }

    #[test]
    fn terminating_a_group_reaches_a_grandchild_that_outlived_its_leader() {
        let dir = tempfile::tempdir().unwrap();
        let flag = dir.path().join("survived");
        let mut leader = Command::new("sh")
            .arg("-c")
            .arg(format!("(sleep 1 && touch {}) &", flag.display()))
            .process_group(0)
            .spawn()
            .unwrap();
        let group = leader.id();
        leader.wait().unwrap();

        terminate_process_group(group).unwrap();

        std::thread::sleep(Duration::from_millis(1500));
        assert!(!flag.exists(), "grandchild outlived its group");
    }

}

/// Ordinary jobs belong to the fresh OS session created for a terminal child.
/// Job control changes process groups, so killing only the leader's group leaks
/// background/stopped jobs. Daemons that create a different session are separate.
#[derive(Debug, Clone)]
pub(crate) struct TerminalJobs {
    session: Arc<std::sync::Mutex<u32>>,
}

impl TerminalJobs {
    pub(crate) fn new(session: u32) -> Self {
        Self {
            session: Arc::new(std::sync::Mutex::new(session)),
        }
    }

    pub(crate) fn terminate(&self) -> std::io::Result<()> {
        let mut session = self.session.lock().expect("owned shell session lock");
        terminate_terminal_jobs(*session)?;
        *session = 0;
        Ok(())
    }
}

impl Drop for TerminalJobs {
    fn drop(&mut self) {
        let _ = self.terminate();
    }
}

fn terminate_terminal_jobs(session: u32) -> std::io::Result<()> {
    if session <= 1 {
        return Ok(());
    }
    let deadline = Instant::now() + Duration::from_secs(3);
    loop {
        let members = crate::journal::OsProcess::session(session)?;
        if members.is_empty() {
            return Ok(());
        }
        for process in members {
            // SAFETY: recheck session membership immediately before signaling;
            // this never targets by cwd or another owner's process group.
            unsafe {
                if libc::getsid(process.pid as i32) == session as i32 {
                    libc::kill(process.pid as i32, libc::SIGKILL);
                }
            }
        }
        if Instant::now() >= deadline {
            return Err(std::io::Error::new(
                std::io::ErrorKind::TimedOut,
                "Shell jobs have not settled",
            ));
        }
        thread::sleep(Duration::from_millis(20));
    }
}

#[cfg(test)]
mod terminal_jobs_tests {
    use super::{ProcessGroupGuard, TerminalJobs};
    use std::os::unix::process::CommandExt;
    use std::process::{Command, Stdio};
    use std::time::{Duration, Instant};

    #[test]
    fn shell_close_settles_background_and_stopped_jobs_without_touching_other_owners() {
        let root = tempfile::tempdir().unwrap();
        let receipt = root.path().join("jobs");
        let mut unrelated = Command::new("sleep")
            .arg("30")
            .process_group(0)
            .spawn()
            .unwrap();
        let independent = ProcessGroupGuard::new(unrelated.id());
        let mut command = Command::new("/bin/bash");
        command.args(["-c", "set -m; sleep 30 & a=$!; sleep 30 & b=$!; kill -STOP $b; printf '%s %s' $a $b > \"$1\"; wait", "fixture"])
            .arg(&receipt).stdin(Stdio::null()).stdout(Stdio::null()).stderr(Stdio::null());
        // SAFETY: this syscall only creates a fresh OS session in the child.
        unsafe {
            command.pre_exec(|| {
                if libc::setsid() < 0 {
                    return Err(std::io::Error::last_os_error());
                }
                Ok(())
            });
        }
        let mut shell = command.spawn().unwrap();
        let jobs = TerminalJobs::new(shell.id());
        let deadline = Instant::now() + Duration::from_secs(3);
        let pids = loop {
            let pids = std::fs::read_to_string(&receipt)
                .unwrap_or_default()
                .split_whitespace()
                .filter_map(|p| p.parse::<u32>().ok())
                .collect::<Vec<_>>();
            if pids.len() == 2 {
                break pids;
            }
            assert!(Instant::now() < deadline, "shell did not report its jobs");
            std::thread::sleep(Duration::from_millis(10));
        };
        jobs.terminate().unwrap();
        shell.wait().unwrap();
        for pid in pids {
            assert!(crate::journal::OsProcess::read(pid)
                .unwrap()
                .is_none_or(|p| p.has_exited()));
        }
        assert!(unrelated.try_wait().unwrap().is_none());
        independent.terminate();
        unrelated.wait().unwrap();
    }
}
