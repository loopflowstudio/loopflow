# Compression review — 2026-09-23

## Effective model

- Shared Rust/Swift `RoadmapSnapshot`, `SessionRecord` and `ActivitySnapshot`
  supply planning, human-work and live-process evidence through RegistryQuery.
  Activity has Exec and ProviderProcess nodes; checkout location is separate
  from Work attribution and native terminal attachment.
- Rust projects Session action descriptors and Work display paths from the
  existing boundary and stored ancestry. CLI text and Swift controls consume
  those values. Flow settlement checks the same policy against its current
  playhead; Swift owns only local presentation and dispatch.
- GUI process preparation clears inherited execution and terminal markers while
  preserving Home/account selection. CLI queries supply explicit scope;
  ConversationLaunch captures the visible subject and honors lf's destination.
- PodiumModel owns readings, last-good evidence and read generations.
  WorkspaceProjection derives typed planning-to-Session associations, preserving
  upcoming Tasks and every unmatched human boundary. TaskQuery selects Active
  or All tasks over that projection; Active includes an existing local worktree,
  open Sessions or owned provider activity in the recorded Task checkout. It
  stores nothing. Unknown filesystem evidence remains explicit.
- TaskDirectiveEditor owns an unsaved draft and its captured Task/Wave target.
  RegistryQuery sends the existing PM update; Podium refreshes authoritative
  planning and invalidates older polling results. The sheet owns submission
  feedback, not a second planning record.
- WorkspaceNavigation owns selected Work, presentation, search, expansion and
  list scroll offset per repository/window. PodiumModel forwards selection to
  that owner; SwiftUI controls the mounted navigator's scrolling.
- SessionsWorkspaceRegistry retains checkout workspaces and repository-level
  WorktreeLayoutStores inside one window, sharing that window's surface pool.
  Outer slots select checkouts; each checkout's MultiplexerStore owns terminals.
  The repository's retained SessionsStore owns opening/prepared presentation and
  resolution errors, and calls shared actions. Rust projects actual client
  terminal attachment.
- GhosttyMetalView owns its surface, latest title and focus request across attachment and
  visibility changes. AppKit owns actual first responder. GhosttyManager owns
  library initialization and configuration, including the measured CoreVideo
  capability adaptation.

## Before and after

### Iteration 17 — hierarchy study and retained ownership (2026-09-24)

No coherent model/API reduction was established. Only this report changes.
The latest slice adds a simulated hierarchy study and design amendments; it
does not replace the native navigator or introduce another production owner.

Compared Rust/Swift `SessionRecord`, `SessionAction`, `RoadmapProject`,
`RoadmapTask`, `TaskWorkspaceSnapshot`, and Activity fields. Followed shared
action checks and their Flow-settlement caller, RegistryQuery's inventory
methods, PodiumModel's readers, WorkspaceProjection, SessionsStore, the
window workspace registry, MultiplexerStore reconciliation and WorktreeLayout.
Inspected Session/Activity fixture assertions and the study's indexing,
selection, compression and retained conversation panels. Searches retain one
Podium caller per inventory read and one production root workspace registry;
SessionScope, FlowResolutionAction and requestedSessionId remain absent.

Candidates retained:

- The study's generic tree is sample presentation data. Replacing typed planning
  and runtime references with it would discard upcoming Work identity and exact
  Session attribution. Native visual compression must preserve those facts.
- Current navigation controls still serve reachable behavior. Removing them
  before implementing the agreed outline would change capabilities. Adding
  unused Monitor content or a second split tree would add scaffolding, not
  simplify the existing multiplexer.
- Outer checkout slots retain complete inner layouts; closing a slot hides its
  terminals, while closing an inner shell ends it. `knownPaths` retains hidden
  and shell-only membership that current Session inventory cannot reconstruct.
- Prepared opening commands must survive polling, while rejected resolution
  coexists with a live terminal. Combining these states would restore the
  earlier error-loss defect. Complete and Flow decisions also have different
  commands and inputs; their shared error tail does not justify a new wrapper.

All 15 source/fixture hashes selected from the iteration 16 review receipt still
match. This verifies unchanged content, not fresh behavioral evidence. No
executable edits or test reruns were needed; `git diff --check` passes. The
study does not establish native terminal retention, Monitor behavior, rendering
performance or human acceptance. Those obligations remain with implementation
and demonstration; this pass performed no app interaction or publication.

### Iteration 16 — live contract comparison and resolution errors (2026-09-24)

No coherent model/API reduction was established. Only this report changes;
the effective ownership model remains unchanged. The preceding review generalized
`completionError` to `resolutionError` so rejected Flow decisions preserve a live
terminal. That correction belongs to review 15, not this compression pass.

Inspected Rust Session projection, action checks and Task Flow settlement through
CLI Session commands, RegistryQuery, Podium's inventory reads, WorkspaceProjection,
and retained SessionsStore/pane controls. Compared Rust/Swift SessionRecord,
SessionAction, RoadmapProject/Task, TaskWorkspaceSnapshot and Activity fields.
Read Session fixture assertions, the rejected-decision native test and the live
comparison procedure. Searches retain one Podium caller per inventory read and
one production root workspace registry; the removed Session scope/navigation
types and FlowResolutionAction remain absent.

Candidates retained:

- Opening failure and resolution rejection can coexist with different terminal
  states. Combining them would restore the reviewed defect. Prepared commands
  also cannot be overwritten by polling; last-good readings serve another lifetime.
- Complete and Flow decisions use different commands and inputs. Their short
  shared error/removal tails do not justify another operation abstraction.
  Direct panes and shell-attached Sessions reuse the same resolution errors,
  while preserving every attached Session's identity.
