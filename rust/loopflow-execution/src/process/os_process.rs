//! One OS identity reader, and the one rule that reads an owner record's
//! liveness from it. Invalid observations are errors, never evidence that a
//! process disappeared.

use std::io;
use std::path::PathBuf;
#[cfg(not(target_os = "macos"))]
use std::process::Command;

#[cfg(target_os = "macos")]
mod macos;

use std::sync::OnceLock;

use crate::store::ProcessIdentityEvidence;
use loopflow_storage::graph_process::{Execution, ExecutionResult, TaskCarrier};
use loopflow_storage::process::AttachmentHolder;
use loopflow_storage::sqlite::SqliteStore;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OsProcess {
    pub pid: u32,
    pub ppid: u32,
    pub pgid: u32,
    pub started_at: i64,
    pub kernel_state: String,
    pub command: String,
}

impl OsProcess {
    /// Argument boundaries from the kernel, excluding the process environment.
    pub fn arguments(pid: u32) -> io::Result<Option<Vec<String>>> {
        #[cfg(target_os = "macos")]
        {
            macos::arguments(pid)
        }
        #[cfg(not(target_os = "macos"))]
        {
            match std::fs::read(format!("/proc/{pid}/cmdline")) {
                Ok(bytes) => Ok(Some(
                    bytes
                        .strip_suffix(&[0])
                        .unwrap_or(&bytes)
                        .split(|byte| *byte == 0)
                        .map(|arg| String::from_utf8_lossy(arg).into_owned())
                        .collect(),
                )),
                Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(None),
                Err(error) => Err(error),
            }
        }
    }

    /// The values of `names` in `pid`'s environment, and nothing else from
    /// it: an environment holds secrets that never reach output. `None` when
    /// the process is gone or not readable.
    pub fn environment(pid: u32, names: &[&str]) -> io::Result<Option<Vec<(String, String)>>> {
        #[cfg(target_os = "macos")]
        {
            macos::environment(pid, names)
        }
        #[cfg(not(target_os = "macos"))]
        {
            match std::fs::read(format!("/proc/{pid}/environ")) {
                Ok(bytes) => Ok(Some(select_environment(
                    bytes
                        .split(|byte| *byte == 0)
                        .filter(|entry| !entry.is_empty()),
                    names,
                ))),
                Err(error)
                    if matches!(
                        error.kind(),
                        io::ErrorKind::NotFound | io::ErrorKind::PermissionDenied
                    ) =>
                {
                    Ok(None)
                }
                Err(error) => Err(error),
            }
        }
    }

    pub fn read(pid: u32) -> io::Result<Option<Self>> {
        crate::observe::process(pid)
    }

    #[cfg(target_os = "macos")]
    pub fn observe(pid: u32) -> io::Result<Option<Self>> {
        macos::read(pid)
    }

