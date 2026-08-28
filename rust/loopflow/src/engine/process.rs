use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::Arc;

use anyhow::{anyhow, Result};

pub(crate) const DISCORD_TOKEN_ENV: &str = "LF_DISCORD_TOKEN";
/// The SSH destination by which the current foreground `lf` was reached.
/// It is invocation context, not durable Home identity.
pub(crate) const SSH_TARGET_ENV: &str = "LF_SSH_TARGET";

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
        crate::engine::agent::register_interrupt_cleanup(move || {
            terminate_process_group(interrupt_pid.swap(0, Ordering::AcqRel));
        });
        Self { pid }
    }

    pub(crate) fn terminate(&self) {
        terminate_process_group(self.pid.swap(0, Ordering::AcqRel));
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

fn terminate_process_group(pid: u32) {
    if pid == 0 {
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
    crate::engine::platform::kill_process(pid);
}

const TMUX_LIVENESS_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(2);

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

pub(crate) fn resolve_lfd_binary() -> PathBuf {
    let cargo_override = std::env::var("CARGO_BIN_EXE_lfd")
        .ok()
        .filter(|path| !path.trim().is_empty())
        .map(PathBuf::from);
    let lf = resolve_lf_binary();
    let lf_sibling = lf
        .parent()
        .map(|parent| parent.join("lfd"))
        .filter(|path| path.is_file());
    let invoked_sibling = std::env::args_os()
        .next()
        .map(PathBuf::from)
        .and_then(|invoked| invoked.parent().map(|parent| parent.join("lfd")))
        .filter(|path| path.is_file());
    let path_binary = which_on_path(Path::new("lfd"));
    let current = std::env::current_exe().ok().filter(|path| {
        path.file_name()
            .and_then(|name| name.to_str())
            .is_some_and(|name| name == "lfd")
    });

    select_lfd_binary(
        crate::build_info::provenance(),
        cargo_override,
        lf_sibling,
        invoked_sibling,
        path_binary,
        current,
    )
}

pub(crate) fn resolve_lfd_binary_checked() -> Result<PathBuf> {
    let candidate = resolve_lfd_binary();
    if candidate.is_absolute() {
        return if candidate.is_file() {
            Ok(candidate)
        } else {
            Err(anyhow!(
                "lfd binary {} does not exist; install the current Home control pair",
                candidate.display()
            ))
        };
    }
    which_on_path(&candidate).ok_or_else(|| {
        anyhow!(
            "cannot resolve an absolute path for `{}`; install lfd beside the current Home lf",
            candidate.display()
        )
    })
}

fn select_lfd_binary(
    provenance: crate::build_info::BuildProvenance,
    cargo_override: Option<PathBuf>,
    lf_sibling: Option<PathBuf>,
    invoked_sibling: Option<PathBuf>,
    path_binary: Option<PathBuf>,
    current_lfd: Option<PathBuf>,
) -> PathBuf {
    if let Some(path) = cargo_override {
        return path;
    }
    if provenance == crate::build_info::BuildProvenance::Development {
        if let Some(path) = lf_sibling {
            return path;
        }
    }
    invoked_sibling
        .or(path_binary)
        .or(current_lfd)
        .unwrap_or_else(|| PathBuf::from("lfd"))
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

/// Resolve the `lf` a Work launch will use: an absolute path that exists.
///
/// `resolve_lf_binary` may hand back the bare name `lf`, which a child resolves
/// against the *login* shell's PATH inside tmux — a third binary, chosen by
/// neither the Work nor its launcher. Work that cannot name its own
/// executable is not created.
pub(crate) fn resolve_pinned_lf_binary() -> Result<PathBuf> {
    let candidate = resolve_lf_binary();
    if candidate.is_absolute() {
        return if candidate.exists() {
            Ok(candidate)
        } else {
            Err(anyhow!(
                "lf binary {} does not exist; set LF_BIN to the lf this Work should run",
                candidate.display()
            ))
        };
    }
    which_on_path(&candidate).ok_or_else(|| {
        anyhow!(
            "cannot resolve an absolute path for `{}`; set LF_BIN to the lf this Work should run",
            candidate.display()
        )
    })
}

/// Pin one process generation to immutable executable bytes.
///
/// The installed `lf` is normally a mutable symlink. Exact-frontier promotion
/// may repoint it while a resident body is running, so the body carries the
/// canonical target in `LF_CONTROL_BIN`. A later body launch deliberately
/// resolves the current Home again and picks up the promoted binary.
pub(crate) fn pin_control_binary(lf_bin: &Path) -> PathBuf {
    std::fs::canonicalize(lf_bin).unwrap_or_else(|_| lf_bin.to_path_buf())
}

/// Capture the current process's control context — this process's `lf`, store,
/// and `LF_HOME` — for propagating down to a vendored subprocess. In a release
/// build this honors `LF_CONTROL_*`, so a running body hands its own Run context
/// (not the machine's Home) to the provider CLI it spawns.
///
/// This is NOT the launch resolver. Use [`current_home_execution_context`] to
/// launch or relaunch Work: launching through the control context would
/// perpetuate the historical binary a legacy body was created with.
pub(crate) fn pinned_execution_context() -> Result<crate::child::ChildExecutionContext> {
    let db_path = crate::store::database_path_from_env()
        .map_err(|error| anyhow!("cannot resolve the Run database path: {error}"))?;
    Ok(crate::child::ChildExecutionContext {
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
pub(crate) fn resolve_current_home_lf_binary() -> PathBuf {
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
pub(crate) fn resolve_current_home_lf_binary_checked() -> Result<PathBuf> {
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

/// Resolve the current Home execution context for launching Work: the
/// current Home `lf`, store, and `LF_HOME`, ignoring every `LF_CONTROL_*` pin.
///
/// This is the launch/relaunch boundary resolver. Work created under one
/// binary and resumed under another launches through the current Home — its
/// worktree, provider history, and directives are unaffected by which binary
/// first created it.
pub(crate) fn current_home_execution_context() -> Result<crate::child::ChildExecutionContext> {
    let db_path = crate::store::current_home_database_path()
        .map_err(|error| anyhow!("cannot resolve the current Home database path: {error}"))?;
    Ok(crate::child::ChildExecutionContext {
        lf_bin: resolve_current_home_lf_binary_checked()?,
        db_path,
        lf_home: crate::store::current_home_lf_home_dir(),
    })
}

pub(crate) fn which_on_path(name: &Path) -> Option<PathBuf> {
    let path = std::env::var_os("PATH")?;
    std::env::split_paths(&path)
        .map(|dir| dir.join(name))
        .find(|candidate| candidate.is_file())
}

pub(crate) fn shell_escape(value: &str) -> String {
    let escaped = value.replace('\'', "'\\''");
    format!("'{escaped}'")
}

pub(crate) async fn tmux_session_exists(session_name: &str) -> Result<bool> {
    let target = format!("={session_name}");
    let mut command = tokio::process::Command::new("tmux");
    command.args(["has-session", "-t", &target]);
    tmux_session_exists_with_timeout(&mut command, TMUX_LIVENESS_TIMEOUT).await
}

pub(crate) async fn send_tmux_input(session_name: &str, input: &str) -> Result<()> {
    let target = format!("={session_name}");
    let status = tokio::process::Command::new("tmux")
        .args(["send-keys", "-t", &target, "-l", "--", input])
        .status()
        .await?;
    if !status.success() {
        return Err(anyhow!(
            "failed to send input to tmux session {session_name}"
        ));
    }
    let status = tokio::process::Command::new("tmux")
        .args(["send-keys", "-t", &target, "Enter"])
        .status()
        .await?;
    if !status.success() {
        return Err(anyhow!(
            "failed to submit input to tmux session {session_name}"
        ));
    }
    Ok(())
}

pub(crate) async fn stop_tmux_session(session_name: &str) -> Result<()> {
    let target = format!("={session_name}");
    let status = tokio::process::Command::new("tmux")
        .args(["kill-session", "-t", &target])
        .status()
        .await?;
    if status.success() {
        return Ok(());
    }
    if !tmux_session_exists(session_name).await? {
        return Ok(());
    }
    Err(anyhow!("failed to stop tmux session {session_name}"))
}

async fn tmux_session_exists_with_timeout(
    command: &mut tokio::process::Command,
    timeout: std::time::Duration,
) -> Result<bool> {
    command
        .stdout(std::process::Stdio::null())
        .kill_on_drop(true);
    let output = match tokio::time::timeout(timeout, command.output()).await {
        Ok(Ok(output)) => output,
        Ok(Err(error)) => return Err(error.into()),
        Err(_) => return Err(anyhow!("tmux session probe timed out")),
    };
    if output.status.success() {
        return Ok(true);
    }
    let stderr = String::from_utf8_lossy(&output.stderr);
    if stderr.contains("can't find session")
        || stderr.contains("no server running")
        || stderr.contains("no sessions")
        || (stderr.contains("error connecting to") && stderr.contains("No such file or directory"))
    {
        Ok(false)
    } else {
        Err(anyhow!("tmux session probe failed: {}", stderr.trim()))
    }
}

pub(crate) async fn start_lf_session(session: &str, cwd: &Path, argv: &[String]) -> Result<()> {
    start_lf_session_with_env(session, cwd, argv, &[]).await
}

/// Start a machine-Home process through the current installed/dev control pair,
/// ignoring a historical body's `LF_CONTROL_*` pins.
pub(crate) async fn start_home_session(session: &str, cwd: &Path, argv: &[String]) -> Result<()> {
    let context = current_home_execution_context()?;
    let lf_bin = context.lf_bin.to_string_lossy().to_string();
    start_session_with_context(session, cwd, argv, &[("LF_BIN", &lf_bin)], context).await
}

pub(crate) async fn start_home_session_for_install_selection(
    session: &str,
    cwd: &Path,
    argv: &[String],
    selection: &crate::machine_install::InstallSelection,
    switch_id: Option<&str>,
) -> Result<()> {
    let cli = selection
        .artifact_set
        .artifact(&crate::machine_install::ArtifactRole::Cli)
        .ok_or_else(|| anyhow!("install switch target has no CLI"))?;
    cli.verify()?;
    let lf_home = selection
        .store
        .parent()
        .ok_or_else(|| anyhow!("install switch target store has no Home directory"))?
        .to_path_buf();
    let context = crate::child::ChildExecutionContext {
        lf_bin: cli.path.clone(),
        db_path: selection.store.clone(),
        lf_home,
    };
    let lf_bin = context.lf_bin.to_string_lossy().to_string();
    let mut environment = vec![("LF_BIN", lf_bin.as_str())];
    if let Some(switch_id) = switch_id {
        environment.push((crate::machine_install::INSTALL_SWITCH_ENV, switch_id));
    }
    start_session_with_context(session, cwd, argv, &environment, context).await
}

pub(crate) async fn start_lf_session_with_env(
    session: &str,
    cwd: &Path,
    argv: &[String],
    env: &[(&str, &str)],
) -> Result<()> {
    let context = pinned_execution_context()?;
    start_session_with_context(session, cwd, argv, env, context).await
}

async fn start_session_with_context(
    session: &str,
    cwd: &Path,
    argv: &[String],
    env: &[(&str, &str)],
    context: crate::child::ChildExecutionContext,
) -> Result<()> {
    let inherited_context = ["LF_TRACE_ID", "LF_PROCESS_ID"]
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

fn extend_session_control_context(
    child_env: &mut Vec<(String, String)>,
    context: &crate::child::ChildExecutionContext,
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
    let clear_context = "if [ -n \"${LF_FORWARDED_SECRET_NAMES:-}\" ]; then unset $LF_FORWARDED_SECRET_NAMES; fi; unset LF_TRACE_ID LF_PROCESS_ID LF_WAVE_ID LF_RUN_ID LF_INSTALL_SWITCH LF_BIN LF_HOME LF_DB_PATH LF_CONTROL_BIN LF_CONTROL_HOME LF_CONTROL_DB_PATH LF_ACCOUNT_LEASE LF_ACCOUNT_SELECTION LF_FORWARDED_PM_TOKEN LF_FORWARDED_PM_PROVIDER LF_FORWARDED_SECRET_NAMES LF_SSH_TARGET LF_LINEAR_WEBHOOK_SECRET LF_LINEAR_VIEWER_ID LF_GITHUB_WEBHOOK_SECRET LF_GITHUB_WEBHOOK_URL LF_LFD_ALLOW_NON_LOOPBACK LF_DISCORD_TOKEN GH_TOKEN OPENCODE_API_KEY CLAUDE_CODE_OAUTH_TOKEN ANTHROPIC_API_KEY CODEX_ACCESS_TOKEN OPENAI_API_KEY";
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
    let mut command = tokio::process::Command::new("tmux");
    for name in forwarded_authority_env_names() {
        command.env_remove(name);
    }
    let status = command
        .args([
            "new-session",
            "-d",
            "-s",
            session,
            "-c",
            cwd,
            "/bin/sh",
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

fn forwarded_authority_env_names() -> Vec<String> {
    let mut names = vec![
        crate::provider_account::lease::ACCOUNT_LEASE_ENV.to_string(),
        crate::provider_account::lease::ACCOUNT_SELECTION_ENV.to_string(),
        "LF_FORWARDED_PM_TOKEN".to_string(),
        "LF_FORWARDED_PM_PROVIDER".to_string(),
        "LF_FORWARDED_SECRET_NAMES".to_string(),
        crate::engine::process::SSH_TARGET_ENV.to_string(),
        "LF_LINEAR_WEBHOOK_SECRET".to_string(),
        "LF_LINEAR_VIEWER_ID".to_string(),
        "LF_GITHUB_WEBHOOK_SECRET".to_string(),
        "LF_GITHUB_WEBHOOK_URL".to_string(),
        "LF_LFD_ALLOW_NON_LOOPBACK".to_string(),
        DISCORD_TOKEN_ENV.to_string(),
        "GH_TOKEN".to_string(),
        "OPENCODE_API_KEY".to_string(),
        "CLAUDE_CODE_OAUTH_TOKEN".to_string(),
        "ANTHROPIC_API_KEY".to_string(),
        "CODEX_ACCESS_TOKEN".to_string(),
        "OPENAI_API_KEY".to_string(),
    ];
    if let Ok(forwarded) = std::env::var("LF_FORWARDED_SECRET_NAMES") {
        names.extend(forwarded.split_whitespace().map(str::to_string));
    }
    names
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
    use std::path::PathBuf;

    use super::{
        extend_session_control_context, forwarded_authority_env_names, lf_session_shell_command,
        pin_control_binary, select_binary_override, select_current_home_binary, select_lfd_binary,
        tmux_session_exists_with_timeout, DISCORD_TOKEN_ENV,
    };
    use crate::build_info::BuildProvenance;
    use crate::child::ChildExecutionContext;

    #[tokio::test]
    async fn hanging_tmux_probe_is_bounded() {
        let mut command = tokio::process::Command::new("/bin/sh");
        command.args(["-c", "sleep 5"]);

        let started = tokio::time::Instant::now();
        let result =
            tmux_session_exists_with_timeout(&mut command, std::time::Duration::from_millis(20))
                .await;

        assert!(result.is_err());
        assert!(started.elapsed() < std::time::Duration::from_secs(1));
    }

    #[tokio::test]
    async fn missing_tmux_socket_means_no_session() {
        let mut command = tokio::process::Command::new("/bin/sh");
        command.args([
            "-c",
            "echo 'error connecting to /tmp/tmux-1001/default (No such file or directory)' >&2; exit 1",
        ]);

        let exists =
            tmux_session_exists_with_timeout(&mut command, std::time::Duration::from_millis(100))
                .await
                .unwrap();

        assert!(!exists);
    }

    #[test]
    fn a_body_generation_keeps_one_binary_across_a_global_repoint() {
        let dir = tempfile::tempdir().unwrap();
        let old = dir.path().join("lf-old");
        let new = dir.path().join("lf-new");
        let installed = dir.path().join("lf");
        std::fs::write(&old, b"old").unwrap();
        std::fs::write(&new, b"new").unwrap();
        std::os::unix::fs::symlink(&old, &installed).unwrap();

        let pinned = pin_control_binary(&installed);
        assert_eq!(pinned, std::fs::canonicalize(&old).unwrap());
        std::fs::remove_file(&installed).unwrap();
        std::os::unix::fs::symlink(&new, &installed).unwrap();

        assert_eq!(std::fs::read(&pinned).unwrap(), b"old");
        assert_eq!(
            std::fs::read(std::fs::canonicalize(&installed).unwrap()).unwrap(),
            b"new"
        );
    }

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

    #[test]
    fn release_daemon_resolution_prefers_the_promoted_target_over_a_stale_store_sibling() {
        assert_eq!(
            select_lfd_binary(
                BuildProvenance::Release,
                None,
                Some(PathBuf::from("/home/op/.lf/bin/lfd")),
                None,
                Some(PathBuf::from("/home/op/.local/bin/lfd")),
                None,
            ),
            PathBuf::from("/home/op/.local/bin/lfd")
        );
        assert_eq!(
            select_lfd_binary(
                BuildProvenance::Development,
                None,
                Some(PathBuf::from("/repo/target/debug/lfd")),
                None,
                Some(PathBuf::from("/home/op/.local/bin/lfd")),
                None,
            ),
            PathBuf::from("/repo/target/debug/lfd")
        );
    }

    /// The launch boundary must resolve the current Home lf (B), never the
    /// historical `LF_CONTROL_BIN` pin (A) — the regression behind stranded
    /// legacy Work bodies. Contrast the two selectors under release provenance:
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

    #[test]
    fn lf_session_clears_parent_identity_and_exports_its_own() {
        let argv = vec![
            "lf".to_string(),
            "__work".to_string(),
            "task".to_string(),
            "tsk_123".to_string(),
        ];
        let command = lf_session_shell_command(&argv, &[("LF_WAVE_ID", "infra")]);

        assert!(command.starts_with(
            "if [ -n \"${LF_FORWARDED_SECRET_NAMES:-}\" ]; then unset $LF_FORWARDED_SECRET_NAMES; fi; unset "
        ));
        assert!(command.contains("LF_WAVE_ID LF_RUN_ID LF_INSTALL_SWITCH"));
        assert!(command.contains("LF_INSTALL_SWITCH LF_BIN"));
        assert!(command.contains("LF_ACCOUNT_LEASE LF_ACCOUNT_SELECTION"));
        assert!(command.contains("LF_DISCORD_TOKEN"));
        assert!(command.contains("GH_TOKEN OPENCODE_API_KEY"));
        assert!(command.ends_with("exec env 'LF_WAVE_ID'='infra' 'lf' '__work' 'task' 'tsk_123'"));
    }

    #[test]
    fn lf_session_without_explicit_identity_does_not_inherit_its_parent() {
        let argv = vec!["lf".to_string(), "wave".to_string(), "child".to_string()];

        let command = lf_session_shell_command(&argv, &[]);

        assert!(command.contains("LF_WAVE_ID LF_RUN_ID"));
        assert!(command.contains("LF_ACCOUNT_LEASE LF_ACCOUNT_SELECTION"));
        assert!(command.ends_with("exec 'lf' 'wave' 'child'"));
    }

    #[test]
    fn durable_session_scrubs_every_named_forwarded_secret() {
        let _lock = crate::journal::test_env_lock();
        let previous = std::env::var_os("LF_FORWARDED_SECRET_NAMES");
        std::env::set_var("LF_FORWARDED_SECRET_NAMES", "SENTRY_TOKEN STRIPE_KEY");

        let names = forwarded_authority_env_names();

        match previous {
            Some(value) => std::env::set_var("LF_FORWARDED_SECRET_NAMES", value),
            None => std::env::remove_var("LF_FORWARDED_SECRET_NAMES"),
        }
        assert!(names.iter().any(|name| name == "LF_ACCOUNT_LEASE"));
        assert!(names.iter().any(|name| name == "GH_TOKEN"));
        assert!(names.iter().any(|name| name == "LF_LINEAR_WEBHOOK_SECRET"));
        assert!(names.iter().any(|name| name == "LF_LFD_ALLOW_NON_LOOPBACK"));
        assert!(names.iter().any(|name| name == "LF_DISCORD_TOKEN"));
        assert!(names.iter().any(|name| name == "SENTRY_TOKEN"));
        assert!(names.iter().any(|name| name == "STRIPE_KEY"));
    }

    #[test]
    fn discord_chat_token_is_scrubbed_from_durable_provider_children() {
        assert!(forwarded_authority_env_names()
            .iter()
            .any(|name| name == DISCORD_TOKEN_ENV));
        let command = lf_session_shell_command(&["lf".into(), "wave".into()], &[]);
        assert!(command.contains("unset "));
        assert!(command.contains(DISCORD_TOKEN_ENV));
    }

    #[test]
    fn lf_session_replaces_tmux_invocation_context() {
        let argv = vec![
            "lf".to_string(),
            "__work".to_string(),
            "task".to_string(),
            "tsk_123".to_string(),
        ];
        let command = lf_session_shell_command(
            &argv,
            &[
                ("LF_TRACE_ID", "run-1"),
                ("LF_PROCESS_ID", "process-1"),
                ("LF_DB_PATH", "/tmp/current.db"),
                ("LF_HOME", "/tmp/lf"),
            ],
        );

        assert!(command.contains("LF_WAVE_ID LF_RUN_ID"));
        assert!(command.contains("LF_ACCOUNT_LEASE LF_ACCOUNT_SELECTION"));
        assert!(command.ends_with(
            "exec env 'LF_TRACE_ID'='run-1' 'LF_PROCESS_ID'='process-1' 'LF_DB_PATH'='/tmp/current.db' 'LF_HOME'='/tmp/lf' 'lf' '__work' 'task' 'tsk_123'"
        ));
    }
}
