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
- [`0bc19e976d` review]: 8 of 9 product-to-code pairings aligned; flagged `task_orders` still marked `-- leaving: task_orders LOO-877` across `leaving_guard.rs`, `due_removals.py`, `check_migrations.py`, `SCHEMA.md`, `reference.md`, and `reference-code.md`. Resolved by `89425281fa` (`keep_task_orders.sql` + `-- keeping:`), `4d4d6f7b3a`, and QA round 8 (`d15cc919ff`).
- [`d15cc919ff` review](concept-review.html) (against base `6601da8ef8` after `8c71894fe4` synced `origin/main`): 9 of 9 pairings align across Working membership, two-order runtime and schema lifecycle (`-- keeping: task_orders LOO-1063`), side-aware drags, non-destructive status toggle and Wave scope, single-owner `heldStatusGroup`, row status vocabulary, Graph step display, OSC 7501 `done`/`error` handling, and packaged fonts. Nothing to change.

## Next

Gate: `origin/main` is synced at `6601da8ef8` (`planning_scope_checks.sql` in
place). Run `documented_commands::` for the edited `docs/dev/*.md`,
`swift/README.md` and `MIGRATIONS.md`, and check
`server_operations_match_their_lf_commands` on main.

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
- `-- keeping: <table>[.<column>] <Task ID>` takes back a leaving mark in a
  later migration. `keep_task_orders.sql` keeps `task_orders` (LOO-1063):
  `check_migrations.py` (`active_leaving_marks`), `due_removals.py`,
  `schema_page.py` and `leaving_guard.rs` replay marks in apply order, so
  `task_orders` left `STILL_READ`, the LOO-877 list and the leaving column of
  `SCHEMA.md`, and no contract of `task_facts_once` may drop it.
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
- Keeping slice: 34 tests in `test_check_migrations.py` and
  `test_due_removals.py`, `leaving_guard::tests`, `schema_page.py --check`,
  `check_architecture.py` passed; `check_migrations.py` passed with
  `planning_scope_checks.sql` copied in from origin/main (`unmerged:
  keep_task_orders.sql`).
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
