# Task-name verification on merged main

Merge: PR #1943, `f3eab806e3fec7c05fb244c1be11138e16ecc16c`. Verification ran in
the Task checkout on a tree identical to origin/main (tree
`22ef8d5a9ed2060ec27ec03dea3f1f47964da8e7`).

## Merge-SHA CI (observed 2026-11-14)

- push / CI: success, run 43681113666.
- push / macOS on main: success, run 43681113631 (in progress at first look,
  completed green later).
- merge_group / CI: success, run 43680389635.
- workflow_run / Packages (main): queued, run 43681185336.

## Local merged-tree observations

- `swift build --build-tests --package-path swift --jobs 4`: passed.
- `uv run python scripts/check_desktop_launch.py` (temp `LF_HOME`): Desktop
  launched, wrote its snapshot and quit. Launch/exit only.
- `-ui-test-mode mock-waves` capture showed an empty sidebar: no rendered
  Task-label evidence. Native rendered labels, Retry interaction and provider
  input remain unverified; recovered notes at `b552c4fefe:scratch/questions.md`
  quote Jack Heart: “ok land it”, keeping those limits.
- Desktop filter run (readability, metadata, cache, navigation, pane control,
  create/reopen, conversation recovery): 131 tests, 1 failure at
  `WorkNavigationTests.swift:787`:
  > Expectation failed: (owner.startingTaskSessions → ["issue-review"]).isEmpty → false

  Cause: `newSessionFailureKeepsItsTask` reused one three-second deadline
  across the failed attempt and the retry, so the retry's wait could be
  skipped. Hosted macOS CI passed the same test; the failure is timing in the
  fixture, not product behavior.

## Fix-forward (committed on this branch)

`newSessionFailureKeepsItsTask` now awaits each attempt's removal from the
originating owner's `startingTaskSessions` through Observation
(`waitForAttempt`); the shared deadline and both sleep/poll loops are gone.
Production already settles that owner in the button's `defer` after recording
failure, so no product, DTO or doc change. Concept review found nothing to
change (`scratch/concept-review.html`).

- `swift build --build-tests --package-path swift --jobs 4`: passed.
- `scripts/test_desktop.sh --skip-build --filter WorkNavigationTests/newSessionFailureKeepsItsTask`: passed, 1 test.
- `uv run python scripts/check_swift_multiplatform_boundaries.py`: passed.

## Remaining

Gate and publish the fix-forward as a new PR (keep #1943 as history), then
verify it merged. The Task stays open until then.