- Planning identity, runtime Work, checkout existence and terminal attachment
  remain independent shared facts. Outer layout membership retains hidden and
  shell-only groups that Session inventory cannot reconstruct.
- Shared action availability does not replace local surface ownership.
  FlowDecision's restricted domain and validation before client stopping versus
  after playhead reload still protect distinct boundaries.

All 11 hashes in iteration 16's receipt and all 12 in review 15's receipt match.
Inspected the existing 154-record comparison and passing 13-test resolution/store
receipt. These are prior results, not fresh tests; the comparison covers one
cached population and supplies no UI-trial or timing credit. No executable edits
were made and no tests rerun. `git diff --check` passes.

The locked-host finding and prepared read-only procedure remain in
[iteration16-proof.md](iteration16-proof.md). This pass performed no app/provider
interaction. Configured contract controls, nested input, human-selected external
trials/edit and measured budgets remain open; no publication or Task completion.

### Iteration 15 — shared Session actions and Work paths (2026-09-24)

No further coherent model/API reduction was established. Before and after this
compression pass, Rust owns Session legality and display paths, Podium owns
readings, and retained window workspaces own terminal presentation. Only this
report changes. The preceding implementation removed Swift's action matrix,
replacement inference and `FlowResolutionAction`; those are implementation
changes, not new reductions made by this pass.

Traced `human_session` projection and operation checks → Task Flow settlement →
CLI list/open/complete/decisions → RegistryQuery → Podium → WorkspaceProjection
and navigator → SessionsStore and pane controls. Compared every Rust/Swift
SessionRecord and SessionAction field, their enums, shared Session fixtures,
twelve-case action fixture and fixture assertions. Followed typed ancestry into
the display path and local terminal attachment into the retained workspace.
Searches retain one Podium caller per inventory read, one production root
workspace registry, and no removed Session scope types, label-query helper or
FlowResolutionAction.

Candidates retained:

- `FlowDecision` accepts only Approve/Iterate at settlement. Replacing it with
  the five-case presentation action enum would admit Open/Move here/Complete
  into a boundary that cannot execute them, requiring new rejection branches.
- Generic Session actions and Flow actions share the same projection. The
  latter adds the real preceding-autonomous-step constraint. Checks before
  stopping a client and after reloading the playhead protect different moments;
  consolidating them by deletion would lose one boundary.
- Typed Work and its display path serve identity and readable ancestry. Shared
  Session state describes the boundary; action reasons describe availability.
  None replaces window-local attachment, opening/prepared state or errors.
  Label, help and unavailable reason serve distinct control presentations; an
  added enabled flag would merely duplicate the reason's absence.
- The remaining two opening-button presentations dispatch the same shared
  descriptors. Extracting their styling would remove neither policy nor an
  owner. Prepared commands, retained terminals, completion errors and last-good
  reads still have independent lifetimes. Outer checkout membership also retains
  hidden and shell-only groups that Session inventory cannot reconstruct.

All nine recorded source/fixture hashes in iteration 15's `hashes.json` still
match. Inspected its passing receipts: 14 focused Swift tests, the separate
native retention test with two cases, ten Rust projection tests, premature
decision rejection, and Ready approval/iteration. These are existing receipts,
not new tests. No executable content changed, so no tests were rerun;
`git diff --check` passes.

The configured four-record decode comparison and failed app count observation
retain their limits in [iteration15-shared-sessions.md](iteration15-shared-sessions.md).
This pass performed no app/provider interaction. Configured contract controls,
nested input, human-selected external trials/edit and measured budgets remain
open. No publication, landing or Task completion occurred.

### Iteration 14 — exact Session picker and nested workspaces (2026-09-24)

No coherent model/API reduction was established. Before and after this pass,
shared planning and Session records supply identity, Podium owns reads, navigation
owns selection, and retained window workspaces own terminal presentation. Only
this report changes; no API, DTO, type, field or persistence path was removed.

Traced RegistryQuery → PodiumModel → WorkspaceProjection → WorkspaceNavigator's
single/multiple Session actions → SessionsView.openSession → retained registry,
WorktreeLayoutStore, MultiplexerStore and SessionsStore. Compared Rust/Swift
SessionRecord, RoadmapProject/Task, TaskWorkspaceSnapshot and Activity fields;
inspected Session fixtures and Swift fixture assertions. Searches still find one
Podium caller per inventory read, one production root registry, and none of the
previously removed scope/navigation types or obsolete activity kind.

Candidates retained:

- The picker carries the existing Session ID into the existing open action.
  Extracting its two button presentations would remove no identity or policy.
- Outer checkout slots can hide retained inner terminals. `knownPaths` retains
  repository membership for hidden and shell-only groups; window-wide registry
  keys and Session cwd cannot replace it. Inner Close records Undo, while Session
  reconciliation clears stale Undo. These are different operations.
- Work attribution, checkout location and actual terminal attachment remain
  independent shared facts. Pane selection also cannot replace native focus:
  the configured receipt explicitly proves only the former.
- Prepared commands, completion errors and last-good readings retain different
  lifetimes. Mounted inventory reconciliation and asynchronous completion cover
  different delivery times; deleting either alone would not consolidate cleanup.
- SessionRecord still lacks shared legal actions/display path. Moving Swift's
  policy into a helper would preserve its authority, not complete LOO-284.

Inspected the [configured receipt](configured-ui-evidence/iteration14/receipt.json)
and passing log. All five recorded Swift source hashes still match. That prior
trial proves exact picker selection, two checkout groups, four retained PTY
children and UI completion; it does not prove keyboard focus, nested drafts or
shell responses. No executable content changed, so no tests were rerun.
`git diff --check` passes. Shared Session integration, the human-selected external
edit/trials and measured budgets remain open. No app/provider interaction,
publication, landing or Task completion occurred in this compression pass.

