//! Machine-global fence between Run reservation and `lf` promotion.

use std::fs::{self, File, OpenOptions};
use std::io;
use std::os::unix::fs::{OpenOptionsExt, PermissionsExt};
use std::path::{Path, PathBuf};

use fs2::FileExt;
use serde::Deserialize;

#[derive(Debug)]
pub(crate) struct PromotionLock {
    _file: File,
}

#[derive(Debug, Clone, Copy)]
enum LockMode {
    Shared,
    Exclusive,
}

pub(crate) fn acquire_exclusive() -> io::Result<PromotionLock> {
    _acquire(&lock_path(), LockMode::Exclusive)
}

pub(crate) async fn acquire_shared() -> io::Result<PromotionLock> {
    let path = lock_path();
    tokio::task::spawn_blocking(move || {
        let lock = _acquire(&path, LockMode::Shared)?;
        if let crate::machine_install::MachineInstallState::Switching(receipt) =
            crate::machine_install::read_state(
                &crate::machine_install::root().map_err(io::Error::other)?,
            )
            .map_err(io::Error::other)?
        {
            return Err(io::Error::new(
                io::ErrorKind::WouldBlock,
                format!(
                    "install switch {} fences Run reservation until settlement",
                    receipt.id
                ),
            ));
        }
        ensure_current_binary_may_reserve(&active_upgrade_path())?;
        Ok(lock)
    })
    .await
    .map_err(|error| io::Error::other(format!("promotion lock task failed: {error}")))?
}

fn lock_path() -> PathBuf {
    crate::machine_install::account_home()
        .expect("resolve OS account home directory for promotion lock")
        .join(".lf/promotion.lock")
}

fn active_upgrade_path() -> PathBuf {
    crate::machine_install::account_home()
        .expect("resolve OS account home directory for promotion fence")
        .join(".lf/upgrades/active.json")
}

pub(crate) fn require_exclusive_holder() -> io::Result<()> {
    let path = lock_path();
    let file = OpenOptions::new()
        .create(true)
        .truncate(false)
        .read(true)
        .write(true)
        .open(path)?;
    match FileExt::try_lock_shared(&file) {
        Err(error) if error.kind() == io::ErrorKind::WouldBlock => Ok(()),
        Ok(()) => {
            FileExt::unlock(&file)?;
            Err(io::Error::other(
                "receipt-scoped candidate operation requires an exclusive promotion coordinator",
            ))
        }
        Err(error) => Err(error),
    }
}

#[derive(Debug, Deserialize)]
struct ActiveUpgradeCandidate {
    source_revision: String,
    package_version: String,
    build_version: Option<String>,
}

#[derive(Debug, Deserialize)]
struct ActiveUpgradeFence {
    id: String,
    candidate: ActiveUpgradeCandidate,
    artifacts_activated: bool,
}

fn ensure_current_binary_may_reserve(path: &Path) -> io::Result<()> {
    let payload = match fs::read(path) {
        Ok(payload) => payload,
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(()),
        Err(error) => return Err(error),
    };
    let fence: ActiveUpgradeFence = serde_json::from_slice(&payload).map_err(|error| {
        io::Error::new(
            io::ErrorKind::InvalidData,
            format!("active Home upgrade fence is unreadable: {error}"),
        )
    })?;
    let expected_version = fence
        .candidate
        .build_version
        .as_deref()
        .unwrap_or(&fence.candidate.package_version);
    if fence.artifacts_activated
        && expected_version == crate::build_info::BUILD_VERSION
        && fence.candidate.source_revision == crate::build_info::source_revision()
    {
        return Ok(());
    }
    Err(io::Error::new(
        io::ErrorKind::WouldBlock,
        format!(
            "Home upgrade {} fences Run reservation for this runtime generation",
            fence.id
        ),
    ))
}

