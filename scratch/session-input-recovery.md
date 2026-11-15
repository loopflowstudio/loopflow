# LOO-907 — Restore Session input after Desktop returns

## Outcome and demo

A Task's surviving conversation accepts typing when Desktop returns after an
unexpected exit, without selecting another Task and returning. Refresh either
repairs the viewer and restores focus, or shows which part is unavailable with its
action. Session identity, provider history, drafts, pending requests and running
work survive; no submitted input is replayed.

Demo (disposable Desktop workspace): open a Task's native conversation, type an
unsent draft, terminate only Desktop, reopen that Task. Continue the same draft and
submit one marker. Break the viewer and use Refresh; the marker reaches the same
provider once. Repeat with another viewer holding input: the pane stays read-only
until Take back.

Chapter KR: work through a Session without getting blocked by Loopflow plumbing.
No numeric target.

## Report and what inspection found

Task brief, November 14: Desktop crashed while working on LOO-906; the reopened
Session refused input; Refresh did not help; leaving and returning eventually did.
No incident timestamp or stack was supplied.

Read-only inspection on November 14:

- Launch journal's latest launch: `2026-11-15T00:39:32Z`, Desktop 0.15.3, revision
  `f2a581a2a`, PID 11516, launch `B5BB928C-A7C0-44D9-9458-0BFF34B5AFD2`; no later
  launch or exit record. `logs/lf.log` has PID 11516 diagnostics through
  `2026-11-15T02:14:32.971Z`. Nothing here shows a crash/relaunch during LOO-906.
- Same log: `main thread hung ms=12907` at `2026-11-15T01:08:55.691Z` and
  `ms=3495` at `2026-11-15T01:58:39.785Z`. A stall is not a crash signature.
- LOO-906 interactive Session `21885589e5be46cbb4614a36f16c1cdb` (created
  `2026-11-15T00:52:36Z`, AgentProcess `6ca992a2-3956-4eb6-ba4a-9690e2cf351d`,
  held terminal `c97d49d2-f25a-45b6-8129-4f991785a12b`); status observations
  continue past the report. They show neither focus nor delivered input.
- No matching November 14 Loopflow crash report in user/system DiagnosticReports.
  `LoopflowReporterDiagnostic-2026-11-14-091306.ips` exists; its name alone does not
  tie it to this incident. Unified log 17:35–18:05 local: AppKit/TextInputUI
  activity for PID 11516, no crash attribution.

The crash cause and the exact transition that failed in the report remain unknown.

## Architecture

| Responsibility | Source |
| --- | --- |
| Execution and native editor | Holder PTY and provider; Desktop is a viewer (`rust/loopflow/src/transport/README.md`) |
| Input authority | Hosted viewer token and `control` in `transport/hosted.rs`; EOF releases a lost viewer's claim |
| Native readiness | `Event::Return`, `Request::Controlled`, `Reply::Controlled` (`transport.rs`, `transport/hosted.rs`) |
| Desktop input capability | `ServerConnectionReading.acceptsInput` = connected ∧ input ∧ nativeReady (`Services/ServerConnection.swift`) |
| Surface lifetime and focus | `GhosttySurfacePool`, `GhosttyMetalView` (`Services/Ghostty/GhosttyTerminalView.swift`) |
| Refresh | `DesktopActions` `refresh-session` → `SessionsWorkspaceRegistry.refreshSession` → `SessionsStore.refresh` (`Views/SessionsView.swift`) |

Swift paths are under `swift/LoopflowMac/`. Facts that still shape the approach:

- `ServerConnection.canRetry` needs a live status fd and an unavailable reading, so
  the outer Refresh must stay usable when the status stream itself is gone.
- The relay publishes native readiness after sending Controlled and ignores
  `Reply::Controlled`. The transport regression below types immediately after
  `native_ready` and loses nothing, so the relay is unchanged; no ack correlation
  was shown to be needed.
- Fresh hosted attachments already resend Return when the native client exists.

## Built (commit `55f3f8274c`)

Source defects found and fixed, each with a headless regression that failed first:

- **Routing.** `refresh-session` / `take-back-session` acted on the last-warmed
  store, not the addressed Work. They now resolve the addressed workspace's
  repository store (`SessionsWorkspaceRegistry.store(for:)`).
- **Retirement race.** `GhosttySurfacePool.release` spawned an untracked 500 ms
  retirement, so Refresh prepared its replacement while the predecessor still held
  the viewer. The pool now retains that job; `SessionsStore.refresh` awaits it,
  overlapping releases chain, and `launchId` drops a superseded completion.
- **Stale failure callbacks.** `onSurfaceFailed` (store and `SessionPaneView`)
  ignores callbacks whose `launchId` is no longer current.
- **Focus on recovery.** `showConnection` restores focus on a paused → usable
  transition, and surface creation does too; only a requested, enabled, visible,
  non-exited, non-detaching view with no open Find bar, no marked text and no other
  first responder gains it. Retiring and destroyed views drop focus first.

Not changed: provider execution, relay protocol, schema, persistent readiness.
Refresh stays non-replacing; Busy stays read-only with Take back.

Docs: `docs/sessions.md#when-desktop-loses-its-connection`,
`docs/troubleshooting.md#desktop-reopened-but-the-session-will-not-accept-typing`,
`swift/README.md`.

Which of these defects, if any, produced the reported failure is not established.
The routing bug matches "Refresh did not help" only when the last-warmed store
differed from the LOO-906 workspace; the focus gap matches "navigation fixed it".

## Checks

- `scripts/test_desktop.sh --filter 'ServerConnectionTests|SessionsStoreTests|WorkspacePanesTests'`
  — 51 passed; new action test failed both cases on original routing.
- `scripts/test_desktop.sh --filter refreshWaitsForPausedViewerRetirement` — failed
  four assertions without the retirement await; passes with it.
- `scripts/test_desktop.sh --filter 'WarmPaneMountTests|recoveryAddressesRestoredTask|refreshSameConversation|taskWorkspaceReopenNamesTheTask|GhosttyTerminalMountTests'`
  — six passed on unmounted Ghostty surfaces; four mounted diagnostics skipped
  headless (“Optional display-session diagnostic”).
- `cargo test -p loopflow --test transport_tests desktop_crash_reopen -- --nocapture`
  — passed: draft kept, submissions once, same Session/AgentProcess/held
  terminal/provider, competing non-replacing attach Busy.
- `cargo fmt`, `git diff --check` — passed.
- Gate: `cargo clippy --all-targets -- -D warnings`;
  `cargo test -p loopflow --test integration documented_commands::`;
  `uv run python scripts/test.py --reuse-passing --rust-filter 'binary(transport_tests) & (test(desktop_crash_reopen) | test(a_held_pane_reads_reconnecting_until_the_next_server_answers))'`.
- Demo: `LOOPFLOW_NATIVE_TESTS=1 swift test --package-path swift --no-parallel --filter GhosttyTerminalMountTests`
  (display session, unrun) and the mounted walkthrough above. Headless checks do
  not show mounted typing, real pending-request preservation, or the crash.

`lf open --host desktop --file docs/sessions.md --file docs/troubleshooting.md`
returned `task_7c45fd8c99864a9cbec4fbda9b652429 has no workspace in this Desktop window; nothing changed.`

## Remaining

- Gate checks above, then the mounted demo walkthrough.
- Crash follow-up: the brief asks for a concrete linked follow-up when the crash is
  a separate defect. No crash signature exists (see `scratch/questions.md`).
- Overlap: #1861 (LOO-774, open at `07a21c5956`: transport endings,
  `connection.rs`, `ServerConnection.swift`) and #1862 (LOO-811, open at
  `753d49e7e2`: provider recovery, Session DTOs, pane presentation), unchanged as of
  this pass. Whichever lands second rebases over the other. Out of scope: LOO-905
  placeholders, LOO-906 startup speed, provider limits, account switching.

Done when returning to the surviving Task permits editing and one new submission
without navigation; Refresh repairs the same Session or names the real failure;
another controller keeps input until Take back; provider, history, drafts, requests
and Graph execution stay intact.
