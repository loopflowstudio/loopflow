# 5b3da9 demo — 2026-10-30 (headless, decision pass 1)

Reviewed: PR #1773 at 99a1ef3581 (LOO-675, in-app library + cheap Work readers).
No interactive demo ran here; the decision rests on CI and Task state.

Observed:
- CI run 41055952060 on HEAD failed:
  - e2e-smoke: `planning_reconnect_tests::planning_refresh_reconnects_repository_planning`, which fails with "planning refresh did not catch up after reconnect" at tests/planning_reconnect_tests.rs:73. This is the planning-refresh path the last ci-fix commit changed.
  - rust-test (library 5/5): `task_graph_launch_tests::interrupting_a_task_with_only_an_open_process_records_it_without_claiming_a_stop` panics at tests/linear/task_graph_launch_tests.rs:1802.
- PR mergeStateStatus is DIRTY: origin/main is 6 commits ahead and the PR conflicts with it.
- Task reason: "ci-fix blocked: open landing store: invalid data: incompatible Loopflow database".
- scratch/5b3da9-design.md is no longer in the checkout (cleared by land prep), so this pass couldn't check the done-whens or the installed-build CPU numbers.

Decision (standing in for Jack Heart): iterate. Repair both tests, then resolve the conflict with main. Restore the design doc's done-whens and the installed-build CPU numbers (idle under 2% of one core; the busy budget) from git history (920c3faa54) so the next demo can check them.
