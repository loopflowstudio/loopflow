//! A server request is a trace, not an OS Process or a Session owner.
use std::cell::{Cell, RefCell};
use std::io::Write;
use std::sync::{Arc, Mutex};
use std::time::Instant;

use anyhow::Result;

use crate::id::TraceId;

thread_local! {
    static ACTIVE: Cell<bool> = const { Cell::new(false) };
    static DIAGNOSTICS: RefCell<Option<Diagnostics>> = const { RefCell::new(None) };
}

pub(crate) fn active() -> bool {
    ACTIVE.with(Cell::get)
}

/// Block-on operations run on this worker. Never install this scope on a
/// shared async executor: only the request's synchronous worker owns it.
pub(crate) fn run<T>(
    trace: &TraceId,
    target: &serde_json::Value,
    run: impl FnOnce() -> Result<T>,
) -> Result<T> {
    scope(true, DIAGNOSTICS.with(|sink| sink.borrow().clone()), || {
        let started = Instant::now();
        let span = tracing::info_span!(target: "loopflow::server::request", "server_request", trace_id = %trace, target = %target);
        let _entered = span.enter();
        tracing::info!(target: "loopflow::server::request", "request started");
        match std::panic::catch_unwind(std::panic::AssertUnwindSafe(run)) {
            Ok(result) => {
                let code = super::command_exit_code(&result);
                let error = result.as_ref().err().map(|error| format!("{error:#}"));
                tracing::info!(target: "loopflow::server::request", duration_ms = started.elapsed().as_millis(), code, error, "request finished");
                result
            }
            Err(panic) => {
                let error = panic
                    .downcast_ref::<&str>()
                    .copied()
                    .or_else(|| panic.downcast_ref::<String>().map(String::as_str))
                    .unwrap_or("request panicked");
                tracing::error!(target: "loopflow::server::request", duration_ms = started.elapsed().as_millis(), code = 101, error, "request panicked; outcome unknown");
                std::panic::resume_unwind(panic)
            }
        }
    })
}

type Diagnostics = Arc<Mutex<Vec<u8>>>;

fn scope<T>(active: bool, diagnostics: Option<Diagnostics>, run: impl FnOnce() -> T) -> T {
    struct Restore(bool, Option<Diagnostics>);
    impl Drop for Restore {
        fn drop(&mut self) {
            ACTIVE.with(|active| active.set(self.0));
            DIAGNOSTICS.with(|sink| *sink.borrow_mut() = self.1.take());
        }
    }
    let _restore = Restore(
        ACTIVE.with(|slot| slot.replace(active)),
        DIAGNOSTICS.with(|slot| slot.replace(diagnostics)),
    );
    run()
}

pub(crate) fn diagnostics<T>(error: &mut dyn Write, run: impl FnOnce() -> Result<T>) -> Result<T> {
    let sink = Arc::new(Mutex::new(Vec::new()));
    let result = scope(active(), Some(sink.clone()), run);
    error.write_all(&sink.lock().expect("diagnostic buffer"))?;
    result
}

/// Diagnostics follow the command's error stream through nested operations.
/// Unscoped CLI callers keep their ordinary stderr.
pub(crate) fn diagnostic(message: std::fmt::Arguments<'_>) {
    DIAGNOSTICS.with(|sink| match sink.borrow().as_ref() {
        Some(sink) => {
            let _ = writeln!(sink.lock().expect("diagnostic buffer"), "{message}");
        }
        None => eprintln!("{message}"),
    });
}

/// Carry request attribution and diagnostics into nested blocking work without
/// installing anything on the process or on another async task.
pub(crate) fn spawn_blocking<F, T>(run: F) -> tokio::task::JoinHandle<T>
where
    F: FnOnce() -> T + Send + 'static,
    T: Send + 'static,
{
    let active = active();
    let diagnostics = DIAGNOSTICS.with(|sink| sink.borrow().clone());
    let agent = crate::config::agent_override();
    let span = tracing::Span::current();
    tokio::task::spawn_blocking(move || {
        let _entered = span.enter();
        scope(active, diagnostics, || {
            crate::config::with_agent_override(agent.as_deref(), run)
        })
    })
}

#[cfg(test)]
mod tests {
    use super::{active, diagnostic, diagnostics, run, spawn_blocking};
    use crate::id::TraceId;

    #[test]
    fn request_unwind_restores_caller_and_nested_workers_keep_no_process() {
        let _ledger = crate::journal::TestLedgerGuard::new();
        let repo = loopflow_test_support::TestRepo::new();
        crate::journal::with_runtime(repo.path(), &["lf".into(), "test".into()], || {
            let caller = crate::journal::current_lf_process_id().unwrap();
            let failure = std::panic::catch_unwind(|| {
                let _ = run(
                    &TraceId::default(),
                    &serde_json::json!({"id": "one"}),
                    || {
                        assert!(active());
                        assert!(crate::journal::current_lf_process_id().is_none());
                        tokio::runtime::Runtime::new().unwrap().block_on(async {
                            spawn_blocking(|| {
                                assert!(active());
                                assert!(crate::journal::current_lf_process_id().is_none());
                                assert!(crate::journal::agent_caller().is_none());
                            })
                            .await
                            .unwrap();
                        });
                        panic!("request failed after admission");
                        #[allow(unreachable_code)]
                        Ok(())
                    },
                );
            });
            assert!(failure.is_err());
            assert!(!active());
            assert_eq!(crate::journal::current_lf_process_id(), Some(caller));
            Ok(())
        })
        .unwrap();
    }

    #[test]
    fn concurrent_requests_keep_their_own_diagnostics_and_agent_defaults() {
        let barrier = std::sync::Barrier::new(2);
        std::thread::scope(|threads| {
            let requests: Vec<_> = ["codex", "claude"]
                .into_iter()
                .map(|agent| {
                    let barrier = &barrier;
                    threads.spawn(move || {
                        let mut output = Vec::new();
                        run(
                            &TraceId::default(),
                            &serde_json::json!({"agent": agent}),
                            || {
                                crate::config::with_agent_override(Some(agent), || {
                                    diagnostics(&mut output, || {
                                        barrier.wait();
                                        assert_eq!(
                                            crate::config::agent_override().as_deref(),
                                            Some(agent)
                                        );
                                        tokio::runtime::Runtime::new().unwrap().block_on(async {
                                            spawn_blocking(|| {
                                                let agent =
                                                    crate::config::agent_override().unwrap();
                                                diagnostic(format_args!("{agent}"));
                                            })
                                            .await
                                            .unwrap();
                                        });
                                        Ok(())
                                    })
                                })
                            },
                        )
                        .unwrap();
                        assert_eq!(String::from_utf8(output).unwrap(), format!("{agent}\n"));
                        assert!(!active());
                    })
                })
                .collect();
            for request in requests {
                request.join().unwrap();
            }
        });
    }
}
