//! Explicit macOS acceptance support. Only synthetic, uniquely scoped items are
//! touched; ordinary unit tests never opt into the native credential dispatch.

use std::io::{Read, Write};
use std::os::unix::process::CommandExt;
use std::path::{Path, PathBuf};
use std::process::{Command, Output, Stdio};
use std::sync::OnceLock;
use std::time::{Duration, Instant};

use serde::{Deserialize, Serialize};
use serde_json::json;

use super::{claude_keychain_service, parse_security_keychain_account};

static NATIVE_HOME: OnceLock<PathBuf> = OnceLock::new();
pub(crate) const CHILD_ROOT: &str = "LOOPFLOW_NATIVE_KEYCHAIN_FIXTURE";

pub(crate) fn is_native_home(path: &Path) -> bool {
    NATIVE_HOME.get().is_some_and(|home| home == path)
}

pub(crate) fn select_native_home(path: PathBuf) {
    assert!(NATIVE_HOME.set(path).is_ok(), "one native home per child");
}

#[link(name = "Security", kind = "framework")]
unsafe extern "C" {
    fn SecKeychainSetUserInteractionAllowed(allowed: u8) -> i32;
}

pub(crate) fn disable_keychain_ui() {
    // SAFETY: this API takes a Boolean and changes only the isolated child's
    // process-global setting. The parent also bounds/reaps security descendants.
    assert_eq!(unsafe { SecKeychainSetUserInteractionAllowed(0) }, 0);
}

/// All fields are allowlisted observations, never credential bytes or diagnostics.
#[derive(Debug, Default, Deserialize, Serialize)]
pub(crate) struct Observation {
    pub native_read: bool,
    pub accepted: bool,
    pub unavailable: bool,
    pub persistence_diagnostic: bool,
    pub no_reconnect: bool,
    pub identity_and_usage: bool,
    pub refresh_requests: usize,
    pub credential_file_absent: bool,
}

#[derive(Debug, Deserialize, Serialize)]
pub(crate) struct Case {
    pub size: usize,
    pub invalid: bool,
    pub unicode: bool,
}

pub(crate) fn credentials(size: usize, unicode: bool) -> (String, String, String) {
    let old = json!({
        "claudeAiOauth": {
            "accessToken": "fixture-old", "refreshToken": "fixture-refresh",
            "expiresAt": 1, "scopes": ["user:profile", "user:inference"]
        },
        "unrelated": {"text": if unicode { "雪\n\"\\" } else { "text\n\"\\" }, "enabled": true}
    });
    let mut new = old.clone();
    new["claudeAiOauth"]
        .as_object_mut()
        .unwrap()
        .remove("expiresAt");
    new["claudeAiOauth"]["accessToken"] = json!("");
    let access = "x".repeat(size.checked_sub(new.to_string().len()).unwrap());
    new["claudeAiOauth"]["accessToken"] = json!(access);
    let new = new.to_string();
    assert_eq!(new.len(), size);
    // Start with printable JSON in both cases; refreshing naturally serializes
    // the escaped Unicode field as UTF-8 and exercises the reader's encoding.
    (old.to_string().replace('雪', "\\u96ea"), new, access)
}

fn security(args: &[&str], input: &[u8]) -> Output {
    run(Command::new("/usr/bin/security").args(args), input)
}

/// Drain stdout, discard stderr and reap the process group even on a timeout.
/// Never include command output in an assertion or error message.
fn run(command: &mut Command, input: &[u8]) -> Output {
    let mut child = command
        .process_group(0)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .expect("start isolated native fixture command");
    let mut stdout = child.stdout.take().unwrap();
    let out = std::thread::spawn(move || {
        let mut bytes = Vec::new();
        stdout.read_to_end(&mut bytes).unwrap();
        bytes
    });
    // Only short setup scripts are supplied here. Refresh payloads are served
    // over loopback, and the production writer owns their transport.
    let input_ok = child.stdin.take().unwrap().write_all(input).is_ok();
    let deadline = Instant::now() + Duration::from_secs(15);
    let status = loop {
        match child.try_wait() {
            Ok(Some(status)) => break status,
            Ok(None) if input_ok && Instant::now() < deadline => {
                std::thread::sleep(Duration::from_millis(20));
            }
            _ => {
                // SAFETY: this child was created as leader of its own process
                // group; the negative pid addresses only its descendants.
                unsafe { libc::kill(-(child.id() as i32), libc::SIGKILL) };
                break child.wait().expect("reap native fixture process group");
            }
        }
    };
    // SAFETY: only this command's process group is addressed. Stop descendants
    // even if their parent exited before closing the inherited output pipes.
    unsafe { libc::kill(-(child.id() as i32), libc::SIGKILL) };
    Output {
        status,
        stdout: out.join().unwrap(),
        stderr: Vec::new(),
    }
}

struct Item {
    service: String,
    account: String,
    removed: bool,
}

