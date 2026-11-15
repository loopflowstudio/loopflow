# LOO-907 — Restore Session input after Desktop returns

## Outcome and demo

A Task's surviving conversation accepts typing when Desktop returns after an
unexpected exit. Selecting another Task and returning is unnecessary. Refresh
either repairs the attachment and restores focus, or explains which part is
unavailable and offers the appropriate action. Session identity, provider history,
drafts, pending requests and running work survive; no submitted input is replayed.

Demo: in a disposable Desktop workspace, open a Task's native conversation, type
an unsent draft, terminate only Desktop, and reopen that Task. Continue editing the
same draft and submit one new marker. Break the viewer connection and use Refresh;
the marker reaches the same provider once. Repeat with another viewer holding
input: the pane stays read-only until Take back is chosen.

The Infrastructure chapter's relevant KR is being able to work through a Session
without getting blocked by Loopflow plumbing. There is no numeric target for this
Task and no new UI performance path.

## Report and investigation

The Task brief records a November 14 report: Desktop crashed while working on
LOO-906; the reopened Session refused input; Refresh did not help; leaving and
returning eventually did. The brief supplies no incident timestamp or stack.
That sequence is the requirement, not a diagnosis.

Read-only inspection on November 14 found:

- The checkout at `3fbf3d2ca3` had no implementation changes or prior scratch plan.
  Two earlier auto runs stopped at kickoff; the current Graph is at kickoff.
- The launch journal's latest launch is `2026-11-15T00:39:32Z`, release Desktop
  0.15.3, revision `f2a581a2a`, PID 11516, launch
  `B5BB928C-A7C0-44D9-9458-0BFF34B5AFD2`. There is no subsequent launch or exit
  record in that journal. `logs/lf.log` has PID 11516 diagnostics through
  `2026-11-15T02:14:32.971Z`. This does not establish an app crash/relaunch during
  LOO-906 and must not be reported as one.
- The same log reports `main thread hung ms=12907` at
  `2026-11-15T01:08:55.691Z` and `main thread hung ms=3495` at
  `2026-11-15T01:58:39.785Z`. A stall is not a crash signature or a demonstrated
  cause of the dead input.
- The LOO-906 interactive Session is `21885589e5be46cbb4614a36f16c1cdb`, created
  `2026-11-15T00:52:36Z`, with AgentProcess
  `6ca992a2-3956-4eb6-ba4a-9690e2cf351d` and held terminal
  `c97d49d2-f25a-45b6-8129-4f991785a12b`. Status observations for that Session
  continue after the report. Those observations establish neither keyboard focus
  nor successful input delivery. No command attached to or modified it.
- The searched user/system DiagnosticReports contain no matching November 14
  ordinary Loopflow crash report. The user directory contains a retired
  `LoopflowReporterDiagnostic-2026-11-14-091306.ips`; its name alone cannot connect
  it to this incident. A unified-log read for 17:35–18:05 local time returned
  AppKit/TextInputUI activity for PID 11516, not a crash attribution.

The crash cause and exact failed transition remain unresolved. Implementation
starts with a deterministic lifecycle reproduction; do not ship a speculative
focus reset as the incident fix. If a distinct crash signature is found, file a
linked Task with that signature and reproduction. If none is available, retain
the unknown cause in delivery notes rather than inventing a follow-up diagnosis.

## Architecture that constrains the change

| Responsibility | Existing source |
| --- | --- |
| Execution and native editor | Holder PTY and provider; Desktop is a viewer (`rust/loopflow/src/transport/README.md`) |
| Current input authority | Hosted viewer token and `control` in `transport/hosted.rs`; EOF releases a lost viewer's claim |
| Native readiness | `Event::Return`, `Request::Controlled`, and `Reply::Controlled` in `transport.rs` / `transport/hosted.rs` |
| Desktop input capability | `ServerConnectionReading.acceptsInput`: connected AND input AND nativeReady (`Services/ServerConnection.swift`) |
| Surface and mount lifetime | `GhosttySurfacePool`, `GhosttyTerminalMount`, `GhosttyMetalView` (`Services/Ghostty/GhosttyTerminalView.swift`) |
| Refresh | `DesktopActions.refresh-session` → `SessionsWorkspaceRegistry.refreshSession` → `SessionsStore.refresh` (`Views/SessionsView.swift`) |
| Workspace restoration | `SessionsWorkspaceRegistry` warm intent and Work-keyed panes; `_terminal` in `Views/WorkspacePanesView.swift` |

Paths in the final four rows are under `swift/LoopflowMac/`.

Important findings:

1. Refresh already releases the pooled viewer, changes `launchId`, then selects
   the same Session with non-replacing reveal. It is not merely a metadata reload.
   Its async method finishes at preparation, before live readiness and focus.
2. `ServerConnection.canRetry` permits retry only with a live status fd and an
   unavailable reading. A connected-but-not-native-ready pane can remain Opening;
   status loss without a live fd has no in-channel retry. The outer Refresh must
   remain usable in both cases.
