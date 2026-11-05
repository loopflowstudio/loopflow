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

## Latest accepted scope — November 5 review

Jack Heart accepted the demonstrated Task behavior as an improvement and expanded
the same automatic opening and prefetch requirement to repository and Wave
Sessions: “auto open everything *AND* prefetch everything”. The earlier Task-only
and started-only scope below is superseded by `loo-672-load-prefetch.md`
(Expanded review direction). No normal selected No session/Open session gate;
initialize absent primary Sessions/history, with loading or actionable failures
shown honestly. Automatic startup is a reading pass for every primary scope,
not permission to run its operator.

**Primaries (implemented November 5).** The registry keeps a `warmPrimaries` set
beside `recentTasks`: the repository and every Wave the window's roadmap shows are
prefetched on every `openWarmConversations` pass through `prefetchPrimary`, retained
outside the sixteen-Task bound, and dropped only by Close (a deliberate visit
readmits). `warmWork` orders selected, visible tiles, primaries, then recent Tasks,
under the same two preparation slots. A primary workspace's intent is always Start;
a Task's still follows its state. With no Session the queue calls
`ensurePrimarySession(wave:)` with the Wave name `prefetchPrimary` records on the
workspace from the roadmap, through the same `--start-if-free --prepare` write the
Open button used; Rust `select_connection` admits the primary, `admit_workspace`
places it in the scope's persistent checkout, and `open_without_replacing` keeps the
first-thread, controlled-elsewhere and consumed-launch refusals. The queue therefore
checks a primary's Machine only, not its checkout, against the workspace location.
The selected-state pane shows opening, the conversation, or a failure with
**Retry**; one placeholder serves every Work, and `WorkModel.openConversation(in:store:)`
routes its button by kind and place. After Close the pane reads **Conversation not open** with
the same button labelled **Open conversation**; a deliberate reselection reopens without it.
Rust `first_turn` has one
`PRIMARY_MESSAGE` for every scope: read, report, change no Work, launch no Graph,
claim or start no Task, treat steers as recorded direction. `repo_session.md` and
`wave_session.md` carry the same reading-pass bullet as `task_session.md`; their
operating pass begins with the person's first request.

**Remaining for primaries.** Native proof: a fresh repository load with no saved
history opens the repository and each Wave conversation unselected, selection
reveals the already started Session, returning to an existing Task conversation
keeps it, and no operator pass or Graph launched (`loo-672-demo.md`, required
demonstration). `prepare-redemo.py` needs a Wave in the loaded repository for that
case. PR #1816's head (`7deedb0306`, `lf pr open`) carries the primaries change
and the ninth-review record, and its body describes the expanded scope; the
commits after it (the in-place tile route, the inlined intent action and these
plan records) are unpublished. `demo.html` still describes the November 4 candidate (`7b23c50c20`,
“Prefetch still needs repair”), and the November 5 native candidates predate the
primaries change.

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
within the automatic-open requirement; executing Task work is not. Every started
Task (idle, active, moving) opens, owned or not: opening claims nothing, so the
"nothing moves an unassigned Task" rule is untouched (sixth concept review,
below, superseding the November 4 owner assumption in `questions.md`). An
unstarted Task has no checkout and so no conversation to open; placing one starts
the Task, which opening never authorizes, so it keeps an explicit Task-start
offer that does not read as a conversation gate. Missing account or Machine
evidence stays an actionable failure; the selected Task never falls back to a
different checkout or Machine.

Fresh repository state discovers idle, active and moving Tasks, including those
absent from saved recent history. Order: selected Task, visible tiles, saved
recent working Tasks, other working Tasks by shared update time, then other saved
unfinished Tasks. Prefetch uses the reveal queue and location validation,
preserves Close, and never waits before rendering.

Jack Heart set the warm bound to sixteen on November 5 (“make it 16”): sixteen
recent unfinished Task conversations across repository windows. Two concurrent
preparations remain a provisional engineering limit. Visible panes stay retained, with exceptions reported. No NSView is
shared between windows. Eviction releases Desktop's viewer and hold; protected
work and other hosts may keep the provider running. Shells keep their own lifecycle.

