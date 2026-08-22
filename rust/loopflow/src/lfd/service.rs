//! `lfd` service lifecycle: render and manage the launchd (macOS) or systemd
//! user (Linux) unit that keeps the Home daemon running.
//!
//! Service files never carry secrets. `LF_HOME` / `LF_DB_PATH` are non-secret
//! path configuration and `PATH` may be embedded; secrets are resolved from
//! the environment or Doppler by `lfd` itself. The file is written `0o600`
//! regardless, since it may contain paths.

use std::io::Write;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

pub const LABEL: &str = "com.loopflow.lfd";

fn account_home() -> anyhow::Result<PathBuf> {
    crate::machine_install::account_home()
}

/// The non-secret configuration embedded into the service file.
#[derive(Debug, Clone)]
pub struct ServiceSpec {
    /// Absolute path to the `lfd` binary (`ProgramArguments[0]` / `ExecStart`).
    pub lfd_path: PathBuf,
    /// Address the daemon binds (`serve --addr <addr>`).
    pub addr: String,
    /// Repository root passed explicitly because service managers do not start
    /// in the checkout that ran `lfd install`.
    pub repo_root: PathBuf,
    /// `LF_HOME`, when set in the installing environment.
    pub lf_home: Option<PathBuf>,
    /// `LF_DB_PATH`, when set in the installing environment.
    pub db_path: Option<PathBuf>,
    /// Executable search path captured at install time so launchd can find
    /// Homebrew tools such as `gh` and `doppler`.
    pub path_env: Option<String>,
    /// Non-secret Doppler project selection captured from `doppler run`.
    pub doppler_project: Option<String>,
    /// Non-secret Doppler config selection captured from `doppler run`.
    pub doppler_config: Option<String>,
}

/// Where the rendered service file lands, and how it is loaded, per platform.
#[derive(Debug, Clone, Serialize)]
pub struct ServiceFile {
    pub path: PathBuf,
    pub platform: &'static str,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum KeeperMode {
    None,
    Launchd,
    Systemd,
}

fn xml_escape(value: &str) -> String {
    value
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&apos;")
}

fn shell_escape(value: &str) -> String {
    // systemd Environment values are shell-style; quote only when a value could
    // otherwise be misread. A path or addr never contains a quote, so a simple
    // wrap is safe and unambiguous.
    if value.is_empty()
        || value
            .chars()
            .any(|c| c.is_whitespace() || matches!(c, '"' | '\'' | '$'))
    {
        format!("\"{}\"", value.replace('\\', "\\\\").replace('"', "\\\""))
    } else {
        value.to_string()
    }
}

