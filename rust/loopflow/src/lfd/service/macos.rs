use anyhow::Result;
use std::path::PathBuf;
use std::process::Command;

use crate::lfd::config::LfdConfig;

use super::{contains_sensitive_service_env, service_environment, write_service_file};

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

pub fn install(_config: &LfdConfig, force: bool) -> Result<()> {
    let plist_path = plist_path()?;
    if plist_path.exists() && !force {
        anyhow::bail!("lfd is already installed. Re-run `lfd install --force` to replace it.");
    }

    let lfd_path = std::env::current_exe()?
        .canonicalize()?
        .to_string_lossy()
        .to_string();
    let path_env = std::env::var("PATH").unwrap_or_default();
    let env_vars = service_environment(&path_env);
    let log_dir = log_dir()?;
    std::fs::create_dir_all(&log_dir)?;

    let program_args = build_program_arguments(&lfd_path);

    if let Some(parent) = plist_path.parent() {
        std::fs::create_dir_all(parent)?;
    }

    let content = render_plist(&program_args, &env_vars, &log_dir)?;

    let plist_str = plist_path.to_string_lossy().to_string();
    let _ = Command::new("launchctl")
        .args(["unload", &plist_str])
        .output();

    write_service_file(
        &plist_path,
        &content,
        contains_sensitive_service_env(&env_vars),
    )?;
    println!("Installed {}", plist_path.display());

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

pub fn uninstall(_config: &LfdConfig) -> Result<()> {
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

pub fn start(config: &LfdConfig, force: bool) -> Result<()> {
    let plist_path = plist_path()?;
    if !plist_path.exists() {
        return Err(anyhow::anyhow!(
            "lfd is not installed — run `lfd install` first"
        ));
    }

    if force {
        return install(config, true);
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

pub fn status(_config: &LfdConfig) -> Result<()> {
    let plist = plist_path()?;
    if !plist.exists() {
        println!("manager: launchd (missing)");
        println!("backend: native");
        println!("remediation: run `lfd install`");
        return Ok(());
    }

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

    println!("backend: native");

    Ok(())
}

fn render_plist(
    program_args: &[String],
    env_vars: &[(String, String)],
    log_dir: &std::path::Path,
) -> Result<String> {
    let program_args_xml = program_args
        .iter()
        .map(|arg| format!("        <string>{}</string>", xml_escape(arg)))
        .collect::<Vec<_>>()
        .join("\n");
    let env_xml = env_vars
        .iter()
        .map(|(name, value)| {
            format!(
                "        <key>{}</key>\n        <string>{}</string>",
                xml_escape(name),
                xml_escape(value)
            )
        })
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
{env_xml}
    </dict>
</dict>
</plist>
"#,
        log_dir = log_dir.display(),
        env_xml = env_xml,
    ))
}

fn xml_escape(value: &str) -> String {
    value
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&apos;")
}

fn build_program_arguments(lfd_path: &str) -> Vec<String> {
    vec![lfd_path.to_string(), "serve".to_string()]
}

#[cfg(test)]
mod tests {
    use super::render_plist;
    use std::path::Path;

    #[test]
    fn render_plist_persists_remote_native_environment() {
        let env_vars = vec![
            ("PATH".to_string(), "/usr/bin:/bin".to_string()),
            ("LFD_HTTP_ADDR".to_string(), "0.0.0.0:2486".to_string()),
            ("LFD_AUTH_TOKEN".to_string(), "token<&>\"".to_string()),
        ];
        let content = render_plist(
            &["/usr/local/bin/lfd".to_string(), "serve".to_string()],
            &env_vars,
            Path::new("/tmp"),
        )
        .expect("render plist");

        assert!(content.contains("<key>LFD_HTTP_ADDR</key>"));
        assert!(content.contains("<string>0.0.0.0:2486</string>"));
        assert!(content.contains("<key>LFD_AUTH_TOKEN</key>"));
        assert!(content.contains("token&lt;&amp;&gt;&quot;"));
    }
}
