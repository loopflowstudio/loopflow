# Keep Task Sessions seamless

Status: **Pursue authorized through demo/review**, 2026-10-16. Jack Heart accepted
the architecture and expanded creation/readiness scope, then explicitly selected
`pursue`. Architecture is accepted; detailed restoration mechanisms remain draft.
No merge authorization.
Task: [LOO-416](https://linear.app/loopflow/issue/LOO-416).

Jack Heart's October 16 acceptance of [per-Session ownership and shared lfdb
layout](session-ownership-research.md) supersedes app-only files and shell-reopen
defaults. The shell probe establishes process survival, not terminal restoration.
[Recurring failures](session-continuity-failures.md) and [caller readiness](session-readiness.md)
retain the research evidence and expanded scope.

## Problem and demo

Switching Tasks should preserve the exact live conversation, scrollback, input
and panes. Closing a repository window or quitting/crashing Desktop should leave
running Sessions working; reopening restores the arrangement and joins those
Sessions. Twelve open Tasks must be a normal working set, not twelve cold launches.
This serves Infrastructure's execution-continuity objective and Product's
Work-centered navigation without making Desktop a process supervisor.

**Demo:** open twelve Tasks with running Sessions, leave unsubmitted text in one,
scroll another, and cycle through all twelve. Return to the same text and scroll
position with no preparation screen or provider launch. Enter a Task from its
Wave tile and return using the same surface. Close/reopen the repo window, then
quit Desktop during a turn and relaunch: the same Session IDs occupy the same
splits, the original turn progressed while absent, and no prompt was resubmitted.
Explicit stop ends only its owned process. Exact post-crash terminal scrollback
remains a review boundary below, not an achieved part of this demo.

## Decisions and risk findings

Accepted direction: [Arrange](arrange-navigation-brief.md), [prior Task brief](arrange-prior-brief.md),
and [remote-work decisions](work-on-another-machine-name.md). Jack Heart, October
7/16: one layout per Work; tiles project primary Session or explicit zoom;
transparent per-conversation holder, no shared daemon or automatic turn/Flow
retry. October 10's Claude/OpenCode stop-and-resume remains valid for explicit
takeover, not automatic viewer restoration. Findings below are from source at
`70f6a628e2a5e63b408d08ed3215e9d6fce7d648`. Reconciled October 16 against
`72dbe0344f`: LOO-495/#1578 now preserves repository identity across moves and
relocates retained Desktop Work. Retention consumes that resolver; it does not
introduce another path-to-Work identity or grant cached locators launch authority.

Baseline `70f6a628e2`: window-owned registries lose panes at close; `WorkCache`
evicts after eight repos and strips liveness. Generic connect may replace providers;
Codex needs its lifeline holder. Renderer restoration and durable drafts are absent;
performance fixtures have three PTYs. The full source table remains in
`evidence/plan-before-retention.txt`.

## Accepted architecture and draft integration contract

**Execution authority stays in Rust's existing Session, AgentProcess, Process and
attachment records.** AgentSession identity/history remains provider-owned. The
Desktop store holds presentation intent only: no PID, attachment token, live
flag, launch argv, saved action availability or retry instruction.

LOO-415 owns holder implementation and public attach/stop transport. A detached
per-Session holder is the accepted ownership direction.
Required interface: attach to this existing Session on its recorded Machine;
detach without ending the provider; resize from the active viewer; explicit stop
with current authority. Attach reports existing-live, ended, active-elsewhere or
unavailable. It never falls through to fresh launch, stop-and-resume, credential
flow or prompt replay. Session, AgentProcess and native thread stay unchanged;
viewer attachment claims change. The holder drains output without a viewer and
owns any lifeline needed for survival. Product memory records Jack's October 11
move of minimum first-start admission into LOO-427; LOO-414 retains unavailable-
coordinator recovery and detached launch retries. LOO-416 consumes these owners.

**Retention:** `LoopflowApp` retains one registry per resolved repository identity
and supplies it to `RepoView`. One window per repo mounts its native views;
closing it releases the mount, not the registry. Quit survival requires the owner
contract. LOO-489/497 own query membership; hiding panes neither stops Sessions
nor completes Tasks. No persisted Task “open” flag.

