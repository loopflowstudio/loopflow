//! `lf runs` — read Home-local Run records.

use std::{
    io::Read,
    path::{Path, PathBuf},
};

use anyhow::{anyhow, Result};

use crate::lf::commands::util::short_id;
use crate::lf::commands::WorkFilter;
use crate::lf::output::{format_cost, truncate, Colors};
pub use crate::run_record::active::{ActiveRun, ActiveRunsSnapshot, DiscoveryState};
pub use crate::run_record::{AttributionSource, RunSnapshot, RunUsage, SubjectAttribution};

const WINDOW_DAYS: i64 = 7;
const MAX_RUNS: usize = 50;

pub fn list_active(json: bool, watch: bool, task: Option<&str>) -> Result<()> {
    let runtime = tokio::runtime::Runtime::new()?;
    let (home, store, task) = runtime.block_on(async {
        let home = crate::store::observability_home_dir();
        let config =
            crate::store::StorageConfig::sqlite(crate::store::observability_database_path()?);
        let store = std::sync::Arc::new(crate::store::open_store(&config).await?);
        let task = match task {
            Some(task) => Some(crate::durable::WorkRef::Task(
                store
                    .get_task_by_issue(task)
                    .await?
                    .ok_or_else(|| anyhow!("Task {task:?} is not registered"))?
                    .id,
            )),
            None => None,
        };
        Ok::<_, anyhow::Error>((home, store, task))
    })?;
    if watch {
        return super::runs_watch::run(&home, &store, task, &runtime);
    }
    runtime.block_on(async {
        let snapshot = crate::run_record::active::snapshot(&home, &store, task).await;
        if json {
            println!("{}", serde_json::to_string(&snapshot)?);
        } else {
            for run in &snapshot.runs {
                println!("{}  {}  {}", run.id, run.harness, run.label);
            }
            for gap in &snapshot.gaps {
                println!("Unavailable: {gap}");
            }
            if snapshot.runs.is_empty() && snapshot.gaps.is_empty() {
                println!("No active Runs.");
            }
        }
        Ok(())
    })
}

/// The Runs matching a filter, newest first, capped. One reader behind
/// `lf runs`, its Work drills, and `lf wave status`'s Runs evidence, so the surfaces
/// can never disagree on what a run is.
pub(crate) fn collect_runs(filter: WorkFilter) -> Result<(Vec<RunSnapshot>, bool)> {
    let since = chrono::Utc::now().timestamp() - WINDOW_DAYS * 24 * 3600;
    let mut runs = collect_runs_started_since(filter, since)?;
    let truncated = cap_runs(&mut runs);
    Ok((runs, truncated))
}

pub(crate) fn collect_runs_started_since(
    filter: WorkFilter,
    since: i64,
) -> Result<Vec<RunSnapshot>> {
    let path = crate::store::observability_database_path()?;
    collect_runs_at(
        &crate::store::observability_home_dir(),
        &path,
        filter,
        None,
        since,
    )
}

/// Select conversations in SQL before reading their subordinate history.
fn collect_runs_at(
    _lf_home: &Path,
    database: &Path,
    filter: WorkFilter,
    parent: Option<&str>,
    since: i64,
) -> Result<Vec<RunSnapshot>> {
    if !database.exists() {
        return Ok(Vec::new());
    }
    let store = crate::store::sqlite::SqliteStore::open_run_ledger_read_only(database)?;
    Ok(store
        .conversation_snapshots(
            filter.wave,
            filter.project,
            filter.task,
            parent,
            since,
            false,
        )?
        .into_iter()
        .map(|(_, snapshot)| snapshot)
        .collect())
}

