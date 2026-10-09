//! Which lf process started each `opencode serve`, for `lf top` and the
//! active-Session reader. Ownership display only: the driver lifeline stops a
//! server whose driver ends, and `harness::engine_orphans` reaps the rest.

use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use fs2::FileExt;
use serde::{Deserialize, Serialize};

const OPENCODE_REGISTRY_FILE: &str = "runtime/opencode-servers.json";

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
    // A driver that was killed never unregisters; its server is gone or reaped.
    entries.retain(|entry| entry.opencode_pid != opencode_pid && pid_is_alive(entry.opencode_pid));
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

fn pid_is_alive(pid: u32) -> bool {
    let Ok(raw) = i32::try_from(pid) else {
        return false;
    };
    if raw <= 0 {
        return false;
    }
    // SAFETY: signal 0 is an existence/permission probe; no pointers are used.
    let result = unsafe { libc::kill(raw, 0) };
    result == 0 || std::io::Error::last_os_error().raw_os_error() == Some(libc::EPERM)
}

#[cfg(test)]
mod tests {
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
}
