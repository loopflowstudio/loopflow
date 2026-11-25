//! The selected lf entry owns service setup. Only the default Machine (`~/.lf`)
//! is registered with launchd or systemd; custom `LF_HOME` servers run as
//! detached processes and exit when idle or when their home is removed.
use anyhow::{Context, Result};
use fs2::FileExt;
use sha2::{Digest, Sha256};
use std::fs;
use std::os::unix::fs::OpenOptionsExt;
use std::path::Path;
use std::process::Command;
use std::time::{Duration, Instant};

mod detached;
#[cfg(target_os = "linux")]
mod linux;

pub fn ensure() -> Result<crate::machine::client::Status> {
    let home = crate::store::lf_home_dir();
    fs::create_dir_all(&home).with_context(|| format!("create Machine home {}", home.display()))?;
    let home = home.canonicalize()?;
    let lock = fs::OpenOptions::new()
        .create(true)
        .truncate(false)
        .read(true)
        .write(true)
        .mode(0o600)
        .open(crate::machine::socket_path()?.with_extension("install-lock"))?;
    lock.lock_exclusive()?;
    // A server that answers is ready for every client; only this command
    // also asks that it be the selected installation, replacing one left
    // running by the build before a promotion.
    let selected = crate::installation::ArtifactIdentity::capture(
        crate::installation::ArtifactRole::Cli,
        &std::env::current_exe()?,
    )?
    .sha256;
    let installed = |status: &crate::machine::client::Status| {
        status.ready && status.executable_sha256.as_ref() == Some(&selected)
    };
    let before = crate::machine::client::status()?;
    if installed(&before) {
        return Ok(before);
    }
    let executable = crate::os_process::resolve_lf_binary()?;
    register(&home, &executable, before.listening)?;
    let deadline = Instant::now() + Duration::from_secs(10);
    loop {
        let status = crate::machine::client::status()?;
        if installed(&status) {
            return Ok(status);
        }
        if Instant::now() >= deadline {
            if status.ready {
                anyhow::bail!(
                    "Loopflow server did not start within 10s: another executable still answers"
                );
            }
            // A socket that never listened has no reason beyond the wait.
            match status.error.filter(|_| status.listening) {
                Some(error) => anyhow::bail!("Loopflow server did not start within 10s: {error}"),
                None => anyhow::bail!("Loopflow server did not start within 10s"),
            }
        }
        std::thread::sleep(Duration::from_millis(100));
    }
}

fn label(home: &Path, namespace: &str) -> String {
    let digest = hex::encode(Sha256::digest(home.as_os_str().as_encoded_bytes()));
    format!("com.loopflow.{namespace}.{}", &digest[..16])
}

fn is_legacy_custom_label(name: &str) -> bool {
    ["com.loopflow.server.", "com.loopflow.machine."]
        .iter()
        .any(|prefix| {
            name.strip_prefix(prefix).is_some_and(|suffix| {
                suffix.len() == 16 && suffix.bytes().all(|byte| byte.is_ascii_hexdigit())
            })
        })
}

fn register(home: &Path, executable: &Path, replace: bool) -> Result<()> {
    if crate::store::custom_home_selected() {
        retire_legacy_custom_home(home)?;
        return detached::start(home, executable, replace);
    }
    register_default(home, executable, replace)
}

#[cfg(target_os = "macos")]
fn register_default(_home: &Path, _executable: &Path, replace: bool) -> Result<()> {
    register_bundled(replace)
}

#[cfg(target_os = "linux")]
fn register_default(home: &Path, executable: &Path, replace: bool) -> Result<()> {
    linux::register_default(home, executable, replace)
}

/// Boot out any legacy hashed launchd job left for `home` by an earlier `lf`
/// and remove its stale plist files.
#[cfg(target_os = "macos")]
fn retire_legacy_custom_home(home: &Path) -> Result<()> {
    // SAFETY: geteuid has no preconditions.
    let domain = format!("gui/{}", unsafe { libc::geteuid() });
    for namespace in ["server", "machine"] {
        let target = format!("{domain}/{}", label(home, namespace));
        let loaded = Command::new("/bin/launchctl")
            .args(["print", &target])
            .output()
            .is_ok_and(|output| output.status.success());
        if loaded {
            let bootout = Command::new("/bin/launchctl")
                .args(["bootout", &target])
                .output()
                .context("retire legacy Loopflow launchd job")?;
            if !bootout.status.success()
                && Command::new("/bin/launchctl")
                    .args(["print", &target])
                    .output()
                    .is_ok_and(|output| output.status.success())
            {
                anyhow::bail!(
                    "Server supervisor: {}",
                    String::from_utf8_lossy(&bootout.stderr)
                );
            }
        }
    }
    for plist in ["server.plist", "machine-service.plist"] {
        let path = home.join(plist);
        if path.exists() {
            let _ = fs::remove_file(path);
        }
    }
    Ok(())
}

