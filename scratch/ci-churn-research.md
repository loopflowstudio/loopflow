# CI churn research: merges, conflicts, retests (2026-09-27 → 2026-10-27)

Scope: the 416 PRs merged into `main` in the last 30 days, 2,571 CI runs, 775 job listings, 73 failed-run logs, and 685 merge commits from `main` that were replayed locally.

## Headline

The merge queue is not where the time goes. Once a PR enters the queue for the last time, it merges in **10 min (p50) / 20 min (p90)**. Hours are lost **before the queue**, in a repair loop: CI fails, the agent re-runs `lf land`, `lf land` merges `main` again, the new `main` conflicts, and CI runs again. **119 of 161 resyncs after arming (74%) came straight after a CI failure, and 51 of those conflicted.**

## 1. CI passes per PR

| per PR | mean | p50 | p90 | max |
|---|---|---|---|---|
| `pull_request` runs | 3.49 | 2 | 7 | 82 |
| `merge_group` runs | 1.15 | 1 | 2 | 3 |
| all | 4.63 | 3 | 8 | 83 |

- Outcomes: `pull_request` 1014 success / 298 failure / 137 cancelled. `merge_group` 438 / 33 / 6.
- 490 of the PR "successes" ran for under 3 min. These are scratch checkpoints, where `candidate=false` skips the suite.
- 131 PRs (31%) had at least one failed PR run. 31 (7%) had a failed queue run. 55 (13%) were queued more than once.
- One queue run costs about 14 macOS and 32 Linux runner-minutes. Wall time is 9 min p50 and 19 min p90.
- Runner waits in the queue: macOS jobs start in about 0 min at p50, but p90 is 6 min for swift-test, 7 for ui-test and **16 for macos-process-test**. The 20–60 min runs Jack described are the tail, not the typical run.
- Total wall time: about 209 h in PR runs and 98 h in queue runs.

## 2. How often PRs resynced or conflicted

**Signal.** `lf land` syncs by merging `main` into the branch. I took every two-parent commit on each PR head (`refs/pull/N/head ^main`) and counted each merge commit once, under the earliest-merged PR that contains it (reused branches would otherwise double-count). I then replayed each one with `git merge-tree --write-tree`. Exit code 1 means the agent had to resolve a conflict.

**Limits.** This misses conflicts that were resolved by rebasing (83 force-push events). It also cannot see agents that abandoned a merge and started over.

| | syncs | PRs ≥1 | conflicting syncs | PRs with a conflict |
|---|---|---|---|---|
| before the PR opened | 253 | 144 | 112 | 63 |
| while the PR was open | 432 | 159 (38%) | 229 | **100 (24%)** |
| total | 685 | 217 (52%) | 341 | **126 (30%)** |

- The worst PR had 28 syncs while open.
- The queue removed PRs for `merge_conflict` 23 times, `failed_checks` 26 times, `manual` 17 times and `checks_timed_out` 2 times.
- Size predicts conflicts, though long-lived PRs confound this. By changed-file quartile, 8% / 12% / 34% / **68%** of PRs had a conflicting sync.

## 3. What conflicts

These are the files from the 341 replayed conflicting merges. "PRs" counts distinct PRs; "a/a" is hunks where both sides inserted at the same spot; "e/e" is hunks where both sides edited the same lines.

