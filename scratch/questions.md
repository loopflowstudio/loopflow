# LOO-867 open choices

- Peer planning import reads `planning_client::connected(repo)` once for the
  imported repository and passes it to every reasserted Task; touched Tasks are
  assumed to belong to that repository.
- `hand_back_in` takes planning connections read before `BEGIN` for the Tasks
  the ended Machine held; a Task claimed there between that read and `BEGIN`
  loads its connection inside the write (rare race, correct result).
- `apply_linear_observation` takes `planning_connected` from its callers. Both
  callers return early unless it is true, so production always passes `true`;
  the tests pass `false`, which is what the old in-store read returned for
  their unconfigured repository.
- `ingest_marker_in` has a third caller the concept review did not list:
  `task_comments::ingest_task_comment`, inside `apply_linear_observation`'s
  write. It takes the same Machine-name map, read before `BEGIN`.
- `rentals::tasks_held_in` is gone. `task_ownership::held_in` returns
  `HeldTask { task, owner, since }` from the store alone, so `tasks_held_by`
  no longer loads Machine declarations once per held Task.
- Outside Task ownership, two rental writes still load placement config after
  `BEGIN`: `set_machine_ownership` (`declared_pools`) and `admit_rental`
  (`ondemand_in`), both in `rentals.rs`. Left alone here: they are Machine
  pool/on-demand writes, not Task ownership. Moving those reads before `BEGIN`
  would be a separate Task.

## Checks

- `cargo check -p loopflow --lib --tests` and `cargo fmt`: clean.
- QA (2026-11-12, before the merge of 4b84a0dc9e): focused lib tests
  (`store::sqlite::{task_ownership,rentals,linear_observation,planning_peers,task_comments}`),
  22 of 22 passed (627 s, load ~155).

## Concept review (2026-11-12)

The change has a clear concept: `OwnershipFacts` is everything an ownership
write needs from outside the store (Git identity, planning connection,
declared Machines, the scope's owning Machine). It is read before `BEGIN` and
passed in as values. Read-only paths (`ownership_in`, `held_in`) load
nothing inside a write. No naming or model change is called for, and no
product choice is open. The remaining in-write config load is the one already
listed: `hand_back_in`'s fallback for a Task claimed after the pre-read.

## Gate (2026-11-12/13, tree cb38c1949b)

- `scripts/test.py --reuse-passing`: stopped at resource preflight (`~/.cache/uv`
  inventory deadline, 42.7 GiB free). Plan run by hand instead:
- `check_architecture.py`: pass. `check_rust_test_inventory.py`: pass. `cargo fmt --check`: pass.
- `cargo build -p loopflow --bin lf`: pass (16 m 36 s, load ~70).
- nextest `test(store::sqlite::{task_ownership,rentals,children::linear_observation_tests,planning_peers,task_comments}::) | test(ops::linear_observe::) | test(store::query_plans::) | test(documented_commands::) | binary(two_people)`:
  not run. Compiling the test binaries outlasted the 2 h limit (load 140–190,
  other checkouts compiling). No test started; CI's `rust-test` owns the result.
- `cargo clippy --all-targets -- -D warnings`: not run (queued behind nextest); CI `rust-lint` owns it.
- PR #1909 CI: every proof job skipped (still so at head 3ec9fe627a). `ci.yml` runs them only once `scratch/` is clear, which happens at `lf pr submit`. Until then, nothing has checked the test binaries or clippy after the merges of 4b84a0dc9e and 87f0e3c60a (current origin/main).
- Implement, compress and realign passes (2026-11-13): no code change, no local rerun; code still matches every claim above.
