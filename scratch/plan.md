# LOO-1063 · Start-Time Stack and sidebar status toggle

## Direction & exact words

- Initial brief: “lets update the order of tasks in the left bar. things keep jumping to the bottom when i enter in them which is weird.  im not sure what i want the default order to be”
- Jack Heart (after Round 1 of [Explore](explore.html)): “ok i think  i like b.  i think we could do something liek what rex does with a button that sorts by terminal status, or alternatively we could sort by what step theyre at” / “i think we need something like moving auto to the bottom, or being able to 'close' certain tasks, but auto has to be smart enough to know like if its moving towards the end, that coutn even if its not explicitly auto”
- Jack Heart (after Round 2 of [Explore](explore.html)): “lets add b1” / “but also show the current step in the leftp ane, just not sort by it now”
- Jack Heart (on OSC 7501 `done` + New Task modal `?` font boxes): “done makes the task seem done but i dont tink thats what it means” / “isnt done then also available” / “yes, file it in, along with your other points” / “sorry, i meant to do both here”
- Jack Heart (during live `/demo`, [demo-notes.md](demo-notes.md)): “whqt is that arrows button” / “well, it needs a on/off state, and maybe an on-hover” / “1a” (toggle view: On shows status-sorted sidebar without overwriting saved drags; Off restores Start-Time Stack / drag order) / “build it and return when ready”

Latest exploration page: [Explore Round 2](explore.html).
Latest concept review: [Concept review at 14f44821e8](concept-review.html).
Latest demo notes: [Demo notes](demo-notes.md).

## Next pass (from concept review)

Two mismatches remain in the Mac sidebar’s **Sort by status** toggle (`swift/LoopflowMac/`):

1. **Replace `selectedSidebarSlot: Int?` with a latched status bucket (`sidebarStatusLatch`).**
   - *Why:* Pinning an integer array index breaks status order in three normal interactions:
     - Leaving a latched Busy Task at slot 0 for the Available Task at slot 1 captures slot 1 before the old row releases, splicing the newly selected Available row into slot 1 and pushing an **Idle** row into slot 0 above it (`WorkNavigationTests.swift:301` currently asserts `["idle", "other", "open"]`).
     - When a Blocked Task above an open Available Task completes or turns Busy, the pinned index pulls a **Busy** row into slot 0 above the unchanged Available row.
     - Turning **Sort by status** On while a Busy Task is open at slot 0 glues it to slot 0 above Blocked and Available rows.
   - *Model to build:*
     - In `swift/LoopflowMac/WorkProjection.swift`, replace `var selectedSidebarSlot: Int?` with `var sidebarStatusLatch: SidebarStatusLatch?` (`struct SidebarStatusLatch: Equatable { let taskID: String; let statusOrder: Int }`). Clear it in `sidebarFilter.didSet` so switching filters (e.g. to Busy) starts fresh in true status order.
     - In `swift/LoopflowMac/WorkModel.swift`:
       - When `isWorkingSidebarScope && navigation.sidebarSortByStatus`, `sidebarRows` stably sorts `filteredSidebarRows` using the latched `statusOrder` for the currently selected Task (when its resolved ID matches `sidebarStatusLatch.taskID`) and `row.statusOrder` for every other row. Saved drag order still breaks ties within each group.
       - `toggleSidebarSortByStatus()`: turning On latches the currently selected Task’s current `statusOrder` from `filteredSidebarRows` (so all rows sort cleanly immediately, and subsequent turns in the open Task hold its bucket); turning Off clears `sidebarStatusLatch = nil`.
       - `setSelection(_:)`: while `sidebarSortByStatus` is On, if the new selection is a Task whose resolved ID differs from the current `sidebarStatusLatch?.taskID`, latch that Task’s `statusOrder` from `filteredSidebarRows` (if present, else `nil`); if it is the same Task already latched (e.g. re-selecting via ⌘1 while `.entered`), preserve the existing latch; if not a Task, set `sidebarStatusLatch = nil`.
       - `selectDestination(_:)` for non-Task destinations (`.dashboard`, `.view`, `.workingTasks`): clear `sidebarStatusLatch = nil`.
       - `applyTaskList(_:wave:reason:readAt:)`: update `taskListReadings[key]` first; then if `isWorkingSidebarScope && navigation.sidebarSortByStatus`, and `selection` is a Task whose resolved ID does not yet match `sidebarStatusLatch?.taskID`, latch its initial `statusOrder` from the newly updated `filteredSidebarRows`.
