# LOO-672 — Ready Task conversations

Accepted design, reconciled November 4, 2026; Jack Heart approved the revised
November 4 design: “ok, proceed.” Implementation and review publication are
authorized through interactive demo; shipping, merging and installation are not.
Decisions and assumptions: `questions.md`. Demo evidence: `loo-672-demo.md`.
Earlier designs, the four November 4 concept-review passes, screenshot repairs and
check evidence: `78f4d8e6c0:scratch/loo-672-ready-conversations.md` and
`3ef57e857394d5a97271ea00b6048ad60c22bf9a:scratch/loo-672-ready-conversations.md`.
The pre-infrastructure plan is superseded and preserved at
`a32c385044:scratch/research/loo-672-pre-infrastructure-plan.md`.

## Accepted outcome and constraints

Task conversations open without a Start or Open click after launch, selection and
restart. For working Tasks (idle, active, moving), automatic preparation creates
the primary Session if absent and starts the native conversation if it has no
history, or resumes existing history. Jack Heart confirmed on November 5 that
background prefetch must open native conversations for started working Tasks
before selection; “unopened” is not their normal state (`loo-672-load-prefetch.md`).
Recent unfinished Tasks stay warm for fast input; returning lands at the bottom.
Completed Tasks release on departure through LOO-717. Close removes warm intent;
refresh cannot undo Close, but a deliberate later visit may reopen the conversation.

Takeover without Fork: the pane keeps the last screen and offers explicit
**Take back**. Selection and polling never transfer another client's input. The
overlay explains the holder using Rust's labels and prepared command. The CLI fork
remains available on the same Task and checkout.

Viewing, prefetch and restore never start a Task Graph or replay interrupted or
captured input. Creating the initial native conversation for a working Task is
within the automatic-open requirement; executing Task work is not. Prefetch does
not reach backlog: unstarted Tasks and unassigned started Tasks (assumption in
`questions.md`). Missing account or Machine evidence stays an actionable failure;
the selected Task never falls back to a different checkout or Machine.

Fresh repository state discovers idle, active and moving Tasks, including those
absent from saved recent history. Order: selected Task, visible tiles, saved
recent working Tasks, other working Tasks by shared update time, then other saved
unfinished Tasks. Prefetch uses the reveal queue and location validation,
preserves Close, and never waits before rendering.

Provisional engineering limits, not a measured budget or Jack's numbers: eight
recent unfinished Task conversations across repository windows and two concurrent
preparations. Visible panes stay retained, with exceptions reported. No NSView is
shared between windows. Eviction releases Desktop's viewer and hold; protected
work and other hosts may keep the provider running. Shells keep their own lifecycle.

## Implementation

**Intent.** `RoadmapTask.conversationIntent` is the one working-Task policy: owned
working Tasks open with Start, everything else reveals history. The intent rides
`session_resume` writes as `--start-if-free` / `--reveal-existing`
(`SessionOpenIntent`, mirrored in Rust `Write::SessionResume`). Rust
`open_without_replacing` keeps the launch lock and transport handshake, attaches
with `Own::Request` instead of `Own::Take`, refuses a controlled conversation, a
missing checkout, a first thread without Start, a provider or account change, and a
consumed first launch with no native history. Only Take back replaces an input
holder. `session_actions` gains Reveal and relabels Move here as Take back; a
consumed first launch offers MoveHere labelled **Try again** in place of Open, with
no new wire kind (`SessionRecord.launchRetry` / `heldElsewhere` read it).

**Queue.** `SessionsWorkspaceRegistry.openWarmConversations` opens the warm set
(selected, visible Tasks, recent) under two preparations across windows
(`RepositoryWorkspaces` supplies `canPrepare`, `onPreparationFinished`,
`onWarmthChanged`). A refused reveal clears when the Task's intent becomes Start; a
superseded refusal records neither error nor offer and the queue retries with the
current intent, including through `openLinkedTask`. Only a refused reveal fetches
the explicit Start offer; a failed Start shows its error with Retry on the
workspace intent. `WorkModel.conversationIsOpening` shows “Opening conversation…”
for an eligible working Task queued behind the slots or its Sessions read.
`confirmMachine` replays Sessions so a quiet fresh repository opens its Tasks.
Warm intent (recent and closed keys) persists per Machine and repository in
UserDefaults; `loadWarmIntent` is the explicit opt-in, so headless registries and
tests that never call it never write defaults.

**Launch from the store (November 5).** Launch no longer waits for a pane: SwiftUI
`.hidden()` never attaches a platform view, and fresh working Tasks were hidden
Dashboard tiles nothing rendered. `SessionsStore` starts a Session's terminal through
`GhosttySurfacePool.start` whenever an item becomes `.prepared` (preparation, Try
again, provider switch, reset), sized to the last mounted pane or 1100×700; the pane
later shown adopts the running surface. The pool's `runtime` is explicit: the app
passes `GhosttyManager.shared`; headless stores and tests pass none and launch
nothing. `prefetch` admits a visible tile so load-started conversations stay warm
after leaving the Dashboard. The live relay writes the `loopflow:input-ready` title
marker only after acknowledged control; `GhosttyManager` turns it into
`ghosttySessionReady`, which marks the pane live.