### Iteration 13 — configured editor Cancel path (2026-09-24)

No coherent model/API reduction was established. The effective model above is
unchanged before and after this pass; only this report changes. No API, DTO,
field, type, persistence path or configuration key was removed.

Traced TaskDirectiveEditor → PodiumModel → RegistryQuery → `pm task update`
→ Rust ownership resolution, provider update and snapshot refresh → shared
roadmap → WorkspaceProjection. Compared Rust/Swift SessionRecord,
RoadmapProject/Task, TaskWorkspaceSnapshot and Activity fields. Inspected the
retained workspace/Session state boundaries and the current directive-test and
configured editor receipts. Searches still find one Podium caller per inventory
read, one root workspace registry, and none of the previously removed navigation
types or obsolete activity kind.

The suspected reductions would discard distinct facts:

- Initial provider text, the unsaved draft and authoritative readback can differ.
  The configured Cancel/reopen result now exercises that distinction. Captured
  Task/Wave targeting prevents navigation from redirecting Save; it is not a
  second planning record.
- Submission feedback belongs to the sheet, transport to RegistryQuery, and
  read publication to Podium. Busy state cannot invalidate an already-running
  poll; the generation is still necessary. An operation wrapper would relocate
  these responsibilities without removing an owner.
- Work attribution, local checkout existence and actual terminal attachment
  remain independent evidence. Derived Active membership does not replace any
  shared field. Outer checkout placement, retained inner terminals and prepared
  Session commands likewise retain different lifetimes and close behavior.
- SessionRecord still lacks LOO-284's shared legal actions/display path. Moving
  the existing Swift policy into a helper would preserve its authority.

The four editor source/test hashes match both recorded editor receipts. Inspected
the existing three-test pass and iteration 13's configured exact-text entry,
Cancel/reopen and unchanged-planning pass. Those are prior receipts, not new
tests in this compression pass. No executable content changed, so no tests were
rerun; `git diff --check` passes.

The configured receipt does not prove Save/rejection interaction, keyboard focus
or an authorized external edit. This pass performed no app/provider interaction.
Shared Session integration, configured nested-workspace proof, human-selected
external trials and performance budgets remain open. No publication, landing or
Task completion occurred.

### Iteration 12 — directive editing and shared evidence

No coherent model/API reduction was established. Before and after this pass,
the implementation has the same owners; only this report changes. No API, DTO,
field, type, persistence path or configuration key was removed.

Traced TaskDirectiveEditor → PodiumModel → RegistryQuery → `pm task update`
→ Rust PM ownership resolution, provider update and snapshot refresh → shared
roadmap → WorkSurfaceView/WorkspaceProjection. Compared Rust/Swift SessionRecord,
RoadmapProject, RoadmapTask, TaskWorkspaceSnapshot and Activity fields, including
`local_exists`, checkout location and terminal attachment. Inspected their fixture
assertions, directive tests, Active membership and Session discovery. Searches
retain one Podium query caller per inventory and one root workspace registry;
the previously removed navigation types and obsolete activity kind remain absent.

Suspected reductions retain distinct facts:

- The sheet's draft can differ from both the captured initial directive and
  refreshed provider text. Deriving it from planning would discard unsaved input;
  publishing it into planning would invent an optimistic authority. The captured
  Task/Wave also prevents navigation from redirecting the write. It reuses the
  existing selection wrapper rather than introducing another target model.
- Busy state and read generation answer different questions. The existing
  refresh flag cannot invalidate a poll already awaiting a response when Save
  succeeds. The generation protects the authoritative read; removing it restores
  the tested stale-poll failure. Transport, refresh publication and sheet feedback
  already sit at their owning boundaries. Moving them into a new operation
  wrapper would add vocabulary without removing an owner.
- Local worktree existence, owned provider activity and unresolved Sessions are
  independent Active-membership evidence. A stored checkout path does not prove
  its existence, attribute a provider to Work, or identify its terminal. Likewise,
  interactive history must survive client exit; the retained client namespace
  cannot be replaced with a current-liveness check. Resolving declared issue
  selectors uses the existing Work resolver, not a title/cwd association.
- Session opening/prepared state, completion errors and retained pane placement
  remain independent of planning edits. The editor supplies no shared Session
  legality or display-path contract; LOO-284 remains outstanding.

Inspected the iteration 12 receipt: three tests pass, covering rejected writes,
unavailable readback, captured targeting, authoritative text and stale-poll
suppression. SHA-256 values of RegistryQuery, PodiumModel, WorkSurfaceView and
TaskDirectiveEditorTests still match `editor-hashes.json`. No executable content
changed and no tests were rerun. `git diff --check` passes.

The receipt does not prove mounted editor draft retention or a real PM edit.
This pass performed no app/provider interaction and leaves the ongoing human demo
untouched. Configured editor/nested-workspace proof, shared Session integration,
human-selected external trials and performance budgets remain open. No publication,
landing or Task completion occurred.

### Iteration 11 — remove the obsolete activity node kind

Rust's `ActivityNodeKind` contains Exec and ProviderProcess, including at the
active PR base. Swift additionally accepted ProviderLaunch. Its only caller
was a compatibility test with an inline installed-Home payload; no current
producer, UI behavior or shared fixture used it. Removed the Swift enum case
and that test. The mirrors now expose the same two node kinds. This removes
one obsolete wire value, not a command, storage record, migration or launch
capability. Unsupported activity payloads surface through the existing read
error path instead of being silently accepted as another kind.

