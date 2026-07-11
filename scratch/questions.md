# Open questions — product branch

## TODO: run_events.context lifecycle collides across branches (low priority)

`run_events.context` was added by editing the historical `057` CREATE in place
(main already carries it), so DBs created before that edit never get the column
and `057` won't re-run. On **main/product**, `validate_run_events_schema` selects
`context`, so those pre-context DBs fail on open — taking down every command that
shares `lfd.db` (that's the `pm show` break we hit; worked around by hand-adding
the column to the local DB).

Does the intelligence branch handle it? **By direction, yes; for existing DBs,
not cleanly:**

- Intelligence's `validate_run_events_schema` no longer selects `context`, and
  `061_trace_capture` relocates that data — so once intelligence lands, nothing
  requires `run_events.context` and the open-failure dissolves.
- But `061_trace_capture` runs an **unguarded** `ALTER TABLE run_events DROP
  COLUMN context`. On a pre-context DB the column isn't there, the DROP fails
  with `no such column: context`, and `061_trace_capture` is NOT in
  `RENAME_CONVERGENCE_MIGRATIONS` / `is_tolerated_migration_error` — so the whole
  migration crashes. It trades product's "select fails" for its own "drop fails"
  on the same DBs.

Fix belongs to **intelligence, one line**: tolerate `no such column` for
`061_trace_capture` (add it to the convergence path) or make the drop a
table-rebuild. Product should NOT add a forward `ADD COLUMN context` — it would
fight the drop.

Also: both branches minted migration **061** (`061_pm_snapshots` vs
`061_trace_capture`). Distinct version strings so both apply, but the number
overlaps and inter-order is undefined — wants a coordination convention
(per-wave ranges, or a lint).

Idea (Jack): give dev runs a separate lfdb so in-flight schema can't corrupt the
real ledger. `lf_home_dir()` already honors `LF_HOME` (lfd/mod.rs:66), so
`scripts/dev-lf` could default `LF_HOME=~/.lf-dev`. Cheap; doesn't solve the
migration-number collision.
