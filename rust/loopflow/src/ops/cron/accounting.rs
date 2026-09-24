//! Original due times survive late wakes, retries, and changes to installed jobs.
//! One obligation document atomically owns its opportunities and coalescing links.

use std::fs::{self, File, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};

use chrono::TimeZone;
use chrono_tz::Tz;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use super::{calendar, CronOutcome, CronReceipt, CronSource, CronSpec};
use crate::durable::{CronReceiptId, HomeId};
use crate::ops::{OpsError, OpsResult};

#[derive(Debug)]
pub(crate) struct CronExecution {
    pub receipt_id: String,
    pub lock_fd: i32,
}

pub(crate) fn validate_execution(
    home: &Path,
    context: &ReleaseObligation,
    execution: &CronExecution,
) -> OpsResult<()> {
    use std::os::fd::FromRawFd;
    use std::os::unix::fs::MetadataExt;
    if execution.lock_fd < 3 {
        return Err(failure("invalid cron execution descriptor"));
    }
    // SAFETY: dup validates the descriptor and transfers ownership of a new fd.
    let fd = unsafe { libc::dup(execution.lock_fd) };
    if fd < 0 {
        return Err(failure("cron execution lock was not inherited"));
    }
    // SAFETY: fd was returned by successful dup and is owned here.
    let inherited = unsafe { File::from_raw_fd(fd) };
    let key = fingerprint(&(&context.repo, &context.wave, &context.flow))?;
    let path = home.join("cron/locks").join(key);
    let expected = OpenOptions::new().read(true).write(true).open(path)?;
    let a = inherited.metadata()?;
    let b = expected.metadata()?;
    if a.dev() != b.dev() || a.ino() != b.ino() {
        return Err(failure("cron execution descriptor names a different job"));
    }
    match fs2::FileExt::try_lock_exclusive(&expected) {
        Ok(()) => return Err(failure("cron execution lock is no longer held")),
        Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => {}
        Err(e) => return Err(e.into()),
    }
    fs2::FileExt::try_lock_exclusive(&inherited)
        .map_err(|_| failure("descriptor does not own the active cron execution"))?;
    Ok(())
}

#[derive(Debug, Serialize, Deserialize)]
struct Trigger {
    id: String,
    requested_at: i64,
    receipt_id: Option<CronReceiptId>,
    error: Option<String>,
    expired_at: Option<i64>,
}

fn trigger_path(spec: &CronSpec) -> OpsResult<PathBuf> {
    let key = fingerprint(&(
        &spec.working_directory.canonicalize()?,
        (&spec.host.home_id, &spec.wave, &spec.flow),
    ))?;
    Ok(spec
        .host
        .lf_home
        .join("cron/triggers")
        .join(format!("{key}.json")))
}

fn read_triggers(path: &Path) -> OpsResult<Vec<Trigger>> {
    if !path.exists() {
        return Ok(Vec::new());
    }
    serde_json::from_slice(&fs::read(path)?)
        .map_err(|e| failure(format!("invalid trigger history {}: {e}", path.display())))
}

fn save_triggers(path: &Path, triggers: &[Trigger]) -> OpsResult<()> {
    let dir = path.parent().expect("trigger path has a directory");
    fs::create_dir_all(dir)?;
    let mut temp = tempfile::NamedTempFile::new_in(dir)?;
    serde_json::to_writer_pretty(&mut temp, triggers).map_err(|e| failure(e.to_string()))?;
    temp.as_file().sync_all()?;
    temp.persist(path).map_err(|e| failure(e.to_string()))?;
    File::open(dir)?.sync_all()?;
    Ok(())
}

pub(crate) fn record_trigger(spec: &CronSpec, now: i64) -> OpsResult<String> {
    let _lock = lock(&spec.host.lf_home)?;
    let path = trigger_path(spec)?;
    let mut triggers = read_triggers(&path)?;
    let id = format!("trigger_{}", uuid::Uuid::new_v4().simple());
    triggers.push(Trigger {
        id: id.clone(),
        requested_at: now,
        receipt_id: None,
        error: None,
        expired_at: None,
    });
    save_triggers(&path, &triggers)?;
    Ok(id)
}

pub(crate) fn trigger_failed(spec: &CronSpec, id: &str, cause: &str) -> OpsResult<()> {
    let _lock = lock(&spec.host.lf_home)?;
    let path = trigger_path(spec)?;
    let mut triggers = read_triggers(&path)?;
    let trigger = triggers
        .iter_mut()
        .find(|t| t.id == id)
        .ok_or_else(|| failure("trigger request missing"))?;
    trigger.error = Some(cause.into());
    save_triggers(&path, &triggers)
}

