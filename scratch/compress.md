# Watch model compression review — 2026-09-23

No further model or API reduction taken. Reviewed the snapshot and grouped output
contracts at `63bfcb9b7`, including directory-recovery code committed by another
run. This pass changes this report and removes a duplicate README example. No
executable changes, DTO removals, or fresh behavioral proof.

## Effective model, before and after

Unchanged: FlowPosition and its claim authorize execution. Task events retain
immutable plans and committed stage/attempt facts. Rust folds them into Watch
snapshots, joining Run manifests without reconstructing missing plans. Journals
and provider-native history own output. TaskOutputSource groups attribution,
ordered records, and source evidence; OutputRecord owns source identity, revision,
and ConversationEvent. Cursors hold private reader progress. Swift decodes the
projections and carries continuation through temporary files.

The earlier removal of TaskOutputRecord and the parallel records array remains
intact. Both commands already share read-only Task lookup and manifest discovery.
Swift reuses PlayheadStepKind and TaskFlowStage. No compatibility alias, second
transcript store, or Swift Watch reducer was found to delete.

## Path and mirrors inspected

Read `work/task/flow_history.rs`, Task event observation exclusion, transactional
history reads and the position writer in `store/sqlite/durable.rs`, and both
`ops/task_watch.rs` and `ops/task_output.rs`. Followed `ops/task.rs` read-only
lookup through CLI dispatch and human/JSON rendering. Reviewed `run_record.rs`
discovery, `native_source.rs`, `output.rs`, and `output/native.rs` continuation
and normalization.

Compared Watch/output DTO fields with `TaskWatch.swift`, `TaskOutput.swift`, both
shared JSON fixtures, and Rust/Swift fixture assertions. Read RegistryQuery's
typed reads and temporary-file transport, the shared PlayheadStepKind, CLI
examples, planning documentation, and the active design. Searched for obsolete
TaskOutputRecord and TaskWatchStepKind names; neither remains.

## Candidates left intentionally

- **Attempts and Runs:** attempts retain entry/readiness/failure and separate
  retries. The Run list also represents auxiliary Runs and bindings whose plans
  are missing. Deriving either exclusively from the other loses evidence.
  Output carries attribution so each page is independently interpretable.
- **State, settlement, active stage:** replacement can leave a bound attempt;
  invocation completion is separate from Task completion. Active position cannot
  be inferred from the last event. Ready summaries and failures survive later
  states, so payload presence cannot replace the attempt-state enum.
- **Stage coordinates and edges:** TaskFlowStage is shared by receipts, active
  position, Run bindings, and transitions. Omitting invocation IDs inside edges
  would introduce another coordinate type and conversions just to save repeated
  fields. TaskWatchTransition restricts the projection to edges; exposing the
  entire TaskFlowEvent enum would move filtering to clients. Kind, human policy,
  authored node ID, and expanded ancestry describe different plan facts.
- **History/discovery envelopes:** TaskFlowHistory holds one transaction's
  position and events; unavailable storage differs from no active position.
  TaskRunManifests carries healthy manifests with attribution uncertainty.
  Neither is a one-object wrapper or parallel store. An enum replacing private
  availability fields alone would remove no owner or public concept.
- **Evidence fields:** page-level discovery gaps may have no known source.
  Source availability, reset, pagination, and malformed records are independent
  observations; an empty page cannot stand in for them.
- **Output identities:** journal protocol does not identify provider. Native
  calls/results share conversation identity but remain separate source records;
  revisions identify changing normalized content. Collapsing these would undo
  the tool-correlation repair. SourcePage owns successful reader continuation;
  TaskOutputSource adds attribution and unavailable-source evidence.
- **Provenance and continuation:** launch-owned location, file identity, Session
  identity, verified headers, and byte anchors serve different checks. OpenCode
  watermark, frozen ceiling, page position, and boundary/unfinished hashes prevent
  different loss/replay cases. A tagged private cursor alone adds variants without
  solving bounded discovery or retained-state growth. Revisit its shape with
  independent history/live continuation. Swift's query-scoped transport file is
  not durable watcher state and should not become a persistent cache.

## Verification and remaining work

No tests, builds, or lint rerun: executable behavior is unchanged. Documentation
whitespace checked with `git diff --check`. Earlier focused passes remain dated
implementation receipts, not fresh validation from this pass.

Independent history/live continuation, bounded discovery/state, complete capture,
the Mac Watch surface, and the configured demonstration remain required in this
PR. Directory recovery is present by source inspection; its behavioral proof
belongs to the repair pass. This review establishes no polling readiness,
publication readiness, or Task completion.

---

# Retained main-view-task evidence (LOO-291)

# Compression review — 2026-09-23

## Effective model

- Shared Rust/Swift `RoadmapSnapshot` and `SessionRecord` supply planning and
  human-work evidence through RegistryQuery. Their shared fields remain intact.
- GUI process preparation clears inherited execution and terminal markers while
  preserving Home/account selection. CLI queries supply explicit scope;
  ConversationLaunch captures the visible subject and honors lf's destination.
- PodiumModel owns readings, last-good evidence and read generations.
  WorkspaceProjection derives typed planning-to-Session associations, preserving
  upcoming Tasks and every unmatched human boundary. It stores nothing.
- WorkspaceNavigation owns selected Work, presentation, search, expansion and
  list scroll offset per repository/window. PodiumModel forwards selection to
  that owner; SwiftUI controls the mounted navigator's scrolling.
- SessionsWorkspaceRegistry retains checkout workspaces and repository-level
  WorktreeLayoutStores inside one window, sharing that window's surface pool.
  Outer slots select checkouts; each checkout's MultiplexerStore owns terminals.
  The repository's retained SessionsStore owns opening/prepared presentation and
  completion errors, and calls shared actions. Rust projects actual client
  terminal attachment.
- GhosttyMetalView owns its surface, latest title and focus request across attachment and
  visibility changes. AppKit owns actual first responder. GhosttyManager owns
  library initialization and configuration, including the measured CoreVideo
  capability adaptation.

## Before and after

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
- Session action policy still awaits LOO-284's shared contract. The current DTO
  has no legal-action/display-path fields; moving the local policy to another
  helper would preserve the duplication rather than remove it.

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