| file | PRs | a/a | e/e | kind |
|---|---|---|---|---|
| wave/infrastructure/MEMORY.md (moved to .lf/wave in #1536) | 45 | 32 | 225 | memory |
| docs/architecture-reference.md | 35 | 10 | 272 | shared prose |
| docs/lf-reference.md | 30 | 9 | 158 | generated |
| rust/loopflow/src/ops/task.rs (6.8k lines) | 30 | 22 | 377 | code |
| docs/lf.md (deleted in #1708) | 29 | 8 | 109 | generated |
| rust/loopflow/src/bin/lf.rs | 26 | 18 | 122 | command registry |
| rust/loopflow/src/lf/mod.rs | 25 | 21 | 93 | command registry |
| ops/human_session.rs | 23 | 35 | 384 | code |
| README.md / TESTING.md | 18 / 16 | | | prose |
| DTO fixtures (dto_fixtures.rs, DTOFixtureTests.swift, tests/fixtures/dto/*) | ~17 | | | registry/fixture |
| store/migrations.rs | 14 merges | 14 | 22 | registry |

Classified per conflicting merge:

- 74% (251) touched code.
- 14% (46) touched only Wave memory.
- 7% (23) touched only prose docs.
- 3% (10) touched only generated or registry files.

Nearly every conflicting merge also hit a docs file. Edit/edit hunks dominate, even in prose: agents rewrite the same paragraphs of the central docs because each PR is asked to keep the docs in sync.

Migration *numbering* is not a hotspot: the draft files under `migrations/drafts` conflicted only 2 times. Cargo.lock conflicted 2 times. Memory conflicts fell after the move to `.lf/wave`: 34 conflicting merges in week 41, then 0–4 per week.

I used real conflicts instead of the changed-files intersection the brief suggested, because the intersection would overstate them.

## 4. Why runs fail around merges

Coverage: all 33 failed queue runs, plus a random 40 of the 272 failed PR runs.

| Queue failure class (n=33) | n | evidence |
|---|---|---|
| Probably flaky: same PR head later merged with no new commits | 15 | rust tests 12, Swift 2, `TEST BUILD FAILED` 1 |
| Real, or fixed by the author: new commits before merge | 8 | rust tests 7, task-installation 1 |
| `scratch/` not clear in the queue | 7 | all before Oct 15 |
| Swift step timeout (20 min) | 2 | infrastructure |
| Cancelled because superseded | 6 cancelled runs | not failures |

Tests that failed in two or more unrelated PRs:

| test | failures |
|---|---|
| `loopflow_warnings_preserve_native_drafts_across_providers` | 3 PRs |
| `session_steers::public_context_capability_survives_claim_changes_and_rejects_children_and_replacement` | 2 |
| `ops::pm::task_planning_tests::native_runner_reconnects_planning_without_another_turn` | 2 |
| `lf::commands::install::artifact_tests::a_settled_install_keeps_only_what_it_can_select_or_is_running` | 2 |
| `task_delete_binary_reconciles_provider_and_local_history` | 2, 30 min apart (more likely `main` was broken) |
| `registry_is_complete` | 2 |

- `land_tests::waited_task_landing_repairs…` failed with `database is locked`.
- The Swift `staleSurfaceAndClosedCallbacksCannotPublish()` (ProgramStatusTests) failed once.
- `swift-test` timed out at 20 min in 4 of the 73 sampled runs.

Failed PR runs mostly come from development in progress, where the head changed afterwards in 38 of 40. The failing jobs are rust-test (254 job failures), scratch-clear (105), swift-test (69) and the ui-test compile check (31).

## 5. Time from ready to merge

"Ready" is the first time auto-merge was enabled or the PR was added to the queue; it was detectable for all 416 PRs.

| interval | p50 | p75 | p90 |
|---|---|---|---|
| created → merged | 0.59 h | 4.6 h | 20.2 h |
| ready → merged | 0.45 h | 1.1 h | 4.1 h |
| ready → final queue entry | 0.24 h | 0.86 h | 3.75 h |
| final queue entry → merged | 0.16 h | 0.20 h | 0.33 h |

Of 1,511 PR-hours from ready to merge, 1,425 were spent before the final queue entry and 86 in the queue.

- 236 PRs (57%) merged within 1 h with no failure and no resync.
- 108 PRs took more than 1 h. Of those, 74 had a PR CI failure after arming, 46 had a conflicting resync and 29 had a queue failure or retry.

## How to make merges (resyncs and conflicts) less common

Ranked by expected impact on this data.

1. **Repair CI without re-syncing.**
   - Evidence: 74% of post-arm resyncs (119 of 161) followed a CI failure, and 51 of those conflicted.
   - `prepare_pr` runs `sync_land` on every `lf land`.
   - Change: push the fix on top and merge `main` only when GitHub reports `CONFLICTING` or the queue ejects the PR for `merge_conflict` (12 conflicting resyncs followed such ejections).
   - Estimate: about 120 fewer resyncs and about 50 fewer conflict resolutions per month.
2. **Quarantine flaky tests.**
   - Evidence: about 15 of 33 queue failures look flaky. Each ejection usually sends the PR back through repair, and so back through step 1.
   - Start with the six tests above and the swift-test timeout.
3. **Stop routing every PR through central prose.**
   - Evidence: architecture-reference.md conflicted in 35 PRs, almost all edit/edit; README, TESTING and docs/*.md add more.
   - Options: split by area, so each PR's doc edit lands in its own file. Or apply "update docs" only when the area changes. Wave memory as one file per entry would remove the remaining `.lf/wave/*/MEMORY.md` collisions (22 merges since the move).
4. **Generate rather than commit, or make registries additive.**
   - Evidence: lf-reference.md conflicted in 30 PRs, lf.md in 29, command registries (`bin/lf.rs`, `lf/mod.rs`) in 25–26, the DTO fixture registry in about 17 and `migrations.rs` in 14 merges.
   - Options: render lf-reference.md in CI or docs builds, or regenerate it after the merge. Turn the registries into per-entry files or inventories.
5. **Split the giant hot modules.** `ops/task.rs` (6.8k lines, 30 PRs) and `human_session.rs` (23 PRs).
6. **Smaller PRs.** The top quartile by size conflicts 68% of the time, the bottom 8%. This is a correlation, not a measured effect.
7. **Smaller levers.**
   - The required `tests-result` passes on a scratch checkpoint, which let 7 PRs with `scratch/` still present into the queue. None since Oct 15.
   - Moving macOS checks out of the PR path would cut latency (p90 waits of 6–16 min) but has little effect on conflicts.
   - Dropping strict up-to-date would likely change little: the resyncs come from `lf land`, not GitHub. This is inferred, not measured.

<details><summary>Appendix: commands to reproduce</summary>

```sh
# PRs
gh pr list --state merged --limit 1000 --search "merged:>=2026-09-27" \
  --json number,headRefName,createdAt,mergedAt,additions,deletions,changedFiles > prs.json
# CI runs, in date windows to stay under the 1000-result cap
for r in 2026-09-20..2026-09-26 2026-09-27..2026-10-01 ... 2026-10-26..2026-10-28; do
  gh api "repos/loopflowstudio/loopflow/actions/workflows/ci.yml/runs?created=$r&per_page=100" --paginate \
    -q '.workflow_runs[]|{id,event,head_branch,head_sha,conclusion,run_attempt,created_at,run_started_at,updated_at}'
done > runs.jsonl
# Map runs to PRs: pull_request runs by headRefName with created_at in [createdAt-2m, mergedAt+1m];
# merge_group runs by head_branch gh-readonly-queue/main/pr-<N>-*
# Timelines and files (GraphQL, 20 PRs/query); PRs with >100 items re-queried without PULL_REQUEST_COMMIT
#   pullRequest(number:N){ files(first:100){nodes{path}} timelineItems(first:100, itemTypes:[AUTO_MERGE_ENABLED_EVENT,
#   AUTO_MERGE_DISABLED_EVENT,ADDED_TO_MERGE_QUEUE_EVENT,REMOVED_FROM_MERGE_QUEUE_EVENT,HEAD_REF_FORCE_PUSHED_EVENT,
#   READY_FOR_REVIEW_EVENT,PULL_REQUEST_COMMIT]){...} }
# Jobs for all merge_group runs + failed PR runs
gh api "repos/loopflowstudio/loopflow/actions/runs/$ID/jobs?per_page=100&filter=all"
# Logs: all 33 failed merge_group runs + random.seed(7) sample of 40 failed PR runs
gh run view $ID -R loopflowstudio/loopflow --log-failed   # grep 'test (\S+) \.\.\. FAILED', 'recorded an issue', 'timed out', 'TEST BUILD FAILED'
# Conflict replay in a scratch bare clone (no refs touched in the worktree)
git clone --bare --reference <repo>/.git https://github.com/loopflowstudio/loopflow.git mirror.git
git -C mirror.git fetch origin '+refs/pull/*/head:refs/pull/*'
git -C mirror.git rev-list --merges --parents --format='%ct %s' refs/pull/$N ^main
git -C mirror.git merge-tree --write-tree --name-only --no-messages $P1 $P2      # exit 1 = conflict; lists files
git -C mirror.git -c merge.conflictStyle=diff3 merge-tree --write-tree $P1 $P2    # parse markers: empty base = add/add
```
Each merge commit is counted once, under the earliest-merged PR containing it, and only if it was committed before that PR merged. "open" means committed between PR creation and merge. A resync's "preceding failure" is a failed run on that PR that ended after ready and before the resync.
</details>
