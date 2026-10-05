//! `lf usage` — direct provider-authored usage from recorded Session inputs.

use anyhow::Result;
use time::OffsetDateTime;

use crate::lf::commands::util::short_id;
use crate::lf::commands::WorkFilter;
use crate::lf::output::{format_cost, format_int, truncate, Colors};
use crate::session_record::SessionHistory;

const REPO_WIDTH: usize = 18;
const WORK_WIDTH: usize = 22;
const SESSION_WIDTH: usize = 22;
const NUM_WIDTH: usize = 12;

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
    let runs = crate::lf::commands::session_history::collect_history(
        WorkFilter {
            wave,
            project,
            task,
        },
        parent,
        since_days(days),
    )?;
    if context {
        // Oldest first: a Task's steps read in the order they ran.
        let home = crate::store::lf_home_dir();
        let report = crate::context_usage::ContextReport::new(
            runs.iter()
                .rev()
                .map(|run| crate::context_usage::step_context(&home, run))
                .collect(),
        );
        if json {
            println!("{}", serde_json::to_string(&report)?);
        } else {
            println!("{}", crate::context_usage::render_report(&report));
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
            time = format_time(run.observed_at),
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

fn format_time(unix: i64) -> String {
    chrono::DateTime::from_timestamp(unix, 0)
        .map(|utc| {
            utc.with_timezone(&chrono::Local)
                .format("%b %-d %H:%M")
                .to_string()
        })
        .unwrap_or_else(|| unix.to_string())
}
