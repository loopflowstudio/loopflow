//! Native dispatch holds the Session fence — the store mutex and SQLite's
//! write lock — while one transport write completes. Whatever holds that fence
//! must finish without help from a thread that may be waiting for it.
//!
//! Two rules keep the fence from joining a cycle with the runtime:
//!
//! - The write is driven and timed on the dispatching thread. A stalled
//!   runtime cannot postpone the deadline, so the fence is always released.
//! - Store work reached from an async task leaves the runtime's worker first,
//!   so waiting for the fence never stops the runtime that the write needs.

use std::fmt::Display;
use std::future::Future;
use std::pin::pin;
use std::sync::Arc;
use std::task::{Context, Poll, Wake, Waker};
use std::thread::{self, Thread};
use std::time::{Duration, Instant};

use futures_util::{Sink, SinkExt};
use tokio::runtime::{Handle, RuntimeFlavor};

use crate::store::{StoreError, StoreResult};

/// Longest a native write may hold the Session fence.
pub(super) const DISPATCH_LIMIT: Duration = Duration::from_secs(2);

struct Unpark(Thread);

impl Wake for Unpark {
    fn wake(self: Arc<Self>) {
        self.0.unpark();
    }
}

/// Drive `future` on this thread until it finishes or `limit` of real time
/// passes. `None` means the limit passed; the future's effect is then unknown.
fn within<F: Future>(limit: Duration, future: F) -> Option<F::Output> {
    let waker = Waker::from(Arc::new(Unpark(thread::current())));
    let mut context = Context::from_waker(&waker);
    let mut future = pin!(future);
    let deadline = Instant::now() + limit;
    loop {
        if let Poll::Ready(output) = future.as_mut().poll(&mut context) {
            return Some(output);
        }
        let remaining = deadline.saturating_duration_since(Instant::now());
        if remaining.is_zero() {
            return None;
        }
        thread::park_timeout(remaining);
    }
}

/// Send one message from a blocking thread that holds the Session fence.
pub(super) fn send_fenced<S, M>(sink: &mut S, message: M) -> StoreResult<()>
where
    S: Sink<M> + Unpin,
    S::Error: Display,
{
    within(DISPATCH_LIMIT, sink.send(message))
        .ok_or_else(|| {
            StoreError::InvalidData("Native dispatch timed out; outcome is unknown".into())
        })?
        .map_err(|error| StoreError::InvalidData(format!("Native dispatch failed: {error}")))
}

/// Run blocking store work from an async task without occupying the runtime
/// worker that drives I/O and timers.
pub(super) fn off_reactor<T>(work: impl FnOnce() -> T) -> T {
    match Handle::try_current().map(|handle| handle.runtime_flavor()) {
        Ok(RuntimeFlavor::MultiThread) => tokio::task::block_in_place(work),
        _ => work(),
    }
}

#[cfg(test)]
mod tests {
    use super::{off_reactor, within};
    use crate::exec::SessionDriver;
    use crate::id::ExecId;
    use crate::store::sqlite::SqliteStore;
    use std::sync::atomic::{AtomicBool, Ordering};
    use std::sync::{mpsc, Arc};
    use std::time::Duration;
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    use tokio::net::UnixStream;

    fn fenced_session(home: &tempfile::TempDir) -> (SqliteStore, SessionDriver) {
        let path = home.path().join("dispatch.db");
        let store = SqliteStore::open_ephemeral(&path).unwrap();
        store.test_session("conversation", "run_00000000000000000000000000000001");
        let exec = ExecId::new();
        rusqlite::Connection::open(&path)
            .unwrap()
            .execute(
                "INSERT INTO execs(id,trace_id,started_at) VALUES(?1,'fixture',1)",
                [exec.as_str()],
            )
            .unwrap();
        let driver = store
            .claim_session_driver("conversation", None, &exec, false)
            .unwrap();
        (store, driver)
    }