/// `lf runs [--wave <name>] [--project <slug>] [--task <id>]`: recent harness
/// launches, optionally drilled to one attributed Work subject.
pub fn list(
    json: bool,
    wave: Option<&str>,
    project: Option<&str>,
    task: Option<&str>,
    parent: Option<&str>,
) -> Result<()> {
    let home = crate::store::observability_home_dir();
    let filter = WorkFilter {
        wave,
        project,
        task,
    };
    // A Task's Runs and a Run's children list whole; other drills are recent.
    let runs = match (parent, task) {
        (Some(parent), _) => {
            let database = crate::store::observability_database_path()?;
            let store = crate::store::sqlite::SqliteStore::open_run_ledger_read_only(&database)?;
            let parent = store.resolve_history_input(parent)?;
            collect_runs_at(&home, &database, filter, Some(&parent), 0)?
        }
        (None, Some(_)) => collect_runs_started_since(filter, 0)?,
        (None, None) => collect_runs(filter)?.0,
    };

    if json {
        println!("{}", serde_json::to_string(&runs)?);
        return Ok(());
    }

    if runs.is_empty() {
        match (parent, wave, project, task) {
            (Some(parent), _, _, _) => println!("No child Runs recorded for {parent}."),
            (None, _, _, Some(task)) => {
                println!("No Runs recorded for {task}.")
            }
            (None, _, Some(project), None) => {
                println!("No Runs recorded for project/{project} in the last {WINDOW_DAYS} days.")
            }
            (None, Some(wave), None, None) => {
                println!("No Runs recorded for wave/{wave} in the last {WINDOW_DAYS} days.")
            }
            (None, None, None, None) => {
                println!("No Runs recorded in the last {WINDOW_DAYS} days.")
            }
        }
        return Ok(());
    }

    let colors = Colors::default();
    println!(
        "{bold}{time:<12}  {repo:<14}  {wave:<10}  {label:<22}  {tokens:>10}  {cost:>8}  {agent:<18}  {status:<12}  RUN{reset}",
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
    for run in &runs {
        println!(
            "{time:<12}  {repo:<14}  {wave:<10}  {label:<22}  {tokens:>10}  {cost:>8}  {agent:<18}  {status:<12}  {id}",
            time = format_time(run.started),
            repo = truncate(&display_repo(run.repo.as_deref()), 14),
            wave = truncate(run.subject("wave").unwrap_or("-"), 10),
            label = truncate(run.label(), 22),
            tokens = run
                .total_tokens()
                .map(format_tokens)
                .unwrap_or_else(|| "-".to_string()),
            cost = run
                .usage
                .cost_usd
                .map(format_cost)
                .unwrap_or_else(|| "-".to_string()),
            agent = truncate(&format_agent(Some(&run.harness), run.model.as_deref()), 18),
            status = run.status(),
            id = short_id(&run.id),
        );
    }
    Ok(())
}

pub fn observe_provider_session() -> Result<()> {
    let run_dir = std::env::var_os(crate::run_record::RUN_DIR_ENV)
        .map(PathBuf::from)
        .ok_or_else(|| anyhow!("provider session callback has no active Run"))?;
    let mut payload = String::new();
    std::io::stdin().read_to_string(&mut payload)?;
    let payload: serde_json::Value = serde_json::from_str(&payload)
        .map_err(|error| anyhow!("invalid provider session callback: {error}"))?;
    let provider_session_id = payload
        .get("session_id")
        .and_then(serde_json::Value::as_str)
        .filter(|session_id| !session_id.is_empty())
        .ok_or_else(|| anyhow!("provider session callback has no session_id"))?;
    let account_id = std::env::var(crate::run_record::PROVIDER_ACCOUNT_ID_ENV)
        .ok()
        .map(|value| crate::store::ProviderAccountId::parse(&value))
        .transpose()
        .map_err(|error| anyhow!("invalid provider account in session callback: {error}"))?;
    crate::run_record::write_provider_session(&run_dir, provider_session_id, account_id)
        .map_err(|error| anyhow!("cannot preserve provider session: {error}"))
}

pub fn inspect(selector: &str, events: bool, final_answer: bool, json: bool) -> Result<()> {
    let home = crate::store::observability_home_dir();
    let database = crate::store::observability_database_path()?;
    let store = crate::store::sqlite::SqliteStore::open_run_ledger_read_only(&database)?;
    let snapshot = store
        .input_snapshot(selector)
        .map_err(|error| anyhow!("Run record unavailable: {error}"))?;
    let input = crate::durable::RunId::parse(&snapshot.id)?;
    let dir = crate::run_record::record_dir(&home, &input)
        .ok_or_else(|| anyhow!("Input {} has no artifact path", snapshot.id))?;
    if events {
        for event in store.input_events(&input)? {
            match event.get("unparsed").and_then(serde_json::Value::as_str) {
                Some(bytes) => println!("{bytes}"),
                None => println!("{}", serde_json::to_string(&event)?),
            }
        }
        return Ok(());
    }
    if final_answer {
        let answer = store
            .input_final_answer(&input)
            .map_err(|error| anyhow!("Run final answer unavailable: {error}"))?;
        return match answer {
            Some(answer) => {
                if !answer.exact {
                    eprintln!(
                        "warning: this Run has no final-answer receipt; showing all streamed prose from its last completed provider turn"
                    );
                }
                println!("{}", answer.text);
                Ok(())
            }
            None if snapshot.outcome.is_none() => Err(anyhow!(
                "Run {} is not settled and has no final answer",
                snapshot.id
            )),
            None => Err(anyhow!(
                "Run {} settled as {} without a final answer",
                snapshot.id,
                snapshot.outcome.as_deref().unwrap_or("unknown")
            )),
        };
    }
    if json {
        println!("{}", serde_json::to_string(&snapshot)?);
        return Ok(());
    }
    println!("Run {}", snapshot.id);
    if let Some(parent) = &snapshot.parent_run_id {
        println!("Parent: {parent}");
    }
    println!("Status: {}", snapshot.status());
    println!(
        "Agent: {}",
        format_agent(Some(&snapshot.harness), snapshot.model.as_deref())
    );
    println!(
        "Working directory: {}",
        snapshot.worktree.as_deref().unwrap_or("unknown")
    );
    let manifest = crate::run_record::read_manifest(&dir).ok();
    println!(
        "Replay: {}",
        match manifest
            .as_ref()
            .and_then(|manifest| manifest.launch.as_ref())
        {
            Some(launch) if launch.replay_unavailable_reason().is_none() => "available",
            Some(_) | None => "unavailable",
        }
    );
    println!("Evidence gaps: {}", snapshot.evidence_gaps);
    Ok(())
}

fn cap_runs(runs: &mut Vec<RunSnapshot>) -> bool {
    let truncated = runs.len() > MAX_RUNS;
    if !truncated {
        return false;
    }
    let unterminated = runs.iter().filter(|run| run.is_unterminated()).count();
    let mut budget = MAX_RUNS.saturating_sub(unterminated);
    runs.retain(|run| {
        if run.is_unterminated() {
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
    use super::format_tokens;

    #[test]
    fn history_window_keeps_boundary_usage_and_gaps_before_payload_reads() {
        use crate::engine::stream::StreamEvent;
        use crate::run_record::{CaptureHandle, RunFlowMembership, RunSpec};
        let ledger = crate::journal::TestLedgerGuard::new();
        let home = ledger.home();
        let database = home.join("loopflow.db");
        crate::store::sqlite::SqliteStore::open_ephemeral(&database).unwrap();
        let since = 1_790_000_000;
        let parent = crate::durable::RunId::new();
        let mut older = None;
        for started in [since - 1, since, since + 1] {
            let capture = CaptureHandle::begin_at(
                home,
                RunSpec {
                    harness: "proof".into(),
                    model: None,
                    surface: "headless".into(),
                    cwd: home.to_owned(),
                    repo: None,
                    worktree: None,
                    skill: None,
                    subjects: Vec::new(),
                    flow: RunFlowMembership::Independent,
                    work: None,
                },
            )
            .unwrap();
            capture.record_stream_event(&StreamEvent::Usage {
                input_tokens: Some(12),
                output_tokens: Some(3),
                cache_read_tokens: None,
            });
            capture.finish("completed").unwrap();
            let dir = capture.artifact_dir();
            let input = capture.run_id();
            // Simulate dated retained history on the final owners.
            let conn = rusqlite::Connection::open(&database).unwrap();
            conn.execute(
                "UPDATE session_events SET observed_at=?2 WHERE receipt_key=?1||':manifest.json'",
                rusqlite::params![input.as_str(), started],
            )
            .unwrap();
            conn.execute(
                "UPDATE agent_sessions SET created_at=?2 WHERE input_id=?1",
                rusqlite::params![input.as_str(), started],
            )
            .unwrap();
            conn.execute(
                "UPDATE agent_session_inputs SET caller_input_id=?2 WHERE input_id=?1",
                rusqlite::params![
                    input.as_str(),
                    (started >= since).then_some(parent.as_str())
                ],
            )
            .unwrap();
            if started == since {
                conn.execute("INSERT INTO session_events(session_id,kind,receipt_key,observed_at,payload)
                    SELECT id,'observed',?1||':events.jsonl:partial',?2,?3 FROM agent_sessions WHERE input_id=?1",
                    rusqlite::params![input.as_str(),since,serde_json::json!({"input_id": input,"source":"events.jsonl:partial","evidence":{"unparsed":"{"}}).to_string()]).unwrap();
            }
            if started < since {
                older = Some(input);
            }
            // The SQL history remains useful after the input payload is unavailable.
            std::fs::remove_dir_all(dir).unwrap();
        }
        let select = |since| {
            super::collect_runs_at(
                home,
                &database,
                super::WorkFilter {
                    wave: None,
                    project: None,
                    task: None,
                },
                None,
                since,
            )
            .unwrap()
        };
        let all = select(0);
        let selected = select(since);
        assert_eq!(all.len(), 3);
        assert_eq!(selected.len(), 2);
        assert_eq!(selected[0].started, since + 1);
        assert_eq!(selected[1].started, since);
        assert_eq!(selected[1].usage.input_tokens, Some(12));
        assert_eq!(selected[1].evidence_gaps, 1);
        assert_eq!(
            selected,
            all.into_iter()
                .filter(|run| run.started >= since)
                .collect::<Vec<_>>()
        );
        let store =
            crate::store::sqlite::SqliteStore::open_run_ledger_read_only(&database).unwrap();
        assert_eq!(
            store
                .conversation_snapshots(None, None, None, None, since, true)
                .unwrap()
                .len(),
            3,
            "activity retains the older work that ended inside the window"
        );
        store.assert_no_historical_runs();
        // An excluded corrupt payload must not make the recent window fail.
        rusqlite::Connection::open(&database).unwrap().execute(
            "UPDATE session_events SET payload='{' WHERE session_id=(SELECT id FROM agent_sessions WHERE input_id=?1)",
            [older.unwrap().as_str()],
        ).unwrap();
        assert_eq!(select(since), selected);
        // Exact parent selection also precedes payload hydration, without a date cap.
        assert_eq!(
            super::collect_runs_at(
                home,
                &database,
                super::WorkFilter::default(),
                Some(parent.as_str()),
                0
            )
            .unwrap(),
            selected
        );
    }

    #[test]
    fn tokens_keep_the_compact_human_format() {
        assert_eq!(format_tokens(184_000), "184.0k");
        assert_eq!(format_tokens(0), "");
    }
}
