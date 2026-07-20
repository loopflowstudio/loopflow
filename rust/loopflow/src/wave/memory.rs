//! The wave's durable shared brain: `wave/<name>/MEMORY.md`.
//!
//! Prompt assembly reads this file directly from the Wave's origin repository.
//! It has no journal mirror, live delta, server route, or replay protocol.

use std::path::{Path, PathBuf};

/// Read-only handle to a Wave's `MEMORY.md`.
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

    /// Current contents, or empty string if the file doesn't exist yet.
    pub fn read(&self) -> String {
        std::fs::read_to_string(&self.path).unwrap_or_default()
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