#[cfg(target_os = "linux")]
fn retire_legacy_custom_home(home: &Path) -> Result<()> {
    linux::retire_legacy_custom_home(home)
}

/// When a server supervised under a legacy custom-home job discovers its home
/// directory has been removed, boot out its own label so the OS stops
/// crash-looping it. Never touches the default `com.loopflow.server` job.
#[cfg(target_os = "macos")]
pub fn retire_self_if_registered() {
    let Ok(name) = std::env::var("XPC_SERVICE_NAME") else {
        return;
    };
    if !is_legacy_custom_label(&name) {
        return;
    }
    // SAFETY: geteuid has no preconditions.
    let target = format!("gui/{}/{name}", unsafe { libc::geteuid() });
    let _ = Command::new("/bin/launchctl")
        .args(["bootout", &target])
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status();
}

#[cfg(target_os = "linux")]
pub fn retire_self_if_registered() {
    linux::retire_self_if_registered();
}

#[cfg(target_os = "macos")]
fn register_bundled(replace: bool) -> Result<()> {
    use std::ffi::CStr;
    unsafe extern "C" {
        fn lf_server_register(
            plist_name: *const std::ffi::c_char,
            legacy_plist: *const std::ffi::c_char,
            replace: bool,
            error: *mut std::ffi::c_char,
            capacity: usize,
        ) -> i32;
    }
    let mut error = [0 as std::ffi::c_char; 1024];
    // SAFETY: the native function writes at most capacity bytes and NUL terminates.
    let result = unsafe {
        lf_server_register(
            c"com.loopflow.server.plist".as_ptr(),
            c"com.loopflow.machine.plist".as_ptr(),
            replace,
            error.as_mut_ptr(),
            error.len(),
        )
    };
    // SAFETY: the buffer is initialized to NUL and the native function preserves termination.
    let message = unsafe { CStr::from_ptr(error.as_ptr()) }.to_string_lossy();
    anyhow::ensure!(result == 0, "{message}");
    Ok(())
}

fn checked(command: &mut Command) -> Result<()> {
    let output = command.output().context("start Server supervisor")?;
    anyhow::ensure!(
        output.status.success(),
        "Server supervisor: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    Ok(())
}

/// Reconcile an existing service through the newly selected CLI after the
/// promotion lock is released. First installation remains demand-driven.
pub fn refresh_after_promotion() {
    let refresh = || -> Result<()> {
        if !crate::machine::socket_path()?.exists() {
            return Ok(());
        }
        let executable = crate::installation::installed_cli()?;
        let mut command = Command::new(executable);
        for (key, _) in std::env::vars_os() {
            if key.to_string_lossy().starts_with("LF_") {
                command.env_remove(key);
            }
        }
        command.args(["install", "server"]);
        checked(&mut command)
    };
    if let Err(error) = refresh() {
        eprintln!("warning: installed Loopflow server is unavailable ({error:#}); `lf server status` reports readiness");
    }
}

/// Default-Machine server registration always belongs to the promoted CLI.
/// An explicit experimental Machine keeps its selected source executable.
pub fn delegate_selected() -> Result<()> {
    use std::os::unix::process::CommandExt;
    if crate::store::custom_home_selected() {
        return Ok(());
    }
    let selected = crate::installation::installed_cli().context(
        "Loopflow server needs an installed lf; source experiments select an explicit LF_HOME",
    )?;
    if selected == fs::canonicalize(std::env::current_exe()?)? {
        return Ok(());
    }
    Err(Command::new(selected)
        .args(std::env::args_os().skip(1))
        .exec())
    .context("start the selected installation's Loopflow server")
}

pub(crate) fn readiness_command() -> Result<std::process::Command> {
    let mut command = std::process::Command::new(crate::os_process::resolve_lf_binary()?);
    command
        .args(["install", "server"])
        .stdin(std::process::Stdio::null());
    Ok(command)
}

#[cfg(test)]
mod tests {
    use super::{is_legacy_custom_label, label};
    use std::path::Path;

    #[test]
    fn legacy_custom_label_matches_only_hashed_labels() {
        let server = label(Path::new("/tmp/custom-home"), "server");
        let machine = label(Path::new("/tmp/custom-home"), "machine");
        assert!(is_legacy_custom_label(&server));
        assert!(is_legacy_custom_label(&machine));
        assert!(!is_legacy_custom_label("com.loopflow.server"));
        assert!(!is_legacy_custom_label("com.loopflow.machine"));
        assert!(!is_legacy_custom_label(
            "com.loopflow.server-proof.3faef379736c4f0b9c9d5cb626fa8810"
        ));
        assert!(!is_legacy_custom_label("com.loopflow.server.short"));
        assert!(!is_legacy_custom_label(
            "com.loopflow.server.zzzzzzzzzzzzzzzz"
        ));
    }
}
