//! One OS identity reader for process gates, activity and orphan settlement.
//! Invalid observations are errors, never evidence that a process disappeared.

use std::io;
use std::path::PathBuf;
#[cfg(not(target_os = "macos"))]
use std::process::Command;

#[cfg(target_os = "macos")]
mod macos;

use super::ProcessIdentityEvidence;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct OsProcess {
    pub(crate) pid: u32,
    pub(crate) ppid: u32,
    pub(crate) pgid: u32,
    pub(crate) started_at: i64,
    pub(crate) kernel_state: String,
    pub(crate) command: String,
}

impl OsProcess {
    #[cfg(target_os = "macos")]
    pub(crate) fn read(pid: u32) -> io::Result<Option<Self>> {
        macos::read(pid)
    }

    #[cfg(not(target_os = "macos"))]
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

    /// The live processes among `pids`, without sampling every other one.
    #[cfg(target_os = "macos")]
    pub(crate) fn read_each(pids: &[u32], _now: i64) -> io::Result<Vec<Self>> {
        pids.iter()
            .filter_map(|pid| macos::read(*pid).transpose())
            .collect()
    }

    #[cfg(not(target_os = "macos"))]
    pub(crate) fn read_each(pids: &[u32], now: i64) -> io::Result<Vec<Self>> {
        if pids.is_empty() {
            return Ok(Vec::new());
        }
        let pids = pids.iter().map(u32::to_string).collect::<Vec<_>>();
        query(&["-p", &pids.join(",")], now)
    }

    #[cfg(not(target_os = "macos"))]
    pub(crate) fn sample(now: i64) -> io::Result<Vec<Self>> {
        query(&["-ax"], now)
    }

    #[cfg(target_os = "macos")]
    pub(crate) fn sample(_now: i64) -> io::Result<Vec<Self>> {
        macos::sample()
    }

    /// The executable file `pid` runs, resolved by the kernel rather than the
    /// path it was launched by; `None` when the kernel withholds it.
    #[cfg(target_os = "macos")]
    pub(crate) fn executable(pid: u32) -> Option<PathBuf> {
        macos::executable(pid)
    }

    #[cfg(not(target_os = "macos"))]
    pub(crate) fn executable(pid: u32) -> Option<PathBuf> {
        let path = std::fs::read_link(format!("/proc/{pid}/exe")).ok()?;
        // Linux names a replaced or removed file `<path> (deleted)`.
        let text = path.to_string_lossy();
        Some(match text.strip_suffix(" (deleted)") {
            Some(path) => PathBuf::from(path),
            None => path,
        })
    }

    /// Zombies and processes the kernel is tearing down can never run user code.
    pub(crate) fn has_exited(&self) -> bool {
        self.kernel_state.starts_with(['Z', 'X'])
    }

    pub(crate) fn evidence(&self, started_at: i64) -> ProcessIdentityEvidence {
        if self.has_exited() || self.started_at.abs_diff(started_at) > 3 {
            ProcessIdentityEvidence::Dead
        } else {
            ProcessIdentityEvidence::Live
        }
    }

    pub(crate) fn matches_start(&self, pid: u32, started_at: i64) -> bool {
        self.pid == pid && self.evidence(started_at) == ProcessIdentityEvidence::Live
    }

    /// Members of a process group that have not exited.
    pub(crate) fn group(pgid: u32) -> io::Result<Vec<Self>> {
        #[cfg(target_os = "macos")]
        let members = macos::group(pgid)?;
        #[cfg(not(target_os = "macos"))]
        let members = Self::sample(time::OffsetDateTime::now_utc().unix_timestamp())?;
        Ok(members
            .into_iter()
            .filter(|process| process.pgid == pgid && !process.has_exited())
            .collect())
    }

    /// Name processes for an error without their arguments: `pid 12 (sleep, S)`.
    pub(crate) fn describe(processes: &[Self]) -> String {
        processes
            .iter()
            .map(|process| {
                let program = process.command.split_whitespace().next().unwrap_or("");
                let name = program.rsplit('/').next().unwrap_or(program);
                format!("pid {} ({name}, {})", process.pid, process.kernel_state)
            })
            .collect::<Vec<_>>()
            .join(", ")
    }
}

