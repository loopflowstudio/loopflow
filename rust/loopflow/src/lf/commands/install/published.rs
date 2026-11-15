//! Stream installs (green main builds and published releases) and the
//! machine-local schedule that follows the recorded stream.

use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};
use std::time::Duration;

use anyhow::{anyhow, bail, Context, Result};
use reqwest::blocking::Client;
use sha2::{Digest, Sha256};

use clap::ValueEnum;

use crate::lf::InstallFrequency;

const REPOSITORY: &str = "loopflowstudio/loopflow";
const RELEASES: &str = "https://github.com/loopflowstudio/loopflow/releases";
/// Packages every green main commit (`.github/workflows/main-packages.yml`).
const MAIN_PACKAGES: &str = "main-packages.yml";
/// The artifact holding that run's signed, notarized Loopflow.dmg.
const MAIN_APP: &str = "Loopflow-dmg";

/// The build line an install follows (`release/README.md`, Streams).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Stream {
    /// The newest green main build.
    Dev,
    /// The newest release.
    Preview,
    /// GitHub's Latest release: the newest one soaked with no open regression.
    Stable,
}

impl Stream {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Dev => "dev",
            Self::Preview => "preview",
            Self::Stable => "stable",
        }
    }

    fn parse(name: &str) -> Option<Self> {
        [Self::Dev, Self::Preview, Self::Stable]
            .into_iter()
            .find(|stream| stream.as_str() == name)
    }
}

/// Install the newest build on `switch`, recording it as this Machine's
/// stream, or on the recorded stream (`stable` when none is recorded).
pub fn install(switch: Option<Stream>) -> Result<()> {
    let home = crate::installation::account_home()?.join(".lf");
    let stream = match switch {
        Some(stream) => {
            record_stream(&home, stream)?;
            println!(
                "stream: {} (recorded in {})",
                stream.as_str(),
                home.join("config.yaml").display()
            );
            stream
        }
        None => recorded_stream(&home)?.unwrap_or(Stream::Stable),
    };
    super::recover_switch(None)?;
    let directory = install_dir()?;
    let app = app_destination(&directory.join("lf"));
    if stream == Stream::Dev {
        main_build(&directory, app.as_deref())?;
    } else {
        let client = http_client()?;
        let tag = stream_release(&client, stream)?;
        install_release(&client, stream, &tag, &directory, app.as_deref())?;
    }
    schedule(None)
}

/// The release a release stream is at: preview's newest, or stable's GitHub Latest.
fn stream_release(client: &Client, stream: Stream) -> Result<String> {
    if stream == Stream::Preview {
        let releases = client
            .get(format!(
                "https://api.github.com/repos/{REPOSITORY}/releases?per_page=20"
            ))
            .send()
            .context("look up the newest Loopflow release")?
            .error_for_status()?
            .text()?;
        return newest_release(&releases);
    }
    let latest = client
        .head(format!("{RELEASES}/latest"))
        .send()
        .context("look up the stable Loopflow release")?
        .error_for_status()?;
    release_tag(latest.url().path())
}

fn install_release(
    client: &Client,
    stream: Stream,
    tag: &str,
    directory: &Path,
    app: Option<&Path>,
) -> Result<()> {
    let stream = stream.as_str();
    if is_current(directory, tag, app) {
        // Matching files can still have an interrupted receipt or a stale gate.
        // Let the same promotion owner reconcile them before reporting success.
        promote_build(&directory.join("lf"), directory, app.map(|app| (app, app)))?;
        println!("{stream} {tag} is already installed.");
        return Ok(());
    }
    let manifest = release_manifest(client, tag)?;
    let installer = release_asset(client, tag, "install.sh")?;
    let temporary = tempfile::tempdir()?;
    run_verified_installer(directory, app, temporary.path(), tag, &manifest, &installer)?;
    println!(
        "{stream}: {tag}\ninstalled: {}",
        directory.join("lf").display()
    );
    Ok(())
}

/// Record the installed Loopflow.app, which updated itself in place (Sparkle), as
/// this Machine's install: migrate the store and link `lf` into it, moving no
/// files. The app's first launch runs this through its own bundled `lf`.
pub fn adopt() -> Result<()> {
    let current = fs::canonicalize(std::env::current_exe()?)?;
    let app = super::installed_app(&current);
    if fs::canonicalize(app.join("Contents/MacOS/lf")).ok() != Some(current.clone()) {
        println!(
            "{} is not {}'s lf; nothing to adopt",
            current.display(),
            app.display()
        );
        return Ok(());
    }
    super::promote(
        super::PromotionArtifacts {
            cli_target: &install_dir()?.join("lf"),
            app_source: Some(&app),
            app_target: Some(&app),
            legacy_app_target: None,
        },
        true,
        false,
    )?;
    schedule(None)
}

/// CLI-only omits initial Desktop setup; an existing app stays in the installation.
fn app_destination(cli: &Path) -> Option<PathBuf> {
    let installed = super::installed_app(cli);
    let app = std::env::var_os("LF_APPLICATIONS_DIR")
        .filter(|value| !value.is_empty())
        .map(|directory| PathBuf::from(directory).join("Loopflow.app"))
        .unwrap_or_else(|| installed.clone());
    (cfg!(target_os = "macos")
        && (std::env::var("LF_INSTALL_CLI_ONLY").as_deref() != Ok("1")
            || installed.exists()
            || app.exists()))
    .then_some(app)
}

fn http_client() -> Result<Client> {
    Ok(Client::builder()
        .timeout(Duration::from_secs(30))
        .user_agent(concat!("lf/", env!("CARGO_PKG_VERSION")))
        .build()?)
}

fn release_asset(client: &Client, tag: &str, name: &str) -> Result<Vec<u8>> {
    Ok(client
        .get(format!("{RELEASES}/download/{tag}/{name}"))
        .send()?
        .error_for_status()?
        .bytes()?
        .to_vec())
}

/// Release `tag`'s `SHA256SUMS`, which names the digest of every asset.
fn release_manifest(client: &Client, tag: &str) -> Result<String> {
    Ok(String::from_utf8(release_asset(
        client,
        tag,
        "SHA256SUMS",
    )?)?)
}

/// The newest published release in the GitHub releases list, which is newest first.
fn newest_release(json: &str) -> Result<String> {
    #[derive(serde::Deserialize)]
    struct Release {
        tag_name: String,
        draft: bool,
        prerelease: bool,
    }
    let releases: Vec<Release> = serde_json::from_str(json).context("read the release list")?;
    releases
        .into_iter()
        .find(|release| !release.draft && !release.prerelease)
        .map(|release| release.tag_name)
        .ok_or_else(|| anyhow!("no published Loopflow release"))
}

/// The stream recorded in `home`'s config.yaml.
fn recorded_stream(home: &Path) -> Result<Option<Stream>> {
    let config = home.join("config.yaml");
    let Some(name) = crate::config::files::read(&config)?
        .get("stream")
        .and_then(|value| value.as_str().map(str::to_string))
    else {
        return Ok(None);
    };
    Stream::parse(&name).map(Some).ok_or_else(|| {
        anyhow!(
            "{}: unknown stream `{name}`; choose one with `lf install --dev`, `--preview` or `--stable`",
            config.display()
        )
    })
}

fn record_stream(home: &Path, stream: Stream) -> Result<()> {
    crate::config::files::update(&home.join("config.yaml"), |config| {
        config["stream"] = stream.as_str().into();
        Ok(())
    })
}

