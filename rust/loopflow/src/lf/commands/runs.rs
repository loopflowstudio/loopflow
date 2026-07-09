//! `lf runs` and `lf trace` — read the local run ledger.
//!
//! Both commands are pure readers over the machine-grain SQLite ledger
//! (`run_events`, written by every `lf` invocation) plus the prompt logs on
//! disk. Local-only: nothing is fetched from anywhere.

use std::collections::{BTreeMap, BTreeSet};
use std::path::PathBuf;

use anyhow::{anyhow, Result};

use crate::journal::open_ledger;
use crate::lf::output::Colors;
use crate::lfdb::RunEventRow;
use crate::wave::journal::short_id;

const WINDOW_DAYS: i64 = 7;
const MAX_RUNS: usize = 50;

/// `lf runs`: timeline of recent runs across every repo on this machine.
/// `--json` emits the same window as a machine-readable array (Loopflow's
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
        "{bold}{time:<12}  {repo:<14}  {wave:<10}  {label:<22}  {tokens:>10}  {cost:>8}  {agent:<18}  {status:<7}  TRACE/SPAN{reset}",
        bold = colors.bold,
        reset = colors.reset,
        time = "TIME",
        repo = "REPO",
        wave = "WAVE",
        label = "RUN",
        tokens = "TOKENS",
        cost = "COST",
        agent = "AGENT",
        status = "STATUS",
    );
    for run in &summaries {
        println!(
            "{time:<12}  {repo:<14}  {wave:<10}  {label:<22}  {tokens:>10}  {cost:>8}  {agent:<18}  {status:<7}  {run}/{span}",
            time = format_time(run.started),
            repo = truncate(&display_repo(run.repo.as_deref()), 14),
            wave = truncate(run.wave.as_deref().unwrap_or("-"), 10),
            label = truncate(&run.label, 22),
            tokens = format_tokens(run.total_tokens()),
            cost = run
                .cost_usd
                .map(format_cost)
                .unwrap_or_else(|| "-".to_string()),
            agent = truncate(&format_agent(run.provider.as_deref(), run.model.as_deref()), 18),
            status = run.status,
            run = short_id(&run.run_id),
            span = short_id(&run.process_id),
        );
    }
    Ok(())
}

/// `lf trace <run-id>`: reconstruct one run (id may be a unique prefix).
pub fn trace(run_id: &str, json: bool) -> Result<()> {
    let store = open_ledger().map_err(|err| anyhow!("run ledger unavailable: {err}"))?;
    let events = store
        .run_events_matching(run_id)
        .map_err(|err| anyhow!("failed to read run ledger: {err}"))?;

    let ids: BTreeSet<&str> = events.iter().map(|event| event.run_id.as_str()).collect();
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

    let spans = trace_spans(&events);
    if json {
        println!("{}", serde_json::to_string(&spans)?);
        return Ok(());
    }

    let colors = Colors::default();
    let full_id = events[0].run_id.clone();
    let start = spans.iter().map(|span| span.started_at).min().unwrap_or(0);
    let end = spans.iter().filter_map(|span| span.ended_at).max();

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
    if let Some(worktree) = header.worktree.as_deref() {
        println!("  worktree  {worktree}");
    }
    println!(
        "  started   {}   duration {}",
        format_time(start),
        end.map(|end| format_duration(end - start))
            .unwrap_or_else(|| "running".to_string()),
    );

    print_span_tree(&spans);
    let total_cost: f64 = spans.iter().filter_map(|span| span.cost_usd).sum();
    let total_tokens: i64 = spans
        .iter()
        .map(|span| span.input_tokens.unwrap_or(0) + span.output_tokens.unwrap_or(0))
        .sum();
    println!(
        "  total     {}   {}",
        format_tokens(total_tokens),
        format_cost(total_cost)
    );

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
    process_id: String,
    parent_process_id: Option<String>,
    repo: Option<String>,
    wave: Option<String>,
    label: String,
    started: i64,
    ended: Option<i64>,
    status: &'static str,
    input_tokens: i64,
    output_tokens: i64,
    cache_read_tokens: i64,
    cost_usd: Option<f64>,
    duration_secs: Option<f64>,
    provider: Option<String>,
    model: Option<String>,
}

impl RunSummary {
    fn total_tokens(&self) -> i64 {
        self.input_tokens + self.output_tokens
    }
}

