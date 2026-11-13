# LOO-889: Check decisions in-process, keep a small set of full-Machine tests

Full plan, measurements and check log: `git show b4ce5d7201:scratch/decisions-in-process.md`
(cleared by gate at `9dab90df`). PR #1924 carries the before/after table and checks.

## State (2026-11-13)

Done when is met; nothing left to implement in this Task.

- Promotion/binding, launch policy, Task Graph explain and Machine dispatch
  decisions run as in-process rows (`adoption::tests::`,
  `run::launch_policy_tests::`, `ops::task::explain::tests::`,
  `ssh::dispatch_tests::`, `task_ownership::tests::`).
- `rust/loopflow/tests/README.md` lists each surface's kept full-Machine tests
  and the wiring each proves.
- PR #1924 body has before/after per touched group.

This Graph run is a restart, not new direction. The previous run reached
`land --wait-and-fix` and failed on the network:
`gh pr list --head … : dial tcp 172.182.252.137:443: connect: no route to host`.
The auto-retry then restarted from kickoff. No Task comment or steer adds scope.

## For the remaining steps

- implement / compress / realign: no code change. Slices 5 (release lock
  matrix) and 6 (local planning, `work_open_tests`, `context_launch_tests`,
  `settings_launch_tests`) were scoped out of Done-when; they need their own
  Task if wanted.
- gate / land: CI for `9dab90df` was running when this restart began. Wait on
  it and fix real failures; a commit of this file re-triggers CI.

## Open outside this Task

- `lf task create --session/--worktree` pays ~0.7 s of foreground git
  (origin fetch in `prepare`, planning push).
- `planning_explain` has no in-process rows; its six `global_commands`
  `task_planning_explain_*` tests (49 s warm) are the next candidate.

## Docs

`rust/loopflow/tests/README.md` (Decision rows and full-Machine tests) and
`scripts/testing.md`; every test and function they name exists at `9dab90df`.
