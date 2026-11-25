//! Wake a reader when another process may have committed. Events only say
//! "look now": `store_revisions` is the authority on what changed, and a
//! timeout covers checkpoints, sleep and anything the kernel dropped.
//!
//! A write to the log is not yet a commit a reader can see: the writer syncs
//! the log and then publishes the commit in the shared index, which raises no
//! event. A reader woken by the write can look before that and find nothing.
//! So after a write the reader is woken again every [`LOOK`] until the log
//! has been quiet for [`IN_FLIGHT`].

use std::path::{Path, PathBuf};
#[cfg(not(target_os = "macos"))]
use std::sync::{Arc, Condvar, Mutex};
use std::time::Duration;
#[cfg(target_os = "macos")]
use std::time::Instant;

/// How often a reader looks again while a commit may be in flight.
#[cfg(target_os = "macos")]
const LOOK: Duration = Duration::from_millis(50);
/// How long after a write to the log its commit may still be unpublished.
#[cfg(target_os = "macos")]
const IN_FLIGHT: Duration = Duration::from_secs(1);

/// Thread-safe handle that breaks a sibling [`StoreChanges::wait`] out of its
/// sleep immediately, without touching the SQLite WAL.
#[derive(Debug, Clone)]
pub struct StoreWaker {
    #[cfg(target_os = "macos")]
    queue: Option<std::sync::Arc<std::os::fd::OwnedFd>>,
    #[cfg(not(target_os = "macos"))]
    signal: Arc<Signal>,
}

impl StoreWaker {
    /// Wake the associated [`StoreChanges::wait`] promptly.
    pub fn wake(&self) {
        #[cfg(target_os = "macos")]
        if let Some(queue) = &self.queue {
            macos::trigger(queue);
        }
        #[cfg(not(target_os = "macos"))]
        {
            let mut woken = self.signal.woken.lock().unwrap_or_else(|e| e.into_inner());
            *woken = true;
            self.signal.changed.notify_one();
        }
    }
}

#[cfg(not(target_os = "macos"))]
#[derive(Debug, Default)]
struct Signal {
    woken: Mutex<bool>,
    changed: Condvar,
}

#[derive(Debug)]
pub struct StoreChanges {
    #[cfg_attr(not(target_os = "macos"), allow(dead_code))]
    wal: PathBuf,
    #[cfg(target_os = "macos")]
    queue: Option<macos::Queue>,
    /// When the log was last seen written or replaced.
    #[cfg(target_os = "macos")]
    written: Option<Instant>,
    #[cfg(not(target_os = "macos"))]
    signal: Arc<Signal>,
}

impl StoreChanges {
    pub fn watch(database: &Path) -> Self {
        let mut wal = database.as_os_str().to_owned();
        wal.push("-wal");
        Self {
            wal: wal.into(),
            #[cfg(target_os = "macos")]
            queue: macos::Queue::open(),
            #[cfg(target_os = "macos")]
            written: None,
            #[cfg(not(target_os = "macos"))]
            signal: Arc::new(Signal::default()),
        }
    }

    /// Return a cloneable handle that wakes [`Self::wait`] from another thread.
    pub fn waker(&self) -> StoreWaker {
        StoreWaker {
            #[cfg(target_os = "macos")]
            queue: self.queue.as_ref().map(macos::Queue::waker_fd),
            #[cfg(not(target_os = "macos"))]
            signal: Arc::clone(&self.signal),
        }
    }

    /// Return once the store may have changed, [`StoreWaker::wake`] fired, or
    /// `timeout` elapsed.
    #[cfg(target_os = "macos")]
    pub fn wait(&mut self, timeout: Duration) {
        let Some(queue) = &mut self.queue else {
            std::thread::sleep(timeout.min(Duration::from_millis(100)));
            return;
        };
        let watching = queue.ensure_attached(&self.wal);
        let in_flight = self.written.is_some_and(|at| at.elapsed() < IN_FLIGHT);
        let timeout = if !watching {
            // No store yet. Its directory may not exist either, but still wait
            // on the kqueue so `StoreWaker::wake` returns immediately.
            timeout.min(Duration::from_millis(100))
        } else if in_flight {
            timeout.min(LOOK)
        } else {
            timeout
        };
        match queue.wait(timeout) {
            macos::Waited::Quiet => {}
            macos::Waited::Written => self.written = Some(Instant::now()),
            // SQLite removes the log when its last connection closes; a
            // replaced file needs a new descriptor.
            macos::Waited::Gone => {
                queue.detach();
                self.written = Some(Instant::now());
            }
        }
    }

