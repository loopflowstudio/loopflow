//! `lf ci` — machine-wide evidence for failed-CI recovery.

use std::collections::{BTreeMap, BTreeSet};

use anyhow::{anyhow, Result};
use serde::Serialize;
use time::{format_description::well_known::Rfc3339, Duration, OffsetDateTime};

use crate::lf::commands::util::parse_since;
use crate::lf::output::truncate;
use crate::store::ci_incidents::CiIncidentReportRow;
use crate::store::open_existing_store;

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct CiReportDto {
    pub generated_at: String,
    pub since: String,
    pub wave: Option<String>,
    pub repo: Option<String>,
    pub summary: CiSummaryDto,
    pub incidents: Vec<CiIncidentDto>,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct CiSummaryDto {
    pub incidents: usize,
    pub pull_requests: usize,
    pub unresolved: usize,
    pub blocked: usize,
    pub autonomous: usize,
    pub human_assisted: usize,
    pub median_detection_seconds: Option<f64>,
    pub p95_detection_seconds: Option<f64>,
    pub median_response_seconds: Option<f64>,
    pub p95_response_seconds: Option<f64>,
    pub median_green_seconds: Option<f64>,
    pub p95_green_seconds: Option<f64>,
    pub median_merge_seconds: Option<f64>,
    pub p95_merge_seconds: Option<f64>,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct CiIncidentDto {
    pub identity: String,
    pub repo: String,
    pub wave: String,
    pub task: String,
    pub task_status: String,
    pub pr_number: u32,
    pub attempt: usize,
    pub fixes_for_pr: usize,
    pub failed_head_sha: String,
    /// The head a ci-fix body shipped for this incident, once one advanced the
    /// head past `failed_head_sha`. `None` while the incident is unrepaired.
    pub repaired_head_sha: Option<String>,
    pub failure_set: Vec<String>,
    pub provider_completed_at: Option<String>,
    pub observed_at: String,
    pub observer: String,
    /// The durable `ci-fix` wake this failure enqueued. `None` only before a wake
    /// is linked — an incident holding `responded_at` without one means a body was
    /// woken by something outside the ledger, which is the bypass this path
    /// exists to prevent.
    pub claimed_run_id: Option<String>,
    pub responded_at: Option<String>,
    pub green_at: Option<String>,
    pub merged_at: Option<String>,
    pub blocked_at: Option<String>,
    pub blocked_reason: Option<String>,
    pub outcome: String,
    pub human_assisted: bool,
    pub detection_seconds: Option<f64>,
    pub response_seconds: Option<f64>,
    pub green_seconds: Option<f64>,
    pub merge_seconds: Option<f64>,
    pub task_cycle_seconds: Option<f64>,
}

pub fn run(since: &str, wave: Option<&str>, repo: Option<&str>, json: bool) -> Result<()> {
    let now = OffsetDateTime::now_utc();
    let since_at = parse_since(since, now)?;
    let runtime = tokio::runtime::Runtime::new()?;
    let rows = runtime.block_on(async {
        let Some(store) = open_existing_store().await else {
            return Ok(Vec::new());
        };
        store
            .ci_incidents_since(since_at, wave, repo)
            .await
            .map_err(|error| anyhow!("failed to read CI incidents: {error}"))
    })?;
    let report = build_report(rows, since_at, now, wave, repo)?;
    if json {
        println!("{}", serde_json::to_string(&report)?);
    } else {
        print_report(&report);
    }
    Ok(())
}

fn build_report(
    rows: Vec<CiIncidentReportRow>,
    since: OffsetDateTime,
    generated_at: OffsetDateTime,
    wave: Option<&str>,
    repo: Option<&str>,
) -> Result<CiReportDto> {
    let mut by_pr: BTreeMap<String, Vec<(OffsetDateTime, String)>> = BTreeMap::new();
    for row in &rows {
        by_pr
            .entry(row.incident.pr_id.to_string())
            .or_default()
            .push((row.incident.created_at, row.incident.identity.clone()));
    }
    for incidents in by_pr.values_mut() {
        incidents.sort();
    }

    let mut incidents = Vec::with_capacity(rows.len());
    for row in rows {
        let attempts = &by_pr[&row.incident.pr_id.to_string()];
        let attempt = attempts
            .iter()
            .position(|(_, identity)| identity == &row.incident.identity)
            .expect("incident grouped by its own PR")
            + 1;
        incidents.push(incident_dto(row, attempt, attempts.len())?);
    }
    let summary = summarize(&incidents);
    Ok(CiReportDto {
        generated_at: format_time(generated_at)?,
        since: format_time(since)?,
        wave: wave.map(str::to_string),
        repo: repo.map(str::to_string),
        summary,
        incidents,
    })
}

fn incident_dto(
    row: CiIncidentReportRow,
    attempt: usize,
    fixes_for_pr: usize,
) -> Result<CiIncidentDto> {
    let incident = row.incident;
    let (observed_at, observer) = match (incident.webhook_received_at, incident.poll_observed_at) {
        (Some(webhook), Some(poll)) if webhook <= poll => (webhook, "webhook"),
        (Some(_), Some(poll)) => (poll, "poll"),
        (Some(webhook), None) => (webhook, "webhook"),
        (None, Some(poll)) => (poll, "poll"),
        (None, None) => (incident.created_at, "unknown"),
    };
    let failure_at = incident.provider_completed_at.unwrap_or(observed_at);
    let outcome = if incident.merged_at.is_some() {
        "merged"
    } else if incident.green_at.is_some() {
        "green"
    } else if incident.blocked_at.is_some() {
        "blocked"
    } else {
        "open"
    };
    Ok(CiIncidentDto {
        identity: incident.identity,
        repo: incident.repo,
        wave: row.wave,
        task: row.task,
        task_status: row.task_status,
        pr_number: incident.pr_number,
        attempt,
        fixes_for_pr,
        failed_head_sha: incident.failed_head_sha,
        repaired_head_sha: incident.repaired_head_sha,
        failure_set: incident.failure_set,
        provider_completed_at: incident
            .provider_completed_at
            .map(format_time)
            .transpose()?,
        observed_at: format_time(observed_at)?,
        observer: observer.to_string(),
        claimed_run_id: incident
            .claimed_run_id
            .as_ref()
            .map(|id| id.as_str().to_string()),
        responded_at: incident.responded_at.map(format_time).transpose()?,
        green_at: incident.green_at.map(format_time).transpose()?,
        merged_at: incident.merged_at.map(format_time).transpose()?,
        blocked_at: incident.blocked_at.map(format_time).transpose()?,
        blocked_reason: incident.blocked_reason,
        outcome: outcome.to_string(),
        human_assisted: row.human_assisted,
        detection_seconds: elapsed(incident.provider_completed_at, Some(observed_at)),
        response_seconds: elapsed(Some(observed_at), incident.responded_at),
        green_seconds: elapsed(Some(failure_at), incident.green_at),
        merge_seconds: elapsed(Some(failure_at), incident.merged_at),
        task_cycle_seconds: elapsed(Some(row.task_started_at), incident.merged_at),
    })
}

fn summarize(incidents: &[CiIncidentDto]) -> CiSummaryDto {
    let pull_requests = incidents
        .iter()
        .map(|incident| (incident.repo.as_str(), incident.pr_number))
        .collect::<BTreeSet<_>>()
        .len();
    let values = |read: fn(&CiIncidentDto) -> Option<f64>| {
        incidents.iter().filter_map(read).collect::<Vec<_>>()
    };
    let detection = values(|incident| incident.detection_seconds);
    let response = values(|incident| incident.response_seconds);
    let green = values(|incident| incident.green_seconds);
    let merge = values(|incident| incident.merge_seconds);
    CiSummaryDto {
        incidents: incidents.len(),
        pull_requests,
        unresolved: incidents
            .iter()
            .filter(|incident| !matches!(incident.outcome.as_str(), "green" | "merged"))
            .count(),
        blocked: incidents
            .iter()
            .filter(|incident| incident.outcome == "blocked")
            .count(),
        // `!human_assisted` alone only says nobody was seen intervening, which is
        // also true of a green that no `ci-fix` wake repaired. A wake that was
        // persisted but never serviced did not own the repair either.
        autonomous: incidents
            .iter()
            .filter(|incident| {
                matches!(incident.outcome.as_str(), "green" | "merged")
                    && incident.claimed_run_id.is_some()
                    && incident.responded_at.is_some()
                    && !incident.human_assisted
            })
            .count(),
        human_assisted: incidents
            .iter()
            .filter(|incident| {
                matches!(incident.outcome.as_str(), "green" | "merged") && incident.human_assisted
            })
            .count(),
        median_detection_seconds: percentile(&detection, 0.5),
        p95_detection_seconds: percentile(&detection, 0.95),
        median_response_seconds: percentile(&response, 0.5),
        p95_response_seconds: percentile(&response, 0.95),
        median_green_seconds: percentile(&green, 0.5),
        p95_green_seconds: percentile(&green, 0.95),
        median_merge_seconds: percentile(&merge, 0.5),
        p95_merge_seconds: percentile(&merge, 0.95),
    }
}

fn elapsed(start: Option<OffsetDateTime>, end: Option<OffsetDateTime>) -> Option<f64> {
    let duration = end? - start?;
    (duration >= Duration::ZERO).then(|| duration.as_seconds_f64())
}

fn percentile(values: &[f64], percentile: f64) -> Option<f64> {
    if values.is_empty() {
        return None;
    }
    let mut values = values.to_vec();
    values.sort_by(f64::total_cmp);
    let index = ((values.len() - 1) as f64 * percentile).ceil() as usize;
    Some(values[index])
}

fn format_time(value: OffsetDateTime) -> Result<String> {
    value
        .format(&Rfc3339)
        .map_err(|error| anyhow!(error.to_string()))
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
        "{} incidents across {} PRs · {} unresolved · {} autonomous · {} human-assisted",
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

#[cfg(test)]
mod tests {
    use super::{parse_since, percentile, summarize, CiIncidentDto};
    use time::OffsetDateTime;

    fn green_incident(trigger: Option<&str>, responded: Option<&str>) -> CiIncidentDto {
        CiIncidentDto {
            identity: "github:ci:loopflow:1034".to_string(),
            repo: "loopflow".to_string(),
            wave: "infrastructure".to_string(),
            task: "W2-293".to_string(),
            task_status: "running".to_string(),
            pr_number: 1034,
            attempt: 1,
            fixes_for_pr: 1,
            failed_head_sha: "0123456789ab".to_string(),
            repaired_head_sha: None,
            failure_set: vec!["rust-test".to_string()],
            provider_completed_at: None,
            observed_at: "2026-07-16T00:00:00Z".to_string(),
            observer: "poll".to_string(),
            claimed_run_id: trigger.map(str::to_string),
            responded_at: responded.map(str::to_string),
            green_at: Some("2026-07-16T00:10:00Z".to_string()),
            merged_at: None,
            blocked_at: None,
            blocked_reason: None,
            outcome: "green".to_string(),
            human_assisted: false,
            detection_seconds: None,
            response_seconds: None,
            green_seconds: None,
            merge_seconds: None,
            task_cycle_seconds: None,
        }
    }

    /// `repaired_head_sha` is the operator-facing half of the settlement-race fix
    /// (W2-320): `lf ci --json` must carry the head a ci-fix body shipped, beside
    /// the head it failed on, or the attribution is invisible.
    #[test]
    fn repaired_head_is_exposed_on_the_json_surface() {
        let mut dto = green_incident(Some("cc_1"), Some("2026-07-16T00:05:00Z"));
        dto.repaired_head_sha = Some("02527e29".to_string());
        let json = serde_json::to_value(&dto).expect("serialize CiIncidentDto");
        assert_eq!(
            json.get("repaired_head_sha")
                .and_then(|value| value.as_str()),
            Some("02527e29"),
            "lf ci --json carries the repaired head beside failed_head_sha"
        );

        dto.repaired_head_sha = None;
        let json = serde_json::to_value(&dto).expect("serialize CiIncidentDto");
        assert!(
            json.get("repaired_head_sha")
                .is_some_and(serde_json::Value::is_null),
            "an unrepaired incident renders repaired_head_sha: null, never a missing key"
        );
    }

    /// The first row is PR #1034: a normal Task body pushed the fix, so no `ci-fix`
    /// wake owned the repair. The second is a green answered by something outside
    /// the ledger, and it is what pins the `claimed_run_id` clause — #1034 alone
    /// is excluded by the `responded_at` clause too, so it would pass without it.
    #[test]
    fn an_untriggered_green_is_not_autonomous() {
        let summary = summarize(&[
            green_incident(None, None),
            green_incident(None, Some("2026-07-16T00:05:00Z")),
        ]);
        assert_eq!(summary.autonomous, 0);
    }

    /// A wake persisted but never serviced did not repair anything.
    #[test]
    fn a_triggered_but_unanswered_green_is_not_autonomous() {
        assert_eq!(
            summarize(&[green_incident(Some("cc_1018"), None)]).autonomous,
            0
        );
    }

    /// A serviced wake still does not own a repair a human was in the loop for.
    #[test]
    fn a_human_assisted_green_is_not_autonomous() {
        let mut incident = green_incident(Some("cc_1018"), Some("2026-07-16T00:05:00Z"));
        incident.human_assisted = true;
        assert_eq!(summarize(&[incident]).autonomous, 0);
    }

    #[test]
    fn a_triggered_and_answered_green_is_autonomous() {
        let incident = green_incident(Some("cc_1018"), Some("2026-07-16T00:05:00Z"));
        assert_eq!(summarize(&[incident]).autonomous, 1);
    }

    #[test]
    fn relative_and_absolute_since_values_parse() {
        let now = OffsetDateTime::UNIX_EPOCH + time::Duration::days(10);
        assert_eq!(
            parse_since("7d", now).unwrap(),
            now - time::Duration::days(7)
        );
        assert_eq!(
            parse_since("1970-01-02T00:00:00Z", now).unwrap(),
            OffsetDateTime::UNIX_EPOCH + time::Duration::days(1)
        );
        assert!(parse_since("week", now).is_err());
    }

    #[test]
    fn percentile_reports_the_observed_tail() {
        assert_eq!(percentile(&[], 0.5), None);
        assert_eq!(percentile(&[1.0, 2.0, 9.0], 0.5), Some(2.0));
        assert_eq!(percentile(&[1.0, 2.0, 9.0], 0.95), Some(9.0));
    }
}
