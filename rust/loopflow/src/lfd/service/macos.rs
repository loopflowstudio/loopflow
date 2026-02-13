use std::path::PathBuf;
use std::process::Command;

/// macOS launchd service management.
///
/// Installs a LaunchAgent plist that starts lfd on login, keeps it alive,
/// and logs to `~/.lf/logs/lfd.log`.
const LABEL: &str = "com.loopflow.lfd";

fn plist_path() -> Result<PathBuf, Box<dyn std::error::Error>> {
    let home = dirs::home_dir().ok_or("no home directory")?;
    Ok(home
        .join("Library/LaunchAgents")
        .join(format!("{LABEL}.plist")))
}

fn log_dir() -> Result<PathBuf, Box<dyn std::error::Error>> {
    let home = dirs::home_dir().ok_or("no home directory")?;
    Ok(home.join(".lf/logs"))
}

pub fn install() -> Result<(), Box<dyn std::error::Error>> {
    let lfd_path = std::env::current_exe()?
        .canonicalize()?
        .to_string_lossy()
        .to_string();
    let path_env = std::env::var("PATH").unwrap_or_default();
    let log_dir = log_dir()?;
    std::fs::create_dir_all(&log_dir)?;

    let plist_path = plist_path()?;
    if let Some(parent) = plist_path.parent() {
        std::fs::create_dir_all(parent)?;
    }

    let content = format!(
        r#"<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
    <key>Label</key>
    <string>{LABEL}</string>
    <key>ProgramArguments</key>
    <array>
        <string>{lfd_path}</string>
        <string>serve</string>
    </array>
    <key>RunAtLoad</key>
    <true/>
    <key>KeepAlive</key>
    <true/>
    <key>ThrottleInterval</key>
    <integer>10</integer>
    <key>ExitTimeOut</key>
    <integer>30</integer>
    <key>StandardOutPath</key>
    <string>{log_dir}/lfd.log</string>
    <key>StandardErrorPath</key>
    <string>{log_dir}/lfd.log</string>
    <key>EnvironmentVariables</key>
    <dict>
        <key>PATH</key>
        <string>{path_env}</string>
    </dict>
</dict>
</plist>
"#,
        log_dir = log_dir.display()
    );

    // Unload existing service before overwriting.
    let plist_str = plist_path.to_string_lossy().to_string();
    let _ = Command::new("launchctl")
        .args(["unload", &plist_str])
        .output();

    std::fs::write(&plist_path, &content)?;
    println!("Installed {}", plist_path.display());

    // Load the service.
    let status = Command::new("launchctl")
        .args(["load", &plist_str])
        .status()?;
    if status.success() {
        println!("lfd service loaded");
    } else {
        eprintln!("Warning: launchctl load failed (exit {})", status);
    }

    Ok(())
}

pub fn uninstall() -> Result<(), Box<dyn std::error::Error>> {
    let plist_path = plist_path()?;
    if !plist_path.exists() {
        println!("Not installed");
        return Ok(());
    }

    let plist_str = plist_path.to_string_lossy().to_string();
    let _ = Command::new("launchctl")
        .args(["unload", &plist_str])
        .status();

    std::fs::remove_file(&plist_path)?;
    println!("Uninstalled {}", plist_path.display());

    Ok(())
}

pub fn start() -> Result<(), Box<dyn std::error::Error>> {
    let plist_path = plist_path()?;
    if !plist_path.exists() {
        return Err("lfd is not installed — run `lfd install` first".into());
    }

    let plist_str = plist_path.to_string_lossy().to_string();
    let status = Command::new("launchctl")
        .args(["load", &plist_str])
        .status()?;

    if status.success() {
        println!("Started lfd");
    } else {
        eprintln!("Failed to start lfd (exit {})", status);
    }

    Ok(())
}

pub fn stop() -> Result<(), Box<dyn std::error::Error>> {
    let plist_path = plist_path()?;
    if !plist_path.exists() {
        return Err("lfd is not installed".into());
    }

    let plist_str = plist_path.to_string_lossy().to_string();
    let status = Command::new("launchctl")
        .args(["unload", &plist_str])
        .status()?;

    if status.success() {
        println!("Stopped lfd");
    } else {
        eprintln!("Failed to stop lfd (exit {})", status);
    }

    Ok(())
}

pub fn status() -> Result<(), Box<dyn std::error::Error>> {
    let output = Command::new("launchctl").args(["list", LABEL]).output()?;

    if output.status.success() {
        let stdout = String::from_utf8_lossy(&output.stdout);
        // launchctl list <label> prints a table with PID, status, label.
        // A non-zero PID means the process is running.
        let running = stdout.lines().any(|line| {
            let parts: Vec<&str> = line.split_whitespace().collect();
            // PID column is first; "-" means not running.
            parts
                .first()
                .is_some_and(|pid| *pid != "-" && pid.parse::<u32>().is_ok())
        });

        if running {
            println!("lfd is running");
        } else {
            println!("lfd is loaded but not running");
        }
        print!("{stdout}");
    } else {
        println!("lfd is not loaded");
    }

    Ok(())
}
