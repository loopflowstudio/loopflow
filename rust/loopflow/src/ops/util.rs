use std::process::{Command, Output};

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

/// Normalize an explicitly selected Wave name for workflow operations.
pub fn resolve_wave_name(explicit: Option<&str>) -> Option<String> {
    explicit.and_then(normalize_wave_name)
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
