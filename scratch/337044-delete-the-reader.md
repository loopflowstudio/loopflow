# 337044: delete the reader floor

Jack Heart's Task brief (2026-11-05). Status: accepted, implementing. Source: the
2026-11-05 upgrade audit, section 2 "Deleted" column and section 4(b).

## Design

`pending_migrations` is the one authority: a ledger that starts with this build's
catalog owes nothing, however long its tail. Every open path follows from that.
`validate_sqlite_schema` compares schema only when the catalog can name the applied
history (`validate_applied_schema`); past it, SQLite reports what is gone at the
statement that reads it. `Compatibility::Exact` and `Behind` merge into `Current`
(nothing to apply). `schema_migrations.reader_floor` stays, unread and unwritten.

## Delete — do not maintain

Done in the working tree: floor semantics and `validate_schema_within`
(`migrations.rs`), `is_additive`, `table_within`, `breaks_readers`, `-- breaks:`
handling, `reader_floor` in `build.rs`; `Compatibility::Behind`,
`raised_reader_floor`, `release_opens_store`, `can-open`, `reader-catalog`, the
package asset (workflow, `publish_release.py`), the Sparkle veto (`AppUpdater.swift`);
rollover `binary_opens_store` and `cannot_open_store`; `BREAKING.md` plumbing in
`release.rs` and `release-notes.md`; the hotfix prune case. No code names
`~/.lf/copies`. `migration_build`'s `sql` field and `(version, path, merged)` tuple fell
with the additivity proof: one `CatalogEntry` now. Shipped `release/v*/BREAKING.md` and `0.14.16.002`'s `-- breaks:`
comment stay as records.

## Remaining

- Rebase onto 843f5c (staging CI rule, its `MIGRATIONS.md` text, and its floor edits
  to `is_additive`, `reader_catalog`, install report) and e46a80 if they land first.
- Gate: Swift build for the `AppUpdater.swift` deletion; affected integration suites.

## PR notes

- Rollback: a build carrying this change classifies a newer store as `current` and
  promotes, additive or with a dropped column
  (`an_older_candidate_selects_a_store_a_newer_build_migrated`). Builds already
  retained in `~/.lf/bin` keep their floor check, so they gain nothing. `lf self install
  rollback` still refuses receipt-managed installs (audit, section 2); unchanged here.
- Demo, private `LF_HOME`, 2026-11-05: additive tail opens, lists and creates Tasks,
  `lf doctor` says compatible. After `DROP COLUMN promoted_at`, `lf task list` still
  works; `lf wave list` exits 1 naming `promoted_at`; `lf doctor` still says compatible.
  The error reaches stderr, not `processes.error`: a private home records no Process
  rows, on the installed 5519340b4 build too. Durable recording is unproven here.
- `lf self install preflight` always reads `~/.lf/loopflow.db`, ignoring `LF_HOME`.

## Checks

`cargo nextest ... test(store::)`: 415/416, the failure fixed in `apply_set`; rerun
of `store::migrations`, `frontier_tests`, `migration_build`, install, rollover,
`ops::release`: 228 passed. Whole `store::` not rerun after that fix (gate).
`cargo test --lib store::` was killed at 30 min under load ~100; no result.
`pytest python/tests/test_release_publisher.py`: 38 passed.
Compress: `cargo nextest` over `migration_build`, `store::migrations`, `frontier_tests`, rollover, `compatibility_tests`: 154 passed; clippy and fmt clean. Whole `store::` stays with gate.