/// `lf runs --json` entry: one folded run from the ledger. Wire type consumed
/// by Loopflow — every field required or explicitly Optional, no serde
/// defaults. `started`/`ended` are unix seconds (the ledger's grain).
#[derive(Debug, serde::Serialize)]
struct RunLedgerEntry {
    id: String,
    run_id: String,
    process_id: String,
    parent_process_id: Option<String>,
    repo: Option<String>,
    wave: Option<String>,
    label: String,
    status: String,
    started: i64,
    ended: Option<i64>,
    input_tokens: i64,
    output_tokens: i64,
    cache_read_tokens: i64,
    cost_usd: Option<f64>,
    duration_secs: Option<f64>,
    provider: Option<String>,
    model: Option<String>,
}

impl From<&RunSummary> for RunLedgerEntry {
    fn from(run: &RunSummary) -> Self {
        Self {
            id: run.process_id.clone(),
            run_id: run.run_id.clone(),
            process_id: run.process_id.clone(),
            parent_process_id: run.parent_process_id.clone(),
            repo: run.repo.clone(),
            wave: run.wave.clone(),
            label: run.label.clone(),
            status: run.status.to_string(),
            started: run.started,
            ended: run.ended,
            input_tokens: run.input_tokens,
            output_tokens: run.output_tokens,
            cache_read_tokens: run.cache_read_tokens,
            cost_usd: run.cost_usd,
            duration_secs: run.duration_secs,
            provider: run.provider.clone(),
            model: run.model.clone(),
        }
    }
}

fn summarize(events: &[RunEventRow]) -> Vec<RunSummary> {
    let mut by_process: BTreeMap<&str, Vec<&RunEventRow>> = BTreeMap::new();
    for event in events {
        by_process.entry(&event.process_id).or_default().push(event);
    }

    by_process
        .into_iter()
        .map(|(process_id, events)| {
            let started = events.iter().map(|e| e.ts).min().unwrap_or(0);
            let terminal = events
                .iter()
                .rev()
                .find(|e| e.node == "run" && e.event != "started");
            let label = events
                .iter()
                .find_map(|e| e.command.as_deref().and_then(command_label))
                .or_else(|| events.iter().find_map(|e| e.flow.clone()))
                .or_else(|| events.iter().find_map(|e| e.skill.clone()))
                .unwrap_or_else(|| "-".to_string());
            RunSummary {
                run_id: events[0].run_id.clone(),
                process_id: process_id.to_string(),
                parent_process_id: events[0].parent_process_id.clone(),
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
                cache_read_tokens: terminal.and_then(|e| e.cache_read_tokens).unwrap_or(0),
                cost_usd: terminal.and_then(|e| e.cost_usd),
                duration_secs: terminal.and_then(|e| e.duration_secs),
                provider: terminal
                    .and_then(|event| event.provider.clone())
                    .or_else(|| events.iter().rev().find_map(|event| event.provider.clone())),
                model: terminal
                    .and_then(|event| event.model.clone())
                    .or_else(|| events.iter().rev().find_map(|event| event.model.clone())),
            }
        })
        .collect()
}

/// One process in a run trace. A trace is the `run_id`; a span is the
/// `process_id`. All fields are required or explicitly optional so this wire
/// shape fails loudly when producer and consumer drift.
#[derive(Debug, Clone, serde::Deserialize, serde::Serialize, PartialEq)]
pub struct SpanDto {
    pub run_id: String,
    pub process_id: String,
    pub parent_process_id: Option<String>,
    pub node: String,
    pub name: Option<String>,
    pub repo: Option<String>,
    pub wave: Option<String>,
    pub flow: Option<String>,
    pub skill: Option<String>,
    pub started_at: i64,
    pub ended_at: Option<i64>,
    pub status: String,
    pub input_tokens: Option<i64>,
    pub output_tokens: Option<i64>,
    pub cache_read_tokens: Option<i64>,
    pub cost_usd: Option<f64>,
    pub duration_secs: Option<f64>,
    pub provider: Option<String>,
    pub model: Option<String>,
}

