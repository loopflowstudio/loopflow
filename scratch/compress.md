# Compression review — 2026-09-23

## Effective model

- Shared Rust/Swift `RoadmapSnapshot` and `SessionRecord` supply planning and
  human-work evidence through RegistryQuery. Their shared fields remain intact.
- The existing Mac CLI process boundary supplies explicit Work targets and
  removes ambient Wave context. General GUI PATH enrichment remains separate.
- PodiumModel owns readings, last-good evidence and read generations.
  WorkspaceProjection derives typed planning-to-Session associations, preserving
  upcoming Tasks and every unmatched human boundary. It stores nothing.
- WorkspaceNavigation owns selected Work, presentation, search, expansion and
  list scroll offset per repository/window. PodiumModel forwards selection to
  that owner; SwiftUI controls the mounted navigator's scrolling.
- SessionsWorkspaceRegistry retains each window's repository workspace.
  MultiplexerStore owns pane layout; GhosttySurfacePool retains native views;
  SessionsStore owns opening/prepared/error presentation and calls shared actions.
- GhosttyMetalView owns its surface and focus request across attachment and
  visibility changes. AppKit owns actual first responder. GhosttyManager owns
  library initialization and configuration, including the measured CoreVideo
  capability adaptation.

## Before and after

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
