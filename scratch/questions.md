# LOO-595 notes

## Root cause (for the PR)

lf disabled its own arm. `enable_auto_merge` replaced any existing GitHub request
(disable, then `gh pr merge --auto --match-head-commit`), and `prepare_pr`'s
unconditional `clear_task_pr_merge_before_head_mutation` disables first too. GitHub
records lf's `--disable-auto` as "Manually disabled by user" under Jack's token
(#1667, 02:32:42Z). The re-arm then exited 0 without arming: gh's own read saw the
PR as already queued ("already queued to merge", as #1666 showed), while lf read
`mergeQueueEntry` only and saw no request. The landing watcher then found the
latest event to be a manual disable and stopped with "run lf arm".

Fix: read `isInMergeQueue` as Queued; keep an exact-head queue entry instead of
replacing it; after arming, confirm GitHub holds a request (3 reads, 1s apart),
ask once more, and otherwise fail with gh's answer instead of "handed off".

## Assumptions

- Not changed: the disable-before-sync in `prepare_pr` when the stored Task request
  doesn't match. With the queue read fixed, a queued exact-head PR takes the
  "already armed" shortcut; other cases now re-arm with verification.
- A non-queue repo answering "clean status" to `--auto` still fails with gh's error;
  loopflow uses a merge queue, so direct merge fallback is left out.

## Remaining

- Gate: integration suites (`land_tests`, `release_tests`, `release_lock_tests`).
- Done-when: the `ship` Graph lands this PR without manual re-arming.

## Proposed Wave memory (infrastructure)

- GitHub's `autoMergeRequest` is null once a PR enters the merge queue, and
  `mergeQueueEntry` can lag `isInMergeQueue`; read both. `gh pr merge --auto` can
  exit 0 without arming, so confirm the request on GitHub after asking.

## Checks

- compress: `cargo test -p loopflow --lib ops::pr::` 44 passed; `cargo clippy -p loopflow --all-targets -- -D warnings` clean. Integration suites deferred to gate. Realign: every fake-gh merge fixture includes `isInMergeQueue` (grep).

## Concept review (2026-10-25)

One concept carries the outcome: a PR's merge request is armed, queued, or absent,
read from GitHub (`isInMergeQueue`, `mergeQueueEntry`, `autoMergeRequest`). Arming
means GitHub holds a request afterwards, not that gh exited 0. lf's own disables
still look like a person's to the timeline outside CI repair windows, but each is
now followed by a verified re-arm or a loud failure, so the watcher can no longer
stop on a disable lf left behind. No design change; no open product choice.
