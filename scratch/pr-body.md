#1811's quit hang passed PR checks and kept `Packages (main)` red for about 22 hours. The `swift-test` job now launches the debug Desktop app and requires it to snapshot and quit, catching that failure before merge.

<!-- loopflow:task-pr-context:start -->
> [!NOTE]
> **Task:** [Pull requests that hang Loopflow.app at launch or quit fail their own checks · LOO-800](https://linear.app/loopflow/issue/LOO-800/pull-requests-that-hang-loopflowapp-at-launch-or-quit-fail-their-own)
> **PR lifecycle:** The pull request is published for review; no Task settlement is requested.
<!-- loopflow:task-pr-context:end -->

## Changes

- `scripts/check_desktop_launch.py` runs `LoopflowMac -ui-test-mode empty-workspaces`. It requires a fresh, non-empty snapshot and exit 0. After 30 seconds it samples the app, kills its process group, and fails with output and thread stacks.
- Release packaging uses the same launcher while hiding SwiftPM build resources; its duplicate launch implementation is removed.
- `swift-test` runs the check after its headless tests, reusing the debug build. Changes to the launcher itself also select this job.
- Swift Testing docs and the repository QA skill describe the command; infrastructure memory records the post-merge-only check failure.

## Verification

- Focused launch, release, macOS classification and release automation tests: **57 passed**, with external networking blocked.
- Current debug app built and passed launch, snapshot and quit in **10.00 seconds**. The initial link failed because the Rust FFI archive was absent; building it first, as CI does, resolved that. The QA instructions now include the prerequisite. Existing Swift warnings and local native deployment-target warnings remain outside this diff.
- Architecture, Ruff lint/format, and diff whitespace checks passed.
- Earlier local debug regression: reverting #1846's Swift changes fails in **34–35 seconds**, printing `View.uiTestSnapshot()` → `-[NSApplication terminate:]` → `nextEventMatchingMask:`. No app process remained. A release build is unnecessary to reproduce it.

## Remaining CI result

The checkpoint CI run [42446708802](https://github.com/loopflowstudio/loopflow/actions/runs/42446708802) deferred the matrix because committed scratch files remain. The first scratch-free candidate must pass the hosted `macos-26` launch step; record its added time here before landing. Local passing launches took 1.1–3.6 seconds in QA and 10.00 seconds after gate's rebuild. Hosted timing is still unmeasured.

The check covers launch, fixture rendering and quit. It does not exercise live registries or Sessions. Signed-package resource validation remains with the release job; the refactor's resource hiding/restoration is covered by focused tests.
