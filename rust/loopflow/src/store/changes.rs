//! Wake a reader when another process may have committed. Events only say
//! "look now": `store_revisions` is the authority on what changed, and a
//! timeout covers checkpoints, sleep and anything the kernel dropped.

use std::path::{Path, PathBuf};
use std::time::Duration;

#[derive(Debug)]
pub(crate) struct StoreChanges {
    #[cfg_attr(not(target_os = "macos"), allow(dead_code))]
    wal: PathBuf,
    #[cfg(target_os = "macos")]
    watch: Option<macos::Watch>,
}

impl StoreChanges {
    pub(crate) fn watch(database: &Path) -> Self {
        let mut wal = database.as_os_str().to_owned();
        wal.push("-wal");
        Self {
            wal: wal.into(),
            #[cfg(target_os = "macos")]
            watch: None,
        }
    }

    /// Return once the store may have changed, or after `timeout`.
    #[cfg(target_os = "macos")]
    pub(crate) fn wait(&mut self, timeout: Duration) {
        if self.watch.is_none() {
            self.watch = macos::Watch::open(&self.wal);
        }
        match &mut self.watch {
            Some(watch) => {
                // SQLite removes the log when its last connection closes; a
                // replaced file needs a new descriptor.
                if !watch.wait(timeout) {
                    self.watch = None;
                }
            }
            // No store yet. Its directory may not exist either.
            None => std::thread::sleep(timeout.min(Duration::from_millis(100))),
        }
    }

    /// Without a vnode watch, reading the revisions is cheap enough to do on a
    /// short clock.
    #[cfg(not(target_os = "macos"))]
    pub(crate) fn wait(&mut self, timeout: Duration) {
        std::thread::sleep(timeout.min(Duration::from_millis(50)));
    }
}

#[cfg(target_os = "macos")]
mod macos {
    use std::ffi::CString;
    use std::os::fd::{AsRawFd, FromRawFd, OwnedFd};
    use std::os::unix::ffi::OsStrExt;
    use std::path::Path;
    use std::time::Duration;

    #[derive(Debug)]
    pub(super) struct Watch {
        queue: OwnedFd,
        // Closing the file removes its registration from the queue.
        _file: OwnedFd,
    }

    impl Watch {
        pub(super) fn open(wal: &Path) -> Option<Self> {
            let path = CString::new(wal.as_os_str().as_bytes()).ok()?;
            // SAFETY: `path` is a valid C string; each returned descriptor is
            // checked and owned exactly once.
            unsafe {
                let file = libc::open(path.as_ptr(), libc::O_EVTONLY | libc::O_CLOEXEC);
                if file < 0 {
                    return None;
                }
                let file = OwnedFd::from_raw_fd(file);
                let queue = libc::kqueue();
                if queue < 0 {
                    return None;
                }
                let queue = OwnedFd::from_raw_fd(queue);
                let change = libc::kevent {
                    ident: file.as_raw_fd() as usize,
                    filter: libc::EVFILT_VNODE,
                    flags: libc::EV_ADD | libc::EV_CLEAR,
                    fflags: libc::NOTE_WRITE
                        | libc::NOTE_EXTEND
                        | libc::NOTE_DELETE
                        | libc::NOTE_RENAME,
                    data: 0,
                    udata: std::ptr::null_mut(),
                };
                let registered = libc::kevent(
                    queue.as_raw_fd(),
                    &change,
                    1,
                    std::ptr::null_mut(),
                    0,
                    std::ptr::null(),
                );
                (registered == 0).then_some(Self { queue, _file: file })
            }
        }

        /// False once the watched file is gone and the watch must be reopened.
        pub(super) fn wait(&mut self, timeout: Duration) -> bool {
            let timeout = libc::timespec {
                tv_sec: timeout.as_secs() as libc::time_t,
                tv_nsec: timeout.subsec_nanos() as libc::c_long,
            };
            // SAFETY: the queue is open and `event` is a valid out-parameter
            // for the single event requested.
            unsafe {
                let mut event: libc::kevent = std::mem::zeroed();
                let count = libc::kevent(
                    self.queue.as_raw_fd(),
                    std::ptr::null(),
                    0,
                    &mut event,
                    1,
                    &timeout,
                );
                count >= 0
                    && (count == 0 || event.fflags & (libc::NOTE_DELETE | libc::NOTE_RENAME) == 0)
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use std::time::{Duration, Instant};

    use super::StoreChanges;

    #[test]
    fn a_commit_from_another_connection_ends_the_wait_early() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("db");
        let reader = rusqlite::Connection::open(&path).unwrap();
        reader
            .execute_batch("PRAGMA journal_mode=WAL; CREATE TABLE t(x);")
            .unwrap();
        let mut changes = StoreChanges::watch(&path);
        // Arm before the write: the first wait opens the log.
        changes.wait(Duration::from_millis(1));
        let writer_path = path.clone();
        let writer = std::thread::spawn(move || {
            std::thread::sleep(Duration::from_millis(100));
            let conn = rusqlite::Connection::open(writer_path).unwrap();
            conn.execute("INSERT INTO t VALUES(1)", []).unwrap();
        });
        let started = Instant::now();
        changes.wait(Duration::from_secs(20));
        assert!(started.elapsed() < Duration::from_secs(10));
        writer.join().unwrap();
    }

    /// SQLite removes the log when its last connection closes. The next
    /// writer's log is a different file, and its commits must still wake.
    #[test]
    fn a_removed_and_recreated_log_is_watched_again() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("db");
        let wal = dir.path().join("db-wal");
        let first = rusqlite::Connection::open(&path).unwrap();
        first
            .execute_batch("PRAGMA journal_mode=WAL; CREATE TABLE t(x);")
            .unwrap();
        let mut changes = StoreChanges::watch(&path);
        changes.wait(Duration::from_millis(1));
        drop(first);
        assert!(!wal.exists());
        // Notices the removal; nothing to watch until a writer returns.
        changes.wait(Duration::from_millis(50));
        changes.wait(Duration::from_millis(50));

        let second = rusqlite::Connection::open(&path).unwrap();
        second.execute("INSERT INTO t VALUES(1)", []).unwrap();
        assert!(wal.exists());
        // Arm on the new log, then expect its next commit promptly.
        changes.wait(Duration::from_millis(1));
        let writer = std::thread::spawn(move || {
            std::thread::sleep(Duration::from_millis(100));
            second.execute("INSERT INTO t VALUES(2)", []).unwrap();
            second
        });
        let started = Instant::now();
        changes.wait(Duration::from_secs(20));
        assert!(started.elapsed() < Duration::from_secs(10));
        drop(writer.join().unwrap());
    }
}
