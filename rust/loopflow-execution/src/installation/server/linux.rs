use anyhow::{Context, Result};
use std::fs;
use std::io;
use std::path::Path;
use std::process::Command;

use super::{checked, detached, label};

pub(super) fn user_manager() -> Result<bool> {
    let result = Command::new("systemctl")
        .args(["--user", "show", "--property=Version", "--value"])
        .env("LC_ALL", "C")
        .output();
    let output = match result {
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(false),
        result => result.context("probe systemd user manager")?,
    };
    if output.status.success() {
        return Ok(true);
    }
    let error = String::from_utf8_lossy(&output.stderr);
    if error.contains("Failed to connect to")
        && [
            "No medium found",
            "No such file or directory",
            "Connection refused",
            "Host is down",
            "not set",
            "not defined",
        ]
        .iter()
        .any(|reason| error.contains(reason))
    {
        return Ok(false);
    }
    anyhow::bail!("systemd user manager: {}", error.trim())
}

pub(super) fn register_default(home: &Path, executable: &Path, replace: bool) -> Result<()> {
    let directory = crate::installation::account_home()?.join(".config/systemd/user");
    let unit = format!("{}.service", label(home, "server"));
    let legacy = format!("{}.service", label(home, "machine"));
    if !user_manager()? {
        // Our units install into default.target.wants. A missing bus cannot
        // answer is-enabled, but its enablement and mask are still on disk.
        for name in [&unit, &legacy] {
            let path = directory.join(name);
            if path.symlink_metadata().is_ok() {
                anyhow::ensure!(
                    directory.join("default.target.wants").join(name).exists()
                        && path.canonicalize()? != Path::new("/dev/null"),
                    "Loopflow server is disabled; enable {name} with systemctl --user"
                );
            }
        }
        return detached::start(home, executable, replace);
    }
    fs::create_dir_all(&directory)?;
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
    fs::write(
        directory.join(&unit),
        format!(
            "[Unit]\nDescription=Loopflow server\n[Service]\nExecStart={executable_argument} server run\nEnvironment={home_environment}\nEnvironment={binary_environment}\nRestart=on-failure\nKillMode=process\n[Install]\nWantedBy=default.target\n"
        ),
    )?;
    checked(Command::new("systemctl").args(["--user", "daemon-reload"]))?;
    checked(Command::new("systemctl").args(["--user", "enable", "--now", &unit]))?;
    if replace {
        checked(Command::new("systemctl").args(["--user", "restart", &unit]))?;
    }
    Ok(())
}

/// Retire any legacy custom-home systemd unit left by an older `lf`.
pub(super) fn retire_legacy_custom_home(home: &Path) -> Result<()> {
    let Ok(account_home) = crate::installation::account_home() else {
        return Ok(());
    };
    let directory = account_home.join(".config/systemd/user");
    let units = [
        format!("{}.service", label(home, "server")),
        format!("{}.service", label(home, "machine")),
    ];
    let present: Vec<&String> = units
        .iter()
        .filter(|name| directory.join(name).symlink_metadata().is_ok())
        .collect();
    if present.is_empty() {
        return Ok(());
    }
    let has_manager = user_manager().unwrap_or(false);
    for name in present {
        if has_manager {
            let _ = Command::new("systemctl")
                .args(["--user", "disable", "--now", name])
                .output();
        }
        let _ = fs::remove_file(directory.join(name));
    }
    if has_manager {
        let _ = Command::new("systemctl")
            .args(["--user", "daemon-reload"])
            .output();
    }
    Ok(())
}

pub(super) fn retire_self_if_registered() {
    if std::env::var_os("INVOCATION_ID").is_none() {
        return;
    }
    let home = crate::store::lf_home_dir();
    let _ = retire_legacy_custom_home(&home);
}