**Accepted persistence direction; record shape pending:** shared Rust-owned intent in lfdb, consumed
by Desktop and the CLI, separate from disposable `WorkCache`. A repository's
record contains outer selection/arrangement and a map by `WorkWorkspaceKey`.
Save each pane tree, stable pane IDs, focus/zoom/hidden panes, presentation selection
and content's recorded Machine/path/Task location. Container query/arrangement
values come from LOO-497's single model integrated by LOO-489; no parallel query
serializer here. LOO-497 owns the migration and shared record/API; LOO-489 owns
save/restore integration. LOO-416 supplies continuity and draft-recovery requirements.
Jack's later October 16 direction authorizes LOO-497's technical foundations,
including typed selection, retained arrangement and durable intent, before prototype
review. Production query grammar and UX integration still await that review.

Use the shared lfdb owner without dual writes. Live PTYs, native views and
provider input remain runtime state; database identity alone cannot restore them.

Presentation belongs to the viewing installation and is independent of the selected
execution Machine. Identity comes from shared Work resolution, not checkout path
or guessed cross-machine Task equivalence. The same repo window can select remote
execution Machines. This does not provide layout synchronization between separate
Desktop installations; that interpretation of D9 remains an explicit review choice.

Capture immutable snapshots on structural changes and serialize transactional writes in
order off the main actor. Expose write failures and retain the last committed state.
Flush pending writes for orderly quit; crash restores the last complete snapshot,
never half a tree. CLI mutation acknowledgment reports durable completion or its
failure. Warm switching renders immediately, outside disk/network waits. Validate
structural references on load without discarding missing Work. Preserve corrupt
or unreadable saved intent for recovery and show the error; do not overwrite
it with an empty layout. Old cache selection can seed a first layout when no
durable record exists, but no permanent dual read/write path remains.

Presentation restores from disk independently of inventory refresh. Resolve Session
actions/location, then attach visible existing-live Sessions before the explicitly
retained working set, with concurrency initially bounded to two. Query matches
alone never launch a provider. Ended Sessions show history and explicit resume;
uncertain/unreachable ones keep their pane and error. Automatic attachment failure
does not loop. Location mismatch preserves the recorded pane; no local checkout
or different-account fallback. Neither stale cache nor a layout file authorizes
takeover. Shared creation/readiness triggers remain to be designed under Jack's
expanded caller scope; navigation is not their only authorized entry point.

## Retention and action semantics

| State | Switch or window close/reopen in same app | Quit/crash and reopen |
|---|---|---|
| Arrangement and Work selection | Same retained objects | Last complete snapshot; same Work/pane IDs |
| Running Session; input delivered to its process | Same provider and surface | Same provider through LOO-415; repaint, never retype input |
| Terminal scrollback, viewport, search, selection | Exact retention on same surface | Exact renderer restoration unresolved; native history/repaint is not equivalent |
| Local IME composition not delivered to provider | Same native view | Not recoverable from provider; never synthesize submission |
| Ordinary shell | Same PTY/cwd/environment | Accepted direction, unbuilt: reconnect to its surviving PTY owner, preserving jobs and environment; same-directory relaunch is explicit recovery after loss |
| Files/logs | Same retained store | Restore owner/path and reread; logs stay evidence |
| App-owned unsent file draft | Same buffer, undo, selection | Persist recovery text, base revision/content and selection with owner/path; restore as draft, never auto-save to worktree; native undo is not serialized |

Draft recovery extends `TaskFileDocument`'s existing revision/merge behavior;
changed disk content uses existing conflict presentation. It is recovery data,
not a second authoritative file store. Changed owners/invalid paths preserve text
without redirecting writes. Provider drafts are not copied into layout files.
A Session discovered inside a shell reattaches by its recorded Session identity
after restart rather than replaying the shell command, preserving its Work owner.

**Interrupt** explicitly targets the current turn through existing owned controls;
it does not reset identity. **Resume** deliberately opens a native client on saved
history after process exit. **Reset** currently uses `SessionsStore.reset`/Session-new
successor semantics and old history. Jack also wants a pleasurable fresh-context
restart after ordinary continuity works reliably: retain the old conversation and
make carried context explicit. Its identity and handoff interaction are downstream
design work, not prescribed by the current API. Distinguish reconnect, process
recovery and fresh context even if product labels later use “restart.”
Provider switching stays explicit. Navigation, hide, viewer close,
cache refresh and app restoration invoke none of these actions.

## Alternatives, scope and failure analysis

WorkCache persistence or generic reconnect cannot prove continuity. Per-Session
owners isolate failures; no evidence justifies a shared server, Swift relay,
tmux holder or custom emulator. LOO-415 owns transport; capacity measurements
must include owner costs. Keep Codex's existing bridge and fences.

