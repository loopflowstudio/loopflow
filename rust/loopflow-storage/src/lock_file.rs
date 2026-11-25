//! Advisory lock files that stay safe under concurrent unlinking.
//!
//! Taking a lock always verifies that the locked descriptor still refers to the
//! path's current inode (`fstat` == `stat(path)`), retrying if a pruner unlinked
//! the file while we waited. Removing a lock file always holds the exclusive
//! lock on that verified inode across `remove_file`, so waiters can never split
//! onto two different inodes.

use std::fs::{File, OpenOptions};
use std::io;
use std::path::Path;
use std::time::{Duration, Instant};

use fs2::FileExt;

fn open_lock(path: &Path) -> io::Result<File> {
    OpenOptions::new()
        .create(true)
        .truncate(false)
        .read(true)
        .write(true)
        .open(path)
}

#[cfg(unix)]
fn same_inode(file: &File, path: &Path) -> io::Result<bool> {
    use std::os::unix::fs::MetadataExt;
    let fd_meta = file.metadata()?;
    let path_meta = match std::fs::metadata(path) {
        Ok(meta) => meta,
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(false),
        Err(error) => return Err(error),
    };
    Ok(fd_meta.dev() == path_meta.dev() && fd_meta.ino() == path_meta.ino())
}

#[cfg(not(unix))]
fn same_inode(_file: &File, path: &Path) -> io::Result<bool> {
    Ok(path.exists())
}

/// Acquire an exclusive lock on `path`, verifying the locked inode still lives at `path`.
pub fn lock_exclusive_verified(path: &Path) -> io::Result<File> {
    loop {
        let file = open_lock(path)?;
        file.lock_exclusive()?;
        if same_inode(&file, path)? {
            return Ok(file);
        }
    }
}

/// Try to acquire an exclusive lock without waiting, verifying the inode still lives at `path`.
pub fn try_lock_exclusive_verified(path: &Path) -> io::Result<Option<File>> {
    loop {
        let file = open_lock(path)?;
        match file.try_lock_exclusive() {
            Ok(()) => {
                if same_inode(&file, path)? {
                    return Ok(Some(file));
                }
            }
            Err(error) if error.kind() == io::ErrorKind::WouldBlock => return Ok(None),
            Err(error) => return Err(error),
        }
    }
}

/// Acquire an exclusive or shared lock on `path` up to `timeout`, verifying the inode.
pub fn lock_with_timeout_verified(
    path: &Path,
    exclusive: bool,
    timeout: Duration,
    poll: Duration,
) -> io::Result<File> {
    let deadline = Instant::now() + timeout;
    loop {
        let file = open_lock(path)?;
        let locked = if exclusive {
            file.try_lock_exclusive()
        } else {
            FileExt::try_lock_shared(&file)
        };
        match locked {
            Ok(()) => {
                if same_inode(&file, path)? {
                    return Ok(file);
                }
            }
            Err(error) if error.kind() == io::ErrorKind::WouldBlock => {
                if Instant::now() >= deadline {
                    return Err(error);
                }
                std::thread::sleep(poll);
            }
            Err(error) => return Err(error),
        }
    }
}

/// Remove `path` only while holding its exclusive lock on the current inode.
/// Returns `Ok(true)` if removed, `Ok(false)` if missing, busy, or already replaced.
pub fn try_remove_if_unlocked(path: &Path) -> io::Result<bool> {
    let file = match OpenOptions::new().read(true).write(true).open(path) {
        Ok(file) => file,
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(false),
        Err(error) => return Err(error),
    };
    match file.try_lock_exclusive() {
        Ok(()) => {}
        Err(error) if error.kind() == io::ErrorKind::WouldBlock => return Ok(false),
        Err(error) => return Err(error),
    }
    if !same_inode(&file, path)? {
        return Ok(false);
    }
    match std::fs::remove_file(path) {
        Ok(()) => Ok(true),
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(false),
        Err(error) => Err(error),
    }
}

#[cfg(test)]
mod tests {
    use std::sync::{Arc, Barrier};
    use std::time::Duration;

    use super::{lock_exclusive_verified, lock_with_timeout_verified, try_remove_if_unlocked};

    #[test]
    fn remove_skips_held_lock_and_removes_released_lock() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("owner.lock");
        let held = lock_exclusive_verified(&path).unwrap();
        assert!(!try_remove_if_unlocked(&path).unwrap());
        assert!(path.exists());
        drop(held);
        assert!(try_remove_if_unlocked(&path).unwrap());
        assert!(!path.exists());
    }

    #[test]
    fn waiter_reopens_after_unlink_and_never_splits_inodes() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("race.lock");

        // Hold lock on inode 1, unlink it while holding, and release after a short delay.
        let holder = lock_exclusive_verified(&path).unwrap();
        let started = Arc::new(Barrier::new(2));
        let waiter_started = started.clone();
        let waiter_path = path.clone();
        let waiter = std::thread::spawn(move || {
            waiter_started.wait();
            lock_with_timeout_verified(
                &waiter_path,
                true,
                Duration::from_secs(5),
                Duration::from_millis(5),
            )
            .unwrap()
        });
        started.wait();
        std::thread::sleep(Duration::from_millis(30));
        std::fs::remove_file(&path).unwrap();
        // A third taker creates inode 2 and holds it while inode 1 is released.
        let second = lock_exclusive_verified(&path).unwrap();
        drop(holder);
        // Waiter must not acquire while `second` holds inode 2.
        std::thread::sleep(Duration::from_millis(40));
        assert!(!waiter.is_finished());
        drop(second);
        let acquired = waiter.join().unwrap();
        assert!(super::same_inode(&acquired, &path).unwrap());
    }
}
