# LOO-1063 · Start-Time Stack + One-Shot Status Sort (B1)

## Direction & exact words

- Initial brief: “lets update the order of tasks in the left bar. things keep jumping to the bottom when i enter in them which is weird.  im not sure what i want the default order to be”
- Jack Heart (after Round 1 of [Explore](explore.html)): “ok i think  i like b.  i think we could do something liek what rex does with a button that sorts by terminal status, or alternatively we could sort by what step theyre at” / “i think we need something like moving auto to the bottom, or being able to 'close' certain tasks, but auto has to be smart enough to know like if its moving towards the end, that coutn even if its not explicitly auto”
- Jack Heart (after Round 2 of [Explore](explore.html)): “lets add b1” / “but also show the current step in the leftp ane, just not sort by it now”
- Jack Heart (on OSC 7501 `done` + New Task modal `?` font boxes): “done makes the task seem done but i dont tink thats what it means” / “isnt done then also available” / “yes, file it in, along with your other points” / “sorry, i meant to do both here”

Latest exploration page: [Explore Round 2](explore.html).
Latest concept review: [Built B1 concept review](concept-review.html).

## Remaining work (from B1 concept review at `1e3b4007ce`)

Slice 1 is built. Slices 2 and 3 remain; Slice 2 (Sort write recovery) is next.

### Slice 1 (built at `820bcdc384`) — Shared status for Task rows and Sort by status

Sort saves Blocked, Error, Available, Idle, Busy, newest first within each group.
A Task with both Busy and Available sorts as Busy and matches both filters.
Current OSC 7501 errors appear on phone rows, match `state:error`, and appear
in selection attention. Desktop drops cached Error on restore alongside other
live activity. Pending Task edits retain all states in declaration order.

On a private copy of the QA Machine with seeded Session reports, `lf desktop sort`
returned Bravo (Blocked), Charlie (Error + Busy), Delta (Available), Hotel and
Foxtrot (Idle), Alpha (Busy + Available). `lf desktop view show 'state:error'`
returned Charlie in both members and attention. Alpha appeared in both the Busy
and Available queries.

- `cargo test -p loopflow-storage --lib status_sort_groups_by_row_status_then_newest_start` passed.
- `cargo test -p loopflow --lib task_concepts_share_lifecycle_activity_membership_and_order` passed.
- `scripts/test_desktop.sh --filter 'TaskStatusTests|TaskConceptTests/missingKeysFailAndSavedActivityCannotReturn|WorkSelectionTests/everyStateRustSendsDecodes'` passed all four tests.
- `server::schema::tests::the_checked_in_schema_is_what_describe_prints` passed against the regenerated fixture.

### Slice 2 — Route **Sort by status** through `PendingChanges`

Rust unified drags and status sorts into one `TaskOrderEdit` write path, while
Swift still runs `WorkModel.sortWorkingTasksByStatus()` outside `PendingChanges`.
That causes three failure modes: a brief Machine blip fails Sort instead of
retrying under one `requestID`, the sidebar button stays enabled when the Machine
is away (`!pendingChanges.acceptsChanges`), and an in-flight `.reorder` preview
can replay over a newer sort result.

What follows across layers:
- **Swift shared (`swift/Loopflow/Services/PendingChanges.swift`):**
  - Add `PendingCommand.sortWorkingTasks(machine: String?, repository: String)`.
  - In `PendingChanges.workingOrder(_:machine:readAt:)`, find the last `.sortWorkingTasks` for `(machine, plan.repository)` in `patches(at: readAt)` and only replay `.reorder` commands that follow it (pre-sort drags are superseded; drags made after Sort still preview on top).
  - On refusal of `.sortWorkingTasks`, follow the same atomic refusal handling as `.reorder` (clear the pending entry on definite refusal and reread while keeping the refusal Notice via `NoticeCenter.reread(keeping:)`).
- **Swift Mac (`swift/LoopflowMac/WorkModel.swift`, `Views/WorkNavigator.swift`, `Views/SessionsView.swift`):**
  - Route `WorkModel.sortWorkingTasksByStatus()` through `pendingChanges.make(.sortWorkingTasks(machine: scope.machineId, repository: repo), subject: "Working Tasks", verb: "sorted", write: .desktopSort(directory: repoPath), notices: navigation.notices, show: nil)` calling `refreshWorkingOrder(in: scope)` on answer.
  - Disable the sidebar **Sort by status** button when `model.repoPath == nil || !model.pendingChanges.acceptsChanges` (and guard the ⌘K `.sortWorkingTasks` path when `!pendingChanges.acceptsChanges` with `PendingChanges.awayText` so it never fires while away).
- **Verification:**
  - `PendingChangesTests` covering a `.reorder` followed by `.sortWorkingTasks` (pre-sort drag no longer replays over the reading; post-sort drag still previews), blip retry under one `requestID`, and definite refusal keeping the Notice.
  - `WorkNavigationTests` / `DashboardWorkspaceTests` covering Sort disabled when `link == .away` and Sort going through `PendingChanges`.

### Slice 3 — Filter internal Graph step sentinels and show `graph_step` on iOS

