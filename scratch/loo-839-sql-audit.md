# LOO-839 SQL audit

Docs: `docs/dev/data.md` § "Query cost and the write lock".

Out of scope (LOO-859): `provider_transitions::latest_in` per Session in
`session_summaries_in`; `export_peer_planning` holding the write lock during
`wave.sync`. Per-request store open is LOO-843.

## Measured (2026-11-12, live store 773 MB, schema = branch frontier)

Copies: `/tmp/lf839/plan.db` (ANALYZEd), `raw.db` (stats dropped, as the live
store runs). Rows: session_events 266k, processes 167k, task_events 73k,
session_steers 43k, planning_peer_changes 36k, agent_sessions 16.5k, tasks 1.2k.

- Live store had **no `sqlite_stat1`**. Every write connection begins
  IMMEDIATE, so any `conn.transaction()` takes the write lock, even read-only.
- Sweep: 2194 SQL literals extracted statically, plus 2952 distinct statements
  traced from lib suites (store/work/server/desktop/journal/session/process);
  113 traced statements scanned a growing table; ranked by time on `raw.db`.
- Statistics (steer, measured on the copy): full ANALYZE 1.8 s;
  `PRAGMA analysis_limit=400; PRAGMA optimize=0x10002` 0.8 s; both hold the
  write lock; 154 stat rows written.

lf.log write holds ≥3 s (count / total s / max s): export_peer_planning 15/93.9/20.3
(LOO-859), import_peer_planning 8/33.6/5.5, record_owner_viewer_hold 6/23.9/6.5,
create_task 6/22.2/4.3, reorder_task 2/20.8/13.0, append_task_comment 1/7.2,
place_task 1/6.1, ensure_primary_session 1/4.5.

## Fixed (before → after, on raw.db)

| what | before | after |
|---|---|---|
| `pending_retirements`: Tasks × Sessions OR-join → live hosts + `session_tasks` probe | 458 ms | 0.5 ms |
| planning clock `max(clock)` (×~10 per `create_task`, under lock) + index | 27 ms | <0.1 ms |
| `held_terminal_processes` / owner retention + partial index | 33 ms | <0.1 ms |
| Session-event delete FK check on `processes.attachment_exit_seq` + index (per removed event in retention compaction, under lock) | 3.4 ms | 0.1 ms |
| `task_of_process`: `INDEXED BY` named the wrong index | 4.5 ms | 1.6 ms |
| `short_task_id`: `substr(lower(id))` scan → primary-key range | scan | SEARCH |

No write lock when nothing changes: `record_owner_viewer_hold` (every viewer
poll), `import_peer_planning` (revision already imported, no active conflicts;
every agent acquired each second and bumped the planning revision),
`server::wake` pending check (`has_undelivered_steers` instead of
`session_steers`), `begin_linear_team_check`, `ensure_primary_session` (reads
the primary first). Read-only snapshots moved to savepoints:
`observe_task_location`, `ci_incidents_since`. `ensure_primary_session` /
`create_session` discover the repository (git) before BEGIN.

Migration `query_cost_indexes.sql`: planning_peer_changes_clock,
processes_held_terminal, task_retirements_unfinished, tasks_abandoned_checkout,
processes_attachment_exit. Server: `server::statistics` (start + ~24 h,
SystemTime). Guard: `store::query_plans` (boundary test now uses it; widening
its tables to `processes` found the FK scan above).

## Fixed in the second pass (before → after, raw.db copy)

