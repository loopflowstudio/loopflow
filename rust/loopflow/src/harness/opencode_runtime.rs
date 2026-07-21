use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::{Duration, Instant};

use anyhow::{Context, Result};
use fs2::FileExt;
use serde::{Deserialize, Serialize};

const OPENCODE_REGISTRY_FILE: &str = "runtime/opencode-servers.json";
const REAP_TERM_GRACE: Duration = Duration::from_secs(2);

/// Outcome of reaping orphaned opencode `serve` processes. Public so the
/// per-wave runtime can call [`reap_orphaned_opencode_servers`] at startup to
/// clear servers — and their descendant process trees — left behind by a
/// crashed `lf wave`.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct OpenCodeReapReport {
    pub reaped: u32,
    pub errors: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub(crate) struct OpenCodeServerEntry {
    pub opencode_pid: u32,
    pub owner_loopflow_pid: u32,
}

pub(crate) fn registered_opencode_servers_at(lf_home: &Path) -> Result<Vec<OpenCodeServerEntry>> {
    let path = lf_home.join(OPENCODE_REGISTRY_FILE);
    let _lock = lock_registry_for_read(&path)?;
    read_registry_entries(&path)
}

pub(crate) fn register_opencode_server(opencode_pid: u32) -> Result<()> {
    register_opencode_server_at_path(&registry_path(), opencode_pid, std::process::id())
}

pub(crate) fn unregister_opencode_server(opencode_pid: u32) -> Result<()> {
    unregister_opencode_server_at_path(&registry_path(), opencode_pid)
}

pub fn reap_orphaned_opencode_servers() -> OpenCodeReapReport {
    reap_orphaned_opencode_servers_at(&crate::store::lf_home_dir())
}

pub(crate) fn reap_orphaned_opencode_servers_at(lf_home: &Path) -> OpenCodeReapReport {
    reap_orphaned_opencode_servers_at_path(
        &lf_home.join(OPENCODE_REGISTRY_FILE),
        |_| true,
        pid_is_alive,
        classify_leader,
        process_group_alive,
        terminate_process_group,
    )
}

pub(crate) fn reap_selected_orphaned_opencode_servers_at(
    lf_home: &Path,
    process_groups: &HashSet<u32>,
) -> OpenCodeReapReport {
    reap_orphaned_opencode_servers_at_path(
        &lf_home.join(OPENCODE_REGISTRY_FILE),
        |pid| process_groups.contains(&pid),
        pid_is_alive,
        classify_leader,
        process_group_alive,
        terminate_process_group,
    )
}

fn registry_path() -> PathBuf {
    crate::store::lf_home_dir().join(OPENCODE_REGISTRY_FILE)
}

fn register_opencode_server_at_path(
    path: &Path,
    opencode_pid: u32,
    owner_loopflow_pid: u32,
) -> Result<()> {
    let _lock = lock_registry(path)?;
    let mut entries = read_registry_entries(path)?;
    entries.retain(|entry| entry.opencode_pid != opencode_pid);
    entries.push(OpenCodeServerEntry {
        opencode_pid,
        owner_loopflow_pid,
    });
    write_registry_entries(path, &entries)
}

fn unregister_opencode_server_at_path(path: &Path, opencode_pid: u32) -> Result<()> {
    let _lock = lock_registry(path)?;
    let mut entries = read_registry_entries(path)?;
    let original_len = entries.len();
    entries.retain(|entry| entry.opencode_pid != opencode_pid);
    if entries.len() == original_len {
        return Ok(());
    }
    write_registry_entries(path, &entries)
}

/// How the leader PID of a registered OpenCode server presents itself when its
/// owner loopflow process is gone. The OpenCode harness spawns `opencode serve`
/// in its own process group (`process_group(0)`), so the registered pid is both
/// the leader and the process-group id; its descendants live in that group.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum LeaderState {
    /// PID no longer exists.
    Dead,
    /// PID alive and its command is `opencode ... serve`.
    Opencode,
    /// PID alive but running something else — recycled into an unrelated process.
    Other,
}

