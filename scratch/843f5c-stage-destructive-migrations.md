# 843f5c: stage destructive migrations, show what every install breaks

Jack Heart's Task brief (2026-11-05), re-scoped by his steer the same day after
the upgrade audit (`loopflow.codex-resume/scratch/upgrade-audit-2026-11-05.md`,
sections 4(b) and 6). Status: accepted, implemented, including both concept
reviews' refinements (below: views, the unreachable unique index, the clone's
reason); compressed 2026-11-05, gate next. The reader floor is deleted by a separate Task; nothing here touches floor
code.

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
- `install.rs`'s `Promotion` wrapper (the app target is read before the call) and
  `check_migrations.py`'s loose `_ordered`/`_migration_file`/`_schema_before`/
  `_empty_store`, now `Catalog` methods; a binary with no process cannot come out
  of `rollover::inventory()`, so settlement asks only whether a superseded binary
  exists (done).

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
Second compress (2026-11-05, `Catalog` + `Promotion` removal): the same lib filter
48 passed, pytest 19 passed, clippy and fmt clean, `lf context` memory at
16000/16000 tokens and scratch under budget.
Second-pass items (2026-11-05, `UniqueIndex.admits_older_rows`, comment-only
expand note, clone wording): `uv run pytest python/tests/test_check_migrations.py`
21 passed; `settings_authority` replayed over its predecessor with the check's own
functions: 17 drops, no break; `cargo test -p loopflow --lib -- lf::commands::rollover::`
4 passed; clippy and fmt clean; `lf context` scratch under budget.
Third compress (2026-11-05, `admits_older_rows(added)` counts the default row
through the index's `WHERE`; `report_after_promote` returns the clone reason
from its match): pytest 21 passed, the lib filter 48 passed, clippy and fmt clean,
`lf context` memory 16000/16000 tokens and scratch under budget.

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

## Concept review, second pass (2026-11-05, headless)

Checked against the implemented change: no floor code is touched
(`migrations.rs` changes only the backup helpers); install prints the advance
and `report_before_promote` before `prepare_artifacts`, settles inside the
promote, and prints the summary, the previous-lf report and any surviving clone
after skill sync; `observe_process` runs after the install command returns; CI
fetches full history (`fetch-depth: 0`, `origin main`) so the first-parent dates
the check reads exist, and `Catalog.ordered()` orders unmerged slugs the way
`order_catalog` does (merged by history, then by name). Probing the check with
its own functions against `origin/main`:

- `settings_authority` replayed over the schema before it: 17 drops, and one
  break: "unique index idx_machines_local on machines; an older writer may
  violate it". That index covers only `is_local`, a column the same migration
  adds with `DEFAULT 0`, under `WHERE is_local=1`. An older writer never names
  the column, so every row it writes carries the default and sits outside the
  index; nothing it does can violate it. The rule is "older SQL still works", and
  this SQL does. Reported as a break, the author is told to stage the index for
  30 days with no benefit, on exactly the shape the incident migration used. The
  counterexample that keeps the break honest: the same index without the
  `WHERE`, or with `WHERE is_local IS NOT NULL`, is violated by the second older
  INSERT, because both rows carry the default.
- A comments-only slug over the full frontier: no drops, no breaks. It passes
  the check, and `order_catalog`/`build_catalog` see an unchanged schema.

Settled by the accepted rule and implemented the same day:

1. **A unique index an older writer cannot reach is additive.** In `_compare`,
   a new unique index on an old table stays a break unless every column it
   indexes (`PRAGMA index_info`) is new in `after` and its partial predicate is
   false for a row of those columns' defaults: evaluate the index SQL's `WHERE`
   text with the new columns bound to their `dflt_value` literals (a one-row
   CTE), and treat a predicate that names an old column, has no `WHERE`, or
   evaluates true as the existing break. Test: the `is_local` shape passes; the
   same index without `WHERE` fails. MIGRATIONS.md's "no unique index joins an
   existing table" becomes "no unique index an older writer can violate".
2. **An expand step with no schema change is a migration whose SQL is only its
   comment.** `settings_authority` moved settings out of the store; the new shape
   was files and code, so there was no SQL to expand with. Jack's rule names an
   expand slug, so that slug is a comment saying what the store stopped being the
   authority for; it merges, starts the clock, and the ledger records it. Say so
   in MIGRATIONS.md's staging section, beside the `-- expand:` example. The
   alternative, letting `-- expand:` name a commit or PR, adds a second marker
   form for one case and is not taken.
3. **The clone's reason, stated right.** MIGRATIONS.md says the older-build
   process "reads the generation the clone holds"; it reads the live, additively
   extended store. The clone is the restore point for the generation that build
   was written against, kept while any such process can still show that the
   migration lost something. `report_after_promote`'s "kept while they run" line
   says the same; when the inventory could not be read it should say the clone
   is kept because live processes are unknown.

No open product choice. The `STAGING_DAYS` hard fail and `-- exports:` as a
declaration stay as recorded in `scratch/questions.md`; Jack's steer said "at
least 30 days", which the check enforces.

## Remaining

- Gate runs the affected suites. The install integration suites
  (`local_promotion`, `one_machine_tests`) were not run here.
- Demo: `lf install` on a Machine with live Sessions shows the inventory line
  before the switch and the result last; a stale-build process keeps the clone.
