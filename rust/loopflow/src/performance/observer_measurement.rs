//! Opt-in whole-loop counters for the ignored observer baseline. Never compiled
//! into an installed binary. Fixture/writer connections open before start().
use std::collections::BTreeMap;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering::Relaxed};
use std::sync::Mutex;

use rusqlite::trace::{TraceEvent, TraceEventCodes};
use rusqlite::{Connection, StatementStatus};
use serde::Serialize;

static ENABLED: AtomicBool = AtomicBool::new(false);
static PROFILE: AtomicBool = AtomicBool::new(false);
static STATEMENTS: AtomicU64 = AtomicU64::new(0);
static ROWS: AtomicU64 = AtomicU64::new(0);
static VM: AtomicU64 = AtomicU64::new(0);
static POLLS: AtomicU64 = AtomicU64::new(0);
static WARNINGS: AtomicU64 = AtomicU64::new(0);
static OPENS: AtomicU64 = AtomicU64::new(0);
static CLOSES: AtomicU64 = AtomicU64::new(0);
static QUERIES: Mutex<BTreeMap<String, (u64, u64)>> = Mutex::new(BTreeMap::new());

pub(crate) fn start() {
    use tracing_subscriber::prelude::*;
    struct Warnings;
    impl<S: tracing::Subscriber> tracing_subscriber::Layer<S> for Warnings {
        fn on_event(
            &self,
            event: &tracing::Event<'_>,
            _: tracing_subscriber::layer::Context<'_, S>,
        ) {
            if *event.metadata().level() <= tracing::Level::WARN {
                WARNINGS.fetch_add(1, Relaxed);
            }
        }
    }
    tracing_subscriber::registry()
        .with(Warnings)
        .with(
            tracing_subscriber::fmt::layer()
                .with_writer(std::io::stderr)
                .with_filter(tracing_subscriber::filter::LevelFilter::WARN),
        )
        .try_init()
        .unwrap();
    ENABLED.store(true, Relaxed);
}

pub(crate) fn stop() {
    ENABLED.store(false, Relaxed);
}

pub(crate) fn begin_window(profile: bool) {
    QUERIES.lock().unwrap().clear();
    PROFILE.store(profile, Relaxed);
}

pub(crate) fn end_window() {
    if PROFILE.swap(false, Relaxed) {
        eprintln!(
            "OBSERVER_PROFILE {}",
            serde_json::to_string(&*QUERIES.lock().unwrap()).unwrap()
        );
    }
}

pub(crate) fn observe(connection: &Connection) {
    if !ENABLED.load(Relaxed) {
        return;
    }
    OPENS.fetch_add(1, Relaxed);
    connection.trace_v2(
        TraceEventCodes::SQLITE_TRACE_STMT
            | TraceEventCodes::SQLITE_TRACE_ROW
            | TraceEventCodes::SQLITE_TRACE_PROFILE
            | TraceEventCodes::SQLITE_TRACE_CLOSE,
        Some(|event| match event {
            TraceEvent::Stmt(_, _) => {
                STATEMENTS.fetch_add(1, Relaxed);
            }
            TraceEvent::Row(_) => {
                ROWS.fetch_add(1, Relaxed);
            }
            TraceEvent::Close(_) => {
                CLOSES.fetch_add(1, Relaxed);
            }
            TraceEvent::Profile(statement, _) => {
                let steps = statement.get_status(StatementStatus::VmStep) as u64;
                VM.fetch_add(steps, Relaxed);
                let sql = statement.sql();
                if sql.contains("SELECT revision FROM store_revisions WHERE domain='task_updates'")
                {
                    POLLS.fetch_add(1, Relaxed);
                }
                if PROFILE.load(Relaxed) {
                    let mut queries = QUERIES.lock().unwrap();
                    let entry = queries.entry(sql.to_string()).or_default();
                    entry.0 += 1;
                    entry.1 += steps;
                }
            }
            _ => {}
        }),
    );
}

#[derive(Debug, Clone, Copy, Serialize)]
pub(crate) struct Snapshot {
    pub statements: u64,
    pub rows: u64,
    pub vm_steps: u64,
    pub polls: u64,
    pub warnings: u64,
    pub opens: u64,
    pub closes: u64,
    pub cpu_ms: f64,
    pub voluntary_switches: i64,
    pub involuntary_switches: i64,
}

pub(crate) fn snapshot() -> Snapshot {
    // SAFETY: getrusage initializes a correctly sized, writable rusage value.
    let usage = unsafe {
        let mut usage = std::mem::zeroed::<libc::rusage>();
        assert_eq!(libc::getrusage(libc::RUSAGE_SELF, &mut usage), 0);
        usage
    };
    Snapshot {
        statements: STATEMENTS.load(Relaxed),
        rows: ROWS.load(Relaxed),
        vm_steps: VM.load(Relaxed),
        polls: POLLS.load(Relaxed),
        warnings: WARNINGS.load(Relaxed),
        opens: OPENS.load(Relaxed),
        closes: CLOSES.load(Relaxed),
        cpu_ms: (usage.ru_utime.tv_sec + usage.ru_stime.tv_sec) as f64 * 1000.0
            + (usage.ru_utime.tv_usec + usage.ru_stime.tv_usec) as f64 / 1000.0,
        voluntary_switches: usage.ru_nvcsw,
        involuntary_switches: usage.ru_nivcsw,
    }
}

impl Snapshot {
    pub(crate) fn since(self, before: Self) -> Self {
        Self {
            statements: self.statements - before.statements,
            rows: self.rows - before.rows,
            vm_steps: self.vm_steps - before.vm_steps,
            polls: self.polls - before.polls,
            warnings: self.warnings - before.warnings,
            opens: self.opens - before.opens,
            closes: self.closes - before.closes,
            cpu_ms: self.cpu_ms - before.cpu_ms,
            voluntary_switches: self.voluntary_switches - before.voluntary_switches,
            involuntary_switches: self.involuntary_switches - before.involuntary_switches,
        }
    }
}
