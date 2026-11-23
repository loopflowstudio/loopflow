use std::collections::HashMap;
use std::io::Read;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, ExitStatus, Output, Stdio};
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::{mpsc, Arc};
use std::thread;
use std::time::{Duration, Instant};

use anyhow::{anyhow, Result};

use crate::process::OsProcess;

pub const DISCORD_TOKEN_ENV: &str = "LF_DISCORD_TOKEN";

/// Bound both execution and inherited output pipes, then reap the owned group.
pub fn bounded_output(command: &mut Command, timeout: Duration) -> std::io::Result<Output> {
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
    pub fn new(pid: u32) -> Self {
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

    pub fn disarm(&self) {
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
            survivors = crate::process::OsProcess::group(pgid)
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
        crate::process::OsProcess::describe(&survivors)
    ))
}

#[cfg(not(unix))]
pub(crate) fn terminate_process_group(pgid: u32) -> Result<()> {
    Err(anyhow!(
        "process group {pgid} cannot be signaled on this platform"
    ))
}

pub mod detached;
pub mod foreground;
#[cfg(test)]
mod installation_tests;
mod lifeline;
pub use lifeline::{
    agent_process_lifeline_path, hold_agent_process_lifeline, inherit_lifelines,
    release_agent_lifeline, spawn_agent_process, spawn_native_agent_process, WATCHDOG,
};

pub(crate) fn current_process_group_id() -> Option<u32> {
    // SAFETY: getpgrp has no preconditions and does not dereference memory.
    let process_group = unsafe { libc::getpgrp() };
    u32::try_from(process_group).ok().filter(|id| *id > 1)
}

