# LOO-589: why `closedWindowDoesNotPublishDelayedOpening` hung CI

## What failed

PR #1671 (docs only) failed `swift-test` twice on October 25: a PR run hit the
5-minute step timeout, and the merge-group run hit the 300 s Swift Testing limit
in `WorkDestinationTests.closedWindowDoesNotPublishDelayedOpening(fails:)`.
GitHub ejected #1671 from the merge queue (`failed_checks`). Anyone in the queue
behind a hang pays the same ejection; the PR itself changed no Swift.

Recovery: the fixture fix is in the working tree (uncommitted, unpublished).
Measured locally under 16 busy loops with `scratch/loo589_repeat.sh` (30 s cap per run):

| build | iterations | pass | fail | hang |
|---|---|---|---|---|
| before (HEAD 860910aaa1) | 200 | 187 | 0 | 13 (6.5%) |
| after (working tree) | 200 | 200 | 0 | 0 |

## Causal chain

1. **The test awaited `opening.value` forever.** `openTaskLink` was suspended
   in the stub at `barrier.wait("read")`, and nothing ever resumed it.
   `opening.cancel()` can't help: `withCheckedContinuation` ignores cancellation.

2. **Why was it never resumed?** Two queries were parked on the key `"read"`.
   `LinkedDestinationBarrier.wait` stored `pending[key] = $0`, so a second
   waiter silently overwrote the first. `release("read")` resumed only the
   survivor. When the survivor was the pane-layout read, the Task read was
   dropped and the test hung. When it was the Task read, the test passed and
   the dropped pane read leaked. Swift logged
   `SWIFT TASK CONTINUATION MISUSE … leaked its continuation` in all 187
   passing before-runs (0 after), so the overwrite happened in every run. Only
   the order changed. Which one won depended on how the
   two tasks were scheduled, so CPU load decided the outcome.

3. **Why were there two queries?** The stub was a catch-all (`{ _, _ in … }`):
   every query the model issued went through the barrier. `destinationModel`
   calls `bindWorkspaces`. With `repoPath: "/origin"`,
   `openSelectedWorkspace` → `workspace.startPersistence(query:repoPath:)`
   starts a background `loadPaneIntent()` → `query.paneLayout(...)`
   (`swift/LoopflowMac/Views/SessionsView.swift:1593-1621`). That query reaches
   the same closure.

4. **Why did a working test start racing?** The test came from #1449/#1601,
   before persistence existed. #1580 (`70e08c2a00`, 2026-10-22) sent pane-layout
   persistence through the shared `RegistryQuery`. That gave every catch-all stub
   a second caller. #1580 edited this file but did not touch this test: it still
   passed most of the time, and nothing reported the overwritten continuation.
   Its sibling `resolvingTaskKeepsTheWorkspaceVisible` has the same catch-all
   stub, but it passes no `repoPath`, so `startPersistence` returns early. It is
   safe by accident. That is the difference between the two tests.

5. **Why did one flaky test cost a merge-queue slot?** The only backstop was
   time. `test_desktop.sh` and the 20-minute queue timeout limit a hang but
   cannot identify it until the 300 s Swift Testing limit expires. The CI design
   (comment at `.github/workflows/ci.yml:423`) intends the PR job to fail slow
   suites first. A race that is rare on the PR runner gets past that check and
   then fails in the queue. The step-summary reporter (`scripts/swift_test_report.py`)
   did name the test, so diagnosis was fast. Only the detection was late.

6. **Why did nobody see it for three days?** Swift printed the leak warning in
   every passing run from #1580 onward, but a warning on stderr doesn't fail the
   test. The runtime detected the bug every time; CI never treated that output
   as a failure.

## Fixes made (working tree, not yet landed)

- Only `--task` queries reach the barrier in this test; any other query throws
  at once. The workspace treats a failed pane read as a persistence error, as it
  does in production.
- `LinkedDestinationBarrier.wait` records an `Issue` and resumes the earlier
  waiter when a key is reused. The same overwrite in any of its ~15 users now
  fails fast, with a message, instead of hanging.

Production code is unchanged: canceling during a Task read is still covered.

## Proposed prevention (not approved work)

- **Stubs route by arguments, not catch-all.** Any `RegistryQuery { _, _ in … }`
  that parks or returns a fixed payload gets a second caller as soon as the model
  issues a new query. Making these stubs match on `args` (as
  `overlappingLinksRespectLaterNavigation` does) and throw on anything else turns
  "new background query" into an immediate, named failure. Proof: grep
  `RegistryQuery { _, _ in` in `swift/LoopflowTests/` and see each one route or
  justify itself.
- **Per-test `.timeLimit(.minutes(1))` on barrier-driven tests** (precedent:
  `ProgramStatusTests.swift:116`). This doesn't raise any limit. It lowers the
  cost of the next unknown hang from 300 s plus ejection to 60 s, and the test
  is still named. It does not remove the queue ejection.
- **No retries.** AGENTS.md rules out retries for flaky tests. The barrier
  assertion is the structural fix: a reused key cannot hang silently anymore.
- **Fail swift-test on `SWIFT TASK CONTINUATION MISUSE`.** This is the
  highest-leverage option: the runtime already detects continuation misuse in
  every run, and CI would catch it on the PR that causes it, not by chance in
  the queue. Precondition: confirm that today's suite emits no such warning,
  e.g. `grep -c 'CONTINUATION MISUSE' output.log` on a green main run. Proof:
  revert this fixture fix on a branch and watch the PR job fail with the
  warning instead of passing 187/200.

## Execution decision (launch-plan, October 25)

This Task ships the fixture fix **and** the CI guard that fails `swift-test` on
`SWIFT TASK CONTINUATION MISUSE`. The precondition is measured: green CI run
40186528557 (job 121148404257) printed exactly two such warnings, both from
`closedWindowDoesNotPublishDelayedOpening` (`fails → false`, `fails → true`).
With this fixture fix the suite is clean, so the guard lands in the same PR
without fixing anything else. Build it where `output.log` is already captured
(the "Run headless model and view tests" step in `.github/workflows/ci.yml`, or
`scripts/swift_test_limit.py`), and print the offending test's name. Done when:
the guard passes on this branch, and reverting the fixture fix locally makes
the same check fail with the warning.

Not pursued, no Tasks filed:
- Routing all 103 `RegistryQuery { _, _ in` stubs. The CI guard and the barrier
  assertion catch the failure this would prevent; rewriting the stubs costs
  more than it saves.
- Per-test `.timeLimit`. The guard catches leaks on a passing run, before a
  hang can happen.

## Open questions

- The first PR-run failure (step timeout, run 40183133999) left no
  test-level evidence in the incident record. It is assumed to be the same test
  on the same race, not verified.

## Check result

Guard lives in `scripts/swift_test_limit.py`. `swift test` relays the runtime warning on stdout, and the event stream lags it, so attribution comes from the last `Test … started.` line in the same output. `WorkDestinationTests` with the fix: rc 0, no warnings. With HEAD's fixture: rc 1, naming both `closedWindowDoesNotPublishDelayedOpening(fails:)` cases. Full suite left to CI.
