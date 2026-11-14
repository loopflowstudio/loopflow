# LOO-890: a failed Task run starts again at the step that failed

## Problem

A Task run that fails mid-segment restarts from the segment's start. LOO-858
(Graph `code`, 2026-11-13): after approval at demo, `gate` ran 6h+ and pushed
`cf57cb01ec`; `land --wait-and-fix` failed on "ssh: connect to host github.com
port 22: Undefined error: 0"; the automatic retry (run `1b2e0799`) began at
`gate` again and redid its merge and test work. Direction given: "yes, make an
auto task".

## Demo

```
lf task run INF-123 held        # held = `task sync --plan`, then `graph show late` (fails)
  The run of held from start failed (attempt 1 of 3); starting it again at <late step>.
lf task status INF-123          # Graph held: stopped at <late step>
# define .lf/graphs/late.yaml
lf task run INF-123             # runs only `graph show late`; `task sync` never reruns
```

## Why it happens (current architecture)

- `task_graphs.run_start` (`store/sqlite/task_work.rs`, `start_run_in`) holds
  the cursor the run set out from. Written once by `lf task run`; cleared by
  `arrive_in` at an interactive step or `end`.
- Each attempt is a child `lf --task X graph run` (`run_task_graph`,
  `lf/commands/graph.rs`) that reads `run_start`. `run_task_graph_for` retries
  up to `TASK_RUN_ATTEMPTS` (3); every attempt reads the same `run_start`.
- The runner already keeps the exact cursor: `GraphRunner::run_with_cursor`
  calls `SkillExecutor::checkpoint` after each tick (`graph/runner.rs`), but
  `RunningGraph::checkpoint` only stores it in memory (`position`).
- A stopped run's next `lf task run` (`select_task_run`, `ops/task.rs`) starts
  at `graph.place_of(run_start)`, except a step that ended on a usage limit,
  found by label (`limit_recovery::ended_step_recovery` → `graph.find(&step)`).
- Status: `read_task_graph` shows `TaskGraphPosition::Run { step: row.step }`,
  the step the Task stood at before the run ("Graph code: running from start"
  for this Task while it ran kickoff).

## Approach

The Graph process persists its checkpoint. `RunningGraph::checkpoint`, for a
Task run's Graph, also writes the cursor to `task_graphs.run_start` guarded by
`lf_process_id = carrier` (new `SqliteStore::advance_task_graph_run`). The
checkpoint after each tick is the cursor of the next step to run, so after a
failure `run_start` names the failed step — with its loop counts, iterate
direction, pending verdict and XOR route. `run_start` changes meaning from
"where the run began" to "where the run's next attempt sets out"; the column
keeps its name (no migration).

Everything else falls out of the one column:

- In-loop retry: the next child `graph run` reads `run_start` as today. No
  change in `run_task_graph`. The retry GraphProcess's first step row is the
  resumed step.
- Next `lf task run` after a stopped run: `select_task_run` starts at the
  stored cursor verbatim (not `place_of(cursor).cursor()`, which drops
  progress). The usage-limit label branch is deleted: `run_start` already
  stands at that step.
- Move to another Machine (`task_graph_record` / `place_moved_task_graph` /
  `world/mod.rs`): carries the resume cursor; the receiving run continues there.
- Status: `TaskGraphPosition::Run` gains `at`, the step at `run_start`
  (`graph.step_name(place_of(run_start))`, a command by its display name);
  `step` keeps meaning the segment origin, which Desktop's stage ladder
  (`TaskWorkView.swift`, `WorkflowDiagram.swift`) matches against
  `WorkflowDefinition.runs`. Decided 2026-11-13 during implement: rewriting
  `step` would put the ladder back at `start` mid-run. `summary` says
  "running at X" / "stopped at X". The retry message names the step it starts at.
- `--from` still wins: its branch precedes the stopped-run branch.

A failed checkpoint write logs a `tracing::warn` and the Graph continues: a
stale `run_start` reruns finished steps, which is today's behavior; failing a
Graph after a six-hour step is worse.

### Which steps rerun

- The failed step always reruns whole. Nothing records progress inside a step;
  an agent step starts a new turn, an operation (`land --wait-and-fix`,
  `rebase`) runs again from its own start and reads current PR/checkout state.
- Steps before it never rerun. Each finished, and the checkpoint after it was
  written before the next step spawned.
- Loop-or-next: a verdict is checkpointed before it settles, so a Graph that
  dies after the decision does not decide again. A failure during the decision
  reruns it with the pass count kept ("Decision pass N" stays right, the loop
  limit holds). A body step in pass ≥2 reruns with the iterate direction it had.
- XOR: a route taken is in the cursor; a failure inside a path resumes inside it.
- A step whose checkout changed (gate merged main, land fetched): earlier steps'
  work is committed in the checkout, and rerunning them was never a repair
  (that is the LOO-858 waste). When earlier steps must run again, `--from`.
- Unchanged stops: provider refusal (`TaskRunStopped::Limit`), the attempt
  limit, `Repeated`, retained-edits stash, held exits (3/130), an interrupt.
  After an interrupt, the next `lf task run` resumes at the interrupted step.

### Concept review (2026-11-13)

One record, one meaning: `run_start` is where the run goes on, and every reader
(retry loop, `select_task_run`, move, status) takes it verbatim. Settled, no change:

- `Run { step, at }` keeps the wire key `step` for the segment origin. Desktop
  binds it as `from`, but renaming the key would make a released Desktop fail
  to decode every Task from a branch `lf`; `at` is additive.
- `run_start` keeps its column name: a rename is a staged migration for no
  behavior.
- A Graph that checkpointed past its last step and died before arriving: the
  cursor has no place, `select_task_run` returns no start, and the next
  `lf task run` ends the Task as that run would have. XOR children never
  checkpoint at their path's end (`finish_inner` returns to the parent).
- A `-i`/`-b` run that decided at an interactive step and died before settling:
  a plain `lf task run` puts the Task at that step and the person decides again.

## Alternatives

- Derive the resume point from GraphProcess history (`graph_process_steps.node`
  + `iterations`, preorder key ↔ `walk` order). Answers "which step", but loses
  iterate direction and pending verdicts (only recoverable by re-decoding a
  decision step's final answer), and needs a carrier-scoped history query in
  two places. Rejected: the checkpoint is exact and already computed.
- Carrier rewrites `run_start` from history before each retry. Same loss, and
  the stopped-run path still needs its own read.

## Scope

Change:
- `lf/commands/graph.rs`: `RunningGraph` keeps the carrier id (replaces
  `task_run: bool`); `checkpoint` writes; retry message names the resume step;
  doc comments on `run_task_graph` / `run_task_graph_for`.
- `store/sqlite/task_work.rs`: `advance_task_graph_run(task, carrier, cursor)`;
  `read_task_graph` step from `run_start`; `TaskGraphRow`/`TaskGraphRecord` docs.
- `ops/task.rs` `select_task_run`: stopped-run branch uses the stored cursor;
  `TaskRunSelection.start` carries the cursor with its `Place` (users:
  `start_task_run`, `world/mod.rs`, `task/explain.rs`, `task/ship.rs`).
- `task_graph.rs`: `TaskGraphPosition::Run` doc, `summary` "at", `guidance`.
- Docs: `docs/orchestration.md` (~l.192 "leaves the Task where that run
  began"), `docs/dev/planning.md` (~l.479), `docs/dev/reference.md` (Task graph
  row), `lf task run` `after_help` in `lf/mod.rs`: "A run that fails starts
  again at the step that failed; the steps before it stay done."

Delete — do not maintain:
- `select_task_run`'s `ended_step_recovery(...).filter(...).and_then(graph.find)`
  branch. `ended_step_recovery` stays for `waiting_on_limit` and the carrier's
  limit checks.

Exclusions: no new table or column; no change to `TASK_RUN_ATTEMPTS`, limit
recovery, or Graphs run outside a Task run (`lf graph <name>` writes nothing).

Forbidden: a second resume record beside `run_start`; reading history to
second-guess `run_start`.

## Tests (`tests/linear/task_graph_launch_tests.rs`)

Built:
- `a_failed_run_starts_again_at_the_step_that_failed` (`held`: attempts 2–3
  record only `graph show late`; status `position.at`; after defining `late`,
  `lf task run` runs only it and arrives).
- `from_starts_a_stopped_run_where_it_names` (`--from` the first step reruns `sync`).
- Renamed/updated: `a_stopped_run_is_started_again_at_the_step_that_failed`,
  `a_run_that_failed_after_moving_agents_starts_again_at_the_step_that_failed`,
  and the limit-wait test (retries record only the failed step).
- DTO: `position.at` in `tests/fixtures/dto/task_work.json`,
  `task_lifecycle.json`, `task_overview.json`, `server_schema.json`,
  `dto_fixtures.rs`; Swift `TaskWork.Position.run` and its tests.
- Not added: a runner unit test for loop-body resume. `graph/runner.rs`
  (`nested_xor_checkpoints_root_and_recovers_a_pinned_pending_decision`)
  already restores direction and passes from a saved cursor; the checkpoint
  written now is that same cursor.

## Done when

`lf task run` on a Graph whose second step fails retries only that step, the
Task's status names that step while it waits or retries, and the next
`lf task run` (or `lf ship`) continues there; `--from` still starts anywhere.

Task brief's `docs/tasks.md` does not exist; restart behavior is documented in
the files below, and `lf help task run` says the same.

## Docs written

`docs/orchestration.md` (where a person takes part), `docs/troubleshooting.md`
(survives + what to run next), `docs/dev/planning.md` (`run_start` meaning),
`docs/dev/reference.md` (Task graph row), `lf task run` `after_help` in
`rust/loopflow/src/lf/mod.rs`. The orchestration example shows status as
"Graph code: stopped at land --wait-and-fix": `summary` must name the step at
`run_start` with "at", using the cmd step's display name.

## Checks

- `cargo test -p loopflow --lib -- task_graph:: task_graph_launch_tests:: ops::task::ship::tests::` — 65 passed after compress (before the main merge `5e18d3f548`). After the merge: 65 passed.
- `server::schema::tests::`, `--test integration dto_fixtures::` — passed after regenerating `server_schema.json`, before the merge; gate reruns.
- Swift (`TaskWork.Position.run` gained `at`) — not built here; gate runs `--swift`.

## Remaining

- Gate: Swift build/tests; CI suites.
- Demo: `lf task run` on `held` as in the Demo section, with `lf task status` naming the resume step.