Traced Rust event/receipt → ActivitySnapshot → RegistryQuery → PodiumModel →
WorkspaceProjection/TaskQuery → navigator and Wave details. Compared every
ActivitySnapshot, ActivityNode, ProviderProcess and SessionRecord field across
Rust/Swift, alongside RoadmapProject/Task and unavailable-Work evidence. Inspected
the activity fixture/round-trip, query contract and Active tests. Followed the
existing Session opening/completion state and window-local checkout registry.
Searches retain one Podium reader per inventory and one root registry; the
obsolete activity kind and previously removed navigation types are absent.

Further suspected reductions would erase independent facts: a live provider's
checkout does not supply Work attribution or native attachment; the derived
Task boolean is consumed by both filtering and its explicit location label.
TaskQuery names two actual views, not another inventory or persistent query
language. Planning completeness is now shared by count, visibility and emptiness
within the navigator. Last-good readings, prepared commands and completion errors
retain separate lifetimes; outer checkout slots and inner terminal panes retain
different close behavior. No broader ownership rewrite is justified by this
contract correction. LOO-284's shared actions/display path remain outstanding.

Focused verification: `RegistryQueryTests/activityDecodes` and
`WorkspaceNavigationTests/workDoesNotRequireSessions` pass (two tests,
`/tmp/loo291-compress11-activity.log`). The initial filter misspelled the DTO test
name and did not run it; the corrected `DTOFixtureTests/activityFixtureRoundTrips`
passes separately (one test, `/tmp/loo291-compress11-fixture.log`). Both commands
use `swift test --package-path swift -Xswiftc -gnone --jobs 4 --filter …`.
No executable edits followed; `git diff --check` passes. No broad gate or
configured trial ran. Other working changes, including the Rust receipt-path
correction, are preserved and are not claimed as this pass's work. The ongoing
human demo and remaining full-Task proof obligations are unchanged.

### Iteration 10 — Session-row return and planning diagnostics

Reviewed HEAD `9b3264efd` and the working Wave-detail diagnostic. The effective
model above is unchanged. No coherent structural reduction was established;
this pass changes only this report. No API, DTO, field, type, persistence path
or configuration key was removed. Existing working changes remain intact.

Traced RegistryQuery → PodiumModel → WorkspaceProjection/Navigation →
WorkspaceNavigator/WorkSurfaceView → SessionsStore and the retained checkout
registry → WorktreeLayoutStore/MultiplexerStore → native surface ownership.
Read scoped ConversationLaunch and the Rust provider-client attachment path.
Compared Rust/Swift SessionRecord, WaveRoadmap, RoadmapProject/Task and
UnavailableProject/TaskEvidence fields, both shared Session fixtures and their
Swift contract coverage. Searches retain one Podium caller per inventory read,
one production root registry, and none of the previously removed scope types.

Suspected reductions retain necessary distinctions:

- Work outside a readable plan and failed/partial planning reads are different
  evidence. Moving the former into Wave details uses the existing shared DTO;
  collapsing it into a read error or a new local projection would lose meaning
  or add another representation. Recorded stranded Tasks remain visible.
- Planning identity, runtime Work identity and actual terminal attachment have
  different owners. Upcoming Tasks cannot derive identity from a Session;
  matching cwd cannot identify its native surface.
- Outer checkout slots and inner terminal panes have different close behavior.
  Per-repository known paths preserve hidden and shell-only groups that Session
  inventory and window-wide registry keys cannot reconstruct.
- Last-good reads, prepared launches and completion errors have independent
  lifetimes. A rejected Complete must coexist with a responding terminal.
  Mounted inventory observers and asynchronous action continuations still cover
  different cleanup delivery times; removing one is not ownership consolidation.
- LOO-284's shared legal actions/display path remain absent. Moving Swift policy
  into another helper would not reduce its authority. The new Task-view query
  proposal is unresolved design, not an implemented model to collapse toward.

Inspected the existing iteration 10 native receipt: one Session-row/nested-layout
test passes with four real PTYs and fixture records. Its SessionsView and test
hashes still match `adaae303…257fc8d` and `4025c72d…457f60a4`. Also inspected the
other writer's `/tmp/loo291-planning-detail-proof.log`: diagnostic relocation
and failed-read retention both pass. Neither receipt is a new test run here.
No executable content changed in this pass; no tests were rerun.
`git diff --check` passes.

The configured launch and foreground limits remain in
[iteration10-implement.md](iteration10-implement.md). This pass did not interact
with the ongoing human demo or replay a retired Session. Configured row/nested
interaction and full Task obligations remain open; no publication or completion
is established.

### Iteration 9 — configured viewport retention

Reviewed HEAD `560783243` and the existing working completion correction.
The effective model above is unchanged before and after this pass. No coherent
model/API reduction was established; only this report changes. No type, field,
DTO, command, persistence path or configuration key was removed.

Traced RegistryQuery → PodiumModel → WorkspaceProjection/Navigation →
SessionsView/Store → retained checkout registry, outer WorktreeLayout and inner
MultiplexerStore → native surface ownership. Inspected scoped conversation
launch, shell attachment, completion, hidden Undo and repository return.
Compared Rust/Swift SessionRecord and RoadmapProject/Task fields, both shared
Session fixtures and their Swift round-trip assertions, and the CLI
list/open/complete/FlowStep contracts. Followed terminal attachment from the
verified provider-client PTY through Rust's projection to the window-local pool.
Negative searches retain one Podium caller per inventory read, one production
root registry, and none of the removed navigation/scope types.

The suspected reductions still remove necessary distinctions:

- Planning identity exists before runtime Work. Work attribution, checkout cwd
  and actual terminal attachment cannot substitute for one another.