pub(crate) fn consume_trigger(spec: &CronSpec, receipt: &CronReceipt) -> OpsResult<bool> {
    let _lock = lock(&spec.host.lf_home)?;
    let path = trigger_path(spec)?;
    let mut triggers = read_triggers(&path)?;
    let mut found = false;
    for trigger in &mut triggers {
        if trigger.receipt_id.is_none() && trigger.requested_at <= receipt.started_at {
            if receipt.started_at > trigger.requested_at + 86400 {
                trigger.expired_at = Some(trigger.requested_at + 86400);
                // A delayed kickstart and a natural firing cannot be distinguished.
                // Preserve conservative intervention provenance for this first wake.
            }
            trigger.receipt_id = Some(receipt.id.clone());
            found = true;
        }
    }
    if found {
        save_triggers(&path, &triggers)?;
    }
    Ok(found)
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ReleaseObligation {
    pub schema_version: u32,
    pub id: String,
    pub repo: PathBuf,
    pub home_id: HomeId,
    pub wave: String,
    pub flow: String,
    pub schedule: String,
    pub timezone: String,
    pub installation_activated_at: i64,
    pub activated_at: i64,
    pub observed_at: i64,
    pub closed_at: Option<i64>,
    pub replaces: Option<String>,
    pub opportunities: Vec<ReleaseOpportunity>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ReleaseOpportunity {
    pub id: String,
    pub due_at: i64,
    pub due_local: String,
    pub next_due_at: i64,
    pub historical_timezone_unknown: bool,
    pub coalesced_into: Option<String>,
    pub overlapping_receipts: Vec<CronReceiptId>,
    pub interventions: Vec<ReleaseIntervention>,
    pub wait: Option<OpportunityWait>,
    pub attempts: Vec<ReleaseAttempt>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct OpportunityWait {
    pub recorded_at: i64,
    pub reason: String,
    pub retry_at: i64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ReleaseAttempt {
    pub receipt_id: CronReceiptId,
    pub started_at: i64,
    pub finished_at: Option<i64>,
    pub source: CronSource,
    pub covered: Vec<String>,
    pub selection: Option<ReleaseSelection>,
    pub target: Option<String>,
    pub telemetry: Option<TelemetryPrerequisite>,
    pub verification: Vec<VerificationEvidence>,
    pub outcome: ScheduledReleaseOutcome,
}

/// Frozen prerequisite observations for one release execution, before any retry.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TelemetryPrerequisite {
    pub schedule: String,
    pub timezone: String,
    pub activated_at: i64,
    pub observed_at: i64,
    pub original: Vec<TelemetryDue>,
    pub current_due_at: i64,
    pub current_receipts: Vec<CronReceiptId>,
    pub recovery_receipt: Option<CronReceiptId>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TelemetryDue {
    pub opportunity_id: String,
    pub due_at: Option<i64>,
    pub uncertainty: Option<String>,
    pub receipts: Vec<CronReceiptId>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ReleaseIntervention {
    pub recorded_at: i64,
    pub target: String,
    pub operation: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ReleaseSelection {
    pub tag: String,
    pub commit: String,
    pub workflow_run_id: Option<u64>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "status", rename_all = "snake_case")]
#[non_exhaustive]
pub enum ScheduledReleaseOutcome {
    Running,
    Deferred {
        reason: String,
        continuation: String,
    },
    Failed {
        cause: String,
    },
    Unverified {
        cause: String,
    },
    Published {
        tag: String,
        commit: String,
        workflow_run_id: u64,
    },
    NoChange {
        previous_tag: String,
        origin_commit: String,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct VerificationEvidence {
    pub name: String,
    pub subject: String,
    pub passed: bool,
    pub evidence_path: PathBuf,
    pub sha256: String,
}

impl ScheduledReleaseOutcome {
    fn settled(&self) -> bool {
        matches!(self, Self::Published { .. } | Self::NoChange { .. })
    }
}

impl ReleaseOpportunity {
    fn settled(&self) -> bool {
        self.attempts.last().is_some_and(|a| a.outcome.settled())
    }
}

fn failure(message: impl Into<String>) -> OpsError {
    OpsError::Message(message.into())
}

fn fingerprint(value: &impl Serialize) -> OpsResult<String> {
    let bytes = serde_json::to_vec(value).map_err(|e| failure(e.to_string()))?;
    Ok(hex::encode(Sha256::digest(bytes)))
}

fn directory(home: &Path) -> PathBuf {
    home.join("cron/obligations")
}

pub(crate) fn claim_execution(spec: &CronSpec) -> OpsResult<Option<File>> {
    let dir = spec.host.lf_home.join("cron/locks");
    fs::create_dir_all(&dir)?;
    let key = fingerprint(&(
        &spec.working_directory.canonicalize()?,
        &spec.wave,
        &spec.flow,
    ))?;
    let file = OpenOptions::new()
        .create(true)
        .truncate(false)
        .read(true)
        .write(true)
        .open(dir.join(key))?;
    match fs2::FileExt::try_lock_exclusive(&file) {
        Ok(()) => Ok(Some(file)),
        Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => Ok(None),
        Err(e) => Err(e.into()),
    }
}

pub(crate) fn record_overlap(
    home: &Path,
    obligation: &str,
    receipt: &CronReceipt,
) -> OpsResult<()> {
    let _lock = lock(home)?;
    let mut record = read(home)?
        .into_iter()
        .find(|r| r.id == obligation)
        .ok_or_else(|| failure(format!("missing release obligation {obligation}")))?;
    materialize(&mut record, receipt.started_at)?;
    // The active owner's attempt and frozen coverage are immutable to contenders.
    let active = record
        .opportunities
        .iter()
        .flat_map(|o| &o.attempts)
        .find(|a| a.finished_at.is_none())
        .map(|a| a.receipt_id.to_string());
    if let Some(latest) = record
        .opportunities
        .iter_mut()
        .rev()
        .find(|o| o.coalesced_into.is_none() && o.attempts.is_empty())
    {
        latest.attempts.push(ReleaseAttempt {
            receipt_id: receipt.id.clone(),
            started_at: receipt.started_at,
            finished_at: receipt.finished_at,
            source: receipt.source,
            covered: vec![latest.id.clone()],
            selection: None,
            target: None,
            telemetry: None,
            verification: Vec::new(),
            outcome: ScheduledReleaseOutcome::Deferred {
                reason: format!(
                    "release execution active: {}",
                    active.as_deref().unwrap_or("owner holds execution lock")
                ),
                continuation: "next configured firing".into(),
            },
        });
    }
    if let Some(due) = record.opportunities.last_mut() {
        if !due.overlapping_receipts.contains(&receipt.id) {
            due.overlapping_receipts.push(receipt.id.clone());
        }
    }
    save(home, &record)
}

pub(crate) fn preflight_failure(
    home: &Path,
    obligation: &str,
    receipt: &CronReceipt,
) -> OpsResult<()> {
    let _lock = lock(home)?;
    let mut record = read(home)?
        .into_iter()
        .find(|r| r.id == obligation)
        .ok_or_else(|| failure("preflight obligation missing"))?;
    materialize(&mut record, receipt.started_at)?;
    if let Some(due) = record
        .opportunities
        .iter_mut()
        .rev()
        .find(|o| o.coalesced_into.is_none() && !o.settled())
    {
        if !due
            .attempts
            .last()
            .is_some_and(|a| matches!(a.outcome, ScheduledReleaseOutcome::Running))
        {
            due.attempts.push(ReleaseAttempt {
                receipt_id: receipt.id.clone(),
                started_at: receipt.started_at,
                finished_at: receipt.finished_at,
                source: receipt.source,
                covered: vec![due.id.clone()],
                selection: due.attempts.last().and_then(|a| a.selection.clone()),
                target: due.attempts.last().and_then(|a| a.target.clone()),
                telemetry: None,
                verification: Vec::new(),
                outcome: ScheduledReleaseOutcome::Failed {
                    cause: receipt
                        .error
                        .clone()
                        .unwrap_or_else(|| "cron preflight failed".into()),
                },
            });
        }
    }
    save(home, &record)
}

pub(super) fn lock(home: &Path) -> OpsResult<File> {
    let dir = directory(home);
    fs::create_dir_all(&dir)?;
    let file = OpenOptions::new()
        .create(true)
        .truncate(false)
        .read(true)
        .write(true)
        .open(dir.join(".lock"))?;
    fs2::FileExt::lock_exclusive(&file)?;
    Ok(file)
}

fn save(home: &Path, obligation: &ReleaseObligation) -> OpsResult<()> {
    let dir = directory(home);
    let bytes = serde_json::to_vec_pretty(obligation).map_err(|e| failure(e.to_string()))?;
    let mut pending = tempfile::NamedTempFile::new_in(&dir)?;
    pending.write_all(&bytes)?;
    pending.as_file().sync_all()?;
    pending
        .persist(dir.join(format!("{}.json", obligation.id)))
        .map_err(|e| failure(format!("persist release obligation: {e}")))?;
    File::open(dir)?.sync_all()?;
    Ok(())
}

pub(super) fn read(home: &Path) -> OpsResult<Vec<ReleaseObligation>> {
    let dir = directory(home);
    if !dir.exists() {
        return Ok(Vec::new());
    }
    let mut result = Vec::new();
    for entry in fs::read_dir(dir)? {
        let path = entry?.path();
        if path.extension().is_none_or(|e| e != "json") {
            continue;
        }
        let record: ReleaseObligation = serde_json::from_slice(&fs::read(&path)?).map_err(|e| {
            failure(format!(
                "invalid release obligation {}: {e}",
                path.display()
            ))
        })?;
        if record.schema_version != 1
            || path.file_stem().and_then(|s| s.to_str()) != Some(record.id.as_str())
        {
            return Err(failure(format!(
                "unsupported or mismatched release obligation {}",
                path.display()
            )));
        }
        validate(&record)?;
        result.push(record);
    }
    let replacements: Vec<_> = result
        .iter()
        .filter_map(|r| {
            r.replaces.as_ref().map(|prior| {
                (
                    prior.clone(),
                    r.activated_at,
                    r.repo.clone(),
                    r.wave.clone(),
                    r.flow.clone(),
                )
            })
        })
        .collect();
    for (prior, boundary, repo, wave, flow) in replacements {
        let previous = result
            .iter_mut()
            .find(|r| r.id == prior)
            .ok_or_else(|| failure(format!("missing predecessor obligation {prior}")))?;
        if previous.repo != repo
            || previous.wave != wave
            || previous.flow != flow
            || previous.activated_at > boundary
        {
            return Err(failure("invalid obligation replacement boundary"));
        }
        previous.closed_at = Some(previous.closed_at.map_or(boundary, |end| end.min(boundary)));
    }
    result.sort_by_key(|r| r.activated_at);
    Ok(result)
}

fn validate(record: &ReleaseObligation) -> OpsResult<()> {
    let mut seen = std::collections::HashSet::new();
    for opportunity in &record.opportunities {
        if !seen.insert(&opportunity.id) {
            return Err(failure(format!("duplicate opportunity {}", opportunity.id)));
        }
        if let Some(owner) = &opportunity.coalesced_into {
            let target = record
                .opportunities
                .iter()
                .find(|o| &o.id == owner)
                .ok_or_else(|| failure(format!("missing coalescing owner {owner}")))?;
            if target.coalesced_into.is_some() || owner == &opportunity.id {
                return Err(failure(format!(
                    "invalid coalescing chain for {}",
                    opportunity.id
                )));
            }
            if !target
                .attempts
                .iter()
                .any(|a| a.covered.contains(&opportunity.id))
            {
                return Err(failure(format!(
                    "missing frozen coverage for {}",
                    opportunity.id
                )));
            }
        }
    }
    Ok(())
}

fn materialize(record: &mut ReleaseObligation, now: i64) -> OpsResult<()> {
    let zone: Tz = record
        .timezone
        .parse()
        .map_err(|e| failure(format!("invalid timezone: {e}")))?;
    let (hour, minute) = super::daily_time_of(&record.schedule)?;
    let start = record
        .opportunities
        .last()
        .map(|o| o.next_due_at)
        .unwrap_or_else(|| {
            let preceding = calendar::at_or_before(&zone, record.activated_at, hour, minute);
            match preceding {
                Some(due) if due == record.activated_at => due,
                Some(due) => calendar::after(&zone, due, hour, minute).unwrap_or(i64::MAX),
                None => i64::MAX,
            }
        });
    let mut due = start;
    while due <= now && record.closed_at.is_none_or(|end| due < end) {
        let next = calendar::after(&zone, due, hour, minute)
            .ok_or_else(|| failure("cannot compute next daily release opportunity"))?;
        record.opportunities.push(ReleaseOpportunity {
            id: format!("opportunity_{}", fingerprint(&(&record.id, due))?),
            due_at: due,
            due_local: zone
                .timestamp_opt(due, 0)
                .single()
                .ok_or_else(|| failure("invalid due timestamp"))?
                .to_rfc3339(),
            next_due_at: next,
            historical_timezone_unknown: due < record.observed_at,
            coalesced_into: None,
            overlapping_receipts: Vec::new(),
            interventions: Vec::new(),
            wait: None,
            attempts: Vec::new(),
        });
        due = next;
    }
    Ok(())
}

/// Persist the installed obligation before replacing its launchd job.
pub(crate) fn observe(
    spec: &CronSpec,
    activated_at: i64,
    now: i64,
    timezone: &str,
) -> OpsResult<String> {
    let _: Tz = timezone
        .parse()
        .map_err(|e| failure(format!("invalid timezone: {e}")))?;
    let _lock = lock(&spec.host.lf_home)?;
    let repo = spec.working_directory.canonicalize()?;
    let mut records = read(&spec.host.lf_home)?;
    records.retain(|r| r.repo == repo && r.wave == spec.wave && r.flow == spec.flow);
    if let Some(current) = records.iter().find(|r| {
        r.closed_at.is_none()
            && r.home_id == spec.host.home_id
            && r.schedule == spec.schedule.expression()
            && r.timezone == timezone
            && r.installation_activated_at == activated_at
    }) {
        return Ok(current.id.clone());
    }
    let segment_start = if records.is_empty() {
        activated_at
    } else {
        now
    };
    let replaces = records
        .iter()
        .find(|r| {
            !records
                .iter()
                .any(|child| child.replaces.as_ref() == Some(&r.id))
        })
        .map(|r| r.id.clone());
    let id = fingerprint(&(
        &repo,
        &spec.host.home_id,
        &spec.wave,
        &spec.flow,
        spec.schedule.expression(),
        segment_start,
        timezone,
        &replaces,
    ))?;
    let record = ReleaseObligation {
        schema_version: 1,
        id: id.clone(),
        repo,
        home_id: spec.host.home_id.clone(),
        wave: spec.wave.clone(),
        flow: spec.flow.clone(),
        schedule: spec.schedule.expression().to_string(),
        timezone: timezone.to_string(),
        installation_activated_at: activated_at,
        activated_at: segment_start,
        observed_at: now,
        closed_at: None,
        replaces,
        opportunities: Vec::new(),
    };
    // Publishing the successor also commits the predecessor's end boundary.
    // Readers apply that link even if the optional predecessor rewrite is interrupted.
    save(&spec.host.lf_home, &record)?;
    for mut previous in records {
        if previous.closed_at.is_none() {
            previous.closed_at = Some(now);
            materialize(&mut previous, now)?;
            save(&spec.host.lf_home, &previous)?;
        }
    }
    Ok(id)
}

pub(crate) fn close(spec: &CronSpec, now: i64) -> OpsResult<()> {
    let _lock = lock(&spec.host.lf_home)?;
    let repo = spec.working_directory.canonicalize()?;
    for mut record in read(&spec.host.lf_home)? {
        if record.repo == repo
            && record.wave == spec.wave
            && record.flow == spec.flow
            && record.closed_at.is_none()
        {
            record.closed_at = Some(now);
            materialize(&mut record, now)?;
            save(&spec.host.lf_home, &record)?;
        }
    }
    Ok(())
}

/// Read-only projection includes due times whose wake never happened.
pub fn history(
    home: &Path,
    repo: &Path,
    wave: &str,
    now: i64,
) -> OpsResult<Vec<ReleaseObligation>> {
    let repo = repo.canonicalize()?;
    let mut records = read(home)?;
    records.retain(|r| r.repo == repo && r.wave == wave);
    for record in &mut records {
        materialize(record, now)?;
    }
    Ok(records)
}

pub(crate) fn begin(
    home: &Path,
    obligation: &str,
    receipt: &CronReceipt,
) -> OpsResult<Option<String>> {
    let _lock = lock(home)?;
    let mut record = read(home)?
        .into_iter()
        .find(|r| r.id == obligation)
        .ok_or_else(|| failure(format!("missing release obligation {obligation}")))?;
    if record.repo != receipt.repo.canonicalize()?
        || record.home_id != receipt.home_id
        || record.wave != receipt.wave
        || record.flow != receipt.flow
        || record.schedule != receipt.schedule
    {
        return Err(failure("cron receipt does not match release obligation"));
    }
    materialize(&mut record, receipt.started_at)?;
    if let Some(owner) = record
        .opportunities
        .iter()
        .find(|o| o.attempts.iter().any(|a| a.receipt_id == receipt.id))
    {
        return Ok(Some(owner.id.clone()));
    }
    let outstanding: Vec<usize> = record
        .opportunities
        .iter()
        .enumerate()
        .filter(|(_, o)| o.coalesced_into.is_none() && !o.settled())
        .map(|(i, _)| i)
        .collect();
    // A prior execution owns its continuation; a fresh backlog belongs to its newest due time.
    let owner = outstanding
        .iter()
        .copied()
        .find(|i| {
            record.opportunities[*i]
                .attempts
                .last()
                .is_some_and(|a| a.selection.is_some())
        })
        .or_else(|| outstanding.last().copied());
    let Some(owner) = owner else {
        save(home, &record)?;
        return Ok(None);
    };
    let owner_id = record.opportunities[owner].id.clone();
    let mut covered: Vec<String> = outstanding
        .iter()
        .map(|i| record.opportunities[*i].id.clone())
        .collect();
    let roots = covered.clone();
    covered.extend(
        record
            .opportunities
            .iter()
            .filter(|o| {
                o.coalesced_into
                    .as_ref()
                    .is_some_and(|id| roots.contains(id))
            })
            .map(|o| o.id.clone()),
    );
    covered.sort();
    covered.dedup();
    for opportunity in &mut record.opportunities {
        if opportunity.id != owner_id && covered.contains(&opportunity.id) {
            opportunity.coalesced_into = Some(owner_id.clone());
        }
    }
    for opportunity in &mut record.opportunities {
        if covered.contains(&opportunity.id) {
            if let Some(attempt) = opportunity.attempts.last_mut() {
                if matches!(attempt.outcome, ScheduledReleaseOutcome::Running) {
                    attempt.outcome = ScheduledReleaseOutcome::Unverified {
                        cause: format!("previous cron execution released its lock without settlement; observed at {}", receipt.started_at),
                    };
                }
            }
        }
    }
    let interventions: Vec<_> = record
        .opportunities
        .iter()
        .filter(|o| covered.contains(&o.id))
        .flat_map(|o| o.interventions.clone())
        .collect();
    for intervention in interventions {
        if !record.opportunities[owner]
            .interventions
            .contains(&intervention)
        {
            record.opportunities[owner].interventions.push(intervention);
        }
    }
    let selection = record.opportunities[owner]
        .attempts
        .last()
        .and_then(|a| a.selection.clone());
    let target = if selection.is_some() {
        record.opportunities[owner]
            .attempts
            .last()
            .and_then(|a| a.target.clone())
    } else {
        None
    };
    record.opportunities[owner].attempts.push(ReleaseAttempt {
        receipt_id: receipt.id.clone(),
        started_at: receipt.started_at,
        finished_at: None,
        source: receipt.source,
        covered,
        selection,
        target,
        telemetry: None,
        verification: Vec::new(),
        outcome: ScheduledReleaseOutcome::Running,
    });
    validate(&record)?;
    save(home, &record)?;
    Ok(Some(owner_id))
}

pub(crate) fn finish_process(home: &Path, receipt: &CronReceipt) -> OpsResult<()> {
    let _lock = lock(home)?;
    for mut record in read(home)? {
        let Some(opportunity) = record
            .opportunities
            .iter_mut()
            .find(|o| o.attempts.iter().any(|a| a.receipt_id == receipt.id))
        else {
            continue;
        };
        let attempt = opportunity
            .attempts
            .iter_mut()
            .find(|a| a.receipt_id == receipt.id)
            .expect("attempt was found above");
        if matches!(attempt.outcome, ScheduledReleaseOutcome::Running) {
            attempt.finished_at = receipt.finished_at;
            attempt.outcome = match receipt.outcome {
                CronOutcome::Failed => ScheduledReleaseOutcome::Failed {
                    cause: receipt
                        .error
                        .clone()
                        .unwrap_or_else(|| "cron target failed without a diagnostic".to_string()),
                },
                CronOutcome::Succeeded => ScheduledReleaseOutcome::Unverified {
                    cause: "target exited successfully without verified release settlement"
                        .to_string(),
                },
                CronOutcome::Running => continue,
            };
        }
        let finished = receipt.finished_at.unwrap_or(receipt.started_at);
        materialize(&mut record, finished)?;
        let zone: Tz = record
            .timezone
            .parse()
            .map_err(|e| failure(format!("invalid timezone: {e}")))?;
        let (hour, minute) = super::daily_time_of(&record.schedule)?;
        let next = calendar::at_or_before(&zone, finished, hour, minute)
            .and_then(|due| calendar::after(&zone, due, hour, minute))
            .ok_or_else(|| failure("cannot compute next configured release firing"))?;
        for due in &mut record.opportunities {
            if due.due_at > receipt.started_at
                && due.attempts.is_empty()
                && due.coalesced_into.is_none()
            {
                due.wait = Some(OpportunityWait {
                    recorded_at: finished,
                    reason: "became due after this wake froze its coverage".into(),
                    retry_at: next,
                });
            }
        }
        save(home, &record)?;
        return Ok(());
    }
    Ok(())
}

fn rejected_settlement(
    home: &Path,
    receipt_id: &str,
    outcome: &ScheduledReleaseOutcome,
    verification: &[VerificationEvidence],
    now: i64,
    cause: &str,
) -> OpsResult<()> {
    let diagnostic = serde_json::json!({"receipt_id": receipt_id, "proposed_outcome": outcome,
        "verification": verification, "observed_at": now, "cause": cause});
    let dir = home.join("cron/rejected-settlements");
    fs::create_dir_all(&dir)?;
    let path = dir.join(format!("{}.json", fingerprint(&diagnostic)?));
    let mut pending = tempfile::NamedTempFile::new_in(&dir)?;
    serde_json::to_writer_pretty(&mut pending, &diagnostic).map_err(|e| failure(e.to_string()))?;
    pending.as_file().sync_all()?;
    pending.persist(&path).map_err(|e| failure(e.to_string()))?;
    File::open(dir)?.sync_all()?;
    Err(failure(format!(
        "{cause}; rejected input retained at {}",
        path.display()
    )))
}

pub(crate) fn settle(
    home: &Path,
    receipt_id: &str,
    outcome: ScheduledReleaseOutcome,
    verification: &[VerificationEvidence],
    now: i64,
) -> OpsResult<()> {
    let _lock = lock(home)?;
    for mut record in read(home)? {
        let Some(opportunity) = record.opportunities.iter_mut().find(|o| {
            o.attempts
                .iter()
                .any(|a| a.receipt_id.as_str() == receipt_id)
        }) else {
            continue;
        };
        if opportunity.coalesced_into.is_some() {
            return rejected_settlement(
                home,
                receipt_id,
                &outcome,
                verification,
                now,
                "release opportunity was collapsed into a newer execution",
            );
        }
        let attempt = opportunity
            .attempts
            .last_mut()
            .expect("matched opportunity has an attempt");
        if attempt.receipt_id.as_str() != receipt_id {
            return rejected_settlement(
                home,
                receipt_id,
                &outcome,
                verification,
                now,
                "release attempt was superseded; retaining the current outcome",
            );
        }
        if attempt.outcome.settled() {
            return if attempt.outcome == outcome && attempt.verification == verification {
                Ok(())
            } else {
                rejected_settlement(
                    home,
                    receipt_id,
                    &outcome,
                    verification,
                    now,
                    "conflicting release settlement; retaining accepted evidence",
                )
            };
        }
        attempt.outcome = outcome;
        attempt.verification = verification.to_vec();
        attempt.finished_at = Some(now);
        save(home, &record)?;
        return Ok(());
    }
    Err(failure(format!(
        "no release opportunity owns receipt {receipt_id}"
    )))
}

pub(crate) fn receipt_context(
    home: &Path,
    repo: &Path,
    receipt_id: &str,
) -> OpsResult<ReleaseObligation> {
    let repo = repo.canonicalize()?;
    let record = read(home)?
        .into_iter()
        .find(|r| {
            r.repo == repo
                && r.opportunities.iter().any(|o| {
                    o.coalesced_into.is_none()
                        && !o.settled()
                        && o.attempts
                            .last()
                            .is_some_and(|a| a.receipt_id.as_str() == receipt_id)
                })
        })
        .ok_or_else(|| {
            failure("release receipt does not own a current opportunity in this repository")
        })?;
    if record.closed_at.is_some() {
        return Err(failure(
            "release obligation was closed; inspect placement before recovery",
        ));
    }
    Ok(record)
}

pub(crate) fn record_target(home: &Path, receipt_id: &str, target: &str) -> OpsResult<()> {
    let _lock = lock(home)?;
    for mut record in read(home)? {
        if let Some(attempt) = record
            .opportunities
            .iter_mut()
            .filter(|o| o.coalesced_into.is_none())
            .filter_map(|o| o.attempts.last_mut())
            .find(|a| a.receipt_id.as_str() == receipt_id)
        {
            if attempt.target.as_ref().is_some_and(|prior| prior != target) {
                return Err(failure("cannot change the target of a scheduled attempt"));
            }
            attempt.target = Some(target.into());
            save(home, &record)?;
            return Ok(());
        }
    }
    Err(failure("scheduled attempt no longer owns its opportunity"))
}

/// Reserve recovery with the original observations in the same atomic write.
pub(crate) fn record_telemetry(
    home: &Path,
    receipt_id: &str,
    telemetry: &TelemetryPrerequisite,
) -> OpsResult<()> {
    let _lock = lock(home)?;
    for mut record in read(home)? {
        if record.closed_at.is_some() {
            continue;
        }
        let Some(attempt) = record
            .opportunities
            .iter_mut()
            .filter(|o| o.coalesced_into.is_none())
            .filter_map(|o| o.attempts.last_mut())
            .find(|a| a.receipt_id.as_str() == receipt_id)
        else {
            continue;
        };
        if !matches!(attempt.outcome, ScheduledReleaseOutcome::Running) {
            return Err(failure(
                "cannot attach telemetry to a finished release attempt",
            ));
        }
        if let Some(prior) = &attempt.telemetry {
            if prior == telemetry {
                return Ok(());
            }
            return Err(failure(
                "cannot replace a release attempt's telemetry or retry reservation",
            ));
        }
        if telemetry
            .original
            .iter()
            .map(|due| &due.opportunity_id)
            .collect::<Vec<_>>()
            != attempt.covered.iter().collect::<Vec<_>>()
        {
            return Err(failure(
                "telemetry observations do not match frozen release coverage",
            ));
        }
        attempt.telemetry = Some(telemetry.clone());
        return save(home, &record);
    }
    Err(failure("scheduled attempt no longer owns its opportunity"))
}

pub(crate) fn record_intervention(
    home: &Path,
    repo: &Path,
    target: &str,
    operation: &str,
) -> OpsResult<()> {
    let _lock = lock(home)?;
    let repo = repo.canonicalize()?;
    for mut record in read(home)?.into_iter().filter(|r| r.repo == repo) {
        let mut changed = false;
        for opportunity in &mut record.opportunities {
            if opportunity.coalesced_into.is_none()
                && !opportunity.settled()
                && opportunity
                    .attempts
                    .iter()
                    .any(|a| a.target.as_deref() == Some(target))
            {
                opportunity.interventions.push(ReleaseIntervention {
                    recorded_at: chrono::Utc::now().timestamp(),
                    target: target.into(),
                    operation: operation.into(),
                });
                changed = true;
            }
        }
        if changed {
            save(home, &record)?;
        }
    }
    Ok(())
}

pub(crate) fn select(home: &Path, receipt_id: &str, selection: ReleaseSelection) -> OpsResult<()> {
    let _lock = lock(home)?;
    for mut record in read(home)? {
        let Some(opportunity) = record.opportunities.iter_mut().find(|o| {
            o.attempts
                .last()
                .is_some_and(|a| a.receipt_id.as_str() == receipt_id)
        }) else {
            continue;
        };
        if opportunity.coalesced_into.is_some() {
            return Err(failure(
                "release opportunity was collapsed into a newer execution",
            ));
        }
        let attempt = opportunity.attempts.last_mut().expect("matched attempt");
        if attempt.outcome.settled() {
            return Err(failure("cannot change settled release selection"));
        }
        let mut selection = selection;
        if let Some(prior) = &attempt.selection {
            if prior.tag != selection.tag || prior.commit != selection.commit {
                return Err(failure("cannot replace the saved exact release candidate"));
            }
            if selection.workflow_run_id.is_none() {
                selection.workflow_run_id = prior.workflow_run_id;
            }
        }
        attempt.selection = Some(selection);
        save(home, &record)?;
        return Ok(());
    }
    Err(failure("release attempt no longer owns the opportunity"))
}

#[cfg(test)]
mod tests {
    use super::{
        begin, close, finish_process, history, observe, read, select, settle, ReleaseSelection,
        ScheduledReleaseOutcome,
    };
    use crate::durable::{CronReceiptId, HomeId};
    use crate::ops::{
        parse_schedule, CronHost, CronOutcome, CronReceipt, CronSource, CronSpec, CronTargetKind,
    };
    use std::path::Path;

    fn spec(root: &Path) -> CronSpec {
        CronSpec {
            wave: "infrastructure".into(),
            flow: "release-run".into(),
            target_kind: CronTargetKind::Flow,
            schedule: parse_schedule("0 0 10 * * *").unwrap(),
            working_directory: root.to_path_buf(),
            lf_path: root.join("lf"),
            host: CronHost {
                home_id: HomeId::new(),
                lf_home: root.join("home"),
                db_path: root.join("db"),
                path_env: "/usr/bin".into(),
            },
        }
    }

    fn receipt(spec: &CronSpec, now: i64) -> CronReceipt {
        CronReceipt {
            schema_version: 1,
            id: CronReceiptId::new(),
            runner_pid: 1,
            home_id: spec.host.home_id.clone(),
            wave: spec.wave.clone(),
            flow: spec.flow.clone(),
            target_kind: spec.target_kind,
            source: CronSource::Scheduled,
            schedule: spec.schedule.expression().into(),
            repo: spec.working_directory.clone(),
            lf_path: spec.lf_path.clone(),
            log_path: spec.working_directory.join("log"),
            started_at: now,
            finished_at: None,
            outcome: CronOutcome::Running,
            exit_code: None,
            error: None,
        }
    }

    #[test]
    fn telemetry_reservation_survives_retry_and_rejects_replacement_or_late_writer() {
        let temp = tempfile::tempdir().unwrap();
        let spec = spec(temp.path());
        let id = observe(&spec, 0, 0, "UTC").unwrap();
        let wake = receipt(&spec, 36001);
        let owner = begin(&spec.host.lf_home, &id, &wake).unwrap().unwrap();
        let prerequisite = super::TelemetryPrerequisite {
            schedule: "0 0 9 * * *".into(),
            timezone: "UTC".into(),
            activated_at: 0,
            observed_at: 36001,
            original: vec![super::TelemetryDue {
                opportunity_id: owner,
                due_at: Some(32400),
                uncertainty: None,
                receipts: vec![CronReceiptId::new()],
            }],
            current_due_at: 32400,
            current_receipts: Vec::new(),
            recovery_receipt: Some(CronReceiptId::new()),
        };
        super::record_telemetry(&spec.host.lf_home, wake.id.as_str(), &prerequisite).unwrap();
        super::record_telemetry(&spec.host.lf_home, wake.id.as_str(), &prerequisite).unwrap();
        let mut replacement = prerequisite.clone();
        replacement.recovery_receipt = Some(CronReceiptId::new());
        assert!(
            super::record_telemetry(&spec.host.lf_home, wake.id.as_str(), &replacement).is_err()
        );
        let next = receipt(&spec, 36002);
        begin(&spec.host.lf_home, &id, &next).unwrap();
        let before = read(&spec.host.lf_home).unwrap();
        assert!(
            super::record_telemetry(&spec.host.lf_home, wake.id.as_str(), &prerequisite).is_err()
        );
        let after = read(&spec.host.lf_home).unwrap();
        assert_eq!(after, before);
        let attempts = &after[0].opportunities[0].attempts;
        assert_eq!(attempts[0].telemetry.as_ref(), Some(&prerequisite));
        assert!(attempts[1].telemetry.is_none());
    }

    #[test]
    fn delayed_wake_collapses_three_due_times_and_keeps_new_due_time_separate() {
        let temp = tempfile::tempdir().unwrap();
        let spec = spec(temp.path());
        let id = observe(&spec, 0, 0, "UTC").unwrap();
        let mut wake = receipt(&spec, 2 * 86400 + 36001);
        let owner = begin(&spec.host.lf_home, &id, &wake).unwrap().unwrap();
        assert_eq!(
            begin(&spec.host.lf_home, &id, &wake).unwrap(),
            Some(owner.clone())
        );
        wake.outcome = CronOutcome::Succeeded;
        wake.finished_at = Some(3 * 86400 + 36001);
        finish_process(&spec.host.lf_home, &wake).unwrap();
        let rows = history(
            &spec.host.lf_home,
            temp.path(),
            &spec.wave,
            wake.finished_at.unwrap(),
        )
        .unwrap();
        let due = &rows[0].opportunities;
        assert_eq!(due.len(), 4);
        assert_eq!(due[0].coalesced_into.as_ref(), Some(&owner));
        assert_eq!(due[1].coalesced_into.as_ref(), Some(&owner));
        assert_eq!(due[2].attempts.len(), 1);
        assert_eq!(due[2].attempts[0].covered.len(), 3);
        assert!(matches!(
            due[2].attempts[0].outcome,
            ScheduledReleaseOutcome::Unverified { .. }
        ));
        assert!(due[3].coalesced_into.is_none());
        assert!(due[3].attempts.is_empty());
        assert_eq!(due[3].wait.as_ref().unwrap().retry_at, 4 * 86400 + 36000);
    }

    #[test]
    fn unchanged_sync_preserves_observation_and_failed_attempts_on_retry() {
        let temp = tempfile::tempdir().unwrap();
        let spec = spec(temp.path());
        let id = observe(&spec, 0, 0, "UTC").unwrap();
        assert_eq!(observe(&spec, 0, 86400, "UTC").unwrap(), id);
        let mut first = receipt(&spec, 36001);
        let owner = begin(&spec.host.lf_home, &id, &first).unwrap();
        select(
            &spec.host.lf_home,
            first.id.as_str(),
            ReleaseSelection {
                tag: "v1.2.3".into(),
                commit: "abc".into(),
                workflow_run_id: Some(42),
            },
        )
        .unwrap();
        first.outcome = CronOutcome::Failed;
        first.finished_at = Some(36002);
        first.error = Some("verification failed".into());
        finish_process(&spec.host.lf_home, &first).unwrap();
        let mut blocked = receipt(&spec, 36003);
        blocked.outcome = CronOutcome::Failed;
        blocked.finished_at = Some(36004);
        blocked.error = Some("placement authority unavailable".into());
        super::preflight_failure(&spec.host.lf_home, &id, &blocked).unwrap();
        let second = receipt(&spec, 86400 + 36001);
        assert_eq!(begin(&spec.host.lf_home, &id, &second).unwrap(), owner);
        let rows = history(
            &spec.host.lf_home,
            temp.path(),
            &spec.wave,
            second.started_at,
        )
        .unwrap();
        assert_eq!(rows[0].observed_at, 0);
        let attempts = &rows[0].opportunities[0].attempts;
        assert_eq!(attempts.len(), 3);
        assert_eq!(attempts[2].selection, attempts[0].selection);
        assert!(matches!(
            attempts[0].outcome,
            ScheduledReleaseOutcome::Failed { .. }
        ));
        assert_eq!(rows[0].opportunities[1].coalesced_into, owner);
    }
    #[test]
    fn failed_unselected_misses_collapse_into_newest_without_losing_failures() {
        let temp = tempfile::tempdir().unwrap();
        let spec = spec(temp.path());
        let id = observe(&spec, 0, 0, "UTC").unwrap();
        let first = receipt(&spec, 86400 + 36000);
        begin(&spec.host.lf_home, &id, &first).unwrap();
        settle(
            &spec.host.lf_home,
            first.id.as_str(),
            ScheduledReleaseOutcome::Failed {
                cause: "verification".into(),
            },
            &[],
            first.started_at + 1,
        )
        .unwrap();
        let second = receipt(&spec, 2 * 86400 + 36000);
        let owner = begin(&spec.host.lf_home, &id, &second).unwrap().unwrap();
        let record = read(&spec.host.lf_home).unwrap().remove(0);
        assert_eq!(record.opportunities.len(), 3);
        assert_eq!(
            record.opportunities[0].coalesced_into.as_ref(),
            Some(&owner)
        );
        assert_eq!(
            record.opportunities[1].coalesced_into.as_ref(),
            Some(&owner)
        );
        assert_eq!(record.opportunities[2].attempts[0].covered.len(), 3);
        assert!(matches!(
            record.opportunities[1].attempts[0].outcome,
            ScheduledReleaseOutcome::Failed { .. }
        ));
        assert!(settle(
            &spec.host.lf_home,
            first.id.as_str(),
            ScheduledReleaseOutcome::Running,
            &[],
            second.started_at
        )
        .is_err());
        assert_eq!(read(&spec.host.lf_home).unwrap()[0], record);
    }

    #[test]
    fn schedule_replacement_retains_due_population_and_corruption_is_not_absence() {
        let temp = tempfile::tempdir().unwrap();
        let mut spec = spec(temp.path());
        let id = observe(&spec, 0, 0, "UTC").unwrap();
        close(&spec, 86400).unwrap();
        spec.schedule = parse_schedule("0 0 11 * * *").unwrap();
        let next = observe(&spec, 86400, 86400, "UTC").unwrap();
        assert_ne!(id, next);
        let rows = history(&spec.host.lf_home, temp.path(), &spec.wave, 86400 + 40000).unwrap();
        assert_eq!(rows.len(), 2);
        assert_eq!(rows[0].opportunities[0].due_at, 36000);
        assert_eq!(rows[0].closed_at, Some(86400));
        assert_eq!(rows[1].opportunities[0].due_at, 86400 + 39600);
        let path = spec
            .host
            .lf_home
            .join("cron/obligations")
            .join(format!("{next}.json"));
        std::fs::write(&path, "{incomplete").unwrap();
        assert!(history(&spec.host.lf_home, temp.path(), &spec.wave, 200000)
            .unwrap_err()
            .to_string()
            .contains(path.to_str().unwrap()));
        assert_eq!(std::fs::read_to_string(path).unwrap(), "{incomplete");
    }

    #[test]
    fn settlement_is_idempotent_and_cannot_be_replaced_by_a_late_process_failure() {
        let temp = tempfile::tempdir().unwrap();
        let spec = spec(temp.path());
        let id = observe(&spec, 0, 0, "UTC").unwrap();
        let mut wake = receipt(&spec, 36000);
        begin(&spec.host.lf_home, &id, &wake).unwrap();
        let outcome = ScheduledReleaseOutcome::NoChange {
            previous_tag: "v1.2.3".into(),
            origin_commit: "abc".into(),
        };
        let verification = vec![super::VerificationEvidence {
            name: "repository-verification".into(),
            subject: "abc".into(),
            passed: true,
            evidence_path: "checks/abc.json".into(),
            sha256: "retained-check-digest".into(),
        }];
        settle(
            &spec.host.lf_home,
            wake.id.as_str(),
            outcome.clone(),
            &verification,
            36001,
        )
        .unwrap();
        settle(
            &spec.host.lf_home,
            wake.id.as_str(),
            outcome.clone(),
            &verification,
            36002,
        )
        .unwrap();
        assert!(settle(
            &spec.host.lf_home,
            wake.id.as_str(),
            ScheduledReleaseOutcome::Failed {
                cause: "late result".into()
            },
            &verification,
            36003
        )
        .is_err());
        let accepted = read(&spec.host.lf_home).unwrap();
        let mut conflicting = verification.clone();
        conflicting[0].passed = false;
        assert!(settle(
            &spec.host.lf_home,
            wake.id.as_str(),
            outcome.clone(),
            &conflicting,
            36003,
        )
        .is_err());
        assert_eq!(read(&spec.host.lf_home).unwrap(), accepted);
        let diagnostics: Vec<serde_json::Value> =
            std::fs::read_dir(spec.host.lf_home.join("cron/rejected-settlements"))
                .unwrap()
                .map(|entry| {
                    serde_json::from_slice(&std::fs::read(entry.unwrap().path()).unwrap()).unwrap()
                })
                .collect();
        assert!(diagnostics
            .iter()
            .any(|d| d["verification"] == serde_json::to_value(&conflicting).unwrap()));
        wake.finished_at = Some(36004);
        wake.outcome = CronOutcome::Failed;
        finish_process(&spec.host.lf_home, &wake).unwrap();
        let record = read(&spec.host.lf_home).unwrap().remove(0);
        assert_eq!(record.opportunities[0].attempts[0].outcome, outcome);
        assert_eq!(
            record.opportunities[0].attempts[0].verification,
            verification
        );
        assert_eq!(record.opportunities[0].attempts[0].finished_at, Some(36001));
        let repeat = receipt(&spec, 36005);
        assert_eq!(begin(&spec.host.lf_home, &id, &repeat).unwrap(), None);
    }
    #[test]
    fn manual_repair_and_trigger_provenance_survive_automatic_retry() {
        let temp = tempfile::tempdir().unwrap();
        let spec = spec(temp.path());
        let id = observe(&spec, 0, 0, "UTC").unwrap();
        let first = receipt(&spec, 36000);
        begin(&spec.host.lf_home, &id, &first).unwrap();
        super::record_target(&spec.host.lf_home, first.id.as_str(), "default").unwrap();
        settle(
            &spec.host.lf_home,
            first.id.as_str(),
            ScheduledReleaseOutcome::Failed {
                cause: "missing check".into(),
            },
            &[],
            36001,
        )
        .unwrap();
        super::record_intervention(
            &spec.host.lf_home,
            temp.path(),
            "default",
            "manual release run",
        )
        .unwrap();
        let next = receipt(&spec, 86400 + 36000);
        begin(&spec.host.lf_home, &id, &next).unwrap();
        let record = read(&spec.host.lf_home).unwrap().remove(0);
        assert_eq!(record.opportunities[1].interventions.len(), 1);
        assert_eq!(record.opportunities[0].attempts.len(), 1);
        let trigger = super::record_trigger(&spec, next.started_at + 1).unwrap();
        super::trigger_failed(&spec, &trigger, "ambiguous launchctl failure").unwrap();
        let later = receipt(&spec, next.started_at + 86402);
        assert!(super::consume_trigger(&spec, &later).unwrap());
        assert!(!super::consume_trigger(&spec, &later).unwrap());
        let requests = super::read_triggers(&super::trigger_path(&spec).unwrap()).unwrap();
        assert_eq!(requests[0].receipt_id.as_ref(), Some(&later.id));
        assert!(requests[0].expired_at.is_some());
        assert_eq!(
            requests[0].error.as_deref(),
            Some("ambiguous launchctl failure")
        );
    }

    #[test]
    fn receipt_identity_without_exact_inherited_execution_lock_is_insufficient() {
        use std::os::fd::AsRawFd;
        let temp = tempfile::tempdir().unwrap();
        let spec = spec(temp.path());
        let id = observe(&spec, 0, 0, "UTC").unwrap();
        let wake = receipt(&spec, 36000);
        begin(&spec.host.lf_home, &id, &wake).unwrap();
        let context =
            super::receipt_context(&spec.host.lf_home, temp.path(), wake.id.as_str()).unwrap();
        let lock = super::claim_execution(&spec).unwrap().unwrap();
        let mut execution = super::CronExecution {
            receipt_id: wake.id.to_string(),
            lock_fd: lock.as_raw_fd(),
        };
        super::validate_execution(&spec.host.lf_home, &context, &execution).unwrap();
        execution.lock_fd = 1;
        assert!(super::validate_execution(&spec.host.lf_home, &context, &execution).is_err());
    }
    #[test]
    fn replacement_link_survives_interrupted_predecessor_write_and_same_second_changes() {
        let temp = tempfile::tempdir().unwrap();
        let mut spec = spec(temp.path());
        let first = observe(&spec, 0, 0, "UTC").unwrap();
        let old = read(&spec.host.lf_home).unwrap().remove(0);
        spec.schedule = parse_schedule("0 0 11 * * *").unwrap();
        let second = observe(&spec, 86400, 86400, "UTC").unwrap();
        // The successor reached disk; its predecessor rewrite did not.
        super::save(&spec.host.lf_home, &old).unwrap();
        let rows = read(&spec.host.lf_home).unwrap();
        assert_eq!(
            rows.iter().find(|r| r.id == first).unwrap().closed_at,
            Some(86400)
        );
        assert_eq!(observe(&spec, 86400, 90000, "UTC").unwrap(), second);
        spec.schedule = parse_schedule("0 0 10 * * *").unwrap();
        let third = observe(&spec, 86400, 86400, "UTC").unwrap();
        assert_ne!(first, third);
        assert_ne!(second, third);
        assert_eq!(read(&spec.host.lf_home).unwrap().len(), 3);
    }
}
