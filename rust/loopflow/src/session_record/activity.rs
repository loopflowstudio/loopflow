//! Read-only CPU/event observations. These never authorize process control.

use std::collections::BTreeSet;
use std::fs::File;
use std::io::{Read, Seek, SeekFrom};
use std::path::Path;
use std::time::Duration;

use serde::{Deserialize, Serialize};
use time::OffsetDateTime;

use super::{record_dir, CaptureEvent, EventEnvelope, SCHEMA_VERSION};

pub(crate) const SAMPLE_INTERVAL: Duration = Duration::from_secs(15);
const MAX_SAMPLE_GAP: i64 = 45;
const STALL_WINDOW: i64 = 300;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
struct CpuProcess {
    pid: u32,
    parent: u32,
    started_at: String,
    cpu_millis: u64,
}

#[derive(Debug, Serialize, Deserialize)]
pub(super) struct Observation {
    body_pid: Option<u32>,
    processes: Option<Vec<CpuProcess>>,
    #[serde(with = "time::serde::rfc3339::option")]
    quiet_since: Option<OffsetDateTime>,
}

#[derive(Debug, Default)]
pub(super) struct Observer {
    body: Option<(u32, String)>,
    previous: Option<(OffsetDateTime, Vec<CpuProcess>)>,
    quiet_since: Option<OffsetDateTime>,
}

impl Observer {
    pub(super) fn note_event(&mut self, now: OffsetDateTime) {
        self.quiet_since = Some(now);
    }

    fn observe(
        &mut self,
        now: OffsetDateTime,
        body_pid: Option<u32>,
        processes: Option<Vec<CpuProcess>>,
    ) -> Observation {
        let processes = processes.filter(|processes| {
            let Some(root) = processes
                .iter()
                .find(|process| Some(process.pid) == body_pid)
            else {
                return false;
            };
            let body = self
                .body
                .get_or_insert_with(|| (root.pid, root.started_at.clone()));
            body.0 == root.pid && body.1 == root.started_at
        });
        self.quiet_since = match (&self.previous, &processes) {
            (Some((at, previous)), Some(current))
                if (0..=MAX_SAMPLE_GAP).contains(&(now - *at).whole_seconds())
                    && previous == current =>
            {
                self.quiet_since
            }
            (_, Some(_)) => Some(now),
            (_, None) => None,
        };
        self.previous = processes.clone().map(|processes| (now, processes));
        Observation {
            body_pid,
            processes,
            quiet_since: self.quiet_since,
        }
    }
}

