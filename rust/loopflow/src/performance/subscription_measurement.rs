//! Opt-in counters for the server subscription experiment. No installed code.
use std::collections::BTreeMap;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering::Relaxed};
use std::sync::Mutex;
use std::time::Instant;

use serde::Serialize;

pub(crate) static ENABLED: AtomicBool = AtomicBool::new(false);
static SQL: AtomicU64 = AtomicU64::new(0);
static ROWS: AtomicU64 = AtomicU64::new(0);
static VM: AtomicU64 = AtomicU64::new(0);
static OPENS: AtomicU64 = AtomicU64::new(0);
static CLOSES: AtomicU64 = AtomicU64::new(0);
static STAGES: Mutex<BTreeMap<&'static str, Count>> = Mutex::new(BTreeMap::new());

#[derive(Debug, Clone, Copy, Default, Serialize)]
pub(crate) struct Count {
    pub count: u64,
    pub wall_ms: f64,
    pub caller_cpu_ms: f64,
}

fn thread_cpu() -> f64 {
    // SAFETY: clock_gettime writes to a valid timespec; this clock is per thread.
    unsafe {
        let mut time = std::mem::zeroed::<libc::timespec>();
        assert_eq!(
            libc::clock_gettime(libc::CLOCK_THREAD_CPUTIME_ID, &mut time),
            0
        );
        time.tv_sec as f64 * 1000.0 + time.tv_nsec as f64 / 1_000_000.0
    }
}

pub(crate) struct Stage(Option<(&'static str, Instant, f64)>);
impl Stage {
    pub(crate) fn new(name: &'static str) -> Self {
        Self(
            ENABLED
                .load(Relaxed)
                .then(|| (name, Instant::now(), thread_cpu())),
        )
    }
}
impl Drop for Stage {
    fn drop(&mut self) {
        if let Some((name, began, cpu)) = self.0 {
            let mut stages = STAGES.lock().unwrap();
            let value = stages.entry(name).or_default();
            value.count += 1;
            value.wall_ms += began.elapsed().as_secs_f64() * 1000.0;
            value.caller_cpu_ms += thread_cpu() - cpu;
        }
    }
}

/// Same builder as Runtime::new. Hooks count actual worker and blocking threads.
pub(crate) struct Runtime(Option<tokio::runtime::Runtime>);
impl Runtime {
    pub(crate) fn build(builder: &mut tokio::runtime::Builder) -> std::io::Result<Self> {
        let _stage = Stage::new("runtime_build");
        builder
            .on_thread_start(|| drop(Stage::new("runtime_thread_start")))
            .on_thread_stop(|| drop(Stage::new("runtime_thread_stop")))
            .build()
            .map(|runtime| Self(Some(runtime)))
    }
}
impl std::ops::Deref for Runtime {
    type Target = tokio::runtime::Runtime;
    fn deref(&self) -> &Self::Target {
        self.0.as_ref().unwrap()
    }
}
impl Drop for Runtime {
    fn drop(&mut self) {
        let _stage = Stage::new("runtime_drop");
        drop(self.0.take());
    }
}

pub(crate) fn observe(connection: &rusqlite::Connection) -> bool {
    if !ENABLED.load(Relaxed) {
        return false;
    }
    OPENS.fetch_add(1, Relaxed);
    // Use the SQLite callback directly to reset VM status after every execution,
    // including cached statements. rusqlite's StmtRef has no reset method.
    unsafe extern "C" fn trace(
        event: u32,
        _: *mut std::ffi::c_void,
        pointer: *mut std::ffi::c_void,
        _: *mut std::ffi::c_void,
    ) -> std::ffi::c_int {
        use rusqlite::ffi;
        match event {
            ffi::SQLITE_TRACE_STMT => {
                SQL.fetch_add(1, Relaxed);
            }
            ffi::SQLITE_TRACE_ROW => {
                ROWS.fetch_add(1, Relaxed);
            }
            ffi::SQLITE_TRACE_CLOSE => {
                CLOSES.fetch_add(1, Relaxed);
            }
            ffi::SQLITE_TRACE_PROFILE => {
                // SAFETY: SQLITE_TRACE_PROFILE supplies the live sqlite3_stmt.
                let steps = unsafe {
                    ffi::sqlite3_stmt_status(pointer.cast(), ffi::SQLITE_STMTSTATUS_VM_STEP, 1)
                };
                VM.fetch_add(steps as u64, Relaxed);
            }
            _ => {}
        }
        0
    }
    // SAFETY: the handle stays live for registration. The callback uses only
    // static atomics, retains no pointers and cannot unwind through SQLite.
    unsafe {
        use rusqlite::ffi;
        assert_eq!(
            ffi::sqlite3_trace_v2(
                connection.handle(),
                ffi::SQLITE_TRACE_STMT
                    | ffi::SQLITE_TRACE_ROW
                    | ffi::SQLITE_TRACE_PROFILE
                    | ffi::SQLITE_TRACE_CLOSE,
                Some(trace),
                std::ptr::null_mut(),
            ),
            ffi::SQLITE_OK
        );
    }
    true
}

#[derive(Debug, Clone, Serialize)]
pub(crate) struct Snapshot {
    pub user_ms: f64,
    pub system_ms: f64,
    pub child_user_ms: f64,
    pub child_system_ms: f64,
    pub statements: u64,
    pub rows: u64,
    pub vm_steps: u64,
    pub opens: u64,
    pub closes: u64,
    pub stages: BTreeMap<&'static str, Count>,
}

fn usage(who: i32) -> (f64, f64) {
    // SAFETY: getrusage initializes a writable rusage of the required size.
    unsafe {
        let mut u = std::mem::zeroed::<libc::rusage>();
        assert_eq!(libc::getrusage(who, &mut u), 0);
        (
            u.ru_utime.tv_sec as f64 * 1000.0 + u.ru_utime.tv_usec as f64 / 1000.0,
            u.ru_stime.tv_sec as f64 * 1000.0 + u.ru_stime.tv_usec as f64 / 1000.0,
        )
    }
}

pub(crate) fn snapshot() -> Snapshot {
    let (user_ms, system_ms) = usage(libc::RUSAGE_SELF);
    let (child_user_ms, child_system_ms) = usage(libc::RUSAGE_CHILDREN);
    Snapshot {
        user_ms,
        system_ms,
        child_user_ms,
        child_system_ms,
        statements: SQL.load(Relaxed),
        rows: ROWS.load(Relaxed),
        vm_steps: VM.load(Relaxed),
        opens: OPENS.load(Relaxed),
        closes: CLOSES.load(Relaxed),
        stages: STAGES.lock().unwrap().clone(),
    }
}
impl Snapshot {
    pub(crate) fn since(mut self, before: Self) -> Self {
        self.user_ms -= before.user_ms;
        self.system_ms -= before.system_ms;
        self.child_user_ms -= before.child_user_ms;
        self.child_system_ms -= before.child_system_ms;
        self.statements -= before.statements;
        self.rows -= before.rows;
        self.vm_steps -= before.vm_steps;
        self.opens -= before.opens;
        self.closes -= before.closes;
        for (name, old) in before.stages {
            let value = self.stages.entry(name).or_default();
            value.count -= old.count;
            value.wall_ms -= old.wall_ms;
            value.caller_cpu_ms -= old.caller_cpu_ms;
        }
        self
    }
}
