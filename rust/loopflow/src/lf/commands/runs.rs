//! `lf runs` — read Home-local Run records.

#[cfg(test)]
use std::path::Path;

use anyhow::{anyhow, Result};

use crate::lf::commands::WorkFilter;
use crate::lf::output::{format_cost, truncate, Colors};
pub use crate::run_record::{AttributionSource, RunSnapshot, RunUsage, SubjectAttribution};
use crate::wave::journal::short_id;

const WINDOW_DAYS: i64 = 7;
const MAX_RUNS: usize = 50;

/// The Runs matching a filter, newest first, capped. One reader behind
/// `lf runs`, its Work drills, and `lf status`'s Runs evidence, so the surfaces
/// can never disagree on what a run is.
pub(crate) fn collect_runs(filter: WorkFilter) -> Result<(Vec<RunSnapshot>, bool)> {
    let since = chrono::Utc::now().timestamp() - WINDOW_DAYS * 24 * 3600;
    let mut runs = collect_runs_started_since(filter, since)?;
    let truncated = cap_runs(&mut runs);
    Ok((runs, truncated))
}

fn collect_runs_started_since(filter: WorkFilter, since: i64) -> Result<Vec<RunSnapshot>> {
    crate::run_record::scan_runs_since(&crate::store::observability_home_dir(), since)
        .map_err(|err| anyhow!("Run records unavailable: {err}"))
        .map(|runs| {
            runs.into_iter()
                .filter(|run| {
                    filter.matches(
                        run.subject("wave"),
                        run.subject("project"),
                        run.subject("task"),
                    )
                })
                .collect()
        })
}

#[cfg(test)]
fn collect_runs_started_since_at(
    lf_home: &Path,
    filter: WorkFilter,
    since: i64,
) -> Result<Vec<RunSnapshot>> {
    crate::run_record::scan_runs_since(lf_home, since)
        .map_err(|err| anyhow!("Run records unavailable: {err}"))
        .map(|runs| {
            runs.into_iter()
                .filter(|run| {
                    filter.matches(
                        run.subject("wave"),
                        run.subject("project"),
                        run.subject("task"),
                    )
                })
                .collect()
        })
}

/// The filtered Run definition without a presentation cap. Compound activity
/// surfaces include both starts and finishes inside their requested window,
/// then cap only after joining Runs to their other durable facts.
pub(crate) fn collect_run_activity_since(
    filter: WorkFilter,
    since: i64,
) -> Result<Vec<RunSnapshot>> {
    crate::run_record::scan_runs_since(&crate::store::observability_home_dir(), 0)
        .map_err(|err| anyhow!("Run records unavailable: {err}"))
        .map(|runs| {
            runs.into_iter()
                .filter(|run| run.started >= since || run.ended.is_some_and(|end| end >= since))
                .filter(|run| {
                    filter.matches(
                        run.subject("wave"),
                        run.subject("project"),
                        run.subject("task"),
                    )
                })
                .collect()
        })
}

/// `lf runs [--wave <name>] [--project <slug>] [--task <id>]`: recent harness
/// launches, optionally drilled to one attributed Work subject.
pub fn list(
    json: bool,
    wave: Option<&str>,
    project: Option<&str>,
    task: Option<&str>,
    run: Option<&str>,
    events: bool,
) -> Result<()> {
    if let Some(run) = run {
        return inspect(run, events, json);
    }
    let (runs, _truncated) = collect_runs(WorkFilter {
        wave,
        project,
        task,
    })?;

    if json {
        println!("{}", serde_json::to_string(&runs)?);
        return Ok(());
    }

    if runs.is_empty() {
        match (wave, project, task) {
            (_, _, Some(task)) => {
                println!("No Runs recorded for {task} in the last {WINDOW_DAYS} days.")
            }
            (_, Some(project), None) => {
                println!("No Runs recorded for project/{project} in the last {WINDOW_DAYS} days.")
            }
            (Some(wave), None, None) => {
                println!("No Runs recorded for wave/{wave} in the last {WINDOW_DAYS} days.")
            }
            (None, None, None) => {
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

fn inspect(selector: &str, events: bool, json: bool) -> Result<()> {
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

/// The Runs attributed to one Wave, newest first.
pub(crate) fn wave_runs(wave: &str) -> Result<(Vec<RunSnapshot>, bool)> {
    let since = chrono::Utc::now().timestamp() - WINDOW_DAYS * 24 * 3600;
    let mut runs = collect_runs_started_since(
        WorkFilter {
            wave: Some(wave),
            project: None,
            task: None,
        },
        since,
    )?;
    let truncated = cap_runs(&mut runs);
    Ok((runs, truncated))
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
    use super::{collect_runs_started_since_at, format_tokens};
    use crate::lf::commands::WorkFilter;
    use crate::run_record::{CaptureHandle, RunSpec, SubjectAttribution};

    #[test]
    fn work_drill_reads_record_subjects_without_a_sql_ledger() {
        let home = tempfile::tempdir().unwrap();
        for task in ["LOO-265", "LOO-999"] {
            let capture = CaptureHandle::begin_at(
                home.path(),
                RunSpec {
                    harness: "codex".to_string(),
                    model: None,
                    surface: "headless".to_string(),
                    cwd: home.path().to_path_buf(),
                    repo: Some(home.path().to_path_buf()),
                    worktree: Some(home.path().to_path_buf()),
                    skill: Some("implement".to_string()),
                    subjects: vec![SubjectAttribution::declared(format!("task:{task}"))],
                },
            )
            .unwrap();
            capture.finish("completed").unwrap();
        }

        let runs = collect_runs_started_since_at(
            home.path(),
            WorkFilter {
                wave: None,
                project: None,
                task: Some("LOO-265"),
            },
            0,
        )
        .unwrap();

        assert_eq!(runs.len(), 1);
        assert_eq!(runs[0].subject("task"), Some("LOO-265"));
    }

    #[test]
    fn tokens_keep_the_compact_human_format() {
        assert_eq!(format_tokens(184_000), "184.0k");
        assert_eq!(format_tokens(0), "");
    }
}
