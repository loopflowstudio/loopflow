# LOO-1063 · Stable Working Task order

The Task brief reads “lets update the order of tasks in the left bar. things keep jumping to the bottom when i enter in them which is weird.  im not sure what i want the default order to be”

## Done when

Open the second of two Working Tasks, send a prompt, and let the conversation
hand back. The row stays second in the unfiltered sidebar. Its status changes,
and the Available and Busy filters change membership in the order of the full
list. ⌘1–9 and ⌃Tab follow the visible rows. The Wave view shows the same relative
order.

Drag the second row above the first from the repository sidebar or Working Tasks
grid. The move shows at once and a successful drag survives refresh. If another
window changed the order since the drag surface read it, the Machine still
places the row on the chosen side of the target. If the Machine refuses a drag
and the next read fails, the last confirmed order is visible with a Notice. A
fresh read can then show current membership and order.

## Model

- **Working membership** comes from `TaskSection`. Starting or claiming can add a
  row. Unassigning, completing, or abandoning can remove one. A conversation can
  change activity while the Task remains Working.
- **Order** comes from saved drag positions in this repository on the Machine,
  then ascending Task rank and canonical Task ID for Tasks without a saved
  position. Rank and ID also give the initial order before a drag. After a drag,
  arrivals without a saved position appear after saved rows. An explicit
  planning rank edit can move rows in the unsaved portion. The choice for review
  is to use the existing rank fallback and give a committed drag precedence over
  subsequent planning rank edits. The planning board's Backlog continues to use
  planning rank. LOO-938 records Jack Heart's earlier words, “your order of your
  claimed tasks is NOT the proejct order, ut your order of hte backlog IS”
  (`git show dd2c3b49f9:scratch/plan.md`).
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
  newer sequence. If either Task is absent, the Machine refuses the drop.

## Shared order

The shared sort key is saved drag position, planning rank and canonical ID.
Repository and Wave plans, the phone list and the roadmap all use it. Activity
still drives status, attention and filters. Authored queries and saved Dashboard
arrangements retain their own composition rules.

The regression follows the rank-2 Task through Idle → Working → Idle before the
first drag, then reads the saved order after dragging it above rank 1. Desktop
replays the activity frames in one selected workspace.

## Concurrent drops

The sidebar and Working Tasks grid carry the side shown at drop through the
preview and both Machine write paths. The Machine applies it to the current
order on each compare-and-commit attempt. A destination-only move still takes
the target's slot, including a grid drop from a Task absent in its reading.

The served regression starts with three Working Tasks on a private Machine.
Another window moves A to C's slot, leaving B, C, A. Dropping B before A then
saves C, B, A. Dropping A after C saves C, A, B. The test reads the Working
list after each write and repeats each relative move through the CLI.

The remaining walkthrough needs an unlocked Desktop for a grid drag, keyboard
navigation and the key-window sidebar filters. [Pre-implementation review](concept-review.html)

## Docs

- `docs/surfaces.md`
- `docs/work.md`
- `swift/README.md`

## Checks

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

## Gate

Run Cargo with the Task's `LF_*` variables cleared and stdin closed.

```sh
cargo test -p loopflow --lib task_concepts_share_lifecycle_activity_membership_and_order
cargo test -p loopflow-execution --lib one_task_order_reorders_working_tasks_and_every_plan_and_arrivals_append
scripts/test_desktop.sh --filter 'TaskConceptTests|WorkNavigationTests'
uv run python scripts/test_ios.py -only-testing:LoopflowiOSTests/TaskFeedTests
```

In Loopflow Dev, select the lower of two Working rows while its conversation
is Available and send a prompt. Watch the status change and hand back in place.
Switch to the Wave and compare relative order. Return to the repository, drag
the lower row up, and refresh. In an unlocked window, exercise a grid drop and
⌘1–9, ⌃Tab and the ⌘K sidebar filters.

The earlier drag feedback and shared-order repairs were LOO-673 (#1791) and
LOO-696 (#1947). LOO-937 changes Working drags into Project rank edits
(`c79764d627` on #1973). That is a separate decision about who owns order.
Carry this Task's activity-independence regression through that overlap.