/// Install the package CI built for the newest green main commit, with its
/// Loopflow.app on a Mac. GitHub keeps those artifacts behind authentication,
/// so this uses the `gh` login.
fn main_build(directory: &Path, app: Option<&Path>) -> Result<()> {
    let target =
        package_target().ok_or_else(|| anyhow!("no main package is built for this platform"))?;
    let runs = gh(&[
        "run",
        "list",
        "--repo",
        REPOSITORY,
        "--workflow",
        MAIN_PACKAGES,
        "--branch",
        "main",
        "--status",
        "success",
        "--limit",
        "1",
        "--json",
        "databaseId,headSha",
    ])?;
    let (run, revision) = newest_run(&runs)?;
    let temporary = tempfile::tempdir()?;
    download_artifact(run, &format!("lf-{target}"), temporary.path())?;
    let archive = temporary.path().join(format!("lf-{target}.tar.gz"));
    let status = Command::new("tar")
        .arg("-xzf")
        .arg(&archive)
        .arg("-C")
        .arg(temporary.path())
        .status()
        .context("unpack the main package")?;
    if !status.success() {
        bail!("could not unpack {} ({status})", archive.display());
    }
    let candidate = temporary.path().join("lf");
    // Mounted until promotion has copied the app out of it.
    let image = app
        .map(|_| DiskImage::download(run, temporary.path()))
        .transpose()?;
    let source = image.as_ref().map(DiskImage::app);
    promote_build(&candidate, directory, source.as_deref().zip(app))?;
    println!(
        "main: {revision}\ninstalled: {}",
        directory.join("lf").display()
    );
    Ok(())
}

fn promote_build(candidate: &Path, directory: &Path, app: Option<(&Path, &Path)>) -> Result<()> {
    let mut promote = Command::new(candidate);
    super::isolate_candidate_command(&mut promote);
    promote
        .args(["install", "promote", "--cli-target"])
        .arg(directory.join("lf"))
        .arg("--sync-skills");
    if let Some((source, target)) = app {
        promote
            .arg("--app-source")
            .arg(source)
            .arg("--app-target")
            .arg(target)
            .arg("--legacy-app-target")
            .arg(target.with_file_name("Concerto.app"));
    }
    let status = promote.status().context("start installation promotion")?;
    if !status.success() {
        bail!(
            "installation promotion failed ({status}); fix the error above and rerun `lf install`"
        );
    }
    Ok(())
}

/// The signed, notarized Loopflow.dmg a main packages run built, mounted
/// read-only and checked the way `install.sh` checks a release's.
struct DiskImage {
    mount: PathBuf,
}

impl DiskImage {
    fn download(run: u64, temporary: &Path) -> Result<Self> {
        download_artifact(run, MAIN_APP, temporary)?;
        Self::mount(&temporary.join("Loopflow.dmg"), &temporary.join("mount"))
    }

    fn mount(dmg: &Path, mount: &Path) -> Result<Self> {
        fs::create_dir(mount)?;
        checked(Command::new("/usr/bin/hdiutil").arg("verify").arg(dmg))?;
        checked(
            Command::new("/usr/bin/hdiutil")
                .args(["attach", "-nobrowse", "-readonly", "-mountpoint"])
                .arg(mount)
                .arg(dmg),
        )?;
        let image = Self {
            mount: mount.to_path_buf(),
        };
        let app = image.app();
        if !app.is_dir() {
            bail!("{} does not contain Loopflow.app", dmg.display());
        }
        checked(
            Command::new("/usr/bin/codesign")
                .args(["--verify", "--deep", "--strict"])
                .arg(&app),
        )?;
        checked(
            Command::new("/usr/sbin/spctl")
                .args(["--assess", "--type", "execute"])
                .arg(&app),
        )?;
        Ok(image)
    }

    fn app(&self) -> PathBuf {
        self.mount.join("Loopflow.app")
    }
}

impl Drop for DiskImage {
    fn drop(&mut self) {
        let _ = Command::new("/usr/bin/hdiutil")
            .args(["detach", "-quiet"])
            .arg(&self.mount)
            .status();
    }
}

fn checked(command: &mut Command) -> Result<()> {
    let output = command
        .output()
        .with_context(|| format!("start {:?}", command.get_program()))?;
    if !output.status.success() {
        bail!(
            "{:?} failed ({}): {}",
            command.get_program(),
            output.status,
            String::from_utf8_lossy(&output.stderr).trim()
        );
    }
    Ok(())
}

/// The package name CI gives this platform's `lf`.
fn package_target() -> Option<&'static str> {
    match (std::env::consts::OS, std::env::consts::ARCH) {
        ("macos", "aarch64") => Some("aarch64-apple-darwin"),
        ("macos", "x86_64") => Some("x86_64-apple-darwin"),
        ("linux", "x86_64") => Some("x86_64-unknown-linux-gnu"),
        ("linux", "aarch64") => Some("aarch64-unknown-linux-gnu"),
        _ => None,
    }
}

fn newest_run(json: &str) -> Result<(u64, String)> {
    #[derive(serde::Deserialize)]
    #[serde(rename_all = "camelCase")]
    struct Run {
        database_id: u64,
        head_sha: String,
    }
    let runs: Vec<Run> = serde_json::from_str(json).context("read the main package runs")?;
    let run = runs
        .into_iter()
        .next()
        .ok_or_else(|| anyhow!("no green main build has packages yet"))?;
    Ok((run.database_id, run.head_sha))
}

fn download_artifact(run: u64, name: &str, directory: &Path) -> Result<()> {
    gh(&[
        "run",
        "download",
        &run.to_string(),
        "--repo",
        REPOSITORY,
        "--name",
        name,
        "--dir",
        &directory.to_string_lossy(),
    ])
    .map(drop)
}

fn gh(args: &[&str]) -> Result<String> {
    let output = Command::new("gh").args(args).output().context(
        "`lf install --dev` downloads CI packages with `gh`; install it and run `gh auth login`",
    )?;
    if !output.status.success() {
        bail!(
            "gh {} failed: {}",
            args[..2].join(" "),
            String::from_utf8_lossy(&output.stderr).trim()
        );
    }
    Ok(String::from_utf8(output.stdout)?)
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
    if let Some(binary) = crate::os_process::which_on_path(Path::new("lf")) {
        if let Some(parent) = binary.parent() {
            if parent == home.join(".local/bin") || parent == home.join(".lf/bin") {
                return Ok(parent.to_path_buf());
            }
        }
    }
    Ok(home.join(".local/bin"))
}

fn is_current(directory: &Path, tag: &str, app: Option<&Path>) -> bool {
    let version = tag.trim_start_matches('v');
    let Ok(lf) = fs::canonicalize(directory.join("lf")) else {
        return false;
    };
    // The public entry must name the bundled helper. Once paired, one preflight
    // checks their shared build and store compatibility; promotion reconciles receipts.
    if app.is_some_and(|app| !has_release_app(app, version, &lf)) {
        return false;
    }
    inspect(&lf, &["--version"]).is_some_and(|output| {
        output.status.success()
            && String::from_utf8_lossy(&output.stdout).trim() == format!("lf {version}")
    }) && inspect(&lf, &["install", "preflight", "--json"])
        .filter(|output| output.status.success())
        .and_then(|output| serde_json::from_slice::<super::BinaryPreflight>(&output.stdout).ok())
        .is_some_and(|preflight| preflight.verdict == super::Verdict::Promote)
}

// Installed binaries can be stale or broken; update detection must not hang.
fn inspect(program: &Path, args: &[&str]) -> Option<Output> {
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .ok()?;
    runtime.block_on(async {
        let mut command = Command::new(program);
        super::isolate_candidate_command(&mut command);
        command.args(args);
        tokio::time::timeout(
            Duration::from_secs(30),
            tokio::process::Command::from(command)
                .kill_on_drop(true)
                .output(),
        )
        .await
        .ok()?
        .ok()
    })
}

