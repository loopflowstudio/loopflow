use std::collections::HashSet;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::str::FromStr;

use chrono::{TimeZone, Timelike, Utc};

use crate::engine::wave_config::WaveCronDef;
use crate::ops::error::{OpsError, OpsResult};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CronSpec {
    pub wave: String,
    pub flow: String,
    pub schedule: Schedule,
    pub working_directory: PathBuf,
    pub lf_path: PathBuf,
}

#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum Schedule {
    /// Fire once a day at a fixed host-local time.
    DailyAt { hour: u32, minute: u32 },
}

/// Reconcile summary from [`sync_crons`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CronSyncResult {
    /// Declared crons now present as launchd jobs (added or replaced).
    pub installed: Vec<InstalledCron>,
    /// Launchd jobs for this wave pruned because the flow is no longer declared.
    pub removed: Vec<InstalledCron>,
    /// Declared crons launchd can't run, each with the reason.
    pub skipped: Vec<SkippedCron>,
}

/// A declared cron the launchd host can't schedule, and why.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SkippedCron {
    pub flow: String,
    pub schedule: String,
    pub reason: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InstalledCron {
    pub wave: String,
    pub flow: String,
    pub label: String,
    pub path: PathBuf,
}

pub trait Launchctl {
    fn load(&self, path: &Path) -> OpsResult<()>;
    fn unload(&self, path: &Path) -> OpsResult<()>;
}

#[derive(Debug)]
pub struct SystemLaunchctl;

impl Launchctl for SystemLaunchctl {
    fn load(&self, path: &Path) -> OpsResult<()> {
        run_launchctl("load", path)
    }

    fn unload(&self, path: &Path) -> OpsResult<()> {
        run_launchctl("unload", path)
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
    if path.exists() {
        let _ = launchctl.unload(&path);
    }
    fs::write(&path, render_plist(spec))?;
    launchctl.load(&path)?;
    Ok(installed_cron(&path, &spec.wave, &spec.flow))
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
    let cron = installed_cron(&path, wave, flow);
    launchctl.unload(&path)?;
    fs::remove_file(&path)?;
    Ok(Some(cron))
}

pub fn list_crons(launch_agents_dir: &Path) -> OpsResult<Vec<InstalledCron>> {
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
        let Some((wave, flow)) = parse_plist_filename(name) else {
            continue;
        };
        crons.push(installed_cron(&path, &wave, &flow));
    }
    crons.sort_by(|a, b| a.label.cmp(&b.label));
    Ok(crons)
}

pub fn parse_schedule(value: &str) -> OpsResult<Schedule> {
    match value {
        "daily" => Ok(Schedule::DailyAt { hour: 3, minute: 0 }),
        _ => Err(OpsError::Parse(format!(
            "unsupported cron schedule: {value}"
        ))),
    }
}

/// The launchd [`Schedule`] for a declared cron expression, or an error
/// describing why launchd can't run it.
pub fn schedule_from_cron(expr: &str) -> OpsResult<Schedule> {
    let (hour, minute) = daily_time_of(expr)?;
    Ok(Schedule::DailyAt { hour, minute })
}

/// Classify a cron expression as a fixed daily time for launchd.
///
/// launchd `StartCalendarInterval` expresses a repeating wall-clock time, not an
/// arbitrary cron expression. We accept only schedules that fire once a day at a
/// fixed hour:minute — parse with the `cron` crate, take two consecutive fires
/// from a fixed anchor, and require them exactly 24h apart. Anything else
/// (sub-daily, weekly, multi-time) is an error so the caller skips it rather than
/// silently mismapping it onto the wrong launchd interval.
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
    let second = fires
        .next()
        .ok_or_else(|| OpsError::Parse(format!("cron schedule '{expr}' fires only once")))?;
    if second - first != chrono::Duration::hours(24) {
        return Err(OpsError::Parse(format!(
            "cron schedule '{expr}' is not a fixed daily time (launchd host supports daily only)"
        )));
    }
    Ok((first.hour(), first.minute()))
}

/// Reconcile the launchd jobs for one wave to match its declared crons.
///
/// Installs (or replaces) a launchd job per declared cron whose schedule launchd
/// can run, prunes jobs for this wave whose flow is no longer declared, and
/// reports declared crons launchd can't run. Idempotent: running it twice against
/// the same declaration leaves the same jobs installed.
pub fn sync_crons(
    launch_agents_dir: &Path,
    wave: &str,
    declared: &[WaveCronDef],
    working_directory: &Path,
    lf_path: &Path,
    launchctl: &dyn Launchctl,
) -> OpsResult<CronSyncResult> {
    let mut installed = Vec::new();
    let mut skipped = Vec::new();
    let mut desired_flows = HashSet::new();

    for cron in declared {
        match schedule_from_cron(&cron.schedule) {
            Ok(schedule) => {
                let spec = CronSpec {
                    wave: wave.to_string(),
                    flow: cron.flow.clone(),
                    schedule,
                    working_directory: working_directory.to_path_buf(),
                    lf_path: lf_path.to_path_buf(),
                };
                installed.push(add_cron(launch_agents_dir, &spec, launchctl)?);
                desired_flows.insert(cron.flow.clone());
            }
            Err(err) => skipped.push(SkippedCron {
                flow: cron.flow.clone(),
                schedule: cron.schedule.clone(),
                reason: err.to_string(),
            }),
        }
    }

    let mut removed = Vec::new();
    for existing in list_crons(launch_agents_dir)? {
        if existing.wave == wave && !desired_flows.contains(&existing.flow) {
            if let Some(cron) =
                remove_cron(launch_agents_dir, &existing.wave, &existing.flow, launchctl)?
            {
                removed.push(cron);
            }
        }
    }

    Ok(CronSyncResult {
        installed,
        removed,
        skipped,
    })
}

