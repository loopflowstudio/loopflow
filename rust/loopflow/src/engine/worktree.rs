use std::path::Path;
use std::process::Command;

use crate::engine::error::CoreError;

pub fn find_worktree_root(path: &Path) -> Result<String, CoreError> {
    let output = Command::new("git")
        .arg("-C")
        .arg(path)
        .arg("rev-parse")
        .arg("--show-toplevel")
        .output()?;
    if !output.status.success() {
        return Err(CoreError::WorktreeError("git rev-parse failed".to_string()));
    }
    Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
}
