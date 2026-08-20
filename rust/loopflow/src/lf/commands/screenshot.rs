use std::fs::{self, File};
use std::io::{Read, Seek, SeekFrom};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::mpsc::{self, Receiver};
use std::thread;
use std::time::{Duration, Instant};

use anyhow::{anyhow, bail, Context, Result};
use reqwest::Url;
use tempfile::{Builder, TempPath};

use crate::engine::process::{which_on_path, ProcessGroupGuard};
use crate::lf::ScreenshotArgs;

const CAPTURE_TIMEOUT: Duration = Duration::from_secs(30);
const POLL_INTERVAL: Duration = Duration::from_millis(20);
const MAX_VIEWPORT_DIMENSION: u32 = 16_384;
const BROWSER_NAME: &str = "chrome-headless-shell";
const PNG_SIGNATURE: &[u8; 8] = b"\x89PNG\r\n\x1a\n";

pub fn run(args: &ScreenshotArgs) -> Result<()> {
    validate_viewport(args.width, args.height)?;

    let parent_pid = parent_process_id();
    let executable = std::env::current_exe().context("resolve the current lf executable")?;
    let mut command = Command::new(executable);
    command
        .arg("__screenshot-supervisor")
        .arg(&args.source)
        .arg("--output")
        .arg(&args.output)
        .arg("--width")
        .arg(args.width.to_string())
        .arg("--height")
        .arg(args.height.to_string())
        .stdin(Stdio::piped())
        .stdout(Stdio::inherit())
        .stderr(Stdio::inherit());
    #[cfg(unix)]
    {
        use std::os::unix::process::CommandExt;
        command.process_group(0);
    }

    let mut supervisor = command
        .spawn()
        .context("start the screenshot lifetime supervisor")?;
    let owner_pipe = supervisor
        .stdin
        .take()
        .context("screenshot supervisor did not expose its control pipe")?;

    loop {
        if parent_pid.is_some() && parent_process_id() != parent_pid {
            drop(owner_pipe);
            let _ = supervisor.wait();
            bail!("screenshot cancelled because its invoking process exited");
        }
        if let Some(status) = supervisor
            .try_wait()
            .context("wait for the screenshot supervisor")?
        {
            drop(owner_pipe);
            if status.success() {
                return Ok(());
            }
            bail!("screenshot supervisor failed with {status}");
        }
        thread::sleep(POLL_INTERVAL);
    }
}

pub fn run_supervisor(args: &ScreenshotArgs) -> Result<()> {
    validate_viewport(args.width, args.height)?;
    let owner_loss = observe_owner_loss();
    let browser = resolve_browser()?;
    let output = capture(args, &browser, &owner_loss, CAPTURE_TIMEOUT)?;
    println!(
        "Captured {} at {}x{} with {}",
        output.display(),
        args.width,
        args.height,
        browser.display()
    );
    Ok(())
}

