//! What remains of the executor after the organ cut: the dispatch helpers
//! `lf q` shares (worktree placement, the tmux wrapper), palette terminal
//! sessions, boot session reconciliation, and the worktree janitor.
//!
//! The agent-spawning engines (docker/local runners), the run-execution
//! chains, and the repair machinery died with the trigger organs — dispatch
//! is `lf q worker run`'s job now, and every worker is a tmux-wrapped `lf`
//! process that registers its own session row.

// TODO(M1): move the remaining session reconciliation and worktree janitor
// mechanisms to the session/worktree owners; keep cleanup idempotent for both
// git worktrees and plain directories.
pub(crate) mod helpers;
pub(crate) mod wave;

use std::path::Path;

use anyhow::Result;

pub(crate) use helpers::resolve_lf_binary;
pub use helpers::{create_run_for_placement, ensure_wave_worktree, Placement};
pub use wave::WaveExecutor;

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct JanitorReport {
    pub removed: u32,
    pub active: u32,
    pub errors: u32,
}

// -- Workspace file helpers (used by `lf` fork steps) ------------------------

pub(crate) fn write_workspace_file(cwd: &Path, relative_path: &str, content: &[u8]) -> Result<()> {
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
