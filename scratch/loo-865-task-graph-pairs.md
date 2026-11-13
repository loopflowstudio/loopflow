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

- Any membership fallback to the cwd rule for Graphs whose `task_id` is NULL.
  The backfill covers history, so NULL means "no Task". Execution observation
  still considers open Graph/step Processes in the checkout, including those
  predating adoption; it does not add them to Graph history.
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
  noted in `scratch/questions.md`. While such a Graph or its step still runs,
  `open_graph_execution` (`ops/task_execution.rs`) reports it in the Task's
  execution and `select_task_run` (`ops/task.rs`) refuses another run. The
  Waves board pays one `task_open_processes` query per detailed Task for this,
  bounded by unfinished Processes and run beside each Task's `git status`;
  not measured on the live store.
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
- A Graph running when its checkout is adopted shows as the Task's execution
  and makes its derived state `moving`, and blocks `lf task run` until it
  finishes, without joining the Task's Graphs.

## Checks

- Implement: `cargo test -p loopflow --lib store::sqlite::task_work::` and the
  migration test; `cargo test -p loopflow --lib server::events::`; `graph_inventory::`.
- Gate: `uv run python scripts/test.py --reuse-passing`; `cargo clippy --all-targets -- -D warnings`.
- Gate, headless on a store copy: apply the branch migration to a `.backup` of
  the live store and compare `lf task status --json` `graph_processes` ids for a
  sample of Tasks before and after.

## Status

Built and behind PR #1910 (CI green on the pre-adoption-repair head,
2026-11-13). Every "Done when" item is implemented; the adopted-mid-Graph
Task state passed its focused test and real-CLI re-walk (QA §7).

- Store copy: `.backup` of the live store (2,319 Graphs, 1,224 Tasks); main's
  `task_graph_pairs` over every Task, then the branch migration: 2,141 pairs on
  434 Tasks both ways, 0 differences, 0 Graphs with more than one Task.
- QA §1–4 (`scratch/qa.html`): a 0.15.3-written store opened by the branch keeps
  every Task's Graphs; new launches, `task.get` `graph.step_finished`,
  `plan.list` latest Graph, `lf graph list --processes`/`--taskless`/`--for-task`
  and `lf task status` agree with `graph_processes.task_id`.
- Gate (last on 4e8851107): `scripts/test.py --reuse-passing` stopped at
  resource preflight (uv cache inventory deadline), so its plan ran by hand:
  fmt, clippy, architecture, test inventory, website `dev.py test` (73), and
  173/173 focused tests. Gate fix: `entry_in` reads the recorded `task_id`.
- Adoption repair (QA §5 → §6): `open_graph_execution` and `select_task_run`
  see open Graph/step Processes in the Task's work. §6 confirmed execution
  `running`, `no_action`, `lf task run` refused with the same sentence, Graph
  row still NULL. `select_task_run` checks the Task's own run first, so its
  "`lf task interrupt`" message is unchanged.
- Task state (QA §6 "Still wrong" 1, header read `idle`): `task_state_sql`
  (`store/sqlite/durable.rs`) has one running branch: own run, ship process,
  or an open `lf` Process that is a Graph or a Graph's child in the Task's work
  (`task_open_process_ids`, relocation-aware). It reports `moving`, or
  `shipping` when approved; done/abandoned take precedence. Live store,
  read-only `EXPLAIN QUERY PLAN` (1,233 Tasks, 172,024 Processes, 2,396 Graphs,
  17,928 Sessions): `processes_unfinished` plus point indexes; all Task states
  in one statement 295 ms. 10,000-Task in-memory fixture: 1,344 ms.
  `cargo test -p loopflow --lib -- adopting_a_checkout_with_a_running_graph_prevents_another_run process_membership open_process task_state`
  — 6 pass (18 min build): snapshot and `task_detail_facts` both `moving`, then
  `idle` once Graph and step exit.

Remaining:
- CI: finish the selected Rust tests not completed by the bounded gate below:
  remaining `task_graph_launch_tests`, `world`, `documented_commands`,
  `task_github_cache_tests`, `two_people` and `transport_tests`.