fn trace_spans(events: &[RunEventRow]) -> Vec<SpanDto> {
    let mut by_process: BTreeMap<&str, Vec<&RunEventRow>> = BTreeMap::new();
    for event in events {
        by_process.entry(&event.process_id).or_default().push(event);
    }
    let mut spans: Vec<_> = by_process
        .into_values()
        .map(|mut process_events| {
            process_events.sort_by_key(|event| (event.ts, event.seq));
            let started = process_events
                .iter()
                .find(|event| event.node == "run" && event.event == "started")
                .copied()
                .unwrap_or(process_events[0]);
            let terminal = process_events
                .iter()
                .rev()
                .find(|event| event.node == "run" && event.event != "started")
                .copied();
            let boundary = terminal.unwrap_or_else(|| {
                process_events
                    .last()
                    .copied()
                    .expect("process has an event")
            });
            SpanDto {
                run_id: started.run_id.clone(),
                process_id: started.process_id.clone(),
                parent_process_id: started.parent_process_id.clone(),
                node: "run".to_string(),
                name: started
                    .command
                    .as_deref()
                    .and_then(parse_argv)
                    .map(|argv| argv.join(" ")),
                repo: started.repo.clone(),
                wave: started.wave.clone(),
                flow: process_events.iter().find_map(|event| event.flow.clone()),
                skill: None,
                started_at: started.ts,
                ended_at: terminal.map(|event| event.ts),
                status: terminal
                    .map(|event| event.event.clone())
                    .unwrap_or_else(|| "open".to_string()),
                input_tokens: boundary.input_tokens,
                output_tokens: boundary.output_tokens,
                cache_read_tokens: boundary.cache_read_tokens,
                cost_usd: boundary.cost_usd,
                duration_secs: boundary.duration_secs,
                provider: process_events
                    .iter()
                    .rev()
                    .find_map(|event| event.provider.clone()),
                model: process_events
                    .iter()
                    .rev()
                    .find_map(|event| event.model.clone()),
            }
        })
        .collect();
    spans.sort_by_key(|span| (span.started_at, span.process_id.clone()));
    spans
}

/// Every row that carries a cumulative usage reading, at its own grain: a skill
/// boundary names its skill, a terminal run row names its command. Feed this to
/// [`own_spend`] to get what each boundary actually spent.
///
/// `trace_spans` folds a whole process into one span, which is right for a
/// process tree and wrong for asking where the tokens went inside it.
pub(crate) fn boundary_spans(events: &[RunEventRow]) -> Vec<SpanDto> {
    let mut rows: Vec<&RunEventRow> = events
        .iter()
        .filter(|event| event.input_tokens.is_some())
        .filter(|event| (event.node == "run" || event.node == "skill") && event.event != "started")
        .collect();
    // own_spend diffs against the previous boundary in the same process, so the
    // rows must reach it in the order the process produced them.
    rows.sort_by_key(|event| (event.ts, event.seq));

    rows.into_iter()
        .map(|event| SpanDto {
            run_id: event.run_id.clone(),
            process_id: event.process_id.clone(),
            parent_process_id: event.parent_process_id.clone(),
            node: event.node.clone(),
            name: event.skill.clone().or_else(|| {
                event
                    .command
                    .as_deref()
                    .and_then(parse_argv)
                    .map(|argv| argv.join(" "))
            }),
            repo: event.repo.clone(),
            wave: event.wave.clone(),
            flow: event.flow.clone(),
            skill: event.skill.clone(),
            started_at: event.ts,
            ended_at: Some(event.ts),
            status: event.event.clone(),
            input_tokens: event.input_tokens,
            output_tokens: event.output_tokens,
            cache_read_tokens: event.cache_read_tokens,
            cost_usd: event.cost_usd,
            duration_secs: event.duration_secs,
            provider: event.provider.clone(),
            model: event.model.clone(),
        })
        .collect()
}

/// Convert cumulative boundary readings into the spend attributable to each
/// boundary. Diffing lives here so CLI and dashboard consumers share one rule.
pub fn own_spend(spans: &[SpanDto]) -> Vec<SpanDto> {
    let mut previous: BTreeMap<&str, BoundaryUsage> = BTreeMap::new();
    spans
        .iter()
        .map(|span| {
            let prior = previous
                .get(span.process_id.as_str())
                .copied()
                .unwrap_or_default();
            let mut own = span.clone();
            own.input_tokens = diff_i64(span.input_tokens, prior.input_tokens);
            own.output_tokens = diff_i64(span.output_tokens, prior.output_tokens);
            own.cache_read_tokens = diff_i64(span.cache_read_tokens, prior.cache_read_tokens);
            own.cost_usd = diff_f64(span.cost_usd, prior.cost_usd);
            own.duration_secs = diff_f64(span.duration_secs, prior.duration_secs);
            previous.insert(
                span.process_id.as_str(),
                BoundaryUsage {
                    input_tokens: span.input_tokens,
                    output_tokens: span.output_tokens,
                    cache_read_tokens: span.cache_read_tokens,
                    cost_usd: span.cost_usd,
                    duration_secs: span.duration_secs,
                },
            );
            own
        })
        .collect()
}