fn capture(
    args: &ScreenshotArgs,
    browser: &Path,
    owner_loss: &Receiver<()>,
    timeout: Duration,
) -> Result<PathBuf> {
    let source = resolve_source(&args.source)?;
    let output = absolute_output(&args.output)?;
    let parent = output
        .parent()
        .context("screenshot output must have a parent directory")?;
    fs::create_dir_all(parent)
        .with_context(|| format!("create screenshot directory {}", parent.display()))?;
    let stage = staged_output(parent)?;
    let profile = tempfile::tempdir().context("create the temporary browser profile")?;
    let mut log = tempfile::tempfile().context("create the browser capture log")?;

    let mut command = Command::new(browser);
    command
        .arg("--headless")
        .arg("--hide-scrollbars")
        .arg(format!("--window-size={},{}", args.width, args.height))
        .arg(format!("--user-data-dir={}", profile.path().display()))
        .arg(format!("--screenshot={}", stage.display()))
        .arg(source.as_str())
        .stdin(Stdio::null())
        .stdout(Stdio::from(
            log.try_clone().context("clone browser capture log")?,
        ))
        .stderr(Stdio::from(
            log.try_clone().context("clone browser capture log")?,
        ));
    #[cfg(unix)]
    {
        use std::os::unix::process::CommandExt;
        command.process_group(0);
    }

    let mut child = command
        .spawn()
        .with_context(|| format!("launch browser backend {}", browser.display()))?;
    let process_group = ProcessGroupGuard::new(child.id());
    let started = Instant::now();

    let status = loop {
        if owner_loss.try_recv().is_ok() {
            process_group.terminate();
            let _ = child.wait();
            bail!("screenshot cancelled because its owner exited");
        }
        if started.elapsed() >= timeout {
            process_group.terminate();
            let _ = child.wait();
            bail!(
                "screenshot exceeded its fixed {}-second lifetime",
                timeout.as_secs_f64()
            );
        }
        if let Some(status) = child.try_wait().context("wait for browser capture")? {
            break status;
        }
        thread::sleep(POLL_INTERVAL);
    };

    // The leader may exit before a renderer or crash handler. The process
    // group belongs only to this capture, so every terminal path closes it.
    process_group.terminate();
    if !status.success() {
        bail!(
            "browser capture failed with {status}: {}",
            capture_log(&mut log)
        );
    }
    validate_png(&stage)?;
    stage
        .persist(&output)
        .map_err(|error| anyhow!("publish screenshot {}: {}", output.display(), error.error))?;
    Ok(output)
}

fn validate_viewport(width: u32, height: u32) -> Result<()> {
    if !(1..=MAX_VIEWPORT_DIMENSION).contains(&width) {
        bail!("screenshot width must be between 1 and {MAX_VIEWPORT_DIMENSION} pixels");
    }
    if !(1..=MAX_VIEWPORT_DIMENSION).contains(&height) {
        bail!("screenshot height must be between 1 and {MAX_VIEWPORT_DIMENSION} pixels");
    }
    Ok(())
}

fn observe_owner_loss() -> Receiver<()> {
    let (sender, receiver) = mpsc::channel();
    thread::spawn(move || {
        let mut stdin = std::io::stdin().lock();
        let mut buffer = [0_u8; 1];
        loop {
            match stdin.read(&mut buffer) {
                Ok(0) | Err(_) => {
                    let _ = sender.send(());
                    return;
                }
                Ok(_) => {}
            }
        }
    });
    receiver
}

fn resolve_browser() -> Result<PathBuf> {
    if let Some(candidate) = which_on_path(Path::new(BROWSER_NAME)) {
        if let Some(browser) = isolated_browser(candidate) {
            return Ok(browser);
        }
    }

    let roots = playwright_browser_roots();
    for root in &roots {
        let mut candidates = Vec::new();
        collect_browser_candidates(root, 0, &mut candidates);
        sort_browser_candidates(&mut candidates);
        for candidate in candidates {
            if let Some(browser) = isolated_browser(candidate) {
                return Ok(browser);
            }
        }
    }

    let searched = roots
        .iter()
        .map(|root| root.display().to_string())
        .collect::<Vec<_>>()
        .join(", ");
    bail!(
        "could not find {BROWSER_NAME} on PATH or in Playwright caches ({searched}); install it with `playwright install --only-shell chromium`"
    )
}

fn playwright_browser_roots() -> Vec<PathBuf> {
    let mut roots = Vec::new();
    if let Some(path) = std::env::var_os("PLAYWRIGHT_BROWSERS_PATH") {
        if !path.is_empty() && path != "0" {
            roots.push(PathBuf::from(path));
        }
    }
    if let Some(cache) = dirs::cache_dir() {
        let standard = cache.join("ms-playwright");
        if !roots.contains(&standard) {
            roots.push(standard);
        }
    }
    roots
}

