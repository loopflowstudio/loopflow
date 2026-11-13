# LOO-865: Read Task–Graph pairs without walking every Graph

## Problem

`task_graph_pairs` (`store/sqlite/task_work.rs`) works out which Graphs belong
to which Tasks on every read. It walks every Graph process, every ancestor of
its cwd, and every Graph's Session links. So `lf task status`, the Waves board
and subscription observers pay for every Graph ever run, however few Tasks they
ask about.

## Demo

`lf task status LOO-865` and the Waves board show the same Graphs as before.
The store's Graph reading for one Task drops from ~27 ms to ~0.02 ms on Jack's
817 MB store. A regression test fails if adding 2,000 unrelated Graphs grows
one Task's reading.

## Approach

Record each Graph's Task once, when the Graph starts: `graph_processes.task_id`.
Reads look pairs up through an index. The launch already knows its Task:
`lf::commands::graph::execute` builds `RunningGraph.task` from the explicit
`--task` binding, or from `run::implicit_binding` → `ops::run::resolve_execution_binding`,
which covers the checkout containing the cwd and an agent's
`WORK_DECLARATION_ENV`. That one resolution becomes the stored fact, and the
read-time cwd/Session rule for Graphs is deleted.

### Probe (2026-11-12, copy of `~/.lf/loopflow.db`: 817 MB, 2,283 Graphs, 169,820 Processes, 1,221 Tasks)

| Read | Today (warm copy) | Recorded `task_id` + index |
|---|---|---|
| one Task (graphs_of_task) | 16 ms ancestor walk + 11 ms Session links | 0.017 ms |
| 30 most recent Tasks | same walk | 0.061 ms |
| every Task | ~37 ms | 0.23 ms |
| backfill migration | n/a | 0.3 s with the mapping materialized first (a correlated UPDATE took 59 s) |

On that store the backfill reproduces today's rule exactly: 2,106 Graphs paired,
0 Graphs paired with more than one Task, 0 reused checkout roots, 0 Session-link
pairs, 0 relocation receipts. One Graph predates the Task that later adopted its
checkout.

### Alternatives rejected

- **Task-driven range probe over `processes.cwd` with a new index.** One Task
  costs 0.2 ms, but every Task costs 130 ms (it visits the 134k Processes
  inside some checkout), worse than today for the Waves board. It also adds an
  index to the hottest insert table. The 7.7 s figure in the Task brief came
  from probing with no cwd index.
- **Copying the Graph's cwd onto `graph_processes` and probing it per Task.**
  Fast in both directions, but it duplicates `processes.cwd` and keeps the
  Session-link walk, which still grows with every Graph.
- **Driving Session links from the Task.** October 18, 2026: each Task rescanned
  all 4,246 Sessions, 2.3 s for 622 Tasks. That is why the Graph-driven form
  exists (test comment in `task_work.rs`).

## Source of truth

`graph_processes.task_id TEXT REFERENCES tasks(id)`, NULL for a Graph with no
Task (a Wave or repository run). It is written in the same INSERT as the Graph
record and never changes; the table stays append-only. Tasks are never deleted
(`DELETE FROM tasks` appears nowhere in `src`).

## Changes

1. **Migration** (`uv run python scripts/new_migration.py graph_process_task`):
   `ALTER TABLE graph_processes ADD COLUMN task_id`. Drop and recreate
   `graph_processes_are_append_only` around the backfill. Backfill from a TEMP
   table holding the relocation receipt if any, else the deepest local checkout
   containing the cwd (ancestor walk, `rtrim(worktree,'/')=path`), else the
   Task bound to a Session linked through `session_events.lf_process_id` or an
   attached agent (`agent_sessions.task_id`). Then `UPDATE … FROM`, then
   `CREATE INDEX graph_processes_task ON graph_processes(task_id) WHERE task_id IS NOT NULL`.
   It is additive: an older lf already refuses any newer schema (exact schema checks).
2. **Writer:** `record_graph_process(graph_process, compiled, task: Option<&TaskId>)`.
   Callers: `graph.rs` execute (`running.task.as_ref()`), `world/mod.rs`
   `graph()` (pass the `task` its caller holds), and the `test_graph` helper in
   `graph_inventory.rs`. `test_graph` resolves its Task with `task_of_process`
   under `cfg(test)`, so cwd-built fixtures keep their meaning.
3. **Readers:** `task_graph_pairs(task_ids)` becomes
   `SELECT task_id, lf_process_id AS graph FROM graph_processes WHERE task_id IN (…)`
   for its three callers (`graphs_of_task` passes `?1`, a canonical `TaskId`;
   `completed_graph_steps` and `latest_task_graphs` pass `json_each`); no join on
   `tasks`, and the partial index serves `IN`. Rewrite the doc comments that justify "driven from the
   Graphs" and "asking per Task repeats that walk".
4. **Tests:** replace the VmStep test at `task_work.rs` ~1880–1960 (unrelated
   Processes, unrelated Sessions) with: one Task's Graph reading does not grow
   when 2,000 Graphs of other Tasks are recorded, and every Task's reading does
   not grow with Sessions that reach no Graph. `every_task_latest_graph_matches_its_own_graphs`
   passes Tasks explicitly. A migration test seeds the released schema (fixture
   under `rust/loopflow/tests/fixtures/migrations/`) with deep, sibling
   (`/repo/task` vs `/repo/task-other`), relocated and outside Graphs, and checks the backfill.

