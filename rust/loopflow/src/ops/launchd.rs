//! The launchd plumbing `lf ci watch --install` uses to keep its watcher alive.

use std::fs::OpenOptions;
use std::io::Write;
#[cfg(unix)]
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::process::Command;

use crate::ops::error::{OpsError, OpsResult};

pub trait Launchctl {
    fn load(&self, path: &Path) -> OpsResult<()>;
    fn unload(&self, path: &Path) -> OpsResult<()>;
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
}

pub fn default_launch_agents_dir() -> OpsResult<PathBuf> {
    let home =
        dirs::home_dir().ok_or_else(|| OpsError::Message("no home directory".to_string()))?;
    Ok(home.join("Library/LaunchAgents"))
}

pub fn resolve_lf_path() -> OpsResult<PathBuf> {
    // Background services follow future promotions; Sessions pin their own runtime.
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

pub(crate) fn write_private_file(path: &Path, bytes: &[u8]) -> OpsResult<()> {
    let mut file = OpenOptions::new()
        .create(true)
        .truncate(true)
        .write(true)
        .open(path)?;
    #[cfg(unix)]
    {
        file.set_permissions(std::fs::Permissions::from_mode(0o600))?;
    }
    file.write_all(bytes)?;
    file.sync_all()?;
    Ok(())
}

pub(crate) fn xml_escape(value: &str) -> String {
    value
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&apos;")
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