impl super::CaptureHandle {
    pub(crate) async fn observe_activity(&self, body_pid: Option<u32>) {
        let processes = sample(body_pid).await;
        self.with_capture(|capture| {
            let observation =
                capture
                    .activity
                    .observe(OffsetDateTime::now_utc(), body_pid, processes);
            capture.append_event(CaptureEvent::Activity { observation })
        });
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Activity {
    Running,
    Stalled,
    Unknown,
}

pub(crate) async fn read(home: &Path, run: &str) -> Activity {
    let Some(dir) = record_dir(home, run) else {
        return Activity::Unknown;
    };
    let Ok(envelope) = latest_event(&dir.join("events.jsonl")) else {
        return Activity::Unknown;
    };
    let now = OffsetDateTime::now_utc();
    match envelope.event {
        CaptureEvent::Activity { observation } => {
            let current = sample(observation.body_pid).await;
            project(&observation, envelope.observed_at, now, current.as_deref())
        }
        _ if (0..STALL_WINDOW).contains(&(now - envelope.observed_at).whole_seconds()) => {
            Activity::Running
        }
        _ => Activity::Unknown,
    }
}

fn project(
    observation: &Observation,
    at: OffsetDateTime,
    now: OffsetDateTime,
    current: Option<&[CpuProcess]>,
) -> Activity {
    let (Some(previous), Some(current), Some(quiet_since), Some(pid)) = (
        &observation.processes,
        current,
        observation.quiet_since,
        observation.body_pid,
    ) else {
        return Activity::Unknown;
    };
    if !(0..=MAX_SAMPLE_GAP).contains(&(now - at).whole_seconds()) {
        return Activity::Unknown;
    }
    let old_root = previous.iter().find(|process| process.pid == pid);
    let root = current.iter().find(|process| process.pid == pid);
    if !matches!((old_root, root), (Some(old), Some(new)) if old.started_at == new.started_at) {
        return Activity::Unknown;
    }
    if previous != current || (now - quiet_since).whole_seconds() < STALL_WINDOW {
        Activity::Running
    } else {
        Activity::Stalled
    }
}

// Bound status reads independently of a Run's transcript size. Ignore only the
// incomplete first/last records; a malformed complete record stays unknown.
fn latest_event(path: &Path) -> std::io::Result<EventEnvelope> {
    let mut file = File::open(path)?;
    let offset = file.metadata()?.len().saturating_sub(64 * 1024);
    file.seek(SeekFrom::Start(offset))?;
    let mut bytes = Vec::new();
    file.read_to_end(&mut bytes)?;
    let mut lines = bytes.split_inclusive(|byte| *byte == b'\n');
    if offset > 0 {
        lines.next();
    }
    let line = lines
        .rfind(|line| line.last() == Some(&b'\n'))
        .ok_or_else(|| std::io::Error::other("no complete activity observation"))?;
    let event: EventEnvelope = serde_json::from_slice(line)?;
    if event.schema_version != SCHEMA_VERSION {
        return Err(std::io::Error::other("unknown capture event schema"));
    }
    Ok(event)
}

async fn sample(body_pid: Option<u32>) -> Option<Vec<CpuProcess>> {
    let pid = body_pid?;
    // Include reaped children's CPU: a short-lived tool must not disappear
    // between samples and make a busy body look idle. No command text is read.
    let output = tokio::time::timeout(
        Duration::from_secs(2),
        tokio::process::Command::new("ps")
            .args(["-S", "-axo", "pid=,ppid=,stat=,lstart=,time="])
            .env("LC_ALL", "C")
            .kill_on_drop(true)
            .output(),
    )
    .await
    .ok()?
    .ok()?;
    if !output.status.success() {
        return None;
    }
    parse_processes(std::str::from_utf8(&output.stdout).ok()?, pid)
}

fn parse_processes(text: &str, pid: u32) -> Option<Vec<CpuProcess>> {
    let mut processes = Vec::new();
    for line in text.lines().filter(|line| !line.trim().is_empty()) {
        let fields = line.split_whitespace().collect::<Vec<_>>();
        if fields.len() != 9 {
            return None;
        }
        if fields[2].starts_with('Z') {
            continue;
        }
        processes.push(CpuProcess {
            pid: fields[0].parse().ok()?,
            parent: fields[1].parse().ok()?,
            started_at: fields[3..8].join(" "),
            cpu_millis: cpu_millis(fields[8])?,
        });
    }
    if !processes.iter().any(|process| process.pid == pid) {
        return None;
    }
    let mut descendants = BTreeSet::from([pid]);
    loop {
        let before = descendants.len();
        for process in &processes {
            if descendants.contains(&process.parent) {
                descendants.insert(process.pid);
            }
        }
        if descendants.len() == before {
            break;
        }
    }
    processes.retain(|process| descendants.contains(&process.pid));
    processes.sort_by_key(|process| process.pid);
    Some(processes)
}

fn cpu_millis(text: &str) -> Option<u64> {
    let (days, clock) = match text.split_once('-') {
        Some((days, clock)) => (days.parse::<u64>().ok()?, clock),
        None => (0, text),
    };
    let (whole, fraction) = clock.split_once('.').unwrap_or((clock, ""));
    let mut seconds = 0_u64;
    for part in whole.split(':') {
        seconds = seconds.checked_mul(60)?.checked_add(part.parse().ok()?)?;
    }
    let millis = if fraction.is_empty() {
        0
    } else {
        if fraction.len() > 3 {
            return None;
        }
        fraction.parse::<u64>().ok()? * 10_u64.pow(3 - fraction.len() as u32)
    };
    days.checked_mul(86_400)?
        .checked_add(seconds)?
        .checked_mul(1000)?
        .checked_add(millis)
}

#[cfg(test)]
mod tests {
    use super::{cpu_millis, parse_processes, project, sample, Activity, CpuProcess, Observer};
    use time::OffsetDateTime;

    fn processes() -> Vec<CpuProcess> {
        vec![
            CpuProcess {
                pid: 10,
                parent: 1,
                started_at: "body birth".into(),
                cpu_millis: 20,
            },
            CpuProcess {
                pid: 11,
                parent: 10,
                started_at: "tool birth".into(),
                cpu_millis: 5,
            },
        ]
    }

    #[test]
    fn inactivity_stalls_but_silent_tool_cpu_and_events_restore_running() {
        let mut observer = Observer::default();
        let start = OffsetDateTime::UNIX_EPOCH;
        let at = start + time::Duration::seconds(300);
        let mut cpu = processes();
        for seconds in (0..300).step_by(15) {
            observer.observe(
                start + time::Duration::seconds(seconds),
                Some(10),
                Some(cpu.clone()),
            );
        }
        let quiet = observer.observe(at, Some(10), Some(cpu.clone()));
        assert_eq!(project(&quiet, at, at, Some(&cpu)), Activity::Stalled);
        observer.note_event(at);
        let event = observer.observe(at, Some(10), Some(cpu.clone()));
        assert_eq!(project(&event, at, at, Some(&cpu)), Activity::Running);
        cpu[1].cpu_millis += 1;
        // Even before the next stored sample, a silent tool's CPU wins.
        assert_eq!(project(&quiet, at, at, Some(&cpu)), Activity::Running);
        let active = observer.observe(at, Some(10), Some(cpu.clone()));
        assert_eq!(project(&active, at, at, Some(&cpu)), Activity::Running);
        observer.note_event(at);
        let fresh = observer.observe(at, Some(10), Some(cpu.clone()));
        assert_eq!(project(&fresh, at, at, Some(&cpu)), Activity::Running);
    }

    #[test]
    fn missing_samples_stale_observers_and_reused_pids_never_prove_stalled() {
        let at = OffsetDateTime::UNIX_EPOCH;
        let cpu = processes();
        let quiet = super::Observation {
            body_pid: Some(10),
            processes: Some(cpu.clone()),
            quiet_since: Some(at - time::Duration::seconds(300)),
        };
        assert_eq!(project(&quiet, at, at, None), Activity::Unknown);
        assert_eq!(
            project(&quiet, at, at + time::Duration::seconds(46), Some(&cpu)),
            Activity::Unknown
        );
        let mut reused = cpu.clone();
        reused[0].started_at = "another body".into();
        assert_eq!(project(&quiet, at, at, Some(&reused)), Activity::Unknown);
        let mut observer = Observer::default();
        observer.observe(at, Some(10), Some(cpu.clone()));
        observer.observe(at + time::Duration::seconds(15), Some(10), None);
        let now = at + time::Duration::seconds(300);
        let restored = observer.observe(now, Some(10), Some(cpu.clone()));
        assert_eq!(project(&restored, now, now, Some(&cpu)), Activity::Running);
    }

    #[test]
    fn reused_body_never_becomes_the_runs_observed_process() {
        let mut observer = Observer::default();
        let start = OffsetDateTime::UNIX_EPOCH;
        let cpu = processes();
        observer.observe(start, Some(10), Some(cpu));
        let mut reused = processes();
        reused[0].started_at = "replacement body".into();
        for seconds in (15..=330).step_by(15) {
            let now = start + time::Duration::seconds(seconds);
            let observation = observer.observe(now, Some(10), Some(reused.clone()));
            assert_eq!(
                project(&observation, now, now, Some(&reused)),
                Activity::Unknown,
                "a reused PID is not this Run's body, even after repeated samples"
            );
        }
    }

    #[test]
    fn cpu_sampling_keeps_exact_descendants_and_unavailable_values() {
        let text = "10 1 S Sun Sep 27 01:00:00 2026 0:00.02\n11 10 S Sun Sep 27 01:00:01 2026 0:00.05\n12 2 S Sun Sep 27 01:00:00 2026 1:00.00\n";
        let cpu = parse_processes(text, 10).unwrap();
        assert_eq!(cpu.iter().map(|p| p.pid).collect::<Vec<_>>(), [10, 11]);
        assert_eq!(cpu[1].cpu_millis, 50);
        assert!(parse_processes(&text.replace("0:00.05", "?"), 10).is_none());
        assert!(parse_processes(text, 99).is_none());
        assert_eq!(cpu_millis("1-02:03:04"), Some(93_784_000));
    }

    #[tokio::test]
    async fn real_process_cpu_sampling_reads_this_body() {
        let pid = std::process::id();
        let cpu = sample(Some(pid)).await.unwrap();
        assert!(cpu.iter().any(|process| process.pid == pid));
        assert!(!cpu
            .iter()
            .find(|process| process.pid == pid)
            .unwrap()
            .started_at
            .is_empty());
    }
}
