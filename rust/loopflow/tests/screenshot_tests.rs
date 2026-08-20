#![cfg(unix)]

use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::process::{Command, Output, Stdio};
use std::thread;
use std::time::{Duration, Instant};

const PNG_SIGNATURE: &[u8; 8] = b"\x89PNG\r\n\x1a\n";

struct CaptureFixture {
    directory: tempfile::TempDir,
    browser_dir: PathBuf,
    source: PathBuf,
    output: PathBuf,
    browser_pid: PathBuf,
    descendant_pid: PathBuf,
}

impl CaptureFixture {
    fn new() -> Self {
        let directory = tempfile::tempdir().unwrap();
        let browser_dir = directory.path().join("bin");
        fs::create_dir(&browser_dir).unwrap();
        let browser = browser_dir.join("chrome-headless-shell");
        fs::write(
            &browser,
            r#"#!/bin/sh
set -eu
printf '%s' "$$" > "$LF_SCREENSHOT_BROWSER_PID_FILE"
sleep 300 &
descendant=$!
printf '%s' "$descendant" > "$LF_SCREENSHOT_DESCENDANT_PID_FILE"

if [ "$LF_SCREENSHOT_FAKE_MODE" = "success" ]; then
    screenshot=""
    for argument in "$@"; do
        case "$argument" in
            --screenshot=*) screenshot=${argument#--screenshot=} ;;
        esac
    done
    printf '\211PNG\r\n\032\nproof' > "$screenshot"
    exit 0
fi

wait "$descendant"
"#,
        )
        .unwrap();
        fs::set_permissions(&browser, fs::Permissions::from_mode(0o755)).unwrap();

        let source = directory.path().join("page.html");
        fs::write(&source, "<h1>capture proof</h1>").unwrap();

        Self {
            browser_dir,
            source,
            output: directory.path().join("capture.png"),
            browser_pid: directory.path().join("browser.pid"),
            descendant_pid: directory.path().join("descendant.pid"),
            directory,
        }
    }

    fn command(&self, mode: &str) -> Command {
        let mut command = Command::new(env!("CARGO_BIN_EXE_lf"));
        command
            .args(["screenshot", self.source.to_str().unwrap(), "--output"])
            .arg(&self.output)
            .args(["--width", "390", "--height", "844"])
            .env(
                "PATH",
                std::env::join_paths(std::iter::once(self.browser_dir.clone()).chain(
                    std::env::split_paths(&std::env::var_os("PATH").unwrap_or_default()),
                ))
                .unwrap(),
            )
            .env("LF_SCREENSHOT_FAKE_MODE", mode)
            .env("LF_SCREENSHOT_BROWSER_PID_FILE", &self.browser_pid)
            .env("LF_SCREENSHOT_DESCENDANT_PID_FILE", &self.descendant_pid)
            .env_remove("PLAYWRIGHT_BROWSERS_PATH")
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());
        command
    }

    fn wait_for_browser_tree(&self) -> (u32, u32) {
        (
            wait_for_pid_file(&self.browser_pid),
            wait_for_pid_file(&self.descendant_pid),
        )
    }
}

#[derive(Default)]
struct ProcessCleanup(Vec<u32>);

impl ProcessCleanup {
    fn add(&mut self, pid: u32) {
        self.0.push(pid);
    }

    fn disarm(mut self) {
        self.0.clear();
    }
}

impl Drop for ProcessCleanup {
    fn drop(&mut self) {
        for pid in self.0.iter().copied() {
            signal(pid, libc::SIGKILL);
        }
    }
}

#[test]
fn screenshot_uses_the_isolated_backend_and_reaps_success_descendants() {
    let fixture = CaptureFixture::new();

    let output = fixture.command("success").output().unwrap();

    assert_success(&output);
    let png = fs::read(&fixture.output).unwrap();
    assert!(png.starts_with(PNG_SIGNATURE));
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("390x844"));
    assert!(stdout.contains(
        fixture
            .browser_dir
            .join("chrome-headless-shell")
            .to_str()
            .unwrap()
    ));
    let descendant = wait_for_pid_file(&fixture.descendant_pid);
    assert_process_gone(descendant);
}

