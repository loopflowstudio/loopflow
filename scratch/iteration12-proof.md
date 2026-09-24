# Iteration 12 — directive editing and configured host boundary

Task details now offer **Edit directive**. The sheet captures the selected
Task/Wave, submits its text through the existing `lf pm task update --id …
--wave … --notes=…` operation, and displays the refreshed shared roadmap after
success. Cancel discards the sheet's local draft. A failed save leaves the
sheet and draft in place; an accepted update followed by an unavailable read
has a distinct error. The Save button and editor are disabled during submission.

Podium remains the planning reader. Its read generation prevents an in-flight
poll started before Save from restoring the prior directive. No optimistic
planning copy, additional durable state, PM writer or Session policy was added.
The editor keeps its captured target if navigation changes during the operation.

## Focused proof

```sh
swift test --package-path swift -Xswiftc -gnone --jobs 4 \
  --filter 'TaskDirectiveEditorTests|WorkspaceNavigationTests/inspectorShowsPlanning'
```

Three tests pass, including two error cases: rejected write and accepted write
with unavailable refreshed planning. They prove captured targeting, exact
multiline text transport (including a leading `--`, quotes and shell characters),
retained last-good planning on error, retry, rendering the provider-returned
directive rather than the submitted text, and stale-poll suppression. The
original Task-directive/Project-proof view test also passes.
[Receipt](configured-ui-evidence/iteration12/directive-tests.log).

The first build caught a test access-level error, which was corrected. An
attempted hosted ViewInspector test then failed to inspect the editor's mounted
`@State`. The library requires inspection hooks in the production view for that
approach. Removed the invalid fixture rather than adding such hooks or moving
production state for the test. The final proof tests the operation and rendered
planning; **interactive sheet behavior, draft retention after rejection and
visual quality still require configured UI confirmation**. There was no real
Linear write or authorized external edit trial in this pass.

Review checked the CLI's existing ownership resolution, provider update and
snapshot refresh. Passing notes as one `--notes=` argument preserves text that
begins with option-like characters without shell interpolation. Errors after
provider mutation are not proof that the remote directive stayed unchanged.
The existing CLI reports them; the sheet retains the user's draft. Captured
identity and read generations cover separate navigation and polling races.
No broader gate ran; no executable edits followed the passing focused command.
The SwiftPM app build and `git diff --check` also pass.

## Configured attempt

Built and signed a disposable `/tmp/loo291-iteration12/Loopflow Trial.app`
from the then-current app source. The exact proof runner was AX trusted and
observed one accessible window in owned PID 22294. Activation returned false,
window raise returned -25206, and system AX focus belonged to PID 395,
`loginwindow`. `IOConsoleUsers` independently reported
`CGSSessionScreenIsLocked=Yes`. The probe stopped before any UI action or input;
it launched no provider and requested only its own app's termination. That PID
was absent afterward. Human demo PID 41489 and Codex PID 37189 remained alive.
[Log](configured-ui-evidence/iteration12/provider.log),
[receipt and binary identities](configured-ui-evidence/iteration12/receipt.json).

All Home/database keys, including `LF_DB_PATH`, were explicitly aligned with
the installed development Home. This was not the earlier mixed-Home condition,
an Accessibility permission failure, or proof of a navigation defect. No
permission was changed or bypassed. Do not rerun the single-use probe unchanged:
its output-directory guard rejects reuse, and future trials need fresh receipts.
Configured row return, nested checkout interaction and associated timings remain
unproven. The earlier bounded provider/viewport/timing passes retain their scope.

## Review build and remaining procedure

The later editor build is signed at
`/tmp/loo291-iteration12-editor/Loopflow Editor.app`. It has its own bundle id and
does not replace the installed app or running human demo. It has not been
launched. After unlocking the Mac, launch it with the aligned Home environment:

```sh
xcrun swift /tmp/loo291-iteration12-editor/open.swift
```

Inspect **LOO-291 → Edit directive** and cancel to review appearance without
writing. For the actual write trial, use the human-selected external Task and
approved text; neither has been selected by this pass. Check rejection preserves
the exact draft, then verify one authorized save against refreshed shared
planning. Use a new disposable conversation for the Session-row/nested-workspace
procedure in `iteration10-implement.md`, preserving all existing human Sessions.
This runner needs an unlocked desktop and foreground ownership, not another
Accessibility grant.

The complete Task still requires shared LOO-284 actions/display-path integration,
configured editor and nested-workspace proof, the selected external work and edit
round trip, and published paint/interaction budgets with the required trials.
No publication, landing or Task completion occurred. Existing other-writer edits
were preserved; this pass added only the editor/query/read-generation change,
its focused tests, README guidance and these proof records.
