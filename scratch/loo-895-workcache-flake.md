# LOO-895: WorkCacheTests waits on events, not a clock

## Problem

`collidingWaveNameSettlesBothPrimaries` failed on hosted macOS ("macOS on main"
run 43547448324, `286889bbec`) at the exact-list check of
`source.preparedPrimaries`. The bisect bot then opened revert PR #1929 against
an unrelated change.

## Findings

- The CI failure took **0.151 s**, not 5 s: the `spin` exited because
  `registry.preparationCount` reached 0, so the time budget did not cause it.
  The list was in the wrong order. With `preparationLimit = 2`, the repository and
  `wave-1` primaries prepare concurrently (`SessionsWorkspaceRegistry.openWarmConversations`,
  `swift/LoopflowMac/Views/SessionsView.swift`), and the `Source` actor appends
  them in whatever order the two reads arrive.
- Base `1d1ea6d45c` (#1923) already compares `preparedPrimaries.sorted()` in this
  test and in `loadOpensRepositoryAndWavePrimaries`. That fixes the observed cause.
  The arrival order is not a product guarantee, so production code doesn't change.
- What remains is the pattern named in the Task: `spin(for:while:)` (end of
  `swift/LoopflowTests/WorkCacheTests.swift`) yields until a condition clears
  **or a deadline passes** (3 s default, 5 s in the page/tile tests). Then it
  falls through to an exact assertion. On a slow runner, an expired deadline looks
  like a product failure.
- All 39 `spin` calls in the file wait for the state their next assertion expects.
  None relies on the deadline running out to show that nothing happened. That
  includes `restartWaitsForFreshEvidence` line 101, where both argument cases
  expect the error to be set.
- The one `Task.sleep` left (`unchangedPollDoesNotRewrite`, 20 ms) only spaces
  out modification times so a rewrite would show. A slow runner makes it stricter,
  not flaky, so it stays.
- Openings start synchronously inside `WorkModel.refresh` → `openWarmConversations`.
  So `preparationCount > 0` already holds when the colliding test starts waiting.
- Local probe on base: `swift test --package-path swift --skip-build --parallel
  --filter WorkCacheTests`, 20/20 passes on an unloaded Mac.

## Approach

Keep the file and its tests. They guard the warm-preparation behavior: Task bound,
primaries, refusals, page turns, focus. No other suite covers that. Remove the clock:

1. `spin` loses its `timeout` parameter. It yields while the condition holds and
   stops on cancellation: `while !Task.isCancelled, await condition()`. The
   `for: .seconds(5)` arguments go.
2. `@Suite("Work cache", .timeLimit(.minutes(1)))`. If an event never arrives, that
   test fails by name with "time limit exceeded" instead of a misleading
   assertion. The cancellation check lets the loop end.
3. `collidingWaveNameSettlesBothPrimaries` waits for what it asserts: all three
   primaries prepared and the queue settled. That means waiting while
   `count < 3 || registry.preparationCount > 0`, and still exiting at
   `count >= 8` so a runaway re-preparation fails the assertion instead of
   spinning out the limit.

Rejected: continuation-based waiters on `Source` and `onPreparationFinished`. They
add machinery for every condition, and a deadline-free `spin` on the
condition already waits on the event.

## Done when

- No deadline is left in `WorkCacheTests.swift`. The colliding-Wave test waits
  for three prepared primaries and an empty queue.
- `swift test --package-path swift --parallel --filter WorkCacheTests` passes
  20 runs in a row under parallel load (run alongside the full Swift suite).
- No production Swift changes. The PR names #1923's `sorted()` as the fix for
  the observed failure, and this change as the fix for the timing pattern.

## Checks

- `WorkCacheTests` ×20 run from the built bundle (`swiftpm-testing-helper … --parallel --filter WorkCacheTests`, see `scratch/qa/run-bundle.sh`) while two full-suite runs overlap it (timestamps in `scratch/qa.html`): 20/20 (QA). Two `swift test` processes do not overlap: the second waits on the `.build` lock, so the earlier "beside a concurrent full run" 20/20 ran unloaded.
- Breaks of `SessionsView.swift`: Wave primary prepared by name (#1874's bug) fails the colliding test in 0.7 s; a stalled preparation queue fails it by name with "Time limit was exceeded: 60.000 seconds" plus the missing-pane checks (QA).
- Same full run, outside scope: built-CLI suites fail without `target/debug/lf`; SessionsStoreTests (`.opening` vs `.prepared`, lines 549–763) and ProgramStatusTests:243 fail when their deadlines run out under load. Same pattern, separate Task.

## Docs

None. The change is test-only (`swift/LoopflowTests/WorkCacheTests.swift`), and no README or guide mentions `WorkCacheTests` or `spin`.