fn reap_orphaned_opencode_servers_at_path(
    path: &Path,
    eligible: impl Fn(u32) -> bool,
    owner_pid_alive: impl Fn(u32) -> bool,
    leader: impl Fn(u32) -> LeaderState,
    group_alive: impl Fn(u32) -> bool,
    terminate_group: impl Fn(u32) -> bool,
) -> OpenCodeReapReport {
    let mut report = OpenCodeReapReport::default();
    let _lock = match lock_registry(path) {
        Ok(lock) => lock,
        Err(err) => {
            tracing::warn!(path = %path.display(), error = %err, "failed to lock OpenCode registry");
            report.errors += 1;
            return report;
        }
    };
    let entries = match read_registry_entries(path) {
        Ok(entries) => entries,
        Err(err) => {
            tracing::warn!(path = %path.display(), error = %err, "failed to read OpenCode registry");
            report.errors += 1;
            return report;
        }
    };

    let mut retained = Vec::with_capacity(entries.len());
    for entry in entries {
        if !eligible(entry.opencode_pid) {
            retained.push(entry);
            continue;
        }
        if owner_pid_alive(entry.owner_loopflow_pid) {
            retained.push(entry);
            continue;
        }

        // Decide whether to reap this entry's process group. The group id is
        // the registered pid (the harness makes the child its own group
        // leader), so killing the group reaps the server AND any descendants
        // it spawned — accurately, without touching unrelated processes.
        let reap_group = match leader(entry.opencode_pid) {
            LeaderState::Opencode => true,
            LeaderState::Dead => group_alive(entry.opencode_pid),
            LeaderState::Other => {
                tracing::info!(
                    opencode_pid = entry.opencode_pid,
                    "orphaned OpenCode pid reused by an unrelated process; leaving it"
                );
                false
            }
        };

        if reap_group {
            if terminate_group(entry.opencode_pid) {
                report.reaped += 1;
            } else {
                tracing::warn!(
                    opencode_pid = entry.opencode_pid,
                    owner_loopflow_pid = entry.owner_loopflow_pid,
                    "failed to terminate orphaned OpenCode process group"
                );
                report.errors += 1;
                retained.push(entry);
            }
        }
        // Falling through (Dead+empty group, or Other) prunes the entry: it is
        // not pushed to `retained`, so the registry drops it.
    }

    if let Err(err) = write_registry_entries(path, &retained) {
        tracing::warn!(
            path = %path.display(),
            error = %err,
            "failed to update OpenCode registry after orphan cleanup"
        );
        report.errors += 1;
    }

    report
}

fn lock_registry(path: &Path) -> Result<std::fs::File> {
    let lock_path = path.with_extension("json.lock");
    if let Some(parent) = lock_path.parent() {
        std::fs::create_dir_all(parent)
            .with_context(|| format!("failed creating runtime dir {}", parent.display()))?;
    }
    let lock = std::fs::OpenOptions::new()
        .create(true)
        .read(true)
        .write(true)
        .truncate(false)
        .open(&lock_path)
        .with_context(|| {
            format!(
                "failed opening OpenCode registry lock {}",
                lock_path.display()
            )
        })?;
    FileExt::lock_exclusive(&lock)
        .with_context(|| format!("failed locking OpenCode registry {}", lock_path.display()))?;
    Ok(lock)
}

fn lock_registry_for_read(path: &Path) -> Result<Option<std::fs::File>> {
    let lock_path = path.with_extension("json.lock");
    let lock = match std::fs::OpenOptions::new().read(true).open(&lock_path) {
        Ok(lock) => lock,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(error) => {
            return Err(error).with_context(|| {
                format!(
                    "failed opening OpenCode registry lock {}",
                    lock_path.display()
                )
            })
        }
    };
    FileExt::lock_shared(&lock)
        .with_context(|| format!("failed locking OpenCode registry {}", lock_path.display()))?;
    Ok(Some(lock))
}

fn read_registry_entries(path: &Path) -> Result<Vec<OpenCodeServerEntry>> {
    let content = match std::fs::read_to_string(path) {
        Ok(content) => content,
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => return Ok(Vec::new()),
        Err(err) => return Err(err.into()),
    };

    if content.trim().is_empty() {
        return Ok(Vec::new());
    }

    serde_json::from_str(&content)
        .with_context(|| format!("failed parsing OpenCode registry at {}", path.display()))
}