- CI: compile the extracted crate (`cargo package --locked --allow-dirty -p loopflow`)
  and run the populated-release/checkout-adoption installation proof
  (`uv run python scripts/test_task_installation.py --test task_adopts_linear_checkout_and_preserves_graph_history`).
  Only archive creation ran locally. The host remained heavily contended and
  free disk ended at 35 GiB against the 32 GiB emergency reserve; no additional
  build or proof container was started.
- Not this change: QA §6 "Still wrong" 2 (a signal-killed provider leaves
  `condition.unresolved_execution` true while `action` is `run`).

`cargo build -p loopflow --bin lf` + QA §7 on 7eac8cef01 — passed:
CLI header and JSON state `moving` → `idle` after Graph exit; history empty,
recorded Task NULL throughout; second run refused. No walkthrough processes remain.

Compress (after 7eac8cef01): `open_graph_execution` picks its Graph in one
`match`; nothing else to reduce. The Graph-or-its-step rule stays in two forms,
SQL in `task_state_sql` and Rust in `open_graph_execution`: sharing it needs a
second open-Process reader that still has to ask which of the two is the Graph.
`cargo test -p loopflow --lib -- adopting_a_checkout_with_a_running_graph_prevents_another_run` — 1 pass.

Since QA §7 the header path has not changed: compress touched only
`open_graph_execution`; merging main (0afb302f52) changed `query_plans.rs`, one
test's index name, and `foreign_key_children.sql`, which drops
`agent_process_session` (no branch query names it). QA §8 re-walked 8dec86f0ca
on a fresh Machine seeded by installed 0.15.3+87f0e3c60: both migrations apply in
order, Graph readings identical, adopted-mid-Graph `moving` → refusal → `idle`
→ recorded run. Gate's Task-state suites passed on 7713aab257.

### Gate on 7713aab257 (2026-11-13)

Plan: Graph launch/writer changes reach Task controls and launch fixtures;
store ownership, migration and Task-state changes reach migration, query-plan,
inventory, Task-work, shared-state, subscription and shared-writer suites;
documentation changes reach command examples and website rendering. No Swift,
Python or provider-lifecycle implementation changed.

Commands below ran with inherited `LF_*` unset and stdin closed.

- `uv run python scripts/check_migrations.py` — passed; `graph_process_task.sql` is the one unmerged migration.
- `uv run python scripts/test.py --list` — architecture, Rust and website selected.
- `uv run python scripts/test.py --reuse-passing --rust-filter '<selected suites below>'` — resource preflight stopped before tests: uv-cache and capture inventory deadlines; a changing build file in another checkout also disappeared during inventory. The documented manual plan followed; this is not a passing runner result.
- `uv run python scripts/check_architecture.py` — passed all seven maps, zero unexplained entries.
- `uv run python scripts/check_rust_test_inventory.py` — passed.
- `cargo fmt --all -- --check` — passed.
- `cargo build -p loopflow --bin lf --jobs 4` — passed (2.23 s).
- `cargo clippy --all-targets --jobs 4 -- -D warnings` — passed, exit 0 (3.64 s cached confirmation). The first attempt printed a successful finish after 8m 25s but its outer process returned 143 without its receipt; the second invocation resolved that uncertainty on identical Rust inputs. No lint diagnostics.
- `LOOPFLOW_TEST_LF=$PWD/target/debug/lf cargo --config 'target."cfg(all())".runner = ["uv", "run", "--no-sync", "python", "/Users/jack/src/loopflow.loo-865-read-task-graph/scripts/test_network.py"]' nextest run -p loopflow --no-fail-fast --build-jobs 4 --test-threads 4 -E '<selected suites below>'` — 266 passed, four interrupted, 183 not started; stopped at the 2,400-second phase limit, not green. Build took 16m 42s, tests 1,372.861 s. Host load was 271–341, with 8.7 GiB swap used at one observation; tests kept completing, with no assertion failure before interruption. CI owns the remaining 187 cases. All recorded descendants exited after SIGINT.
- `cd website && uv run python dev.py test` — 73 passed (212.17 s), including portable architecture and accessibility. Reference generation used the installed CLI; its `help --all --json` SHA-256 matched the candidate byte for byte (`26a2262841e3babab5e61d66c8e794368c87a75c29f9a80d52431f3cdc614bcf`). Website server/browser processes exited.
- `cargo package --locked --allow-dirty -p loopflow --no-verify` — archive creation passed: 940 files, 16.3 MiB (3.5 MiB compressed); checked that the migration and released fixture occur once each in the archive. Compilation remains with CI. Cargo warned that the existing locked `chacha20 0.10.0` is yanked; dependencies are unchanged by this branch.
- `git diff --check` — passed.

