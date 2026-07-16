//! `lf runs`, `lf execs`, and `lf trace` — read local execution evidence.
//!
//! Runs are agent-backed skill launches, the grain that owns context and token
//! evidence. Execs are `lf` processes, the grain that owns argv and process
//! lineage. Trace joins the two through `run_id` and `process_id`. All commands
//! are local-only readers; nothing is fetched from anywhere.

use std::collections::{BTreeMap, BTreeSet};
use std::io::{BufRead, BufReader};
use std::path::Path;

use anyhow::{anyhow, Result};

use crate::journal::open_ledger;
use crate::lf::output::{format_cost, truncate, Colors};
use crate::store::RunEventRow;
use crate::wave::journal::short_id;

const WINDOW_DAYS: i64 = 7;
const MAX_RUNS: usize = 50;

/// `lf runs`: recent agent-backed skill launches across this machine.
pub fn list(json: bool) -> Result<()> {
    let store = open_ledger().map_err(|err| anyhow!("run ledger unavailable: {err}"))?;
    let since = chrono::Utc::now().timestamp() - WINDOW_DAYS * 24 * 3600;
    let events = store
        .list_run_events_since(since)
        .map_err(|err| anyhow!("failed to read run ledger: {err}"))?;
    let launches = store
        .agent_launches_since(since)
        .map_err(|err| anyhow!("failed to read skill launches: {err}"))?
        .into_iter()
        .filter(|launch| launch.skill.is_some())
        .collect::<Vec<_>>();
    let launch_ids = launches
        .iter()
        .map(|launch| launch.id.clone())
        .collect::<Vec<_>>();
    let turns = store
        .agent_turns_for_launches(&launch_ids)
        .map_err(|err| anyhow!("failed to read run turns: {err}"))?;

    let mut runs = summarize_runs(&events, &launches, &turns);
    sort_runs(&mut runs);
    cap_runs(&mut runs);

    if json {
        println!("{}", serde_json::to_string(&runs)?);
        return Ok(());
    }

    if runs.is_empty() {
        println!("No skill runs recorded in the last {WINDOW_DAYS} days.");
        return Ok(());
    }

    let colors = Colors::default();
    println!(
        "{bold}{time:<12}  {repo:<14}  {wave:<10}  {label:<22}  {context:>9}  {tokens:>10}  {cost:>8}  {agent:<18}  {status:<7}  RUN{reset}",
        bold = colors.bold,
        reset = colors.reset,
        time = "TIME",
        repo = "REPO",
        wave = "WAVE",
        label = "RUN",
        context = "CONTEXT",
        tokens = "TOKENS",
        cost = "COST",
        agent = "AGENT",
        status = "STATUS",
    );
    for run in &runs {
        println!(
            "{time:<12}  {repo:<14}  {wave:<10}  {label:<22}  {context:>9}  {tokens:>10}  {cost:>8}  {agent:<18}  {status:<7}  {id}",
            time = format_time(run.started),
            repo = truncate(&display_repo(Some(&run.repo)), 14),
            wave = truncate(run.wave.as_deref().unwrap_or("-"), 10),
            label = truncate(&run.label(), 22),
            context = format_tokens(run.supplied_context_tokens),
            tokens = format_tokens(run.total_tokens()),
            cost = run
                .cost_usd
                .map(format_cost)
                .unwrap_or_else(|| "-".to_string()),
            agent = truncate(
                &format_agent(Some(run.provider.as_str()), run.model.as_deref()),
                18
            ),
            status = run.status,
            id = short_id(&run.id),
        );
    }
    Ok(())
}

/// `lf execs`: recent `lf` processes across every repo on this machine.
pub fn list_execs(json: bool) -> Result<()> {
    let store = open_ledger().map_err(|err| anyhow!("exec ledger unavailable: {err}"))?;
    let since = chrono::Utc::now().timestamp() - WINDOW_DAYS * 24 * 3600;
    let events = store
        .list_run_events_since(since)
        .map_err(|err| anyhow!("failed to read exec ledger: {err}"))?;

    let mut execs = summarize_execs(&events);
    execs.sort_by_key(|exec| std::cmp::Reverse(exec.started));
    execs.truncate(MAX_RUNS);

    if json {
        println!("{}", serde_json::to_string(&execs)?);
        return Ok(());
    }
    if execs.is_empty() {
        println!("No execs recorded in the last {WINDOW_DAYS} days.");
        return Ok(());
    }

    let colors = Colors::default();
    println!(
        "{bold}{time:<12}  {repo:<14}  {wave:<10}  {label:<38}  {status:<7}  EXEC{reset}",
        bold = colors.bold,
        reset = colors.reset,
        time = "TIME",
        repo = "REPO",
        wave = "WAVE",
        label = "EXEC",
        status = "STATUS",
    );
    for exec in &execs {
        println!(
            "{time:<12}  {repo:<14}  {wave:<10}  {label:<38}  {status:<7}  {id}",
            time = format_time(exec.started),
            repo = truncate(&display_repo(exec.repo.as_deref()), 14),
            wave = truncate(exec.wave.as_deref().unwrap_or("-"), 10),
            label = truncate(&exec.label, 38),
            status = exec.status,
            id = short_id(&exec.id),
        );
    }
    Ok(())
}

/// A capture lifecycle is considered lost beyond 48h: terminal launches whose
/// artifacts vanished that long ago are safe to tombstone, and `capturing`
/// launches still stuck past this point are provably dead. Younger missing
/// captures are reported as candidates to investigate, not swept — the
/// anti-masking line that keeps a live capture regression visible.
const RECONCILE_AGE_GUARD_HOURS: i64 = 48;

