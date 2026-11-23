# LOO-1063 · Start-Time Stack + One-Shot Status Sort (B1)

## Direction & exact words

- Initial brief: “lets update the order of tasks in the left bar. things keep jumping to the bottom when i enter in them which is weird.  im not sure what i want the default order to be”
- Jack Heart (after Round 1 of [Explore](explore.html)): “ok i think  i like b.  i think we could do something liek what rex does with a button that sorts by terminal status, or alternatively we could sort by what step theyre at” / “i think we need something like moving auto to the bottom, or being able to 'close' certain tasks, but auto has to be smart enough to know like if its moving towards the end, that coutn even if its not explicitly auto”
- Jack Heart (after Round 2 of [Explore](explore.html)): “lets add b1” / “but also show the current step in the leftp ane, just not sort by it now”
- Jack Heart (on OSC 7501 `done` + New Task modal `?` font boxes): “done makes the task seem done but i dont tink thats what it means” / “isnt done then also available” / “yes, file it in, along with your other points” / “sorry, i meant to do both here”

Latest exploration page: [Explore Round 2](explore.html).

## Implemented in this checkout

The Machine now returns the same start-time order to every Working Task view.
Both the sidebar button and `lf desktop sort` save a status snapshot through
the same write as a drag. The sidebar reads the step from the Task's captured
Graph cursor, including while a run advances between interactive steps.

On a private Machine, a drag saved First, Second. Starting Third produced
Third, First, Second. Running `lf desktop sort` on the three idle Tasks and
reopening the view returned Third, Second, First.

For 300 IDs on a synthetic store with 10,000 Tasks and 406,666 events, median
query times were 0.78 ms for start times and 0.33 ms for Graph steps. On the
1,553-Task Machine store they were 0.56 ms and 2.68 ms. SQLite used the Task
and Graph primary keys and the ownership and event indexes by Task ID.

The five focused Rust tests passed. They covered a new arrival after a drag,
changing conversation status after a sort, the parked and running Graph step,
the required nullable wire field, and a real CLI and server editing one order.

The installed Dev and QA app bundles lacked `LoopflowSwift_Loopflow.bundle`.
The release app contained it, including the named Old Standard TT and Atkinson
faces. All SwiftPM app builders now copy their resources with the executable.
A private Dev bundle contained all nine font files byte-for-byte. Both the Dev
and release install tests passed.

The status query now recognizes a live interactive Session's finished turn,
including when read through the one-shot sort. On 300 Machine Sessions with
program reports the availability lookup took 0.72 ms median across 15 reads.
SQLite looked up each Session and activity row by primary key.

The focused Task-order and held-terminal tests passed with OSC 7501 `done`.
After a real keystroke, the held Session's stored done/error records were
empty. Thirteen headless Desktop tests passed across eight suites, including
sidebar and grid order, the Graph step label, status precedence and the actual
glyphs for the New Task title and workflow steps.

`cargo fmt --check`, `cargo clippy --all-targets -- -D warnings`, and Ruff
passed. The unlocked Desktop walkthrough and iOS Simulator feed check remain
for gate.

## B1 scope

Keep everything PR #2025 already built (no activity-driven jumping on turns, side-aware `DropSide` drags, `PendingChanges` immediate preview and refusal recovery) and add **B1 (Start-Time Stack + Rex-style one-shot status sort) + current step display**:

1. **Start-Time Stack default order (replaces `planning_rank` fallback):**
   - Decouple Working Task order from backlog `planning_rank` (matching LOO-938: “your order of your claimed tasks is NOT the proejct order, ut your order of hte backlog IS”).
   - In `work_in` (`rust/loopflow-storage/src/sqlite/selections.rs`), compute each Task's `started_at` in one batch query (claim/start time from `task_ownership.at` / `task_events` `'started'`, falling back to `tasks.created_at`) — no schema migration.
   - Unsaved Working Tasks sort **newest-started first** (`started_at` descending, canonical Task ID tiebreaker).
   - **New arrivals prepend at the top:** Working Tasks not present in `task_orders` appear **before** saved rows (sorted newest-started first), so starting a new Task places it at `⌘1` instead of burying it at the bottom after earlier drags.
   - Conversation activity (`Available`, `Busy`, `Blocked`, `Idle`) still never moves a row on its own.