- Outer checkout slots and inner terminal panes have different close behavior.
  Per-repository `knownPaths` retains hidden and shell-only groups; neither the
  window-wide registry keys nor Session cwd inventory supplies that membership.
- Last-good readings, prepared launches and rejected completion errors have
  independent lifetimes. A completion rejection must coexist with the live
  terminal and survive polling; combining it with opening failure restores the
  reproduced defect.
- Mounted inventory observers and asynchronous action continuations cover
  external resolution and completion after unmounting. Removing one delivery
  path without consolidating all retained-workspace cleanup would lose behavior.
- Selected pane, native focus request and AppKit first responder remain distinct.
  The configured wheel proof uses Ghostty's existing scroll path and retained
  view; it introduces no second viewport controller to remove.
- LOO-284's shared legal actions/display path remain absent. Relocating Swift's
  existing policy would not reduce its authority or satisfy that contract.

Inspected the iteration 8 focused receipt: two tests/four cases pass. Current
production/test hashes still match `adaae303…257fc8d` and
`63042239…286db1`. No executable content changed, so no tests were rerun.
`git diff --check` passes.

The subsequent configured provider proof and iteration 9 viewport receipt retain
their stated scope in [configured-ui-proof.md](configured-ui-proof.md) and
[iteration9-proof.md](iteration9-proof.md). This pass launched no app or provider
and replayed no retired proof Session. Session-row return, nested checkout
interaction, other destinations and full Task trials/budgets remain open. The
latest recorded input boundary is Warp owning AX focus, not missing permission
or a locked desktop. No publication or Task completion is established.

### Iteration 8 — completion errors and retained terminals

Reviewed HEAD `560783243` plus the working completion correction. Before and
after this compression pass, the effective model is unchanged. No coherent
model/API reduction was established; this pass changes only this report. No
field, type, route, DTO, persistence path or configuration key was removed.
Existing implementation and other writers' changes remain intact.

Traced RegistryQuery → PodiumModel → WorkspaceProjection/Navigation →
SessionsView/Store → retained workspace registry and MultiplexerStore. Inspected
direct and shell-attached completion, inventory reconciliation, repository
unmount, surface release and hidden Undo. Compared every SessionRecord and
RoadmapProject/Task field between Rust and Swift, read both shared Session
fixtures and the list/open/complete/FlowStep query contracts. Negative searches
still find one Podium caller per inventory read, one production root registry,
and none of the removed navigation/scope types listed in earlier reviews.

Suspected reductions retained deliberately:

- `completionError` and opening `.failed` cannot become one mutually exclusive
  state. Rejected completion coexists with a live terminal; attachment polling
  updates that terminal state without resolving the rejected action. Combining
  them restores the reproduced error-loss defect. The error is retained UI
  state, not another shared lifecycle or wire field.
- `isCompleting` describes an in-flight action; the retained error describes its
  unsuccessful result. Neither derives the other. Moving the busy flag into a
  new operation model would add vocabulary without deleting an owner.
- A shell can carry multiple independently completable Sessions. The derived
  `completionItems` collection preserves their identities; treating the pane
  as one Session or choosing its first attachment loses existing capability.
  Both direct and attached actions already use one completion operation.
- Action continuations and mounted inventory observers cover different delivery
  times. Consolidating cleanup requires covering external disappearance,
  repository unmount, hidden layouts and FlowStep decisions together. Deleting
  one call site or extracting a partial helper does not establish that ownership.
- Work attribution, checkout grouping and terminal attachment remain separate
  facts in the shared DTO. Shared Session legal actions/display path remain
  absent; moving the local policy into another helper would not supply LOO-284.

Inspected iteration 8's [before](configured-ui-evidence/iteration8/rejection-before.log)
and [after](configured-ui-evidence/iteration8/rejection-after.log) receipts. The
existing focused command selects `shellSessionCompletion` and
`workspaceRetainsNativeSplit`: two tests/four cases pass after correction.
Current production/test SHA-256 values still match that proof's recorded
`adaae303…257fc8d` and `63042239…286db1`. No executable content changed and no
tests were rerun. `git diff --check` passes. These remain native PTY proofs with
fixture records and mocked completion, not configured-provider evidence.

The locked-desktop boundary in `iteration8-proof.md` was not retried during
compression. Full Task scope, configured proof gaps and publication disposition
remain unchanged.

### Iteration 7 — integrated checkout and terminal ownership

Reviewed HEAD `b12beba8b` and the working integration, including the concurrent
appearance, fixture and completion corrections. No coherent model/API reduction
was established. This pass changes only this report: no fields, types, routes,
DTOs, persistence paths or configuration keys were removed. Other working-tree
changes remain their existing writer's work.

Traced RegistryQuery → PodiumModel → WorkspaceProjection/Navigation →
ConversationLaunch and SessionsView → retained registry, outer WorktreeLayout,
inner MultiplexerStore and native surface pool. Followed direct Session opening,
shell attachment, repository return, hidden groups, completion and Undo. Compared
every SessionRecord and RoadmapProject/Task field between Rust and Swift; read
the shared Session fixtures and Swift round-trip assertions, query contract,
provider-client receipt writer and actual-PTY check. Read the workspace/scope
tests and the native shell-attachment assertions. Also traced the integrated
current-Wave predicate through ls/roadmap and forget through its transactional
storage boundary. Negative searches still find one Podium caller per inventory
read, one production root registry, and none of the removed navigation/scope
types listed in earlier reviews.

Suspected reductions retained deliberately:

- Work identity, `cwd` and `terminal_ids` are independent facts: attribution,
  checkout grouping and active client attachment. A shell may change directory
  or launch successive providers; another window cannot acquire its surface by
  matching Work or cwd. The terminal marker and verified PTY are both necessary
  to reject inherited markers after an external handoff. No DTO field is merely
  an alias for another in this path.
