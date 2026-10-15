# Work multiplexer and Task workspaces

Accepted October 15, 2026 — Jack Heart accepted [Study 06](sidebar-prototypes/06-stationary-waves.html): “yeah this looks good,” then requested “lets build this and ship it.” LOO-462 / Product, PR #1531. Scope is frozen: one final implementation pass, then only polish, bug fixing, performance, testing and validation. Dashboards (LOO-489), theming (LOO-488) and terminal shader/contrast work (LOO-487) retain their separate ownership.

## What to build

A stationary Wave hierarchy with full-size Available Tasks above it, and retained Work workspaces with independent inner pane multiplexers.

Jack: “there needs to be one selection state”; “two distinct levels of hierarchy”; “forward/back nav was keyboard only”; “sidebar should also be draggable to resize”. Ink on Paper remains accepted. The rejected header-relocation design is preserved in git at `6a2622bc71:scratch/task-workspaces.md`, not maintained as another navigation mode.

## The demo

The repository stays in the header. Click a Wave: its existing row selects, its conversation opens, and Tasks appear directly beneath it. Other expanded branches stay open. Click a Task: only that destination selects. Fold through the Task-count button without navigating or changing selection. Search filters both sections, hides nonmatching Waves, and never changes selection.

Available contains Tasks only, with the same row size/details as ordinary Tasks plus Wave context. Each Task appears once: Available Tasks are excluded from Wave branches and their counts. Available opens the exact conversation. A retained Session in another Work's pane layout still highlights one sidebar destination, not both its workspace owner and Task. Availability changes use current shared evidence; Wave conversations never move into Available.

Command-click opens Work beside the focused workspace. Ordinary click replaces the focused slot, or focuses Work already visible. Close hides a workspace while retaining panes, surfaces and file drafts. Each inner split stays within its Work. Narrow outer layouts reflow vertically. Context headers appear only with multiple workspaces or a collapsed sidebar. Primary single-workspace views have no middle breadcrumb. Secondary Sessions retain rename/bind actions.

Drag the sidebar within 260–600pt while reserving content space; collapse/restore retains width. Command-[ / ] traverse history with no visible history buttons. Type a draft, open another Task beside it, split one workspace, switch focus, resize/collapse, hide/reopen and return through history: identity and drafts survive.

Task lifecycle feedback distinguishes command execution from confirmation. A successful command says “Task updated; refreshing…”. Confirmation expires after ten seconds independent of heartbeats; failure says the update was accepted but refresh is unconfirmed. Explicit Refresh readings never repeats the mutation. This repairs the indefinite busy state documented in the completion investigation, without claiming its historical latency cause is known.

## Data structures / Key functions

- `WorkWorkspaceKey` identifies whole Work; `WorkLayout` is the outer binary split tree. `SessionsWorkspaceRegistry.layout` owns placement; `openWork` is derived.
- Each `SessionsWorkspace` retains its `MultiplexerStore`, pane focus, presentation, documents and surfaces. `SplitDivider` shares geometry only.
- `WorkProjection.navigator(...)` derives one partition into Available Tasks and Wave branches from existing readings. It stores no inventory or availability state.
- `WorkNavigation.expandedWaves` and `listScrollOffset` own presentation, independently of Work selection. `WorkModel.navigatorSelection` projects that selection through an exact retained available Session without storing another selected value.
- `select(reference, besideFocused:)`, `hideWorkspace(key)` and `WorkLayout.replacing/removing/resizing` own outer navigation. `split(pane, axis:)` remains inside its workspace.
- `TaskRunDraft.phase` distinguishes running command/awaiting observation; `acting` is derived. Existing checkpoint waiters own deadlines and settle once, including late replies and reader loss.

## Current system / Delete — do not maintain

Remove the native drill-in branch, Wave breadcrumb header, bounded compact attention queue, overflow menu, `WorkTaskList`, and `taskListScrollOffsets`, with their exclusive drill-in tests. Replace them in the same diff with the single scrollable partition and independent Wave folding. Preserve metadata links, errors, selected terminal Tasks, exact Session opening, retained identities, history and sidebar resizing.

Earlier cuts removed adjacent Wave/Task columns, remembered Wave destinations, `WorkBreadcrumbBar` and flat stored `openWork`. Do not restore them. Available replaces shared Session Waiting through Swift, Rust, JSON and `lf session list --available`; unrelated Task/Flow waiting remains. No new schema migration.

Replace stored `TaskRunDraft.acting` and unbounded checkpoint continuations, preserving authoritative confirmation and originating-Task ownership. Diagnostic history remains an explicit Debug subscription; do not restore its cost to normal Task frames.

## Constraints / Forbidden outcomes

No ancestor selection, duplicate Task rows, duplicate surface mounts, shared inner panes across Work, draft loss on hide, navigation launching a Flow, or search changing selection. No optimistic completion, automatic command retry, or broad automatic refresh after writes. Explicit Refresh remains permitted.

Inactive workspaces keep live panes and offer focus for Workflow actions; the existing reader watches the focused Task. Width and outer placement persist within the window only. Native compositor/input timing remains distinct from headless identity and layout tests. Scope is closed; unrelated repository renaming and hook investigation are not inferred additions.

## Internal slices / Done when

Indivisible presentation change, one PR. **This slice:** finish native stationary navigation, deduplication and lifecycle confirmation repair; then gate and land. Remaining work is verification, defects found by it, and delivery only.

Headless gate: `scripts/test_desktop.sh -Xswiftc -gnone` builds the app and passes the desktop suite, including stationary navigation, availability partition, exact selection, retained nested layouts, cache, history and confirmation expiry. Rust availability/Work-stream checks and clippy retain their existing evidence unless integrated source changes invalidate it; full platform matrix belongs to CI. Native visual judgment does not replace these tests.

Check: first native focused pass, `scripts/test_desktop.sh --filter 'WorkNavigationTests|WorkModelTests|WorkCacheTests' -Xswiftc -gnone`, PASS (88); final regression/gate pending.
