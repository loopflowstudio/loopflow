# Open choices made for review (LOO-1082)

- Quiet `task-session` during running steps: run starts and intermediate step ticks for a Task's own conversation write no update (as conducted children already do). Taking up a Graph at `start` is part of launch and also stays quiet. Passed over: waking on run start only, time-based throttling.
- `build.yaml` keeps its nested two-loop topology; only comments and docs teach the outer-review-loop pattern.
- Manual commits apply the branch push default to selected paths too. The independent index still limits the commit to the named files. A clean checkout can push an earlier local checkpoint on the next `lf commit`.
- The decision prompt uses the same name for the return target as the Task's Graph position. A command shows its arguments and an XOR shows its router.

The commit workflow keeps release pushes unconditional and checks the branch for manual and step checkpoints. Task updates keep the existing saved reading format and attach the run error when delivering a change.

The loop cap (`max_loop_passes`) and the scratch-only commit-message skip were dropped by steers; see `scratch/plan.md`. `scratch/concept-review.html` records the post-build review at `2c05f47fc4`.
- Truthful `Blocked` status in `ops::task_execution::project_execution()` for `GraphOutcome::Blocked` and post-step `graph.run` failures (such as exhausted output validation), keeping `GraphOutcome::Waiting` and unrecorded-exit kills `Idle`. Passed over: leaving `project_execution()` as `Idle` when the last step exited `0`.
- A blocked decision and a waiting handoff both exit `GraphHeld::EXIT` (3); Task status tells them apart by the recorded error's `GraphHeld::BLOCKED_PREFIX`. Passed over: a separate exit code for blocked, which would touch every caller matching exit 3.
