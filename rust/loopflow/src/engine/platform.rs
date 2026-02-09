use std::process::Command;

/// Open a URL in the default browser.
pub fn open_url(url: &str) {
    let cmd = if cfg!(target_os = "macos") {
        "open"
    } else {
        "xdg-open"
    };
    let _ = Command::new(cmd).arg(url).status();
}

/// Send SIGTERM to a process by PID.
pub fn kill_process(pid: u32) {
    let _ = Command::new("kill")
        .arg("-TERM")
        .arg(pid.to_string())
        .status();
}
