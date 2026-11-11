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
