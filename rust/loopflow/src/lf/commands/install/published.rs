//! Published updates and their optional machine-local schedule.

use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};
use std::time::Duration;

use anyhow::{anyhow, bail, Context, Result};
use reqwest::blocking::Client;
use sha2::{Digest, Sha256};

use crate::lf::InstallFrequency;

const RELEASES: &str = "https://github.com/loopflowstudio/loopflow/releases";

pub fn latest() -> Result<()> {
    let client = Client::builder().timeout(Duration::from_secs(30)).build()?;
    let response = client
        .head(format!("{RELEASES}/latest"))
        .send()
        .context("look up the latest published Loopflow release")?
        .error_for_status()?;
    let tag = release_tag(response.url().path())?;
    let directory = install_dir()?;
    let applications = std::env::var_os("LF_APPLICATIONS_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("/Applications"));
    let cli_only = std::env::var("LF_INSTALL_CLI_ONLY").as_deref() == Ok("1");
    if is_current(
        &directory,
        &tag,
        (cfg!(target_os = "macos") && !cli_only).then_some(applications.as_path()),
    ) {
        println!("Published release {tag} is already installed.");
        return Ok(());
    }
    let temporary = tempfile::tempdir()?;
    let base = format!("{RELEASES}/download/{tag}");
    let manifest = client
        .get(format!("{base}/SHA256SUMS"))
        .send()?
        .error_for_status()?
        .text()?;
    let installer = client
        .get(format!("{base}/install.sh"))
        .send()?
        .error_for_status()?
        .bytes()?;
    run_verified_installer(&directory, temporary.path(), &tag, &manifest, &installer)?;
    println!(
        "release: {tag}\ninstalled: {}",
        directory.join("lf").display()
    );
    Ok(())
}

fn release_tag(path: &str) -> Result<String> {
    let tag = path
        .strip_prefix("/loopflowstudio/loopflow/releases/tag/")
        .filter(|tag| tag.starts_with('v') && tag.len() > 1 && !tag.contains('/'))
        .ok_or_else(|| anyhow!("latest release did not resolve to a pinned release tag: {path}"))?;
    Ok(tag.to_string())
}

fn install_dir() -> Result<PathBuf> {
    if let Some(directory) = std::env::var_os("LF_INSTALL_DIR").filter(|value| !value.is_empty()) {
        let directory = PathBuf::from(directory);
        let directory = match directory.strip_prefix("~") {
            Ok(relative) => dirs::home_dir()
                .context("cannot determine the installation home")?
                .join(relative),
            Err(_) => directory,
        };
        return Ok(std::env::current_dir()?.join(directory));
    }
    let home = dirs::home_dir().context("cannot determine the installation home")?;
    if let Some(binary) = crate::engine::process::which_on_path(Path::new("lf")) {
        if let Some(parent) = binary.parent() {
            if parent == home.join(".local/bin") || parent == home.join(".lf/bin") {
                return Ok(parent.to_path_buf());
            }
        }
    }
    Ok(home.join(".local/bin"))
}

fn is_current(directory: &Path, tag: &str, applications: Option<&Path>) -> bool {
    let version = tag.trim_start_matches('v');
    if !["lf", "lfd"].iter().all(|name| {
        inspect(&directory.join(name), &["--version"]).is_some_and(|output| {
            output.status.success()
                && String::from_utf8_lossy(&output.stdout).trim() == format!("{name} {version}")
        })
    }) {
        return false;
    }
    let published = inspect(&directory.join("lf"), &["install", "preflight", "--json"])
        .filter(|output| output.status.success())
        .and_then(|output| serde_json::from_slice::<serde_json::Value>(&output.stdout).ok())
        .is_some_and(|preview| {
            preview["candidate"]["authority"] == "published"
                && preview["verdict"]["kind"] == "promote"
        });
    published && applications.is_none_or(|root| has_release_app(root, version))
}

// Installed binaries can be stale or broken; update detection must not hang.
fn inspect(program: &Path, args: &[&str]) -> Option<Output> {
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .ok()?;
    runtime.block_on(async {
        tokio::time::timeout(
            Duration::from_secs(30),
            tokio::process::Command::new(program)
                .args(args)
                .kill_on_drop(true)
                .output(),
        )
        .await
        .ok()?
        .ok()
    })
}

fn has_release_app(applications: &Path, version: &str) -> bool {
    let contents = applications.join("Loopflow.app/Contents");
    let info = inspect(
        Path::new("/usr/bin/plutil"),
        &[
            "-convert",
            "json",
            "-o",
            "-",
            &contents.join("Info.plist").to_string_lossy(),
        ],
    )
    .filter(|output| output.status.success())
    .and_then(|output| serde_json::from_slice::<serde_json::Value>(&output.stdout).ok());
    info.is_some_and(|info| {
        info["CFBundleShortVersionString"] == version && info["CFBundleVersion"] == version
    }) && ["Loopflow", "lf", "lfd"].iter().all(|name| {
        fs::metadata(contents.join("MacOS").join(name))
            .is_ok_and(|metadata| metadata.is_file() && metadata.permissions().mode() & 0o111 != 0)
    })
}

fn run_verified_installer(
    directory: &Path,
    temporary: &Path,
    tag: &str,
    manifest: &str,
    installer: &[u8],
) -> Result<()> {
    let expected = manifest
        .lines()
        .find_map(|line| {
            let mut fields = line.split_whitespace();
            let digest = fields.next()?;
            (fields.next()?.trim_start_matches('*') == "install.sh").then_some(digest)
        })
        .context("published SHA256SUMS does not name install.sh")?;
    let actual = format!("{:x}", Sha256::digest(installer));
    if actual != expected {
        bail!("digest mismatch for install.sh: expected {expected}, downloaded {actual}");
    }
    let path = temporary.join("install.sh");
    fs::write(&path, installer)?;
    let status = Command::new("/bin/sh")
        .arg(path)
        .args(["--version", tag])
        .env("LF_INSTALL_DIR", directory)
        .status()
        .context("start the published installer")?;
    if !status.success() {
        bail!(
            "published installation failed ({status}); fix the error above and rerun `lf install`"
        );
    }
    Ok(())
}

pub fn schedule(frequency: InstallFrequency) -> Result<()> {
    if !cfg!(target_os = "macos") {
        bail!("automatic installation currently uses macOS launchd; run `lf install` to update manually");
    }
    let home = dirs::home_dir().context("cannot determine the installation home")?;
    let binary = install_dir()?.join("lf");
    let logs = home.join("Library/Logs/Loopflow");
    let path = home.join("Library/LaunchAgents/com.loopflow.refresh.plist");
    fs::create_dir_all(&logs)?;
    fs::create_dir_all(path.parent().expect("launch agent has a parent"))?;
    let payload = schedule_plist(&home, &binary, &logs, frequency);
    let uid = Command::new("id")
        .arg("-u")
        .output()
        .context("read launchd user")?;
    if !uid.status.success() {
        bail!("cannot determine launchd user");
    }
    let domain = format!("gui/{}", String::from_utf8_lossy(&uid.stdout).trim());
    let label = format!("{domain}/com.loopflow.refresh");
    let loaded = Command::new("launchctl")
        .args(["print", &label])
        .output()?
        .status
        .success();
    if loaded && fs::read_to_string(&path).is_ok_and(|current| current == payload) {
        println!("Installation already scheduled: {}", path.display());
        return Ok(());
    }
    if loaded
        && !Command::new("launchctl")
            .args(["bootout", &label])
            .status()?
            .success()
    {
        bail!("could not unload the previous installation schedule; rerun `lf install schedule`");
    }
    fs::write(&path, payload)?;
    if !Command::new("launchctl")
        .args(["bootstrap", &domain])
        .arg(&path)
        .status()?
        .success()
    {
        bail!("could not load the installation schedule; rerun `lf install schedule`");
    }
    let cadence = match frequency {
        InstallFrequency::Weekly => "weekly (Monday at 09:00 local time)",
        InstallFrequency::Daily => "daily (09:00 local time)",
        InstallFrequency::Hourly => "hourly",
        InstallFrequency::FiveMinutes => "every five minutes",
    };
    println!(
        "Installation scheduled at login and {cadence}: {}; logs: {}",
        path.display(),
        logs.join("refresh.log").display()
    );
    Ok(())
}

fn schedule_plist(home: &Path, binary: &Path, logs: &Path, frequency: InstallFrequency) -> String {
    let escape = |path: &Path| {
        path.to_string_lossy()
            .replace('&', "&amp;")
            .replace('<', "&lt;")
            .replace('>', "&gt;")
    };
    let binary_path = escape(binary);
    let bin = escape(binary.parent().expect("installation binary has a parent"));
    let home = escape(home);
    let log = escape(&logs.join("refresh.log"));
    let calendar = match frequency {
        InstallFrequency::Weekly => "<dict><key>Weekday</key><integer>1</integer><key>Hour</key><integer>9</integer><key>Minute</key><integer>0</integer></dict>".to_string(),
        InstallFrequency::Daily => "<dict><key>Hour</key><integer>9</integer><key>Minute</key><integer>0</integer></dict>".to_string(),
        InstallFrequency::Hourly => "<dict><key>Minute</key><integer>0</integer></dict>".to_string(),
        InstallFrequency::FiveMinutes => format!(
            "<array>{}</array>",
            (0..60).step_by(5).map(|minute| format!(
                "<dict><key>Minute</key><integer>{minute}</integer></dict>"
            )).collect::<String>()
        ),
    };
    format!(
        r#"<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0"><dict>
<key>Label</key><string>com.loopflow.refresh</string>
<key>ProgramArguments</key><array><string>{binary_path}</string><string>install</string></array>
<key>EnvironmentVariables</key><dict><key>LF_INSTALL_DIR</key><string>{bin}</string><key>PATH</key><string>{bin}:{home}/.cargo/bin:/opt/homebrew/bin:/usr/local/bin:/usr/bin:/bin:/usr/sbin:/sbin</string></dict>
<key>RunAtLoad</key><true/>
<key>StartCalendarInterval</key>{calendar}
<key>StandardOutPath</key><string>{log}</string>
<key>StandardErrorPath</key><string>{log}</string>
</dict></plist>
"#
    )
}

#[cfg(test)]
mod tests {
    use super::{is_current, release_tag, run_verified_installer};
    use sha2::{Digest, Sha256};
    use std::fs;
    use std::os::unix::fs::PermissionsExt;
    use std::path::Path;

    fn binary(path: &Path, name: &str, authority: &str) {
        fs::write(path, format!(
            "#!/bin/sh\nif [ \"$1\" = --version ]; then echo '{name} 9.9.9'; else echo '{{\"candidate\":{{\"authority\":\"{authority}\"}},\"verdict\":{{\"kind\":\"promote\"}}}}'; fi\n"
        )).unwrap();
        fs::set_permissions(path, fs::Permissions::from_mode(0o755)).unwrap();
    }

    #[test]
    fn installed_release_must_have_both_published_binaries() {
        let temp = tempfile::tempdir().unwrap();
        binary(&temp.path().join("lf"), "lf", "published");
        assert!(!is_current(temp.path(), "v9.9.9", None));
        binary(&temp.path().join("lfd"), "lfd", "published");
        assert!(is_current(temp.path(), "v9.9.9", None));
        assert!(!is_current(temp.path(), "v9.9.10", None));
        binary(&temp.path().join("lf"), "lf", "validation_only");
        assert!(!is_current(temp.path(), "v9.9.9", None));
        binary(&temp.path().join("lf"), "lf", "published");
        let script = fs::read_to_string(temp.path().join("lf")).unwrap();
        fs::write(
            temp.path().join("lf"),
            script.replace("\"promote\"", "\"promote_and_migrate\""),
        )
        .unwrap();
        assert!(!is_current(temp.path(), "v9.9.9", None));
    }

    #[test]
    fn installer_verifies_bytes_and_passes_the_pinned_release() {
        let temp = tempfile::tempdir().unwrap();
        let payload = b"#!/bin/sh\nprintf '%s' \"$2\" > \"$LF_INSTALL_DIR/installed\"\n";
        let manifest = format!("{:x}  install.sh\n", Sha256::digest(payload));
        let tag = release_tag("/loopflowstudio/loopflow/releases/tag/v9.9.9").unwrap();
        run_verified_installer(temp.path(), temp.path(), &tag, &manifest, payload).unwrap();
        assert_eq!(
            fs::read_to_string(temp.path().join("installed")).unwrap(),
            "v9.9.9"
        );
        fs::remove_file(temp.path().join("installed")).unwrap();
        let error =
            run_verified_installer(temp.path(), temp.path(), &tag, "00 install.sh", payload)
                .unwrap_err();
        assert!(error.to_string().contains("digest mismatch"));
        assert!(!temp.path().join("installed").exists());
        assert!(release_tag("/loopflowstudio/loopflow/releases/latest").is_err());
    }

    #[test]
    fn failed_installer_remains_a_failure() {
        let temp = tempfile::tempdir().unwrap();
        let payload = b"#!/bin/sh\nexit 23\n";
        let manifest = format!("{:x} *install.sh\n", Sha256::digest(payload));
        let error = run_verified_installer(temp.path(), temp.path(), "v9.9.9", &manifest, payload)
            .unwrap_err();
        assert!(error.to_string().contains("published installation failed"));
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn current_cli_does_not_hide_a_missing_stale_or_incomplete_app() {
        let temp = tempfile::tempdir().unwrap();
        binary(&temp.path().join("lf"), "lf", "published");
        binary(&temp.path().join("lfd"), "lfd", "published");
        let applications = temp.path().join("Applications");
        let current = || is_current(temp.path(), "v9.9.9", Some(&applications));
        assert!(!current());
        let contents = applications.join("Loopflow.app/Contents");
        fs::create_dir_all(contents.join("MacOS")).unwrap();
        for name in ["Loopflow", "lf", "lfd"] {
            binary(&contents.join("MacOS").join(name), name, "published");
        }
        for (version, expected) in [("9.9.8", false), ("9.9.9", true)] {
            fs::write(contents.join("Info.plist"), format!(
                "<plist version=\"1.0\"><dict><key>CFBundleShortVersionString</key><string>{version}</string><key>CFBundleVersion</key><string>{version}</string></dict></plist>"
            )).unwrap();
            assert_eq!(current(), expected);
        }
        fs::remove_file(contents.join("MacOS/Loopflow")).unwrap();
        assert!(!current());
        fs::write(contents.join("Info.plist"), "invalid plist").unwrap();
        assert!(!current());
    }
}
