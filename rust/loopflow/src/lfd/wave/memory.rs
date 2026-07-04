//! The wave's durable shared brain: `wave/<name>/MEMORY.md`.
//!
//! The server reads it to answer chat and to seed each progress pass. It is
//! deliberately curated — the mind updates it when it learns something, not
//! mechanically per turn (the journal carries the raw history; see
//! [`super::journal`]). Nothing in the server writes it today; the write side
//! arrives with the mind phase, as `MemoryUpdated` journal events alongside
//! the file edit. It is a plain Markdown file — not an IPC channel. Concerto
//! never reads it; the thread is the live surface.

use std::path::{Path, PathBuf};

/// Read handle to a wave's `MEMORY.md`.
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

    /// A short head of memory for chat context — the first `max_lines`
    /// non-empty lines. MEMORY can grow large; chat only needs the gist.
    pub fn head(&self, max_lines: usize) -> String {
        self.read()
            .lines()
            .filter(|l| !l.trim().is_empty())
            .take(max_lines)
            .collect::<Vec<_>>()
            .join("\n")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn head_returns_leading_nonempty_lines() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let dir = tmp.path().join("wave/ship");
        std::fs::create_dir_all(&dir).expect("dir");
        std::fs::write(dir.join("MEMORY.md"), "one\n\ntwo\nthree\n").expect("seed");

        let memory = Memory::for_wave(tmp.path(), "ship");
        assert_eq!(memory.head(2), "one\ntwo");
    }

    #[test]
    fn read_missing_file_is_empty() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let memory = Memory::for_wave(tmp.path(), "ghost");
        assert_eq!(memory.read(), "");
    }
}
