//! Machine-global fence between Run reservation and `lf` promotion.

use std::fs::{self, File, OpenOptions};
use std::io;
use std::path::{Path, PathBuf};

use fs2::FileExt;

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
    tokio::task::spawn_blocking(move || _acquire(&path, LockMode::Shared))
        .await
        .map_err(|error| io::Error::other(format!("promotion lock task failed: {error}")))?
}

fn lock_path() -> PathBuf {
    dirs::home_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join(".lf/promotion.lock")
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
        LockMode::Shared => FileExt::lock_shared(&file)?,
        LockMode::Exclusive => FileExt::lock_exclusive(&file)?,
    }
    Ok(PromotionLock { _file: file })
}