#[cfg(not(target_os = "macos"))]
fn query(selection: &[&str], now: i64) -> io::Result<Vec<OsProcess>> {
    let output = Command::new("ps")
        .args(selection)
        .args(["-ww", "-o", "pid=,ppid=,pgid=,state=,etime=,command="])
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

#[cfg(any(not(target_os = "macos"), test))]
fn parse(line: &str, now: i64) -> Option<OsProcess> {
    // The command is the final column and may itself contain whitespace.
    // ps pads columns, so consume each fixed field from the remaining suffix.
    let mut remaining = line.trim();
    let mut next = || {
        let end = remaining.find(char::is_whitespace)?;
        let field = &remaining[..end];
        remaining = remaining[end..].trim_start();
        Some(field)
    };
    let pid = next()?.parse().ok()?;
    let ppid = next()?.parse().ok()?;
    let pgid = next()?.parse().ok()?;
    let kernel_state = next()?.to_owned();
    let elapsed = i64::try_from(elapsed_seconds(next()?)?).ok()?;
    if remaining.is_empty() {
        return None;
    }
    Some(OsProcess {
        pid,
        ppid,
        pgid,
        started_at: now.checked_sub(elapsed)?,
        kernel_state,
        command: remaining.to_owned(),
    })
}

#[cfg(any(not(target_os = "macos"), test))]
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
    use std::os::unix::process::CommandExt;

    use super::{parse, OsProcess};
    use crate::journal::ProcessIdentityEvidence;

    #[test]
    fn executable_is_the_running_file_not_its_launch_path() {
        let expected = std::fs::canonicalize(std::env::current_exe().unwrap()).unwrap();
        let actual = OsProcess::executable(std::process::id()).unwrap();
        assert_eq!(std::fs::canonicalize(actual).unwrap(), expected);
    }

    #[test]
    fn activity_and_control_share_identity_and_zombie_evidence() {
        for (state, age, start, evidence) in [
            ("S", "01:00", 99_940, ProcessIdentityEvidence::Live),
            ("R", "02:00:00", 92_800, ProcessIdentityEvidence::Live),
            ("Z", "1-00:00:00", 13_600, ProcessIdentityEvidence::Dead),
            ("S", "2-01:02:03", -76_523, ProcessIdentityEvidence::Live),
            ("S", "01:00", 99_930, ProcessIdentityEvidence::Dead),
        ] {
            let line = format!("  10   1  10 {state} {age} /path with spaces/codex app-server");
            let process = parse(&line, 100_000).unwrap();
            assert_eq!(process.ppid, 1);
            assert_eq!(process.command, "/path with spaces/codex app-server");
            assert_eq!(process.evidence(start), evidence);
            assert_eq!(
                process.matches_start(10, start),
                evidence == ProcessIdentityEvidence::Live
            );
            assert!(!process.matches_start(11, start));
        }
        for line in [
            "10 1 10 S ? fixture",
            "10 1 10 S 00:bad:01 fixture",
            "10 1 10 S 18446744073709551615-00:00 fixture",
            "bad 1 10 S 00:00 fixture",
            "10 1 bad S 00:00 fixture",
            "10 1 10 S 00:00",
            "10 bad 10 S 00:00 fixture",
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
        assert_eq!(sample.pgid, observed.pgid);
        assert_eq!(evidence, ProcessIdentityEvidence::Live);
        assert_eq!(
            crate::journal::process_identity_evidence(child.id(), observed.started_at),
            ProcessIdentityEvidence::Dead
        );
    }

    #[test]
    fn group_lists_live_members_until_they_are_killed() {
        let leader = Child(
            std::process::Command::new("/bin/sleep")
                .env_clear()
                .arg("60")
                .process_group(0)
                .spawn()
                .unwrap(),
        );
        let pgid = leader.0.id();
        let helper = Child(
            std::process::Command::new("/bin/sleep")
                .env_clear()
                .arg("60")
                .process_group(pgid as i32)
                .spawn()
                .unwrap(),
        );
        let mut members = OsProcess::group(pgid)
            .unwrap()
            .into_iter()
            .map(|process| process.pid)
            .collect::<Vec<_>>();
        members.sort();
        assert_eq!(members, [pgid, helper.0.id()]);
        assert!(OsProcess::describe(&OsProcess::group(pgid).unwrap()).contains("(sleep, "));
        drop(helper);
        drop(leader);
        assert!(OsProcess::group(pgid).unwrap().is_empty());
    }

    #[test]
    fn zombie_is_dead_before_its_parent_reaps_it() {
        let mut owned = Child(
            std::process::Command::new("/bin/sh")
                .env_clear()
                .args(["-c", "exit 0"])
                .process_group(0)
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
        assert_eq!(observed.pgid, child.id());
        assert!(OsProcess::group(observed.pgid).unwrap().is_empty());
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
