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
use std::path::Path;

use anyhow::{anyhow, Result};
use time::{Duration, OffsetDateTime};

use crate::journal::open_ledger;
use crate::lf::output::Colors;
use crate::lfdb::RunEventRow;

/// A node value the current binary understands. `step` is the pre-054 spelling
/// of `skill`; rows carrying it are history the readers silently drop.
const NODES: [&str; 3] = ["run", "flow", "skill"];
const EVENTS: [&str; 4] = ["started", "completed", "errored", "escalated"];

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "snake_case")]
#[non_exhaustive]
pub enum Status {
    Ok,
    Warn,
    Fail,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
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
#[derive(Debug, serde::Serialize)]
struct DoctorReport<'a> {
    rows: usize,
    checks: &'a [Check],
}

pub fn run(json: bool) -> Result<()> {
    let store = open_ledger()?;
    let events = store.list_run_events_since(0)?;
    let mut checks = audit(&events);
    checks.push(check_capture(&store, &events)?);
    if json {
        println!(
            "{}",
            serde_json::to_string(&DoctorReport {
                rows: events.len(),
                checks: &checks,
            })?
        );
    } else {
        print_checks(&checks, events.len());
    }

    if checks.iter().any(|check| check.status == Status::Fail) {
        return Err(anyhow!("run ledger audit failed"));
    }
    Ok(())
}

