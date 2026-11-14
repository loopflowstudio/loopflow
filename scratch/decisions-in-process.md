# LOO-889: Check decisions in-process, keep a small set of full-Machine tests

Full plan, measurements and check log: `git show b4ce5d7201:scratch/decisions-in-process.md`.
Earlier restart notes: `git show b9b1a2da2f^:scratch/decisions-in-process.md`,
`git show 784b175a63^:scratch/decisions-in-process.md`.
PR #1924 carries the before/after table and checks.

## State (2026-11-14, 00:20Z)

Done-when is met; nothing left to implement in this Task. The plan is unchanged.

- Promotion/binding, launch policy, Task Graph explain and Machine dispatch
  decisions run as in-process rows (`adoption::tests::`,
  `run::launch_policy_tests::`, `ops::task::explain::tests::`,
  `ssh::dispatch_tests::`, `task_ownership::tests::`). 35 rows, 5.8 s wall
  under nextest (`b9b1a2da2f^:scratch/qa-logs/rows.log`).
- `rust/loopflow/tests/README.md` lists each surface's kept full-Machine tests
  and the wiring each proves.
- Demo: `cargo nextest run -p loopflow --lib -E 'test(adoption::tests::) |
  test(launch_policy_tests::) | test(explain::tests::) | test(ssh::dispatch_tests::)
  | test(task_ownership::tests::)'` → 35 passed in under 10 s on a quiet Machine; 80 s at load average ~190 (QA 2026-11-13, `scratch/qa.html`).

This kickoff is the fourth Graph run (`e61630d8`). `3bd9ea27` and `392e8061`
stopped at `land --wait-and-fix`, `ca8e4f0e` at `pr-publish`; the first on
`gh pr list --head … : dial tcp 172.182.252.137:443: connect: no route to host`,
the other two reasons are not in `lf task status`. No Task comment or steer
adds scope.

## CI on this PR

A head that carries `scratch/` files runs no suites: `scratch-clear` reports
`candidate != 'true'` and every other job is skipped, yet the run concludes
`success` (`.github/workflows/ci.yml:86-123`). Read at 00:20Z:

| Head | Scratch in tree | CI run | What ran |
| -- | -- | -- | -- |
| `4ba42d46` | no | passed | full suites |
| `f5a28f36` | no | 43529173281 cancelled | superseded |
| `a4e2d438` | yes | 43530090285 success | only `scratch-clear`, `macos-inputs` |
| `5f9f1ca4` | yes | 43530783093 success | only `scratch-clear`, `macos-inputs` |
| `784b175a` | no | 43531279509 cancelled | superseded by `cf2b7c3d` |
| `cf2b7c3d` | yes | 43532376900 success | only `scratch-clear`, `macos-inputs` |

Code is byte-identical from `4ba42d46` to `cf2b7c3d` (`git diff --stat` shows
only scratch deletions). origin/main is still `39b37794`; PR is MERGEABLE.

So each restart note pushed to the branch replaces a head whose suites were
running with one that skips them; it happened again at `cf2b7c3d`. The last
finished full-suite run is on `4ba42d46`. A green run on a scratch-carrying head says
nothing about the suites.

## For the remaining steps

- implement / compress / realign: no code change. Slices 5 (release lock
  matrix) and 6 (local planning, `work_open_tests`, `context_launch_tests`,
  `settings_launch_tests`) are outside Done-when.
- gate / land: the head that lands has no scratch files and a finished
  full-suite run. Fix real failures; don't push only to re-trigger.
  `lf pr land` clears this note.

## Open outside this Task

- `lf task create --session/--worktree` pays ~0.7 s of foreground git
  (origin fetch in `prepare`, planning push).
- `planning_explain` has no in-process rows; its six `global_commands`
  `task_planning_explain_*` tests (49 s warm) are the next candidate.

## Docs

No user-facing docs change; this is contributor test layout. Shipped docs:
`rust/loopflow/tests/README.md` (§ Decision rows and full-Machine tests) and
`scripts/testing.md`.
