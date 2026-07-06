//! `lf runs` and `lf trace` — read the local run ledger.
//!
//! Both commands are pure readers over the machine-grain SQLite ledger
//! (`run_events`, written by every `lf` invocation) plus the prompt logs on
//! disk. Local-only: nothing is fetched from anywhere.

use std::collections::BTreeMap;
use std::path::PathBuf;

use anyhow::{anyhow, Result};

use crate::journal::open_ledger;
use crate::lf::output::Colors;
use crate::lfdb::RunEventRow;
use crate::wave::journal::short_id;

const WINDOW_DAYS: i64 = 7;
const MAX_RUNS: usize = 50;

/// `lf runs`: timeline of recent runs across every repo on this machine.
/// `--json` emits the same window as a machine-readable array (Concerto's
/// run-history snapshot) — the durable ledger the live `op` frames mirror.
pub fn list(json: bool) -> Result<()> {
    let store = open_ledger().map_err(|err| anyhow!("run ledger unavailable: {err}"))?;
    let since = chrono::Utc::now().timestamp() - WINDOW_DAYS * 24 * 3600;
    let events = store
        .list_run_events_since(since)
        .map_err(|err| anyhow!("failed to read run ledger: {err}"))?;

    let mut summaries = summarize(&events);
    summaries.sort_by_key(|run| std::cmp::Reverse(run.started));
    summaries.truncate(MAX_RUNS);

    if json {
        let entries: Vec<RunLedgerEntry> = summaries.iter().map(RunLedgerEntry::from).collect();
        println!("{}", serde_json::to_string(&entries)?);
        return Ok(());
    }

    if summaries.is_empty() {
        println!("No runs recorded in the last {WINDOW_DAYS} days.");
        return Ok(());
    }

    let colors = Colors::default();
    println!(
        "{bold}{time:<12}  {repo:<14}  {wave:<10}  {label:<24}  {dur:>8}  {tokens:>10}  {status:<7}  ID{reset}",
        bold = colors.bold,
        reset = colors.reset,
        time = "TIME",
        repo = "REPO",
        wave = "WAVE",
        label = "RUN",
        dur = "DURATION",
        tokens = "TOKENS",
        status = "STATUS",
    );
    for run in &summaries {
        println!(
            "{time:<12}  {repo:<14}  {wave:<10}  {label:<24}  {dur:>8}  {tokens:>10}  {status:<7}  {id}",
            time = format_time(run.started),
            repo = truncate(run.repo.as_deref().unwrap_or("-"), 14),
            wave = truncate(run.wave.as_deref().unwrap_or("-"), 10),
            label = truncate(&run.label, 24),
            dur = run
                .ended
                .map(|end| format_duration(end - run.started))
                .unwrap_or_else(|| "…".to_string()),
            tokens = format_tokens(run.total_tokens()),
            status = run.status,
            id = short_id(&run.run_id),
        );
    }
    Ok(())
}

