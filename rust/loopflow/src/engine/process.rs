use std::path::{Path, PathBuf};

use anyhow::{anyhow, Result};

pub(crate) fn current_process_group_id() -> Option<u32> {
    // SAFETY: getpgrp has no preconditions and does not dereference memory.
    let process_group = unsafe { libc::getpgrp() };
    u32::try_from(process_group).ok().filter(|id| *id > 1)
}

pub(crate) fn resolve_lf_binary() -> PathBuf {
    if let Some(path) = select_binary_override(
        crate::build_info::provenance(),
        std::env::var_os(crate::store::CONTROL_BIN_ENV),
        std::env::var_os("LF_BIN"),
    ) {
        return path;
    }

    if let Ok(path) = std::env::var("CARGO_BIN_EXE_lf") {
        let trimmed = path.trim();
        if !trimmed.is_empty() {
            return PathBuf::from(trimmed);
        }
    }

    if let Ok(current) = std::env::current_exe() {
        if current
            .file_name()
            .and_then(|name| name.to_str())
            .is_some_and(|name| name == "lf")
        {
            return current;
        }
        if let Some(parent) = current.parent() {
            let sibling = parent.join("lf");
            if sibling.exists() {
                return sibling;
            }
        }
    }

    PathBuf::from("lf")
}

fn select_binary_override(
    provenance: crate::build_info::BuildProvenance,
    control: Option<std::ffi::OsString>,
    ordinary: Option<std::ffi::OsString>,
) -> Option<PathBuf> {
    let selected = if provenance.is_release() {
        control.or(ordinary)
    } else {
        ordinary
    }?;
    if selected.is_empty() {
        None
    } else {
        Some(PathBuf::from(selected))
    }
}

/// The launch-boundary counterpart to [`select_binary_override`]: it takes only
/// the ordinary `LF_BIN` value and has no control input at all. The current
/// Home is never resolved through `LF_CONTROL_BIN`, in any provenance — that
/// pin is the historical binary a legacy body must stop relaunching through.
fn select_current_home_binary(ordinary: Option<std::ffi::OsString>) -> Option<PathBuf> {
    ordinary
        .filter(|value| !value.is_empty())
        .map(PathBuf::from)
}

/// Resolve the `lf` a Session will be pinned to: an absolute path that exists.
///
/// `resolve_lf_binary` may hand back the bare name `lf`, which a child resolves
/// against the *login* shell's PATH inside tmux — a third binary, chosen by
/// neither the Session nor its launcher. A Session that cannot name its own
/// executable is not created.
pub(crate) fn resolve_pinned_lf_binary() -> Result<PathBuf> {
    let candidate = resolve_lf_binary();
    if candidate.is_absolute() {
        return if candidate.exists() {
            Ok(candidate)
        } else {
            Err(anyhow!(
                "lf binary {} does not exist; set LF_BIN to the lf this Session should run",
                candidate.display()
            ))
        };
    }
    which_on_path(&candidate).ok_or_else(|| {
        anyhow!(
            "cannot resolve an absolute path for `{}`; set LF_BIN to the lf this Session should run",
            candidate.display()
        )
    })
}

/// Capture the current process's control context — this process's `lf`, store,
/// and `LF_HOME` — for propagating down to a vendored subprocess. In a release
/// build this honors `LF_CONTROL_*`, so a running body hands its own session's
/// context (not the machine's Home) to the provider CLI it spawns.
///
/// This is NOT the launch resolver. Use [`current_home_execution_context`] to
/// launch or relaunch a Session: launching through the control context would
/// perpetuate the historical binary a legacy body was created with.
pub(crate) fn pinned_execution_context() -> Result<crate::child_session::ChildExecutionContext> {
    let db_path = crate::store::database_path_from_env()
        .map_err(|error| anyhow!("cannot resolve the Session's database path: {error}"))?;
    Ok(crate::child_session::ChildExecutionContext {
        lf_bin: resolve_pinned_lf_binary()?,
        db_path,
        lf_home: crate::store::lf_home_dir(),
    })
}

/// Resolve the current Home `lf` binary, never the historical `LF_CONTROL_BIN`.
///
/// `resolve_lf_binary` prefers `LF_CONTROL_BIN` in a release build — the pin a
/// legacy body carries from whichever binary created it. Relaunching through
/// that is exactly the stranding this resolver exists to prevent, so the
/// control override is deliberately skipped: `LF_BIN` (the current Home), then
/// the installed `lf` on `PATH`, then this executable, then the bare name.
fn resolve_current_home_lf_binary() -> PathBuf {
    if let Some(bin) = select_current_home_binary(std::env::var_os("LF_BIN")) {
        return bin;
    }
    if let Ok(path) = std::env::var("CARGO_BIN_EXE_lf") {
        let trimmed = path.trim();
        if !trimmed.is_empty() {
            return PathBuf::from(trimmed);
        }
    }
    if let Some(installed) = which_on_path(Path::new("lf")) {
        return installed;
    }
    if let Ok(current) = std::env::current_exe() {
        if current
            .file_name()
            .and_then(|name| name.to_str())
            .is_some_and(|name| name == "lf")
        {
            return current;
        }
        if let Some(parent) = current.parent() {
            let sibling = parent.join("lf");
            if sibling.exists() {
                return sibling;
            }
        }
    }
    PathBuf::from("lf")
}

/// The current Home `lf`, resolved to an absolute path that exists. Mirrors
/// [`resolve_pinned_lf_binary`] but over [`resolve_current_home_lf_binary`].
fn resolve_current_home_lf_binary_checked() -> Result<PathBuf> {
    let candidate = resolve_current_home_lf_binary();
    if candidate.is_absolute() {
        return if candidate.exists() {
            Ok(candidate)
        } else {
            Err(anyhow!(
                "lf binary {} does not exist; set LF_BIN to the current Home lf",
                candidate.display()
            ))
        };
    }
    which_on_path(&candidate).ok_or_else(|| {
        anyhow!(
            "cannot resolve an absolute path for `{}`; set LF_BIN to the current Home lf",
            candidate.display()
        )
    })
}

