# Open choices made for review (LOO-1082)

- Quiet `task-session` during running steps: run starts and intermediate step ticks for a Task's own conversation write no update (as conducted children already do). Passed over: waking on run start only, time-based throttling.
- `build.yaml` keeps its nested two-loop topology; only comments and docs teach the outer-review-loop pattern.
- Manual commits apply the branch push default to selected paths too. The independent index still limits the commit to the named files. A clean checkout can push an earlier local checkpoint on the next `lf commit`.
- The decision prompt uses the same name for the return target as the Task's Graph position. A command shows its arguments and an XOR shows its router.

The loop cap (`max_loop_passes`) and the scratch-only commit-message skip were dropped by steers; see `scratch/plan.md`. `scratch/concept-review.html` predates those steers.
