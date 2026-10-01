# LOO-365: `lf ci watch`

Status: implemented on this branch, not yet reviewed or published.

## What it is

One repository-wide program with one job: watch the repository's PR checks and
start a ci-fix when a recorded landing fails. It is not a general watcher.
Decisions are Jack Heart's, from the LOO-365 thread on 2026-10-01.

```bash
lf ci watch               # foreground, until stopped
lf ci watch --once        # one pass
lf ci watch --install     # launchd service running the same command
lf ci watch --uninstall
lf ci watch --status      # live or not, last poll, what it started and why
```

Loopflow Desktop runs the same command for each repository open in a window and
stops it on quit (`CIWatchers`, hooked from `PodiumView`).

## How it works

- **Detection is REST with `If-None-Match`.** Per pass: the open-PR list, then
  `commits/{head}/check-runs` and `commits/{head}/status` per PR. Required check
  names come from `rules/branches/{base}` (rulesets) and classic branch
  protection, reread hourly. `MergeGateReading::from_checks` decides the gate.
- **Repair is the existing landing check.** A failing landing goes to
  `pr_landing::repair_landing`, which is `reconcile_pr_landing` with a repairing
  driver: landing lock, generation claim, a confirming GitHub observation,
  `ci_incidents`, `repair_reservation`, `admit_ci_fix`. That path already stays
  silent for a queued PR. The watcher adds no second claim.
- **`provider_completed_at`** is filled from the failing checks' `completed_at`.
- **Pacing:** 60 s ±15 %, doubling to a 5 min cap after a degraded pass, and
  waiting for the reset when fewer than 500 core requests remain.
- **One live copy per repository:** `<git-dir>/lf-ci-watch.lock`. A second copy
  stands by and takes over when the first exits; `--once` exits instead.
- **State:** `<git-dir>/loopflow/ci-watch.json`, read by `--status`.

## What changed in the cron

`lf task reconcile` (the one-minute launchd job) and `lf pr reconcile` no longer
start repairs. On a failing landing they record the incident and return.

What remains in the cron, because something still needs it:

- resuming enrolled Tasks' saved Flows (`reconcile_task`);
- settling verified merges and cleaning up landed checkouts;
- recording failures, and keeping each landing's pending-CI clock;
- blocking a landing whose auto-merge request was revoked or whose timeout
  rerun allowance is spent.

A release's own landing (`reconcile_armed_pr`) still repairs: it is a foreground
command that owns its landing and cannot assume a watcher.

## Delete — do not maintain

- The repair branch of the scheduled check: `reconcile_repository_async` now
  passes a driver with `repairs: false`. Required behavior preserved on the
  surviving path: `lf_pr_land_returns_before_later_checks_repair_and_observe_merge`
  now drives repair through `lf ci watch --once` for every variant it covered
  (repair, failed start, blocked, awaiting queue, saved Flow).
- Nothing else was deleted. Jack's earlier decision to delete the cron outright
  was narrowed by the supervising request to its repair path.

## Remaining

- **Watcher state in the Task workspace (LOO-353).** `--status --json` and the
  state file exist; no Desktop view reads them yet. `CiWatchStatus` gets a
  Swift mirror and a DTO fixture when one does.
- **Confirmation still uses GraphQL.** Steady-state polling is REST, but the
  confirming observation, the repair worker's re-check and `ci-fix` itself read
  GraphQL. With GraphQL exhausted the watcher detects a failure and cannot start
  the repair. A REST observation needs merge-queue membership, which REST does
  not expose.
- **Desktop Background progress copy** still describes only the minute check.
- **Live proof.** No real failing landing was repaired by this build. Verified:
  a real `--once` pass against GitHub with an empty Home, and the end-to-end
  landing test against a simulated GitHub.

## Checks

2026-10-01: `cargo fmt`, `cargo clippy -p loopflow --all-targets -- -D warnings`, `cargo test -p loopflow --lib -- ops::ci_watch ops::pr_landing` pass after the compress pass (25 tests); earlier on this branch `ops::cron lf::tests`, `--test land_tests lf_pr_land_returns_before`, `--test release_tests release_run_repairs_failed_checks`, `--test documented_commands`, `--test golden_prompt`, `--test scheduled_task_tests` and `swift test --filter CIWatchersTests` passed; full suites belong to gate/CI.
