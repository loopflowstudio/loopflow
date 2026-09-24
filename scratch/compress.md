# Compression review — 2026-09-23

## Effective model

- Shared Rust/Swift `RoadmapSnapshot` and `SessionRecord` supply planning and
  human-work evidence through RegistryQuery. Their shared fields remain intact.
- PodiumModel owns readings, last-good evidence and read generations.
  WorkspaceProjection derives typed planning-to-Session associations, preserving
  upcoming Tasks and every unmatched human boundary. It stores nothing.
- WorkspaceNavigation owns selected Work, presentation, search and expansion per
  repository/window. PodiumModel forwards selection to that owner.
- SessionsWorkspaceRegistry retains each window's repository workspace.
  MultiplexerStore owns pane layout; GhosttySurfacePool retains native views;
  SessionsStore owns opening/prepared/error presentation and calls shared actions.
- GhosttyMetalView owns its surface and focus request across attachment and
  visibility changes. AppKit owns actual first responder. GhosttyManager owns
  library initialization and configuration, including the measured CoreVideo
  capability adaptation.

## Before and after

Earlier compression removed duplicate selected Work, SessionScope and its
filter/resolution methods, pane-tree scope props, unused SessionItem aliases,
and the unreachable inspector sheet/Task-terminal observation. The root switch,
PodiumConsole, Sessions-only hierarchy read and independent polling remain absent.

The latest implement pass already made the native reduction: deleting the
SwiftUI focus Coordinator and moving its one request into the retained native
view. This pass found no further coherent structural reduction. It corrects one
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

## Verification and remaining work

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
