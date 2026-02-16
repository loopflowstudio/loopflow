use anyhow::{bail, Result};
use std::path::PathBuf;
use std::process::Command;

use crate::lfd::config::{LfdConfig, RuntimeBackend};

use super::compose;

/// macOS launchd service management.
///
/// Installs a LaunchAgent plist that starts lfd on login and keeps it alive.
const LABEL: &str = "com.loopflow.lfd";

fn plist_path() -> Result<PathBuf> {
    let home = dirs::home_dir().ok_or_else(|| anyhow::anyhow!("no home directory"))?;
    Ok(home
        .join("Library/LaunchAgents")
        .join(format!("{LABEL}.plist")))
}

fn log_dir() -> Result<PathBuf> {
    let home = dirs::home_dir().ok_or_else(|| anyhow::anyhow!("no home directory"))?;
    Ok(home.join(".lf/logs"))
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

    let lfd_path = std::env::current_exe()?
        .canonicalize()?
        .to_string_lossy()
        .to_string();
    let path_env = std::env::var("PATH").unwrap_or_default();
    let log_dir = log_dir()?;
    std::fs::create_dir_all(&log_dir)?;

    if config.runtime_backend == RuntimeBackend::Compose {
        compose::ensure_docker_available()?;
        compose::write_managed_compose_file(config)?;
        compose::pull()?;
    }

    let program_args = build_program_arguments(config.runtime_backend, &lfd_path)?;

    let plist_path = plist_path()?;
    if let Some(parent) = plist_path.parent() {
        std::fs::create_dir_all(parent)?;
    }

    let content = render_plist(&program_args, &path_env, &log_dir)?;

    let plist_str = plist_path.to_string_lossy().to_string();
    let _ = Command::new("launchctl")
        .args(["unload", &plist_str])
        .output();

    std::fs::write(&plist_path, &content)?;
    println!("Installed {}", plist_path.display());

    let status = Command::new("launchctl")
        .args(["load", &plist_str])
        .status()?;
    if status.success() {
        println!("lfd service loaded ({})", config.runtime_backend.as_str());
    } else {
        eprintln!("Warning: launchctl load failed (exit {})", status);
    }

    Ok(())
}

pub fn uninstall(_config: &LfdConfig) -> Result<()> {
    let backend = installed_backend()?;
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

    if let Some(RuntimeBackend::Compose) = backend {
        let _ = compose::teardown();
    }

    Ok(())
}

pub fn start(config: &LfdConfig, force: bool) -> Result<()> {
    let plist_path = plist_path()?;
    if !plist_path.exists() {
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

    let plist_str = plist_path.to_string_lossy().to_string();
    let status = Command::new("launchctl")
        .args(["load", &plist_str])
        .status()?;

    if status.success() {
        println!("Started lfd ({})", config.runtime_backend.as_str());
    } else {
        eprintln!("Failed to start lfd (exit {})", status);
    }

    Ok(())
}

pub fn stop(_config: &LfdConfig) -> Result<()> {
    let plist_path = plist_path()?;
    if !plist_path.exists() {
        return Err(anyhow::anyhow!("lfd is not installed"));
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

pub fn status(config: &LfdConfig) -> Result<()> {
    let plist = plist_path()?;
    if !plist.exists() {
        println!("manager: launchd (missing)");
        println!("backend: {}", config.runtime_backend.as_str());
        println!("remediation: run `lfd install`");
        return Ok(());
    }

    let backend = installed_backend()?.unwrap_or(RuntimeBackend::Native);
    let output = Command::new("launchctl").args(["list", LABEL]).output()?;

    if output.status.success() {
        let stdout = String::from_utf8_lossy(&output.stdout);
        let running = stdout.lines().any(|line| {
            let parts: Vec<&str> = line.split_whitespace().collect();
            parts
                .first()
                .is_some_and(|pid| *pid != "-" && pid.parse::<u32>().is_ok())
        });
        println!(
            "manager: launchd ({})",
            if running { "running" } else { "loaded" }
        );
    } else {
        println!("manager: launchd (not_loaded)");
    }

    println!("backend: {}", backend.as_str());

    if backend == RuntimeBackend::Compose {
        compose::print_service_status();
    }

    Ok(())
}

fn render_plist(
    program_args: &[String],
    path_env: &str,
    log_dir: &std::path::Path,
) -> Result<String> {
    let program_args_xml = program_args
        .iter()
        .map(|arg| format!("        <string>{arg}</string>"))
        .collect::<Vec<_>>()
        .join("\n");

    Ok(format!(
        r#"<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
    <key>Label</key>
    <string>{LABEL}</string>
    <key>ProgramArguments</key>
    <array>
{program_args_xml}
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
        log_dir = log_dir.display(),
    ))
}

fn build_program_arguments(backend: RuntimeBackend, lfd_path: &str) -> Result<Vec<String>> {
    match backend {
        RuntimeBackend::Native => Ok(vec![lfd_path.to_string(), "serve".to_string()]),
        RuntimeBackend::Compose => {
            let mut args = vec!["/usr/bin/env".to_string(), "docker".to_string()];
            args.extend(compose::compose_program_args()?);
            Ok(args)
        }
    }
}

fn installed_backend() -> Result<Option<RuntimeBackend>> {
    let plist = plist_path()?;
    if !plist.exists() {
        return Ok(None);
    }

    let content = std::fs::read_to_string(plist)?;
    Ok(Some(detect_backend_from_plist(&content)))
}

fn detect_backend_from_plist(content: &str) -> RuntimeBackend {
    if content.contains("<string>docker</string>") && content.contains("<string>compose</string>") {
        RuntimeBackend::Compose
    } else {
        RuntimeBackend::Native
    }
}

fn teardown_backend(backend: RuntimeBackend) -> Result<()> {
    if backend == RuntimeBackend::Compose {
        compose::teardown()?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::{detect_backend_from_plist, RuntimeBackend};

    #[test]
    fn detects_compose_backend_from_plist_content() {
        let content = r#"
<array>
    <string>/usr/bin/env</string>
    <string>docker</string>
    <string>compose</string>
</array>
"#;
        assert_eq!(detect_backend_from_plist(content), RuntimeBackend::Compose);
    }

    #[test]
    fn detects_native_backend_from_plist_content() {
        let content = r#"
<array>
    <string>/usr/local/bin/lfd</string>
    <string>serve</string>
</array>
"#;
        assert_eq!(detect_backend_from_plist(content), RuntimeBackend::Native);
    }
}
