# LOO-867 open choices

- Peer planning import reads `planning_client::connected(repo)` once for the
  imported repository and passes it to every reasserted Task; touched Tasks are
  assumed to belong to that repository.
- `hand_back_in` takes planning connections read before `BEGIN` for the Tasks
  the ended Machine held; a Task claimed there between that read and `BEGIN`
  loads its connection inside the write (rare race, correct result).
- `apply_linear_observation` now loads the Task's planning connection before
  every observation (~10 ms, outside the lock), not only when a reassert writes.

## Checks

- `cargo check -p loopflow --lib --tests` after compress: passes, no warnings. `cargo fmt`: clean.
- Focused lib tests (`store::sqlite::task_ownership::`, `rentals`,
  `linear_observation`, `planning_peers`, `scope_defaults`): 3 of 20 passed,
  then the process sat in state `T` (stopped) at load 40–115; tests in other
  worktrees were stopped the same way. Gate runs them, plus clippy.
