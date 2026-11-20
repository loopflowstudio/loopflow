# LOO-988 verify, 2026-11-19

PR #1990 merged as 6f9763f809 (2026-11-20T01:45:21Z). Outcome: verified for Rust; Swift pending in CI.

- `git diff --stat HEAD origin/main -- . ':!scratch'`: empty. The branch tree is main's, so the PR's recorded runs describe main.
- `gh run list --commit 6f9763f809d100ff5afb37d17a5a22399898acf0`:
  - merge_group CI 44654694143: success. All eight `rust-test` shards (library 0-4 of 5, integration 1-3 of 3), rust-lint, python-test, e2e-smoke, task-installation green. `macos` skipped there.
  - push CI 44655915898: success. `merge-proof` passed and `rust-test` was skipped on it, so the push run adds no second Rust test run; rust-build, rust-lint, e2e-smoke, task-installation ran and passed.
  - "macOS on main" 44655915891: in_progress when this was written (`swift-test`, `loopflow-ui-test`). The two Swift fixes (`WorkDestinationTests`, `taskLifecycleUpdatesOpenSidebar`) have no post-merge result yet.
  - "Packages (main)" 44656066861: queued.
- `grep -rn "fn codex_service_tier" rust/loopflow/tests`: six tests in `context_launch_tests.rs:110-135`; the single 120s test is gone (#1982).
- `journal::with_process` is `#[cfg(not(test))]` on main; `tests/support/mod.rs` sets `LOOPFLOW_TEST_LF` from `CARGO_BIN_EXE_lf`; both New Task tests set `canPrepare = { false }`.

Not run here: `cargo test -p loopflow-execution --lib -- terminal_holder:: human_session` was started twice on this tree and stopped both times while still compiling (load 42, then 72). No local Swift rerun. Later: "macOS on main" 44655915891 runs the Swift suites; `gh run view 44655915891`.
