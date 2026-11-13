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
- `rentals::tasks_held_in` had one labelled caller left (`tasks_held_by`, which
  needs only `since`). It is now inlined there as `held_in` + `ownership_in`.

## Checks

- `cargo check -p loopflow --lib --tests`: clean, no warnings. `cargo fmt`: clean.
- Focused lib tests (`store::sqlite::{task_ownership,rentals,linear_observation,planning_peers,task_comments}`):
  binary built (15 min at load ~110), 22 tests selected, none finished in 5 min;
  process sat in state `T`. Stopped by hand; gate runs them, plus clippy.