2. **One-shot “Sort by status” button (Rex-style):**
   - Add a Machine-side one-shot sort mutation that reads the repository's current Working Tasks and atomically commits `task_orders` sorted by terminal/session status: **`Blocked` → `Available` → `Idle` → `Busy`**, tie-broken by newest-started first, then canonical Task ID.
   - Expose it through the shared server protocol / CLI (e.g., `lf desktop sort` or alongside `lf desktop move`) and a compact **Sort by status** button in Desktop's sidebar header (`WorkNavigator`, plus `⌘K` command).
   - Because it writes `task_orders` once, rows stay completely frozen after the sort across subsequent prompts and hand-backs until the next drag or sort click. New Tasks started afterwards still prepend at the top.

3. **Show current Graph step in the left pane (display only — do not sort by it):**
   - Carry each Task's current Graph step (e.g. `plan`, `build`, `ship-review`, `deliver`, `verify`, or `None` when the Task has no Graph) on the sidebar reading / row data (batch-read from `task_graphs` / live `graph_processes` with no schema migration; remember DTO rules: explicit required/nullable key and round-trip fixtures in Rust & Swift).
   - Display the current step concisely in the left bar's `TaskRow` metadata line alongside the identifier and status word when a Task has a Graph step. Do not sort by step in this pass.

4. **Fix OSC 7501 `done` handling across `available_sql`, Task rows, and held terminals:**
   - Live interactive Antigravity and Claude Code sessions natively emit OSC 7501 `state=done` at turn end (unread finished turn, waiting for input), and `lf` emits `state=done` on exit code `0` (`transport::report_exit`, `transport/hosted.rs`).
   - Update `available_sql` (`rust/loopflow-storage/src/sqlite/sessions.rs`) so a live interactive Session (`agent_ended_at IS NULL AND interactive=1`) reporting `state='done'` — when no record on that Session is `'working'` — counts as `Available`, matching `TURN_LIVE` (`IN ('idle','blocked','done')`) and feeding the `Available` filter and one-shot status sort accurately.
   - Update `ProgramStatusState` presentation (`swift/Loopflow/Models/ProgramStatus.swift`, `swift/LoopflowMac/SessionProgramStatus.swift`): don't display green `"Done"` on a Working Task row (which collides with `TaskState::Done`) and don't let a finished subprocess/exited Session's `done` outrank another live Session's `idle` (`Available`) in `summaryOrder`.
   - Clear sticky `done`/`error` records on user interaction in server-side held terminals (`rust/loopflow-terminal/src/ghostty_vt.rs`, `transport/hosted.rs`, `transport/registry.rs`), matching Desktop's `GhosttyTerminalView` `.interaction` reducer so `done` doesn't stay stuck in `session_activity.program_status`.

5. **Fix missing-glyph `?` boxes in the New Task modal (`NewTaskSheet` & `NewTaskWorkflow`):**
   - In the Desktop New Task sheet (`⌘N`, `swift/LoopflowMac/Views/NewTaskSheet.swift`), `.serif(.display)` on `"What do you want?"` and the Graph step labels on the workflow timeline below `"In loopflow ⌄, running code ⌄ · the default"` (`NewTaskWorkflow`) render as missing-glyph `?` boxes. Audit bundled font registration and `.serif` / workflow font lookup in Desktop so all glyphs render cleanly with working fallback.

6. **Open threads kept outside this pass:**
   - Sorting by Graph step (`B2`).
   - Smart “heading to the end” sink for `Auto` / `deliver` / settling PRs (`B3`).
   - Manual “Close / Park” for individual Working Tasks (`B4`).

## Done when

Start a new Working Task while several Working Tasks already exist (including after a manual drag): it lands at the top (`⌘1`), not at the bottom. Open a Working Task, send a prompt, and let the conversation hand back: the row stays in place throughout.