fn sort_browser_candidates(candidates: &mut [PathBuf]) {
    candidates.sort_by(|left, right| {
        playwright_revision(right)
            .cmp(&playwright_revision(left))
            .then_with(|| right.cmp(left))
    });
}

fn playwright_revision(path: &Path) -> Option<u64> {
    path.ancestors().find_map(|ancestor| {
        ancestor
            .file_name()?
            .to_str()?
            .strip_prefix("chromium_headless_shell-")?
            .parse()
            .ok()
    })
}

fn collect_browser_candidates(root: &Path, depth: usize, candidates: &mut Vec<PathBuf>) {
    if depth > 4 {
        return;
    }
    let Ok(entries) = fs::read_dir(root) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_file() && path.file_name().is_some_and(|name| name == BROWSER_NAME) {
            candidates.push(path);
        } else if path.is_dir() {
            collect_browser_candidates(&path, depth + 1, candidates);
        }
    }
}

fn isolated_browser(candidate: PathBuf) -> Option<PathBuf> {
    let canonical = candidate.canonicalize().ok()?;
    (canonical.is_file()
        && canonical
            .file_name()
            .is_some_and(|name| name == BROWSER_NAME))
    .then_some(canonical)
}

fn resolve_source(source: &str) -> Result<Url> {
    if let Ok(url) = Url::parse(source) {
        return Ok(url);
    }
    let path = Path::new(source)
        .canonicalize()
        .with_context(|| format!("resolve screenshot source {source}"))?;
    Url::from_file_path(&path).map_err(|_| {
        anyhow!(
            "cannot convert screenshot source {} to a file URL",
            path.display()
        )
    })
}

fn absolute_output(output: &Path) -> Result<PathBuf> {
    if output.is_absolute() {
        Ok(output.to_path_buf())
    } else {
        Ok(std::env::current_dir()
            .context("resolve the current directory")?
            .join(output))
    }
}

fn staged_output(parent: &Path) -> Result<TempPath> {
    Builder::new()
        .prefix(".lf-screenshot-")
        .suffix(".png")
        .tempfile_in(parent)
        .context("create screenshot staging file")
        .map(|file| file.into_temp_path())
}

fn validate_png(path: &Path) -> Result<()> {
    let mut file = File::open(path)
        .with_context(|| format!("browser did not create screenshot {}", path.display()))?;
    let mut signature = [0_u8; PNG_SIGNATURE.len()];
    file.read_exact(&mut signature).with_context(|| {
        format!(
            "browser created an incomplete screenshot {}",
            path.display()
        )
    })?;
    if &signature != PNG_SIGNATURE {
        bail!("browser output {} is not a PNG", path.display());
    }
    Ok(())
}

fn capture_log(log: &mut File) -> String {
    let _ = log.seek(SeekFrom::Start(0));
    let mut text = String::new();
    let _ = log.read_to_string(&mut text);
    let trimmed = text.trim();
    if trimmed.is_empty() {
        "no browser output".to_string()
    } else {
        trimmed.to_string()
    }
}

fn parent_process_id() -> Option<u32> {
    #[cfg(unix)]
    {
        // SAFETY: getppid has no preconditions and does not dereference memory.
        let pid = unsafe { libc::getppid() };
        u32::try_from(pid).ok().filter(|pid| *pid > 1)
    }
    #[cfg(not(unix))]
    None
}

#[cfg(test)]
mod tests {
    use std::fs;
    #[cfg(unix)]
    use std::os::unix::fs::symlink;
    #[cfg(unix)]
    use std::os::unix::fs::PermissionsExt;
    use std::path::PathBuf;
    #[cfg(unix)]
    use std::sync::mpsc;
    #[cfg(unix)]
    use std::thread;
    #[cfg(unix)]
    use std::time::{Duration, Instant};

