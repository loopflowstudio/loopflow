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
use std::sync::Arc;
#[cfg(not(target_os = "macos"))]
use std::sync::{Condvar, Mutex};
use std::time::Duration;
#[cfg(target_os = "macos")]
use std::time::Instant;

/// How often a reader looks again while a commit may be in flight.
#[cfg(target_os = "macos")]
const LOOK: Duration = Duration::from_millis(50);
/// How long after a write to the log its commit may still be unpublished.
#[cfg(target_os = "macos")]
const IN_FLIGHT: Duration = Duration::from_secs(1);

/// Handle that wakes a thread blocked in [`StoreChanges::wait_with_history`].
#[derive(Debug, Clone)]
pub struct StoreWaker {
    #[cfg(target_os = "macos")]
    queue: Option<Arc<std::os::fd::OwnedFd>>,
    #[cfg(not(target_os = "macos"))]
    signal: Arc<Signal>,
}

impl StoreWaker {
    /// Interrupt the associated wait without writing to SQLite.
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
    #[cfg_attr(not(target_os = "macos"), allow(dead_code))]
    history_wal: PathBuf,
    #[cfg(target_os = "macos")]
    queue: Option<Arc<std::os::fd::OwnedFd>>,
    #[cfg(target_os = "macos")]
    watch: Option<macos::Watch>,
    /// When either watched log was last seen written or replaced.
    #[cfg(target_os = "macos")]
    written: Option<Instant>,
    #[cfg(not(target_os = "macos"))]
    signal: Arc<Signal>,
}

impl StoreChanges {
    pub fn watch(database: &Path) -> Self {
        let mut wal = database.as_os_str().to_owned();
        wal.push("-wal");
        let mut history_wal = crate::history::history_path(database).into_os_string();
        history_wal.push("-wal");
        Self {
            wal: wal.into(),
            history_wal: history_wal.into(),
            #[cfg(target_os = "macos")]
            queue: macos::create_queue().map(Arc::new),
            #[cfg(target_os = "macos")]
            watch: None,
            #[cfg(target_os = "macos")]
            written: None,
            #[cfg(not(target_os = "macos"))]
            signal: Arc::new(Signal::default()),
        }
    }

    pub fn waker(&self) -> StoreWaker {
        StoreWaker {
            #[cfg(target_os = "macos")]
            queue: self.queue.clone(),
            #[cfg(not(target_os = "macos"))]
            signal: Arc::clone(&self.signal),
        }
    }

    /// Return on a store change, a wake signal, or after `timeout`.
    pub fn wait(&mut self, timeout: Duration) {
        self.wait_with_history(timeout, false);
    }

    /// Return once the store (or `history.db` when `watch_history` is true)
    /// may have changed, a wake signal arrived, or `timeout` elapsed.
    #[cfg(target_os = "macos")]
    pub fn wait_with_history(&mut self, timeout: Duration, watch_history: bool) {
        let history_target = watch_history.then_some(self.history_wal.as_path());
        if self.watch.is_none() {
            if let Some(queue) = &self.queue {
                self.watch = macos::Watch::open(queue.clone(), &self.wal, history_target);
            }
        } else if let Some(watch) = &mut self.watch {
            watch.sync_history(history_target);
        }
        let in_flight = self.written.is_some_and(|at| at.elapsed() < IN_FLIGHT);
        let waiting_for_history = watch_history
            && self
                .watch
                .as_ref()
                .is_none_or(|watch| !watch.watching_history());
        let timeout = if in_flight {
            timeout.min(LOOK)
        } else if waiting_for_history {
            timeout.min(Duration::from_millis(100))
        } else {
            timeout
        };
        match &mut self.watch {
            Some(watch) => match watch.wait(timeout) {
                macos::Waited::Quiet => {}
                macos::Waited::Written => self.written = Some(Instant::now()),
                // SQLite removes the log when its last connection closes; a
                // replaced file needs a new descriptor.
                macos::Waited::Gone => {
                    self.watch = None;
                    self.written = Some(Instant::now());
                }
            },
            // No store yet. Its directory may not exist either.
            None => {
                let timeout = timeout.min(Duration::from_millis(100));
                if let Some(queue) = &self.queue {
                    macos::wait_event(queue, timeout);
                } else {
                    std::thread::sleep(timeout);
                }
            }
        }
    }