/// Resolve the current Home execution context for launching a Session: the
/// current Home `lf`, store, and `LF_HOME`, ignoring every `LF_CONTROL_*` pin.
///
/// This is the launch/relaunch boundary resolver. A Session created under one
/// binary and resumed under another launches through the current Home — its
/// worktree, provider history, and directives are unaffected by which binary
/// first created it.
pub(crate) fn current_home_execution_context() -> Result<crate::child_session::ChildExecutionContext>
{
    let db_path = crate::store::current_home_database_path()
        .map_err(|error| anyhow!("cannot resolve the current Home database path: {error}"))?;
    Ok(crate::child_session::ChildExecutionContext {
        lf_bin: resolve_current_home_lf_binary_checked()?,
        db_path,
        lf_home: crate::store::current_home_lf_home_dir(),
    })
}

fn which_on_path(name: &Path) -> Option<PathBuf> {
    let path = std::env::var_os("PATH")?;
    std::env::split_paths(&path)
        .map(|dir| dir.join(name))
        .find(|candidate| candidate.is_file())
}

pub(crate) fn shell_escape(value: &str) -> String {
    let escaped = value.replace('\'', "'\\''");
    format!("'{escaped}'")
}

/// Whether this machine can look for tmux sessions at all.
///
/// Ask PATH, not the binary. A generic `--version` probe reports tmux as absent
/// on every machine — tmux only accepts `-V` — which silently downgrades Session
/// liveness to "unknowable" and hides processes that are actually gone.
pub(crate) fn tmux_installed() -> bool {
    which_on_path(Path::new("tmux")).is_some()
}

/// A fake `tmux` on `PATH`, so a probe's verdict is a fact about the fixture and
/// not about the host running the suite.
///
/// The production rule it serves is total: a tmux we cannot run, or one that
/// fails in a way we do not model, is [`Presence::Unprovable`] — never absence.
/// That decides a probe before the signal identity is read, so a test about a
/// process group must pin tmux or it is asserting a fact about the runner.
///
/// Shaped after `AmbientGuard`'s fake `gh` (`task/runner/ci_fix_lifecycle_tests.rs`),
/// including its rule: model an outage by **replacing** the script, never by
/// deleting it — a deleted fake falls through to the host's real tmux and lands in
/// a different branch than the one under test. [`Self::unspawnable`] needs no tmux
/// at all, so it replaces `PATH` rather than deleting out of a prepended dir.
///
/// Holds [`crate::journal::test_env_lock`] for the guard's life: a second lock
/// would serialize only against other `FakeTmux` instances and still race every
/// other `PATH` test in the crate.
#[cfg(test)]
pub(crate) struct FakeTmux {
    _lock: std::sync::MutexGuard<'static, ()>,
    _bin: tempfile::TempDir,
    previous_path: Option<std::ffi::OsString>,
}

#[cfg(test)]
impl FakeTmux {
    /// The server answers and lists no sessions: authoritative absence, read off
    /// the exit code and set membership without touching stderr prose.
    pub(crate) fn no_session() -> Self {
        Self::script("exit 0\n")
    }

    /// No server, reported the way tmux 3.6a actually reports it when the socket
    /// is gone. The wording that used to read `Unprovable` and silently disable
    /// every release on a host whose last body took the server with it.
    pub(crate) fn no_server() -> Self {
        Self::script(
            "echo 'error connecting to /tmp/tmux-501/default (No such file or directory)' >&2\n\
             exit 1\n",
        )
    }

    /// A live session named `session`.
    pub(crate) fn live_session(session: &str) -> Self {
        Self::script(&format!("echo '{session}'\nexit 0\n"))
    }

    /// No tmux anywhere on `PATH`, so the probe cannot spawn it at all.
    pub(crate) fn unspawnable() -> Self {
        Self::new(None)
    }

    fn script(body: &str) -> Self {
        Self::new(Some(body))
    }

    fn new(body: Option<&str>) -> Self {
        use std::os::unix::fs::PermissionsExt;
        let lock = crate::journal::test_env_lock();
        let bin = tempfile::TempDir::new().expect("temp bin dir");
        let previous_path = std::env::var_os("PATH");

        let path = match body {
            Some(body) => {
                let tmux = bin.path().join("tmux");
                std::fs::write(&tmux, format!("#!/bin/sh\n{body}")).expect("write fake tmux");
                let mut perms = std::fs::metadata(&tmux)
                    .expect("stat fake tmux")
                    .permissions();
                perms.set_mode(0o755);
                std::fs::set_permissions(&tmux, perms).expect("chmod fake tmux");
                // Prepend, like AmbientGuard: the rest of PATH stays usable.
                match &previous_path {
                    Some(previous) => std::ffi::OsString::from(format!(
                        "{}:{}",
                        bin.path().display(),
                        previous.to_string_lossy()
                    )),
                    None => std::ffi::OsString::from(bin.path().display().to_string()),
                }
            }
            // Nothing else on PATH, so `tmux` resolves nowhere on any host.
            None => std::ffi::OsString::from(bin.path().display().to_string()),
        };
        std::env::set_var("PATH", path);
        Self {
            _lock: lock,
            _bin: bin,
            previous_path,
        }
    }
}

#[cfg(test)]
impl Drop for FakeTmux {
    fn drop(&mut self) {
        match self.previous_path.take() {
            Some(previous) => std::env::set_var("PATH", previous),
            None => std::env::remove_var("PATH"),
        }
    }
}

pub(crate) async fn tmux_session_exists(session_name: &str) -> Result<bool> {
    // A missing session is the answer, not an error: tmux's "can't find session"
    // on stderr would otherwise scribble over a caller's own output.
    let status = tokio::process::Command::new("tmux")
        .args(["has-session", "-t", session_name])
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status()
        .await
        .map_err(|err| anyhow!("tmux session probe failed: {err}"))?;
    Ok(status.success())
}

