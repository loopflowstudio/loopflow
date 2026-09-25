# Task cursor store contribution

2026-09-25. Owned files: `rust/loopflow/src/durable.rs`,
`rust/loopflow/src/store/sqlite/durable.rs`, and this note only.

## Contract

`FlowPosition.cursor` is the sole execution position. The existing SQL
`step_index` and `iteration` project the root cursor; `review_json` contains
its entire serialized tree. A reader recognizes a cursor by its `index` field.
Absent that field, the existing legacy progress decoder preserves flat
FlowProgress/LoopReview decisions, counters, direction, blocked failure evidence,
and the root index/iteration. Claim and reclaim normalize that old state under
their existing transaction. No schema or second lifecycle authority is added.

Current plan and StepRef project the selected leaf. StepRef retains root index
and iteration for the existing exact human authority. `current_checked` returns
None for an unavailable leaf, allowing store validation to reject invalid state
without a panic. Pending routing and loop decisions both count as mechanical
continuations for the controller.

Verdicts and routes write only active leaf state while saving the entire cursor.
`record_flow_route(task_id, run_id, path)` requires the current bound worker,
a current XOR, and a declared path. Identical writes are idempotent; conflicting,
unknown, unrelated, and stale writes fail without mutation. Failure/release clear
leaf verdict and route in Rust, retaining the selected body, counters, direction,
and parent cursor. Reclaim preserves pending evidence for recovery.

## Proof boundary

The finish line is real SQLite round-trip, save/reclaim, and stale-authority
rejection with a selected nested body. A serialized cursor alone or compile pass
does not meet it. Tests simulate no provider/PM transport: they use the existing
temporary SQLite setup and the real store APIs.

The new nested test exercises leaf router and loop-decide cases, save/reclaim,
idempotent route writes, invalid/conflicting/unrelated/stale route rejection,
wrong-kind route rejection, preservation of root progress, and clearing pending
leaf evidence after failure or release. Existing legacy tests also check a
nonzero root iteration survives old progress decoding.

**9 tests passed, 0 failed** in the integrated repository (68.20 seconds).
The focused command was:

```sh
env -u LF_CONTROL_HOME -u LF_CONTROL_DB_PATH -u LF_HOME -u LF_DB_PATH \
  -u LF_RUN_CONTEXT -u LF_RUN_ID -u LF_RUN_DIR \
  cargo test -p loopflow --lib store::sqlite::durable::durable_store_tests:: -- --nocapture
```

File-scoped rustfmt and diff whitespace checks passed after implementation.
All-target Clippy was attempted with warnings denied:

```sh
env -u LF_CONTROL_HOME -u LF_CONTROL_DB_PATH -u LF_HOME -u LF_DB_PATH \
  -u LF_RUN_CONTEXT -u LF_RUN_ID -u LF_RUN_DIR \
  cargo clippy --all-targets -j4 -- -D warnings
```

It failed on two diagnostics in unowned files: `only_used_in_recursion` for
`catalog_root` at lf/commands/install.rs:419 and `unnecessary_cast` for
`position.cursor.index as usize` at ops/human_session.rs:2012. No Clippy
all-target pass is claimed; neither external file was changed here.
The first focused run waited for the shared build-directory lock, then failed
before tests on external references: E0308 in controller/task/mod.rs passed a
usize cursor index to the u32 test helper; E0609 in store/mod.rs still accessed
settled.step_index. Main subsequently updated those references. A read-only
`lf ps --json` observed the live machine without changing execution ownership.

## Review and integration

Store ownership, worker claim/version fencing, and human approval authority remain
at their existing owners. The JSON cursor is not an extra playhead alongside
SQL: SQL indexes are projections and mismatches are rejected. The reader never
reloads a Flow from source to recover a selected branch.

Main owns shared cursor/traversal methods, the store trait facade, controller/CLI,
and all external initializers. This bounded contribution cannot establish those
adapters, live provider/Ask/Session behavior, or full Task/ordinary parity.
No commit, push, installation, Session launch, delegation, or PM mutation was made.
