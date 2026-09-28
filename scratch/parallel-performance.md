# Schema validation performance — contributor

2026-09-28 · LOO-298 · Owned files: this note and
`rust/loopflow/src/store/migrations.rs`. No build slot granted yet.

## Early approach and proof plan

Observed: `_validate_development_schema` replays every released migration and
the applied draft prefix into a new in-memory database on every call.
`validate_schema` repeats the released replay. Ordinary command journal writes
open the store repeatedly, so a single command can pay the reconstruction cost
several times. The previous fleet sample identified this same replay stack.

An isolated Python/SQLite 3.50.4 experiment replayed the 54 canonical files in
0.149 seconds without a surrounding transaction and 0.148 seconds with one.
This is a separate SQLite build and measures empty schema replay only; grouping
statements in a transaction did not provide a useful reduction in that sample.
No Rust, provider or installed database was used.

Proposed implementation: one process-local memo of **expected schema values**,
keyed by the exact ordered embedded SQL bytes, shared by canonical, draft and
historical-prefix comparisons. Build each requested schema once; serialize its
construction so concurrent first opens do not replay it together. Read and
compare the actual connection's full product schema on every call. Keep
migration-ledger, checksum, prefix and foreign-key checks on their current
paths. Never memoize acceptance of a database or read a persistent cache.

This targets repeated opens within each command and the 51-writer fleet. A new
process still pays one reconstruction per distinct requested schema; it is not
a claim of zero cold-start cost. No build.rs, generated artifact or shared-file
dependency is required. The measured end-to-end effect must decide whether
this simple reduction is sufficient.

After a supervisor build slot, with authority scrubbed and a disposable default
Home, nice +10, four build workers and a 900-second phase bound:

```sh
cargo nextest run -p loopflow --lib --test-threads 4 --no-fail-fast -E 'test(store::migrations::tests::)'
cargo nextest run -p loopflow --test store_contention --test-threads 4 --no-fail-fast
cargo clippy --all-targets -j 4 -- -D warnings
cargo build -p loopflow --bin lf -j 4
```

New focused regressions will prime expected-schema reuse, then alter a different
store or its schema/ledger/foreign-key data and require rejection. Historical
prefix and changed-draft cases retain their existing proofs. Materialize drafts
only in a disposable complete source snapshot for the final integrated proof.

Benchmark the copied candidate against the retained baseline binary
`/tmp/loo298-exec-latency-zev1ow10/lf`, each with its own fresh private Home and
an allowlisted environment, outside a repository. Record first and repeated
`session list --all --json` calls, stdout/stderr, executable hashes and host
conditions. Original receipt: `/tmp/loo298-exec-latency-zev1ow10/receipt.json`
(3.799 seconds first, 2.469/2.476/2.370 seconds warm). Concurrent execution-model
edits mean this end-to-end comparison alone cannot attribute every delta to this
change. No installed acceptance follows.

## Source ready — build slot needed

The change is ready for compilation and verification. All edits are unstaged in
the two owned paths; other contributors' files remain untouched. No Cargo
command or global formatter ran.

`expected_schema` now serves all four schema comparisons. Its key is a vector
of static SQL strings compared by contents and order, not migration names,
length, store path, schema version or a database-validation result. The value
retains the existing complete table/index/trigger SQL and foreign-key metadata.
Only successful expected-schema construction is retained. The mutex covers
construction, so concurrent initial callers share one result; the actual-store
comparison runs after releasing the mutex. Failures propagate normally.

Two new behavioral tests in the owned file cover:

- historical prefixes and different SQL with identical migration identities,
  using two independent databases;
- schema/table, index and trigger drift after reuse, changed canonical and
  draft receipts, a missing canonical-prefix row, and invalid foreign-key data.
  Rollback restores acceptance, and another unchanged database remains accepted.