    /// Without a vnode watch, reading the revisions is cheap enough to do on a
    /// short clock.
    #[cfg(not(target_os = "macos"))]
    pub fn wait(&mut self, timeout: Duration) {
        let mut woken = self.signal.woken.lock().unwrap_or_else(|e| e.into_inner());
        if !*woken {
            let (guard, _) = self
                .signal
                .changed
                .wait_timeout(woken, timeout.min(Duration::from_millis(50)))
                .unwrap_or_else(|e| e.into_inner());
            woken = guard;
        }
        *woken = false;
    }
}

#[cfg(target_os = "macos")]
mod macos {
    use std::ffi::CString;
    use std::os::fd::{AsRawFd, FromRawFd, OwnedFd};
    use std::os::unix::ffi::OsStrExt;
    use std::path::Path;
    use std::sync::Arc;
    use std::time::Duration;

    const USER_IDENT: usize = 1;

    #[derive(Debug)]
    pub(super) struct Queue {
        queue: Arc<OwnedFd>,
        // Closing the file removes its registration from the queue.
        file: Option<OwnedFd>,
    }

    /// Apply one kevent change to `queue`.
    fn apply(queue: &OwnedFd, change: libc::kevent) -> bool {
        // SAFETY: `queue` is an open kqueue and `change` is a valid single-element changelist.
        unsafe {
            libc::kevent(
                queue.as_raw_fd(),
                &change,
                1,
                std::ptr::null_mut(),
                0,
                std::ptr::null(),
            ) == 0
        }
    }

    impl Queue {
        pub(super) fn open() -> Option<Self> {
            // SAFETY: `kqueue` returns a fresh descriptor checked below and owned once.
            let queue = unsafe {
                let fd = libc::kqueue();
                if fd < 0 {
                    return None;
                }
                Arc::new(OwnedFd::from_raw_fd(fd))
            };
            let change = libc::kevent {
                ident: USER_IDENT,
                filter: libc::EVFILT_USER,
                flags: libc::EV_ADD | libc::EV_CLEAR,
                fflags: 0,
                data: 0,
                udata: std::ptr::null_mut(),
            };
            apply(&queue, change).then_some(Self { queue, file: None })
        }

        pub(super) fn waker_fd(&self) -> Arc<OwnedFd> {
            Arc::clone(&self.queue)
        }

        pub(super) fn ensure_attached(&mut self, wal: &Path) -> bool {
            if self.file.is_some() {
                return true;
            }
            let Ok(path) = CString::new(wal.as_os_str().as_bytes()) else {
                return false;
            };
            // SAFETY: `path` is a valid C string; the opened descriptor is
            // checked below and owned once.
            let file = unsafe {
                let fd = libc::open(path.as_ptr(), libc::O_EVTONLY | libc::O_CLOEXEC);
                if fd < 0 {
                    return false;
                }
                OwnedFd::from_raw_fd(fd)
            };
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
            if apply(&self.queue, change) {
                self.file = Some(file);
                true
            } else {
                false
            }
        }

        pub(super) fn detach(&mut self) {
            self.file = None;
        }

