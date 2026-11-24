# LOO-1063 · Start-Time Stack and sidebar status toggle

## Direction & exact words

- Initial brief: “lets update the order of tasks in the left bar. things keep jumping to the bottom when i enter in them which is weird.  im not sure what i want the default order to be”
- Jack Heart (after Round 1 of [Explore](explore.html)): “ok i think  i like b.  i think we could do something liek what rex does with a button that sorts by terminal status, or alternatively we could sort by what step theyre at” / “i think we need something like moving auto to the bottom, or being able to 'close' certain tasks, but auto has to be smart enough to know like if its moving towards the end, that coutn even if its not explicitly auto”
- Jack Heart (after Round 2 of [Explore](explore.html)): “lets add b1” / “but also show the current step in the leftp ane, just not sort by it now”
- Jack Heart (on OSC 7501 `done` + New Task modal `?` font boxes): “done makes the task seem done but i dont tink thats what it means” / “isnt done then also available” / “yes, file it in, along with your other points” / “sorry, i meant to do both here”
- Jack Heart (during live `/demo`, [demo-notes.md](demo-notes.md)): “whqt is that arrows button” / “well, it needs a on/off state, and maybe an on-hover” / “1a” (toggle view: On shows status-sorted sidebar without overwriting saved drags; Off restores Start-Time Stack / drag order) / “build it and return when ready”
- Jack Heart on `#1973` (LOO-937) vs LOO-1063, 2026-11-24: “1063 is right”. Working sidebar order stays Machine-local in `task_orders`, apart from Project/backlog `planning_rank`.

## Concept reviews

- [`14f44821e8` review]: replaced integer slot pinning with `StatusGroup` and extended sorting to Wave sidebars (`be25d7ddcb`).
- [`5317225dbe` review]: unified `heldStatusGroup` ownership in `WorkNavigation` + pre-update latching in `WorkModel.applyTaskList` and aligned two stale status sentences in [swift/README.md](../swift/README.md) (`10df280f65`, simplified in `aef236355e`).
- [`0bc19e976d` review](concept-review.html) (against base `62a368e5cc` after merge `8742243abf` brought in `#1973`): 8 of 9 product-to-code pairings align, including all runtime ordering, drag persistence, status-toggle, status-group hold, Graph step, OSC 7501, and font pairings. One pairing contradicts the product: `task_orders` is permanently live in runtime, yet immutable `task_facts_once.sql:84` still marks `-- leaving: task_orders LOO-877`, leaving `leaving_guard.rs` (`STILL_READ`), `scripts/due_removals.py`, `scripts/check_migrations.py`, `rust/loopflow-storage/SCHEMA.md`, `docs/dev/reference.md`, and `docs/dev/reference-code.md` claiming `task_orders` is leaving for deletion on `2026-12-24` and `tasks.planning_rank` is the single Task order.

## Next slice (retract `task_orders` leaving mark forward)

Choice recorded for review (reversible; option `1a` / `2a` in [concept-review.html](concept-review.html)): add `-- keeping: <table>[.<column>] <Task ID>` as the forward retraction of `-- leaving:` in immutable migration history, rather than leaving `task_orders` in `STILL_READ` permanently and relying on a manual note on LOO-877.

1. **`scripts/check_migrations.py`:**
   - Parse `-- keeping: <table>[.<column>] <Task ID>` with the same grammar as `-- leaving:`.
   - Add one shared chronological resolver `active_leaving_marks(catalog)` that replays `catalog.ordered()` in order: `-- leaving:` sets `active[member] = (migration_name, mark)` and `-- keeping:` removes `member`.
   - In `_check_staging`, require every `-- keeping:` mark to name a member that exists in `after` and is currently in `active` before that migration.
   - In `_check_contract`, if `expand_file` originally carried `leaving_marks`, require every dropped item to be covered by the marks from `expand_file` that are still active in `active_leaving_marks(catalog)` (so retracting one or all marks blocks `DROP TABLE task_orders` in a contract of `task_facts_once`).
2. **`scripts/due_removals.py` & `scripts/schema_page.py`:**
   - Both read `check_migrations.active_leaving_marks(catalog)` instead of raw `leaving_marks(...)`, so kept members disappear from `due_removals.py` and render as live tables in `SCHEMA.md`.
3. **`rust/loopflow-storage/src/leaving_guard.rs`:**
   - Replay `-- leaving:` (insert) and `-- keeping:` (remove) in `migration_sqls()` catalog order, and delete `("task_orders", "LOO-1063")` from `STILL_READ`.
4. **Forward migration & generated schema:**
   - Run `uv run python scripts/new_migration.py keep_task_orders` and add `-- keeping: task_orders LOO-1063` with a short comment explaining that Machine-local Working order stays in `task_orders` apart from Project/backlog `tasks.planning_rank`.
   - Regenerate `rust/loopflow-storage/SCHEMA.md` via `uv run python scripts/schema_page.py` so `task_orders` appears as a live table.
5. **Architecture & migration docs:**
   - Update `docs/dev/reference.md` (Desktop workspace Persistence cell) and `docs/dev/reference-code.md` (Desktop arrangement row) to record `task_orders` as Machine-local Working Task order apart from `tasks.planning_rank`.
   - Document `-- keeping: <table>[.<column>] <Task ID>` briefly under `### Leaving marks` in `rust/loopflow-storage/src/MIGRATIONS.md`.
