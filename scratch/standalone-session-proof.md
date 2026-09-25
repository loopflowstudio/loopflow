# Standalone Flow Session adapter

2026-09-25. Bounded contribution for LOO-295. Owned files:
`ops/flow_session.rs`, `lf/commands/session.rs`, this note, and only
`Boundary.ready_summary: Option<String>` plus its initializer in
`ops/flow_run.rs`. Other runtime files were inspected, not edited here.

## Implemented contract

The saved `FlowRun` owns the human boundary. `SessionKind::Flow` is its user
surface; no Task, Session lifecycle record, scheduler, or database table was
added. IDs are `flow:<invocation UUID>:<boundary UUID>`. Every operation validates
the saved invocation and current human boundary, including the pinned leaf
skill inside selected nested XOR cursors.

The adapter exposes `list`, `surface`, `open`, `mark_ready`, `decide`,
`settle_saved`, `pinned_skill`, `worktree`, and exact Session ID parsing/formatting.
Listing uses a single saved record snapshot per projection, avoiding a stale
second read if approval arrives while enumerating. Launch preserves cwd, model,
selectors, saved message and direction. Skill content comes from the saved
cursor, never a fresh source lookup. The prompt supplies exact Session
Advance/Iterate commands and explicitly separates readiness and Ask completion
from Flow approval.

Opening uses the existing Session launch lock, `spawn_session_run` publication
checks, native resume, owned-client stop and manifest machinery. Concurrent
startup rereads under the existing lock. A published native Run is retained if
its history is unavailable; no replacement provider is silently launched.
Provider exit leaves approval pending. Readiness requires the exact bound Run
and updates only the ready summary.

A decision saves the typed `FlowVerdict` and completes the exact boundary under
`flow_run::update` before attempting provider stop. Duplicate, stale and invalid
navigation requests fail without changing the record. Invalid Iterate cannot
hide a review behind an unusable saved decision. A stop failure appears in the
returned handoff reason; approval remains saved. Handoff names the invocation
and its explicit `lf flow resume <id>` continuation. This adapter does not
launch a successor from the review provider process.

`settle_saved` consumes an already authorized human verdict once, under the
same record owner, before the ordinary driver resumes. Advance and authored
repeat gates use `finish_step`. An unannotated human Iterate revisits the
preceding autonomous skill, or that skill's repeat target: delivery after
loop-decide revisits implementation. Human revision carries direction and
increments visit identity without spending an autonomous repeat allowance.
Nested settlement updates only the leaf; the engine still owns returning to
the outer suffix.

## Integration hooks for main

These changes were requested and are now visible in main's concurrent source,
but were not made by this contribution:

- Export `pub(crate) mod flow_session` from `ops/mod.rs`.
- Expose existing `human_session::{spawn_session_run, lock_session_launch,
  resume_native_run, native_session_state, human_open_argv, HumanSessionToken}`
  as `pub(crate)` without creating a second launch implementation.
- Add the internal token variant
  `StandaloneFlow { token: crate::ops::flow_run::StepToken }`.
  This does not add a public Session kind.
- Central Session list includes `flow_session::list`; boundary Run IDs are
  excluded from Interactive projections. Open delegates parsed standalone IDs
  to `flow_session::open`; Complete rejects those IDs. Readiness delegates the
  new token to `mark_ready`; pinned skill lookup delegates to `pinned_skill`;
  decision cwd delegates to `worktree`.
- The CLI contribution itself routes standalone Session Advance/Iterate to
  `decide`, preserving the Task return type and Ask Complete semantics.
- The executor begins a human boundary and returns Waiting. Before recovery,
  the driver calls `flow_session::settle_saved(invocation)`.
- `flow_run::recover` skips human boundaries. A provider completion receipt
  must never become human approval or erase a saved human verdict.

Review found a second concrete integration race: approval may arrive after
begin_boundary but before the Waiting checkpoint. The old checkpoint copied
its older cursor over the saved verdict. Main's current checkpoint now preserves
an accepted decision when the position is unchanged and rejects conflicts.
This is source evidence; the focused proof here does not inject that driver
race.

## Proof

The focused repository test passed on the final owned Rust content: **1 passed,
0 failed**, with 1599 filtered out; execution took 0.12 seconds after compilation.
The first compile caught two test-only `Skill.content` Option mismatches, both
fixed before passing. No external compile errors were changed here.

```sh
env -u LF_CONTROL_HOME -u LF_CONTROL_DB_PATH -u LF_HOME -u LF_DB_PATH \
  -u LF_RUN_CONTEXT -u LF_RUN_ID -u LF_RUN_DIR -u LF_FLOW_STEP \
  -u LF_HUMAN_SESSION cargo test -p loopflow --lib \
  ops::flow_session::tests::saved_nested_human_review_requires_exact_approval_and_recovers_once \
  -- --exact --nocapture
```

One behavioral test uses real serialized Flow records in a temporary Home,
restoring its environment afterwards. It proves:

- A nested saved skill remains available with no installed source Flow.
- Exact IDs and review commands, one Flow Session projection, and Waiting/Ready
  states. No native provider is spawned.
- Wrong-Run readiness is rejected; correct readiness leaves the cursor intact
  and cannot settle the boundary.
- Persisted Iterate survives reread; duplicate approval and late readiness leave
  the whole serialized record unchanged; settlement happens once.
- Delivery Iterate returns to implementation with direction while retaining the
  outer cursor; later visits receive new boundary UUIDs and reject old approval.
- Advance consumes the leaf verdict/direction and leaves the outer suffix for
  the engine. Invalid Iterate at an initial human node leaves it visible, and
  a later Advance returns the expected continuation handoff.

File-scoped rustfmt checks passed for both owned Rust files. No all-target
Clippy or full-suite pass is claimed. A read-only `lf ps --json` observed machine
health while the shared build lock delayed compilation; no processes were
signaled or changed.

## Remaining integration limits

The Task controller still has its own nearest-autonomous-skill Iterate helper
in the source last inspected. Main must share the revision-target rule with
Task human decisions to prove delivery parity. This adapter cannot edit that
controller, the shared reducer, or the engine.

The proof does not exercise a live native provider, terminal/desktop handoff,
owned-client stopping, process-publication crashes, competing open commands,
or real provider interruption. Open uses the established machinery rather than
a test factory. It retains that machinery's pre-publication crash window.
The test checks persisted approval and settlement, not an OS process crash.
No real Session, provider, worker, installation, commit, push, PM mutation, or
production migration was performed. Full entry-point and Task/standalone parity
remain main's integration responsibility.

## Main integration follow-through

Main wired the listed hooks, pinned unbound review launches, and duplicate
Interactive suppression. Human recovery skips provider receipts. The checkpoint
race now has a regression in flow_run.rs. The fixture takes the shared environment
lock. Design/demo builtins now declare their revision edges; gates predating
explicit edges share Task's preceding-autonomous-step fallback. The duplicate
human navigation enum was removed. Session decisions request continuation through
the existing Home launcher before stopping the review provider; launch failure
retains approval with an explicit recovery command. These additions require the
final integrated proof, separately from this contribution's initial pass.
