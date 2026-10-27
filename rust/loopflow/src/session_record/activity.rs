//! Read-only CPU and provider-signal observations. These never authorize
//! process control.
//!
//! The launching process samples its provider body's CPU every
//! `SAMPLE_INTERVAL` into `session_activity`; a status read judges Running or
//! Stalled from that row alone.

use std::collections::BTreeSet;
use std::time::Duration;

use time::OffsetDateTime;

use crate::session::SessionLiveness;
use crate::store::sqlite::SqliteStore;

pub(crate) const SAMPLE_INTERVAL: Duration = Duration::from_secs(15);
const MAX_SAMPLE_GAP: i64 = 45;
const STALL_WINDOW: i64 = 300;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct CpuProcess {
    pid: u32,
    parent: u32,
    started_at: String,
    cpu_millis: u64,
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

    /// Since when the body and its tools used no CPU; `None` when this sample
    /// could not read them or the PID now names another process.
    fn observe(
        &mut self,
        now: OffsetDateTime,
        body_pid: Option<u32>,
        processes: Option<Vec<CpuProcess>>,
    ) -> Option<OffsetDateTime> {
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
        self.previous = processes.map(|processes| (now, processes));
        self.quiet_since
    }
}

impl super::CaptureHandle {
    pub(crate) async fn observe_activity(&self, body_pid: Option<u32>) {
        let processes = sample(body_pid).await;
        self.with_capture(|capture| {
            let now = OffsetDateTime::now_utc();
            let quiet_since = capture.activity.observe(now, body_pid, processes);
            let Some((session, attachment)) = &capture.attachment else {
                return Ok(());
            };
            super::row_store(&capture.dir)
                .and_then(|store| {
                    store.record_session_cpu(
                        session,
                        attachment,
                        now.unix_timestamp(),
                        quiet_since.map(OffsetDateTime::unix_timestamp),
                    )
                })
                .map_err(std::io::Error::other)
        });
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Activity {
    Running,
    Stalled,
    Unknown,
}

/// Whether a step's Session is progressing, judged from its stored liveness;
/// `step_started_at` covers a Session whose provider has not spoken yet.
pub(crate) fn read(store: &SqliteStore, session: &str, step_started_at: i64) -> Activity {
    let now = OffsetDateTime::now_utc().unix_timestamp();
    match store.session_liveness(session) {
        Ok(liveness) => project(liveness, step_started_at, now),
        Err(error) => {
            tracing::warn!(%error, session, "Session liveness unreadable");
            Activity::Unknown
        }
    }
}

fn project(liveness: Option<SessionLiveness>, step_started_at: i64, now: i64) -> Activity {
    let recent = |at: i64| (0..STALL_WINDOW).contains(&(now - at));
    let Some(liveness) = liveness else {
        return if recent(step_started_at) {
            Activity::Running
        } else {
            Activity::Unknown
        };
    };
    let fresh_sample = liveness
        .cpu_sampled_at
        .is_some_and(|at| (0..=MAX_SAMPLE_GAP).contains(&(now - at)));
    let cpu_quiet_since = liveness.cpu_quiet_since.filter(|_| fresh_sample);
    if recent(liveness.observed_at) || cpu_quiet_since.is_some_and(recent) {
        Activity::Running
    } else if cpu_quiet_since.is_some() {
        Activity::Stalled
    } else {
        Activity::Unknown
    }
}

pub(crate) async fn sample(body_pid: Option<u32>) -> Option<Vec<CpuProcess>> {
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
    use crate::session::SessionLiveness;
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

    fn at(seconds: i64) -> OffsetDateTime {
        OffsetDateTime::UNIX_EPOCH + time::Duration::seconds(seconds)
    }

    #[test]
    fn unchanged_cpu_is_quiet_until_a_tool_or_event_moves() {
        let mut observer = Observer::default();
        let mut cpu = processes();
        for seconds in (0..=300).step_by(15) {
            assert_eq!(
                observer.observe(at(seconds), Some(10), Some(cpu.clone())),
                Some(at(0))
            );
        }
        cpu[1].cpu_millis += 1;
        assert_eq!(
            observer.observe(at(315), Some(10), Some(cpu.clone())),
            Some(at(315))
        );
        observer.note_event(at(320));
        assert_eq!(
            observer.observe(at(330), Some(10), Some(cpu)),
            Some(at(320))
        );
    }

    #[test]
    fn missing_samples_and_reused_pids_have_no_quiet_time() {
        let mut observer = Observer::default();
        let cpu = processes();
        observer.observe(at(0), Some(10), Some(cpu.clone()));
        assert_eq!(observer.observe(at(15), Some(10), None), None);
        assert_eq!(
            observer.observe(at(30), Some(10), Some(cpu.clone())),
            Some(at(30))
        );
        let mut reused = processes();
        reused[0].started_at = "replacement body".into();
        for seconds in (45..=400).step_by(15) {
            assert_eq!(
                observer.observe(at(seconds), Some(10), Some(reused.clone())),
                None,
                "a reused PID is not this Session's provider"
            );
        }
    }

    #[test]
    fn a_step_stalls_only_when_a_fresh_sample_and_its_provider_are_both_quiet() {
        let now = 1_000;
        let reading = |observed_at, sampled: Option<i64>, quiet: Option<i64>| {
            Some(SessionLiveness {
                observed_at,
                cpu_sampled_at: sampled,
                cpu_quiet_since: quiet,
            })
        };
        // A silent long tool keeps its CPU moving.
        assert_eq!(
            project(reading(0, Some(990), Some(980)), 0, now),
            Activity::Running
        );
        // The provider spoke recently, whatever the CPU says.
        assert_eq!(
            project(reading(900, Some(990), Some(0)), 0, now),
            Activity::Running
        );
        assert_eq!(project(reading(900, None, None), 0, now), Activity::Running);
        // Five quiet minutes on a fresh sample.
        assert_eq!(
            project(reading(600, Some(990), Some(700)), 0, now),
            Activity::Stalled
        );
        // A stale or unreadable sample proves nothing.
        assert_eq!(
            project(reading(600, Some(950), Some(0)), 0, now),
            Activity::Unknown
        );
        assert_eq!(
            project(reading(600, Some(990), None), 0, now),
            Activity::Unknown
        );
        assert_eq!(project(reading(600, None, None), 0, now), Activity::Unknown);
        // A Session that has not spoken yet runs only while its step is young.
        assert_eq!(project(None, 800, now), Activity::Running);
        assert_eq!(project(None, 600, now), Activity::Unknown);
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