/// `lf runs reconcile`: make the ledger and the trace root agree about what
/// survives. Tombstones terminal captures whose conversation artifacts are gone,
/// finalizes orphaned `capturing` launches, and removes artifact directories no
/// launch row claims. Dry-run by default; `--apply` writes. A red `lf doctor`
/// capture check means un-acknowledged loss — this is the explicit
/// acknowledgment.
pub fn reconcile(apply: bool, all: bool, json: bool) -> Result<()> {
    let store = open_ledger().map_err(|err| anyhow!("run ledger unavailable: {err}"))?;
    let launches = store
        .agent_launches_since(0)
        .map_err(|err| anyhow!("failed to read skill launches: {err}"))?;
    let events = store
        .list_run_events_since(0)
        .map_err(|err| anyhow!("failed to read run ledger: {err}"))?;

    let now = chrono::Utc::now().timestamp();
    let plan = plan_reconcile(&launches, &events, now, all, &|path| {
        crate::trace::resolve_artifact(path).is_ok_and(|path| path.is_file())
    });
    let orphans = plan_orphans(&launches, now, all)?;

    if apply {
        for entry in &plan.pruned {
            store.reconcile_launch_capture(&entry.id, "pruned", Some(&entry.reason), now)?;
        }
        for entry in &plan.finalized {
            store.reconcile_launch_capture(&entry.id, "partial", Some(&entry.reason), now)?;
        }
        for orphan in &orphans {
            let path = crate::trace::resolve_artifact(&orphan.artifact_dir)?;
            std::fs::remove_dir_all(&path).map_err(|err| {
                anyhow!("failed to remove orphan artifact {}: {err}", path.display())
            })?;
        }
    }

    let report = ReconcileReport {
        applied: apply,
        pruned: plan.pruned,
        recent_missing: plan.recent_missing,
        finalized: plan.finalized,
        orphans,
    };

    if json {
        println!("{}", serde_json::to_string(&report)?);
        return Ok(());
    }

    let verb = if apply { "applied" } else { "dry-run" };
    let orphan_bytes: u64 = report.orphans.iter().map(|orphan| orphan.bytes).sum();
    println!("reconcile ({verb})");
    println!("  {} pruned", report.pruned.len());
    if !report.recent_missing.is_empty() {
        println!(
            "  {} recent missing — investigate before reconciling (use --all to include)",
            report.recent_missing.len()
        );
    }
    println!("  {} finalized", report.finalized.len());
    println!(
        "  {} orphan artifacts ({})",
        report.orphans.len(),
        format_bytes(orphan_bytes)
    );
    if !apply && (!report.pruned.is_empty() || !report.finalized.is_empty()) {
        println!(
            "run with --apply to tombstone {} and finalize {}.",
            report.pruned.len(),
            report.finalized.len()
        );
    }
    Ok(())
}

/// Artifact directories under the trace root that no launch row claims — the
/// reverse of a dangling reference. Nothing in the ledger points at them, so
/// they are unreadable evidence and unbounded disk.
///
/// The age guard matters here for a different reason than it does for pruning:
/// `begin` writes the directory *before* inserting its row, so a just-created
/// capture is briefly a legitimate orphan. Removing one would delete a live
/// run's transcript out from under it.
fn plan_orphans(
    launches: &[crate::trace::AgentLaunchRow],
    now: i64,
    all: bool,
) -> Result<Vec<OrphanEntry>> {
    let claimed: BTreeSet<&str> = launches
        .iter()
        .map(|launch| launch.artifact_dir.as_str())
        .collect();
    let guard = now - RECONCILE_AGE_GUARD_HOURS * 3600;
    let mut orphans = Vec::new();
    for artifact_dir in crate::trace::list_launch_artifact_dirs()? {
        if claimed.contains(artifact_dir.as_str()) {
            continue;
        }
        let path = crate::trace::resolve_artifact(&artifact_dir)?;
        let (bytes, modified) = directory_size_and_mtime(&path);
        if !all && modified >= guard {
            continue;
        }
        orphans.push(OrphanEntry {
            artifact_dir,
            bytes,
            modified,
        });
    }
    Ok(orphans)
}

/// Total bytes and newest mtime beneath `path`, ignoring entries that vanish or
/// deny access mid-walk — a best-effort measure, never a reason to fail.
fn directory_size_and_mtime(path: &Path) -> (u64, i64) {
    let mut bytes = 0;
    let mut newest = 0;
    let mut stack = vec![path.to_path_buf()];
    while let Some(current) = stack.pop() {
        let Ok(entries) = std::fs::read_dir(&current) else {
            continue;
        };
        for entry in entries.flatten() {
            let Ok(metadata) = entry.metadata() else {
                continue;
            };
            if metadata.is_dir() {
                stack.push(entry.path());
                continue;
            }
            bytes += metadata.len();
            if let Some(modified) = metadata
                .modified()
                .ok()
                .and_then(|time| time.duration_since(std::time::UNIX_EPOCH).ok())
            {
                newest = newest.max(modified.as_secs() as i64);
            }
        }
    }
    (bytes, newest)
}

fn format_bytes(bytes: u64) -> String {
    const UNITS: [&str; 4] = ["B", "KB", "MB", "GB"];
    let mut value = bytes as f64;
    let mut unit = 0;
    while value >= 1024.0 && unit < UNITS.len() - 1 {
        value /= 1024.0;
        unit += 1;
    }
    if unit == 0 {
        format!("{bytes} B")
    } else {
        format!("{value:.1} {}", UNITS[unit])
    }
}