Review finding: the existing exact-draft apply path returns after the schema
comparison; the explicit validation path performs `foreign_key_check` afterward.
This change preserves both paths and does not widen their acceptance or claim
to add a missing check. The new data-drift case targets explicit validation.
Historical adoption's short pre-provenance prefixes previously reconstructed
without a ledger table; the common builder initializes the same excluded
bookkeeping table as canonical validation. Their product schema comparisons
remain exact, and the existing divergent/permuted preservation tests must pass.

Non-building checks:

```sh
rustfmt --edition 2021 --check rust/loopflow/src/store/migrations.rs
git diff --check -- rust/loopflow/src/store/migrations.rs scratch/parallel-performance.md
```

The initial formatter check found only wrapping in the new test. Formatted this
owned file alone. Both checks then pass. Production diff against the starting
file: **+48 / −34 = +14** lines; test/import additions **+85 / −1**. The
four reconstruction loops are replaced by one builder; this is reuse of an
immutable expected value, not deletion of actual validation or a whole-branch
size claim.

First build-slot command, before the affected migration suite listed above:

```sh
cargo nextest run -p loopflow --lib --test-threads 4 --no-fail-fast -E 'test(expected_schema_reuse_)'
```

Use the supervisor's existing bounded/private-Home runner for every Cargo
command. No Rust behavioral result, contention result or candidate timing has
been obtained yet. The baseline is preserved; the next step is the supervisor's
build slot, followed by those focused proofs and the paired command measurement.

## Supervisor verification

Resource preflight passed with 67.1 GiB free against the 64 GiB floor. The
bounded private-Home runner executed the first command above successfully:
`parallel-schema-focused.log` records both new regressions passing in 1.722 s
after a 30.03 s compilation. This proves their drift/prefix cases on that
compiled source, not end-to-end performance or the final integrated branch.

The subsequent affected migration suite (`parallel-schema-migrations.log`)
exited 101 during compilation: the concurrently authored H7 API removes
ProjectFlowPlan and adds flow/status while shared callers still use the previous
API. No migration test ran in that command. The supervisor released the build
slot and deferred candidate build, latency comparison and wider proof until
those callers are integrated. No contributor-owned source was overwritten to
make an intermediate build pass. No latency improvement is claimed yet.

## Paired CLI measurement after shared integration

The supervisor measured copied baseline and candidate executables on
2026-09-28 with distinct fresh disposable Homes, an allowlisted environment,
no inherited execution authority, and a cwd outside any repository. Five calls
per binary were interleaved, reversing order between rounds. Every call to
`session list --all --json` returned `[]`, exit 0, with no stderr.

| Measurement | Baseline | Candidate |
| --- | --- | --- |
| First call, including private-Home initialization | 3.668 s | 2.370 s |
| Median of four subsequent fresh-process calls | 2.354 s | 0.704 s |
| Subsequent range | 2.315–2.367 s | 0.697–0.774 s |

The repeated-call median is about 70% lower in this sample. This compares two
branch binaries, including other integrated changes; it does not isolate the
schema memo's causal contribution. Empty inventory does not establish dense
Session performance, Desktop latency, installed acceptance or final-tree proof.
Each invocation remains a fresh process; only the private Home is reused.

Reproduction: `.lf/tmp/measure-schema-cli.py`. Receipt and copied binaries:
`/var/folders/m6/r3tllnrs1yq7yfbwm680tss40000gn/T/loo298-schema-latency-qdc_onrg/receipt.json`.
Baseline SHA-256: `c460d301f993b54c05a6ab53ac2dfb1aeb89d570c88fad630a046cd0c692ff9a`.
Candidate SHA-256: `5dd1c368f1ed383821ec81eb4df3cbbef981771d7e3823fdbeb177767b59dda6`.
The copy was checked against the source hash before and after copying. Free
disk was 65.76 GiB; one-minute host load was 5.21 before and 5.56 after. The
managed worker's serialized compilation overlapped part of this sample.
Broader migration/contention and canonical proofs remain due.
