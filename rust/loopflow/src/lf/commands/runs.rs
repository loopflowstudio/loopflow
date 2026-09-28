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
/// `lf runs`, its Work drills, and `lf status`'s Runs evidence, so the surfaces
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
    let mut runs = collect_runs_at(
        &crate::store::observability_home_dir(),
        &path,
        filter,
        None,
        since,
    )?;
    runs.retain(|run| run.started >= since);
    Ok(runs)
}

/// Runs are rows: one query selects them, and each Run's usage and launch
/// evidence are read from its own record.
fn collect_runs_at(
    lf_home: &Path,
    database: &Path,
    filter: WorkFilter,
    parent: Option<&str>,
    since: i64,
) -> Result<Vec<RunSnapshot>> {
    if !database.exists() {
        return Ok(Vec::new());
    }
    let store = crate::store::sqlite::SqliteStore::open_run_ledger_read_only(database)?;
    let runs = store.runs(filter.wave, filter.project, filter.task, parent, since)?;
    let runs = crate::run_record::run_snapshots(lf_home, runs)
        .map_err(|error| anyhow!("Run record unavailable: {error}"))?;
    Ok(runs.into_iter().map(|(_, snapshot)| snapshot).collect())
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
            let (_, manifest) = crate::run_record::resolve_manifest(&home, parent)
                .map_err(|error| anyhow!("Run record unavailable: {error}"))?;
            let database = crate::store::observability_database_path()?;
            let parent = manifest.run_id.to_string();
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

pub fn resume_run(selector: &str) -> Result<()> {
    let home = crate::store::observability_home_dir();
    let (dir, manifest) = crate::run_record::resolve_manifest(&home, selector)
        .map_err(|error| anyhow!("Run record unavailable: {error}"))?;
    let provider_session = crate::run_record::read_provider_session(&dir)
        .map_err(|error| anyhow!("Run events unavailable: {error}"))?
        .ok_or_else(|| anyhow!("Run {} has no provider session to resume", manifest.run_id))?;
    crate::lf::commands::util::resume_session(
        &manifest.harness,
        manifest.model.as_deref(),
        &manifest.cwd,
        &manifest.run_id,
        &dir,
        &provider_session,
    )
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
    let (dir, manifest) = crate::run_record::resolve_manifest(&home, selector)
        .map_err(|error| anyhow!("Run record unavailable: {error}"))?;
    if events {
        match std::fs::read_to_string(dir.join("events.jsonl")) {
            Ok(contents) => print!("{contents}"),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
            Err(error) => return Err(anyhow!("Run events unavailable: {error}")),
        }
        return Ok(());
    }
    let snapshot = crate::run_record::read_run_snapshot(&dir)
        .map_err(|error| anyhow!("Run record unavailable: {error}"))?;
    if final_answer {
        let answer = crate::run_record::read_final_answer(&dir)
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
    println!("Working directory: {}", manifest.cwd.display());
    println!(
        "Replay: {}",
        match manifest.launch.as_ref() {
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
    fn tokens_keep_the_compact_human_format() {
        assert_eq!(format_tokens(184_000), "184.0k");
        assert_eq!(format_tokens(0), "");
    }
}