#[test]
fn killing_public_capture_leaves_no_browser_tree_or_partial_output() {
    let fixture = CaptureFixture::new();
    fs::write(&fixture.output, b"prior image").unwrap();
    let mut cleanup = ProcessCleanup::default();
    let mut capture = fixture.command("hold").spawn().unwrap();
    cleanup.add(capture.id());
    let (browser, descendant) = fixture.wait_for_browser_tree();
    cleanup.add(browser);
    cleanup.add(descendant);

    signal(capture.id(), libc::SIGKILL);
    capture.wait().unwrap();

    assert_process_gone(browser);
    assert_process_gone(descendant);
    assert_eq!(fs::read(&fixture.output).unwrap(), b"prior image");
    cleanup.disarm();
}

#[test]
fn killing_invoking_parent_leaves_no_browser_tree_or_partial_output() {
    let fixture = CaptureFixture::new();
    fs::write(&fixture.output, b"prior image").unwrap();
    let caller_script = fixture.directory.path().join("caller.sh");
    let public_pid_file = fixture.directory.path().join("public.pid");
    fs::write(
        &caller_script,
        r#"#!/bin/sh
"$LF_SCREENSHOT_BIN" screenshot "$LF_SCREENSHOT_SOURCE" --output "$LF_SCREENSHOT_OUTPUT" --width 390 --height 844 &
capture=$!
printf '%s' "$capture" > "$LF_SCREENSHOT_PUBLIC_PID_FILE"
wait "$capture"
"#,
    )
    .unwrap();
    fs::set_permissions(&caller_script, fs::Permissions::from_mode(0o755)).unwrap();

    let mut command = Command::new(&caller_script);
    let configured = fixture.command("hold");
    for (key, value) in configured.get_envs() {
        match value {
            Some(value) => {
                command.env(key, value);
            }
            None => {
                command.env_remove(key);
            }
        }
    }
    command
        .env("LF_SCREENSHOT_BIN", env!("CARGO_BIN_EXE_lf"))
        .env("LF_SCREENSHOT_SOURCE", &fixture.source)
        .env("LF_SCREENSHOT_OUTPUT", &fixture.output)
        .env("LF_SCREENSHOT_PUBLIC_PID_FILE", &public_pid_file)
        .stdout(Stdio::null())
        .stderr(Stdio::null());

    let mut cleanup = ProcessCleanup::default();
    let mut caller = command.spawn().unwrap();
    cleanup.add(caller.id());
    let public = wait_for_pid_file(&public_pid_file);
    cleanup.add(public);
    let (browser, descendant) = fixture.wait_for_browser_tree();
    cleanup.add(browser);
    cleanup.add(descendant);

    signal(caller.id(), libc::SIGKILL);
    caller.wait().unwrap();

    assert_process_gone(public);
    assert_process_gone(browser);
    assert_process_gone(descendant);
    assert_eq!(fs::read(&fixture.output).unwrap(), b"prior image");
    cleanup.disarm();
}

fn wait_for_pid_file(path: &Path) -> u32 {
    // Process startup is not part of the two-second cleanup contract and can
    // contend when these independent capture cases run in parallel.
    let deadline = Instant::now() + Duration::from_secs(10);
    loop {
        if let Ok(text) = fs::read_to_string(path) {
            if let Ok(pid) = text.parse() {
                return pid;
            }
        }
        assert!(
            Instant::now() < deadline,
            "process did not record its pid in {}",
            path.display()
        );
        thread::sleep(Duration::from_millis(10));
    }
}

fn assert_process_gone(pid: u32) {
    let deadline = Instant::now() + Duration::from_secs(2);
    while process_exists(pid) && Instant::now() < deadline {
        thread::sleep(Duration::from_millis(10));
    }
    assert!(!process_exists(pid), "process {pid} survived owner cleanup");
}

fn process_exists(pid: u32) -> bool {
    let Ok(pid) = i32::try_from(pid) else {
        return false;
    };
    // SAFETY: signal 0 only probes a numeric pid and does not deliver a signal.
    let result = unsafe { libc::kill(pid, 0) };
    result == 0 || std::io::Error::last_os_error().raw_os_error() == Some(libc::EPERM)
}

fn signal(pid: u32, signal: i32) {
    if let Ok(pid) = i32::try_from(pid) {
        // SAFETY: the tests record pids for only their own short-lived fake
        // browser processes and capture launchers.
        unsafe {
            libc::kill(pid, signal);
        }
    }
}

fn assert_success(output: &Output) {
    assert!(
        output.status.success(),
        "lf screenshot failed:\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
}
