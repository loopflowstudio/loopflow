use anyhow::{bail, Result};
use std::path::PathBuf;
use std::process::Command;

use crate::lfd::config::{LfdConfig, RuntimeBackend};

use super::compose;

// Linux systemd user service management.
//
// Installs a systemd user unit that starts lfd on login and restarts on crash.

const UNIT_NAME: &str = "lfd.service";
const UNIT_ID: &str = "lfd";

fn unit_path() -> Result<PathBuf> {
    let config = dirs::config_dir().ok_or_else(|| anyhow::anyhow!("no config directory"))?;
    Ok(config.join("systemd/user").join(UNIT_NAME))
}

pub fn install(config: &LfdConfig, force: bool) -> Result<()> {
    if let Some(existing_backend) = installed_backend()? {
        if existing_backend != config.runtime_backend {
            if !force {
                bail!(
                    "lfd is already installed with `{}` backend. Re-run `lfd install --force` to replace it.",
                    existing_backend.as_str()
                );
            }
            teardown_backend(existing_backend)?;
        }
    }

    if config.runtime_backend == RuntimeBackend::Compose {
        compose::ensure_docker_available()?;
        compose::write_managed_compose_file(config)?;
        compose::pull()?;
    }

    let lfd_path = std::env::current_exe()?
        .canonicalize()?
        .to_string_lossy()
        .to_string();
    let path_env = std::env::var("PATH").unwrap_or_default();
    let exec_start = build_exec_start(config.runtime_backend, &lfd_path)?;

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
ExecStart={exec_start}
Restart=on-failure
RestartSec=5
Environment=PATH={path_env}

[Install]
WantedBy=default.target
"#
    );

    std::fs::write(&unit_path, &content)?;
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
        println!("lfd service started ({})", config.runtime_backend.as_str());
    } else {
        eprintln!("Warning: systemctl start failed (exit {})", status);
    }

    Ok(())
}

pub fn uninstall(_config: &LfdConfig) -> Result<()> {
    let backend = installed_backend()?;
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

    if let Some(RuntimeBackend::Compose) = backend {
        let _ = compose::down();
        let _ = compose::remove_managed_compose_file();
    }

    Ok(())
}

pub fn start(config: &LfdConfig, force: bool) -> Result<()> {
    let unit_path = unit_path()?;
    if !unit_path.exists() {
        return Err(anyhow::anyhow!(
            "lfd is not installed — run `lfd install` first"
        ));
    }

    let installed_backend = installed_backend()?.unwrap_or(RuntimeBackend::Native);
    if installed_backend != config.runtime_backend {
        if !force {
            bail!(
                "lfd is installed with `{}` backend, but config mode expects `{}`. Run `lfd install --force`.",
                installed_backend.as_str(),
                config.runtime_backend.as_str()
            );
        }
        return install(config, true);
    }

    if config.runtime_backend == RuntimeBackend::Compose {
        compose::ensure_docker_available()?;
    }

    let status = Command::new("systemctl")
        .args(["--user", "start", UNIT_ID])
        .status()?;

    if status.success() {
        println!("Started lfd ({})", config.runtime_backend.as_str());
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

pub fn status(config: &LfdConfig) -> Result<()> {
    let unit = unit_path()?;
    if !unit.exists() {
        println!("manager: systemd (missing)");
        println!("backend: {}", config.runtime_backend.as_str());
        println!("remediation: run `lfd install`");
        return Ok(());
    }

    let backend = installed_backend()?.unwrap_or(RuntimeBackend::Native);

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
    println!("backend: {}", backend.as_str());

    if backend == RuntimeBackend::Compose {
        match compose::service_status() {
            Ok(rows) if rows.is_empty() => println!("compose: no services"),
            Ok(rows) => {
                for row in rows {
                    let health = row.health.unwrap_or_else(|| "n/a".to_string());
                    println!(
                        "service: {} state={} health={}",
                        row.service, row.state, health
                    );
                }
            }
            Err(err) => println!("compose: unhealthy ({err})"),
        }
    }

    Ok(())
}

fn build_exec_start(backend: RuntimeBackend, lfd_path: &str) -> Result<String> {
    match backend {
        RuntimeBackend::Native => Ok(format!("{lfd_path} serve")),
        RuntimeBackend::Compose => {
            let mut args = vec!["/usr/bin/env".to_string(), "docker".to_string()];
            args.extend(compose::compose_program_args()?);
            Ok(args.join(" "))
        }
    }
}

fn installed_backend() -> Result<Option<RuntimeBackend>> {
    let unit = unit_path()?;
    if !unit.exists() {
        return Ok(None);
    }

    let content = std::fs::read_to_string(unit)?;
    Ok(Some(detect_backend_from_unit(&content)))
}

fn detect_backend_from_unit(content: &str) -> RuntimeBackend {
    if content.contains("docker compose") {
        RuntimeBackend::Compose
    } else {
        RuntimeBackend::Native
    }
}

fn teardown_backend(backend: RuntimeBackend) -> Result<()> {
    if backend == RuntimeBackend::Compose {
        let _ = compose::down();
        let _ = compose::remove_managed_compose_file();
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::{detect_backend_from_unit, RuntimeBackend};

    #[test]
    fn detects_compose_backend_from_unit_content() {
        let content = "ExecStart=/usr/bin/env docker compose -f /tmp/docker-compose.yml up";
        assert_eq!(detect_backend_from_unit(content), RuntimeBackend::Compose);
    }

    #[test]
    fn detects_native_backend_from_unit_content() {
        let content = "ExecStart=/usr/local/bin/lfd serve";
        assert_eq!(detect_backend_from_unit(content), RuntimeBackend::Native);
    }
}
