use std::path::{Path, PathBuf};

use anyhow::{anyhow, Result};

pub(crate) fn resolve_lf_binary() -> PathBuf {
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

pub(crate) async fn start_lf_session(session: &str, cwd: &Path, argv: &[String]) -> Result<()> {
    start_lf_session_with_env(session, cwd, argv, &[]).await
}

pub(crate) async fn start_lf_session_with_env(
    session: &str,
    cwd: &Path,
    argv: &[String],
    env: &[(&str, &str)],
) -> Result<()> {
    let inherited_context = ["LF_RUN_ID", "LF_PROCESS_ID", "LF_HOME", "LF_DB_PATH"]
        .into_iter()
        .filter(|key| !env.iter().any(|(explicit, _)| explicit == key))
        .filter_map(|key| std::env::var(key).ok().map(|value| (key, value)))
        .collect::<Vec<_>>();
    let mut child_env = env.to_vec();
    child_env.extend(
        inherited_context
            .iter()
            .map(|(key, value)| (*key, value.as_str())),
    );
    let shell_command = lf_session_shell_command(argv, &child_env);
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
    let clear_context = "unset LF_RUN_ID LF_PROCESS_ID LF_WAVE_ID LF_CHANNEL LF_PROJECT_SESSION_ID LF_PROJECT_GENERATION LF_TASK_SESSION_ID LF_TASK_GENERATION LF_HOME LF_DB_PATH";
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
    use super::{lf_session_shell_command, tmux_installed};

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
            "unset LF_RUN_ID LF_PROCESS_ID LF_WAVE_ID LF_CHANNEL LF_PROJECT_SESSION_ID LF_PROJECT_GENERATION LF_TASK_SESSION_ID LF_TASK_GENERATION LF_HOME LF_DB_PATH; exec env 'LF_TASK_SESSION_ID'='task-1' 'LF_WAVE_ID'='infra' 'lf' '__task'"
        );
    }

    #[test]
    fn lf_session_without_explicit_identity_does_not_inherit_its_parent() {
        let argv = vec!["lf".to_string(), "wave".to_string(), "child".to_string()];

        let command = lf_session_shell_command(&argv, &[]);

        assert_eq!(
            command,
            "unset LF_RUN_ID LF_PROCESS_ID LF_WAVE_ID LF_CHANNEL LF_PROJECT_SESSION_ID LF_PROJECT_GENERATION LF_TASK_SESSION_ID LF_TASK_GENERATION LF_HOME LF_DB_PATH; exec 'lf' 'wave' 'child'"
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
            "unset LF_RUN_ID LF_PROCESS_ID LF_WAVE_ID LF_CHANNEL LF_PROJECT_SESSION_ID LF_PROJECT_GENERATION LF_TASK_SESSION_ID LF_TASK_GENERATION LF_HOME LF_DB_PATH; exec env 'LF_RUN_ID'='run-1' 'LF_PROCESS_ID'='process-1' 'LF_DB_PATH'='/tmp/current.db' 'LF_HOME'='/tmp/lf' 'lf' '__task'"
        );
    }
}
