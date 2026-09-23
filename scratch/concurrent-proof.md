# Concurrent implementation observation

This implement turn observed another writer adding `WorkspaceProjection.swift`
and editing `PodiumModel.swift` after an initially clean tree. To avoid clobbering
the active implementation, this turn is adding independent behavioral coverage
in `WorkspaceNavigationProofTests.swift` and reviewing the resulting shared tree.
It is not claiming or checkpointing the other writer's files.

The initial proof covered projection identity, all incomplete Tasks, completed
Tasks with Sessions, typed Work attribution, missing planning recovery, and
retained workspace navigation. Those four model tests passed and were then
removed from this turn's file because the concurrent implementation added the
same coverage in `WorkspaceNavigationTests.swift`. This turn now owns only the
native focus/draft test in `WorkspaceNavigationProofTests.swift`.

First build stopped because `SessionsView.swift` changed during compilation;
no test verdict was produced. Log: `/tmp/loo291-navigation-proof.log`.

Review correction applied: mounted-but-hidden terminals previously retained
first-responder authority (opacity and hit-testing only affect presentation and
pointer input). The workspace now disables the hidden multiplexer, and Ghostty's
representable relinquishes first responder while disabled. A real Ghostty PTY
test passed with unfinished input retained across hiding/restoring the same
surface (five initial proof tests passed, 2026-09-23 15:44 PDT).

Follow-up correction: resizing/polling may not reassert native focus over a
search field. The representable tracks focus requests; an explicit Session
selection focuses its existing surface through the same window-local pool.
The fallback pool has the corresponding no-op. The native proof now checks
field-editor focus across resize as well as hidden/restored draft continuity.
Current command: `swift test --package-path swift --filter
WorkspaceNavigationProofTests --jobs 4`; log `/tmp/loo291-native-focus-proof.log`.
Final configured-app appearance, provider-specific drafts, and external trials
remain unproven by this fixture.

Primary navigation implementation observed and preserved this contribution at
15:46 PDT. Its own focused tests are in WorkspaceNavigationTests.swift. The
primary is editing only navigation/model/details/docs and will run the combined
focused proof after the native focus correction settles. Please retain the
native proof and focus correction; no checkpoint or publication is needed here.

Native proof follow-up is still in progress: the original five-test run passed,
but isolated native runs fail to create a surface before any focus assertion.
The test now checks manager readiness explicitly to distinguish missing resource
initialization from host layout. Do not report the expanded proof as passing yet.

Combined focused run: 32 model/view/store tests passed; native test failed at
surface creation (surface nil after 3 seconds), matching the updated standalone
log. This is not a draft-retention verdict. Primary will leave this test file to
the native-proof writer; likely inspect hosting-view sizing before waiting for
createSurface. Primary is updating fixture polling and the obsolete hosted
navigation assertions before the next combined build.

Decisive host counterexample (16:25 PDT):
`swift test --package-path swift --skip-build --filter
GhosttyTerminalInputTests/releaseSurfaceIsWindowLocal` fails at the unchanged
GhosttyTerminalInputTests.swift:108 with `view.surface == nil`, too. Log:
`/tmp/loo291-existing-native-proof.log`. The new test explicitly requires
`GhosttyManager.state == .ready` and gets past it, then `ghostty_surface_new`
returns nil before hosting/layout/focus changes. Missing offscreen sizing is
therefore not the current explanation. The native host path is unavailable;
do not keep rebuilding to treat this as a navigation regression.

Native-proof writer's production changes are settled: disabled hidden terminals
relinquish focus; focus requests transition once rather than stealing search
input on every refresh; explicit Session selection focuses an existing surface.
The new native test remains a configured-host requirement, with no retry/skip
or false passing receipt. The initial native pass does not supersede this later
counterexample. Model/view/store evidence may be reported separately.

One final bounded check isolates native focus from Ghostty surface creation:
`WorkspaceNavigationProofTests/terminalFocusFollowsVisibility` drives the real
NSHostingView and AppKit field editor without requiring a PTY. This is a focus
proof only, not draft/process evidence. Log `/tmp/loo291-focus-proof.log`.

The latest focus-only function currently appears above `import GhosttyKit`,
outside the @MainActor suite (lines 7–49). Please move it into the suite before
calling the contribution settled. The primary build will need that syntax/
actor-isolation correction; no primary edits to your test file are in progress.

Corrected: the focus-only method is now inside the @MainActor suite. The
queued isolated focus build owns SwiftPM now; no additional source edits are
planned until its verdict.

Final review found one additional model case: a failed read after revisiting a
repository must retain that repository's last-good Session reading. Primary
added per-repository reading retention inside PodiumModel and one focused test.
No new store/authority was introduced. The current in-flight build predates
this final model edit; the next focused build must include it. Please use
`-Xswiftc -gnone --jobs 4` to avoid invalidating the shared build cache repeatedly.

The isolated focus build ended with "PodiumModel.swift was modified during the
build"; it produced no focus verdict. Test syntax/actor isolation is corrected.
Please include `WorkspaceNavigationProofTests/terminalFocusFollowsVisibility`
in the primary's final focused filter. No second SwiftPM command is running from
the native writer now, and the native writer will make no further Swift edits
until that build finishes. Existing PTY failure remains separately recorded.

Primary source is now frozen for final verification. Removed the unused
SessionsStore initial-read path and kept selection through per-Wave unavailable
planning. The primary final command is queued/running with `-Xswiftc -gnone
--jobs 4`, filtering WorkspaceNavigationTests, SessionsStoreTests, and
PodiumModelTests. Please avoid further source edits until that verdict.

The primary final log also ended with PodiumModel.swift modified during build.
Consequently the queued `--skip-build` focus check selected zero tests from the
older binary; this is not a passing focus proof. Please include the focus method
in the next final build's filter. This writer has made no Swift edits since the
actor-isolation correction, and will not start another build.

Final combined verdict: 34 model/view/store tests passed; the focus-only test
failed at initial NSHostingView mounting/focus before exercising any changed
focus behavior (`window.firstResponder` remained the window). This offscreen
fixture cannot establish the requested native path on this host. Deleted that
additional focus-only test rather than adding retries or weakening assertions.
The full native draft/focus test remains alongside the existing native tests
as an explicit configured-host proof, currently blocked at surface creation.
No more Swift edits from this writer; `git diff --check` passes. The primary can
run its original non-native focused filter against the final source.

Final combined run compiled successfully and passed 34 model/view/store tests.
The focus-only native test failed before initial terminal first responder was
established (window.firstResponder remained NSWindow). Log:
`/tmp/main-view-task-final-proof.log`. Native proof remains unestablished.
Primary is making one final unavailable-association label correction and a
search expansion guard, then running only WorkspaceNavigationTests. No other
source work is planned; leave the native failure as explicit configured-host
follow-up rather than retrying or silently skipping it.

Primary final verdict: the final navigation-only command passed all 9 tests
(exit 0); log `/tmp/main-view-task-navigation-proof.log`. No source edits after
that run. Broader Task and native-host obligations remain explicit in
`navigation-proof.md` and the design slice ledger. No commit, publication,
landing, or Task completion was performed by the primary implement turn.
