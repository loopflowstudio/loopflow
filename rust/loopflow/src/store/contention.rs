//! Name who holds the store's write lock. SQLite cannot say which connection
//! holds it, and a waiter that times out knows only its own wait. So every write
//! connection reports its own long holds, every lf.log line names the process
//! and binary that wrote it, and `lf doctor` reads both back grouped by binary.

use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use rusqlite::{ffi, Connection};

use crate::diagnostic_log::{field, read_tail};

/// Longer than any healthy write, and well inside the 15 s other writers wait.
const SLOW_WRITE: Duration = if cfg!(test) {
    Duration::from_millis(200)
} else {
    Duration::from_secs(3)
};

/// VM steps between checks for a newly taken write lock: microseconds apart.
const PROGRESS_STEPS: i32 = 1000;

const SLOW_WRITE_MESSAGE: &str = "held the store write lock";
const LOCK_FAILURE: &str = "database is locked";

// ---------------------------------------------------------------------------
// Writer side
// ---------------------------------------------------------------------------

/// The connection's raw handle, read only by hooks that connection owns.
struct Handle(*mut ffi::sqlite3);

// SAFETY: SQLite runs a connection's hooks only inside that connection's own
// API calls, on the calling thread, while it is open; the hooks are dropped
// with the connection, so the handle is never used from two threads at once.
unsafe impl Send for Handle {}

impl Handle {
    fn writing(&self) -> bool {
        // SAFETY: see `Handle`; a null schema asks about every attached database.
        unsafe { ffi::sqlite3_txn_state(self.0, std::ptr::null()) == ffi::SQLITE_TXN_WRITE }
    }
}

/// Report any write transaction on `conn` that holds the lock for `SLOW_WRITE`
/// or longer, naming the Rust function that committed or rolled it back.
///
/// The clock starts at the first VM step or row change after the lock is
/// taken, so Rust work between `BEGIN` and the first statement goes unmeasured.
pub(crate) fn watch_write_holds(conn: &Connection) -> rusqlite::Result<()> {
    // SAFETY: the handle outlives every hook installed below; see `Handle`.
    let handle = Handle(unsafe { conn.handle() });
    let since = Arc::new(Mutex::new(None::<Instant>));

    let stepping = since.clone();
    conn.progress_handler(
        PROGRESS_STEPS,
        Some(move || {
            let mut since = stepping.lock().expect("write hold mutex poisoned");
            if since.is_none() && handle.writing() {
                *since = Some(Instant::now());
            }
            false
        }),
    )?;
    // Short writes finish before a progress step; a changed row proves the lock.
    let changing = since.clone();
    conn.update_hook(Some(move |_, _: &str, _: &str, _| {
        changing
            .lock()
            .expect("write hold mutex poisoned")
            .get_or_insert_with(Instant::now);
    }))?;
    let committing = since.clone();
    conn.commit_hook(Some(move || {
        report(committing.lock().expect("write hold mutex poisoned").take());
        false
    }))?;
    conn.rollback_hook(Some(move || {
        report(since.lock().expect("write hold mutex poisoned").take());
    }))?;
    Ok(())
}

fn report(since: Option<Instant>) {
    let Some(held) = since.map(|since| since.elapsed()) else {
        return;
    };
    if held >= SLOW_WRITE {
        tracing::warn!(
            held_seconds = held.as_secs_f64(),
            call_site = %call_site(),
            "{SLOW_WRITE_MESSAGE}"
        );
    }
}

/// The innermost Loopflow function on the stack outside these hooks.
fn call_site() -> String {
    let own = |name: &str| {
        name.strip_prefix(concat!(module_path!(), "::"))
            .is_some_and(|rest| matches!(rest, "watch_write_holds" | "report" | "call_site"))
    };
    std::backtrace::Backtrace::force_capture()
        .to_string()
        .lines()
        .filter_map(frame_function)
        .find(|name| {
            (name.starts_with("loopflow::") || name.starts_with("<loopflow::")) && !own(name)
        })
        .unwrap_or_else(|| "unknown".to_owned())
}

