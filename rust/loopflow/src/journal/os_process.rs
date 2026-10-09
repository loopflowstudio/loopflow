//! One OS identity reader for process gates, activity and orphan settlement.
//! Invalid observations are errors, never evidence that a process disappeared.

use std::io;
use std::process::Command;

use super::ProcessIdentityEvidence;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct OsProcess {
    pub(crate) pid: u32,
    pub(crate) started_at: i64,
    pub(crate) kernel_state: String,
}

impl OsProcess {
    pub(crate) fn read(pid: u32) -> io::Result<Option<Self>> {
        let mut rows = query(
            &["-p", &pid.to_string()],
            time::OffsetDateTime::now_utc().unix_timestamp(),
        )?;
        match rows.as_slice() {
            [] => Ok(None),
            [row] if row.pid == pid => Ok(rows.pop()),
            _ => Err(io::Error::other("process query returned an unexpected PID")),
        }
    }

    pub(crate) fn sample(now: i64) -> io::Result<Vec<Self>> {
        query(&["-ax"], now)
    }

    pub(crate) fn evidence(&self, started_at: i64) -> ProcessIdentityEvidence {
        if self.kernel_state.starts_with('Z') || self.started_at.abs_diff(started_at) > 3 {
            ProcessIdentityEvidence::Dead
        } else {
            ProcessIdentityEvidence::Live
        }
    }

    pub(crate) fn matches_start(&self, pid: u32, started_at: i64) -> bool {
        self.pid == pid && self.evidence(started_at) == ProcessIdentityEvidence::Live
    }
}

fn query(selection: &[&str], now: i64) -> io::Result<Vec<OsProcess>> {
    let output = Command::new("ps")
        .args(selection)
        .args(["-o", "pid=,state=,etime="])
        .output()?;
    if !output.status.success() {
        if output.status.code() == Some(1) && output.stdout.is_empty() && output.stderr.is_empty() {
            return Ok(Vec::new());
        }
        return Err(io::Error::other(format!(
            "process query failed: {}",
            String::from_utf8_lossy(&output.stderr).trim()
        )));
    }
    String::from_utf8_lossy(&output.stdout)
        .lines()
        .filter(|line| !line.trim().is_empty())
        .map(|line| {
            parse(line, now)
                .ok_or_else(|| io::Error::other("process query returned invalid identity or age"))
        })
        .collect()
}

fn parse(line: &str, now: i64) -> Option<OsProcess> {
    let mut fields = line.split_whitespace();
    let pid = fields.next()?.parse().ok()?;
    let kernel_state = fields.next()?.to_owned();
    let elapsed = i64::try_from(elapsed_seconds(fields.next()?)?).ok()?;
    if fields.next().is_some() {
        return None;
    }
    Some(OsProcess {
        pid,
        started_at: now.checked_sub(elapsed)?,
        kernel_state,
    })
}

pub(crate) fn elapsed_seconds(value: &str) -> Option<u64> {
    let (days, clock) = match value.split_once('-') {
        Some((days, clock)) => (days.parse().ok()?, clock),
        None => (0_u64, value),
    };
    let parts = clock
        .split(':')
        .map(str::parse::<u64>)
        .collect::<Result<Vec<_>, _>>()
        .ok()?;
    let clock = match parts.as_slice() {
        [minutes, seconds] => minutes.checked_mul(60)?.checked_add(*seconds)?,
        [hours, minutes, seconds] => hours
            .checked_mul(3_600)?
            .checked_add(minutes.checked_mul(60)?)?
            .checked_add(*seconds)?,
        _ => return None,
    };
    days.checked_mul(86_400)?.checked_add(clock)
}

#[cfg(test)]
mod tests {
    use super::{parse, OsProcess};
    use crate::journal::ProcessIdentityEvidence;

    #[test]
    fn activity_and_control_share_identity_and_zombie_evidence() {
        for (line, start, evidence) in [
            ("10 S 01:00", 99_940, ProcessIdentityEvidence::Live),
            ("10 R 02:00:00", 92_800, ProcessIdentityEvidence::Live),
            ("10 Z 1-00:00:00", 13_600, ProcessIdentityEvidence::Dead),
            ("10 S 2-01:02:03", -76_523, ProcessIdentityEvidence::Live),
            ("10 S 01:00", 99_930, ProcessIdentityEvidence::Dead),
        ] {
            let process = parse(line, 100_000).unwrap();
            assert_eq!(process.evidence(start), evidence);
            assert_eq!(
                process.matches_start(10, start),
                evidence == ProcessIdentityEvidence::Live
            );
            assert!(!process.matches_start(11, start));
        }
        for line in [
            "10 S ?",
            "10 S 00:bad:01",
            "10 S 18446744073709551615-00:00",
            "bad S 00:00",
            "10 S 00:00 extra",
        ] {
            assert!(parse(line, 100_000).is_none(), "{line}");
        }
    }

    struct Child(std::process::Child);

    impl Drop for Child {
        fn drop(&mut self) {
            let _ = self.0.kill();
            let _ = self.0.wait();
        }
    }

    #[test]
    fn single_and_inventory_reads_agree_for_a_throwaway_process() {
        let mut owned = Child(
            std::process::Command::new("/bin/sleep")
                .env_clear()
                .arg("60")
                .spawn()
                .unwrap(),
        );
        let child = &mut owned.0;
        let observed = OsProcess::read(child.id()).unwrap().unwrap();
        let sampled = OsProcess::sample(time::OffsetDateTime::now_utc().unix_timestamp()).unwrap();
        let sample = sampled
            .iter()
            .find(|process| process.pid == child.id())
            .unwrap();
        let evidence = crate::journal::process_identity_evidence(child.id(), observed.started_at);
        child.kill().unwrap();
        child.wait().unwrap();
        assert!(sample.matches_start(observed.pid, observed.started_at));
        assert_eq!(evidence, ProcessIdentityEvidence::Live);
        assert_eq!(
            crate::journal::process_identity_evidence(child.id(), observed.started_at),
            ProcessIdentityEvidence::Dead
        );
    }

    #[test]
    fn zombie_is_dead_before_its_parent_reaps_it() {
        let mut owned = Child(
            std::process::Command::new("/bin/sh")
                .env_clear()
                .args(["-c", "exit 0"])
                .spawn()
                .unwrap(),
        );
        let child = &mut owned.0;
        // SAFETY: waitid inspects only this fixture's child and initializes the
        // supplied siginfo buffer. WNOWAIT leaves the zombie available to ps.
        let result = unsafe {
            let mut info = std::mem::zeroed::<libc::siginfo_t>();
            libc::waitid(
                libc::P_PID,
                child.id(),
                &mut info,
                libc::WEXITED | libc::WNOWAIT,
            )
        };
        assert_eq!(result, 0, "{}", std::io::Error::last_os_error());
        let observed = OsProcess::read(child.id()).unwrap().unwrap();
        assert!(observed.kernel_state.starts_with('Z'));
        assert_eq!(
            crate::journal::process_identity_evidence(child.id(), observed.started_at),
            ProcessIdentityEvidence::Dead
        );
        let sampled = OsProcess::sample(time::OffsetDateTime::now_utc().unix_timestamp()).unwrap();
        let sample = sampled
            .iter()
            .find(|process| process.pid == child.id())
            .unwrap();
        assert_eq!(
            sample.evidence(observed.started_at),
            ProcessIdentityEvidence::Dead
        );
        assert!(!sample.matches_start(child.id(), observed.started_at));
        child.wait().unwrap();
    }
}