- Outer and inner split trees have different leaves and close semantics. Hiding
  a checkout retains its terminals; closing a shell ends it. `knownPaths` also
  retains hidden membership within one repository, whereas registry keys span
  the whole window and Session cwd values omit shell-only groups. Deriving all
  three from one inventory would lose that distinction.
- ConversationScope includes repository entry and captures launch arguments;
  navigation uses planning identity before runtime Work exists. Replacing either
  with Session attribution would lose upcoming Tasks or repository conversations.
  The small launch wrapper does not introduce another lifecycle or authority;
  moving its functions alone would not simplify the model.
- Last-good readings, prepared commands and opening errors have different
  lifetimes. Likewise, native title retention survives remount while the pane's
  title state triggers rendering. Removing the latter requires a complete native
  observation change, not deletion of the retained metadata. Multiplexer revision
  notifications still bridge its existing owner into SwiftUI.
- Mounted inventory observers and asynchronous action continuations deliver
  cleanup at different times. The concurrent Complete correction uses
  reconciliation rather than undoable close. Deleting one delivery path would
  not consolidate ownership across repository unmount, hidden checkout groups,
  external disappearance and FlowStep decisions. No partial cleanup extraction
  was made on top of that correction.
- Current-Wave filtering already has one shared predicate. Forget is a separate
  exact-registration mutation: filesystem/live checks and transactional stored
  history checks protect different facts. A navigation filter cannot substitute
  for deletion eligibility. No storage fossil or migration reduction was found.
- Shared Session legal actions/display path remain absent from this DTO. Moving
  Swift's existing policy into another helper would preserve its authority,
  not accomplish LOO-284 integration.

No tests were run because this pass changes no executable content. Inspected the
concurrent `lf-new-implementation/reconciliation-after.log`: two tests/four cases
pass for mounted completion and hidden Undo. That is the other writer's focused
native/model receipt, not a new compression gate or configured-provider proof.
Iteration 7's configured provider, repository and timing receipts retain their
recorded pre-integration binary identities; they do not validate this integrated
working tree. Full Task proof and publication disposition are unchanged.

### Iteration 6 — one Session-opening result path

Reviewed HEAD `9cfaae907` and the configured proof. Before, `select`, `moveHere`
and `recover` both published opening results into SessionsStore and returned
an optional SessionRecord. Every production caller discarded that return.
`requestedSessionId` tracked the latest request only to suppress an obsolete
return value; it never guarded record publication or selected a pane.

After, opening publishes only through the existing per-Session state. Removed
`requestedSessionId` and the three optional returns; made `recover` private and
consolidated its existing state guard instead of repeating it in `select`.
Updated every UI caller and the focused tests to observe the retained prepared
record/error. Pane selection still happens before the asynchronous open through
MultiplexerStore. An older opening result can prepare its own Session without
moving the selected pane, exactly as before. No DTO, CLI command, shared legal
action, persistence path or provider launch behavior changed.

Inspected LocalWaveAgentLauncher → RegistryQuery → PodiumModel →
WorkspaceProjection/Navigation → SessionsStore → MultiplexerStore/surface pool,
including open, explicit Move here, polling, completion and hidden Undo cleanup.
Compared Rust/Swift SessionRecord and RoadmapProject/Task fields and their fixture
assertions. Negative searches retain one Podium caller per inventory read, one
root workspace registry, and none of the removed navigation/scope types.

Other suspected reductions remain inappropriate: last-good readings and
prepared/error state have different lifetimes; ambient Work removal belongs at
the explicit CLI boundary rather than general PATH enrichment used by terminals;
query execution preserves doctor's nonzero JSON result while checked controls
require success. Removing those distinctions or moving absent LOO-284 policy
between helpers would not reduce ownership. Completion cleanup still needs a
complete retained-workspace treatment before any delivery path can be deleted.

Configured provider continuation/completion now has real evidence, superseding
the earlier generic AX gap. Exact draft fidelity, the unobserved launch,
configured repository/scroll/visual checks and controlled timings remain open in
`configured-ui-proof.md`. This reduction does not reinterpret those receipts or
replay the completed proof Session.

Focused command: `swift test --package-path swift -Xswiftc -gnone --jobs 4
--filter 'SessionsStoreTests/(opensOnlyTheSelectedSession|interactiveSelectionRequiresExplicitMove|reconcilePreservesPreparedLaunch|failedOpenSurvivesPolling)'`.
Four tests (five cases) passed, exit 0; `/tmp/loo291-compress-opening-state.log`.
No Swift edits followed. No broader gate or configured trial was rerun. The
review bundle/receipts still identify the preceding implementation binary;
this pass does not relabel them as a fresh configured result.


### Iteration 5 — completion after repository navigation

Reviewed HEAD `100c518c6`. The effective model is unchanged; no coherent
model/API reduction was established. This pass changes only this report. No
type, field, DTO, route, persistence path or configuration key was removed.

Traced WorkspaceProjection through Podium's per-repository readings, retained
SessionsWorkspace/Store, mounted observers, Complete and FlowStep continuations,
and MultiplexerStore reconciliation/Close view/Undo. Compared Rust/Swift
SessionRecord and roadmap Project/Task fields and RegistryQuery's Session
contract. Searches still find one Podium caller per inventory read, one root
workspace registry, and none of the removed navigation/scope types.