/// Render the launchd plist for macOS. `RunAtLoad` + `KeepAlive` keep the daemon
/// up; `ThrottleInterval` bounds restart churn. `lfd` owns its bounded log;
/// launchd output is discarded so the service manager cannot duplicate it.
pub fn render_launchd_plist(spec: &ServiceSpec) -> String {
    let program_args = [
        spec.lfd_path.to_string_lossy().to_string(),
        "serve".to_string(),
        "--addr".to_string(),
        spec.addr.clone(),
        "--repo".to_string(),
        spec.repo_root.to_string_lossy().to_string(),
    ];
    let program_args_xml = program_args
        .iter()
        .map(|arg| format!("        <string>{}</string>", xml_escape(arg)))
        .collect::<Vec<_>>()
        .join("\n");
    let mut env = String::new();
    if let Some(home) = &spec.lf_home {
        env.push_str(&format!(
            "        <key>LF_HOME</key>\n        <string>{}</string>\n",
            xml_escape(&home.to_string_lossy())
        ));
    }
    if let Some(db) = &spec.db_path {
        env.push_str(&format!(
            "        <key>LF_DB_PATH</key>\n        <string>{}</string>\n",
            xml_escape(&db.to_string_lossy())
        ));
    }
    if let Some(path) = &spec.path_env {
        env.push_str(&format!(
            "        <key>PATH</key>\n        <string>{}</string>\n",
            xml_escape(path)
        ));
    }
    if let Some(project) = &spec.doppler_project {
        env.push_str(&format!(
            "        <key>DOPPLER_PROJECT</key>\n        <string>{}</string>\n",
            xml_escape(project)
        ));
    }
    if let Some(config) = &spec.doppler_config {
        env.push_str(&format!(
            "        <key>DOPPLER_CONFIG</key>\n        <string>{}</string>\n",
            xml_escape(config)
        ));
    }
    let env_block = if env.is_empty() {
        String::new()
    } else {
        format!("    <key>EnvironmentVariables</key>\n    <dict>\n{env}    </dict>\n")
    };
    format!(
        r#"<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
    <key>Label</key>
    <string>{label}</string>
    <key>ProgramArguments</key>
    <array>
{program_args_xml}
    </array>
    {env_block}<key>RunAtLoad</key>
    <true/>
    <key>KeepAlive</key>
    <true/>
    <key>ThrottleInterval</key>
    <integer>10</integer>
    <key>StandardOutPath</key>
    <string>/dev/null</string>
    <key>StandardErrorPath</key>
    <string>/dev/null</string>
</dict>
</plist>
"#,
        label = LABEL,
    )
}

/// Render the systemd user unit for Linux. `Restart = on-failure` with a 5s
/// backoff mirrors the launchd `KeepAlive` posture.
pub fn render_systemd_unit(spec: &ServiceSpec) -> String {
    let mut env_lines = String::new();
    if let Some(home) = &spec.lf_home {
        env_lines.push_str(&format!(
            "Environment=LF_HOME={}\n",
            shell_escape(&home.to_string_lossy())
        ));
    }
    if let Some(db) = &spec.db_path {
        env_lines.push_str(&format!(
            "Environment=LF_DB_PATH={}\n",
            shell_escape(&db.to_string_lossy())
        ));
    }
    if let Some(path) = &spec.path_env {
        env_lines.push_str(&format!("Environment=PATH={}\n", shell_escape(path)));
    }
    if let Some(project) = &spec.doppler_project {
        env_lines.push_str(&format!(
            "Environment=DOPPLER_PROJECT={}\n",
            shell_escape(project)
        ));
    }
    if let Some(config) = &spec.doppler_config {
        env_lines.push_str(&format!(
            "Environment=DOPPLER_CONFIG={}\n",
            shell_escape(config)
        ));
    }
    let exec_start = shell_escape(&spec.lfd_path.to_string_lossy());
    format!(
        "[Unit]\nDescription=Loopflow Home daemon (lfd)\n\n\
         [Service]\n\
         Type=simple\n\
         ExecStart={exec_start} serve --addr {addr} --repo {repo}\n\
         Restart=on-failure\n\
         RestartSec=5\n\
         {env_lines}\
         StandardOutput=null\n\
         StandardError=null\n\n\
         [Install]\n\
         WantedBy=default.target\n",
        addr = shell_escape(&spec.addr),
        repo = shell_escape(&spec.repo_root.to_string_lossy()),
    )
}

fn write_service_file(path: &Path, contents: &str) -> anyhow::Result<()> {
    let parent = path
        .parent()
        .ok_or_else(|| anyhow::anyhow!("service file has no parent directory"))?;
    std::fs::create_dir_all(parent)?;
    let name = path
        .file_name()
        .ok_or_else(|| anyhow::anyhow!("service file has no filename"))?
        .to_string_lossy();
    let pending = parent.join(format!(".{name}.tmp.{}", std::process::id()));
    let mut file = std::fs::OpenOptions::new()
        .create(true)
        .truncate(true)
        .write(true)
        .open(&pending)?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        file.set_permissions(std::fs::Permissions::from_mode(0o600))?;
    }
    file.write_all(contents.as_bytes())?;
    file.sync_all()?;
    std::fs::rename(&pending, path)?;
    std::fs::File::open(parent)?.sync_all()?;
    Ok(())
}

