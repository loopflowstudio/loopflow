//! Release settlement and scheduled verification are independent of process exit status.
use std::collections::HashSet;
use std::fs::{self, File};
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use super::accounting::{self, ObligationSegment, ReleaseOpportunity, ScheduledReleaseOutcome};
use super::{read_receipts, receipt_root, CronOutcome, CronReceipt, CronSource};
use crate::durable::TaskId;
use crate::ops::{OpsError, OpsResult};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FailureDisposition {
    pub subject: String,
    pub owner: TaskId,
    pub reason: String,
    pub recorded_at: i64,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ReleaseHistory {
    pub window_start: i64,
    pub observed_at: i64,
    pub observation_frontier: Option<i64>,
    pub obligations: Vec<ObligationSegment>,
    pub receipts: Vec<CronReceipt>,
    pub dispositions: Vec<FailureDisposition>,
    pub summary: ReleaseSummary,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ReleaseSummary {
    pub due: usize,
    pub accounted: usize,
    pub historical_timezone_unknown: usize,
    pub collapsed: usize,
    pub executions: usize,
    pub on_time: usize,
    pub caught_up: usize,
    pub published: usize,
    pub no_change: usize,
    pub unresolved: usize,
    pub closed_unsettled: Vec<String>,
    pub failed_verifications: usize,
    pub undispositioned_failures: Vec<String>,
    pub late_dispositions: Vec<String>,
    pub qualifying_pairs: Vec<[String; 2]>,
}

fn disposition_path(home: &Path, subject: &str) -> PathBuf {
    home.join("cron/dispositions")
        .join(format!("{}.json", hex::encode(Sha256::digest(subject))))
}

fn read_dispositions(home: &Path) -> OpsResult<Vec<FailureDisposition>> {
    let dir = home.join("cron/dispositions");
    if !dir.exists() {
        return Ok(Vec::new());
    }
    let mut dispositions = Vec::new();
    for entry in fs::read_dir(dir)? {
        let path = entry?.path();
        if path.extension().is_none_or(|e| e != "json") {
            continue;
        }
        let rows: Vec<FailureDisposition> =
            serde_json::from_slice(&fs::read(&path)?).map_err(|e| {
                OpsError::Message(format!("invalid disposition {}: {e}", path.display()))
            })?;
        if rows
            .iter()
            .any(|r| disposition_path(home, &r.subject) != path)
        {
            return Err(OpsError::Message(format!(
                "mismatched disposition {}",
                path.display()
            )));
        }
        dispositions.extend(rows);
    }
    dispositions.sort_by_key(|r| r.recorded_at);
    Ok(dispositions)
}

pub fn disposition(
    home: &Path,
    wave: &str,
    subject: &str,
    owner: TaskId,
    reason: &str,
    now: i64,
) -> OpsResult<()> {
    if reason.trim().is_empty() {
        return Err(OpsError::Message("repair reason cannot be empty".into()));
    }
    let _lock = accounting::lock(home)?;
    let receipt_failed = read_receipts(&receipt_root(home), wave, None)?
        .iter()
        .any(|r| r.id.as_str() == subject && r.outcome == CronOutcome::Failed);
    let mut obligations = accounting::read(home)?;
    for record in obligations
        .iter_mut()
        .filter(|r| r.wave == wave && r.closed_at.is_some())
    {
        accounting::materialize(record, now)?;
    }
    let closed_unsettled = obligations
        .iter()
        .filter(|r| r.wave == wave)
        .flat_map(|r| r.closed_unsettled())
        .any(|o| o.id == subject);
    let opportunity_failed = obligations
        .iter()
        .filter(|r| r.wave == wave)
        .flat_map(|r| &r.opportunities)
        .any(|o| {
            o.attempts.iter().any(|a| {
                (o.id == subject || a.receipt_id.as_str() == subject)
                    && matches!(
                        a.outcome,
                        ScheduledReleaseOutcome::Failed { .. }
                            | ScheduledReleaseOutcome::Unverified { .. }
                    )
            })
        });
    if !receipt_failed && !opportunity_failed && !closed_unsettled {
        return Err(OpsError::Message(format!(
            "{subject} does not name retained failure evidence or a closed unsettled opportunity for {wave}"
        )));
    }
    let mut rows: Vec<_> = read_dispositions(home)?
        .into_iter()
        .filter(|r| r.subject == subject)
        .collect();
    rows.push(FailureDisposition {
        subject: subject.into(),
        owner,
        reason: reason.into(),
        recorded_at: now,
    });
    let path = disposition_path(home, subject);
    let dir = path.parent().expect("disposition path has a parent");
    fs::create_dir_all(dir)?;
    let mut temp = tempfile::NamedTempFile::new_in(dir)?;
    serde_json::to_writer_pretty(&mut temp, &rows).map_err(|e| OpsError::Parse(e.to_string()))?;
    temp.as_file().sync_all()?;
    temp.persist(&path)
        .map_err(|e| OpsError::Message(e.to_string()))?;
    File::open(dir)?.sync_all()?;
    Ok(())
}

pub fn release_history(
    home: &Path,
    repo: &Path,
    wave: &str,
    days: u32,
    now: i64,
) -> OpsResult<ReleaseHistory> {
    let obligations = accounting::history(home, repo, wave, now)?;
    let window_start = now.saturating_sub(i64::from(days) * 86400);
    let repo = repo.canonicalize()?;
    let linked: HashSet<_> = obligations
        .iter()
        .flat_map(|o| &o.opportunities)
        .flat_map(|o| &o.attempts)
        .filter_map(|a| a.telemetry.as_ref())
        .flat_map(|t| {
            t.original
                .iter()
                .flat_map(|d| &d.receipts)
                .chain(&t.current_receipts)
                .chain(t.recovery_receipt.iter())
        })
        .collect();
    let receipts: Vec<_> = read_receipts(&receipt_root(home), wave, None)?
        .into_iter()
        .filter(|r| {
            r.repo.canonicalize().ok().as_ref() == Some(&repo)
                && (r.started_at >= window_start || linked.contains(&r.id))
        })
        .collect();
    let dispositions = read_dispositions(home)?;
    let summary = summarize(&obligations, &receipts, &dispositions, window_start);
    Ok(ReleaseHistory {
        window_start,
        observed_at: now,
        observation_frontier: obligations
            .iter()
            .filter(|o| o.flow == "release-run")
            .map(|o| o.observed_at)
            .min(),
        obligations,
        receipts,
        dispositions,
        summary,
    })
}

fn summarize(
    obligations: &[ObligationSegment],
    receipts: &[CronReceipt],
    dispositions: &[FailureDisposition],
    since: i64,
) -> ReleaseSummary {
    let mut summary = ReleaseSummary {
        due: 0,
        accounted: 0,
        historical_timezone_unknown: 0,
        collapsed: 0,
        executions: 0,
        on_time: 0,
        caught_up: 0,
        published: 0,
        no_change: 0,
        unresolved: 0,
        closed_unsettled: Vec::new(),
        failed_verifications: 0,
        undispositioned_failures: Vec::new(),
        late_dispositions: Vec::new(),
        qualifying_pairs: Vec::new(),
    };
    let mut published = std::collections::HashSet::new();
    let mut counted_owners = std::collections::HashSet::new();
    for record in obligations {
        // Closure has no future wake. Keep its repair obligation visible beyond
        // the reporting window without inventing an attempt or a process exit.
        for owner in record.closed_unsettled() {
            summary.closed_unsettled.push(owner.id.clone());
            record_failure(
                &mut summary,
                dispositions,
                &owner.id,
                None,
                record
                    .closed_at
                    .expect("closed owner has a closure timestamp"),
            );
        }
        summary.executions += record
            .opportunities
            .iter()
            .flat_map(|o| &o.attempts)
            .filter(|a| a.started_at >= since)
            .count();
        let due: Vec<_> = record
            .opportunities
            .iter()
            .filter(|o| o.due_at >= since)
            .collect();
        for opportunity in &due {
            summary.due += 1;
            summary.historical_timezone_unknown +=
                usize::from(opportunity.historical_timezone_unknown);
            summary.collapsed += usize::from(opportunity.coalesced_into.is_some());
            let owner = opportunity
                .coalesced_into
                .as_ref()
                .and_then(|id| {
                    obligations
                        .iter()
                        .flat_map(|r| &r.opportunities)
                        .find(|o| &o.id == id)
                })
                .unwrap_or(opportunity);
            summary.accounted += usize::from(
                !owner.attempts.is_empty() || owner.wait.is_some() || record.closed_at.is_some(),
            );
            if let Some(first) = obligations
                .iter()
                .flat_map(|r| &r.opportunities)
                .flat_map(|o| &o.attempts)
                .filter(|a| a.covered.contains(&opportunity.id))
                .min_by_key(|a| a.started_at)
            {
                if first.started_at >= opportunity.due_at
                    && first.started_at < opportunity.due_at + 60
                {
                    summary.on_time += 1;
                } else if first.started_at >= opportunity.due_at + 60 {
                    summary.caught_up += 1;
                }
            }
            let settled = owner.attempts.last().is_some_and(|a| {
                matches!(
                    a.outcome,
                    ScheduledReleaseOutcome::Published { .. }
                        | ScheduledReleaseOutcome::NoChange { .. }
                )
            });
            summary.unresolved += usize::from(!settled);
            if counted_owners.insert(&owner.id) {
                if let Some(attempt) = owner.attempts.last() {
                    match &attempt.outcome {
                        ScheduledReleaseOutcome::Published { tag, commit, .. } => {
                            if published.insert((tag, commit)) {
                                summary.published += 1;
                            }
                        }
                        ScheduledReleaseOutcome::NoChange { .. } => summary.no_change += 1,
                        _ => {}
                    }
                }
            }
            for attempt in &opportunity.attempts {
                if matches!(
                    attempt.outcome,
                    ScheduledReleaseOutcome::Failed { .. }
                        | ScheduledReleaseOutcome::Unverified { .. }
                ) {
                    record_failure(
                        &mut summary,
                        dispositions,
                        attempt.receipt_id.as_str(),
                        Some(&opportunity.id),
                        attempt.finished_at.unwrap_or(attempt.started_at),
                    );
                }
            }
        }
        for pair in due.windows(2) {
            let [left, right] = pair else {
                unreachable!("windows(2)")
            };
            if left.next_due_at != right.due_at {
                continue;
            }
            if let (Some(a), Some(b)) = (
                qualifying(obligations, left),
                qualifying(obligations, right),
            ) {
                let one_publication = matches!(a, ScheduledReleaseOutcome::Published { .. })
                    || matches!(b, ScheduledReleaseOutcome::Published { .. });
                let duplicate = matches!((a, b), (ScheduledReleaseOutcome::Published { tag: a_tag, commit: a_commit, .. }, ScheduledReleaseOutcome::Published { tag: b_tag, commit: b_commit, .. }) if a_tag == b_tag && a_commit == b_commit);
                if one_publication && !duplicate {
                    summary
                        .qualifying_pairs
                        .push([left.id.clone(), right.id.clone()]);
                }
            }
        }
    }
    for receipt in receipts.iter().filter(|r| r.outcome == CronOutcome::Failed) {
        if receipt.flow == "telemetry-daily" {
            summary.failed_verifications += 1;
        }
        let owner = obligations
            .iter()
            .flat_map(|r| &r.opportunities)
            .find(|o| o.attempts.iter().any(|a| a.receipt_id == receipt.id));
        record_failure(
            &mut summary,
            dispositions,
            receipt.id.as_str(),
            owner.map(|o| o.id.as_str()),
            receipt.finished_at.unwrap_or(receipt.started_at),
        );
    }
    summary.undispositioned_failures.sort();
    summary.undispositioned_failures.dedup();
    summary.late_dispositions.sort();
    summary.late_dispositions.dedup();
    summary
}

fn qualifying<'a>(
    obligations: &'a [ObligationSegment],
    opportunity: &'a ReleaseOpportunity,
) -> Option<&'a ScheduledReleaseOutcome> {
    if opportunity.historical_timezone_unknown
        || opportunity.coalesced_into.is_some()
        || obligations
            .iter()
            .flat_map(|r| &r.opportunities)
            .filter(|o| {
                o.id == opportunity.id || o.coalesced_into.as_ref() == Some(&opportunity.id)
            })
            .any(|o| {
                !o.interventions.is_empty()
                    || o.attempts.iter().any(|a| a.source != CronSource::Scheduled)
            })
    {
        return None;
    }
    let attempt = opportunity.attempts.last()?;
    if attempt.started_at < opportunity.due_at {
        return None;
    }
    match &attempt.outcome {
        ScheduledReleaseOutcome::Published { .. } | ScheduledReleaseOutcome::NoChange { .. }
            if complete_verification(&attempt.verification) =>
        {
            Some(&attempt.outcome)
        }
        _ => None,
    }
}

