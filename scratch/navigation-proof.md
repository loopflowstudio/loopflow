# Unified navigation evidence

## Iteration 7 update

See [configured proof](configured-ui-proof.md) for exact provider text, completion,
repository/list retention, matched-population count timings, and the title/contrast
corrections. The missing-Session observation is explained by mismatched Home reads.
The extended native title regression passes both existing completion cases before
the separate lf-new integration. These receipts do not validate the integrated head. The
configured scroll attempt did not establish a moved terminal viewport; later
inactive-app and launch failures remain recorded, not recast as permission errors.

## Observable result

One compact planning list contains autonomous, upcoming, and Session-bearing
Tasks. Work selection inspects without launching a provider. Exact Session
selection uses the existing opening path. Full-width/list-plus-workspace changes
preserve the window's terminal owner, split layout, focus and conversation.
Unavailable readings remain explicit; completing a Session does not complete
its Task.

## Source reconciliation

Read the prepared checkout, canonical main's SessionRecord/SessionsView, the
Rust SessionRecord and roadmap projection, and lf-new's current SessionsView.
The compared Session/view files were identical before this slice. No legal-action
or display-path Session fields exist here yet. Native continuation/Move here
remain the existing path. The separate lf-new checkout was not edited.

## Review findings resolved

- Removed the cascading console and Sessions-only hierarchy read.
- Centralized polling in Podium; publication of Session evidence is independent
  of waiting for planning. Model generations reject old-scope readings.
- Joined Task runtime Work identity explicitly; wrong-kind and planning-id-only
  matches cannot steal a Session. All unmatched human boundaries stay reachable.
- Retained the existing workspace owner and presentation store across repo
  changes. Hiding the work list changes no pane tree or provider.
- Concurrent review found invisible terminals could retain keyboard authority.
  The shared correction disables hidden terminals, releases native focus, and
  avoids stealing focus from search during polling/resize.
- Replaced obsolete hosted cascade checks with compact-list/A-D assertions.
  These test sources have not been run as configured UI proof in this turn.

## Recorded validation

**Iteration 6 configured planning and AX navigation, 2026-09-23:**
The minimal AppKit control exposes one AX window and an onscreen CG window
(`/tmp/loo291-ax-boundary.log`). Launching the current temporary review bundle
with activation requested also exposes workspace controls. This supersedes the
earlier inaccessible-control observation; it does not establish why those
earlier launches differed. No production accessibility change was made.

The real app then showed `Planning unavailable`: the inherited `LF_WAVE_ID`
selected the launching agent's Wave despite `roadmap --all`, and resolution
rejected the app's `/` cwd. Receipt:
`/tmp/loo291-configured-values-probe.log`. The same installed CLI succeeds from
`/` when only `LF_WAVE_ID` is removed (`/tmp/loo291-iteration6-root-roadmap.json`).
LocalWaveAgentLauncher's existing process boundary now removes that variable
after enriching PATH. Home/registry configuration and explicit command targets
are preserved. No shared API, planning scope model or second reader was added.

`swift build --package-path swift -Xswiftc -gnone --jobs 4 --product LoopflowMac`
passes (`/tmp/loo291-iteration6-app-build.log`). The updated executable was copied
only into the existing temporary review bundle, retaining its development
configuration pointing to the installed `lf`. The first configured AX trial
passed with deliberately unrelated `LF_WAVE_ID=loo291-unrelated-launching-wave`:
17 accessible Task rows, exact LOO-291 selection, authoritative directive,
explicit no-Session state, Show/Hide work list, return to overview, exact Task
search and search retention through a 16-second polling interval. Receipt:
`/tmp/loo291-configured-navigation.log`. The accessible-row count is not a
complete portfolio count, and the polling interval is not a latency budget.

This is real LaunchServices/AX interaction against the long-lived registry,
without fixture or capture mode. The subsequent owned provider trial establishes
native continuation and UI completion with companion and Task survival. Exact
draft fidelity remains a gap: provider history caught two missing characters
that the reply-only assertion missed. One launch also exposed no Task controls
within the observation window. [Configured proof](configured-ui-proof.md) records
these failures, per-run permission/window evidence, scoped timings, exact
receipts and the remaining procedure. It supersedes a generic AX-blocked status;
external trials, visual/scroll proof and controlled timing comparisons remain.

**Iteration 5 completion across repository navigation, 2026-09-23:**
The mounted native regression now runs two serialized cases: complete while
viewing the original workspace, and complete after switching repositories. An
AsyncStream holds the mocked completion response until the test releases it.
In the second case, the original native views are detached before success;
cleanup must remove the Session pane and surface before that repository returns.
The active repository retains its own Session reading, selection, pane layout
and focus. Undo cannot restore the completed pane. Returning to the original
repository retains the selected incomplete Task and companion surface, shows
no open Sessions, and receives a reply from the original companion child.

