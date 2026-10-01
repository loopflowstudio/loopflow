//! `lf runs` — read retained AgentSession input and provider history.

use std::{
    io::Read,
    path::{Path, PathBuf},
};

use anyhow::{anyhow, Result};

use crate::lf::commands::util::short_id;
use crate::lf::commands::WorkFilter;
use crate::lf::output::{format_cost, truncate, Colors};
pub use crate::session_record::active::{ActiveSession, ActiveSessionsSnapshot, DiscoveryState};
pub use crate::session_record::{SessionHistory, SessionUsage};

const WINDOW_DAYS: i64 = 7;
const MAX_RUNS: usize = 50;

pub fn list_active(json: bool, watch: bool, task: Option<&str>) -> Result<()> {
    let runtime = tokio::runtime::Runtime::new()?;
    let (home, store, task) = runtime.block_on(async {
        let home = crate::store::lf_home_dir();
        let config = crate::store::StorageConfig::sqlite(crate::store::database_path_from_env()?);
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
        let snapshot = crate::session_record::active::snapshot(&home, &store, task).await;
        if json {
            println!("{}", serde_json::to_string(&snapshot)?);
        } else {
            for session in &snapshot.sessions {
                println!("{}  {}", session.id, session.title);
            }
            for gap in &snapshot.gaps {
                println!("Unavailable: {gap}");
            }
            if snapshot.sessions.is_empty() && snapshot.gaps.is_empty() {
                println!("No active Sessions.");
            }
        }
        Ok(())
    })
}

/// Shared Session history, newest input first. SQL selects the recent budget
/// before decoding; exact Task and caller-input drills remain complete.
pub(crate) fn collect_runs(filter: WorkFilter) -> Result<(Vec<SessionHistory>, bool)> {
    let since = chrono::Utc::now().timestamp() - WINDOW_DAYS * 24 * 3600;
    let database = crate::store::database_path_from_env()?;
    if !database.exists() {
        return Ok((Vec::new(), false));
    }
    let store = crate::store::sqlite::SqliteStore::open_execs_read_only(&database)?;
    Ok(store.recent_conversation_history(
        filter.wave,
        filter.project,
        filter.task,
        since,
        MAX_RUNS,
    )?)
}

pub(crate) fn collect_runs_started_since(
    filter: WorkFilter,
    since: i64,
) -> Result<Vec<SessionHistory>> {
    let path = crate::store::database_path_from_env()?;
    collect_runs_at(&crate::store::lf_home_dir(), &path, filter, None, since)
}

/// Select conversations in SQL before reading their subordinate history.
fn collect_runs_at(
    _lf_home: &Path,
    database: &Path,
    filter: WorkFilter,
    parent: Option<&str>,
    since: i64,
) -> Result<Vec<SessionHistory>> {
    if !database.exists() {
        return Ok(Vec::new());
    }
    let store = crate::store::sqlite::SqliteStore::open_execs_read_only(database)?;
    Ok(store.conversation_history(
        filter.wave,
        filter.project,
        filter.task,
        parent,
        since,
        false,
    )?)
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
    let home = crate::store::lf_home_dir();
    let filter = WorkFilter {
        wave,
        project,
        task,
    };
    // A Task's history and a capture's callers list whole; other drills are recent.
    let runs = match (parent, task) {
        (Some(parent), _) => {
            let database = crate::store::database_path_from_env()?;
            let store = crate::store::sqlite::SqliteStore::open_execs_read_only(&database)?;
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
            (Some(parent), _, _, _) => println!("No caller-input history recorded for {parent}."),
            (None, _, _, Some(task)) => {
                println!("No Session history recorded for {task}.")
            }
            (None, _, Some(project), None) => {
                println!("No Session history recorded for project/{project} in the last {WINDOW_DAYS} days.")
            }
            (None, Some(wave), None, None) => {
                println!(
                    "No Session history recorded for wave/{wave} in the last {WINDOW_DAYS} days."
                )
            }
            (None, None, None, None) => {
                println!("No Session history recorded in the last {WINDOW_DAYS} days.")
            }
        }
        return Ok(());
    }

    let colors = Colors::default();
    println!(
        "{bold}{time:<12}  {repo:<14}  {wave:<10}  {label:<22}  {tokens:>10}  {cost:>8}  {agent:<18}  {status:<12}  INPUT{reset}",
        bold = colors.bold,
        reset = colors.reset,
        time = "TIME",
        repo = "REPO",
        wave = "WAVE",
        label = "HISTORY",
        tokens = "TOKENS",
        cost = "COST",
        agent = "AGENT",
        status = "STATUS",
    );
    for run in &runs {
        println!(
            "{time:<12}  {repo:<14}  {wave:<10}  {label:<22}  {tokens:>10}  {cost:>8}  {agent:<18}  {status:<12}  {id}",
            time = format_time(run.observed_at),
            repo = truncate(&display_repo(run.repo.as_deref()), 14),
            wave = truncate(run.wave_name.as_deref().unwrap_or("-"), 10),
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
            id = short_id(run.selector()),
        );
    }
    Ok(())
}