Drag any row above or below another from the repository sidebar or Working Tasks grid: the move shows at once, keeps the chosen side across concurrent reorders, survives refresh, and recovers cleanly with a Notice on refusal.

Click the sidebar's **Sort by status** button (or run the CLI sort command): `Blocked` rows move above `Available`, then `Idle`, then `Busy` (newest-started first within each bucket), persisted to `task_orders`. Opening the top waiting row and sending a prompt leaves it at `⌘1` instead of dropping it back down. Wave views, `lf desktop ls`, and the phone feed show the same relative order.

## Model

- **Working membership** comes from `TaskSection`. Starting or claiming can add a
  row. Unassigning, completing, or abandoning can remove one. A conversation can
  change activity while the Task remains Working.
- **Order** is a Start-Time Stack: unsaved Working Tasks (including brand-new
  arrivals) come first in descending `started_at` order (tie-broken by canonical
  Task ID), followed by Tasks in their saved `task_orders` sequence. A manual
  drag or one-shot **Sort by status** writes the full current Working Task
  sequence to `task_orders`, freezing it until the next drag, sort, or new
  arrival. Backlog order continues to use `planning_rank`.
- **Activity** comes from attached Sessions. It feeds status, attention and
  filters. An interactive Session handing back can make a Task Available.
  Working reports can make it Busy. Multiple reports or Sessions can put a Task
  in both filters. An unknown activity reading can make an activity query
  incomplete while leaving order resolvable.
- **Selection** names the open Task by ID. Filters and shortcuts use the visible
  sequence. The selected workspace and its conversation survive a status change
  while the row can leave a filtered list.
- **A drop** names a Task, a target Task and a placement before or after it.
  The drag surface determines the side from the order it showed. The same
  placement drives the preview and the write. The Machine resolves both IDs
  against its current Working reading and writes `task_orders` atomically
  against the saved sequence it read. A retry uses the same side against the
  newer sequence. If either Task is absent from that reading, the Machine
  refuses the drop and reports the absent Task's ID. A destination-only command
  and a grid drop whose source is absent from the receiving grid use the
  target's slot. The grid waits for a confirmed read in that case.

## Shared order

The shared sort key is Start-Time Stack: unsaved Working Tasks (including newly
started arrivals) sort first by descending `started_at` (derived in one batch
query from `task_ownership.at` / `task_events` `'started'`, falling back to
`tasks.created_at`) with canonical Task ID breaking ties, followed by Tasks in
their saved `task_orders` sequence. Repository and Wave plans, the phone list,
and the roadmap all use it. Activity drives status, attention, filters, and the
one-shot **Sort by status** write (`Blocked → Available → Idle → Busy`, newest
started first within each bucket), never automatic row movement. Authored
queries and saved Dashboard arrangements retain their own composition rules.

## Concurrent drops

The sidebar and Working Tasks grid carry the side shown at drop through the
preview and both Machine write paths. The Machine applies it to the current
order on each compare-and-commit attempt. A destination-only move still takes
the target's slot, including a grid drop from a Task absent in its reading.

The served regression starts with three Working Tasks on a private Machine.
Another window moves A to C's slot, leaving B, C, A. Dropping B before A then
saves C, B, A. Dropping A after C saves C, A, B. The test reads the Working
list after each write and repeats each relative move through the CLI.

## Concept review

The first built review found that the preview put B before A while the write
could put B after A. The second build carries the side through `DropSide` and
applies it on the Machine. The served regression and the stale drag in the
private Desktop both exercised a Machine order newer than the drag. A missing
target now returns `not_found`, so `PendingChanges` can remove the preview and
show the refusal while the affected lists reread. Neither a conversation turn
nor a saved Dashboard edit writes the repository's Working order.

Superseded initial-order choice: the earlier review kept rank and canonical Task
ID as the initial order ([Built concept review](concept-review.html)); B1 now
replaces that default with Start-Time Stack + the one-shot **Sort by status**
button ([Explore Round 2](explore.html)).

## Docs

- `docs/surfaces.md`
- `docs/work.md`
- `swift/README.md`

## Earlier drag checks