impl Item {
    fn create(home: &Path, account: &str, old: &str) -> Self {
        let service = claude_keychain_service(home);
        // A hash collision is not permission to overwrite somebody else's item.
        assert_eq!(
            security(&["find-generic-password", "-s", &service], &[])
                .status
                .code(),
            Some(44),
            "native fixture service must be absent before setup"
        );
        let mut item = Self {
            service,
            account: account.into(),
            removed: false,
        };
        let script = format!(
            "add-generic-password -a \"{}\" -s \"{}\" -X {}\n",
            item.account,
            item.service,
            hex::encode(old.as_bytes())
        );
        assert!(script.len() < 4096);
        let output = security(&["-i"], script.as_bytes());
        let stored = item.read();
        if !output.status.success() || stored.as_deref() != Some(old.as_bytes()) {
            eprintln!(
                "native setup: write_status={:?}, expected_bytes={}, read_bytes={:?}, exact={}",
                output.status.code(),
                old.len(),
                stored.as_ref().map(Vec::len),
                stored.as_deref() == Some(old.as_bytes())
            );
            let _ = item.remove();
            panic!("native Keychain setup unavailable; no acceptance cases completed");
        }
        item
    }

    fn read(&self) -> Option<Vec<u8>> {
        let output = security(&["find-generic-password", "-s", &self.service, "-w"], &[]);
        if !output.status.success() {
            return None;
        }
        let bytes = output.stdout.strip_suffix(b"\n")?;
        // security -w hex-renders non-printable bytes. This independent observer
        // compares actual bytes, even if the production reader rejects that form.
        Some(if bytes.starts_with(b"{") {
            bytes.to_vec()
        } else {
            hex::decode(bytes).ok()?
        })
    }

    fn account_unchanged(&self) -> bool {
        let output = security(&["find-generic-password", "-s", &self.service], &[]);
        output.status.success()
            && String::from_utf8(output.stdout)
                .ok()
                .and_then(|text| parse_security_keychain_account(&text))
                .as_deref()
                == Some(&self.account)
    }

    fn remove(&mut self) -> bool {
        let deleted = security(
            &[
                "delete-generic-password",
                "-s",
                &self.service,
                "-a",
                &self.account,
            ],
            &[],
        );
        self.removed = (deleted.status.success() || deleted.status.code() == Some(44))
            && security(&["find-generic-password", "-s", &self.service], &[])
                .status
                .code()
                == Some(44);
        self.removed
    }
}

impl Drop for Item {
    fn drop(&mut self) {
        if !self.removed && !self.remove() {
            eprintln!(
                "native fixture cleanup failed for disposable service {}",
                self.service
            );
        }
    }
}

pub(crate) fn run_cases(test_name: &str) {
    assert_eq!(
        std::env::var("LOOPFLOW_NETWORK_ISOLATED").as_deref(),
        Ok("1"),
        "run native acceptance through scripts/test_network.py"
    );
    let _env = loopflow_test_support::env_lock().lock().unwrap();
    let mut completed = 0;
    let mut passed = true;
    for (size, account, invalid, unicode) in [
        (512, "lf001", false, false),
        (2048, "lf001", false, false),
        (8192, "lf001", false, false),
        (512, "loopflow-native-fixture-long-account", false, false),
        (2048, "loopflow-native-fixture-long-account", false, false),
        (8192, "loopflow-native-fixture-long-account", false, false),
        (512, "lf001", false, true),
        (512, "lf001", true, false),
    ] {
        let root = tempfile::Builder::new()
            .prefix("loopflow-native-keychain-")
            .tempdir()
            .unwrap();
        let native = root.path().join("native");
        std::fs::create_dir(&native).unwrap();
        let (old, expected, _) = credentials(size, unicode);
        let mut item = Item::create(&native, account, &old);
        std::fs::write(
            root.path().join("case.json"),
            serde_json::to_vec(&Case {
                size,
                invalid,
                unicode,
            })
            .unwrap(),
        )
        .unwrap();
        let output = run(
            Command::new(std::env::current_exe().unwrap())
                .args(["--exact", test_name, "--ignored", "--nocapture"])
                .env(CHILD_ROOT, root.path())
                .env("PATH", "/usr/bin:/bin:/usr/sbin:/sbin")
                .env("USER", "different-fixture-user"),
            &[],
        );
        let observation = std::fs::read(root.path().join("observation.json"))
            .ok()
            .and_then(|bytes| serde_json::from_slice::<Observation>(&bytes).ok());
        let stored = item.read();
        let exact = stored.as_deref() == Some(expected.as_bytes());
        let old_readable = stored.as_deref() == Some(old.as_bytes());
        let account_preserved = item.account_unchanged();
        let cleaned = item.remove();
        let case_passed = output.status.success()
            && observation.as_ref().is_some_and(|o| {
                o.native_read
                    && o.credential_file_absent
                    && o.no_reconnect
                    && if invalid {
                        o.unavailable && old_readable && o.refresh_requests == 0
                    } else {
                        o.refresh_requests == 1 && o.accepted && o.identity_and_usage && exact
                    }
            })
            && account_preserved
            && cleaned;
        completed += usize::from(observation.is_some());
        passed &= case_passed;
        // Only booleans, counts and fixed case labels leave the fixture.
        println!(
            "{}",
            json!({"bytes": size, "long_account": account.len() > 5,
                "invalid_json": invalid, "unicode": unicode, "child_completed": output.status.success(),
            "observation": observation, "exact_replacement": exact,
            "old_readable": old_readable, "stored_bytes": stored.as_ref().map(Vec::len),
            "account_preserved": account_preserved, "cleaned": cleaned, "passed": case_passed})
        );
        if !cleaned {
            break;
        }
    }
    assert_eq!(completed, 8, "all native cases must execute");
    assert!(
        passed,
        "native credential acceptance failed; see sanitized observations"
    );
}
