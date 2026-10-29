# LOO-685: heal a stacked child's recorded base once its parent merged

Jack Heart requested this on 2026-10-29 after LOO-669 (#1763) looped
demo → gate → land 38 times on `UnsafeSyncBase`. Status: implemented, awaiting gate.

## Change

- `task_stack` (rust/loopflow/src/ops/task.rs): once the parent PR merged and the
  child's HEAD contains its `merge_commit`, that commit is the fork base. Sync,
  land and `lf sync` then merge main plainly; `record_stack_sync` records the new
  base and clears the parent as before. A child holding neither the recorded
  base nor the merge commit still hits `validate_fork_base`'s refusal.
- `run_task_graph_for` (lf/commands/graph.rs): when all 3 attempts fail with the
  same failure cause, the run ends with `RepeatedRunFailure` carrying that error.
- `lf ship` (lf/commands/ship.rs): on `RepeatedRunFailure` it withdraws Auto-Ship
  through `ship_halt` (attributed to Loopflow, never kills its own process), so
  `restart_ships` doesn't restart the identical refusal every backoff.

Why it looped: `deliver` runs `land --wait-and-fix` as a plain step, so a refusal
fails the Graph; `lf task run` retried it 3 times from after `demo`, then
`restart_ships` started the run again, indefinitely.

Proposed Wave memory (infrastructure, not written: no memory-curating Graph):
a deterministic refusal must end Auto-Ship, not feed its restart loop.

## Delete — do not maintain

Nothing replaced; `ship_stop` and `ship_halt` share one `withdraw`.

## Checks

Compress (2026-10-29): `sync_tests::{stacked,saved_graph_sync_follows}` (6 ok), `--lib task_graph_launch_tests::task_ship_stops` (1 ok), clippy clean; gate owns the affected suites.

## Concept review (2026-10-29)

No change. The heal adapts the recorded base instead of adding a guard; a
deterministic refusal surfaces in Task history as "Stopped shipping: Loopflow:
<cause>", and `lf ship` resumes it. Jack's Task brief already settles that a
repeated identical refusal stops the run; the outage trade-off stays in
`scratch/questions.md`.
