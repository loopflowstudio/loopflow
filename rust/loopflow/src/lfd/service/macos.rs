use std::fs;
use std::process::Command;

use anyhow::{Context, Result};

use crate::lfd::paths;

const LABEL: &str = "studio.loopflow.lfd";

/// Generate the launchd plist XML for the current binary.
fn generate_plist(lfd_path: &str, log_dir: &str) -> String {
    format!(
        r#"<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
    <key>Label</key>
    <string>{LABEL}</string>
    <key>ProgramArguments</key>
    <array>
        <string>{lfd_path}</string>
        <string>run</string>
    </array>
    <key>RunAtLoad</key>
    <true/>
    <key>KeepAlive</key>
    <dict>
        <key>SuccessfulExit</key>
        <false/>
    </dict>
    <key>ThrottleInterval</key>
    <integer>5</integer>
    <key>ProcessType</key>
    <string>Adaptive</string>
    <key>StandardOutPath</key>
    <string>{log_dir}/lfd.log</string>
    <key>StandardErrorPath</key>
    <string>{log_dir}/lfd.err</string>
    <key>EnvironmentVariables</key>
    <dict>
        <key>LFD_STORAGE</key>
        <string>sqlite</string>
    </dict>
</dict>
</plist>
"#
    )
}

/// Bootstrap domain for the current user (gui/<uid>).
fn bootstrap_domain() -> String {
    let uid = Command::new("id")
        .arg("-u")
        .output()
        .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string())
        .unwrap_or_else(|_| "501".to_string());
    format!("gui/{uid}")
}

pub fn install() -> Result<()> {
    let plist_path = paths::plist_path();
    let log_dir = paths::log_dir();
    let lfd_path = std::env::current_exe().context("failed to determine lfd binary path")?;

    // Create parent directories.
    if let Some(parent) = plist_path.parent() {
        fs::create_dir_all(parent).context("failed to create LaunchAgents directory")?;
    }
    fs::create_dir_all(&log_dir).context("failed to create log directory")?;

    // If already loaded, bootout first so we can update the plist.
    let domain = bootstrap_domain();
    let _ = Command::new("launchctl")
        .args(["bootout", &format!("{domain}/{LABEL}")])
        .output();

    let plist = generate_plist(&lfd_path.to_string_lossy(), &log_dir.to_string_lossy());
    fs::write(&plist_path, &plist).context("failed to write plist")?;

    println!("Installed: {}", plist_path.display());
    println!("Service will start on next login (RunAtLoad=true).");
    println!("To start now: lfd start");

    Ok(())
}

pub fn uninstall() -> Result<()> {
    let plist_path = paths::plist_path();

    if !plist_path.exists() {
        println!("Not installed.");
        return Ok(());
    }

    // Bootout (stops and removes from launchd).
    let domain = bootstrap_domain();
    let _ = Command::new("launchctl")
        .args(["bootout", &format!("{domain}/{LABEL}")])
        .output();

    fs::remove_file(&plist_path).context("failed to remove plist")?;
    println!("Uninstalled: {}", plist_path.display());

    Ok(())
}

pub fn start() -> Result<()> {
    let plist_path = paths::plist_path();
    if !plist_path.exists() {
        anyhow::bail!("Service not installed. Run `lfd install` first.");
    }

    let domain = bootstrap_domain();
    let output = Command::new("launchctl")
        .args(["bootstrap", &domain, &plist_path.to_string_lossy()])
        .output()
        .context("failed to run launchctl bootstrap")?;

    if output.status.success() {
        println!("Started lfd.");
    } else {
        let stderr = String::from_utf8_lossy(&output.stderr);
        // Error 37 = already loaded, which is fine.
        if stderr.contains("37:") || stderr.contains("already loaded") {
            println!("lfd is already running.");
        } else {
            anyhow::bail!("Failed to start lfd: {stderr}");
        }
    }

    Ok(())
}

pub fn stop() -> Result<()> {
    let domain = bootstrap_domain();
    let output = Command::new("launchctl")
        .args(["bootout", &format!("{domain}/{LABEL}")])
        .output()
        .context("failed to run launchctl bootout")?;

    if output.status.success() {
        println!("Stopped lfd.");
    } else {
        let stderr = String::from_utf8_lossy(&output.stderr);
        if stderr.contains("not find") || stderr.contains("36:") {
            println!("lfd is not running.");
        } else {
            anyhow::bail!("Failed to stop lfd: {stderr}");
        }
    }

    Ok(())
}

pub fn status() -> Result<()> {
    let domain = bootstrap_domain();
    let output = Command::new("launchctl")
        .args(["print", &format!("{domain}/{LABEL}")])
        .output()
        .context("failed to run launchctl print")?;

    if output.status.success() {
        let stdout = String::from_utf8_lossy(&output.stdout);
        // Extract PID and state from launchctl print output.
        let mut pid = None;
        let mut state = "unknown";
        for line in stdout.lines() {
            let trimmed = line.trim();
            if trimmed.starts_with("pid = ") {
                pid = trimmed.strip_prefix("pid = ").map(|s| s.trim().to_string());
            }
            if trimmed.starts_with("state = ") {
                state = if trimmed.contains("running") {
                    "running"
                } else {
                    "not running"
                };
            }
        }
        match pid {
            Some(p) => println!("lfd is running (PID {p})."),
            None => println!("lfd is {state}."),
        }
    } else {
        println!("lfd is not running.");
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn plist_contains_label() {
        let plist = generate_plist("/usr/local/bin/lfd", "/var/log/lfd");
        assert!(plist.contains(LABEL));
    }

    #[test]
    fn plist_contains_binary_path() {
        let plist = generate_plist("/opt/homebrew/bin/lfd", "/var/log/lfd");
        assert!(plist.contains("/opt/homebrew/bin/lfd"));
    }

    #[test]
    fn plist_contains_run_subcommand() {
        let plist = generate_plist("/usr/local/bin/lfd", "/var/log/lfd");
        assert!(plist.contains("<string>run</string>"));
    }

    #[test]
    fn plist_has_keep_alive() {
        let plist = generate_plist("/usr/local/bin/lfd", "/var/log/lfd");
        assert!(plist.contains("KeepAlive"));
        assert!(plist.contains("SuccessfulExit"));
    }

    #[test]
    fn plist_has_throttle_interval() {
        let plist = generate_plist("/usr/local/bin/lfd", "/var/log/lfd");
        assert!(plist.contains("ThrottleInterval"));
    }

    #[test]
    fn plist_has_process_type_adaptive() {
        let plist = generate_plist("/usr/local/bin/lfd", "/var/log/lfd");
        assert!(plist.contains("Adaptive"));
    }

    #[test]
    fn plist_has_log_paths() {
        let plist = generate_plist("/usr/local/bin/lfd", "/Users/jack/Library/Logs/lfd");
        assert!(plist.contains("/Users/jack/Library/Logs/lfd/lfd.log"));
        assert!(plist.contains("/Users/jack/Library/Logs/lfd/lfd.err"));
    }

    #[test]
    fn plist_is_valid_xml() {
        let plist = generate_plist("/usr/local/bin/lfd", "/var/log/lfd");
        assert!(plist.starts_with("<?xml"));
        assert!(plist.contains("</plist>"));
    }
}
