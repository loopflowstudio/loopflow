use std::path::{Path, PathBuf};

use anyhow::{anyhow, Result};

pub(crate) fn current_process_group_id() -> Option<u32> {
    // SAFETY: getpgrp has no preconditions and does not dereference memory.
    let process_group = unsafe { libc::getpgrp() };
    u32::try_from(process_group).ok().filter(|id| *id > 1)
}

pub(crate) fn resolve_lf_binary() -> PathBuf {
    if crate::build_info::provenance().is_release() {
        if let Ok(path) = std::env::var(crate::store::CONTROL_BIN_ENV) {
            let trimmed = path.trim();
            if !trimmed.is_empty() {
                return PathBuf::from(trimmed);
            }
        }
    }
    if let Ok(path) = std::env::var("LF_BIN") {
        let trimmed = path.trim();
        if !trimmed.is_empty() {
            return PathBuf::from(trimmed);
        }
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

/// Capture the execution context to pin on a Session being created: this
/// process's `lf`, this process's store, this process's `LF_HOME` — all
/// absolute, all resolved exactly once.
pub(crate) fn pinned_execution_context() -> Result<crate::child_session::ChildExecutionContext> {
    let db_path = crate::store::database_path_from_env()
        .map_err(|error| anyhow!("cannot resolve the Session's database path: {error}"))?;
    Ok(crate::child_session::ChildExecutionContext {
        lf_bin: resolve_pinned_lf_binary()?,
        db_path,
        lf_home: crate::store::lf_home_dir(),
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
enum ProcessSignalTarget {
    Group(u32),
    Process(u32),
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
    if !crate::build_info::provenance().is_release() {
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
    let environment = child_env
        .iter()
        .map(|(key, value)| (key.as_str(), value.as_str()))
        .collect::<Vec<_>>();
    let shell_command = lf_session_shell_command(argv, &environment);
    start_tmux_session(session, &cwd.display().to_string(), &shell_command).await
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

    use super::{lf_session_shell_command, reap_child_process, tmux_installed};

    /// The probe must agree with whether tmux can actually be run. The previous
    /// `--version` probe disagreed on every machine that has tmux, which pinned
    /// Session liveness to "unknowable" and let gone processes read as running.
    #[test]
    fn tmux_probe_agrees_with_running_tmux() {
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

    #[tokio::test]
    async fn bounded_reap_kills_the_runner_process_group_and_its_grandchild() {
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