No production changes or test-only production seams were needed. Both cases use
the existing Complete button, repository identity boundary and window registry.
The other repository's Session is explicitly unbound; it is not attributed to
the original Task. The test isolates shared CLI side effects and uses real
AppKit/Ghostty views and local PTYs. It does not establish backend completion,
provider continuation, blocked-caller release or configured UI interaction.

Command: `swift test --package-path swift -Xswiftc -gnone --jobs 4 --filter
WorkspaceNavigationProofTests/workspaceRetainsNativeSplit`. Final receipt:
`/tmp/loo291-completion-repository-proof.log`: one parameterized test passed both
cases, exit 0. No Swift edits followed. This supersedes the iteration 4
mounted-completion/Undo receipt for the expanded test. The independent navigator
scroll proof is unchanged. Configured interaction and same-population timings
remain missing; no broader gate or external-product trial was run.

**Iteration 4 mounted completion, 2026-09-23:**
Extended `workspaceRetainsNativeSplit` through the existing Complete button after
its navigation and child-response assertions. The shared CLI completion response
is mocked; the mounted SwiftUI actions, AppKit window, Ghostty surfaces and cat
children are real. Completion removes the Session reading and pane, releases its
surface, retains the selected incomplete Task, and leaves exactly the companion
pane focused. Task details show the explicit no-Session state. Returning to
terminals focuses the same companion surface, and its original child replies to
new input. No production API or test-only production seam was added.

Final command:

```sh
swift test --package-path swift -Xswiftc -gnone --jobs 4 \
  --filter WorkspaceNavigationProofTests/workspaceRetainsNativeSplit
```

One test passed, exit 0; `/tmp/loo291-mounted-completion-proof.log`. No Swift
edits followed. This supersedes the earlier mounted-workspace receipt for current
test source. No broad gate ran. It proves local completion reconciliation, not
backend completion, provider continuation or blocked-caller release.

Before using this local proof, retried external accessibility through normal
LaunchServices rather than direct executable launch. The temporary review bundle
used the current SwiftPM binary and existing development `lf` configuration, with
no fixture or capture mode. NSWorkspace returned the exact owned process (59815).
The AX probe reported trust but exposed only application/menu elements after ten
seconds; it could not reach workspace controls. `terminate()` returned true and a
subsequent process check confirmed exit. Probe and receipt:
`/tmp/loo291-configured-probe.swift` and `/tmp/loo291-configured-probe.log`.
No user process or Session was changed. This rules out a fix from merely changing
that launch method; it does not establish the underlying accessibility cause.
Configured interaction, visual quality and timing evidence remain required.

**Iteration 3 navigator scroll, 2026-09-23:**
`WorkspaceNavigationProofTests.navigatorRetainsScroll` mounts SessionsView with
80 fixture Tasks, scrolls its real NSScrollView to 1,200 points, opens Task details,
shows the list beside details, refreshes planning, returns to the overview, then
switches repositories and returns. It reads the native clip-view bounds rather
than asserting only on saved model state. A different repository starts at zero.

Before the fix, only repository return failed: the native offset was zero, a
1,200-point difference (`/tmp/loo291-navigator-scroll-before.log`, exit 1).
WorkspaceNavigation now owns the retained offset alongside selection/search;
the view's SwiftUI ScrollPosition is its mounted scroll control, initialized
from the retained offset. Geometry updates capture that render's navigation
owner, so they do not look up a different repository at callback time. No wire
field, disk persistence, launch path or second workspace store was added.

Review checked the ownership boundary: SwiftUI still owns scrolling and clamps
positions when content changes. Retention lasts for the window visit, matching
the other navigation state. No row-ID hierarchy or parallel scroll controller
is introduced. This proof uses native programmatic scrolling and ViewInspector
toolbar actions with fixture planning; it is not external mouse/AX interaction,
configured provider continuation, a live-registry budget or an external trial.

Final command:

```sh
swift test --package-path swift -Xswiftc -gnone --jobs 4 \
  --filter WorkspaceNavigationProofTests/navigatorRetainsScroll
```

One test passed, exit 0; `/tmp/loo291-navigator-scroll-final.log`. No Swift source
or test edits followed. `git diff --check` passes. No broader test gate ran.
The design's configured interaction and before/after timing checks remain open;
this result closes the local navigator retention gap only.

