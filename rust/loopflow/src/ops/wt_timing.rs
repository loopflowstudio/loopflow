//! Timing of real `lf wt list` invocations on this machine.
//!
//! Each invocation appends one line to `<Home>/perf/wt-list.jsonl`. The file is
//! written beside the store, never through it: under SQLite contention the
//! Process row is the record that goes missing, and those are the slowest runs.
//! A sample holds durations, counts, the repository root and the `lf` version;
//! no arguments, branch names, output or credentials.
use std::collections::BTreeMap;
use std::fs::{self, OpenOptions};
use std::io::{Read, Seek, SeekFrom, Write};
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::time::Duration;

use fs2::FileExt;
use serde::{Deserialize, Serialize};

use crate::engine::worktrees::{Listing, RemoteOutcome};
use crate::journal::ReceiptCost;

/// Samples kept after a trim. The file is trimmed when it holds twice as many.
pub const RETAINED_SAMPLES: usize = 500;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Sample {
    /// Unix seconds when the invocation ended.
    pub at: i64,
    pub version: String,
    pub repo: String,
    pub json: bool,
    pub sync: bool,
    pub outcome: Outcome,
    /// Process start to process end, including both Process receipts.
    pub total_ms: u64,
    /// Process start until the listing began: launch, routing, Process admission
    /// and the start receipt.
    pub startup_ms: u64,
    /// Absent when the command ended before the listing was read.
    pub listing: Option<ListingSample>,
    /// Absent when the process was interrupted or had no Process.
    pub receipts: Option<ReceiptSample>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
#[non_exhaustive]
pub enum Outcome {
    Succeeded,
    Failed,
    Interrupted,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct ListingSample {
    pub worktrees: usize,
    /// Local Git and the remote run side by side; these overlap.
    pub local_git_ms: u64,
    pub remote_ms: u64,
    pub remote: RemoteOutcome,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct ReceiptSample {
    /// Time spent writing the start and finish Process receipts to SQLite.
    pub wait_ms: u64,
    pub unrecorded: u32,
}

#[derive(Debug)]
struct Pending {
    repo: String,
    json: bool,
    sync: bool,
    startup: Duration,
    listing: Option<ListingSample>,
}

static PENDING: Mutex<Option<Pending>> = Mutex::new(None);

fn millis(duration: Duration) -> u64 {
    u64::try_from(duration.as_millis()).unwrap_or(u64::MAX)
}

/// Start timing this process's listing. A library caller has no process clock
/// and records nothing.
pub fn begin(repo: &Path, json: bool, sync: bool) {
    let Some(startup) = crate::journal::process_elapsed() else {
        return;
    };
    *PENDING.lock().expect("listing timing mutex poisoned") = Some(Pending {
        repo: repo.display().to_string(),
        json,
        sync,
        startup,
        listing: None,
    });
    crate::engine::agent::register_interrupt_cleanup(|| write_pending(Outcome::Interrupted, None));
}

pub fn listed(listing: &Listing) {
    if let Some(pending) = PENDING
        .lock()
        .expect("listing timing mutex poisoned")
        .as_mut()
    {
        pending.listing = Some(ListingSample {
            worktrees: listing.worktrees.len(),
            local_git_ms: millis(listing.local_git),
            remote_ms: millis(listing.remote),
            remote: listing.remote_outcome,
        });
    }
}

/// Write the sample once the process's finish receipt has been attempted.
pub(crate) fn finish(succeeded: bool, receipts: Option<ReceiptCost>) {
    let outcome = if succeeded {
        Outcome::Succeeded
    } else {
        Outcome::Failed
    };
    write_pending(outcome, receipts);
}

fn write_pending(outcome: Outcome, receipts: Option<ReceiptCost>) {
    let Some(pending) = PENDING
        .lock()
        .expect("listing timing mutex poisoned")
        .take()
    else {
        return;
    };
    let sample = Sample {
        at: time::OffsetDateTime::now_utc().unix_timestamp(),
        version: crate::build_info::BUILD_VERSION.to_string(),
        repo: pending.repo,
        json: pending.json,
        sync: pending.sync,
        outcome,
        total_ms: crate::journal::process_elapsed().map_or(0, millis),
        startup_ms: millis(pending.startup),
        listing: pending.listing,
        receipts: receipts.map(|cost| ReceiptSample {
            wait_ms: millis(cost.waited),
            unrecorded: cost.unrecorded,
        }),
    };
    if let Err(error) = append(&crate::store::lf_home_dir(), &sample) {
        tracing::debug!(%error, "listing timing sample not written");
    }
}

pub fn samples_path(home: &Path) -> PathBuf {
    home.join("perf").join("wt-list.jsonl")
}

/// Append one sample, trimming the file to its newest `RETAINED_SAMPLES` once
/// it holds twice that. Timing never creates a Home.
fn append(home: &Path, sample: &Sample) -> std::io::Result<()> {
    if !home.is_dir() {
        return Ok(());
    }
    let path = samples_path(home);
    fs::create_dir_all(path.parent().expect("samples path has a parent"))?;
    let mut file = OpenOptions::new()
        .create(true)
        .truncate(false)
        .read(true)
        .write(true)
        .open(path)?;
    file.lock_exclusive()?;
    let mut existing = String::new();
    file.read_to_string(&mut existing)?;
    let mut line = serde_json::to_string(sample).map_err(std::io::Error::other)?;
    line.push('\n');
    let lines: Vec<&str> = existing.lines().collect();
    if lines.len() + 1 >= RETAINED_SAMPLES * 2 {
        let mut kept = lines[lines.len() + 1 - RETAINED_SAMPLES..].join("\n");
        kept.push('\n');
        kept.push_str(&line);
        file.set_len(0)?;
        file.seek(SeekFrom::Start(0))?;
        file.write_all(kept.as_bytes())?;
    } else {
        // A crashed writer can leave a partial last line; keep it apart.
        if !existing.is_empty() && !existing.ends_with('\n') {
            line.insert(0, '\n');
        }
        file.seek(SeekFrom::End(0))?;
        file.write_all(line.as_bytes())?;
    }
    Ok(())
}

// ── report ──────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Report {
    pub path: String,
    pub samples: usize,
    /// Lines this `lf` could not read as a sample.
    pub unreadable: usize,
    pub retained_limit: usize,
    pub groups: Vec<Group>,
}

/// Invocations of one `lf` version in one repository and mode.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Group {
    pub repo: String,
    pub version: String,
    pub json: bool,
    pub sync: bool,
    pub samples: usize,
    pub total_ms: Spread,
    pub startup_ms: Spread,
    /// Over the samples that reached the listing; `None` when none did.
    pub local_git_ms: Option<Spread>,
    pub remote_ms: Option<Spread>,
    /// Over the samples whose receipts were attempted.
    pub receipt_ms: Option<Spread>,
    pub failed: usize,
    pub interrupted: usize,
    pub remote_timed_out: usize,
    pub remote_unavailable: usize,
    pub receipts_unrecorded: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct Spread {
    pub median: u64,
    pub p95: u64,
    pub max: u64,
}

/// Nearest-rank percentiles, so every reported value is a real sample.
fn spread(mut values: Vec<u64>) -> Option<Spread> {
    values.sort_unstable();
    let rank = |percent: usize| values[(values.len() * percent).div_ceil(100).max(1) - 1];
    (!values.is_empty()).then(|| Spread {
        median: rank(50),
        p95: rank(95),
        max: values[values.len() - 1],
    })
}

pub fn report(home: &Path) -> std::io::Result<Report> {
    let path = samples_path(home);
    let contents = match fs::read_to_string(&path) {
        Ok(contents) => contents,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => String::new(),
        Err(error) => return Err(error),
    };
    let lines: Vec<&str> = contents.lines().filter(|line| !line.is_empty()).collect();
    let samples: Vec<Sample> = lines
        .iter()
        .filter_map(|line| serde_json::from_str(line).ok())
        .collect();
    let mut grouped: BTreeMap<(&str, &str, bool, bool), Vec<&Sample>> = BTreeMap::new();
    for sample in &samples {
        grouped
            .entry((&sample.repo, &sample.version, sample.json, sample.sync))
            .or_default()
            .push(sample);
    }
    let groups = grouped
        .into_iter()
        .map(|((repo, version, json, sync), samples)| {
            let listings = || samples.iter().filter_map(|sample| sample.listing);
            let receipts = || samples.iter().filter_map(|sample| sample.receipts);
            let count = |outcome| {
                samples
                    .iter()
                    .filter(|sample| sample.outcome == outcome)
                    .count()
            };
            let remote = |outcome| listings().filter(|l| l.remote == outcome).count();
            let total = |pick: fn(&Sample) -> u64| {
                spread(samples.iter().map(|sample| pick(sample)).collect())
                    .expect("a group has at least one sample")
            };
            Group {
                repo: repo.to_string(),
                version: version.to_string(),
                json,
                sync,
                samples: samples.len(),
                total_ms: total(|sample| sample.total_ms),
                startup_ms: total(|sample| sample.startup_ms),
                local_git_ms: spread(listings().map(|l| l.local_git_ms).collect()),
                remote_ms: spread(listings().map(|l| l.remote_ms).collect()),
                receipt_ms: spread(receipts().map(|r| r.wait_ms).collect()),
                failed: count(Outcome::Failed),
                interrupted: count(Outcome::Interrupted),
                remote_timed_out: remote(RemoteOutcome::TimedOut),
                remote_unavailable: remote(RemoteOutcome::Unavailable),
                receipts_unrecorded: receipts().filter(|r| r.unrecorded > 0).count(),
            }
        })
        .collect();
    Ok(Report {
        path: path.display().to_string(),
        samples: samples.len(),
        unreadable: lines.len() - samples.len(),
        retained_limit: RETAINED_SAMPLES * 2,
        groups,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample(total_ms: u64) -> Sample {
        Sample {
            at: 1_791_000_000 + i64::try_from(total_ms).unwrap(),
            version: "1.2.3".to_string(),
            repo: "/src/loopflow".to_string(),
            json: false,
            sync: false,
            outcome: Outcome::Succeeded,
            total_ms,
            startup_ms: 100,
            listing: Some(ListingSample {
                worktrees: 51,
                local_git_ms: total_ms / 2,
                remote_ms: total_ms / 4,
                remote: RemoteOutcome::Answered,
            }),
            receipts: Some(ReceiptSample {
                wait_ms: 20,
                unrecorded: 0,
            }),
        }
    }

    #[test]
    fn report_summarizes_recorded_invocations() {
        let home = tempfile::tempdir().unwrap();
        for total in 1..=20 {
            append(home.path(), &sample(total * 100)).unwrap();
        }
        let slow = Sample {
            outcome: Outcome::Interrupted,
            listing: None,
            receipts: None,
            ..sample(44_000)
        };
        append(home.path(), &slow).unwrap();
        let timed_out = Sample {
            json: true,
            listing: Some(ListingSample {
                worktrees: 51,
                local_git_ms: 400,
                remote_ms: 10_000,
                remote: RemoteOutcome::TimedOut,
            }),
            receipts: Some(ReceiptSample {
                wait_ms: 15_000,
                unrecorded: 2,
            }),
            ..sample(25_500)
        };
        append(home.path(), &timed_out).unwrap();

        let report = report(home.path()).unwrap();

        assert_eq!((report.samples, report.unreadable), (22, 0));
        let [text, json] = report.groups.as_slice() else {
            panic!("expected text and JSON groups: {report:?}");
        };
        assert_eq!((text.samples, text.interrupted, text.failed), (21, 1, 0));
        assert_eq!(text.version, "1.2.3");
        assert_eq!(
            text.total_ms,
            Spread {
                median: 1_100,
                p95: 2_000,
                max: 44_000
            }
        );
        // The interrupted run never reached the listing.
        assert_eq!(text.local_git_ms.unwrap().max, 1_000);
        assert!(json.json);
        assert_eq!((json.remote_timed_out, json.receipts_unrecorded), (1, 1));
        assert_eq!(json.receipt_ms.unwrap().median, 15_000);
    }

    #[test]
    fn retention_keeps_the_newest_samples() {
        let home = tempfile::tempdir().unwrap();
        for total in 0..(RETAINED_SAMPLES as u64 * 2 + 10) {
            append(home.path(), &sample(total)).unwrap();
        }

        let contents = fs::read_to_string(samples_path(home.path())).unwrap();
        let kept: Vec<Sample> = contents
            .lines()
            .map(|line| serde_json::from_str(line).unwrap())
            .collect();

        assert!(kept.len() <= RETAINED_SAMPLES * 2);
        assert!(kept.len() >= RETAINED_SAMPLES);
        assert_eq!(
            kept.last().unwrap().total_ms,
            RETAINED_SAMPLES as u64 * 2 + 9
        );
        assert!(kept
            .windows(2)
            .all(|pair| pair[0].total_ms < pair[1].total_ms));
    }

    #[test]
    fn a_missing_home_is_not_created_and_reports_nothing() {
        let root = tempfile::tempdir().unwrap();
        let home = root.path().join("absent");

        append(&home, &sample(1)).unwrap();

        assert!(!home.exists());
        assert_eq!(report(&home).unwrap().samples, 0);
    }

    #[test]
    fn an_unreadable_line_is_counted_not_fatal() {
        let home = tempfile::tempdir().unwrap();
        append(home.path(), &sample(300)).unwrap();
        let path = samples_path(home.path());
        let mut contents = fs::read_to_string(&path).unwrap();
        contents.push_str("{\"at\":1");
        fs::write(&path, contents).unwrap();
        append(home.path(), &sample(400)).unwrap();

        let report = report(home.path()).unwrap();

        assert_eq!((report.samples, report.unreadable), (2, 1));
    }
}