#[derive(Clone, Copy, Default)]
struct BoundaryUsage {
    input_tokens: Option<i64>,
    output_tokens: Option<i64>,
    cache_read_tokens: Option<i64>,
    cost_usd: Option<f64>,
    duration_secs: Option<f64>,
}

fn diff_i64(value: Option<i64>, previous: Option<i64>) -> Option<i64> {
    value.map(|value| value.saturating_sub(previous.unwrap_or(0)).max(0))
}

fn diff_f64(value: Option<f64>, previous: Option<f64>) -> Option<f64> {
    value.map(|value| (value - previous.unwrap_or(0.0)).max(0.0))
}

fn status_label(event: &str) -> &'static str {
    match event {
        "completed" => "ok",
        "errored" => "error",
        "escalated" => "escal.",
        _ => "running",
    }
}

fn print_span_tree(spans: &[SpanDto]) {
    let mut children: BTreeMap<Option<&str>, Vec<&SpanDto>> = BTreeMap::new();
    for span in spans {
        children
            .entry(span.parent_process_id.as_deref())
            .or_default()
            .push(span);
    }
    for roots in children.values_mut() {
        roots.sort_by_key(|span| (span.started_at, span.process_id.as_str()));
    }
    let process_ids: BTreeSet<&str> = spans.iter().map(|span| span.process_id.as_str()).collect();
    let mut roots: Vec<_> = spans
        .iter()
        .filter(|span| {
            span.parent_process_id
                .as_deref()
                .is_none_or(|parent| !process_ids.contains(parent))
        })
        .collect();
    roots.sort_by_key(|span| (span.started_at, span.process_id.as_str()));
    for root in roots {
        print_span(root, 1, &children);
    }
}

fn print_span(span: &SpanDto, depth: usize, children: &BTreeMap<Option<&str>, Vec<&SpanDto>>) {
    let indent = "  ".repeat(depth);
    let name = span.name.as_deref().unwrap_or("?");
    let duration = span
        .ended_at
        .map(|ended| format_duration(ended - span.started_at))
        .unwrap_or_else(|| "open".to_string());
    let tokens = span.input_tokens.unwrap_or(0) + span.output_tokens.unwrap_or(0);
    let cost = span.cost_usd.map(format_cost).unwrap_or_default();
    let agent = format_agent(span.provider.as_deref(), span.model.as_deref());
    println!(
        "{indent}├─ {name:<28} {duration:>8} {tokens:>10} {cost:>8} {agent:<18} {status:<9} span:{id}",
        name = truncate(name, 28),
        tokens = format_tokens(tokens),
        status = span.status,
        id = short_id(&span.process_id),
    );
    if let Some(nested) = children.get(&Some(span.process_id.as_str())) {
        for child in nested {
            print_span(child, depth + 1, children);
        }
    }
}

fn parse_argv(json: &str) -> Option<Vec<String>> {
    serde_json::from_str(json).ok()
}

fn command_label(json: &str) -> Option<String> {
    parse_argv(json).map(|argv| argv.into_iter().skip(1).collect::<Vec<_>>().join(" "))
}

fn format_agent(provider: Option<&str>, model: Option<&str>) -> String {
    match (provider, model) {
        (Some(provider), Some(model)) => format!("{provider}:{model}"),
        (Some(provider), None) => provider.to_string(),
        (None, Some(model)) => model.to_string(),
        (None, None) => "-".to_string(),
    }
}

