# LOO-1063 · Stable Working Task order

The Task brief reads “lets update the order of tasks in the left bar. things keep jumping to the bottom when i enter in them which is weird.  im not sure what i want the default order to be”

## Done when

Open the second of two Working Tasks, send a prompt, and let the conversation
hand back. The row stays second in the unfiltered sidebar. Its status changes,
and the Available and Busy filters change membership in the order of the full
list. ⌘1–9 and ⌃Tab follow the visible rows. The Wave view shows the same relative
order.

Drag the second row above the first from the repository sidebar or Working Tasks
grid. The move shows at once and a successful drag survives refresh. If the
Machine refuses a drag and the next read fails, the last confirmed order is
visible with a Notice. A fresh read can then show current membership and order.

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
- **A drop** is one edit to the repository's Working order, by Task ID and
  destination Task ID. The Machine resolves current Working membership and
  writes `task_orders` atomically against the saved sequence it read.
  The drag preview is temporary and uses the last reading underneath it. A
  refusal clears that preview and reports the error. A committed write is
  reflected by a sufficiently recent Machine reading.

## Shared order

The shared sort key is saved drag position, planning rank and canonical ID.
Repository and Wave plans, the phone list and the roadmap all use it. Activity
still drives status, attention and filters. Authored queries and saved Dashboard
arrangements retain their own composition rules.

The regression follows the rank-2 Task through Idle → Working → Idle before the
first drag, then reads the saved order after dragging it above rank 1. Desktop
replays the activity frames in one selected workspace.

## Next slice · Show and recover a drop

Keep immediate feedback in the repository sidebar and Working Tasks grid.
Replace `moveRepositoryMember`'s rewrite of `selectionPlans` and
`taskListReadings` with a pending move applied at display time to their
repository members. Use the existing pending-write and read-revision lifecycle.
Apply the preview to the latest confirmed readings using the side of the target
chosen at the drop. Applying the same preview to a frame that already shows the
move should keep it on that side. When either member is absent, show the
current membership. Keep raw readings available for failed reads.

A definite refusal removes that move and posts a Notice even when the reread
fails. Use the existing server receipt or read revision to retire an accepted
move when its order arrives. If a newer reading arrived during the write, the
refusal reveals that reading. It never restores an old copied frame. Scope the
preview to the repository and Machine of the drop. Keep `desktop move` and the
current `task_orders` write as the saved order's owner.

In `WorkNavigationTests`, hold a drop to check immediate feedback, then reject it
and fail the follow-up read. Check the last confirmed order in both displays,
the Notice, and a later successful reading. Include a confirmed frame arriving
before the write answer to catch a double-applied move. The sidebar docs
already describe this behavior.

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
the lower row up, and refresh.

The earlier drag feedback and shared-order repairs were LOO-673 (#1791) and
LOO-696 (#1947). LOO-937 changes Working drags into Project rank edits
(`c79764d627` on #1973). That is a separate decision about who owns order.
Carry this Task's activity-independence regression through that overlap.
