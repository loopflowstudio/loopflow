//! `lf doctor` — the ledger reports on itself.
//!
//! Every wave question is a query against the run ledger, so a ledger that is
//! wrong, deaf, or ambiguous makes every downstream answer confidently wrong.
//! These checks exist because each one failed silently at least once: a schema
//! drift dropped 29 hours of writes while `debug!` swallowed the error, a
//! column rename left `node='step'` and `node='skill'` meaning the same thing,
//! and `lf runs` still splices one process's label onto another's cost.
//!
//! Checks are pure functions of the rows, so they are tested without a store.

use std::collections::{BTreeSet, HashMap, HashSet};

use anyhow::Result;
use time::{Duration, OffsetDateTime};

use crate::journal::open_ledger;
use crate::lf::output::Colors;
use crate::lfdb::RunEventRow;

/// A node value the current binary understands. `step` is the pre-054 spelling
/// of `skill`; rows carrying it are history the readers silently drop.
const NODES: [&str; 3] = ["run", "flow", "skill"];
const EVENTS: [&str; 4] = ["started", "completed", "errored", "escalated"];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Status {
    Ok,
    Warn,
    Fail,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Check {
    pub name: &'static str,
    pub status: Status,
    pub detail: String,
}

impl Check {
    fn ok(name: &'static str, detail: impl Into<String>) -> Self {
        Self {
            name,
            status: Status::Ok,
            detail: detail.into(),
        }
    }
    fn warn(name: &'static str, detail: impl Into<String>) -> Self {
        Self {
            name,
            status: Status::Warn,
            detail: detail.into(),
        }
    }
    fn fail(name: &'static str, detail: impl Into<String>) -> Self {
        Self {
            name,
            status: Status::Fail,
            detail: detail.into(),
        }
    }
}

/// Exits non-zero when any check fails, so a cron can gate on it.
pub fn run() -> Result<()> {
    let events = open_ledger()?.list_run_events_since(0)?;
    let checks = audit(&events);
    print_checks(&checks, events.len());

    if checks.iter().any(|check| check.status == Status::Fail) {
        std::process::exit(1);
    }
    Ok(())
}

pub fn audit(events: &[RunEventRow]) -> Vec<Check> {
    if events.is_empty() {
        return vec![Check::warn("continuity", "ledger is empty")];
    }
    vec![
        check_continuity(events),
        check_vocabulary(events),
        check_attribution(events),
        check_identity(events),
        check_coverage(events),
    ]
}

/// Silence longer than this is reported. The 29.2-hour outage is the reason
/// the number exists; a day-granularity check missed it entirely, because the
/// silence began mid-day and ended mid-day and both days held rows.
const MAX_SILENCE_HOURS: f64 = 24.0;

/// A day the ledger recorded nothing is a day it may not have been listening —
/// but so is a long silence inside two busy days. Measure both.
fn check_continuity(events: &[RunEventRow]) -> Check {
    let days: BTreeSet<_> = events.iter().filter_map(|e| day_of(e.ts)).collect();
    let (Some(first), Some(last)) = (days.first(), days.last()) else {
        return Check::warn("continuity", "no timestamps");
    };

    let mut gaps = Vec::new();
    let mut cursor = *first;
    while cursor < *last {
        cursor += Duration::days(1);
        if cursor < *last && !days.contains(&cursor) {
            gaps.push(cursor.to_string());
        }
    }

    let span = format!("{first} → {last}");
    if !gaps.is_empty() {
        return Check::fail(
            "continuity",
            format!("{} gap-day(s) in {span}: {}", gaps.len(), gaps.join(", ")),
        );
    }

    let silence = longest_silence_hours(events);
    if silence > MAX_SILENCE_HOURS {
        return Check::warn(
            "continuity",
            format!(
                "no gap-days ({span}), but {silence:.1}h of silence — was the ledger listening?"
            ),
        );
    }
    Check::ok(
        "continuity",
        format!("no gap-days ({span}); longest silence {silence:.1}h"),
    )
}

/// The largest interval between consecutive recorded events, in hours.
fn longest_silence_hours(events: &[RunEventRow]) -> f64 {
    let mut stamps: Vec<i64> = events.iter().map(|event| event.ts).collect();
    stamps.sort_unstable();
    stamps
        .windows(2)
        .map(|pair| pair[1] - pair[0])
        .max()
        .unwrap_or(0) as f64
        / 3600.0
}

/// A half-landed rename leaves two spellings of one concept, and every query
/// grouping on it silently drops history.
fn check_vocabulary(events: &[RunEventRow]) -> Check {
    let mut unknown: HashMap<String, usize> = HashMap::new();
    for event in events {
        if !NODES.contains(&event.node.as_str()) {
            *unknown.entry(format!("node={}", event.node)).or_default() += 1;
        }
        if !EVENTS.contains(&event.event.as_str()) {
            *unknown.entry(format!("event={}", event.event)).or_default() += 1;
        }
    }
    if unknown.is_empty() {
        return Check::ok("vocabulary", "node and event values are all known");
    }
    let mut parts: Vec<_> = unknown
        .into_iter()
        .map(|(value, count)| format!("{value} ({count} rows)"))
        .collect();
    parts.sort();
    Check::fail(
        "vocabulary",
        format!("values outside the closed set: {}", parts.join(", ")),
    )
}

/// `run_id` names a tree of processes: a nested `lf` inherits `LF_RUN_ID`. When
/// one id carries several commands, no reader can say which process spent the
/// tokens on a terminal row that names no command at all.
fn check_attribution(events: &[RunEventRow]) -> Check {
    let mut commands: HashMap<&str, HashSet<&str>> = HashMap::new();
    let mut terminal = 0usize;
    let mut terminal_unnamed = 0usize;

    for event in events {
        if let Some(command) = event.command.as_deref() {
            commands.entry(&event.run_id).or_default().insert(command);
        }
        if event.node == "run" && event.event != "started" {
            terminal += 1;
            if event.command.is_none() && event.flow.is_none() && event.skill.is_none() {
                terminal_unnamed += 1;
            }
        }
    }

    let shared = commands.values().filter(|set| set.len() > 1).count();
    if shared == 0 && terminal_unnamed == 0 {
        return Check::ok("attribution", "every terminal row names its work");
    }
    Check::fail(
        "attribution",
        format!(
            "{shared} run_id(s) carry >1 command; {terminal_unnamed}/{terminal} terminal rows name no command, flow, or skill"
        ),
    )
}

/// `repo` is a basename, so temp git roots mint a fresh "identity" each run.
fn check_identity(events: &[RunEventRow]) -> Check {
    let repos: HashSet<&str> = events.iter().filter_map(|e| e.repo.as_deref()).collect();
    let junk = repos
        .iter()
        .filter(|repo| looks_like_temp_root(repo))
        .count();
    if junk == 0 {
        return Check::ok(
            "identity",
            format!("{} repo value(s), all stable", repos.len()),
        );
    }
    Check::warn(
        "identity",
        format!(
            "{junk}/{} repo value(s) look like temp roots — `repo` is a basename, not an identity",
            repos.len()
        ),
    )
}

fn looks_like_temp_root(repo: &str) -> bool {
    let trimmed = repo.trim_start_matches('.');
    trimmed.starts_with("tmp") || trimmed.starts_with("temp")
}

/// A run that launched an agent and recorded no tokens is a run whose cost is
/// lost. Skill rows prove an agent ran; the terminal row should carry the spend.
fn check_coverage(events: &[RunEventRow]) -> Check {
    let agent_runs: HashSet<&str> = events
        .iter()
        .filter(|e| e.node == "skill" || e.node == "step")
        .map(|e| e.run_id.as_str())
        .collect();
    if agent_runs.is_empty() {
        return Check::ok("coverage", "no agent-bearing runs recorded");
    }

    let with_tokens: HashSet<&str> = events
        .iter()
        .filter(|e| e.node == "run" && e.event != "started" && e.input_tokens.is_some())
        .map(|e| e.run_id.as_str())
        .collect();
    let covered = agent_runs.intersection(&with_tokens).count();
    let total = agent_runs.len();
    let pct = (covered as f64 / total as f64) * 100.0;

    let detail = format!("{covered}/{total} agent-bearing runs carry tokens ({pct:.0}%)");
    if covered == total {
        Check::ok("coverage", detail)
    } else {
        Check::warn("coverage", detail)
    }
}

fn day_of(ts: i64) -> Option<time::Date> {
    OffsetDateTime::from_unix_timestamp(ts)
        .ok()
        .map(|dt| dt.date())
}

fn print_checks(checks: &[Check], rows: usize) {
    let colors = Colors::default();
    println!("ledger: {rows} run events\n");
    for check in checks {
        let (mark, color) = match check.status {
            Status::Ok => ("ok  ", colors.green),
            Status::Warn => ("warn", colors.yellow),
            Status::Fail => ("FAIL", colors.red),
        };
        println!(
            "{color}{mark}{reset}  {bold}{name:<13}{reset} {detail}",
            color = color,
            reset = colors.reset,
            bold = colors.bold,
            mark = mark,
            name = check.name,
            detail = check.detail,
        );
    }
}

#[cfg(test)]
mod tests {
    use super::{audit, check_continuity, looks_like_temp_root, Status};
    use crate::lfdb::RunEventRow;

    const DAY: i64 = 86_400;

    fn row(run_id: &str, ts: i64, node: &str, event: &str) -> RunEventRow {
        RunEventRow {
            run_id: run_id.to_string(),
            seq: 0,
            ts,
            repo: Some("loopflow".to_string()),
            worktree: None,
            wave: None,
            node: node.to_string(),
            event: event.to_string(),
            command: None,
            flow: None,
            skill: None,
            step_index: None,
            error: None,
            input_tokens: None,
            output_tokens: None,
            cache_read_tokens: None,
            cost_usd: None,
            duration_secs: None,
            provider: None,
        }
    }

    fn named(mut row: RunEventRow, command: &str) -> RunEventRow {
        row.command = Some(command.to_string());
        row
    }

    fn status_of(rows: &[RunEventRow], name: &str) -> Status {
        audit(rows)
            .into_iter()
            .find(|check| check.name == name)
            .expect("check exists")
            .status
    }

    #[test]
    fn a_missing_day_is_a_failure() {
        // The 29-hour outage looked exactly like this: writes, silence, writes.
        let rows = [
            row("a", DAY, "run", "completed"),
            row("b", DAY * 3, "run", "completed"),
        ];
        let check = check_continuity(&rows);
        assert_eq!(check.status, Status::Fail);
        assert!(check.detail.contains("1 gap-day"), "{}", check.detail);
    }

    #[test]
    fn a_long_silence_inside_two_busy_days_is_still_caught() {
        // The real 29.2h outage: rows early on day 1, rows late on day 2, no
        // missing day at all. A gap-day check calls this healthy. It is not.
        let rows = [
            row("a", DAY, "run", "completed"),              // day 1, 00:00
            row("b", DAY + 3600, "run", "completed"),       // day 1, 01:00
            row("c", DAY * 2 + 79_200, "run", "completed"), // day 2, 22:00
        ];
        let check = check_continuity(&rows);
        assert_eq!(check.status, Status::Warn, "{}", check.detail);
        assert!(check.detail.contains("silence"), "{}", check.detail);
    }

    #[test]
    fn consecutive_days_have_no_gap() {
        let rows = [
            row("a", DAY, "run", "completed"),
            row("b", DAY + 3600, "run", "completed"),
            row("c", DAY * 2, "run", "completed"),
        ];
        assert_eq!(check_continuity(&rows).status, Status::Ok);
    }

    #[test]
    fn a_half_landed_rename_is_caught() {
        // `step` is the pre-054 spelling of `skill`. Both in one ledger means
        // every query grouping on node silently drops history.
        let rows = [
            row("a", DAY, "run", "completed"),
            row("a", DAY, "step", "completed"),
        ];
        assert_eq!(status_of(&rows, "vocabulary"), Status::Fail);
    }

    #[test]
    fn one_run_id_carrying_two_commands_is_unattributable() {
        let rows = [
            named(row("shared", DAY, "run", "started"), r#"["lf","wave"]"#),
            named(row("shared", DAY, "run", "started"), r#"["lf","op","pm"]"#),
            row("shared", DAY, "run", "completed"),
        ];
        let check = audit(&rows)
            .into_iter()
            .find(|c| c.name == "attribution")
            .unwrap();
        assert_eq!(check.status, Status::Fail);
        assert!(check.detail.contains("1 run_id"), "{}", check.detail);
    }

    #[test]
    fn a_terminal_row_that_names_its_work_attributes_cleanly() {
        let mut terminal = row("a", DAY, "run", "completed");
        terminal.command = Some(r#"["lf","code"]"#.to_string());
        let rows = [
            named(row("a", DAY, "run", "started"), r#"["lf","code"]"#),
            terminal,
        ];
        assert_eq!(status_of(&rows, "attribution"), Status::Ok);
    }

    #[test]
    fn temp_roots_are_not_identities() {
        assert!(looks_like_temp_root("tmp.Rf5ZtVARiJ"));
        assert!(looks_like_temp_root(".tmpzt80hK"));
        assert!(!looks_like_temp_root("loopflow"));
        assert!(!looks_like_temp_root("manabot"));
    }

    #[test]
    fn an_agent_run_without_tokens_is_a_coverage_warning() {
        let rows = [
            row("a", DAY, "skill", "completed"),
            row("a", DAY, "run", "completed"),
        ];
        assert_eq!(status_of(&rows, "coverage"), Status::Warn);
    }
}