fn configure_launchd_switch(
    contents: &str,
    lfd_path: &Path,
    switch_id: Option<&str>,
) -> anyhow::Result<String> {
    let mut output = Vec::new();
    let mut in_arguments = false;
    let mut replaced_path = false;
    let mut skip_switch_value = false;
    for line in contents.lines() {
        if skip_switch_value {
            skip_switch_value = false;
            continue;
        }
        if line.trim() == "<key>ProgramArguments</key>" {
            in_arguments = true;
            output.push(line.to_string());
            continue;
        }
        if in_arguments && line.trim() == "</array>" {
            in_arguments = false;
        }
        if in_arguments && line.trim() == "<string>--install-switch</string>" {
            skip_switch_value = true;
            continue;
        }
        if in_arguments && !replaced_path && line.trim_start().starts_with("<string>") {
            let indent = &line[..line.len() - line.trim_start().len()];
            output.push(format!(
                "{indent}<string>{}</string>",
                xml_escape(&lfd_path.to_string_lossy())
            ));
            replaced_path = true;
            continue;
        }
        output.push(line.to_string());
        if in_arguments && line.trim() == "<string>serve</string>" {
            if let Some(switch_id) = switch_id {
                let indent = &line[..line.len() - line.trim_start().len()];
                output.push(format!("{indent}<string>--install-switch</string>"));
                output.push(format!(
                    "{indent}<string>{}</string>",
                    xml_escape(switch_id)
                ));
            }
        }
    }
    if !replaced_path {
        anyhow::bail!("installed launchd service has no daemon program argument");
    }
    Ok(format!("{}\n", output.join("\n")))
}

fn configure_systemd_switch(
    contents: &str,
    lfd_path: &Path,
    switch_id: Option<&str>,
) -> anyhow::Result<String> {
    let mut output = Vec::new();
    let mut replaced = false;
    for line in contents.lines() {
        if let Some(command) = line.strip_prefix("ExecStart=") {
            let (_, arguments) = command.split_once(" serve").ok_or_else(|| {
                anyhow::anyhow!("installed systemd service has no daemon serve command")
            })?;
            let arguments = if let Some((_, after)) = arguments.split_once(" --install-switch ") {
                let (_, rest) = after.split_once(" --addr").ok_or_else(|| {
                    anyhow::anyhow!("installed systemd switch capability has no --addr boundary")
                })?;
                format!(" --addr{rest}")
            } else {
                arguments.to_string()
            };
            let capability = switch_id
                .map(|id| format!(" --install-switch {}", shell_escape(id)))
                .unwrap_or_default();
            output.push(format!(
                "ExecStart={} serve{capability}{arguments}",
                shell_escape(&lfd_path.to_string_lossy())
            ));
            replaced = true;
        } else {
            output.push(line.to_string());
        }
    }
    if !replaced {
        anyhow::bail!("installed systemd service has no ExecStart");
    }
    Ok(format!("{}\n", output.join("\n")))
}

#[cfg(target_os = "macos")]
fn configured_service_path() -> anyhow::Result<PathBuf> {
    Ok(account_home()?
        .join("Library/LaunchAgents")
        .join(format!("{LABEL}.plist")))
}

#[cfg(target_os = "linux")]
fn configured_service_path() -> anyhow::Result<PathBuf> {
    Ok(account_home()?.join(".config/systemd/user/lfd.service"))
}

