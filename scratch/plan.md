# LOO-1063 · Start-Time Stack and sidebar status toggle

## Direction & exact words

- Initial brief: “lets update the order of tasks in the left bar. things keep jumping to the bottom when i enter in them which is weird.  im not sure what i want the default order to be”
- Jack Heart (after Round 1 of [Explore](explore.html)): “ok i think  i like b.  i think we could do something liek what rex does with a button that sorts by terminal status, or alternatively we could sort by what step theyre at” / “i think we need something like moving auto to the bottom, or being able to 'close' certain tasks, but auto has to be smart enough to know like if its moving towards the end, that coutn even if its not explicitly auto”
- Jack Heart (after Round 2 of [Explore](explore.html)): “lets add b1” / “but also show the current step in the leftp ane, just not sort by it now”
- Jack Heart (on OSC 7501 `done` + New Task modal `?` font boxes): “done makes the task seem done but i dont tink thats what it means” / “isnt done then also available” / “yes, file it in, along with your other points” / “sorry, i meant to do both here”
- Jack Heart (during live `/demo`, [demo-notes.md](demo-notes.md)): “whqt is that arrows button” / “well, it needs a on/off state, and maybe an on-hover” / “1a” (toggle view: On shows status-sorted sidebar without overwriting saved drags; Off restores Start-Time Stack / drag order) / “build it and return when ready”

Latest exploration page: [Explore Round 2](explore.html).
Latest concept review: [Built B1 concept review](concept-review.html).
Latest demo notes: [Demo notes](demo-notes.md).

## Sidebar toggle from the demo

Implemented the window-local status toggle in the repository sidebar and ⌘K.
Turning it on groups the current reading by Blocked, Error, Available, Idle
and Busy. The open Task keeps its visible slot. Inside each group the other
rows keep their saved order. Turning it off returns to that order and enables
sidebar dragging. The grid and phone continue to use the shared Machine order.

The one-shot CLI command, served write, storage sort and pending-sort preview
are gone. The remaining Machine write places a dragged Task relative to its
target. The toggle can be used while disconnected.

Focused checks on 2026-11-23:
- Desktop passed `statusToggleRestoresSavedOrderWithoutChangingTheGrid`,
  `statusToggleKeepsTheOpenTaskInItsSlotUntilNavigation` and
  `workingDragsPreviewImmediatelyAndRetryInOrder` with WindowServer denied.
  The button tap, accessible On/Off value and both tooltips ran headless.
- Rust passed the shared Task concepts, concurrent Task order commit and
  new-arrival/drag tests. The regenerated server schema removes `desktop.sort`.
- `cargo fmt --check` and `cargo clippy --all-targets -- -D warnings` passed.

After simplification, four focused Rust tests passed for arrivals, stale writes,
served drops and roadmap order. Eight headless Desktop tests passed for
filtering, status grouping, navigation and drag recovery. `cargo fmt --check`
passed.

Round five exercised the mounted Desktop toggle, live status changes,
saved drags, offline sorting, the Wave and Working Tasks grid, and the iPhone
SE Simulator feed against a private Machine. The active icon now uses the
theme's contrast-adjusted accent. Older servers can supply Task rows without
`graph_step`, and current servers still include it. See [QA round five](qa.html).

At gate, check the hover highlight, physical keyboard shortcuts and system
grid drag in an unlocked Desktop window. A returning locked-screen launch
still has the workspace restore problem recorded in [questions.md](questions.md).

## Earlier slices (one-shot sort since replaced by the toggle)

- Slice 1 (`820bcdc384`): shared row status. A Task with both Busy and
  Available groups as Busy and matches both filters. Current OSC 7501 errors
  appear on phone rows, match `state:error` and appear in selection attention.
  Desktop drops cached Error on restore. Pending Task edits keep every state
  in declaration order. On a private QA Machine copy, `lf desktop view show
  'state:error'` returned the Error Task in members and attention.
