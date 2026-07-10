use std::path::Path;
use std::process::{Command, Output};

use crate::engine::git::current_branch;
use crate::engine::identity::WaveId;
use crate::engine::naming::git_user;
use crate::engine::worktrees::wave_name_from_worktree;

pub fn command_exists(name: &str) -> bool {
    Command::new(name)
        .arg("--version")
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status()
        .map(|status| status.success())
        .unwrap_or(false)
}

pub fn stderr_from_output(output: &Output) -> String {
    String::from_utf8_lossy(&output.stderr).trim().to_string()
}

/// Resolve the wave name for workflow operations.
///
/// Priority: explicit wave name > worktree directory name > branch name component.
pub fn resolve_wave_name(repo: &Path, explicit: Option<&str>) -> Option<String> {
    if let Some(name) = explicit.and_then(normalize_wave_name) {
        return Some(name);
    }
    if let Some(name) = wave_name_from_worktree(repo).and_then(|name| normalize_wave_name(&name)) {
        return Some(name);
    }
    if let Ok(Some(branch)) = current_branch(repo) {
        let user = git_user(repo).unwrap_or_else(|_| "user".to_string());
        return WaveId::parse(&branch, &user)
            .map(|id| id.wave_name().to_string())
            .and_then(|name| normalize_wave_name(&name));
    }
    None
}

pub fn normalize_wave_name(value: &str) -> Option<String> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        return None;
    }
    let normalized = trimmed
        .strip_prefix("wave/")
        .unwrap_or(trimmed)
        .trim_matches('/');
    if normalized.is_empty() {
        None
    } else {
        Some(normalized.to_string())
    }
}