fn check_capture(
    store: &crate::lfdb::sqlite::SqliteStore,
    events: &[RunEventRow],
) -> Result<Check> {
    let required_after = store.trace_capture_required_after()?;
    let launches = store.agent_launches_since(required_after)?;
    let launch_ids = launches
        .iter()
        .map(|launch| launch.id.clone())
        .collect::<Vec<_>>();
    let turns = store.agent_turns_for_launches(&launch_ids)?;
    let turn_ids = turns.iter().map(|turn| turn.id.clone()).collect::<Vec<_>>();
    let assets = store.context_assets_for_turns(&turn_ids)?;

    let launch_processes: HashSet<&str> = launches
        .iter()
        .map(|launch| launch.process_id.as_str())
        .collect();
    let missing_launches: HashSet<&str> = events
        .iter()
        .filter(|event| event.ts >= required_after && event.provider.is_some())
        .map(|event| event.process_id.as_str())
        .filter(|process| !launch_processes.contains(process))
        .collect();

    let mut failures = Vec::new();
    let mut prompt_only = 0;
    for launch in &launches {
        if crate::trace::resolve_artifact(&launch.artifact_dir).is_err()
            || crate::trace::resolve_artifact(&launch.conversation_path).is_err()
        {
            failures.push(format!("{} has an unsafe artifact path", launch.id));
        }
        if launch.capture_status == "prompt_only" {
            prompt_only += 1;
        }
        if launch.capture_status == "partial" {
            failures.push(format!("{} is partial", launch.id));
        }
        if launch.capture_status == "capturing"
            && events.iter().any(|event| {
                event.process_id == launch.process_id
                    && event.node == "run"
                    && event.event != "started"
            })
        {
            failures.push(format!(
                "{} stayed capturing after its process ended",
                launch.id
            ));
        }
        if launch.capture_status == "complete" {
            match crate::trace::resolve_artifact(&launch.conversation_path)
                .and_then(|path| crate::trace::read_conversation(&path))
            {
                Ok(records) => {
                    if records
                        .windows(2)
                        .any(|pair| pair[1].seq != pair[0].seq + 1)
                    {
                        failures.push(format!("{} has non-monotonic events", launch.id));
                    }
                }
                Err(error) => failures.push(format!("{}: {error}", launch.id)),
            }
            if !turns.iter().any(|turn| {
                turn.launch_id == launch.id
                    && matches!(turn.status.as_str(), "completed" | "failed" | "interrupted")
            }) {
                failures.push(format!("{} has no terminal turn", launch.id));
            }
        }
    }

    let mut assets_by_turn: HashMap<&str, Vec<&crate::trace::ContextAssetRow>> = HashMap::new();
    for asset in &assets {
        assets_by_turn
            .entry(asset.turn_id.as_str())
            .or_default()
            .push(asset);
    }
    for turn in turns
        .iter()
        .filter(|turn| turn.context_coverage == "assembled")
    {
        if !crate::trace::resolve_artifact(&turn.task_prompt_path)
            .is_ok_and(|path| path.is_file())
            || turn
                .system_prompt_path
                .as_deref()
                .is_some_and(|path| {
                    !crate::trace::resolve_artifact(path).is_ok_and(|path| path.is_file())
                })
        {
            failures.push(format!("{} is missing a prompt artifact", turn.id));
        }
        let Some(turn_assets) = assets_by_turn.get(turn.id.as_str()) else {
            failures.push(format!("{} has no context assets", turn.id));
            continue;
        };
        let system: u64 = turn_assets
            .iter()
            .filter(|row| row.asset.channel == crate::trace::ContextChannel::System)
            .map(|row| row.asset.attributed_tokens)
            .sum();
        let task: u64 = turn_assets
            .iter()
            .filter(|row| row.asset.channel == crate::trace::ContextChannel::Task)
            .map(|row| row.asset.attributed_tokens)
            .sum();
        if system as i64 != turn.system_tokens || task as i64 != turn.task_tokens {
            failures.push(format!("{} has mismatched asset tokens", turn.id));
        }
    }

    for process_id in launch_processes {
        let terminal = events
            .iter()
            .filter(|event| event.process_id == process_id)
            .filter(|event| event.node == "run" && event.event != "started")
            .max_by_key(|event| event.seq);
        let Some(terminal) = terminal else {
            continue;
        };
        let process_launches: HashSet<&str> = launches
            .iter()
            .filter(|launch| launch.process_id == process_id)
            .map(|launch| launch.id.as_str())
            .collect();
        let process_turns = turns
            .iter()
            .filter(|turn| process_launches.contains(turn.launch_id.as_str()))
            .collect::<Vec<_>>();
        let input: i64 = process_turns
            .iter()
            .filter_map(|turn| turn.provider_input_tokens)
            .sum();
        let output: i64 = process_turns
            .iter()
            .filter_map(|turn| turn.provider_output_tokens)
            .sum();
        let cache: i64 = process_turns
            .iter()
            .filter_map(|turn| turn.cache_read_tokens)
            .sum();
        if terminal.input_tokens.is_some_and(|value| value != input)
            || terminal.output_tokens.is_some_and(|value| value != output)
            || terminal
                .cache_read_tokens
                .is_some_and(|value| value != cache)
        {
            failures.push(format!(
                "{process_id} turn usage disagrees with its terminal row"
            ));
        }
    }

    if !missing_launches.is_empty() {
        failures.push(format!(
            "{} agent-bearing process(es) have no launch",
            missing_launches.len()
        ));
    }
    if !failures.is_empty() {
        return Ok(Check::fail(
            "capture",
            format!(
                "{} failure(s); {} launches, {} turns, {} bytes: {}",
                failures.len(),
                launches.len(),
                turns.len(),
                launches
                    .iter()
                    .map(|launch| launch.conversation_bytes)
                    .sum::<i64>(),
                failures.into_iter().take(4).collect::<Vec<_>>().join("; ")
            ),
        ));
    }
    if prompt_only > 0 {
        return Ok(Check::warn(
            "capture",
            format!(
                "{} launches and {} turns are consistent; {prompt_only} interactive launch(es) are prompt-only",
                launches.len(),
                turns.len()
            ),
        ));
    }
    Ok(Check::ok(
        "capture",
        format!(
            "{} launches, {} turns, {} assets, {} bytes",
            launches.len(),
            turns.len(),
            assets.len(),
            launches
                .iter()
                .map(|launch| launch.conversation_bytes)
                .sum::<i64>()
        ),
    ))
}

pub fn audit(events: &[RunEventRow]) -> Vec<Check> {
    if events.is_empty() {
        return vec![Check::warn("continuity", "ledger is empty")];
    }
    let now = OffsetDateTime::now_utc().unix_timestamp();
    vec![
        check_continuity(events, now),
        check_vocabulary(events),
        check_attribution(events),
        check_identity(events),
        check_lineage(events),
        check_coverage(events),
    ]
}