/// `   7: loopflow::a::b::{{closure}}::h0123…` → `loopflow::a::b`.
fn frame_function(line: &str) -> Option<String> {
    let (index, name) = line.trim_start().split_once(": ")?;
    if !index.chars().all(|c| c.is_ascii_digit()) {
        return None;
    }
    let mut name = name.trim();
    if let Some((path, hash)) = name.rsplit_once("::h") {
        if hash.len() == 16 && hash.chars().all(|c| c.is_ascii_hexdigit()) {
            name = path;
        }
    }
    // `{{closure}}` or `{closure#2}`, depending on the symbol mangling.
    while let Some((outer, _)) = name
        .rsplit_once("::")
        .filter(|(_, last)| last.starts_with('{'))
    {
        name = outer;
    }
    Some(name.to_owned())
}

// ---------------------------------------------------------------------------
// Reading it back
// ---------------------------------------------------------------------------

/// Contention one binary logged: as victim (lock failures) or holder (slow writes).
#[derive(Debug, Clone, Default, PartialEq)]
pub struct BinaryContention {
    pub lock_failures: usize,
    pub slow_writes: usize,
    pub longest_hold_seconds: f64,
    pub call_sites: BTreeSet<String>,
    pub pids: BTreeSet<u32>,
}

/// Binary label for lines written before lf named itself in lf.log.
const UNIDENTIFIED: &str = "unidentified (older lf)";

/// Contention the end of `log` records at or after `since` (Unix seconds),
/// keyed by `lf <version> (<revision>)`. A missing log records none.
pub fn read_contention(
    log: &Path,
    since: i64,
) -> std::io::Result<BTreeMap<String, BinaryContention>> {
    Ok(summarize(&read_tail(log)?, since))
}

fn summarize(log: &str, since: i64) -> BTreeMap<String, BinaryContention> {
    let mut binaries: BTreeMap<String, BinaryContention> = BTreeMap::new();
    for line in log.lines() {
        let failed = line.contains(LOCK_FAILURE);
        let slow = line.contains(SLOW_WRITE_MESSAGE);
        if !failed && !slow {
            continue;
        }
        let logged = line
            .split_whitespace()
            .next()
            .and_then(|stamp| chrono::DateTime::parse_from_rfc3339(stamp).ok());
        if logged.is_none_or(|logged| logged.timestamp() < since) {
            continue;
        }
        let binary = match (field(line, "lf"), field(line, "rev")) {
            (Some(version), Some(revision)) => format!("lf {version} ({revision})"),
            _ => UNIDENTIFIED.to_owned(),
        };
        let entry = binaries.entry(binary).or_default();
        if let Some(pid) = field(line, "pid").and_then(|pid| pid.parse().ok()) {
            entry.pids.insert(pid);
        }
        if slow {
            entry.slow_writes += 1;
            let held = field(line, "held_seconds").and_then(|held| held.parse().ok());
            entry.longest_hold_seconds = entry.longest_hold_seconds.max(held.unwrap_or(0.0));
            if let Some(site) = field(line, "call_site") {
                entry.call_sites.insert(site.to_owned());
            }
        } else {
            entry.lock_failures += 1;
        }
    }
    binaries
}

#[cfg(test)]
mod tests {
    use std::io::Write;
    use std::sync::{Arc, Mutex};

    use super::{frame_function, summarize, UNIDENTIFIED};
    use crate::diagnostic_log::{field, IdentifiedLines};
    use crate::store::sqlite::SqliteStore;

    #[derive(Clone, Default)]
    struct Captured(Arc<Mutex<Vec<u8>>>);

    impl Write for Captured {
        fn write(&mut self, bytes: &[u8]) -> std::io::Result<usize> {
            self.0.lock().unwrap().extend_from_slice(bytes);
            Ok(bytes.len())
        }
        fn flush(&mut self) -> std::io::Result<()> {
            Ok(())
        }
    }

    fn slow_write_in_named_function(store: &SqliteStore) {
        let mut conn = store.conn.lock().unwrap();
        conn.execute("CREATE TABLE IF NOT EXISTS held(x)", [])
            .unwrap();
        let tx = conn.transaction().unwrap();
        // A changed row starts the clock; DDL alone may never tick the progress handler.
        tx.execute("INSERT INTO held VALUES (0)", []).unwrap();
        std::thread::sleep(std::time::Duration::from_millis(300));
        tx.execute("INSERT INTO held VALUES (1)", []).unwrap();
        tx.commit().unwrap();
    }

