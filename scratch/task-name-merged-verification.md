# Task-name verification on merged main

Merge: PR #1943, `f3eab806e3fec7c05fb244c1be11138e16ecc16c`.
`git fetch origin main` resolved main to that commit. `git diff HEAD origin/main`
was empty; both tree IDs were `22ef8d5a9ed2060ec27ec03dea3f1f47964da8e7`.
The Task branch remains in place: `git merge --ff-only origin/main` refused
divergent history without changing files. Verification uses the identical main
tree in the Task checkout.

## Observations

- `swift build --build-tests --package-path swift --jobs 4` printed
  `Build complete! (104.51s)` and exited 0.
- `uv run python scripts/check_desktop_launch.py`, with inherited `LF_*`
  removed and temporary `LF_HOME`, printed `Desktop launched, wrote its
  snapshot and quit`; exit 0. This checks launch/exit, not populated rows.
- A second launch used `-ui-test-mode mock-waves`, temporary `LF_HOME`, an
  explicit checkout `LF_BIN`, and snapshot controls. It captured
  `/tmp/loo-905-main-fixture.png` and exited. The image showed an empty sidebar;
  it supplies no rendered Task-label validation. The first unbundled fixture
  capture lacked an explicit helper and displayed the missing-helper error.
  No populated snapshot or native-input success is claimed.

## Exact merge SHA CI

`gh run list --commit f3eab806e3fec7c05fb244c1be11138e16ecc16c`
reported these independently:

- **push / CI: success**, https://github.com/loopflowstudio/loopflow/actions/runs/43681113666.
- **push / macOS on main: in progress**, https://github.com/loopflowstudio/loopflow/actions/runs/43681113631.
  Swift tests and Xcode build checks were still running at observation time.
- **merge_group / CI: success**, https://github.com/loopflowstudio/loopflow/actions/runs/43680389635.
- **workflow_run / Packages (main): in progress**, https://github.com/loopflowstudio/loopflow/actions/runs/43681185336.

PR checks and merge-queue results are not substituted for push results.

## Decision basis

Recovered notes at `b552c4fefe:scratch/questions.md` quote Jack Heart:
“ok land it”, with continuation through merge and completion while retaining
native-verification limits. The prior QA observations distinguish socket labels
from pixels, and the design's Done when section reports missing native checks
as unverified. Current AGENTS.md makes display diagnostics optional for headless
acceptance. Missing rendered evidence alone is no concrete repairable defect.
The gate repair keeping saved rows until Task-list membership answers is present
on main. The preceding implementation made progress: all Task planning
readability checks passed on the merged tree.

## Merged verification failure

`scripts/test_desktop.sh --skip-build --filter 'TaskPlanningReadabilityTests|TaskRowMetadataTests|WorkCacheTests|WorkNavigationTests|DesktopPaneControlTests|WorkOpeningCLITests/localCreatedAndReopenedNamesPrecedeRoadmapDelivery|WorkDestinationTests/composedOpeningWaitsForNativeSessionAndComparison|WorkDestinationTests/conversationAttachmentRecovery'`
exited 1 and printed:

> Test run with 131 tests in 7 suites failed after 156.915 seconds with 1 issue.

The sole failure, at `WorkNavigationTests.swift:787`, was:

> Expectation failed: (owner.startingTaskSessions → ["issue-review"]).isEmpty → false

`newSessionFailureKeepsItsTask` creates one three-second deadline for the first
failure, then reuses it for the subsequent successful retry after multiple view
inspections and navigation. Its final wait can skip entirely when that earlier
deadline has expired. Repair the fixture to await completion of each distinct
attempt deterministically; do not widen the timeout or rerun unchanged until
green. Check that the production action settles the original navigation owner.
This test is part of the Task's modified navigation coverage and must be fixed
here under AGENTS.md's no-flaky-tests rule. No product defect is established by
this assertion alone.

Readability, metadata, cache, pane control, conversation readiness/recovery and
local create/complete/reopen checks passed. The real-CLI create/reopen test
printed `passed after 65.599 seconds`. A diagnostic sample while output was
buffered showed an idle main run loop; tests subsequently finished, so no hang
is claimed. No retry was performed.

## Fix-forward plan (2026-11-14)

Await each attempt's removal from the originating navigation owner's
`startingTaskSessions` through Observation. The production button already captures
that owner and removes the Task in `defer`, after recording any failure. Keep the
existing button, navigation, error-visibility and successful-retry assertions.

Delete — do not maintain: the shared three-second deadline and both sleep/poll
loops in `newSessionFailureKeepsItsTask`. No production path or data migration is
replaced. No product behavior or documentation change is needed.

Checks (inherited `LF_*` cleared for build/test):
- `swift build --build-tests --package-path swift --jobs 4`: passed, 74.05 s.
- `scripts/test_desktop.sh --skip-build --filter WorkNavigationTests/newSessionFailureKeepsItsTask`: passed, 1 test in 3.370 s, WindowServer denied.
- `uv run python scripts/check_swift_multiplatform_boundaries.py`: passed.

Remaining: caller's gate/publication and merged verification for the fix-forward
PR, retaining #1943 as delivery history. The Task stays open. Hosted macOS push
CI remained in progress at the last observation.
No server, watcher or app launched by the earlier verification remains running.

## Concept review of the fix-forward (2026-11-14)

Outcome for loop-or-next: nothing to change in the design; no open product
choice. Advance with the existing test repair. No code changed in this review.

The interaction remains: recognize a named Task during loading, retry a failed
read without losing its workspace, and open a conversation with separate input
readiness. New session failures belong to the Task and repository where the
attempt began. Returning there shows the failure and permits another attempt;
another repository must not inherit the error or pending state.

The pending patch matches that ownership: each wait observes the originating
navigation owner's `startingTaskSessions`, including the successful retry.
The test and button run on the main actor, so the condition check and observation
registration cannot interleave with the button's completion. Observation can
wake before mutation completes; the loop rechecks after returning to the actor.
The production action captures that owner and settles it in `defer` after
recording failure. No persistence, DTO, transport, read owner or usage-doc change
is needed. The shared elapsed deadline was the mismatch, not the interaction.

Counterexamples retained: navigation to another Task and repository while the
first attempt waits; failure visible only on return to its Task; retry clears
that Task's error and settles its own pending state. Planning has independent
counterexamples: a healthy subscription outranks a retained roadmap name, a
successful empty list ends saved membership, and readable identity does not
establish conversation readiness. The merged implementation keeps these rules.

Review used the original design at
`b552c4fefe:scratch/task-planning-readability.md`, current production code,
the pending test diff and the recorded checks above. No test was rerun here;
native rendered labels, Retry interaction and provider input remain unverified.
The next delivery step owns publication and merged verification of the repair.
Review page: `scratch/concept-review.html`.
