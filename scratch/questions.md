# W2-237 open questions

## Dogfood criterion (human run required)

The done-when mapping includes: "Dogfood W2-151 through review and merge, verify
final PM completion happens exactly once." This requires a live Linear
connection and a human review approval — not provable in a headless run. The
code path is the one this PR hardens (the integration test in `pr_tests.rs`
reproduces the W2-151 ordering and proves Linear cannot close early), but the
live dogfood is a human run after merge.

## Rebase conflict resolution

Two upstream changes touched the same code as the completion gate:

1. **Directive-incorporation gate** (upstream `has_pending_directive` check in
   `reconcile_task_pr_with_authority`) — composed with the review gate: the
   `completes` check includes `&& !has_pending_directive(session)`, and
   `task_completion_gate` also checks the directive so `advance_completion_after_gate`
   doesn't bypass it. Without the gate-level directive check, the reconcile
   advance would complete a Task with a pending directive (caught by the
   `observed_merge_waits_for_an_unincorporated_directive_before_completion` test).

2. **Ambient-Wave classification refactor** (upstream moved `WaveId` out of
   `ops/task.rs` top-level imports to `ops::util`) — resolved by dropping the
   top-level `WaveId` import (the test module re-imports it locally).
