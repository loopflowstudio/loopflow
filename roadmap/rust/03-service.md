# 03: Service Integration

Run lfd as a user service via launchd (macOS) or systemd (Linux).

## Context

lfd runs as a foreground process. Users must manually start it. For waves to work continuously (loop, watch, cron), lfd needs to run persistently.

## Goal

1. `lfd install` installs lfd as a user service
2. Service starts on login
3. Service restarts on crash
4. Logs go to appropriate system location
5. `lfd uninstall` removes the service

## Architecture

```
┌─────────────────────────────────────────────────────────────┐
│ macOS                                                       │
│                                                             │
│  ~/Library/LaunchAgents/                                    │
│    └── studio.loopflow.lfd.plist                           │
│                                                             │
│  ~/Library/Logs/lfd/                                        │
│    ├── lfd.log                                              │
│    └── lfd.err                                              │
│                                                             │
│  ~/Library/Application Support/lf/                          │
│    └── lfd.db (SQLite)                                      │
│                                                             │
└─────────────────────────────────────────────────────────────┘

┌─────────────────────────────────────────────────────────────┐
│ Linux                                                       │
│                                                             │
│  ~/.config/systemd/user/                                    │
│    └── lfd.service                                          │
│                                                             │
│  journald (systemctl --user status lfd)                     │
│                                                             │
│  ~/.local/share/lf/                                         │
│    └── lfd.db (SQLite)                                      │
│                                                             │
└─────────────────────────────────────────────────────────────┘
```

## Implementation

### CLI Commands

```rust
// rust/lfd/src/main.rs

#[derive(Subcommand)]
enum Commands {
    /// Run the daemon (foreground)
    Run,
    /// Install as system service
    Install,
    /// Uninstall system service
    Uninstall,
    /// Start the service
    Start,
    /// Stop the service
    Stop,
    /// Show service status
    Status,
    /// Run database migrations
    Migrate,
}

fn main() -> Result<()> {
    let cli = Cli::parse();

    match cli.command {
        Commands::Run => run_daemon(),
        Commands::Install => install_service(),
        Commands::Uninstall => uninstall_service(),
        Commands::Start => start_service(),
        Commands::Stop => stop_service(),
        Commands::Status => show_status(),
        Commands::Migrate => run_migrations(),
    }
}
```

### macOS launchd

