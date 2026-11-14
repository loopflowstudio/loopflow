# LOO-872: no write lock when nothing changes

## Problem

Every write connection begins `IMMEDIATE` (`configure_write_connection`,
`store/sqlite.rs`), so a `conn.transaction()` that only reads, or an
autocommit `UPDATE`/`INSERT OR IGNORE` that matches nothing, still takes the
lock every agent on the Machine waits on. LOO-839 fixed the ones it measured;
nothing keeps them fixed. `contention::watch_write_holds` only logs holds over
3 s. One empty write is cheap; one on a per-hook or per-read path waits up to
15 s behind any other writer.

## Demo

```bash
cargo test -p loopflow --lib store::query_plans:: server::boundary::tests::
```

A path that regresses fails with its call site:

```text
took the write lock and changed nothing:
  <loopflow::store::sqlite::SqliteStore>::record_owner_viewer_hold::<&loopflow::id::LfProcessId>
```

## Approach

The hooks `watch_write_holds` already installs see every write transaction
end. In test builds they also note whether a row changed, and a transaction
that ended holding the lock with no changed row is recorded with
`contention::call_site()`. `query_plans::assert_no_empty_writes(store, run)` fails naming
those call sites for `store`'s connection and returns `run`'s result, so it
wraps `statements(store, run)` directly. Production behavior is unchanged: no logging, no new state
outside `#[cfg(test)]`.

What decides "took the lock" (checked against bundled SQLite 3 in
libsqlite3-sys 0.38.2, `sqlite3.c`):

- The commit hook runs only when a btree is in a write transaction
  (`vdbeCommit`: `needXcommit && db->xCommitCallback`), so a commit hook call
  is a write. That covers empty `IMMEDIATE` commits and autocommit writes that
  matched no row.
- The rollback hook also runs for a read-only transaction ending
  (`sqlite3RollbackAll`: `inTrans || !db->autoCommit`), e.g.
  `ReadSnapshot`'s `BEGIN DEFERRED … ROLLBACK` and any dropped deferred
  transaction. And by then the btree state is already reset, so
  `sqlite3_txn_state` can't tell. The progress handler's `since` tells them
  apart, if it runs after `BEGIN IMMEDIATE`: `vdbe_return` checks progress at
  the end of every statement, so with `PROGRESS_STEPS = 1` in test builds
  `since` is set before a dropped `IMMEDIATE` transaction's rollback.
- `update_hook` marks a change. It misses `WITHOUT ROWID` tables; the only one
  is `store_revisions`, written by triggers that fire on a rowid table's
  change, so a real change always marks. DDL marks nothing: a path that only
  creates a temp table reads as empty (none seen on hot paths).

Recording is per connection, not thread-local: Work reads such as
`registered_task_status` go through `journal::request::spawn_blocking` and
tokio, so the statement that ends a transaction may run off the test's thread.
A `#[cfg(test)]` registry keyed by the connection's handle address holds the
call sites while `empty_writes` runs. (`statements()` is thread-local and has
the same blind spot; leave it unless a test needs it.)

Call site: the innermost `loopflow::` frame outside `store::contention` and
`store::query_plans` (`contention::call_site`). The one regression run named
the public store method, so no second frame is reported.

## Where it asserts

Each assertion wraps the whole path: a write that changes a row is never
reported, so a first call that records a receipt or a hold needs no carve-out.

1. `store::query_plans` tests: the repeated `record_owner_viewer_hold(Viewed)`,
   `pending_retirements`, `sessions_for_agent_session`,
   `resolve_history_input`, `unrecorded_agent_ancestry`, a second
   `session_steers` and `session_steer_source`.
2. `server::boundary::tests::boundary_statements_never_scan_tables_that_grow_with_use`:
   the boundary sequence (Start resume, Tool, Compact, Tool, Stop).
3. Work reads on the write store:
   `ops::task::registered_task_status` (`lf task status`, Desktop, screens),
   reached by `ops/human_session/tests.rs`
   `task_detail_reads_capacity_once_for_all_sessions`; `read_dashboard` and
   `resolve_selection` (`store/sqlite/selections.rs` fixtures). The Work
   subscription itself reads through `open_read_only` with `query_only`, so
   it cannot take the lock; no assertion there.