pub fn observe_provider_session() -> Result<()> {
    let run_dir = std::env::var_os(crate::session_record::RUN_DIR_ENV)
        .map(PathBuf::from)
        .ok_or_else(|| anyhow!("provider session callback has no active Session capture"))?;
    let mut payload = String::new();
    std::io::stdin().read_to_string(&mut payload)?;
    let payload: serde_json::Value = serde_json::from_str(&payload)
        .map_err(|error| anyhow!("invalid provider session callback: {error}"))?;
    let provider_session_id = payload
        .get("session_id")
        .and_then(serde_json::Value::as_str)
        .filter(|session_id| !session_id.is_empty())
        .ok_or_else(|| anyhow!("provider session callback has no session_id"))?;
    let account_id = std::env::var(crate::session_record::PROVIDER_ACCOUNT_ID_ENV)
        .ok()
        .map(|value| crate::store::ProviderAccountId::parse(&value))
        .transpose()
        .map_err(|error| anyhow!("invalid provider account in session callback: {error}"))?;
    crate::session_record::write_provider_session(&run_dir, provider_session_id, account_id)
        .map_err(|error| anyhow!("cannot preserve provider session: {error}"))
}

pub fn inspect(selector: &str, events: bool, final_answer: bool, json: bool) -> Result<()> {
    let home = crate::store::lf_home_dir();
    let database = crate::store::database_path_from_env()?;
    let store = crate::store::sqlite::SqliteStore::open_execs_read_only(&database)?;
    let snapshot = store
        .input_history(selector)
        .map_err(|error| anyhow!("Session capture unavailable: {error}"))?;
    let input = crate::session_record::parse_artifact_key(snapshot.selector())?;
    let dir = crate::session_record::record_dir(&home, &input)
        .ok_or_else(|| anyhow!("Input {} has no artifact path", snapshot.selector()))?;
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
            .map_err(|error| anyhow!("Capture final answer unavailable: {error}"))?;
        return match answer {
            Some(answer) => {
                if !answer.exact {
                    eprintln!(
                        "warning: this capture has no final-answer receipt; showing all streamed prose from its last completed provider turn"
                    );
                }
                println!("{}", answer.text);
                Ok(())
            }
            None if snapshot.recorded_outcome.is_none() => Err(anyhow!(
                "Capture {} is not settled and has no final answer",
                snapshot.selector()
            )),
            None => Err(anyhow!(
                "Capture {} settled as {} without a final answer",
                snapshot.selector(),
                snapshot.status()
            )),
        };
    }
    if json {
        println!("{}", serde_json::to_string(&snapshot)?);
        return Ok(());
    }
    println!(
        "Session {} · input {}",
        snapshot.session_id,
        snapshot.selector()
    );
    if let Some(parent) = &snapshot.caller_artifact_key {
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
    let manifest = crate::session_record::read_manifest(&dir).ok();
    println!(
        "Replay: {}",
        match manifest
            .as_ref()
            .and_then(|manifest| manifest.exec.as_ref())
        {
            Some(launch) if launch.replay_unavailable_reason().is_none() => "available",
            Some(_) | None => "unavailable",
        }
    );
    println!("Evidence gaps: {}", snapshot.evidence_gaps);
    Ok(())
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
