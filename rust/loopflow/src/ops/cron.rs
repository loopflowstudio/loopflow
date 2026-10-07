pub mod accounting;
pub(crate) mod calendar;
pub mod history;

use std::collections::HashSet;
use std::fs::{self, OpenOptions};
use std::io::Write;
#[cfg(unix)]
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, ExitStatus, Stdio};
use std::str::FromStr;
use std::time::{Duration, Instant};

use chrono::{TimeZone, Timelike, Utc};
use serde::{Deserialize, Serialize};

use crate::durable::{CronReceiptId, MachineId};
use crate::ops::error::{OpsError, OpsResult};

const RECEIPT_STALE_AFTER: i64 = 6 * 60 * 60;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CronHost {
    pub machine_id: MachineId,
    pub lf_home: PathBuf,
    pub path_env: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CronSpec {
    pub wave: String,
    pub flow: String,
    pub target_kind: CronTargetKind,
    pub schedule: CronSchedule,
    pub working_directory: PathBuf,
    pub lf_path: PathBuf,
    pub host: CronHost,
}

impl CronSpec {
    fn log_path(&self) -> PathBuf {
        self.working_directory.join(format!(
            ".lf/logs/cron.{}.{}.log",
            self.wave,
            self.flow.replace('/', ".")
        ))
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CronSchedule {
    expression: String,
    hour: u32,
    minute: u32,
    every_minute: bool,
}

impl CronSchedule {
    pub(crate) fn every_minute(&self) -> bool {
        self.every_minute
    }

    pub fn expression(&self) -> &str {
        &self.expression
    }

    pub(crate) fn hour(&self) -> u32 {
        self.hour
    }

    pub(crate) fn minute(&self) -> u32 {
        self.minute
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
#[non_exhaustive]
pub enum CronTargetKind {
    Flow,
    Skill,
    Repository,
}

impl CronTargetKind {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Flow => "flow",
            Self::Skill => "skill",
            Self::Repository => "repository",
        }
    }
}

impl FromStr for CronTargetKind {
    type Err = OpsError;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        match value {
            "flow" => Ok(Self::Flow),
            "skill" => Ok(Self::Skill),
            "repository" => Ok(Self::Repository),
            _ => Err(OpsError::Parse(format!(
                "unknown cron target kind {value:?}"
            ))),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
#[non_exhaustive]
pub enum CronSource {
    Scheduled,
    Triggered,
    Recovery,
    Manual,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
#[non_exhaustive]
pub enum CronOutcome {
    Running,
    Succeeded,
    Failed,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize)]
pub struct CronReceipt {
    pub schema_version: u32,
    pub id: CronReceiptId,
    pub runner_pid: u32,
    pub runner_started_at: Option<i64>,
    #[serde(alias = "home_id")]
    pub machine_id: MachineId,
    pub wave: String,
    pub flow: String,
    pub target_kind: CronTargetKind,
    pub source: CronSource,
    pub schedule: String,
    pub repo: PathBuf,
    pub lf_path: PathBuf,
    pub log_path: PathBuf,
    pub started_at: i64,
    pub finished_at: Option<i64>,
    pub outcome: CronOutcome,
    pub exit_code: Option<i32>,
    pub error: Option<String>,
}

impl CronReceipt {
    pub(crate) fn runner_evidence(&self) -> crate::journal::ProcessIdentityEvidence {
        self.runner_started_at.map_or(
            crate::journal::ProcessIdentityEvidence::Unknown,
            |started_at| crate::journal::process_identity_evidence(self.runner_pid, started_at),
        )
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct CronObligation {
    pub(crate) target_kind: CronTargetKind,
    pub(crate) wave: String,
    pub(crate) flow: String,
    pub(crate) schedule: CronSchedule,
    pub(crate) machine_id: MachineId,
    pub(crate) activated_at: i64,
    pub(crate) receipts: Vec<CronReceipt>,
    pub(crate) repo: PathBuf,
    pub(crate) lf_path: PathBuf,
    pub(crate) log_path: PathBuf,
}

/// Reconcile summary from [`sync_crons`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CronSyncResult {
    /// Declared crons now present as launchd jobs (added or replaced).
    pub installed: Vec<InstalledCron>,
    /// Launchd jobs for this wave pruned because the flow is no longer declared.
    pub removed: Vec<InstalledCron>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct InstalledCron {
    pub wave: String,
    pub flow: String,
    pub label: String,
    pub path: PathBuf,
    pub schedule: String,
    pub target_kind: CronTargetKind,
    pub machine_id: MachineId,
    pub activated_at: i64,
    pub repo: PathBuf,
    pub lf_path: PathBuf,
    pub loaded: bool,
    pub latest_receipt: Option<CronReceipt>,
}

pub trait Launchctl {
    fn load(&self, path: &Path) -> OpsResult<()>;
    fn unload(&self, path: &Path) -> OpsResult<()>;
    fn is_loaded(&self, label: &str) -> OpsResult<bool>;
    fn trigger(&self, label: &str) -> OpsResult<()>;
}

#[derive(Debug)]
pub struct SystemLaunchctl;

impl Launchctl for SystemLaunchctl {
    fn load(&self, path: &Path) -> OpsResult<()> {
        run_launchctl_path("load", path)
    }

    fn unload(&self, path: &Path) -> OpsResult<()> {
        run_launchctl_path("unload", path)
    }

    fn is_loaded(&self, label: &str) -> OpsResult<bool> {
        Ok(Command::new("launchctl")
            .args(["print", &launchd_service(label)?])
            .output()?
            .status
            .success())
    }

    fn trigger(&self, label: &str) -> OpsResult<()> {
        let service = launchd_service(label)?;
        let output = Command::new("launchctl")
            .args(["kickstart", &service])
            .output()?;
        if output.status.success() {
            return Ok(());
        }
        Err(OpsError::CommandFailed {
            command: format!("launchctl kickstart {service}"),
            stderr: String::from_utf8_lossy(&output.stderr).into_owned(),
        })
    }
}

pub fn add_cron(
    launch_agents_dir: &Path,
    spec: &CronSpec,
    launchctl: &dyn Launchctl,
) -> OpsResult<InstalledCron> {
    fs::create_dir_all(launch_agents_dir)?;
    if let Some(parent) = spec.log_path().parent() {
        fs::create_dir_all(parent)?;
    }
    let path = plist_path(launch_agents_dir, &spec.wave, &spec.flow);
    let now = Utc::now().timestamp();
    let activated_at = if path.exists() {
        let prior = read_cron_obligation(&path)?;
        let prior_spec = read_cron_spec(&path)?;
        retain_installed_obligation(&prior_spec, prior.activated_at, now)?;
        if matches!(prior_spec.flow.as_str(), "release-run" | "telemetry-daily")
            && (prior_spec.host.lf_home != spec.host.lf_home
                || prior_spec.host.machine_id != spec.host.machine_id
                || prior_spec.working_directory.canonicalize()?
                    != spec.working_directory.canonicalize()?)
        {
            accounting::close(&prior_spec, now)?;
        }
        if same_obligation(&prior, spec)
            && prior_spec.host.lf_home == spec.host.lf_home
            && prior_spec.working_directory.canonicalize()?
                == spec.working_directory.canonicalize()?
        {
            prior.activated_at
        } else {
            now
        }
    } else {
        now
    };
    if matches!(spec.flow.as_str(), "release-run" | "telemetry-daily") {
        let zone = iana_time_zone::get_timezone().map_err(|e| OpsError::Message(e.to_string()))?;
        accounting::observe(spec, activated_at, now, &zone)?;
    }
    if path.exists() {
        let _ = launchctl.unload(&path);
    }
    write_private_file(&path, render_plist(spec, activated_at).as_bytes())?;
    launchctl.load(&path)?;
    inspect_cron(&path, launchctl)
}

pub fn remove_cron(
    launch_agents_dir: &Path,
    wave: &str,
    flow: &str,
    launchctl: &dyn Launchctl,
) -> OpsResult<Option<InstalledCron>> {
    let path = plist_path(launch_agents_dir, wave, flow);
    if !path.exists() {
        return Ok(None);
    }
    let cron = inspect_cron(&path, launchctl)?;
    if matches!(cron.flow.as_str(), "release-run" | "telemetry-daily") {
        let spec = read_cron_spec(&path)?;
        let now = Utc::now().timestamp();
        retain_installed_obligation(&spec, cron.activated_at, now)?;
        accounting::close(&spec, now)?;
    }
    launchctl.unload(&path)?;
    fs::remove_file(&path)?;
    Ok(Some(cron))
}

fn retain_installed_obligation(spec: &CronSpec, activated_at: i64, now: i64) -> OpsResult<()> {
    if !matches!(spec.flow.as_str(), "release-run" | "telemetry-daily") {
        return Ok(());
    }
    let repo = spec.working_directory.canonicalize()?;
    // The predecessor keeps its observed timezone. Only a legacy job without
    // retained evidence needs today's zone, explicitly unknown before now.
    let retained = accounting::read(&spec.host.lf_home)?.iter().any(|s| {
        s.repo == repo
            && s.machine_id == spec.host.machine_id
            && s.wave == spec.wave
            && s.flow == spec.flow
            && s.schedule == spec.schedule.expression()
            && s.installation_activated_at == activated_at
            && s.closed_at.is_none()
    });
    if !retained {
        let zone = iana_time_zone::get_timezone().map_err(|e| OpsError::Message(e.to_string()))?;
        accounting::observe(spec, activated_at, now, &zone)?;
    }
    Ok(())
}

pub fn list_crons(
    launch_agents_dir: &Path,
    launchctl: &dyn Launchctl,
) -> OpsResult<Vec<InstalledCron>> {
    let mut crons = Vec::new();
    if !launch_agents_dir.is_dir() {
        return Ok(crons);
    }
    for entry in fs::read_dir(launch_agents_dir)? {
        let entry = entry?;
        let path = entry.path();
        let Some(name) = path.file_name().and_then(|name| name.to_str()) else {
            continue;
        };
        if !is_cron_plist(name) {
            continue;
        }
        crons.push(inspect_cron(&path, launchctl)?);
    }
    crons.sort_by(|a, b| a.label.cmp(&b.label));
    Ok(crons)
}

pub(crate) fn list_cron_obligations(launch_agents_dir: &Path) -> OpsResult<Vec<CronObligation>> {
    let mut obligations = Vec::new();
    if !launch_agents_dir.is_dir() {
        return Ok(obligations);
    }
    for entry in fs::read_dir(launch_agents_dir)? {
        let entry = entry?;
        let path = entry.path();
        let Some(name) = path.file_name().and_then(|name| name.to_str()) else {
            continue;
        };
        if is_cron_plist(name) {
            obligations.push(read_cron_obligation(&path)?);
        }
    }
    obligations.sort_by(|left, right| {
        left.wave
            .cmp(&right.wave)
            .then_with(|| left.flow.cmp(&right.flow))
    });
    Ok(obligations)
}

pub fn parse_schedule(value: &str) -> OpsResult<CronSchedule> {
    match value {
        "every-minute" => schedule_from_cron("0 * * * * *"),
        "daily" => schedule_from_cron("0 0 3 * * *"),
        _ => schedule_from_cron(value),
    }
}

/// The launchd [`CronSchedule`] for a declared cron expression, or an error
/// describing why launchd can't run it.
pub fn schedule_from_cron(expr: &str) -> OpsResult<CronSchedule> {
    if matches!(expr.trim(), "0 * * * * *" | "0 * * * * * *") {
        return Ok(CronSchedule {
            expression: expr.to_string(),
            hour: 0,
            minute: 0,
            every_minute: true,
        });
    }
    let (hour, minute) = daily_time_of(expr)?;
    Ok(CronSchedule {
        expression: expr.to_string(),
        hour,
        minute,
        every_minute: false,
    })
}

/// Classify a cron expression as a fixed daily time for launchd.
///
/// launchd `StartCalendarInterval` expresses a repeating wall-clock time, not an
/// arbitrary cron expression. We accept only schedules that fire once a day at a
/// fixed hour:minute. Validate more than a full year of consecutive fires so a
/// weekday, month-day, or annual gap cannot masquerade as daily merely because
/// its first two occurrences happen to be 24 hours apart.
///
/// # Errors
/// Returns [`OpsError::Parse`] for an unparseable expression or one that is not a
/// fixed daily time.
pub fn daily_time_of(expr: &str) -> OpsResult<(u32, u32)> {
    let schedule = cron::Schedule::from_str(expr)
        .map_err(|err| OpsError::Parse(format!("invalid cron schedule '{expr}': {err}")))?;
    let anchor = Utc
        .with_ymd_and_hms(2020, 1, 1, 0, 0, 0)
        .single()
        .expect("2020-01-01T00:00:00Z is a valid instant");
    let mut fires = schedule.after(&anchor);
    let first = fires
        .next()
        .ok_or_else(|| OpsError::Parse(format!("cron schedule '{expr}' never fires")))?;
    let mut prior = first;
    for _ in 0..370 {
        let next = fires.next().ok_or_else(|| {
            OpsError::Parse(format!("cron schedule '{expr}' does not fire daily"))
        })?;
        if next - prior != chrono::Duration::hours(24) {
            return Err(OpsError::Parse(format!(
                "cron schedule '{expr}' is not a fixed daily time (launchd host supports daily only)"
            )));
        }
        prior = next;
    }
    Ok((first.hour(), first.minute()))
}

/// Validate that every spec belongs to one Wave with one entry per target.
///
/// # Errors
/// Returns [`OpsError::Parse`] when a spec belongs elsewhere or duplicates a
/// target.
pub fn validate_cron_specs(wave: &str, specs: &[CronSpec]) -> OpsResult<()> {
    let mut flows = HashSet::new();
    for spec in specs {
        if spec.wave != wave {
            return Err(OpsError::Parse(format!(
                "cron target {} belongs to Wave {}, not {wave}",
                spec.flow, spec.wave
            )));
        }
        if !flows.insert(&spec.flow) {
            return Err(OpsError::Parse(format!(
                "Wave {wave} declares cron target {} more than once",
                spec.flow
            )));
        }
    }
    Ok(())
}

/// Reconcile launchd jobs for one wave to its validated declarations.
pub fn sync_crons(
    launch_agents_dir: &Path,
    wave: &str,
    specs: &[CronSpec],
    launchctl: &dyn Launchctl,
) -> OpsResult<CronSyncResult> {
    validate_cron_specs(wave, specs)?;

    let desired_flows = specs
        .iter()
        .map(|spec| spec.flow.clone())
        .collect::<HashSet<_>>();
    let mut installed = Vec::new();
    for spec in specs {
        installed.push(add_cron(launch_agents_dir, spec, launchctl)?);
    }

    let mut removed = Vec::new();
    for existing in list_crons(launch_agents_dir, launchctl)? {
        if existing.wave == wave && !desired_flows.contains(&existing.flow) {
            if let Some(cron) =
                remove_cron(launch_agents_dir, &existing.wave, &existing.flow, launchctl)?
            {
                removed.push(cron);
            }
        }
    }

    Ok(CronSyncResult { installed, removed })
}

pub fn run_cron(
    launch_agents_dir: &Path,
    wave: &str,
    flow: &str,
    current_machine: &MachineId,
    placed_machine: &MachineId,
    source: CronSource,
) -> OpsResult<CronReceipt> {
    run_cron_recorded(
        launch_agents_dir,
        wave,
        flow,
        current_machine,
        placed_machine,
        source,
        &mut |_| Ok(()),
    )
}

pub(crate) fn run_cron_recorded(
    launch_agents_dir: &Path,
    wave: &str,
    flow: &str,
    current_machine: &MachineId,
    placed_machine: &MachineId,
    source: CronSource,
    record: &mut dyn FnMut(&CronReceipt) -> OpsResult<()>,
) -> OpsResult<CronReceipt> {
    let path = plist_path(launch_agents_dir, wave, flow);
    let spec = read_cron_spec(&path)?;
    validate_installed_spec(&spec, wave, flow)?;
    let root = receipt_root(&spec.host.lf_home);
    let mut receipt = new_receipt(&spec, current_machine, source);
    if source == CronSource::Scheduled && accounting::consume_trigger(&spec, &receipt)? {
        receipt.source = CronSource::Triggered;
    }
    write_receipt(&root, &receipt)?;
    if let Err(error) = record(&receipt) {
        receipt.finished_at = Some(Utc::now().timestamp());
        receipt.outcome = CronOutcome::Failed;
        receipt.error = Some(format!(
            "prerequisite reservation failed before launch: {error}"
        ));
        write_receipt(&root, &receipt)?;
        return Err(error);
    }

    let obligation = if matches!(flow, "release-run" | "telemetry-daily") {
        let zone = iana_time_zone::get_timezone().map_err(|e| OpsError::Message(e.to_string()))?;
        let obligation = read_cron_obligation(&path)?;
        Some(accounting::observe(
            &spec,
            obligation.activated_at,
            receipt.started_at,
            &zone,
        )?)
    } else {
        None
    };
    let release_obligation = obligation.filter(|_| flow == "release-run");

    let placement_error = if spec.host.machine_id != *placed_machine {
        Some(format!(
            "installed for Machine {}, but Wave {wave} is placed on {placed_machine}; run `lf wave cron sync --wave {wave}` on the placed Machine",
            spec.host.machine_id
        ))
    } else if *current_machine != *placed_machine {
        Some(format!(
            "current Machine is {current_machine}, but Wave {wave} is placed on {placed_machine}"
        ))
    } else {
        None
    };
    if let Some(error) = placement_error {
        receipt.finished_at = Some(Utc::now().timestamp());
        receipt.outcome = CronOutcome::Failed;
        receipt.error = Some(error.clone());
        if let Some(id) = &release_obligation {
            accounting::preflight_failure(&spec.host.lf_home, id, &receipt)?;
        }
        write_receipt(&root, &receipt)?;
        return Err(OpsError::Message(format!(
            "refusing cron {wave}/{flow}: {error}"
        )));
    }

    let _execution = if release_obligation.is_some() || flow == "telemetry-daily" {
        let lease = accounting::claim_execution(&spec)?;
        if lease.is_none() {
            receipt.finished_at = Some(Utc::now().timestamp());
            receipt.outcome = CronOutcome::Failed;
            receipt.error = Some("cron execution already active".into());
            write_receipt(&root, &receipt)?;
            if let Some(id) = &release_obligation {
                let continuation = accounting::record_overlap(&spec.host.lf_home, id, &receipt)?;
                receipt.error = Some(format!("cron execution already active; {continuation}"));
                write_receipt(&root, &receipt)?;
            }
            if source == CronSource::Recovery {
                return Err(OpsError::ReleaseDeferred {
                    reason: receipt.error.clone().expect("overlap cause set"),
                    continuation: format!(
                        "observe the active prerequisite at {}",
                        receipt.log_path.display()
                    ),
                });
            }
            return Err(OpsError::Message(
                receipt.error.clone().expect("overlap cause set"),
            ));
        }
        if let Some(id) = &release_obligation {
            if source == CronSource::Scheduled
                && accounting::begin(&spec.host.lf_home, id, &receipt)?.is_none()
            {
                receipt.finished_at = Some(Utc::now().timestamp());
                receipt.outcome = CronOutcome::Succeeded;
                receipt.exit_code = Some(0);
                write_receipt(&root, &receipt)?;
                return Ok(receipt);
            }
        }
        lease
    } else {
        None
    };
    let mut child = match spawn_cron_target(&spec, _execution.as_ref(), &receipt) {
        Ok(child) => child,
        Err(error) => {
            receipt.finished_at = Some(Utc::now().timestamp());
            receipt.outcome = CronOutcome::Failed;
            receipt.error = Some(format!(
                "could not start target: {error}; see {}",
                receipt.log_path.display()
            ));
            write_receipt(&root, &receipt)?;
            return Err(OpsError::CommandFailed {
                command: format!("cron {wave}/{flow}"),
                stderr: receipt.error.clone().unwrap_or_default(),
            });
        }
    };
    // A deadline or observation error is not a child exit. Keep the reserved
    // Running receipt intact and its inherited lock effective.
    let timeout = (source == CronSource::Recovery).then_some(Duration::from_secs(60 * 60));
    let status = wait_for_cron_target(&mut child, &receipt, timeout)?;
    receipt.finished_at = Some(Utc::now().timestamp());
    receipt.exit_code = status.code();
    if status.success() {
        receipt.outcome = CronOutcome::Succeeded;
    } else {
        receipt.outcome = CronOutcome::Failed;
        receipt.error = Some(format!(
            "target exited {}; see {}",
            status_label(&status),
            receipt.log_path.display()
        ));
    }
    write_receipt(&root, &receipt)?;
    if spec.schedule.every_minute {
        let _ = prune_minute_receipts(&root, &spec, &receipt.id);
    }
    if status.success() {
        Ok(receipt)
    } else {
        Err(OpsError::CommandFailed {
            command: format!("cron {wave}/{flow}"),
            stderr: receipt.error.clone().unwrap_or_default(),
        })
    }
}

/// Persist a terminal receipt when scheduled execution cannot read its Machine
/// placement authority. The installed plist still supplies the durable receipt
/// location and non-secret job identity; no target is started.
pub fn record_cron_preflight_failure(
    launch_agents_dir: &Path,
    wave: &str,
    flow: &str,
    source: CronSource,
    error: &str,
) -> OpsResult<CronReceipt> {
    let spec = read_cron_spec(&plist_path(launch_agents_dir, wave, flow))?;
    validate_installed_spec(&spec, wave, flow)?;
    let root = receipt_root(&spec.host.lf_home);
    let mut receipt = new_receipt(&spec, &spec.host.machine_id, source);
    write_receipt(&root, &receipt)?;
    receipt.finished_at = Some(Utc::now().timestamp());
    receipt.outcome = CronOutcome::Failed;
    receipt.error = Some(format!("Machine placement preflight failed: {error}"));
    if matches!(flow, "release-run" | "telemetry-daily") {
        let zone = iana_time_zone::get_timezone().map_err(|e| OpsError::Message(e.to_string()))?;
        let prior = read_cron_obligation(&plist_path(launch_agents_dir, wave, flow))?;
        let id = accounting::observe(&spec, prior.activated_at, receipt.started_at, &zone)?;
        if flow == "release-run" {
            accounting::preflight_failure(&spec.host.lf_home, &id, &receipt)?;
        }
    }
    write_receipt(&root, &receipt)?;
    Ok(receipt)
}

pub fn receipt_root(lf_home: &Path) -> PathBuf {
    lf_home.join("cron/receipts")
}

pub fn list_cron_receipts(
    root: &Path,
    wave: &str,
    flow: Option<&str>,
    days: u32,
) -> OpsResult<Vec<CronReceipt>> {
    let since = Utc::now()
        .timestamp()
        .saturating_sub(i64::from(days).saturating_mul(24 * 60 * 60));
    let mut receipts = read_receipts(root, wave, flow)?;
    receipts.retain(|receipt| receipt.started_at >= since);
    receipts.sort_by(|left, right| {
        right
            .started_at
            .cmp(&left.started_at)
            .then_with(|| right.id.as_str().cmp(left.id.as_str()))
    });
    Ok(receipts)
}

pub fn latest_cron_receipt(root: &Path, wave: &str, flow: &str) -> OpsResult<Option<CronReceipt>> {
    let mut receipts = read_receipts(root, wave, Some(flow))?;
    receipts.sort_by_key(|receipt| receipt.started_at);
    Ok(receipts.pop())
}

pub(crate) fn cron_receipt_ids(
    root: &Path,
    wave: &str,
    flow: &str,
) -> OpsResult<Vec<CronReceiptId>> {
    Ok(read_receipts(root, wave, Some(flow))?
        .into_iter()
        .map(|receipt| receipt.id)
        .collect())
}

pub fn receipt_is_stale(receipt: &CronReceipt, now: i64) -> bool {
    receipt.outcome == CronOutcome::Running
        && (now.saturating_sub(receipt.started_at) >= RECEIPT_STALE_AFTER
            || !process_alive(receipt.runner_pid))
}

pub fn record_cron_trigger(launch_agents_dir: &Path, wave: &str, flow: &str) -> OpsResult<String> {
    let spec = read_cron_spec(&plist_path(launch_agents_dir, wave, flow))?;
    accounting::record_trigger(&spec, Utc::now().timestamp())
}

pub fn record_cron_trigger_failure(
    launch_agents_dir: &Path,
    wave: &str,
    flow: &str,
    id: &str,
    cause: &str,
) -> OpsResult<()> {
    let spec = read_cron_spec(&plist_path(launch_agents_dir, wave, flow))?;
    accounting::trigger_failed(&spec, id, cause)
}

pub fn trigger_cron(launchctl: &dyn Launchctl, wave: &str, flow: &str) -> OpsResult<()> {
    launchctl.trigger(&label(wave, flow))
}

pub fn wait_for_cron_receipt(
    root: &Path,
    wave: &str,
    flow: &str,
    prior_receipts: &[CronReceiptId],
    started_after: i64,
    timeout: Duration,
) -> OpsResult<CronReceipt> {
    let deadline = Instant::now() + timeout;
    loop {
        let now = Utc::now().timestamp();
        let mut receipts = read_receipts(root, wave, Some(flow))?;
        receipts.retain(|receipt| {
            receipt.source != CronSource::Manual
                && !prior_receipts.contains(&receipt.id)
                && receipt.started_at >= started_after
                && (receipt.outcome != CronOutcome::Running || receipt_is_stale(receipt, now))
        });
        receipts.sort_by(|left, right| {
            right
                .started_at
                .cmp(&left.started_at)
                .then_with(|| right.id.as_str().cmp(left.id.as_str()))
        });
        if let Some(receipt) = receipts.into_iter().next() {
            return Ok(receipt);
        }
        if Instant::now() >= deadline {
            return Err(OpsError::Message(format!(
                "timed out after {}s waiting for cron {wave}/{flow}; inspect `lf wave cron history --wave {wave} --flow {flow}`",
                timeout.as_secs()
            )));
        }
        std::thread::sleep(Duration::from_millis(250));
    }
}

pub fn parse_wait_duration(value: &str) -> OpsResult<Duration> {
    let value = value.trim();
    let (number, multiplier) = if let Some(number) = value.strip_suffix('s') {
        (number, 1)
    } else if let Some(number) = value.strip_suffix('m') {
        (number, 60)
    } else if let Some(number) = value.strip_suffix('h') {
        (number, 60 * 60)
    } else {
        (value, 1)
    };
    let amount: u64 = number.parse().map_err(|_| {
        OpsError::Parse(format!(
            "invalid duration {value:?}; use seconds, 10s, 5m, or 1h"
        ))
    })?;
    let seconds = amount
        .checked_mul(multiplier)
        .ok_or_else(|| OpsError::Parse(format!("duration {value:?} is larger than 24h")))?;
    if seconds == 0 || seconds > 24 * 60 * 60 {
        return Err(OpsError::Parse(format!(
            "duration {value:?} must be between 1s and 24h"
        )));
    }
    Ok(Duration::from_secs(seconds))
}

pub fn default_launch_agents_dir() -> OpsResult<PathBuf> {
    let home =
        dirs::home_dir().ok_or_else(|| OpsError::Message("no home directory".to_string()))?;
    Ok(home.join("Library/LaunchAgents"))
}

pub fn resolve_lf_path() -> OpsResult<PathBuf> {
    // Scheduled work follows future promotions; Sessions pin their own runtime.
    if !crate::store::custom_home_selected() {
        let gate = crate::installation::root()
            .and_then(|root| {
                crate::installation::entry_gate_path(&root, &crate::installation::ArtifactRole::Cli)
            })
            .map_err(|error| OpsError::Message(error.to_string()))?;
        if gate.is_file() {
            return Ok(gate);
        }
    }
    std::env::current_exe().map_err(Into::into)
}

fn inspect_cron(path: &Path, launchctl: &dyn Launchctl) -> OpsResult<InstalledCron> {
    let spec = read_cron_spec(path)?;
    let obligation = read_cron_obligation(path)?;
    let label = label(&spec.wave, &spec.flow);
    Ok(InstalledCron {
        wave: spec.wave.clone(),
        flow: spec.flow.clone(),
        label: label.clone(),
        path: path.to_path_buf(),
        schedule: spec.schedule.expression().to_string(),
        target_kind: spec.target_kind,
        machine_id: spec.host.machine_id.clone(),
        activated_at: obligation.activated_at,
        repo: spec.working_directory.clone(),
        lf_path: spec.lf_path.clone(),
        loaded: launchctl.is_loaded(&label)?,
        latest_receipt: latest_cron_receipt(
            &receipt_root(&spec.host.lf_home),
            &spec.wave,
            &spec.flow,
        )?,
    })
}

fn read_cron_spec(path: &Path) -> OpsResult<CronSpec> {
    let content = fs::read_to_string(path).map_err(|error| {
        OpsError::Message(format!(
            "cron is not installed at {}: {error}",
            path.display()
        ))
    })?;
    let required = |key: &str| {
        plist_string(&content, key).ok_or_else(|| {
            OpsError::Parse(format!(
                "{} is missing required {key} metadata; run `lf wave cron sync --wave <wave>`",
                path.display()
            ))
        })
    };
    Ok(CronSpec {
        wave: required("LoopflowWave")?,
        flow: required("LoopflowFlow")?,
        target_kind: required("LoopflowTargetKind")?.parse()?,
        schedule: schedule_from_cron(&required("LoopflowSchedule")?)?,
        working_directory: PathBuf::from(required("LoopflowRepo")?),
        lf_path: PathBuf::from(required("LoopflowLfPath")?),
        host: CronHost {
            machine_id: MachineId::parse(&required("LoopflowHomeId")?)
                .map_err(|error| OpsError::Parse(error.to_string()))?,
            lf_home: PathBuf::from(required("LoopflowLfHome")?),
            path_env: required("LoopflowPath")?,
        },
    })
}

fn read_cron_obligation(path: &Path) -> OpsResult<CronObligation> {
    let content = fs::read_to_string(path)?;
    let spec = read_cron_spec(path)?;
    let receipts = read_receipts(
        &receipt_root(&spec.host.lf_home),
        &spec.wave,
        Some(&spec.flow),
    )?;
    let activated_at = match plist_string(&content, "LoopflowActivatedAt") {
        Some(value) => value.parse::<i64>().map_err(|error| {
            OpsError::Parse(format!(
                "{} has invalid LoopflowActivatedAt {value:?}: {error}; run `lf wave cron sync --wave {}`",
                path.display(),
                spec.wave
            ))
        })?,
        None => receipts
            .iter()
            .filter(|receipt| {
                receipt.source == CronSource::Scheduled
                    && receipt.wave == spec.wave
                    && receipt.flow == spec.flow
                    && receipt.machine_id == spec.host.machine_id
                    && receipt.schedule == spec.schedule.expression()
            })
            .map(|receipt| receipt.started_at)
            .min()
            .or_else(|| file_timestamp(path))
            .ok_or_else(|| {
                OpsError::Message(format!(
                    "cannot recover activation time for legacy cron {}; run `lf wave cron sync --wave {}`",
                    path.display(),
                    spec.wave
                ))
            })?,
    };
    let log_path = spec.log_path();
    Ok(CronObligation {
        target_kind: spec.target_kind,
        repo: spec.working_directory,
        lf_path: spec.lf_path,
        log_path,
        wave: spec.wave,
        flow: spec.flow,
        schedule: spec.schedule,
        machine_id: spec.host.machine_id,
        activated_at,
        receipts,
    })
}

fn same_obligation(prior: &CronObligation, spec: &CronSpec) -> bool {
    prior.wave == spec.wave
        && prior.flow == spec.flow
        && prior.schedule == spec.schedule
        && prior.machine_id == spec.host.machine_id
}

fn file_timestamp(path: &Path) -> Option<i64> {
    let metadata = fs::metadata(path).ok()?;
    let timestamp = metadata.created().or_else(|_| metadata.modified()).ok()?;
    i64::try_from(
        timestamp
            .duration_since(std::time::UNIX_EPOCH)
            .ok()?
            .as_secs(),
    )
    .ok()
}

fn spawn_cron_target(
    spec: &CronSpec,
    execution: Option<&std::fs::File>,
    receipt: &CronReceipt,
) -> std::io::Result<Child> {
    if let Some(parent) = spec.log_path().parent() {
        fs::create_dir_all(parent)?;
    }
    let stdout = OpenOptions::new()
        .create(true)
        .append(true)
        .open(spec.log_path())?;
    if spec.schedule.every_minute && stdout.metadata()?.len() > MINUTE_LOG_LIMIT {
        stdout.set_len(0)?;
    }
    let stderr = stdout.try_clone()?;
    let mut command = Command::new(&spec.lf_path);
    if let Some(file) =
        execution.filter(|_| receipt.flow == "release-run" && receipt.source != CronSource::Manual)
    {
        use std::os::fd::AsRawFd;
        command.args([
            "--__cron-receipt",
            receipt.id.as_str(),
            "--__cron-lock-fd",
            &file.as_raw_fd().to_string(),
        ]);
    }
    if spec.target_kind == CronTargetKind::Repository {
        command.args(["task", "reconcile", "--json"]);
    } else {
        command.args([
            "--wave",
            &spec.wave,
            "--batch",
            spec.target_kind.as_str(),
            "--",
            &spec.flow,
        ]);
    }
    command
        .current_dir(&spec.working_directory)
        .env_clear()
        .env("PATH", &spec.host.path_env)
        .env("LF_HOME", &spec.host.lf_home)
        .stdout(Stdio::from(stdout))
        .stderr(Stdio::from(stderr));
    if let Some(home) = dirs::home_dir() {
        command.env("HOME", home);
    }
    for key in ["USER", "TMPDIR", "LANG", "LC_ALL", "LC_CTYPE"] {
        if let Some(value) = std::env::var_os(key) {
            command.env(key, value);
        }
    }
    #[cfg(unix)]
    if let Some(file) = execution {
        use std::os::fd::AsRawFd;
        use std::os::unix::process::CommandExt;
        let fd = file.as_raw_fd();
        // SAFETY: the parent keeps the file alive until spawn returns. The child
        // only changes an fd flag with an async-signal-safe syscall before exec.
        unsafe {
            command.pre_exec(move || {
                if libc::fcntl(fd, libc::F_SETFD, 0) == -1 {
                    return Err(std::io::Error::last_os_error());
                }
                Ok(())
            });
        }
    }
    command.spawn()
}

fn wait_for_cron_target(
    child: &mut Child,
    receipt: &CronReceipt,
    timeout: Option<Duration>,
) -> OpsResult<ExitStatus> {
    let continuation = || {
        format!(
            "inspect receipt {} at {}; retry at the next configured firing after the existing job lock is free",
            receipt.id,
            receipt.log_path.display()
        )
    };
    let observation_error = |error| OpsError::CommandFailed {
        command: format!("observe cron target {}", receipt.id),
        stderr: format!("exit result is unknown: {error}; {}", continuation()),
    };
    let Some(timeout) = timeout else {
        return child.wait().map_err(observation_error);
    };
    let deadline = Instant::now() + timeout;
    loop {
        if let Some(status) = child.try_wait().map_err(observation_error)? {
            return Ok(status);
        }
        let remaining = deadline.saturating_duration_since(Instant::now());
        if remaining.is_zero() {
            return Err(OpsError::ReleaseDeferred {
                reason: format!(
                    "telemetry recovery {} has no terminal result after {}s; the check has not been stopped",
                    receipt.id,
                    timeout.as_secs()
                ),
                continuation: continuation(),
            });
        }
        std::thread::sleep(remaining.min(Duration::from_millis(250)));
    }
}

fn validate_installed_spec(spec: &CronSpec, wave: &str, flow: &str) -> OpsResult<()> {
    if spec.wave == wave && spec.flow == flow {
        return Ok(());
    }
    Err(OpsError::Message(format!(
        "installed cron metadata names {}/{} instead of {wave}/{flow}",
        spec.wave, spec.flow
    )))
}

fn new_receipt(spec: &CronSpec, machine_id: &MachineId, source: CronSource) -> CronReceipt {
    CronReceipt {
        schema_version: 1,
        id: CronReceiptId::new(),
        runner_pid: std::process::id(),
        runner_started_at: crate::journal::process_started_at(std::process::id())
            .ok()
            .flatten(),
        machine_id: machine_id.clone(),
        wave: spec.wave.clone(),
        flow: spec.flow.clone(),
        target_kind: spec.target_kind,
        source,
        schedule: spec.schedule.expression().to_string(),
        repo: spec.working_directory.clone(),
        lf_path: spec.lf_path.clone(),
        log_path: spec.log_path(),
        started_at: Utc::now().timestamp(),
        finished_at: None,
        outcome: CronOutcome::Running,
        exit_code: None,
        error: None,
    }
}

pub(crate) fn read_receipts(
    root: &Path,
    wave: &str,
    flow: Option<&str>,
) -> OpsResult<Vec<CronReceipt>> {
    let wave_root = root.join(safe_component(wave));
    if !wave_root.is_dir() {
        return Ok(Vec::new());
    }
    let flow_dirs = match flow {
        Some(flow) => vec![wave_root.join(safe_component(flow))],
        None => fs::read_dir(&wave_root)?
            .filter_map(Result::ok)
            .filter_map(|entry| entry.file_type().ok()?.is_dir().then_some(entry.path()))
            .collect(),
    };
    let mut receipts = Vec::new();
    for dir in flow_dirs {
        if !dir.is_dir() {
            continue;
        }
        for entry in fs::read_dir(dir)? {
            let entry = entry?;
            if entry.path().extension().and_then(|value| value.to_str()) != Some("json") {
                continue;
            }
            let bytes = fs::read(entry.path())?;
            let receipt: CronReceipt = serde_json::from_slice(&bytes).map_err(|error| {
                OpsError::Parse(format!(
                    "invalid cron receipt {}: {error}",
                    entry.path().display()
                ))
            })?;
            if receipt.schema_version != 1 {
                return Err(OpsError::Parse(format!(
                    "unsupported cron receipt schema {} in {}",
                    receipt.schema_version,
                    entry.path().display()
                )));
            }
            if receipt.wave == wave && flow.is_none_or(|flow| receipt.flow == flow) {
                receipts.push(receipt);
            }
        }
    }
    Ok(receipts)
}

const MINUTE_RECEIPTS_KEPT: usize = 10;
const MINUTE_LOG_LIMIT: u64 = 1024 * 1024;

/// Keep recent receipts plus the latest success and failure.
fn prune_minute_receipts(root: &Path, spec: &CronSpec, current: &CronReceiptId) -> OpsResult<()> {
    let mut receipts = read_receipts(root, &spec.wave, Some(&spec.flow))?;
    receipts.sort_by_key(|receipt| std::cmp::Reverse(receipt.started_at));
    let latest = |outcome| {
        receipts
            .iter()
            .find(|receipt| receipt.outcome == outcome)
            .map(|receipt| receipt.id.clone())
    };
    let kept = [latest(CronOutcome::Succeeded), latest(CronOutcome::Failed)];
    let dir = root
        .join(safe_component(&spec.wave))
        .join(safe_component(&spec.flow));
    for receipt in receipts.iter().skip(MINUTE_RECEIPTS_KEPT) {
        if &receipt.id == current || kept.contains(&Some(receipt.id.clone())) {
            continue;
        }
        fs::remove_file(dir.join(format!(
            "{}-{}.json",
            receipt.started_at,
            receipt.id.as_str()
        )))?;
    }
    Ok(())
}

fn write_receipt(root: &Path, receipt: &CronReceipt) -> OpsResult<()> {
    let dir = root
        .join(safe_component(&receipt.wave))
        .join(safe_component(&receipt.flow));
    fs::create_dir_all(&dir)?;
    #[cfg(unix)]
    fs::set_permissions(&dir, fs::Permissions::from_mode(0o700))?;
    let path = dir.join(format!(
        "{}-{}.json",
        receipt.started_at,
        receipt.id.as_str()
    ));
    let temporary = dir.join(format!(".{}.tmp", receipt.id.as_str()));
    let bytes = serde_json::to_vec_pretty(receipt)
        .map_err(|error| OpsError::Parse(format!("serialize cron receipt: {error}")))?;
    write_private_file(&temporary, &bytes)?;
    fs::rename(&temporary, path)?;
    sync_directory(&dir)?;
    if receipt.flow == "release-run" && receipt.finished_at.is_some() {
        if let Some(home) = root.parent().and_then(Path::parent) {
            accounting::finish_process(home, receipt)?;
        }
    }
    Ok(())
}

pub(crate) fn write_private_file(path: &Path, bytes: &[u8]) -> OpsResult<()> {
    let mut file = OpenOptions::new()
        .create(true)
        .truncate(true)
        .write(true)
        .open(path)?;
    #[cfg(unix)]
    {
        file.set_permissions(fs::Permissions::from_mode(0o600))?;
    }
    file.write_all(bytes)?;
    file.sync_all()?;
    Ok(())
}

fn sync_directory(path: &Path) -> OpsResult<()> {
    #[cfg(unix)]
    fs::File::open(path)?.sync_all()?;
    Ok(())
}

fn plist_path(dir: &Path, wave: &str, flow: &str) -> PathBuf {
    dir.join(format!("{}.plist", label(wave, flow).replace('/', ".")))
}

pub fn repository_cron_key(repo: &Path, home: &MachineId) -> String {
    use sha2::{Digest, Sha256};
    let mut hash = Sha256::new();
    hash.update(repo.as_os_str().as_encoded_bytes());
    hash.update([0]);
    hash.update(home.as_str());
    format!("repository-{}", &hex::encode(hash.finalize())[..24])
}

fn label(wave: &str, flow: &str) -> String {
    format!("loopflow.cron.{wave}.{flow}").replace('/', ".")
}

fn is_cron_plist(name: &str) -> bool {
    name.strip_prefix("loopflow.cron.")
        .and_then(|name| name.strip_suffix(".plist"))
        .is_some_and(|name| !name.is_empty())
}

fn render_plist(spec: &CronSpec, activated_at: i64) -> String {
    let interval = if spec.schedule.every_minute {
        format!(
            "    <key>StartCalendarInterval</key>\n    <array>{}</array>",
            (0..60)
                .map(|minute| format!("<dict><key>Minute</key><integer>{minute}</integer></dict>"))
                .collect::<String>()
        )
    } else {
        format!(
        "    <key>StartCalendarInterval</key>\n    <dict>\n        <key>Hour</key>\n        <integer>{}</integer>\n        <key>Minute</key>\n        <integer>{}</integer>\n    </dict>",
        spec.schedule.hour, spec.schedule.minute
    )
    };
    let args = [
        spec.lf_path.to_string_lossy().to_string(),
        "wave".to_string(),
        "cron".to_string(),
        "run".to_string(),
        "--scheduled".to_string(),
        "--wave".to_string(),
        spec.wave.clone(),
        "--flow".to_string(),
        spec.flow.clone(),
    ];
    let program_args_xml = args
        .iter()
        .map(|arg| format!("        <string>{}</string>", xml_escape(arg)))
        .collect::<Vec<_>>()
        .join("\n");
    let metadata = [
        ("LoopflowWave", spec.wave.clone()),
        ("LoopflowFlow", spec.flow.clone()),
        ("LoopflowTargetKind", spec.target_kind.as_str().to_string()),
        ("LoopflowSchedule", spec.schedule.expression.clone()),
        ("LoopflowHomeId", spec.host.machine_id.to_string()),
        ("LoopflowActivatedAt", activated_at.to_string()),
        (
            "LoopflowRepo",
            spec.working_directory.to_string_lossy().to_string(),
        ),
        ("LoopflowLfPath", spec.lf_path.to_string_lossy().to_string()),
        (
            "LoopflowLfHome",
            spec.host.lf_home.to_string_lossy().to_string(),
        ),
        ("LoopflowPath", spec.host.path_env.clone()),
        (
            "LoopflowLogPath",
            spec.log_path().to_string_lossy().to_string(),
        ),
    ]
    .iter()
    .map(|(key, value)| {
        format!(
            "    <key>{key}</key>\n    <string>{}</string>",
            xml_escape(value)
        )
    })
    .collect::<Vec<_>>()
    .join("\n");

    format!(
        r#"<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
    <key>Label</key>
    <string>{label}</string>
{metadata}
    <key>ProgramArguments</key>
    <array>
{program_args_xml}
    </array>
    <key>EnvironmentVariables</key>
    <dict>
        <key>PATH</key>
        <string>{path_env}</string>
        <key>LF_HOME</key>
        <string>{lf_home}</string>
    </dict>
{interval}
    <key>WorkingDirectory</key>
    <string>{working_directory}</string>
    <key>StandardOutPath</key>
    <string>{log_path}</string>
    <key>StandardErrorPath</key>
    <string>{log_path}</string>
</dict>
</plist>
"#,
        label = xml_escape(&label(&spec.wave, &spec.flow)),
        path_env = xml_escape(&spec.host.path_env),
        lf_home = xml_escape(&spec.host.lf_home.to_string_lossy()),
        working_directory = xml_escape(&spec.working_directory.to_string_lossy()),
        log_path = xml_escape(&spec.log_path().to_string_lossy()),
    )
}

fn plist_string(content: &str, key: &str) -> Option<String> {
    let marker = format!("<key>{key}</key>");
    let value = content.split_once(&marker)?.1;
    let value = value.split_once("<string>")?.1;
    let value = value.split_once("</string>")?.0;
    Some(xml_unescape(value))
}

pub(crate) fn xml_escape(value: &str) -> String {
    value
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&apos;")
}

fn xml_unescape(value: &str) -> String {
    value
        .replace("&apos;", "'")
        .replace("&quot;", "\"")
        .replace("&gt;", ">")
        .replace("&lt;", "<")
        .replace("&amp;", "&")
}

fn safe_component(value: &str) -> String {
    value
        .chars()
        .map(|character| {
            if character.is_ascii_alphanumeric() || matches!(character, '-' | '_' | '.') {
                character
            } else {
                '_'
            }
        })
        .collect()
}

fn launchd_service(label: &str) -> OpsResult<String> {
    let output = Command::new("id").arg("-u").output()?;
    if !output.status.success() {
        return Err(OpsError::CommandFailed {
            command: "id -u".to_string(),
            stderr: String::from_utf8_lossy(&output.stderr).into_owned(),
        });
    }
    let uid = String::from_utf8_lossy(&output.stdout).trim().to_string();
    Ok(format!("gui/{uid}/{label}"))
}

fn run_launchctl_path(action: &str, path: &Path) -> OpsResult<()> {
    let output = Command::new("launchctl").arg(action).arg(path).output()?;
    if output.status.success() {
        return Ok(());
    }
    Err(OpsError::CommandFailed {
        command: format!("launchctl {action} {}", path.display()),
        stderr: String::from_utf8_lossy(&output.stderr).into_owned(),
    })
}

pub(crate) fn process_alive(pid: u32) -> bool {
    let Ok(pid) = i32::try_from(pid) else {
        return false;
    };
    // SAFETY: signal 0 does not mutate the target; it only checks whether the
    // process exists and is signalable by this user.
    let result = unsafe { libc::kill(pid, 0) };
    result == 0 || std::io::Error::last_os_error().raw_os_error() == Some(libc::EPERM)
}

fn status_label(status: &std::process::ExitStatus) -> String {
    status
        .code()
        .map(|code| format!("with status {code}"))
        .unwrap_or_else(|| "after a signal".to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::RefCell;

    use clap::Parser;

    use crate::lf::{Cli, Commands, FlowCommand, SkillCommand};

    #[derive(Debug, Default)]
    struct FakeLaunchctl {
        loaded: RefCell<HashSet<String>>,
    }

    impl Launchctl for FakeLaunchctl {
        fn load(&self, path: &Path) -> OpsResult<()> {
            let content = fs::read_to_string(path)?;
            let wave = plist_string(&content, "LoopflowWave")
                .ok_or_else(|| OpsError::Parse("missing wave".to_string()))?;
            let flow = plist_string(&content, "LoopflowFlow")
                .ok_or_else(|| OpsError::Parse("missing flow".to_string()))?;
            self.loaded.borrow_mut().insert(label(&wave, &flow));
            Ok(())
        }

        fn unload(&self, path: &Path) -> OpsResult<()> {
            if let Ok(content) = fs::read_to_string(path) {
                if let (Some(wave), Some(flow)) = (
                    plist_string(&content, "LoopflowWave"),
                    plist_string(&content, "LoopflowFlow"),
                ) {
                    self.loaded.borrow_mut().remove(&label(&wave, &flow));
                }
            }
            Ok(())
        }

        fn is_loaded(&self, label: &str) -> OpsResult<bool> {
            Ok(self.loaded.borrow().contains(label))
        }

        fn trigger(&self, label: &str) -> OpsResult<()> {
            if self.loaded.borrow().contains(label) {
                Ok(())
            } else {
                Err(OpsError::Message(format!("{label} is not loaded")))
            }
        }
    }

    fn host(root: &Path) -> CronHost {
        CronHost {
            machine_id: MachineId::new(),
            lf_home: root.join("home"),
            path_env: "/usr/bin:/bin".to_string(),
        }
    }

    fn spec(root: &Path, lf_path: &Path) -> CronSpec {
        named_spec(
            root,
            lf_path,
            "reliability",
            "wave-report",
            "0 0 3 * * *",
            CronTargetKind::Flow,
        )
    }

    fn named_spec(
        root: &Path,
        lf_path: &Path,
        wave: &str,
        flow: &str,
        schedule: &str,
        target_kind: CronTargetKind,
    ) -> CronSpec {
        CronSpec {
            wave: wave.to_string(),
            flow: flow.to_string(),
            target_kind,
            schedule: parse_schedule(schedule).unwrap(),
            working_directory: root.join("repo"),
            lf_path: lf_path.to_path_buf(),
            host: host(root),
        }
    }

    #[test]
    fn cron_runner_identity_preserves_unknown_history_and_rejects_reused_pid() {
        use crate::journal::ProcessIdentityEvidence;

        let temp = tempfile::TempDir::new().unwrap();
        let cron = spec(temp.path(), Path::new("/usr/bin/true"));
        let receipt = new_receipt(&cron, &cron.host.machine_id, CronSource::Scheduled);
        assert_eq!(receipt.runner_evidence(), ProcessIdentityEvidence::Live);

        let mut reused = receipt.clone();
        reused.runner_started_at = Some(receipt.runner_started_at.unwrap() - 86_400);
        assert_eq!(reused.runner_evidence(), ProcessIdentityEvidence::Dead);

        // Historical schema-1 receipts have no start identity. Neither an old
        // timestamp nor a missing PID can turn that absence into ownership proof.
        let mut historical = serde_json::to_value(&receipt).unwrap();
        historical
            .as_object_mut()
            .unwrap()
            .remove("runner_started_at");
        historical["runner_pid"] = u32::MAX.into();
        historical["started_at"] = 1.into();
        let historical: CronReceipt = serde_json::from_value(historical).unwrap();
        assert_eq!(historical.runner_started_at, None);
        assert_eq!(
            historical.runner_evidence(),
            ProcessIdentityEvidence::Unknown
        );
        assert_eq!(historical.outcome, CronOutcome::Running);
        assert_eq!(historical.finished_at, None);
    }

    #[test]
    fn telemetry_recovery_defers_while_the_existing_executor_owns_the_job() {
        let temp = tempfile::TempDir::new().unwrap();
        let launcher = temp.path().join("telemetry");
        let effect = temp.path().join("launched");
        fs::write(
            &launcher,
            format!("#!/bin/sh\ntouch '{}'\n", effect.display()),
        )
        .unwrap();
        fs::set_permissions(&launcher, fs::Permissions::from_mode(0o755)).unwrap();
        let spec = named_spec(
            temp.path(),
            &launcher,
            "infra",
            "telemetry-daily",
            "0 0 9 * * *",
            CronTargetKind::Flow,
        );
        fs::create_dir_all(&spec.working_directory).unwrap();
        add_cron(temp.path(), &spec, &FakeLaunchctl::default()).unwrap();
        let owner = accounting::claim_execution(&spec).unwrap().unwrap();
        let error = run_cron(
            temp.path(),
            &spec.wave,
            &spec.flow,
            &spec.host.machine_id,
            &spec.host.machine_id,
            CronSource::Recovery,
        )
        .unwrap_err();
        assert!(matches!(error, OpsError::ReleaseDeferred { .. }));
        assert!(!effect.exists());
        drop(owner);
        let receipt = run_cron(
            temp.path(),
            &spec.wave,
            &spec.flow,
            &spec.host.machine_id,
            &spec.host.machine_id,
            CronSource::Recovery,
        )
        .unwrap();
        assert_eq!(receipt.outcome, CronOutcome::Succeeded);
        assert!(effect.exists());
    }

    #[test]
    fn telemetry_deadline_preserves_unknown_receipt_and_surviving_child_exclusion() {
        struct Target {
            child: Child,
            allow: PathBuf,
        }
        impl Drop for Target {
            fn drop(&mut self) {
                let _ = fs::write(&self.allow, "finish");
                let _ = self.child.wait();
            }
        }

        let temp = tempfile::TempDir::new().unwrap();
        let launcher = temp.path().join("telemetry");
        let started = temp.path().join("started");
        let allow = temp.path().join("allow");
        let completed = temp.path().join("completed");
        fs::write(
            &launcher,
            format!(
                "#!/bin/sh\necho start >> '{}'\nwhile [ ! -f '{}' ]; do sleep 0.01; done\necho complete >> '{}'\n",
                started.display(), allow.display(), completed.display()
            ),
        )
        .unwrap();
        fs::set_permissions(&launcher, fs::Permissions::from_mode(0o755)).unwrap();
        let spec = named_spec(
            temp.path(),
            &launcher,
            "infra",
            "telemetry-daily",
            "0 0 9 * * *",
            CronTargetKind::Flow,
        );
        fs::create_dir_all(&spec.working_directory).unwrap();
        add_cron(temp.path(), &spec, &FakeLaunchctl::default()).unwrap();
        let receipt = new_receipt(&spec, &spec.host.machine_id, CronSource::Recovery);
        let root = receipt_root(&spec.host.lf_home);
        write_receipt(&root, &receipt).unwrap();
        let owner = accounting::claim_execution(&spec).unwrap().unwrap();
        let mut target = Target {
            child: spawn_cron_target(&spec, Some(&owner), &receipt).unwrap(),
            allow,
        };

        // Exercise the production wait with a short deadline, without adding a
        // runtime override for the release operation's one-hour policy.
        let before = Instant::now();
        let error =
            wait_for_cron_target(&mut target.child, &receipt, Some(Duration::from_millis(50)))
                .unwrap_err();
        assert!(before.elapsed() < Duration::from_secs(5));
        match error {
            OpsError::ReleaseDeferred {
                reason,
                continuation,
            } => {
                assert!(reason.contains(receipt.id.as_str()));
                assert!(reason.contains("has not been stopped"));
                assert!(continuation.contains(&receipt.log_path.display().to_string()));
            }
            other => panic!("expected deferred observation, got {other:?}"),
        }
        assert!(target.child.try_wait().unwrap().is_none());
        assert!(!completed.exists());
        drop(owner);
        assert!(accounting::claim_execution(&spec).unwrap().is_none());
        let contender = || {
            run_cron(
                temp.path(),
                &spec.wave,
                &spec.flow,
                &spec.host.machine_id,
                &spec.host.machine_id,
                CronSource::Recovery,
            )
        };
        assert!(matches!(contender(), Err(OpsError::ReleaseDeferred { .. })));
        assert_eq!(
            read_receipts(&root, &spec.wave, Some(&spec.flow))
                .unwrap()
                .into_iter()
                .find(|r| r.id == receipt.id)
                .unwrap(),
            receipt
        );

        fs::write(&target.allow, "finish").unwrap();
        assert!(target.child.wait().unwrap().success());
        assert_eq!(fs::read_to_string(&started).unwrap(), "start\n");
        assert_eq!(fs::read_to_string(&completed).unwrap(), "complete\n");
        let recovered = contender().unwrap();
        assert_eq!(recovered.outcome, CronOutcome::Succeeded);
        assert_ne!(recovered.id, receipt.id);
        let receipts = read_receipts(&root, &spec.wave, Some(&spec.flow)).unwrap();
        assert_eq!(receipts.iter().find(|r| r.id == receipt.id), Some(&receipt));
        assert_eq!(
            fs::read_to_string(&completed).unwrap(),
            "complete\ncomplete\n"
        );
    }

    #[test]
    #[cfg(unix)]
    fn telemetry_exit_observation_error_is_failure_without_a_fabricated_exit() {
        let temp = tempfile::TempDir::new().unwrap();
        let spec = spec(temp.path(), Path::new("/usr/bin/true"));
        fs::create_dir_all(&spec.working_directory).unwrap();
        let receipt = new_receipt(&spec, &spec.host.machine_id, CronSource::Recovery);
        let root = receipt_root(&spec.host.lf_home);
        write_receipt(&root, &receipt).unwrap();
        let mut child = spawn_cron_target(&spec, None, &receipt).unwrap();
        let mut status = 0;
        // SAFETY: this is our own child PID and status points to a valid integer.
        // Reap outside Child to reproduce an unavailable exit observation.
        assert_eq!(
            unsafe { libc::waitpid(child.id() as libc::pid_t, &mut status, 0) },
            child.id() as libc::pid_t
        );
        let error = wait_for_cron_target(&mut child, &receipt, Some(Duration::from_millis(50)))
            .unwrap_err();
        assert!(matches!(error, OpsError::CommandFailed { .. }));
        assert_eq!(
            read_receipts(&root, &spec.wave, Some(&spec.flow)).unwrap(),
            vec![receipt]
        );
    }

    #[test]
    fn released_receipts_keep_their_machine_identity() {
        let temp = tempfile::tempdir().unwrap();
        let spec = spec(temp.path(), Path::new("/usr/bin/true"));
        let receipt = new_receipt(&spec, &spec.host.machine_id, CronSource::Scheduled);
        let mut released = serde_json::to_value(&receipt).unwrap();
        let object = released.as_object_mut().unwrap();
        let id = object.remove("machine_id").unwrap();
        object.insert("home_id".into(), id);
        let root = receipt_root(&spec.host.lf_home);
        let dir = root.join(&spec.wave).join(&spec.flow);
        fs::create_dir_all(&dir).unwrap();
        fs::write(
            dir.join(format!("{}-{}.json", receipt.started_at, receipt.id)),
            serde_json::to_vec(&released).unwrap(),
        )
        .unwrap();
        assert_eq!(
            read_receipts(&root, &spec.wave, Some(&spec.flow)).unwrap(),
            vec![receipt.clone()]
        );
        let current = serde_json::to_value(receipt).unwrap();
        assert!(current.get("machine_id").is_some());
        assert!(current.get("home_id").is_none());
    }

    #[test]
    fn add_list_remove_round_trips_loaded_launchd_spec() {
        let temp = tempfile::TempDir::new().unwrap();
        let launchctl = FakeLaunchctl::default();
        let spec = spec(temp.path(), Path::new("/usr/bin/true"));
        fs::create_dir_all(&spec.working_directory).unwrap();

        let installed = add_cron(temp.path(), &spec, &launchctl).unwrap();
        assert_eq!(installed.label, "loopflow.cron.reliability.wave-report");
        assert!(installed.loaded);
        assert_eq!(installed.schedule, "0 0 3 * * *");
        assert_eq!(installed.machine_id, spec.host.machine_id);
        assert!(installed.activated_at > 0);
        assert_eq!(
            fs::metadata(&installed.path).unwrap().permissions().mode() & 0o777,
            0o600
        );

        #[cfg(target_os = "macos")]
        assert!(Command::new("plutil")
            .args(["-lint", installed.path.to_str().unwrap()])
            .output()
            .unwrap()
            .status
            .success());

        let content = fs::read_to_string(&installed.path).unwrap();
        assert!(content.contains("<string>cron</string>"));
        assert!(content.contains("<string>run</string>"));
        assert!(content.contains("<string>--scheduled</string>"));
        assert!(content.contains("<key>EnvironmentVariables</key>"));
        assert!(content.contains("<key>PATH</key>"));
        assert!(content.contains("<key>LF_HOME</key>"));
        assert!(content.contains("<key>LoopflowActivatedAt</key>"));
        assert!(!content.contains("DOPPLER_TOKEN"));
        assert!(!content.contains("LF_RUN_ID"));

        let resynced = add_cron(temp.path(), &spec, &launchctl).unwrap();
        assert_eq!(resynced.activated_at, installed.activated_at);

        assert_eq!(
            list_crons(temp.path(), &launchctl).unwrap(),
            vec![installed.clone()]
        );

        let removed = remove_cron(temp.path(), "reliability", "wave-report", &launchctl).unwrap();
        assert_eq!(removed, Some(installed.clone()));
        assert!(!installed.path.exists());
    }

    #[test]
    fn legacy_cron_activation_is_recovered_from_its_first_scheduled_receipt() {
        let temp = tempfile::TempDir::new().unwrap();
        let launchctl = FakeLaunchctl::default();
        let spec = spec(temp.path(), Path::new("/usr/bin/true"));
        fs::create_dir_all(&spec.working_directory).unwrap();
        let installed = add_cron(temp.path(), &spec, &launchctl).unwrap();
        let content = fs::read_to_string(&installed.path).unwrap();
        let activation = format!(
            "    <key>LoopflowActivatedAt</key>\n    <string>{}</string>\n",
            installed.activated_at
        );
        fs::write(&installed.path, content.replace(&activation, "")).unwrap();
        let mut receipt = new_receipt(&spec, &spec.host.machine_id, CronSource::Scheduled);
        receipt.started_at = 1_787_419_441;
        receipt.finished_at = Some(receipt.started_at + 60);
        receipt.outcome = CronOutcome::Succeeded;
        receipt.exit_code = Some(0);
        write_receipt(&receipt_root(&spec.host.lf_home), &receipt).unwrap();

        let obligations = list_cron_obligations(temp.path()).unwrap();

        assert_eq!(obligations.len(), 1);
        assert_eq!(obligations[0].activated_at, receipt.started_at);
        assert_eq!(obligations[0].receipts, vec![receipt]);
    }

    #[test]
    fn list_rejects_a_loopflow_job_without_durable_metadata() {
        let temp = tempfile::TempDir::new().unwrap();
        let path = temp.path().join("loopflow.cron.legacy.job.plist");
        fs::write(
            path,
            "<plist><dict><key>Label</key><string>loopflow.cron.legacy.job</string></dict></plist>",
        )
        .unwrap();

        let error = list_crons(temp.path(), &FakeLaunchctl::default()).unwrap_err();
        assert!(error.to_string().contains("missing required LoopflowWave"));
        assert!(error.to_string().contains("lf wave cron sync"));
    }

    #[test]
    fn repository_tick_runs_each_minute_with_no_agent_or_wave() {
        let temp = tempfile::TempDir::new().unwrap();
        let executable = temp.path().join("fake-lf");
        fs::write(&executable, "#!/bin/sh\nprintf '%s\\n' \"$@\"\n").unwrap();
        fs::set_permissions(&executable, fs::Permissions::from_mode(0o700)).unwrap();
        let mut cron = spec(temp.path(), &executable);
        cron.wave.clear();
        cron.flow = super::repository_cron_key(&cron.working_directory, &cron.host.machine_id);
        cron.target_kind = CronTargetKind::Repository;
        cron.schedule = parse_schedule("every-minute").unwrap();
        fs::create_dir_all(&cron.working_directory).unwrap();
        let plist = super::render_plist(&cron, 0);
        assert_eq!(plist.matches("<key>Minute</key>").count(), 60);
        assert!(!plist.contains("<key>Hour</key>"));
        assert!(!plist.contains("KeepAlive"));
        let receipt = super::new_receipt(&cron, &cron.host.machine_id, CronSource::Scheduled);
        assert!(super::spawn_cron_target(&cron, None, &receipt)
            .unwrap()
            .wait()
            .unwrap()
            .success());
        assert_eq!(
            fs::read_to_string(cron.log_path()).unwrap(),
            "task\nreconcile\n--json\n"
        );
        assert_ne!(
            cron.flow,
            super::repository_cron_key(&cron.working_directory, &MachineId::new())
        );
    }

    #[test]
    fn minute_checks_keep_bounded_receipts_and_the_latest_success() {
        let temp = tempfile::TempDir::new().unwrap();
        let agents = temp.path().join("agents");
        let launchctl = FakeLaunchctl::default();
        let mut cron = spec(temp.path(), Path::new("/usr/bin/true"));
        cron.wave.clear();
        cron.flow = super::repository_cron_key(&cron.working_directory, &cron.host.machine_id);
        cron.target_kind = CronTargetKind::Repository;
        cron.schedule = parse_schedule("every-minute").unwrap();
        fs::create_dir_all(&cron.working_directory).unwrap();
        add_cron(&agents, &cron, &launchctl).unwrap();
        let run = || {
            run_cron(
                &agents,
                "",
                &cron.flow,
                &cron.host.machine_id,
                &cron.host.machine_id,
                CronSource::Scheduled,
            )
        };
        for _ in 0..14 {
            run().unwrap();
        }
        let root = receipt_root(&cron.host.lf_home);
        let receipts = list_cron_receipts(&root, "", Some(&cron.flow), 1).unwrap();
        assert!(receipts.len() <= super::MINUTE_RECEIPTS_KEPT + 1);
        assert_eq!(receipts[0].outcome, CronOutcome::Succeeded);

        let mut failing = cron.clone();
        failing.lf_path = PathBuf::from("/usr/bin/false");
        add_cron(&agents, &failing, &launchctl).unwrap();
        for _ in 0..14 {
            assert!(run().is_err());
        }
        let receipts = list_cron_receipts(&root, "", Some(&cron.flow), 1).unwrap();
        assert!(receipts.len() <= super::MINUTE_RECEIPTS_KEPT + 2);
        assert!(receipts
            .iter()
            .any(|receipt| receipt.outcome == CronOutcome::Succeeded));
    }

    #[test]
    fn parse_schedule_accepts_alias_and_cron_expression() {
        assert_eq!(
            parse_schedule("daily").unwrap(),
            CronSchedule {
                expression: "0 0 3 * * *".to_string(),
                hour: 3,
                minute: 0,
                every_minute: false,
            }
        );
        assert_eq!(
            parse_schedule("0 30 17 * * *").unwrap(),
            CronSchedule {
                expression: "0 30 17 * * *".to_string(),
                hour: 17,
                minute: 30,
                every_minute: false,
            }
        );
    }

    #[test]
    fn daily_time_of_rejects_non_daily_schedules() {
        assert!(daily_time_of("0 0 * * * *").is_err());
        assert!(daily_time_of("0 0 9 * * MON").is_err());
        assert!(daily_time_of("0 0 9 * * MON-FRI *").is_err());
        assert!(daily_time_of("0 0 9 1 * * *").is_err());
        assert!(daily_time_of("not a schedule").is_err());
    }

    #[test]
    fn telemetry_installation_retains_legacy_replacement_move_and_removal() {
        let temp = tempfile::tempdir().unwrap();
        let agents = temp.path().join("agents");
        fs::create_dir_all(&agents).unwrap();
        let launchctl = FakeLaunchctl::default();
        let mut telemetry = named_spec(
            temp.path(),
            Path::new("/usr/bin/true"),
            "infra",
            "telemetry-daily",
            "0 0 9 * * *",
            CronTargetKind::Flow,
        );
        // A pre-cutover plist has no retained segment yet.
        let path = plist_path(&agents, &telemetry.wave, &telemetry.flow);
        fs::write(&path, render_plist(&telemetry, 0)).unwrap();
        telemetry.schedule = parse_schedule("0 0 8 * * *").unwrap();
        add_cron(&agents, &telemetry, &launchctl).unwrap();
        let rows = accounting::read(&telemetry.host.lf_home).unwrap();
        assert_eq!(rows.len(), 2);
        let old = rows.iter().find(|r| r.schedule == "0 0 9 * * *").unwrap();
        assert_eq!(old.activated_at, 0);
        assert!(old.observed_at > old.activated_at);
        assert!(old.closed_at.is_some());
        let current = rows.iter().find(|r| r.closed_at.is_none()).unwrap();
        assert_eq!(current.replaces.as_ref(), Some(&old.id));
        add_cron(&agents, &telemetry, &launchctl).unwrap();
        assert_eq!(accounting::read(&telemetry.host.lf_home).unwrap(), rows);

        let old_home = telemetry.host.lf_home.clone();
        telemetry.host.lf_home = temp.path().join("other-home");
        telemetry.host.machine_id = MachineId::new();
        add_cron(&agents, &telemetry, &launchctl).unwrap();
        assert!(accounting::read(&old_home)
            .unwrap()
            .iter()
            .all(|s| s.closed_at.is_some()));
        remove_cron(&agents, &telemetry.wave, &telemetry.flow, &launchctl).unwrap();
        let moved = accounting::read(&telemetry.host.lf_home).unwrap();
        assert_eq!(moved.len(), 1);
        assert_eq!(moved[0].machine_id, telemetry.host.machine_id);
        assert!(moved[0].closed_at.is_some());
        assert!(moved[0].opportunities.is_empty());
        assert!(!path.exists());
    }

    #[test]
    fn sync_validates_all_specs_before_installing() {
        let temp = tempfile::TempDir::new().unwrap();
        let agents = temp.path().join("agents");
        let launchctl = FakeLaunchctl::default();
        let misplaced = named_spec(
            temp.path(),
            Path::new("/usr/bin/true"),
            "other",
            "telemetry-daily",
            "0 0 9 * * *",
            CronTargetKind::Flow,
        );
        let error = sync_crons(&agents, "infra", &[misplaced], &launchctl).unwrap_err();
        assert!(error.to_string().contains("belongs to Wave other"));
        assert!(!agents.exists());

        let duplicate = named_spec(
            temp.path(),
            Path::new("/usr/bin/true"),
            "infra",
            "telemetry-daily",
            "0 0 9 * * *",
            CronTargetKind::Flow,
        );
        let error = sync_crons(
            &agents,
            "infra",
            &[duplicate.clone(), duplicate],
            &launchctl,
        )
        .unwrap_err();
        assert!(error.to_string().contains("more than once"));
        assert!(!agents.exists());
    }

    #[test]
    fn sync_installs_declared_and_prunes_undeclared() {
        let temp = tempfile::TempDir::new().unwrap();
        let agents = temp.path().join("agents");
        let launchctl = FakeLaunchctl::default();
        let telemetry = named_spec(
            temp.path(),
            Path::new("/usr/bin/true"),
            "infra",
            "telemetry-daily",
            "0 0 9 * * *",
            CronTargetKind::Flow,
        );
        let first = sync_crons(&agents, "infra", &[telemetry], &launchctl).unwrap();
        assert_eq!(first.installed.len(), 1);
        assert!(first.removed.is_empty());

        let release = named_spec(
            temp.path(),
            Path::new("/usr/bin/true"),
            "infra",
            "release-run",
            "0 0 10 * * *",
            CronTargetKind::Skill,
        );
        let second = sync_crons(&agents, "infra", &[release], &launchctl).unwrap();
        assert_eq!(second.installed[0].flow, "release-run");
        assert_eq!(second.removed[0].flow, "telemetry-daily");
    }

    #[test]
    fn scheduled_work_follows_the_entry_gate_without_reinstalling_the_job() {
        let temp = tempfile::TempDir::new().unwrap();
        let agents = temp.path().join("agents");
        let root = crate::installation::root_for_home(temp.path());
        let role = crate::installation::ArtifactRole::Cli;
        let gate =
            crate::installation::install_entry_gate(&root, &role, Path::new("/usr/bin/false"))
                .unwrap();
        let spec = spec(temp.path(), &gate);
        fs::create_dir_all(&spec.working_directory).unwrap();
        let installed = add_cron(&agents, &spec, &FakeLaunchctl::default()).unwrap();
        let declaration = fs::read(&installed.path).unwrap();

        crate::installation::install_entry_gate(&root, &role, Path::new("/usr/bin/true")).unwrap();
        let receipt = run_cron(
            &agents,
            &spec.wave,
            &spec.flow,
            &spec.host.machine_id,
            &spec.host.machine_id,
            CronSource::Scheduled,
        )
        .unwrap();

        assert_eq!(receipt.outcome, CronOutcome::Succeeded);
        assert_eq!(receipt.lf_path, gate);
        assert_eq!(fs::read(&installed.path).unwrap(), declaration);
    }

    #[test]
    fn runner_persists_success_and_failure_receipts() {
        let temp = tempfile::TempDir::new().unwrap();
        let agents = temp.path().join("agents");
        let launchctl = FakeLaunchctl::default();
        let success = spec(temp.path(), Path::new("/usr/bin/true"));
        fs::create_dir_all(&success.working_directory).unwrap();
        add_cron(&agents, &success, &launchctl).unwrap();

        let receipt = run_cron(
            &agents,
            &success.wave,
            &success.flow,
            &success.host.machine_id,
            &success.host.machine_id,
            CronSource::Manual,
        )
        .unwrap();
        assert_eq!(receipt.outcome, CronOutcome::Succeeded);
        assert_eq!(receipt.exit_code, Some(0));
        let prior_receipts = cron_receipt_ids(
            &receipt_root(&success.host.lf_home),
            &success.wave,
            &success.flow,
        )
        .unwrap();

        let mut failure = success.clone();
        failure.lf_path = PathBuf::from("/usr/bin/false");
        add_cron(&agents, &failure, &launchctl).unwrap();
        assert!(run_cron(
            &agents,
            &failure.wave,
            &failure.flow,
            &failure.host.machine_id,
            &failure.host.machine_id,
            CronSource::Scheduled,
        )
        .is_err());

        let receipts = list_cron_receipts(
            &receipt_root(&failure.host.lf_home),
            &failure.wave,
            Some(&failure.flow),
            1,
        )
        .unwrap();
        assert_eq!(receipts.len(), 2);
        assert!(receipts
            .iter()
            .any(|receipt| receipt.outcome == CronOutcome::Succeeded));
        let failed = receipts
            .iter()
            .find(|receipt| receipt.outcome == CronOutcome::Failed)
            .unwrap();
        assert_eq!(failed.exit_code, Some(1));
        assert!(failed.error.as_deref().unwrap().contains("see "));
        let waited = wait_for_cron_receipt(
            &receipt_root(&failure.host.lf_home),
            &failure.wave,
            &failure.flow,
            &prior_receipts,
            failed.started_at,
            Duration::from_secs(1),
        )
        .unwrap();
        assert_eq!(waited.id, failed.id);
    }

    #[test]
    fn runner_uses_the_explicit_target_and_scrubs_task_authority() {
        let temp = tempfile::TempDir::new().unwrap();
        let executable = temp.path().join("fake-lf");
        fs::write(
            &executable,
            "#!/bin/sh\nprintf '%s\\n' \"$@\"\nprintf 'LF_RUN_ID=%s\\n' \"${LF_RUN_ID-unset}\"\n",
        )
        .unwrap();
        fs::set_permissions(&executable, fs::Permissions::from_mode(0o700)).unwrap();

        let agents = temp.path().join("agents");
        let launchctl = FakeLaunchctl::default();
        for (kind, name) in [
            (CronTargetKind::Flow, "show"),
            (CronTargetKind::Skill, "list"),
        ] {
            let cron = named_spec(
                temp.path(),
                &executable,
                "infrastructure/release",
                name,
                "0 0 3 * * *",
                kind,
            );
            fs::create_dir_all(&cron.working_directory).unwrap();
            add_cron(&agents, &cron, &launchctl).unwrap();
            assert!(cron.log_path().parent().unwrap().is_dir());
            run_cron(
                &agents,
                &cron.wave,
                &cron.flow,
                &cron.host.machine_id,
                &cron.host.machine_id,
                CronSource::Scheduled,
            )
            .unwrap();

            let output = fs::read_to_string(cron.log_path()).unwrap();
            let lines: Vec<_> = output.lines().collect();
            let (authority, args) = lines.split_last().unwrap();
            assert_eq!(*authority, "LF_RUN_ID=unset");
            let cli =
                Cli::try_parse_from(std::iter::once("lf").chain(args.iter().copied())).unwrap();
            match (kind, cli.command) {
                (
                    CronTargetKind::Flow,
                    Some(Commands::Flow {
                        cmd: FlowCommand::External(args),
                    }),
                )
                | (
                    CronTargetKind::Skill,
                    Some(Commands::Skill {
                        cmd: SkillCommand::External(args),
                    }),
                ) => {
                    assert_eq!(args, [name]);
                }
                (_, command) => panic!("scheduled definition became a command: {command:?}"),
            }
        }
    }

    #[test]
    fn placement_drift_fails_with_a_terminal_receipt_before_launch() {
        let temp = tempfile::TempDir::new().unwrap();
        let agents = temp.path().join("agents");
        let launchctl = FakeLaunchctl::default();
        let cron = spec(temp.path(), Path::new("/usr/bin/true"));
        fs::create_dir_all(&cron.working_directory).unwrap();
        add_cron(&agents, &cron, &launchctl).unwrap();
        let placed_machine = MachineId::new();

        let error = run_cron(
            &agents,
            &cron.wave,
            &cron.flow,
            &cron.host.machine_id,
            &placed_machine,
            CronSource::Scheduled,
        )
        .unwrap_err();
        assert!(error.to_string().contains("is placed on"));

        let receipts = list_cron_receipts(
            &receipt_root(&cron.host.lf_home),
            &cron.wave,
            Some(&cron.flow),
            1,
        )
        .unwrap();
        assert_eq!(receipts.len(), 1);
        assert_eq!(receipts[0].outcome, CronOutcome::Failed);
        assert!(receipts[0]
            .error
            .as_deref()
            .unwrap()
            .contains(placed_machine.as_str()));
    }

    #[test]
    fn registry_preflight_failure_is_durable_without_starting_the_target() {
        let temp = tempfile::TempDir::new().unwrap();
        let agents = temp.path().join("agents");
        let launchctl = FakeLaunchctl::default();
        let cron = spec(temp.path(), Path::new("/usr/bin/true"));
        fs::create_dir_all(&cron.working_directory).unwrap();
        add_cron(&agents, &cron, &launchctl).unwrap();

        let receipt = record_cron_preflight_failure(
            &agents,
            &cron.wave,
            &cron.flow,
            CronSource::Scheduled,
            "registry schema is incompatible; run `lf machine doctor`",
        )
        .unwrap();
        assert_eq!(receipt.outcome, CronOutcome::Failed);
        assert_eq!(receipt.exit_code, None);
        assert!(receipt
            .error
            .as_deref()
            .unwrap()
            .contains("registry schema is incompatible"));
        assert!(!cron.log_path().exists());
    }

    #[test]
    fn stale_running_receipt_is_derived_without_rewriting_it() {
        let temp = tempfile::TempDir::new().unwrap();
        let started_at = Utc::now().timestamp();
        let receipt = CronReceipt {
            schema_version: 1,
            id: CronReceiptId::new(),
            runner_pid: u32::MAX,
            runner_started_at: None,
            machine_id: MachineId::new(),
            wave: "infra".to_string(),
            flow: "telemetry".to_string(),
            target_kind: CronTargetKind::Flow,
            source: CronSource::Scheduled,
            schedule: "0 0 9 * * *".to_string(),
            repo: PathBuf::from("/repo"),
            lf_path: PathBuf::from("/bin/lf"),
            log_path: PathBuf::from("/repo/cron.log"),
            started_at,
            finished_at: None,
            outcome: CronOutcome::Running,
            exit_code: None,
            error: None,
        };
        let root = receipt_root(temp.path());
        write_receipt(&root, &receipt).unwrap();
        let path = root.join("infra/telemetry").join(format!(
            "{}-{}.json",
            receipt.started_at,
            receipt.id.as_str()
        ));
        let before = fs::read(&path).unwrap();

        let first = list_cron_receipts(&root, "infra", Some("telemetry"), 1).unwrap();
        let second = list_cron_receipts(&root, "infra", Some("telemetry"), 1).unwrap();

        assert_eq!(first, vec![receipt.clone()]);
        assert_eq!(second, first);
        assert!(receipt_is_stale(&second[0], started_at));
        assert_eq!(second[0].outcome, CronOutcome::Running);
        assert_eq!(fs::read(path).unwrap(), before);
    }

    #[test]
    fn duration_parser_is_bounded_and_explicit() {
        assert_eq!(
            parse_wait_duration("15m").unwrap(),
            Duration::from_secs(900)
        );
        assert_eq!(
            parse_wait_duration("3h").unwrap(),
            Duration::from_secs(10_800)
        );
        assert!(parse_wait_duration("0s").is_err());
        assert!(parse_wait_duration("25h").is_err());
        assert!(parse_wait_duration("later").is_err());
    }
    #[test]
    fn release_wake_links_one_execution_to_all_misses_without_promoting_zero_exit() {
        use std::os::unix::fs::PermissionsExt;
        let temp = tempfile::tempdir().unwrap();
        let executable = temp.path().join("lf");
        fs::write(&executable, "#!/bin/sh\nexit 0\n").unwrap();
        fs::set_permissions(&executable, fs::Permissions::from_mode(0o755)).unwrap();
        let mut spec = named_spec(
            temp.path(),
            &executable,
            "infrastructure",
            "release-run",
            "0 0 0 * * *",
            CronTargetKind::Flow,
        );
        spec.working_directory = temp.path().to_path_buf();
        let agents = temp.path().join("agents");
        fs::create_dir(&agents).unwrap();
        let now = Utc::now().timestamp();
        fs::write(
            plist_path(&agents, &spec.wave, &spec.flow),
            render_plist(&spec, now - 3 * 86400),
        )
        .unwrap();
        let receipt = run_cron(
            &agents,
            &spec.wave,
            &spec.flow,
            &spec.host.machine_id,
            &spec.host.machine_id,
            CronSource::Scheduled,
        )
        .unwrap();
        assert_eq!(receipt.outcome, CronOutcome::Succeeded);
        let records =
            accounting::history(&spec.host.lf_home, temp.path(), &spec.wave, now).unwrap();
        assert!(records[0].opportunities.len() >= 3);
        let attempts: Vec<_> = records[0]
            .opportunities
            .iter()
            .flat_map(|o| &o.attempts)
            .collect();
        assert_eq!(attempts.len(), 1);
        assert_eq!(attempts[0].receipt_id, receipt.id);
        assert_eq!(attempts[0].covered.len(), records[0].opportunities.len());
        assert!(matches!(
            attempts[0].outcome,
            accounting::ScheduledReleaseOutcome::Unverified { .. }
        ));

        // A competing physical wake cannot add an attempt to this frozen owner.
        let _owner = accounting::claim_execution(&spec).unwrap().unwrap();
        for _ in 0..2 {
            let error = run_cron(
                &agents,
                &spec.wave,
                &spec.flow,
                &spec.host.machine_id,
                &spec.host.machine_id,
                CronSource::Scheduled,
            )
            .unwrap_err();
            assert!(error.to_string().contains("next configured release due"));
        }
        let report =
            history::release_history(&spec.host.lf_home, temp.path(), &spec.wave, 0, now + 86400)
                .unwrap();
        assert_eq!(report.receipts.len(), 3);
        assert!(report.receipts.iter().any(|r| r.id == receipt.id));
        for physical in report.receipts.iter().filter(|r| r.id != receipt.id) {
            assert_eq!(physical.outcome, CronOutcome::Failed);
            assert!(physical
                .error
                .as_ref()
                .unwrap()
                .contains("next configured release due"));
        }
        assert_eq!(
            report.obligations[0]
                .opportunities
                .iter()
                .map(|o| o.attempts.len())
                .sum::<usize>(),
            1
        );
        assert_eq!(report.summary.published + report.summary.no_change, 0);
        assert!(report.summary.qualifying_pairs.is_empty());
    }
}