Plus one test of the check itself: a dropped `IMMEDIATE` transaction that
read, an empty `IMMEDIATE` commit and an autocommit `UPDATE` matching nothing
are each reported; `BEGIN DEFERRED … ROLLBACK`, a read-only savepoint and a
transaction that changed a row are not.

## Fixing what it finds

Read first and return before opening the transaction, or use a savepoint
(deferred: takes the lock only at its first write). A finding that needs a
rewrite gets its own Task, as LOO-865..870 did, and its assertion lists the
known call site with that Task's ID until it lands.

## Docs

Written: `docs/dev/data.md` "Query cost and the write lock", the paragraph
after the foreign-key one (`empty_writes` example, failure text, what is not
reported, how to fix). Implement keeps the example's signature and failure
text true, or edits the doc.

## Status

Built: `contention::empty_writes` (per-connection registry keyed by handle
address, `PROGRESS_STEPS = 1` in test builds), `query_plans::empty_writes` (self-test only) and
`assert_no_empty_writes`, the self-test, and assertions in both query_plans
path tests, the boundary sequence, `task_detail_reads_capacity_once_for_all_sessions`
(now `multi_thread`, `block_in_place` around `registered_task_status`) and
`query_selection_uses_current_reported_attention_and_prunes_ended_activity`.
Docs match the built API. Merged origin/main through #1910 without conflicts.

Remaining: gate (clippy `--all-targets`, `documented_commands::`), publish.

## Concept review (2026-11-13)

One concept, one name: an empty write is a transaction that held the write
lock and changed no row; `assert_no_empty_writes` is the only public entry and
the docs teach exactly it. Three of four call sites nest it around
`statements()`; a combined helper would hide which check failed, so they stay
separate. Open nit for gate, not a redesign: the private
`query_plans::empty_writes` wrapper exists only for the self-test, which could
call `contention::empty_writes::record` directly. No open product choice.

## QA (2026-11-13)

Four injected regressions (read-first dropped in `record_owner_viewer_hold`,
IMMEDIATE read in `task_ownership`, empty autocommit UPDATE in
`read_dashboard`, zero-statement dropped transaction in `boundary::answer`)
each failed their test naming that function. Added `#[track_caller]` so the
panic points at the test. `a_steer_is_delivered_once_the_hook_says_it_printed_it`
failed under load (2 s `HOOK_DEADLINE` crossed between answers); it and the Pi
test now pin `created_at` ahead. Report: `scratch/qa.html`.

## Exclusions

- No production logging or metric for empty writes.
- No audit of the ~160 transaction sites the fixtures don't reach.
- No change to `IMMEDIATE` as the write connection's default.

## Done when

`cargo test -p loopflow --lib store::query_plans:: server::boundary::tests::
store::contention::` and the human_session/selections tests named above pass
with `empty_writes` assertions, and reintroducing an empty write on any of
those paths (e.g. dropping the read-first check in `record_owner_viewer_hold`)
fails with that function's name.

## Run results (2026-11-13, load average ~430)

- Dropping the read-first `return` in `record_owner_viewer_hold` fails
  `retirement_sweeps_and_viewer_polls_plan_by_index` with
  `<loopflow::store::sqlite::SqliteStore>::record_owner_viewer_hold::<&loopflow::id::LfProcessId>`.
- `store::query_plans:: store::contention:: store::sqlite::selections::`
  (35 tests): base real 449 s / user 148 s; with `PROGRESS_STEPS = 1` real
  657 s / user 119 s. Wall time tracks Machine load, CPU time did not rise, so
  the step count stays 1 for every test connection.
- `a_slow_writer_names_itself…` failed once under load: its `CREATE TABLE`
  alone held the lock past the 200 ms test `SLOW_WRITE` and matched first
  (held < 0.3). The table is now created before the watched span.

## Checks

- Focused (17 tests: query_plans, contention, boundary, human_session, selections): pass after compress (322 s, load ~600).
- `cargo clippy -p loopflow --lib -- -D warnings`: clean.
- Gate: `test(documented_commands::)` for `docs/dev/data.md`, plus
  `cargo clippy --all-targets -- -D warnings`.