fn complete_verification(proofs: &[accounting::VerificationEvidence]) -> bool {
    [
        "scheduled-telemetry",
        "repository-verification",
        "public-release",
        "hosted-workflow",
    ]
    .iter()
    .all(|name| proofs.iter().any(|p| p.name == *name && p.passed))
        && proofs.iter().all(|p| p.passed)
}

fn record_failure(
    summary: &mut ReleaseSummary,
    dispositions: &[FailureDisposition],
    subject: &str,
    opportunity: Option<&str>,
    failed_at: i64,
) {
    match dispositions
        .iter()
        .filter(|r| r.subject == subject || Some(r.subject.as_str()) == opportunity)
        .min_by_key(|r| r.recorded_at)
    {
        None => summary.undispositioned_failures.push(subject.into()),
        Some(r) if r.recorded_at > failed_at + 86400 => {
            summary.late_dispositions.push(subject.into())
        }
        Some(_) => {}
    }
}

#[cfg(test)]
mod tests {
    use super::{summarize, ReleaseHistory};
    use crate::ops::CronSource;

    fn fixture() -> ReleaseHistory {
        serde_json::from_str(include_str!(
            "../../../../../tests/fixtures/dto/release_history.json"
        ))
        .unwrap()
    }

    #[test]
    fn history_fixture_retains_failed_verification_and_late_ownership_beside_settlements() {
        let report = fixture();
        let original: serde_json::Value = serde_json::from_str(include_str!(
            "../../../../../tests/fixtures/dto/release_history.json"
        ))
        .unwrap();
        assert_eq!(serde_json::to_value(&report).unwrap(), original);
        let summary = summarize(
            &report.obligations,
            &report.receipts,
            &report.dispositions,
            0,
        );
        assert_eq!(serde_json::to_value(summary).unwrap(), original["summary"]);
    }