```rust
// rust/lfd/src/service/macos.rs

const PLIST_NAME: &str = "studio.loopflow.lfd.plist";

pub fn install() -> Result<()> {
    let plist_dir = dirs::home_dir()
        .ok_or_else(|| anyhow!("no home dir"))?
        .join("Library/LaunchAgents");

    fs::create_dir_all(&plist_dir)?;

    let lfd_path = std::env::current_exe()?;
    let log_dir = dirs::home_dir().unwrap().join("Library/Logs/lfd");
    fs::create_dir_all(&log_dir)?;

    let plist = format!(r#"<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
    <key>Label</key>
    <string>studio.loopflow.lfd</string>
    <key>ProgramArguments</key>
    <array>
        <string>{}</string>
        <string>run</string>
    </array>
    <key>RunAtLoad</key>
    <true/>
    <key>KeepAlive</key>
    <dict>
        <key>SuccessfulExit</key>
        <false/>
    </dict>
    <key>StandardOutPath</key>
    <string>{}/lfd.log</string>
    <key>StandardErrorPath</key>
    <string>{}/lfd.err</string>
    <key>EnvironmentVariables</key>
    <dict>
        <key>LFD_STORAGE</key>
        <string>sqlite</string>
    </dict>
</dict>
</plist>"#,
        lfd_path.display(),
        log_dir.display(),
        log_dir.display()
    );

    let plist_path = plist_dir.join(PLIST_NAME);
    fs::write(&plist_path, plist)?;

    println!("Installed: {}", plist_path.display());
    println!("Run: launchctl load {}", plist_path.display());

    Ok(())
}

pub fn uninstall() -> Result<()> {
    let plist_path = dirs::home_dir()
        .ok_or_else(|| anyhow!("no home dir"))?
        .join("Library/LaunchAgents")
        .join(PLIST_NAME);

    if plist_path.exists() {
        // Unload first
        let _ = Command::new("launchctl")
            .args(["unload", &plist_path.to_string_lossy()])
            .status();

        fs::remove_file(&plist_path)?;
        println!("Uninstalled: {}", plist_path.display());
    } else {
        println!("Not installed");
    }

    Ok(())
}

pub fn start() -> Result<()> {
    let plist_path = dirs::home_dir()
        .ok_or_else(|| anyhow!("no home dir"))?
        .join("Library/LaunchAgents")
        .join(PLIST_NAME);

    let status = Command::new("launchctl")
        .args(["load", &plist_path.to_string_lossy()])
        .status()?;

    if status.success() {
        println!("Started lfd");
    } else {
        println!("Failed to start lfd");
    }

    Ok(())
}

pub fn stop() -> Result<()> {
    let plist_path = dirs::home_dir()
        .ok_or_else(|| anyhow!("no home dir"))?
        .join("Library/LaunchAgents")
        .join(PLIST_NAME);

    let status = Command::new("launchctl")
        .args(["unload", &plist_path.to_string_lossy()])
        .status()?;

    if status.success() {
        println!("Stopped lfd");
    } else {
        println!("Failed to stop lfd");
    }

    Ok(())
}

pub fn status() -> Result<()> {
    let output = Command::new("launchctl")
        .args(["list", "studio.loopflow.lfd"])
        .output()?;

    if output.status.success() {
        println!("lfd is running");
        println!("{}", String::from_utf8_lossy(&output.stdout));
    } else {
        println!("lfd is not running");
    }

    Ok(())
}
```

### Linux systemd

```rust
// rust/lfd/src/service/linux.rs

const SERVICE_NAME: &str = "lfd.service";

pub fn install() -> Result<()> {
    let service_dir = dirs::config_dir()
        .ok_or_else(|| anyhow!("no config dir"))?
        .join("systemd/user");

    fs::create_dir_all(&service_dir)?;

    let lfd_path = std::env::current_exe()?;

    let unit = format!(r#"[Unit]
Description=Loopflow Daemon
After=network.target

[Service]
Type=simple
ExecStart={} run
Restart=on-failure
RestartSec=5
Environment=LFD_STORAGE=sqlite

[Install]
WantedBy=default.target
"#,
        lfd_path.display()
    );

    let service_path = service_dir.join(SERVICE_NAME);
    fs::write(&service_path, unit)?;

    // Reload systemd
    Command::new("systemctl")
        .args(["--user", "daemon-reload"])
        .status()?;

    // Enable service
    Command::new("systemctl")
        .args(["--user", "enable", "lfd"])
        .status()?;

    println!("Installed: {}", service_path.display());
    println!("Run: systemctl --user start lfd");

    Ok(())
}

pub fn uninstall() -> Result<()> {
    // Disable and stop
    let _ = Command::new("systemctl")
        .args(["--user", "disable", "--now", "lfd"])
        .status();

    let service_path = dirs::config_dir()
        .ok_or_else(|| anyhow!("no config dir"))?
        .join("systemd/user")
        .join(SERVICE_NAME);

    if service_path.exists() {
        fs::remove_file(&service_path)?;

        Command::new("systemctl")
            .args(["--user", "daemon-reload"])
            .status()?;

        println!("Uninstalled");
    } else {
        println!("Not installed");
    }

    Ok(())
}

pub fn start() -> Result<()> {
    let status = Command::new("systemctl")
        .args(["--user", "start", "lfd"])
        .status()?;

    if status.success() {
        println!("Started lfd");
    } else {
        println!("Failed to start lfd");
    }

    Ok(())
}

pub fn stop() -> Result<()> {
    let status = Command::new("systemctl")
        .args(["--user", "stop", "lfd"])
        .status()?;

    if status.success() {
        println!("Stopped lfd");
    } else {
        println!("Failed to stop lfd");
    }

    Ok(())
}

pub fn status() -> Result<()> {
    let output = Command::new("systemctl")
        .args(["--user", "status", "lfd"])
        .output()?;

    println!("{}", String::from_utf8_lossy(&output.stdout));

    Ok(())
}
```

