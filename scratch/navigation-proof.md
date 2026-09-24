# Unified navigation evidence

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