## Implementation

**Intent.** `RoadmapTask.conversationIntent` is the one working-Task policy: working
Tasks (idle, active, moving) open with Start, owned or not, since opening claims
nothing; unstarted and finished Tasks reveal history. `RoadmapTask.awaitsStart` is
unstarted and open: that Task has no checkout, so its Session slot, tile and pane
show `TaskStartOffer` (Rust's `task run` Start and help) in place of any
conversation placeholder. The intent rides
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
the explicit Start offer (`RegistryQuery.startOffer`, one `--explain` read that
offers nothing on a failed read); a failed Start shows its error with Retry on the
workspace intent. A Dashboard tile is the same `ConversationPlaceholder` as the
pane. A primary's Retry and a visible tile's button (while a board is
presented) re-enter the warm opener in place through
`SessionsWorkspaceRegistry.openConversation(in:store:)`: Rust's Start after a
refused reveal, else Retry on the workspace intent, readmitting a closed one, with
the same Machine check and the Wave name `prefetchPrimary` left on the workspace;
the board stays. The in-place route keys on any presented board, `selectedViewPlan`
or `navigation.waveView`, so a tile on the repository page or a Wave view attaches in
place too (`repositoryPageTileRetriesInPlace`). Only a selected Task's pane opens through its link, which can
resolve the checkout its Start creates; the error overlay above panes that are
already showing (`conversationPanes`) is the same Retry through the same opener,
not a second route. `WorkModel.conversationIsOpening` shows “Opening conversation…”
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

**First turn.** Any primary's first turn without a request is a reading pass:
`PRIMARY_MESSAGE` in `first_turn` and the three session skills say to read, report,
leave a stopped run stopped, claim or start nothing and treat steers as recorded
direction. Each automatic start spends one provider turn, so a load can spend up to
sixteen Task turns plus one per primary; the “no new agent turn” acceptance holds
only for existing history.

**Folder trust.** `restore_task_checkout` now calls `mark_lf_created`, so Task
checkouts carry the created marker again (none did since #1648). Adopted checkouts
stay unmarked and keep Claude's dialog; the launcher reports it as `pending_input`
so the Session reads Available before any hook runs.

**Shared matching.** `SessionRecord.taskMemberships` / `names(task:)` replace six
hand-written Task matches (memberships, checkout Task, own Work) so membership
reconciliation and conversation lookup agree.

### Delete — do not maintain

- History-only reveal as the automatic policy for working Task load, linked
  selection, prefetch and Retry. Reveal stays for unstarted and completed Tasks
  and the CLI's explicit `--reveal-existing` contract.
- The `!isUnassigned` term in `RoadmapTask.conversationIntent`, the owner-based
  `awaitsStart`, the Task/primary placeholder pair (`No Task conversation open` /
  `task-open-conversation`, `Conversation not open` / `primary-open-conversation`),
  `TaskConversationButton`, `startTaskConversation`, `WorkModel.openPrimaryConversation`,
  the sidebar's three Retry openers and the unreachable "has not reported this
  repository yet" copy: deleted November 5 (sixth concept review). One placeholder,
  one `ConversationOpenButton`, one `openConversation(in:store:)` routed by kind; the
  refused-reveal tests moved to the unstarted fixture Task `prefetch-15`.
- The selected-state **No session open / Open conversation** gate for repository
  and Wave Work, the `key.kind == .task || id != nil` admission in the warm queue,
  and `TASK_PRIMARY_MESSAGE` as a Task-only first turn: deleted November 5.
- `missingTaskConversationStartsExplicitly` and the sidebar refusal assertion;
  automatic preparation of both absence cases replaced them.
- The hidden warm host and any launch that depends on a pane mounting.
- The warm queue's `reveal` naming: it is `openWarmConversations`/`openings`.
- The `runtime?.owner != nil` term in `SessionsContentView.showsTerminals`: deleted
  November 5 (seventh concept review). A Task's panes follow its state and the
  person's choice, never its owner; a finished Task shows its record until a reveal
  presents its conversation.
- `DashboardConversationOffer` and `attachTaskConversation`, the Dashboard tile's
  own placeholder ("No conversation open", a button always labelled **Open
  conversation**), the primary tile's actionless "No primary Session" fallback and
  `SessionPaneView.emptyWorkspace`: deleted November 5 (eighth concept review).
  One `ConversationPlaceholder` serves pane and tile; attaching in place is where
  the button routes, not a second view. `SessionsWorkspace.openPrimaryConversation`,
  the primary Retry's own opener without the Machine check: deleted the same day
  for the registry opener.
- Preserve explicit Start as recovery outside eligible working Task navigation;
  preserve Rust's Start/Reveal/Take back intents, Close and late-result fences,
  selected-first scheduling, input claims and LOO-717 retention. No new launch,
  migration, retirement, provider quota or snapshot owner is introduced.

## Open

Warm slots are not filtered by person: a teammate's owned Task on an unreachable
Machine fails quietly and occupies one. Desktop has no local person identity to
compare. Acceptable while Jack is the only owner; revisit before shared repositories.

Sync, November 5 (re-checked at every realign against `27dbe31b69`, unmoved since;
the one commit after `384505b120`, #1827, touches migration CI and `lf install` only): the stack parent's
tip is already contained and origin/main is not merged; the parent (#1811)
integrates first. A trial merge of origin/main conflicts in three parent files (`transport.rs`, `transport/lifetime.rs`,
`tests/README.md`) and two of this branch's: a comment line in `bin/lf.rs` and the
Sessions `onChange` block, where main's `reconcileMembership` now returns a
selection to apply before `openWarmConversations`. At the main sync,
`open_without_replacing` still decides owner liveness with `answering` alone
(`human_session.rs:1035`) while main's `attaches` consults `owner_retention`
(`ae3c91abee`, `86fea8578f`); confirm a retained unresponsive owner with no native
history refuses rather than counting as a first thread. `answering` turns private
on main; only this module calls it after the merge. `c59cfcc631` (#1807) adopted
checkouts stay unmarked, matching the trust assumption.

Publication, November 5: PR #1816's body, refreshed at `lf pr open`, carries the
expanded scope (primaries on load, the bounded warm set, one placeholder and one
opener), the store-launch proof and the native cases still open; its tile sentence
names Dashboard tiles only, so the next publication says any presented board.

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

Concept review, November 5 (sixth pass, against the primaries change): the
expanded experience holds and its concepts did not grow: which Work opens itself,
what may open and in what order, when a provider starts, who may take input, what
a first turn may do. Three implications of the accepted decisions, implemented the
same day, none a new product choice: started unassigned Tasks open too, since the
reading pass claims nothing and "nothing moves an unassigned Task" is untouched
(restoring the `!isUnassigned` term is the one-line reversal if the demo says
otherwise); an unstarted Task's pane offers Rust's Start, not a conversation gate,
because it has no checkout and placing one starts the Task; and Task and primary
panes share one placeholder and one Retry routed by kind, whose fallback note is
the saved-Work reason or the read error. Completed Tasks keep history-only reveal
and LOO-717 release.

Settled as the product rule, not a limitation: a primary never operates
unprompted; opening reads, and operation begins with the person's first request,
from Desktop or `lf resume`. The November 4 and 5 assumptions in `questions.md`
record the CLI consequence. Open choices unchanged: the five demo questions of
the fifth pass, less the first, which Jack's direction answered.

Concept review, November 5 (seventh pass, against the sixth-review implications):
the experience and its five concepts hold; the three implications are in the tree
as recorded, the three skills and `PRIMARY_MESSAGE` agree, and `conversationIsOpening`
leaves no primary on a gate. One residue of the deleted owner rule remains, in
`SessionsContentView.showsTerminals`: a Task's panes show when it is working, when
someone owns it, or when the person chose them. Working Tasks are now covered by
their intent and unstarted ones by `awaitsStart`, so the owner term decides only a
finished Task's default: an owned finished Task without history shows the shared
placeholder with Rust's Start or a refusal, while the same Task nobody owns shows
its record, and a refused reveal's offer or error is never rendered there. Who owns
a finished Task says nothing about whether it has a conversation to show. Done the
same day (`977ff61b8b`): `showsTerminals` is working intent or the person's choice,
so a finished Task shows its conversation once the selected reveal presents it
(`presentSession` already chooses terminals) and its record otherwise, with Toggle
still reaching the placeholder; two `WorkspacePanesTests` cover a finished unowned
Task revealing into visible panes and a finished owned Task without history showing
its record. Consequence, settled by the accepted "preserve explicit Start as
recovery" line: `RegistryQuery.startOffer` and `conversationStart` reach a person
only through a chosen terminals view on a finished Task without history, since an
unstarted Task's pane is `TaskStartOffer` and never renders them; they and the
`prefetch-15` tests stay as that recovery. No new product choice: the four demo
questions of the fifth pass stand; the finished-Task case joins the demo list in
`loo-672-demo.md` as a one-selection check.

Concept review, November 5 (eighth pass, against the seventh-review residue): the
experience and its five concepts hold, `showsTerminals` reads state and choice
only, the two finished-Task tests exist and `SessionOpenIntent` is one type from
`SessionConnectionArgs::intent` through `open_without_replacing`. One placeholder
is still two: the Dashboard tile renders `DashboardConversationOffer`, "No
conversation open" with a button always labelled **Open conversation**, whose
action `attachTaskConversation` prepares the Task's intent and places the record
but handles no refusal. The selected pane for the same Task renders "Conversation
not open" or "Conversation failed to open" with `ConversationOpenButton`, labelled
by Rust's Start after a refused reveal, **Retry** after a failure, else Open. So a
working tile whose opening failed says **Open conversation** where the pane says
**Retry**, and a finished tile without history reveals, is refused with "No
existing conversation; start one explicitly", shows that sentence as its note and
offers the same button again: its own text names a recovery the tile cannot
reach, while the pane fetches `startOffer` and labels the button with it. Who
chose the view says nothing about what the state is. Settled by the sixth pass's
"one placeholder serves every Work" and the accepted "preserve explicit Start as
recovery": no new product choice. Done the same day: one `ConversationPlaceholder`
for pane and tile, the tile's button routed in place through the warm opener (see
Implementation), the two tile views and `attachTaskConversation` deleted. The
"Dashboard rule" the review named is the `WorkDestinationTests` case
`dashboardTileOpensConversationInPlace`, kept under that name on the shared
`open-conversation` identifier: a failed tile says **Retry** and attaches in
place; a second case shows a finished tile without history offering Rust's Start
label after its refused reveal and starting in place. Open choices unchanged: the
five demo questions stand as recorded defaults.

Concept review, November 5 (ninth pass, against the eighth-review tile direction):
the experience and its five concepts hold. One `ConversationPlaceholder` renders in
pane and tile, the two tile views and `attachTaskConversation` are gone, a primary's
Retry and a tile's button enter the registry opener, and an unknown Machine never
reaches the placeholder. One residue, landed the same day: `presentWaveTiles` opens
the repository page's and a Wave view's Task tiles like a Dashboard's, but `inTile`
read `selectedViewPlan` alone, nil for `.work(nil)` and `.work(wave)`, so those
tiles' **Retry** navigated into the Task (the repository page in Jack Heart's
screenshot). `inTile` now keys on any presented board (`selectedViewPlan` or
`navigation.waveView`) and visibility, with a `WorkDestinationTests` case where a
repository-page tile retries in place and the page stays; `.entered` keeps the
pane's navigating Retry. Checked and unchanged: a Wave's `machine` is its Tasks'
default Machine, so every visible Wave's primary prefetches in the local checkout;
the pane overlay's Retry runs the same opener. Open choices unchanged.

Concept review, November 5 (tenth pass, against the ninth-review tile route): the
experience and its five concepts hold, and the tree matches the plan: `inTile` keys
on `selectedViewPlan` or `navigation.waveView` plus visibility,
`repositoryPageTileRetriesInPlace` asserts the board and selection unchanged, one
`PRIMARY_MESSAGE` and one `ConversationPlaceholder` remain, and the working tree is
clean at `4a73d37b86`. Nothing to change and no new product choice; the five demo
questions stand as recorded defaults. Per Jack Heart's operator correction in
`loo-672-load-prefetch.md`, the next work is the native repository, Wave and Task
startup proof in `loo-672-demo.md`, not further review.

## Verification

Primaries (November 5): `cargo test -p loopflow --lib ops::human_session::` and
`scripts/test_desktop.sh --filter 'WorkCacheTests|WorkspaceRetentionTests|WorkspacePanesTests|WorkNavigationTests|WorkCLISubscriptionTests'`.
Cases: primaries open on load beside the Task bound before selection; a queued
primary shows opening and a failed one Retry, never an Open gate; primaries stay
warm and Close releases until a deliberate visit; the real-CLI load prepares the
repository primary with no agent process.

Focused checks cover automatic working Task preparation for absent and prepared
Sessions, identity across refresh and selection, bounded fresh-load prefetch,
unavailable-to-Start transition, visible tiles past the warm limit, superseded
refusals through the queue and a link, Close, elsewhere ownership, consumed-input
refusal and its **Try again**, an unmounted start and a runtimeless store that
only prepares (`WarmPaneMountTests`), a finished unowned Task revealing into panes
and a finished owned Task without history showing its record
(`WorkspacePanesTests`), a failed tile saying **Retry** and attaching in place and
a finished tile offering Rust's Start after a refused reveal
(`WorkDestinationTests`). Headless model and CLI checks stand in for preparation,
not native rendering or input acceptance; the rendered proof and remaining demo
work are in `loo-672-demo.md`. Gate owns affected suites and static analysis.

Pass history, November 4–5 (details in git): Rust `ops::human_session::` 36–37
passed and clippy clean at every Rust-touching pass; the Desktop filter above plus
`WarmPaneMountTests|WorkDestinationTests|DashboardWorkspaceTests` passed at each
implement, compress and realign (165, 167, 180 and 181 tests across 7–8 suites as
cases were added); three inherited expectations were corrected at the primaries
compress (the fixture's second Wave belongs to another repository, `hostUse` is
Task-checkout presence, a Wave pane fills on load) and two `WorkDestinationTests`
fixtures answer only Task or named-Session resumes. The rendered proof's candidate
predates `83b3d64a93`'s shared Task matching, the primaries change and these
passes. Realigns checked plan claims against the tree by inspection with no code
change; origin/main moved to `27dbe31b69` with the same five conflicting files.
Compress, November 5 (eighth-review tile direction): Swift-only; the primary
Retry joins the registry opener and `openPrimaryConversation` is deleted;
`scripts/test_desktop.sh --filter 'WorkNavigationTests|WorkDestinationTests|WorkCacheTests|DashboardWorkspaceTests|WorkspacePanesTests|WorkspaceRetentionTests|WarmPaneMountTests|WorkCLISubscriptionTests'`
181 passed in 8 suites; Rust unchanged, no rerun. Realign after it: inspection only
(shared placeholder in tile and pane, both tile tests present, deleted identifiers
absent under `swift/`, PR head and origin/main unmoved); that result reused.
Compress, November 5 (ninth pass): the in-place tile route and its repository-page
test were already in the tree; `SessionOpenIntent.action` inlined at its one use; the
same filter 182 passed in 8 suites; Rust unchanged, no rerun. Realign after it:
inspection only (`inTile` keys on `selectedViewPlan` or `navigation.waveView`,
`waveView` is nil for an entered Task so its pane still navigates, the
repository-page test asserts the board and selection unchanged, PR head and
origin/main as recorded); that 182 result reused.
