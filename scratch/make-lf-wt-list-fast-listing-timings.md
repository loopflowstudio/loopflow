# LOO-375 PR 3 (#1456): landing is waiting on an installed fix

October 6: `lf sync origin/jack/make-lf-wt-list-fast-listing-timings` recorded the
PR's own remote tip `64a0457eb` as its base (was `c41895363`). `lf pr land` and
`lf sync origin/main` refuse the range as contaminated. No commit is lost: HEAD
`e69717ddd` equals the pushed PR head and contains main through `501073137`.

- Do not run the printed `git rebase --onto origin/main 64a0457eb …`: it drops
  the feature commits.
- No installed 0.13.4 command moves the base back.
- Fix: PR #1459 (auto-merge requested). After a release carrying it is
  installed, run `lf pr land` here; the base heals to the fork point from main
  because `64a0457eb` is in `origin/<branch>`'s reflog (expires after 90 days).

Remaining Task work is unchanged: ≤1 s warm p95 online is unmet; installed
`lf wt timing` after release owns the measurement.
