# First-Home initialization repair — source ready

2026-09-28 · LOO-298 · Bounded contribution authorized by Jack Heart.
Only `rust/loopflow/src/store/migrations.rs` and this note changed. Existing
expected-schema memo and drift tests remain intact. No shared-file patch is
needed. No Cargo/Swift command, benchmark, installed-Home access, provider
request, commit or Flow action ran here.

## Observation and scope

The supplied receipt at
`/var/folders/m6/r3tllnrs1yq7yfbwm680tss40000gn/T/loo298-first-home-execs-bf8h6n0w/receipt.json`
records concurrent `session list --all --json` and `usage --json` against a
pristine disposable Home. Both returned zero and `[]`; usage warned that its
ledger was unavailable because the database was incompatible. Two succeeded
Exec rows do not establish admission: usage had only its completion event and
its stored start equaled completion. Candidate SHA-256:
`5dd1c368f1ed383821ec81eb4df3cbbef981771d7e3823fdbeb177767b59dda6`.
This contribution read that evidence; it did not rerun or upgrade its result.

Source showed development ledger detection, canonical initialization, release
adoption decisions, applied-prefix validation and draft application spanning
multiple snapshots/transactions. A contender could read the predecessor before
waiting for a writer and then apply those stale decisions to its successor.
This explains a possible failure path; the retained CLI receipt does not locate
its exact failing SQL read.

## Repair

- Current-store detection reads ledger, ordered prefix, checksums and actual
  schema in one deferred read transaction. It releases that snapshot before
  requesting the writer lock. Already-current opens take no writer lock.
- An advancing development open uses the existing exclusive migration
  transaction discipline for the whole operation: canonical initialization or
  byte-identical release adoption, prefix/schema validation, draft append and
  final validation. It rereads everything after acquiring the lock. No pending
  count, ledger-existence decision or adoption plan crosses that boundary.
- Adoption no longer starts its own transaction. The single outer transaction
  also rolls back adoption if a later draft fails. Canonical initialization and
  drafts become visible together; a failed first draft leaves no partial
  canonical database. Canonical foreign-key/JSON checks remain before drafts.
- Canonical and development validation, plus canonical migration-needed
  detection, use the same read-snapshot helper. An existing caller transaction
  or SAVEPOINT is reused without commit/rollback or nested BEGIN. The helper
  explicitly requests Deferred regardless of the connection default.
- Canonical migration still calls its backup callback inside the same exclusive
  transaction, under the existing migration-file lock. No new filesystem lock,
  promotion lock acquisition, migration authority or persistent owner is added.
  Foreign-key disabling/restoration remains in the shared transaction wrapper.

The expected-schema memo still caches only expected SQL results. Actual schema,
checksums, exact prefixes and foreign-key evidence are still inspected. The
already-current development apply path retains its prior schema-only final
check; explicit validation continues to check foreign keys. No released SQL,
checksums or another contributor's draft changed.

## Added proofs — authored, not executed

`development_initializers_recheck_after_another_writer_commits` uses two real
SQLite connections and WAL. Its three cases cover first initialization, a draft
append, and release adoption with an unreleased draft retained. A test-only
SQLite busy callback and channels stop the contender at writer contention; the
winner commits before the contender acquires its transaction. No timing sleep
or production retry was added. Assertions require both callers to succeed,
complete canonical receipts, exactly two draft receipts, a non-idempotent data
change performed once, and preservation of the earlier draft timestamp. The
adoption fixture derives provenance from the last canonical release, including
when drafts are materialized, and has no final-head Chapter-table assumption.

`development_validation_keeps_one_snapshot_without_blocking_a_writer` advances
another WAL connection between validations while the reader retains its
snapshot. Canonical and development validations must remain consistent; after
release the reader sees the new schema. An already-current development apply
must also succeed with zero busy timeout while another connection holds a
writer reservation.

`failed_first_draft_rolls_back_canonical_initialization` requires a failed first
draft to leave no user tables, no open transaction, and foreign keys restored;
a subsequent valid initialization must work.

Only these nonbuilding checks ran and passed:

```sh
rustfmt --edition 2021 --check rust/loopflow/src/store/migrations.rs
git diff --check -- rust/loopflow/src/store/migrations.rs scratch/parallel-initialization.md
```

The owned Rust file was formatted alone. These checks establish parsing/layout,
not typing or behavior. No failing-before/passing-after test run is claimed.

## Supervisor proof commands

Use the existing resource preflight, scrubbed LF/LOOPFLOW authority, disposable
Homes, nice +10, four Cargo workers and the single assigned build slot. Run the
smallest proof first:

```sh
cargo nextest run -p loopflow -j 4 --lib --test-threads 4 --no-fail-fast -E 'test(development_initializers_recheck_after_another_writer_commits) | test(development_validation_keeps_one_snapshot_without_blocking_a_writer) | test(failed_first_draft_rolls_back_canonical_initialization) | test(expected_schema_reuse_) | test(backup_snapshot_and_migration_share_one_exclusive_transaction) | test(current_schema_does_not_take_the_database_write_lock)'
```

Then run the affected migration/adoption/rejection suite and contention suite,
including a disposable materialized source snapshot after H7 integration:

```sh
cargo nextest run -p loopflow -j 4 --lib --test-threads 4 --no-fail-fast -E 'test(store::migrations::tests::)'
cargo nextest run -p loopflow -j 4 --test store_contention --test-threads 4 --no-fail-fast
cargo clippy --all-targets -j 4 -- -D warnings
```

Retain checksum, corruption, divergent/permuted history, adoption rollback and
backup tests. Do not infer their pass from the new focused regressions.

## Chapter fixture integration

Three existing adoption tests share `chapter_development_home`, which replays
all pending releases and then inserts/reads `wave_chapters`:

- `installed_development_home_keeps_chapters_when_its_draft_is_released`
- `installed_development_release_keeps_unreleased_drafts`
- `installed_development_release_rejects_changed_evidence_without_losing_data`

Materializing H7's drop makes their final-head assumption obsolete. These were
left intact, as requested. Replace their final-head preservation payload with a
surviving canonical record (for example a Wave), use that record for schema
corruption and unchanged-data assertions, and keep the unreleased annotation
receipt/time proof. Preserve the separate historical-prefix Chapter assertions
in `task_issue_identities` / `task_deletions` migration tests: those intentionally
stop before H7 and must still prove their historical behavior. Do not weaken
those assertions to make a final-head fixture pass.

## Remaining main-owned proof

Rebuild/copy the candidate and rerun `.lf/tmp/probe-first-home-execs.py` through
the supervisor's serialized slot. Require real admission and original start
metadata plus started/completed events for both actual commands, not merely two
terminal Exec rows. Main owns the journal's best-effort start-write policy;
this repair does not make it fail closed, fabricate lost starts or make a
completion-only record valid admission. Validation-only callers may still
observe an honestly uninitialized snapshot; this change does not turn that
into an empty valid store or initialize it without authority. Any such separate
read/admission failure must remain visible.

Review found no need for shared-file changes to implement migration
serialization. Existing command/Started policy, configured acceptance,
performance and actual concurrent-CLI success remain unproven here.
