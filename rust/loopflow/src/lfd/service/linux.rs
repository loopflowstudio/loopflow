use std::path::PathBuf;
use std::process::Command;

// Linux systemd user service management.
//
// Installs a systemd user unit that starts lfd on login, restarts on crash,
// and logs through journald.

const UNIT_NAME: &str = "lfd.service";

fn unit_path() -> Result<PathBuf, Box<dyn std::error::Error>> {
    let config = dirs::config_dir().ok_or("no config directory")?;
    Ok(config.join("systemd/user").join(UNIT_NAME))
}

pub fn install() -> Result<(), Box<dyn std::error::Error>> {
    let lfd_path = std::env::current_exe()?
        .canonicalize()?
        .to_string_lossy()
        .to_string();
    let path_env = std::env::var("PATH").unwrap_or_default();

    let unit_path = unit_path()?;
    if let Some(parent) = unit_path.parent() {
        std::fs::create_dir_all(parent)?;
    }

    let content = format!(
        r#"[Unit]
Description=Loopflow Daemon
After=network.target

[Service]
Type=simple
ExecStart={lfd_path} serve
Restart=on-failure
RestartSec=5
Environment=PATH={path_env}
Environment=LFD_STORAGE=sqlite

[Install]
WantedBy=default.target
"#
    );

    std::fs::write(&unit_path, &content)?;
    println!("Installed {}", unit_path.display());

    // Reload systemd so it picks up the new unit.
    let _ = Command::new("systemctl")
        .args(["--user", "daemon-reload"])
        .status();

    // Enable the service to start on login.
    let status = Command::new("systemctl")
        .args(["--user", "enable", "lfd"])
        .status()?;
    if status.success() {
        println!("Enabled lfd (starts on login)");
    }

    // Start immediately.
    let status = Command::new("systemctl")
        .args(["--user", "start", "lfd"])
        .status()?;
    if status.success() {
        println!("lfd service started");
    } else {
        eprintln!("Warning: systemctl start failed (exit {})", status);
    }

    Ok(())
}

pub fn uninstall() -> Result<(), Box<dyn std::error::Error>> {
    let unit_path = unit_path()?;
    if !unit_path.exists() {
        println!("Not installed");
        return Ok(());
    }

    // Stop and disable in one shot.
    let _ = Command::new("systemctl")
        .args(["--user", "disable", "--now", "lfd"])
        .status();

    std::fs::remove_file(&unit_path)?;

    let _ = Command::new("systemctl")
        .args(["--user", "daemon-reload"])
        .status();

    println!("Uninstalled lfd");

    Ok(())
}

pub fn start() -> Result<(), Box<dyn std::error::Error>> {
    let unit_path = unit_path()?;
    if !unit_path.exists() {
        return Err("lfd is not installed — run `lfd install` first".into());
    }

    let status = Command::new("systemctl")
        .args(["--user", "start", "lfd"])
        .status()?;

    if status.success() {
        println!("Started lfd");
    } else {
        eprintln!("Failed to start lfd (exit {})", status);
    }

    Ok(())
}

pub fn stop() -> Result<(), Box<dyn std::error::Error>> {
    let unit_path = unit_path()?;
    if !unit_path.exists() {
        return Err("lfd is not installed".into());
    }

    let status = Command::new("systemctl")
        .args(["--user", "stop", "lfd"])
        .status()?;

    if status.success() {
        println!("Stopped lfd");
    } else {
        eprintln!("Failed to stop lfd (exit {})", status);
    }

    Ok(())
}

pub fn status() -> Result<(), Box<dyn std::error::Error>> {
    let output = Command::new("systemctl")
        .args(["--user", "status", "lfd"])
        .output()?;

    let stdout = String::from_utf8_lossy(&output.stdout);
    if stdout.is_empty() {
        println!("lfd is not installed");
    } else {
        print!("{stdout}");
    }

    Ok(())
}
