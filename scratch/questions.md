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

- `cargo check -p loopflow --lib --tests`: clean, no warnings. `cargo fmt`: clean.
- Focused lib tests (`store::sqlite::{task_ownership,rentals,linear_observation,planning_peers,task_comments}`):
  binary built (15 min at load ~110), 22 tests selected, none finished in 5 min;
  process sat in state `T`. Stopped by hand; gate runs them, plus clippy.
- After `HeldTask` (6bd724d6ee): `cargo check -p loopflow --lib --tests` clean;
  `cargo test --lib -- store::sqlite::rentals store::sqlite::task_ownership`
  ran 3 of 16 (all ok) before the 30-minute limit stopped it. Gate runs the rest.