Selected suites: `test(store::sqlite::task_work::) | test(store::sqlite::graph_inventory::) | test(store::sqlite::durable::) | test(store::migrations::) | test(store::query_plans::) | test(ops::task::) | test(ops::task_execution::) | test(task_graph_launch_tests::) | test(task_github_cache_tests::) | test(lf::commands::waves::tests::) | test(lf::commands::graph::tests::) | test(server::events::) | test(documented_commands::) | test(world::) | binary(two_people) | binary(transport_tests)`.

All selected migration, query-plan, Task-work, Graph inventory, Task controls,
Task execution, Wave and subscription tests passed. This includes the new
backfill, adoption-state/refusal regression and the 2,000-unrelated-Graphs
read-cost assertion. The macOS linker warned about a large unwind table; no
compile error. Gate corrected the QA report's stale “Nothing in the product”
summary to name the adoption repairs; production code is unchanged by this pass.

## Docs

`docs/dev/data.md`, `planning.md`, `reference-models.md`, `index.md` (+ regenerated `index.html`); user-facing `docs/work.md` (an adopted checkout's earlier Graphs stay unattached). `lf task status` output is unchanged.

## Concept review (2026-11-13)

A Graph runs *for* a Task (or a Wave, or nothing); the launch already decides
which, and the store now records that decision. A Graph run with an explicit
`--wave` inside a Task's checkout no longer counts as the Task's. Writers are
the launch (`graph.rs` execute), `World::graph` and test fixtures.

Second pass (2026-11-13): nothing to change. A Graph's Wave stays derived (its
Task's Wave, which `lf task move` can change, else its steps' captures), so
only the Task is recorded. The adoption choice in `scratch/questions.md` gained
its sharpest case: a Graph still running when the checkout is adopted.

Third pass (2026-11-13, after the adoption repair): nothing to change. Two
readings now exist and each has one meaning: a Task's Graphs are the ones
launched for it (recorded); a Task's execution is whatever still runs in its
work (open Processes by checkout). The adopted-mid-Graph case walks through as:
status shows it running, `lf task run` refuses with the same sentence, and it
ends by finishing or being stopped where it was started. `lf task interrupt`
records its interrupt on the Task's Work, which a Graph launched without that
Work never reads; its receipt already says only "a Task run or a `--task`
Session stops on it". No open product choice beyond the one in
`scratch/questions.md`, which has its direction.

Fourth pass (2026-11-13, after the Task state repair): nothing to change. State
now follows the execution reading, so header, board, `execution` and `lf task
run` say the same thing about one Task. "Execution is whatever runs in the
Task's work" is by checkout containment, which reaches three cases the recorded
association excludes; each follows from the rule and none is in the live store:

- A `--wave` Graph run inside a Task's checkout makes that Task `moving` and
  holds its `lf task run`, while staying out of its Graphs.
- A nested Task's Graph makes the enclosing Task `moving` too (Process
  membership is every containing checkout, not the deepest).
- Auto-Ship approved while an earlier Graph still runs in the adopted checkout:
  state reads `shipping`; `restart_ships` (`ops/task/ship.rs`) skips only a run
  in `task_graphs`, so the ship process starts, is refused by `select_task_run`,
  and retries with its usual backoff until the Graph exits. Read from code, not run.