/// The reconcile decision for every launch, derived without touching the store
/// or the clock: `now` is supplied and `artifact_present` resolves a
/// conversation path to "is the file really there". Pure so the age guard and
/// the dead-process rules can be tested directly — the classification is the
/// part that must not be wrong, since `--apply` acts on it.
fn plan_reconcile(
    launches: &[crate::trace::AgentLaunchRow],
    events: &[crate::store::RunEventRow],
    now: i64,
    all: bool,
    artifact_present: &dyn Fn(&str) -> bool,
) -> ReconcilePlan {
    const TERMINAL: [&str; 3] = ["complete", "partial", "prompt_only"];
    let guard = now - RECONCILE_AGE_GUARD_HOURS * 3600;
    let stamp = chrono::DateTime::from_timestamp(now, 0)
        .unwrap_or_default()
        .format("%Y-%m-%dT%H:%M:%SZ")
        .to_string();
    let pruned_reason = format!("conversation artifact absent at reconcile {stamp}");
    let finalized_reason = "capture interrupted; process ended without finalizing";

    let mut plan = ReconcilePlan::default();
    for launch in launches {
        let process_ended = events.iter().any(|event| {
            event.process_id == launch.process_id && event.node == "run" && event.event != "started"
        });
        let stale = launch.started_at < guard;

        if !artifact_present(&launch.conversation_path) {
            // Artifact is gone. Terminal launches are always candidates; a
            // `capturing` launch is a candidate only once its process is
            // provably dead (or stale past the guard) — a live process may
            // still be writing, and `begin` always creates the file, so an
            // absent file on a live `capturing` launch is a transient race,
            // not a tombstone target.
            let dead = TERMINAL.contains(&launch.capture_status.as_str())
                || (launch.capture_status == "capturing" && (process_ended || stale));
            if !dead {
                continue;
            }
            // Age guard: a terminal launch is old when it ended before the
            // guard; a `capturing` launch never set `ended_at`, so fall back to
            // the stale-start check. Without this line, `--apply` during an
            // active capture regression would tombstone the evidence.
            let old = launch.ended_at.is_some_and(|ended| ended < guard) || stale;
            if old || all {
                plan.pruned
                    .push(ReconcileEntry::from_launch(launch, &pruned_reason));
            } else {
                plan.recent_missing
                    .push(ReconcileEntry::from_launch(launch, &pruned_reason));
            }
            continue;
        }

        // File is present. Finalize an orphaned `capturing` launch whose
        // process ended without calling `finish()` — making it terminal and
        // honest rather than stuck.
        if launch.capture_status == "capturing" && (process_ended || stale) {
            plan.finalized
                .push(ReconcileEntry::from_launch(launch, finalized_reason));
        }
    }
    plan
}

#[derive(Debug, Default)]
struct ReconcilePlan {
    pruned: Vec<ReconcileEntry>,
    recent_missing: Vec<ReconcileEntry>,
    finalized: Vec<ReconcileEntry>,
}

#[derive(Debug, serde::Serialize)]
struct ReconcileEntry {
    id: String,
    run_id: String,
    reason: String,
    ended_at: Option<i64>,
}

impl ReconcileEntry {
    fn from_launch(launch: &crate::trace::AgentLaunchRow, reason: &str) -> Self {
        Self {
            id: launch.id.clone(),
            run_id: launch.run_id.clone(),
            reason: reason.to_string(),
            ended_at: launch.ended_at,
        }
    }
}

#[derive(Debug, serde::Serialize)]
struct OrphanEntry {
    artifact_dir: String,
    bytes: u64,
    modified: i64,
}

#[derive(Debug, serde::Serialize)]
struct ReconcileReport {
    applied: bool,
    pruned: Vec<ReconcileEntry>,
    recent_missing: Vec<ReconcileEntry>,
    finalized: Vec<ReconcileEntry>,
    orphans: Vec<OrphanEntry>,
}

