//! How each Desktop bundle's last launch ended, for `lf doctor`.
//!
//! Desktop's launch journal (`desktop-cache/timings/launches.ndjson`) records
//! each launch and at most one `exit` line: a quit and who asked, an `exit()` and
//! its caller, a catchable signal, or an uncaught exception. A launch with no
//! `exit` line ended where Desktop could not see it (SIGKILL, power loss), so the
//! check asks macOS: runningboardd logs `termination reported by launchd
//! (namespace, code, wait status)` for every app. Desktop writes a vitals line
//! to lf.log every 300 s, so the query searches from the launch's last line (or
//! its start) through one interval plus slack, never past the next launch.
//! Every end names how long the launch lived; every end but a quit appends the
//! last vitals (or read-storm) line, the state Desktop was in before it went.
//!
//! Each bundle lists, newest first, up to three launches that ended abnormally in
//! the last 24 h, its latest end, and its last quit in that time.

use std::path::Path;
use std::process::Command;
use std::time::Duration;

use chrono::{DateTime, Local, Utc};
use serde_json::{Map, Value};

use crate::diagnostic_log::{field, log_path, read_tail};
use crate::lf::commands::doctor::Check;

const NAME: &str = "desktop-exit";
const RECENT: chrono::TimeDelta = chrono::TimeDelta::hours(24);
/// One `DesktopVitals` interval plus slack: an unrecorded end follows the last line within it.
const HEARTBEAT: chrono::TimeDelta = chrono::TimeDelta::seconds(300 + 60);
const LISTED: usize = 3;
const SEARCH_TIMEOUT: Duration = Duration::from_secs(20);
/// A recorded `exit()` names its caller; the system record around it names the status.
const EXIT_SLACK: chrono::TimeDelta = chrono::TimeDelta::seconds(60);

/// runningboardd's `(namespace, code, wait status)` for a process launchd reaped.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Termination {
    namespace: i64,
    code: i64,
    status: i64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum SystemRecord {
    Found(Termination),
    Missing,
    Unavailable,
}

pub(crate) fn check() -> Check {
    let journal = crate::store::lf_home_dir()
        .join("desktop-cache")
        .join("timings")
        .join("launches.ndjson");
    let journal = std::fs::read_to_string(journal).unwrap_or_default();
    let log = read_tail(&log_path()).unwrap_or_default();
    report(&journal, &log, Utc::now(), pid_alive, system_record)
}

#[derive(Debug)]
struct Launch {
    id: String,
    app: String,
    at: Option<DateTime<Utc>>,
    pid: Option<u32>,
    bundle: Option<String>,
    exit: Option<Map<String, Value>>,
}

impl Launch {
    fn title(&self) -> String {
        let dev = if self.bundle.as_deref().is_some_and(|b| b.ends_with(".dev")) {
            " Dev"
        } else {
            ""
        };
        match self.pid {
            Some(pid) => format!("Loopflow{dev} {} (pid {pid})", self.app),
            None => format!("Loopflow{dev} {}", self.app),
        }
    }
}