The new two-case native proof closes the earlier Complete-after-unmount coverage
gap. It also covers Undo after Complete, superseding iteration 4's coverage note.
It does not cover FlowStep resolution after unmounting or external resolution
of a Session already hidden by Close view. Cleanup consolidation must encompass
those paths at the retained workspace boundary; deleting an observer or action
tail alone would remove a distinct delivery path. No partial helper extraction
is warranted. Podium's read invalidation and SessionsStore's prepared/error state
still have different lifetimes. Likewise, Complete and FlowStep decisions remain
distinct shared operations, and the absent LOO-284 action/display contract cannot
be replaced by moving Swift policy between helpers.

Inspected `/tmp/loo291-completion-repository-proof.log`: the focused
`WorkspaceNavigationProofTests/workspaceRetainsNativeSplit` test passed both
serialized cases. No executable content changed and no tests were rerun. This
remains native fixture evidence with a mocked CLI response; configured provider
interaction, caller release, visual quality and timing obligations remain open.
Full LOO-291 scope and publication disposition are unchanged.

### Iteration 4 — completion reconciliation

Reviewed HEAD `1013225cb`. The effective model above is unchanged. No coherent
model/API reduction was established; this pass changes only this report. No
type, field, route, DTO, persistence path or configuration key was removed.

Traced the mounted Complete action through RegistryQuery, SessionsStore removal,
PodiumModel's repository-specific reading/generation update, pane reconciliation
and native surface release. Read the corresponding FlowStep decision path,
SessionsWorkspace lifetime, MultiplexerStore close/undo/reconcile behavior, and
the new completion assertions. Compared Rust/Swift SessionRecord and roadmap
Project/Task fields, RegistryQuery's list/open/complete/resolve contract, and the
shared fixture assertions. Negative searches still find one Podium caller per
inventory read, one root workspace registry, and none of the removed navigation
or scope types.

The suspected reductions need distinct treatment:

- Reading removal and presentation removal are not duplicate Session authority.
  Podium invalidates an in-flight read and updates the originating repository;
  SessionsStore retains prepared commands and opening/error state. Combining
  them would erase the separation between shared evidence and local presentation.
- Surface/pane cleanup is repeated across action continuations and mounted
  observers. Deleting one call site alone is not a coherent reduction: initial
  mounting, external disappearance, and an action finishing after repository
  navigation have different delivery paths. Consolidation belongs at the retained
  workspace boundary and must cover all of them together. The current mounted
  completion proof does not cover completion after unmounting or Close-view Undo.
  In particular, `close` records undo and chooses the nearest pane, whereas
  `reconcileSessions` clears undo and chooses the first surviving pane. Treating
  these methods as interchangeable would change behavior. This pass defers that
  larger ownership change rather than merely extracting a cleanup helper.
- Complete and FlowStep decisions remain separate shared operations. Their short
  success/error tails do not justify a new operation abstraction or a local
  legality model. LOO-284's projected action/display contract is still absent.
- The completion extension reuses the existing mounted native fixture, its
  children and workspace. Splitting it into another harness would duplicate the
  navigation setup; its assertions establish Task and companion survival beyond
  the existing store-level completion checks.

Inspected `/tmp/loo291-mounted-completion-proof.log`: the final focused
`WorkspaceNavigationProofTests/workspaceRetainsNativeSplit` run passed one test.
No executable content changed and no tests were rerun. This remains a real native
workspace with a mocked completion response, not configured provider continuation
or caller-release evidence. Configured interaction, visual quality, timing and
the remaining full LOO-291 scope are unchanged.

### Iteration 3 — navigator retention

Reviewed HEAD `007c48664`. No coherent code reduction was established. The model
above remains unchanged by this pass; no API, DTO, field, type, route, persistence
path or configuration key was removed. Only this report changes.

Traced `listScrollOffset` from WorkspaceNavigation through PodiumModel's retained
repository navigation to WorkspaceNavigator's initializer and geometry callback.
Read the mounted `navigatorRetainsScroll` regression, SessionsView's repository
workspace and pane observation, MultiplexerStore's notification boundary, and
RegistryQuery's Session operations. Compared Rust/Swift SessionRecord and
RoadmapProject/Task fields and their shared fixture coverage. Searches still find
one desktop caller per inventory read, one root workspace registry, and none of
the removed scope/navigation types.

The retained scalar and mounted ScrollPosition are not interchangeable: the
scalar records observed geometry for a later mount; ScrollPosition supplies the
current view's scroll request. Its SDK contract includes optional point/edge
values and user-positioned state, not an always-present viewport offset. Moving
that binding into navigation would not establish equivalent restoration and
would carry framework control state across view lifetimes. The geometry callback
captures its render's navigation owner. No second scroll controller or row-ID
inventory is needed.

The new native-scroll fixture complements the retained-terminal proofs: it
exercises a long planning list and independent repository offsets without
requiring Ghostty. Combining these tests would couple distinct failure boundaries.
The existing layout/focus/zoom observation bridge still needs a complete
multiplexer observation change to remove coherently. Planning/runtime identity
and last-good/prepared state retain their separate meanings. Session policy still
awaits the absent shared LOO-284 action/display contract; relocating it would not
reduce its authority.

Inspected `/tmp/loo291-navigator-scroll-final.log`: the focused navigator test
passed once against this source. No executable changes or test reruns in this
pass. Local navigator retention is now covered; configured provider interaction,
resolution, visual quality and timing comparisons remain unproven. Full LOO-291
scope and publication disposition are unchanged.

### Iteration 2 — mounted-workspace proof

Reviewed HEAD `2108eaba5`. No further coherent reduction was established; this
pass changes only this report. The effective model above is unchanged. No API,
DTO, route, field, type, persistence path or configuration key was removed.