3. `GhosttyMetalView.updateFocus` acts when the requested focus changes;
   `viewDidMoveToWindow` handles a pending request. `showConnection` toggles
   Ghostty readonly state but does not reconcile focus. Navigation remounts or
   changes focus, so it is a useful counterexample, not yet the cause.
4. The relay currently publishes native readiness after sending a Controlled
   request for a Return event; it ignores `Reply::Controlled`. Write success and
   server acknowledgement are different facts. Test the acknowledgement ordering
   before changing this protocol; do not infer readiness from a snapshot/title.
5. Fresh hosted attachments already resend Return when the native client exists,
   including a new attachment that catches up from a byte position. Merely adding
   another Return on every reconnect does not address a demonstrated omission.
6. Existing `a_held_pane_reads_reconnecting_until_the_next_server_answers` covers
   server replacement and readiness. It does not kill/recreate Desktop or verify
   actual typed bytes. `ServerConnectionTests` cover socket readings, not mounted
   AppKit focus. Ghostty mount tests require a display and are skipped headless.

Historical Wave memory describes per-Session owners; the current implementation
uses the shared server and holders. Keep the memory's no-replay, exact-control and
store-independent byte-path obligations, using the current implementation.

## Chosen approach

Repair the existing viewer lifecycle end to end. Keep one live attachment reading
and one surface owner. Saved Work/layout and Session rows identify what to open;
they never imply input readiness. No schema migration, new recovery service,
provider restart, or persistent readiness record is needed.

The surface pool owns retiring viewers through its existing 500 ms leave grace.
Refresh awaits that same job before preparation; overlapping Refresh requests share
it, and `launchId` rejects a superseded completion. This is ephemeral cleanup,
not another readiness or input-control record.

Opening after relaunch, explicit Refresh, and navigation return must converge on
the same current surface and capability. Identify the failing ordering in the
first slice, then repair its owning layer:

- In the relay, correlate native readiness with the current attachment's
  acknowledgement if the regression demonstrates the existing send-only sequence
  is insufficient. Reconnection revokes old readiness; late responses cannot
  enable a replacement surface. Keep changes within existing wire concepts.
- In Desktop, resolve Refresh against the addressed Session and Machine, and
  follow its new `launchId` through connection recovery. Show preparation,
  attachment/readiness and failure truthfully rather than treating preparation as
  a successful recovery. Ignore completion from a replaced or closed viewer.
- Reconcile AppKit and Ghostty focus when the selected pane becomes usable after
  recovery. Only the currently mounted, enabled, selected pane may gain focus;
  background warm panes and retired mounts cannot. Preserve search-field and IME
  ownership during ordinary updates. No timer that repeatedly steals focus.
- Keep Refresh non-replacing. Busy remains read-only with Take back; an exited
  provider offers existing same-Session resume behavior. Missing readiness stays
  visibly paused with Refresh available; an unavailable server names its existing
  recovery action. A failed status stream must never default to usable input.

Do not add an automatic retry of submitted bytes. A new viewer checks current
claims; it does not silently take control from a surviving client. Repairing a
viewer must not run a Task, advance its Graph, or choose a new provider/account.

Alternatives rejected: reopening the provider loses the surviving editor and can
duplicate work; clearing the entire workspace pool disrupts unrelated Sessions;
forcing focus on every render breaks search and typing elsewhere; declaring
readiness from painted history hides the failure. A broad transport redesign is
unnecessary for this outcome.

## Remaining verification and delivery

### Concept review — November 14

No design change or open product choice. The interaction remains: reopen the
Task and continue typing; Refresh repairs its viewer; Take back deliberately
moves input. Retry connection is the existing narrower connection repair and
needs no new concept or API. No skill instruction changes are needed.

The largest mismatch was treating the displayed or prepared pane as a usable
conversation. Keep preparation, current input capability and keyboard focus
distinct internally, without making people manage them. The implemented change
routes actions through the addressed Work's repository store, retains viewer
retirement in the surface pool and fences replaced callbacks with `launchId`.
The existing live reading still determines usable input; readiness introduces
no persisted state. The provider and holder retain execution and the editor.

Normal path: select the restored Task; the current mounted pane accepts typing
when its live reading permits it. Failure: a missing status stream pauses input,
and Refresh remains available outside that stream. Recovery: Refresh retires the
old viewer, reveals the same Session and reconciles selected focus when usable.
Counterexamples remain required: another controller keeps input until Take back;
Find, IME and another editor retain focus; overlapping Refresh and late callbacks
cannot revive an old viewer; provider exit offers history continuation without
promising its lost editor. None calls for a new recovery record or execution owner.

Next show the disposable mounted crash/relaunch and Refresh walkthrough already
specified below. Headless regressions establish routing, retirement and transport
continuity separately; they do not establish mounted typing or explain the
reported crash. This review ran no checks and changed no implementation.

