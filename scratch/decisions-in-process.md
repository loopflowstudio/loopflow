# LOO-889: Check decisions in-process, keep a small set of full-Machine tests

Full plan, measurements and check log: `git show b4ce5d7201:scratch/decisions-in-process.md`.
The previous restart note: `git show b9b1a2da2f^:scratch/decisions-in-process.md`.
PR #1924 carries the before/after table and checks.

## State (2026-11-13, 23:50Z)

Done-when is met; nothing left to implement in this Task.

- Promotion/binding, launch policy, Task Graph explain and Machine dispatch
  decisions run as in-process rows (`adoption::tests::`,
  `run::launch_policy_tests::`, `ops::task::explain::tests::`,
  `ssh::dispatch_tests::`, `task_ownership::tests::`). 35 rows, 5.8 s
  wall under nextest (`b9b1a2da2f^:scratch/qa-logs/rows.log`).
- `rust/loopflow/tests/README.md` lists each surface's kept full-Machine tests
  and the wiring each proves.

This kickoff is the third Graph run. Runs `3bd9ea27` and `392e8061` both
stopped at `land --wait-and-fix`; the first on
`gh pr list --head … : dial tcp 172.182.252.137:443: connect: no route to host`,
the second's reason was not found in `lf task status`. No Task comment or steer
adds scope.

## For the remaining steps

- implement / compress / realign: no code change. Slices 5 (release lock
  matrix) and 6 (local planning, `work_open_tests`, `context_launch_tests`,
  `settings_launch_tests`) are outside Done-when.
- gate / land: PR head `f5a28f36`, MERGEABLE, origin/main has nothing newer.
  At 23:50Z its CI had lint, build, python, e2e and library shards 0/1/2/5
  green; library 3/4, integration 1–3, macos-26 and task-installation still
  running. Wait on that run and fix real failures; don't push only to
  re-trigger.

## Open outside this Task

- `lf task create --session/--worktree` pays ~0.7 s of foreground git
  (origin fetch in `prepare`, planning push).
- `planning_explain` has no in-process rows; its six `global_commands`
  `task_planning_explain_*` tests (49 s warm) are the next candidate.

## Docs

No user-facing docs change; this is contributor test layout. Shipped docs:
`rust/loopflow/tests/README.md` (§ Decision rows and full-Machine tests) and
`scripts/testing.md`. write-docs read both; nothing to revise.