`task_graphs.step` stores `task_work::START` (`"start"`) and `task_work::END`
(`"end"`). `TaskGraph::read` maps both to `current_step = None`, but
`fill_graph_steps_in` returns them verbatim so the sidebar prints literal `start`
and `end` as step names. Meanwhile, iOS decodes `WorkViewPlan.Content.graphStep`
and drops it in `TaskListView`.

What follows across layers:
- **Rust storage (`rust/loopflow-storage/src/sqlite/selections.rs`):**
  - In `fill_graph_steps_in`, omit `task_work::START` and `task_work::END` when parked (`run_start` is `None`) — and if `running_step` falls back to `parked`, omit `START`/`END` there too — so `graph_step` is `None` when not at an actual step.
- **Swift iOS (`swift/LoopflowiOS/TaskListView.swift`):**
  - Pass `context: task.graphStep` to `TaskRow(...)` in `TaskListView.link(_:)`.
- **Verification:**
  - Extend the Rust `fill_graph_steps_in` / selection test to assert `graph_step == None` for `task_graphs.step = "start"` and `"end"`, and `Some("review")` / running cursor step for real steps.
  - Assert `graphStep` renders in `TaskListView` / `TaskRow` on iOS (or unit test on the row inputs).

### Choices made for review (headless Graph)

Recorded in [concept-review.html](concept-review.html#questions):
1. **Dual-state `Busy` + `Available` Task in Sort by status:** Sort as `Busy` to match the word **Busy** printed on Mac and iOS rows (passed over changing row display to `Available`, because `working` beating `idle` is an established OSC 7501 & `TaskStatusTests` rule).
2. **OSC 7501 `error` in Sort by status:** Add `SelectionState::Error` and sort `Blocked → Error → Available → Idle → Busy` (passed over folding `Error` into `Blocked`, which would hide `Error` on iOS).
3. **Local preview for Sort by status:** No guessed local preview (passed over adding `started_at` to `ViewContent` solely for a transient preview); `PendingChanges` handles retry, offline disable, and superseding pre-sort drags cleanly.

## Working order & built behavior (already implemented at `1e3b4007ce`)

- Repository, Wave, roadmap and phone views share the Machine's Working order.
  A new Working Task precedes saved rows. Unsaved rows sort by first claim
  (`MIN(o.at)` excluding `unclaim`), falling back to first `started` event and
  then `created_at`, newest first with canonical Task ID breaking ties. Later
  handoffs keep original start time. Backlog keeps its planning rank. LOO-938:
  “your order of your claimed tasks is NOT the proejct order, ut your order of hte backlog IS”.
- A sidebar or grid drag saves the full Working order, carrying `DropSide`
  (`before`/`after`) through preview and both Machine write paths. The Machine
  applies it on each compare-and-commit attempt. Missing targets return
  `not_found` naming the absent Task, clearing the preview and keeping the
  refusal Notice through a failed reread. Saved Dashboards keep their own
  arrangements.
- **Sort by status** in the sidebar or ⌘K, and `lf desktop sort`, save a
  one-shot status snapshot (updated in Slice 1 to `Blocked, Error, Available,
  Idle, Busy`, newest first within each group). Subsequent turns leave the
  saved rows in place.
- `TaskSection` determines membership. Attached Sessions supply activity.
  A Task can match both Available and Busy in filters. Shortcuts follow the
  filtered list and the open conversation stays selected when its row leaves it.
- A live interactive Session's finished turn (`done` without `working`) reads
  Available on the Task row (terminal pane keeps `Finished`). Finished or
  failed reports clear on typing in both Ghostty and held terminals.
- All SwiftPM app builders (`scripts/app_bundle.py`) copy `*.bundle` resources
  for Dev, QA, and Release builds. `AppFontRegistration` scans packaged `.ttf`
  files by absolute URL with a same-size system fallback.
- Graph-step sorting (`B2`), automatic sinking (`B3`), manual parking (`B4`),
  LOO-937 Project-rank ownership, and `TaskCondition::Blocked` in selection
  reads stay outside LOO-1063 ([explore.html](explore.html),
  [questions.md](questions.md)).

## Earlier verification notes (preserved from prior passes)

- Synthetic store (10,000 Tasks, 406,666 events) median query times for 300 IDs:
  0.78 ms for start times, 0.33 ms for Graph steps; 1,553-Task Machine store:
  0.56 ms and 2.68 ms; 300 Machine Sessions availability lookup: 0.72 ms median.
- Private Machine QA rounds 1–3 recorded in [QA](qa.html). Round 3 verified a
  returning store, new arrival prepending, `lf desktop sort`, Graph step display,
  and packaged Dev bundle fonts. Unlocked Desktop walkthrough (live sidebar Sort
  button, ⌘K sort, grid drag, ⌘1–9, ⌃Tab, live OSC 7501 `done` pane, iOS
  Simulator feed) remains for gate.

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

In Loopflow Dev, select the lower of two Working rows while its conversation
is Available and send a prompt. Watch the status change and hand back in place.
Click **Sort by status**, open the first waiting row and send another prompt.
Watch its Graph step advance in place. Start a new Task and jump to it with ⌘1.
Switch to the Wave and compare relative order. Return to the repository, drag
the lower row up, and refresh. In an unlocked window, exercise a grid drop and
⌘1–9, ⌃Tab and the ⌘K sidebar filters.