/// `lf trace <exec-or-trace-id>`: reconstruct one process tree.
pub fn trace(
    exec_id: &str,
    json: bool,
    content: bool,
    events_mode: bool,
    jsonl: bool,
    launch_prefix: Option<&str>,
    turn_prefix: Option<&str>,
) -> Result<()> {
    if launch_prefix.is_some() && !events_mode && !content {
        return Err(anyhow!("--launch requires --events or --content"));
    }
    let store = open_ledger().map_err(|err| anyhow!("run ledger unavailable: {err}"))?;
    let matches = store
        .run_events_matching_exec(exec_id)
        .map_err(|err| anyhow!("failed to read run ledger: {err}"))?;
    let trace_matches = store
        .run_events_matching(exec_id)
        .map_err(|err| anyhow!("failed to read trace: {err}"))?;
    let trace_id = trace_id_for_address(exec_id, &matches, &trace_matches)?;
    let events = if matches.is_empty() {
        trace_matches
    } else {
        store
            .run_events_matching(&trace_id)
            .map_err(|err| anyhow!("failed to read trace: {err}"))?
    };

    let spans = trace_spans(&events);
    let launches = store.agent_launches_matching(&trace_id)?;
    if events_mode {
        return trace_events(&launches, launch_prefix, jsonl);
    }
    let launch_ids = launches
        .iter()
        .map(|launch| launch.id.clone())
        .collect::<Vec<_>>();
    let turns = store.agent_turns_for_launches(&launch_ids)?;
    if content {
        let dto = trace_content(&launches, &turns, launch_prefix, turn_prefix)?;
        println!("{}", serde_json::to_string(&dto)?);
        return Ok(());
    }
    if json {
        let turn_ids = turns.iter().map(|turn| turn.id.clone()).collect::<Vec<_>>();
        println!(
            "{}",
            serde_json::to_string(&TraceDto {
                trace_id: events[0].run_id.clone(),
                spans,
                launches,
                turns,
                assets: store.context_assets_for_turns(&turn_ids)?,
                decisions: store.context_decisions_for_turns(&turn_ids)?,
            })?
        );
        return Ok(());
    }

    let colors = Colors::default();
    let full_id = events[0].run_id.clone();
    let start = spans.iter().map(|span| span.started_at).min().unwrap_or(0);
    let end = spans.iter().filter_map(|span| span.ended_at).max();

    let header = &events[0];
    println!(
        "{bold}trace {id}{reset}  {repo}{wave}",
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

    for launch in &launches {
        println!(
            "\n  launch    {}  {}{}  {} / {}",
            short_id(&launch.id),
            launch.provider,
            launch
                .model
                .as_deref()
                .map(|model| format!(":{model}"))
                .unwrap_or_default(),
            launch.capture_status,
            launch.outcome,
        );
        if let Some(reason) = &launch.incomplete_reason {
            println!("    incomplete  {reason}");
        }
        if let Some(session_id) = &launch.provider_session_id {
            println!("    session     {session_id}");
        }
        if let Some(session_path) = &launch.provider_session_path {
            let availability = if std::path::Path::new(session_path).exists() {
                "available"
            } else {
                "expired"
            };
            println!("    vendor      {session_path} ({availability})");
        }
        println!(
            "    events      {}",
            crate::trace::resolve_artifact(&launch.conversation_path)?.display()
        );
        for turn in turns.iter().filter(|turn| turn.launch_id == launch.id) {
            println!(
                "    turn {}  {} tokens  provider input {}  {}",
                turn.ordinal,
                turn.supplied_context_tokens,
                turn.provider_input_tokens
                    .map(|value| value.to_string())
                    .unwrap_or_else(|| "-".to_string()),
                turn.status,
            );
            if let Some(system) = &turn.system_prompt_path {
                println!(
                    "      system  {}",
                    crate::trace::resolve_artifact(system)?.display()
                );
            }
            println!(
                "      task    {}",
                crate::trace::resolve_artifact(&turn.task_prompt_path)?.display()
            );
        }
    }

    Ok(())
}

fn trace_id_for_address(
    address: &str,
    exec_matches: &[crate::store::RunEventRow],
    trace_matches: &[crate::store::RunEventRow],
) -> Result<String> {
    let exec_ids = exec_matches
        .iter()
        .map(|event| event.process_id.as_str())
        .collect::<BTreeSet<_>>();
    match exec_ids.len() {
        1 => return Ok(exec_matches[0].run_id.clone()),
        2.. => {
            return Err(anyhow!(
                "exec '{address}' is ambiguous — matches: {}",
                exec_ids
                    .into_iter()
                    .map(short_id)
                    .collect::<Vec<_>>()
                    .join(", ")
            ))
        }
        _ => {}
    }

    let trace_ids = trace_matches
        .iter()
        .map(|event| event.run_id.as_str())
        .collect::<BTreeSet<_>>();
    match trace_ids.len() {
        0 => Err(anyhow!(
            "no exec or trace matching '{address}' in the ledger"
        )),
        1 => Ok(trace_matches[0].run_id.clone()),
        _ => Err(anyhow!(
            "trace '{address}' is ambiguous — matches: {}",
            trace_ids
                .into_iter()
                .map(short_id)
                .collect::<Vec<_>>()
                .join(", ")
        )),
    }
}

#[derive(Debug, serde::Serialize)]
pub struct TraceContentDto {
    pub address: crate::lf::commands::context::TraceAddress,
    pub system_prompt: TraceArtifactDto,
    pub task_prompt: TraceArtifactDto,
    pub conversation: TraceArtifactDto,
}

#[derive(Debug, serde::Serialize)]
pub struct TraceArtifactDto {
    pub path: Option<String>,
    pub content: Option<String>,
    pub unavailable_reason: Option<String>,
}

fn trace_content(
    launches: &[crate::trace::AgentLaunchRow],
    turns: &[crate::trace::AgentTurnRow],
    launch_prefix: Option<&str>,
    turn_prefix: Option<&str>,
) -> Result<TraceContentDto> {
    let selected_launches = launches
        .iter()
        .filter(|launch| launch_prefix.is_none_or(|prefix| launch.id.starts_with(prefix)))
        .collect::<Vec<_>>();
    let launch = match selected_launches.as_slice() {
        [] => return Err(anyhow!("no captured launch matches the requested trace")),
        [launch] => *launch,
        _ if launch_prefix.is_none() => {
            return Err(anyhow!(
                "--content needs --launch when a trace has multiple launches"
            ))
        }
        _ => return Err(anyhow!("launch prefix is ambiguous")),
    };
    let selected_turns = turns
        .iter()
        .filter(|turn| turn.launch_id == launch.id)
        .filter(|turn| turn_prefix.is_none_or(|prefix| turn.id.starts_with(prefix)))
        .collect::<Vec<_>>();
    let turn = match selected_turns.as_slice() {
        [] => return Err(anyhow!("no captured turn matches the requested trace")),
        [turn] => *turn,
        _ if turn_prefix.is_none() => {
            return Err(anyhow!(
                "--content needs --turn when a launch has multiple turns"
            ))
        }
        _ => return Err(anyhow!("turn prefix is ambiguous")),
    };

    Ok(TraceContentDto {
        address: crate::lf::commands::context::TraceAddress {
            run_id: launch.run_id.clone(),
            launch_id: launch.id.clone(),
            turn_id: turn.id.clone(),
        },
        system_prompt: turn.system_prompt_path.as_deref().map_or_else(
            || TraceArtifactDto::unavailable("turn has no system prompt"),
            read_trace_artifact,
        ),
        task_prompt: read_trace_artifact(&turn.task_prompt_path),
        conversation: read_trace_artifact(&launch.conversation_path),
    })
}

impl TraceArtifactDto {
    fn unavailable(reason: &str) -> Self {
        Self {
            path: None,
            content: None,
            unavailable_reason: Some(reason.to_string()),
        }
    }
}

fn read_trace_artifact(relative: &str) -> TraceArtifactDto {
    let path = match crate::trace::resolve_artifact(relative) {
        Ok(path) => path,
        Err(error) => return TraceArtifactDto::unavailable(&error.to_string()),
    };
    match std::fs::read_to_string(&path) {
        Ok(content) => TraceArtifactDto {
            path: Some(path.to_string_lossy().to_string()),
            content: Some(content),
            unavailable_reason: None,
        },
        Err(error) => TraceArtifactDto {
            path: Some(path.to_string_lossy().to_string()),
            content: None,
            unavailable_reason: Some(error.to_string()),
        },
    }
}

#[derive(Debug, serde::Serialize)]
pub struct TraceDto {
    pub trace_id: String,
    pub spans: Vec<SpanDto>,
    pub launches: Vec<crate::trace::AgentLaunchRow>,
    pub turns: Vec<crate::trace::AgentTurnRow>,
    pub assets: Vec<crate::trace::ContextAssetRow>,
    pub decisions: Vec<crate::trace::ContextDecisionRow>,
}

fn trace_events(
    launches: &[crate::trace::AgentLaunchRow],
    launch_prefix: Option<&str>,
    jsonl: bool,
) -> Result<()> {
    let selected = launches
        .iter()
        .filter(|launch| launch_prefix.is_none_or(|prefix| launch.id.starts_with(prefix)))
        .collect::<Vec<_>>();
    if selected.is_empty() {
        return Err(anyhow!("no captured launch matches the requested trace"));
    }
    if launch_prefix.is_some() && selected.len() > 1 {
        return Err(anyhow!("launch prefix is ambiguous"));
    }
    if jsonl && selected.len() > 1 {
        return Err(anyhow!(
            "--jsonl needs --launch when a trace contains multiple launches"
        ));
    }

    for launch in selected {
        let path = crate::trace::resolve_artifact(&launch.conversation_path)?;
        let file = std::fs::File::open(&path).map_err(|error| {
            anyhow!(
                "normalized conversation missing at {}: {error}",
                path.display()
            )
        })?;
        if !jsonl && launches.len() > 1 {
            println!(
                "launch {}  {}  {}",
                short_id(&launch.id),
                launch.provider,
                launch.capture_status
            );
        }
        let mut reader = BufReader::new(file);
        loop {
            let mut line = String::new();
            if reader.read_line(&mut line)? == 0 {
                break;
            }
            if line.trim().is_empty() {
                continue;
            }
            let parsed = serde_json::from_str::<crate::trace::RecordedConversationEvent>(&line);
            if parsed.is_err() && !line.ends_with('\n') {
                break;
            }
            if jsonl {
                print!("{line}");
                continue;
            }
            let event = parsed?;
            print_recorded_event(&event);
        }
    }
    Ok(())
}

fn print_recorded_event(event: &crate::trace::RecordedConversationEvent) {
    use crate::trace::RecordedConversationPayload;

    match &event.payload {
        RecordedConversationPayload::UserInput { op, text } => {
            println!("user ({op})\n{text}\n");
        }
        RecordedConversationPayload::Conversation { event } => {
            println!("{}  {:?}", event.event_type(), event);
        }
        RecordedConversationPayload::LegacyText { stream, text } => {
            println!("{stream}\n{text}\n");
        }
        RecordedConversationPayload::LegacyTool { name, summary } => {
            println!("tool {name}  {summary}");
        }
        RecordedConversationPayload::Usage { usage } => {
            println!(
                "usage  input {} output {} cache {}",
                usage.input_tokens,
                usage.output_tokens,
                usage.cache_read_tokens.unwrap_or(0)
            );
        }
        RecordedConversationPayload::Result { status, .. } => println!("result  {status}"),
        RecordedConversationPayload::CaptureError { message } => {
            println!("capture error  {message}");
        }
    }
}

/// One agent-backed skill launch. This is the shared wire type for `lf runs`
/// and the Runs evidence in `lf status`.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct SkillRunEntry {
    /// Agent launch id: the run's stable identity.
    pub id: String,
    pub trace_id: String,
    pub exec_id: String,
    pub parent_exec_id: Option<String>,
    pub repo: String,
    pub worktree: String,
    pub wave: Option<String>,
    pub flow: Option<String>,
    pub skill: String,
    pub status: String,
    pub started: i64,
    pub ended: Option<i64>,
    pub turns: i64,
    pub system_tokens: i64,
    pub task_tokens: i64,
    pub supplied_context_tokens: i64,
    pub input_tokens: Option<i64>,
    pub output_tokens: Option<i64>,
    pub reasoning_tokens: Option<i64>,
    pub cache_read_tokens: Option<i64>,
    pub cache_write_tokens: Option<i64>,
    pub cost_usd: Option<f64>,
    pub duration_secs: Option<f64>,
    pub provider: String,
    pub model: Option<String>,
    pub surface: String,
    pub capture_status: String,
}