2. **Apply Sort by status to every Working sidebar scope (`.repository` and `.wave`), not only `.repository`.**
   - *Why:* Restricting the toggle to `scope == .repository` is a leftover from the deleted one-shot `task_orders` write. Both Repository and Wave sidebars show Working Tasks from `taskList`, and the toggle is now a local view sort that writes nothing. Clicking a Wave should keep the toggle visible and active instead of hiding it and silently unsorting while `sidebarSortByStatus` stays `true`.
   - *What follows across files:*
     - Add one predicate on `WorkNavigation.SidebarScope` (e.g. `var isWorkingList: Bool { if case .dashboard = self { false } else { true } }`) and use it in `WorkModel.sidebarRows`, `WorkNavigator` (showing `SidebarStatusToggle`), and `WorkPalette` (showing the `Sort by status` action in ⌘K).
     - Saved Dashboards (`.dashboard`) continue to keep their own arrangement order and hide the toggle.
     - Sidebar drag-to-reorder stays `scope == .repository && !model.navigation.sidebarSortByStatus` (dragging mutates the repository’s shared `task_orders`).
     - Update `docs/surfaces.md` so the sidebar toggle paragraph reflects repository and Wave sidebars and status-group latching.
   - *Tests to update / add in `swift/LoopflowTests/WorkNavigationTests.swift`:*
     - Update `statusToggleKeepsTheOpenTaskInItsSlotUntilNavigation`: leaving a latched Busy Task for an Available Task puts Available at slot 0 above Idle and Busy (`["other", "idle", "open"]`, never `["idle", "other", "open"]`); turning Sort On while Busy is at slot 0 sorts Blocked/Available to the top and latches Busy.
     - Add coverage for: (a) a Blocked Task completing or turning Busy above an open Available Task never pulls Busy above Available; (b) re-selecting the already-open Task (including `.entered` via ⌘1) keeps its latch; (c) switching to a Wave scope keeps the toggle visible, keeps sorting its Working Tasks, and keeps drag disabled; switching to a saved Dashboard hides the toggle and preserves Dashboard order.

Choices made for review (recorded in [concept-review.html](concept-review.html#questions)): latch status bucket instead of integer slot (`1a`), apply toggle to Repository and Wave Working sidebars (`2a`), sort cleanly on toggle-on before latching (`3a`).

## Built and aligned

- **Shared Working order (Start-Time Stack):** Repository, Wave, roadmap, CLI (`lf task list`, `lf wave show`), Working Tasks grid, and iOS share `ordered_tasks_in` + `TaskOrder` (`New(Reverse(started_at)) < Saved(pos) < Planning(rank)`). `start_times_in` reads first non-unclaim ownership, then first `started` event, then `created_at` (preserving original start time across handoffs). Backlog keeps its planning rank.
- **Side-aware drags & recovery:** Sidebar (repository scope, sort Off) and Working Tasks grid drags route through `WorkModel.reorderWorkingTask` → `PendingChanges` (`orderWrites` serialized per repository across reconnects) → `desktop.dashboard.move` with `DropSide` (`before`/`after`) → `SqliteStore::reorder_working_task` CAS loop. Absent targets refuse as `not_found` naming the Task; `NoticeCenter.reread(keeping:)` protects refusal Notices across failed rereads.
- **One-shot sort deleted:** `desktop.view.sort`, `TaskOrderEdit::SortByStatus`, `status_bucket`, and pending-sort preview are removed; `server_schema.json` regenerated.
- **Status vocabulary & OSC 7501 `done`/`error`:** `SelectionState::Error` projects across queries, `attention`, Mac, and iOS; `Busy` beats `Available` on dual-state rows while both filters include them; a live interactive Session’s finished turn (`done` without `working`) reads `Available` on Task rows while the terminal pane keeps `Finished`; typing clears `done`/`error` in Ghostty and held terminals.
- **Current Graph step:** `fill_task_contents_in` batches live `run_start` cursor or parked step (stripping `START`/`END`) across all plans into `ViewContent.graph_step` (optional on decode for older Machines); shown on Mac and iOS `TaskRow`.
- **Packaged New Task fonts:** `scripts/app_bundle.py` copies `*.bundle` resources for Dev, QA, and Release; `AppFontRegistration` scans packaged `.ttf` files with same-size system fallback.
- **Out of scope:** Graph-step sorting (`B2`), smart Auto sinking (`B3`), manual Close/Park (`B4`), LOO-937 Project rank, `TaskCondition::Blocked` in selection queries ([explore.html](explore.html), [questions.md](questions.md)).

## Measurements

- Synthetic store (10,000 Tasks, 406,666 events), 300 IDs: 0.78 ms start times, 0.33 ms Graph steps; 1,553-Task Machine store: 0.56 ms and 2.68 ms; 300-Session availability lookup 0.72 ms median.
- `EXPLAIN QUERY PLAN` confirmed indexed lookups on 10,011-Task / 1,000-Graph / 103-saved-order stores.
- Private QA rounds 1–5: [qa.html](qa.html).

## Docs

- `docs/surfaces.md` (update sidebar toggle scope & latching description in the next pass)
- `docs/work.md`
- `swift/README.md`

## Gate

Run Cargo with the Task's `LF_*` variables cleared and stdin closed.

```sh
cargo test -p loopflow --lib task_concepts_share_lifecycle_activity_membership_and_order
cargo test -p loopflow-execution --lib one_task_order_reorders_working_tasks_and_every_plan_and_arrivals_prepend
cargo test -p loopflow --test served_reads working_task_drop_keeps_its_side_after_another_window_reorders
scripts/test_desktop.sh --filter 'TaskConceptTests|TaskStatusTests|PendingChangesTests|WorkNavigationTests|DashboardWorkspaceTests'
uv run python scripts/test_ios.py -only-testing:LoopflowiOSTests/TaskFeedTests
```

After the sync onto main, `served_reads` `server_operations_match_their_lf_commands` failed at `config scope --json` (`/private/var/folders` vs `/var/folders`), a test and read that came from main ([questions.md](questions.md)). Gate checks whether main fails the same way.

In Loopflow Dev (unlocked window), verify: hover highlight on `SidebarStatusToggle`, sending a prompt in a Working Task (sort Off and On), switching away from a latched Busy Task to an Available Task (confirming strict status order), toggling Sort by status in a Wave sidebar, grid drag, `⌘1–9`, `⌃Tab`, and `⌘K` sidebar filters.
