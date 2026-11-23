# LOO-1063 · Start-Time Stack + One-Shot Status Sort (B1)

## Direction & exact words

- Initial brief: “lets update the order of tasks in the left bar. things keep jumping to the bottom when i enter in them which is weird.  im not sure what i want the default order to be”
- Jack Heart (after Round 1 of [Explore](explore.html)): “ok i think  i like b.  i think we could do something liek what rex does with a button that sorts by terminal status, or alternatively we could sort by what step theyre at” / “i think we need something like moving auto to the bottom, or being able to 'close' certain tasks, but auto has to be smart enough to know like if its moving towards the end, that coutn even if its not explicitly auto”
- Jack Heart (after Round 2 of [Explore](explore.html)): “lets add b1” / “but also show the current step in the leftp ane, just not sort by it now”
- Jack Heart (on OSC 7501 `done` + New Task modal `?` font boxes): “done makes the task seem done but i dont tink thats what it means” / “isnt done then also available” / “yes, file it in, along with your other points” / “sorry, i meant to do both here”

Latest exploration page: [Explore Round 2](explore.html).
Latest concept review: [Built B1 concept review](concept-review.html).

## Remaining work (from B1 concept review at `1e3b4007ce`)

All three slices are built. The unlocked Desktop walkthrough remains for gate.

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

### Slice 2 (built at `ca02700fb8`) — Sort recovery on Desktop

The sidebar and ⌘K submit Sort through `PendingChanges`. Each Working list
sends its drags and sorts in click order across reconnects. Sort supersedes
earlier drag previews in that list, and later drags preview immediately.
Both controls disable while the Machine is away. A refusal clears the pending
sort and its explanation survives a failed reread.

- `scripts/test_desktop.sh --filter 'statusSortSupersedes|refusedStatusSort|sortRetriesAfter|sortIsDisabled|sortByStatusUpdates|refusedWorkingTaskDrop|workingTaskDropAcrossReceipt'` passed all seven tests.

### Slice 3 (built at `ecf3ac66eb`) — Current Graph step on Mac and phone

Selections use the running cursor when it names a step, then the parked step.
The parked start and end positions have no step name. The phone draws the same
step in its Task row as the sidebar.

- `cargo test -p loopflow-execution --lib sidebar_step_follows_the_captured_cursor`, `cargo fmt --check`, and `cargo clippy --all-targets -- -D warnings` passed.
- `uv run python scripts/test_ios.py -only-testing:LoopflowiOSTests/TaskFeedTests/testTaskRowShowsItsCurrentGraphStep` passed on the iOS 26.2 Simulator.
- On a fresh private Machine, `lf desktop view show 'kind:task state:working' --json` reported `null`, `"review"`, and `null` for FIX-2 parked at start, review, and end.

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