    #[test]
    fn collapsed_intervention_cannot_qualify_a_later_automatic_pair() {
        for source in [
            CronSource::Scheduled,
            CronSource::Triggered,
            CronSource::Manual,
        ] {
            let mut report = fixture();
            let obligation = &mut report.obligations[0];
            obligation.installation_activated_at = -86400;
            obligation.activated_at = -86400;
            obligation.observed_at = -86400;
            let opportunities = &mut obligation.opportunities;
            let mut earlier = opportunities[0].clone();
            earlier.id = "earlier_failed_opportunity".into();
            earlier.due_at -= 86400;
            earlier.due_local = "1969-12-31T10:00:00+00:00".into();
            earlier.next_due_at -= 86400;
            earlier.coalesced_into = Some(opportunities[0].id.clone());
            let attempt = &mut earlier.attempts[0];
            attempt.receipt_id = crate::durable::CronReceiptId::new();
            attempt.started_at -= 86400;
            attempt.finished_at = attempt.finished_at.map(|finished| finished - 86400);
            attempt.source = source;
            attempt.covered = vec![earlier.id.clone()];
            attempt.selection = None;
            attempt.verification.clear();
            attempt.outcome = crate::ops::cron::accounting::ScheduledReleaseOutcome::Failed {
                cause: "verification failed before selecting a candidate".into(),
            };
            opportunities[0].attempts[0]
                .covered
                .push(earlier.id.clone());
            opportunities.insert(0, earlier);
            if source == CronSource::Manual {
                let earlier = obligation.opportunities.remove(0);
                let mut predecessor = obligation.clone();
                predecessor.id = "predecessor".into();
                predecessor.closed_at = Some(0);
                predecessor.opportunities = vec![earlier];
                obligation.replaces = Some(predecessor.id.clone());
                report.obligations.insert(0, predecessor);
            }

            let summary = summarize(
                &report.obligations,
                &report.receipts,
                &report.dispositions,
                0,
            );
            assert_eq!(summary.published, 1);
            assert_eq!(summary.no_change, 1);
            assert_eq!(
                summary.qualifying_pairs.len(),
                usize::from(source == CronSource::Scheduled),
                "source {source:?}"
            );
        }
    }