**Iteration 2 mounted-workspace proof, 2026-09-23:**
`WorkspaceNavigationProofTests.workspaceRetainsNativeSplit` now hosts the real
`SessionsView` in an AppKit window with two retained Ghostty surfaces. It invokes
the existing SwiftUI toolbar actions through ViewInspector: show list, Work
details, All work, Return to terminals, hide list. It then switches repository
and reconstructs the view with the same repository identity boundary used by
Podium. At each transition it checks the original surfaces, split layout and
focused pane; hidden terminals relinquish first responder. Returning restores
native focus and selected Work. A Session's unfinished input reaches its same
cat child afterward, and the companion child still accepts input.

The companion emits 120 numbered history rows. The test scrolls to the top and
checks the same first five viewport lines across navigation and repository
return. This checks actual Ghostty scroll position rather than a saved model
value. Initial fixture failures assumed the first row lacked a login banner;
the final fixture retains that banner and emits history from the child to avoid
PTY input echo interleaving with cat output. No production assertion or behavior
was weakened to pass.

Final command:

```sh
swift test --package-path swift -Xswiftc -gnone --jobs 4 \
  --filter WorkspaceNavigationProofTests/workspaceRetainsNativeSplit
```

One test passed, exit 0; `/tmp/loo291-mounted-navigation-final.log`. No Swift
edits followed this pass. The implementation needed no production correction.
Only this new behavioral proof ran; no broad gate was run.

Limits: planning and Session records are fixtures, both surfaces are prepared
before mounting, and the actions use ViewInspector rather than external mouse
or accessibility input. This is native integration evidence, not configured
provider startup/continuation, Session resolution, a paint budget, navigator
scroll retention, or any external-product trial. The configured app interaction
gap recorded in `review-slice.md` remains; no publication or completion follows
from this test.

Iteration 2 review strengthened the companion-child assertion: PTY input echo
alone no longer suffices; the test waits for both cat children to reply after
navigation. The same focused command passed one test, exit 0, in
`/tmp/loo291-review-mounted-child-proof.log`. This supersedes the prior mounted
test receipt for current test source. Production code and the proof limits above
are unchanged.

**Latest implement result:** surface initialization is restored and both
`hiddenTerminalPreservesDraft` and `releaseSurfaceIsWindowLocal` pass with the
real Ghostty artifact and PTYs. CoreVideo rejected display-link creation;
Ghostty mapped that error to OutOfMemory. The app now uses its timer renderer
when that capability is unavailable. Restoring the surface exposed and enabled
fixing the resize/search focus bug. Exact before/after commands and limits are in
`native-surface-diagnostic.md`. The failures below are historical; configured
full-navigation, provider, split/scroll, and external-work proof remain open.

The first combined focused run passed 32 model/view/store tests. Its new native
PTY proof failed before creating a surface; no draft-retention verdict resulted.
The independent native writer subsequently reproduced surface-creation failure
with the pre-existing window-isolation proof. This does not establish native
navigation behavior. The later `native-surface-diagnostic.md` supersedes the
host/rendering hypothesis: an isolated AppKit host reported OutOfMemory despite
screen/Metal availability. Its cause was unknown at that point; the later
CoreVideo diagnosis and passing native receipts are linked above. See `concurrent-proof.md`
for the earlier counterexample and focus-only follow-up.

After the per-repository reading fix, the combined command
`swift test --package-path swift -Xswiftc -gnone --jobs 4 --filter
'WorkspaceNavigationTests|SessionsStoreTests|PodiumModelTests|WorkspaceNavigationProofTests/terminalFocusFollowsVisibility'`
compiled and passed 34 model/view/store tests. Its native focus-only test failed
before the terminal became first responder (the responder remained NSWindow).
The combined command therefore exited with failure; this is not an all-green
receipt. Log: `/tmp/main-view-task-final-proof.log`.

The last edit distinguishes unavailable Work-to-Session association from zero
Sessions and preserves saved expansion while searching. Its focused navigation
result is recorded below.
No broad gate, Xcode fallback build, permissioned UI trial, external-product
trial, performance budget, or full LOO-291 completion is claimed.

Final focused proof:

```text
swift test --package-path swift -Xswiftc -gnone --jobs 4 --filter WorkspaceNavigationTests
9 tests passed; exit 0
```

Log: `/tmp/main-view-task-navigation-proof.log`. This final run includes stable
ranked Task identity, exact typed Session attribution, completed/unmatched
Sessions, A/D and repository state retention, search expansion, last-good reads,
slow planning, completion independent of Task completion, Task directive/KR
rendering, and unavailable-association presentation. This receipt predates the
later selection corrections and focused runs in `review-slice.md`. These are
model/view fixtures, not configured provider trials.

The concurrent native writer removed the additional focus-only fixture after
its hosting failure rather than adding retries. The retained native draft test
subsequently passed after the rendering/focus corrections linked above. This
earlier navigation-only command did not run it and remains model/view evidence.