impl SkillRunEntry {
    pub(crate) fn label(&self) -> String {
        self.flow
            .as_deref()
            .filter(|flow| *flow != self.skill.as_str())
            .map(|flow| format!("{flow}/{}", self.skill))
            .unwrap_or_else(|| self.skill.clone())
    }

    pub(crate) fn total_tokens(&self) -> i64 {
        self.input_tokens.unwrap_or(0) + self.output_tokens.unwrap_or(0)
    }

    fn is_active(&self) -> bool {
        self.ended.is_none()
    }
}

/// One `lf` process folded out of `run_events`. `lf execs` keeps this diagnostic
/// grain separate from the skill runs that own model evidence.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct ExecLedgerEntry {
    /// Exec/process id: the stable identity shown by `lf execs`.
    pub id: String,
    pub trace_id: String,
    pub parent_exec_id: Option<String>,
    pub repo: Option<String>,
    pub wave: Option<String>,
    pub label: String,
    pub status: String,
    pub started: i64,
    pub ended: Option<i64>,
}

/// The skill runs one Wave produced, newest first.
pub(crate) fn wave_runs(wave: &str) -> Result<(Vec<SkillRunEntry>, bool)> {
    let store = open_ledger().map_err(|err| anyhow!("run ledger unavailable: {err}"))?;
    let since = chrono::Utc::now().timestamp() - WINDOW_DAYS * 24 * 3600;
    let events = store
        .list_run_events_since(since)
        .map_err(|err| anyhow!("failed to read run ledger: {err}"))?;
    let launches = store
        .agent_launches_since(since)
        .map_err(|err| anyhow!("failed to read skill launches: {err}"))?
        .into_iter()
        .filter(|launch| launch.wave.as_deref() == Some(wave) && launch.skill.is_some())
        .collect::<Vec<_>>();
    let launch_ids = launches
        .iter()
        .map(|launch| launch.id.clone())
        .collect::<Vec<_>>();
    let turns = store
        .agent_turns_for_launches(&launch_ids)
        .map_err(|err| anyhow!("failed to read run turns: {err}"))?;

    let mut runs = summarize_runs(&events, &launches, &turns);
    sort_runs(&mut runs);
    let truncated = cap_runs(&mut runs);
    Ok((runs, truncated))
}

fn sort_runs(runs: &mut [SkillRunEntry]) {
    runs.sort_by(|left, right| {
        right
            .started
            .cmp(&left.started)
            .then_with(|| right.id.cmp(&left.id))
    });
}