fn write_registry_entries(path: &Path, entries: &[OpenCodeServerEntry]) -> Result<()> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)
            .with_context(|| format!("failed creating runtime dir {}", parent.display()))?;
    }

    let json = serde_json::to_string_pretty(entries)
        .context("failed serializing OpenCode server registry")?;
    std::fs::write(path, json)
        .with_context(|| format!("failed writing OpenCode registry at {}", path.display()))?;
    Ok(())
}

fn classify_leader(pid: u32) -> LeaderState {
    if !pid_is_alive(pid) {
        return LeaderState::Dead;
    }
    if process_looks_like_opencode_serve(pid) {
        LeaderState::Opencode
    } else {
        LeaderState::Other
    }
}

fn process_looks_like_opencode_serve(pid: u32) -> bool {
    let output = match Command::new("ps")
        .arg("-o")
        .arg("command=")
        .arg("-p")
        .arg(pid.to_string())
        .output()
    {
        Ok(output) => output,
        Err(_) => return false,
    };

    if !output.status.success() {
        return false;
    }

    let command = String::from_utf8_lossy(&output.stdout).to_ascii_lowercase();
    command.contains("opencode") && command.contains("serve")
}

#[cfg(unix)]
fn pid_is_alive(pid: u32) -> bool {
    if pid == 0 {
        return false;
    }
    let Ok(raw) = i32::try_from(pid) else {
        return false;
    };
    // SAFETY: signal 0 is an existence/permission probe; no pointers are used.
    let result = unsafe { libc::kill(raw, 0) };
    result == 0 || std::io::Error::last_os_error().raw_os_error() == Some(libc::EPERM)
}

#[cfg(not(unix))]
fn pid_is_alive(pid: u32) -> bool {
    if pid == 0 {
        return false;
    }
    Command::new("kill")
        .arg("-0")
        .arg(pid.to_string())
        .status()
        .is_ok_and(|status| status.success())
}

/// Probe whether a process group still has any live member.
#[cfg(unix)]
fn process_group_alive(pgid: u32) -> bool {
    if pgid == 0 {
        return false;
    }
    let Ok(raw) = i32::try_from(pgid) else {
        return false;
    };
    // SAFETY: a negative pid targets the process group; signal 0 only probes.
    let result = unsafe { libc::kill(-raw, 0) };
    result == 0 || std::io::Error::last_os_error().raw_os_error() == Some(libc::EPERM)
}

#[cfg(not(unix))]
fn process_group_alive(_pgid: u32) -> bool {
    false
}

/// Reap an orphaned OpenCode process group: SIGTERM (let the server close its
/// port and clean up its own children), wait, then SIGKILL the stragglers.
/// Returns true if the group has no live members afterwards.
#[cfg(unix)]
fn terminate_process_group(pgid: u32) -> bool {
    if pgid == 0 {
        return false;
    }
    if crate::engine::process::current_process_group_id() == Some(pgid) {
        tracing::warn!(pgid, "refusing to reap current process group");
        return false;
    }
    let Ok(raw) = i32::try_from(pgid) else {
        return false;
    };

    if signal_group(raw, libc::SIGTERM) {
        return true;
    }
    if group_gone(raw, REAP_TERM_GRACE) {
        return true;
    }
    signal_group(raw, libc::SIGKILL);
    group_gone(raw, REAP_TERM_GRACE)
}

#[cfg(not(unix))]
fn terminate_process_group(_pgid: u32) -> bool {
    false
}

#[cfg(unix)]
fn signal_group(pgid: i32, signal: libc::c_int) -> bool {
    // SAFETY: kill with a negative pid signals the process group; no pointers.
    let result = unsafe { libc::kill(-pgid, signal) };
    if result == 0 {
        return false;
    }
    let err = std::io::Error::last_os_error();
    if err.raw_os_error() == Some(libc::ESRCH) {
        return true;
    }
    tracing::warn!(pgid, signal, error = %err, "failed to signal OpenCode process group");
    false
}

