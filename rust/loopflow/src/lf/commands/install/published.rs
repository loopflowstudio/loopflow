//! Stream installs (green main builds and published releases) and the optional
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
/// The reader floors a release's build opens (`lf self install reader-catalog`).
const READER_CATALOG: &str = "reader-catalog.txt";
/// Starts Loopflow.app's own update check (`LoopflowApp.handleDeepLink`).
const APP_UPDATE_LINK: &str = "loopflow://update";

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
    if stream == Stream::Dev {
        return main_build();
    }
    if cfg!(target_os = "macos") {
        finish_adoption(&applications_dir())?;
    }
    let client = http_client()?;
    let tag = stream_release(&client, stream)?;
    install_release(&client, stream, &tag)
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

fn install_release(client: &Client, stream: Stream, tag: &str) -> Result<()> {
    let directory = install_dir()?;
    let applications = applications_dir();
    let cli_only = std::env::var("LF_INSTALL_CLI_ONLY").as_deref() == Ok("1");
    let stream = stream.as_str();
    if is_current(
        &directory,
        tag,
        (cfg!(target_os = "macos") && !cli_only).then_some(applications.as_path()),
    ) {
        println!("{stream} {tag} is already installed.");
        return Ok(());
    }
    let manifest = release_manifest(client, tag)?;
    if !release_opens_store(client, tag, &manifest)? {
        println!(
            "{stream} is at {tag}, which cannot open this Machine's store: a newer lf migrated it. \
             Keeping the installed lf; `lf install` moves to {stream} once it reaches a build that opens the store."
        );
        return Ok(());
    }
    let app = applications.join("Loopflow.app");
    if cfg!(target_os = "macos") && !cli_only && updates_itself(&app) && app_is_running(&app)? {
        // Replacing a running bundle under it is Sparkle's job, not ours.
        let status = Command::new("/usr/bin/open")
            .args(["-g", APP_UPDATE_LINK])
            .status()
            .context("ask Loopflow.app to update")?;
        if !status.success() {
            bail!(
                "could not ask {} to update ({status}); quit it and rerun `lf install`",
                app.display()
            );
        }
        println!(
            "{stream} is at {tag}; Loopflow.app is running, so it installs the update and `lf` follows it."
        );
        return Ok(());
    }
    let installer = release_asset(client, tag, "install.sh")?;
    let temporary = tempfile::tempdir()?;
    run_verified_installer(&directory, temporary.path(), tag, &manifest, &installer)?;
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
    let app = applications_dir().join("Loopflow.app");
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
    )
}

/// Adopt Loopflow.app when Sparkle replaced it and its first launch hasn't run
/// yet: the app's `lf` is ahead of the store, so every terminal `lf` refuses
/// until something migrates it. Moves no files.
fn finish_adoption(applications: &Path) -> Result<()> {
    let lf = applications.join("Loopflow.app/Contents/MacOS/lf");
    if preflight_verdict(&lf).as_deref() != Some("promote_and_migrate") {
        return Ok(());
    }
    let status = Command::new(&lf)
        .args(["install", "adopt"])
        .status()
        .with_context(|| format!("adopt {}", lf.display()))?;
    if !status.success() {
        bail!(
            "adopting the updated {} failed ({status}); fix the error above and rerun `lf install`",
            lf.display()
        );
    }
    Ok(())
}

