# LOO-576: the sidebar lists the selected Wave or Dashboard's Tasks

Status: implemented October 24, 2026 (Jack waived design review). Direction is
Jack Heart's (LOO-519 concept review, October 24); implementation choices below are
the plan's. Remaining: gate acceptance run; animation judged at demo.

## Problem

Desktop's sidebar and main area disagree about what you are looking at. Opening a
saved Dashboard leaves the sidebar on Working Tasks, because `sidebarTaskIDs`
(`swift/LoopflowMac/WorkModel.swift:1665`) reads only `sidebarWaveID` and the
filter, never `navigation.destination`. ⌘1–9 and ⌃⇥ (`selectWorkingTask(slot:)`,
`cycleWorkingTask`, `WorkModel.swift:1692`) always walk `workingTaskIDs`. The row
slot hints (`WorkNavigator.swift:62`) also come from `workingTaskIDs`, so inside a
Wave a ⌘3 hint can point at a Task you can't see. A tile click zooms with a crumb
back to its Dashboard (`zoomTile` → `.zoomed(from:work:)`). A sidebar click on the
same Task goes to `.work(task)` and drops the crumb.

Jack's direction: clicking a Wave or Dashboard animates it into the header, and
the sidebar shows that scope's Tasks. In a Dashboard, the sidebar lists the
Dashboard's Tasks.

Beneficiaries: Jack navigating Desktop, and agents whose 1-based position targets
(LOO-519) count the list on screen.

## Demo

1. Open Loopflow Desktop on this repository and click the pinned Dashboard
   "Review". Its row label slides up into the sidebar header, which reads
   `‹ loopflow` / `REVIEW  3`. The rows below are the Dashboard's three Tasks in its
   arrangement order, with hints ⌘1–⌘3.
2. Press ⌘2. The second tile's Task opens full, and the titlebar reads
   `loopflow › Review / <Task>`. Press ⌃⇥ to go to the third member, then once more
   to wrap to the first. Each stays inside Review.
3. Click a sidebar row. You get the same result as clicking that row's tile: the
   crumb still passes through Review.
4. Click `‹ loopflow`. The header animates back to Working Tasks, and ⌘1 selects the
   first Working Task again.
5. Click a Wave. Its name moves into the header, and ⌘1 now selects that Wave's
   first Task, not the first Working Task.

## Chosen approach

### One scope, read from existing navigation state

`WorkNavigation` (per window, `WorkProjection.swift:197`) already holds both facts:

- `sidebarWaveID`: the stored Wave scope. It is stored because selecting a Task
  inside a Wave is an ordinary `.work(task)` destination.
- `destination.trailBoard`: the Dashboard you are showing or zoomed from.

Add one computed value and no new stored state:

```swift
enum SidebarScope: Equatable { case repository, wave(String), dashboard(String) }
var sidebarScope: SidebarScope {
    if case .saved(let id) = trailBoard { return .dashboard(id) }
    return sidebarWaveID.map(SidebarScope.wave) ?? .repository
}
```

A saved Dashboard is the scope while you are on it or zoomed from it. Leaving the
board leaves the scope, so the two can't disagree. Working Tasks is the repository
scope's own board. Its sidebar list is already the same list, so selecting it
highlights its row and doesn't drill in (assumption, `scratch/questions.md`).

### The list on screen is the only list

`WorkModel.sidebarTaskIDs` derives from `sidebarScope`, then applies the existing
filter:

| Scope | Task rows | Wave rows (`sidebarWaves`) |
|---|---|---|
| repository | `workingTaskIDs` (repository plan) | every visible Wave |
| wave(id) | the Wave's built-in plan Tasks (unchanged) | its child Waves (unchanged) |
| dashboard(id) | `dashboard(id).projection.visible` Task members, in arrangement order, hidden occurrences excluded, first occurrence wins | that projection's Wave members |

Every positional consumer reads this one list:

