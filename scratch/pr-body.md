Fixes LOO-577. On PR #1668 a CI repair turned auto-merge off before pushing its fix, as repairs do. `lf land --wait-and-fix` read that as a person dequeuing the PR and stopped with "run lf arm to resume landing". The repair turned auto-merge back on 30 minutes later, but nothing was watching anymore, so the Workflow stayed stopped.

## What changes

- **`lf land` waiter:** when auto-merge is off while a repair on the PR is still running, the waiter keeps watching. The `Failing` case already did this. The live repair reservation records that lf turned auto-merge off, so this adds no new table.
- **`lf release run`:** `wait_for_pr_merge` checks for a live repair before it reads GitHub's removal history. While a repair runs, it keeps polling and leaves the old head un-armed.
- `repair_running` and `pr_repair_processes` now look up repairs by `(repo, pr)` rather than by landing id. A repair reserved by any landing on the PR counts, whether `lf land` or `lf ci watch` started it.

If auto-merge is off and no repair is running, both waiters still block, the same as before.

## Proof

- `repair_disarm_keeps_landing_until_the_repair_rearms_and_merges`: auto-merge is off during a live repair, the landing keeps watching, then the PR merges, with no second repair.
- `release_run_waits_out_a_repair_disarm_but_stops_on_a_persons`: during a live repair, the release finishes without re-arming. With no repair, it fails with `ManualMergeRemoval`.
- `cargo fmt --check`, `cargo clippy --all-targets -- -D warnings`, `--lib pr_landing` (20), `--lib ci_watch` (9) and `--test release_tests` (75) all pass. The new release test takes about 27s because of the existing 10s merge poll.

## Not done here

- A repair that dies after turning auto-merge off and never turns it back on still produces the generic "run lf arm" block. The message should say that the repair ended without re-arming.
- Arming at an exact head may make the repair's disarm unnecessary. A pushed head would already cancel the old request. Check this before more readers have to special-case the disarm.
- A held landing can't resume: once the step exits, nothing watches a later re-arm. Not surveyed: whether Desktop or `lf task status` show this repair window as "not landing".
