# 843f5c: stage destructive migrations, show what every install breaks

Jack Heart's Task brief (2026-11-05), re-scoped by his steer the same day after
the upgrade audit (`loopflow.codex-resume/scratch/upgrade-audit-2026-11-05.md`,
sections 4(b) and 6). Status: accepted, implemented, including the concept
review's view refinement (below); gate next. The reader floor is deleted by a
separate Task; nothing here touches floor code.

## Outcomes

1. **Destructive migrations are staged.** Jack's 2026-10-27 rule: expand, wait
   for older lf to exit, export, drop. MIGRATIONS.md and AGENTS.md carry it;
   `check_migrations.py` applies the branch's slugs over `origin/main`'s schema in
   `build.rs` order and compares shapes. A slug is additive (tables and views
   keep their columns, new columns optional, no unique index on an old table;
   CHECK-only rebuilds, view redefinitions, trigger and index changes and data
   writes pass)
   or a contract: `-- expand: <migration>` on `origin/main` for `STAGING_DAYS`
   (30, one place) by first-parent add date, `-- exports: <where>`, dropping only
   what the expand step superseded. CI measurement only.
2. **Every `lf install` says what runs before it changes anything.** The candidate
   prints the store advance and `rollover::inventory()` (the list `lf doctor`
   reads), then proceeds. Skill sync and link registration run next; the promoted
   line, the previous-lf report and any kept clone print last. The report now says
   a Session comes back through `lf resume` once its client is stopped and the
   old lf has exited.
3. **Install records itself as a Process after the commit.** `lf self install`
   still dispatches before journal emission (#1074); it calls `observe_process`
   after the command returns, when the store has settled. A failed observation
   logs at `warn`, so a missing row names itself in `lf.log`.
4. **An install keeps its pre-migration clone while an older build runs.**
   `settle_migration_backups` prunes only when the inventory shows no superseded
   process; MIGRATIONS.md's backup section now matches the code and the file
   name `loopflow.db.backup-<version>-<16hex>` beside the store.

## Evidence recorded

- 10-30: 0.15.0 predates slug ids (#1755); `lf.log` 2026-10-30T12:34:50Z shows
  `lf=0.15.0 rev=acd37dde7` failing on `ship_stop`'s ledger row. Closed by Jack.
- 19:57 `lf install` has no Process row (ledger empty 03:50Z to 04:10Z for it);
  design since #1074 plus a debug-only observation failure. The 09:15 row's origin
  stays unknown (`scratch/questions.md`).
- The disk cleanup (`scripts/disk_cleanup.py`) removes nothing under `~/.lf`;
  `docs/troubleshooting.md` and `docs/architecture-reference.md` claimed a week's
  retention of captures, logs and all but the newest backup and are corrected.
  Nothing in lf writes `~/.lf/backups`; MIGRATIONS.md names the clone's real
  location and no longer describes that directory.
- `rust/loopflow/src/store/MIGRATIONS.md` keeps its reader-floor section
  ("Older lf keeps opening an additively extended store") for the floor Task;
  it sits beside the staging section until that Task removes it.
- Wave memory (LOO-656 section) now carries the staging rule, the 2026-11-04
  cause and what `lf install` shows; "Review replacement (2026-10-04)" retired to
  `3b07e13404` to stay under the memory budget.

## Delete — do not maintain

- `report_after_promote`'s "`lf resume` reopens it" sentence (replaced).
- Startup `observe_process` for install commands (moved after the command).
- MIGRATIONS.md "Rename, drop and reshape directly" and "two newest backups"
  (replaced).
- `prune_migration_backups(store, 0)` at settlement without checking processes.
- `check_migrations.py`'s second first-parent walk (`_first_parent_added`) and the
  re-parsing of drop strings into kind/table/column (done).

## Checks

`cargo test -p loopflow --lib -- lf::commands::install:: lf::commands::rollover:: backup`
48 passed; `cargo test --bin lf only_changes_to_work` passed; `cargo clippy
--all-targets -D warnings` clean; `cargo fmt --check` clean. After compress (one
first-parent `git log` gives slug order and add dates; drops are checked against
`Schema.members()`; `_check_contract` split out): `uv run pytest
python/tests/test_check_migrations.py` 19 passed (views, 2026-11-05); `check_migrations.py` on this
tree: no unmerged migration, and a probe contract against real history reported
`ship_stop` merged 6 days ago, clears 2026-11-29. Realign (2026-11-05): doc-only
corrections, no code change; `lf context` shows memory and scratch under budget.

## Concept review (2026-11-05, headless)

What a person meets: `lf install` prints the live processes and the store
advance, proceeds, and ends with the promoted line, the previous-lf report and
any kept clone, so `tail` reads the result. A migration author meets one rule in
AGENTS.md and MIGRATIONS.md with two markers, and CI names the day a contract
clears. The concepts hold: additive-or-contract, expand step, export, clone kept
while an older build runs, install as a Process. Nothing here needs a person's
decision; the floor's absence is the floor Task's.

One mismatch the accepted rule already settles. The rule is "older SQL still
works": a `waves` rebuild that keeps every column is additive even though its
`CREATE TABLE` text changed. `_compare` applies that to tables (column maps from
`PRAGMA table_xinfo`) but holds views to whitespace-normalized definition text, so
redefining `wave_addresses` to add a column fails as "not additive" with advice to
stage it, which for a view means a second name and a 30-day drop of the first: an
action the rule imposes without benefit. The text check also misses the opposite
case: `wave_removal` recreates `wave_addresses` byte-identical (`SELECT w.*`), so
a rebuild that dropped a `waves` column would pass the view while its columns
shrank. Views are read like tables; measure them like tables.

Implemented the same day: `Schema.views` is `dict[str, dict[str, Column]]` read
through the same `PRAGMA table_xinfo`; `_compare` treats a view like a table (a
lost view column is a drop a contract may stage, `members()` includes
`column <view>.<col>`, an unchanged column set passes whatever the definition
says); the view-changed break is gone. MIGRATIONS.md says "every view keeps its
columns". Tests: a redefined view that adds a column passes; one that loses a
column needs a contract.

Recorded, not changed: the kept clone's test is "any superseded process", so an
install with no migration also keeps the clone while a process on the prior build
runs, though that build reads the current generation. Without the floor the
inventory has no per-build generation to compare, and the clone's cost is disk
until the next commit; a finer test is the floor Task's to want.

## Remaining

- Gate runs the affected suites. The install integration suites
  (`local_promotion`, `one_machine_tests`) were not run here.
- Demo: `lf install` on a Machine with live Sessions shows the inventory line
  before the switch and the result last; a stale-build process keeps the clone.
