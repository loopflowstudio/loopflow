use std::path::Path;
use std::process::Command;

use crate::error::CoreError;

pub fn get_status(repo: &Path) -> Result<String, CoreError> {
    let output = Command::new("git")
        .arg("-C")
        .arg(repo)
        .arg("status")
        .arg("--porcelain")
        .output()?;
    if !output.status.success() {
        return Err(CoreError::ExecutionFailed("git status failed".to_string()));
    }
    Ok(String::from_utf8_lossy(&output.stdout).to_string())
}

pub fn get_diff(repo: &Path) -> Result<String, CoreError> {
    let output = Command::new("git")
        .arg("-C")
        .arg(repo)
        .arg("diff")
        .output()?;
    if !output.status.success() {
        return Err(CoreError::ExecutionFailed("git diff failed".to_string()));
    }
    Ok(String::from_utf8_lossy(&output.stdout).to_string())
}
