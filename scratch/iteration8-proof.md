# Iteration 8 — completion rejection and configured launch boundary

Review follow-up: the desktop became available and the signed build passed
configured New conversation, exact provider draft retention, UI completion and
original-shell survival. See the [current review](review-slice.md) and
[receipt](configured-ui-evidence/iteration8/review/provider.log). The Session
created by this single-use probe is now completed; **do not rerun the archived
probe**. Earlier locked-screen attempts below remain historical evidence.

## Implemented

Rejected Complete now leaves the terminal live and keeps its error visible through
inventory reconciliation. `SessionItem.completionError` is local presentation in
the existing SessionsStore, separate from opening failure. Retry clears that
error; successful completion removes the item. The shared Session DTO, process
ownership, legal actions and persistence are unchanged. Existing shell completion,
multiple attached Sessions and direct Session cleanup remain intact.

The review failure exposed a concrete conflation: `complete` stored rejection as
`.failed`, while `reconcile` replaced that state with `.live` for a locally attached
client. The strengthened existing native test reconciles the shared inventory
after rejection and checks the visible error, retry availability, live state and
same responding PTY. It fails before correction and passes afterward. A repeated
identical model reading alone did not reproduce this: SwiftUI's onChange compares
record values. The final regression directly exercises its existing reconciliation
owner. This establishes the production failure mechanism without claiming every
earlier intermittent ViewInspector failure has the same cause.

## Focused proof

```sh
swift test --package-path swift -Xswiftc -gnone --jobs 4 --filter 'WorkspaceNavigationProofTests/(shellSessionCompletion|workspaceRetainsNativeSplit)'
```

Two tests/four cases pass, exit 0. [Before](configured-ui-evidence/iteration8/rejection-before.log)
reproduces the missing rejection message; [after](configured-ui-evidence/iteration8/rejection-after.log)
also covers direct completion while viewing and after repository navigation.
These use real AppKit/Ghostty PTYs and fixture Session records with mocked CLI
completion. They do not prove backend completion or provider continuation.
No Swift edits followed the passing run. No affected-suite/full gate ran.

Review checked that polling cannot alter action-error lifetime, completion never
converts a live terminal into an opening placeholder, retry uses the existing
shared operation, and removal clears the error with the Session. The optional
field is UI state, not a wire default or another lifecycle owner. README now
states the rejection/retry behavior. No unrelated API or FlowStep policy changed.

## Configured boundary

Prepared `/tmp/loo291-iteration8/Loopflow Proof.app` from the existing disposable
integration bundle and this checkout's newly linked SwiftPM executable. It has
a distinct bundle identifier and the same installed development CLI/Home as prior
configured proof. No installed app was replaced. [Binary/source receipt](configured-ui-evidence/iteration8/binary.json).

The initial two attempts exposed one AX window but remained inactive and could
not find the real Task control within twenty seconds. The second attempt recorded
normal activate/AXRaise results and its accessible tree; it exposed only the
application/menu boundary. No New conversation action was reached.

Verification found the copied bundle's signature invalid after replacing its
executable. Re-signed only this disposable bundle ad hoc; `codesign --verify
--deep --strict` passed. Its next attempt still failed activation: AX trusted,
AXWindows success/count one, `activate` false, AXRaise -25206, and no Task control.
That correction therefore did not establish the cause of the inaccessible UI.
The executable hash in the receipt distinguishes pre-signing and signed artifacts.

A subsequent read-only console check established `CGSSessionScreenIsLocked = 1`
and `kCGSessionLoginDoneKey = 1`, with AX trust true. This is a locked desktop,
not evidence that Accessibility permission must be changed. No permission was
changed and no unlock was attempted. [Console receipt](configured-ui-evidence/iteration8/console-state.log),
[final probe](configured-ui-evidence/iteration8/provider-probe.swift),
[final launch receipt](configured-ui-evidence/iteration8/provider.log).
The check establishes the current host boundary; it does not retroactively prove
the reason for every earlier launch failure.

All three owned app PIDs (28259, 46912, 65817) exited and were absent from ps.
No new provider was launched, no owned Session receipt was created, and no existing
Session was opened, moved or completed. [Final state](configured-ui-evidence/iteration8/final-state.json).
The three failed launches are retained separately; none is a successful timing
sample. Their underlying signature defect is fixed in the reviewable artifact.

## Prepared procedure — executed by the subsequent review

The following procedure was prepared before the successful review trial above.
It is retained as history, not an instruction to replay its completed identity.

Unlock the Mac's existing desktop normally. Accessibility is already granted to
the exact probe runner; no new permission request is indicated by this evidence.
Then run `xcrun swift /tmp/loo291-iteration8/provider-probe.swift` once from this
Task checkout. The probe launches only its disposable bundle, selects LOO-291,
presses New conversation, and requires a new exact shared Task Session plus a
provider PID descending from its owned app. It preserves the client through
navigation, verifies exact submitted provider-history text, completes only that
owned Session, and checks the original launch shell still responds. It stops on
missing ownership, permission or focus evidence. If it launches a Session and
later fails, inspect its saved exact identity before any retry; do not replay
against another matching title or a completed conversation.

Record successful interaction and defined paint/readiness endpoints against the
signed binary before reviewing publication. Configured terminal scrolling,
shared LOO-284 integration, directive editing, human-selected external trials
and published budgets with required sample sizes remain outstanding. Earlier
provider and count-timing receipts retain their pre-integration binary identities.
This implement pass closes the bounded rejection defect, not full LOO-291.
No publication, landing, Task completion or PM write occurred.