/// `lf trace <run-id>`: reconstruct one run (id may be a unique prefix).
pub fn trace(run_id: &str) -> Result<()> {
    let store = open_ledger().map_err(|err| anyhow!("run ledger unavailable: {err}"))?;
    let events = store
        .run_events_matching(run_id)
        .map_err(|err| anyhow!("failed to read run ledger: {err}"))?;

    let mut ids: Vec<&str> = events.iter().map(|e| e.run_id.as_str()).collect();
    ids.dedup();
    match ids.len() {
        0 => return Err(anyhow!("no run matching '{run_id}' in the ledger")),
        1 => {}
        _ => {
            return Err(anyhow!(
                "'{run_id}' is ambiguous — matches: {}",
                ids.into_iter().map(short_id).collect::<Vec<_>>().join(", ")
            ))
        }
    }

    let colors = Colors::default();
    let full_id = events[0].run_id.clone();
    let start = events.first().expect("nonempty").ts;
    let end = events
        .iter()
        .rev()
        .find(|e| e.node == "run" && e.event != "started")
        .map(|e| e.ts);

    let header = &events[0];
    println!(
        "{bold}run {id}{reset}  {repo}{wave}",
        bold = colors.bold,
        reset = colors.reset,
        id = full_id,
        repo = header.repo.as_deref().unwrap_or("-"),
        wave = header
            .wave
            .as_deref()
            .map(|w| format!("  wave:{w}"))
            .unwrap_or_default(),
    );
    if let Some(argv) = header.command.as_deref().and_then(parse_argv) {
        println!("  command   {}", argv.join(" "));
    }
    if let Some(worktree) = header.worktree.as_deref() {
        println!("  worktree  {worktree}");
    }
    println!(
        "  started   {}   duration {}",
        format_time(start),
        end.map(|end| format_duration(end - start))
            .unwrap_or_else(|| "running".to_string()),
    );

    // Step/flow timeline with per-step durations and token deltas (token
    // fields on step boundaries are cumulative snapshots).
    let mut open: BTreeMap<String, i64> = BTreeMap::new();
    let mut last_tokens: i64 = 0;
    for event in &events {
        let key = match (
            event.node.as_str(),
            event.step.as_deref(),
            event.flow.as_deref(),
        ) {
            ("step", Some(step), _) => format!("step:{step}:{}", event.step_index.unwrap_or(0)),
            ("flow", _, flow) => format!("flow:{}", flow.unwrap_or("")),
            _ => continue,
        };
        let name = event
            .step
            .as_deref()
            .or(event.flow.as_deref())
            .unwrap_or("?")
            .to_string();
        match event.event.as_str() {
            "started" => {
                open.insert(key, event.ts);
                if event.node == "flow" {
                    println!("  flow      {name}");
                }
            }
            "completed" | "errored" | "escalated" => {
                let started = open.remove(&key);
                if event.node == "step" {
                    let tokens = event
                        .input_tokens
                        .unwrap_or(0)
                        .saturating_add(event.output_tokens.unwrap_or(0));
                    let delta = (tokens - last_tokens).max(0);
                    if tokens > 0 {
                        last_tokens = tokens;
                    }
                    println!(
                        "  ├─ {name:<20}  {dur:>8}  {tokens:>10}  {status}",
                        dur = started
                            .map(|s| format_duration(event.ts - s))
                            .unwrap_or_default(),
                        tokens = if delta > 0 {
                            format_tokens(delta)
                        } else {
                            String::new()
                        },
                        status = match event.event.as_str() {
                            "completed" => "ok".to_string(),
                            other => format!(
                                "{other}{}",
                                event
                                    .error
                                    .as_deref()
                                    .map(|e| format!(": {}", truncate(e, 60)))
                                    .unwrap_or_default()
                            ),
                        },
                    );
                }
            }
            _ => {}
        }
    }

    // Terminal run row: totals + error.
    if let Some(terminal) = events
        .iter()
        .rev()
        .find(|e| e.node == "run" && e.event != "started")
    {
        let tokens = terminal.input_tokens.unwrap_or(0) + terminal.output_tokens.unwrap_or(0);
        let mut line = format!("  status    {}", status_label(&terminal.event));
        if tokens > 0 {
            line.push_str(&format!("   tokens {}", format_tokens(tokens)));
        }
        if let Some(cache) = terminal.cache_read_tokens.filter(|c| *c > 0) {
            line.push_str(&format!(" (+{} cache read)", format_tokens(cache)));
        }
        if let Some(cost) = terminal.cost_usd {
            line.push_str(&format!("   cost ${cost:.2}"));
        }
        println!("{line}");
        if let Some(error) = terminal.error.as_deref() {
            println!("  error     {}", truncate(error, 200));
        }
    }

    for path in prompt_logs(&full_id) {
        println!("  prompt    {}", path.display());
    }
    if let Some(output) = output_log(&full_id) {
        println!("  output    {}", output.display());
    }

    Ok(())
}

#[derive(Debug)]
struct RunSummary {
    run_id: String,
    repo: Option<String>,
    wave: Option<String>,
    label: String,
    started: i64,
    ended: Option<i64>,
    status: &'static str,
    input_tokens: i64,
    output_tokens: i64,
}

impl RunSummary {
    fn total_tokens(&self) -> i64 {
        self.input_tokens + self.output_tokens
    }
}