    #[cfg(unix)]
    use super::capture;
    use super::{
        isolated_browser, resolve_source, sort_browser_candidates, validate_viewport, BROWSER_NAME,
    };
    #[cfg(unix)]
    use crate::lf::ScreenshotArgs;
    #[cfg(unix)]
    #[test]
    fn viewport_is_bounded() {
        assert!(validate_viewport(1, 16_384).is_ok());
        assert!(validate_viewport(0, 900).is_err());
        assert!(validate_viewport(1440, 16_385).is_err());
    }

    #[test]
    fn local_source_becomes_a_file_url() {
        let directory = tempfile::tempdir().unwrap();
        let source = directory.path().join("page.html");
        fs::write(&source, "<h1>proof</h1>").unwrap();

        let resolved = resolve_source(source.to_str().unwrap()).unwrap();

        assert_eq!(resolved.scheme(), "file");
        assert!(resolved.path().ends_with("/page.html"));
    }

    #[test]
    fn playwright_candidates_use_numeric_revision_order() {
        let mut candidates = vec![
            PathBuf::from(
                "/cache/chromium_headless_shell-9999/chrome-headless-shell/chrome-headless-shell",
            ),
            PathBuf::from(
                "/cache/chromium_headless_shell-10000/chrome-headless-shell/chrome-headless-shell",
            ),
        ];

        sort_browser_candidates(&mut candidates);

        assert!(candidates[0]
            .to_string_lossy()
            .contains("chromium_headless_shell-10000"));
    }

    #[cfg(unix)]
    #[test]
    fn branded_chrome_symlink_is_not_an_isolated_backend() {
        let directory = tempfile::tempdir().unwrap();
        let branded = directory.path().join("Google Chrome");
        fs::write(&branded, "browser").unwrap();
        let alias = directory.path().join(BROWSER_NAME);
        symlink(&branded, &alias).unwrap();

        assert!(isolated_browser(alias).is_none());
    }

    #[cfg(unix)]
    #[test]
    fn timeout_reaps_the_fake_browser_group_and_preserves_output() {
        let directory = tempfile::tempdir().unwrap();
        let browser = directory.path().join(BROWSER_NAME);
        let browser_pid = directory.path().join("browser.pid");
        let descendant_pid = directory.path().join("descendant.pid");
        fs::write(
            &browser,
            format!(
                "#!/bin/sh\nprintf '%s' \"$$\" > '{}'\nsleep 300 &\nprintf '%s' \"$!\" > '{}'\nwait\n",
                browser_pid.display(),
                descendant_pid.display()
            ),
        )
        .unwrap();
        fs::set_permissions(&browser, fs::Permissions::from_mode(0o755)).unwrap();
        let output = directory.path().join("capture.png");
        fs::write(&output, b"prior image").unwrap();
        let args = ScreenshotArgs {
            source: "about:blank".to_string(),
            output: output.clone(),
            width: 390,
            height: 844,
        };
        let (_owner, owner_loss) = mpsc::channel();

        let error = capture(&args, &browser, &owner_loss, Duration::from_millis(500)).unwrap_err();

        assert!(error.to_string().contains("fixed 0.5-second lifetime"));
        assert_eq!(fs::read(&output).unwrap(), b"prior image");
        for pid_file in [&browser_pid, &descendant_pid] {
            let pid = fs::read_to_string(pid_file)
                .unwrap()
                .parse::<i32>()
                .unwrap();
            let deadline = Instant::now() + Duration::from_secs(2);
            while process_exists(pid) && Instant::now() < deadline {
                thread::sleep(Duration::from_millis(10));
            }
            assert!(!process_exists(pid), "process {pid} survived timeout");
        }
    }

    #[cfg(unix)]
    fn process_exists(pid: i32) -> bool {
        // SAFETY: signal 0 only probes a numeric pid and does not deliver a signal.
        let result = unsafe { libc::kill(pid, 0) };
        result == 0 || std::io::Error::last_os_error().raw_os_error() == Some(libc::EPERM)
    }
}