fn report(
    journal: &str,
    log: &str,
    now: DateTime<Utc>,
    alive: impl Fn(u32) -> bool,
    lookup: impl Fn(u32, DateTime<Utc>, DateTime<Utc>) -> SystemRecord,
) -> Check {
    let launches = parse(journal);
    if launches.is_empty() {
        return Check::ok(NAME, "no Desktop launches recorded");
    }
    // Launches from before the journal named bundles matter only until one does.
    let bundled = launches.iter().any(|launch| launch.bundle.is_some());
    let mut bundles: Vec<Option<&str>> = Vec::new();
    for launch in &launches {
        let bundle = launch.bundle.as_deref();
        if (bundle.is_some() || !bundled) && !bundles.contains(&bundle) {
            bundles.push(bundle);
        }
    }
    let mut warn = false;
    let mut lines = Vec::new();
    for bundle in bundles {
        let group: Vec<&Launch> = launches
            .iter()
            .filter(|launch| launch.bundle.as_deref() == bundle)
            .collect();
        let mut ends: Vec<(&Launch, Option<DateTime<Utc>>)> = Vec::new();
        for (index, launch) in group.iter().enumerate().rev() {
            let next = group.get(index + 1).and_then(|next| next.at);
            let gone = launch.pid.is_some_and(|pid| !alive(pid));
            if launch.exit.is_some() || next.is_some() || gone {
                ends.push((launch, next));
            } else if index + 1 == group.len() {
                lines.push(format!(
                    "{} running since {}",
                    launch.title(),
                    launch
                        .at
                        .map_or("an unknown time".into(), |at| clock(at, now))
                ));
            }
        }
        let recent = |(launch, next): &(&Launch, Option<DateTime<Utc>>)| {
            ended_at(launch, *next, now).is_none_or(|at| now - at <= RECENT)
        };
        let abnormal: Vec<usize> = (0..ends.len())
            .filter(|&index| !quit(ends[index].0) && recent(&ends[index]))
            .collect();
        warn |= !abnormal.is_empty();
        // The newest abnormal ends, the latest end, and the last recent quit, newest first.
        let last_quit = ends.iter().position(|end| quit(end.0) && recent(end));
        for (index, (launch, next)) in ends.iter().enumerate() {
            if index == 0
                || Some(index) == last_quit
                || abnormal.iter().take(LISTED).any(|&shown| shown == index)
            {
                lines.push(describe(launch, *next, log, now, &lookup));
            }
        }
        if abnormal.len() > LISTED {
            lines.push(format!("… and {} earlier", abnormal.len() - LISTED));
        }
    }
    let detail = lines.join(&format!("\n{:20}", ""));
    if warn {
        Check::warn(NAME, detail)
    } else {
        Check::ok(NAME, detail)
    }
}

fn quit(launch: &Launch) -> bool {
    launch
        .exit
        .as_ref()
        .and_then(|exit| exit.get("reason"))
        .and_then(Value::as_str)
        == Some("quit")
}

/// When `launch` ended: its exit line's time, else by the next launch (or now).
fn ended_at(
    launch: &Launch,
    next: Option<DateTime<Utc>>,
    now: DateTime<Utc>,
) -> Option<DateTime<Utc>> {
    match &launch.exit {
        Some(exit) => exit.get("at").and_then(Value::as_str).and_then(parse_time),
        None => Some(next.unwrap_or(now)),
    }
}

/// The line for one ended launch.
fn describe(
    launch: &Launch,
    next: Option<DateTime<Utc>>,
    log: &str,
    now: DateTime<Utc>,
    lookup: &impl Fn(u32, DateTime<Utc>, DateTime<Utc>) -> SystemRecord,
) -> String {
    let title = launch.title();
    let lines = launch
        .pid
        .map(|pid| desktop_lines(log, pid, launch.at))
        .unwrap_or_default();
    // A quit needs no explaining; any other end shows how Desktop last looked.
    let vitals = if quit(launch) {
        String::new()
    } else {
        last_vitals(&lines, now)
    };
    let Some(exit) = &launch.exit else {
        let end = next.unwrap_or(now);
        let Some(pid) = launch.pid else {
            return format!(
                "{title} ended without a record before {}{} (its Desktop predates exit records)",
                clock(end, now),
                lived(launch, end)
            );
        };
        let start = lines
            .first()
            .map(|(at, _)| *at)
            .or(launch.at)
            .unwrap_or(end - HEARTBEAT);
        let end = end.min(start + HEARTBEAT);
        let what = match lookup(pid, start, end) {
            SystemRecord::Found(termination) => {
                format!("{} before {}", explain(termination), clock(end, now))
            }
            SystemRecord::Missing => format!(
                "ended without a record between {} and {} (SIGKILL before the first heartbeat, power loss, or the system log rotated)",
                clock(start, now),
                clock(end, now)
            ),
            SystemRecord::Unavailable => format!(
                "ended without a record before {}; system record unavailable",
                clock(end, now)
            ),
        };
        return format!("{title} {what}{}{vitals}", lived(launch, end));
    };

    let text = |key: &str| exit.get(key).and_then(Value::as_str).unwrap_or("");
    let at = exit.get("at").and_then(Value::as_str).and_then(parse_time);
    let when = at.map_or("at an unknown time".into(), |at| clock(at, now));
    let what = match text("reason") {
        "quit" => {
            let sender = match (text("sender"), text("sender_pid")) {
                ("", "") => String::new(),
                ("", pid) => format!(": Quit requested by pid {pid}"),
                (name, "") => format!(": Quit requested by {name}"),
                (name, pid) => format!(": Quit requested by {name} (pid {pid})"),
            };
            let why = match text("quit_reason") {
                "" => String::new(),
                why => format!(" for {why}"),
            };
            format!("quit {when}{sender}{why}")
        }
        "exit" => {
            let caller = match text("caller") {
                "" => String::new(),
                caller => format!(" from {caller}"),
            };
            let status = match (launch.pid, at) {
                (Some(pid), Some(at)) => match lookup(pid, at - EXIT_SLACK, at + EXIT_SLACK) {
                    SystemRecord::Found(termination) => format!("; {}", explain(termination)),
                    _ => String::new(),
                },
                _ => String::new(),
            };
            format!("called exit() {when}{caller}{status}")
        }
        "signal" => format!("was killed {when} by {}", text("signal")),
        "exception" => format!(
            "raised an uncaught {} {when}: {}",
            text("name"),
            text("message")
        ),
        other => format!("ended {when} ({other})"),
    };
    let lived = at.map_or_else(String::new, |at| lived(launch, at));
    format!("{title} {what}{lived}{vitals}")
}

