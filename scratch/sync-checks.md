2026-11-09: `cargo test -p loopflow --lib -- server::service::tests::typed_events server::subscriptions::tests::events_survive` (LF_* unset, stdin closed): 2 passed on merge 4c4c3a12b. Linker warned that __eh_frame exceeds 16 MB; gate/CI own static analysis and affected suites.
`cargo test -p loopflow --lib events`: stopped because the broad filter selected 46 tests, including unrelated journal/process tests; replaced by the focused command above.
`cargo fmt` and `git diff --check`: passed.
2026-11-09: `cargo nextest run -p loopflow --lib -E 'test(server::) | test(lf::commands::subscribe::)'` (LF_* unset, stdin closed): 64 passed, including one nextest leak warning; before the final checkpoint-ordering repair.
2026-11-09: `cargo nextest run -p loopflow --lib -E 'test(server::subscriptions::)'` (LF_* unset, stdin closed): 14 passed after the checkpoint-ordering repair, including its deterministic regression.
2026-11-09: `cargo fmt`, `git diff --check`, and `cargo clippy --all-targets -- -D warnings`: passed after the final repair.
