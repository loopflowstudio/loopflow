//! Machine-global lock serializing `lf` promotion.

use std::fs::{self, File, OpenOptions};
use std::io;
use std::path::{Path, PathBuf};

use fs2::FileExt;

#[derive(Debug)]
pub(crate) struct PromotionLock {
    _file: File,
}

pub(crate) fn acquire_exclusive() -> io::Result<PromotionLock> {
    _acquire(&lock_path())
}

fn lock_path() -> PathBuf {
    crate::machine_install::account_home()
        .expect("resolve OS account home directory for promotion lock")
        .join(".lf/promotion.lock")
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

fn _acquire(path: &Path) -> io::Result<PromotionLock> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    let file = OpenOptions::new()
        .create(true)
        .truncate(false)
        .read(true)
        .write(true)
        .open(path)?;
    FileExt::lock_exclusive(&file)?;
    Ok(PromotionLock { _file: file })
}