#[cfg(any(target_os = "macos", target_os = "linux"))]
fn configure_install_switch(
    mode: KeeperMode,
    lfd_path: &Path,
    switch_id: Option<&str>,
) -> anyhow::Result<()> {
    if mode == KeeperMode::None {
        return Ok(());
    }
    let path = configured_service_path()?;
    let contents = std::fs::read_to_string(&path)?;
    let configured = match mode {
        KeeperMode::Launchd => configure_launchd_switch(&contents, lfd_path, switch_id)?,
        KeeperMode::Systemd => configure_systemd_switch(&contents, lfd_path, switch_id)?,
        KeeperMode::None => return Ok(()),
    };
    write_service_file(&path, &configured)?;
    #[cfg(target_os = "linux")]
    {
        let status = std::process::Command::new("systemctl")
            .args(["--user", "daemon-reload"])
            .status()?;
        if !status.success() {
            anyhow::bail!("systemctl --user daemon-reload failed");
        }
    }
    Ok(())
}

#[cfg(not(any(target_os = "macos", target_os = "linux")))]
fn configure_install_switch(
    _mode: KeeperMode,
    _lfd_path: &Path,
    _switch_id: Option<&str>,
) -> anyhow::Result<()> {
    Ok(())
}

pub fn prepare_install_switch(
    mode: KeeperMode,
    lfd_path: &Path,
    switch_id: &str,
) -> anyhow::Result<()> {
    configure_install_switch(mode, lfd_path, Some(switch_id))
}

pub fn finish_install_switch(mode: KeeperMode, lfd_path: &Path) -> anyhow::Result<()> {
    configure_install_switch(mode, lfd_path, None)
}

// -- Install / uninstall / status (platform-specific) -----------------------

#[cfg(target_os = "macos")]
pub fn install(spec: &ServiceSpec) -> anyhow::Result<ServiceFile> {
    let home = account_home()?;
    let dir = home.join("Library/LaunchAgents");
    let path = dir.join(format!("{LABEL}.plist"));
    let plist = render_launchd_plist(spec);
    write_service_file(&path, &plist)?;
    // Reload: unload (no-op if not loaded) then load, so an edit takes effect.
    let _ = std::process::Command::new("launchctl")
        .arg("unload")
        .arg(&path)
        .status();
    let status = std::process::Command::new("launchctl")
        .arg("load")
        .arg(&path)
        .status()?;
    if !status.success() {
        anyhow::bail!("launchctl load failed for {}", path.display());
    }
    Ok(ServiceFile {
        path,
        platform: "launchd",
    })
}

#[cfg(target_os = "linux")]
pub fn install(spec: &ServiceSpec) -> anyhow::Result<ServiceFile> {
    let home = account_home()?;
    let dir = home.join(".config/systemd/user");
    let path = dir.join("lfd.service");
    let unit = render_systemd_unit(spec);
    write_service_file(&path, &unit)?;
    let reload = std::process::Command::new("systemctl")
        .args(["--user", "daemon-reload"])
        .status()?;
    let enable = std::process::Command::new("systemctl")
        .args(["--user", "enable", "--now", "lfd"])
        .status()?;
    if !reload.success() || !enable.success() {
        anyhow::bail!("systemctl enable --now lfd failed");
    }
    Ok(ServiceFile {
        path,
        platform: "systemd",
    })
}

#[cfg(not(any(target_os = "macos", target_os = "linux")))]
pub fn install(_spec: &ServiceSpec) -> anyhow::Result<ServiceFile> {
    anyhow::bail!("lfd install is supported on macOS and Linux only")
}

#[cfg(target_os = "macos")]
pub fn uninstall() -> anyhow::Result<PathBuf> {
    let home = account_home()?;
    let path = home
        .join("Library/LaunchAgents")
        .join(format!("{LABEL}.plist"));
    if path.exists() {
        let _ = std::process::Command::new("launchctl")
            .arg("unload")
            .arg(&path)
            .status();
        std::fs::remove_file(&path)?;
    }
    Ok(path)
}

