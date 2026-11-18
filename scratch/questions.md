# LOO-968 open choices (kickoff, 2026-11-18, headless)

- Stop doesn't kill the run; it stops at the next interactive step or landing. Simpler than today's kill. Reverse if a stop must be immediate.
- `ship_settle_merge` goes: a hand-merged PR is met by the Graph's land step. Check the land step really returns success on an already-merged PR before deleting.
- Auto mode survives taking up another graph (columns stay on the row).
- Migration NULLs `tasks.shipped_at` so released lf stops relaunching `lf ship` during upgrade skew.
- Branch stacks on LOO-959 (#1972); rebase onto main if it merges first.

## Concept review (2026-11-18, headless)

- Named the fact Auto-Ship (existing product word), not "auto mode" from the quote. Reverse by renaming; nothing has shipped.
- One mark `auto_ship` on|stopped instead of on/off pairs. Reverse if someone needs who approved before a stop.
- Dropped the route approval: a no-interactive-ahead run merges but shows `moving`, not `shipping`/Autonomous. Reverse by restoring the write in the run's start path.

## Implement (2026-11-18, headless)

- Migration is `task_graph_auto_ship`, not `auto_ship`: unmerged migrations apply in name order after merged ones, and it must follow LOO-959's `process_rows_on_owners` (it reads `task_ship_events.caller_session_id`). Rename freely once #1972 merges.
- `lf task hold` on a Task with no Graph saves nothing and answers "was not shipping": the mark lives on `task_graphs`. Passed over: taking up the default graph just to hold it, which would make an unstarted Task idle.
- Reopening a Task clears its Auto-Ship mark (NULL), as reopening withdrew the approval before.
- The Runner's interactive decision moved into the executor: `GraphRunner::stopping_at_interactive` is gone, `RunningGraph::stands_in` reads Auto-Ship at each interactive step, and `SkillOutcome::Unapproved` became `StopsAt` (the landing stop and the interactive stop are one outcome).
- `TaskGraphRecord.auto_ship` carries `#[serde(default)]` for Moves from released senders; LOO-971 removes it with the `approval` key.
- Hand-merged PRs on graphs with no land step: the run still finishes its steps into `end` and completion settles; no merge check between steps was added.

## Compress (2026-11-18, headless)

- `a_restarted_wait_keeps_its_explicit_agent` and `a_shipping_tasks_restarted_wait_keeps_its_explicit_agent` time out "waiting for the restart" here, with and without the compress edits (HEAD 77c670ae85). Gate should check whether CI sees it.