- `cargo test -p loopflow --lib task_concepts_share_lifecycle_activity_membership_and_order`:
  the old sort returned FIX-2, FIX-1 on the first Idle report. The new sort
  passed the turn and drag checks with the committed fixture.
- Branch `lf desktop view show` on a private Machine: repository and Product
  Wave returned FIX-1, FIX-2 for all three reports. Available and Busy queries
  switched FIX-2 at each transition.
- `scripts/test_desktop.sh --filter TaskConceptTests` passed all seven tests,
  including the selected conversation, both sidebar filters and keyboard slots.
- `cargo fmt` and `cargo clippy --all-targets -- -D warnings` passed.
- The live refusal test first showed “Tasks couldn't be read: Read unavailable”
  where the drop Notice belonged. After the fix, both lists kept the newer
  confirmed order and the Notice showed “b couldn't be reordered: Order changed”.
- `scripts/test_desktop.sh --filter 'TaskConceptTests|WorkNavigationTests'`
  passed 58 tests. The shared pending-change tests and the one-shot Working Tasks
  writer test passed 17 more.
- With a concurrent planning revision on a refused drop, the sidebar and grid
  both kept `c, b, a` after the reread failed. The focused Desktop run now
  shows the confirmed `c, a, b` for refusals with and without that revision.
  All 28 selected tests passed.
- The private Machine walkthrough recorded in [QA](qa.html) observed the
  sidebar through a turn, drag, arrival, relaunch, failed drop and recovery.
  The grid displayed the saved order. The live grid drag, keyboard shortcuts
  and key-window filter still need an unlocked walkthrough.
- `cargo test -p loopflow --test served_reads working_task_drop_keeps_its_side_after_another_window_reorders`
  reproduced C, A, B before the fix and passed both sides after it. Moving B
  to C's slot on a saved Dashboard then produced B, C, A on that Dashboard
  while the Working list stayed C, A, B.
- `cargo test -p loopflow-execution --lib dashboard_order_moves_under_its_revision_and_only_reordering_touches`
  passed. Desktop passed 24 focused tests covering drag feedback, write
  receipts, refusals, conversation turns and Dashboard navigation.
- `scripts/test_desktop.sh --filter 'workingTaskDropAcrossReceipt|refusedWorkingTaskDrop|sidebarAndGridShareTheOrderTheRustWriterReturns|draggedRowLandsOnRelease|droppingTileCOnAMovesItThroughTheSharedOrderCommand'`
  passed all five tests, including the grid drop with an absent source and the
  destination-only saved Dashboard move.
- QA round 2 ([QA](qa.html) §6): a stale sidebar drag in the hooked app sent
  `D before A` and the Machine saved C, B, D, A over the newer order. A
  refused drop now names the absent Task and returns `not_found` over the
  socket instead of `outcome_unknown`. The `served_reads` regression passed
  again at `e71370f26c` during concept review.

## Gate

Run Cargo with the Task's `LF_*` variables cleared and stdin closed.

```sh
cargo test -p loopflow --lib task_concepts_share_lifecycle_activity_membership_and_order
cargo test -p loopflow-execution --lib one_task_order_reorders_working_tasks_and_every_plan_and_arrivals_prepend
scripts/test_desktop.sh --filter 'TaskConceptTests|WorkNavigationTests'
uv run python scripts/test_ios.py -only-testing:LoopflowiOSTests/TaskFeedTests
```

In Loopflow Dev, select the lower of two Working rows while its conversation
is Available and send a prompt. Watch the status change and hand back in place.
Click **Sort by status**, open the first waiting row and send another prompt.
Watch its Graph step advance in place. Start a new Task and jump to it with ⌘1.
Switch to the Wave and compare relative order. Return to the repository, drag
the lower row up, and refresh. In an unlocked window, exercise a grid drop and
⌘1–9, ⌃Tab and the ⌘K sidebar filters.

The earlier drag feedback and shared-order repairs were LOO-673 (#1791) and
LOO-696 (#1947). LOO-937 changes Working drags into Project rank edits
(`c79764d627` on #1973). That is a separate decision about who owns order.
Carry this Task's activity-independence regression through that overlap.