    #[cfg(not(target_os = "macos"))]
    pub fn observe(pid: u32) -> io::Result<Option<Self>> {
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
    pub fn read_each(pids: &[u32], _now: i64) -> io::Result<Vec<Self>> {
        pids.iter()
            .filter_map(|pid| Self::read(*pid).transpose())
            .collect()
    }

    #[cfg(not(target_os = "macos"))]
    pub fn read_each(pids: &[u32], now: i64) -> io::Result<Vec<Self>> {
        if crate::observe::active() {
            return pids
                .iter()
                .filter_map(|pid| Self::read(*pid).transpose())
                .collect();
        }
        if pids.is_empty() {
            return Ok(Vec::new());
        }
        let pids = pids.iter().map(u32::to_string).collect::<Vec<_>>();
        query(&["-p", &pids.join(",")], now)
    }

    #[cfg(not(target_os = "macos"))]
    pub fn sample(now: i64) -> io::Result<Vec<Self>> {
        query(&["-ax"], now)
    }

    #[cfg(target_os = "macos")]
    pub fn sample(_now: i64) -> io::Result<Vec<Self>> {
        macos::sample()
    }

    /// Every process with `command` set to its kernel name, never its arguments.
    #[cfg(target_os = "macos")]
    pub(crate) fn sample_names() -> io::Result<Vec<Self>> {
        macos::sample_names()
    }

    #[cfg(not(target_os = "macos"))]
    pub(crate) fn sample_names() -> io::Result<Vec<Self>> {
        query_column(
            &["-ax"],
            "comm",
            time::OffsetDateTime::now_utc().unix_timestamp(),
        )
    }

    /// The directory `pid` runs in; `None` when the kernel withholds it.
    #[cfg(target_os = "macos")]
    pub fn cwd(pid: u32) -> Option<PathBuf> {
        macos::cwd(pid)
    }

    /// Live OS threads in this process, read directly from the kernel without forking.
    pub fn current_thread_count() -> u64 {
        #[cfg(target_os = "macos")]
        {
            macos::thread_count(std::process::id())
        }
        #[cfg(not(target_os = "macos"))]
        {
            std::fs::read_dir("/proc/self/task")
                .map(|entries| entries.count() as u64)
                .unwrap_or(0)
        }
    }

    #[cfg(not(target_os = "macos"))]
    pub fn cwd(pid: u32) -> Option<PathBuf> {
        std::fs::read_link(format!("/proc/{pid}/cwd")).ok()
    }

    /// Only this owned OS session, without reading unrelated processes' argv.
    pub fn session(session: u32) -> io::Result<Vec<Self>> {
        #[cfg(target_os = "macos")]
        let pids = macos::pids()?;
        #[cfg(not(target_os = "macos"))]
        let pids = std::fs::read_dir("/proc")?
            .filter_map(|entry| entry.ok()?.file_name().to_str()?.parse::<u32>().ok())
            .collect::<Vec<_>>();
        let mut processes = Vec::new();
        for pid in pids {
            // SAFETY: getsid reads the kernel's session membership for this PID.
            if unsafe { libc::getsid(pid as i32) } != session as i32 {
                continue;
            }
            if let Some(process) = Self::read(pid)? {
                if !process.has_exited() {
                    processes.push(process);
                }
            }
        }
        Ok(processes)
    }

    /// The executable file `pid` runs, resolved by the kernel rather than the
    /// path it was launched by; `None` when the kernel withholds it.
    #[cfg(target_os = "macos")]
    pub fn executable(pid: u32) -> Option<PathBuf> {
        macos::executable(pid)
    }

    #[cfg(not(target_os = "macos"))]
    pub fn executable(pid: u32) -> Option<PathBuf> {
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

    pub fn evidence(&self, started_at: i64) -> ProcessIdentityEvidence {
        if self.has_exited() || self.started_at.abs_diff(started_at) > 3 {
            ProcessIdentityEvidence::Dead
        } else {
            ProcessIdentityEvidence::Live
        }
    }

    pub fn matches_start(&self, pid: u32, started_at: i64) -> bool {
        self.pid == pid && self.evidence(started_at) == ProcessIdentityEvidence::Live
    }

    /// Members of a process group that have not exited.
    pub fn group(pgid: u32) -> io::Result<Vec<Self>> {
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
    pub fn describe(processes: &[Self]) -> String {
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
    query_column(selection, "command", now)
}

/// `last` is the final, free-text column: `command` or `comm`.
#[cfg(not(target_os = "macos"))]
fn query_column(selection: &[&str], last: &str, now: i64) -> io::Result<Vec<OsProcess>> {
    let output = Command::new("ps")
        .args(selection)
        .args([
            "-ww",
            "-o",
            &format!("pid=,ppid=,pgid=,state=,etime=,{last}="),
        ])
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

/// `NAME=value` entries whose name is one of `names`.
fn select_environment<'a>(
    entries: impl Iterator<Item = &'a [u8]>,
    names: &[&str],
) -> Vec<(String, String)> {
    entries
        .filter_map(|entry| {
            let entry = String::from_utf8_lossy(entry);
            let (name, value) = entry.split_once('=')?;
            names
                .contains(&name)
                .then(|| (name.to_owned(), value.to_owned()))
        })
        .collect()
}

// ---------------------------------------------------------------------------
// Owner liveness

/// What an owner record (a Graph run or step, a carrier, a Session's agent or
/// attachment holder) says about the OS process it runs as.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RecordedProcess {
    pub ended: bool,
    pub pid: Option<u32>,
    pub os_started_at: Option<i64>,
    /// When the record began; a record with no pid that began before this
    /// Machine booted has exited.
    pub started_at: Option<i64>,
}

impl RecordedProcess {
    /// Ended is dead; a pid with its OS start is one OS lookup; a pid without
    /// it cannot be told from a reused one; no pid is dead only across a boot.
    pub fn evidence(&self) -> ProcessIdentityEvidence {
        self.evidence_by(process_identity_evidence)
    }

    pub fn evidence_by(
        &self,
        mut observe: impl FnMut(u32, i64) -> ProcessIdentityEvidence,
    ) -> ProcessIdentityEvidence {
        if self.ended {
            return ProcessIdentityEvidence::Dead;
        }
        match (self.pid, self.os_started_at) {
            (Some(pid), Some(started_at)) => observe(pid, started_at),
            (Some(_), None) => ProcessIdentityEvidence::Unknown,
            (None, _) if self.started_at.is_some_and(began_before_boot) => {
                ProcessIdentityEvidence::Dead
            }
            (None, _) => ProcessIdentityEvidence::Unknown,
        }
    }
}

impl From<&Execution> for RecordedProcess {
    fn from(run: &Execution) -> Self {
        Self {
            ended: run.ended_at.is_some(),
            pid: run.pid,
            os_started_at: run.os_started_at,
            started_at: Some(run.started_at),
        }
    }
}

impl From<&AttachmentHolder> for RecordedProcess {
    fn from(holder: &AttachmentHolder) -> Self {
        Self {
            ended: false,
            pid: holder.pid,
            os_started_at: holder.os_started_at,
            started_at: None,
        }
    }
}

/// One OS lookup: whether `pid` is still the process that started at `started_at`.
pub(crate) fn process_identity_evidence(pid: u32, started_at: i64) -> ProcessIdentityEvidence {
    match OsProcess::read(pid) {
        Ok(Some(process)) => process.evidence(started_at),
        Ok(None) => ProcessIdentityEvidence::Dead,
        Err(_) => ProcessIdentityEvidence::Unknown,
    }
}

/// A Graph step its Graph reserved but has not started yet has no pid; it
/// stands or falls with the Graph run that would start it.
pub fn step_evidence(step: &Execution, graph: &Execution) -> ProcessIdentityEvidence {
    if step.ended_at.is_none() && step.pid.is_none() {
        return RecordedProcess::from(graph).evidence();
    }
    RecordedProcess::from(step).evidence()
}

/// A Task carrier with no OS process ended, or was moved here from another
/// Machine.
pub fn carrier_evidence(carrier: &TaskCarrier) -> ProcessIdentityEvidence {
    match carrier.pid {
        Some(_) => RecordedProcess {
            ended: false,
            pid: carrier.pid,
            os_started_at: carrier.os_started_at,
            started_at: None,
        }
        .evidence(),
        None => ProcessIdentityEvidence::Dead,
    }
}

/// Only the lf holding a reserved Session agent starts it, so a reservation
/// stands or falls with its holder; with none, nothing will start it.
pub fn session_agent_evidence(
    agent: RecordedProcess,
    holder: Option<&AttachmentHolder>,
) -> ProcessIdentityEvidence {
    if !agent.ended && agent.pid.is_none() {
        let holder = holder.map_or(ProcessIdentityEvidence::Dead, |holder| {
            RecordedProcess::from(holder).evidence()
        });
        if holder == ProcessIdentityEvidence::Dead {
            return ProcessIdentityEvidence::Dead;
        }
    }
    agent.evidence()
}

/// The liveness of agent `process` while it is its Session's current one.
pub(crate) fn agent_evidence(
    store: &SqliteStore,
    process: &loopflow_model::id::AgentProcessId,
) -> ProcessIdentityEvidence {
    let Ok(Some(agent)) = store.agent_state(process) else {
        return ProcessIdentityEvidence::Unknown;
    };
    session_agent_evidence(
        RecordedProcess {
            ended: agent.ended_at.is_some(),
            pid: agent.pid,
            os_started_at: agent.os_started_at,
            started_at: agent.started_at,
        },
        agent.holder.as_ref(),
    )
}

/// A recorded run's result as this Machine sees it: one that never wrote its
/// end and whose pid is dead ended unseen, so it reads `Unknown`.
pub fn observed_result(run: &Execution) -> ExecutionResult {
    match run.result() {
        ExecutionResult::Running
            if RecordedProcess::from(run).evidence() == ProcessIdentityEvidence::Dead =>
        {
            ExecutionResult::Unknown
        }
        result => result,
    }
}

#[cfg(any(test, feature = "test-support"))]
thread_local! {
    static TEST_MACHINE_BOOTED_AT: std::cell::Cell<Option<i64>> = const { std::cell::Cell::new(None) };
}

/// Pretend this machine booted at `at` for the rest of the test thread.
#[cfg(any(test, feature = "test-support"))]
pub fn set_test_machine_booted_at(at: Option<i64>) {
    TEST_MACHINE_BOOTED_AT.with(|cell| cell.set(at));
}

/// When this machine last booted. No local process survives that boundary, so
/// work that began earlier and never settled has exited.
pub(crate) fn machine_booted_at() -> Option<i64> {
    #[cfg(any(test, feature = "test-support"))]
    if let Some(at) = TEST_MACHINE_BOOTED_AT.with(std::cell::Cell::get) {
        return Some(at);
    }
    static BOOTED_AT: OnceLock<Option<i64>> = OnceLock::new();
    *BOOTED_AT.get_or_init(read_machine_booted_at)
}

pub(crate) fn began_before_boot(started_at: i64) -> bool {
    machine_booted_at().is_some_and(|booted_at| started_at < booted_at)
}

fn read_machine_booted_at() -> Option<i64> {
    if let Ok(stat) = std::fs::read_to_string("/proc/stat") {
        return stat
            .lines()
            .find_map(|line| line.strip_prefix("btime "))
            .and_then(|value| value.trim().parse().ok());
    }
    let output = std::process::Command::new("sysctl")
        .args(["-n", "kern.boottime"])
        .output()
        .ok()
        .filter(|output| output.status.success())?;
    parse_sysctl_boottime(&String::from_utf8_lossy(&output.stdout))
}

/// `{ sec = 1791140551, usec = 157377 } Sun Oct  4 12:02:31 2026`
fn parse_sysctl_boottime(value: &str) -> Option<i64> {
    let seconds = value.split_once("sec = ")?.1;
    seconds.split([',', ' ']).next()?.parse().ok()
}

pub fn process_started_at(pid: u32) -> Result<Option<i64>, std::io::Error> {
    Ok(OsProcess::read(pid)?.map(|process| process.started_at))
}

#[cfg(test)]
mod tests {
    use std::os::unix::process::CommandExt;

    #[test]
    fn a_record_without_os_identity_is_dead_only_after_a_restart() {
        let unobserved = |_: u32, _: i64| -> ProcessIdentityEvidence { unreachable!() };
        let began = |started_at| super::RecordedProcess {
            ended: false,
            pid: None,
            os_started_at: None,
            started_at,
        };
        super::set_test_machine_booted_at(Some(1_000));
        let before = began(Some(999)).evidence_by(unobserved);
        let since = began(Some(1_001)).evidence_by(unobserved);
        let unrecorded = began(None).evidence_by(unobserved);
        super::set_test_machine_booted_at(None);
        assert_eq!(before, ProcessIdentityEvidence::Dead);
        assert_eq!(since, ProcessIdentityEvidence::Unknown);
        assert_eq!(unrecorded, ProcessIdentityEvidence::Unknown);
    }

    #[test]
    fn a_reserved_step_ends_with_the_graph_that_would_start_it() {
        use loopflow_model::id::LfProcessId;
        let run = |ended_at: Option<i64>| loopflow_storage::graph_process::Execution {
            id: LfProcessId::default(),
            started_at: 1,
            ended_at,
            pid: None,
            os_started_at: None,
            exit_code: None,
            error: None,
        };
        let reserved = run(None);
        let live_graph = loopflow_storage::graph_process::Execution {
            pid: Some(std::process::id()),
            os_started_at: super::process_started_at(std::process::id()).unwrap(),
            ..run(None)
        };
        assert_eq!(
            super::step_evidence(&reserved, &run(Some(2))),
            ProcessIdentityEvidence::Dead
        );
        assert_eq!(
            super::step_evidence(&reserved, &live_graph),
            ProcessIdentityEvidence::Live
        );
    }

    #[test]
    fn boot_time_parses_and_bounds_what_can_still_run() {
        assert_eq!(
            super::parse_sysctl_boottime(
                "{ sec = 1791140551, usec = 157377 } Sun Oct  4 12:02:31 2026"
            ),
            Some(1_791_140_551)
        );
        assert_eq!(super::parse_sysctl_boottime("unavailable"), None);
        // This test began after the machine that runs it booted.
        let now = time::OffsetDateTime::now_utc().unix_timestamp();
        assert!(super::machine_booted_at().is_some_and(|booted_at| booted_at <= now));
        assert!(!super::began_before_boot(now));
        assert!(super::began_before_boot(1));
    }

    use super::{parse, OsProcess};
    use crate::store::ProcessIdentityEvidence;

    #[test]
    fn a_child_reads_with_its_kernel_name_and_directory() {
        let directory = tempfile::tempdir().unwrap();
        let mut child = std::process::Command::new("sleep")
            .arg("30")
            .current_dir(directory.path())
            .spawn()
            .unwrap();
        let named = OsProcess::sample_names().unwrap();
        let cwd = OsProcess::cwd(child.id());
        child.kill().unwrap();
        child.wait().unwrap();
        let process = named.iter().find(|p| p.pid == child.id()).unwrap();
        assert_eq!(process.command, "sleep");
        assert_eq!(process.ppid, std::process::id());
        assert_eq!(cwd, Some(directory.path().canonicalize().unwrap()));
    }

    #[test]
    fn kernel_arguments_match_the_running_program() {
        assert_eq!(
            OsProcess::arguments(std::process::id()).unwrap(),
            Some(std::env::args().collect())
        );
    }

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
        let evidence = super::process_identity_evidence(child.id(), observed.started_at);
        child.kill().unwrap();
        child.wait().unwrap();
        assert!(sample.matches_start(observed.pid, observed.started_at));
        assert_eq!(sample.pgid, observed.pgid);
        assert_eq!(evidence, ProcessIdentityEvidence::Live);
        assert_eq!(
            super::process_identity_evidence(child.id(), observed.started_at),
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
            super::process_identity_evidence(child.id(), observed.started_at),
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
