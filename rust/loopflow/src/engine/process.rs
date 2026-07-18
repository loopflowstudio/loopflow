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
pub(crate) fn pinned_execution_context() -> Result<crate::child::ChildExecutionContext> {
    let db_path = crate::store::database_path_from_env()
        .map_err(|error| anyhow!("cannot resolve the Session's database path: {error}"))?;
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
pub(crate) fn current_home_execution_context() -> Result<crate::child::ChildExecutionContext> {
    let db_path = crate::store::current_home_database_path()
        .map_err(|error| anyhow!("cannot resolve the current Home database path: {error}"))?;
    Ok(crate::child::ChildExecutionContext {
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
    let clear_context = "unset LF_TRACE_ID LF_PROCESS_ID LF_WAVE_ID LF_CHANNEL LF_RUN_CONTEXT LF_RUN_LEASE LF_BIN LF_HOME LF_DB_PATH LF_CONTROL_BIN LF_CONTROL_HOME LF_CONTROL_DB_PATH";
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
    use std::path::PathBuf;

    use super::{
        extend_session_control_context, lf_session_shell_command,
        reject_detached_forwarded_account, select_binary_override, select_current_home_binary,
        tmux_installed,
    };
    use crate::build_info::BuildProvenance;
    use crate::child::ChildExecutionContext;

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
            &[("LF_RUN_CONTEXT", "agent"), ("LF_WAVE_ID", "infra")],
        );

        assert_eq!(
            command,
            "unset LF_TRACE_ID LF_PROCESS_ID LF_WAVE_ID LF_CHANNEL LF_RUN_CONTEXT LF_RUN_LEASE LF_BIN LF_HOME LF_DB_PATH LF_CONTROL_BIN LF_CONTROL_HOME LF_CONTROL_DB_PATH; exec env 'LF_RUN_CONTEXT'='agent' 'LF_WAVE_ID'='infra' 'lf' '__task'"
        );
    }

    #[test]
    fn lf_session_without_explicit_identity_does_not_inherit_its_parent() {
        let argv = vec!["lf".to_string(), "wave".to_string(), "child".to_string()];

        let command = lf_session_shell_command(&argv, &[]);

        assert_eq!(
            command,
            "unset LF_TRACE_ID LF_PROCESS_ID LF_WAVE_ID LF_CHANNEL LF_RUN_CONTEXT LF_RUN_LEASE LF_BIN LF_HOME LF_DB_PATH LF_CONTROL_BIN LF_CONTROL_HOME LF_CONTROL_DB_PATH; exec 'lf' 'wave' 'child'"
        );
    }

    #[test]
    fn lf_session_replaces_tmux_invocation_context() {
        let argv = vec!["lf".to_string(), "__task".to_string()];
        let command = lf_session_shell_command(
            &argv,
            &[
                ("LF_TRACE_ID", "run-1"),
                ("LF_PROCESS_ID", "process-1"),
                ("LF_DB_PATH", "/tmp/current.db"),
                ("LF_HOME", "/tmp/lf"),
            ],
        );

        assert_eq!(
            command,
            "unset LF_TRACE_ID LF_PROCESS_ID LF_WAVE_ID LF_CHANNEL LF_RUN_CONTEXT LF_RUN_LEASE LF_BIN LF_HOME LF_DB_PATH LF_CONTROL_BIN LF_CONTROL_HOME LF_CONTROL_DB_PATH; exec env 'LF_TRACE_ID'='run-1' 'LF_PROCESS_ID'='process-1' 'LF_DB_PATH'='/tmp/current.db' 'LF_HOME'='/tmp/lf' 'lf' '__task'"
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
}
