# LOO-1098: wake Antigravity on reopen and enforce harness parity through the interface and tests

History and 5 Whys: [5whys.md](5whys.md). Concept review: [concept-review.html](concept-review.html).

## Direction & findings

### Live Antigravity probes (2026-11-24)

- **Held-terminal paste-mode wake stopped (moved to LOO-1099):** A disposable
  PTY running `agy 1.5.16` emitted `ESC[?2004h` at the workspace trust dialog.
  With a trusted workspace, the slash menu, the model picker opened with Enter,
  and returning to the prompt produced no paste-mode transition. A restarted
  server can also adopt a terminal holding an unsubmitted draft with no
  keystroke history. Live idle wake for an already-open `agy` belongs to
  LOO-1099; any replacement there must pass those live checks and preserve
  drafts across server replacement.
- **Reopen with waiting steers works live (built & QA'd at `c0b28b7dc6`):**
  `agy --conversation <id> -i <prompt>` records a second `USER_INPUT` on the
  original transcript, runs `PreInvocation` before anyone types, and reports
  `Stop` at turn end. Verified on real `agy 1.5.16` (private Machine): quit the
  Task conversation, save a repository steer, run `lf resume --task <id>` →
  original transcript recorded the turn and exact answer `VIOLET`, `lf session
  steers` showed one receipt (`delivered`) and zero pending, and reopening with
  nothing waiting passed no `-i` and started no turn.

### Post-build concept review & parity direction (2026-11-24)

Jack Heart's direction on the parity contract built in pass 1:
- `"i would rather make it harder to add a harnes if it makes it harder to miss imortant behavior. when i add a harness, i want it to have all the good stuff"`
- `"1. Just remove the table. Instead, just make sure that all behaviors are asserted to work and testeed  across all harnesses."`
- `"and that the Harness interface demands full functionality"`

The second pass removed `harness/parity.rs` and the generated README table.
Each harness declares its caller environment, headless driver recognition,
sign-in advice, idle endpoint, empty-thread replay and reopen delivery. Storage
returns candidate endpoints. Both the hook and the server ask the launcher
which endpoint belongs to the current provider.

The worker, launch and conformance fixtures now assert their outcomes directly.
The reopen checks partition `HarnessKind::ALL` by how the steer arrives:
the server test receives the provider turn on its endpoint, and the native
fixture runs `lf resume --task` and checks the hook text and receipt.
The missing-CLI fixture checks the same four system directories its sandbox hides.

## Checks in this checkout

- The candidate query on 22,000 synthetic sessions, 200 unfinished, searched
  `agent_sessions_unended_agent` and both join indexes. A warm in-memory read
  took 3.63 ms for 200 rows, 0.035 ms for one session (three batches of 100).
  The storage query-plan test already traces both forms against Machine statistics.
- `cargo build -p loopflow --bin lf` passed. The macOS linker warned about
  compact unwind offsets in the debug binary.
- Focused nextest run 64d46d07-1a72-4fbb-9d7f-6c9b1bc5d642 passed all seven
  checks: both wake paths, the boundary with a stale endpoint, the
  query plan, the native Antigravity reopen, the four-kind launch matrix
  and the missing-CLI fallback.
- `cargo fmt --check` and `cargo clippy --all-targets -- -D warnings` passed.

## Acceptance at gate

```sh
cargo test -p loopflow --test integration documented_commands::
cargo test -p loopflow-storage --lib query_plans::
cargo test -p loopflow-execution --lib harness::conformance_tests::
cargo test -p loopflow-execution --lib harness::launch_matrix::every_harness_runs_all_four_launch_kinds
cargo test -p loopflow-execution --lib harness::environment_tests::
cargo test -p loopflow --lib server::wake::tests::
cargo test -p loopflow --lib server::boundary::tests::
cargo test -p loopflow --lib task_graph_launch_tests::
cargo test -p loopflow --test session_launch_contract a_reopened_antigravity_task_hears_repository_direction_once
```

Already-open Antigravity idle wake belongs to LOO-1099. Viewer handoff belongs
to LOO-1097. Headless unanswered questions on Claude/Codex/Antigravity belong
to Task 48edb9.
