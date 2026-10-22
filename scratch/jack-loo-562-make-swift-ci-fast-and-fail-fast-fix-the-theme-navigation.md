# LOO-562: Swift CI fast and fail fast

Jack Heart asked (2026-10-21) that `swift-test` report a real failure within
minutes, name the test, and not run when a change cannot affect Desktop.

## Done in this branch

- Hang: misattributed to the theme test (see `questions.md`); the event
  streams of four failed CI runs show a `BuiltWorld` test never ending, which
  stands as the CI reproduction. `BuiltWorld` deleted; world suites use `World.shared`/`World.fresh`
  (`terminationHandler`, file output). Lifecycle suites share one build.
- Slowness: `World` defers planning exchange; density world build 219 s → ~50 s
  locally, lifecycle 34 s → 14 s.
- Limit: `scripts/swift_test_limit.py` wraps `swift test` in `test_desktop.sh`;
  300 s per test function; step timeout stays 20 min as outer guard. It stops
  the whole descendant tree: swift test puts its helper, and Process each
  fixture child, in a new process group, and a surviving helper would hold
  CI's `| tee` open until the step timeout.
- Filter: `desktop-inputs` job + `scripts/desktop_inputs.py`; Desktop jobs skip
  only on explicit `desktop=false`; `tests-result` accepts that skip only.

## Baseline (CI, successful runs 2026-10-21)

Swift build 31 s; CLI build 84–130 s; test run 842 s, of which density 506 s,
lifecycle 117 s, Local wave launcher 49 s, CLI→Work sidebar 40 s, world reads 35 s.

## Remaining

- Publish the PR (none exists yet), then CI proof on it: a deliberately hung
  test fails within the limit (temporary commit, then revert); step times after
  against the baseline; a docs-only change skipping Desktop.
- CLI build (84–130 s) is unchanged: already behind `rust-cache` and skipped
  on merge-proof reuse. Revisit only if post-change step times show it leading.

## Delete — do not maintain

- `swift/LoopflowTests/BuiltWorld.swift` (deleted; behavior kept by `World`).

Local checks: world Swift suites 6/6 pass (197 s); deliberate hung Swift test
through `| tee` named and stopped at 304 s, no leftovers; Rust world_recipes 3/3;
limiter/classifier/CI-proof pytest pass; fmt + clippy clean.
