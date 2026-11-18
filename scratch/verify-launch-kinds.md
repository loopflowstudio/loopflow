# Verify LOO-924 on main

Outcome: **Verified**, plus one follow-up (Task 87b7c0).

- PR #1966 merged 2026-11-18T05:47Z as `8a7a22a6e9`.
- CI on the merge SHA: merge_group CI success (run 44245480355); push CI success (run 44246686085); push "macOS on main" (run 44246686192) was still pending when checked.
- `cargo test -p loopflow-execution --lib harness::launch_matrix` on detached `origin/main`: 3 passed, including `every_harness_runs_all_four_launch_kinds`.
- `fn run_interactive`, `run_native_agent`, `run_native_session`: gone from main, so one interactive path remains.
- Literal harness-name comparisons outside test files: 92 at `470da475a1`, 22 on main. Most of the 22 sit in `#[cfg(test)]` modules or parsers (`HarnessKind`, storage `Provider`), or check binary names (`pi.rs`). One real leftover: `agent/session.rs:98` `agent == "claude"`. Follow-up Task 87b7c0 covers it.
- The `attachment: Option<…>` field on `AgentConfig` is gone. `fact_writer.rs` still has its own optional attachment.
