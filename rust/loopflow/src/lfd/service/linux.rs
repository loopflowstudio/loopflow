use anyhow::Result;
use std::path::PathBuf;
use std::process::Command;

use crate::lfd::config::LfdConfig;

use super::{contains_sensitive_service_env, service_environment, write_service_file};

// Linux systemd user service management.
//
// Installs a systemd user unit that starts lfd on login and restarts on crash.

const UNIT_NAME: &str = "lfd.service";
const UNIT_ID: &str = "lfd";

fn unit_path() -> Result<PathBuf> {
    let config = dirs::config_dir().ok_or_else(|| anyhow::anyhow!("no config directory"))?;
    Ok(config.join("systemd/user").join(UNIT_NAME))
}

pub fn install(_config: &LfdConfig, force: bool) -> Result<()> {
    let unit_path = unit_path()?;
    if unit_path.exists() && !force {
        anyhow::bail!("lfd is already installed. Re-run `lfd install --force` to replace it.");
    }

    let lfd_path = std::env::current_exe()?
        .canonicalize()?
        .to_string_lossy()
        .to_string();
    let path_env = std::env::var("PATH").unwrap_or_default();
    let env_vars = service_environment(&path_env);
    let exec_start = build_exec_start(&lfd_path);

    if let Some(parent) = unit_path.parent() {
        std::fs::create_dir_all(parent)?;
    }

    let content = render_unit(&exec_start, &env_vars);

    write_service_file(
        &unit_path,
        &content,
        contains_sensitive_service_env(&env_vars),
    )?;
    println!("Installed {}", unit_path.display());

    let _ = Command::new("systemctl")
        .args(["--user", "daemon-reload"])
        .status();

    let status = Command::new("systemctl")
        .args(["--user", "enable", UNIT_ID])
        .status()?;
    if status.success() {
        println!("Enabled lfd (starts on login)");
    }

    let status = Command::new("systemctl")
        .args(["--user", "start", UNIT_ID])
        .status()?;
    if status.success() {
        println!("lfd service started");
    } else {
        eprintln!("Warning: systemctl start failed (exit {})", status);
    }

    Ok(())
}

pub fn uninstall(_config: &LfdConfig) -> Result<()> {
    let unit_path = unit_path()?;
    if !unit_path.exists() {
        println!("Not installed");
        return Ok(());
    }

    let _ = Command::new("systemctl")
        .args(["--user", "disable", "--now", UNIT_ID])
        .status();

    std::fs::remove_file(&unit_path)?;

    let _ = Command::new("systemctl")
        .args(["--user", "daemon-reload"])
        .status();

    println!("Uninstalled lfd");

    Ok(())
}

pub fn start(config: &LfdConfig, force: bool) -> Result<()> {
    let unit_path = unit_path()?;
    if !unit_path.exists() {
        return Err(anyhow::anyhow!(
            "lfd is not installed — run `lfd install` first"
        ));
    }

    if force {
        return install(config, true);
    }

    let status = Command::new("systemctl")
        .args(["--user", "start", UNIT_ID])
        .status()?;

    if status.success() {
        println!("Started lfd");
    } else {
        eprintln!("Failed to start lfd (exit {})", status);
    }

    Ok(())
}

pub fn stop(_config: &LfdConfig) -> Result<()> {
    let unit_path = unit_path()?;
    if !unit_path.exists() {
        return Err(anyhow::anyhow!("lfd is not installed"));
    }

    let status = Command::new("systemctl")
        .args(["--user", "stop", UNIT_ID])
        .status()?;

    if status.success() {
        println!("Stopped lfd");
    } else {
        eprintln!("Failed to stop lfd (exit {})", status);
    }

    Ok(())
}

pub fn status(_config: &LfdConfig) -> Result<()> {
    let unit = unit_path()?;
    if !unit.exists() {
        println!("manager: systemd (missing)");
        println!("backend: native");
        println!("remediation: run `lfd install`");
        return Ok(());
    }

    let output = Command::new("systemctl")
        .args(["--user", "is-active", UNIT_ID])
        .output()?;
    let active = String::from_utf8_lossy(&output.stdout).trim().to_string();
    let manager_state = if active.is_empty() {
        "unknown"
    } else {
        active.as_str()
    };

    println!("manager: systemd ({manager_state})");
    println!("backend: native");

    Ok(())
}

fn render_unit(exec_start: &str, env_vars: &[(String, String)]) -> String {
    let env_lines = env_vars
        .iter()
        .map(|(name, value)| format!("Environment=\"{}={}\"", name, systemd_escape(value)))
        .collect::<Vec<_>>()
        .join("\n");
    format!(
        r#"[Unit]
Description=Loopflow Daemon
After=network.target

[Service]
Type=simple
ExecStart={exec_start}
Restart=on-failure
RestartSec=5
{env_lines}

[Install]
WantedBy=default.target
"#
    )
}

fn systemd_escape(value: &str) -> String {
    value.replace('\\', "\\\\").replace('"', "\\\"")
}

fn build_exec_start(lfd_path: &str) -> String {
    format!("{lfd_path} serve")
}

#[cfg(test)]
mod tests {
    use super::render_unit;

    #[test]
    fn render_unit_persists_remote_native_environment() {
        let env_vars = vec![
            ("PATH".to_string(), "/usr/bin:/bin".to_string()),
            ("LFD_HTTP_ADDR".to_string(), "0.0.0.0:2486".to_string()),
            ("LFD_AUTH_TOKEN".to_string(), "token\"value".to_string()),
        ];
        let content = render_unit("/usr/local/bin/lfd serve", &env_vars);

        assert!(content.contains("Environment=\"LFD_HTTP_ADDR=0.0.0.0:2486\""));
        assert!(content.contains("Environment=\"LFD_AUTH_TOKEN=token\\\"value\""));
    }
}
