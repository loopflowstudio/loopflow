//! Output streaming hub with file-backed persistence.
//!
//! Output lines are broadcast in-memory AND appended to per-run log files
//! at `~/.lf/output/<wave_run_id>.log`. The logs endpoint replays from
//! the file, then follows live — no race conditions.

use std::collections::HashMap;
use std::fs::{self, File, OpenOptions};
use std::io::{BufRead, BufReader, Write};
use std::path::{Path, PathBuf};
use std::sync::Mutex;

use tokio::sync::broadcast;
use tracing::warn;

use crate::lfd::security::{path_within_root_existing, path_within_root_planned, validate_safe_id};

#[derive(Debug, Clone)]
pub struct OutputEvent {
    pub wave_id: String,
    pub wave_run_id: String,
    pub agent_id: String,
    pub text: String,
}

/// File writer cache — one writer per wave_run_id.
struct Writers {
    dir: PathBuf,
    files: HashMap<String, File>,
}

impl Writers {
    fn new(dir: PathBuf) -> Self {
        fs::create_dir_all(&dir).ok();
        Self {
            dir,
            files: HashMap::new(),
        }
    }

    fn append(&mut self, wave_run_id: &str, line: &str) {
        let Some(relative) = relative_log_path(wave_run_id) else {
            warn!(wave_run_id = %wave_run_id, "rejecting unsafe wave_run_id for output log");
            return;
        };

        let file = if let Some(file) = self.files.get_mut(wave_run_id) {
            file
        } else {
            let path = match path_within_root_planned(&self.dir, &relative) {
                Ok(path) => path,
                Err(err) => {
                    warn!(wave_run_id = %wave_run_id, error = %err, "failed to resolve output log path");
                    return;
                }
            };
            let file = match OpenOptions::new().create(true).append(true).open(path) {
                Ok(file) => file,
                Err(err) => {
                    warn!(wave_run_id = %wave_run_id, error = %err, "failed to open output log file");
                    return;
                }
            };
            self.files.entry(wave_run_id.to_string()).or_insert(file)
        };
        let _ = writeln!(file, "{line}");
    }
}

#[derive(Clone)]
pub struct OutputHub {
    sender: broadcast::Sender<OutputEvent>,
    writers: std::sync::Arc<Mutex<Writers>>,
    output_dir: PathBuf,
}

impl std::fmt::Debug for OutputHub {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("OutputHub")
            .field("output_dir", &self.output_dir)
            .finish()
    }
}

impl OutputHub {
    pub fn new(buffer: usize, output_dir: PathBuf) -> Self {
        let (sender, _) = broadcast::channel(buffer);
        let writers = std::sync::Arc::new(Mutex::new(Writers::new(output_dir.clone())));
        Self {
            sender,
            writers,
            output_dir,
        }
    }

    /// Append to log file, then broadcast. File write happens first so
    /// that replay-then-follow has a clean dedup boundary.
    pub fn send(&self, event: OutputEvent) {
        if let Ok(mut w) = self.writers.lock() {
            w.append(&event.wave_run_id, &event.text);
        }
        let _ = self.sender.send(event);
    }

    pub fn subscribe(&self) -> broadcast::Receiver<OutputEvent> {
        self.sender.subscribe()
    }

    /// Read the log file for a wave run. Returns lines and the byte offset
    /// at end of read (for dedup with the broadcast stream).
    pub fn read_log(&self, wave_run_id: &str) -> Option<(Vec<String>, u64)> {
        let relative = relative_log_path(wave_run_id)?;
        let path = path_within_root_existing(&self.output_dir, &relative).ok()?;
        let file = File::open(&path).ok()?;
        let metadata = file.metadata().ok()?;
        let size = metadata.len();
        let reader = BufReader::new(file);
        let lines: Vec<String> = reader.lines().map_while(Result::ok).collect();
        Some((lines, size))
    }

    /// Output directory path.
    pub fn output_dir(&self) -> &Path {
        &self.output_dir
    }
}

fn relative_log_path(wave_run_id: &str) -> Option<PathBuf> {
    validate_safe_id(wave_run_id).ok()?;
    Some(PathBuf::from(format!("{wave_run_id}.log")))
}

#[cfg(test)]
mod tests {
    use super::{OutputEvent, OutputHub};
    use tempfile::tempdir;

    #[test]
    fn read_log_returns_none_for_unsafe_wave_run_id() {
        let tmp = tempdir().expect("tempdir");
        let hub = OutputHub::new(8, tmp.path().to_path_buf());
        assert!(hub.read_log("../escape").is_none());
    }

    #[test]
    fn send_ignores_unsafe_wave_run_id_without_creating_file() {
        let tmp = tempdir().expect("tempdir");
        let hub = OutputHub::new(8, tmp.path().to_path_buf());
        let outside = tmp.path().parent().expect("parent").join("escape.log");

        hub.send(OutputEvent {
            wave_id: "wave-1".to_string(),
            wave_run_id: "../escape".to_string(),
            agent_id: "agent-1".to_string(),
            text: "nope".to_string(),
        });

        assert!(!outside.exists());
    }
}