pub fn default_launch_agents_dir() -> OpsResult<PathBuf> {
    let home =
        dirs::home_dir().ok_or_else(|| OpsError::Message("no home directory".to_string()))?;
    Ok(home.join("Library/LaunchAgents"))
}

pub fn resolve_lf_path() -> OpsResult<PathBuf> {
    std::env::current_exe().map_err(Into::into)
}

fn plist_path(dir: &Path, wave: &str, flow: &str) -> PathBuf {
    dir.join(format!("{}.plist", label(wave, flow).replace('/', ".")))
}

fn installed_cron(path: &Path, wave: &str, flow: &str) -> InstalledCron {
    InstalledCron {
        wave: wave.to_string(),
        flow: flow.to_string(),
        label: label(wave, flow),
        path: path.to_path_buf(),
    }
}

fn label(wave: &str, flow: &str) -> String {
    format!("loopflow.cron.{wave}.{flow}").replace('/', ".")
}

fn parse_plist_filename(name: &str) -> Option<(String, String)> {
    let stem = name
        .strip_prefix("loopflow.cron.")?
        .strip_suffix(".plist")?;
    let mut parts = stem.splitn(2, '.');
    let wave = parts.next()?.to_string();
    let flow = parts.next()?.to_string();
    if wave.is_empty() || flow.is_empty() {
        None
    } else {
        Some((wave, flow))
    }
}

fn render_plist(spec: &CronSpec) -> String {
    let interval = match spec.schedule {
        Schedule::DailyAt { hour, minute } => format!(
            "    <key>StartCalendarInterval</key>\n    <dict>\n        <key>Hour</key>\n        <integer>{hour}</integer>\n        <key>Minute</key>\n        <integer>{minute}</integer>\n    </dict>"
        ),
    };
    let args = [
        spec.lf_path.to_string_lossy().to_string(),
        spec.flow.clone(),
        "--wave".to_string(),
        spec.wave.clone(),
    ];
    let program_args_xml = args
        .iter()
        .map(|arg| format!("        <string>{}</string>", xml_escape(arg)))
        .collect::<Vec<_>>()
        .join("\n");

    format!(
        r#"<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
    <key>Label</key>
    <string>{label}</string>
    <key>ProgramArguments</key>
    <array>
{program_args_xml}
    </array>
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
        working_directory = xml_escape(&spec.working_directory.to_string_lossy()),
        log_path = xml_escape(&format!(
            "{}/.lf/logs/cron.{}.{}.log",
            spec.working_directory.display(),
            spec.wave,
            spec.flow.replace('/', ".")
        )),
    )
}

fn xml_escape(value: &str) -> String {
    value
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&apos;")
}

