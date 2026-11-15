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
Separate investigation: [LOO-911](https://linear.app/loopflow/issue/LOO-911),
local Task `task_3c153eb44d4847c7857e7c221b583d74`, filed and synchronized
November 14; linked back here and from LOO-907's thread. Its brief retains the
observations above and calls for correlating the reported exit and both hangs,
then a reproduction/fix or bounded diagnostics if historical records cannot
identify the event. Neither the exit nor the stalls are attributed to the input
failure. This investigation does not delay the recovery fix.

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

### Delete — do not maintain

Removed last-warmed-store routing from `refreshSession` / `takeBackSession`,
untracked viewer retirement from `GhosttySurfacePool.release`, and unconditional
stale surface-failure callbacks. Their required behavior survives on addressed
repository routing, pool-owned retirement and current-`launchId` callbacks; no
parallel resolver, lifetime owner or compatibility path remains.

The compression pass removes caller-owned generation checks: both surface-failure
callbacks pass their `SessionItem` to `SessionsStore.recordPaneFailure`, which
checks `launchId` once beside the state transition. Focus recovery reuses
`acceptsFirstResponder` for enabled/exited/detaching state. No deletion targets
remain; retirement ordering and addressed-store routing still earn their place.

Competing-viewer iteration deletes Desktop's exit-on-control-loss branch and its
initial Busy failure, plus reveal's refusal of an active Session even when its
held terminal answers. Terminal viewers still leave on control loss. Desktop keeps
the live stream read-only, Refresh attaches passively on Busy, and Move here
requests control on that connection. Toolbar Take back retires read-only panes
before preparing resume, as it already does for ended panes. No protocol changes.

Source defects found and fixed; routing and retirement regressions failed on the
prior code:

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

- **Held conversations had no reveal action (found in QA).** Since #1887 a held
  Task conversation listed only `open`. Desktop restores panes and refreshes with
  reveal intent, and `adoptPreparation` fails a reveal without a `reveal` action:
  a live QA Desktop, killed and relaunched on a private world, showed "Session
  failed to open · Reveal preparation is unavailable" over a running provider.
  `held_actions` now offers `reveal` too, which also gives prepare's reason an
  action to carry. Regression: `a_held_task_conversation_without_a_view_offers_attach`.

- **Take back ignored a displaced viewer (found in QA).** A terminal `lf session
  resume` ends Desktop's view of a held conversation; the pane's relay reads
  `ended` while the store item stays `.live`, so `moveHere` → `prepare` returned
  early. `moveHere` now releases a failed attachment (awaiting retirement) first.
  Regression `WarmPaneMountTests/takeBackReplacesEndedViewer` failed three
  expectations without it.

This held-reveal defect reproduces the reported shape (reopened pane refuses
input, Refresh repeats the same reveal). Whether it was the incident's cause is
not established.
The routing bug matches "Refresh did not help" only when the last-warmed store
differed from the LOO-906 workspace; the focus gap matches "navigation fixed it".

## Checks

- `cargo test -p loopflow --test transport_tests desktop_crash_reopen -- --nocapture`
  — competing-viewer iteration passed: control loss keeps Desktop's output live;
  Refresh remains read-only while the terminal submits; Move here returns input
  and ends that terminal viewer successfully; provider/Session/AgentProcess/held
  terminal unchanged, four submissions once each. The expanded regression first
  exposed reveal's pre-attachment refusal, "This conversation is controlled
  elsewhere; use Take back to continue here". The debug linker warned about its
  oversized `__eh_frame`; gate owns static checks.

- `scripts/test_desktop.sh --filter WarmPaneMountTests` — competing-viewer iteration:
  CLI/Swift built; four tests passed, including both read-only and ended Take back
  cases and overlapping Refresh retirement. Build warnings were in unchanged
  Swift code; mounted focus remains for demo.
- `scripts/test_desktop.sh --filter 'SessionsStoreTests|WorkDestinationTests|WarmPaneMountTests|WorkspacePanesTests|GhosttyTerminalMountTests'` — compression pass: CLI and Swift built; 125 tests passed, four display diagnostics skipped; Refresh rejects stale failures and reports current ones. Mounted checks remain for demo.
- `scripts/test_desktop.sh --skip-build --filter startThenCompetingHolder` — one passed after the failure-callback API cutover.
- `scripts/test_desktop.sh --filter 'ServerConnectionTests|SessionsStoreTests|WorkspacePanesTests'`
  — 51 passed; new action test failed both cases on original routing.
- `scripts/test_desktop.sh --filter refreshWaitsForPausedViewerRetirement` — failed
  four assertions without the retirement await; passes with it.
- `scripts/test_desktop.sh --filter 'WarmPaneMountTests|recoveryAddressesRestoredTask|refreshSameConversation|taskWorkspaceReopenNamesTheTask|GhosttyTerminalMountTests'`
  — six passed on unmounted Ghostty surfaces; four mounted diagnostics skipped
  headless (“Optional display-session diagnostic”).
- `cargo fmt`, `git diff --check` — passed.
- Gate: `cargo clippy --all-targets -- -D warnings`;
  `cargo test -p loopflow --test integration documented_commands::`;
  `cargo test -p loopflow --lib transport::viewer_tests::`;
  `uv run python scripts/test.py --reuse-passing --rust-filter 'binary(transport_tests) & (test(desktop_crash_reopen) | test(a_held_pane_reads_reconnecting_until_the_next_server_answers))'`.
- Demo: `LOOPFLOW_NATIVE_TESTS=1 swift test --package-path swift --no-parallel --filter GhosttyTerminalMountTests`
  (display session, unrun) and the mounted walkthrough above. Headless checks do
  not show mounted typing, real pending-request preservation, or the crash.

`lf open --host desktop --file docs/sessions.md --file docs/troubleshooting.md`
returned `task_7c45fd8c99864a9cbec4fbda9b652429 has no workspace in this Desktop window; nothing changed.`

## QA (November 14, `scratch/qa.html`)

Live QA bundle (`com.loopflow.mac.qa907`, branch Desktop + bundled `lf`) on an
lf-world one-task Machine, driven with `lf desktop send/send-key/capture/do`:
Desktop SIGKILLed three times with the Task open; after the fixes it reopened onto
the same provider (one PID throughout) with the unsent draft, which completed and
submitted once. Refresh repaired an ended pane; Take back after a terminal
takeover moved input back and the terminal said so. Not reached: screenshots
after the first (display stopped returning images), mounted focus/typing through
AppKit, a real provider. Observed for LOO-911: Desktop main thread hung 65 s on
first Task open and 61 s on one Refresh, at load average 160–240; a sampled repeat
did not hang.

QA found that another viewer displaced Desktop into a failed pane; the competing-
viewer iteration (commit `765c7944d5`) repairs that lifecycle and its transport
and Swift regressions pass. QA did not rerun live against it; mounted confirmation
remains for demo.

## Remaining

### Concept review — competing viewers (November 14)

Outcome: advance; no design change or open product choice. The requirement that
another controller keeps input until Take back already settles QA's displacement
question. The current iteration implements that interaction: control loss leaves
Desktop connected read-only; Busy on reopen or Refresh attaches passively to the
answering held terminal. Preparation grants no control. Move here requests it on
the connection; toolbar Take back retires that viewer before replacing attachment.
The existing connection reading derives `.elsewhere` from connected without input;
there is no new Session state, persistence or protocol.

Counterexamples retained: a live competing controller must still submit after
Refresh; loss of control must not conceal later provider output; actual provider
exit must still end the pane; an unavailable holder must not permit duplicate
execution. The expanded transport regression covers displacement, passive Refresh,
explicit takeover and once-only submissions with unchanged execution identity;
the Swift regression covers Take back from both read-only and ended panes. Their
recorded passing results above do not establish mounted acceptance. QA's failed
pane observation remains historical, rather than a current product choice.

Usage docs already describe this behavior; no skill instruction changes are
needed. Next show the mounted competing-viewer walkthrough alongside crash/reopen
and focus recovery. Review page: `scratch/concept-review.html`.

- Gate checks above (add `cargo test -p loopflow --lib a_held_task_conversation_without_a_view_offers_attach` and `scripts/test_desktop.sh --filter WarmPaneMountTests`), then the mounted demo walkthrough.
- Overlap: #1861 (LOO-774, transport endings) merged 2026-11-15 and is this
  branch's base (`a7244b4c2b`, merged in at `d622e6b574`). #1862 (LOO-811, open
  at `3bec58a76f`: provider recovery, Session DTOs, pane presentation) still
  overlaps; whichever lands second rebases over the other. Out of scope: LOO-905
  placeholders, LOO-906 startup speed, provider limits, account switching.

Done when returning to the surviving Task permits editing and one new submission
without navigation; Refresh repairs the same Session or names the real failure;
another controller keeps input until Take back; provider, history, drafts, requests
and Graph execution stay intact.