The implementation stays in one PR on the current Task. Gate owns the standard
CI matrix, Linux coverage, Clippy and documented command checks. The existing
server-replacement fixture remains a useful gate check; this change adds no
transport protocol or provider execution path.

Mounted acceptance remains the disposable Desktop walkthrough above: kill only
Desktop, reopen the Task, edit the surviving native draft, submit one marker,
and repeat Refresh with a competing controller. Real pending-request preservation
and the original crash are not established by stand-in or headless tests.
The production mount diagnostics cover delayed readiness, Find focus and retired
viewers; their headless skips are not mounted acceptance.

Delete — do not maintain:

- The last-warmed store as the target of `refreshSession` / `takeBackSession`.
  Addressed Work selects its existing repository store, as inspection already does.
- Changed-selection as the only trigger that gives a recovering native surface
  focus. A transition from paused to usable also reconciles focus; unchanged
  readings leave editors alone.
- Failure callbacks addressed by Session id alone. The current `launchId` owns
  preparation failures; replaced viewers cannot mark a replacement failed.
- The untracked retirement Task in `GhosttySurfacePool.release`. The pool now
  retains that job until completion, so Refresh cannot prepare while its paused
  predecessor is still alive. Ordinary leave and its grace remain intact.

Keep non-replacing intent, snapshot fidelity, native drafts, pending requests,
shell lifetimes and other-viewer authority. The regression uses the current wire;
no new readiness flag, status feed, execution owner or protocol is selected.

PR overlap checked November 14: #1861 (LOO-774) is open at `07a21c5956` and
changes transport endings, `connection.rs`, and `ServerConnection.swift`; #1862
(LOO-811) is open at `753d49e7e2` and changes provider recovery, Session DTOs and
pane presentation. The implementation pass re-read the shared diffs; ending reasons and
usage-limit choices remain with those PRs. LOO-905 placeholder redesign,
LOO-906 startup speed, provider limits, account switching, installation and
workspace-wide UI redesign are outside this change.

## Acceptance and checks

User docs remain in `docs/sessions.md#when-desktop-loses-its-connection` and
`docs/troubleshooting.md#desktop-reopened-but-the-session-will-not-accept-typing`.
They retain the intended reopen/Refresh behavior, surviving versus ended
providers and deliberate input takeover.

- `scripts/test_desktop.sh --filter 'ServerConnectionTests|SessionsStoreTests|WorkspacePanesTests'`
  — 51 existing headless tests passed; the new action test failed both cases with
  original routing (Refresh retained its launch id, both actions stayed pending;
  three issues). This is a source regression, not attribution of the reported crash.
- `scripts/test_desktop.sh --filter refreshWaitsForPausedViewerRetirement`
  — without the retirement await, the headless native test failed four assertions:
  replacement preparation twice observed the predecessor still alive, and the
  previous surface was still present when Refresh returned.
- `scripts/test_desktop.sh --filter 'WarmPaneMountTests|recoveryAddressesRestoredTask|refreshSameConversation|taskWorkspaceReopenNamesTheTask|GhosttyTerminalMountTests'`
  — Desktop rebuilt; six active tests passed, including both addressed actions,
  managed Task reopening and overlapping Refresh on actual unmounted Ghostty
  surfaces. CoreVideo returned -6661 and Ghostty used timer rendering. Four
  mounted diagnostics skipped with “Optional display-session diagnostic”.
- `cargo test -p loopflow --test transport_tests desktop_crash_reopen -- --nocapture`
  — passed: acknowledged draft retained, new submissions once, unchanged
  Session/AgentProcess/held-terminal/provider, competing non-replacing attach Busy.
- `cargo fmt` and `git diff --check` — passed.
- `cargo clippy --all-targets -- -D warnings` and
  `cargo test -p loopflow --test integration documented_commands::` — gate.
- `uv run python scripts/test.py --reuse-passing --rust-filter 'binary(transport_tests) & (test(desktop_crash_reopen) | test(a_held_pane_reads_reconnecting_until_the_next_server_answers))'`
  — gate's named lifecycle checks; CI owns the remaining suites.
- `LOOPFLOW_NATIVE_TESTS=1 swift test --package-path swift --no-parallel --filter GhosttyTerminalMountTests`
  — optional display-session diagnostic, still unrun. Full Desktop crash/relaunch
  with a real provider belongs to demo; headless checks cannot establish it.

Initial docs presentation: `lf open --host desktop --file docs/sessions.md --file docs/troubleshooting.md`
returned `task_7c45fd8c99864a9cbec4fbda9b652429 has no workspace in this Desktop window; nothing changed.`
The docs stayed in the checkout; this was not mounted review.

Done when returning to the surviving Task permits editing and one new submission
without navigation; Refresh repairs the same Session or explains the real failure;
another controller retains input until Take back; provider/history/drafts/requests
and Graph execution remain intact. The original crash cause remains unidentified.

`lf context` — memory and scratch within token and byte limits; no limits changed.
