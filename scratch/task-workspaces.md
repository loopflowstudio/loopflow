# Work multiplexer and Task workspaces

October 15, 2026 — Jack Heart's sidebar review. Accepted direction; interaction
mechanisms below remain a draft for the clickable prototype, not launch approval.
Continues [native navigation](native-wave-task-navigator.md).

## What to build

A resizable navigation sidebar and an outer Work multiplexer containing retained
Task workspaces, each with its own inner pane multiplexer.

Jack: “there needs to be one selection state”; “two distinct levels of hierarchy”;
“the wave jumps into a bread crumb in the top row when you select a specific task”;
“forward/back nav was keyboard only”; “sidebar should also be draggable to resize”.

## Accepted direction

- One active Work selection. Remembered Wave scope never paints another selection.
- Search removes nonmatching Waves; Wave-name matches retain their Tasks. Search
  does not switch the active Work. Clearing it restores the navigation view.
- Selecting a Task moves its Wave into the sidebar's Repo / Wave breadcrumb and
  gives sibling Tasks the sidebar width. Collapse lives in the sidebar; a narrow
  edge control restores it. Remember width and enforce usable pane widths.
- No visible Back/Forward buttons; keyboard history remains.
- Separate Work and pane layout/focus. Shared layout mechanics may serve both;
  a Task never becomes a terminal pane and an inner split never adds another Task.

## The demo

[Prototype 04](sidebar-prototypes/04-task-workspaces.html): select a Task, resize
and collapse the sidebar, switch Waves and return to a draft. Show two Tasks,
split one internally, then focus the other: only one sidebar row is selected,
and each Task keeps its own panes and draft. Close an outer workspace and reopen
it; its inner state survives. Search `recovery`: unrelated Waves disappear.

## Data structures and key functions

Existing `WorkWorkspaceKey` remains Work identity. `SessionsWorkspace` already
owns `MultiplexerStore`, pane focus, presentation and document drafts. The
window's `SessionsWorkspaceRegistry` retains them and owns native surfaces.

Proposed `WorkLayout` leaves hold workspace keys, while inner `LayoutNode` leaves
hold pane IDs/content. Share split axes, ratios and divider gestures through a
small typed layout primitive only if both consumers need it. Keep terminal
lifetimes in the existing pool.

`showWork(key, placement: replaceFocused | besideFocused)` edits outer placement.
`focusWork(key)` updates the one Work selection and restores inner focus.
`splitPane(work, pane, axis)` changes only that workspace's inner layout.
`hideWork(key)` removes placement, retaining clients, layout and drafts.
`resizeSidebar(width)` changes only window navigation geometry.

## Current system / Delete — do not maintain

`SessionsWorkspaceRegistry.openWork` plus `selectedWork` records a flat open set
and one displayed workspace; `SessionsContentView` renders that single selection.
Replace flat placement with one outer layout, not a synchronized second list.
Reconcile the registry's focus and `WorkNavigation.selection` through the existing
WorkModel owner rather than adding a third selection. Retain per-Work history,
drafts, surface pool, remote execution identity and truthful Session status.

Remove `WorkBreadcrumbBar`'s always-visible hierarchy and Back button in the
single-workspace presentation, its exclusive chrome tests, and the fixed 430pt
sidebar sizing. Preserve keyboard actions, Task lifecycle/Workflow controls,
file access and all Session actions; move their presentation to their Task owner.

## Constraints and proposed mechanisms

The prototype hides outer Task headers when one Task is visible and the sidebar
is expanded. Multiple Tasks, or a collapsed sidebar, reveal a compact header per
workspace with its Wave and Task. Pane headers identify program/content only.
This conditional header policy remains reviewable.

Command-click to add Work beside the focused workspace, normal click to replace
the focused slot, and close-to-hide are prototype proposals. No live process
control, multiselect set, reorder behavior or persistent layout schema is approved.
No schema migration is currently planned.

## Forbidden outcomes

Highlighting a remembered Wave and selected Task/repo; flattening terminal panes
across Tasks; copying a Session surface into two mounts; losing drafts on hide;
using an outer click to create a shell; a global breadcrumb pretending to identify
several Tasks; search selecting work without a click.

## Internal slices

Indivisible presentation change: internal slices, one PR. **This slice:** prototype
and interaction review. Then replace flat placement with typed outer layout and
its minimum renderer cutover; move context/controls to workspace ownership and
remove redundant chrome; integrate responsive sidebar and nested keyboard focus.
Existing search/selection bug fixes proceed within LOO-462 independently.

## Done when

`scripts/test_desktop.sh --filter 'WorkNavigationTests|DesktopPaneControlTests|WorkNavigationProofTests' -Xswiftc -gnone` must build and pass. Focused tests prove independent
inner layouts, single selection, retained surface identity, keyboard history,
search and resize/collapse restoration. Prototype checks exercise the same
navigation interactions with fictional data. Native demo judges orientation,
usable minimum widths and focus; it does not substitute for those checks.

Open: exact add/replace gesture, workspace limit, mixed Repo/Wave/Task layouts,
and whether each repository restores an outer arrangement across launch.

Check: `uv run --with playwright python /tmp/loo462-check-workspaces.py` PASS — one selection, nested layouts, retained panes/drafts, search, resizing, collapse and reopen; no JavaScript errors or horizontal overflow. Single/multiple workspace screenshots reviewed.