### Platform Dispatch

```rust
// rust/lfd/src/service/mod.rs

#[cfg(target_os = "macos")]
mod macos;
#[cfg(target_os = "linux")]
mod linux;

pub fn install_service() -> Result<()> {
    #[cfg(target_os = "macos")]
    return macos::install();

    #[cfg(target_os = "linux")]
    return linux::install();

    #[cfg(not(any(target_os = "macos", target_os = "linux")))]
    return Err(anyhow!("service installation not supported on this platform"));
}

pub fn uninstall_service() -> Result<()> {
    #[cfg(target_os = "macos")]
    return macos::uninstall();

    #[cfg(target_os = "linux")]
    return linux::uninstall();

    #[cfg(not(any(target_os = "macos", target_os = "linux")))]
    return Err(anyhow!("service not supported on this platform"));
}

// ... similar for start, stop, status
```

### Data Directories

```rust
// rust/lfd/src/paths.rs

pub fn data_dir() -> PathBuf {
    #[cfg(target_os = "macos")]
    {
        dirs::data_local_dir()
            .unwrap_or_else(|| PathBuf::from("/tmp"))
            .join("lf")
    }

    #[cfg(target_os = "linux")]
    {
        dirs::data_dir()
            .unwrap_or_else(|| PathBuf::from("/tmp"))
            .join("lf")
    }

    #[cfg(target_os = "windows")]
    {
        dirs::data_local_dir()
            .unwrap_or_else(|| PathBuf::from("C:\\temp"))
            .join("lf")
    }
}

pub fn db_path() -> PathBuf {
    data_dir().join("lfd.db")
}

pub fn socket_path() -> PathBuf {
    #[cfg(unix)]
    {
        dirs::runtime_dir()
            .unwrap_or_else(|| PathBuf::from("/tmp"))
            .join("lfd.sock")
    }

    #[cfg(windows)]
    {
        // Windows uses TCP
        PathBuf::new()
    }
}

pub fn log_dir() -> PathBuf {
    #[cfg(target_os = "macos")]
    {
        dirs::home_dir()
            .unwrap()
            .join("Library/Logs/lfd")
    }

    #[cfg(target_os = "linux")]
    {
        // systemd handles logging via journald
        dirs::cache_dir()
            .unwrap_or_else(|| PathBuf::from("/tmp"))
            .join("lfd/logs")
    }
}
```

## Usage

```bash
# Install as service
lfd install

# Start the service
lfd start
# Or: launchctl load ~/Library/LaunchAgents/studio.loopflow.lfd.plist
# Or: systemctl --user start lfd

# Check status
lfd status

# View logs (macOS)
tail -f ~/Library/Logs/lfd/lfd.log

# View logs (Linux)
journalctl --user -u lfd -f

# Stop the service
lfd stop

# Uninstall
lfd uninstall
```

## Done When

- [ ] `lfd install` creates launchd plist on macOS
- [ ] `lfd install` creates systemd unit on Linux
- [ ] Service starts on user login
- [ ] Service restarts on crash (with backoff)
- [ ] `lfd start` / `lfd stop` work
- [ ] `lfd status` shows running state
- [ ] `lfd uninstall` removes service cleanly
- [ ] Logs go to appropriate location
- [ ] SQLite database created in data directory
- [ ] Works with Homebrew-installed lfd

## Dependencies

- Requires: Phase 1 lfd working (it does)
- Enables: Persistent wave execution (loop, watch, cron)