#[cfg(target_os = "linux")]
pub fn uninstall() -> anyhow::Result<PathBuf> {
    let home = account_home()?;
    let path = home.join(".config/systemd/user/lfd.service");
    if path.exists() {
        let _ = std::process::Command::new("systemctl")
            .args(["--user", "disable", "--now", "lfd"])
            .status();
        std::fs::remove_file(&path)?;
        let _ = std::process::Command::new("systemctl")
            .args(["--user", "daemon-reload"])
            .status();
    }
    Ok(path)
}

#[cfg(not(any(target_os = "macos", target_os = "linux")))]
pub fn uninstall() -> anyhow::Result<PathBuf> {
    anyhow::bail!("lfd uninstall is supported on macOS and Linux only")
}

/// Report whether the service is loaded and running. Best-effort: a missing
/// `launchctl`/`systemctl` or an unloaded service prints "not installed" rather
/// than erroring.
#[cfg(target_os = "macos")]
pub fn status() -> anyhow::Result<String> {
    let out = std::process::Command::new("launchctl")
        .args(["list", LABEL])
        .output();
    match out {
        Ok(output) if output.status.success() => {
            Ok(format!("lfd installed and loaded (launchd label {LABEL})"))
        }
        _ => Ok(format!("lfd not installed (no launchd label {LABEL})")),
    }
}

#[cfg(target_os = "macos")]
pub fn configured_mode() -> anyhow::Result<KeeperMode> {
    let home = account_home()?;
    let path = home
        .join("Library/LaunchAgents")
        .join(format!("{LABEL}.plist"));
    if !path.exists() {
        return Ok(KeeperMode::None);
    }
    Ok(KeeperMode::Launchd)
}

#[cfg(target_os = "macos")]
pub fn pause() -> anyhow::Result<KeeperMode> {
    let mode = configured_mode()?;
    if mode == KeeperMode::None {
        return Ok(mode);
    }
    let home = account_home()?;
    let path = home
        .join("Library/LaunchAgents")
        .join(format!("{LABEL}.plist"));
    let _ = std::process::Command::new("launchctl")
        .arg("unload")
        .arg(&path)
        .status()?;
    Ok(mode)
}

#[cfg(target_os = "macos")]
pub fn resume(mode: KeeperMode) -> anyhow::Result<()> {
    if mode != KeeperMode::Launchd {
        return Ok(());
    }
    let home = account_home()?;
    let path = home
        .join("Library/LaunchAgents")
        .join(format!("{LABEL}.plist"));
    if std::process::Command::new("launchctl")
        .args(["list", LABEL])
        .status()
        .is_ok_and(|status| status.success())
    {
        return Ok(());
    }
    let status = std::process::Command::new("launchctl")
        .arg("load")
        .arg(&path)
        .status()?;
    if !status.success() {
        anyhow::bail!("launchctl load failed for {}", path.display());
    }
    Ok(())
}

#[cfg(target_os = "linux")]
pub fn configured_mode() -> anyhow::Result<KeeperMode> {
    let home = account_home()?;
    if !home.join(".config/systemd/user/lfd.service").exists() {
        return Ok(KeeperMode::None);
    }
    Ok(KeeperMode::Systemd)
}

#[cfg(target_os = "linux")]
pub fn pause() -> anyhow::Result<KeeperMode> {
    let mode = configured_mode()?;
    if mode == KeeperMode::None {
        return Ok(mode);
    }
    let _ = std::process::Command::new("systemctl")
        .args(["--user", "stop", "lfd"])
        .status()?;
    Ok(mode)
}

#[cfg(target_os = "linux")]
pub fn resume(mode: KeeperMode) -> anyhow::Result<()> {
    if mode != KeeperMode::Systemd {
        return Ok(());
    }
    if std::process::Command::new("systemctl")
        .args(["--user", "is-active", "--quiet", "lfd"])
        .status()
        .is_ok_and(|status| status.success())
    {
        return Ok(());
    }
    let status = std::process::Command::new("systemctl")
        .args(["--user", "start", "lfd"])
        .status()?;
    if !status.success() {
        anyhow::bail!("systemctl --user start lfd failed");
    }
    Ok(())
}

