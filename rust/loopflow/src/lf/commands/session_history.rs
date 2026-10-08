//! Read retained Session inputs and observe provider conversations.

use std::io::Read;

use anyhow::{anyhow, Result};

use crate::lf::commands::WorkFilter;
pub use crate::session_record::active::{ActiveSessionsSnapshot, DiscoveryState};
pub use crate::session_record::SessionHistory;

const WINDOW_DAYS: i64 = 7;
const MAX_SESSIONS: usize = 50;

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
        return super::session_watch::run(&home, &store, task, &runtime);
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
pub(crate) fn collect_recent_history(filter: WorkFilter) -> Result<(Vec<SessionHistory>, bool)> {
    let since = chrono::Utc::now().timestamp() - WINDOW_DAYS * 24 * 3600;
    let database = crate::store::database_path_from_env()?;
    if !database.exists() {
        return Ok((Vec::new(), false));
    }
    let store = crate::store::sqlite::SqliteStore::open_processes_read_only(&database)?;
    Ok(store.recent_conversation_history(
        filter.wave,
        filter.project,
        filter.task,
        since,
        MAX_SESSIONS,
    )?)
}

/// Select conversations in SQL before reading their subordinate history.
pub(crate) fn collect_history(
    filter: WorkFilter,
    parent: Option<&str>,
    since: i64,
) -> Result<Vec<SessionHistory>> {
    let database = crate::store::database_path_from_env()?;
    if !database.exists() {
        return Ok(Vec::new());
    }
    let store = crate::store::sqlite::SqliteStore::open_processes_read_only(&database)?;
    Ok(store.conversation_history(
        filter.wave,
        filter.project,
        filter.task,
        parent,
        since,
        false,
    )?)
}

/// Supply a name to a plain native conversation without changing its execution.
pub fn name_native_session(provider: &str) -> Result<()> {
    let payload: serde_json::Value = serde_json::from_reader(std::io::stdin().lock())?;
    match crate::engine::terminal_title::name_native_session(provider, &payload) {
        Ok(Some(output)) => println!("{output}"),
        Ok(None) => {}
        Err(error) => eprintln!("Session title unavailable: {error:#}"),
    }
    Ok(())
}

/// Record a provider callback against its owning Session capture.
pub fn observe_provider_session() -> Result<()> {
    let key = crate::session_record::inherited_capture_key()?
        .ok_or_else(|| anyhow!("provider session callback has no active Session capture"))?;
    let capture_dir = crate::session_record::capture_dir(&key)?;
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
    crate::session_record::write_provider_session(&capture_dir, provider_session_id, account_id)
        .map_err(|error| anyhow!("cannot preserve provider session: {error}"))
}

pub fn inspect(
    selector: &str,
    events: bool,
    final_answer: bool,
    context: bool,
    json: bool,
) -> Result<()> {
    let home = crate::store::lf_home_dir();
    let database = crate::store::database_path_from_env()?;
    let store = crate::store::sqlite::SqliteStore::open_processes_read_only(&database)?;
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
    if context {
        let step = crate::context_usage::step_context(&home, &snapshot);
        if json {
            println!("{}", serde_json::to_string(&step)?);
        } else {
            println!("{}", crate::context_usage::render_step(&step));
        }
        return Ok(());
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
        snapshot
            .model
            .as_ref()
            .map(|model| format!("{}:{model}", snapshot.harness))
            .unwrap_or_else(|| snapshot.harness.clone())
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
            .and_then(|manifest| manifest.process.as_ref())
        {
            Some(launch) if launch.replay_unavailable_reason().is_none() => "available",
            Some(_) | None => "unavailable",
        }
    );
    println!("Evidence gaps: {}", snapshot.evidence_gaps);
    println!(
        "{}",
        crate::context_usage::render_step(&crate::context_usage::step_context(&home, &snapshot))
    );
    Ok(())
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
