# LOO-534: lifecycle world recipe

Status: implemented, gated and published as [#1621](https://github.com/loopflowstudio/loopflow/pull/1621)
(2026-10-20); main merged at 52ed99b138 (#1622's `wave-backlog`, `task-review`, `wave-lifecycle`
recipes adapted to `task(Option<&str>)` and dated `complete`). Main merged again at f8d1b4f3cf
(2026-10-21) for #1610 (LOO-523): `WaveTaskGroups` groups Owned/Unowned. Remaining: CI, review and merge; Task completion follows delivery.
CI's matrix runs only once scratch is cleared (`scratch-clear` candidate), so it
runs at submit; #1621's head 40b769765f has only `scratch-clear` passing, the rest skipped. Jack Heart requested this as an
autonomous follow-up to lf-world (#1612) on 2026-10-20.

## Problem

Task-lifecycle views regress quietly: hand-built inline JSON rows in
`TaskHistoryFilterTests` prove predicates, not what `lf` actually emits for a
paused, abandoned, canceled, standalone or long-completed Task. The portfolio
world already shows one such regression: a completed Task reads as `waiting` in
`lf wave show` and appears under **Started** in Desktop's Wave inventory.

## Demo

```bash
lf-world build lifecycle /tmp/lc
cd /tmp/lc/repo && env $(jq -r 'to_entries[]|"\(.key)=\(.value)"' ../env.json) lf wave show release
```

Completed and abandoned Tasks sit under LATER as `clear`, never `waiting`.
The upstream-canceled Task keeps its started work current until someone
disposes of it. Opening that world in Desktop (`live` mode, world env): the Wave's
inventory lists only current Tasks under Owned / Unowned, with no
history control. Project history's filter admits two completions (7 Days),
three at 30, four at 0; canceled and abandoned Tasks never appear. The paused Task carries its Paused
label and is absent from the working sidebar; standalone Tasks appear without a
Wave.

## Reproduced bug (portfolio world, 2026-10-20)

`lf wave show desktop --json` on a fresh portfolio world: completed Task `88a613`
has `runtime.status=done`, `section=later`, but `condition.state="waiting"`
(`next_move` owner `wave`, reason `done`). Two causes:

1. **Rust** — `derive_task_condition` (`rust/loopflow/src/lf/commands/waves.rs`, the
   `(state, reason)` ladder near the `NextMoveOwner` fallthrough) never looks at
   terminal state, so a settled Task falls to `Waiting` whenever its next-move
   owner is not the Task.
2. **Desktop** — `WaveTaskInventory` (`swift/LoopflowMac/Views/WorkSurfaceView.swift`)
   groups every Task by `runtime.started` alone. It replaced the Wave detail
   when the work-first home landed (#1531) and never picked up #1403's
   `TaskHistoryFilter`; only the Portfolio window's `WaveDetailPane` still uses it.

Both are fixed in this Task. Nothing needs filing.

## Recipe: `lifecycle` (rung: artifacts)

One Wave `release` with a KR, plus standalone Tasks. Times relative to build.

| Task | Built through | Expect |
|---|---|---|
| Shipped yesterday | checkout, Session, Flow ok, complete @1d | Completed in 7d |
| Shipped this week | checkout, Flow ok, complete @5d | Completed in 7d |
| Shipped last fortnight | checkout, Flow ok, complete @12d, checkout removed | Completed in 30d only; missing checkout stays history |
| Shipped last quarter | complete @40d | Completed only at 0 (all) |
| Paused mid-flight | checkout, Session, Flow ok, pause | current, Owned, Paused, not in sidebar |
| Abandoned attempt | checkout, Flow failed, `task_abandon` | never in Completed or current |
| Canceled upstream | checkout, Flow ok, planning state `canceled` | never in Completed; stays current (started local work is unresolved) |
| Lost checkout | checkout, Session, worktree dir deleted | current, condition blocked/recovery |
| In progress | checkout, Session, Flow ok @0.05d | current, Owned |
| Not started | created only | current, Unowned |
| Standalone idea | `task(None, …)` | standalone, unowned |
| Standalone fix | `task(None, …)`, checkout, Session | standalone, owned |

## Approach

### lf-world additions (`rust/loopflow/src/world/mod.rs`)

- `task(wave: Option<&str>, …)` — `None` creates a standalone Task through
  `task_create` (LOO-505). Existing recipes pass `Some`.
- `complete(task, note, completed_days_ago)` — `task_complete`, then rewrite the
  Task's `Disposition.planning_completed_at` through `planning_write::local` in one
  transaction. Same idiom as `flow()`, which hands backdated rows to production
  writers. Production completion keeps reading the clock; no clock parameter
  is threaded through `task_state_delivery::queue_in`.
- `pause(task)` → `ops::task::task_pause`; `abandon(task)` →
  `ops::task::lifecycle::task_abandon`.
- `cancel_upstream(task)` — writes `Disposition{state:"canceled", completed:0}`
  through `planning_write::local`, the writer accepted Linear observations use
  for existing records. A Task becomes planning-canceled without `abandoned_at`.
- Every world commits `.lf/config.yaml` with `agent: claude`. Without it the
  default agent is the first installed harness, and an installed OpenCode made
  every started Task read `blocked` (no managed execution boundary).
- `remove_checkout(task)` — `remove_dir_all` on the worktree, leaving Git's
  worktree metadata. That matches a person deleting the folder.

### Rust fix

In `derive_task_condition`, a Task whose runtime is terminal (done/abandoned) with
no unresolved execution is `Clear` ("Task is complete" / "Task is abandoned"),
ahead of retained failed execution, launch refusal and missing-checkout rungs.
Unresolved execution keeps today's ladder; sections are unchanged.
Decision (2026-10-20, implement): no planning-terminal rung. Every
planning-canceled Task in the recipe has started work, so `unresolved_execution`
holds and the rung could not fire; an unstarted canceled Task is not modeled.

### Desktop fix

Extract `WaveTaskGroups` next to `TaskHistoryFilter`: from tasks alone, it
returns current Owned / Unowned Tasks by rank (`TaskHistoryFilter`'s default,
completion off; grouped on `runtime.owner != nil`, main's #1610 rule). The
history filter stays necessary: completion and abandonment keep the last owner,
so a done Task would otherwise sit under Owned. `WaveTaskInventory` renders those groups with no
history control, keeping #1594's assertion; completed history stays behind
Project history. Product memory: "A Wave pairs its conversation with all
current planned and started Tasks".

### Sidebar coverage

Jack Heart's October 17 LOO-489 Console decision superseded #1443's sidebar
rule (Product memory: "Completion does not automatically remove retained work";
"Pause hides membership"). Coverage targets the current contract: retained
done/abandoned Tasks stay in `workingTaskIDs`, and a retained paused Task leaves
it. The tension with the brief is recorded in `scratch/questions.md`.

## Source of truth and consumers

- Truth: SQLite Task rows (planning disposition, `paused`, `abandoned_at`,
  workflow/process evidence) written only by production writers.
- Derived: `lf wave show --json` (`TaskConditionSnapshot`, sections), and from it
  Desktop `RoadmapSnapshot` → `WorkProjection`, `WaveTaskGroups`, `workingTaskIDs`.
- Wire DTOs: unchanged shape. Only the `condition.state` value changes for
  settled Tasks.
- CI/test runners: `lf-world` must be built wherever Swift tests build `lf`.

## Acceptance at gate (headless)

1. `cargo test -p loopflow --test integration world_recipes` (LF_* unset,
   `< /dev/null`): every recipe still builds. A new `lifecycle_world_reports_each_state`
   reads `lf wave show --all --json` and asserts:
   - four completed Tasks with `completed_at` ≈ 1/5/12/40 days ago (±1 h);
   - completed and abandoned Tasks have `condition.state == "clear"`;
   - paused `runtime.status == "paused"`; abandoned `status == "abandoned"`;
     canceled `task.state == "canceled"`, runtime `not_ready`, unresolved execution;
   - lost checkout `local_progress.state == "missing"`;
   - two Tasks with no `wave_id` in top-level `tasks`.
2. `scripts/test_desktop.sh --filter LifecycleWorldTests` (after
   `cargo build -p loopflow --bin lf --bin lf-world`): a Swift suite builds the
   world with `target/debug/lf-world`, opens `WorkModel` with a `RegistryQuery`
   running `target/debug/lf` under the world env (the `WorkCLIFixture` pattern),
   and asserts:
   - `WaveTaskGroups`: completed/abandoned absent; paused, canceled
     upstream, lost checkout and in-progress under Owned; Unowned holds only the
     never-checked-out Task;
   - `TaskHistoryFilter` over the settled world rows admits 7 → 2, 30 → 3,
     0 → 4 completions; never abandoned/canceled;
   - `visibleWork.standaloneTasks` holds both standalone Tasks;
   - after `select(.task)` on a completed, abandoned, in-progress and paused
     Task, `workingTaskIDs` contains the first three and excludes the paused one.
3. `cargo fmt --check`, `cargo clippy --all-targets -- -D warnings`.

Existing inline-JSON tests in `TaskHistoryFilterTests` stay: they own pure view
logic and transitional states (Jack Heart's 2026-10-20 direction).

## Absent and error states

- World build failure aborts `lf-world build` with the failing writer's error
  and leaves the directory for inspection. Worlds are built fresh, never repaired.
- Missing `target/debug/lf-world` fails the Swift suite with the build command
  (`#require`, as `WorkCLIFixture` does for `lf`).
- A Task with unknown local progress keeps today's `Unknown` condition. The
  terminal rule only fires on settled evidence.

## Exclusions

- `duplicate` planning state, Linear-linked Tasks and PR/merge history.
- Portfolio-window `WaveDetailPane` changes. Its overlap with
  `WaveTaskInventory` is a separate reduction.
- Screenshot/UI-host captures of the lifecycle world. They need a display and
  are optional demo evidence.

## Affected surfaces

- `rust/loopflow/src/world/{mod.rs,recipes.rs}`, `rust/loopflow/tests/world_recipes.rs`
- `rust/loopflow/src/lf/commands/waves.rs` (`derive_task_condition` + unit test)
- `swift/LoopflowMac/TaskHistoryFilter.swift` (`WaveTaskGroups`),
  `swift/LoopflowMac/Views/WorkSurfaceView.swift`, new
  `swift/LoopflowTests/LifecycleWorldTests.swift`
- `.github/workflows/ci.yml` (Swift job build line), `scripts/test.py` (Swift
  prebuild), TESTING.md (Swift prerequisites)

## Delete — do not maintain

Nothing structural. `WaveTaskInventory`'s inline `started` grouping is replaced
by `WaveTaskGroups`, not kept beside it.

## Forbidden outcomes

- A world-only store writer or raw `UPDATE tasks` that bypasses `planning_write`.
- A clock-override env var or parameter added to production completion.
- Checked-in JSON captured from a world; tests build the world each run.
- A second history filter for the inventory; it reuses `TaskHistoryFilter`.

## Slices

### Main moved — done (2026-10-21)

Jack Heart's #1610 (LOO-523) is an accepted product decision: Task ownership
(person, Machine) replaced write-once `started_at`; `RoadmapTaskRuntime.started`
is gone and `WaveTaskInventory` groups Owned / Unowned. `lf task checkout`
(the world's `checkout`) now claims an unowned Task.

1. Merge `origin/main`. Only `WorkSurfaceView.swift` conflicts (git merge-tree);
   resolve to main's labels on top of `WaveTaskGroups`.
2. `WaveTaskGroups`: `started`/`notStarted` → `owned`/`unowned` on
   `runtime?.owner != nil`; callers in `WorkSurfaceView`, `LifecycleWorldTests`
   (lines asserting `current.started`/`notStarted`) follow. No `started` field
   or Started label survives.
3. Settled: the world's `checkout` claim records an owner in a fresh world;
   `World::new` needed no identity change.

Focused check: `cargo test -p loopflow --test integration world_recipes` (LF_*
unset, `< /dev/null`) and `scripts/test_desktop.sh --filter
'LifecycleWorldTests|TaskHistoryFilterTests|WorkNavigationTests'`.

### Original order

One PR. Order: (1) world API + recipe + Rust integration test, which fails on the
`waiting` assertion; (2) Rust `derive_task_condition` fix; (3) `WaveTaskGroups` +
inventory + Swift suite + CI build lines.

## Done when

`lf-world build lifecycle` yields a world where `lf wave show` never calls a
settled Task waiting. Desktop's Wave inventory shows only current Tasks by
default and completed history by range. Both are proved headlessly against
the built world, not hand-written rows.

## Checks

Kickoff: portfolio world reproduced `88a613` done with `condition.state=waiting`.
`cargo test -p loopflow --test integration world_recipes`: passed 2/2; fmt, clippy passed (publish).
`scripts/test_desktop.sh --filter 'LifecycleWorldTests|TaskHistoryFilterTests|WorkNavigationTests|WaveDetail'`: 50 passed (publish). Full Swift suite: gate/CI.
Gate: `cargo fmt --check`, `cargo clippy --all-targets -D warnings`, `world_recipes` 2/2, `commands::waves` 19/19, integration `wave` 15/15: passed.
Gate: `scripts/test_desktop.sh -Xswiftc -gnone`: 615/616; `ProgramStatusTests` "later status event recovers an exited observer" missed its 5 s deadline by 2 ms under full-suite load, passed on rerun with `LifecycleWorldTests` (14/14). Untouched by this branch.
Publish revision: inventory is current-only with no history control (#1594's assertion); completed ranges are proved on `TaskHistoryFilter` over world rows.
Realign after main merge (2026-10-20): `cargo test -p loopflow --test integration world_recipes` 3/3 passed at 52ed99b138; Swift and full CI deferred to #1621's CI.
Kickoff (2026-10-21): `git merge-tree HEAD origin/main` at f8d1b4f3cf — one conflict, `WorkSurfaceView.swift`; branch code still reads removed `runtime.started`.
Main moved (2026-10-21): `world_recipes` 3/3; `scripts/test_desktop.sh --filter 'LifecycleWorldTests|TaskHistoryFilterTests|WorkNavigationTests'` 42/42. fmt/clippy and full Swift: gate/CI.
Compress (2026-10-21): `WaveTaskGroups` drops its unused `now`; `scripts/test_desktop.sh --filter 'LifecycleWorldTests|TaskHistoryFilterTests|WorkNavigationTests'` 42/42. fmt/clippy and full Swift: gate/CI.
Realign (2026-10-21): origin/main unchanged since f8d1b4f3cf, merges clean; no stale `started` names; no checks rerun. Full CI: submit.
