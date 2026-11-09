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
    isolate(|| {
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

/// Give a client operation its own settings and caller scope, including reads.
pub(crate) fn isolate<T>(run: impl FnOnce() -> T) -> T {
    let _settings = crate::config::settings::replace_invocation(serde_yaml_ng::Value::Null);
    scope(true, DIAGNOSTICS.with(|sink| sink.borrow().clone()), run)
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
    let settings = crate::config::settings::invocation_values();
    let span = tracing::Span::current();
    tokio::task::spawn_blocking(crate::git::inherit_reads(move || {
        let _entered = span.enter();
        scope(active, diagnostics, || {
            let _settings = crate::config::settings::replace_invocation(settings);
            run()
        })
    }))
}

#[cfg(test)]
mod tests {
    use super::{active, diagnostic, diagnostics, run, spawn_blocking};
    use crate::id::TraceId;

    #[test]
    fn subscription_git_retention_follows_nested_workers_without_leaking_into_requests() {
        let repo = loopflow_test_support::TestRepo::new();
        let runtime = tokio::runtime::Runtime::new().unwrap();
        let status = || {
            let directory = repo.path().to_owned();
            runtime.block_on(async {
                spawn_blocking(move || {
                    crate::git::git_stdout(&directory, &["status", "--porcelain"]).unwrap()
                })
                .await
                .unwrap()
            })
        };
        crate::git::retain_reads(true, || assert_eq!(status(), ""));
        std::fs::write(repo.path().join("new.txt"), "text").unwrap();
        crate::git::retain_reads(true, || assert_eq!(status(), ""));
        let seen = run(&TraceId::default(), &serde_json::json!({}), || Ok(status())).unwrap();
        assert!(seen.contains("new.txt"), "{seen:?}");
    }

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
    fn request_settings_ignore_the_launcher_and_restore_after_failure() {
        let _env = loopflow_test_support::env_lock().lock().unwrap();
        let _agent = crate::lf::commands::graph::EnvVarGuard::set(
            crate::config::AGENT_OVERRIDE_ENV,
            "claude",
        );
        let _profile = crate::lf::commands::graph::EnvVarGuard::set(
            crate::config::remote::PROFILE_ENV,
            "invalid launcher profile",
        );
        let _settings = crate::config::settings::select_invocation(
            serde_yaml_ng::from_str("agent: codex\nyolo: true").unwrap(),
        );
        let result: anyhow::Result<()> = run(&TraceId::default(), &serde_json::json!({}), || {
            assert!(crate::config::agent_override().is_none());
            assert!(crate::config::remote::incoming()?.is_none());
            anyhow::bail!("failed request")
        });
        assert!(result.is_err());
        assert_eq!(crate::config::agent_override().as_deref(), Some("codex"));
        assert_eq!(
            crate::config::settings::invocation_values()["yolo"],
            serde_yaml_ng::Value::Bool(true)
        );
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
                                let _settings = crate::config::settings::select_invocation(
                                    serde_yaml_ng::from_str(&format!(
                                        "agent: {agent}\nyolo: false"
                                    ))
                                    .unwrap(),
                                );
                                diagnostics(&mut output, || {
                                    barrier.wait();
                                    assert_eq!(
                                        crate::config::agent_override().as_deref(),
                                        Some(agent)
                                    );
                                    tokio::runtime::Runtime::new().unwrap().block_on(async {
                                        spawn_blocking(|| {
                                            let agent = crate::config::agent_override().unwrap();
                                            diagnostic(format_args!("{agent}"));
                                        })
                                        .await
                                        .unwrap();
                                    });
                                    Ok(())
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
