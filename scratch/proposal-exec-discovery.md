# Proposal: typed Exec discovery

2026-09-29 · LOO-298 · Prepared for Jack Heart. **UNAPPLIED, UNCOMPILED, UNTESTED in Rust.**
Main owns source integration, builds/tests and Git. This contribution writes only
this artifact and `.lf/tmp/exec-discovery-proposal/`. No branch executable, Home,
provider, PM mutation, Git mutation or delegation was used.

## Scope and implementation

Reuse `Exec` for summary and exact detail; its existing wire fields are unchanged.
Add `ExecFilter`, `ExecCursor`, `ExecPage`, typed outcome/work filters and the
existing Store/SqliteStore query adapters. These are query values, not lifecycle
objects. Four files; no migrations, indexes, CLI naming, Swift or native changes.

- Filters: exact Exec ID, recorded repository, direct parent, calling Session,
  literal command/identity containment, terminal outcome and performed Task/Wave.
  Caller identity and command context remain distinct from performed work.
- Work selection unions native `session_events.kind='started'` Exec references
  with mechanical `flow_events.kind='operation_started'` Exec references through
  the owning Flow. Deduplicate before paging. One Exec may match several Tasks.
  Never infer work from current Session binding, driver, process ancestry or PRs.
- A materialized ID page precedes Exec decoding and command-Wave enrichment.
  Decode at most `limit + 1` rows; return at most `limit`, plus a continuation when
  the lookahead exists. No Session payload, capture, manifest or transcript is read.
  Exact lookup and page projection share one row decoder and command-context query.
- Nonzero page size is explicit. Order is `started_at DESC, id ASC`, matching the
  existing recency index. Cursor comparison retains timestamp ties. No offset or
  unlimited sentinel is added. This is an implementation proposal, not Jack's
  approval of a final public paging contract.
- Cursor pages are stable for fixed data and unaffected by Session renames.
  They are not a snapshot across writes: late imports or changed outcome filters
  can change membership. Reuse filters while continuing; refresh from page one
  for changed data. Consumers must not treat a partial page as complete inventory.
- Contains uses `instr`, preserving literal `%`/`_`; command folding uses SQLite
  `lower()` (ASCII behavior, not Unicode case folding). Identity containment is
  case-sensitive. Exact/prefix resolution is separate: exact wins, prefix uses an
  indexed UUID-text range with a two-ID ambiguity check, absence returns `None`.
- `Unknown` outcome means SQL NULL, never Running. Preserve parent, `via_agent`,
  caller generation/token, outcome, exit and signal missingness exactly. No query
  grants provider/Flow authority or uses completion to establish process death.

## Evidence boundaries and review

Native origin is recorded by `record_session_turn_origin`; its start row retains
original Task/Wave. Imported observations and missed starts without an exact Exec
reference cannot supply that relation. Known Exec rows remain in unfiltered
history; observations without an Exec cannot manufacture a row or a Task match. This does not prove
complete all-provider work attribution: direct-provider observations can lack a
native start/Exec relation. No artifact/current-driver fallback is proposed.

Mechanical events reference their Flow's Task/Wave; current store writes preserve
that captured ownership and historical import compares it. No accepted whole-Flow
binding operation exists. A future reattribution API would need event ownership
preservation before this join could remain historically correct. Unknown imported
mechanical Execs remain NULL and cannot manufacture commands.

Review kept the existing narrow command-Wave lookup, ignored Skill trace labels,
and moved it after the bounded ID page. The query has no payload hydration or artifact I/O, but command
argv strings themselves have no byte limit. Work matching and contains scans are
not bounded CPU work merely because returned/enriched rows are bounded.

## Actual validation

Private Rust files passed `rustfmt --edition 2021 --config skip_children=true`
and `--check`: syntax/formatting only. All **7 hunks across 4 files** passed an
independent count/old-context/in-memory replay check; output matches proposed
bytes. Read-only `git apply --check` passes. No shared source was patched.

