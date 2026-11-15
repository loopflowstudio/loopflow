# LOO-904: Files and conversation recovery

Jack Heart supplied a Desktop screenshot showing LOO-903 with Files spinning under
“Changed files” and a global “Conversation change unconfirmed” / Inspect notice.
Asked which issue to address, he replied: “first and third”.

Trace the initial Files read and conversation-reset recovery. Files must show scratch
and changes or a useful error. Recovery must explain the affected conversation and
provide a way to resolve an inspected interruption without replaying a reset.

Observed on November 14:
- Direct live-server `task.changes` for LOO-903 returned its scratch inventory in
  0.04 seconds. The screenshot's stalled state has not been reproduced natively.
- The only saved submission belongs to manabot's November 10 Session reset.
  Its server log ended with `dispatch is still busy; retry after the current send
  finishes`. The receipt remains at `stopping`; absence of a live provider cannot
  safely authorize another reset.

Remaining: native review of actual pane switching and initial Files population,
plus notice context/dismissal in
Light and Dark. Headless view inspection covers both color schemes; it does not
establish mounted visual acceptance.
- At 18:07 the Machine load average was 285; the active shared Swift compilation
  had received 22 seconds of CPU over 9.5 minutes wall time. Validation is still
  running, with no compiler error reported at that observation.

Checks:
- `swift build --build-tests --package-path swift`: passed after fixing the new
  tests to use the existing decode-only JSON fixture. Existing TaskWorkView and
  test warnings remain outside the patch.
- `scripts/test_desktop.sh --skip-build --filter 'TaskFilesTests|CreationSubmissionTests'`:
  41 tests passed; the optional display diagnostic was skipped. This includes
  Light/Dark retry and dismissal controls. Native walkthrough remains for review.

## Duplicate connection labels

Jack Heart supplied a second screenshot and asked: “another issue is what are
these New Conversation things”. It shows the same transition explanation under
TaskWorkflowBar and again inside Ghostty's connection bar. Both are app-owned.
Remove the global duplicate and keep the terminal connection bar quiet for normal
started/retained/resumed transitions. Forks, leaving a previous provider thread
behind, and connection/input problems still need a notice.

Checks for duplicate labels:
- `swift build --build-tests --package-path swift`: passed.
- `scripts/test_desktop.sh --skip-build --filter 'ServerConnectionTests|ProviderTransitionTests|CreationSubmissionTests'`:
  20 tests passed. Normal ready connections have no banner; forks, replacement,
  opening, read-only, reconnect, and failed-status paths remain covered.
- Native Light/Dark walkthrough remains for review; the running app was not replaced.