pub(crate) fn persist_upgrade_fence(payload: &[u8]) -> io::Result<()> {
    let path = active_upgrade_path();
    let incoming: ActiveUpgradeFence = serde_json::from_slice(payload).map_err(io::Error::other)?;
    match fs::read(&path) {
        Ok(existing) => {
            let existing: ActiveUpgradeFence =
                serde_json::from_slice(&existing).map_err(io::Error::other)?;
            if existing.id != incoming.id {
                return Err(io::Error::new(
                    io::ErrorKind::AlreadyExists,
                    format!(
                        "Home upgrade {} is already active; recover or settle it before starting {}",
                        existing.id, incoming.id
                    ),
                ));
            }
        }
        Err(error) if error.kind() == io::ErrorKind::NotFound => {}
        Err(error) => return Err(error),
    }
    let parent = path
        .parent()
        .expect("active Home upgrade fence has a parent directory");
    fs::create_dir_all(parent)?;
    fs::set_permissions(parent, fs::Permissions::from_mode(0o700))?;
    let pending = path.with_extension(format!("json.tmp.{}", std::process::id()));
    let mut file = OpenOptions::new()
        .create(true)
        .truncate(true)
        .write(true)
        .mode(0o600)
        .open(&pending)?;
    use std::io::Write;
    file.write_all(payload)?;
    file.write_all(b"\n")?;
    file.sync_all()?;
    fs::rename(&pending, &path)?;
    File::open(parent)?.sync_all()?;
    Ok(())
}

pub(crate) fn clear_upgrade_fence(upgrade_id: &str) -> io::Result<()> {
    let path = active_upgrade_path();
    let payload = match fs::read(&path) {
        Ok(payload) => payload,
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(()),
        Err(error) => return Err(error),
    };
    let fence: ActiveUpgradeFence = serde_json::from_slice(&payload).map_err(io::Error::other)?;
    if fence.id != upgrade_id {
        return Err(io::Error::other(format!(
            "active Home upgrade is {}, not {upgrade_id}",
            fence.id
        )));
    }
    fs::remove_file(&path)?;
    if let Some(parent) = path.parent() {
        File::open(parent)?.sync_all()?;
    }
    Ok(())
}

fn _acquire(path: &Path, mode: LockMode) -> io::Result<PromotionLock> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    let file = OpenOptions::new()
        .create(true)
        .truncate(false)
        .read(true)
        .write(true)
        .open(path)?;
    match mode {
        LockMode::Shared => FileExt::try_lock_shared(&file)?,
        LockMode::Exclusive => FileExt::lock_exclusive(&file)?,
    }
    Ok(PromotionLock { _file: file })
}

#[cfg(test)]
mod tests {
    use std::fs;

    use super::{_acquire, ensure_current_binary_may_reserve, LockMode};

    #[test]
    fn run_reservation_defers_instead_of_blocking_behind_an_upgrade() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("promotion.lock");
        let _upgrade = _acquire(&path, LockMode::Exclusive).unwrap();

        let error = _acquire(&path, LockMode::Shared).unwrap_err();

        assert_eq!(error.kind(), std::io::ErrorKind::WouldBlock);
    }

    #[test]
    fn durable_fence_blocks_the_old_generation_after_the_file_lock_is_released() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("active.json");
        fs::write(
            &path,
            r#"{
                "id":"upgrade_next",
                "candidate":{
                    "source_revision":"next-revision",
                    "package_version":"99.0.0",
                    "build_version":"99.0.0+next"
                },
                "artifacts_activated":false
            }"#,
        )
        .unwrap();

        let error = ensure_current_binary_may_reserve(&path).unwrap_err();

        assert_eq!(error.kind(), std::io::ErrorKind::WouldBlock);
        assert!(error.to_string().contains("upgrade_next"));
    }

    #[test]
    fn durable_fence_allows_only_the_activated_candidate_generation() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("active.json");
        fs::write(
            &path,
            serde_json::json!({
                "id": "upgrade_current",
                "candidate": {
                    "source_revision": crate::build_info::source_revision(),
                    "package_version": env!("CARGO_PKG_VERSION"),
                    "build_version": crate::build_info::BUILD_VERSION,
                },
                "artifacts_activated": true,
            })
            .to_string(),
        )
        .unwrap();

        ensure_current_binary_may_reserve(&path).unwrap();
    }
}
