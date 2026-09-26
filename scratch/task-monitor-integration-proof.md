# Monitor pane identity and keyboard ownership — 2026-09-24

The concurrent implementation supplies Monitor content, one Podium active-Run
reading and retained Task pane choices. This implement turn preserved that work
and corrected two reproduced integration defects. It added no second layout,
read owner, terminal pool or shared lifecycle.

## Corrections

Task choices now retain the expected `PaneState` alongside checkout placement.
When another Task's Session replaces the same pane, returning to the first Task
cannot restore that unrelated Session merely because the pane ID survived. The
existing initial-Monitor path handles a choice whose pane content no longer
exists. Closing a pane likewise cannot revive it through a stale choice.

Monitor now has a native non-terminal first responder. Clearing terminal focus
to nil had let AppKit choose another visible terminal. The native target requests
focus when its pane becomes focused or attaches; ordinary refreshes do not steal
focus from controls. Removed the ineffective window-pool `clearFocus` method,
its fallback stub and all three callers. Monitor still has no TerminalIdentity.

## Proof

The [before run](monitor-integration-evidence/before.log) reproduced both defects:
Task A returned to Task B's Session, and opening Monitor left a terminal as first
responder. The first compiler attempt failed on a Swift assertion expanding an
optional closure invocation in the new test. Separating unwrap and invocation
fixed the test compilation; that attempt supplies no product verdict.

The [final run](monitor-integration-evidence/verified.log) passes both focused
tests in `TaskMonitorProofTests`:

- Two Task Sessions reuse one pane; returning to the earlier Task restores
  neither the later Task's content nor its selected Session ID.
- A mounted direct Session and companion shell run owned `/bin/cat` PTYs.
  Opening Monitor retains all three panes in the original split tree. Ordinary
  window-dispatched typing while Monitor is focused enters neither terminal.
  Resize, Monitor zoom/unzoom and Session return preserve both exact surfaces,
  restore native Session focus, and produce the retained draft and companion
  replies. Closing Monitor preserves both terminals.

Command: `swift test --package-path swift -Xswiftc -gnone --jobs 4 --filter
TaskMonitorProofTests`. Exit zero; two tests pass. `git diff --check` passes.
The [receipt](monitor-integration-evidence/receipt.json) records source hashes.
The concurrent writer's shell-attached proof and shared active-Run presentation
tests remain separate; the duplicate gap-only assertion was removed from this
suite. No broad gate or Rust rerun was needed for these Swift corrections.

Review focused on independent lifetimes: a pane ID does not identify its current
content, and selected layout state does not establish AppKit input ownership.
Both findings changed the implementation. Native focus stays with the view that
receives it; no additional pool-wide focus controller remains.

## Remaining acceptance

This is native fixture/PTY proof, not configured-provider use or human acceptance.
Monitor still refreshes on opening and request, with explicit observation time;
bounded native discovery and automatic refresh remain core requirements. Both
experience measurement runners, baseline/budgets, the simplified human demo and
the retained external-work trials remain open. The concurrent fallback attempt
stopped at resource preflight; it supplies no fallback compilation verdict.
No publication, installation, PM mutation or Task completion occurred here.