/// Every live tmux session name, in one subprocess. The batched form of
/// [`tmux_session_exists`]: a caller checking many sessions looks each up in the
/// returned set instead of paying a `has-session` fork per name. No server
/// running means no sessions, which tmux reports as a non-zero exit — that is an
/// empty set, not an error.
pub(crate) async fn tmux_live_sessions() -> Result<std::collections::HashSet<String>> {
    let output = tokio::process::Command::new("tmux")
        .args(["list-sessions", "-F", "#{session_name}"])
        .stderr(std::process::Stdio::null())
        .output()
        .await
        .map_err(|err| anyhow!("tmux session list failed: {err}"))?;
    if !output.status.success() {
        // "no server running" / "no sessions" — nothing is live.
        return Ok(std::collections::HashSet::new());
    }
    Ok(String::from_utf8_lossy(&output.stdout)
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty())
        .map(str::to_string)
        .collect())
}

pub(crate) async fn reap_child_process(
    process: &crate::child_session::ChildProcessGeneration,
    grace: std::time::Duration,
) -> Result<()> {
    if tmux_installed() && tmux_session_exists(&process.tmux_name).await? {
        let status = tokio::process::Command::new("tmux")
            .args(["kill-session", "-t", &process.tmux_name])
            .status()
            .await
            .map_err(|error| anyhow!("failed to stop tmux body {}: {error}", process.tmux_name))?;
        if !status.success() && tmux_session_exists(&process.tmux_name).await? {
            return Err(anyhow!(
                "tmux body {} survived kill-session",
                process.tmux_name
            ));
        }
    }

    let mut identities = Vec::new();
    if let Some(group) = process.process_group_id {
        if current_process_group_id() == Some(group) {
            return Err(anyhow!(
                "refusing to reap current process group {group} for generation {}",
                process.generation
            ));
        }
        identities.push(ProcessSignalTarget::Group(group));
    }
    if let Some(pid) = process.pid {
        if pid == std::process::id() {
            return Err(anyhow!(
                "refusing to reap current process for generation {}",
                process.generation
            ));
        }
        if process.process_group_id != Some(pid) {
            identities.push(ProcessSignalTarget::Process(pid));
        }
    }
    if identities.is_empty() {
        if tmux_installed() && tmux_session_exists(&process.tmux_name).await? {
            return Err(anyhow!(
                "generation {} has no signal identity and tmux is still alive",
                process.generation
            ));
        }
        return Ok(());
    }

    for identity in &identities {
        signal_process_target(*identity, libc::SIGTERM)?;
    }
    if wait_for_process_exit(&identities, grace).await {
        return confirm_tmux_reaped(process).await;
    }
    for identity in &identities {
        if process_target_exists(*identity) {
            signal_process_target(*identity, libc::SIGKILL)?;
        }
    }
    if wait_for_process_exit(&identities, grace).await {
        confirm_tmux_reaped(process).await
    } else {
        Err(anyhow!(
            "generation {} survived bounded TERM/KILL reap",
            process.generation
        ))
    }
}

async fn confirm_tmux_reaped(process: &crate::child_session::ChildProcessGeneration) -> Result<()> {
    if tmux_installed() && tmux_session_exists(&process.tmux_name).await? {
        Err(anyhow!(
            "tmux body {} survived bounded reap",
            process.tmux_name
        ))
    } else {
        Ok(())
    }
}

#[derive(Debug, Clone, Copy)]
pub(crate) enum ProcessSignalTarget {
    Group(u32),
    Process(u32),
}

/// What a liveness question was actually answered with.
///
/// The third arm is the point. A reaper may fail open — concluding "nothing to
/// kill" costs it nothing, because evidence releases the lease later anyway. A
/// *release* must fail closed: concluding "nothing is there" when the question
/// went unanswered unbars a second body for the Session. So an unanswered
/// question is never a "no"; it is [`Presence::Unprovable`], and the lease holds.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Presence {
    /// Positively answered: nothing is there.
    Absent,
    /// Positively answered: something is there.
    Present,
    /// The host could not be asked. Never read this as absence.
    Unprovable,
}

/// The addressable form of a recorded identity, or `None` when it is not one.
///
/// Two ids are rejected because the kernel *would* answer them, for a different
/// question than the one being asked: `kill(0, …)` signals the caller's own
/// process group, and `kill(-1, …)` is a POSIX broadcast over every process the
/// caller may signal — which is what `Group(1)` negates to. Reading either as
/// absence is the worst available answer.
fn signal_probe_id(target: ProcessSignalTarget) -> Option<i32> {
    let raw = match target {
        ProcessSignalTarget::Group(group) => -i32::try_from(group).ok()?,
        ProcessSignalTarget::Process(pid) => i32::try_from(pid).ok()?,
    };
    if raw == 0 || raw == -1 {
        return None;
    }
    Some(raw)
}

/// Classify one `kill(id, 0)` outcome: the syscall's two facts in, a verdict out.
///
/// Deliberately *not* [`process_target_exists`], whose `bool` folds `ESRCH` and
/// every errno it does not model into the same `false`. That collapse is
/// harmless for a bounded waiter, where `false` means "stop waiting", and
/// fail-open for a release.
fn classify_signal_probe(returned: i32, errno: Option<i32>) -> Presence {
    if returned == 0 {
        return Presence::Present;
    }
    match errno {
        // No process answers. The one authoritative absence a signal can report.
        Some(libc::ESRCH) => Presence::Absent,
        // POSIX only returns EPERM for `kill(-pgid, …)` when the group had at
        // least one member the caller lacked permission for. Something is there;
        // that we cannot tell our body from a stranger is exactly why we block.
        Some(libc::EPERM) => Presence::Present,
        _ => Presence::Unprovable,
    }
}

fn probe_signal_target(target: ProcessSignalTarget) -> Presence {
    let Some(raw) = signal_probe_id(target) else {
        return Presence::Unprovable;
    };
    // SAFETY: signal 0 performs an existence/permission probe and uses no pointers.
    let returned = unsafe { libc::kill(raw, 0) };
    let errno = if returned == 0 {
        None
    } else {
        std::io::Error::last_os_error().raw_os_error()
    };
    classify_signal_probe(returned, errno)
}

