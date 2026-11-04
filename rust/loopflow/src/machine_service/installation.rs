//! The selected lf entry owns service setup. Development Machines use distinct
//! supervisor identities; neither registration nor removal changes Work.
use anyhow::{Context, Result};
use fs2::FileExt;
use serde::Serialize;
use sha2::{Digest, Sha256};
use std::fs;
use std::os::unix::fs::OpenOptionsExt;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::{Duration, Instant};

#[derive(Debug, Serialize)]
pub struct Status {
    pub socket: PathBuf,
    pub ready: bool,
    pub instance: Option<String>,
    pub error: Option<String>,
}

pub fn status() -> Result<Status> {
    let socket = super::socket_path()?;
    match super::client::connect(super::wire::Operation::Bootstrap) {
        Ok((_, response)) => {
            let expected = crate::installation::ArtifactIdentity::capture(
                crate::installation::ArtifactRole::Cli,
                &std::env::current_exe()?,
            )?
            .sha256;
            let compatible = matches!(&response.result, super::wire::Reply::Bootstrap { version, executable_sha256, .. } if *version == super::wire::VERSION && executable_sha256 == &expected);
            let failure = match &response.result {
                super::wire::Reply::Error { code, message } => format!("{code:?}: {message}"),
                _ => "Machine service uses another executable or protocol; obtain the selected installation"
                    .into(),
            };
            Ok(Status {
                socket,
                ready: compatible,
                instance: Some(response.instance),
                error: (!compatible).then_some(failure),
            })
        }
        Err(error) => Ok(Status {
            socket,
            ready: false,
            instance: None,
            error: Some(error.to_string()),
        }),
    }
}

pub fn ensure() -> Result<Status> {
    let lock = fs::OpenOptions::new()
        .create(true)
        .truncate(false)
        .read(true)
        .write(true)
        .mode(0o600)
        .open(super::socket_path()?.with_extension("install-lock"))?;
    lock.lock_exclusive()?;
    let before = status()?;
    if before.ready {
        return Ok(before);
    }
    let home = crate::store::lf_home_dir().canonicalize()?;
    let executable = crate::os_process::resolve_lf_binary()?;
    register(&home, &executable, before.instance.is_some())?;
    let deadline = Instant::now() + Duration::from_secs(10);
    loop {
        let status = status()?;
        if status.ready {
            return Ok(status);
        }
        anyhow::ensure!(
            Instant::now() < deadline,
            "Machine service unavailable: {}",
            status.error.as_deref().unwrap_or("not ready")
        );
        std::thread::sleep(Duration::from_millis(100));
    }
}

fn label(home: &Path) -> String {
    let digest = hex::encode(Sha256::digest(home.as_os_str().as_encoded_bytes()));
    format!("com.loopflow.machine.{}", &digest[..16])
}

#[cfg(target_os = "macos")]
fn register(home: &Path, executable: &Path, replace: bool) -> Result<()> {
    // A signed app owns its SMAppService registration. An explicit development
    // Machine has no bundle and uses a separate launchctl user job.
    if !crate::store::custom_home_selected() {
        return register_bundled(replace);
    }
    let label = label(home);
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
            .any(|line| line.contains(&format!("\"{label}\"")) && line.contains("true")),
        "Machine service is disabled in Background Items; enable it in System Settings"
    );
    let path = home.join("machine-service.plist");
    let escape = |value: &str| {
        value
            .replace('&', "&amp;")
            .replace('<', "&lt;")
            .replace('>', "&gt;")
            .replace('"', "&quot;")
    };
    let log = home.join("logs/machine-service.log");
    fs::create_dir_all(log.parent().expect("log has a parent"))?;
    let xml = format!(
        r#"<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0"><dict>
<key>Label</key><string>{label}</string>
<key>ProgramArguments</key><array><string>{executable}</string><string>machine</string><string>service</string><string>run</string></array>
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
        fn lf_machine_service_register(
            plist_name: *const std::ffi::c_char,
            replace: bool,
            error: *mut std::ffi::c_char,
            capacity: usize,
        ) -> i32;
    }
    let mut error = [0 as std::ffi::c_char; 1024];
    // SAFETY: the native function writes at most capacity bytes and NUL terminates.
    let result = unsafe {
        lf_machine_service_register(
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
    let unit = format!("{}.service", label(home));
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
    fs::write(directory.join(&unit), format!("[Unit]\nDescription=Loopflow Machine\n[Service]\nExecStart={executable_argument} machine service run\nEnvironment={home_environment}\nEnvironment={binary_environment}\nRestart=on-failure\nKillMode=control-group\n[Install]\nWantedBy=default.target\n"))?;
    checked(Command::new("systemctl").args(["--user", "daemon-reload"]))?;
    checked(Command::new("systemctl").args(["--user", "enable", "--now", &unit]))?;
    if replace {
        checked(Command::new("systemctl").args(["--user", "restart", &unit]))?;
    }
    Ok(())
}

fn checked(command: &mut Command) -> Result<()> {
    let output = command.output().context("start Machine supervisor")?;
    anyhow::ensure!(
        output.status.success(),
        "Machine supervisor: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    Ok(())
}

/// Reconcile an existing service through the newly selected CLI after the
/// promotion lock is released. First installation remains demand-driven.
pub fn refresh_after_promotion() {
    let refresh = || -> Result<()> {
        if !super::socket_path()?.exists() {
            return Ok(());
        }
        let executable = crate::installation::installed_cli()?;
        let mut command = Command::new(executable);
        for (key, _) in std::env::vars_os() {
            if key.to_string_lossy().starts_with("LF_") {
                command.env_remove(key);
            }
        }
        command.args(["machine", "service", "ensure"]);
        checked(&mut command)
    };
    if let Err(error) = refresh() {
        eprintln!("warning: installed Machine service is unavailable ({error:#}); `lf machine service ensure` reports readiness");
    }
}

/// Default-Machine service registration always belongs to the promoted CLI.
/// An explicit experimental Machine keeps its selected source executable.
pub fn delegate_selected() -> Result<()> {
    use std::os::unix::process::CommandExt;
    if crate::store::custom_home_selected() {
        return Ok(());
    }
    let selected = crate::installation::installed_cli().context(
        "Machine service needs an installed lf; source experiments select an explicit LF_HOME",
    )?;
    if selected == fs::canonicalize(std::env::current_exe()?)? {
        return Ok(());
    }
    Err(Command::new(selected)
        .args(std::env::args_os().skip(1))
        .exec())
    .context("start the selected installation's Machine service")
}