`probe.py` replays 55 canonical SQL files plus 31 dependency-ordered drafts into
Python SQLite 3.50.4 memory, then uses the proposal's extracted SELECT/work SQL
and literal fixture seeds. Foreign keys remain enabled during seeds; final check
is empty. Task deduplication and unknown-owner exclusion assertions pass there.
Two earlier probe attempts stopped on ambiguous statement extraction in the
helper; narrowing extraction to the test module fixed setup, without a product
change. This is SQL-only validation, not application initialization/migration proof.

The private fixture has **10,004 Execs, 2 Sessions, 1 Flow, 2 Tasks**. Plans show
recency/cursor index access, parent equality lookup, and indexed prefix ranges.
Work uses the existing Task/Flow indexes and scans the partial native-start index;
contains scans eligible Exec rows. Temporary sorts remain, including parent ties
and command-context selection. No new index is proposed from this small fixture.
`probe-results.json` retains plans/SQL and single-fetch timings; these are neither
cold/warm application latency nor representative history-density acceptance.

Four unapplied Rust proofs cover: exact/null command outcomes and causal fields;
literal search and exact/prefix ambiguity; filtered fixed-data pages with ties;
multiple Sessions/Tasks plus mechanical work deduplicated before paging, with
pre-bind/unknown Exec evidence excluded. The work fixture includes unreadable
history bodies and a schema-valid incomplete capture, requiring no hydration.
No immutable artifact directory exists. Main must execute these proofs.

## Patch and fingerprints

Apply hunks only from `.lf/tmp/exec-discovery-proposal/exec-discovery.patch`.
Patch SHA-256: `33f0f1496e10182e332129b195c78c1b1ad8296db48bf439494d6d8633052dd9`.
Size: 27,643 bytes. Private proposed full files are not integration replacements.
Initial/closing HEAD: `91e0215c17f97c0616d0e138b66d0c16161b650a`.
No drift among 98 captured source/context/SQL files at validation close; main's
concurrent numeric-wire/landing edits outside that set remain untouched.
`initial.json` and `validation.json` retain all hashes, replay results and counts.

| Patched source | Initial = closing SHA-256 |
| --- | --- |
| `rust/loopflow/src/exec.rs` | `5fae6bf05f52d0f78d2ea76a3799a4ec475d9eee492f3786c70bd114f2b7728b` |
| `rust/loopflow/src/store/sqlite/execs.rs` | `f1552cc5efa2357a9097da3fd704986c115540457134494b5991fa65bc2f41bc` |
| `rust/loopflow/src/store/mod.rs` | `d6658fb6ace5a765ecbbd93529f8e6bbf754c4e989278e9dd03d043c874f43a4` |
| `rust/loopflow/src/store/execs.rs` | New file; absent at closing check |

Production delta: **+261 / −33 = net +228**, against saved originals, excluding
the appended test module; no renames. This is proposal cost, not whole-branch size.

## Main proof commands and remaining consumers

After reviewing/applying only these hunks in the serialized build slot:

```sh
uv run python scripts/resource_envelope.py
uv run python .lf/tmp/cut-i/run.py exec-discovery-store cargo nextest run -p loopflow --lib --no-fail-fast -E 'test(exec_discovery_) | test(activity_reads_exact_exec_rows_without_replaying_command_events)'
uv run python .lf/tmp/cut-i/run.py exec-discovery-clippy cargo clippy --all-targets -- -D warnings
cargo fmt --check
```

No SQL migration change requires a new canonical matrix just for this proposal.
Rust compilation/tests, bundled-SQLite plans and combined-byte behavior remain
unproved. CLI selectors/public Task-ID resolution, final wire/Swift consumers,
Desktop refresh reconciliation, representative dense timing and summary byte-cost
measurements remain main's separate work. Existing native decision failures and
configured/installed acceptance are unchanged. No code-complete claim follows.
