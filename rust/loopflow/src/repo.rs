//! Repository-root discovery shared across architectural layers.

use anyhow::{anyhow, Context, Result};
use std::path::{Path, PathBuf};

pub fn discover_repo_root(cwd: &Path) -> Result<Option<PathBuf>> {
    let has_git_context = std::env::var_os("GIT_DIR").is_some()
        || cwd
            .ancestors()
            .any(|path| path.join(".git").symlink_metadata().is_ok());
    let output = match std::process::Command::new("git")
        .args(["rev-parse", "--show-toplevel"])
        .env("LC_ALL", "C")
        .current_dir(cwd)
        .output()
    {
        Ok(output) => output,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound && !has_git_context => {
            std::fs::metadata(cwd).context("read the working directory")?;
            return Ok(None);
        }
        Err(error) => return Err(error).context("discover the current Git repository"),
    };
    if output.status.success() {
        return Ok(Some(PathBuf::from(
            String::from_utf8(output.stdout)?.trim(),
        )));
    }
    let error = String::from_utf8_lossy(&output.stderr);
    if !has_git_context && error.contains("not a git repository") {
        return Ok(None);
    }
    Err(anyhow!(
        "cannot discover Git repository in {}: {}",
        cwd.display(),
        error.trim()
    ))
}

pub fn require_repo_root(cwd: &Path, operation: &str) -> Result<PathBuf> {
    discover_repo_root(cwd)?
        .ok_or_else(|| anyhow!("Run {operation} from a Git repository you want to update."))
}

/// Resolve directory-scoped context without requiring Git metadata.
pub fn working_directory() -> Result<PathBuf> {
    let cwd = std::env::current_dir()?;
    Ok(discover_repo_root(&cwd)?.unwrap_or(cwd))
}

pub fn find_repo_root() -> Result<PathBuf> {
    require_repo_root(&std::env::current_dir()?, "this command")
}