/// Silence longer than this is reported. The 29.2-hour outage is the reason
/// the number exists; a day-granularity check missed it entirely, because the
/// silence began mid-day and ended mid-day and both days held rows.
const MAX_SILENCE_HOURS: f64 = 24.0;

/// A day the ledger recorded nothing is a day it may not have been listening —
/// but so is a long silence inside two busy days. Measure both.
fn check_continuity(events: &[RunEventRow], now: i64) -> Check {
    let days: BTreeSet<_> = events.iter().filter_map(|e| day_of(e.ts)).collect();
    let (Some(first), Some(last_event_day)) = (days.first(), days.last()) else {
        return Check::warn("continuity", "no timestamps");
    };
    let last = day_of(now)
        .map(|today| today.max(*last_event_day))
        .unwrap_or(*last_event_day);

    let mut gaps = Vec::new();
    let mut cursor = *first;
    while cursor < last {
        cursor += Duration::days(1);
        if cursor < last && !days.contains(&cursor) {
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

    let silence = longest_silence_hours(events, now);
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

/// The largest interval between consecutive recorded events or between the
/// latest event and now, in hours. The tail matters most during an active
/// outage: until a later write succeeds, there is no second event to expose it.
fn longest_silence_hours(events: &[RunEventRow], now: i64) -> f64 {
    let mut stamps: Vec<i64> = events.iter().map(|event| event.ts).collect();
    stamps.push(now);
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

/// A process may name only one command, and its terminal row names that work.
fn check_attribution(events: &[RunEventRow]) -> Check {
    let mut commands: HashMap<&str, HashSet<&str>> = HashMap::new();
    let mut terminal = 0usize;
    let mut terminal_unnamed = 0usize;

    for event in events {
        if let Some(command) = event.command.as_deref() {
            commands
                .entry(&event.process_id)
                .or_default()
                .insert(command);
        }
        if event.node == "run" && event.event != "started" {
            terminal += 1;
            if event.command.is_none() && event.flow.is_none() && event.skill.is_none() {
                terminal_unnamed += 1;
            }
        }
    }

    let ambiguous = commands.values().filter(|set| set.len() > 1).count();
    if ambiguous == 0 && terminal_unnamed == 0 {
        return Check::ok("attribution", "every terminal row names its work");
    }
    Check::fail(
        "attribution",
        format!(
            "{ambiguous} process_id(s) carry >1 command; {terminal_unnamed}/{terminal} terminal rows name no command, flow, or skill"
        ),
    )
}

/// Repo identity is the absolute main-repo root, never a basename.
fn check_identity(events: &[RunEventRow]) -> Check {
    let repos: HashSet<Option<&str>> = events.iter().map(|event| event.repo.as_deref()).collect();
    let invalid = repos
        .iter()
        .filter(|repo| repo.is_none_or(|repo| !Path::new(repo).is_absolute()))
        .count();
    if invalid == 0 {
        return Check::ok(
            "identity",
            format!("{} repo value(s), all absolute", repos.len()),
        );
    }
    Check::fail(
        "identity",
        format!(
            "{invalid}/{} repo value(s) are missing or not absolute",
            repos.len()
        ),
    )
}

fn check_lineage(events: &[RunEventRow]) -> Check {
    let processes: HashMap<&str, &str> = events
        .iter()
        .map(|event| (event.process_id.as_str(), event.run_id.as_str()))
        .collect();
    let dangling: HashSet<&str> = events
        .iter()
        .filter_map(|event| {
            let parent = event.parent_process_id.as_deref()?;
            (processes.get(parent).copied() != Some(event.run_id.as_str())).then_some(parent)
        })
        .collect();
    if dangling.is_empty() {
        return Check::ok("lineage", "every parent process resolves");
    }
    Check::fail(
        "lineage",
        format!(
            "{} parent process id(s) are missing or belong to another trace",
            dangling.len()
        ),
    )
}

/// A run that launched an agent and recorded no tokens is a run whose cost is
/// lost. Skill rows prove an agent ran; the terminal row should carry the spend.
fn check_coverage(events: &[RunEventRow]) -> Check {
    let agent_processes: HashSet<&str> = events
        .iter()
        .filter(|event| event.node == "skill" || event.provider.is_some())
        .map(|event| event.process_id.as_str())
        .collect();
    if agent_processes.is_empty() {
        return Check::ok("coverage", "no agent-bearing runs recorded");
    }

    let with_tokens: HashSet<&str> = events
        .iter()
        .filter(|e| e.node == "run" && e.event != "started" && e.input_tokens.is_some())
        .map(|event| event.process_id.as_str())
        .collect();
    let covered = agent_processes.intersection(&with_tokens).count();
    let total = agent_processes.len();
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
    use super::{audit, check_continuity, Status};
    use crate::lfdb::RunEventRow;

    const DAY: i64 = 86_400;

    fn row(run_id: &str, ts: i64, node: &str, event: &str) -> RunEventRow {
        RunEventRow {
            run_id: run_id.to_string(),
            process_id: run_id.to_string(),
            parent_process_id: None,
            seq: 0,
            ts,
            repo: Some("/src/loopflow".to_string()),
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
            model: None,
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
        let check = check_continuity(&rows, DAY * 3);
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
        let check = check_continuity(&rows, DAY * 2 + 79_200);
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
        assert_eq!(check_continuity(&rows, DAY * 2).status, Status::Ok);
    }

    #[test]
    fn an_active_silence_after_the_last_event_is_caught() {
        let rows = [
            row("a", DAY, "run", "completed"),
            row("b", DAY + 3600, "run", "completed"),
        ];

        let check = check_continuity(&rows, DAY + 26 * 3600);
        assert_eq!(check.status, Status::Warn, "{}", check.detail);
        assert!(
            check.detail.contains("25.0h of silence"),
            "{}",
            check.detail
        );
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
    fn one_process_carrying_two_commands_is_unattributable() {
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
        assert!(check.detail.contains("1 process_id"), "{}", check.detail);
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
    fn two_processes_in_one_trace_are_attributable() {
        let mut parent = named(row("shared", DAY, "run", "completed"), r#"["lf","wave"]"#);
        parent.process_id = "parent".to_string();
        let mut child = named(row("shared", DAY, "run", "completed"), r#"["lf","pm"]"#);
        child.process_id = "child".to_string();
        child.parent_process_id = Some("parent".to_string());
        assert_eq!(status_of(&[parent, child], "attribution"), Status::Ok);
    }

    #[test]
    fn a_repo_basename_fails_identity() {
        let mut event = row("a", DAY, "run", "completed");
        event.repo = Some("loopflow".to_string());
        assert_eq!(status_of(&[event], "identity"), Status::Fail);
    }

    #[test]
    fn a_dangling_parent_process_id_fails_the_doctor() {
        let mut event = row("a", DAY, "run", "completed");
        event.parent_process_id = Some("missing".to_string());
        assert_eq!(status_of(&[event], "lineage"), Status::Fail);
    }

    #[test]
    fn a_parent_from_another_trace_fails_lineage() {
        let mut parent = row("trace-a", DAY, "run", "completed");
        parent.process_id = "parent".to_string();
        let mut child = row("trace-b", DAY, "run", "completed");
        child.process_id = "child".to_string();
        child.parent_process_id = Some("parent".to_string());
        assert_eq!(status_of(&[parent, child], "lineage"), Status::Fail);
    }

    #[test]
    fn an_agent_run_without_tokens_is_a_coverage_warning() {
        let rows = [
            row("a", DAY, "skill", "completed"),
            row("a", DAY, "run", "completed"),
        ];
        assert_eq!(status_of(&rows, "coverage"), Status::Warn);
    }

    #[test]
    fn an_inline_agent_with_provider_and_tokens_is_covered() {
        let mut terminal = row("a", DAY, "run", "completed");
        terminal.provider = Some("claude".to_string());
        terminal.input_tokens = Some(100);
        assert_eq!(status_of(&[terminal], "coverage"), Status::Ok);
    }
}
