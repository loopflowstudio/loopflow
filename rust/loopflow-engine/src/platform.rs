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