fn cap_runs(runs: &mut Vec<SkillRunEntry>) -> bool {
    let truncated = runs.len() > MAX_RUNS;
    if !truncated {
        return false;
    }
    let active = runs.iter().filter(|run| run.is_active()).count();
    let mut budget = MAX_RUNS.saturating_sub(active);
    runs.retain(|run| {
        if run.is_active() {
            return true;
        }
        if budget == 0 {
            return false;
        }
        budget -= 1;
        true
    });
    true
}

fn summarize_runs(
    events: &[RunEventRow],
    launches: &[crate::trace::AgentLaunchRow],
    turns: &[crate::trace::AgentTurnRow],
) -> Vec<SkillRunEntry> {
    let process_parents = events
        .iter()
        .map(|event| (event.process_id.as_str(), event.parent_process_id.clone()))
        .collect::<BTreeMap<_, _>>();
    let mut turns_by_launch: BTreeMap<&str, Vec<&crate::trace::AgentTurnRow>> = BTreeMap::new();
    for turn in turns {
        turns_by_launch
            .entry(turn.launch_id.as_str())
            .or_default()
            .push(turn);
    }

    launches
        .iter()
        .filter_map(|launch| {
            let skill = launch.skill.clone()?;
            let turns = turns_by_launch
                .get(launch.id.as_str())
                .map(Vec::as_slice)
                .unwrap_or_default();
            Some(SkillRunEntry {
                id: launch.id.clone(),
                trace_id: launch.run_id.clone(),
                exec_id: launch.process_id.clone(),
                parent_exec_id: process_parents
                    .get(launch.process_id.as_str())
                    .cloned()
                    .flatten(),
                repo: launch.repo.clone(),
                worktree: launch.worktree.clone(),
                wave: launch.wave.clone(),
                flow: launch.flow.clone(),
                skill,
                status: launch_status_label(&launch.outcome).to_string(),
                started: launch.started_at,
                ended: launch.ended_at,
                turns: turns.len() as i64,
                system_tokens: turns.iter().map(|turn| turn.system_tokens).sum(),
                task_tokens: turns.iter().map(|turn| turn.task_tokens).sum(),
                supplied_context_tokens: turns
                    .iter()
                    .map(|turn| turn.supplied_context_tokens)
                    .sum(),
                input_tokens: sum_optional_i64(turns.iter().map(|turn| turn.provider_input_tokens)),
                output_tokens: sum_optional_i64(
                    turns.iter().map(|turn| turn.provider_output_tokens),
                ),
                reasoning_tokens: sum_optional_i64(turns.iter().map(|turn| turn.reasoning_tokens)),
                cache_read_tokens: sum_optional_i64(
                    turns.iter().map(|turn| turn.cache_read_tokens),
                ),
                cache_write_tokens: sum_optional_i64(
                    turns.iter().map(|turn| turn.cache_write_tokens),
                ),
                cost_usd: sum_optional_f64(turns.iter().map(|turn| turn.cost_usd)),
                duration_secs: launch
                    .ended_at
                    .map(|ended| ended.saturating_sub(launch.started_at).max(0) as f64),
                provider: launch.provider.clone(),
                model: launch.model.clone(),
                surface: launch.surface.clone(),
                capture_status: launch.capture_status.clone(),
            })
        })
        .collect()
}

fn sum_optional_i64(values: impl Iterator<Item = Option<i64>>) -> Option<i64> {
    values.fold(None, |total, value| match (total, value) {
        (None, None) => None,
        (total, value) => Some(total.unwrap_or(0) + value.unwrap_or(0)),
    })
}

fn sum_optional_f64(values: impl Iterator<Item = Option<f64>>) -> Option<f64> {
    values.fold(None, |total, value| match (total, value) {
        (None, None) => None,
        (total, value) => Some(total.unwrap_or(0.0) + value.unwrap_or(0.0)),
    })
}