    #[test]
    fn a_slow_writer_names_itself_and_doctor_groups_it_by_binary() {
        let dir = tempfile::tempdir().unwrap();
        let store = SqliteStore::open_ephemeral(&dir.path().join("loopflow.db")).unwrap();
        let log = Captured::default();
        let writer = log.clone();
        let subscriber = tracing_subscriber::fmt()
            .with_ansi(false)
            .with_writer(move || writer.clone())
            .event_format(IdentifiedLines)
            .finish();
        tracing::subscriber::with_default(subscriber, || slow_write_in_named_function(&store));

        let text = String::from_utf8(log.0.lock().unwrap().clone()).unwrap();
        let line = text
            .lines()
            .find(|line| line.contains("held the store write lock"))
            .unwrap_or_else(|| panic!("no slow-writer line in {text:?}"));
        assert_eq!(field(line, "pid"), Some(&*std::process::id().to_string()));
        assert!(
            field(line, "call_site")
                .unwrap()
                .ends_with("slow_write_in_named_function"),
            "{line}"
        );
        let held: f64 = field(line, "held_seconds").unwrap().parse().unwrap();
        assert!(held >= 0.3, "{line}");

        let summary = summarize(&text, 0);
        let (binary, contention) = summary.iter().next().unwrap();
        assert!(binary.starts_with("lf "), "{binary}");
        assert_eq!(contention.slow_writes, 1);
        assert!(contention.pids.contains(&std::process::id()));
    }

    #[test]
    fn a_fast_write_is_not_reported() {
        let dir = tempfile::tempdir().unwrap();
        let store = SqliteStore::open_ephemeral(&dir.path().join("loopflow.db")).unwrap();
        let log = Captured::default();
        let writer = log.clone();
        let subscriber = tracing_subscriber::fmt()
            .with_writer(move || writer.clone())
            .finish();
        tracing::subscriber::with_default(subscriber, || {
            store
                .conn
                .lock()
                .unwrap()
                .execute_batch("CREATE TABLE quick(x); INSERT INTO quick VALUES (1);")
                .unwrap();
        });
        assert!(log.0.lock().unwrap().is_empty());
    }

    #[test]
    fn lock_failures_group_by_binary_and_respect_the_window() {
        let log = "\
2026-10-27T00:02:00.000000Z  WARN loopflow::journal: ledger insert failed error=sqlite error: database is locked waited_seconds=15.5
2026-10-27T04:19:00.000000Z  WARN loopflow::journal: ledger insert failed error=sqlite error: database is locked waited_seconds=15.5
2026-10-27T04:19:01.000000Z  WARN loopflow::harness::activity_tracker: failed to save Session activity error=database is locked pid=41 lf=0.14.18 rev=aaaaaaaaa
2026-10-27T04:19:02.000000Z  WARN loopflow::store::contention: held the store write lock held_seconds=9.12 call_site=loopflow::store::sqlite::retention::retain_history_in pid=7 lf=0.14.16 rev=d49c57c12
2026-10-27T04:19:03.000000Z  WARN loopflow::store::contention: held the store write lock held_seconds=2.5 call_site=loopflow::store::sqlite::session_events::record_usage pid=8 lf=0.14.16 rev=d49c57c12
2026-10-27T04:19:04.000000Z  WARN loopflow::disk: unrelated pid=7 lf=0.14.16 rev=d49c57c12
";
        let since = chrono::DateTime::parse_from_rfc3339("2026-10-27T04:00:00Z")
            .unwrap()
            .timestamp();
        let summary = summarize(log, since);

        assert_eq!(summary[UNIDENTIFIED].lock_failures, 1);
        assert_eq!(summary["lf 0.14.18 (aaaaaaaaa)"].lock_failures, 1);
        let holder = &summary["lf 0.14.16 (d49c57c12)"];
        assert_eq!(holder.slow_writes, 2);
        assert_eq!(holder.longest_hold_seconds, 9.12);
        assert_eq!(holder.pids.len(), 2);
        assert!(holder
            .call_sites
            .contains("loopflow::store::sqlite::retention::retain_history_in"));
    }

    #[test]
    fn frames_drop_hashes_and_closures() {
        assert_eq!(
            frame_function("  12: loopflow::store::retain::{{closure}}::h0123456789abcdef")
                .as_deref(),
            Some("loopflow::store::retain")
        );
        assert_eq!(
            frame_function("   3: loopflow::store::retain::{closure#2}").as_deref(),
            Some("loopflow::store::retain")
        );
        assert_eq!(frame_function("             at ./src/store.rs:12:5"), None);
    }
}