pub(super) fn app_info(app: &Path) -> Option<serde_json::Value> {
    inspect(
        Path::new("/usr/bin/plutil"),
        &[
            "-convert",
            "json",
            "-o",
            "-",
            &app.join("Contents/Info.plist").to_string_lossy(),
        ],
    )
    .filter(|output| output.status.success())
    .and_then(|output| serde_json::from_slice(&output.stdout).ok())
}

fn has_release_app(app: &Path, version: &str, resolved_cli: &Path) -> bool {
    let helper = app.join("Contents/MacOS/lf");
    app_info(app).is_some_and(|info| {
        info["CFBundleShortVersionString"] == version && info["CFBundleVersion"] == version
    }) && ["Loopflow", "lf"].iter().all(|name| {
        fs::metadata(app.join("Contents/MacOS").join(name))
            .is_ok_and(|metadata| metadata.is_file() && metadata.permissions().mode() & 0o111 != 0)
    }) && fs::canonicalize(&helper).is_ok_and(|path| path == resolved_cli)
}

fn manifest_digest<'a>(manifest: &'a str, name: &str) -> Option<&'a str> {
    manifest.lines().find_map(|line| {
        let mut fields = line.split_whitespace();
        let digest = fields.next()?;
        (fields.next()?.trim_start_matches('*') == name).then_some(digest)
    })
}

fn verify_asset(manifest: &str, name: &str, bytes: &[u8]) -> Result<()> {
    let expected = manifest_digest(manifest, name)
        .with_context(|| format!("published SHA256SUMS does not name {name}"))?;
    let actual = format!("{:x}", Sha256::digest(bytes));
    if actual != expected {
        bail!("digest mismatch for {name}: expected {expected}, downloaded {actual}");
    }
    Ok(())
}

fn run_verified_installer(
    directory: &Path,
    app: Option<&Path>,
    temporary: &Path,
    tag: &str,
    manifest: &str,
    installer: &[u8],
) -> Result<()> {
    verify_asset(manifest, "install.sh", installer)?;
    let path = temporary.join("install.sh");
    fs::write(&path, installer)?;
    let mut command = Command::new("/bin/sh");
    command
        .arg(path)
        .args(["--version", tag])
        .env("LF_INSTALL_DIR", directory)
        .env("LF_INSTALL_CLI_ONLY", if app.is_some() { "0" } else { "1" });
    if let Some(app) = app {
        command.env(
            "LF_APPLICATIONS_DIR",
            app.parent().context("app destination has no parent")?,
        );
    }
    let status = command.status().context("start the published installer")?;
    if !status.success() {
        bail!(
            "published installation failed ({status}); fix the error above and rerun `lf install`"
        );
    }
    Ok(())
}

/// Run `lf install` on the recorded stream on a schedule: `frequency`, else the
/// installed schedule's own, else daily. Every install sets it, so a Machine
/// updates itself without being asked.
pub fn schedule(frequency: Option<InstallFrequency>) -> Result<()> {
    let home = dirs::home_dir().context("cannot determine the installation home")?;
    // launchd and cron schedule for the whole account; a disposable HOME
    // (release smoke tests) must not replace the account's job.
    if home != crate::installation::account_home()?
        || crate::store::lf_home_dir() != home.join(".lf")
    {
        println!(
            "Not scheduling installation or cleanup for disposable home {}",
            home.display()
        );
        return Ok(());
    }
    let binary = install_dir()?.join("lf");
    let logs = crate::store::lf_home_dir().join("logs");
    fs::create_dir_all(&logs)?;
    if cfg!(target_os = "macos") {
        launchd_schedule(&home, &binary, &logs, frequency)?;
        launchd_cleanup(&home, &binary, &logs)?;
        launchd_renewal(&home, &binary, &logs)
    } else if cfg!(target_os = "linux") {
        cron_schedule(&home, &binary, &logs, frequency)?;
        cron_renewal(&home, &binary, &logs)
    } else {
        bail!("automatic cleanup needs launchd on macOS or cron on Linux")
    }
}

/// Rental Claude logins renew every 2 hours: an access token lasts about 8,
/// and is resent once within 2 of expiry.
const RENEWAL_SECONDS: u32 = 2 * 60 * 60;
const RENEWAL_LABEL: &str = "com.loopflow.rental-renewal";
const RENEWAL_CRON_MARKER: &str = "# loopflow rental renewal";

/// The frequency whose rendered schedule is already installed, else daily.
fn installed_frequency(installed: impl Fn(InstallFrequency) -> bool) -> InstallFrequency {
    InstallFrequency::value_variants()
        .iter()
        .copied()
        .find(|frequency| installed(*frequency))
        .unwrap_or(InstallFrequency::Daily)
}

fn cadence(frequency: InstallFrequency) -> &'static str {
    match frequency {
        InstallFrequency::Weekly => "weekly (Monday at 09:00 local time)",
        InstallFrequency::Daily => "daily (09:00 local time)",
        InstallFrequency::Hourly => "hourly",
        InstallFrequency::FiveMinutes => "every five minutes",
    }
}

fn launchd_schedule(
    home: &Path,
    binary: &Path,
    logs: &Path,
    frequency: Option<InstallFrequency>,
) -> Result<()> {
    let path = home.join("Library/LaunchAgents/com.loopflow.refresh.plist");
    let current = fs::read_to_string(&path).ok();
    let frequency = frequency.unwrap_or_else(|| {
        installed_frequency(|frequency| {
            current.as_deref().is_some_and(|current| {
                current.contains(&format!(
                    "<key>StartCalendarInterval</key>{}",
                    schedule_calendar(frequency)
                ))
            })
        })
    });
    let payload = schedule_plist(home, binary, logs, frequency);
    if !install_launchd(home, "com.loopflow.refresh", &payload)? {
        println!("Installation already scheduled: {}", path.display());
        return Ok(());
    }
    println!(
        "Installation scheduled at login and {}: {}; logs: {}",
        cadence(frequency),
        path.display(),
        logs.join("refresh.log").display()
    );
    Ok(())
}

/// Write and (re)load the LaunchAgent `name` unless `payload` is already loaded;
/// `true` when it changed.
fn install_launchd(home: &Path, name: &str, payload: &str) -> Result<bool> {
    let path = home.join(format!("Library/LaunchAgents/{name}.plist"));
    fs::create_dir_all(path.parent().expect("launch agent has a parent"))?;
    let current = fs::read_to_string(&path).ok();
    let uid = Command::new("id")
        .arg("-u")
        .output()
        .context("read launchd user")?;
    if !uid.status.success() {
        bail!("cannot determine launchd user");
    }
    let domain = format!("gui/{}", String::from_utf8_lossy(&uid.stdout).trim());
    let label = format!("{domain}/{name}");
    let loaded = Command::new("launchctl")
        .args(["print", &label])
        .output()?
        .status
        .success();
    if loaded && current.as_deref() == Some(payload) {
        return Ok(false);
    }
    if loaded
        && !Command::new("launchctl")
            .args(["bootout", &label])
            .status()?
            .success()
    {
        bail!("could not unload the previous {name} schedule; rerun `lf install schedule`");
    }
    fs::write(&path, payload)?;
    if !Command::new("launchctl")
        .args(["bootstrap", &domain])
        .arg(&path)
        .status()?
        .success()
    {
        bail!("could not load the {name} schedule; rerun `lf install schedule`");
    }
    Ok(true)
}

/// `lf task retire` every minute, separate from the update schedule.
fn launchd_cleanup(home: &Path, binary: &Path, logs: &Path) -> Result<()> {
    let payload = cleanup_plist(home, binary, logs);
    if install_launchd(home, "com.loopflow.cleanup", &payload)? {
        println!(
            "Cleanup scheduled every minute; logs: {}",
            logs.join("workspace-retirement.log").display()
        );
    }
    Ok(())
}