/// `, <span> after launch` up to `end`, when both ends are known.
fn lived(launch: &Launch, end: DateTime<Utc>) -> String {
    match launch.at {
        Some(start) if end >= start => format!(", {} after launch", duration(end - start)),
        _ => String::new(),
    }
}

/// `; last vitals HH:MM: …` from the newest heartbeat (plain or storm) among `lines`.
fn last_vitals(lines: &[(DateTime<Utc>, &str)], now: DateTime<Utc>) -> String {
    let Some((at, line, footprint, reads)) = lines.iter().find_map(|(at, line)| {
        Some((
            at,
            line,
            field(line, "footprint_mb")?,
            field(line, "reads")?,
        ))
    }) else {
        return String::new();
    };
    let killed = match field(line, "killed") {
        None | Some("0") => String::new(),
        Some(killed) => format!(", {killed} killed"),
    };
    format!(
        "; last vitals {}: {footprint} MB, {reads} lf reads/5 min{killed}",
        minute(*at, now)
    )
}

fn parse(journal: &str) -> Vec<Launch> {
    let mut launches: Vec<Launch> = Vec::new();
    for line in journal.lines() {
        // A torn or foreign line costs only itself.
        let Ok(Value::Object(record)) = serde_json::from_str::<Value>(line) else {
            continue;
        };
        let text = |key: &str| record.get(key).and_then(Value::as_str).map(str::to_owned);
        let Some(id) = text("launch") else { continue };
        match record.get("event").and_then(Value::as_str) {
            Some("launch") => launches.push(Launch {
                app: text("app").unwrap_or_else(|| "unknown".into()),
                at: text("at").as_deref().and_then(parse_time),
                pid: record
                    .get("pid")
                    .and_then(Value::as_u64)
                    .and_then(|pid| u32::try_from(pid).ok()),
                bundle: text("bundle"),
                exit: None,
                id,
            }),
            Some("exit") => {
                if let Some(launch) = launches.iter_mut().rev().find(|launch| launch.id == id) {
                    launch.exit.get_or_insert(record);
                }
            }
            _ => {}
        }
    }
    launches
}

/// Desktop's lf.log lines for `pid` during this launch with their times, newest first.
fn desktop_lines(log: &str, pid: u32, since: Option<DateTime<Utc>>) -> Vec<(DateTime<Utc>, &str)> {
    let pid = pid.to_string();
    log.lines()
        .rev()
        .filter(|line| field(line, "desktop").is_some() && field(line, "pid") == Some(pid.as_str()))
        .filter_map(|line| Some((line.split_whitespace().next().and_then(parse_time)?, line)))
        .take_while(|(at, _)| since.is_none_or(|since| *at >= since))
        .collect()
}

fn parse_time(text: &str) -> Option<DateTime<Utc>> {
    DateTime::parse_from_rfc3339(text)
        .ok()
        .map(|at| at.with_timezone(&Utc))
}

fn explain(termination: Termination) -> String {
    let Termination {
        namespace,
        code,
        status,
    } = termination;
    match namespace {
        0 if status & 0x7f == 0 => format!("exited with status {}", (status >> 8) & 0xff),
        0 => format!("was killed by {}", signal_name(status & 0x7f)),
        1 => format!("was ended by jetsam (memory pressure, code {code})"),
        2 if code == 9 => "was killed by SIGKILL (Force Quit, kill -9, or the system)".to_string(),
        2 => format!("was killed by {}", signal_name(code)),
        _ => {
            format!("was ended by the system (namespace {namespace}, code {code}, status {status})")
        }
    }
}

