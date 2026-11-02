pub mod clipboard;
pub(crate) mod file_exchange;
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

/// Send SIGTERM to a process by PID. Signals directly: ending a process must
/// launch nothing, since the caller's own executable may already be gone.
pub fn kill_process(pid: u32) {
    // Zero would signal the caller's whole process group.
    let Some(pid) = libc::pid_t::try_from(pid).ok().filter(|pid| *pid > 0) else {
        return;
    };
    // SAFETY: kill only sends a signal; an invalid or exited pid returns ESRCH.
    unsafe {
        libc::kill(pid, libc::SIGTERM);
    }
}
