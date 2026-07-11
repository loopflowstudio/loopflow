use std::path::Path;

use anyhow::Result;

pub fn ensure_wave_worktree(main_repo: &Path, wave_name: &str) -> Result<(String, String)> {
    let lease = crate::engine::worktrees::ensure_wave_worktree(main_repo, wave_name)?;
    Ok((lease.path.to_string_lossy().to_string(), lease.branch))
}

pub(crate) fn write_workspace_file(
    cwd: &Path,
    relative_path: &str,
    content: &[u8],
) -> Result<()> {
    let path = cwd.join(relative_path);
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    std::fs::write(path, content)?;
    Ok(())
}

pub(crate) fn remove_workspace_file(cwd: &Path, relative_path: &str) -> Result<()> {
    let path = cwd.join(relative_path);
    match std::fs::remove_file(path) {
        Ok(()) => Ok(()),
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(err) => Err(err.into()),
    }
}

pub(crate) fn cleanup_workspace_worktree(worktree: &Path) -> Result<()> {
    if !worktree.exists() {
        return Ok(());
    }

    if worktree.join(".git").exists() {
        crate::engine::worktree::remove_worktree(worktree, true)?;
    } else {
        std::fs::remove_dir_all(worktree)?;
    }
    Ok(())
}
