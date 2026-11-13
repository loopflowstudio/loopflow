# LOO-870: query-plan guard sees large partial-index walks and FK child lookups

## Problem

`store::query_plans::growing_scans` (rust/loopflow/src/store/query_plans.rs)
exempts every walk through a partial index, on the theory that a partial index
holds only a live subset. Some partial indexes cover most of a growing table:
`session_observation_receipt` (168k of 266k Session events) and
`session_capture_key` (16.6k) hid the `json_extract` scans LOO-839 fixed
(59 ms and 28 ms). `EXPLAIN QUERY PLAN` also never shows the child lookups SQLite
runs when a parent row is deleted or its key updated, so a missing FK-child index
is invisible to the guard; LOO-839 found one (`processes.attachment_exit_seq`,
3.4 ms per removed event during retention compaction) only by widening the table
list.

## Demo

`cargo test -p loopflow --lib store::query_plans::` fails, naming the index and
its measured size, when a statement walks a large partial index:

```
analyzed: SCAN session_events USING INDEX session_observation_receipt (168288 rows measured)
  in SELECT ... WHERE kind='observed' AND json_extract(payload,'$.evidence.provider_session_id')=?1
```

and fails, naming the foreign key, when deleting a parent would scan a growing
child table. With the indexes this branch adds, both pass.

## Approach

**Partial-index walks.** A `SCAN <t> USING [COVERING] INDEX <partial>` of a
growing table passes only when the index's measured row count (first number of
its `sqlite_stat1` row in `MEASURED`) is under 1,000. Live subsets are bounded
by concurrency (hundreds of agents): `open_sessions` 620, `processes_unfinished`
741, `agent_process_unfinished` 68. History-shaped ones are not:
`session_capture_key` 16,643, `session_observation_receipt` 168,288. `SEARCH`
through any index stays fine. The row count is a property of the data, so both
passes (unanalyzed and measured) judge a walk by the measured size: collect plan
details from both passes first, apply `MEASURED`, then read `sqlite_stat1` and
filter.

**Every partial index on a growing table is measured.** Fixture `ANALYZE` counts
a handful of rows, which is how the hole opened. A test fails when a partial
index on a `GROWING` table has no row in `MEASURED`. Today that is
`processes_attachment_exit` (17,586 on the Machine), `steers_session`,
`steers_repo`, `steers_task` (2,913), `steers_wave`,
`task_retirements_unfinished`, `tasks_abandoned_checkout`. Regenerate `MEASURED`
from the Machine store for the tables it already lists plus `steers` and
`task_retirements`, and update its date comment. The old comment's reason for
leaving `steers` to the fixture (NULL-heavy owner columns) predates the partial
`steers_*` indexes; if real stats steer a steer read onto a scan, that is a
finding to fix, not a reason to keep fixture stats.

Read the probe read-only:
`sqlite3 "file:$HOME/.lf/loopflow.db?mode=ro" "SELECT tbl,idx,stat FROM sqlite_stat1 WHERE tbl IN (...)"`.

**FK child lookups.** New `pub(crate) fn foreign_key_lookups(store) -> Vec<String>`:
for every foreign key whose child table is in `GROWING`
(`pragma_foreign_key_list` over `sqlite_master`), the statement SQLite runs for
the child check, `SELECT 1 FROM <child> WHERE <c1>=? AND <c2>=?`. A test passes
them to `growing_scans`, so FK checks get the same two-pass planning and the
same partial-index rule with no second classifier. A partial index
`WHERE col IS NOT NULL` serves `col=?` (the planner proves the implication;
confirmed on the Machine store: `agent_sessions WHERE held_terminal_id=?`
searches `agent_sessions_held_terminal`). Every FK counts regardless of
`ON DELETE` action or whether the parent is deleted today: SQLite checks children
on any parent delete or key update, and "never deleted" is not something the
schema states.

Schema-wide, not per traced path: an index's existence is a schema property,
and the bug it catches lives in paths (retention, cleanup) nobody traces.

## Risk findings (Machine store, 2026-11-13, installed schema)

FK child lookups that plan as `SCAN` of a growing table:

| child column | parent | rows in child |
|---|---|---|
| `processes.agent_session_id` | `agent_sessions` | 166k (18,308 non-null, all `kind='agent'`) |
| `agent_sessions.wave_id` | `waves` | 16k |
| `steer_batches.agent_process_id` | `processes` | 11k (all non-null) |
| `steers.project_id` | `projects` | small today |
| `tasks.checkout_machine_id` | `machines` | 1.2k |
| `tasks.parent_pr_id` | `task_prs` | 1.2k |

`processes.agent_session_id` has `agent_process_session ... WHERE kind='agent'`,
which `agent_session_id=?` alone cannot use. Replace it with
`processes_agent_session ON processes(agent_session_id) WHERE agent_session_id IS NOT NULL`:
same rows on the Machine, and `kind='agent' AND agent_session_id=?` still
uses it. No `INDEXED BY` names it; a plan assertion in
`store/sqlite/task_work.rs` (test near line 1144) checks for
`agent_process_session` and takes the new name.

The branch schema may differ from the installed one; the new FK test is the
authoritative list.

Partial-index `SCAN`s the new rule will newly flag in existing tests are
unknown until it runs (`query_plans` tests, `server::boundary`). Fix each with an
index or query rewrite in this branch; a fix needing a redesign goes to a
linked Task, as LOO-839 did.

## Changes

1. `query_plans.rs`: partial-walk threshold, collect-then-filter across both
   passes, `foreign_key_lookups`, regenerated `MEASURED`. Failure text names
   the index's measured rows.
2. Migration (`uv run python scripts/new_migration.py foreign_key_children`):
   indexes for the FK children above (partial `IS NOT NULL` where the column is
   mostly NULL), the `agent_process_session` replacement, plus whatever the FK
   test finds on the branch schema.
3. Tests in `query_plans.rs`:
   - `a_walk_of_a_large_partial_index_is_a_scan`: the LOO-839
     `kind='observed' AND json_extract(...)` statement is flagged naming
     `session_observation_receipt`; `SELECT ... FROM processes WHERE completed_at IS NULL`
     (`processes_unfinished`) is not.
   - `every_partial_index_on_a_growing_table_is_measured`.
   - `foreign_key_children_on_growing_tables_are_indexed`:
     `growing_scans(&store, &foreign_key_lookups(&store))` is empty.
4. `docs/dev/data.md` § "Query cost and the write lock": replace "walking a
   partial index ... does not" with the measured-size rule; say FK child lookups
   are checked from the schema.

## Exclusions

- Per-path FK tracing (adding child lookups for each traced parent DELETE/UPDATE).
- Tables outside `GROWING`; adding a table to it is a separate finding.
- Write-lock guard (LOO-872).

## Done when

A statement that walks a history-sized partial index, or a parent delete that
would scan a growing child table, fails `store::query_plans` with the index or
foreign key named; the current schema and hot paths pass.

## Checks

- `cargo test -p loopflow --lib store::query_plans::` — implement, gate
- `cargo test -p loopflow --lib server::boundary::tests::boundary_statements_never_scan_tables_that_grow_with_use` — implement, gate
- migration checks via `uv run python scripts/test.py --reuse-passing` — gate