**First turn.** A Task primary's first turn without a request is a reading pass:
`TASK_PRIMARY_MESSAGE`, selected by primary scope in `first_turn`, and
`task_session.md` say to read, report, leave a stopped run stopped and treat steers
as recorded direction. Repository and Wave primaries keep their prior text. Each
automatic start spends one provider turn, so a load can spend up to eight; the
“no new agent turn” acceptance holds only for existing history.

**Folder trust.** `restore_task_checkout` now calls `mark_lf_created`, so Task
checkouts carry the created marker again (none did since #1648). Adopted checkouts
stay unmarked and keep Claude's dialog; the launcher reports it as `pending_input`
so the Session reads Available before any hook runs.

**Shared matching.** `SessionRecord.taskMemberships` / `names(task:)` replace six
hand-written Task matches (memberships, checkout Task, own Work) so membership
reconciliation and conversation lookup agree.

### Delete — do not maintain

- History-only reveal as the automatic policy for working Task load, linked
  selection, prefetch and Retry. Reveal stays for other Work and the CLI's explicit
  `--reveal-existing` contract.
- `missingTaskConversationStartsExplicitly` and the sidebar refusal assertion;
  automatic preparation of both absence cases replaced them.
- The hidden warm host and any launch that depends on a pane mounting.
- The warm queue's `reveal` naming: it is `openWarmConversations`/`openings`.
- Preserve explicit Start as recovery outside eligible working Task navigation;
  preserve Rust's Start/Reveal/Take back intents, Close and late-result fences,
  selected-first scheduling, input claims and LOO-717 retention. No new launch,
  migration, retirement, provider quota or snapshot owner is introduced.

## Open

Warm slots are not filtered by person: a teammate's owned Task on an unreachable
Machine fails quietly and occupies one. Desktop has no local person identity to
compare. Acceptable while Jack is the only owner; revisit before shared repositories.

Sync, November 5: the stack parent's tip is already contained and origin/main is
not merged; the parent (#1811) integrates first. A trial merge of origin/main
conflicts in three parent files (`transport.rs`, `transport/lifetime.rs`,
`tests/README.md`) and two of this branch's: a comment line in `bin/lf.rs` and the
Sessions `onChange` block, where main's `reconcileMembership` now returns a
selection to apply before `openWarmConversations`. At the main sync,
`open_without_replacing` still decides owner liveness with `answering` alone
(`human_session.rs:1043`) while main's `attaches` consults `owner_retention`
(`ae3c91abee`, `86fea8578f`); confirm a retained unresponsive owner with no native
history refuses rather than counting as a first thread. `answering` turns private
on main; only this module calls it after the merge. `c59cfcc631` (#1807) adopted
checkouts stay unmarked, matching the trust assumption.

Publication, November 5: PR #1816's body now carries the store-launch proof,
launching from the store before a pane mounts, the pending main sync and the
current acceptance; it no longer says automatic opening was never observed natively.

Concept review, November 5 (fifth pass, against the store-launch change): the
experience and its four concepts are unchanged; launching from the store is the
mechanism the accepted experience always implied, since a conversation that waits
for a pane is an unopened conversation. No implement iteration. Two recorded
defaults now apply at load rather than on selection, which widens their effect
without changing the question: a quit or reboot during the load window can leave
several Tasks on **Try again** instead of one (`questions.md`, consumed first
launch), and a load fills the Available filter with every working Task's reading
pass (`questions.md`, Available after the automatic pass). Both stay recorded
defaults for Jack Heart to confirm at the demo. The Task description's acceptance
line “no new agent turn starts merely from opening, prefetching or resuming a
view” is superseded for history-less working Tasks by Jack's November 4 and 5
direction; publication states the current acceptance (one reading pass per Task
lifetime, none for existing history) rather than the original line. Compress note,
no design change: `GhosttySurfacePool.start`'s `.uninitialized, .initializing`
return is unreachable because `GhosttyManager.initialize()` finishes synchronously
in `.ready` or `.failed`.

## Verification

Focused checks cover automatic working Task preparation for absent and prepared
Sessions, identity across refresh and selection, bounded fresh-load prefetch
excluding unassigned Tasks, unavailable-to-Start transition, visible tiles past the
warm limit, superseded refusals through the queue and a link, Close, elsewhere
ownership, consumed-input refusal and its **Try again**, an unmounted start and a
runtimeless store that only prepares (`WarmPaneMountTests`). Headless model and CLI
checks stand in for preparation, not native rendering or input acceptance; the
rendered proof and remaining demo work are in `loo-672-demo.md`. Gate owns
affected suites and static analysis.

Checks: November 4 `cargo test -p loopflow --lib ops::human_session::` 37 passed;
clippy and the 714-test Desktop suite passed before publication. November 5
`scripts/test_desktop.sh --filter 'WorkspaceRetentionTests|WarmPaneMountTests|WorkCacheTests'`
43 passed after the tile-retention fix. Compress, November 5: Swift-only edits;
`scripts/test_desktop.sh --filter 'WorkDestinationTests|SessionsStoreTests|WorkspaceRetentionTests|WorkCacheTests|WorkspacePanesTests|WorkSurfaceTests|SessionsPolishTests|DTOFixtureTests'`
203 passed in 8 suites; Rust unchanged, no rerun. Realign, November 5: plan claims
checked against the tree by inspection (intent, warm queue, store launch, first
turn, trust marker, shared matching, named tests all present); no code change, no
rerun. The rendered proof's candidate predates `83b3d64a93`'s shared Task matching.
