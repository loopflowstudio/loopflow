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

## Remaining (in the brief's scope, not yet fixed)

- `task_ownership` `claim_task`/`handoff_task`/`unassign_task`: `git config`
  spawned under the write lock via `my_key_in`/`person()`; resolve identity
  before BEGIN and pass it through the `_in` helpers.
- `reorder_task`, `save_dashboard`: whole-repository Work read plus 3 queries
  per Task inside the write transaction (13 s hold observed).
- `import_peer_planning` on a moved revision still re-imports the whole history
  and runs `ensure_repository_in` (file IO, sometimes git) per Wave under lock.
- `place_task`/`create_task`: config file reads (`planning_client::connected`,
  `global_settings`) under lock.
- `durable::add_machine`, `upsert_provider_account`: YAML writes under lock.
- `session_steers`/`session_steer_source` still reconcile under the write lock
  (CLI reads); `record_session_event` duplicate check inside IMMEDIATE.
- Reads: `pending_inputs` (Session history) 241 ms; `unrecorded_agent_ancestry`
  192 ms (doctor/prune); `sessions_for_agent_session` 59 ms (json_extract scan);
  `task_graph_pairs` 40 ms per Work projection; `resolve_history_input` 28 ms.
- FK children without an index whose parents are rarely deleted:
  `session_events.task_id/wave_id`, `session_steers.steer_id`,
  `planning_peer_routes.mutation_id`.

## Checks

- `cargo test -p loopflow --lib store::query_plans:: server::boundary:: server::statistics::` — pass (implement).
- Touched modules (task_retirement, steers, wake, planning_peers, linear_team_checks, sessions, ci_incidents, repositories, children, task_work, durable, graph_inventory, task_ownership, ops::task::retirement, ops::planning_sync): 118 pass.
- Compress (`linear_team_checks` helper, stale doc comments): `cargo test -p loopflow --lib -- store::query_plans:: server::boundary:: linear_team_checks sqlite::sessions::` — 26 pass.
- `cargo clippy --all-targets -- -D warnings` clean; `documented_commands::` pass.
- Gate: `binary(two_people)`, `binary(transport_tests)` (store-wide schema), migration upgrade fixtures.
