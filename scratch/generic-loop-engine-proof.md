# Ordinary Flow engine contribution

2026-09-25. Scope: `rust/loopflow/src/engine/execution.rs` and this note only.
Implements the ordinary engine portion of `loopflow-integration-contract.md`.
No commits, publication, installation, provider launch, Ask, store mutation, or
Task authority changes were performed.

## Runtime contract

- The ordinary engine accepts backward edges and delegates their meaning to
  `transitions::finish_step`. It has no Task-only repeat rejection.
- `ExecutionCursor` holds shared transition progress and a backward-traversal
  iteration count. Missing progress/iteration in older serialized cursors means
  their initial state. Counts are independent by authored edge and survive
  forward transitions, as defined by the shared reducer.
- `ExecutionContext` retains display progress and carries direction. Implicit
  forward movement retains direction through the entire body, including Ops
  and newly entered XOR paths; explicit Advance clears it in the shared reducer.
- `SkillOutcome::Decided(FlowVerdict)` records Advance/Iterate. Blocked is a
  separate skill/Flow outcome. Missing/empty decisions and exhausted budgets
  stop before later steps. Waiting retains the exact current boundary.
- `SkillExecutor::checkpoint(&ExecutionCursor)` has a default no-op. An adapter
  that needs durability must implement it. The engine awaits it with the **root**
  cursor after every successful tick, including route selection, nested skills,
  Ops, waits, blockers, and decision settlement. Failure stops execution before
  another boundary starts. The engine adds no persistence implementation.
- A tick runs at most one provider/operation boundary. Decided saves its verdict
  in one tick; a subsequent tick consumes it without another provider call.
  A restored pending verdict takes the same settlement path. Nested ticks unwind
  before checkpointing, preserving the complete parent/child state.
- `NestedCursor::Xor` includes `steps: Vec<ConcreteStep>` for the selected body.
  Resume, current-skill lookup, and advancing an authorized wait use those saved
  steps rather than reopening the source Flow. Route selection checkpoints before
  executing its first child. Empty selected paths still checkpoint their route.
- Human authorization remains the caller's responsibility. The wait-advancement
  helper uses the shared reducer; it cannot silently bypass a required verdict.

## Focused proof

**12 tests passed** against the real `execution.rs`, `transitions.rs`, `flow.rs`,
`error.rs`, and `builtins.rs` modules in a temporary Cargo harness. The harness
uses path imports, the repository's generated builtin catalogs, and ordinary
Rust dependencies; it substitutes no transition/parser/loader implementation.
Only executor side effects are simulated by the existing RecordingExecutor.

Coverage:

- Two independent loops run their initial/final sections once, retain separate
  budgets, and carry direction across all body skills.
- Overlapping loops cannot replenish an exhausted edge; the pending verdict,
  cursor, direction, and counters remain available at the stop.
- Missing/empty/explicitly blocked decisions and a one-pass limit never run
  final steps.
- A failure after saving a decision resumes from serialized state, consumes the
  decision once, repeats once, and does not rerun the deciding provider. Running
  the completed cursor again performs no work.
- Two nested XOR levels checkpoint the root after each boundary. After saving
  the innermost decision, deleting both source flows does not prevent recovery;
  neither router reruns and the outer suffix executes once.
- Direction reaches Ops, XOR routing, and every selected child before returning
  to the parent deciding occurrence.
- An empty route is saved before the suffix; nested blockers retain the selected
  route and prevent the suffix.
- Existing selected-XOR and Op behavior remain covered. Waiting inside XOR
  survives serialization and source deletion, and authorized continuation resumes
  at the next saved skill.

Harness commands (local temporary path, not a repository artifact):

```sh
env -u LF_CONTROL_HOME -u LF_CONTROL_DB_PATH -u LF_HOME -u LF_DB_PATH \
  -u LF_RUN_CONTEXT -u LF_RUN_ID -u LF_RUN_DIR \
  cargo test --offline \
  --manifest-path /var/folders/m6/r3tllnrs1yq7yfbwm680tss40000gn/T/loopflow-engine-proof-f2smw1ub/Cargo.toml \
  engine::execution::tests -- --nocapture

env -u LF_CONTROL_HOME -u LF_CONTROL_DB_PATH -u LF_HOME -u LF_DB_PATH \
  -u LF_RUN_CONTEXT -u LF_RUN_ID -u LF_RUN_DIR \
  cargo clippy --offline \
  --manifest-path /var/folders/m6/r3tllnrs1yq7yfbwm680tss40000gn/T/loopflow-engine-proof-f2smw1ub/Cargo.toml \
  --all-targets -- -D warnings
```

Harness Clippy passed with warnings denied. Its first run identified an explicit
MutexGuard drop that Clippy still treated as crossing await; the test now clones
its saved snapshots before awaiting. Final harness tests passed after that fix.
`rustfmt --check --edition 2021 rust/loopflow/src/engine/execution.rs` and scoped
`git diff --check` passed.

## Repository build counterexamples

Attempted the requested repository command three times as external edits changed:

```sh
env -u LF_CONTROL_HOME -u LF_CONTROL_DB_PATH -u LF_HOME -u LF_DB_PATH \
  -u LF_RUN_CONTEXT -u LF_RUN_ID -u LF_RUN_DIR \
  cargo test -p loopflow --lib engine::execution::tests -- --nocapture
```

No repository test executed in those attempts. Initial failures were outside the
owned file: the CLI executor had not yet supplied context direction or matched
Blocked; concurrent Ask edits referenced unfinished test state and record fields.
Those changed during this contribution. The last attempt failed with E0277 at
`lf/mod.rs:15`: `Cli` derives Clone but `Commands` did not implement Clone. No
external compile error was repaired in this contribution. The harness pass is
module-level proof, not a passing repository build or CLI integration proof.

## Review and remaining integration

Review preserved one transition owner and one root checkpoint surface. Nested
execution no longer hides multiple unpersisted boundaries inside a single engine
call. Saving a verdict before consuming it makes the recovery test distinguish
actual saved-result recovery from merely rerunning a successful provider.

The adapter must persist invocation identity, exact decision authority, blockers,
human approval, and active execution evidence. It must reject stale decisions
and implement the checkpoint hook; the engine's default no-op does not establish
any of these guarantees. An operation's effect-before-checkpoint crash window
still needs its own receipt/idempotence contract.

This contribution pins a **selected body's steps for that cursor visit**. It does
not precompile every unselected XOR descendant or router definition, fence the
existing route-verdict file, or preserve a discarded child body when a later
outer pass selects the XOR again. Whole-invocation definition pinning and exact
routing decisions remain integration obligations. Older nested cursors without
saved `steps` require explicit migration/disposition rather than silently
reloading mutable source.

Remaining acceptance: run the focused command in the integrated repository;
prove real CLI checkpoint/decision recording, Task/ordinary parity, exact human
handoff, and Blocked/Ask recovery. No live-provider or deployment claim follows
from the tests here.