Re-read WorkspaceProjection/Navigation, PodiumModel, SessionsView/Store and its
workspace registry, MultiplexerStore's notification boundary, RegistryQuery's
Session operations, and the native focus/rendering seams. Compared the shared
Rust/Swift SessionRecord and RoadmapProject/Task fields and inspected their DTO
fixture coverage. Searches still find one desktop caller for each inventory read,
one root window registry, and none of the removed navigation/scope types.

The new `workspaceRetainsNativeSplit` fixture introduces no production owner or
test-only production API. Its query closure isolates registry side effects; its
native surfaces exercise the existing workspace. It complements rather than
replaces `hiddenTerminalPreservesDraft`: the latter covers search focus through
resize and manually focused Task terminals, while the former covers mounted
navigation, repository return, companion splits and actual viewport retention.
Combining them would obscure those distinct failure boundaries.

The layout/focus/zoom snapshots remain a notification-to-SwiftUI rendering bridge;
all mutations still belong to MultiplexerStore. Removing that bridge requires a
complete observation change across its consumers. The new fixture does not cover
close/undo, zoom or resolution timing, so it does not justify that wider change.
Planning/runtime identity, last-good/prepared state, and pane/native focus retain
the distinct lifetimes explained below. LOO-284's absent shared action contract
still prevents deleting the existing Session policy coherently.

Inspected the existing final receipt at
`/tmp/loo291-mounted-navigation-final.log`: one mounted-workspace test passed.
No executable content changed and no tests were rerun in this compression pass.
The fixture now proves native split and terminal-scroll retention; configured
provider interaction/resolution, navigator scroll, visual quality and timings
remain open. Full LOO-291 scope and publication disposition are unchanged.

### Earlier native-focus compression

Earlier compression removed duplicate selected Work, SessionScope and its
filter/resolution methods, pane-tree scope props, unused SessionItem aliases,
and the unreachable inspector sheet/Task-terminal observation. The root switch,
PodiumConsole, Sessions-only hierarchy read and independent polling remain absent.

The preceding native-restoration pass already made the native reduction: deleting the
SwiftUI focus Coordinator and moving its one request into the retained native
view. That compression pass found no further coherent structural reduction. It corrected one
behavior lost by that reduction: an enabled Task terminal has no selected-pane
binding (`isFocused` defaults false), but must keep focus acquired by clicking.
Previously every resize cleared that focus. The native update now receives
enabled and selected state separately, releasing first responder on disabling
or selection loss, not on an unchanged false selection. It retains one request
field and adds no coordinator or state owner. No route, DTO, event, schema or
configuration key changes.

## Paths and mirrors inspected

Read PodiumModel, WorkspaceProjection/Navigation, SessionsView/Store and its pane
props, root registry construction, GhosttyManager, GhosttyTerminalRepresentable,
GhosttyMetalView and pool, their native regression, and MultiplexerStore's
notification boundary. Checked TaskWorkspaceView and Session/shell terminal
callers. Compared SessionRecord and RoadmapProject/Task fields in Rust and Swift,
RegistryQuery list/open/resolve/complete methods, and shared Session/roadmap
fixture assertions. Rechecked the accepted design, wave context and native proof.

Negative searches still find one Podium caller for each shared inventory read
and one root window registry. Removed navigation/scope types and the focus
Coordinator remain absent.

## Intentional boundaries

- Planning identity must precede runtime Work identity; collapsing them would
  re-key upcoming Tasks when runtime appears. Derived workspace rows are not a
  second durable inventory.
- Last-good readings and prepared/error presentation have different lifetimes.
  A polling response must not overwrite a prepared replacement command.
- Pane selection, a native focus request and first responder differ: search may
  own keyboard input while a pane remains selected. Removing the request would
  restore focus theft on refresh or lose pre-attachment focus.
- The CoreVideo probe is transient and released. It configures Ghostty's existing
  timer path only on failure. It cannot merge with the view's CADisplayLink,
  which drives native draw/command-block refresh callbacks; deleting either based
  on the shared name would change rendering behavior without visual proof.
- Optional surface pools still serve two real callers: retained Session/shell
  panes and the existing Task terminal. Requiring a pool everywhere would expand
  ownership scope rather than remove a compatibility alias.
- Layout/focus/zoom snapshots still bridge the single MultiplexerStore notifier
  into SwiftUI. A complete Observation conversion could remove that bridge, but
  changes multiplexer update timing beyond the two newly proven native fixtures.
  No partial prop cleanup is justified before configured split/scroll proof.
- Session action policy now comes from the shared contract. The DTO's action
  descriptors and Work display path replace Swift's legality and label inference;
  local surface ownership and prepared commands remain presentation facts.
  Historical reviews above describe the contract before iteration 15.

## Earlier native-focus verification

Extended the existing real-PTY fixture with the Task terminal's manual-focus
case. Before the correction it failed after resize: first responder became
NSWindow. Receipt: `/tmp/loo291-compress-manual-focus-before.log`, exit 1.
The focused corrected result is recorded below. Earlier model/selection and
Session-action receipts remain in `review-slice.md`; none is a new compression
or configured-app result. The independent window-isolation pass remains in
`native-surface-diagnostic.md` and was not rerun for this focus-only correction.

Final command: `swift test --package-path swift -Xswiftc -gnone --jobs 4 --filter
WorkspaceNavigationProofTests/hiddenTerminalPreservesDraft`. One test passed,
exit 0; `/tmp/loo291-compress-manual-focus-after.log`. This includes retained
draft, hidden focus release, search focus through resize, and manual focus with
no selected-pane binding. No Swift edits followed the pass. `git diff --check`
also passes. No broader test gate was run.

Configured navigation, split/scroll retention, visual quality and timings remain
unproven. Full LOO-291 scope remains unchanged. Nothing was published, landed,
or marked complete by this pass.
