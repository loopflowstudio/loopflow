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
        let file = self
            .files
            .entry(wave_run_id.to_string())
            .or_insert_with(|| {
                OpenOptions::new()
                    .create(true)
                    .append(true)
                    .open(self.dir.join(format!("{wave_run_id}.log")))
                    .expect("failed to open output log file")
            });
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
        let path = self.output_dir.join(format!("{wave_run_id}.log"));
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