/// Classify a non-zero `tmux list-sessions`: did tmux say "there is nothing
/// here", or did it fail to answer?
///
/// tmux reports an absent server two ways, and **both must be recognized**:
///
/// ```text
/// no server running on /tmp/tmux-501/default
/// error connecting to /tmp/tmux-501/default (No such file or directory)
/// ```
///
/// The second is what tmux 3.6a actually prints when the socket does not exist,
/// and missing it is not a harmless conservatism: a host whose last body exited
/// has no server, so every probe there would read `Unprovable` and no lease
/// would ever release — the feature would be a no-op in exactly the situation it
/// exists for. A body's tmux session dying *with* the server is W2-230's own
/// shape.
///
/// The connect error is only absence when it is `ENOENT` — the socket is not
/// there, so no server is. Any other connect failure (`Permission denied`, or
/// tmux 3.6a's `Socket operation on non-socket`) means a socket we could not
/// question, which is `Unprovable`.
///
/// Matching prose is the soft spot, and it is soft in the safe direction: an
/// unrecognized wording falls to `Unprovable`, the lease stays blocked, and the
/// behavior is today's. No rewording can produce a wrong release — it can only
/// cost a release that should have happened, which is why the recognized set has
/// to track what tmux really prints.
fn classify_tmux_probe_failure(stderr: &str) -> Presence {
    let stderr = stderr.to_ascii_lowercase();
    let no_server = stderr.contains("no server running")
        || stderr.contains("no sessions")
        || (stderr.contains("error connecting to") && stderr.contains("no such file or directory"));
    if no_server {
        Presence::Absent
    } else {
        Presence::Unprovable
    }
}

/// Whether the recorded tmux session is live.
///
/// Deliberately not [`tmux_session_exists`], which maps *any* non-zero exit to
/// `Ok(false)` — a server that errored and a server reporting no such session
/// are the same value there. An unspawnable tmux (including tmux absent from
/// this process's `PATH`, which is a fact about this process, not about the
/// body) is `Unprovable`, never absence.
async fn probe_tmux_session(session_name: &str) -> Presence {
    let Ok(output) = tokio::process::Command::new("tmux")
        .args(["list-sessions", "-F", "#{session_name}"])
        .output()
        .await
    else {
        return Presence::Unprovable;
    };
    if output.status.success() {
        // The server answered authoritatively; membership decides. The common
        // case never reads the stderr prose at all.
        let live = String::from_utf8_lossy(&output.stdout)
            .lines()
            .map(str::trim)
            .any(|name| name == session_name);
        return if live {
            Presence::Present
        } else {
            Presence::Absent
        };
    }
    classify_tmux_probe_failure(&String::from_utf8_lossy(&output.stderr))
}

/// Whether a recorded body generation is still running on this host.
///
/// `Absent` requires *positive* answers: the tmux session is positively gone
/// **and** one signal identity is positively gone. Any unanswered question
/// anywhere demotes the verdict to `Unprovable`.
///
/// It reads **one** signal identity — the process group, falling back to the pid
/// only when no group was recorded — and never both. A pid is a recycled
/// resource, so on an old generation the recorded pid may name an unrelated
/// stranger; vetoing on it would pin the lease forever against any future
/// evidence, which is the very defect this probe exists to clear. Past the
/// authoritative identity, extra vetoes add ways to be permanently wrong, not
/// safety.
///
/// tmux carries the `Present` veto rather than the pid because the group is not
/// reliably the lf body's: `observe_provider` overwrites it with the harness's
/// own child group for opencode and codex bodies, while the lf body always runs
/// inside its recorded tmux session. So a live session proves a live body
/// whatever the group names, and the group still covers the reverse — tmux gone,
/// provider group orphaned.
pub(crate) async fn probe_child_body_presence(
    process: &crate::child_session::ChildProcessGeneration,
) -> Presence {
    match probe_tmux_session(&process.tmux_name).await {
        Presence::Present => return Presence::Present,
        Presence::Unprovable => return Presence::Unprovable,
        Presence::Absent => {}
    }
    let target = match (process.process_group_id, process.pid) {
        (Some(group), _) => ProcessSignalTarget::Group(group),
        (None, Some(pid)) => ProcessSignalTarget::Process(pid),
        // Nothing to ask, and tmux already answered that the body is gone.
        (None, None) => return Presence::Absent,
    };
    probe_signal_target(target)
}

fn signal_process_target(target: ProcessSignalTarget, signal: libc::c_int) -> Result<()> {
    let raw = match target {
        ProcessSignalTarget::Group(group) => -i32::try_from(group)
            .map_err(|_| anyhow!("process group {group} exceeds supported range"))?,
        ProcessSignalTarget::Process(pid) => {
            i32::try_from(pid).map_err(|_| anyhow!("process {pid} exceeds supported range"))?
        }
    };
    // SAFETY: kill receives a validated PID/process-group id and no pointers.
    let result = unsafe { libc::kill(raw, signal) };
    if result == 0 {
        return Ok(());
    }
    let error = std::io::Error::last_os_error();
    if error.raw_os_error() == Some(libc::ESRCH) {
        Ok(())
    } else {
        Err(anyhow!("failed to signal {target:?}: {error}"))
    }
}

fn process_target_exists(target: ProcessSignalTarget) -> bool {
    let raw = match target {
        ProcessSignalTarget::Group(group) => i32::try_from(group).map_or(0, |group| -group),
        ProcessSignalTarget::Process(pid) => i32::try_from(pid).unwrap_or(0),
    };
    if raw == 0 {
        return false;
    }
    // SAFETY: signal 0 performs an existence/permission probe and uses no pointers.
    let result = unsafe { libc::kill(raw, 0) };
    result == 0 || std::io::Error::last_os_error().raw_os_error() == Some(libc::EPERM)
}

async fn wait_for_process_exit(
    targets: &[ProcessSignalTarget],
    grace: std::time::Duration,
) -> bool {
    let deadline = tokio::time::Instant::now() + grace;
    loop {
        if targets.iter().all(|target| !process_target_exists(*target)) {
            return true;
        }
        if tokio::time::Instant::now() >= deadline {
            return false;
        }
        tokio::time::sleep(std::time::Duration::from_millis(25)).await;
    }
}