fn display_repo(repo: Option<&str>) -> String {
    repo.and_then(|value| std::path::Path::new(value).file_name())
        .and_then(|value| value.to_str())
        .or(repo)
        .unwrap_or("-")
        .to_string()
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

fn format_cost(value: f64) -> String {
    format!("${value:.2}")
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
    use super::{
        boundary_spans, format_duration, format_tokens, own_spend, summarize, trace_spans, SpanDto,
    };
    use crate::lfdb::RunEventRow;

    fn row(run_id: &str, seq: i64, ts: i64, node: &str, event: &str) -> RunEventRow {
        RunEventRow {
            run_id: run_id.to_string(),
            process_id: run_id.to_string(),
            parent_process_id: None,
            seq,
            ts,
            repo: Some("/src/loopflow".to_string()),
            worktree: None,
            wave: None,
            node: node.to_string(),
            event: event.to_string(),
            command: Some(r#"["lf","gate"]"#.to_string()),
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
    fn two_processes_sharing_a_run_id_summarize_separately() {
        let mut parent_start = row("66863649", 0, 100, "run", "started");
        parent_start.process_id = "parent".to_string();
        parent_start.command = Some(r#"["lf","wave","intel"]"#.to_string());
        let mut parent_end = parent_start.clone();
        parent_end.seq = 1;
        parent_end.ts = 120;
        parent_end.event = "completed".to_string();
        parent_end.cost_usd = Some(1.25);

        let mut child_start = row("66863649", 0, 105, "run", "started");
        child_start.process_id = "child".to_string();
        child_start.parent_process_id = Some("parent".to_string());
        child_start.command = Some(r#"["lf","pm","show"]"#.to_string());
        let mut child_end = child_start.clone();
        child_end.seq = 1;
        child_end.ts = 110;
        child_end.event = "errored".to_string();
        child_end.cost_usd = Some(0.05);

        let summaries = summarize(&[parent_start, child_start, child_end, parent_end]);
        assert_eq!(summaries.len(), 2);
        let parent = summaries
            .iter()
            .find(|summary| summary.process_id == "parent")
            .unwrap();
        let child = summaries
            .iter()
            .find(|summary| summary.process_id == "child")
            .unwrap();
        assert_eq!(parent.label, "wave intel");
        assert_eq!(parent.cost_usd, Some(1.25));
        assert_eq!(child.label, "pm show");
        assert_eq!(child.cost_usd, Some(0.05));
        assert_eq!(child.status, "error");
    }

    #[test]
    fn a_span_that_never_closed_is_open_not_zero_width() {
        let spans = trace_spans(&[row("abc", 0, 100, "run", "started")]);
        assert_eq!(spans.len(), 1);
        assert_eq!(spans[0].status, "open");
        assert_eq!(spans[0].ended_at, None);
    }

    /// The charts group these rows and must reconcile with `lf usage`. A skill
    /// boundary reads cumulative, so the terminal run row that follows it spends
    /// nothing of its own — summing every boundary yields the run total exactly
    /// once.
    #[test]
    fn boundary_spend_sums_to_the_run_total_without_double_counting() {
        let mut events = vec![
            row("trace", 1, 100, "run", "started"),
            row("trace", 2, 110, "skill", "completed"),
            row("trace", 3, 120, "skill", "completed"),
            row("trace", 4, 130, "run", "completed"),
        ];
        events[1].skill = Some("implement".to_string());
        events[1].input_tokens = Some(100);
        events[2].skill = Some("gate".to_string());
        events[2].input_tokens = Some(150); // cumulative
        events[3].input_tokens = Some(150); // the run total

        let spend = own_spend(&boundary_spans(&events));
        let total: i64 = spend.iter().map(|s| s.input_tokens.unwrap_or(0)).sum();

        assert_eq!(total, 150, "boundaries must sum to the run total");
        assert_eq!(spend.len(), 3);
        assert_eq!(spend[0].skill.as_deref(), Some("implement"));
        assert_eq!(spend[0].input_tokens, Some(100));
        assert_eq!(spend[1].skill.as_deref(), Some("gate"));
        assert_eq!(spend[1].input_tokens, Some(50));
        // The terminal row adds nothing: its process already reported everything.
        assert_eq!(spend[2].input_tokens, Some(0));
    }

    #[test]
    fn own_spend_diffs_consecutive_boundaries_within_a_process() {
        let boundary = |cost, input| SpanDto {
            run_id: "trace".to_string(),
            process_id: "span".to_string(),
            parent_process_id: None,
            node: "skill".to_string(),
            name: Some("implement".to_string()),
            repo: Some("/repo".to_string()),
            wave: None,
            flow: None,
            skill: Some("implement".to_string()),
            started_at: input,
            ended_at: Some(input + 1),
            status: "completed".to_string(),
            input_tokens: Some(input),
            output_tokens: Some(input / 10),
            cache_read_tokens: Some(0),
            cost_usd: Some(cost),
            duration_secs: Some(input as f64 / 10.0),
            provider: Some("claude".to_string()),
            model: Some("opus".to_string()),
        };
        let own = own_spend(&[boundary(1.0, 100), boundary(1.25, 150)]);
        assert_eq!(own[0].cost_usd, Some(1.0));
        assert_eq!(own[1].cost_usd, Some(0.25));
        assert_eq!(own[1].input_tokens, Some(50));
        assert_eq!(own[1].output_tokens, Some(5));
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
