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

## QA (2026-11-18, headless)

- A released `lf ship` that dies across the upgrade is not carried again: its carrier lived on `task_ship_events`, and `task_graphs.run_pid` is NULL, so `died_run` sees nothing. The Task reads `idle`, "Auto-Ship: on", "process left no exit record … launch fresh work"; `lf task run` carries it on and stands in. Left as the plan's "honest answer". Reverse by having `task_graph_auto_ship` copy a live ship carrier's pid onto `task_graphs.run_pid` so the branch's pass carries it as `lf task run`.
- A Graph process killed with its carrier stays open until the hourly prune, so the Task reads `shipping` (was `moving` on the base) with nothing running. Base behavior; `lf machine prune` settles it.
- Five recovery fixtures time out here on HEAD 3f2a4f7e89 without the QA edits too (run alone, `--test-threads=1`): `a_restarted_wait_with_no_explicit_agent_ignores_the_readers`, `a_restarted_wait_keeps_its_explicit_agent`, `a_shipping_tasks_restarted_wait_keeps_its_explicit_agent`, `a_task_run_takes_its_waves_agent_without_flags` ("timed out waiting for the Task conversation and Graph launches"), `a_task_run_whose_carrier_died_is_carried_again_and_gives_up_after_five`. The live world's recovery carried a killed run within a minute, so this looks like the fixture's MachineServer, not the pass. Gate should check CI.
- `a_second_ship_refuses_while_a_run_carries_the_task` is gone: a ship on a live run now reports that run (`a_ship_on_a_running_task_reports_its_run_and_starts_nothing`).

## Concept review after QA (2026-11-18, headless)

- Nothing to change before gate; page `scratch/concept-review.html`. Open: the stop's five names (`task hold`, Stop Auto-Ship, "stopped shipping", `ship_stop`, `ship --stop` alias), `-b` writing a lasting mark, `shipping`/Autonomous wording. Gate can switch the fixtures from the `ship --stop` alias to `task hold`.

## Human review (2026-11-19)

Jack, on the concept review's "one idea, three words" (shipping state, Autonomous section, Auto-Ship mark): "I tink this should be in scope"