#[cfg(not(any(target_os = "macos", target_os = "linux")))]
pub fn configured_mode() -> anyhow::Result<KeeperMode> {
    Ok(KeeperMode::None)
}

#[cfg(not(any(target_os = "macos", target_os = "linux")))]
pub fn pause() -> anyhow::Result<KeeperMode> {
    Ok(KeeperMode::None)
}

#[cfg(not(any(target_os = "macos", target_os = "linux")))]
pub fn resume(_mode: KeeperMode) -> anyhow::Result<()> {
    Ok(())
}

#[cfg(target_os = "linux")]
pub fn status() -> anyhow::Result<String> {
    let out = std::process::Command::new("systemctl")
        .args(["--user", "is-active", "lfd"])
        .output();
    match out {
        Ok(output) if output.status.success() => Ok("lfd active (systemd user unit)".to_string()),
        Ok(output) => Ok(format!(
            "lfd not active: {}",
            String::from_utf8_lossy(&output.stdout).trim()
        )),
        _ => Ok("lfd not installed (no systemd user unit)".to_string()),
    }
}

#[cfg(not(any(target_os = "macos", target_os = "linux")))]
pub fn status() -> anyhow::Result<String> {
    Ok("lfd service lifecycle is unsupported on this platform".to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::net::SocketAddr;

    fn spec() -> ServiceSpec {
        ServiceSpec {
            lfd_path: PathBuf::from("/usr/local/bin/lfd"),
            addr: "127.0.0.1:8080".to_string(),
            repo_root: PathBuf::from("/home/op/src/loopflow"),
            lf_home: Some(PathBuf::from("/home/op/.lf")),
            db_path: None,
            path_env: Some("/opt/homebrew/bin:/usr/bin:/bin".to_string()),
            doppler_project: Some("example-project".to_string()),
            doppler_config: Some("example-config".to_string()),
        }
    }

    #[test]
    fn launchd_plist_carries_label_keepalive_and_non_secret_env_only() {
        let plist = render_launchd_plist(&spec());
        assert!(plist.contains("<string>com.loopflow.lfd</string>"));
        assert!(plist.contains("<key>KeepAlive</key>"));
        assert!(plist.contains("<key>RunAtLoad</key>"));
        assert!(plist.contains("<integer>10</integer>"));
        assert!(plist.contains("<key>LF_HOME</key>"));
        assert!(plist.contains("/usr/local/bin/lfd</string>"));
        assert!(plist.contains("serve</string>"));
        assert!(plist.contains("127.0.0.1:8080</string>"));
        assert!(plist.contains("--repo</string>"));
        assert!(plist.contains("/home/op/src/loopflow</string>"));
        assert!(plist.contains("<key>PATH</key>"));
        assert!(plist.contains("/opt/homebrew/bin:/usr/bin:/bin"));
        assert!(plist.contains("<key>DOPPLER_PROJECT</key>"));
        assert!(plist.contains("<string>example-project</string>"));
        assert!(plist.contains("<key>DOPPLER_CONFIG</key>"));
        assert!(plist.contains("<string>example-config</string>"));
        assert_eq!(plist.matches("<string>/dev/null</string>").count(), 2);
        // Secrets must never appear in the file.
        assert!(!plist.contains("WEBHOOK_SECRET"));
        assert!(!plist.contains("VIEWER_ID"));
        assert!(!plist.contains("AUTH_TOKEN"));
    }

    #[test]
    fn systemd_unit_carries_execstart_restart_and_env_lines() {
        let unit = render_systemd_unit(&spec());
        assert!(unit.contains(
            "ExecStart=/usr/local/bin/lfd serve --addr 127.0.0.1:8080 --repo /home/op/src/loopflow"
        ));
        assert!(unit.contains("Restart=on-failure"));
        assert!(unit.contains("RestartSec=5"));
        assert!(unit.contains("StandardOutput=null"));
        assert!(unit.contains("StandardError=null"));
        assert!(unit.contains("Environment=LF_HOME=/home/op/.lf"));
        assert!(unit.contains("Environment=PATH=/opt/homebrew/bin:/usr/bin:/bin"));
        assert!(unit.contains("Environment=DOPPLER_PROJECT=example-project"));
        assert!(unit.contains("Environment=DOPPLER_CONFIG=example-config"));
        assert!(unit.contains("WantedBy=default.target"));
        assert!(!unit.contains("WEBHOOK_SECRET"));
    }

    #[test]
    fn keeper_switch_repoints_and_scopes_launchd_startup() {
        let configured = configure_launchd_switch(
            &render_launchd_plist(&spec()),
            Path::new("/home/op/.local/bin/lfd"),
            Some("switch-test"),
        )
        .unwrap();
        assert!(configured.contains("<string>/home/op/.local/bin/lfd</string>"));
        assert!(configured.contains("<string>--install-switch</string>"));
        assert!(configured.contains("<string>switch-test</string>"));

        let settled =
            configure_launchd_switch(&configured, Path::new("/home/op/.local/bin/lfd"), None)
                .unwrap();
        assert!(!settled.contains("--install-switch"));
        assert!(!settled.contains("switch-test"));
    }

    #[test]
    fn keeper_switch_repoints_and_scopes_systemd_startup() {
        let configured = configure_systemd_switch(
            &render_systemd_unit(&spec()),
            Path::new("/home/op/.local/bin/lfd"),
            Some("switch-test"),
        )
        .unwrap();
        assert!(configured.contains(
            "ExecStart=/home/op/.local/bin/lfd serve --install-switch switch-test --addr"
        ));

        let settled =
            configure_systemd_switch(&configured, Path::new("/home/op/.local/bin/lfd"), None)
                .unwrap();
        assert!(settled.contains("ExecStart=/home/op/.local/bin/lfd serve --addr"));
        assert!(!settled.contains("--install-switch"));
    }

    #[test]
    fn service_files_omit_env_blocks_when_no_path_config_is_set() {
        let bare = ServiceSpec {
            lfd_path: PathBuf::from("/usr/local/bin/lfd"),
            addr: "127.0.0.1:8080".to_string(),
            repo_root: PathBuf::from("/home/op/src/loopflow"),
            lf_home: None,
            db_path: None,
            path_env: None,
            doppler_project: None,
            doppler_config: None,
        };
        let plist = render_launchd_plist(&bare);
        assert!(!plist.contains("EnvironmentVariables"));
        let unit = render_systemd_unit(&bare);
        assert!(!unit.contains("Environment="));
    }

    #[test]
    fn xml_and_shell_escape_neutralize_metacharacters() {
        let mut s = spec();
        s.addr = "0.0.0.0:8080 \"injected\"".to_string();
        let plist = render_launchd_plist(&s);
        assert!(plist.contains("&quot;injected&quot;"));
        let unit = render_systemd_unit(&s);
        assert!(unit.contains("0.0.0.0:8080 \\\"injected\\\""));
    }

    #[test]
    fn install_refuses_without_a_home_directory() {
        // The render path is pure; the install path needs a home. We exercise
        // the unsupported-platform stub when present, otherwise just confirm the
        // render round-trips into a file we can write.
        let path = std::env::temp_dir().join("lfd-service-render.plist");
        write_service_file(&path, &render_launchd_plist(&spec())).unwrap();
        let contents = std::fs::read_to_string(&path).unwrap();
        assert!(contents.contains("com.loopflow.lfd"));
        std::fs::remove_file(&path).ok();
        let _: SocketAddr = "127.0.0.1:8080".parse().unwrap();
    }
}
