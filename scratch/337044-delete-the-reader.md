# 337044: delete the reader floor

Jack Heart's Task brief (2026-11-05). Status: accepted, implemented; concept review
2026-11-05 changed the `reader_floor` write, implement next, then gate. Source: the 2026-11-05 upgrade audit, section 2 "Deleted"
column and section 4(b).

## Design

`pending_migrations` is the one authority: a ledger that starts with this build's
catalog owes nothing, however long its tail. Every open path follows from that.
`validate_sqlite_schema` compares schema only when the catalog can name the applied
history (`validate_applied_schema`); past it, SQLite reports what is gone at the
statement that reads it. `Compatibility::Exact` and `Behind` merge into `Current`
(nothing to apply). `schema_migrations.reader_floor` stays and this build never reads
it; until installed builds age out, each new row repeats the ledger's newest floor
(see Next implement).

## Deleted — do not maintain

Floor semantics and `validate_schema_within` (`migrations.rs`), `is_additive`,
`table_within`, `breaks_readers`, `-- breaks:` handling, `reader_floor` in
`build.rs`; `Compatibility::Behind`, `raised_reader_floor`, `release_opens_store`,
`can-open`, `reader-catalog`, the package asset (`package-build.yml`,
`publish_release.py`), the Sparkle veto (`AppUpdater.swift`); rollover
`binary_opens_store` and `cannot_open_store`; `BREAKING.md` plumbing in
`release.rs` and `release-notes.md`; the hotfix prune case. No code names
`~/.lf/copies`. `migration_build`'s `sql` field and `(version, path, merged)`
tuple fell with the additivity proof: one `CatalogEntry` now. The string "needs an
lf that knows" survives only in shipped records (`RELEASE_NOTES.md`,
`release/v*/`), which stay as written.

Brief items that needed no change: no fixture under `rust/loopflow/tests/` asserted
a floor refusal (the only floor tests were `python/tests/test_release_publisher.py`,
updated, and unit tests in `src`, replaced by
`an_older_lf_keeps_working_on_a_store_a_newer_build_extended` and
`an_older_candidate_selects_a_store_a_newer_build_migrated`);
`docs/architecture/data.md` never described the floor.

## Next implement: carry the floor forward

Concept review found the "stop writing" decision fails the brief's own test. The brief
stops the write only if NULL is harmless, and it is not. Builds already installed still
run `admitted_newer_floor`, which reads NULL as the row naming itself. On main an
additive migration inherits the head floor and those builds keep working. On this
branch the next additive migration, however harmless, refuses every one of them,
long-lived processes included: the 2026-11-04 incident once more, caused by the change
meant to end it. The brief's fallback, writing the migration's own name, does the same.

Measured with installed 0.15.1+5519340b4, private `LF_HOME`, an added nullable column
plus a ledger row, `lf doctor`:

| New row's `reader_floor` | Result |
| --- | --- |
| NULL (this branch) | `FAIL store ... this store needs an lf that knows later_additive` |
| Own name (brief's fallback) | no `ok store` line, same refusal path |
| Previous row's floor | `ok store selected database is compatible with this build` |

Doctor runs the same `validate_sqlite_schema` a shared ordinary open runs (`sqlite.rs`).
A private home's open also goes through `apply_sqlite_with_backup`, which refuses any
unknown row on the installed build, so `lf wave list` cannot show this there.

The brief's outcome ("an older lf opens a store a newer lf migrated"), its harmless
condition and AGENTS.md's shared-file exception settle it: the ledger is read by released
lf and this build at once, so the branch keeps the released shape readable, marks the
fallback with the Task that removes it, and files that Task in the same change. A dated
stage is what Jack Heart's 2026-10-27 staging rule allows.

- `insert_applied_migration` sets the new row's `reader_floor` to the ledger's newest
  recorded floor, behind the existing column check (`ledger_has_reader_floor`, stores
  before 0.14.11 lack the column). It reads no floor to decide anything. No compiled
  `reader_floor` returns: `Migration`, `build.rs` and `is_additive` stay deleted.
- A store advancing across historical migrations carries an older floor past a breaking
  one, so a shipped build may pass the floor check and then meet its own
  `validate_schema_within` refusal or SQLite's error. Same outcome, other words; accepted.
- Mark the write with the removal Task, filed in this change: delete the write 30 days
  after the release carrying this one, the staging window, when no older process remains.
- Test: applying a migration writes the previous row's floor, and NULL never follows a
  recorded floor. Rewrite the `MIGRATIONS.md` line "new rows leave it `NULL`" to match,
  and say why in the same two sentences.
- Demo: repeat the table above with the branch build writing the row, not `sqlite3`.

Unchanged limit: a carried floor admits shipped builds past additive migrations only.
A shipped build's `validate_schema_within` still refuses a changed object, including a
CHECK-only rebuild 843f5c counts as additive. That is today's behaviour, and those builds
age out inside the 30-day window a contract waits for anyway.

## Remaining

- Gate: affected suites once (whole `store::`, install, rollover, `ops::release`,
  `test_release_publisher.py`), Swift build for the `AppUpdater.swift` deletion.
- Rebase onto 843f5c when it lands (its branch now carries the additive-or-contract
  rule in `scripts/check_migrations.py`, `-- expand:`/`-- exports:` markers and a
  rewritten staging section; it still has three implement items, then gate). Both
  branches edit `MIGRATIONS.md`, `install.rs`, `rollover.rs`, `migrations.rs`,
  `lf.rs`, `docs/architecture-reference.md`, `scratch/questions.md` and the LOO-656
  section of Wave memory, which both trimmed to fit 16000 tokens; the second to
  land re-trims. Its install line "advances the store from X to Y" stays; only the
  floor line goes. e46a80 (store-error tolerance) has no commits yet.

## PR notes

- Rollback: a build carrying this change classifies a newer store as `current` and
  promotes, additive or with a dropped column
  (`an_older_candidate_selects_a_store_a_newer_build_migrated`). Builds already
  retained in `~/.lf/bin` keep their floor check, and the carried floor admits them past
  additive migrations only. `lf self install rollback` still refuses receipt-managed installs (audit, section 2); unchanged here.
- Demo, private `LF_HOME`, 2026-11-05: additive tail opens, lists and creates Tasks,
  `lf doctor` says compatible. After `DROP COLUMN promoted_at`, `lf task list` still
  works; `lf wave list` exits 1 naming `promoted_at`; `lf doctor` still says compatible.
  The error reaches stderr, not `processes.error`: a private home records no Process
  rows, on the installed 5519340b4 build too. Durable recording is e46a80's contract,
  unproven here.
- `lf self install preflight` always reads `~/.lf/loopflow.db`, ignoring `LF_HOME`.
- 843f5c keeps the pre-migration clone while any superseded build runs; without a
  floor the inventory has no per-build generation, so an install with no migration
  keeps the clone too (disk until the next commit). Left as is; a finer test is a
  product choice for Jack Heart.

## Checks

Compress (2026-11-05): `cargo nextest` over `migration_build`, `store::migrations`,
`frontier_tests`, rollover, `compatibility_tests`: 154 passed; `pytest
python/tests/test_release_publisher.py` 38 passed; clippy and fmt clean. Realign:
`cargo check -p loopflow --all-targets` clean; whole `store::` deferred to gate.
