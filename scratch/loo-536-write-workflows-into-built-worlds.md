# LOO-536 · Write Workflows into built worlds

Status: implemented 2026-10-21 (kickoff 2026-10-20); demo and post-merge comments remain. Jack Heart requested this 2026-10-20 as
an autonomous follow-up to lf-world (#1612); inline JSON stays for pure view
logic, failure/transitional states, terminal/focus proofs and DTO fixtures.

## Problem

LOO-434 (Workflow experience) needs a Task standing mid-Workflow to design
against; LOO-392 (Flow exec step row clips) needs a Flow with six or more steps.
Neither exists in any built world: `World` has no Workflow writer, and
`World::flow` invents a graph from a hand-written skill list, so every recorded
Flow is one or two fake steps (`implement`, `gate`) that match no authored Flow.

## Demo

```bash
cargo build -p loopflow --bin lf-world --bin lf
target/debug/lf-world build one-task /tmp/w
env $(jq -r 'to_entries[]|"\(.key)=\(.value)"' /tmp/w/env.json) \
  target/debug/lf task status <ISSUE>          # "Workflow feature: at demo"
env … target/debug/lf flow list --processes --for-task <ISSUE>   # task-design, pursue
env … target/debug/lf flow show --processes <PURSUE-ID> --json  # six steps, keys 0–5
scripts/loopflow-dev.py run-debug --world /tmp/w
```

In Desktop the Task shows the `feature` Workflow waiting at `demo` with its move
history; opening the `pursue` exec draws six steps (implement, compress, sync,
realign, loop-or-next, pr-publish) and the row clips at the default split —
LOO-392 reproduced. `lf-world build portfolio` adds a Task stopped on a failed
`pursue` edge and one a person moved to `demo` by hand.

## Approach

**Flows come from authored definitions.** `World::flow` takes a Flow name (and
the parent Process that started it, if any), loads
it from the world repository with the production loader (`flow::load_flow` +
`compile_flow`, as `ops::task::load_task_flow` does) and builds `FlowGraph::new`
from the compiled steps. Real `pursue` compiles flat to six nodes (keys 0–5,
`refresh` expanded, `loop-or-next` with `returns_to: 0`), which is exactly the
LOO-392 shape. One step Process per top-level node, in preorder: skills record
`lf -b skill <label>`, commands their `Command::argv()`. An `Outcome` replaces
the outcome string: `Succeeded`, or `FailedAt(k)` which records steps 0..=k with
step k failed and nothing after it. Single pass, so iterations stay `[[]]`; XOR
graphs are out (no recipe needs one — refuse with an error naming the Flow).

**Workflow writers follow `lf task run` and `lf task workflow move`.** Each
records its Process as the mover, the way `World::flow` already records
Processes, and calls the store writers those ops call after their checks:

| World method | Production path | Process recorded |
|---|---|---|
| `task_run(task, requested, ended_days_ago, minutes, outcome)` | `ops::task::select_task_run` (already `pub(crate)`: Wave → repository → builtin selection, edge-name matching and refusals listing outgoing edges); `take_up_workflow` when it takes up; `choose_workflow_edge`; Flow via `World::flow` with the Task run as parent; `arrive_workflow_edge` on success. `TaskRunSelection::Flow` records the Flow alone, Task off any Workflow | `lf task run ISSUE <requested>`, parent of the Flow process, completed with the Flow's outcome |
| `move_to(task, node, note)` | `ops::task::validate_workflow_move` (made `pub(crate)`), then `sqlite.set_workflow_node` | `lf task workflow move ISSUE <node>` |

Take-up and the first choice share one Task-run Process, as in production:
`lf task run ISSUE feature` takes up `feature` and leaves `start` by its one
edge. A failed edge leaves the Task on the edge with a completed Task run, so
status reads `stopped on pursue` — the real post-failure state. `task_run`
refuses an edge with no Flow or into `end` (completion belongs to
`World::complete`); `move_to` refuses `end` the same way.

**Recipes.**
- `one-task`: `task_run("feature")` (takes up, runs `task-design` → design),
  then `task_run("pursue")` (→ demo, six steps, succeeded). Its Session stays.
  Demos string adds "Task waiting at a Workflow node; six-step Flow exec (LOO-392)".
- `portfolio`: "Record a reader floor" runs `task_run("code")` whose `pursue`
  edge fails at `sync` (stopped edge, three step rows). "Open conversations
  faster" runs `task_run("feature")` to design, then a person moves it to
  `demo` with the note "Built in the Session" (a `set` move by a person). "Show
  Task rows by state" runs `task_run("pursue")` with no Workflow (a plain Flow)
  and completes. The `implement`/`gate` pseudo-flows go.

## Risk findings

- **Move times are build time.** `append_workflow_move` and `choose_edge_in`
  stamp `now_unix()`; Flows are dated days back. History order (by `seq`) is
  right; timestamps cluster at build. Accepted: adding an `at` parameter would
  reshape production writers for a fixture. Workflow-carrying Flows in recipes
  end within the last hour so the skew is small. LOO-434's history display should
  not be judged on absolute move times from a world.
- **A running edge can't be built honestly.** `read_workflow` judges `running`
  from the carrying Process: no `completed_at` plus non-Dead journal evidence. A
  fabricated open Process with no pid reads as running forever. Excluded; the
  running-edge state stays in inline JSON (transitional state, per Jack's rule).
- **`record_flow_process` sets `started_at` to now** on the Task, as it does for
  existing world Flows. Unchanged.
- **Ops-level entry points need a registered current Process**
  (`journal::current_lf_process_id`) and stamp real time; `task_place` also
  checks out, selects agents and comments. The store writers are what those ops
  call after their checks, so the world uses them directly, consistent with
  `record_flow_process`/`record_flow_step` today.
- **Builtin definitions resolve in the world repo**: `load_workflow` falls back
  to builtins (`feature`, `code`) and `load_flow` to builtin `pursue`,
  `task-design`; no files need writing into the world repo.
- **Production never takes up without choosing** (checked 2026-10-21):
  `traverse_workflow` takes up and chooses in one process, so a standalone
  `take_up` writer would build a state `lf` cannot reach. Merged into
  `task_run`. A hand move is reachable only after take-up, hence the
  `feature` → design → move-to-demo path in `portfolio`.
- **`pursue`'s graph is pinned** by `flow::graph` tests (labels implement,
  compress, sync, realign, loop-or-next, pr-publish; one return), so a builtin
  edit that shrinks it fails there before LOO-392's reproduction silently goes.

## Alternatives considered

- *Keep the skill-list `flow` and add a second named-Flow writer.* Two writers
  for one record; the skill list can't produce a real graph. Rejected.
- *Drive `lf task run` as a subprocess with stand-in agents.* Proves more of the
  runner but launches providers and stamps real time; worlds are rows/artifacts,
  not executions. Rejected.
- *Hand-build an edge-less Workflow row in SQL.* Bypasses the writers; a schema
  change would strand it. Rejected — the reason lf-world exists.

## Contract

- **Source of truth:** `task_workflows`/`task_workflow_moves`, `processes`,
  `flow_processes`/`flow_process_steps` in the world's `LF_HOME`. CLI status,
  `lf flow` and Desktop read them unchanged.
- **Affected surfaces:** `rust/loopflow/src/world/{mod,recipes}.rs`,
  `tests/world_recipes.rs`. The `portfolio` screenshots in
  `scripts/screenshots.yaml` render from the changed world — regenerate at gate,
  no manifest change. `docs/architecture-reference.md`'s Fake world row: mention
  Workflows. No DTO, schema or CLI change.
- **Exclusions:** fixing LOO-392's layout; LOO-434's design; running edges;
  reaching `end` through an edge; XOR Flows; dated moves.

## Delete — do not maintain

- `World::flow`'s `skills: &[&str]` parameter and the ad-hoc `ConcreteSkill`
  construction from names; the `"implement"`/`["implement","gate"]` pseudo-flows
  in both recipes.
- The `outcome: &str` parameter (replaced by `Outcome`).

## Done when

1. `one-task`'s Task reads `Workflow feature: at demo` in `lf task status`, with
   history took_up, chose, arrived, chose, arrived; its `pursue` exec has six
   step Processes on graph nodes 0–5.
2. `portfolio` has a Task stopped on a failed `pursue` edge (three step rows)
   and one at `demo` by a person's `set` move with its note.
3. `cargo test -p loopflow --test integration world_recipes` asserts (1) and the stopped
   edge through `lf task status --json` / `lf flow list --processes --json`
   (headless; expected: pass).
4. Desktop opens the one-task world and shows the clipped six-step row (demo,
   by eye; LOO-392's own render proof stays with LOO-392).
5. After merge, LOO-392 and LOO-434 carry a progress comment naming the recipe
   and the `lf-world build one-task <dir>` / `run-debug --world` commands.

## Implementation notes (2026-10-21)

- `World::flow` became private: every recipe Flow now goes through
  `task_run`, so a public taskless writer would have no caller. Steps record
  under an `lf --task ISSUE flow -- NAME` Flow process, child of the
  `lf task run ISSUE <requested>` Process; starts are offset one second
  (run → Flow → first step) so Process lists sort causally.
- `load_task_flow` and `validate_workflow_move` are now `pub(crate)`.
- Rebased onto main `b4d544656d` (2026-10-21), which added the `lifecycle`,
  `density`, `wave-backlog`, `task-review` and `wave-lifecycle` recipes (#1617,
  #1621) with 13 more skill-list `world.flow` calls. Each now runs
  `task_run(task, "pursue", …)`: succeeded stays `Succeeded`; the old failed
  `implement`/`gate` runs (lifecycle's abandoned attempt, wave-backlog's failed
  Task) fail at `implement` (`FailedAt(0)`). Step durations kept their per-step
  minutes, so these Flows now span six steps. Their consumers
  (`tests/world_recipes.rs`, `LifecycleWorldTests`, `WorkDensityWorldTests`)
  assert Task state, not Flow shape.
- Wave memory took main's curation; the branch's own memory wording edits were
  dropped (memory is read-only to this Task).
- Tests live in `tests/world_recipes.rs`, compiled into the `integration`
  binary; step evidence reads `lf flow show --processes <id> --json`.

## Remaining

- Sync onto main `9c08610519` (#1619, LOO-531). It edits `portfolio`'s
  "Open conversations faster" block (adds `world.uncommitted` README edit and
  `world.comment` after the old `world.flow`), so `recipes.rs` will conflict:
  keep `task_run("feature")` + `move_to(…, "demo", …)`, then main's
  uncommitted/comment calls. It also binds Sessions then appends an input and
  writes `config.yaml` with person `Demo` — no overlap with the Workflow writers.
- Gate: affected suites, including the Swift built-world suites
  (`LifecycleWorldTests`, `WorkDensityWorldTests`, and #1619's `WorldReadTests`,
  which builds `portfolio`) whose recipes changed Flow shape; regenerate
  `portfolio` screenshots. `WorldReadTests`' `taskFlowProcesses` row expects
  exactly one non-empty Flow on the running Task: still true (one
  `task-design` Flow; the move to `demo` records no Flow). Its Wave/roadmap
  Task counts are unchanged, since no recipe adds or removes Tasks.
- Done-when 4 (Desktop demo by eye) and 5 (LOO-392/LOO-434 comments after merge).

Check (compress, 2026-10-21): `cargo test -p loopflow --test integration world_recipes` — 3 passed; second pass dropped `World::flow`'s unused id return, `cargo clippy -p loopflow --lib -- -D warnings` clean; gate owns affected suites.
