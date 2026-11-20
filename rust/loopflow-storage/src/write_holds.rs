//! Observe the lifetime of SQLite write locks on each connection.

use rusqlite::{ffi, Connection};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

/// Longer than any healthy write, and well inside the 15 s other writers wait.
const SLOW_WRITE: Duration = if cfg!(any(test, feature = "test-support")) {
    Duration::from_millis(200)
} else {
    Duration::from_secs(3)
};

/// VM steps between checks for a newly taken write lock: microseconds apart.
/// Tests check after every step, so a transaction that took the lock and only
/// read is seen holding it before it rolls back (see `empty_writes`).
const PROGRESS_STEPS: i32 = if cfg!(any(test, feature = "test-support")) {
    1
} else {
    1000
};

pub const SLOW_WRITE_MESSAGE: &str = "held the store write lock";

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
pub fn watch_write_holds(conn: &Connection) -> rusqlite::Result<()> {
    // SAFETY: the handle outlives every hook installed below; see `Handle`.
    let handle = Handle(unsafe { conn.handle() });
    #[cfg(any(test, feature = "test-support"))]
    let connection = handle.0 as usize;
    let hold = Arc::new(Mutex::new(Hold::default()));

    let stepping = hold.clone();
    conn.progress_handler(
        PROGRESS_STEPS,
        Some(move || {
            let mut hold = stepping.lock().expect("write hold mutex poisoned");
            if hold.since.is_none() && handle.writing() {
                hold.since = Some(Instant::now());
            }
            false
        }),
    )?;
    // Short writes finish before a progress step; a changed row proves the lock.
    let changing = hold.clone();
    conn.update_hook(Some(move |_, _: &str, _: &str, _| {
        let mut hold = changing.lock().expect("write hold mutex poisoned");
        hold.since.get_or_insert_with(Instant::now);
        hold.changed = true;
    }))?;
    // A commit hook runs only for a transaction that took the write lock.
    let committing = hold.clone();
    conn.commit_hook(Some(move || {
        let hold = std::mem::take(&mut *committing.lock().expect("write hold mutex poisoned"));
        #[cfg(any(test, feature = "test-support"))]
        if !hold.changed {
            empty_writes::note(connection);
        }
        report(hold.since);
        false
    }))?;
    // A rollback hook also runs when a read ends, so only a seen lock counts.
    conn.rollback_hook(Some(move || {
        let hold = std::mem::take(&mut *hold.lock().expect("write hold mutex poisoned"));
        #[cfg(any(test, feature = "test-support"))]
        if hold.since.is_some() && !hold.changed {
            empty_writes::note(connection);
        }
        report(hold.since);
    }))?;
    Ok(())
}

/// The write transaction a connection is in, as its hooks have seen it.
#[derive(Default)]
struct Hold {
    since: Option<Instant>,
    // Only tests read it, to find writes that changed nothing.
    #[cfg_attr(not(test), allow(dead_code))]
    changed: bool,
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
            .is_some_and(|rest| {
                matches!(
                    rest,
                    "watch_write_holds" | "report" | "call_site" | "empty_writes::note"
                )
            })
    };
    std::backtrace::Backtrace::force_capture()
        .to_string()
        .lines()
        .filter_map(frame_function)
        .find(|name| {
            [
                "loopflow::",
                "<loopflow::",
                "loopflow_storage::",
                "<loopflow_storage::",
                "loopflow_execution::",
                "<loopflow_execution::",
            ]
            .iter()
            .any(|prefix| name.starts_with(prefix))
                && !own(name)
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

/// Write transactions that took the lock and changed no row, recorded per
/// connection while a test asks: the transaction may end on another thread.
#[cfg(any(test, feature = "test-support"))]
pub mod empty_writes {
    use std::collections::BTreeMap;
    use std::sync::Mutex;

    use rusqlite::Connection;

    /// Call sites by connection handle address, for connections being watched.
    static RECORDED: Mutex<BTreeMap<usize, Vec<String>>> = Mutex::new(BTreeMap::new());

    pub(super) fn note(connection: usize) {
        if let Some(sites) = RECORDED.lock().unwrap().get_mut(&connection) {
            sites.push(super::call_site());
        }
    }

    /// The call site of each write transaction on `conn` that ends during `run`
    /// without changing a row.
    pub fn record<R>(conn: &Mutex<Connection>, run: impl FnOnce() -> R) -> (R, Vec<String>) {
        // SAFETY: only the address is kept, as a key, while `conn` is open.
        let connection = unsafe { conn.lock().unwrap().handle() } as usize;
        RECORDED.lock().unwrap().insert(connection, Vec::new());
        let result = run();
        (
            result,
            RECORDED.lock().unwrap().remove(&connection).unwrap(),
        )
    }
}

#[cfg(test)]
mod tests {
    use super::{call_site, frame_function};

    #[test]
    fn call_sites_name_storage_callers() {
        assert!(call_site().starts_with("loopflow_storage::write_holds::tests::"));
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