/// `lf runs --json` entry: one folded run from the ledger. Wire type consumed
/// by Concerto — every field required or explicitly Optional, no serde
/// defaults. `started`/`ended` are unix seconds (the ledger's grain).
#[derive(Debug, serde::Serialize)]
struct RunLedgerEntry {
    id: String,
    repo: Option<String>,
    wave: Option<String>,
    label: String,
    status: String,
    started: i64,
    ended: Option<i64>,
    input_tokens: i64,
    output_tokens: i64,
}

impl From<&RunSummary> for RunLedgerEntry {
    fn from(run: &RunSummary) -> Self {
        Self {
            id: run.run_id.clone(),
            repo: run.repo.clone(),
            wave: run.wave.clone(),
            label: run.label.clone(),
            status: run.status.to_string(),
            started: run.started,
            ended: run.ended,
            input_tokens: run.input_tokens,
            output_tokens: run.output_tokens,
        }
    }
}

fn summarize(events: &[RunEventRow]) -> Vec<RunSummary> {
    let mut by_run: BTreeMap<&str, Vec<&RunEventRow>> = BTreeMap::new();
    for event in events {
        by_run.entry(&event.run_id).or_default().push(event);
    }

    by_run
        .into_iter()
        .map(|(run_id, events)| {
            let started = events.iter().map(|e| e.ts).min().unwrap_or(0);
            let terminal = events
                .iter()
                .rev()
                .find(|e| e.node == "run" && e.event != "started");
            let label = events
                .iter()
                .find_map(|e| e.flow.clone())
                .or_else(|| events.iter().find_map(|e| e.step.clone()))
                .or_else(|| {
                    events
                        .iter()
                        .find_map(|e| e.command.as_deref().and_then(parse_argv))
                        .map(|argv| argv.into_iter().skip(1).collect::<Vec<_>>().join(" "))
                })
                .unwrap_or_else(|| "-".to_string());
            RunSummary {
                run_id: run_id.to_string(),
                repo: events.iter().find_map(|e| e.repo.clone()),
                wave: events.iter().find_map(|e| e.wave.clone()),
                label,
                started,
                ended: terminal.map(|e| e.ts),
                status: terminal
                    .map(|e| status_label(&e.event))
                    .unwrap_or("running"),
                input_tokens: terminal.and_then(|e| e.input_tokens).unwrap_or(0),
                output_tokens: terminal.and_then(|e| e.output_tokens).unwrap_or(0),
            }
        })
        .collect()
}

fn status_label(event: &str) -> &'static str {
    match event {
        "completed" => "ok",
        "errored" => "error",
        "escalated" => "escal.",
        _ => "running",
    }
}

fn parse_argv(json: &str) -> Option<Vec<String>> {
    serde_json::from_str(json).ok()
}

/// Prompt/context logs for a run: `~/.lf/logs/<repo>/<worktree>/*-<id>-*.md`.
fn prompt_logs(run_id: &str) -> Vec<PathBuf> {
    let Some(home) = dirs::home_dir() else {
        return Vec::new();
    };
    let needle = format!("-{run_id}-");
    let mut matches = Vec::new();
    let logs_root = home.join(".lf/logs");
    for repo in read_dirs(&logs_root) {
        for worktree in read_dirs(&repo) {
            if let Ok(entries) = std::fs::read_dir(&worktree) {
                for entry in entries.flatten() {
                    let path = entry.path();
                    if path
                        .file_name()
                        .and_then(|n| n.to_str())
                        .is_some_and(|name| name.contains(&needle))
                    {
                        matches.push(path);
                    }
                }
            }
        }
    }
    matches.sort();
    matches
}

fn output_log(run_id: &str) -> Option<PathBuf> {
    let path = dirs::home_dir()?
        .join(".lf/output")
        .join(format!("{run_id}.log"));
    path.exists().then_some(path)
}

fn read_dirs(root: &std::path::Path) -> Vec<PathBuf> {
    std::fs::read_dir(root)
        .map(|entries| {
            entries
                .flatten()
                .map(|entry| entry.path())
                .filter(|path| path.is_dir())
                .collect()
        })
        .unwrap_or_default()
}

fn format_time(unix: i64) -> String {
    chrono::DateTime::from_timestamp(unix, 0)
        .map(|utc| {
            utc.with_timezone(&chrono::Local)
                .format("%m-%d %H:%M")
                .to_string()
        })
        .unwrap_or_else(|| unix.to_string())
}

