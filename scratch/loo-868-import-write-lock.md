# LOO-868: hold the write lock only for the writes in a peer planning import

## Problem

`SqliteStore::import_peer_planning` (`rust/loopflow/src/store/sqlite/planning_peers.rs`)
runs on every planning acquire (`ops/planning_sync.rs::acquire_destination`) once
a destination's revision moves. Inside `BEGIN IMMEDIATE` it still:

1. `route_repository`: joins every owned record to `planning_peer_changes` and
   `INSERT OR IGNORE`s every owned change, even when all are routed.
2. `export_in(&tx, repo, destination, true)`: reads the repository's whole
   planning history (the `OR` predicate forces a full scan of
   `planning_peer_changes`), decodes it, and `merge` canonicalizes and validates
   both documents in full.
3. The record writes, observations, ownership markers, conflicts, import row.

Only 3 changes rows. Every other agent on the Machine waits behind 1 and 2.

## Measured (store copy, 2026-11-13, `~/.lf/loopflow.db`, 38,772 changes)

From LOO-839 (whole import): 124 ms with nothing unseen, 327 ms with 200
unrouted changes, 429 ms with 2,000. Importing a repository's own export with
every change unseen failed with `incomplete planning record`, so that path was
never timed.

This kickoff, sqlite3 on a fresh copy:

| query (held under the lock today) | loopflow (23,632 owned) | manabot (11,298 owned) |
|---|---|---|
| owned ⋈ changes join (what `route_repository` / `has_unrouted_changes` scan) | 15 ms | 10 ms |
| `export_rows` (full scan, 10.2 MB / 5.5 MB of mutation JSON) | 80 ms | 73 ms |
| `route_repository` insert, 8,659 rows newly routed | — | 39 ms |

The rest of the 124–429 ms is Rust: decoding 10 MB of JSON, canonicalizing and
validating the local export twice inside `merge`, the winners/heads passes.

## Status (2026-11-13, implement)

Built in `planning_peers.rs`: `Import::read` (`plan` in a deferred tx) and
`Import::write` (`BEGIN IMMEDIATE`, planning-revision recheck, route, apply).
Remaining: the competing-writer probe on a store copy (gate) and the PR-body number.

Departures from the plan below:

- **`conn.transaction()` is `IMMEDIATE` on this store** (`sqlite.rs` sets the
  default behavior). The plan's read tx uses `TransactionBehavior::Deferred`
  explicitly. `export_peer_planning`'s "read" tx from LOO-859 had the same
  trap and held the write lock across `export_rows`; it is `Deferred` now too.
- Ownership in `record_rows` is one `owner_query` lookup per record (shared with
  `require_repository`), bound as `?3`, rather than `EXISTS(owned)`.
- `unrouted_changes` drives `owned CROSS JOIN planning_peer_changes` so each
  owned record's history comes from `planning_peer_changes_object`; the CTE's
  `task_comments` alias is `m` so it no longer shares `c` with the changes table.
- `plan` reads `in_conflict` itself, so a recompute under the lock sees it current.
- `export_in(…, true)` keeps callers (`peer_planning_status`, `peer_planning_journal`
  with the active destination); only the import stopped using it.

### Delete — do not maintain

- `route_repository`, `has_unrouted_changes`: replaced by `unrouted_changes` +
  `route_changes` in both import and export. Done.

### Checks

- `cargo test -p loopflow --lib -- store::sqlite::planning_peers:: store::sqlite::planning_write:: store::sqlite::children store::sqlite::task_ownership ops::planning_sync store::query_plans::`: 45 passed (implement).
- Compress (shared `Import::read`, `record_revision`, `touched_records` in status): same 45 passed; clippy clean.
- Recompute test fails with the recheck disabled ("remote" overwrites "local"); passes with it.
- `cargo clippy -p loopflow --all-targets -- -D warnings`, `cargo fmt`: clean (implement).
- Competing-writer probe on a store copy, 2,000 unrouted: not run; gate.
- Integration callers (`machine_commands`, `planning_foreground_tests`): CI.

## Approach: plan under a read snapshot, write under the lock

Split the body into two private functions over one `ImportPlan` (shipped as
`Import::plan` and `Import::write`):

- `plan_import(conn, repo, destination, incoming, in_conflict) -> ImportPlan`
  reads only. It runs in a deferred transaction (as `export_peer_planning` does
  since LOO-859), so it never takes the write lock and every query sees one
  commit. It returns:
  - `revision`: `store_revisions.planning`, read inside the same snapshot;
  - `unrouted`: ids of owned changes with no route to this destination
    (the `has_unrouted_changes` query, returning ids instead of `EXISTS`);
  - `unseen` (`unseen_in`, unchanged);
  - `merged`, `grouped` edits and `heads` for the touched records only.
- `apply_import(tx, plan, …)` does today's write half: `retain` + route the
  unseen changes, `ensure_record`/`write` with savepoints and retry, WaveProject
  pass, `observe`, ownership markers and steers, `reassert_in`, conflicts,
  import row.

`import_peer_planning` becomes: early-return check (unchanged) → canonicalize and
validate incoming → register repository → read config / machine names →
`plan_import` in a read tx → `BEGIN IMMEDIATE` → `require_destination` → if
`store_revisions.planning` moved, `plan_import(&tx, …)` again under the lock →
route `plan.unrouted` → `apply_import` → commit.

