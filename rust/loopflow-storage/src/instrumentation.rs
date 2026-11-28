//! SQLite tracing and volume counters.
use rusqlite::trace::{TraceEvent, TraceEventCodes};
use std::cell::RefCell;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::Arc;

pub static ENABLED: AtomicBool = AtomicBool::new(false);
pub static CONNECTIONS: AtomicU64 = AtomicU64::new(0);
pub static STATEMENTS: AtomicU64 = AtomicU64::new(0);
pub static ROWS: AtomicU64 = AtomicU64::new(0);
pub static SQL_PROFILES: AtomicU64 = AtomicU64::new(0);
static REQUEST_TRACING: AtomicBool = AtomicBool::new(false);
static OPENED_CONNECTIONS: AtomicU64 = AtomicU64::new(0);

/// Record a physical SQLite `Connection::open*` call for process diagnostics.
pub fn record_connection_open() {
    OPENED_CONNECTIONS.fetch_add(1, Ordering::Relaxed);
}

/// Cumulative physical SQLite connections opened by this process.
pub fn opened_connections() -> u64 {
    OPENED_CONNECTIONS.load(Ordering::Relaxed)
}

/// Install request counters on server connections, including connections reused later.
/// Ordinary CLI processes keep their tracing off unless explicitly profiled.
pub fn enable_request_tracing() {
    REQUEST_TRACING.store(true, Ordering::Relaxed);
}

pub static SQL_NANOS: AtomicU64 = AtomicU64::new(0);

pub fn observe_sqlite(connection: &rusqlite::Connection) {
    attach_trace(connection, true);
}

/// Re-arm tracing on a pooled connection checked out for reuse without
/// double-counting physical connection opens.
pub(crate) fn rearm_sqlite(connection: &rusqlite::Connection) {
    attach_trace(connection, false);
}

fn attach_trace(connection: &rusqlite::Connection, count_open: bool) {
    #[cfg(any(test, feature = "test-support"))]
    if subscription::observe(connection, count_open) {
        assert!(
            !ENABLED.load(Ordering::Relaxed),
            "measurement tracers must not overlap"
        );
        return;
    }
    if !ENABLED.load(Ordering::Relaxed) && !REQUEST_TRACING.load(Ordering::Relaxed) {
        return;
    }
    let mut events = TraceEventCodes::SQLITE_TRACE_STMT | TraceEventCodes::SQLITE_TRACE_ROW;
    if ENABLED.load(Ordering::Relaxed) {
        if count_open {
            CONNECTIONS.fetch_add(1, Ordering::Relaxed);
        }
        events |= TraceEventCodes::SQLITE_TRACE_PROFILE;
    }
    connection.trace_v2(events, Some(count_sqlite));
}

#[cfg(any(test, feature = "test-support"))]
mod subscription;

fn count_sqlite(event: TraceEvent<'_>) {
    SCOPED.with(|slot| {
        if let Some(work) = slot.borrow().as_ref() {
            match &event {
                TraceEvent::Stmt(..) => {
                    work.statements.fetch_add(1, Ordering::Relaxed);
                }
                TraceEvent::Row(..) => {
                    work.rows.fetch_add(1, Ordering::Relaxed);
                }
                _ => {}
            }
        }
    });
    if !ENABLED.load(Ordering::Relaxed) {
        return;
    }
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

/// A request's SQL work, isolated from simultaneous requests on other workers.
#[derive(Debug, Clone, Copy, serde::Serialize, serde::Deserialize, schemars::JsonSchema)]
pub struct SqlWork {
    pub statements: u64,
    pub rows: u64,
}
/// One evaluation can be shared by several subscriptions. Its identity lets
/// clients count that physical work once within an interaction.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, schemars::JsonSchema)]
pub struct SqlReceipt {
    pub id: String,
    pub statements: u64,
    pub rows: u64,
}

impl SqlWork {
    pub fn receipt(self) -> SqlReceipt {
        SqlReceipt {
            id: uuid::Uuid::new_v4().to_string(),
            statements: self.statements,
            rows: self.rows,
        }
    }
}

#[derive(Debug, Default)]
struct ScopedWork {
    statements: AtomicU64,
    rows: AtomicU64,
}
thread_local! { static SCOPED: RefCell<Option<Arc<ScopedWork>>> = const { RefCell::new(None) }; }

fn scope<T>(work: Option<Arc<ScopedWork>>, run: impl FnOnce() -> T) -> T {
    struct Restore(Option<Arc<ScopedWork>>);
    impl Drop for Restore {
        fn drop(&mut self) {
            SCOPED.with(|slot| *slot.borrow_mut() = self.0.take());
        }
    }
    let _restore = Restore(SCOPED.with(|slot| slot.replace(work)));
    run()
}

pub fn capture<T>(run: impl FnOnce() -> T) -> (T, SqlWork) {
    let work = Arc::new(ScopedWork::default());
    let value = scope(Some(work.clone()), run);
    (
        value,
        SqlWork {
            statements: work.statements.load(Ordering::Relaxed),
            rows: work.rows.load(Ordering::Relaxed),
        },
    )
}

/// Carry the request's counter into an owned blocking worker; never infer it
/// from process-wide totals, which include unrelated requests.
pub fn inherit<T>(run: impl FnOnce() -> T) -> impl FnOnce() -> T {
    let work = SCOPED.with(|slot| slot.borrow().clone());
    move || scope(work, run)
}

#[cfg(test)]
mod request_tests {
    #[test]
    fn concurrent_scopes_count_reused_connections_and_inherited_workers() {
        super::enable_request_tracing();
        std::thread::scope(|threads| {
            let jobs: Vec<_> = [2, 7].into_iter().map(|rows| threads.spawn(move || {
                let connection = rusqlite::Connection::open_in_memory().unwrap();
                super::observe_sqlite(&connection);
                let (_, work) = super::capture(|| {
                    connection.query_row("SELECT 1", [], |_| Ok(())).unwrap();
                    let child = super::inherit(move || {
                        let connection = rusqlite::Connection::open_in_memory().unwrap();
                        super::observe_sqlite(&connection);
                        let mut query = connection.prepare("WITH RECURSIVE n(x) AS (VALUES(1) UNION ALL SELECT x+1 FROM n WHERE x < ?1) SELECT x FROM n").unwrap();
                        query.query_map([rows], |_| Ok(())).unwrap().collect::<Result<Vec<_>,_>>().unwrap();
                    });
                    std::thread::spawn(child).join().unwrap();
                });
                assert_eq!(work.statements, 2);
                assert_eq!(work.rows, rows as u64 + 1);
                let (_, empty) = super::capture(|| {});
                assert_eq!(empty.statements, 0);
            })).collect();
            for job in jobs {
                job.join().unwrap();
            }
        });
    }
}