    #[test]
    fn collapsing_a_failed_due_preserves_its_first_attempt_timing() {
        let mut report = fixture();
        let opportunities = &mut report.obligations[0].opportunities;
        opportunities[0].coalesced_into = Some(opportunities[1].id.clone());
        opportunities[0].attempts[0].selection = None;
        opportunities[0].attempts[0].outcome =
            crate::ops::cron::accounting::ScheduledReleaseOutcome::Failed {
                cause: "verification failed before selection".into(),
            };
        let earlier = opportunities[0].id.clone();
        opportunities[1].attempts[0].covered.push(earlier);
        let summary = summarize(
            &report.obligations,
            &report.receipts,
            &report.dispositions,
            0,
        );
        assert_eq!(summary.on_time, 2);
        assert_eq!(summary.caught_up, 0);
        assert_eq!(summary.collapsed, 1);
    }

    #[test]
    fn collapsed_manual_unknown_or_unverified_rows_cannot_manufacture_a_pair() {
        for alteration in 0..7 {
            let mut report = fixture();
            let opportunity = &mut report.obligations[0].opportunities[1];
            match alteration {
                0 => opportunity.coalesced_into = Some("opportunity_0".into()),
                1 => opportunity.attempts[0].source = CronSource::Triggered,
                2 => opportunity.historical_timezone_unknown = true,
                3 => opportunity.attempts[0].verification.clear(),
                4 => opportunity.interventions.push(
                    crate::ops::cron::accounting::ReleaseIntervention {
                        recorded_at: 120000,
                        target: "default".into(),
                        operation: "manual publication repair".into(),
                    },
                ),
                5 => opportunity.attempts[0].verification[0].passed = false,
                6 => {
                    // A previous attempt's checks cannot qualify its unverified retry.
                    let mut retry = opportunity.attempts[0].clone();
                    retry.receipt_id = crate::durable::CronReceiptId::new();
                    retry.started_at += 60;
                    retry.finished_at = retry.finished_at.map(|finished| finished + 60);
                    retry.verification.clear();
                    opportunity.attempts[0].outcome =
                        crate::ops::cron::accounting::ScheduledReleaseOutcome::Failed {
                            cause: "failed after checks completed".into(),
                        };
                    opportunity.attempts.push(retry);
                }
                _ => unreachable!(),
            }
            let summary = summarize(
                &report.obligations,
                &report.receipts,
                &report.dispositions,
                0,
            );
            assert!(
                summary.qualifying_pairs.is_empty(),
                "alteration {alteration}"
            );
            assert_eq!(summary.failed_verifications, 1);
            assert_eq!(summary.late_dispositions.len(), 1);
        }
    }
}
