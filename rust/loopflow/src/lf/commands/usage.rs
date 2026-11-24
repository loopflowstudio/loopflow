//! `lf history usage` — direct provider-authored usage from recorded Session inputs.

use anyhow::{Context, Result};
use time::OffsetDateTime;

use crate::lf::commands::WorkFilter;
use loopflow_execution::output::short_id;
use loopflow_execution::output::{format_cost, format_int, truncate, Colors};
use loopflow_execution::session_record::SessionHistory;

const REPO_WIDTH: usize = 18;
const WORK_WIDTH: usize = 22;
const SESSION_WIDTH: usize = 22;
const NUM_WIDTH: usize = 12;

/// Read all turns of one Session, including a rental's last collected reading.
pub(super) fn session(id: &str, json: bool) -> Result<()> {
    let store = loopflow_storage::sqlite::SqliteStore::open_read_only(
        &loopflow_execution::store::database_path_from_env()?,
        loopflow_execution::store::token_key::encryption_key,
    )?;
    let (origin, collected_at, usage) = if store.session(id)?.is_some() {
        (
            loopflow_execution::store::machines::local_machine(&store)?.id,
            None,
            loopflow_storage::session_record::reduce_usage(&store.session_turns(id)?),
        )
    } else {
        let history = store
            .saved_copied_history(id)?
            .with_context(|| format!("Session {id} was not found"))?;
        (
            history.session.origin,
            Some(history.copied_at),
            history.usage,
        )
    };
    if json {
        println!(
            "{}",
            serde_json::json!({ "session_id":id, "origin_machine_id":origin,
            "collected_at":collected_at, "usage":usage })
        );
    } else {
        println!("Session {id} · from {origin}");
        if let Some(at) = collected_at {
            println!("Collected {}", loopflow_execution::output::format_time(at));
        }
        println!(
            "Input {} · output {} · cache read {} · cost {}",
            format_optional(usage.input_tokens),
            format_optional(usage.output_tokens),
            format_optional(usage.cache_read_tokens),
            usage
                .cost_usd
                .map(format_cost)
                .unwrap_or_else(|| "-".into())
        );
    }
    Ok(())
}

/// Print Session input usage without inventing completeness or provider finality.
pub fn run(
    context: bool,
    json: bool,
    days: u32,
    wave: Option<&str>,
    project: Option<&str>,
    task: Option<&str>,
    parent: Option<&str>,
) -> Result<()> {
    let days = if parent.is_some() { 0 } else { days };
    let runs = sessions(
        days,
        WorkFilter {
            wave,
            project,
            task,
        },
        parent,
    )?;
    if context {
        let report = context_report(&runs)?;
        if json {
            println!("{}", serde_json::to_string(&report)?);
        } else {
            println!(
                "{}",
                loopflow_execution::context_usage::render_report(&report)
            );
        }
        return Ok(());
    }
    if json {
        println!("{}", serde_json::to_string(&runs)?);
        return Ok(());
    }
    print_report(&runs, days);
    Ok(())
}

/// The Session inputs `lf usage` reports over `days` (0 is all time).
pub(crate) fn sessions(
    days: u32,
    filter: WorkFilter,
    parent: Option<&str>,
) -> Result<Vec<SessionHistory>> {
    crate::lf::commands::session_history::collect_history(filter, parent, since_days(days))
}

/// `lf usage --context`: what each of `runs`' inputs was made of.
pub fn context_report(
    runs: &[SessionHistory],
) -> Result<loopflow_execution::context_usage::ContextReport> {
    // Oldest first: a Task's steps read in the order they ran.
    let home = loopflow_execution::store::lf_home_dir();
    let steps = if runs.is_empty() {
        Vec::new()
    } else {
        let store = loopflow_storage::sqlite::SqliteStore::open_read_only(
            &loopflow_execution::store::database_path_from_env()?,
            loopflow_execution::store::token_key::encryption_key,
        )?;
        runs.iter()
            .rev()
            .map(|run| loopflow_execution::context_usage::step_context(&home, &store, run))
            .collect()
    };
    Ok(loopflow_execution::context_usage::ContextReport::new(steps))
}

fn since_days(days: u32) -> i64 {
    if days == 0 {
        0
    } else {
        OffsetDateTime::now_utc().unix_timestamp() - i64::from(days) * 86_400
    }
}

fn print_report(runs: &[SessionHistory], days: u32) {
    let window = if days == 0 {
        "all time".to_string()
    } else {
        format!("last {days} days")
    };
    if runs.is_empty() {
        println!("No provider usage recorded ({window}).");
        return;
    }

    let colors = Colors::default();
    println!("{}SESSION USAGE ({window}){}", colors.bold, colors.reset);
    println!(
        "{bold}{time:<12}  {repo:<REPO_WIDTH$}  {work:<WORK_WIDTH$}  {run:<SESSION_WIDTH$}  {input:>NUM_WIDTH$}  {output:>NUM_WIDTH$}  {cache:>NUM_WIDTH$}  {cost:>9}  {finality:>9}  {gaps:>5}  INPUT{reset}",
        bold = colors.bold,
        reset = colors.reset,
        time = "TIME",
        repo = "REPO",
        work = "WORK",
        run = "SESSION",
        input = "INPUT",
        output = "OUTPUT",
        cache = "CACHE READ",
        cost = "COST",
        finality = "FINAL",
        gaps = "GAPS",
    );
    for run in runs {
        println!(
            "{time:<12}  {repo:<REPO_WIDTH$}  {work:<WORK_WIDTH$}  {run:<SESSION_WIDTH$}  {input:>NUM_WIDTH$}  {output:>NUM_WIDTH$}  {cache:>NUM_WIDTH$}  {cost:>9}  {finality:>9}  {gaps:>5}  {id}",
            time = loopflow_execution::output::format_time(run.observed_at),
            repo = truncate(&display_repo(run.repo.as_deref()), REPO_WIDTH),
            work = truncate(&run.work_label(), WORK_WIDTH),
            run = truncate(run.label(), SESSION_WIDTH),
            input = format_optional(run.usage.input_tokens),
            output = format_optional(run.usage.output_tokens),
            cache = format_optional(run.usage.cache_read_tokens),
            cost = run
                .usage
                .cost_usd
                .map(format_cost)
                .unwrap_or_else(|| "-".to_string()),
            finality = format!("{}/{}", run.usage.final_streams, run.usage.streams),
            gaps = run.evidence_gaps,
            id = short_id(run.selector()),
        );
    }
}

fn format_optional(value: Option<i64>) -> String {
    value
        .and_then(|value| u64::try_from(value).ok())
        .map(format_int)
        .unwrap_or_else(|| "-".to_string())
}

fn display_repo(repo: Option<&str>) -> String {
    repo.and_then(|value| std::path::Path::new(value).file_name())
        .and_then(|value| value.to_str())
        .unwrap_or("-")
        .to_string()
}