fn format_duration(secs: i64) -> String {
    let secs = secs.max(0);
    if secs >= 3600 {
        format!("{}h{:02}m", secs / 3600, (secs % 3600) / 60)
    } else if secs >= 60 {
        format!("{}m{:02}s", secs / 60, secs % 60)
    } else {
        format!("{secs}s")
    }
}

fn format_tokens(value: i64) -> String {
    if value >= 1_000_000 {
        format!("{:.1}M", value as f64 / 1_000_000.0)
    } else if value >= 1_000 {
        format!("{:.1}k", value as f64 / 1_000.0)
    } else if value > 0 {
        value.to_string()
    } else {
        String::new()
    }
}

fn truncate(value: &str, width: usize) -> String {
    if value.chars().count() <= width {
        return value.to_string();
    }
    let head: String = value.chars().take(width.saturating_sub(1)).collect();
    format!("{head}\u{2026}")
}

#[cfg(test)]
mod tests {
    use super::{format_duration, format_tokens, summarize};
    use crate::lfdb::RunEventRow;

    fn row(run_id: &str, seq: i64, ts: i64, node: &str, event: &str) -> RunEventRow {
        RunEventRow {
            run_id: run_id.to_string(),
            seq,
            ts,
            repo: Some("loopflow".to_string()),
            worktree: None,
            wave: None,
            node: node.to_string(),
            event: event.to_string(),
            command: Some(r#"["lf","gate"]"#.to_string()),
            flow: None,
            step: None,
            step_index: None,
            error: None,
            input_tokens: None,
            output_tokens: None,
            cache_read_tokens: None,
            cost_usd: None,
            duration_secs: None,
        }
    }

    #[test]
    fn summarize_folds_run_events_into_one_summary() {
        let mut terminal = row("abc", 1, 110, "run", "completed");
        terminal.input_tokens = Some(1000);
        terminal.output_tokens = Some(200);
        let events = vec![row("abc", 0, 100, "run", "started"), terminal];

        let summaries = summarize(&events);
        assert_eq!(summaries.len(), 1);
        let run = &summaries[0];
        assert_eq!(run.run_id, "abc");
        assert_eq!(run.started, 100);
        assert_eq!(run.ended, Some(110));
        assert_eq!(run.status, "ok");
        assert_eq!(run.total_tokens(), 1200);
        assert_eq!(run.label, "gate"); // from command argv
    }

    #[test]
    fn summarize_marks_unterminated_runs_as_running() {
        let events = vec![row("abc", 0, 100, "run", "started")];
        let summaries = summarize(&events);
        assert_eq!(summaries[0].status, "running");
        assert_eq!(summaries[0].ended, None);
    }

    #[test]
    fn json_entry_carries_fold_and_stable_keys() {
        use super::RunLedgerEntry;
        let mut terminal = row("abc", 1, 110, "run", "completed");
        terminal.input_tokens = Some(1000);
        terminal.output_tokens = Some(200);
        let events = vec![row("abc", 0, 100, "run", "started"), terminal];
        let summaries = summarize(&events);
        let entry = RunLedgerEntry::from(&summaries[0]);
        let value = serde_json::to_value(&entry).expect("serialize");
        assert_eq!(value["id"], "abc");
        assert_eq!(value["status"], "ok");
        assert_eq!(value["started"], 100);
        assert_eq!(value["ended"], 110);
        assert_eq!(value["label"], "gate");
        // Explicitly-Optional fields stay present (a running run's `ended` is
        // null, never absent) — one stable shape for the client.
        let running = summarize(&[row("xyz", 0, 100, "run", "started")]);
        let entry = RunLedgerEntry::from(&running[0]);
        let value = serde_json::to_value(&entry).expect("serialize");
        assert_eq!(value["ended"], serde_json::Value::Null);
        assert_eq!(value["status"], "running");
    }

    #[test]
    fn human_formats() {
        assert_eq!(format_duration(42), "42s");
        assert_eq!(format_duration(125), "2m05s");
        assert_eq!(format_duration(3700), "1h01m");
        assert_eq!(format_tokens(184_000), "184.0k");
        assert_eq!(format_tokens(0), "");
    }
}
