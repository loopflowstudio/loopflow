# Work multiplexer and Task workspaces

October 15, 2026 — Jack Heart accepted Ink on Paper, requested selection in
Needs you, and authorized building the complete prototype into Swift. This
continues LOO-462's [native navigation](native-wave-task-navigator.md).

## What to build

A resizable drill-in sidebar and an outer Work multiplexer containing retained
workspaces, each with its own inner pane multiplexer.

Jack: “there needs to be one selection state”; “two distinct levels of hierarchy”;
“the wave jumps into a bread crumb in the top row when you select a specific task”;
“forward/back nav was keyboard only”; “sidebar should also be draggable to resize”.
Final styling: “Ink on Paper”; “selection also highlights in needs youwhen necessary”.

## Accepted behavior / The demo

October 15 native review — Jack Heart rejected the Wave selection relocating
into the header: “having the selection jump to the header when you click on a
wave is super jarring.” The drill-in replacement described below is the current
implementation, no longer an accepted navigation outcome. Ink on Paper, one Work
selection, Needs you mirroring and retained workspaces remain accepted.

Replacement under discussion: keep Wave rows in the sidebar and reveal Tasks
below their Wave, preserving the clicked row and existing expanded branches.
Wave selection opens its conversation; Task selection highlights only the Task
(and its Needs you appearance). Expansion is presentation state, independent of
Work selection. A task-count control could fold a branch without chevrons or
navigation. Jack authorized prototyping this interaction on October 15; native
implementation remains unapproved. [Study 06](sidebar-prototypes/06-stationary-waves.html)
implements stationary Wave rows, independent branch folding through Task counts,
retained drafts and mirrored Needs you selection. The alternative
is Figma's stacked sections: a persistent Wave index above the scoped Task list.
Tradeoff: inline branches consume vertical space; stacked sections separate
Tasks spatially from their Wave. No return to adjacent sidebar columns is proposed.

Repository entry shows Needs you and a single Wave list. A Wave opens its
conversation and replaces that list with Tasks. Repo / Wave breadcrumbs return
to their conversations; selecting a Task keeps its siblings visible. Search hides
nonmatches without navigating. Each Wave retains its Task-list scroll position.

Burgundy fill and cream text mark selected Work, with no edge stripe. Ancestors
are context. Needs you mirrors the same selection; it also recognizes the exact
Session retained in a different workspace. A selected overflow entry remains
visible in the three-row queue. These are multiple representations of one
selection, not independent selected values.

Command-click opens Work beside the focused workspace. Ordinary click replaces
the focused slot, or focuses Work already visible. Close hides a workspace while
retaining its panes, surfaces and file drafts. Each inner split stays within its
Work. Narrow outer layouts reflow vertically. Context headers appear only with
multiple workspaces or a collapsed sidebar. Primary single-workspace views have
no middle breadcrumb. Secondary Sessions retain rename/bind actions.

Sidebar collapse belongs to the sidebar; the edge control restores it at its
remembered width. Drag adjusts width within 260–600pt while reserving content
space. Command-[ / ] traverse history without visible Back/Forward buttons.

Demo: open Product → Task, type a draft, Command-click another Task, split a pane
in one workspace, switch focus, resize/collapse, hide/reopen a workspace, and use
keyboard history. Needs you and the Task list highlight the same selected Work.
[Prototype 04](sidebar-prototypes/04-task-workspaces.html) mirrors this direction;
[study 05](sidebar-prototypes/05-selection-comps.html) retains the alternatives.

## Data structures and key functions

`WorkWorkspaceKey` identifies whole Work. `WorkLayout` is a binary split tree
whose leaves hold these keys. `SessionsWorkspaceRegistry.layout` replaces flat
placement; its `openWork` is derived. Existing `selectedWork` and WorkNavigation
remain connected through WorkModel's selection path, not another selection owner.

Each `SessionsWorkspace` retains its `MultiplexerStore`, pane focus, presentation,
documents and pooled surfaces. `SplitDivider` shares only divider geometry and
an update closure; it cannot exchange Work keys and pane IDs.

- `select(reference, besideFocused:)` changes placement and one Work selection.
- `hideWorkspace(key)` removes placement, retains clients, and focuses surviving Work.
- `WorkLayout.replacing/removing/resizing` changes only outer geometry.
- `split(pane, axis:)` remains inside its owning workspace.
- `resizeSidebar(width, availableWidth:)` changes navigation geometry only.
- `isSelected(reference/attention)` reconciles planning/runtime aliases and exact Sessions.

## Current system / Delete — do not maintain

Removed persistent adjacent Wave/Task columns, remembered Wave-click destinations,
`selectWaveContext`, `navigatorWaveID`, `waveDestinations`, and their exclusive
column/restore expectations. Wave clicks now open Wave conversations.

Removed `WorkBreadcrumbBar`, its visible Back/collapse controls, and always-on
ancestor presentation. `WorkSessionToolbar` preserves secondary Session actions;
Task Details, Files, Flow logs, shell creation and zoom restoration belong to the
workspace. Fixed 430pt width is replaced by the resizable sidebar.

The registry's stored flat `openWork` is replaced by `WorkLayout`; terminal pool,
location ownership, retained documents, inspection and exact Session opening
survive. Desktop text inspection distinguishes visible unfocused Work from hidden
Work. No database migration or compatibility path. Review caught and fixed stale inactive Workflow controls, missing selection on attention entries, and prepared-checkout location precedence.

## Constraints / Forbidden outcomes

One Work selection, even when mirrored in the queue. No ancestor highlighting,
duplicate surface mounts, terminal panes shared between Work layouts, draft loss
on hide, navigation launching a Flow, or search changing selection.

The existing Work reader watches Workflow detail for the focused Task. Inactive
workspaces keep live panes and expose a focus action for Workflow controls rather
than presenting stale actions. Task controls can scroll horizontally in narrow
workspaces. Outer layout and sidebar width persist within the window, not across
app launches. Mixed Repo/Wave/Task placement uses the same layout; no extra mode.

## Internal slices / Done when

Indivisible presentation change, one PR. **This slice:** implemented and headless-verified; native visual/focus review remains. Headless acceptance:
`scripts/test_desktop.sh --filter 'WorkNavigationTests|DesktopPaneControlTests|TaskRowMetadataTests' -Xswiftc -gnone`.
Expect app/test build and all focused cases passing: drill-in and filtering,
attention selection, independent layouts, retained identity/drafts, history and
resize/collapse. Native mounted PTY/focus/geometry judgment remains demo/CI work.
Landing is not authorized by implementation.

## Selection references

Official screenshots inspected October 15: [Notion's workspace guide](https://www.notion.com/en-gb/help/intro-to-workspaces)
shows neutral page fill; [Figma's sidebar guide](https://help.figma.com/hc/en-us/articles/360039831974-Explore-the-navigation-bar-and-left-sidebar)
shows neutral page context and blue layer selection. Ink on Paper is Loopflow's
chosen adaptation, not a claim about either product. The top-rule experiment was
rejected; its history remains in commit `5359c2d77a`.

Check: `uv run --with playwright python /tmp/loo462-check-stationary.py` PASS — unchanged Wave click geometry, independent branches, queue mirroring, retained draft and search; screenshot inspected. Native baseline app build and 75 focused tests passed at 7d10108eaf; native acceptance remains separate.