fn summarize_execs(events: &[RunEventRow]) -> Vec<ExecLedgerEntry> {
    let mut by_process: BTreeMap<&str, Vec<&RunEventRow>> = BTreeMap::new();
    for event in events {
        by_process.entry(&event.process_id).or_default().push(event);
    }

    by_process
        .into_iter()
        .map(|(process_id, events)| {
            let terminal = events
                .iter()
                .filter(|e| e.node == "run" && e.event != "started")
                .max_by_key(|event| event.seq);
            ExecLedgerEntry {
                id: process_id.to_string(),
                trace_id: events[0].run_id.clone(),
                parent_exec_id: events[0].parent_process_id.clone(),
                repo: events.iter().find_map(|e| e.repo.clone()),
                wave: events.iter().find_map(|e| e.wave.clone()),
                label: events
                    .iter()
                    .find_map(|e| e.command.as_deref().and_then(command_label))
                    .or_else(|| events.iter().find_map(|e| e.flow.clone()))
                    .or_else(|| events.iter().find_map(|e| e.skill.clone()))
                    .unwrap_or_else(|| "-".to_string()),
                started: events.iter().map(|e| e.ts).min().unwrap_or(0),
                ended: terminal.map(|e| e.ts),
                status: terminal
                    .map(|e| status_label(&e.event))
                    .unwrap_or("running")
                    .to_string(),
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
    /// Event position within the process. Together with `process_id`, this is
    /// the stable identity of a boundary row.
    pub seq: i64,
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
            process_events.sort_by_key(|event| event.seq);
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
                seq: started.seq,
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
    // own_spend diffs against the previous boundary in the same process. `seq`
    // is the production order; wall time can jump backwards under clock sync.
    rows.sort_by_key(|event| (event.process_id.as_str(), event.seq));

    rows.into_iter()
        .map(|event| SpanDto {
            run_id: event.run_id.clone(),
            process_id: event.process_id.clone(),
            parent_process_id: event.parent_process_id.clone(),
            seq: event.seq,
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

fn launch_status_label(outcome: &str) -> &'static str {
    match outcome {
        "completed" => "ok",
        "failed" | "interrupted" => "error",
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

pub(crate) fn format_tokens(value: i64) -> String {
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

#[cfg(test)]
mod tests {
    use super::{
        boundary_spans, format_duration, format_tokens, own_spend, plan_orphans, plan_reconcile,
        summarize_execs, trace_id_for_address, trace_spans, SpanDto,
    };
    use crate::store::RunEventRow;
    use crate::trace::AgentLaunchRow;

    const NOW: i64 = 1_800_000_000;
    const HOUR: i64 = 3_600;

    /// A launch whose conversation artifact is named but whose presence the
    /// test decides via the `artifact_present` closure.
    fn launch(
        id: &str,
        capture_status: &str,
        started_at: i64,
        ended_at: Option<i64>,
    ) -> AgentLaunchRow {
        AgentLaunchRow {
            id: id.to_string(),
            run_id: format!("run-{id}"),
            process_id: id.to_string(),
            started_at,
            ended_at,
            repo: "/src/loopflow".to_string(),
            worktree: "/src/loopflow".to_string(),
            wave: None,
            flow: None,
            skill: None,
            project: None,
            task: None,
            provider: "codex".to_string(),
            model: None,
            surface: "headless".to_string(),
            capture_status: capture_status.to_string(),
            incomplete_reason: None,
            outcome: "completed".to_string(),
            artifact_dir: format!("{id}/dir"),
            conversation_path: format!("{id}/conversation.jsonl"),
            provider_events_path: None,
            provider_session_id: None,
            provider_session_path: None,
            conversation_event_count: 1,
            conversation_bytes: 10,
        }
    }

    fn absent(_: &str) -> bool {
        false
    }

    fn present(_: &str) -> bool {
        true
    }

    #[test]
    fn a_long_gone_terminal_capture_is_tombstoned() {
        let launches = vec![launch(
            "old",
            "complete",
            NOW - 200 * HOUR,
            Some(NOW - 100 * HOUR),
        )];

        let plan = plan_reconcile(&launches, &[], NOW, false, &absent);

        assert_eq!(plan.pruned.len(), 1, "{plan:?}");
        assert_eq!(plan.pruned[0].id, "old");
        assert!(
            plan.pruned[0]
                .reason
                .contains("conversation artifact absent"),
            "{}",
            plan.pruned[0].reason
        );
        assert!(plan.recent_missing.is_empty(), "{plan:?}");
    }

    #[test]
    fn a_recently_missing_capture_is_reported_not_swept() {
        // The anti-masking line: a capture that vanished an hour ago is
        // evidence of a live regression, not history to acknowledge.
        let launches = vec![launch(
            "fresh",
            "complete",
            NOW - 2 * HOUR,
            Some(NOW - HOUR),
        )];

        let plan = plan_reconcile(&launches, &[], NOW, false, &absent);

        assert!(
            plan.pruned.is_empty(),
            "fresh loss must not tombstone: {plan:?}"
        );
        assert_eq!(plan.recent_missing.len(), 1, "{plan:?}");
        assert_eq!(plan.recent_missing[0].id, "fresh");
    }

    #[test]
    fn all_overrides_the_age_guard_for_recent_losses() {
        let launches = vec![launch(
            "fresh",
            "complete",
            NOW - 2 * HOUR,
            Some(NOW - HOUR),
        )];

        let plan = plan_reconcile(&launches, &[], NOW, true, &absent);

        assert_eq!(plan.pruned.len(), 1, "{plan:?}");
        assert!(plan.recent_missing.is_empty(), "{plan:?}");
    }

    #[test]
    fn an_orphaned_capturing_launch_with_its_file_is_finalized_partial() {
        // Interrupted run: `begin` wrote the file, the process died before
        // `finish`. The file is here, so this is finalization, not a tombstone.
        let launches = vec![launch("stuck", "capturing", NOW - HOUR, None)];
        let events = vec![row("stuck", 1, NOW - HOUR, "run", "completed")];

        let plan = plan_reconcile(&launches, &events, NOW, false, &present);

        assert_eq!(plan.finalized.len(), 1, "{plan:?}");
        assert_eq!(plan.finalized[0].id, "stuck");
        assert!(
            plan.finalized[0].reason.contains("capture interrupted"),
            "{}",
            plan.finalized[0].reason
        );
        assert!(plan.pruned.is_empty(), "{plan:?}");
    }

    #[test]
    fn a_live_capturing_launch_is_left_alone() {
        // Still running, no terminal run event, inside the guard: the file may
        // be mid-write and the row is not ours to touch.
        let launches = vec![launch("live", "capturing", NOW - HOUR, None)];

        let plan = plan_reconcile(&launches, &[], NOW, false, &absent);

        assert!(plan.pruned.is_empty(), "{plan:?}");
        assert!(plan.recent_missing.is_empty(), "{plan:?}");
        assert!(plan.finalized.is_empty(), "{plan:?}");
    }

    #[test]
    fn an_intact_terminal_capture_needs_no_reconciliation() {
        let launches = vec![launch(
            "good",
            "complete",
            NOW - 200 * HOUR,
            Some(NOW - 100 * HOUR),
        )];

        let plan = plan_reconcile(&launches, &[], NOW, false, &present);

        assert!(plan.pruned.is_empty(), "{plan:?}");
        assert!(plan.finalized.is_empty(), "{plan:?}");
    }

    /// Write an artifact directory under the trace root with no launch row
    /// claiming it, returning its relative path.
    fn orphan_dir(guard: &crate::journal::TestLedgerGuard, rel: &str) -> String {
        let dir = guard.home().join("traces").join(rel);
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("conversation.jsonl"), b"{\"seq\":0}\n").unwrap();
        rel.to_string()
    }

    #[test]
    fn an_unclaimed_artifact_directory_is_an_orphan_only_once_it_ages_out() {
        // `begin` writes the directory before inserting its row, so a fresh
        // unclaimed directory may be a live capture mid-flight — removing it
        // would delete a running transcript.
        let guard = crate::journal::TestLedgerGuard::new();
        let rel = orphan_dir(&guard, "run-x/proc-x/launch-x");
        let written = std::fs::metadata(guard.home().join("traces").join(&rel))
            .unwrap()
            .modified()
            .unwrap()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs() as i64;

        assert!(
            plan_orphans(&[], written + HOUR, false).unwrap().is_empty(),
            "a just-written orphan must be left alone"
        );

        let forced = plan_orphans(&[], written + HOUR, true).unwrap();
        assert_eq!(forced.len(), 1, "{forced:?}");
        assert_eq!(forced[0].artifact_dir, rel);
        assert!(forced[0].bytes > 0, "{forced:?}");

        let aged = plan_orphans(&[], written + 72 * HOUR, false).unwrap();
        assert_eq!(aged.len(), 1, "past the guard it needs no --all: {aged:?}");
    }

    #[test]
    fn an_artifact_directory_a_launch_claims_is_never_an_orphan() {
        let guard = crate::journal::TestLedgerGuard::new();
        let rel = orphan_dir(&guard, "run-y/proc-y/launch-y");
        let mut claimed = launch("launch-y", "complete", NOW - 200 * HOUR, None);
        claimed.artifact_dir = rel;

        let orphans = plan_orphans(&[claimed], NOW, true).unwrap();

        assert!(orphans.is_empty(), "{orphans:?}");
    }

    #[test]
    fn an_already_pruned_launch_is_not_reconciled_twice() {
        // Idempotence: re-running reconcile must not re-tombstone a tombstone.
        let launches = vec![launch(
            "done",
            "pruned",
            NOW - 200 * HOUR,
            Some(NOW - 100 * HOUR),
        )];

        let plan = plan_reconcile(&launches, &[], NOW, false, &absent);

        assert!(plan.pruned.is_empty(), "{plan:?}");
        assert!(plan.recent_missing.is_empty(), "{plan:?}");
        assert!(plan.finalized.is_empty(), "{plan:?}");
    }

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
    fn summarize_execs_folds_process_events_into_one_summary() {
        let terminal = row("abc", 1, 110, "run", "completed");
        let events = vec![row("abc", 0, 100, "run", "started"), terminal];

        let summaries = summarize_execs(&events);
        assert_eq!(summaries.len(), 1);
        let run = &summaries[0];
        assert_eq!(run.trace_id, "abc");
        assert_eq!(run.started, 100);
        assert_eq!(run.ended, Some(110));
        assert_eq!(run.status, "ok");
        assert_eq!(run.label, "gate"); // from command argv
    }

    #[test]
    fn summarize_execs_marks_unterminated_processes_as_running() {
        let events = vec![row("abc", 0, 100, "run", "started")];
        let summaries = summarize_execs(&events);
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

        let mut child_start = row("66863649", 0, 105, "run", "started");
        child_start.process_id = "child".to_string();
        child_start.parent_process_id = Some("parent".to_string());
        child_start.command = Some(r#"["lf","pm","show"]"#.to_string());
        let mut child_end = child_start.clone();
        child_end.seq = 1;
        child_end.ts = 110;
        child_end.event = "errored".to_string();

        let summaries = summarize_execs(&[parent_start, child_start, child_end, parent_end]);
        assert_eq!(summaries.len(), 2);
        let parent = summaries
            .iter()
            .find(|summary| summary.id == "parent")
            .unwrap();
        let child = summaries
            .iter()
            .find(|summary| summary.id == "child")
            .unwrap();
        assert_eq!(parent.label, "wave intel");
        assert_eq!(child.label, "pm show");
        assert_eq!(child.status, "error");
    }

    #[test]
    fn trace_addresses_accept_the_run_id_carried_by_context_evidence() {
        let events = vec![row("trace-address", 0, 100, "run", "started")];

        let trace_id = trace_id_for_address("trace-add", &[], &events).unwrap();

        assert_eq!(trace_id, "trace-address");
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
    fn boundary_spend_follows_process_sequence_when_wall_time_moves_backwards() {
        let mut first = row("trace", 1, 200, "skill", "completed");
        first.input_tokens = Some(100);
        let mut second = row("trace", 2, 100, "run", "completed");
        second.input_tokens = Some(150);

        let spend = own_spend(&boundary_spans(&[first, second]));
        assert_eq!(spend[0].seq, 1);
        assert_eq!(spend[0].input_tokens, Some(100));
        assert_eq!(spend[1].seq, 2);
        assert_eq!(spend[1].input_tokens, Some(50));
    }

    #[test]
    fn own_spend_diffs_consecutive_boundaries_within_a_process() {
        let boundary = |cost, input| SpanDto {
            run_id: "trace".to_string(),
            process_id: "span".to_string(),
            parent_process_id: None,
            seq: input,
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
        let terminal = row("abc", 1, 110, "run", "completed");
        let events = vec![row("abc", 0, 100, "run", "started"), terminal];
        let value = serde_json::to_value(&summarize_execs(&events)[0]).expect("serialize");
        assert_eq!(value["id"], "abc");
        assert_eq!(value["status"], "ok");
        assert_eq!(value["started"], 100);
        assert_eq!(value["ended"], 110);
        assert_eq!(value["label"], "gate");
        // Explicitly-Optional fields stay present (a running run's `ended` is
        // null, never absent) — one stable shape for the client.
        let running = summarize_execs(&[row("xyz", 0, 100, "run", "started")]);
        let value = serde_json::to_value(&running[0]).expect("serialize");
        assert_eq!(value["ended"], serde_json::Value::Null);
        assert_eq!(value["status"], "running");
    }

    #[test]
    fn repeated_skill_boundaries_have_distinct_wire_identity() {
        let mut events = vec![
            row("trace", 1, 100, "skill", "completed"),
            row("trace", 2, 100, "skill", "completed"),
        ];
        for event in &mut events {
            event.skill = Some("implement".to_string());
            event.input_tokens = Some(10 * event.seq);
        }

        let boundaries = boundary_spans(&events);
        assert_eq!(boundaries.len(), 2);
        assert_ne!(boundaries[0].seq, boundaries[1].seq);
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
