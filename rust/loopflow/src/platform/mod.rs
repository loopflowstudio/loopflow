pub mod clipboard;
#[cfg(target_os = "macos")]
pub(crate) mod fs_events;

use std::process::Command;

/// Open a URL in the default browser and report whether the platform opener succeeded.
pub fn open_url_checked(url: &str) -> std::io::Result<()> {
    let cmd = if cfg!(target_os = "macos") {
        "open"
    } else {
        "xdg-open"
    };
    let status = Command::new(cmd).arg(url).status()?;
    if status.success() {
        Ok(())
    } else {
        Err(std::io::Error::other(format!(
            "{cmd} exited with status {status}"
        )))
    }
}

/// Send SIGTERM to a process by PID.
pub fn kill_process(pid: u32) {
    let _ = Command::new("kill")
        .arg("-TERM")
        .arg(pid.to_string())
        .status();
}