fn run_launchctl(action: &str, path: &Path) -> OpsResult<()> {
    let output = Command::new("launchctl").arg(action).arg(path).output()?;
    if output.status.success() {
        return Ok(());
    }
    Err(OpsError::CommandFailed {
        command: format!("launchctl {action} {}", path.display()),
        stderr: String::from_utf8_lossy(&output.stderr).into_owned(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::RefCell;

    #[derive(Debug, Default)]
    struct FakeLaunchctl {
        calls: RefCell<Vec<String>>,
    }

    impl Launchctl for FakeLaunchctl {
        fn load(&self, path: &Path) -> OpsResult<()> {
            self.calls
                .borrow_mut()
                .push(format!("load {}", path.display()));
            Ok(())
        }

        fn unload(&self, path: &Path) -> OpsResult<()> {
            self.calls
                .borrow_mut()
                .push(format!("unload {}", path.display()));
            Ok(())
        }
    }

    #[test]
    fn add_list_remove_round_trips_launchd_plist() {
        let temp = tempfile::TempDir::new().unwrap();
        let launchctl = FakeLaunchctl::default();
        let spec = CronSpec {
            wave: "reliability".to_string(),
            flow: "wave-report".to_string(),
            schedule: Schedule::DailyAt { hour: 3, minute: 0 },
            working_directory: temp.path().join("repo"),
            lf_path: PathBuf::from("/bin/lf"),
        };
        fs::create_dir_all(&spec.working_directory).unwrap();

        let installed = add_cron(temp.path(), &spec, &launchctl).unwrap();
        assert_eq!(installed.label, "loopflow.cron.reliability.wave-report");
        assert!(installed.path.exists());
        assert!(spec.working_directory.join(".lf/logs").is_dir());

        let content = fs::read_to_string(&installed.path).unwrap();
        assert!(content.contains("<string>/bin/lf</string>"));
        assert!(content.contains("<string>wave-report</string>"));
        assert!(content.contains("<string>--wave</string>"));
        assert!(content.contains("<string>reliability</string>"));
        assert!(content.contains("<key>StartCalendarInterval</key>"));
        assert!(content.contains(&format!(
            "<string>{}</string>",
            spec.working_directory.display()
        )));

        let crons = list_crons(temp.path()).unwrap();
        assert_eq!(crons, vec![installed.clone()]);

        let removed = remove_cron(temp.path(), "reliability", "wave-report", &launchctl).unwrap();
        assert_eq!(removed, Some(installed.clone()));
        assert!(!installed.path.exists());
        assert_eq!(
            launchctl.calls.borrow().as_slice(),
            &[
                format!("load {}", installed.path.display()),
                format!("unload {}", installed.path.display())
            ]
        );
    }

    #[test]
    fn parse_schedule_rejects_unknown_values() {
        let err = parse_schedule("hourly").unwrap_err();
        assert!(err.to_string().contains("unsupported cron schedule"));
    }

    #[test]
    fn daily_time_of_extracts_fixed_hour_minute() {
        // sec min hour dom month dow -> 09:00 every day
        assert_eq!(daily_time_of("0 0 9 * * *").unwrap(), (9, 0));
        assert_eq!(daily_time_of("0 30 17 * * *").unwrap(), (17, 30));
    }

    #[test]
    fn daily_time_of_rejects_non_daily_schedules() {
        // Hourly fires 24x/day, not a single daily time.
        assert!(daily_time_of("0 0 * * * *").is_err());
        // Weekly (only Mondays) is not a fixed daily time either.
        assert!(daily_time_of("0 0 9 * * MON").is_err());
        // Garbage.
        assert!(daily_time_of("not a schedule").is_err());
    }

    #[test]
    fn schedule_from_cron_maps_to_daily_at() {
        assert_eq!(
            schedule_from_cron("0 0 9 * * *").unwrap(),
            Schedule::DailyAt { hour: 9, minute: 0 }
        );
    }

    #[test]
    fn sync_installs_declared_and_prunes_undeclared() {
        let temp = tempfile::TempDir::new().unwrap();
        let agents = temp.path().join("agents");
        let repo = temp.path().join("repo");
        fs::create_dir_all(&repo).unwrap();
        let launchctl = FakeLaunchctl::default();
        let lf_path = PathBuf::from("/bin/lf");

        // First sync: one declared cron installs one launchd job.
        let first = sync_crons(
            &agents,
            "infra",
            &[WaveCronDef {
                flow: "telemetry-daily".to_string(),
                schedule: "0 0 9 * * *".to_string(),
            }],
            &repo,
            &lf_path,
            &launchctl,
        )
        .unwrap();
        assert_eq!(first.installed.len(), 1);
        assert!(first.removed.is_empty());
        assert!(first.skipped.is_empty());
        let installed = &first.installed[0];
        assert_eq!(installed.flow, "telemetry-daily");
        let plist = fs::read_to_string(&installed.path).unwrap();
        assert!(plist.contains("<integer>9</integer>"));

        // Second sync with a different flow: the old job is pruned, the new one installed.
        let second = sync_crons(
            &agents,
            "infra",
            &[WaveCronDef {
                flow: "release-check".to_string(),
                schedule: "0 0 6 * * *".to_string(),
            }],
            &repo,
            &lf_path,
            &launchctl,
        )
        .unwrap();
        assert_eq!(second.installed.len(), 1);
        assert_eq!(second.installed[0].flow, "release-check");
        assert_eq!(second.removed.len(), 1);
        assert_eq!(second.removed[0].flow, "telemetry-daily");

        // Empty declaration prunes everything for the wave.
        let third = sync_crons(&agents, "infra", &[], &repo, &lf_path, &launchctl).unwrap();
        assert!(third.installed.is_empty());
        assert_eq!(third.removed.len(), 1);
        assert!(list_crons(&agents).unwrap().is_empty());
    }

    #[test]
    fn sync_skips_schedules_launchd_cannot_run() {
        let temp = tempfile::TempDir::new().unwrap();
        let agents = temp.path().join("agents");
        let repo = temp.path().join("repo");
        fs::create_dir_all(&repo).unwrap();
        let launchctl = FakeLaunchctl::default();

        let result = sync_crons(
            &agents,
            "infra",
            &[WaveCronDef {
                flow: "hourly-thing".to_string(),
                schedule: "0 0 * * * *".to_string(),
            }],
            &repo,
            &PathBuf::from("/bin/lf"),
            &launchctl,
        )
        .unwrap();
        assert!(result.installed.is_empty());
        assert_eq!(result.skipped.len(), 1);
        assert_eq!(result.skipped[0].flow, "hourly-thing");
        assert!(list_crons(&agents).unwrap().is_empty());
    }
}
