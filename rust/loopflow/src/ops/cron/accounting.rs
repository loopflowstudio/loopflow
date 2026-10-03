//! Original due times survive late wakes, retries, and changes to installed jobs.
//! One attempt write freezes coverage across same-Home segments; readers derive owners.

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
    context: &ObligationSegment,
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

/// One observed calendar/placement segment; only release jobs own opportunities.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ObligationSegment {
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
    pub execution_obligation: Option<String>,
    pub selection: Option<ReleaseSelection>,
    pub replacement: Option<CandidateReplacement>,
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
    pub obligation_id: Option<String>,
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

/// Exact rejected source and affirmative unpublished evidence authorizing one successor.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CandidateReplacement {
    pub rejected: ReleaseSelection,
    pub inspection: VerificationEvidence,
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

impl ObligationSegment {
    /// Unfinished execution owners retain repair obligations after their schedule closes.
    pub fn closed_unsettled(&self) -> impl Iterator<Item = &ReleaseOpportunity> {
        self.opportunities
            .iter()
            .filter(|o| self.closed_at.is_some() && o.coalesced_into.is_none() && !o.settled())
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
    if let Some(index) = record
        .opportunities
        .iter()
        .rposition(|o| o.coalesced_into.is_none() && o.attempts.is_empty())
    {
        let continuation =
            retry_continuation(&record, &record.opportunities[index].id, receipt.started_at)?;
        let latest = &mut record.opportunities[index];
        latest.attempts.push(ReleaseAttempt {
            receipt_id: receipt.id.clone(),
            started_at: receipt.started_at,
            finished_at: receipt.finished_at,
            source: receipt.source,
            covered: vec![latest.id.clone()],
            execution_obligation: Some(obligation.into()),
            selection: None,
            replacement: None,
            target: None,
            telemetry: None,
            verification: Vec::new(),
            outcome: ScheduledReleaseOutcome::Deferred {
                reason: format!(
                    "release execution active: {}",
                    active.as_deref().unwrap_or("owner holds execution lock")
                ),
                continuation,
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
                execution_obligation: Some(obligation.into()),
                selection: due.attempts.last().and_then(|a| a.selection.clone()),
                replacement: None,
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

fn save(home: &Path, obligation: &ObligationSegment) -> OpsResult<()> {
    let dir = directory(home);
    let mut document = serde_json::to_value(obligation).map_err(|e| failure(e.to_string()))?;
    // Collapse links are a projection of frozen attempt coverage, never a second writer.
    for due in document["opportunities"]
        .as_array_mut()
        .expect("opportunities is an array")
    {
        due.as_object_mut()
            .expect("opportunity is an object")
            .remove("coalesced_into");
    }
    let bytes = serde_json::to_vec_pretty(&document).map_err(|e| failure(e.to_string()))?;
    let mut pending = tempfile::NamedTempFile::new_in(&dir)?;
    pending.write_all(&bytes)?;
    pending.as_file().sync_all()?;
    pending
        .persist(dir.join(format!("{}.json", obligation.id)))
        .map_err(|e| failure(format!("persist cron obligation: {e}")))?;
    File::open(dir)?.sync_all()?;
    Ok(())
}

pub(super) fn read(home: &Path) -> OpsResult<Vec<ObligationSegment>> {
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
        let record: ObligationSegment = serde_json::from_slice(&fs::read(&path)?)
            .map_err(|e| failure(format!("invalid cron obligation {}: {e}", path.display())))?;
        if record.schema_version != 1
            || path.file_stem().and_then(|s| s.to_str()) != Some(record.id.as_str())
        {
            return Err(failure(format!(
                "unsupported or mismatched cron obligation {}",
                path.display()
            )));
        }
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
    derive_coverage(&mut result)?;
    result.sort_by_key(|r| r.activated_at);
    Ok(result)
}

// A successor can continue only its uninterrupted same-Home predecessor chain.
fn predecessors(records: &[ObligationSegment], start: usize) -> OpsResult<Vec<usize>> {
    let mut chain = vec![start];
    let mut current = start;
    while let Some(id) = &records[current].replaces {
        let prior = records
            .iter()
            .position(|r| &r.id == id)
            .ok_or_else(|| failure(format!("missing predecessor obligation {id}")))?;
        if chain.contains(&prior) {
            return Err(failure("cyclic obligation replacement"));
        }
        if records[prior].home_id != records[start].home_id {
            break;
        }
        chain.push(prior);
        current = prior;
    }
    Ok(chain)
}

fn derive_coverage(records: &mut [ObligationSegment]) -> OpsResult<()> {
    use std::collections::{HashMap, HashSet};
    let mut locations = HashMap::new();
    for (segment, record) in records.iter().enumerate() {
        for due in &record.opportunities {
            if locations.insert(due.id.clone(), segment).is_some() {
                return Err(failure(format!("duplicate opportunity {}", due.id)));
            }
        }
    }
    let mut claims: HashMap<String, HashSet<String>> = HashMap::new();
    for (segment, record) in records.iter().enumerate() {
        for owner in &record.opportunities {
            for attempt in &owner.attempts {
                let execution = match &attempt.execution_obligation {
                    Some(id) => records
                        .iter()
                        .position(|r| &r.id == id)
                        .ok_or_else(|| failure(format!("missing execution obligation {id}")))?,
                    None => segment,
                };
                let chain = predecessors(records, execution)?;
                if !chain.contains(&segment) || !attempt.covered.contains(&owner.id) {
                    return Err(failure("invalid release coverage owner"));
                }
                for id in &attempt.covered {
                    let location = locations
                        .get(id)
                        .ok_or_else(|| failure(format!("missing covered opportunity {id}")))?;
                    if !chain.contains(location) {
                        return Err(failure("release coverage crosses obligation authority"));
                    }
                    if id != &owner.id {
                        claims
                            .entry(id.clone())
                            .or_default()
                            .insert(owner.id.clone());
                    }
                }
            }
        }
    }
    fn root(
        id: &str,
        claims: &HashMap<String, HashSet<String>>,
        visiting: &mut HashSet<String>,
        resolved: &mut HashMap<String, String>,
    ) -> OpsResult<String> {
        if let Some(owner) = resolved.get(id) {
            return Ok(owner.clone());
        }
        if !visiting.insert(id.into()) {
            return Err(failure("cyclic release coverage"));
        }
        let mut roots = HashSet::new();
        if let Some(owners) = claims.get(id) {
            for owner in owners {
                roots.insert(root(owner, claims, visiting, resolved)?);
            }
        } else {
            roots.insert(id.to_string());
        }
        visiting.remove(id);
        if roots.len() != 1 {
            return Err(failure(format!("conflicting frozen coverage for {id}")));
        }
        let owner = roots.into_iter().next().expect("one coverage owner");
        resolved.insert(id.into(), owner.clone());
        Ok(owner)
    }
    let mut resolved = HashMap::new();
    for record in records {
        for due in &mut record.opportunities {
            let owner = root(&due.id, &claims, &mut HashSet::new(), &mut resolved)?;
            due.coalesced_into = (owner != due.id).then_some(owner);
        }
    }
    Ok(())
}

pub(super) fn materialize(record: &mut ObligationSegment, now: i64) -> OpsResult<()> {
    if record.flow != "release-run" {
        return Ok(());
    }
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
    let record = ObligationSegment {
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
) -> OpsResult<Vec<ObligationSegment>> {
    let repo = repo.canonicalize()?;
    let _lock = directory(home).exists().then(|| lock(home)).transpose()?;
    let mut records = read(home)?;
    records.retain(|r| r.repo == repo && r.wave == wave);
    for record in &mut records {
        materialize(record, now)?;
    }
    Ok(records)
}

/// Bind original release dues to observed telemetry segments, never today's schedule.
pub(crate) fn telemetry_due(
    segments: &[ObligationSegment],
    context: &ObligationSegment,
    opportunity: &ReleaseOpportunity,
    receipts: &[CronReceipt],
) -> OpsResult<TelemetryDue> {
    let mut result = TelemetryDue {
        opportunity_id: opportunity.id.clone(),
        obligation_id: None,
        due_at: None,
        uncertainty: Some(format!(
            "no retained telemetry obligation on Home {} at original release due {}",
            context.home_id, opportunity.due_at
        )),
        receipts: Vec::new(),
    };
    let Some(segment) = segments.iter().find(|s| {
        s.repo == context.repo
            && s.wave == context.wave
            && s.flow == "telemetry-daily"
            && s.home_id == context.home_id
            && s.activated_at <= opportunity.due_at
            && s.closed_at.is_none_or(|end| opportunity.due_at < end)
    }) else {
        return Ok(result);
    };
    result.obligation_id = Some(segment.id.clone());
    let zone: Tz = segment
        .timezone
        .parse()
        .map_err(|e| failure(format!("invalid timezone: {e}")))?;
    let (hour, minute) = super::daily_time_of(&segment.schedule)?;
    let due = calendar::at_or_before(&zone, opportunity.due_at, hour, minute)
        .ok_or_else(|| failure("cannot identify original telemetry interval"))?;
    if due < segment.activated_at {
        result.uncertainty =
            Some("original release precedes this telemetry segment's first due".into());
        return Ok(result);
    }
    if due < segment.observed_at {
        result.uncertainty = Some(
            "telemetry schedule or timezone was not observed for this original due time".into(),
        );
        return Ok(result);
    }
    let end = calendar::after(&zone, due, hour, minute)
        .ok_or_else(|| failure("cannot identify telemetry interval end"))?;
    let end = segment.closed_at.map_or(end, |closed| end.min(closed));
    result.due_at = Some(due);
    result.uncertainty = None;
    result.receipts = receipts
        .iter()
        .filter(|r| {
            (r.repo == segment.repo || r.repo.canonicalize().ok().as_ref() == Some(&segment.repo))
                && r.home_id == segment.home_id
                && r.wave == segment.wave
                && r.flow == segment.flow
                && r.schedule == segment.schedule
                && r.source == CronSource::Scheduled
                && r.target_kind == super::CronTargetKind::Flow
                && r.started_at >= due
                && r.started_at < end
        })
        .map(|r| r.id.clone())
        .collect();
    Ok(result)
}

pub(crate) fn begin(
    home: &Path,
    obligation: &str,
    receipt: &CronReceipt,
) -> OpsResult<Option<String>> {
    let _lock = lock(home)?;
    let mut records = read(home)?;
    let execution = records
        .iter()
        .position(|r| r.id == obligation)
        .ok_or_else(|| failure(format!("missing release obligation {obligation}")))?;
    let record = &records[execution];
    if record.repo != receipt.repo.canonicalize()?
        || record.home_id != receipt.home_id
        || record.wave != receipt.wave
        || record.flow != receipt.flow
        || record.schedule != receipt.schedule
        || record.closed_at.is_some()
    {
        return Err(failure(
            "cron receipt does not match an open release obligation",
        ));
    }
    // Re-entry cannot expand an execution's already frozen due set.
    if let Some(owner) = records
        .iter()
        .flat_map(|r| &r.opportunities)
        .find(|o| o.attempts.iter().any(|a| a.receipt_id == receipt.id))
    {
        return Ok(Some(owner.id.clone()));
    }
    let chain = predecessors(&records, execution)?;
    // Each id must exist durably before the single authoritative owner write.
    // An interrupted prefix leaves unclaimed dues that the next wake can materialize again.
    for index in &chain {
        materialize(&mut records[*index], receipt.started_at)?;
        save(home, &records[*index])?;
    }
    let mut outstanding = Vec::new();
    for segment in &chain {
        for (due, opportunity) in records[*segment].opportunities.iter().enumerate() {
            if opportunity.coalesced_into.is_none() && !opportunity.settled() {
                outstanding.push((*segment, due));
            }
        }
    }
    outstanding.sort_by_key(|(s, d)| records[*s].opportunities[*d].due_at);
    let selected: Vec<_> = outstanding
        .iter()
        .copied()
        .filter(|(s, d)| {
            records[*s].opportunities[*d]
                .attempts
                .last()
                .is_some_and(|a| a.selection.is_some())
        })
        .collect();
    if selected.len() > 1 {
        return Err(failure(
            "multiple outstanding release candidates require repair before continuation",
        ));
    }
    let Some((segment, due)) = selected
        .first()
        .copied()
        .or_else(|| outstanding.last().copied())
    else {
        return Ok(None);
    };
    let roots: Vec<_> = outstanding
        .iter()
        .map(|(s, d)| records[*s].opportunities[*d].id.clone())
        .collect();
    let mut covered: Vec<_> = chain
        .iter()
        .flat_map(|s| &records[*s].opportunities)
        .filter(|o| {
            roots.contains(&o.id)
                || o.coalesced_into
                    .as_ref()
                    .is_some_and(|id| roots.contains(id))
        })
        .map(|o| o.id.clone())
        .collect();
    covered.sort();
    let owner = &mut records[segment].opportunities[due];
    let owner_id = owner.id.clone();
    let selection = owner.attempts.last().and_then(|a| a.selection.clone());
    let target = selection
        .as_ref()
        .and_then(|_| owner.attempts.last().and_then(|a| a.target.clone()));
    if let Some(attempt) = owner.attempts.last_mut() {
        if matches!(attempt.outcome, ScheduledReleaseOutcome::Running) {
            attempt.outcome = ScheduledReleaseOutcome::Unverified {
                cause: format!(
                    "previous cron execution released its lock without settlement; observed at {}",
                    receipt.started_at
                ),
            };
        }
    }
    owner.attempts.push(ReleaseAttempt {
        receipt_id: receipt.id.clone(),
        started_at: receipt.started_at,
        finished_at: None,
        source: receipt.source,
        covered,
        execution_obligation: Some(obligation.into()),
        selection,
        replacement: None,
        target,
        telemetry: None,
        verification: Vec::new(),
        outcome: ScheduledReleaseOutcome::Running,
    });
    derive_coverage(&mut records)?;
    save(home, &records[segment])?;
    Ok(Some(owner_id))
}

pub(crate) fn finish_process(home: &Path, receipt: &CronReceipt) -> OpsResult<()> {
    let _lock = lock(home)?;
    let mut records = read(home)?;
    let Some(owner) = records.iter().position(|r| {
        r.opportunities
            .iter()
            .any(|o| o.attempts.iter().any(|a| a.receipt_id == receipt.id))
    }) else {
        return Ok(());
    };
    let attempt = records[owner]
        .opportunities
        .iter_mut()
        .flat_map(|o| &mut o.attempts)
        .find(|a| a.receipt_id == receipt.id)
        .expect("receipt owner was found");
    let execution_id = attempt.execution_obligation.clone();
    if matches!(attempt.outcome, ScheduledReleaseOutcome::Running) {
        attempt.finished_at = receipt.finished_at;
        attempt.outcome = match receipt.outcome {
            CronOutcome::Failed => ScheduledReleaseOutcome::Failed {
                cause: receipt
                    .error
                    .clone()
                    .unwrap_or_else(|| "cron target failed without a diagnostic".into()),
            },
            CronOutcome::Succeeded => ScheduledReleaseOutcome::Unverified {
                cause: "target exited successfully without verified release settlement".into(),
            },
            CronOutcome::Running => return Ok(()),
        };
    }
    let execution = match execution_id {
        Some(id) => records
            .iter()
            .position(|r| r.id == id)
            .ok_or_else(|| failure("missing release execution obligation"))?,
        None => owner,
    };
    let record = &mut records[execution];
    let finished = receipt.finished_at.unwrap_or(receipt.started_at);
    materialize(record, finished)?;
    if record.closed_at.is_none() {
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
    }
    save(home, &records[owner])?;
    if execution != owner {
        save(home, &records[execution])?;
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
) -> OpsResult<ObligationSegment> {
    let repo = repo.canonicalize()?;
    let _lock = lock(home)?;
    let records = read(home)?;
    let record = records
        .iter()
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
    if execution_context(&records, receipt_id)?.closed_at.is_some() {
        return Err(failure(
            "release execution obligation was closed; inspect placement before recovery",
        ));
    }
    Ok(record.clone())
}

pub(crate) fn execution_context<'a>(
    records: &'a [ObligationSegment],
    receipt_id: &str,
) -> OpsResult<&'a ObligationSegment> {
    for record in records {
        for due in &record.opportunities {
            if let Some(attempt) = due
                .attempts
                .iter()
                .find(|a| a.receipt_id.as_str() == receipt_id)
            {
                let id = attempt.execution_obligation.as_ref().unwrap_or(&record.id);
                return records
                    .iter()
                    .find(|r| &r.id == id)
                    .ok_or_else(|| failure("missing release execution obligation"));
            }
        }
    }
    Err(failure("release attempt missing"))
}

pub(crate) fn overlap_continuation(home: &Path, receipt_id: &str, now: i64) -> OpsResult<String> {
    let _lock = lock(home)?;
    let records = read(home)?;
    for record in &records {
        if let Some(opportunity) = record.opportunities.iter().find(|o| {
            o.coalesced_into.is_none()
                && !o.settled()
                && o.attempts
                    .last()
                    .is_some_and(|a| a.receipt_id.as_str() == receipt_id)
        }) {
            return retry_continuation(
                execution_context(&records, receipt_id)?,
                &opportunity.id,
                now,
            );
        }
    }
    Err(failure("release attempt no longer owns the opportunity"))
}

fn retry_continuation(
    record: &ObligationSegment,
    opportunity: &str,
    now: i64,
) -> OpsResult<String> {
    if let Some(closed) = record.closed_at {
        return Ok(format!(
            "obligation {} closed at {closed}; no future firing on original Home {}; record repair there: lf cron disposition {opportunity} --wave {} --owner <task-work-id> --reason <repair-plan>",
            record.id, record.home_id, record.wave
        ));
    }
    let zone: Tz = record
        .timezone
        .parse()
        .map_err(|e| failure(format!("invalid timezone: {e}")))?;
    let (hour, minute) = super::daily_time_of(&record.schedule)?;
    let next = calendar::at_or_before(&zone, now, hour, minute)
        .and_then(|due| calendar::after(&zone, due, hour, minute))
        .ok_or_else(|| failure("cannot compute next configured release firing"))?;
    Ok(format!(
        "next configured release due {next}; obligation {}; Home {}; observed at {now}",
        record.id, record.home_id
    ))
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

pub(crate) fn authorize_replacement(
    home: &Path,
    receipt_id: &str,
    rejected: &ReleaseSelection,
    inspection: VerificationEvidence,
) -> OpsResult<()> {
    let _lock = lock(home)?;
    for mut record in read(home)? {
        let Some(opportunity) = record.opportunities.iter_mut().find(|o| {
            o.attempts
                .last()
                .is_some_and(|a| a.receipt_id.as_str() == receipt_id)
        }) else {
            continue;
        };
        let attempt = opportunity.attempts.last_mut().expect("matched attempt");
        if opportunity.coalesced_into.is_some()
            || !matches!(attempt.outcome, ScheduledReleaseOutcome::Running)
            || attempt.selection.as_ref() != Some(rejected)
            || attempt.replacement.is_some()
        {
            return Err(failure(
                "release replacement no longer owns the exact running candidate",
            ));
        }
        attempt.replacement = Some(CandidateReplacement {
            rejected: rejected.clone(),
            inspection,
        });
        save(home, &record)?;
        return Ok(());
    }
    Err(failure("release attempt no longer owns the opportunity"))
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
        if !matches!(attempt.outcome, ScheduledReleaseOutcome::Running) {
            return Err(failure("cannot change a finished release selection"));
        }
        let mut selection = selection;
        if let Some(prior) = &attempt.selection {
            if prior.tag != selection.tag || prior.commit != selection.commit {
                if !attempt
                    .replacement
                    .as_ref()
                    .is_some_and(|r| &r.rejected == prior)
                {
                    return Err(failure("cannot replace the saved exact release candidate"));
                }
            } else if selection.workflow_run_id.is_none() {
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
                path_env: "/usr/bin".into(),
            },
        }
    }

    fn receipt(spec: &CronSpec, now: i64) -> CronReceipt {
        CronReceipt {
            schema_version: 1,
            id: CronReceiptId::new(),
            runner_pid: 1,
            runner_started_at: None,
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
    fn replacement_wake_recovers_atomic_coverage_at_every_write_boundary() {
        use crate::ops::cron::history::release_history;
        for boundary in 0..=4 {
            let temp = tempfile::tempdir().unwrap();
            let mut job = spec(temp.path());
            let home = job.host.lf_home.clone();
            let first = observe(&job, 0, 0, "UTC").unwrap();
            let wake = receipt(&job, 36000);
            let owner = begin(&home, &first, &wake).unwrap().unwrap();
            let candidate = ReleaseSelection {
                tag: "v1.2.3".into(),
                commit: "original".into(),
                workflow_run_id: Some(42),
            };
            select(&home, wake.id.as_str(), candidate.clone()).unwrap();
            settle(
                &home,
                wake.id.as_str(),
                ScheduledReleaseOutcome::Failed {
                    cause: "required verification failed".into(),
                },
                &[],
                36001,
            )
            .unwrap();
            let failed = read(&home).unwrap()[0].opportunities[0].attempts[0].clone();
            job.schedule = parse_schedule("0 0 11 * * *").unwrap();
            observe(&job, 40000, 40000, "UTC").unwrap();
            job.schedule = parse_schedule("0 0 12 * * *").unwrap();
            let successor = observe(&job, 130000, 130000, "UTC").unwrap();
            let retry = receipt(&job, 2 * 86400 + 43200);
            let mut records = read(&home).unwrap();
            let index = records.iter().position(|r| r.id == successor).unwrap();
            let chain = super::predecessors(&records, index).unwrap();
            assert_eq!(chain.len(), 3);
            // Reconstruct each durable prefix left by interruption before the owner write.
            for index in chain.iter().take(boundary.min(3)) {
                super::materialize(&mut records[*index], retry.started_at).unwrap();
                super::save(&home, &records[*index]).unwrap();
            }
            if boundary == 4 {
                assert_eq!(
                    begin(&home, &successor, &retry).unwrap(),
                    Some(owner.clone())
                );
            }
            assert_eq!(
                begin(&home, &successor, &retry).unwrap(),
                Some(owner.clone())
            );
            let rows = read(&home).unwrap();
            let due: Vec<_> = rows.iter().flat_map(|r| &r.opportunities).collect();
            assert_eq!(due.len(), 3);
            let original = due.iter().find(|o| o.id == owner).unwrap();
            assert_eq!(original.attempts.len(), 2);
            assert_eq!(original.attempts[0], failed);
            let attempt = original.attempts.last().unwrap();
            assert_eq!(attempt.covered.len(), 3);
            assert_eq!(attempt.selection.as_ref(), Some(&candidate));
            assert!(due
                .iter()
                .all(|o| o.id == owner || o.coalesced_into.as_ref() == Some(&owner)));
            assert_eq!(
                super::receipt_context(&home, temp.path(), retry.id.as_str())
                    .unwrap()
                    .id,
                first
            );
            assert_eq!(
                super::execution_context(&rows, retry.id.as_str())
                    .unwrap()
                    .id,
                successor
            );
            assert!(
                super::overlap_continuation(&home, retry.id.as_str(), retry.started_at)
                    .unwrap()
                    .contains("next configured release due 302400")
            );
            // Time passing and re-entry cannot expand this execution's frozen coverage.
            let mut reentry = retry.clone();
            reentry.started_at += 86400;
            begin(&home, &successor, &reentry).unwrap();
            assert_eq!(read(&home).unwrap(), rows);
            // Publication survived the process, but settlement did not. A new execution
            // must reconcile the same candidate and fence the interrupted writer.
            let recovered = receipt(&job, retry.started_at + 1);
            assert_eq!(
                begin(&home, &successor, &recovered).unwrap(),
                Some(owner.clone())
            );
            let recovered_rows = read(&home).unwrap();
            let recovered_attempt = recovered_rows
                .iter()
                .flat_map(|r| &r.opportunities)
                .find(|o| o.id == owner)
                .unwrap()
                .attempts
                .last()
                .unwrap();
            assert_eq!(recovered_attempt.selection.as_ref(), Some(&candidate));
            assert_eq!(recovered_attempt.covered, attempt.covered);
            let telemetry = super::TelemetryPrerequisite {
                schedule: "0 0 9 * * *".into(),
                timezone: "UTC".into(),
                activated_at: 0,
                observed_at: recovered.started_at,
                current_due_at: 2 * 86400 + 32400,
                current_receipts: Vec::new(),
                recovery_receipt: None,
                original: recovered_attempt
                    .covered
                    .iter()
                    .map(|id| super::TelemetryDue {
                        opportunity_id: id.clone(),
                        obligation_id: None,
                        due_at: None,
                        uncertainty: Some("no retained telemetry".into()),
                        receipts: Vec::new(),
                    })
                    .collect(),
            };
            super::record_telemetry(&home, recovered.id.as_str(), &telemetry).unwrap();
            assert!(super::record_telemetry(&home, retry.id.as_str(), &telemetry).is_err());
            let published = ScheduledReleaseOutcome::Published {
                tag: candidate.tag.clone(),
                commit: candidate.commit.clone(),
                workflow_run_id: 42,
            };
            assert!(settle(
                &home,
                wake.id.as_str(),
                published.clone(),
                &[],
                retry.started_at + 1
            )
            .is_err());
            settle(
                &home,
                recovered.id.as_str(),
                published.clone(),
                &[],
                retry.started_at + 2,
            )
            .unwrap();
            settle(
                &home,
                recovered.id.as_str(),
                published,
                &[],
                retry.started_at + 3,
            )
            .unwrap();
            assert!(settle(
                &home,
                retry.id.as_str(),
                ScheduledReleaseOutcome::Failed {
                    cause: "late interrupted writer".into(),
                },
                &[],
                recovered.started_at + 3
            )
            .is_err());
            let report =
                release_history(&home, temp.path(), &job.wave, 1, retry.started_at + 3).unwrap();
            assert_eq!(report.summary.published, 1);
            assert_eq!(report.summary.unresolved, 0);
            assert!(report.summary.closed_unsettled.is_empty());
            assert!(report.summary.qualifying_pairs.is_empty());
        }
    }

    #[test]
    fn retained_segment_documents_derive_coverage_without_rewriting_history() {
        let temp = tempfile::tempdir().unwrap();
        let job = spec(temp.path());
        let home = &job.host.lf_home;
        let id = observe(&job, 0, 0, "UTC").unwrap();
        let wake = receipt(&job, 122400);
        begin(home, &id, &wake).unwrap();
        let before = read(home).unwrap();
        let mut legacy = serde_json::to_value(&before[0]).unwrap();
        for due in legacy["opportunities"].as_array_mut().unwrap() {
            for attempt in due["attempts"].as_array_mut().unwrap() {
                attempt
                    .as_object_mut()
                    .unwrap()
                    .remove("execution_obligation");
            }
        }
        let path = super::directory(home).join(format!("{id}.json"));
        let bytes = serde_json::to_vec_pretty(&legacy).unwrap();
        std::fs::write(&path, &bytes).unwrap();
        let after = read(home).unwrap();
        assert_eq!(
            after[0].opportunities[0].coalesced_into,
            before[0].opportunities[0].coalesced_into
        );
        assert_eq!(
            after[0].opportunities[1].attempts[0].covered,
            before[0].opportunities[1].attempts[0].covered
        );
        assert_eq!(
            super::execution_context(&after, wake.id.as_str())
                .unwrap()
                .id,
            id
        );
        assert_eq!(std::fs::read(path).unwrap(), bytes);
    }

    #[test]
    fn replacement_cannot_continue_across_a_home_boundary() {
        let temp = tempfile::tempdir().unwrap();
        let mut job = spec(temp.path());
        let home = job.host.lf_home.clone();
        let first = observe(&job, 0, 0, "UTC").unwrap();
        let wake = receipt(&job, 36000);
        let old_owner = begin(&home, &first, &wake).unwrap().unwrap();
        select(
            &home,
            wake.id.as_str(),
            ReleaseSelection {
                tag: "v1.2.3".into(),
                commit: "old-home".into(),
                workflow_run_id: None,
            },
        )
        .unwrap();
        let original_home = job.host.home_id.clone();
        job.host.home_id = HomeId::new();
        observe(&job, 40000, 40000, "UTC").unwrap();
        // Returning to the first Home still must not leap over the intervening authority.
        job.host.home_id = original_home;
        let successor = observe(&job, 50000, 50000, "UTC").unwrap();
        let retry = receipt(&job, 122400);
        let owner = begin(&home, &successor, &retry).unwrap().unwrap();
        assert_ne!(owner, old_owner);
        let rows = read(&home).unwrap();
        assert_eq!(rows[0].opportunities[0].attempts.len(), 1);
        assert!(rows[0].opportunities[0].coalesced_into.is_none());
        let attempt = rows.last().unwrap().opportunities[0]
            .attempts
            .last()
            .unwrap();
        assert_eq!(attempt.covered, vec![owner]);
        assert!(attempt.selection.is_none());
    }

    #[test]
    fn overlap_continuation_uses_current_calendar_and_preserves_closed_ownership() {
        let temp = tempfile::tempdir().unwrap();
        let job = spec(temp.path());
        let home = &job.host.lf_home;
        let id = observe(&job, 0, 0, "UTC").unwrap();
        let wake = receipt(&job, 36000);
        let owner = begin(home, &id, &wake).unwrap().unwrap();
        let original = read(home).unwrap().remove(0).opportunities;
        for (now, next) in [(36000, 122400), (120000, 122400), (208800, 295200)] {
            // Telemetry can outlive the entry snapshot's next due. The continuation
            // must still name a future firing without expanding frozen coverage.
            if now == 208800 {
                assert!(original[0].next_due_at < now);
            }
            let continuation = super::overlap_continuation(home, wake.id.as_str(), now).unwrap();
            assert!(
                continuation.contains(&format!("next configured release due {next}")),
                "{continuation}"
            );
            assert!(continuation.contains(job.host.home_id.as_str()));
        }
        let blocked = receipt(&job, 122400);
        super::record_overlap(home, &id, &blocked).unwrap();
        let record = read(home).unwrap().remove(0);
        assert_eq!(record.opportunities[0], original[0]);
        let ScheduledReleaseOutcome::Deferred {
            reason,
            continuation,
        } = &record.opportunities[1].attempts[0].outcome
        else {
            panic!("overlap did not defer");
        };
        assert!(reason.contains(wake.id.as_str()));
        assert!(continuation.contains("next configured release due 208800"));
        close(&job, 122401).unwrap();
        let continuation = super::overlap_continuation(home, wake.id.as_str(), 300000).unwrap();
        assert!(continuation.contains("closed at 122401"));
        assert!(continuation.contains(&format!("lf cron disposition {owner}")));
        assert!(continuation.contains(job.host.home_id.as_str()));
        assert!(!continuation.contains("next configured release due"));
        assert_eq!(read(home).unwrap()[0].opportunities[0], original[0]);
    }

    #[test]
    fn overlap_continuation_uses_retained_timezone_across_dst() {
        use chrono::TimeZone;
        use chrono_tz::America::Los_Angeles;

        let temp = tempfile::tempdir().unwrap();
        let mut job = spec(temp.path());
        job.schedule = parse_schedule("0 30 2 * * *").unwrap();
        let now = Los_Angeles
            .with_ymd_and_hms(2026, 3, 7, 2, 30, 0)
            .unwrap()
            .timestamp();
        let next = Los_Angeles
            .with_ymd_and_hms(2026, 3, 9, 2, 30, 0)
            .unwrap()
            .timestamp();
        let id = observe(&job, now, now, "America/Los_Angeles").unwrap();
        let wake = receipt(&job, now);
        begin(&job.host.lf_home, &id, &wake).unwrap();
        let continuation =
            super::overlap_continuation(&job.host.lf_home, wake.id.as_str(), now).unwrap();
        assert!(
            continuation.contains(&format!("next configured release due {next}")),
            "{continuation}"
        );
    }

    #[test]
    fn closed_release_owners_retain_evidence_and_accept_dated_repair() {
        use crate::durable::TaskId;
        use crate::ops::cron::history::{disposition, release_history};

        for change in ["remove", "schedule", "timezone", "home"] {
            let temp = tempfile::tempdir().unwrap();
            let mut job = spec(temp.path());
            let home = job.host.lf_home.clone();
            let original_home = job.host.home_id.clone();
            let id = observe(&job, 0, 0, "UTC").unwrap();
            let wake = receipt(&job, 86400 + 36000);
            let owner = begin(&home, &id, &wake).unwrap().unwrap();
            let candidate = ReleaseSelection {
                tag: "v1.2.3".into(),
                commit: "exact-candidate".into(),
                workflow_run_id: Some(42),
            };
            select(&home, wake.id.as_str(), candidate).unwrap();
            let original = read(&home).unwrap().remove(0);
            let closed = 2 * 86400 + 36001;
            match change {
                "remove" => close(&job, closed).unwrap(),
                "schedule" => {
                    job.schedule = parse_schedule("0 0 11 * * *").unwrap();
                    observe(&job, closed, closed, "UTC").unwrap();
                    // The successor is durable, but the predecessor rewrite was interrupted.
                    super::save(&home, &original).unwrap();
                }
                "timezone" => {
                    observe(&job, 0, closed, "America/New_York").unwrap();
                }
                "home" => {
                    close(&job, closed).unwrap();
                    job.host.home_id = HomeId::new();
                    job.host.lf_home = temp.path().join("other-home");
                    observe(&job, closed, closed, "UTC").unwrap();
                }
                _ => unreachable!(),
            }
            // The third missed due has no attempt; the first two share one owner.
            let now = closed + 2 * 86400;
            let report = release_history(&home, temp.path(), &job.wave, 1, now).unwrap();
            let retained = report.obligations.iter().find(|r| r.id == id).unwrap();
            assert_eq!(retained.home_id, original_home);
            assert_eq!(&retained.opportunities[..2], &original.opportunities);
            let missed = retained.opportunities[2].id.clone();
            assert!(retained.opportunities[2].attempts.is_empty());
            assert_eq!(
                report.summary.closed_unsettled,
                vec![owner.clone(), missed.clone()]
            );
            for subject in [&owner, &missed] {
                assert!(
                    report.summary.undispositioned_failures.contains(subject),
                    "{change}: {report:?}"
                );
                disposition(
                    &home,
                    &job.wave,
                    subject,
                    TaskId::new(),
                    "reconcile retained candidate on original Home",
                    now,
                )
                .unwrap();
                disposition(
                    &home,
                    &job.wave,
                    subject,
                    TaskId::new(),
                    "repair remains assigned",
                    now + 1,
                )
                .unwrap();
            }
            let after = release_history(&home, temp.path(), &job.wave, 1, now + 1).unwrap();
            assert_eq!(after.dispositions.len(), 4);
            assert!(after.summary.late_dispositions.contains(&owner));
            assert!(after.summary.late_dispositions.contains(&missed));
            assert!(!after.summary.undispositioned_failures.contains(&owner));
            assert!(after.summary.qualifying_pairs.is_empty());
            assert_eq!(
                after.obligations.iter().find(|r| r.id == id).unwrap(),
                retained
            );
            if change == "home" {
                assert!(disposition(
                    &job.host.lf_home,
                    &job.wave,
                    &owner,
                    TaskId::new(),
                    "cannot transfer ownership",
                    now
                )
                .is_err());
            }
            // An operation already executing still owns its exact final write.
            settle(
                &home,
                wake.id.as_str(),
                ScheduledReleaseOutcome::Published {
                    tag: "v1.2.3".into(),
                    commit: "exact-candidate".into(),
                    workflow_run_id: 42,
                },
                &[],
                now + 2,
            )
            .unwrap();
            assert!(disposition(
                &home,
                &job.wave,
                &owner,
                TaskId::new(),
                "not a closed blocker anymore",
                now + 3
            )
            .is_err());
        }
    }

    #[test]
    fn telemetry_segments_preserve_original_schedule_timezone_and_home() {
        let temp = tempfile::tempdir().unwrap();
        let release = spec(temp.path());
        let release_id = observe(&release, 0, 0, "UTC").unwrap();
        let mut telemetry = release.clone();
        telemetry.flow = "telemetry-daily".into();
        telemetry.schedule = parse_schedule("0 0 9 * * *").unwrap();
        let first = observe(&telemetry, 0, 0, "UTC").unwrap();
        let mut failed = receipt(&telemetry, 32401);
        failed.outcome = CronOutcome::Failed;
        let failure_bytes = serde_json::to_vec(&failed).unwrap();
        telemetry.schedule = parse_schedule("0 0 8 * * *").unwrap();
        let second = observe(&telemetry, 86400, 86400, "UTC").unwrap();
        let recovered = receipt(&telemetry, 86400 + 28801);
        let third = observe(&telemetry, 86400, 2 * 86400, "America/New_York").unwrap();
        let eastern = receipt(&telemetry, 2 * 86400 + 46801);
        // A receipt on the new Home cannot stand in for the old Home's check.
        telemetry.host.home_id = HomeId::new();
        let fourth = observe(&telemetry, 3 * 86400, 3 * 86400, "UTC").unwrap();
        let mut wrong_home = receipt(&telemetry, failed.started_at);
        wrong_home.schedule = failed.schedule.clone();
        super::close(&telemetry, 4 * 86400).unwrap();
        let receipts = vec![
            failed.clone(),
            recovered.clone(),
            eastern.clone(),
            wrong_home,
        ];
        let wake = receipt(&release, 4 * 86400 + 50400);
        begin(&release.host.lf_home, &release_id, &wake).unwrap();
        let segments = history(
            &release.host.lf_home,
            temp.path(),
            &release.wave,
            wake.started_at,
        )
        .unwrap();
        let context = segments.iter().find(|s| s.id == release_id).unwrap();
        let mut dues = context.opportunities.clone();
        // The release follows 08:00 Eastern (13:00 UTC) on the third date.
        dues[2].due_at = 2 * 86400 + 50400;
        let original: Vec<_> = dues
            .iter()
            .map(|o| super::telemetry_due(&segments, context, o, &receipts).unwrap())
            .collect();
        assert_eq!(original[0].obligation_id.as_ref(), Some(&first));
        assert_eq!(original[0].due_at, Some(32400));
        assert_eq!(original[0].receipts, vec![failed.id]);
        assert_eq!(original[1].obligation_id.as_ref(), Some(&second));
        assert_eq!(original[1].due_at, Some(86400 + 28800));
        assert_eq!(original[1].receipts, vec![recovered.id]);
        assert_eq!(original[2].obligation_id.as_ref(), Some(&third));
        assert_eq!(original[2].due_at, Some(2 * 86400 + 46800));
        assert_eq!(original[2].receipts, vec![eastern.id]);
        assert!(original[3..]
            .iter()
            .all(|d| d.due_at.is_none() && d.uncertainty.is_some()));
        let moved = segments.iter().find(|s| s.id == fourth).unwrap();
        assert_eq!(moved.closed_at, Some(4 * 86400));
        assert!(segments
            .iter()
            .filter(|s| s.flow == "telemetry-daily")
            .all(|s| s.opportunities.is_empty()));
        assert_eq!(serde_json::to_vec(&receipts[0]).unwrap(), failure_bytes);
    }

    #[test]
    fn telemetry_cutover_does_not_invent_historical_timezone_or_cross_closure() {
        let temp = tempfile::tempdir().unwrap();
        let release = spec(temp.path());
        let release_id = observe(&release, 0, 0, "UTC").unwrap();
        let mut telemetry = release.clone();
        telemetry.flow = "telemetry-daily".into();
        telemetry.schedule = parse_schedule("0 0 9 * * *").unwrap();
        let id = observe(&telemetry, 0, 86400, "UTC").unwrap();
        super::close(&telemetry, 86400 + 40000).unwrap();
        let before = receipt(&telemetry, 86400 + 32401);
        let after = receipt(&telemetry, 86400 + 40001);
        let segments = history(
            &release.host.lf_home,
            temp.path(),
            &release.wave,
            2 * 86400 + 36000,
        )
        .unwrap();
        let context = segments.iter().find(|s| s.id == release_id).unwrap();
        let receipts = vec![before.clone(), after];
        let original: Vec<_> = context
            .opportunities
            .iter()
            .map(|o| super::telemetry_due(&segments, context, o, &receipts).unwrap())
            .collect();
        assert_eq!(original[0].obligation_id.as_ref(), Some(&id));
        assert!(original[0].due_at.is_none());
        assert_eq!(original[1].receipts, vec![before.id]);
        assert!(original[2].obligation_id.is_none());
        assert!(original[2].uncertainty.is_some());
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
                obligation_id: None,
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
    fn candidate_replacement_preserves_failure_and_owner_across_interruption() {
        let temp = tempfile::tempdir().unwrap();
        let spec = spec(temp.path());
        let home = &spec.host.lf_home;
        let id = observe(&spec, 0, 0, "UTC").unwrap();
        let first = receipt(&spec, 36001);
        let owner = begin(home, &id, &first).unwrap().unwrap();
        let rejected = ReleaseSelection {
            tag: "v1.0.1".into(),
            commit: "rejected".into(),
            workflow_run_id: Some(7),
        };
        let successor = ReleaseSelection {
            tag: "v1.0.2".into(),
            commit: "prepared".into(),
            workflow_run_id: None,
        };
        select(home, first.id.as_str(), rejected.clone()).unwrap();
        settle(
            home,
            first.id.as_str(),
            ScheduledReleaseOutcome::Failed {
                cause: "packaged preflight rejected".into(),
            },
            &[],
            36002,
        )
        .unwrap();
        let failed = read(home).unwrap()[0].opportunities[0].attempts[0].clone();
        let retry = receipt(&spec, 36003);
        assert_eq!(begin(home, &id, &retry).unwrap(), Some(owner.clone()));
        assert!(select(home, retry.id.as_str(), successor.clone()).is_err());
        let proof = super::VerificationEvidence {
            name: "release-source-inspection".into(),
            subject: "rejected".into(),
            passed: true,
            evidence_path: temp.path().join("inspection"),
            sha256: "digest".into(),
        };
        super::authorize_replacement(home, retry.id.as_str(), &rejected, proof.clone()).unwrap();
        // Interrupted after authorization, before successor selection: still resume the old owner.
        let next = receipt(&spec, 122401);
        assert_eq!(begin(home, &id, &next).unwrap(), Some(owner));
        assert!(
            super::authorize_replacement(home, retry.id.as_str(), &rejected, proof.clone())
                .is_err()
        );
        assert!(select(home, retry.id.as_str(), successor.clone()).is_err());
        assert!(select(home, next.id.as_str(), successor.clone()).is_err());
        super::authorize_replacement(home, next.id.as_str(), &rejected, proof).unwrap();
        select(home, next.id.as_str(), successor.clone()).unwrap();
        assert!(select(home, next.id.as_str(), rejected).is_err());
        let record = read(home).unwrap().remove(0);
        let attempts = &record.opportunities[0].attempts;
        assert_eq!(attempts[0], failed);
        assert_eq!(attempts[2].selection.as_ref(), Some(&successor));
        assert_eq!(
            attempts[2].selection.as_ref().unwrap().workflow_run_id,
            None
        );
        assert_eq!(
            record.opportunities[1].coalesced_into,
            Some(record.opportunities[0].id.clone())
        );
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
        assert_eq!(record.opportunities[0].interventions.len(), 1);
        assert!(record.opportunities[1].interventions.is_empty());
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