#[cfg(unix)]
fn group_gone(pgid: i32, grace: Duration) -> bool {
    let deadline = Instant::now() + grace;
    loop {
        // SAFETY: signal 0 probes the process group; no pointers are used.
        let result = unsafe { libc::kill(-pgid, 0) };
        if result != 0 && std::io::Error::last_os_error().raw_os_error() == Some(libc::ESRCH) {
            return true;
        }
        if Instant::now() >= deadline {
            return false;
        }
        std::thread::sleep(Duration::from_millis(50));
    }
}

#[cfg(test)]
mod tests {
    use std::collections::HashSet;
    use std::sync::Mutex;

    use tempfile::tempdir;

    use super::*;

    fn registry_path(root: &Path) -> PathBuf {
        root.join("runtime").join("opencode-servers.json")
    }

    fn entry(opencode_pid: u32, owner_loopflow_pid: u32) -> OpenCodeServerEntry {
        OpenCodeServerEntry {
            opencode_pid,
            owner_loopflow_pid,
        }
    }

    #[test]
    fn register_and_unregister_opencode_server_updates_registry() {
        let tmp = tempdir().expect("tempdir");
        let path = registry_path(tmp.path());

        register_opencode_server_at_path(&path, 111, 222).expect("register pid");
        let entries = read_registry_entries(&path).expect("read entries");
        assert_eq!(entries, vec![entry(111, 222)]);

        register_opencode_server_at_path(&path, 111, 444).expect("overwrite existing pid");
        let entries = read_registry_entries(&path).expect("read entries");
        assert_eq!(entries, vec![entry(111, 444)]);

        unregister_opencode_server_at_path(&path, 111).expect("unregister pid");
        let entries = read_registry_entries(&path).expect("read entries");
        assert!(entries.is_empty());
    }

    #[test]
    fn reading_an_absent_registry_creates_no_runtime_state() {
        let tmp = tempdir().expect("tempdir");

        assert!(registered_opencode_servers_at(tmp.path())
            .expect("read absent registry")
            .is_empty());
        assert!(!tmp.path().join("runtime").exists());
    }

    #[test]
    fn reap_kills_the_process_group_of_an_orphaned_opencode_server() {
        let tmp = tempdir().expect("tempdir");
        let path = registry_path(tmp.path());
        // Owner 1 alive (retained); owner 2 dead, leader is a live opencode server.
        write_registry_entries(&path, &[entry(10, 1), entry(11, 2), entry(12, 2)])
            .expect("write registry");

        let owner_alive: HashSet<u32> = [1].into_iter().collect();
        let opencode_pids: HashSet<u32> = [11].into_iter().collect();
        let killed = Mutex::new(Vec::new());

        let report = reap_orphaned_opencode_servers_at_path(
            &path,
            |_| true,
            |pid| owner_alive.contains(&pid),
            |pid| {
                if opencode_pids.contains(&pid) {
                    LeaderState::Opencode
                } else {
                    LeaderState::Dead
                }
            },
            |_| false,
            |pid| {
                killed.lock().expect("lock killed list").push(pid);
                true
            },
        );

        assert_eq!(
            report,
            OpenCodeReapReport {
                reaped: 1,
                errors: 0
            }
        );
        assert_eq!(*killed.lock().expect("lock killed list"), vec![11]);
        // Owner-alive entry retained; the dead leader at pid 12 pruned (no group).
        assert_eq!(
            read_registry_entries(&path).expect("read entries"),
            vec![entry(10, 1)]
        );
    }

    #[test]
    fn reap_kills_surviving_children_when_the_leader_is_dead() {
        let tmp = tempdir().expect("tempdir");
        let path = registry_path(tmp.path());
        // Owner dead, leader pid gone, but children still live in its group.
        write_registry_entries(&path, &[entry(21, 2)]).expect("write registry");

        let alive_groups: HashSet<u32> = [21].into_iter().collect();
        let killed = Mutex::new(Vec::new());

        let report = reap_orphaned_opencode_servers_at_path(
            &path,
            |_| true,
            |_| false,
            |_| LeaderState::Dead,
            |pgid| alive_groups.contains(&pgid),
            |pid| {
                killed.lock().expect("lock killed list").push(pid);
                true
            },
        );

        assert_eq!(
            report,
            OpenCodeReapReport {
                reaped: 1,
                errors: 0
            }
        );
        assert_eq!(*killed.lock().expect("lock killed list"), vec![21]);
        assert!(read_registry_entries(&path)
            .expect("read entries")
            .is_empty());
    }