### Narrow the local history to touched records

The merge is per field: `validate` requires a parent to edit the same key, and
`winning_heads`/`heads_by_key` group by key. So merging only the local history
of the records `unseen` touches gives exactly the winners and heads the whole
document gives for those records. Add a record-scoped export:

```sql
SELECT c.id,c.mutation FROM planning_peer_changes c
WHERE c.object_id=?4 AND c.record_table=?5
  AND (c.id IN (SELECT mutation_id FROM planning_peer_routes WHERE repo=?1 AND destination=?2)
       OR EXISTS(SELECT 1 FROM owned o WHERE o.record_table=c.record_table AND o.object_id=c.object_id))
```

run once per touched record through `prepare_cached`, driven by the
`planning_peer_changes_object(object_id,field)` index. Whole records, not just
touched fields: the `incomplete planning record` check needs a winning edit for
every stored field of the record. Check the plan with `EXPLAIN QUERY PLAN`
(`store/query_plans.rs` has the pattern); `owned` must not materialize the whole
CTE per record — if it does, replace `EXISTS(owned)` with a per-table ownership
check for the one record (`require_repository`'s joins).

`export_in(…, include_current=true)` then has no caller; `export_rows` keeps
its export caller. `ReusedChange` across records stays caught by
`planning_write::retain`, which compares against every stored mutation.

### Why the planning revision is the right recheck

Everything `plan_import` reads lives in tables whose triggers advance
`store_revisions` domain `planning` (`tasks`, `waves`, `projects`,
`task_comments`, `planning_peer_routes`, `planning_peer_conflicts`,
`planning_peer_imports`), except `planning_peer_changes`, which is in
`NEVER_DISPLAYED`. Local edits append to `planning_peer_changes` only through
`planning_write::capture`, which `write_record` calls after updating the
record row in the same transaction, so a local change moves the revision with
it. `implement` adds `max(rowid)` of `planning_peer_changes` to the recheck
only if it finds an appender outside that path.

A moved revision costs one recompute under the lock, and that recompute is
now record-scoped, so the worst case is still far below today's.

### `in_conflict`

With active conflicts `unseen` is the whole incoming document, so record
scoping covers every record and costs what today's path does. Leave it: the
`imported && !in_conflict` early return already keeps the common case off this
path, and conflicts are rare.

## Alternatives considered

- **Narrow only, keep everything under the lock.** One function, no recheck. Under-lock cost becomes the record-scoped
  reads plus merge — likely tens of ms for 2,000 unseen. Rejected: `route_repository` (39 ms
  on manabot) and the merge stay under the lock, and the Task asks for writes only.
- **`PRAGMA data_version` as the recheck.** Moves on any commit from another
  connection; with hundreds of agents writing Sessions it would always force
  the recompute.
- **Compare the per-record change ids instead of the revision.** Exact for
  local edits but misses ownership moves (`owned`) and concurrent routes from
  another importer of the same destination. The revision covers both.

## Demo

On a copy of the Machine store, a competing writer thread loops `BEGIN
IMMEDIATE; COMMIT` and records its longest wait while one import runs with
2,000 unrouted incoming changes (the LOO-839 case). Before: the writer waits
roughly the import's ~430 ms. After: it waits only for the write phase. The
probe lives in `scratch/` or an `#[ignore]` test that reads the copy's path
from an env var; nothing timing-based is committed.

## Done when

- A planning acquire that imports a moved revision holds the store write lock
  only for `require_destination`, the revision recheck, routing the
  precomputed ids and the record writes; no `export_rows` or whole-history
  merge runs under it.
- Imports produce the same records, observations, conflicts and routes as
  before: existing `planning_peers`, `planning_write`, `children` and
  `task_ownership` import tests pass unchanged.
- Competing-writer wait on the store copy for the 2,000-unseen case drops
  from ~430 ms to the write phase; the number goes in the PR body.

## Tests (focused)

- New, `planning_peers.rs`: plan an import, then make a local edit to a touched
  record on the same store, then apply; the result equals a fresh import of the
  same document (exercises the moved-revision recompute). Calls the private
  functions directly; no production hook.
- New: import a moved revision with nothing unseen and nothing unrouted inside
  `query_plans::assert_no_empty_writes`; only the import row changes.
- New: record-scoped export plan uses `planning_peer_changes_object`
  (`EXPLAIN QUERY PLAN`), alongside the existing query-plan checks.
- `cargo test -p loopflow --lib store::sqlite::planning_peers:: store::sqlite::planning_write::`
  — run by implement.
- Competing-writer probe on a store copy — run by gate; result in the PR body.

## Open finding (out of scope)

`incomplete planning record` on a full self-import means some local records
hold a field with no retained change. A peer edit to such a record returns
`Err` from the whole import, not a conflict, so every later acquire of that
destination would fail the same way. Not changed here; worth its own Task if
a probe on the copy shows which records and fields lack history.

## Exclusions

- `export_peer_planning` locking (LOO-859, merged in #1901).
- The `in_conflict` full recompute.
- The `incomplete planning record` behavior above.

## Docs

None to change: no README, guide or reference describes how a planning import
takes the store write lock (`grep` over `docs/` and `rust/` Markdown, 2026-11-13).
The behavior people notice is shorter write waits, which goes in the PR body.