        pub(super) fn wait(&mut self, timeout: Duration) -> Waited {
            let timeout = libc::timespec {
                tv_sec: timeout.as_secs() as libc::time_t,
                tv_nsec: timeout.subsec_nanos() as libc::c_long,
            };
            // SAFETY: `self.queue` is open and `events` is a valid
            // out-parameter for up to two registered events.
            unsafe {
                let mut events: [libc::kevent; 2] = std::mem::zeroed();
                let count = libc::kevent(
                    self.queue.as_raw_fd(),
                    std::ptr::null(),
                    0,
                    events.as_mut_ptr(),
                    events.len() as libc::c_int,
                    &timeout,
                );
                if count <= 0 {
                    return Waited::Quiet;
                }
                let mut outcome = Waited::Quiet;
                for event in &events[..count as usize] {
                    if event.filter == libc::EVFILT_VNODE {
                        if event.flags & libc::EV_ERROR != 0
                            || event.fflags & (libc::NOTE_DELETE | libc::NOTE_RENAME) != 0
                        {
                            return Waited::Gone;
                        }
                        outcome = Waited::Written;
                    }
                }
                outcome
            }
        }
    }

    pub(super) fn trigger(queue: &OwnedFd) {
        let change = libc::kevent {
            ident: USER_IDENT,
            filter: libc::EVFILT_USER,
            flags: 0,
            fflags: libc::NOTE_TRIGGER,
            data: 0,
            udata: std::ptr::null_mut(),
        };
        apply(queue, change);
    }

    /// How a wait on the log ended.
    pub(super) enum Waited {
        /// The timeout passed or `StoreWaker::wake` fired, with no WAL event.
        Quiet,
        Written,
        /// The watched file is gone and the watch must be reopened.
        Gone,
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

    /// A reader woken by a write can look before the commit is published,
    /// and nothing else may ever write: it is woken again shortly, not after
    /// its whole timeout.
    #[cfg(target_os = "macos")]
    #[test]
    fn a_write_is_followed_by_another_look() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("db");
        let conn = rusqlite::Connection::open(&path).unwrap();
        conn.execute_batch("PRAGMA journal_mode=WAL; CREATE TABLE t(x);")
            .unwrap();
        let mut changes = StoreChanges::watch(&path);
        changes.wait(Duration::from_millis(1));
        conn.execute("INSERT INTO t VALUES(1)", []).unwrap();
        let started = Instant::now();
        // The write, then a look with nothing new written.
        changes.wait(Duration::from_secs(20));
        changes.wait(Duration::from_secs(20));
        assert!(started.elapsed() < Duration::from_secs(10));
        // Once the log has been quiet, a wait lasts its timeout again.
        std::thread::sleep(super::IN_FLIGHT);
        let started = Instant::now();
        changes.wait(Duration::from_millis(300));
        assert!(started.elapsed() >= Duration::from_millis(300));
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

    /// `StoreWaker::wake` breaks a 20 s wait immediately without touching the
    /// WAL, both before a WAL exists and after one is armed, and resets cleanly
    /// so the next wait observes its timeout.
    #[test]
    fn waker_ends_the_wait_without_a_wal_write() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("db");

        // Before any WAL file exists: a pre-wait wake is not lost.
        let mut changes = StoreChanges::watch(&path);
        let waker = changes.waker();
        waker.wake();
        let started = Instant::now();
        changes.wait(Duration::from_secs(20));
        assert!(started.elapsed() < Duration::from_secs(2));

        // With an armed WAL and no writes: a sibling thread wakes a 20 s wait immediately.
        let conn = rusqlite::Connection::open(&path).unwrap();
        conn.execute_batch("PRAGMA journal_mode=WAL; CREATE TABLE t(x);")
            .unwrap();
        let mut changes = StoreChanges::watch(&path);
        let waker = changes.waker();
        changes.wait(Duration::from_millis(1));
        let nudger = std::thread::spawn(move || {
            std::thread::sleep(Duration::from_millis(80));
            waker.wake();
        });
        let started = Instant::now();
        changes.wait(Duration::from_secs(20));
        assert!(started.elapsed() < Duration::from_secs(5));
        nudger.join().unwrap();

        // After the user event is consumed, the next wait honors its timeout.
        let started = Instant::now();
        changes.wait(Duration::from_millis(150));
        assert!(started.elapsed() >= Duration::from_millis(100));
    }
}
