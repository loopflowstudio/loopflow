use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

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
    Daily,
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
        "daily" => Ok(Schedule::Daily),
        _ => Err(OpsError::Parse(format!(
            "unsupported cron schedule: {value}"
        ))),
    }
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
        Schedule::Daily => {
            "    <key>StartCalendarInterval</key>\n    <dict>\n        <key>Hour</key>\n        <integer>3</integer>\n        <key>Minute</key>\n        <integer>0</integer>\n    </dict>"
        }
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
            wave: "memory".to_string(),
            flow: "export-memory".to_string(),
            schedule: Schedule::Daily,
            working_directory: temp.path().join("repo"),
            lf_path: PathBuf::from("/bin/lf"),
        };
        fs::create_dir_all(&spec.working_directory).unwrap();

        let installed = add_cron(temp.path(), &spec, &launchctl).unwrap();
        assert_eq!(installed.label, "loopflow.cron.memory.export-memory");
        assert!(installed.path.exists());
        assert!(spec.working_directory.join(".lf/logs").is_dir());

        let content = fs::read_to_string(&installed.path).unwrap();
        assert!(content.contains("<string>/bin/lf</string>"));
        assert!(content.contains("<string>export-memory</string>"));
        assert!(content.contains("<string>--wave</string>"));
        assert!(content.contains("<string>memory</string>"));
        assert!(content.contains("<key>StartCalendarInterval</key>"));
        assert!(content.contains(&format!(
            "<string>{}</string>",
            spec.working_directory.display()
        )));

        let crons = list_crons(temp.path()).unwrap();
        assert_eq!(crons, vec![installed.clone()]);

        let removed = remove_cron(temp.path(), "memory", "export-memory", &launchctl).unwrap();
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
}
