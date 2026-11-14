# Checks

- compress, 2026-11-13: `cargo nextest run -p loopflow -E 'test(store::sqlite::steers) | test(harness::task_updates) | test(server::boundary) | test(server::wake) | test(store::query_plans)'` 49 pass; `cargo clippy --all-targets -- -D warnings` clean. Full suites: CI.
