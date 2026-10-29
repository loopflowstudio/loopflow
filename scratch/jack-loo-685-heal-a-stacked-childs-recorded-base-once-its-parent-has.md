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

## Demo (2026-10-29, headless, standing in for Jack Heart)

Reviewed through real git fixtures, not the installed lf: a child stacked on a
parent that squash-merged, with later main commits, syncs onto main and keeps
only `child.txt` in its diff (`stacked_child_rebased_onto_main_syncs_from_its_parents_merge`);
a child holding neither base still refuses (`stacked_sync_refuses_when_base_is_not_an_ancestor`).
`lf ship` withdraws Auto-Ship after three identical failures
(`task_ship_stops_shipping_when_its_run_fails_the_same_way_every_attempt`).
Result: 5/5 `sync_tests::stacked*` ok, 1/1 ship-stop ok. No human feedback; the
LOO-669 shape is reproduced and healed. Not proven: installed acceptance on a
live stacked Task — belongs to verify after land. Next: gate.

## Gate (2026-10-29)

Review: no code changes; `docs/orchestration.md` now says a ship whose run fails
identically every attempt withdraws Auto-Ship and `lf ship` resumes it.
`scripts/test.py --reuse-passing --rust-filter 'test(sync_tests::) | test(task_graph_launch_tests::) | test(ops::task::) | binary(task_pr_range_tests) | binary(task_pr_authority_tests) | …'`: FAIL at resource preflight (9.4 GiB free < 32 GiB reserve; `resource_envelope.py --recover` freed nothing, all large builds belong to other Tasks or are recent) — Rust tests, clippy and the website suite (docs edit) deferred to CI.
`cargo fmt --check`, `check_architecture.py`, `check_rust_test_inventory.py`: pass.
