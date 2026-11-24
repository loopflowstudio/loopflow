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

## Built and aligned

- **Mac sidebar status sort:** The repository and Wave sidebars share a window-local toggle in the header and ⌘K. The selected Task keeps its status group across activity updates and repeated visits to the open row. Rows within each group follow shared order. Changing the filter releases the group. Saved Dashboards use arrangement order. Sidebar drags save order only in the repository with sorting off.
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

- `docs/surfaces.md`
- `docs/work.md`
- `swift/README.md`

## Verification

`scripts/test_desktop.sh --filter 'WorkNavigationTests'` passed all 56 tests, including the Wave toggle and open-Task group across navigation and list updates. The display was locked for this pass. Live hover and keyboard input remain for QA.

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