fn applications_dir() -> PathBuf {
    std::env::var_os("LF_APPLICATIONS_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("/Applications"))
}

/// Succeed only when release `tag` can open this Machine's store; Loopflow.app's
/// updater asks before installing a release.
pub fn can_open(tag: &str) -> Result<()> {
    let client = http_client()?;
    let manifest = release_manifest(&client, tag)?;
    if !release_opens_store(&client, tag, &manifest)? {
        bail!(
            "{tag} cannot open this Machine's store: a newer lf migrated it. \
             Loopflow stays on this build until its stream reaches one that opens the store."
        );
    }
    println!("{tag} opens this Machine's store");
    Ok(())
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

/// Whether release `tag` opens the store, from its digest-checked reader catalog.
fn release_opens_store(client: &Client, tag: &str, manifest: &str) -> Result<bool> {
    // Releases before v0.16 publish no catalog; promotion still refuses them.
    if manifest_digest(manifest, READER_CATALOG).is_none() {
        return Ok(true);
    }
    let catalog = release_asset(client, tag, READER_CATALOG)?;
    verify_asset(manifest, READER_CATALOG, &catalog)?;
    opens_store(
        &crate::store::production_database_path(),
        &String::from_utf8_lossy(&catalog),
    )
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

/// Whether a build with this reader catalog opens the store. An absent store opens.
fn opens_store(store: &Path, catalog: &str) -> Result<bool> {
    if !store.exists() {
        return Ok(true);
    }
    let conn = rusqlite::Connection::open_with_flags(
        store,
        rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY | rusqlite::OpenFlags::SQLITE_OPEN_NO_MUTEX,
    )?;
    let catalog: Vec<String> = catalog
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty())
        .map(str::to_string)
        .collect();
    Ok(crate::store::migrations::can_open(&conn, &catalog)?)
}

/// The stream recorded in `home`'s config.yaml.
fn recorded_stream(home: &Path) -> Result<Option<Stream>> {
    let config = home.join("config.yaml");
    let Some(name) = crate::config::read_global_config_at(&config)?
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
    crate::config::update_global_config_at(home, |config| {
        config["stream"] = stream.as_str().into();
        Ok(())
    })
}

/// Install the package CI built for the newest green main commit. GitHub keeps
/// those artifacts behind authentication, so this uses the `gh` login.
fn main_build() -> Result<()> {
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
    gh(&[
        "run",
        "download",
        &run.to_string(),
        "--repo",
        REPOSITORY,
        "--name",
        &format!("lf-{target}"),
        "--dir",
        &temporary.path().to_string_lossy(),
    ])?;
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
    let directory = install_dir()?;
    let status = Command::new(&candidate)
        .args(["self", "install", "promote", "--cli-target"])
        .arg(directory.join("lf"))
        .arg("--sync-skills")
        .status()
        .context("start the main build's promotion")?;
    if !status.success() {
        bail!("installing main {revision} failed ({status}); fix the error above and rerun `lf install --dev`");
    }
    println!(
        "main: {revision}\ninstalled: {}",
        directory.join("lf").display()
    );
    schedule(None)
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

fn is_current(directory: &Path, tag: &str, applications: Option<&Path>) -> bool {
    let version = tag.trim_start_matches('v');
    let lf = directory.join("lf");
    let installed = inspect(&lf, &["--version"]).is_some_and(|output| {
        output.status.success()
            && String::from_utf8_lossy(&output.stdout).trim() == format!("lf {version}")
    });
    installed
        && preflight_verdict(&lf).as_deref() == Some("promote")
        && applications.is_none_or(|root| has_release_app(root, version))
}

/// What `lf install preflight` says installing `lf` would do to the store.
fn preflight_verdict(lf: &Path) -> Option<String> {
    let output = inspect(lf, &["install", "preflight", "--json"])
        .filter(|output| output.status.success())?;
    let preview: serde_json::Value = serde_json::from_slice(&output.stdout).ok()?;
    preview["verdict"]["kind"].as_str().map(str::to_string)
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

fn app_info(app: &Path) -> Option<serde_json::Value> {
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

fn has_release_app(applications: &Path, version: &str) -> bool {
    let app = applications.join("Loopflow.app");
    app_info(&app).is_some_and(|info| {
        info["CFBundleShortVersionString"] == version && info["CFBundleVersion"] == version
    }) && ["Loopflow", "lf"].iter().all(|name| {
        fs::metadata(app.join("Contents/MacOS").join(name))
            .is_ok_and(|metadata| metadata.is_file() && metadata.permissions().mode() & 0o111 != 0)
    })
}

/// An app built with Sparkle's signing key installs its own updates.
fn updates_itself(app: &Path) -> bool {
    app_info(app).is_some_and(|info| {
        info["SUPublicEDKey"]
            .as_str()
            .is_some_and(|key| !key.is_empty())
    })
}

fn app_is_running(app: &Path) -> Result<bool> {
    Ok(!super::running_app_processes(&[app.join("Contents/MacOS/Loopflow")])?.is_empty())
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
    temporary: &Path,
    tag: &str,
    manifest: &str,
    installer: &[u8],
) -> Result<()> {
    verify_asset(manifest, "install.sh", installer)?;
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

/// Run `lf install` on the recorded stream on a schedule: `frequency`, else the
/// installed schedule's own, else daily. Every install sets it, so a Machine
/// updates itself without being asked.
pub fn schedule(frequency: Option<InstallFrequency>) -> Result<()> {
    let home = dirs::home_dir().context("cannot determine the installation home")?;
    // launchd and cron schedule for the whole account; a disposable HOME
    // (release smoke tests) must not replace the account's job.
    if home != crate::installation::account_home()? {
        println!(
            "Not scheduling installation: HOME {} is not this account's home",
            home.display()
        );
        return Ok(());
    }
    let binary = install_dir()?.join("lf");
    let logs = crate::store::lf_home_dir().join("logs");
    fs::create_dir_all(&logs)?;
    if cfg!(target_os = "macos") {
        launchd_schedule(&home, &binary, &logs, frequency)
    } else {
        cron_schedule(&home, &binary, &logs, frequency)
    }
}

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
    fs::create_dir_all(path.parent().expect("launch agent has a parent"))?;
    let current = fs::read_to_string(&path).ok();
    let frequency = frequency.unwrap_or_else(|| {
        installed_frequency(|frequency| {
            current.as_deref() == Some(&schedule_plist(home, binary, logs, frequency))
        })
    });
    let payload = schedule_plist(home, binary, logs, frequency);
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
    if loaded && current.as_deref() == Some(payload.as_str()) {
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
    println!(
        "Installation scheduled at login and {}: {}; logs: {}",
        cadence(frequency),
        path.display(),
        logs.join("refresh.log").display()
    );
    Ok(())
}

/// Marks the one crontab line `lf install schedule` owns.
const CRON_MARKER: &str = "# loopflow install schedule";

fn cron_schedule(
    home: &Path,
    binary: &Path,
    logs: &Path,
    frequency: Option<InstallFrequency>,
) -> Result<()> {
    let listed = Command::new("crontab").arg("-l").output().context(
        "automatic installation uses cron; install it, or run `lf install` to update manually",
    )?;
    // `crontab -l` fails when the user has no crontab yet.
    let current = if listed.status.success() {
        String::from_utf8(listed.stdout)?
    } else {
        String::new()
    };
    let frequency = frequency.unwrap_or_else(|| {
        installed_frequency(|frequency| {
            let line = cron_line(home, binary, logs, frequency);
            current.lines().any(|entry| entry == line)
        })
    });
    let table = cron_table(&current, &cron_line(home, binary, logs, frequency));
    if table == current {
        println!("Installation already scheduled in your crontab");
        return Ok(());
    }
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
        bail!("could not install the installation schedule; rerun `lf install schedule`");
    }
    println!(
        "Installation scheduled {} in your crontab; logs: {}",
        cadence(frequency),
        logs.join("refresh.log").display()
    );
    Ok(())
}

fn cron_line(home: &Path, binary: &Path, logs: &Path, frequency: InstallFrequency) -> String {
    let when = match frequency {
        InstallFrequency::Weekly => "0 9 * * 1",
        InstallFrequency::Daily => "0 9 * * *",
        InstallFrequency::Hourly => "0 * * * *",
        InstallFrequency::FiveMinutes => "*/5 * * * *",
    };
    let bin = binary.parent().expect("installation binary has a parent");
    format!(
        "{when} LF_INSTALL_DIR='{}' PATH='{}:{}/.cargo/bin:/usr/local/bin:/usr/bin:/bin' '{}' install >> '{}' 2>&1 {CRON_MARKER}",
        bin.display(),
        bin.display(),
        home.display(),
        binary.display(),
        logs.join("refresh.log").display()
    )
}

/// `current` with its Loopflow line replaced by `line`; other entries stay.
fn cron_table(current: &str, line: &str) -> String {
    let mut table: String = current
        .lines()
        .filter(|entry| !entry.ends_with(CRON_MARKER))
        .map(|entry| format!("{entry}\n"))
        .collect();
    table.push_str(line);
    table.push('\n');
    table
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
    use super::{
        cron_table, finish_adoption, is_current, newest_release, newest_run, opens_store,
        package_target, record_stream, recorded_stream, release_tag, run_verified_installer,
        updates_itself, Stream, CRON_MARKER,
    };
    use sha2::{Digest, Sha256};
    use std::fs;
    use std::os::unix::fs::PermissionsExt;
    use std::path::Path;

    fn binary(path: &Path, name: &str) {
        fs::write(path, format!(
            "#!/bin/sh\ncase \"$*\" in\n  --version) echo '{name} 9.9.9' ;;\n  'install preflight --json') echo '{{\"candidate\":{{}},\"verdict\":{{\"kind\":\"promote\"}}}}' ;;\n  *) exit 2 ;;\nesac\n"
        )).unwrap();
        fs::set_permissions(path, fs::Permissions::from_mode(0o755)).unwrap();
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
    fn a_build_whose_catalog_lacks_the_stores_floor_cannot_open_it() {
        let temp = tempfile::tempdir().unwrap();
        let store = temp.path().join("loopflow.db");
        assert!(opens_store(&store, "").unwrap());
        let connection = rusqlite::Connection::open(&store).unwrap();
        crate::store::migrations::apply_sqlite(&connection).unwrap();
        drop(connection);
        let current = crate::store::migrations::reader_catalog().join("\n");
        assert!(opens_store(&store, &current).unwrap());
        let older = crate::store::migrations::reader_catalog()
            .split_last()
            .unwrap()
            .1
            .join("\n");
        assert!(!opens_store(&store, &older).unwrap());
    }

    #[test]
    fn the_cron_schedule_replaces_only_its_own_line() {
        let ours = format!("0 9 * * 1 lf install {CRON_MARKER}");
        let current = format!("MAILTO=me\n*/10 * * * * backup\n{ours}\n");
        let daily = format!("0 9 * * * lf install {CRON_MARKER}");
        assert_eq!(
            cron_table(&current, &daily),
            format!("MAILTO=me\n*/10 * * * * backup\n{daily}\n")
        );
        assert_eq!(cron_table("", &daily), format!("{daily}\n"));
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
        binary(&temp.path().join("lf"), "lf");
        let applications = temp.path().join("Applications");
        let current = || is_current(temp.path(), "v9.9.9", Some(&applications));
        assert!(!current());
        let contents = applications.join("Loopflow.app/Contents");
        fs::create_dir_all(contents.join("MacOS")).unwrap();
        for name in ["Loopflow", "lf"] {
            binary(&contents.join("MacOS").join(name), name);
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

    /// Between Sparkle's install-on-quit and the app's next launch, `lf install`
    /// migrates the store through the app's own `lf` instead of downloading.
    #[test]
    fn an_app_ahead_of_the_store_is_adopted_in_place() {
        let temp = tempfile::tempdir().unwrap();
        let macos = temp.path().join("Loopflow.app/Contents/MacOS");
        fs::create_dir_all(&macos).unwrap();
        finish_adoption(temp.path()).unwrap();
        let adopted = temp.path().join("adopted");
        let app_lf = |verdict: &str| {
            fs::write(macos.join("lf"), format!(
                "#!/bin/sh\ncase \"$*\" in\n  'install preflight --json') echo '{{\"verdict\":{{\"kind\":\"{verdict}\"}}}}' ;;\n  'install adopt') touch '{}' ;;\n  *) exit 2 ;;\nesac\n",
                adopted.display()
            )).unwrap();
            fs::set_permissions(macos.join("lf"), fs::Permissions::from_mode(0o755)).unwrap();
        };
        app_lf("promote");
        finish_adoption(temp.path()).unwrap();
        assert!(!adopted.exists());
        app_lf("promote_and_migrate");
        finish_adoption(temp.path()).unwrap();
        assert!(adopted.exists());
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn only_an_app_with_a_sparkle_key_updates_itself() {
        let temp = tempfile::tempdir().unwrap();
        let app = temp.path().join("Loopflow.app");
        assert!(!updates_itself(&app));
        fs::create_dir_all(app.join("Contents")).unwrap();
        for (key, expected) in [("", false), ("test-public-key", true)] {
            fs::write(app.join("Contents/Info.plist"), format!(
                "<plist version=\"1.0\"><dict><key>SUPublicEDKey</key><string>{key}</string></dict></plist>"
            )).unwrap();
            assert_eq!(updates_itself(&app), expected);
        }
    }
}