    /// One worker is the smallest runtime in which a waiting reader can stop
    /// everything else. A hang fails here instead of stalling the suite.
    fn on_one_worker<T: Send + 'static>(
        scenario: impl std::future::Future<Output = T> + Send + 'static,
    ) -> T {
        let (done, outcome) = mpsc::channel();
        std::thread::spawn(move || {
            let runtime = tokio::runtime::Builder::new_multi_thread()
                .worker_threads(1)
                .enable_all()
                .build()
                .unwrap();
            let _ = done.send(runtime.block_on(async { tokio::spawn(scenario).await.unwrap() }));
        });
        outcome
            .recv_timeout(Duration::from_secs(20))
            .expect("fenced dispatch and its reader deadlocked")
    }

    /// More bytes than a socket pair buffers, so the write waits for its peer.
    const UNBUFFERED: usize = 8 << 20;

    #[test]
    fn reader_waiting_for_the_fence_leaves_the_runtime_running() {
        let home = tempfile::tempdir().unwrap();
        let (store, driver) = fenced_session(&home);
        let (written, read, ticked) = on_one_worker(async move {
            let (mut engine, mut peer) = UnixStream::pair().unwrap();
            let (held, holding) = tokio::sync::oneshot::channel();
            let ticked = Arc::new(AtomicBool::new(false));
            let writer_store = store.clone();
            let writer = tokio::task::spawn_blocking(move || {
                writer_store.with_session_driver("conversation", &driver, || {
                    held.send(()).unwrap();
                    // Let the reader reach the fence before the write needs
                    // the runtime.
                    std::thread::sleep(Duration::from_millis(100));
                    Ok(within(
                        Duration::from_secs(10),
                        engine.write_all(&vec![0; UNBUFFERED]),
                    )
                    .map(|written| written.is_ok()))
                })
            });
            // The provider drains the socket and a timer fires only while the
            // runtime keeps running behind the waiting reader.
            let drain = tokio::spawn(async move {
                let mut bytes = vec![0; UNBUFFERED];
                peer.read_exact(&mut bytes).await.is_ok()
            });
            let tick = ticked.clone();
            tokio::spawn(async move {
                tokio::time::sleep(Duration::from_millis(10)).await;
                tick.store(true, Ordering::Release);
            });
            holding.await.unwrap();
            // The notification reader records history from the runtime.
            let read = off_reactor(|| store.session_thread("conversation"));
            let written = writer.await.unwrap();
            assert!(drain.await.unwrap());
            (written, read, ticked.load(Ordering::Acquire))
        });
        assert_eq!(written.unwrap(), Some(true), "dispatch reached its peer");
        assert_eq!(
            read.unwrap(),
            None,
            "history read completed after the fence"
        );
        assert!(ticked, "the runtime made progress behind the reader");
    }

    #[test]
    fn stalled_runtime_cannot_hold_the_fence_past_its_limit() {
        let home = tempfile::tempdir().unwrap();
        let (store, driver) = fenced_session(&home);
        let (written, read) = on_one_worker(async move {
            let (mut engine, _unread_peer) = UnixStream::pair().unwrap();
            let (held, holding) = tokio::sync::oneshot::channel();
            let writer_store = store.clone();
            let writer = tokio::task::spawn_blocking(move || {
                writer_store.with_session_driver("conversation", &driver, || {
                    held.send(()).unwrap();
                    Ok(within(
                        Duration::from_millis(300),
                        engine.write_all(&vec![0; UNBUFFERED]),
                    )
                    .is_some())
                })
            });
            holding.await.unwrap();
            // The only worker waits here, so no runtime timer or I/O event
            // can end the write; its own deadline must.
            let read = store.session_thread("conversation");
            (writer.await.unwrap(), read)
        });
        assert!(!written.unwrap(), "an unread write reports its timeout");
        assert_eq!(
            read.unwrap(),
            None,
            "the reader acquired the released fence"
        );
    }
}
