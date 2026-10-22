//! `lf install gh`: put the GitHub CLI in `~/.local/bin` from GitHub's own
//! release archive, without a package manager.
//!
//! The archive must match the release's published SHA-256 checksum before
//! anything is extracted. An existing `gh` on PATH is left alone. GitHub owns
//! the binary; running this again installs its latest release.

use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::Duration;

use anyhow::{anyhow, bail, Context, Result};
use reqwest::blocking::Client;
use sha2::{Digest, Sha256};

const LATEST_RELEASE: &str = "https://api.github.com/repos/cli/cli/releases/latest";
const RELEASE_DOWNLOADS: &str = "https://github.com/cli/cli/releases/download";

pub fn install_gh() -> Result<()> {
    let home = dirs::home_dir().context("cannot determine the home directory")?;
    if let Some(existing) = crate::os_process::which_on_path(Path::new("gh")) {
        println!(
            "GitHub CLI is already installed: {}\nNext: lf account connect github",
            existing.display()
        );
        return Ok(());
    }
    let client = Client::builder()
        .timeout(Duration::from_secs(120))
        .user_agent(concat!("loopflow/", env!("CARGO_PKG_VERSION")))
        .build()?;
    let tag = client
        .get(LATEST_RELEASE)
        .send()
        .and_then(|response| response.error_for_status())
        .and_then(|response| response.json::<serde_json::Value>())
        .context("find the latest GitHub CLI release")?["tag_name"]
        .as_str()
        .map(str::to_string)
        .ok_or_else(|| anyhow!("the latest GitHub CLI release names no tag"))?;
    let version = tag.trim_start_matches('v');
    let archive_name = archive_name(version)?;
    let download = |name: &str| {
        client
            .get(format!("{RELEASE_DOWNLOADS}/{tag}/{name}"))
            .send()
            .and_then(|response| response.error_for_status())
            .and_then(|response| response.bytes())
            .with_context(|| format!("download {name}"))
    };
    println!("Installing GitHub CLI {version}");
    let checksums = download(&format!("gh_{version}_checksums.txt"))?;
    let archive = download(&archive_name)?;
    let gh = install_release(
        &archive_name,
        &archive,
        &String::from_utf8_lossy(&checksums),
        &home,
    )?;
    println!(
        "GitHub CLI is installed: {}\nNext: lf account connect github",
        gh.display()
    );
    Ok(())
}

/// The release archive for this machine, e.g. `gh_2.62.0_macOS_arm64.zip`.
fn archive_name(version: &str) -> Result<String> {
    let arch = match std::env::consts::ARCH {
        "aarch64" => "arm64",
        "x86_64" => "amd64",
        other => bail!("GitHub CLI publishes no {other} build"),
    };
    Ok(match std::env::consts::OS {
        "macos" => format!("gh_{version}_macOS_{arch}.zip"),
        "linux" => format!("gh_{version}_linux_{arch}.tar.gz"),
        other => bail!("lf installs the GitHub CLI on macOS and Linux, not {other}"),
    })
}

/// Verify `archive` against the release checksums, then place its `bin/gh` in
/// `home/.local/bin` and confirm it runs.
fn install_release(name: &str, archive: &[u8], checksums: &str, home: &Path) -> Result<PathBuf> {
    let expected = checksums
        .lines()
        .find_map(|line| {
            let (digest, file) = line.split_once(char::is_whitespace)?;
            (file.trim() == name).then(|| digest.to_ascii_lowercase())
        })
        .ok_or_else(|| anyhow!("the release checksums do not list {name}"))?;
    if hex::encode(Sha256::digest(archive)) != expected {
        bail!("{name} does not match its published checksum; nothing was installed");
    }
    let work = tempfile::tempdir()?;
    let archive_path = work.path().join(name);
    std::fs::write(&archive_path, archive)?;
    let extracted = work.path().join("extracted");
    std::fs::create_dir(&extracted)?;
    let status = if name.ends_with(".zip") {
        Command::new("/usr/bin/ditto")
            .args(["-x", "-k"])
            .arg(&archive_path)
            .arg(&extracted)
            .status()
    } else {
        Command::new("tar")
            .arg("-xzf")
            .arg(&archive_path)
            .arg("-C")
            .arg(&extracted)
            .status()
    }
    .context("extract the GitHub CLI archive")?;
    if !status.success() {
        bail!("extracting {name} failed ({status})");
    }
    let stem = name.trim_end_matches(".zip").trim_end_matches(".tar.gz");
    let binary = extracted.join(stem).join("bin/gh");
    if !binary.is_file() {
        bail!("{name} has no {stem}/bin/gh");
    }
    let bin = home.join(".local/bin");
    std::fs::create_dir_all(&bin)?;
    let staged = tempfile::NamedTempFile::new_in(&bin)?;
    std::fs::copy(&binary, staged.path())?;
    staged
        .persist(bin.join("gh"))
        .context("place gh in ~/.local/bin")?;
    let gh = bin.join("gh");
    let runs = Command::new(&gh)
        .arg("--version")
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .is_ok_and(|status| status.success());
    if !runs {
        bail!("gh was placed in {}, but it does not run", bin.display());
    }
    Ok(gh)
}

#[cfg(test)]
mod tests {
    use std::os::unix::fs::PermissionsExt;
    use std::process::Command;

    use sha2::{Digest, Sha256};

    use super::{archive_name, install_release};

    fn release(version: &str) -> (String, Vec<u8>) {
        let name = archive_name(version).unwrap();
        let stem = name.trim_end_matches(".zip").trim_end_matches(".tar.gz");
        let root = tempfile::tempdir().unwrap();
        let bin = root.path().join(stem).join("bin");
        std::fs::create_dir_all(&bin).unwrap();
        std::fs::write(bin.join("gh"), "#!/bin/sh\necho 'gh version fixture'\n").unwrap();
        std::fs::set_permissions(bin.join("gh"), std::fs::Permissions::from_mode(0o755)).unwrap();
        let archive = root.path().join(&name);
        let status = if name.ends_with(".zip") {
            Command::new("/usr/bin/ditto")
                .args(["-c", "-k", "--keepParent"])
                .arg(root.path().join(stem))
                .arg(&archive)
                .status()
        } else {
            Command::new("tar")
                .arg("-czf")
                .arg(&archive)
                .arg("-C")
                .arg(root.path())
                .arg(stem)
                .status()
        };
        assert!(status.unwrap().success());
        (name, std::fs::read(archive).unwrap())
    }

    #[test]
    fn verified_release_lands_in_local_bin_and_runs() {
        let home = tempfile::tempdir().unwrap();
        let (name, archive) = release("9.9.9");
        let checksums = format!(
            "{}  other.zip\n{}  {name}\n",
            "0".repeat(64),
            hex::encode(Sha256::digest(&archive))
        );
        let gh = install_release(&name, &archive, &checksums, home.path()).unwrap();
        assert_eq!(gh, home.path().join(".local/bin/gh"));
    }

    #[test]
    fn mismatched_or_unlisted_archives_install_nothing() {
        let home = tempfile::tempdir().unwrap();
        let (name, archive) = release("9.9.9");
        let wrong = format!("{}  {name}\n", "0".repeat(64));
        let error = install_release(&name, &archive, &wrong, home.path()).unwrap_err();
        assert!(error.to_string().contains("does not match"));
        let error = install_release(&name, &archive, "", home.path()).unwrap_err();
        assert!(error.to_string().contains("do not list"));
        assert!(!home.path().join(".local/bin/gh").exists());
    }
}
