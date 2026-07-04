//! The wave's durable shared brain: `wave/<name>/MEMORY.md`.
//!
//! The server reads it to answer chat and to seed each progress pass, and folds
//! durable facts back into it as progress narrates. It is a plain Markdown file
//! — not an IPC channel. Concerto never reads it; the thread is the live surface.

use std::path::{Path, PathBuf};
use std::sync::Mutex;

use time::OffsetDateTime;

const LOG_HEADER: &str = "## Wave log";

/// Handle to a wave's `MEMORY.md`. Cheap to clone the path; writes are
/// serialized through an internal lock so concurrent progress folds don't
/// interleave.
#[derive(Debug)]
pub struct Memory {
    path: PathBuf,
    write_lock: Mutex<()>,
}

impl Memory {
    /// `wave/<name>/MEMORY.md`, resolved against the repo root.
    pub fn for_wave(repo_root: &Path, wave: &str) -> Self {
        Self {
            path: repo_root.join("wave").join(wave).join("MEMORY.md"),
            write_lock: Mutex::new(()),
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

    /// Fold one durable fact into memory as a timestamped bullet under the
    /// wave log. Best-effort: a write failure is logged, not propagated — a
    /// dropped fact must not stall progress.
    pub fn fold_fact(&self, fact: &str) {
        let fact = fact.trim();
        if fact.is_empty() {
            return;
        }
        let _guard = self.write_lock.lock().expect("memory lock poisoned");
        if let Err(err) = self.append_bullet(fact) {
            tracing::warn!(error = %err, path = %self.path.display(), "failed to fold fact into MEMORY");
        }
    }

    fn append_bullet(&self, fact: &str) -> std::io::Result<()> {
        if let Some(parent) = self.path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let mut body = self.read();
        if !body.contains(LOG_HEADER) {
            if !body.is_empty() && !body.ends_with('\n') {
                body.push('\n');
            }
            if !body.is_empty() {
                body.push('\n');
            }
            body.push_str(LOG_HEADER);
            body.push('\n');
        }
        if !body.ends_with('\n') {
            body.push('\n');
        }
        let stamp = OffsetDateTime::now_utc()
            .format(&time::format_description::well_known::Rfc3339)
            .unwrap_or_default();
        body.push_str(&format!("- {stamp} — {}\n", one_line(fact)));
        std::fs::write(&self.path, body)
    }
}

/// Collapse a multi-line fact into a single bullet-friendly line.
fn one_line(text: &str) -> String {
    text.split_whitespace().collect::<Vec<_>>().join(" ")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fold_fact_appends_bullet_under_log_header() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let memory = Memory::for_wave(tmp.path(), "ship");
        memory.fold_fact("landed auth");
        memory.fold_fact("wired chat");

        let body = memory.read();
        assert!(body.contains(LOG_HEADER));
        assert!(body.contains("landed auth"));
        assert!(body.contains("wired chat"));
        // Header appears exactly once even across folds.
        assert_eq!(body.matches(LOG_HEADER).count(), 1);
    }

    #[test]
    fn fold_fact_preserves_existing_body() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let dir = tmp.path().join("wave/ship");
        std::fs::create_dir_all(&dir).expect("dir");
        std::fs::write(dir.join("MEMORY.md"), "# Ship\n\nGoal: ship it.\n").expect("seed");

        let memory = Memory::for_wave(tmp.path(), "ship");
        memory.fold_fact("first pass done");

        let body = memory.read();
        assert!(body.contains("Goal: ship it."));
        assert!(body.contains("first pass done"));
    }

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
