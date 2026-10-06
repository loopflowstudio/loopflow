# v0.13.6

<!-- loopflow:release-notes=narrative;gate=safe -->

v0.13.6 keeps Desktop current as work changes across windows, the CLI, and local checkouts. A persistent workspace stream replaces repeated refresh readers, while concurrent GitHub requests shorten worktree listings. This patch also removes an obsolete telemetry output that was causing scheduled release telemetry to fail.

## Follow work without refreshing

Desktop now follows Task and Session changes automatically, rereading only affected parts of the workspace. On macOS, checkout file and Git metadata watchers also keep local progress and Task file comparisons current after edits and commits, including in linked worktrees.

- Changes written to the local store from another window or the CLI appear without a manual refresh.
- Transcript traffic no longer triggers workspace summary reads; token totals refresh at most every ten seconds.
- Stale frames are rejected after local edits or selection changes. If a reader fails, Desktop preserves its last reading with an unavailable indicator and retries with backoff.
- Planning reads gather unfinished Execs together, reuse Git answers, and query existing checkouts concurrently to reduce background work.

Linear changes still require sync into the local store, and the outline continues to show only started Tasks. Recorded reader measurements were approximately 0.3 seconds from a Task commit to a frame, 0.13 seconds for a Session, and two seconds for a checkout edit; final window rendering was not remeasured.

## Spend less time listing worktrees

`lf wt list` now queries GitHub in concurrent batches of 16 branches and reads local branch and worktree metadata concurrently. In the recorded 55-worktree case, listing medians fell from 1.42–1.53 seconds to 1.02–1.07 seconds.

- Local and remote checks share branch heads instead of reading them twice.
- If any GitHub request fails, partial answers are discarded, preserving unknown PR status and the existing remote fallback behavior.

The one-second online p95 target remains unmet: measured candidate p95 ranged from 1.11 to 1.82 seconds. Measurements preceded the final shared branch-head read; installed validation remains outstanding.

## Operational notes

The lifecycle scorecard no longer emits an observation for the retired `task-loop-trust` metric. It retains lifecycle evidence and an empty `metric_observations` list in the report envelope, while strict metric validation remains in place. Missing database, history, or policy files still fail without emitting an envelope.

Recorded verification includes 48 worktree tests, Rust lint checks, and 12 lifecycle scorecard tests. The installed scorecard returned 35 report rows against the checkout. No release was launched to verify automatic scheduled release recovery.