    /// Without a vnode watch, reading the revisions is cheap enough to do on a
    /// short clock.
    #[cfg(not(target_os = "macos"))]
    pub fn wait_with_history(&mut self, timeout: Duration, _watch_history: bool) {
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

    const WAKE_IDENT: usize = 1;

    pub(super) fn create_queue() -> Option<OwnedFd> {
        // SAFETY: `kqueue()` has no preconditions; fd is checked and owned.
        let queue = unsafe {
            let fd = libc::kqueue();
            (fd >= 0).then(|| OwnedFd::from_raw_fd(fd))?
        };
        let user = libc::kevent {
            ident: WAKE_IDENT,
            filter: libc::EVFILT_USER,
            flags: libc::EV_ADD | libc::EV_CLEAR,
            fflags: 0,
            data: 0,
            udata: std::ptr::null_mut(),
        };
        // SAFETY: `queue` is a valid kqueue fd.
        let ok = unsafe {
            libc::kevent(
                queue.as_raw_fd(),
                &user,
                1,
                std::ptr::null_mut(),
                0,
                std::ptr::null(),
            ) == 0
        };
        ok.then_some(queue)
    }

    pub(super) fn trigger(queue: &OwnedFd) {
        let user = libc::kevent {
            ident: WAKE_IDENT,
            filter: libc::EVFILT_USER,
            flags: 0,
            fflags: libc::NOTE_TRIGGER,
            data: 0,
            udata: std::ptr::null_mut(),
        };
        // SAFETY: `queue` is a valid kqueue fd.
        unsafe {
            libc::kevent(
                queue.as_raw_fd(),
                &user,
                1,
                std::ptr::null_mut(),
                0,
                std::ptr::null(),
            );
        }
    }

    #[derive(Debug)]
    pub(super) struct Watch {
        queue: Arc<OwnedFd>,
        // Closing a file descriptor removes its registration from the queue.
        _primary: OwnedFd,
        history: Option<OwnedFd>,
    }

    fn open_vnode(path: &Path) -> Option<OwnedFd> {
        let cpath = CString::new(path.as_os_str().as_bytes()).ok()?;
        // SAFETY: `cpath` is a valid C string; the returned descriptor is
        // checked and owned exactly once.
        unsafe {
            let file = libc::open(cpath.as_ptr(), libc::O_EVTONLY | libc::O_CLOEXEC);
            (file >= 0).then(|| OwnedFd::from_raw_fd(file))
        }
    }

    fn register_vnode(queue: &OwnedFd, file: &OwnedFd) -> bool {
        let change = libc::kevent {
            ident: file.as_raw_fd() as usize,
            filter: libc::EVFILT_VNODE,
            flags: libc::EV_ADD | libc::EV_CLEAR,
            fflags: libc::NOTE_WRITE | libc::NOTE_EXTEND | libc::NOTE_DELETE | libc::NOTE_RENAME,
            data: 0,
            udata: std::ptr::null_mut(),
        };
        // SAFETY: `queue` and `file` are valid open descriptors.
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

    impl Watch {
        pub(super) fn open(
            queue: Arc<OwnedFd>,
            wal: &Path,
            history_wal: Option<&Path>,
        ) -> Option<Self> {
            let primary = open_vnode(wal)?;
            if !register_vnode(&queue, &primary) {
                return None;
            }
            let mut watch = Self {
                queue,
                _primary: primary,
                history: None,
            };
            watch.sync_history(history_wal);
            Some(watch)
        }

        pub(super) fn watching_history(&self) -> bool {
            self.history.is_some()
        }

        pub(super) fn sync_history(&mut self, history_wal: Option<&Path>) {
            match history_wal {
                Some(path) if self.history.is_none() => {
                    if let Some(file) = open_vnode(path) {
                        if register_vnode(&self.queue, &file) {
                            self.history = Some(file);
                        }
                    }
                }
                None if self.history.is_some() => {
                    self.history = None;
                }
                _ => {}
            }
        }

        pub(super) fn wait(&mut self, timeout: Duration) -> Waited {
            match wait_event(&self.queue, timeout) {
                None => Waited::Quiet,
                Some(event) if event.filter == libc::EVFILT_USER => Waited::Quiet,
                Some(event) if event.fflags & (libc::NOTE_DELETE | libc::NOTE_RENAME) == 0 => {
                    Waited::Written
                }
                Some(event)
                    if self
                        .history
                        .as_ref()
                        .is_some_and(|file| event.ident == file.as_raw_fd() as usize) =>
                {
                    self.history = None;
                    Waited::Written
                }
                Some(_) => Waited::Gone,
            }
        }
    }

    pub(super) fn wait_event(queue: &OwnedFd, timeout: Duration) -> Option<libc::kevent> {
        let timeout = libc::timespec {
            tv_sec: timeout.as_secs() as libc::time_t,
            tv_nsec: timeout.subsec_nanos() as libc::c_long,
        };
        // SAFETY: the queue is open and `event` is a valid out-parameter
        // for the single event requested.
        unsafe {
            let mut event: libc::kevent = std::mem::zeroed();
            let count = libc::kevent(
                queue.as_raw_fd(),
                std::ptr::null(),
                0,
                &mut event,
                1,
                &timeout,
            );
            (count == 1).then_some(event)
        }
    }

    /// How a wait on the log ended.
    pub(super) enum Waited {
        /// The timeout passed with no event.
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
        changes.wait(Duration::from_millis(30));
        assert!(started.elapsed() >= Duration::from_millis(30));
    }
}