Exclude navigation/query redesign, external multiplexers, remote launch/auth
implementation and Task completion changes. Shell survival
now has a feasible per-shell ownership candidate; its integration must share the
relay transport without inventing an agent conversation for a shell. Exact post-crash
scrollback and cross-installation sync remain review boundaries in [questions](questions.md).
Jack selected first-wins duplicate presentation in LOO-497; integration remains unbuilt.
The draft does not waive a stronger interpretation of accepted behavior.

## Delete — do not maintain

- Remove `WorkSnapshot.Repository.selection` and `WorkCache.saveSelection` as
  persistence owners once callers use the durable store; keep cached wire reads
  with unknown liveness. Any one-time seed is discarded after durable creation.
- Removed saved-live-Session startup through `SessionsStore.recover`/generic
  `connect`; selection now uses attach-only restoration. Preserve explicit resume, takeover,
  reset and provider-switch behavior and authority tests.
- LOO-489/497 reshape `SessionsWorkspaceRegistry.revealSession` relocation where
  container projection would remove the Task's own pane. They own membership;
  LOO-416 preserves the single `TerminalIdentity` and surface across that cutover.
- Do not repair tmux or obsolete relay paths here. LOO-415 deletes what its
  transport replaces. Preserve Codex generations/fences, native history,
  unknown-outcome handling and exact owned-process shutdown tests.

Forbidden: two layouts per Task, saved argv replay, mirrored Session liveness,
fresh-provider fallback, a terminal copy per query, LRU killing live work or
another arrangement store. One NSView has one mount: LOO-489/497's first-wins
projection preserves query occurrences while displaying one native surface.

## Readiness from every caller

Jack requested [prefetch and creation-lifetime analysis](session-readiness.md):
evaluate and change `task create`, Session `ensure`, and their callers as needed.
Repository, Wave and Task conversations must share auto-creation and lifecycle
semantics. Evaluate every ensured Session, including bare Session admission and arbitrarily
nested lf invocations. Desktop is a viewer, never the required owner or scheduler.
Provider-start handoff and caller-death proof belong in shared Rust paths. Compare
metadata preparation, owner startup and terminal preattachment costs before
selecting warming policy; do not equate Session existence with readiness.

## Internal slices and acceptance

One integrated LOO-416 outcome; serialization alone cannot complete it.

1. **Retention implemented; durable recovery remains.** Application-owned
   registries and selection adoption exist, with `WorkspaceRetentionTests`.
   LOO-497's shared durable contract and LOO-489's save/restore integration must
   cover atomic recovery and app-owned drafts without another serializer. Add headless
   `WorkspaceRestorationTests`: twelve Work keys, splits, hidden panes, remote
   content, drafts and changed file revisions; recreate application state from
   disk and compare observations. Missing inventory pages preserve saved members.
   Focused command: `scripts/test_desktop.sh -Xswiftc -gnone --filter WorkspaceRestorationTests`.
   Expect same owners/IDs/ratios/drafts, fresh native incarnations after app death,
   no provider spawn/input submission and visible corrupt-store failure.
2. **Attachment cutover — depends on LOO-415.** Add headless
   `WorkspaceSessionLifecycleTests` and detached-viewer cases in Rust
   `session_lifecycle_tests`. In a disposable Machine with candidate CLI and
   provider stand-ins: create a Session, persist layout, end only the viewer
   normally and by SIGKILL, observe continuing provider progress, recreate
   Desktop state, attach and submit one unique marker. Assert identical
   Session/AgentProcess/native thread, changed viewer claim, one launch/turn,
   and explicit stop. Include detached output, ended process, active-elsewhere,
   absent endpoint and unreachable Machine; fake live flags are insufficient.
3. **Twelve-Task switching/capacity.** Extend `DesktopPerformanceTests` and its
   existing runner to twelve actual live Sessions, then 24/48 for capacity
   characterization. Integrate LOO-489 tiles with the same surfaces. Headless
   model tests require zero warm-switch preparation/spawn, no `.opening`
   transition and retained Work state. Native proof checks input echo, scroll
   position, same surface incarnation, unsent text and tile→Task→tile mounting.

**Verification:** build changed code and run focused behavioral suites. Gate owns
affected Desktop/session/control/file/DTO suites plus the candidate CLI transport
integration under `scripts/test_network.py` with inherited LF variables unset and
stdin closed. Rust changes require fmt/clippy and migration frontier/DTO fixtures.
Headless checks must exercise production restored registries, not serialization
alone. Native display/provider/capacity proof remains demo evidence and cannot be
claimed from fake status or three cat PTYs. Compare baseline/candidate twelve actual
Sessions (then 24/48), twenty warm attempts, p50/p95 input-ready latency, RSS/CPU,
owner/surface/launch counts, failed attempts and wasted warming. No new caching
policy without evidence. Prior detailed proposed commands and display runner plans
are preserved in `evidence/loo-416-plan-before-stack-compression.txt`.

