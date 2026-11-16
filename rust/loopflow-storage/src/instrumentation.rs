//! SQLite tracing and volume counters.
use rusqlite::trace::{TraceEvent, TraceEventCodes};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};

pub static ENABLED: AtomicBool = AtomicBool::new(false);
pub static CONNECTIONS: AtomicU64 = AtomicU64::new(0);
pub static STATEMENTS: AtomicU64 = AtomicU64::new(0);
pub static ROWS: AtomicU64 = AtomicU64::new(0);
pub static SQL_PROFILES: AtomicU64 = AtomicU64::new(0);
pub static SQL_NANOS: AtomicU64 = AtomicU64::new(0);

pub fn observe_sqlite(connection: &rusqlite::Connection) {
    #[cfg(any(test, feature = "test-support"))]
    if subscription::observe(connection) {
        assert!(
            !ENABLED.load(Ordering::Relaxed),
            "measurement tracers must not overlap"
        );
        return;
    }
    if ENABLED.load(Ordering::Relaxed) {
        CONNECTIONS.fetch_add(1, Ordering::Relaxed);
        connection.trace_v2(
            TraceEventCodes::SQLITE_TRACE_STMT
                | TraceEventCodes::SQLITE_TRACE_ROW
                | TraceEventCodes::SQLITE_TRACE_PROFILE,
            Some(count_sqlite),
        );
    }
}

#[cfg(any(test, feature = "test-support"))]
mod subscription;

fn count_sqlite(event: TraceEvent<'_>) {
    match event {
        TraceEvent::Stmt(..) => {
            STATEMENTS.fetch_add(1, Ordering::Relaxed);
        }
        TraceEvent::Row(..) => {
            ROWS.fetch_add(1, Ordering::Relaxed);
        }
        TraceEvent::Profile(_, duration) => {
            SQL_PROFILES.fetch_add(1, Ordering::Relaxed);
            SQL_NANOS.fetch_add(
                u64::try_from(duration.as_nanos()).unwrap_or(u64::MAX),
                Ordering::Relaxed,
            );
        }
        _ => {}
    }
}

#[cfg(any(test, feature = "test-support"))]
pub static SUBSCRIPTION_ENABLED: AtomicBool = AtomicBool::new(false);
#[cfg(any(test, feature = "test-support"))]
pub use subscription::snapshot as subscription_snapshot;

#[cfg(any(test, feature = "test-support"))]
/// Synchronous reads on the calling thread. CLI receipts cover worker-thread APIs.
pub mod testing {
    use std::cell::Cell;

    use rusqlite::trace::{TraceEvent, TraceEventCodes};
    use rusqlite::{Connection, StatementStatus};

    #[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
    pub struct SqliteWork {
        pub statements: usize,
        pub rows: usize,
        pub vm_steps: usize,
    }

    thread_local! {
        static WORK: Cell<SqliteWork> = const { Cell::new(SqliteWork {
            statements: 0, rows: 0, vm_steps: 0,
        }) };
    }

    pub fn observe(connection: &Connection) {
        connection.trace_v2(
            TraceEventCodes::SQLITE_TRACE_STMT
                | TraceEventCodes::SQLITE_TRACE_ROW
                | TraceEventCodes::SQLITE_TRACE_PROFILE,
            Some(|event| {
                let mut work = WORK.get();
                match event {
                    TraceEvent::Stmt(..) => work.statements += 1,
                    TraceEvent::Row(..) => work.rows += 1,
                    TraceEvent::Profile(statement, _) => {
                        work.vm_steps += statement.get_status(StatementStatus::VmStep) as usize;
                    }
                    _ => {}
                }
                WORK.set(work);
            }),
        );
    }

    pub fn measure<T>(read: impl FnOnce() -> T) -> (T, SqliteWork) {
        WORK.set(SqliteWork::default());
        let result = read();
        (result, WORK.get())
    }
}
