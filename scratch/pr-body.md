## Evaluate

Run the long-lived Home compatibility boundary:

```bash
scripts/dev-lf install preflight --json 2>/dev/null \
  | jq '.candidate_compatibility'
```

Before: the installed reader reported 1,770 capture integrity failures.

After: the candidate reads 3,681 of 3,681 complete captures, retains 71 partial
captures separately, and resolves all 98 executable lifecycle references. The
overall preflight still refuses this source build for validation-only authority
and unrelated pending development migrations.

Focused gate checks:

```bash
cargo fmt --all -- --check
cargo clippy --all-targets -- -D warnings
cargo test -p loopflow --lib conversation_reader -- --nocapture
cargo test -p loopflow --lib compatibility_tests -- --nocapture
cargo test -p loopflow --test local_promotion -- --nocapture
```

All pass: 5 reader tests, 5 install compatibility tests, and 4 promotion
integration tests.

## Why it matters

A migrated SQLite ledger was not sufficient proof that its immutable JSONL
artifacts remained readable. Promotion now validates the file schema history
that the long-lived Home actually contains, so a release cannot strand local
trace evidence while its database migrations appear healthy.

## What changed

- Added one strict schema-v1 reader boundary for the six observed historical
  `usage` and `turn_usage` shapes. Current `usage_checkpoint` events decode
  normally; unknown fields, nulls, corrupt records, and unknown versions fail
  closed.
- Extended candidate preflight to audit complete captures from one
  SQLite-consistent snapshot before migrations retire their legacy index, then
  validate executable references after migration.
- Preserved typed failure outcomes for missing, unsafe, unreadable, truncated,
  unsupported, and corrupt captures. Partial captures stay separately counted.
- Replaced fresh-Home testing guidance with the long-lived candidate-copy proof
  and added fixtures for the compatibility and failure boundaries.

## Risks / Not included

The promotion audit is linear in complete capture files and assumes completed
artifacts are immutable. Concurrent capture locking remains LOO-238's scope.
This does not add a legacy writer or legacy `lf doctor` path; current Run
records remain the active trace model. Replay, exact context manifests, and
metrics remain owned by their existing tasks.