6. **Tests & checks:**
   - `python/tests/test_check_migrations.py`: cover `-- keeping:` validation (unknown member, un-marked member), blocking a contract from dropping a kept member (both partial and full retraction), and re-leaving a previously kept member with a fresh clock.
   - `python/tests/test_due_removals.py`: cover a kept member disappearing from `removals(...)` while remaining marks on the same Task stay scheduled.
   - `rust/loopflow-storage/src/leaving_guard.rs`: verify `task_orders` is readable/writable without a `STILL_READ` entry while genuinely leaving members remain denied.
   - Run `uv run pytest python/tests/test_check_migrations.py python/tests/test_due_removals.py`, `uv run python scripts/schema_page.py --check`, `uv run python scripts/check_architecture.py`, `uv run python scripts/due_removals.py`, and focused `leaving_guard` tests (with `LF_*` unset and `< /dev/null`). Note: `scripts/check_migrations.py` against `origin/main` also expects `planning_scope_checks.sql` once synced at gate.

Then gate (`documented_commands::` for `swift/README.md` and `server_operations_match_their_lf_commands` check on main).

## Built

- Working Tasks with no saved position appear first, newest started first. The
  first claim survives handoffs. Backlog keeps its planning rank. The repository,
  Wave, roadmap, CLI, Working Tasks grid, and iOS use the shared Working order.
  `#1973`'s uncalled `SqliteStore::reorder_task` and `planning_order::move_in`
  were deleted (`aef236355e`); Project rank edits go through `edit_task`.
- The Mac toggle starts Off and is shared by the repository and Wave sidebars
  for that repository in the window. The header and ⌘K group Blocked, Error,
  Available, Idle, and Busy. The open Task keeps its group through activity
  updates and repeated visits to its row; `WorkNavigation` owns release on any
  selection switch, filter change, or non-Working scope, and `applyTaskList`
  captures the pre-turn group before replacing readings (`10df280f65`). Rows in
  each group follow shared order. Saved Dashboards use their arrangement. The
  one-shot Machine sort was removed.
- The repository sidebar with sorting Off and the Working Tasks grid save drags.
  Concurrent drops keep their chosen side, and a refused drop restores the last
  Machine order and names the absent Task when a target or source is missing.
- A live interactive Session’s finished turn reads Available on the Task row
  and Finished in the terminal. Busy takes precedence over Available for a row
  that matches both filters. Input clears finished and error reports in both
  Ghostty and held terminals.
- Mac and iOS show the Task’s Graph cursor when the Machine sends a step. New Task
  fonts ship in Dev, QA, and Release bundles with a same-size system fallback.

User docs are in [surfaces.md](../docs/surfaces.md), [work.md](../docs/work.md),
and the [Swift README](../swift/README.md). Deferred ideas stay in
[questions.md](questions.md).

## Observed

- After compression, the repository selection SQL returned 2,000 rows from a
  10,013-Task fixture. `EXPLAIN QUERY PLAN` used `idx_tasks_repository` and
  the open-Graph index, with no temporary sort.
- `scripts/test_desktop.sh --skip-build --filter 'WorkNavigationTests|TaskConceptTests'`
  passed 63 tests after rebuilding Desktop. Unlocked-window QA remains below.
- 29 focused Rust tests passed across storage lifecycle, selection reads,
  roadmap membership and concurrent drops. `cargo fmt --all --check` and
  `cargo clippy --all-targets -- -D warnings` passed.
- `scripts/test_desktop.sh --filter 'WorkNavigationTests'` passed all 56 tests
  after the group hold and Wave toggle landed.
- [Private Machine QA round 5](qa.html) verified live OSC 7501 transitions,
  restoration of shared order after toggling Off, and the iOS feed. The
  installed `250e266b4` server served rows without `graph_step` and refused
  drops with the newer `side` field; stored order survived the refused drop.
- For 300 IDs, the synthetic store (10,000 Tasks, 406,666 events) took 0.78 ms
  for start times and 0.33 ms for Graph steps. The 1,553-Task Machine store
  took 0.56 ms and 2.68 ms. The median lookup for 300 Sessions’ availability
  was 0.72 ms.
- `EXPLAIN QUERY PLAN` confirmed indexed lookups with 10,011 Tasks, 1,000
  Graphs, and 103 saved positions.
- [QA round 6](qa.html) at `f2ffa0863` passed the group checks behind an
  in-use display.
- [QA round 7](qa.html) at `aef236355e` ran the merged build on the private
  Machine: Working drops, Project rank edits, arrivals and the status toggle
  in the repository and Wave sidebars behaved as built. No defects.
- After the `#1973` merge (`8742243abf`), `leaving_guard`, the 25
  `store::selections::tests`, and
  `task_concepts_share_lifecycle_activity_membership_and_order` passed.

## Remaining verification

In an unlocked key window on an idle display: hover the toggle in both
states, drag on the Working Tasks grid, and use ⌘1–9, ⌃Tab and ⌘K. Use
[Desktop QA](../.lf/qa/desktop.md) for the private Machine setup. Earlier
locked-screen workspace restore and Wave-to-grid findings are in [questions.md](questions.md).

At gate, also run `cargo test -p loopflow --test served_reads server_operations_match_their_lf_commands`
on main. The post-sync run failed at `config scope --json` because the server
reported `/private/var/folders` and the CLI reported `/var/folders`. The operation
and test arrived with `250e266b4`.

Run Cargo with the Task's `LF_*` variables cleared and stdin closed.

```sh
cargo test -p loopflow --lib task_concepts_share_lifecycle_activity_membership_and_order
cargo test -p loopflow-execution --lib one_task_order_reorders_working_tasks_and_every_plan_and_arrivals_prepend
cargo test -p loopflow --test served_reads working_task_drop_keeps_its_side_after_another_window_reorders
scripts/test_desktop.sh --filter 'TaskConceptTests|TaskStatusTests|PendingChangesTests|WorkNavigationTests|DashboardWorkspaceTests'
uv run python scripts/test_ios.py -only-testing:LoopflowiOSTests/TaskFeedTests
```