/// `lf machine credentials renew` every 2 hours. The rental logins are files
/// under `~/.lf/accounts`, so the job reads no Keychain item.
fn launchd_renewal(home: &Path, binary: &Path, logs: &Path) -> Result<()> {
    let payload = renewal_plist(home, binary, logs);
    if install_launchd(home, RENEWAL_LABEL, &payload)? {
        println!(
            "Rental logins renew every 2 hours; logs: {}",
            logs.join("rental-renewal.log").display()
        );
    }
    Ok(())
}

fn renewal_plist(home: &Path, binary: &Path, logs: &Path) -> String {
    launch_agent_plist(
        RENEWAL_LABEL,
        home,
        binary,
        &["machine", "credentials", "renew"],
        &logs.join("rental-renewal.log"),
        &format!("<key>StartInterval</key><integer>{RENEWAL_SECONDS}</integer>"),
    )
}

const CLEANUP_CRON_MARKER: &str = "# loopflow cleanup schedule";

fn cleanup_plist(home: &Path, binary: &Path, logs: &Path) -> String {
    let binary = plist_path(binary);
    let machine = plist_path(&home.join(".lf"));
    let log = plist_path(&logs.join("workspace-retirement.log"));
    let home = plist_path(home);
    format!(
        r#"<?xml version="1.0" encoding="UTF-8"?>
<plist version="1.0"><dict>
<key>Label</key><string>com.loopflow.cleanup</string>
<key>ProgramArguments</key><array><string>{binary}</string><string>task</string><string>retire</string></array>
<key>EnvironmentVariables</key><dict><key>LF_HOME</key><string>{machine}</string><key>HOME</key><string>{home}</string><key>PATH</key><string>/opt/homebrew/bin:/usr/local/bin:/usr/bin:/bin:/usr/sbin:/sbin</string></dict>
<key>WorkingDirectory</key><string>{machine}</string>
<key>RunAtLoad</key><true/>
<key>StartInterval</key><integer>60</integer>
<key>StandardOutPath</key><string>{log}</string><key>StandardErrorPath</key><string>{log}</string>
</dict></plist>
"#
    )
}

fn cleanup_cron_line(home: &Path, binary: &Path, logs: &Path) -> String {
    format!(
        "* * * * * cd {} && LF_HOME={} {} task retire >> {} 2>&1 {CLEANUP_CRON_MARKER}",
        cron_quote(&home.join(".lf").to_string_lossy()),
        cron_quote(&home.join(".lf").to_string_lossy()),
        cron_quote(&binary.to_string_lossy()),
        cron_quote(&logs.join("workspace-retirement.log").to_string_lossy())
    )
}

/// Marks the one crontab line `lf install schedule` owns.
const CRON_MARKER: &str = "# loopflow install schedule";

fn cron_schedule(
    home: &Path,
    binary: &Path,
    logs: &Path,
    frequency: Option<InstallFrequency>,
) -> Result<()> {
    let current = read_crontab()?;
    let frequency = frequency.unwrap_or_else(|| {
        installed_frequency(|frequency| {
            let line = cron_line(home, binary, logs, frequency);
            current.lines().any(|entry| {
                entry.ends_with(CRON_MARKER)
                    && entry
                        .split_whitespace()
                        .take(5)
                        .eq(line.split_whitespace().take(5))
            })
        })
    });
    let table = cron_table(
        &cron_table(
            &current,
            CRON_MARKER,
            &cron_line(home, binary, logs, frequency),
        ),
        CLEANUP_CRON_MARKER,
        &cleanup_cron_line(home, binary, logs),
    );
    if table == current {
        println!("Installation and cleanup already scheduled in your crontab");
        return Ok(());
    }
    write_crontab(&table)?;
    println!(
        "Installation scheduled {} in your crontab; logs: {}",
        cadence(frequency),
        logs.join("refresh.log").display()
    );
    Ok(())
}

fn read_crontab() -> Result<String> {
    let listed = Command::new("crontab").arg("-l").output().context(
        "automatic installation and cleanup need cron; install it and rerun `lf install schedule`",
    )?;
    // `crontab -l` fails when the user has no crontab yet.
    Ok(if listed.status.success() {
        String::from_utf8(listed.stdout)?
    } else {
        String::new()
    })
}

fn write_crontab(table: &str) -> Result<()> {
    let mut child = Command::new("crontab")
        .arg("-")
        .stdin(std::process::Stdio::piped())
        .spawn()
        .context("start crontab")?;
    std::io::Write::write_all(
        child.stdin.as_mut().expect("crontab stdin is piped"),
        table.as_bytes(),
    )?;
    if !child.wait()?.success() {
        bail!("could not install the schedules; rerun `lf install schedule`");
    }
    Ok(())
}

fn cron_renewal(home: &Path, binary: &Path, logs: &Path) -> Result<()> {
    let current = read_crontab()?;
    let table = cron_table(
        &current,
        RENEWAL_CRON_MARKER,
        &renewal_cron_line(home, binary, logs),
    );
    if table != current {
        write_crontab(&table)?;
        println!(
            "Rental logins renew every 2 hours in your crontab; logs: {}",
            logs.join("rental-renewal.log").display()
        );
    }
    Ok(())
}

fn renewal_cron_line(home: &Path, binary: &Path, logs: &Path) -> String {
    cron_entry(
        "0 */2 * * *",
        home,
        binary,
        "machine credentials renew",
        &logs.join("rental-renewal.log"),
        RENEWAL_CRON_MARKER,
    )
}

fn cron_line(home: &Path, binary: &Path, logs: &Path, frequency: InstallFrequency) -> String {
    let when = match frequency {
        InstallFrequency::Weekly => "0 9 * * 1",
        InstallFrequency::Daily => "0 9 * * *",
        InstallFrequency::Hourly => "0 * * * *",
        InstallFrequency::FiveMinutes => "*/5 * * * *",
    };
    cron_entry(
        when,
        home,
        binary,
        "install",
        &logs.join("refresh.log"),
        CRON_MARKER,
    )
}

/// One crontab line running the installed `lf <command>` with its install
/// directory and PATH, appending to `log`, owned by `marker`.
fn cron_entry(
    when: &str,
    home: &Path,
    binary: &Path,
    command: &str,
    log: &Path,
    marker: &str,
) -> String {
    let bin = binary.parent().expect("installation binary has a parent");
    format!(
        "{when} LF_INSTALL_DIR={} PATH={} {} {command} >> {} 2>&1 {marker}",
        cron_quote(&bin.to_string_lossy()),
        cron_quote(&format!(
            "{}:{}/.cargo/bin:/usr/local/bin:/usr/bin:/bin",
            bin.display(),
            home.display()
        )),
        cron_quote(&binary.to_string_lossy()),
        cron_quote(&log.to_string_lossy())
    )
}

fn cron_quote(value: &str) -> String {
    // Cron consumes percent escapes before the shell parses its quoted words.
    format!("'{}'", value.replace('\'', "'\"'\"'")).replace('%', "\\%")
}

/// `current` with its `marker` line replaced by `line`; other entries stay.
fn cron_table(current: &str, marker: &str, line: &str) -> String {
    let mut table: String = current
        .lines()
        .filter(|entry| !entry.ends_with(marker))
        .map(|entry| format!("{entry}\n"))
        .collect();
    table.push_str(line);
    table.push('\n');
    table
}

fn schedule_calendar(frequency: InstallFrequency) -> String {
    match frequency {
        InstallFrequency::Weekly => "<dict><key>Weekday</key><integer>1</integer><key>Hour</key><integer>9</integer><key>Minute</key><integer>0</integer></dict>".to_string(),
        InstallFrequency::Daily => "<dict><key>Hour</key><integer>9</integer><key>Minute</key><integer>0</integer></dict>".to_string(),
        InstallFrequency::Hourly => "<dict><key>Minute</key><integer>0</integer></dict>".to_string(),
        InstallFrequency::FiveMinutes => format!(
            "<array>{}</array>",
            (0..60).step_by(5).map(|minute| format!(
                "<dict><key>Minute</key><integer>{minute}</integer></dict>"
            )).collect::<String>()
        ),
    }
}

