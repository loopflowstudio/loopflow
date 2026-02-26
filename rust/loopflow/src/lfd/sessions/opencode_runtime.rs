use std::path::{Path, PathBuf};
use std::process::Command;

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};

const OPENCODE_REGISTRY_FILE: &str = "runtime/opencode-servers.json";

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub(crate) struct OpenCodeReapReport {
    pub(crate) reaped: u32,
    pub(crate) errors: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
struct OpenCodeServerEntry {
    opencode_pid: u32,
    owner_lfd_pid: u32,
    created_at: i64,
}

pub(crate) fn register_opencode_server(opencode_pid: u32) -> Result<()> {
    let now = time::OffsetDateTime::now_utc().unix_timestamp();
    register_opencode_server_at_path(&registry_path(), opencode_pid, std::process::id(), now)
}

pub(crate) fn unregister_opencode_server(opencode_pid: u32) -> Result<()> {
    unregister_opencode_server_at_path(&registry_path(), opencode_pid)
}

pub(crate) fn reap_orphaned_opencode_servers() -> OpenCodeReapReport {
    reap_orphaned_opencode_servers_at_path(
        &registry_path(),
        pid_is_alive,
        pid_is_alive,
        process_looks_like_opencode_serve,
        terminate_process,
    )
}

fn registry_path() -> PathBuf {
    crate::lfd::lf_home_dir().join(OPENCODE_REGISTRY_FILE)
}

fn register_opencode_server_at_path(
    path: &Path,
    opencode_pid: u32,
    owner_lfd_pid: u32,
    created_at: i64,
) -> Result<()> {
    let mut entries = read_registry_entries(path)?;
    entries.retain(|entry| entry.opencode_pid != opencode_pid);
    entries.push(OpenCodeServerEntry {
        opencode_pid,
        owner_lfd_pid,
        created_at,
    });
    write_registry_entries(path, &entries)
}

fn unregister_opencode_server_at_path(path: &Path, opencode_pid: u32) -> Result<()> {
    let mut entries = read_registry_entries(path)?;
    let original_len = entries.len();
    entries.retain(|entry| entry.opencode_pid != opencode_pid);
    if entries.len() == original_len {
        return Ok(());
    }
    write_registry_entries(path, &entries)
}

fn reap_orphaned_opencode_servers_at_path(
    path: &Path,
    owner_pid_alive: impl Fn(u32) -> bool,
    process_pid_alive: impl Fn(u32) -> bool,
    process_matches_opencode: impl Fn(u32) -> bool,
    terminate_pid: impl Fn(u32) -> bool,
) -> OpenCodeReapReport {
    let mut report = OpenCodeReapReport::default();
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
        if owner_pid_alive(entry.owner_lfd_pid) {
            retained.push(entry);
            continue;
        }

        if !process_pid_alive(entry.opencode_pid) {
            continue;
        }

        if !process_matches_opencode(entry.opencode_pid) {
            continue;
        }

        if terminate_pid(entry.opencode_pid) {
            report.reaped += 1;
        } else {
            tracing::warn!(
                opencode_pid = entry.opencode_pid,
                owner_lfd_pid = entry.owner_lfd_pid,
                "failed to terminate orphaned OpenCode server"
            );
            report.errors += 1;
            retained.push(entry);
        }
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

fn terminate_process(pid: u32) -> bool {
    if pid == 0 {
        return false;
    }

    Command::new("kill")
        .arg("-TERM")
        .arg(pid.to_string())
        .status()
        .is_ok_and(|status| status.success())
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

    fn entry(opencode_pid: u32, owner_lfd_pid: u32, created_at: i64) -> OpenCodeServerEntry {
        OpenCodeServerEntry {
            opencode_pid,
            owner_lfd_pid,
            created_at,
        }
    }

    #[test]
    fn register_and_unregister_opencode_server_updates_registry() {
        let tmp = tempdir().expect("tempdir");
        let path = registry_path(tmp.path());

        register_opencode_server_at_path(&path, 111, 222, 333).expect("register pid");
        let entries = read_registry_entries(&path).expect("read entries");
        assert_eq!(entries, vec![entry(111, 222, 333)]);

        register_opencode_server_at_path(&path, 111, 444, 555).expect("overwrite existing pid");
        let entries = read_registry_entries(&path).expect("read entries");
        assert_eq!(entries, vec![entry(111, 444, 555)]);

        unregister_opencode_server_at_path(&path, 111).expect("unregister pid");
        let entries = read_registry_entries(&path).expect("read entries");
        assert!(entries.is_empty());
    }

    #[test]
    fn reap_orphaned_opencode_servers_reaps_owned_servers_and_prunes_stale_entries() {
        let tmp = tempdir().expect("tempdir");
        let path = registry_path(tmp.path());
        write_registry_entries(
            &path,
            &[
                entry(10, 1, 1),
                entry(11, 2, 1),
                entry(12, 2, 1),
                entry(13, 2, 1),
            ],
        )
        .expect("write registry");

        let owner_alive: HashSet<u32> = [1].into_iter().collect();
        let running_pids: HashSet<u32> = [10, 11, 13].into_iter().collect();
        let opencode_pids: HashSet<u32> = [11].into_iter().collect();
        let killed = Mutex::new(Vec::new());

        let report = reap_orphaned_opencode_servers_at_path(
            &path,
            |pid| owner_alive.contains(&pid),
            |pid| running_pids.contains(&pid),
            |pid| opencode_pids.contains(&pid),
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
        assert_eq!(
            read_registry_entries(&path).expect("read entries"),
            vec![entry(10, 1, 1)]
        );
    }

    #[test]
    fn reap_orphaned_opencode_servers_is_idempotent() {
        let tmp = tempdir().expect("tempdir");
        let path = registry_path(tmp.path());
        write_registry_entries(&path, &[entry(20, 2, 1)]).expect("write registry");

        let first = reap_orphaned_opencode_servers_at_path(
            &path,
            |_| false,
            |pid| pid == 20,
            |pid| pid == 20,
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
            |_| false,
            |pid| pid == 20,
            |pid| pid == 20,
            |_| true,
        );
        assert_eq!(second, OpenCodeReapReport::default());
    }
}
