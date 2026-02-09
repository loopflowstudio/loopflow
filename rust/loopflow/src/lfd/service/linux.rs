use std::fs;
use std::process::Command;

use anyhow::{Context, Result};

use crate::lfd::paths;

/// Generate the systemd user unit file for the current binary.
fn generate_unit(lfd_path: &str) -> String {
    format!(
        r#"[Unit]
Description=Loopflow Daemon
After=network.target

[Service]
Type=simple
ExecStart={lfd_path} run
Restart=on-failure
RestartSec=1s
RestartSteps=5
RestartMaxDelaySec=60s
Environment=LFD_STORAGE=sqlite

[Install]
WantedBy=default.target
"#
    )
}

pub fn install() -> Result<()> {
    let service_path = paths::service_path();
    let lfd_path = std::env::current_exe().context("failed to determine lfd binary path")?;

    if let Some(parent) = service_path.parent() {
        fs::create_dir_all(parent).context("failed to create systemd user directory")?;
    }

    let unit = generate_unit(&lfd_path.to_string_lossy());
    fs::write(&service_path, &unit).context("failed to write service unit")?;

    // Reload systemd to pick up the new unit.
    let _ = Command::new("systemctl")
        .args(["--user", "daemon-reload"])
        .status();

    // Enable so it starts on login.
    let _ = Command::new("systemctl")
        .args(["--user", "enable", "lfd.service"])
        .status();

    println!("Installed: {}", service_path.display());
    println!("Service enabled (starts on login).");
    println!("To start now: lfd start");
    println!();
    println!("Note: to keep lfd running after logout on a headless machine:");
    println!("  loginctl enable-linger $USER");

    Ok(())
}

pub fn uninstall() -> Result<()> {
    let service_path = paths::service_path();

    if !service_path.exists() {
        println!("Not installed.");
        return Ok(());
    }

    // Disable and stop.
    let _ = Command::new("systemctl")
        .args(["--user", "disable", "--now", "lfd.service"])
        .status();

    fs::remove_file(&service_path).context("failed to remove service unit")?;

    let _ = Command::new("systemctl")
        .args(["--user", "daemon-reload"])
        .status();

    println!("Uninstalled: {}", service_path.display());

    Ok(())
}

pub fn start() -> Result<()> {
    let service_path = paths::service_path();
    if !service_path.exists() {
        anyhow::bail!("Service not installed. Run `lfd install` first.");
    }

    let status = Command::new("systemctl")
        .args(["--user", "start", "lfd.service"])
        .status()
        .context("failed to run systemctl start")?;

    if status.success() {
        println!("Started lfd.");
    } else {
        anyhow::bail!("Failed to start lfd. Check: systemctl --user status lfd.service");
    }

    Ok(())
}

pub fn stop() -> Result<()> {
    let status = Command::new("systemctl")
        .args(["--user", "stop", "lfd.service"])
        .status()
        .context("failed to run systemctl stop")?;

    if status.success() {
        println!("Stopped lfd.");
    } else {
        anyhow::bail!("Failed to stop lfd. Check: systemctl --user status lfd.service");
    }

    Ok(())
}

pub fn status() -> Result<()> {
    let output = Command::new("systemctl")
        .args(["--user", "status", "lfd.service"])
        .output()
        .context("failed to run systemctl status")?;

    let stdout = String::from_utf8_lossy(&output.stdout);
    if stdout.is_empty() {
        println!("lfd is not installed or not running.");
    } else {
        print!("{stdout}");
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn generate_unit_produces_valid_config() {
        let unit = generate_unit("/opt/custom/bin/lfd");

        // Binary path and subcommand.
        assert!(unit.contains("ExecStart=/opt/custom/bin/lfd run"));

        // Restart behavior with exponential backoff.
        assert!(unit.contains("Restart=on-failure"));
        assert!(unit.contains("RestartSec=1s"));
        assert!(unit.contains("RestartSteps=5"));
        assert!(unit.contains("RestartMaxDelaySec=60s"));

        // Metadata and install target.
        assert!(unit.contains("Description=Loopflow Daemon"));
        assert!(unit.contains("WantedBy=default.target"));
    }
}