- `selectSidebarTask(slot:)` and `cycleSidebarTask(_:)` replace
  `selectWorkingTask(slot:)` and `cycleWorkingTask` (menu at
  `SessionsView.swift:65–69`, shortcut monitor at `SessionsView.swift:1137,1168`).
  The "Working Tasks" menu becomes "Sidebar Tasks".
- Slot hints come from `sidebarTaskIDs.prefix(9)`.
- The sidebar-only palette (`searchDestinations(sidebarOnly:)`) uses
  `sidebarTaskIDs` + `sidebarWaves`, not every visible roadmap.
- LOO-519's `lf desktop ls` and position targets read `sidebarTaskIDs` through the
  Desktop control channel when that Task builds them. This Task adds no CLI surface.

### Selecting a member keeps its board

`select(_:preservingOpening:recordingHistory:)` gets one rule. When the requested
reference is a Task and the current `trailBoard` contains it, the destination
becomes `.zoomed(from: board, work:)` rather than `.work`. Board membership is
`workingTaskMembers` for Working Tasks and `projection.visible` for a saved
Dashboard. Sidebar rows, ⌘N, ⌃⇥, the palette and the jump bar all go through
`select`, so a sidebar click behaves like the tile. Only the tile supplies a zoom
origin; the other paths simply appear. Selecting a Task that isn't a member is
ordinary `.work(task)` navigation, and the scope falls back to
`sidebarWaveID`/repository.

`selectDashboard(_:)` clears `sidebarWaveID`. A Dashboard is a repository-level
scope, and its back control returns to the repository.

**Open question 2 is resolved: `zoomed` stays.** It is the destination that carries
the board crumb. It now covers every in-board Task selection, not only tile clicks.

### Header animation

The header region in `WorkNavigator` (back control + `listTitle`) already exists
for Wave scope. Extend it to Dashboard scope: the title shows the Dashboard name and
the empty state reads "No Tasks in this Dashboard". `DashboardSidebar` is already
shown only at repository scope.

- An `@Namespace` in `WorkNavigator` is passed to `DashboardSidebar`. The Wave or
  Dashboard row label and the header title share
  `matchedGeometryEffect(id: scopeKey)`, with the row as source while unscoped and
  the header as source while scoped.
- `.animation(.snappy(duration: 0.25), value: model.navigation.sidebarScope)` goes
  on the sidebar content only. Main content and terminals must not join the
  transaction, so the change uses no `withAnimation` around `selectDashboard`.
  Under Reduce Motion, the change happens with no animation.
- On a scope change, `WorkTaskList` scrolls to the top so the header is on screen
  even when you clicked a Wave far down the list.

### Reaching out of a scope (open question 1)

Nothing new is added. These controls already exist and stay as they are:

- the header's back control: to the repository
- the titlebar jump bar `›` after the repository crumb ("Waves and Dashboards")
- ⌘K Find Work: any Task, Wave or Dashboard
- the existing "Open Task is outside this list" notice for a filtered or Wave scope

## Risk findings

- **Earlier slot decision.** The October 17 LOO-489 memory says "Cmd-number uses
  stable slots; Ctrl-Tab cycles them" over the Working Tasks order. Jack's
  October 24 direction replaces the list those slots count. Order inside each scope
  stays stable: plan order and the Dashboard's arrangement order. Under the
  Available or Running filter, though, slots shift as Session attention changes,
  which the October 17 rule was protecting against. The plan's choice is to count
  the on-screen (filtered) list, because LOO-519 targets count what is on screen.
  This tension is open in `scratch/questions.md`.
- **Animation cost.** Earlier sidebar work moved hover and scroll off observation
  for per-frame cost (`sidebarScrollOffset`, `RowHover`). The matched-geometry
  pair animates two labels and a list swap inside the sidebar only. Existing
  `DesktopPerformanceTests` sidebar readiness (`DesktopPerformanceTests.swift:444`)
  must stay green.
