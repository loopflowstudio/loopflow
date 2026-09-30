# Skip foreign-Team Projects (LOO-359)

Jack Heart requested this repair on 2026-09-30 after Technical Architecture,
a foreign-Team Project in Infrastructure's Initiative, blocked the entire sweep.

## Design

Filter Projects with known Teams that exclude the repository Team before read
validation, title normalization, issue enumeration, and sync rename planning.
Preserve strict ownership validation for missing Teams, shared repository/foreign
Teams, and explicit mutations. Retained Project/Task history remains intact.
Sweep reports each skipped Project once by stable ID, including JSON output;
Project-level entries have no issue. Eligible repository issues keep the existing
preview, exclusion, fresh-membership check, and cancellation path.

## Delete — do not maintain

The unconditional foreign-Team rejection is removed from Project enumeration in
sweep, planning refresh, and wave sync. Keep the ownership validator for mutations
and repository-owned snapshot integrity. No schema migration or parallel owner;
no remaining deletion targets.

## Remaining acceptance

- Configured acceptance still needs a live preview naming Technical Architecture
  once and cancellation of eligible LOO-309/LOO-329 issues if still open. No live
  issue was canceled during implementation or gate; demo owns this acceptance.

## Review — 2026-09-30

Sync's second Project enumeration also needed filtering: fixing only its preview
would have allowed foreign Project renames. That loop now skips foreign ownership
and validates remaining ownership before renaming. Explicit Task mutations retain
their strict validator. Snapshot refresh only upserts observed Projects/Tasks;
omitting a foreign Project does not delete retained history. Sweep's existing
fresh membership and worker/PR exclusions remain unchanged.

Compression keeps the foreign-Team predicate private to the operations module,
where all four callers live. The provider model retains its strict validator.
The stateful GraphQL fixture now locks once per request, removing repeated locks
and reading Project identity and title from the same state. No asynchronous work
runs while the request holds that lock.

## Gate — 2026-09-30

Review found no further code changes needed. The skip predicate runs before
Project validation and title normalization in the changed readers; sync repeats
the ownership decision before renaming. Explicit issue mutation still resolves
fresh ownership through the strict validator. Snapshot omission does not delete
retained Project/Task records. The documented optional issue matches both JSON
serialization and text rendering; there is no Swift mirror of SweepEntry.

Materialized checks used a disposable source copy, leaving the assigned checkout
and installed Home untouched. Inherited `LF_*`/`LOOPFLOW_*` authority was cleared,
development provenance was explicit, and `LF_BIN` pointed at the compiled source
CLI. The initial copy setup encountered the Ghostty submodule directory before
any test ran; the corrected Rust-only copy omitted that unrelated submodule.
The disposable copy was removed after the suite. Fixture results do not establish
configured Linear acceptance or shipment. Hosted CI owns the full matrix.

Gate check: `cargo fmt --all -- --check`, `cargo clippy --all-targets --jobs 4 -- -D warnings`, `uv run python scripts/check_architecture.py`, `uv run pytest python/tests/test_loopflow_skill_alignment.py -q` (4), `uv run python dev.py test` from website (78 passed, 3 skipped), and `git diff --check d296b480` passed; in a disposable source copy after `scripts/canonicalize_migrations.py 0.12.28 --materialize-for-tests`, `cargo build -p loopflow --bin lf --jobs 4` and `cargo nextest run -p loopflow --lib --build-jobs 4 --test-threads 4 -E 'test(ops::pm::) | test(pm::linear::) | test(ops::linear_observe::)' --no-fail-fast` passed (91); configured preview/apply remains with demo and full matrix with CI.
