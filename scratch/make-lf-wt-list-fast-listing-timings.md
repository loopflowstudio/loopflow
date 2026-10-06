# LOO-375 PR 3: GitHub's answer time

Jack Heart's October 5 steer: continue from installed evidence and diagnose the
largest remaining cost. Installed 0.13.4 reads 1.80 s median (three samples),
1.62 s of it waiting on GitHub.

## Done in this PR

- GitHub is asked 16 branches per request, side by side; one failed or stopped
  request leaves every branch unknown (`engine/worktrees.rs`).
- Default branch and worktree list, then origin URL and branch heads, are read
  side by side before the request.
- Report: `scripts/benchmarks/wt-list/README.md`, section "GitHub's answer time".

## Remaining on the Task

- ≤1 s warm p95 online is unmet: about 1.03 s median, p95 1.1–1.8 s on a fresh
  Home. A request costs about 0.4 s before GitHub does any work. Going lower
  needs PR state or `remote_gone` from something other than the remote; no
  such source was chosen.
- Installed `lf wt timing` for this change after a release carries it; seven
  installed samples so far are not a p95.

## Checks

`cargo test -p loopflow --lib engine::worktrees` 22 passed; `--test worktree_tests`
26 passed; `cargo clippy --all-targets -- -D warnings` clean. Gate owns the suites.
