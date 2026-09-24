# Iteration 14 — configured Session picker and nested workspace

The configured app now proves exact Session selection from a multiple-Session
picker, return to its existing provider pane across two checkout groups, retained
PTY processes through Work details, and UI completion without losing those panes.
This is a targeted Accessibility trial with real shared reads and a real provider.
It sends no keyboard input and does not prove unfinished drafts or shell responses.

## Change and authority

The live LOO-291 population includes the human's existing Session. A fresh proof
Session therefore produces a picker instead of a single Session button. Picker
items lacked the stable accessibility identifiers already supplied by individual
rows. Added `session-row-<id>` to those existing menu actions in
`WorkspaceNavigator.swift`. The configured menu exposes both exact identities;
the probe selects only its newly created Session. No title matching, new Session
policy, provider replacement, planning write or ownership change was introduced.

The picker uses AXPress; the checkout picker uses AXShowMenu. Their actual
advertised actions differ. The probe now follows those controls. Source review
also confirms that main's Session DTO still lacks LOO-284's shared actions/display
path; this accessibility correction does not substitute for that integration.

## Passing configured trial

Build: `/tmp/loo291-iteration14/Loopflow Nested.app`, distinct signed disposable
bundle, this checkout's SwiftPM executable and existing resources. Reads and
actions both use `target/debug/lf` and the explicitly aligned installed development
Home documented in the [receipt](configured-ui-evidence/iteration14/receipt.json).
No fixture mode or installed-app replacement was used.

At 2026-09-24T14:24:52Z, owned app PID 20745 was Accessibility trusted, had one
accessible window, and was inactive. All actions addressed elements beneath that
owned app. The [executed probe](configured-ui-evidence/iteration14/nested-probe.swift)
and [passing log](configured-ui-evidence/iteration14/nested.log) record:

1. Created a repository terminal, selected LOO-291, and used New conversation.
   Shared reads discovered `run_522857d1d4b1446fbdd7939a9931e2c4`, attributed to
   LOO-291 with a native terminal ID. Its provider descended from the owned app.
2. Selected that exact Session from the two-item picker. The provider PID/birth
   receipt remained equal, and no transfer action appeared.
3. Added a Task companion, split the outer workspace, restored the main checkout,
   and added its companion: two checkout groups with two panes each.
4. Selected the exact Session from the other checkout. All four panes remained;
   the pane containing its completion action reported selected (`active`). This
   proves pane selection, not system keyboard focus.
5. Visited Work details and returned. The exact provider receipt and all four
   direct PTY child receipts remained equal.
6. Completed only the disposable Session through the UI. The provider exited and
   the shared Session disappeared; all four panes and PTY children remained.

The PTY children are `/usr/bin/login` processes, each with its own tty and recorded
start time. Log assertions calling them shell PID/birth receipts refer to those
PTY children; they are not proof that shell commands responded. Their unchanged
receipts establish retained terminal processes at that boundary.

The passing trial's before/after Session payloads are identical. LOO-291 remains
incomplete, and the human's existing Session remains unresolved. After the owned
app exited, its provider and all four recorded PTY children were absent. No user
Session was opened, moved or resolved.

## Earlier attempts and cleanup

The initial keyboard trial observed the owned app focused, then Chrome frontmost
and system AX focus unavailable (-25212). It stopped before any terminal/provider
action. This is not a missing-permission finding; no permission change is needed
for the passing targeted path. Keyboard/draft proof still needs an uninterrupted
foreground window.

Subsequent failures exposed probe assumptions: inherited toolbar/pane identifiers,
the multiple-Session picker, its numeric AXTitle rather than AXDescription, its
supported menu action, the checkout picker's different action, and direct PTY
children being `login` rather than shell executables. The build initially exposed
the new item IDs in a separate read-only picker inspection before the full pass.
All failed logs and executed scripts remain in the iteration14 evidence directory.
They are not counted as successful trials.

Five earlier proof Sessions were closed when their owned apps exited and then
completed through the shared CLI. The first cleanup command incorrectly included
unsupported `--json`; it made no mutation, and the documented command succeeded.
The sixth Session completed through the passing UI trial. All six identities are
retired; do not replay them. Final reads contain none of them. All recorded owned
app/provider processes and the passing trial's PTY children are absent; see
`receipt.json` and `cleanup.log`.

## Verification and remaining scope

`swift build --package-path swift -Xswiftc -gnone --jobs 4 --product LoopflowMac`
passes. Strict signature verification and the configured probe pass. Source and
binary hashes are retained in the receipt. No source edits followed the passing
trial. `git diff --check` passes. No unit suite or broad gate was rerun for the
one-line accessibility correction; the configured exact-item selection exercises
that change directly.

Review: the existing action owns opening, its identifier carries the existing
Session identity, and the existing workspace retains the panes. No extra reader,
store, lifecycle branch or terminal owner was added. The shared inventory and
owned client receipts support the stated result; neither app activation nor an
active pane label is presented as keyboard-input proof.

The previous editor rejection proof remains an injected transport failure, not a
real PM rejection. An external proving Task and exact directive text were requested
from the human this turn and remain unselected. The authorized edit, ten external
trials, shared LOO-284 integration, keyboard/draft proof across nested groups, and
published paint/readiness budgets with twenty long-lived-registry trials remain
open. No timing series was collected here. No publication, landing or Task
completion occurred.
