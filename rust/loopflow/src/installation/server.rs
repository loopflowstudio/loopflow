//! The selected lf entry owns service setup. Development Machines use distinct
//! supervisor identities; neither registration nor removal changes Work.
use anyhow::{Context, Result};
use fs2::FileExt;
use sha2::{Digest, Sha256};
use std::fs;
use std::os::unix::fs::OpenOptionsExt;
use std::path::Path;
use std::process::Command;
use std::time::{Duration, Instant};

pub fn ensure() -> Result<crate::server::client::Status> {
    let lock = fs::OpenOptions::new()
        .create(true)
        .truncate(false)
        .read(true)
        .write(true)
        .mode(0o600)
        .open(crate::server::socket_path()?.with_extension("install-lock"))?;
    lock.lock_exclusive()?;
    let before = crate::server::client::status()?;
    if before.ready {
        return Ok(before);
    }
    let home = crate::store::lf_home_dir().canonicalize()?;
    let executable = crate::os_process::resolve_lf_binary()?;
    register(&home, &executable, before.listening)?;
    let deadline = Instant::now() + Duration::from_secs(10);
    loop {
        let status = crate::server::client::status()?;
        if status.ready {
            return Ok(status);
        }
        anyhow::ensure!(
            Instant::now() < deadline,
            "Loopflow server unavailable: {}",
            status.error.as_deref().unwrap_or("not ready")
        );
        std::thread::sleep(Duration::from_millis(100));
    }
}

fn label(home: &Path, namespace: &str) -> String {
    let digest = hex::encode(Sha256::digest(home.as_os_str().as_encoded_bytes()));
    format!("com.loopflow.{namespace}.{}", &digest[..16])
}

#[cfg(target_os = "macos")]
fn register(home: &Path, executable: &Path, replace: bool) -> Result<()> {
    // A signed app owns its SMAppService registration. An explicit development
    // Machine has no bundle and uses a separate launchctl user job.
    if !crate::store::custom_home_selected() {
        return register_bundled(replace);
    }
    let legacy_label = label(home, "machine");
    let label = label(home, "server");
    // SAFETY: geteuid has no preconditions.
    let domain = format!("gui/{}", unsafe { libc::geteuid() });
    let disabled = Command::new("/bin/launchctl")
        .args(["print-disabled", &domain])
        .output()?;
    anyhow::ensure!(
        disabled.status.success(),
        "launchd user domain is unavailable"
    );
    let text = String::from_utf8_lossy(&disabled.stdout);
    anyhow::ensure!(
        !text
            .lines()
            .any(|line| (line.contains(&format!("\"{label}\""))
                || line.contains(&format!("\"{legacy_label}\"")))
                && line
                    .split_once("=>")
                    .is_some_and(|(_, value)| matches!(value.trim(), "disabled" | "true"))),
        "Loopflow server is disabled in Background Items; enable it in System Settings"
    );
    let legacy_target = format!("{domain}/{legacy_label}");
    if Command::new("/bin/launchctl")
        .args(["print", &legacy_target])
        .output()?
        .status
        .success()
    {
        checked(Command::new("/bin/launchctl").args(["bootout", &legacy_target]))?;
    }
    let legacy_plist = home.join("machine-service.plist");
    if legacy_plist.exists() {
        fs::remove_file(legacy_plist)?;
    }
    let path = home.join("server.plist");
    let escape = |value: &str| {
        value
            .replace('&', "&amp;")
            .replace('<', "&lt;")
            .replace('>', "&gt;")
            .replace('"', "&quot;")
    };
    let log = home.join("logs/server.log");
    fs::create_dir_all(log.parent().expect("log has a parent"))?;
    let xml = format!(
        r#"<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0"><dict>
<key>Label</key><string>{label}</string>
<key>ProgramArguments</key><array><string>{executable}</string><string>server</string><string>run</string></array>
<key>EnvironmentVariables</key><dict><key>LF_HOME</key><string>{home}</string><key>LF_BIN</key><string>{executable}</string></dict>
<key>RunAtLoad</key><true/><key>KeepAlive</key><true/>
<key>AbandonProcessGroup</key><true/>
<key>StandardOutPath</key><string>{log}</string><key>StandardErrorPath</key><string>{log}</string>
</dict></plist>"#,
        executable = escape(&executable.display().to_string()),
        home = escape(&home.display().to_string()),
        log = escape(&log.display().to_string())
    );
    fs::write(&path, xml)?;
    let target = format!("{domain}/{label}");
    if Command::new("/bin/launchctl")
        .args(["print", &target])
        .output()?
        .status
        .success()
    {
        // Do not kill an active service on another ensure call.
        let mut command = Command::new("/bin/launchctl");
        command.arg("kickstart");
        if replace {
            command.arg("-k");
        }
        checked(command.arg(&target))?;
    } else {
        checked(
            Command::new("/bin/launchctl")
                .arg("bootstrap")
                .arg(&domain)
                .arg(path),
        )?;
    }
    Ok(())
}

