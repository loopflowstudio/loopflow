# LOO-988 verify, 2026-11-19

PR #1990 merged as 6f9763f809 (2026-11-20T01:45:21Z).

- `git diff --quiet HEAD origin/main` at branch HEAD 46583cd625: identical tree, so the PR's recorded runs describe main.
- `gh run list --commit 6f9763f809…`: merge_group CI 44654694143 success; push CI 44655915898 in_progress; "macOS on main" 44655915891 queued. Push run not finished when this was written.
- `grep -rn "fn codex_service_tier" rust/loopflow/tests`: six tests in `context_launch_tests.rs:110-135`; the single 120s test is gone (#1982).
- `journal::with_process` is `#[cfg(not(test))]` on main; `tests/support/mod.rs` sets `LOOPFLOW_TEST_LF` from `CARGO_BIN_EXE_lf`; both New Task tests set `canPrepare = { false }`.

Not run here: a local rerun of `human_session`, `terminal_holder::` and `session_launch_contract` on main was started and stopped while still compiling (load 42). No Swift rerun. Later: push CI runs the suites.
