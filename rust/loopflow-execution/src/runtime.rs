//! One shared Tokio runtime handle per process.
//!
//! `lf server run` builds a bounded multi-thread runtime and installs its handle
//! here via [`install`]. Synchronous callers (CLI commands, `read_stream`,
//! blocking ops) run futures through [`block_on`] instead of constructing fresh
//! `Runtime::new()` pools on every request or subscription.
//!
//! Inside [`block_on`], [`in_block_on`] reports `true` on the calling thread so
//! synchronous SQLite helpers (`store::run_sqlite`) execute directly on that
//! already-dedicated blocking thread instead of re-entering the runtime's
//! blocking pool and risking saturation deadlocks.

use std::cell::Cell;
use std::future::Future;
use std::sync::{LazyLock, RwLock};

use tokio::runtime::{Handle, Runtime};

static INSTALLED: RwLock<Option<(u64, Handle)>> = RwLock::new(None);
static NEXT_TOKEN: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(1);

static FALLBACK: LazyLock<Runtime> = LazyLock::new(|| {
    tokio::runtime::Builder::new_multi_thread()
        .worker_threads(2)
        .max_blocking_threads(8)
        .thread_name("lf-async")
        .enable_all()
        .build()
        .expect("build shared Loopflow async runtime")
});

thread_local! {
    static IN_BLOCK_ON: Cell<bool> = const { Cell::new(false) };
}

/// Guard returned by [`install`] that clears the installed server handle on drop.
#[derive(Debug)]
pub struct InstalledHandle {
    token: u64,
}

impl Drop for InstalledHandle {
    fn drop(&mut self) {
        let mut guard = INSTALLED.write().unwrap_or_else(|p| p.into_inner());
        if guard
            .as_ref()
            .is_some_and(|(token, _)| *token == self.token)
        {
            *guard = None;
        }
    }
}

/// Register the process's shared async runtime (called once by `lf server run`).
pub fn install(handle: Handle) -> InstalledHandle {
    let token = NEXT_TOKEN.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    *INSTALLED.write().unwrap_or_else(|p| p.into_inner()) = Some((token, handle));
    InstalledHandle { token }
}

/// The runtime handle [`block_on`] will drive futures on: the active Tokio
/// runtime when already inside one, else the installed server runtime, else
/// a lazily created small multi-thread runtime for bare CLI processes.
pub fn handle() -> Handle {
    if let Ok(current) = Handle::try_current() {
        return current;
    }
    if let Some((_, handle)) = INSTALLED.read().unwrap_or_else(|p| p.into_inner()).as_ref() {
        return handle.clone();
    }
    FALLBACK.handle().clone()
}

/// Whether the current thread is driving a future via [`block_on`].
///
/// Always `false` on Tokio async worker threads (including tasks spawned via
/// `tokio::spawn` from inside [`block_on`]), and `true` on the blocking or CLI
/// thread directly polling the future.
pub(crate) fn in_block_on() -> bool {
    IN_BLOCK_ON.with(Cell::get)
}

/// Drive `future` to completion on the shared process runtime without
/// constructing a new Tokio runtime.
pub fn block_on<F: Future>(future: F) -> F::Output {
    struct Restore(bool);
    impl Drop for Restore {
        fn drop(&mut self) {
            IN_BLOCK_ON.with(|flag| flag.set(self.0));
        }
    }
    let handle = handle();
    let _restore = Restore(IN_BLOCK_ON.with(|flag| flag.replace(true)));
    handle.block_on(future)
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use super::{block_on, in_block_on, install};

    #[test]
    fn block_on_works_from_plain_thread_and_installed_handle() {
        assert!(!in_block_on());
        let value = block_on(async {
            assert!(in_block_on());
            tokio::time::sleep(Duration::from_millis(1)).await;
            42
        });
        assert_eq!(value, 42);
        assert!(!in_block_on());

        let rt = tokio::runtime::Builder::new_multi_thread()
            .worker_threads(1)
            .max_blocking_threads(2)
            .enable_all()
            .build()
            .unwrap();
        let _guard = install(rt.handle().clone());
        let spawned_flag = block_on(async {
            assert!(in_block_on());
            tokio::spawn(async { in_block_on() }).await.unwrap()
        });
        assert!(
            !spawned_flag,
            "tasks spawned onto async workers must not inherit in_block_on"
        );
    }

    #[test]
    fn block_on_inside_saturated_blocking_pool_runs_sqlite_inline_without_deadlock() {
        let dir = tempfile::tempdir().unwrap();
        let store = crate::store::opening::open_ephemeral(&dir.path().join("db")).unwrap();
        let rt = tokio::runtime::Builder::new_multi_thread()
            .worker_threads(1)
            .max_blocking_threads(1)
            .enable_all()
            .build()
            .unwrap();
        // Occupy the single blocking thread, then call block_on -> run_sqlite.
        let revisions = rt.block_on(async move {
            tokio::time::timeout(
                Duration::from_secs(5),
                tokio::task::spawn_blocking(move || {
                    block_on(async move {
                        crate::store::run_sqlite(&store, |sqlite| sqlite.revisions()).await
                    })
                }),
            )
            .await
            .expect("saturated blocking pool deadlocked")
            .unwrap()
            .unwrap()
        });
        assert!(revisions.planning >= 0);
    }
}
