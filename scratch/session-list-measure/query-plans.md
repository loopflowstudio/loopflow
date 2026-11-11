# Executed SQL and query plans

All 119 SQL shapes, query plans, execution counts, inclusive profile times,
fullscan and VM steps, sorts and supplemental-plan notes are retained in
[queries.json](queries.json), keyed by Q001–Q119. The former generated Markdown
was verified byte-for-byte against these records before removing its duplicate
SQL and tables on 2026-11-11.

Captured with SQLite 3.53.2 and NULL placeholder bindings; no bound values or
result content. Inclusive profile time includes nested reads and profiler
overhead, and is not wall time. Baseline interpretation and measurement limits:
[optimization plan](../optimize-session-list.md).

2026-11-11 flag filters: SQLite 3.50.4, in-memory copy of the retained schema
with 10,000 synthetic Tasks, 100 Projects and 1,000 Sessions. `EXPLAIN QUERY PLAN`
uses `idx_projects_wave_updated` → `idx_tasks_project` for Wave Task IDs;
`session_inventory` for primary selection and `tasks_primary_session` for
Task-primary probes (including bare primary selection). No Task table scan.

2026-11-11 `--headless`/completed filters: SQLite 3.50.4, memory copy of the
retained after.db with synthetic Tasks bringing it to 10,000 (14,804 Sessions).
The retained summary query with the new predicates and no OFFSET selects
`session_inventory (interactive, completed_at)` by default and scans the partial
`open_sessions` index with headless included. Both retain `tasks_checkout_root`
and `tasks_primary_session` probes. No Task table scan.