fn plist_path(path: &Path) -> String {
    path.to_string_lossy()
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
}

fn schedule_plist(home: &Path, binary: &Path, logs: &Path, frequency: InstallFrequency) -> String {
    launch_agent_plist(
        "com.loopflow.refresh",
        home,
        binary,
        &["install"],
        &logs.join("refresh.log"),
        &format!(
            "<key>RunAtLoad</key><true/>\n<key>StartCalendarInterval</key>{}",
            schedule_calendar(frequency)
        ),
    )
}

/// A LaunchAgent running the installed `lf args…` with its install directory
/// and PATH, logging to `log`; `timing` holds its schedule keys.
fn launch_agent_plist(
    label: &str,
    home: &Path,
    binary: &Path,
    args: &[&str],
    log: &Path,
    timing: &str,
) -> String {
    let program: String = std::iter::once(plist_path(binary))
        .chain(args.iter().map(|arg| arg.to_string()))
        .map(|arg| format!("<string>{arg}</string>"))
        .collect();
    let bin = plist_path(binary.parent().expect("installation binary has a parent"));
    let home = plist_path(home);
    let log = plist_path(log);
    format!(
        r#"<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0"><dict>
<key>Label</key><string>{label}</string>
<key>ProgramArguments</key><array>{program}</array>
<key>EnvironmentVariables</key><dict><key>LF_INSTALL_DIR</key><string>{bin}</string><key>PATH</key><string>{bin}:{home}/.cargo/bin:/opt/homebrew/bin:/usr/local/bin:/usr/bin:/bin:/usr/sbin:/sbin</string></dict>
{timing}
<key>StandardOutPath</key><string>{log}</string>
<key>StandardErrorPath</key><string>{log}</string>
</dict></plist>
"#
    )
}

#[cfg(test)]
mod tests {
    #[cfg(target_os = "macos")]
    use super::{checked, launchd_schedule, DiskImage};
    use super::{
        cron_table, is_current, newest_release, newest_run, package_target, record_stream,
        recorded_stream, release_tag, run_verified_installer, InstallFrequency, Stream,
        CRON_MARKER,
    };
    use sha2::{Digest, Sha256};
    use std::fs;
    use std::os::unix::fs::PermissionsExt;
    use std::path::Path;
    #[cfg(target_os = "macos")]
    use std::process::Command;