    #[test]
    fn reap_prunes_a_fully_dead_tree_without_signalling() {
        let tmp = tempdir().expect("tempdir");
        let path = registry_path(tmp.path());
        write_registry_entries(&path, &[entry(31, 2)]).expect("write registry");

        let killed = Mutex::new(Vec::new());
        let report = reap_orphaned_opencode_servers_at_path(
            &path,
            |_| true,
            |_| false,
            |_| LeaderState::Dead,
            |_| false,
            |pid| {
                killed.lock().expect("lock killed list").push(pid);
                true
            },
        );

        assert_eq!(report, OpenCodeReapReport::default());
        assert!(killed.lock().expect("lock killed list").is_empty());
        assert!(read_registry_entries(&path)
            .expect("read entries")
            .is_empty());
    }

    #[test]
    fn reap_leaves_a_reused_pid_alone() {
        let tmp = tempdir().expect("tempdir");
        let path = registry_path(tmp.path());
        // Owner dead, but the pid now runs an unrelated process.
        write_registry_entries(&path, &[entry(41, 2)]).expect("write registry");

        let killed = Mutex::new(Vec::new());
        let report = reap_orphaned_opencode_servers_at_path(
            &path,
            |_| true,
            |_| false,
            |_| LeaderState::Other,
            |_| true,
            |pid| {
                killed.lock().expect("lock killed list").push(pid);
                true
            },
        );

        // Not reaped (would risk killing an unrelated group), entry pruned.
        assert_eq!(report, OpenCodeReapReport::default());
        assert!(killed.lock().expect("lock killed list").is_empty());
        assert!(read_registry_entries(&path)
            .expect("read entries")
            .is_empty());
    }

    #[test]
    fn reap_retains_an_entry_when_termination_fails() {
        let tmp = tempdir().expect("tempdir");
        let path = registry_path(tmp.path());
        write_registry_entries(&path, &[entry(51, 2)]).expect("write registry");

        let report = reap_orphaned_opencode_servers_at_path(
            &path,
            |_| true,
            |_| false,
            |_| LeaderState::Opencode,
            |_| true,
            |_| false,
        );

        assert_eq!(
            report,
            OpenCodeReapReport {
                reaped: 0,
                errors: 1
            }
        );
        assert_eq!(
            read_registry_entries(&path).expect("read entries"),
            vec![entry(51, 2)]
        );
    }

    #[test]
    fn selected_reap_preserves_unlisted_orphans() {
        let tmp = tempdir().expect("tempdir");
        let path = registry_path(tmp.path());
        write_registry_entries(&path, &[entry(60, 2), entry(61, 2)]).expect("write registry");

        let report = reap_orphaned_opencode_servers_at_path(
            &path,
            |pid| pid == 60,
            |_| false,
            |_| LeaderState::Opencode,
            |_| true,
            |_| true,
        );

        assert_eq!(report.reaped, 1);
        assert_eq!(
            read_registry_entries(&path).expect("read entries"),
            vec![entry(61, 2)]
        );
    }

    #[test]
    fn reap_is_idempotent() {
        let tmp = tempdir().expect("tempdir");
        let path = registry_path(tmp.path());
        write_registry_entries(&path, &[entry(20, 2)]).expect("write registry");

        let first = reap_orphaned_opencode_servers_at_path(
            &path,
            |_| true,
            |_| false,
            |pid| {
                if pid == 20 {
                    LeaderState::Opencode
                } else {
                    LeaderState::Dead
                }
            },
            |_| false,
            |_| true,
        );
        assert_eq!(
            first,
            OpenCodeReapReport {
                reaped: 1,
                errors: 0
            }
        );

        let second = reap_orphaned_opencode_servers_at_path(
            &path,
            |_| true,
            |_| false,
            |pid| {
                if pid == 20 {
                    LeaderState::Opencode
                } else {
                    LeaderState::Dead
                }
            },
            |_| false,
            |_| true,
        );
        assert_eq!(second, OpenCodeReapReport::default());
    }
}