pub(crate) async fn start_lf_session(session: &str, cwd: &Path, argv: &[String]) -> Result<()> {
    start_lf_session_with_env(session, cwd, argv, &[]).await
}

pub(crate) async fn start_lf_session_with_env(
    session: &str,
    cwd: &Path,
    argv: &[String],
    env: &[(&str, &str)],
) -> Result<()> {
    reject_detached_forwarded_account(crate::provider_account::lease::account_lease_active())?;
    let context = pinned_execution_context()?;
    let inherited_context = ["LF_RUN_ID", "LF_PROCESS_ID"]
        .into_iter()
        .filter(|key| !env.iter().any(|(explicit, _)| explicit == key))
        .filter_map(|key| std::env::var(key).ok().map(|value| (key, value)))
        .collect::<Vec<_>>();
    let mut child_env = env
        .iter()
        .map(|(key, value)| ((*key).to_string(), (*value).to_string()))
        .collect::<Vec<_>>();
    child_env.extend(
        inherited_context
            .iter()
            .map(|(key, value)| ((*key).to_string(), value.clone())),
    );
    extend_session_control_context(&mut child_env, &context, crate::build_info::provenance());
    let environment = child_env
        .iter()
        .map(|(key, value)| (key.as_str(), value.as_str()))
        .collect::<Vec<_>>();
    let shell_command = lf_session_shell_command(argv, &environment);
    start_tmux_session(session, &cwd.display().to_string(), &shell_command).await
}

fn reject_detached_forwarded_account(forwarded: bool) -> Result<()> {
    if forwarded {
        return Err(anyhow!(
            "cannot launch a detached session from an ephemeral forwarded provider account; \
             keep the remote command in the foreground or authenticate on the remote host"
        ));
    }
    Ok(())
}

fn extend_session_control_context(
    child_env: &mut Vec<(String, String)>,
    context: &crate::child_session::ChildExecutionContext,
    provenance: crate::build_info::BuildProvenance,
) {
    let pinned = [
        (
            crate::store::CONTROL_BIN_ENV,
            context.lf_bin.to_string_lossy().to_string(),
        ),
        (
            crate::store::CONTROL_HOME_ENV,
            context.lf_home.to_string_lossy().to_string(),
        ),
        (
            crate::store::CONTROL_DB_PATH_ENV,
            context.db_path.to_string_lossy().to_string(),
        ),
    ];
    for (key, value) in pinned {
        if !child_env.iter().any(|(existing, _)| existing == key) {
            child_env.push((key.to_string(), value));
        }
    }
    if !provenance.is_release() {
        for (ordinary, control) in [
            ("LF_HOME", crate::store::CONTROL_HOME_ENV),
            ("LF_DB_PATH", crate::store::CONTROL_DB_PATH_ENV),
        ] {
            if child_env.iter().any(|(existing, _)| existing == ordinary) {
                continue;
            }
            let value = child_env
                .iter()
                .find(|(key, _)| key == control)
                .map(|(_, value)| value.clone());
            if let Some(value) = value {
                child_env.push((ordinary.to_string(), value));
            }
        }
    }
}

pub(crate) fn lf_session_shell_command(argv: &[String], env: &[(&str, &str)]) -> String {
    let command = argv
        .iter()
        .map(|arg| shell_escape(arg))
        .collect::<Vec<_>>()
        .join(" ");
    let env = env
        .iter()
        .map(|(key, value)| format!("{}={}", shell_escape(key), shell_escape(value)))
        .collect::<Vec<_>>()
        .join(" ");
    let clear_context = "unset LF_RUN_ID LF_PROCESS_ID LF_WAVE_ID LF_CHANNEL LF_PROJECT_SESSION_ID LF_PROJECT_GENERATION LF_PROJECT_LEASE_TOKEN LF_TASK_SESSION_ID LF_TASK_GENERATION LF_TASK_LEASE_TOKEN LF_BIN LF_HOME LF_DB_PATH LF_CONTROL_BIN LF_CONTROL_HOME LF_CONTROL_DB_PATH";
    if env.is_empty() {
        format!("{clear_context}; exec {command}")
    } else {
        format!("{clear_context}; exec env {env} {command}")
    }
}

pub(crate) async fn start_tmux_session(
    session: &str,
    cwd: &str,
    shell_command: &str,
) -> Result<()> {
    let status = tokio::process::Command::new("tmux")
        .args([
            "new-session",
            "-d",
            "-s",
            session,
            "-c",
            cwd,
            "/bin/zsh",
            "-lc",
            shell_command,
        ])
        .status()
        .await
        .map_err(|err| anyhow!("tmux failed to spawn: {err}"))?;
    if !status.success() {
        return Err(anyhow!("tmux failed to launch session '{session}'"));
    }
    let _ = tokio::process::Command::new("tmux")
        .args(["set-option", "-t", session, "mouse", "on"])
        .status()
        .await;
    Ok(())
}

