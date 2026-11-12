# LOO-859 plan

## Built

- Session summary transition lookup: migration `session_summary_indexes.sql`
  adds partial index `session_provider_transitions`; `provider_transitions.rs`
  queries by receipt-key prefix (`provider-transition:`, legacy `agent-session:`)
  instead of `json_extract` over the whole history. Every row of either type on
  this Machine's store carries one of those prefixes (2531 + 218 rows).
- `lf wave sync` / `wave.sync` on a Wave with no `linear.initiative` returns
  `PlanningRefresh::Unconnected`: no planning delivery, no Linear call, no write
  lock, success. Text mode prints the line in docs/work.md.
- `export_peer_planning`: reads in a deferred transaction, takes the write lock
  only to route owned changes that have no route yet, and decodes/validates the
  4.8 MB of mutations after the connection is released. Routes now commit even
  when validation fails (before, the rollback re-inserted 10,022 manabot routes
  on every sync).
- Planning frames: `identity` treats `acquisition.observed_at` and
  `project_readiness.observed_at` as reading time. `LinearError::RateLimited`
  text no longer embeds the reset epoch, so a retried delivery's saved error is
  stable.

## Measured (2026-11-12, snapshot of ~/.lf/loopflow.db, 16,488 Sessions / 266,057 events)

- Transition lookup across all Sessions: 0.323 s → 0.008 s; plan
  `SEARCH session_events USING INDEX session_provider_transitions (session_id=?)`.
- Peer export SQL for manabot: routing insert 0.19 s (10,022 rows, every sync,
  rolled back), export read 0.09 s; new unrouted check 0.006 s.
- Live `lf plan list --all --json --subscribe` for 45 s: 6 frames of 1.93 MB;
  every diff was `project_readiness.observed_at`, `acquisition.observed_at` or
  one Task's rate-limit error text (task_690c9bb3…, retry epoch changing).
- Live planning revision: +142 in 30 s. Changed rows: Linear refresh restamps
  `pm_items.observed_at`, `pm_projects.observed_at`, `tasks/projects.pm_snapshot_synced_at`;
  delivery retries rewrite `task_changes`/`task_state_deliveries` errors;
  `ci_incidents.updated_at`, `pr_landings.generation`.

## Remaining

- After `lf install`: server CPU with loopflow + manabot open, `held the store
  write lock` lines per wave.sync in lf.log, Session list read time. (verify/demo)
- Config reparse per request (`settings::validate_value` in samples) not
  addressed: config reads go through `observe` for subscription dependencies,
  so a cache must still record them. Candidate follow-up Task.
- Linear refresh restamping observation columns still bumps the planning
  revision and rebuilds readers (frames are no longer sent for it).

## Checks

- `cargo test -p loopflow --lib -- store::sqlite::planning_peers:: store::sqlite::planning_write:: lf::commands::subscribe::tests:: store::sqlite::provider_transitions::` — 26 passed
- `cargo test -p loopflow --lib wave_sync_without_an_initiative` — passed
- `cargo clippy --all-targets -- -D warnings` — clean
- compress (doc comment fix, `has_unrouted_changes` helper): `cargo test -p loopflow --lib -- store::sqlite::planning_peers:: wave_sync_without_an_initiative` — 5 passed; clippy clean
- gate: `documented_commands::`, `lf::tests::`, `wave_resolution_matrix::`, `local_planning::`, `binary(two_people)`, `binary(transport_tests)` (store trigger/schema)