## Delete: do not maintain

- In `task_graph_pairs`: the `f`/`up` ancestor walk, the `checkouts` CTE, the
  `links` CTE with its `session_events_process` probe, and the relocation
  branch. `has_relocations`, `process_membership`, `session_membership` and
  `lf_process_ids` stay: Process and Session membership still use them.
- The test assertions that pin the Graph-driven shape ("Graph reads start from
  the Graphs…").

## Forbidden outcomes

- Any read-time fallback to the cwd rule for Graphs whose `task_id` is NULL.
  The backfill covers history, so NULL means "no Task".
- A second writer of `task_id`, or an UPDATE path. The append-only trigger stays.

## Absent and error states

- A Graph with no Task: NULL, and it appears in no Task's Graphs (as today for a
  cwd outside every checkout).
- A Task with no Graphs: empty list / no latest Graph, as today.

## Exclusions and semantic change

- A Task that later adopts a checkout (`lf task create --worktree`, `lf session
  bind`) does not pick up Graphs that ran there before it existed. Today it
  does; the live store has one such Graph. The Task's Process list
  (`lf_process_ids`) still shows that Process by cwd. Taking the simpler option;
  noted in `scratch/questions.md`.
- Graph association now follows launch binding (canonicalized cwd, work
  declaration), not the lexical cwd test. Symlinked checkout paths resolve the
  same way the launch does.
- A Graph in a nested checkout is the deepest Task's only; the enclosing Task
  no longer lists it (its Sessions still do). The live store had none.
- A Task whose worktree is cleared keeps the Graphs started for it; before, they
  dropped out with the checkout.
- A Session that links a Graph after launch no longer moves the Graph to its
  Task. The Graph summary's `task_id` (`entry_in`) reads the recorded column
  too, so inventory, `--taskless` and Task status agree.
- `completed_graph_steps` for every Task still costs ~40 ms in the step join
  (14k steps). Callers ask about watched Tasks, so this is out of scope.

## Done when

- `lf task status <id>`, `lf task status --json` `graph_processes`, the Waves
  board's latest Graph and `GraphStepFinished` subscription events show the
  same Graphs as before on a migrated copy of a real store.
- One Task's Graph reading is independent of how many Graphs other Tasks ran.

## Checks

- Implement: `cargo test -p loopflow --lib store::sqlite::task_work::` and the
  migration test; `cargo test -p loopflow --lib server::events::`; `graph_inventory::`.
- Gate: `uv run python scripts/test.py --reuse-passing`; `cargo clippy --all-targets -- -D warnings`.
- Gate, headless on a store copy: apply the branch migration to a `.backup` of
  the live store and compare `lf task status --json` `graph_processes` ids for a
  sample of Tasks before and after.

## Status

Built and gated; left: CI (`binary(session_launch_contract)`, full matrix). PR #1910.

- Store copy 2026-11-13: `.backup` of the live store (2,319 Graphs, 1,224 Tasks); main's `task_graph_pairs` over every Task, then the branch migration. 2,141 pairs on 434 Tasks both ways, 0 differences, 0 Graphs with more than one Task.
- QA 2026-11-13 (`scratch/qa.html`): a 0.15.3-written private store opened by the branch keeps every Task's Graphs identical; new launches, `task.get` `graph.step_finished`, `plan.list` latest Graph, `lf graph list --processes`/`--taskless`/`--for-task` and `lf task status` agree with `graph_processes.task_id`.
- Gate 2026-11-13 (last on 4e8851107): `scripts/test.py --reuse-passing` stopped at resource preflight (uv cache inventory deadline), so its plan ran by hand. nextest test processes on this Machine enter `TN` until SIGCONT, other worktrees' too; runs continued only their own descendants. fmt, clippy, architecture, test inventory, website `dev.py test` (73), and 173/173 focused tests (`task_work::`, `graph_inventory::`, `bind_attribution::`, migration test, `server::events::`, `graph_tests::`, `task_graph_launch_tests::`, `planning_foreground_tests::`, `graph_discovery_tests::`, `machine_commands::`, `record_graph` integration, `documented_commands::`): pass.
- Gate fixes: `entry_in` derived a Graph's `task_id` from the cwd/Session rule while `graphs_of_task` read the column; it now reads the column.
- Compress 2026-11-13: `graphs_of_task` probes `task_id IN (?1)` directly. A second pass on 4c54a93ec found nothing further to cut; `entry_in`'s per-Graph `task_id` lookup replaces an equal per-Graph `task_of_process` call.

## Docs

`docs/dev/data.md`, `planning.md`, `reference-models.md`, `index.md` (+ regenerated `index.html`); user-facing `docs/work.md` (an adopted checkout's earlier Graphs stay unattached). `lf task status` output is unchanged.

## Concept review (2026-11-13)

A Graph runs *for* a Task (or a Wave, or nothing); the launch already decides
which, and the store now records that decision. A Graph run with an explicit
`--wave` inside a Task's checkout no longer counts as the Task's. Writers are
the launch (`graph.rs` execute), `World::graph` and test fixtures.