    fn binary(path: &Path, name: &str) {
        let candidate =
            serde_json::to_string(&crate::lf::commands::install::CandidateIdentity::current())
                .unwrap();
        fs::write(path, format!(
            "#!/bin/sh\ncase \"$*\" in\n  --version) echo '{name} 9.9.9' ;;\n  'install preflight --json') echo '{{\"candidate\":{candidate},\"verdict\":{{\"kind\":\"promote\"}}}}' ;;\n  *) exit 2 ;;\nesac\n"
        )).unwrap();
        fs::set_permissions(path, fs::Permissions::from_mode(0o755)).unwrap();
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn scheduling_preserves_existing_cadence_when_install_location_changes() {
        let _lock = crate::journal::test_env_lock();
        let _restore = crate::test_ambient::EnvGuard::clear(&["PATH"]);
        let temp = tempfile::tempdir().unwrap();
        let bin = temp.path().join("bin");
        fs::create_dir(&bin).unwrap();
        std::os::unix::fs::symlink("/usr/bin/id", bin.join("id")).unwrap();
        let launchctl = bin.join("launchctl");
        fs::write(&launchctl, "#!/bin/sh\n[ \"$1\" != print ]\n").unwrap();
        fs::set_permissions(&launchctl, fs::Permissions::from_mode(0o755)).unwrap();
        std::env::set_var("PATH", &bin);
        let path = temp
            .path()
            .join("Library/LaunchAgents/com.loopflow.refresh.plist");
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        let payload = super::schedule_plist(
            temp.path(),
            &bin.join("lf"),
            temp.path(),
            crate::lf::InstallFrequency::Hourly,
        );
        fs::write(
            &path,
            payload.replace(&bin.display().to_string(), "/previous/bin"),
        )
        .unwrap();
        super::launchd_schedule(temp.path(), &bin.join("lf"), temp.path(), None).unwrap();
        assert_eq!(fs::read_to_string(path).unwrap(), payload);
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn schedule_is_independent_of_checkout_and_retains_its_cadence() {
        use crate::lf::InstallFrequency;

        let _lock = crate::journal::test_env_lock();
        let _restore = crate::test_ambient::EnvGuard::clear(&["PATH"]);
        let temp = tempfile::tempdir().unwrap();
        let bin = temp.path().join("bin");
        fs::create_dir(&bin).unwrap();
        std::os::unix::fs::symlink("/usr/bin/id", bin.join("id")).unwrap();
        let launchctl = bin.join("launchctl");
        fs::write(&launchctl, "#!/bin/sh\nexit 0\n").unwrap();
        fs::set_permissions(&launchctl, fs::Permissions::from_mode(0o755)).unwrap();
        std::env::set_var("PATH", &bin);
        let binary = temp.path().join("installed & current/lf");
        let path = temp
            .path()
            .join("Library/LaunchAgents/com.loopflow.refresh.plist");
        for (frequency, expected) in [
            (
                InstallFrequency::Daily,
                serde_json::json!({"Hour": 9, "Minute": 0}),
            ),
            (InstallFrequency::Hourly, serde_json::json!({"Minute": 0})),
            (
                InstallFrequency::Weekly,
                serde_json::json!({"Weekday": 1, "Hour": 9, "Minute": 0}),
            ),
            (
                InstallFrequency::FiveMinutes,
                serde_json::json!([
                    {"Minute": 0}, {"Minute": 5}, {"Minute": 10}, {"Minute": 15},
                    {"Minute": 20}, {"Minute": 25}, {"Minute": 30}, {"Minute": 35},
                    {"Minute": 40}, {"Minute": 45}, {"Minute": 50}, {"Minute": 55}
                ]),
            ),
        ] {
            super::launchd_schedule(temp.path(), &binary, temp.path(), Some(frequency)).unwrap();
            let output = Command::new("/usr/bin/plutil")
                .args(["-convert", "json", "-o", "-"])
                .arg(&path)
                .output()
                .unwrap();
            assert!(output.status.success());
            let plist: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
            assert_eq!(plist["StartCalendarInterval"], expected);
            assert_eq!(
                plist["EnvironmentVariables"]["LF_INSTALL_DIR"],
                binary.parent().unwrap().to_str().unwrap()
            );
            assert_eq!(
                plist["ProgramArguments"],
                serde_json::json!([binary, "install"])
            );
            assert!(plist.get("WorkingDirectory").is_none());
            assert_eq!(plist["RunAtLoad"], true);
            let modified = fs::metadata(&path).unwrap().modified().unwrap();
            super::launchd_schedule(temp.path(), &binary, temp.path(), None).unwrap();
            assert_eq!(fs::metadata(&path).unwrap().modified().unwrap(), modified);
        }
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn schedule_renews_rental_logins_every_two_hours() {
        let _lock = crate::journal::test_env_lock();
        let _restore = crate::test_ambient::EnvGuard::clear(&["PATH"]);
        let temp = tempfile::tempdir().unwrap();
        let bin = temp.path().join("bin");
        fs::create_dir(&bin).unwrap();
        std::os::unix::fs::symlink("/usr/bin/id", bin.join("id")).unwrap();
        let launchctl = bin.join("launchctl");
        fs::write(&launchctl, "#!/bin/sh\nexit 0\n").unwrap();
        fs::set_permissions(&launchctl, fs::Permissions::from_mode(0o755)).unwrap();
        std::env::set_var("PATH", &bin);
        let agents = temp.path().join("Library/LaunchAgents");
        fs::create_dir_all(&agents).unwrap();
        super::launchd_renewal(temp.path(), &bin.join("lf"), temp.path()).unwrap();
        let output = Command::new("/usr/bin/plutil")
            .args(["-convert", "json", "-o", "-"])
            .arg(agents.join("com.loopflow.rental-renewal.plist"))
            .output()
            .unwrap();
        assert!(output.status.success());
        let plist: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
        assert_eq!(plist["StartInterval"], 7200);
        assert_eq!(
            plist["ProgramArguments"],
            serde_json::json!([bin.join("lf"), "machine", "credentials", "renew"])
        );
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn cli_only_skips_setup_but_keeps_an_existing_app() {
        let _lock = crate::journal::test_env_lock();
        let _restore =
            crate::test_ambient::EnvGuard::clear(&["LF_INSTALL_CLI_ONLY", "LF_APPLICATIONS_DIR"]);
        let temp = tempfile::tempdir().unwrap();
        std::env::set_var("LF_INSTALL_CLI_ONLY", "1");
        std::env::set_var("LF_APPLICATIONS_DIR", temp.path());
        assert!(super::app_destination(&temp.path().join("lf")).is_none());
        let payload =
            b"#!/bin/sh\nprintf '%s' \"$LF_INSTALL_CLI_ONLY\" > \"$LF_INSTALL_DIR/mode\"\n";
        let manifest = format!("{:x}  install.sh\n", Sha256::digest(payload));
        run_verified_installer(temp.path(), None, temp.path(), "v9.9.9", &manifest, payload)
            .unwrap();
        assert_eq!(fs::read_to_string(temp.path().join("mode")).unwrap(), "1");
        fs::create_dir(temp.path().join("Loopflow.app")).unwrap();
        let app = super::app_destination(&temp.path().join("lf"));
        assert!(app.is_some());
        run_verified_installer(
            temp.path(),
            app.as_deref(),
            temp.path(),
            "v9.9.9",
            &manifest,
            payload,
        )
        .unwrap();
        assert_eq!(fs::read_to_string(temp.path().join("mode")).unwrap(), "0");
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn ordinary_and_scheduled_updates_pass_the_custom_destination_to_older_installers() {
        let _lock = crate::journal::test_env_lock();
        let _restore = crate::test_ambient::EnvGuard::clear(&[
            "LF_INSTALL_CLI_ONLY",
            "LF_APPLICATIONS_DIR",
            "LF_INSTALL_DIR",
        ]);
        let temp = tempfile::tempdir().unwrap();
        let directory = temp.path().join("bin");
        fs::create_dir(&directory).unwrap();
        let app = temp.path().join("Custom Tools/Loopflow.app");
        let helper = app.join("Contents/MacOS/lf");
        fs::create_dir_all(helper.parent().unwrap()).unwrap();
        fs::write(&helper, "old helper").unwrap();
        std::os::unix::fs::symlink(
            "../Custom Tools/Loopflow.app/Contents/MacOS/lf",
            directory.join("lf"),
        )
        .unwrap();
        let app = app.canonicalize().unwrap();
        // An older installer only understands its environment, not link discovery.
        let payload = br##"#!/bin/sh
set -eu
[ "$LF_INSTALL_CLI_ONLY" = 0 ]
printf updated > "$LF_APPLICATIONS_DIR/Loopflow.app/Contents/MacOS/lf"
"##;
        let manifest = format!("{:x}  install.sh\n", Sha256::digest(payload));
        let plist = temp.path().join("schedule.plist");
        fs::write(
            &plist,
            super::schedule_plist(
                temp.path(),
                &directory.join("lf"),
                temp.path(),
                crate::lf::InstallFrequency::Daily,
            ),
        )
        .unwrap();
        let output = Command::new("/usr/bin/plutil")
            .args(["-convert", "json", "-o", "-"])
            .arg(&plist)
            .output()
            .unwrap();
        assert!(output.status.success());
        let schedule: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
        for scheduled in [false, true] {
            for cli_only in ["0", "1"] {
                std::env::set_var(
                    "LF_INSTALL_DIR",
                    if scheduled {
                        schedule["EnvironmentVariables"]["LF_INSTALL_DIR"]
                            .as_str()
                            .unwrap()
                    } else {
                        directory.to_str().unwrap()
                    },
                );
                std::env::set_var("LF_INSTALL_CLI_ONLY", cli_only);
                let directory = super::install_dir().unwrap();
                let destination = super::app_destination(&directory.join("lf"));
                assert_eq!(destination.as_deref(), Some(app.as_path()));
                fs::write(&helper, "old helper").unwrap();
                run_verified_installer(
                    &directory,
                    destination.as_deref(),
                    temp.path(),
                    "v9.9.9",
                    &manifest,
                    payload,
                )
                .unwrap();
                assert_eq!(fs::read_to_string(&helper).unwrap(), "updated");
                assert_eq!(
                    directory.join("lf").canonicalize().unwrap(),
                    helper.canonicalize().unwrap()
                );
            }
        }
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn explicit_destination_wins_without_splitting_an_existing_cli_only_install() {
        let _lock = crate::journal::test_env_lock();
        let _restore =
            crate::test_ambient::EnvGuard::clear(&["LF_INSTALL_CLI_ONLY", "LF_APPLICATIONS_DIR"]);
        let temp = tempfile::tempdir().unwrap();
        let installed = temp.path().join("Old/Loopflow.app/Contents/MacOS/lf");
        fs::create_dir_all(installed.parent().unwrap()).unwrap();
        fs::write(&installed, "old helper").unwrap();
        let cli = temp.path().join("lf");
        std::os::unix::fs::symlink(&installed, &cli).unwrap();
        let destination = temp.path().join("New/Loopflow.app");
        std::env::set_var("LF_INSTALL_CLI_ONLY", "1");
        std::env::set_var("LF_APPLICATIONS_DIR", destination.parent().unwrap());
        assert_eq!(super::app_destination(&cli), Some(destination.clone()));
        // A detached public entry still discovers an explicitly located app.
        fs::remove_file(&cli).unwrap();
        fs::write(&cli, "detached").unwrap();
        fs::create_dir_all(&destination).unwrap();
        assert_eq!(super::app_destination(&cli), Some(destination));
    }

    #[test]
    fn an_installed_build_cannot_report_success_when_promotion_fails() {
        let temp = tempfile::tempdir().unwrap();
        let cli = temp.path().join("lf");
        fs::write(&cli, "#!/bin/sh\nexit 23\n").unwrap();
        fs::set_permissions(&cli, fs::Permissions::from_mode(0o755)).unwrap();
        let error = super::promote_build(&cli, temp.path(), None).unwrap_err();
        assert!(error.to_string().contains("installation promotion failed"));
    }

    #[test]
    fn installed_release_needs_only_the_verified_cli() {
        let temp = tempfile::tempdir().unwrap();
        binary(&temp.path().join("lf"), "lf");
        assert!(is_current(temp.path(), "v9.9.9", None));
        assert!(!is_current(temp.path(), "v9.9.10", None));
        let script = fs::read_to_string(temp.path().join("lf")).unwrap();
        fs::write(
            temp.path().join("lf"),
            script.replace("\"promote\"", "\"promote_and_migrate\""),
        )
        .unwrap();
        assert!(!is_current(temp.path(), "v9.9.9", None));
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn launchd_schedule_preserves_frequency_and_reuses_its_job() {
        use crate::lf::commands::graph::EnvVarGuard;
        use std::os::unix::fs::symlink;

        let _env = loopflow_test_support::env_lock().lock().unwrap();
        let home = tempfile::tempdir().unwrap();
        let bin = home.path().join("bin");
        fs::create_dir_all(&bin).unwrap();
        symlink("/usr/bin/id", bin.join("id")).unwrap();
        let launchctl = bin.join("launchctl");
        fs::write(&launchctl, "#!/bin/sh\ncase $1 in\nprint) test -f \"$HOME/loaded\";;\nbootstrap) : > \"$HOME/loaded\";;\nbootout) /bin/rm \"$HOME/loaded\";;\nesac\n").unwrap();
        fs::set_permissions(&launchctl, fs::Permissions::from_mode(0o755)).unwrap();
        let _home = EnvVarGuard::set("HOME", home.path().to_str().unwrap());
        let _path = EnvVarGuard::set("PATH", bin.to_str().unwrap());
        let binary = home.path().join("installed & current/lf");
        let logs = home.path().join("logs");
        let path = home
            .path()
            .join("Library/LaunchAgents/com.loopflow.refresh.plist");
        let run = |frequency| launchd_schedule(home.path(), &binary, &logs, frequency).unwrap();
        let read_plist = || -> serde_json::Value {
            let output = Command::new("/usr/bin/plutil")
                .args(["-convert", "json", "-o", "-"])
                .arg(&path)
                .output()
                .unwrap();
            assert!(output.status.success());
            serde_json::from_slice(&output.stdout).unwrap()
        };
        run(None);
        let plist = read_plist();
        assert_eq!(
            plist["ProgramArguments"],
            serde_json::json!([binary, "install"])
        );
        assert!(plist.get("WorkingDirectory").is_none());
        assert_eq!(plist["RunAtLoad"], true);
        assert_eq!(
            plist["StartCalendarInterval"],
            serde_json::json!({"Hour": 9, "Minute": 0})
        );
        for (frequency, expected) in [
            (
                InstallFrequency::Daily,
                serde_json::json!({"Hour": 9, "Minute": 0}),
            ),
            (InstallFrequency::Hourly, serde_json::json!({"Minute": 0})),
            (
                InstallFrequency::FiveMinutes,
                serde_json::json!([
                    {"Minute": 0}, {"Minute": 5}, {"Minute": 10}, {"Minute": 15},
                    {"Minute": 20}, {"Minute": 25}, {"Minute": 30}, {"Minute": 35},
                    {"Minute": 40}, {"Minute": 45}, {"Minute": 50}, {"Minute": 55}
                ]),
            ),
            (
                InstallFrequency::Weekly,
                serde_json::json!({"Weekday": 1, "Hour": 9, "Minute": 0}),
            ),
        ] {
            run(Some(frequency));
            assert_eq!(read_plist()["StartCalendarInterval"], expected);
            assert!(home.path().join("loaded").exists());
            let modified = fs::metadata(&path).unwrap().modified().unwrap();
            run(None);
            assert_eq!(fs::metadata(&path).unwrap().modified().unwrap(), modified);
            fs::remove_file(home.path().join("loaded")).unwrap();
            run(None);
            assert!(home.path().join("loaded").exists());
            assert_eq!(read_plist()["StartCalendarInterval"], expected);
        }
    }

    #[test]
    fn cleanup_schedule_never_provisions_a_disposable_home() {
        use crate::lf::commands::graph::EnvVarGuard;
        let _env = loopflow_test_support::env_lock().lock().unwrap();
        let home = tempfile::tempdir().unwrap();
        let _home = EnvVarGuard::set("HOME", home.path().to_str().unwrap());
        let _machine = EnvVarGuard::set("LF_HOME", home.path().to_str().unwrap());
        let _path = EnvVarGuard::set("PATH", home.path().to_str().unwrap());
        super::schedule(None).unwrap();
        assert_eq!(fs::read_dir(home.path()).unwrap().count(), 0);
    }

    #[test]
    fn cleanup_schedule_keeps_one_minute_job_separate_from_updates() {
        let home = Path::new("/home/person's % home");
        let binary = home.join("bin/lf");
        let logs = home.join(".lf/logs");
        let update = super::cron_line(home, &binary, &logs, InstallFrequency::Daily);
        let cleanup = super::cleanup_cron_line(home, &binary, &logs);
        let current = "MAILTO=me\n0 0 * * * backup\n";
        let table = |current: &str, update: &str| {
            let updated = cron_table(current, CRON_MARKER, update);
            cron_table(&updated, super::CLEANUP_CRON_MARKER, &cleanup)
        };
        let first = table(current, &update);
        assert_eq!(first, format!("{current}{update}\n{cleanup}\n"));
        assert_eq!(table(&first, &update), first);
        let hourly = super::cron_line(home, &binary, &logs, InstallFrequency::Hourly);
        assert_eq!(
            table(&first, &hourly),
            format!("{current}{hourly}\n{cleanup}\n")
        );
        assert!(cleanup.starts_with("* * * * * "));
        assert!(cleanup.contains(" task retire "));
        assert!(cleanup.contains("LF_HOME="));
        // Both commands quote paths for the shell after cron unescapes percent.
        for line in [&update, &cleanup] {
            assert!(line.contains("\\%"));
            let words = shlex::split(&line.replace("\\%", "%")).unwrap();
            assert!(words.contains(&binary.to_string_lossy().into_owned()));
        }
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn cleanup_schedule_pins_machine_and_reloads_only_when_needed() {
        use crate::lf::commands::graph::EnvVarGuard;
        let _env = loopflow_test_support::env_lock().lock().unwrap();
        let home = tempfile::tempdir().unwrap();
        let bin = home.path().join("bin");
        fs::create_dir_all(&bin).unwrap();
        std::os::unix::fs::symlink("/usr/bin/id", bin.join("id")).unwrap();
        fs::write(bin.join("launchctl"), "#!/bin/sh\ncase $1 in\nprint) test -f \"$HOME/loaded\";;\nbootstrap) : > \"$HOME/loaded\";;\nbootout) /bin/rm \"$HOME/loaded\";;\nesac\n").unwrap();
        fs::set_permissions(bin.join("launchctl"), fs::Permissions::from_mode(0o755)).unwrap();
        let _home = EnvVarGuard::set("HOME", home.path().to_str().unwrap());
        let _path = EnvVarGuard::set("PATH", bin.to_str().unwrap());
        let binary = home.path().join("installed & current/lf");
        let payload = super::cleanup_plist(home.path(), &binary, &home.path().join(".lf/logs"));
        let path = home
            .path()
            .join("Library/LaunchAgents/com.loopflow.cleanup.plist");
        let run = || super::install_launchd(home.path(), "com.loopflow.cleanup", &payload).unwrap();
        run();
        let modified = fs::metadata(&path).unwrap().modified().unwrap();
        for _ in 0..3 {
            run();
        }
        assert_eq!(fs::metadata(&path).unwrap().modified().unwrap(), modified);
        let output = Command::new("/usr/bin/plutil")
            .args(["-convert", "json", "-o", "-"])
            .arg(&path)
            .output()
            .unwrap();
        assert!(output.status.success());
        let plist: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
        assert_eq!(
            plist["ProgramArguments"],
            serde_json::json!([binary, "task", "retire"])
        );
        assert_eq!(
            plist["EnvironmentVariables"]["LF_HOME"],
            home.path().join(".lf").to_str().unwrap()
        );
        assert_eq!(plist["StartInterval"], 60);
        assert_eq!(plist["RunAtLoad"], true);
        assert!(plist.get("KeepAlive").is_none());
        fs::remove_file(home.path().join("loaded")).unwrap();
        run();
        assert!(home.path().join("loaded").exists());
    }

    #[test]
    fn installer_verifies_bytes_and_passes_the_pinned_release() {
        let temp = tempfile::tempdir().unwrap();
        let payload = b"#!/bin/sh\nprintf '%s' \"$2\" > \"$LF_INSTALL_DIR/installed\"\n";
        let manifest = format!("{:x}  install.sh\n", Sha256::digest(payload));
        let tag = release_tag("/loopflowstudio/loopflow/releases/tag/v9.9.9").unwrap();
        run_verified_installer(temp.path(), None, temp.path(), &tag, &manifest, payload).unwrap();
        assert_eq!(
            fs::read_to_string(temp.path().join("installed")).unwrap(),
            "v9.9.9"
        );
        fs::remove_file(temp.path().join("installed")).unwrap();
        let error = run_verified_installer(
            temp.path(),
            None,
            temp.path(),
            &tag,
            "00 install.sh",
            payload,
        )
        .unwrap_err();
        assert!(error.to_string().contains("digest mismatch"));
        assert!(!temp.path().join("installed").exists());
        assert!(release_tag("/loopflowstudio/loopflow/releases/latest").is_err());
    }

    #[test]
    fn main_installs_the_newest_green_run_for_this_platform() {
        let runs = r#"[{"databaseId":42,"headSha":"5c9c22ac36f327d11368654218902a63f3f88b1b"}]"#;
        assert_eq!(
            newest_run(runs).unwrap(),
            (42, "5c9c22ac36f327d11368654218902a63f3f88b1b".to_string())
        );
        assert!(newest_run("[]")
            .unwrap_err()
            .to_string()
            .contains("no green main build"));
        assert!(package_target().is_some_and(|target| target.contains(std::env::consts::ARCH)));
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn a_main_build_app_without_a_developer_id_signature_is_refused() {
        let temp = tempfile::tempdir().unwrap();
        let source = temp.path().join("source");
        let macos = source.join("Loopflow.app/Contents/MacOS");
        fs::create_dir_all(&macos).unwrap();
        binary(&macos.join("Loopflow"), "Loopflow");
        let dmg = temp.path().join("Loopflow.dmg");
        checked(
            Command::new("/usr/bin/hdiutil")
                .args(["create", "-quiet", "-format", "UDZO", "-srcfolder"])
                .arg(&source)
                .arg(&dmg),
        )
        .unwrap();
        let mount = temp.path().join("mount");
        let error = DiskImage::mount(&dmg, &mount).err().unwrap().to_string();
        assert!(error.contains("codesign"), "{error}");
        assert!(!mount.join("Loopflow.app").exists());
    }

    #[test]
    fn a_switch_is_recorded_beside_other_machine_settings() {
        let temp = tempfile::tempdir().unwrap();
        let home = temp.path().join(".lf");
        let config = home.join("config.yaml");
        assert_eq!(recorded_stream(&home).unwrap(), None);
        fs::create_dir_all(&home).unwrap();
        fs::write(&config, "repo_root: /src\n").unwrap();
        record_stream(&home, Stream::Preview).unwrap();
        assert_eq!(recorded_stream(&home).unwrap(), Some(Stream::Preview));
        record_stream(&home, Stream::Dev).unwrap();
        assert_eq!(recorded_stream(&home).unwrap(), Some(Stream::Dev));
        assert!(fs::read_to_string(&config)
            .unwrap()
            .contains("repo_root: /src"));
        fs::write(&config, "stream: nightly\n").unwrap();
        assert!(recorded_stream(&home)
            .unwrap_err()
            .to_string()
            .contains("unknown stream `nightly`"));
    }

    #[test]
    fn preview_is_the_newest_published_release() {
        let releases = r#"[
            {"tag_name":"v0.17.0-rc.1","draft":false,"prerelease":true},
            {"tag_name":"v0.16.2","draft":false,"prerelease":false},
            {"tag_name":"v0.16.1","draft":false,"prerelease":false}
        ]"#;
        assert_eq!(newest_release(releases).unwrap(), "v0.16.2");
        assert!(newest_release("[]").is_err());
    }

    #[test]
    fn the_cron_schedule_replaces_only_its_own_line() {
        let ours = format!("0 9 * * 1 lf install {CRON_MARKER}");
        let current = format!("MAILTO=me\n*/10 * * * * backup\n{ours}\n");
        let daily = format!("0 9 * * * lf install {CRON_MARKER}");
        assert_eq!(
            cron_table(&current, CRON_MARKER, &daily),
            format!("MAILTO=me\n*/10 * * * * backup\n{daily}\n")
        );
        assert_eq!(cron_table("", CRON_MARKER, &daily), format!("{daily}\n"));
        let renewal = super::renewal_cron_line(
            Path::new("/home/me"),
            Path::new("/home/me/.local/bin/lf"),
            Path::new("/home/me/.lf/logs"),
        );
        assert!(renewal.starts_with("0 */2 * * * "), "{renewal}");
        assert!(renewal.contains("'/home/me/.local/bin/lf' machine credentials renew"));
        assert_eq!(
            cron_table(&format!("{daily}\n"), super::RENEWAL_CRON_MARKER, &renewal),
            format!("{daily}\n{renewal}\n")
        );
    }

    #[test]
    fn failed_installer_remains_a_failure() {
        let temp = tempfile::tempdir().unwrap();
        let payload = b"#!/bin/sh\nexit 23\n";
        let manifest = format!("{:x} *install.sh\n", Sha256::digest(payload));
        let error =
            run_verified_installer(temp.path(), None, temp.path(), "v9.9.9", &manifest, payload)
                .unwrap_err();
        assert!(error.to_string().contains("published installation failed"));
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn current_cli_does_not_hide_a_missing_stale_or_incomplete_app() {
        let temp = tempfile::tempdir().unwrap();
        binary(&temp.path().join("lf"), "lf");
        let applications = temp.path().join("Applications");
        let current = || {
            is_current(
                temp.path(),
                "v9.9.9",
                Some(&applications.join("Loopflow.app")),
            )
        };
        assert!(!current());
        let contents = applications.join("Loopflow.app/Contents");
        fs::create_dir_all(contents.join("MacOS")).unwrap();
        for name in ["Loopflow", "lf"] {
            binary(&contents.join("MacOS").join(name), name);
        }
        fs::remove_file(temp.path().join("lf")).unwrap();
        std::os::unix::fs::symlink(contents.join("MacOS/lf"), temp.path().join("lf")).unwrap();
        for (version, expected) in [("9.9.8", false), ("9.9.9", true)] {
            fs::write(contents.join("Info.plist"), format!(
                "<plist version=\"1.0\"><dict><key>CFBundleShortVersionString</key><string>{version}</string><key>CFBundleVersion</key><string>{version}</string></dict></plist>"
            )).unwrap();
            assert_eq!(current(), expected);
        }
        let helper = contents.join("MacOS/lf");
        let original = fs::read_to_string(&helper).unwrap();
        fs::write(
            &helper,
            original.replace("\"promote\"", "\"reject\",\"reasons\":[\"newer store\"]"),
        )
        .unwrap();
        assert!(!current());
        fs::write(&helper, &original).unwrap();
        // Same marketing version on an independently promoted CLI cannot hide
        // a different build still installed in Desktop.
        fs::remove_file(temp.path().join("lf")).unwrap();
        binary(&temp.path().join("lf"), "lf");
        let source = crate::lf::commands::install::CandidateIdentity::current().source_revision;
        fs::write(&helper, original.replace(&source, "different-build")).unwrap();
        assert!(!current());
        fs::write(&helper, &original).unwrap();
        assert!(
            !current(),
            "even matching independent files must repair the public link"
        );
        fs::remove_file(temp.path().join("lf")).unwrap();
        std::os::unix::fs::symlink(&helper, temp.path().join("lf")).unwrap();
        assert!(current());
        fs::remove_file(contents.join("MacOS/Loopflow")).unwrap();
        assert!(!current());
        fs::write(contents.join("Info.plist"), "invalid plist").unwrap();
        assert!(!current());
    }
}