- Slice 2 (`ca02700fb8`): Working drags go through `PendingChanges` in click
  order across reconnects; a refused drop clears its preview and its Notice
  survives a failed reread. The pending-sort half was deleted with the toggle.
- Slice 3 (`ecf3ac66eb`): current Graph step on Mac and phone rows. Running
  cursor's step first, then the parked step; parked start/end show none.
  `sidebar_step_follows_the_captured_cursor` and iOS
  `TaskFeedTests/testTaskRowShowsItsCurrentGraphStep` passed. On a fresh
  private Machine, `lf desktop view show 'kind:task state:working' --json`
  reported `null`, `"review"`, `null` for a Task parked at start, review, end.
- Choices recorded in [concept-review.html](concept-review.html#questions):
  Busy beats Available for a dual-state Task (matches the printed **Busy**);
  `SelectionState::Error` exists so iOS can show Error instead of folding it
  into Blocked.

## Working order (implemented at `1e3b4007ce`)

- Repository, Wave, roadmap and phone views share the Machine's Working order.
  A new Working Task precedes saved rows. Unsaved rows sort by first claim
  (`MIN(o.at)` excluding `unclaim`), falling back to first `started` event and
  then `created_at`, newest first with canonical Task ID breaking ties. Later
  handoffs keep original start time. Backlog keeps its planning rank. LOO-938:
  “your order of your claimed tasks is NOT the proejct order, ut your order of hte backlog IS”.
- A sidebar or grid drag saves the full Working order, carrying `DropSide`
  (`before`/`after`) through preview and both Machine write paths. Missing
  targets return `not_found` naming the absent Task. Saved Dashboards keep
  their own arrangements.
- `TaskSection` determines membership; attached Sessions supply activity.
  Shortcuts follow the filtered list and the open conversation stays selected
  when its row leaves it.
- A live interactive Session's finished turn (`done` without `working`) reads
  Available on the Task row; the terminal pane keeps `Finished`. Finished or
  failed reports clear on typing in Ghostty and held terminals.
- `scripts/app_bundle.py` copies `*.bundle` resources for Dev, QA and Release.
  `AppFontRegistration` scans packaged `.ttf` files with a same-size fallback.
- Out of scope: Graph-step sorting (`B2`), automatic sinking (`B3`), manual
  parking (`B4`), LOO-937 Project rank ([explore.html](explore.html),
  [questions.md](questions.md)).

## Measurements

- Synthetic store (10,000 Tasks, 406,666 events), 300 IDs: 0.78 ms start
  times, 0.33 ms Graph steps; 1,553-Task Machine store: 0.56 ms and 2.68 ms;
  300-Session availability lookup 0.72 ms median.
- `EXPLAIN QUERY PLAN` on a 10,011-Task store with 1,000 Graph rows showed
  indexed lookups for Tasks, primary Sessions, checkouts, Graphs and the local
  Machine.
- On an in-memory copy with 10,012 Tasks and 103 saved orders, the saved-order lookup used the repository and Task order primary-key indexes.
- Private QA rounds 1–4: [qa.html](qa.html).

## Docs

- `docs/surfaces.md`
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

After the sync onto main, `served_reads` `server_operations_match_their_lf_commands`
failed at `config scope --json` (`/private/var/folders` vs `/var/folders`), a
test and read that came from main ([questions.md](questions.md)). Gate checks
whether main fails the same way.

In Loopflow Dev, select the lower of two Working rows while its conversation
is Available and send a prompt. Watch the status change and hand back in place.
Turn on **Sort by status**, open the first waiting row and send another prompt.
Check its slot while it turns Busy, then select another row to release it.
Use ⌘K to turn sorting off. Start a new Task and jump to it with ⌘1.
Switch to the Wave and compare relative order. Return to the repository, drag
the lower row up, and refresh. In an unlocked window, exercise a grid drop and
⌘1–9, ⌃Tab and the ⌘K sidebar filters.
