//! File-backed run output logging.
//!
//! Output lines are appended to per-run log files at `~/.lf/output/<run_id>.log`;
//! the logs endpoint replays from the file. The old in-process broadcast bus
//! (which fed the deleted `/ws` output stream) is gone — waves write their own
//! logs and a client reads them from disk. No streaming center.

use std::collections::HashMap;
use std::fs::{self, File, OpenOptions};
use std::io::{BufRead, BufReader, Write};
use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime};

use tracing::warn;

use crate::lfd::security::{path_within_root_existing, path_within_root_planned, validate_safe_id};

#[derive(Debug, Clone)]
pub struct OutputEvent {
    pub wave_id: String,
    pub run_id: String,
    pub agent_id: String,
    pub text: String,
}

/// File writer cache — one writer per run_id. Retained run-output write
/// machinery: the in-process broadcast bus that fed the old `/ws` output stream
/// is gone (waves write their own logs now), so nothing feeds this on the lfd
/// side; it stays as the append primitive for run-output logging.
#[allow(dead_code)]
struct Writers {
    dir: PathBuf,
    files: HashMap<String, File>,
}

#[allow(dead_code)]
impl Writers {
    fn new(dir: PathBuf) -> Self {
        fs::create_dir_all(&dir).ok();
        Self {
            dir,
            files: HashMap::new(),
        }
    }

    fn append(&mut self, run_id: &str, line: &str) {
        let Some(relative) = relative_log_path(run_id) else {
            warn!(run_id = %run_id, "rejecting unsafe run_id for output log");
            return;
        };

        let file = if let Some(file) = self.files.get_mut(run_id) {
            file
        } else {
            let path = match path_within_root_planned(&self.dir, &relative) {
                Ok(path) => path,
                Err(err) => {
                    warn!(run_id = %run_id, error = %err, "failed to resolve output log path");
                    return;
                }
            };
            let file = match OpenOptions::new().create(true).append(true).open(path) {
                Ok(file) => file,
                Err(err) => {
                    warn!(run_id = %run_id, error = %err, "failed to open output log file");
                    return;
                }
            };
            self.files.entry(run_id.to_string()).or_insert(file)
        };
        let _ = writeln!(file, "{line}");
    }

    fn close(&mut self, run_id: &str) {
        self.files.remove(run_id);
    }
}

/// Running token totals for one run, summed from `StreamEvent::Usage` events.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct UsageTotals {
    pub input_tokens: u64,
    pub output_tokens: u64,
    pub cache_read_tokens: u64,
}

impl UsageTotals {
    pub fn is_empty(&self) -> bool {
        self.input_tokens == 0 && self.output_tokens == 0 && self.cache_read_tokens == 0
    }
}

/// Replay a wave run's persisted output from its log file. Returns the lines
/// and the byte offset at end of read. Pure file read against the durable
/// per-run log — there is no live broadcast to follow anymore (the wave writes
/// its own logs; a client reads them from disk, no center).
pub fn read_output_log(output_dir: &Path, run_id: &str) -> Option<(Vec<String>, u64)> {
    let relative = relative_log_path(run_id)?;
    let path = path_within_root_existing(output_dir, &relative).ok()?;
    let file = File::open(&path).ok()?;
    let metadata = file.metadata().ok()?;
    let size = metadata.len();
    let reader = BufReader::new(file);
    let lines: Vec<String> = reader.lines().map_while(Result::ok).collect();
    Some((lines, size))
}

/// Delete output log files older than `max_age` based on filesystem mtime.
pub fn prune_output_logs(dir: &Path, max_age: Duration) {
    let entries = match fs::read_dir(dir) {
        Ok(entries) => entries,
        Err(err) => {
            tracing::debug!(dir = %dir.display(), error = %err, "skipping output log pruning");
            return;
        }
    };
    let now = SystemTime::now();
    let mut pruned = 0u32;
    for entry in entries.flatten() {
        let path = entry.path();
        if path.extension().and_then(|e| e.to_str()) != Some("log") {
            continue;
        }
        let mtime = match entry.metadata().and_then(|m| m.modified()) {
            Ok(t) => t,
            Err(_) => continue,
        };
        if now.duration_since(mtime).unwrap_or(Duration::ZERO) > max_age
            && fs::remove_file(&path).is_ok()
        {
            pruned += 1;
        }
    }
    if pruned > 0 {
        tracing::info!(count = pruned, dir = %dir.display(), "pruned old output logs");
    }
}

fn relative_log_path(run_id: &str) -> Option<PathBuf> {
    validate_safe_id(run_id).ok()?;
    Some(PathBuf::from(format!("{run_id}.log")))
}

#[cfg(test)]
mod tests {
    use super::read_output_log;
    use std::fs;
    use tempfile::tempdir;

    #[test]
    fn read_output_log_returns_none_for_unsafe_run_id() {
        let tmp = tempdir().expect("tempdir");
        assert!(read_output_log(tmp.path(), "../escape").is_none());
    }

    #[test]
    fn read_output_log_replays_a_run_log_file() {
        let tmp = tempdir().expect("tempdir");
        fs::write(tmp.path().join("run-1.log"), "first\nsecond\n").expect("write log");

        let (lines, size) = read_output_log(tmp.path(), "run-1").expect("log replays");
        assert_eq!(lines, vec!["first".to_string(), "second".to_string()]);
        assert!(size > 0);
    }

    #[test]
    fn read_output_log_returns_none_when_no_file() {
        let tmp = tempdir().expect("tempdir");
        assert!(read_output_log(tmp.path(), "run-missing").is_none());
    }
}
