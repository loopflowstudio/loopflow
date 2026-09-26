# Shared transition contribution

Scope: `rust/loopflow/src/engine/transitions.rs` only, plus this requested note.
No exports, adapters, persistence, flows, or other documentation changed by this
contribution. Other integration edits are concurrent work.

## Decisions

- `FlowProgress.repeats` keys the deciding occurrence's stable ID. Counts record
  backward traversals, not visits or worker retries. Forward transitions retain
  all counters, including when another edge revisits a completed section.
- A limit of 3 permits the initial pass and two backward traversals. A limit of
  1 permits only forward completion. Exhaustion preserves the pending verdict;
  an explicit Next can still finish at the limit. Comparison before increment
  avoids overflow even for a persisted `u32::MAX` counter.
- A repeat occurrence requires a verdict with non-whitespace evidence. Ordinary
  steps default to Next. Any supplied verdict must have evidence; Blocked
  returns its exact summary. Repeat without a declared edge is an error.
- Next clears direction and the pending verdict, as the contract specifies.
  Repeat carries the summary verbatim as direction and consumes the verdict.
  Every Blocked outcome and error leaves the entire progress value unchanged.
- The reducer validates its cursor and the current edge's nonempty ID/target,
  positive budget, and earlier target. Definition expansion retains ownership of
  global occurrence uniqueness. The reducer imposes no restrictions on human
  or operation steps inside a repeated section or on overlapping edges.
- Human approval, stale-result rejection, atomic settlement, and xor child
  cursor ownership belong to the adapters. This function supplies no authority.

## Proof

Eight unit tests cover ordinary forward/terminal transitions, repeated visits
and direction consumption, independent counters, overlapping-edge exhaustion,
missing/empty/explicitly blocked evidence without mutation, limit boundaries
including overflow, invalid cursors/edges without mutation, and serialization
of a pending decision followed by the same pure transition after recovery.

Formatting: `rustfmt --check --edition 2021
rust/loopflow/src/engine/transitions.rs` passed.

The module was initially unexported. Main added its export while this
contribution was in progress. Targeted verification passed: **8 passed, 0
failed**, with 1574 tests filtered out, using:

```sh
env -u LF_CONTROL_HOME -u LF_CONTROL_DB_PATH -u LF_HOME -u LF_DB_PATH \
  cargo test -p loopflow --lib engine::transitions::tests -- --nocapture
```

## Integration proof still needed

Run one saved definition with two backward edges through both ordinary Flow and
Task adapters and compare transitions. Persist a decision, interrupt before
settlement, and prove recovery consumes it exactly once without a new model Run.
Reject stale worker and human decisions at the transactional boundary. Exercise
human and operation steps inside repeated sections, xor-local edge resolution,
and legacy progress translation. The pure serialization test does not establish
those runtime guarantees.

Review: the state has one cursor owner outside this module and one counter per
authored edge here; no runtime or I/O dependency was introduced. Counters survive
forward progress to prevent overlapping edges from replenishing each other's
budgets. No persistence or delivery claims follow from these tests.
