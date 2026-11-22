//! `lf ci` — machine-wide evidence for failed-CI recovery.

use anyhow::Result;
use loopflow_execution::ops::ci::CiReportDto;
use loopflow_execution::output::truncate;

pub fn run(since: &str, wave: Option<&str>, repo: Option<&str>, json: bool) -> Result<()> {
    let report = loopflow_execution::engine::repo::ci(since, wave, repo)?;
    if json {
        println!("{}", serde_json::to_string(&report)?);
    } else {
        print_report(&report);
    }
    Ok(())
}

fn format_duration(value: Option<f64>) -> String {
    let Some(seconds) = value else {
        return "—".to_string();
    };
    if seconds < 60.0 {
        format!("{seconds:.1}s")
    } else if seconds < 3600.0 {
        format!("{:.1}m", seconds / 60.0)
    } else if seconds < 86_400.0 {
        format!("{:.1}h", seconds / 3600.0)
    } else {
        format!("{:.1}d", seconds / 86_400.0)
    }
}

fn print_report(report: &CiReportDto) {
    if report.incidents.is_empty() {
        println!("No CI incidents since {}", report.since);
        return;
    }
    println!(
        "{:<20} {:<15} {:<11} {:>5} {:>7} {:>8} {:>8} {:>8} {:>8}  outcome",
        "repo", "wave", "task", "PR", "fixes", "detect", "respond", "green", "merge"
    );
    for incident in &report.incidents {
        println!(
            "{:<20} {:<15} {:<11} {:>5} {:>7} {:>8} {:>8} {:>8} {:>8}  {}",
            truncate(&incident.repo, 20),
            truncate(&incident.wave, 15),
            truncate(&incident.task, 11),
            format!("#{}", incident.pr_number),
            format!("{}/{}", incident.attempt, incident.fixes_for_pr),
            format_duration(incident.detection_seconds),
            format_duration(incident.response_seconds),
            format_duration(incident.green_seconds),
            format_duration(incident.merge_seconds),
            incident.outcome,
        );
    }
    let summary = &report.summary;
    println!();
    println!(
        "{} incidents across {} PRs · {} unresolved · {} autonomous · {} assisted",
        summary.incidents,
        summary.pull_requests,
        summary.unresolved,
        summary.autonomous,
        summary.human_assisted,
    );
    println!(
        "median / p95: detect {} / {} · respond {} / {} · green {} / {} · merge {} / {}",
        format_duration(summary.median_detection_seconds),
        format_duration(summary.p95_detection_seconds),
        format_duration(summary.median_response_seconds),
        format_duration(summary.p95_response_seconds),
        format_duration(summary.median_green_seconds),
        format_duration(summary.p95_green_seconds),
        format_duration(summary.median_merge_seconds),
        format_duration(summary.p95_merge_seconds),
    );
}
