# LOO-623: a Wave's sidebar lists only its active Tasks

Status: implemented 2026-10-26 (design review skipped at Jack Heart's request). Product direction accepted by Jack
Heart on October 26 (LOO-576 conversation): "The most important filter, though,
is that it's only the active tasks" … "yeah fix that asap".

## Problem

After LOO-576 (#1669) the Desktop sidebar follows the selected scope. At the
repository it lists started Tasks; inside a Wave it lists every uncompleted
Task, unstarted ones included, and ⌘1–9 / ⌃⇥ count those rows. Jack wants the
sidebar to be the list of work in flight everywhere. Unstarted Tasks are started
from the Wave view in the middle (its redesign is LOO-594), which keeps them.

## Demo

Select a Wave holding one started and one unstarted Task. The sidebar shows one
row, the started Task, with `⌘1`; pressing ⌘1 opens it. The Wave board in the
middle still shows both tiles, the unstarted one offering Start. An empty
result reads "No working Tasks in this Wave".

## Risk findings

- **One rule already exists, in Rust.** `rust/loopflow/src/work/selection.rs`
  (built-in plan resolution) includes a Task in the repository plan when
  `open == Some(true)`, and in a Wave plan when `open.is_some()` and not Done,
  so the board can offer Start. Swift has no per-Task started fact of its own;
  it consumes plan membership.
- **The repository plan covers every Wave's started Tasks.** Its scope is the
  whole repository (`wave == None` → every Task in scope), so "Wave plan member
  ∩ repository plan member" is exactly "in this Wave (or a descendant) and
  started". Both plans arrive in the same `selectionPlans` value, so neither
  can be stale relative to the other; an unknown Task state (`open == None`) is
  excluded from both and already reported as unavailable.
- **Entering from the Wave board is unaffected.** `taskIDs(on: .wave)` uses
  `waveTiles`, so clicking an unstarted tile still enters it with the Wave as
  its board; only the sidebar list and its slot shortcuts narrow.

## Alternatives

1. **Swift intersection (chosen).** `sidebarMembers` for `.wave(id)` keeps the
   Wave plan's members that the repository plan also lists. Reuses the single
   Rust rule, plan order stays the Wave's, no wire change.
2. Change the Rust Wave plan to `open == Some(true)` and give the board another
   source of unstarted Tasks. Breaks the board (LOO-594 territory) and needs a
   second plan or DTO field: a second definition, rejected.
3. Add a `started` flag per member to the DTO. Duplicates what repository plan
   membership already states; DTO + fixture churn across Rust and Swift for no
   new fact. Rejected.

## Design

`swift/LoopflowMac/WorkModel.swift`:

```swift
case .wave(let id):
    // Working Tasks only, as at the repository; the Wave board keeps unstarted ones.
    let working = Set(workingTaskMembers.map(\.work))
    return (builtInPlan(.wave(id))?.resolution.members ?? [])
        .filter { $0.work.kind != .task || working.contains($0.work) }
```

Child Wave members are not Tasks and stay (`sidebarWaves` reads them). `waveTiles(_:)`,
`wavePlan`, `presentWaveTiles` and `taskIDs(on: .wave)` are unchanged.

`swift/LoopflowMac/Views/WorkNavigator.swift`: the Wave empty text becomes
"No working Tasks in this Wave", matching the repository's wording.

Dashboards are out of scope: a Dashboard is an authored selection whose
members are what its author asked for.

## Source of truth and consumers

- Source: Rust built-in selection plans (`work/selection.rs`); the started
  rule is the repository plan's `open == Some(true)`. Unchanged.
- Derived: `WorkModel.sidebarTaskIDs` → `WorkNavigator` rows and slot hints,
  `selectSidebarTask` (⌘1–9), `cycleSidebarTask` (⌃⇥), `WorkPalette`
  sidebar-only search, `DesktopActions` sidebar targets. All follow
  automatically.
- No DTO, CLI or Rust change.

## Forbidden outcomes

- A second "started" predicate in Swift (state strings, sessions, PR presence).
- Narrowing `waveTiles` or the Wave plan, which would remove Start from the board.

## Tests

- New `WorkNavigationTests` case: Wave with started `issue-now` and unstarted
  `issue-later`; sidebar lists `["issue-now"]`, `selectSidebarTask(slot: 0)`
  selects it, `waveTiles("wave-1")` still lists both.
- Update existing expectations that encoded the old behavior:
  `workingTasksFollowTheRepositoryPlan` (Wave sidebar → `["issue-now"]`) and
  `waveScopeSlotsCountTheWave` (Wave sidebar → `["issue-now"]`, slot 0 →
  `issue-now`). Fold the new proof into `waveScopeSlotsCountTheWave` if it
  reads cleaner than a separate case.

## Acceptance check at gate

`scripts/test_desktop.sh --filter 'WorkNavigationTests|WorkModelTests|TaskRowMetadataTests' -Xswiftc -gnone`
passes, including the new case; `WorkModelTests` and `TaskRowMetadataTests` read `sidebarTaskIDs` at repository scope and must stay green.

## Done when

Inside a Wave the sidebar, ⌘1–9 and ⌃⇥ show and count only started Tasks, in
the Wave's plan order, while the Wave board still offers Start on unstarted ones.

## Remaining

- Gate: `scripts/test_desktop.sh -Xswiftc -gnone` → 621/628; the 7 CLI-backed failures needed `target/debug/lf`, and after `cargo build` their rerun passed 15/15. `TaskRowMetadataTests.linksPreserveWorkspace` now supplies the repository plan too, since a Wave row must be started.
- Realign 2026-10-26: code matches the design; no upstream change since #1669.
- Check: `scripts/test_desktop.sh --filter 'WorkNavigationTests|WorkModelTests|TaskRowMetadataTests' -Xswiftc -gnone` → 84 passed (compress: no code change, not rerun).
- Concept review 2026-10-26: no change. "Working" now means one thing at every built-in scope, read from the one Rust rule. An unstarted Task entered from the board has no sidebar row until it starts. Jack's "only the active tasks" already settles that, so it's no open choice.