fn signal_name(number: i64) -> String {
    const NAMES: [&str; 32] = [
        "",
        "SIGHUP",
        "SIGINT",
        "SIGQUIT",
        "SIGILL",
        "SIGTRAP",
        "SIGABRT",
        "SIGEMT",
        "SIGFPE",
        "SIGKILL",
        "SIGBUS",
        "SIGSEGV",
        "SIGSYS",
        "SIGPIPE",
        "SIGALRM",
        "SIGTERM",
        "SIGURG",
        "SIGSTOP",
        "SIGTSTP",
        "SIGCONT",
        "SIGCHLD",
        "SIGTTIN",
        "SIGTTOU",
        "SIGIO",
        "SIGXCPU",
        "SIGXFSZ",
        "SIGVTALRM",
        "SIGPROF",
        "SIGWINCH",
        "SIGINFO",
        "SIGUSR1",
        "SIGUSR2",
    ];
    usize::try_from(number)
        .ok()
        .and_then(|index| NAMES.get(index))
        .filter(|name| !name.is_empty())
        .map_or_else(|| format!("signal {number}"), |name| (*name).to_string())
}

/// Local time to the second; the date too when it is not today.
fn clock(at: DateTime<Utc>, now: DateTime<Utc>) -> String {
    local(at, now, "%H:%M:%S")
}

/// Local time to the minute; the date too when it is not today.
fn minute(at: DateTime<Utc>, now: DateTime<Utc>) -> String {
    local(at, now, "%H:%M")
}

fn local(at: DateTime<Utc>, now: DateTime<Utc>, time: &str) -> String {
    let local = at.with_timezone(&Local);
    if local.date_naive() == now.with_timezone(&Local).date_naive() {
        local.format(time).to_string()
    } else {
        local.format(&format!("%b %-d {time}")).to_string()
    }
}

fn duration(span: chrono::TimeDelta) -> String {
    let seconds = span.num_seconds();
    match seconds {
        ..60 => format!("{seconds}s"),
        60..3600 => format!("{}m", seconds / 60),
        3600..86_400 => format!("{}h{}m", seconds / 3600, seconds % 3600 / 60),
        _ => format!("{}d{}h", seconds / 86_400, seconds % 86_400 / 3600),
    }
}

fn pid_alive(pid: u32) -> bool {
    let Ok(pid) = i32::try_from(pid) else {
        return false;
    };
    // SAFETY: signal 0 only checks that the process exists; nothing is delivered.
    let found = unsafe { libc::kill(pid, 0) } == 0;
    found || std::io::Error::last_os_error().raw_os_error() == Some(libc::EPERM)
}

/// runningboardd's record of `pid`'s end within [start, end], from the unified log.
fn system_record(pid: u32, start: DateTime<Utc>, end: DateTime<Utc>) -> SystemRecord {
    // zsh's builtin `log` shadows this; the path is explicit.
    let binary = Path::new("/usr/bin/log");
    if !binary.exists() {
        return SystemRecord::Unavailable;
    }
    let format = "%Y-%m-%d %H:%M:%S+0000";
    let mut command = Command::new(binary);
    command
        .args(["show", "--style", "compact", "--start"])
        .arg(start.format(format).to_string())
        .arg("--end")
        .arg(end.format(format).to_string())
        .arg("--predicate")
        .arg(format!(
            "process == \"runningboardd\" AND eventMessage CONTAINS \":{pid}] termination reported by launchd\""
        ));
    match crate::os_process::bounded_output(&mut command, SEARCH_TIMEOUT) {
        Ok(output) if output.status.success() => {
            parse_termination(&String::from_utf8_lossy(&output.stdout))
                .map_or(SystemRecord::Missing, SystemRecord::Found)
        }
        _ => SystemRecord::Unavailable,
    }
}

fn parse_termination(output: &str) -> Option<Termination> {
    const MARKER: &str = "termination reported by launchd (";
    let start = output.rfind(MARKER)? + MARKER.len();
    let inner = output[start..].split(')').next()?;
    let mut numbers = inner.split(',').map(|part| part.trim().parse::<i64>());
    let (Some(Ok(namespace)), Some(Ok(code)), Some(Ok(status)), None) = (
        numbers.next(),
        numbers.next(),
        numbers.next(),
        numbers.next(),
    ) else {
        return None;
    };
    Some(Termination {
        namespace,
        code,
        status,
    })
}