**Done when:** twelve opened Tasks retain text/scrollback/input on warm switching
without loading or relaunch; last-viewer close/quit/crash leaves the original
provider working; relaunch restores Work and attaches exactly once; missing or
stopped Sessions remain truthful and never silently restart; shared state API
and click view agree. Full completion also requires resolving post-crash
renderer-state and cross-installation interpretations instead of silently
excluding accepted behavior. Transport delivery and native capacity remain
unproved dependencies.

Review finding: retaining generic connect fallback could make restart tests pass
while replacing execution. Require attach-only results and preserve explicit
resume; prove surface reuse separately from provider survival.

## Implementation boundary — October 17, 2026

Full LOO-416 acceptance remains open. Jack Heart authorized stacking on LOO-415;
parent `2523534b29` is integrated by `7bfa10fa72`, with prior retention checkpointed
in `172ce48a39`. Application retention and immediate known-repository presentation
remain; pending Task links retain fresh-validation boundaries.

Desktop selection now uses `session attach --if-free --prepare --json`; it cannot
resume history or displace another viewer's input claim. Reconnect is a typed
Session action; explicit Open/Move actions remain available when attachment is
unavailable. Cancellation discards late preparation. Repository/Wave/Task ensure
share preparation, and existing executions return attach-only commands. Task filing
now commits its primary Session; Wave admission commits its own and repository
Sessions without capture, provider effects or Started reservation. No warming
policy was selected.

Shared reads recovered using released lf 0.14.4 after discovering inherited
LF_BIN pinned 0.14.3. October 17 status confirms this implement step belongs to
live Flow `53d045e1-9dc6-44ea-b4f5-a2345446478a`; LOO-497 has a live implementation.
Latest shared status reconfirms its live implement step and published #1580 head
`d9141c282f`, containing earlier foundations, not the current
pane/draft integration. LOO-489/497 retain native persistence and shared record
ownership; no sibling dirty code was copied. Their supported handoff is still
needed for restoration, shell Process identity and draft recovery.

Remaining: bounded CLI preparation/cancellation beyond
the transport handshake; nested caller-death boundary proof; durable restoration;
twelve actual providers and 24/48 capacity measurements; display/native remount;
exact post-crash terminal state and cross-installation interpretation. Model
switching tests do not establish native/provider readiness. Infrastructure memory
remains read-only; its 209-token overage belongs to its owning Flow.

### Admission revision — accepted internal repair, October 17

The predecessor always captured input on conversation insertion and irreversibly
marked its Task Started. The initial attempt and counterexample are archived in
`evidence/metadata-admission-counterexample.patch`. They explain the required
nullable-input cut; the patch is evidence, not a second implementation.

Operator steer 1482 confirms this is an authorized internal repair, not a new
approval boundary. Admit the Session with no capture; Task filing commits its
primary identity atomically. Explicit Wave admission shares the owner and admits
its repository conversation. Prepare resolves the Work checkout; requested
execution reserves input and marks Started. Passive reads do neither. Preserve
existing Session/history identities and prospective attribution.

Deleted: mandatory `LfSession.artifact_key`; unconditional conversation-insert/bind
Task-start triggers; metadata admission's unconditional capture. Preserve actual
input reservation, immutable capture history, explicit bind semantics and the
released-schema upgrade. The existing `session_transport.sql` draft now moves
Started authority to first capture or binding previously captured work. Historical
start times remain immutable. Readers accept uncaptured Sessions; execution reserves
input on the same identity, after resolving the Task checkout. Both terminal and
headless explicit connection use that owner. Concurrent ensure reuses the primary.
Focused checks now cover terminal and headless first start, concurrent ensure,
uncaptured bind, retained history and the creator-exit transport boundary.

Review findings resolved: generic reconnect fallback; reconnect labels dispatching
resume; active-provider metadata preventing safe free-viewer attachment. The
production button tests cover unavailable and ended viewers with distinct recovery.

Checks: `cargo fmt --check`, `cargo clippy --all-targets -- -D warnings`, network-isolated Rust admission/readiness/migration/primary/creator-exit tests, and `scripts/test_desktop.sh -Xswiftc -gnone --filter 'WorkspaceSessionLifecycleTests|SessionsStoreTests'` passed (20 Swift tests); unchanged Registry/destination/DTO checks also passed. Gate owns remaining affected suites and full acceptance; the display host owns native/provider/capacity demonstration.
