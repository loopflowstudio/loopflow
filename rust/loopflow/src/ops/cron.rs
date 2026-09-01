use std::collections::HashSet;
use std::fs::{self, OpenOptions};
use std::io::Write;
#[cfg(unix)]
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::str::FromStr;
use std::time::{Duration, Instant};

use chrono::{TimeZone, Timelike, Utc};
use serde::{Deserialize, Serialize};

use crate::durable::{CronReceiptId, HomeId};
use crate::ops::error::{OpsError, OpsResult};

const RECEIPT_STALE_AFTER: i64 = 6 * 60 * 60;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CronHost {
    pub home_id: HomeId,
    pub lf_home: PathBuf,
    pub db_path: PathBuf,
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
}

impl CronSchedule {
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
}

impl CronTargetKind {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Flow => "flow",
            Self::Skill => "skill",
        }
    }
}

impl FromStr for CronTargetKind {
    type Err = OpsError;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        match value {
            "flow" => Ok(Self::Flow),
            "skill" => Ok(Self::Skill),
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
    pub home_id: HomeId,
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

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct CronObligation {
    pub(crate) wave: String,
    pub(crate) flow: String,
    pub(crate) target_kind: CronTargetKind,
    pub(crate) schedule: CronSchedule,
    pub(crate) home_id: HomeId,
    pub(crate) activated_at: i64,
    pub(crate) receipts: Vec<CronReceipt>,
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
    pub home_id: HomeId,
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
            .args(["kickstart", "-k", &service])
            .output()?;
        if output.status.success() {
            return Ok(());
        }
        Err(OpsError::CommandFailed {
            command: format!("launchctl kickstart -k {service}"),
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
    fs::create_dir_all(spec.working_directory.join(".lf/logs"))?;
    let path = plist_path(launch_agents_dir, &spec.wave, &spec.flow);
    let now = Utc::now().timestamp();
    let activated_at = if path.exists() {
        let prior = read_cron_obligation(&path)?;
        if same_obligation(&prior, spec) {
            prior.activated_at
        } else {
            now
        }
    } else {
        now
    };
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
    launchctl.unload(&path)?;
    fs::remove_file(&path)?;
    Ok(Some(cron))
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
        "daily" => schedule_from_cron("0 0 3 * * *"),
        _ => schedule_from_cron(value),
    }
}

/// The launchd [`CronSchedule`] for a declared cron expression, or an error
/// describing why launchd can't run it.
pub fn schedule_from_cron(expr: &str) -> OpsResult<CronSchedule> {
    let (hour, minute) = daily_time_of(expr)?;
    Ok(CronSchedule {
        expression: expr.to_string(),
        hour,
        minute,
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
    current_home: &HomeId,
    placed_home: &HomeId,
    source: CronSource,
) -> OpsResult<CronReceipt> {
    let path = plist_path(launch_agents_dir, wave, flow);
    let spec = read_cron_spec(&path)?;
    validate_installed_spec(&spec, wave, flow)?;
    let root = receipt_root(&spec.host.lf_home);
    let mut receipt = new_receipt(&spec, current_home, source);
    write_receipt(&root, &receipt)?;

    let placement_error = if spec.host.home_id != *placed_home {
        Some(format!(
            "installed for Home {}, but Wave {wave} is placed on {placed_home}; run `lf cron sync --wave {wave}` on the placed Home",
            spec.host.home_id
        ))
    } else if *current_home != *placed_home {
        Some(format!(
            "current Home is {current_home}, but Wave {wave} is placed on {placed_home}"
        ))
    } else {
        None
    };
    if let Some(error) = placement_error {
        receipt.finished_at = Some(Utc::now().timestamp());
        receipt.outcome = CronOutcome::Failed;
        receipt.error = Some(error.clone());
        write_receipt(&root, &receipt)?;
        return Err(OpsError::Message(format!(
            "refusing cron {wave}/{flow}: {error}"
        )));
    }

    let result = spawn_cron_target(&spec);
    receipt.finished_at = Some(Utc::now().timestamp());
    match result {
        Ok(status) if status.success() => {
            receipt.outcome = CronOutcome::Succeeded;
            receipt.exit_code = status.code();
            write_receipt(&root, &receipt)?;
            Ok(receipt)
        }
        Ok(status) => {
            receipt.outcome = CronOutcome::Failed;
            receipt.exit_code = status.code();
            receipt.error = Some(format!(
                "target exited {}; see {}",
                status_label(&status),
                receipt.log_path.display()
            ));
            write_receipt(&root, &receipt)?;
            Err(OpsError::CommandFailed {
                command: format!("cron {wave}/{flow}"),
                stderr: receipt.error.clone().unwrap_or_default(),
            })
        }
        Err(error) => {
            receipt.outcome = CronOutcome::Failed;
            receipt.error = Some(format!(
                "could not start target: {error}; see {}",
                receipt.log_path.display()
            ));
            write_receipt(&root, &receipt)?;
            Err(OpsError::CommandFailed {
                command: format!("cron {wave}/{flow}"),
                stderr: receipt.error.clone().unwrap_or_default(),
            })
        }
    }
}

/// Persist a terminal receipt when scheduled execution cannot read its Home
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
    let mut receipt = new_receipt(&spec, &spec.host.home_id, source);
    write_receipt(&root, &receipt)?;
    receipt.finished_at = Some(Utc::now().timestamp());
    receipt.outcome = CronOutcome::Failed;
    receipt.error = Some(format!("Home placement preflight failed: {error}"));
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
            receipt.source == CronSource::Scheduled
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
                "timed out after {}s waiting for cron {wave}/{flow}; inspect `lf cron history --wave {wave} --flow {flow}`",
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
        home_id: spec.host.home_id.clone(),
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
                "{} is missing required {key} metadata; run `lf cron sync --wave <wave>`",
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
            home_id: HomeId::parse(&required("LoopflowHomeId")?)
                .map_err(|error| OpsError::Parse(error.to_string()))?,
            lf_home: PathBuf::from(required("LoopflowLfHome")?),
            db_path: PathBuf::from(required("LoopflowDbPath")?),
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
                "{} has invalid LoopflowActivatedAt {value:?}: {error}; run `lf cron sync --wave {}`",
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
                    && receipt.target_kind == spec.target_kind
                    && receipt.home_id == spec.host.home_id
                    && receipt.schedule == spec.schedule.expression()
            })
            .map(|receipt| receipt.started_at)
            .min()
            .or_else(|| file_timestamp(path))
            .ok_or_else(|| {
                OpsError::Message(format!(
                    "cannot recover activation time for legacy cron {}; run `lf cron sync --wave {}`",
                    path.display(),
                    spec.wave
                ))
            })?,
    };
    Ok(CronObligation {
        wave: spec.wave,
        flow: spec.flow,
        target_kind: spec.target_kind,
        schedule: spec.schedule,
        home_id: spec.host.home_id,
        activated_at,
        receipts,
    })
}

fn same_obligation(prior: &CronObligation, spec: &CronSpec) -> bool {
    prior.wave == spec.wave
        && prior.flow == spec.flow
        && prior.target_kind == spec.target_kind
        && prior.schedule == spec.schedule
        && prior.home_id == spec.host.home_id
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

fn spawn_cron_target(spec: &CronSpec) -> std::io::Result<std::process::ExitStatus> {
    if let Some(parent) = spec.log_path().parent() {
        fs::create_dir_all(parent)?;
    }
    let stdout = OpenOptions::new()
        .create(true)
        .append(true)
        .open(spec.log_path())?;
    let stderr = stdout.try_clone()?;
    let mut command = Command::new(&spec.lf_path);
    command
        .args([
            "--wave",
            &spec.wave,
            "--batch",
            spec.target_kind.as_str(),
            &spec.flow,
        ])
        .current_dir(&spec.working_directory)
        .env_clear()
        .env("PATH", &spec.host.path_env)
        .env("LF_HOME", &spec.host.lf_home)
        .env("LF_DB_PATH", &spec.host.db_path)
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
    command.status()
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

fn new_receipt(spec: &CronSpec, home_id: &HomeId, source: CronSource) -> CronReceipt {
    CronReceipt {
        schema_version: 1,
        id: CronReceiptId::new(),
        runner_pid: std::process::id(),
        home_id: home_id.clone(),
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

fn read_receipts(root: &Path, wave: &str, flow: Option<&str>) -> OpsResult<Vec<CronReceipt>> {
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
    Ok(())
}

fn write_private_file(path: &Path, bytes: &[u8]) -> OpsResult<()> {
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

fn label(wave: &str, flow: &str) -> String {
    format!("loopflow.cron.{wave}.{flow}").replace('/', ".")
}

fn is_cron_plist(name: &str) -> bool {
    name.strip_prefix("loopflow.cron.")
        .and_then(|name| name.strip_suffix(".plist"))
        .is_some_and(|name| !name.is_empty())
}

fn render_plist(spec: &CronSpec, activated_at: i64) -> String {
    let interval = format!(
        "    <key>StartCalendarInterval</key>\n    <dict>\n        <key>Hour</key>\n        <integer>{}</integer>\n        <key>Minute</key>\n        <integer>{}</integer>\n    </dict>",
        spec.schedule.hour, spec.schedule.minute
    );
    let args = [
        spec.lf_path.to_string_lossy().to_string(),
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
        ("LoopflowHomeId", spec.host.home_id.to_string()),
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
        (
            "LoopflowDbPath",
            spec.host.db_path.to_string_lossy().to_string(),
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
        <key>LF_DB_PATH</key>
        <string>{db_path}</string>
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
        db_path = xml_escape(&spec.host.db_path.to_string_lossy()),
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

fn xml_escape(value: &str) -> String {
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

fn process_alive(pid: u32) -> bool {
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
            home_id: HomeId::new(),
            lf_home: root.join("home"),
            db_path: root.join("home/loopflow.db"),
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
    fn add_list_remove_round_trips_loaded_launchd_spec() {
        let temp = tempfile::TempDir::new().unwrap();
        let launchctl = FakeLaunchctl::default();
        let spec = spec(temp.path(), Path::new("/usr/bin/true"));
        fs::create_dir_all(&spec.working_directory).unwrap();

        let installed = add_cron(temp.path(), &spec, &launchctl).unwrap();
        assert_eq!(installed.label, "loopflow.cron.reliability.wave-report");
        assert!(installed.loaded);
        assert_eq!(installed.schedule, "0 0 3 * * *");
        assert_eq!(installed.home_id, spec.host.home_id);
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
        assert!(content.contains("<key>LF_DB_PATH</key>"));
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
        let mut receipt = new_receipt(&spec, &spec.host.home_id, CronSource::Scheduled);
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
        assert!(error.to_string().contains("lf cron sync"));
    }

    #[test]
    fn parse_schedule_accepts_alias_and_cron_expression() {
        assert_eq!(
            parse_schedule("daily").unwrap(),
            CronSchedule {
                expression: "0 0 3 * * *".to_string(),
                hour: 3,
                minute: 0,
            }
        );
        assert_eq!(
            parse_schedule("0 30 17 * * *").unwrap(),
            CronSchedule {
                expression: "0 30 17 * * *".to_string(),
                hour: 17,
                minute: 30,
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
            &success.host.home_id,
            &success.host.home_id,
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
            &failure.host.home_id,
            &failure.host.home_id,
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
        let cron = spec(temp.path(), &executable);
        fs::create_dir_all(&cron.working_directory).unwrap();
        add_cron(&agents, &cron, &launchctl).unwrap();
        run_cron(
            &agents,
            &cron.wave,
            &cron.flow,
            &cron.host.home_id,
            &cron.host.home_id,
            CronSource::Scheduled,
        )
        .unwrap();

        assert_eq!(
            fs::read_to_string(cron.log_path()).unwrap(),
            "--wave\nreliability\n--batch\nflow\nwave-report\nLF_RUN_ID=unset\n"
        );
    }

    #[test]
    fn placement_drift_fails_with_a_terminal_receipt_before_launch() {
        let temp = tempfile::TempDir::new().unwrap();
        let agents = temp.path().join("agents");
        let launchctl = FakeLaunchctl::default();
        let cron = spec(temp.path(), Path::new("/usr/bin/true"));
        fs::create_dir_all(&cron.working_directory).unwrap();
        add_cron(&agents, &cron, &launchctl).unwrap();
        let placed_home = HomeId::new();

        let error = run_cron(
            &agents,
            &cron.wave,
            &cron.flow,
            &cron.host.home_id,
            &placed_home,
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
            .contains(placed_home.as_str()));
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
            "registry schema is incompatible; run `lf doctor`",
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
            home_id: HomeId::new(),
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
}