/// Resolve a stable installed entry, the Desktop helper running this process,
/// or an explicit private-Home executable.
/// Keep symlinks intact so later launches pick up installation changes.
pub fn resolve_lf_binary() -> Result<PathBuf> {
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
pub fn execution_context() -> Result<loopflow_storage::child::ChildExecutionContext> {
    Ok(loopflow_storage::child::ChildExecutionContext {
        lf_bin: resolve_lf_binary()?,
        lf_home: crate::store::lf_home_dir(),
    })
}

/// Where the tools lf starts live (providers, `gh`, `git`, `node`). launchd,
/// systemd and an SSH command give lf a `PATH` holding none of them; a
/// terminal's usually holds all of them.
const TOOL_DIRS: [&str; 7] = [
    "~/.local/bin",
    // OpenCode's installer puts it here and nowhere else.
    "~/.opencode/bin",
    "/opt/homebrew/bin",
    "/opt/homebrew/sbin",
    "/usr/local/bin",
    "/usr/local/sbin",
    "~/.cargo/bin",
];

/// `TOOL_DIRS` with `~/` under `home`.
pub fn tool_dirs(home: &Path) -> [String; TOOL_DIRS.len()] {
    TOOL_DIRS.map(|entry| match entry.strip_prefix("~/") {
        Some(rest) => home.join(rest).to_string_lossy().into_owned(),
        None => entry.to_owned(),
    })
}

/// `path` with each tool directory it lacks appended. Appending only resolves
/// what `path` could not, so a terminal's own order still wins.
pub fn tool_path(path: &str, home: &Path) -> String {
    let mut entries: Vec<String> = path
        .split(':')
        .filter(|entry| !entry.is_empty())
        .map(str::to_owned)
        .collect();
    for entry in tool_dirs(home) {
        if !entries.contains(&entry) {
            entries.push(entry);
        }
    }
    entries.join(":")
}

/// Give this process, and everything it starts, the tool directories. Call
/// first in `main`, while the process has one thread.
pub fn repair_path() {
    let Some(home) = dirs::home_dir() else {
        return;
    };
    let path = match std::env::var("PATH") {
        Ok(path) if !path.is_empty() => path,
        // Not ours to rewrite: joining it as text would drop its entries.
        Err(std::env::VarError::NotUnicode(_)) => return,
        // With none set the OS searches these; setting one must not lose them.
        _ => "/usr/bin:/bin".to_owned(),
    };
    std::env::set_var("PATH", tool_path(&path, &home));
}

pub fn which_on_path(name: &Path) -> Option<PathBuf> {
    which_in(name, &std::env::var_os("PATH")?)
}

/// The executable `command` would start, resolved on the `PATH` it will run
/// with, as spawning it resolves it.
pub(crate) fn which_command(command: &std::process::Command) -> Option<PathBuf> {
    let path = match command.get_envs().find(|(name, _)| *name == "PATH") {
        Some((_, path)) => path.map(std::ffi::OsStr::to_os_string),
        None => std::env::var_os("PATH"),
    };
    which_in(Path::new(command.get_program()), &path?)
}

fn which_in(name: &Path, path: &std::ffi::OsStr) -> Option<PathBuf> {
    if name.components().count() > 1 {
        return executable_file(name).then(|| name.to_path_buf());
    }
    std::env::split_paths(path)
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
pub fn exit_reason(stderr: &str) -> String {
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
    fn a_program_is_found_only_as_an_executable_file() {
        use std::os::unix::fs::{symlink, PermissionsExt};
        let found = |path: &std::path::Path| super::which_on_path(path).is_some();

        let root = tempfile::tempdir().unwrap();
        let provider = root.path().join("provider");
        std::fs::write(&provider, "#!/bin/sh\nexit 1\n").unwrap();
        std::fs::set_permissions(&provider, std::fs::Permissions::from_mode(0o644)).unwrap();
        assert!(!found(&provider));
        std::fs::set_permissions(&provider, std::fs::Permissions::from_mode(0o755)).unwrap();
        assert!(found(&provider));
        let linked = root.path().join("linked-provider");
        symlink(&provider, &linked).unwrap();
        assert!(found(&linked));
        std::fs::remove_file(provider).unwrap();
        assert!(!found(&linked));
        assert!(!found(root.path()));
    }

    #[test]
    fn a_bare_path_gains_the_tool_directories_after_its_own() {
        let home = std::path::Path::new("/Users/maya");
        let path = super::tool_path("/usr/bin:/bin:/opt/homebrew/bin", home);
        let entries: Vec<_> = path.split(':').collect();
        assert_eq!(entries[..3], ["/usr/bin", "/bin", "/opt/homebrew/bin"]);
        assert_eq!(entries[3], "/Users/maya/.local/bin");
        assert!(entries.contains(&"/Users/maya/.opencode/bin"));
        assert!(entries.contains(&"/Users/maya/.cargo/bin"));
        assert_eq!(
            entries
                .iter()
                .filter(|e| **e == "/opt/homebrew/bin")
                .count(),
            1
        );
        assert_eq!(super::tool_path(&path, home), path);
    }

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

/// The terminal holder created the shell's OS session before launch.
pub(crate) fn terminate_shell_jobs(session: u32) -> std::io::Result<()> {
    if session <= 1 {
        return Ok(());
    }
    terminate_session_jobs(
        session,
        OsProcess::session(session)?,
        &[(libc::SIGKILL, Duration::from_secs(3))],
    )
}

/// A worker is the leader of the OS session created before its provider execs.
pub(crate) fn terminate_worker_jobs(pid: u32, started: i64) -> Result<()> {
    let members = OsProcess::session(pid)?;
    anyhow::ensure!(
        members.is_empty() || members.iter().any(|p| p.matches_start(pid, started)),
        "AgentProcess {pid} no longer owns the OS session"
    );
    terminate_session_jobs(
        pid,
        members,
        &[
            (libc::SIGTERM, TERMINATE_GRACE),
            (libc::SIGKILL, KILL_GRACE),
        ],
    )?;
    Ok(())
}

/// The first reading belongs to a verified session. Carry identities across
/// readings so a recycled session number grants no authority over new jobs.
fn terminate_session_jobs(
    session: u32,
    mut members: Vec<OsProcess>,
    signals: &[(i32, Duration)],
) -> std::io::Result<()> {
    if members.is_empty() {
        return Ok(());
    }
    for &(signal, grace) in signals {
        let deadline = Instant::now() + grace;
        let mut signaled = HashMap::new();
        loop {
            for member in &members {
                if !signaled
                    .get(&member.pid)
                    .is_some_and(|birth| member.matches_start(member.pid, *birth))
                {
                    signal_session_job(session, member, signal)?;
                    signaled.insert(member.pid, member.started_at);
                }
            }
            let survivors = OsProcess::session(session)?;
            if survivors.is_empty() {
                return Ok(());
            }
            // At least one member must still pin the original session ID.
            if !survivors.iter().any(|p| {
                signaled
                    .get(&p.pid)
                    .is_some_and(|birth| p.matches_start(p.pid, *birth))
            }) {
                return Err(std::io::Error::other(format!(
                    "cannot establish ownership of remaining jobs in OS session {session}: {}",
                    OsProcess::describe(&survivors)
                )));
            }
            members = survivors;
            if Instant::now() >= deadline {
                break;
            }
            thread::sleep(Duration::from_millis(25));
        }
    }
    Err(std::io::Error::new(
        std::io::ErrorKind::TimedOut,
        format!(
            "jobs in OS session {session} have not settled: {}",
            OsProcess::describe(&members)
        ),
    ))
}

fn signal_session_job(session: u32, member: &OsProcess, signal: i32) -> std::io::Result<()> {
    if !OsProcess::observe(member.pid)?
        .is_some_and(|p| p.matches_start(member.pid, member.started_at))
        || OsProcess::session_id(member.pid)? != Some(session)
    {
        return Ok(());
    }
    // SAFETY: this positive PID still has the captured birth and session.
    if unsafe { libc::kill(member.pid as i32, signal) } == -1 {
        let error = std::io::Error::last_os_error();
        if error.raw_os_error() != Some(libc::ESRCH) {
            return Err(error);
        }
    }
    Ok(())
}

#[cfg(test)]
mod terminal_jobs_tests {
    use super::{terminate_shell_jobs, ProcessGroupGuard};
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
        terminate_shell_jobs(shell.id()).unwrap();
        shell.wait().unwrap();
        for pid in pids {
            assert!(crate::process::OsProcess::read(pid)
                .unwrap()
                .is_none_or(|p| p.has_exited()));
        }
        assert!(unrelated.try_wait().unwrap().is_none());
        independent.terminate();
        unrelated.wait().unwrap();
    }
}