#[cfg(target_os = "macos")]
fn register_bundled(replace: bool) -> Result<()> {
    use std::ffi::CStr;
    unsafe extern "C" {
        fn lf_server_register(
            plist_name: *const std::ffi::c_char,
            legacy_plist: *const std::ffi::c_char,
            replace: bool,
            error: *mut std::ffi::c_char,
            capacity: usize,
        ) -> i32;
    }
    let mut error = [0 as std::ffi::c_char; 1024];
    // SAFETY: the native function writes at most capacity bytes and NUL terminates.
    let result = unsafe {
        lf_server_register(
            c"com.loopflow.server.plist".as_ptr(),
            c"com.loopflow.machine.plist".as_ptr(),
            replace,
            error.as_mut_ptr(),
            error.len(),
        )
    };
    // SAFETY: the buffer is initialized to NUL and the native function preserves termination.
    let message = unsafe { CStr::from_ptr(error.as_ptr()) }.to_string_lossy();
    anyhow::ensure!(result == 0, "{message}");
    Ok(())
}

#[cfg(target_os = "linux")]
fn register(home: &Path, executable: &Path, replace: bool) -> Result<()> {
    let directory = crate::installation::account_home()?.join(".config/systemd/user");
    fs::create_dir_all(&directory)?;
    let unit = format!("{}.service", label(home, "server"));
    let legacy = format!("{}.service", label(home, "machine"));
    // An existing disabled or masked unit is a person's choice, not first use.
    for name in [&unit, &legacy] {
        if directory.join(name).symlink_metadata().is_ok() {
            let enabled = Command::new("systemctl")
                .args(["--user", "is-enabled", name])
                .output()?;
            anyhow::ensure!(
                enabled.status.success(),
                "Loopflow server is disabled; enable {name} with systemctl --user"
            );
        }
    }
    if directory.join(&legacy).exists() {
        checked(Command::new("systemctl").args(["--user", "disable", "--now", &legacy]))?;
        fs::remove_file(directory.join(&legacy))?;
    }
    let quote = |value: &str| {
        format!(
            "\"{}\"",
            value
                .replace('\\', "\\\\")
                .replace('"', "\\\"")
                .replace('\n', "\\n")
                .replace('\r', "\\r")
                .replace('%', "%%")
        )
    };
    let executable_argument = quote(&executable.display().to_string().replace('$', "$$"));
    let home_environment = quote(&format!("LF_HOME={}", home.display()));
    let binary_environment = quote(&format!("LF_BIN={}", executable.display()));
    fs::write(directory.join(&unit), format!("[Unit]\nDescription=Loopflow server\n[Service]\nExecStart={executable_argument} server run\nEnvironment={home_environment}\nEnvironment={binary_environment}\nRestart=on-failure\nKillMode=control-group\n[Install]\nWantedBy=default.target\n"))?;
    checked(Command::new("systemctl").args(["--user", "daemon-reload"]))?;
    checked(Command::new("systemctl").args(["--user", "enable", "--now", &unit]))?;
    if replace {
        checked(Command::new("systemctl").args(["--user", "restart", &unit]))?;
    }
    Ok(())
}

fn checked(command: &mut Command) -> Result<()> {
    let output = command.output().context("start Server supervisor")?;
    anyhow::ensure!(
        output.status.success(),
        "Server supervisor: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    Ok(())
}

/// Reconcile an existing service through the newly selected CLI after the
/// promotion lock is released. First installation remains demand-driven.
pub fn refresh_after_promotion() {
    let refresh = || -> Result<()> {
        if !crate::server::socket_path()?.exists() {
            return Ok(());
        }
        let executable = crate::installation::installed_cli()?;
        let mut command = Command::new(executable);
        for (key, _) in std::env::vars_os() {
            if key.to_string_lossy().starts_with("LF_") {
                command.env_remove(key);
            }
        }
        command.args(["install", "server"]);
        checked(&mut command)
    };
    if let Err(error) = refresh() {
        eprintln!("warning: installed Loopflow server is unavailable ({error:#}); `lf server status` reports readiness");
    }
}

/// Default-Machine server registration always belongs to the promoted CLI.
/// An explicit experimental Machine keeps its selected source executable.
pub fn delegate_selected() -> Result<()> {
    use std::os::unix::process::CommandExt;
    if crate::store::custom_home_selected() {
        return Ok(());
    }
    let selected = crate::installation::installed_cli().context(
        "Loopflow server needs an installed lf; source experiments select an explicit LF_HOME",
    )?;
    if selected == fs::canonicalize(std::env::current_exe()?)? {
        return Ok(());
    }
    Err(Command::new(selected)
        .args(std::env::args_os().skip(1))
        .exec())
    .context("start the selected installation's Loopflow server")
}

pub(crate) fn readiness_command() -> Result<std::process::Command> {
    let mut command = std::process::Command::new(crate::os_process::resolve_lf_binary()?);
    command
        .args(["install", "server"])
        .stdin(std::process::Stdio::null());
    Ok(command)
}
