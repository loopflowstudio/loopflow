# Session and Flow metadata discovery — private patch

2026-09-29 · LOO-298 · Prepared for Jack Heart and main `run_344d4bfe`.

**Ready for main's review and application; unapplied and uncompiled.**
`metadata-discovery.patch` is the deliverable. `proposed/` contains complete private
files against captured originals; **do not copy those files over main's work**.
`replay/` demonstrates exact hunk application onto the observed current source,
including the Desktop controls contribution. All writes by this contribution
remain under this directory. No provider, product test, Home, Git mutation or
publication ran.

## Read boundary and product behavior

`human_session::list` now calls one typed `session_summaries` SQL query and a
passive `summary_surface`. Filters, literal contains search, title/ID order,
limit/offset and Desktop's complete-inventory reconciliation stay unchanged.
A materialized selected page precedes metadata joins. It returns Session scalar
fields, nullable typed ancestry, retained labels/placement, and a typed
`FlowSummary`; it never decodes a Flow graph, selected completion, native history,
request or transcript. `SqliteStore::flow_summary(id)` exposes the same exact-ID
Flow projection. This is not a new saved-Flow inventory CLI or paging contract.

The prior list path through `surface → managed_review/task_flow → full capture`,
`input_provider_session → summary_for_input → historical payloads`, and repeated
full Work/Flow reads is removed from listing. `surface`, `owned_target`, full
Flow reads and native/history validation **remain for exact state-changing
operations**. No provider API, claim, navigation or completion check changes.
The existing full-row `Store::sessions` API remains for its other callers/tests;
passive public inventory no longer uses it. No prior private cache is included.

Local exact client receipts still supply Active and terminal IDs so mounted
surface adoption keeps working. No receipt/history is synthesized from SQL
outcomes. Without an observed client, explicit readiness or closure, inventory
returns new wire state `unknown`, rather than guessing Waiting or Closed. A
missing/failed client observation or placement keeps the row with disabled
actions and an explanation. A managed remote Session retains its Home route;
missing placement never falls back to a local launch. Exact Ready feedback is
retained. A completed Flow alone does not complete a Session or prove process
exit. Connect/complete obtain current facts and validate as before.

Known Flow membership retains numeric node/iterations without opening its graph.
Current selected input, pending review or terminal Flow state establish relative
position; missing selection is new occurrence `unknown`, not Earlier. Unknown
membership is never Independent. Only an explicit supported retained manifest
fact establishes Independent when the authoritative Session has no Flow FK.
Rust/Swift enums, CLI text, state colors, membership help and a shared fixture
change together. An unknown position does not prevent viewing an already-known
node in the same captured diagram; the existing invocation/node fence remains.
Three public fixture expectations deliberately change from inferred waiting/
closed to unknown; their identity, feedback, active-client, reconnect and
completion assertions remain.

## Metadata ownership, backfill and conflict handling

One new forward draft, `index_session_metadata`, adds two covering expression
indexes. **No column, product object, new writer or lifecycle cache** is added.
SQLite projects the existing Flow name and retained input-membership discriminant
at index creation and every source insert/update; selected reads fetch indexed
scalars. The exact SQL expression is shared by Flow summary readers and mirrored
in the forward index definition. Final query plans must be checked on bundled
SQLite after any expression/schema change.

Flow writes remain in `store/sqlite/flows.rs` (`insert_in`, import and cursor/
state/selection transactions), plus existing Session review/reservation updates.
Those transactions automatically maintain the index. Existing captures, claims
and parent validators are unchanged. Draft dependencies are `record_native_input`
and `retain_input_sql_evidence`; the build's draft discovery already registers
new files. No applied migration changes.

Manifest evidence writers remain `run_record::RunRecorder` and
`ops/session_import::history`; both use the existing observation envelope and
`retain_history_in`. That writer compares exact receipt payloads: identical replay
is a no-op and conflicting replay is rejected transactionally. The index accepts
only `kind=observed`, the exact current-input `:manifest.json` receipt, matching
input/run IDs and schema 1. Missing/unsupported evidence projects NULL, never an
invented independent relationship. Source FK/ancestry still wins over that fact.
Malformed unrelated transcript payloads cannot break metadata reads or index
creation. A migration transaction populates indexes from retained rows; no
per-Home scan/import or read-time mutation is introduced.