- **Unavailable Dashboard.** `dashboard(id)` can be nil (deleted, failed read). The
  scope stays `.dashboard(id)` while the destination names it. The header shows
  "Dashboard unavailable" with the back control, the sidebar's existing error
  banner carries the `selectionPlans.errorMessage` reason, and the empty list reads
  "This Dashboard is no longer available." It never falls back to Working Tasks,
  because a fallback would bring back the disagreement this Task removes.
- **Non-Task members.** Dashboard members can be Waves. They go in the Waves
  section, are not slot-numbered, and are excluded from ⌘N/⌃⇥, which select Tasks.

## Scope

Affected surfaces: `WorkProjection.swift` (`SidebarScope`), `WorkModel.swift`
(`sidebarTaskIDs`, `sidebarWaves`, select rule, slot and cycle, `selectDashboard`),
`Views/WorkNavigator.swift` (header, hints, namespace, scroll-to-top),
`Views/DashboardView.swift` (`DashboardSidebar` namespace), `Views/SessionsView.swift`
(menu and shortcut wiring), `Views/WorkPalette.swift` (sidebar-only search). There
is no Rust, DTO or store change.

Exclusions:

- `lf desktop ls`, position targets and `state` readback (LOO-519)
- reordering Dashboard members by dragging sidebar rows (drag stays
  repository-scope only)
- iOS
- Waiting navigation (LOO-402)

## Delete — do not maintain

- `WorkModel.selectWorkingTask(slot:)` and `cycleWorkingTask(_:)`, replaced by the
  sidebar-list versions. No compatibility aliases.
- The slot-hint dictionary built from `workingTaskIDs` in `WorkNavigator`.

Must survive: `workingTaskIDs` and `workingTaskMembers` (repository scope, Working
Tasks grid, palette ranking, drag order), `zoomTile` (tile origin), and the
outside-list notice.

Forbidden: a second stored scope field beside `sidebarWaveID` + `destination`, and
any path that lists Working Tasks while a saved Dashboard is the destination.

## Done when

- On a saved Dashboard (or zoomed from one), the sidebar's Task rows equal the
  Dashboard's visible Task members in arrangement order, and the header names the
  Dashboard with a back control.
- ⌘N and ⌃⇥ select from that list in every scope, and the hints match it.
- Selecting a member from the sidebar, palette or shortcut yields
  `.zoomed(from: .saved(id), work:)`, the same destination as its tile.
- Back returns to the repository scope with Working Tasks.
- Scope changes animate the row into the header in the sidebar only.

Acceptance check at gate (headless):
`swift test --package-path swift --filter 'WorkDestinationTests|DashboardWorkspaceTests|WorkNavigationTests|DesktopPerformanceTests'`
passes, including the new tests:

- `dashboardScopeListsMembersAndKeepsTheBoard` (in `WorkNavigationTests`): a fixture whose Dashboard order
  differs from Working Tasks. Covers `selectDashboard`, `sidebarTaskIDs` order,
  `selectSidebarTask(slot: 1)`, the `cycleSidebarTask` wrap, the `.zoomed`
  destination, and `select(nil)` restoring `workingTaskIDs`.
- `waveScopeSlotsCountTheWave`
- `dashboardSidebarHeader`: `WorkNavigator` renders the Dashboard title,
  `work-sidebar-back`, and ⌘1 on the first member row

The animation itself is judged at demo/review.

Implementation notes: the in-board rule lives in public `WorkModel.select(_:)`, so
Back/Forward replay exact destinations. Selecting a Working Tasks member also zooms
(updated `workingTasksDashboardKeepsSidebarTaskOnTheBoard`). One `sidebarMembers` switch feeds both `sidebarTaskIDs` and `sidebarWaves`;
in-board selection reuses `zoom(into:from:)`, and re-selecting the zoomed member
records no visit.

Check result: `swift test --filter WorkNavigationTests` passed (41 tests, realign); full acceptance filter incl. `DesktopPerformanceTests` deferred to gate.
