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

pub(crate) fn shell_escape(value: &str) -> String {
    let escaped = value.replace('\'', "'\\''");
    format!("'{escaped}'")
}

pub(crate) async fn tmux_session_exists(session_name: &str) -> Result<bool> {
    let status = tokio::process::Command::new("tmux")
        .args(["has-session", "-t", session_name])
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
    let shell_command = lf_session_shell_command(argv, env);
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
    let clear_identity = "unset LFD_SESSION_INHERITED LFD_SESSION_ID LFD_WAVE_ID LFD_CHANNEL LFD_PROJECT_SESSION_ID LFD_PROJECT_GENERATION LFD_TASK_SESSION_ID LFD_TASK_GENERATION LF_TASK_SESSION_ID LF_TASK_GENERATION";
    if env.is_empty() {
        format!("{clear_identity}; exec {command}")
    } else {
        format!("{clear_identity}; exec env {env} {command}")
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
    use super::lf_session_shell_command;

    #[test]
    fn lf_session_clears_parent_identity_and_exports_its_own() {
        let argv = vec!["lf".to_string(), "__task".to_string()];
        let command = lf_session_shell_command(
            &argv,
            &[("LFD_TASK_SESSION_ID", "task-1"), ("LFD_WAVE_ID", "infra")],
        );

        assert_eq!(
            command,
            "unset LFD_SESSION_INHERITED LFD_SESSION_ID LFD_WAVE_ID LFD_CHANNEL LFD_PROJECT_SESSION_ID LFD_PROJECT_GENERATION LFD_TASK_SESSION_ID LFD_TASK_GENERATION LF_TASK_SESSION_ID LF_TASK_GENERATION; exec env 'LFD_TASK_SESSION_ID'='task-1' 'LFD_WAVE_ID'='infra' 'lf' '__task'"
        );
    }

    #[test]
    fn lf_session_without_explicit_identity_does_not_inherit_its_parent() {
        let argv = vec!["lf".to_string(), "serve".to_string(), "child".to_string()];

        let command = lf_session_shell_command(&argv, &[]);

        assert_eq!(
            command,
            "unset LFD_SESSION_INHERITED LFD_SESSION_ID LFD_WAVE_ID LFD_CHANNEL LFD_PROJECT_SESSION_ID LFD_PROJECT_GENERATION LFD_TASK_SESSION_ID LFD_TASK_GENERATION LF_TASK_SESSION_ID LF_TASK_GENERATION; exec 'lf' 'serve' 'child'"
        );
    }
}
