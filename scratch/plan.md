# LOO-1063 · Stable Working Task order

The Task brief reads “lets update the order of tasks in the left bar. things keep jumping to the bottom when i enter in them which is weird.  im not sure what i want the default order to be”

## Done when

Open a Working Task, type in its conversation, and let the conversation return to
Available. The row keeps its slot as its status changes. ⌘1–9 and ⌃Tab keep following
the displayed rows. Available and Busy show the matching rows in their shared order. A
deliberate drag moves the row and the new order survives refresh.

## Design

Use the repository’s saved drag positions first, followed by ascending Task rank and
canonical Task ID for Tasks without a saved position. Also use rank and ID when the
repository has never been reordered. A new Working Task joins the unsaved tail when
there is a saved order, and a planning rank edit can move a Task in that tail. A
completed or unassigned Task leaves Working according to `TaskSection`. The default is a
choice for review. It uses the existing planning fallback and gives drag sole control of
the saved Working order. LOO-938 records Jack Heart’s earlier direction, “your order of
your claimed tasks is NOT the proejct order, ut your order of hte backlog IS” (`git show
dd2c3b49f9:scratch/plan.md`).

`TaskOrder::key` in `rust/loopflow-storage/src/sqlite/selections.rs` is the shared rule
for selection results and roadmap snapshots. Delete its `!waiting` field and the
`states` parameter at both callers. A probe compiled from the current key flipped `[A,
B]` to `[B, A]` when higher-rank A switched from Available to Busy, then put A first
when it became Blocked. Explicit drag positions held through every phase.
`sqlite/sessions.rs::available_sql` derives availability from live reports and current
attachment activity, so a conversation can change the old key without moving any
planning record. `WorkModel.taskRows` consumes the server’s member sequence, and
`setSelection` writes the selected workspace to the Desktop cache.

The same order reaches repository and Wave selections and boards through
`desktop.view.get`, the iPhone feed through the all-repository view read, and roadmap
consumers through `waves.rs::snapshot_tasks`, including `lf plan list`, `lf wave show`,
and Desktop’s Wave picker. A Dashboard in automatic mode follows its query’s result
order. Touched Dashboard arrangements use their saved sequence. Activity still provides
status, attention and filter membership. Empty results are empty, failed reads show the
last good result where available, and fresh results replace cached ones. The sort uses
fields already in each bulk read.

The previous repairs were LOO-673 (#1791) for immediate drag feedback and LOO-696
(#1947) for one order across ordinary views. The repair here removes the activity term
from that shared order. A Swift-only freeze would leave other consumers with a moving
order. Last-visited ordering would move the row during the reported interaction.
LOO-937’s open PR #1973 (`c79764d627`) also removes activity from the order, but
replaces saved drag positions with Project rank edits. The review choice here is to land
the source-level repair independently of that redesign, with its two-task regression to
carry across the overlap.

## Build and gate

1. Extend `task_concepts_share_lifecycle_activity_membership_and_order` in `waves.rs`.
   Before any drag, use two Working Tasks with ranks `1` and `2`. Report Idle and then
   Working from the rank-2 Task’s live Session. Assert `[1, 2]` in repository and Wave
   view-plan members, the mobile all-repository projection, and the roadmap. Check that
   the Task leaves the Available query and enters Busy. This phase should fail on the
   current key while rank 2 is Available. Remove the activity term, then move rank 2
   above rank 1 and verify `[2, 1]` on another refresh. Use the existing saved-order
   test for arrivals and rejected drops.
2. Carry the paired observations into `TaskConceptTests`. Apply the Available frame,
   select rank 2, then apply the Busy frame. The sidebar and Wave picker keep the same
   IDs in order, the selected Task remains in place, and the Available and Busy filters
   show the appropriate subset. Update the sidebar docs with the default and the drag
   behavior.

Headless gate commands run with the Task’s `LF_*` variables cleared and stdin closed for
Cargo:

```sh
cargo test -p loopflow --lib task_concepts_share_lifecycle_activity_membership_and_order
cargo test -p loopflow-execution --lib one_task_order_reorders_working_tasks_and_every_plan_and_arrivals_append
scripts/test_desktop.sh --filter 'TaskConceptTests|WorkNavigationTests'
uv run python scripts/test_ios.py -only-testing:LoopflowiOSTests/TaskFeedTests
```

The phone suite exercises the real feed’s consumption of ordered view plans. The Rust
scenario exercises the order and activity change in the plans it receives.

**Demo.** In Loopflow Dev, open a repository with two Working Tasks in default order.
Pick the lower row while its conversation is Available and send a prompt. Watch the
status change while both rows keep their slots. Switch to the Wave to see the same
relative order, return to the repository, drag one row, and refresh to see the new order
stick.
