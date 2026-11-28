//! SQL-only counters for subscription measurement.
use std::sync::atomic::{AtomicU64, Ordering::Relaxed};
static SQL: AtomicU64 = AtomicU64::new(0);
static ROWS: AtomicU64 = AtomicU64::new(0);
static VM: AtomicU64 = AtomicU64::new(0);
static OPENS: AtomicU64 = AtomicU64::new(0);
static CLOSES: AtomicU64 = AtomicU64::new(0);
pub(crate) fn observe(connection: &rusqlite::Connection, count_open: bool) -> bool {
    if !super::SUBSCRIPTION_ENABLED.load(Relaxed) {
        return false;
    }
    if count_open {
        OPENS.fetch_add(1, Relaxed);
    }
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

#[derive(Debug)]
pub struct Snapshot {
    pub statements: u64,
    pub rows: u64,
    pub vm_steps: u64,
    pub opens: u64,
    pub closes: u64,
}
pub fn snapshot() -> Snapshot {
    Snapshot {
        statements: SQL.load(Relaxed),
        rows: ROWS.load(Relaxed),
        vm_steps: VM.load(Relaxed),
        opens: OPENS.load(Relaxed),
        closes: CLOSES.load(Relaxed),
    }
}