#[cfg(test)]
mod tests {
    use std::cell::Cell;

    use chrono::{DateTime, TimeDelta, Utc};

    use super::{minute, parse_termination, report, SystemRecord, Termination};
    use crate::lf::commands::doctor::Status;

    fn now() -> DateTime<Utc> {
        "2026-10-29T16:30:00Z".parse().unwrap()
    }

    fn launch(id: &str, at: &str, pid: u32, bundle: &str) -> String {
        format!(
            r#"{{"app":"0.15.1","at":"{at}","bundle":"{bundle}","event":"launch","launch":"{id}","ms":12,"os":"26.0.1","pid":{pid},"rev":"b11130060"}}"#
        )
    }

    fn exit(id: &str, at: &str, fields: &str) -> String {
        format!(r#"{{"app":"0.15.1","at":"{at}","event":"exit","launch":"{id}",{fields}}}"#)
    }

    const MAC: &str = "com.loopflow.mac";

    fn never(_: u32, _: DateTime<Utc>, _: DateTime<Utc>) -> SystemRecord {
        panic!("no system record lookup expected")
    }

    #[test]
    fn a_quit_is_ok_and_names_who_asked() {
        let journal = [
            launch("A", "2026-10-29T15:00:00Z", 8811, MAC),
            exit(
                "A",
                "2026-10-29T15:20:00Z",
                r#""reason":"quit","sender":"Dock","sender_pid":"612""#,
            ),
        ]
        .join("\n");
        let check = report(&journal, "", now(), |_| false, never);
        assert_eq!(check.status, Status::Ok);
        assert!(check.detail.contains("Loopflow 0.15.1 (pid 8811) quit "));
        assert!(check
            .detail
            .contains(": Quit requested by Dock (pid 612), 20m after launch"));
    }

    #[test]
    fn a_recorded_exit_warns_with_its_caller_and_the_system_status() {
        let journal = [
            launch("A", "2026-10-28T23:47:00Z", 52901, MAC),
            exit(
                "A",
                "2026-10-29T14:27:25Z",
                r#""reason":"exit","caller":"LoopflowMac 0x1 ghostty_app_tick + 12""#,
            ),
        ]
        .join("\n");
        let check = report(
            &journal,
            "",
            now(),
            |_| false,
            |pid, start, end| {
                assert_eq!(pid, 52901);
                assert!(end - start <= TimeDelta::minutes(2));
                SystemRecord::Found(Termination {
                    namespace: 0,
                    code: 0,
                    status: 256,
                })
            },
        );
        assert_eq!(check.status, Status::Warn);
        assert!(check.detail.contains("called exit() "));
        assert!(check.detail.contains(
            "from LoopflowMac 0x1 ghostty_app_tick + 12; exited with status 1, 14h40m after launch"
        ));
    }

    #[test]
    fn an_unrecorded_end_asks_the_system_log_within_the_heartbeat_window() {
        let journal = [
            launch("A", "2026-10-29T15:00:00Z", 9446, MAC),
            launch("B", "2026-10-29T16:20:00Z", 9500, MAC),
        ]
        .join("\n");
        let log = "2026-10-29T16:10:00.123Z  INFO loopflow::desktop: vitals footprint_mb=412 peak_mb=530 reads=61 failed=0 killed=0 top=task:40 pid=9446 desktop=0.15.1 rev=b11130060\n\
                   2026-10-29T16:12:00.000Z  WARN loopflow::journal: unrelated pid=9446 lf=0.15.1 rev=b11130060";
        let searched = Cell::new(None);
        let killed = report(
            &journal,
            log,
            now(),
            |pid| pid == 9500,
            |_, start, end| {
                searched.set(Some((start, end)));
                SystemRecord::Found(Termination {
                    namespace: 2,
                    code: 9,
                    status: 9,
                })
            },
        );
        assert_eq!(killed.status, Status::Warn);
        assert!(killed.detail.contains(
            "Loopflow 0.15.1 (pid 9446) was killed by SIGKILL (Force Quit, kill -9, or the system)"
        ));
        let vitals = minute("2026-10-29T16:10:00Z".parse().unwrap(), now());
        assert!(killed.detail.ends_with(&format!(
            ", 1h16m after launch; last vitals {vitals}: 412 MB, 61 lf reads/5 min"
        )));
        let (start, end) = searched.get().unwrap();
        assert_eq!(
            start,
            "2026-10-29T16:10:00.123Z".parse::<DateTime<Utc>>().unwrap()
        );
        assert_eq!(
            end,
            "2026-10-29T16:16:00.123Z".parse::<DateTime<Utc>>().unwrap()
        );

        let status = report(
            &journal,
            log,
            now(),
            |pid| pid == 9500,
            |_, _, _| {
                SystemRecord::Found(Termination {
                    namespace: 0,
                    code: 0,
                    status: 256,
                })
            },
        );
        assert!(status
            .detail
            .contains("(pid 9446) exited with status 1 before "));

        let missing = report(
            &journal,
            log,
            now(),
            |pid| pid == 9500,
            |_, _, _| SystemRecord::Missing,
        );
        assert!(missing
            .detail
            .contains(" (SIGKILL before the first heartbeat"));
    }

    #[test]
    fn a_storm_heartbeat_shows_its_killed_reads() {
        let journal = [
            launch("A", "2026-10-29T15:00:00Z", 9446, MAC),
            exit(
                "A",
                "2026-10-29T15:40:00Z",
                r#""reason":"exit","caller":"LoopflowMac 0x1 main + 4""#,
            ),
        ]
        .join("\n");
        let log = "2026-10-29T15:35:00Z  WARN loopflow::desktop: lf reads cancelled at storm rate footprint_mb=980 peak_mb=1100 reads=4210 failed=3 killed=4100 top=task:4000 pid=9446 desktop=0.15.1\n\
                   2026-10-29T15:38:00Z  WARN loopflow::desktop: main thread hung ms=2400 pid=9446 desktop=0.15.1";
        let check = report(
            &journal,
            log,
            now(),
            |_| false,
            |_, _, _| SystemRecord::Missing,
        );
        assert_eq!(check.status, Status::Warn);
        let vitals = minute("2026-10-29T15:35:00Z".parse().unwrap(), now());
        assert!(check.detail.ends_with(&format!(
            ", 40m after launch; last vitals {vitals}: 980 MB, 4210 lf reads/5 min, 4100 killed"
        )));
    }

    fn found(namespace: i64, code: i64, status: i64) -> SystemRecord {
        SystemRecord::Found(Termination {
            namespace,
            code,
            status,
        })
    }

    #[test]
    fn a_night_long_gap_searches_one_heartbeat_past_the_last_line() {
        let journal = [
            launch("A", "2026-10-28T08:00:00Z", 9446, MAC),
            launch("B", "2026-10-29T08:00:00Z", 9500, MAC),
        ]
        .join("\n");
        let log = "2026-10-28T09:10:00Z  INFO loopflow::desktop: vitals pid=9446 desktop=0.15.1";
        let searched = Cell::new(None);
        let check = report(
            &journal,
            log,
            now(),
            |pid| pid == 9500,
            |_, start, end| {
                searched.set(Some((start, end)));
                found(2, 9, 9)
            },
        );
        assert_eq!(check.status, Status::Warn);
        assert!(check.detail.contains("(pid 9446) was killed by SIGKILL"));
        let (start, end) = searched.get().unwrap();
        assert_eq!(
            start,
            "2026-10-28T09:10:00Z".parse::<DateTime<Utc>>().unwrap()
        );
        assert_eq!(
            end,
            "2026-10-28T09:16:00Z".parse::<DateTime<Utc>>().unwrap()
        );
    }

    #[test]
    fn a_quit_then_a_force_quit_lists_both() {
        let journal = [
            launch("A", "2026-10-29T14:00:00Z", 100, MAC),
            exit(
                "A",
                "2026-10-29T14:30:00Z",
                r#""reason":"quit","sender":"Dock""#,
            ),
            launch("B", "2026-10-29T14:31:00Z", 200, MAC),
            launch("C", "2026-10-29T15:00:00Z", 300, MAC),
        ]
        .join("\n");
        let check = report(
            &journal,
            "",
            now(),
            |pid| pid == 300,
            |_, _, _| found(2, 9, 9),
        );
        assert_eq!(check.status, Status::Warn);
        let lines: Vec<&str> = check.detail.lines().collect();
        assert_eq!(lines.len(), 3);
        assert!(lines[0].contains("(pid 300) running since "));
        assert!(lines[1].contains("(pid 200) was killed by SIGKILL"));
        assert!(lines[2].contains("(pid 100) quit "));
    }

    #[test]
    fn two_unrecorded_exits_list_both() {
        let journal = [
            launch("A", "2026-10-29T14:00:00Z", 100, MAC),
            launch("B", "2026-10-29T15:00:00Z", 200, MAC),
        ]
        .join("\n");
        let check = report(&journal, "", now(), |_| false, |_, _, _| found(0, 0, 256));
        assert_eq!(check.status, Status::Warn);
        let lines: Vec<&str> = check.detail.lines().collect();
        assert_eq!(lines.len(), 2);
        assert!(lines[0].contains("(pid 200) exited with status 1 before "));
        assert!(lines[1].contains("(pid 100) exited with status 1 before "));
    }

    #[test]
    fn more_than_three_abnormal_ends_name_the_newest_three() {
        let journal = (1..=4)
            .map(|hour| {
                launch(
                    &hour.to_string(),
                    &format!("2026-10-29T1{hour}:00:00Z"),
                    hour,
                    MAC,
                )
            })
            .collect::<Vec<_>>()
            .join("\n");
        let check = report(&journal, "", now(), |_| false, |_, _, _| found(2, 9, 9));
        assert_eq!(check.status, Status::Warn);
        let lines: Vec<&str> = check.detail.lines().collect();
        assert_eq!(lines.len(), 4);
        assert!(lines[0].contains("(pid 4) was killed"));
        assert!(lines[2].contains("(pid 2) was killed"));
        assert!(lines[3].ends_with("… and 1 earlier"));
    }

    #[test]
    fn dev_and_release_bundles_report_separately() {
        let journal = [
            launch("A", "2026-10-29T15:00:00Z", 100, MAC),
            launch("D", "2026-10-29T15:05:00Z", 200, "com.loopflow.mac.dev"),
            exit(
                "D",
                "2026-10-29T15:06:00Z",
                r#""reason":"signal","signal":"SIGTERM""#,
            ),
            "{torn".to_string(),
        ]
        .join("\n");
        let check = report(&journal, "", now(), |pid| pid == 100, never);
        assert_eq!(check.status, Status::Warn);
        let lines: Vec<&str> = check.detail.lines().collect();
        assert_eq!(lines.len(), 2);
        assert!(lines[0].contains("Loopflow 0.15.1 (pid 100) running since "));
        assert!(lines[1].contains("Loopflow Dev 0.15.1 (pid 200) was killed "));
        assert!(lines[1].contains(" by SIGTERM, 1m after launch"));
    }

    #[test]
    fn older_journals_report_without_a_lookup_until_bundles_appear() {
        let old = [
            r#"{"app":"0.15.0","at":"2026-10-28T23:47:00Z","event":"launch","launch":"O1","ms":12}"#,
            r#"{"app":"0.15.0","at":"2026-10-29T14:28:12Z","event":"launch","launch":"O2","ms":12}"#,
        ]
        .join("\n");
        let check = report(&old, "", now(), |_| true, never);
        assert_eq!(check.status, Status::Warn);
        assert!(check
            .detail
            .contains("Loopflow 0.15.0 ended without a record before "));

        let upgraded = format!("{old}\n{}", launch("N", "2026-10-29T16:00:00Z", 300, MAC));
        let check = report(&upgraded, "", now(), |_| true, never);
        assert_eq!(check.status, Status::Ok);
        assert!(check
            .detail
            .starts_with("Loopflow 0.15.1 (pid 300) running since "));
    }

    #[test]
    fn no_journal_is_ok() {
        let check = report("", "", now(), |_| true, never);
        assert_eq!(check.status, Status::Ok);
        assert_eq!(check.detail, "no Desktop launches recorded");
    }

    #[test]
    fn the_system_log_line_parses_its_termination() {
        let line = "2026-10-29 07:27:25.606 Df runningboardd[401:bc6806b] [com.apple.runningboard:ttl] [app<application.com.loopflow.mac.354611862.354611868(501)>:52901] termination reported by launchd (0, 0, 256)";
        assert_eq!(
            parse_termination(line),
            Some(Termination {
                namespace: 0,
                code: 0,
                status: 256
            })
        );
        assert_eq!(parse_termination("Timestamp  Ty Process[PID:TID]"), None);
    }
}