pub(crate) fn tmux_session_slug(value: &str) -> String {
    value
        .chars()
        .map(|ch| {
            if ch.is_ascii_alphanumeric() || matches!(ch, '-' | '_') {
                ch
            } else {
                '-'
            }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use std::io::{BufRead, BufReader};
    use std::os::unix::process::CommandExt;
    use std::path::PathBuf;

    use super::{
        classify_signal_probe, classify_tmux_probe_failure, extend_session_control_context,
        lf_session_shell_command, probe_child_body_presence, reap_child_process,
        reject_detached_forwarded_account, select_binary_override, select_current_home_binary,
        signal_probe_id, tmux_installed, FakeTmux, Presence, ProcessSignalTarget,
    };
    use crate::build_info::BuildProvenance;
    use crate::child_session::ChildExecutionContext;

    #[test]
    fn development_ignores_stale_control_binary_override() {
        assert_eq!(
            select_binary_override(
                BuildProvenance::Development,
                Some("/production/lf".into()),
                Some("/development/lf".into()),
            ),
            Some(PathBuf::from("/development/lf"))
        );
        assert_eq!(
            select_binary_override(
                BuildProvenance::Release,
                Some("/production/lf".into()),
                Some("/ambient/lf".into()),
            ),
            Some(PathBuf::from("/production/lf"))
        );
    }

    /// The launch boundary must resolve the current Home lf (B), never the
    /// historical `LF_CONTROL_BIN` pin (A) — the regression behind stranded
    /// legacy Sessions. Contrast the two selectors under release provenance:
    /// the old override picks the control pin A, the current-Home selector
    /// picks B and has no way to reach A at all.
    #[test]
    fn current_home_binary_never_resolves_through_the_control_pin() {
        // Old behavior (the bug): a release build prefers LF_CONTROL_BIN (A),
        // even when the current Home LF_BIN (B) is present.
        assert_eq!(
            select_binary_override(
                BuildProvenance::Release,
                Some("/old/A/lf".into()),
                Some("/current/B/lf".into()),
            ),
            Some(PathBuf::from("/old/A/lf")),
        );
        // Fixed: the current-Home selector has no control input, so with
        // LF_BIN=B it resolves B — A is unreachable, in any provenance.
        assert_eq!(
            select_current_home_binary(Some("/current/B/lf".into())),
            Some(PathBuf::from("/current/B/lf")),
        );
        // Empty or absent LF_BIN falls through to PATH/installed lf, never to A.
        assert_eq!(select_current_home_binary(None), None);
        assert_eq!(select_current_home_binary(Some("".into())), None);
    }

    #[test]
    fn persisted_control_binary_wins_over_relaunching_callers_binary() {
        let mut environment = vec![(
            crate::store::CONTROL_BIN_ENV.to_string(),
            "/persisted/lf".to_string(),
        )];
        let caller = ChildExecutionContext {
            lf_bin: PathBuf::from("/caller/lf"),
            lf_home: PathBuf::from("/caller/home"),
            db_path: PathBuf::from("/caller/loopflow.db"),
        };

        extend_session_control_context(&mut environment, &caller, BuildProvenance::Release);

        assert!(environment.iter().any(|(key, value)| {
            key == crate::store::CONTROL_BIN_ENV && value == "/persisted/lf"
        }));
        assert!(!environment
            .iter()
            .any(|(key, value)| { key == crate::store::CONTROL_BIN_ENV && value == "/caller/lf" }));
    }

    /// The probe must agree with whether tmux can actually be run. The previous
    /// `--version` probe disagreed on every machine that has tmux, which pinned
    /// Session liveness to "unknowable" and let gone processes read as running.
    #[test]
    fn tmux_probe_agrees_with_running_tmux() {
        // Both sides of this comparison resolve through `PATH`.
        let _env_lock = crate::journal::test_env_lock();
        let runnable = std::process::Command::new("tmux")
            .arg("-V")
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .status()
            .map(|status| status.success())
            .unwrap_or(false);
        assert_eq!(tmux_installed(), runnable);
    }

    #[test]
    fn lf_session_clears_parent_identity_and_exports_its_own() {
        let argv = vec!["lf".to_string(), "__task".to_string()];
        let command = lf_session_shell_command(
            &argv,
            &[("LF_TASK_SESSION_ID", "task-1"), ("LF_WAVE_ID", "infra")],
        );

        assert_eq!(
            command,
            "unset LF_RUN_ID LF_PROCESS_ID LF_WAVE_ID LF_CHANNEL LF_PROJECT_SESSION_ID LF_PROJECT_GENERATION LF_PROJECT_LEASE_TOKEN LF_TASK_SESSION_ID LF_TASK_GENERATION LF_TASK_LEASE_TOKEN LF_BIN LF_HOME LF_DB_PATH LF_CONTROL_BIN LF_CONTROL_HOME LF_CONTROL_DB_PATH; exec env 'LF_TASK_SESSION_ID'='task-1' 'LF_WAVE_ID'='infra' 'lf' '__task'"
        );
    }

    #[test]
    fn lf_session_without_explicit_identity_does_not_inherit_its_parent() {
        let argv = vec!["lf".to_string(), "wave".to_string(), "child".to_string()];

        let command = lf_session_shell_command(&argv, &[]);

        assert_eq!(
            command,
            "unset LF_RUN_ID LF_PROCESS_ID LF_WAVE_ID LF_CHANNEL LF_PROJECT_SESSION_ID LF_PROJECT_GENERATION LF_PROJECT_LEASE_TOKEN LF_TASK_SESSION_ID LF_TASK_GENERATION LF_TASK_LEASE_TOKEN LF_BIN LF_HOME LF_DB_PATH LF_CONTROL_BIN LF_CONTROL_HOME LF_CONTROL_DB_PATH; exec 'lf' 'wave' 'child'"
        );
    }

    #[test]
    fn lf_session_replaces_tmux_invocation_context() {
        let argv = vec!["lf".to_string(), "__task".to_string()];
        let command = lf_session_shell_command(
            &argv,
            &[
                ("LF_RUN_ID", "run-1"),
                ("LF_PROCESS_ID", "process-1"),
                ("LF_DB_PATH", "/tmp/current.db"),
                ("LF_HOME", "/tmp/lf"),
            ],
        );

        assert_eq!(
            command,
            "unset LF_RUN_ID LF_PROCESS_ID LF_WAVE_ID LF_CHANNEL LF_PROJECT_SESSION_ID LF_PROJECT_GENERATION LF_PROJECT_LEASE_TOKEN LF_TASK_SESSION_ID LF_TASK_GENERATION LF_TASK_LEASE_TOKEN LF_BIN LF_HOME LF_DB_PATH LF_CONTROL_BIN LF_CONTROL_HOME LF_CONTROL_DB_PATH; exec env 'LF_RUN_ID'='run-1' 'LF_PROCESS_ID'='process-1' 'LF_DB_PATH'='/tmp/current.db' 'LF_HOME'='/tmp/lf' 'lf' '__task'"
        );
    }

    #[test]
    fn detached_session_rejects_an_ephemeral_forwarded_account() {
        assert!(reject_detached_forwarded_account(false).is_ok());
        assert!(reject_detached_forwarded_account(true)
            .unwrap_err()
            .to_string()
            .contains("cannot launch a detached session"));
    }

    /// The errno table, proven over plain values.
    ///
    /// Deliberately not manufactured from a real privileged process: the only
    /// way to make a real `EPERM` is a process this user may not signal, which
    /// is a fact about the CI user rather than about the code — as root no
    /// `EPERM` arises at all. `Group(1)` is not a fixture for it either: that
    /// negates to `kill(-1, 0)`, a POSIX broadcast over the whole machine.
    #[test]
    fn classify_signal_probe_reads_each_answer_for_what_it_is() {
        assert_eq!(classify_signal_probe(0, None), Presence::Present);
        assert_eq!(
            classify_signal_probe(-1, Some(libc::ESRCH)),
            Presence::Absent
        );
        assert_eq!(
            classify_signal_probe(-1, Some(libc::EPERM)),
            Presence::Present
        );
        // The arm `process_target_exists` folds into `false` — i.e. "absent".
        // Reusing that bool here would release a lease over a body the kernel
        // refused to answer about.
        assert_eq!(
            classify_signal_probe(-1, Some(libc::EINVAL)),
            Presence::Unprovable
        );
        assert_eq!(classify_signal_probe(-1, None), Presence::Unprovable);
    }

    /// Both ids the kernel *would* answer, for the wrong question.
    #[test]
    fn signal_probe_id_rejects_the_self_group_and_broadcast_ids() {
        // kill(0, …) signals the caller's own process group.
        assert_eq!(signal_probe_id(ProcessSignalTarget::Group(0)), None);
        assert_eq!(signal_probe_id(ProcessSignalTarget::Process(0)), None);
        // Group(1) negates to kill(-1, …): a broadcast over every process the
        // caller may signal, which answers `Present` on any host at all.
        assert_eq!(signal_probe_id(ProcessSignalTarget::Group(1)), None);
        // A real body identity still resolves, negated for the group form.
        assert_eq!(
            signal_probe_id(ProcessSignalTarget::Group(4242)),
            Some(-4242)
        );
        assert_eq!(
            signal_probe_id(ProcessSignalTarget::Process(4242)),
            Some(4242)
        );
    }

    /// `process_target_exists` answers `false` — "absent" — to an id that does
    /// not fit `i32`, via `map_or(0, …)`. The release must not.
    #[test]
    fn signal_probe_id_rejects_an_unrepresentable_id() {
        let too_large = u32::try_from(i32::MAX).expect("i32::MAX fits u32") + 1;
        assert_eq!(signal_probe_id(ProcessSignalTarget::Group(too_large)), None);
        assert_eq!(
            signal_probe_id(ProcessSignalTarget::Process(too_large)),
            None
        );
    }

    #[test]
    fn both_tmux_wordings_for_an_absent_server_read_as_absence() {
        assert_eq!(
            classify_tmux_probe_failure("no server running on /tmp/tmux-501/default"),
            Presence::Absent
        );
        assert_eq!(
            classify_tmux_probe_failure("no sessions\n"),
            Presence::Absent
        );
        // Verbatim from tmux 3.6a with an absent socket. Reading this as
        // `Unprovable` silently disables every release on a host whose last body
        // took the server with it -- W2-230's own shape. A test covering only
        // `no server running` passes with that defect fully present.
        assert_eq!(
            classify_tmux_probe_failure(
                "error connecting to /tmp/tmux-501/default (No such file or directory)"
            ),
            Presence::Absent
        );
        // A socket we could not question is not an absent one: something is there
        // and it refused us. `tmux_session_exists` maps every line below to
        // `Ok(false)` -- "no session" -- which is why a release cannot use it.
        assert_eq!(
            classify_tmux_probe_failure(
                "error connecting to /tmp/tmux-501/default (Permission denied)"
            ),
            Presence::Unprovable
        );
        assert_eq!(
            classify_tmux_probe_failure(
                "error connecting to /tmp/lf.sock (Socket operation on non-socket)"
            ),
            Presence::Unprovable
        );
        assert_eq!(classify_tmux_probe_failure(""), Presence::Unprovable);
    }

    fn probe_fixture(
        tmux_name: &str,
        pid: Option<u32>,
        process_group_id: Option<u32>,
    ) -> crate::child_session::ChildProcessGeneration {
        crate::child_session::ChildProcessGeneration {
            generation: 1,
            pid,
            process_group_id,
            tmux_name: tmux_name.to_string(),
            agent: "fake".to_string(),
            provider: "fake".to_string(),
            provider_session_id: None,
            started_at: time::OffsetDateTime::now_utc(),
            state: crate::child_session::ChildLeaseState::Revoked,
            outcome: None,
            provenance: None,
        }
    }

    /// The pid-veto regression, at the probe. A recycled pid names a stranger,
    /// and vetoing on it would pin the lease forever against any future
    /// evidence — the very defect the release exists to clear. With a group
    /// recorded, the pid is never consulted.
    #[tokio::test]
    async fn a_live_unrelated_pid_does_not_block_absence_when_a_group_is_recorded() {
        let mut stranger = std::process::Command::new("/bin/sh")
            .arg("-c")
            .arg("sleep 60")
            .spawn()
            .expect("spawn an unrelated live process");
        let live_pid = stranger.id();
        // An absent group, plus a pid that is emphatically alive.
        let process = probe_fixture(
            &format!("lf-probe-absent-{live_pid}"),
            Some(live_pid),
            Some(absent_process_group()),
        );

        // tmux answers `no sessions` so this test's subject is the signal
        // identity, not whether the host has tmux.
        let _tmux = FakeTmux::no_session();
        let presence = probe_child_body_presence(&process).await;

        stranger.kill().expect("kill the unrelated process");
        stranger.wait().expect("reap the unrelated process");
        assert_eq!(presence, Presence::Absent);
    }

    /// The reverse: with no group recorded, the pid is the only identity there
    /// is, so it decides.
    #[tokio::test]
    async fn a_live_pid_is_present_when_no_group_was_recorded() {
        let mut body = std::process::Command::new("/bin/sh")
            .arg("-c")
            .arg("sleep 60")
            .spawn()
            .expect("spawn a live body");
        let live_pid = body.id();
        let process = probe_fixture(&format!("lf-probe-pid-{live_pid}"), Some(live_pid), None);

        let _tmux = FakeTmux::no_session();
        let presence = probe_child_body_presence(&process).await;

        body.kill().expect("kill the body");
        body.wait().expect("reap the body");
        assert_eq!(presence, Presence::Present);
    }

    #[tokio::test]
    async fn a_live_process_group_is_present() {
        let mut child = std::process::Command::new("/bin/sh")
            .arg("-c")
            .arg("sleep 60")
            .process_group(0)
            .spawn()
            .expect("spawn an isolated process group");
        let group = child.id();
        let process = probe_fixture(&format!("lf-probe-live-{group}"), Some(group), Some(group));

        let _tmux = FakeTmux::no_session();
        let presence = probe_child_body_presence(&process).await;

        child.kill().expect("kill the group leader");
        child.wait().expect("reap the group leader");
        assert_eq!(presence, Presence::Present);
    }

    /// An unaddressable identity is never absence, however it got recorded.
    #[tokio::test]
    async fn an_unaddressable_identity_is_unprovable_not_absent() {
        let too_large = u32::try_from(i32::MAX).expect("i32::MAX fits u32") + 1;
        let _tmux = FakeTmux::no_session();
        for group in [0, 1, too_large] {
            let process = probe_fixture(&format!("lf-probe-bad-{group}"), None, Some(group));
            assert_eq!(
                probe_child_body_presence(&process).await,
                Presence::Unprovable,
                "group {group} must not read as absence"
            );
        }
    }

    /// A generation with no signal identity at all rests on tmux alone.
    #[tokio::test]
    async fn a_generation_with_no_identity_is_absent_once_tmux_is_gone() {
        let process = probe_fixture("lf-probe-no-identity-at-all", None, None);
        let _tmux = FakeTmux::no_session();
        assert_eq!(probe_child_body_presence(&process).await, Presence::Absent);
    }

    /// The tmux tri-state, end to end through a real spawn.
    ///
    /// Each arm is a case `tmux_session_exists` collapses into `Ok(false)` —
    /// "no session" — so a release that reused it would read a server it could
    /// not question as proof the body is gone.
    #[tokio::test]
    async fn the_tmux_probe_never_reads_an_unanswered_question_as_absence() {
        // A recorded session the server positively lists: a live body.
        let process = probe_fixture("lf-tmux-live", None, None);
        {
            let _tmux = FakeTmux::live_session("lf-tmux-live");
            assert_eq!(probe_child_body_presence(&process).await, Presence::Present);
        }
        // The server answers and does not list it: authoritative absence.
        {
            let _tmux = FakeTmux::no_session();
            assert_eq!(probe_child_body_presence(&process).await, Presence::Absent);
        }
        // No server running, reported the way tmux reports it.
        {
            let _tmux = FakeTmux::no_server();
            assert_eq!(probe_child_body_presence(&process).await, Presence::Absent);
        }
        // And tmux we cannot run at all is a fact about this process's PATH,
        // never about the body. This is the exact shape CI hits.
        {
            let _tmux = FakeTmux::unspawnable();
            assert_eq!(
                probe_child_body_presence(&process).await,
                Presence::Unprovable
            );
        }
    }

    /// A process group that has certainly exited: spawn one, reap it, and reuse
    /// its id. Racy only if the kernel recycles this exact pgid within the test,
    /// which would make the probe read `Present` — a false failure, never a
    /// false pass.
    fn absent_process_group() -> u32 {
        let mut child = std::process::Command::new("/bin/sh")
            .arg("-c")
            .arg("exit 0")
            .process_group(0)
            .spawn()
            .expect("spawn a short-lived process group");
        let group = child.id();
        child.wait().expect("reap the short-lived group");
        group
    }

    /// Resolves `tmux` through `PATH` (`confirm_tmux_reaped`), so it holds the
    /// crate-wide env lock: without it a concurrent [`FakeTmux`] answers
    /// `has-session` for this test's body and the reap reads as survived.
    #[allow(clippy::await_holding_lock)] // the env lock is the test serializer
    #[tokio::test]
    async fn bounded_reap_kills_the_runner_process_group_and_its_grandchild() {
        let _env_lock = crate::journal::test_env_lock();
        let mut child = std::process::Command::new("/bin/sh")
            .arg("-c")
            .arg("sleep 60 & echo $!; wait")
            .process_group(0)
            .stdout(std::process::Stdio::piped())
            .spawn()
            .expect("spawn isolated process group");
        let group = child.id();
        let stdout = child.stdout.take().expect("capture grandchild pid");
        let mut line = String::new();
        BufReader::new(stdout)
            .read_line(&mut line)
            .expect("read grandchild pid");
        let grandchild: u32 = line.trim().parse().expect("grandchild pid");
        let waiter = std::thread::spawn(move || child.wait().expect("reap shell"));
        let process = crate::child_session::ChildProcessGeneration {
            generation: 1,
            pid: Some(group),
            process_group_id: Some(group),
            tmux_name: format!("lf-reap-test-{group}"),
            agent: "fake".to_string(),
            provider: "fake".to_string(),
            provider_session_id: None,
            started_at: time::OffsetDateTime::now_utc(),
            state: crate::child_session::ChildLeaseState::Revoked,
            outcome: Some(crate::child_session::ChildBodyOutcome::Superseded {
                reason: "test".to_string(),
            }),
            provenance: None,
        };

        reap_child_process(&process, std::time::Duration::from_secs(2))
            .await
            .unwrap();
        waiter.join().unwrap();
        // SAFETY: signal 0 is an existence probe and uses no pointers.
        assert_ne!(unsafe { libc::kill(group as i32, 0) }, 0);
        // SAFETY: signal 0 is an existence probe and uses no pointers.
        assert_ne!(unsafe { libc::kill(grandchild as i32, 0) }, 0);
    }
}