| what | before | after |
|---|---|---|
| claim/handoff/unassign/place: Git email + name + `planning_client::connected` under the lock (`RepoFacts` read before BEGIN) | ~43 ms warm, 190 ms cold | ~1.5 ms (placement + declarations remain) |
| `reorder_task`, `save_dashboard`: Work read in the write transaction | `work_in` 180–220 ms warm (13 s hold observed) | revision/order check + write; Work read in a snapshot |
| `import_peer_planning` moved revision: merge/write only fields with unrouted changes; repository registration before BEGIN; `ensure_record` no longer runs Git locator IO for a registered repository | whole history re-applied | 124 ms whole import with nothing unseen, 327 ms with 200, 429 ms with 2,000 |
| `add_machine`, `upsert_provider_account`: YAML written before the row, no transaction | YAML under lock | none |
| `session_steers` / `session_steer_source`: read-only check, write lock only when reconcile changes something; `record_session_event` duplicate check before IMMEDIATE | IMMEDIATE every read | snapshot read |
| `unrecorded_agent_ancestry` + `processes_unrecorded_ancestry` | 190 ms | <0.1 ms |
| `sessions_for_agent_session` + `agent_sessions_provider_thread`, `session_events_provider_session` | 58 ms | <0.1 ms |
| `resolve_history_input`: prefix ranges + `session_capture_caller` | 28 ms | <0.1 ms |
| `pending_inputs`: window pushed into both branches (1-day window) | 270 ms | 240 ms |

FK-child indexes: session_events_task, session_events_wave, session_steers_steer,
planning_peer_routes_mutation. `store::query_plans` covers Session lookups and
steer reads, plus a test that an unchanged steer read does not wait on another
writer.

## Filed as linked Tasks

- LOO-865 `task_graph_pairs` walks every Graph: 40–65 ms per projection, 37–53 ms for one Task. Task-driven SQL: 0.1 ms one Task, 7.7 s all Tasks.
- LOO-866 `pending_inputs` still walks every capture (240 ms).
- LOO-867 placement reads under the ownership lock (~1.5 ms); `connected` (~10 ms) in `reassert_in`/`hand_back_in`.
- LOO-868 `export_in` over the whole planning history under the lock on a moved revision.
- LOO-869 `work_in` runs 3 queries per Task (180–220 ms for 750 Tasks) on every selection read.
- LOO-870 `query_plans` exempts partial-index walks and cannot see FK checks.
- LOO-872 test that store paths take no write lock when they change nothing (the guard below).

## Write-lock guard, filed as LOO-872 (concept review, 2026-11-12)

The design has two rules and one guard. Scans have `store::query_plans`;
"open no write transaction when nothing changes" has none, and every write
connection begins IMMEDIATE, so a read-only `conn.transaction()` takes the lock
silently. `contention::watch_write_holds` reports only holds past 3 s; a fast
empty write is invisible, though it queues behind every writer and makes each
writer behind it wait. The fixes in both passes were found by reading code.

Direction for LOO-872:

- `query_plans` gains a sibling to `statements`: run a closure and return the
  call sites of write transactions that took the lock and changed no row
  (the update hook already marks a change; the commit and rollback hooks see
  the end). Test-only, beside the scan helper, same fixture.
- Run it over the paths `query_plans` already drives (boundary, retirement,
  viewer hold, steer reads, planning import with nothing unseen) asserting
  none, and over `lf task status`/Work reads if a fixture reaches them.
- Fix what it finds the way this branch did (read first, or a savepoint); a
  find that needs a rewrite goes to a linked Task like LOO-865..870.
- `docs/dev/data.md` § "Query cost and the write lock" names the check next to
  the scan guard.

Not production logging: an empty write is cheap alone and only costs under
contention, so a log line per one would be noise; the test catches the spelling.

## Checks

- Touched modules (durable, sessions, selections, task_ownership, planning_peers, planning_write, repositories, children, task_work, provider_account, machine): 222 pass; steers, session_events, query_plans pass.
- `steers::tests::only_abandoned_prewrite_reservations_return_to_pending` failed once (steers_tests.rs:388) in a 5-module parallel run; passed 3/3 alone and in the steers module run. Gate reruns it.
- `cargo clippy --all-targets -- -D warnings` clean.
- Compress (shared `task_repo_in`, `seed_task` without `expect`, direct peer-import record set): task_ownership, planning_peers, children, query_plans 27 pass; clippy clean.
- Gate: `binary(two_people)`, `binary(transport_tests)` (store-wide schema), migration upgrade fixtures.
