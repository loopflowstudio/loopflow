//! The wave's durable shared brain: `wave/<name>/MEMORY.md`.
//!
//! The server reads it to answer chat and to seed each progress pass. It is
//! deliberately curated — the loop updates it when it learns something, not
//! mechanically per turn (the journal carries the raw history; see
//! [`super::journal`]). The live server holds the pen: `lf memory update` →
//! the server's memory route → [`super::runtime::WaveRuntime::update_memory`],
//! which journals `MemoryUpdated` alongside the file edit. `lf memory add`
//! publishes a replayable fact without accreting raw bullets into this file.
//! Direct file edits are for serverless waves only (the loop's file tools
//! editing a worktree copy while seeds read the origin's was a live bug). It
//! is a plain Markdown file — not an IPC channel. Loopflow never reads it; the
//! thread is the live surface.

use std::path::{Path, PathBuf};

/// Handle to a wave's `MEMORY.md`. Reads are free; writes belong to the
/// runtime (one pen, under its lock).
#[derive(Debug)]
pub struct Memory {
    path: PathBuf,
}

impl Memory {
    /// `wave/<name>/MEMORY.md`, resolved against the repo root.
    pub fn for_wave(repo_root: &Path, wave: &str) -> Self {
        Self {
            path: repo_root.join("wave").join(wave).join("MEMORY.md"),
        }
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    /// Current contents, or empty string if the file doesn't exist yet.
    pub fn read(&self) -> String {
        std::fs::read_to_string(&self.path).unwrap_or_default()
    }

    /// Replace the file's contents, creating `wave/<name>/` if needed. Called
    /// only by the runtime, which journals `MemoryUpdated` under its lock.
    ///
    /// # Errors
    /// File I/O.
    pub fn write(&self, content: &str) -> std::io::Result<()> {
        if let Some(parent) = self.path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        std::fs::write(&self.path, content)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn read_missing_file_is_empty() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let memory = Memory::for_wave(tmp.path(), "ghost");
        assert_eq!(memory.read(), "");
    }
}
