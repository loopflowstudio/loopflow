# LOO-562: Swift CI fast and fail fast

Jack Heart asked (2026-10-21) that `swift-test` report a real failure within
minutes, name the test, and not run when a change cannot affect Desktop; then
(comment, 18:55 PT) that macOS minutes go only to what gates a merge.

## Done in this branch

- Hang: misattributed to the theme test (see `questions.md`); the event
  streams of four failed CI runs show a `BuiltWorld` test never ending.
  Main's #1645 then deleted every world suite (LOO-564 brings them back);
  after rebasing, this branch keeps no Swift world code.
- Slowness: `World` defers the Git planning exchange and publishes once in
  `finish`; density world build 219 s → ~50 s locally, lifecycle 34 s → 14 s.
  LOO-564 restores the suites on top of this.
- Limit: `scripts/swift_test_limit.py` wraps `swift test` in `test_desktop.sh`;
  300 s per test function; step timeout stays 20 min as outer guard. It stops
  the whole descendant tree: swift test puts its helper, and Process each
  fixture child, in a new process group, and a surviving helper would hold
  CI's `| tee` open until the step timeout.
- Filter: `macos-inputs` job + `scripts/macos_inputs.py`; `swift-test`,
  `loopflow-ui-test` and `macos-process-test` skip only on explicit
  `macos=false`; `tests-result` accepts that skip only.
- Push to main after a queue proof: the macOS jobs build only to warm caches
  (`swift build --build-tests`, `cargo test --no-run`); no test re-run.
- Superseded queue runs: `cancel-superseded` (merge_group) cancels the same
  PR's earlier active queue runs; `merge-queue-cleanup.yml` (pull_request
  dequeued) cancels a departed PR's queue runs. Both run
  `scripts/cancel_merge_queue_runs.py`, which never fails the proof.
- runner=null cancellations (Jack's 01:29 UTC question): every such job was
  queued without a runner when its whole run was cancelled: the operator's
  three cancels at 01:57Z (runs 39705964655, 39706220934, 39707359846), PR
  concurrency on a new head (39714712410, 39714212449, 39697255569) and a
  re-run of #1638's PR run (39698907828, attempts 2–3). A queued job's
  "started" time is its queue time, so it reads as a seconds-long cancel.
  Not runner capacity; the cancellation jobs above remove the cause.

## Baseline (CI, successful runs 2026-10-21)

Swift build 31 s; CLI build 84–130 s; test run 842 s, of which density 506 s,
lifecycle 117 s, Local wave launcher 49 s, CLI→Work sidebar 40 s, world reads 35 s.
Main's #1645 removed the world suites, so the next green run on this PR sets
the new baseline.

## Remaining

- CI proof on the PR: a deliberately hung test fails within the limit
  (temporary commit, then revert); step times against the baseline; a
  docs-only change skipping the macOS jobs; a rebuilt queue entry cancelling
  its predecessor.
- CLI build (84–130 s) is unchanged: already behind `rust-cache` and skipped
  on merge-proof reuse. Revisit only if post-change step times show it leading.

## Concept review (2026-10-21)

No change. The exchange deferral is process-wide but only `lf-world` (a
one-shot process) constructs `World`, so it never reaches other `lf` work or
the shared integration binary. `macos_inputs.py` and `scripts/test.py`'s
swift predicate answer different questions on purpose: CI skips only when a
change *cannot* reach the macOS jobs (deny-list, unknown runs); the local gate
picks the suite a change most likely affects (allow-list). Merging them would
either widen the local gate to every Rust change or let CI skip world tests on
Rust changes. Per-save Git exchange cost in real `lf task create` stays a
candidate Reliability Task, outside LOO-562.

## Checks (gate, 2026-10-21, after rebase onto main at ed568b1ff5)

- `pytest test_ci_proof test_macos_inputs test_cancel_merge_queue_runs test_swift_test_limit`: 61 passed.
- `ruff check`/`format --check` on changed Python: clean (26 pre-existing findings in untouched benchmark/migration files).
- `cargo fmt --check`, `cargo clippy --all-targets -D warnings`: clean.
- `cargo test -p loopflow --test integration world_recipes::`: 5 passed (187 s).
- `scripts/test_desktop.sh -Xswiftc -gnone`: 614 tests in 78 suites passed, 180 s run, 352 s wall; multiplatform boundaries clean.
- CI-only: the hung-test proof, the skip on a docs-only PR, and queue-run cancellation need hosted runs; deferred to the PR's CI.