## Evidence actually obtained

- Private Python SQLite 3.50.4 replay of 86 captured canonical/draft SQL files,
  then the new indexes over populated synthetic rows. Six checks cover backfill,
  unknowns, unreadable capture/event detail, automatic source-update projection,
  unsupported membership schema and foreign keys.
- Final `EXPLAIN` bytecode contains **no Flow/event table body reads and no JSON
  function evaluation** in the metadata SELECT. Plans, exact SQL and scope are in
  `probe-results.json`. This is SQL-expression evidence, not Rust/application,
  latency, materialized-runner or configured acceptance.
- An earlier direct outer join generated a JSON fallback path. The retained
  `probe-first.log` records its failed assertion. The final bounded materialized
  Flow projection removes that fallback. Only the red log is retained for that first failure; `probe-results.json` and
  `probe-latest-bytecode.json` describe the final default-inventory query.
- Private Rust parsed/formatted with rustfmt and Swift parsed with swift-format;
  no typecheck/build. Exact original replay, exact current-source hunk replay and
  read-only `git apply --check` pass as recorded in `source-manifest.json`.

The patch is an honest **+405 / −6 production lines** (Rust source before trailing
test modules, non-test Swift, SQL; physical lines including blanks/comments).
Tests, fixtures and docs are excluded from that count. `source-manifest.json`
contains per-file counts, full patch hash, source hashes, timestamps and drift.
This is removal of a costly read path, not a net code-size reduction.

## Integration and smallest behavioral proof for main

Apply reviewed hunks; recheck the manifest against current bytes. Main's native
files are untouched. Shared moving files are `human_session.rs`, command Session
text, the public cutover test, Desktop state views and docs. Exact replay already
preserves main's controls/bind/interactive-all edits. Do not apply the released
`.lf/tmp/session-summary-proposal` cache patch alongside this replacement.

Use main's serialized isolated runner, private Homes and existing resource bounds:

```sh
uv run python .lf/tmp/cut-i/run.py metadata-unit cargo test -p loopflow -j 4 --lib session_metadata -- --test-threads=1
```

Five authored Rust tests cover missing/corrupt detail while exact reads still
fail; scope/order/literal search before decoding; receipt replay/conflict;
retained ancestry with unknown placement/occurrence and feedback; and the shared
wire case. They are **proposed source, not executed results**.

Run the existing public `session_cutover_tests` cases
`inventory_scopes_before_paging_and_keeps_worktree_repository_identity`,
`interactive_session_is_rows_from_launch_to_completion`,
`ask_session_is_rows_from_request_to_answer`, and
`session_list_reads_a_taskless_review_from_sql`, plus affected exact-action tests
in `session_cli_tests`. Reuse main's unchanged native proof; no provider experiment
is required merely for this projection. Exercise supported source and materialized
migration variants, including populated index backfill and unchanged import
conflict/history/Started assertions. The Python schema replay does not substitute
for that runner or bundled SQLite plan proof.

Run affected Swift DTO/RegistryQuery/SessionControls/navigation/rename tests and
the retained mounted terminal proof. Verify Unknown wire round-trip, all/headless
mode, last-good inventory, same Session/terminal/draft after refresh and identical
captured diagram navigation. Then formatting/all-target Clippy, migration and
architecture checks, and regenerate the website docs through its existing
`dev.py sync-docs` path. No generated website files were written here.

## Boundaries retained

Names, exact readiness text and iteration tuples remain variable-size row
metadata. Listing still performs exact local receipt/OS checks and prepares
Home-aware command arguments for selected rows. This proposal removes capture/
history work; it does not claim constant bytes, bounded OS cost, dense latency
budgets or a final lightweight Desktop wire. Full saved-Flow inventory/paging,
fixed-dataset paging proof, dense cold/warm measurements and configured Desktop/
provider/installed acceptance remain separate. It does not resolve unknown native
identity or import missing evidence merely to paint a complete row.

Review found and fixed three concrete issues before handback: expression-index
outer-join fallback, missing exhaustive Swift membership handling, and a fixture
that inferred Closed from native history after client exit. No product ambiguity
requires a new decision to review this patch.
