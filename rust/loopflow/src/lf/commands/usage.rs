//! `lf usage` — direct provider-authored usage from Home-local Run records.

use std::path::Path;

use anyhow::{anyhow, Result};
use time::OffsetDateTime;

use crate::controller::wave::journal::short_id;
use crate::lf::output::{format_cost, format_int, truncate, Colors};
use crate::run_record::RunSnapshot;

const REPO_WIDTH: usize = 18;
const RUN_WIDTH: usize = 22;
const NUM_WIDTH: usize = 12;

/// Print recent direct usage evidence. JSON is the same ordered Run projection
/// used by `lf runs`; it does not invent interval completeness or provider
/// finality.
pub fn run(json: bool, days: u32) -> Result<()> {
    let since = since_days(days);
    let runs = collect_since_at(&crate::store::observability_home_dir(), since)?;
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

fn collect_since_at(home: &Path, since: i64) -> Result<Vec<RunSnapshot>> {
    crate::run_record::scan_runs_since(home, since)
        .map_err(|error| anyhow!("Run records unavailable: {error}"))
}

fn print_report(runs: &[RunSnapshot], days: u32) {
    let window = if days == 0 {
        "all time".to_string()
    } else {
        format!("last {days} days")
    };
    if runs.is_empty() {
        println!("No direct Run usage recorded ({window}).");
        return;
    }

    let colors = Colors::default();
    println!("{}DIRECT RUN USAGE ({window}){}", colors.bold, colors.reset);
    println!(
        "{bold}{time:<12}  {repo:<REPO_WIDTH$}  {run:<RUN_WIDTH$}  {input:>NUM_WIDTH$}  {output:>NUM_WIDTH$}  {cache:>NUM_WIDTH$}  {cost:>9}  {finality:>9}  {gaps:>5}  RUN{reset}",
        bold = colors.bold,
        reset = colors.reset,
        time = "TIME",
        repo = "REPO",
        run = "RUN",
        input = "INPUT",
        output = "OUTPUT",
        cache = "CACHE READ",
        cost = "COST",
        finality = "FINAL",
        gaps = "GAPS",
    );
    for run in runs {
        println!(
            "{time:<12}  {repo:<REPO_WIDTH$}  {run:<RUN_WIDTH$}  {input:>NUM_WIDTH$}  {output:>NUM_WIDTH$}  {cache:>NUM_WIDTH$}  {cost:>9}  {finality:>9}  {gaps:>5}  {id}",
            time = format_time(run.started),
            repo = truncate(&display_repo(run.repo.as_deref()), REPO_WIDTH),
            run = truncate(run.label(), RUN_WIDTH),
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
            id = short_id(&run.id),
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
    repo.and_then(|value| Path::new(value).file_name())
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

#[cfg(test)]
mod tests {
    use super::collect_since_at;
    use crate::engine::stream::StreamEvent;
    use crate::run_record::{CaptureHandle, RunSpec, SubjectAttribution};

    #[test]
    fn usage_reads_direct_bundle_evidence_without_a_sql_ledger() {
        let home = tempfile::tempdir().unwrap();
        let capture = CaptureHandle::begin_at(
            home.path(),
            RunSpec {
                harness: "codex".to_string(),
                model: Some("gpt".to_string()),
                surface: "headless".to_string(),
                cwd: home.path().to_path_buf(),
                repo: Some(home.path().to_path_buf()),
                worktree: Some(home.path().to_path_buf()),
                skill: Some("implement".to_string()),
                subjects: vec![SubjectAttribution::declared("task:LOO-265".to_string())],
            },
        )
        .unwrap();
        capture.record_stream_event(&StreamEvent::Usage {
            input_tokens: Some(12),
            output_tokens: None,
            cache_read_tokens: Some(4),
        });
        capture.finish("completed").unwrap();

        let runs = collect_since_at(home.path(), 0).unwrap();
        assert_eq!(runs.len(), 1);
        assert_eq!(runs[0].usage.input_tokens, Some(12));
        assert_eq!(runs[0].usage.output_tokens, None);
        assert_eq!(runs[0].usage.cache_read_tokens, Some(4));
        assert_eq!(runs[0].usage.final_streams, 0);
        assert_eq!(runs[0].usage.gaps, 0);
    }
}
