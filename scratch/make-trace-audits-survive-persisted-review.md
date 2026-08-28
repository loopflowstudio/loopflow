# Gate review: persisted capture schemas

## What was implemented

Candidate promotion now audits every complete legacy conversation artifact
named by a SQLite-consistent Home snapshot. It reads historical schema-v1
`usage` and `turn_usage` records through one exact normalization boundary,
reports partial and broken captures separately, then applies candidate
migrations to the snapshot and validates executable lifecycle references.

The active Run-record model remains unchanged. The legacy artifact reader is
module-private and exists only for candidate promotion; no legacy writer or
ordinary CLI reader was restored.

## Key choices

- Decode the current schema first. Historical normalization runs only when the
  current DTO rejects an event, so `usage_checkpoint` remains the active
  vocabulary.
- Accept only the six field sets observed on the long-lived Home. Unknown
  fields, null usage values, corrupt JSON, and unknown schema versions fail
  closed rather than receiving defaults.
- Audit legacy captures before a candidate migration may retire
  `agent_invocations`, then validate executable references in the resulting
  candidate schema. Both results derive from one checked snapshot.
- Keep partial rows visible but outside the complete-capture failure set. A
  partial capture is retained evidence, not fabricated readable history.
- Return only `Complete` or `Truncated` from the artifact reader. Decoded events
  are validated and discarded because promotion does not consume transcripts.

## How it fits together

```text
Home SQLite -> consistent copy -> audit complete legacy artifacts in Home/traces
                               -> apply candidate migrations
                               -> resolve executable lifecycle references
                               -> CandidateCompatibility -> promotion verdict
```

`lf doctor` continues to audit current Run records. Candidate promotion is the
only production caller of the versioned legacy reader.

## Risks and bottlenecks

- Capture validation is linear in complete artifacts. The current Home scan
  reads 3,681 files; this is acceptable for a release/promotion boundary, not a
  hot command path. Revisit bounded sampling only after measured disk or time
  pressure makes the full proof impractical.
- SQLite is copied consistently, while artifacts are read from the real Home
  trace root. Complete artifacts are treated as immutable; concurrent capture
  locking remains LOO-238's scope.
- Schema version 1 is executable and unknown versions fail closed. A future
  schema version must add an explicit reader branch before promotion can accept
  it.
- A validation-only source build still refuses global promotion. The current
  Home also has unrelated pending development migrations; neither refusal is a
  capture-compatibility failure.

## What's not included

- Exact context manifests (LOO-143), replay (LOO-129), trace metrics (LOO-138),
  and concurrent capture locking (LOO-238).
- A legacy capture writer, a legacy `lf doctor` path, or a second trace store.
- Compression, rotation, remote telemetry, or cross-machine artifact replay.

## Validation

Gate checks on head `359042a139906d596a753b61cb2b0cf257d453f2`:

- `cargo fmt --all -- --check` — pass.
- `cargo clippy --all-targets -- -D warnings` — pass.
- `cargo test -p loopflow --lib conversation_reader -- --nocapture` — 5 pass.
- `cargo test -p loopflow --lib compatibility_tests -- --nocapture` — 5 pass.
- `cargo test -p loopflow --test local_promotion -- --nocapture` — 4 pass.

The real Home proof from the immediately preceding review was reused because
the tracked and untracked tree and the command plan were identical: 3,681 of
3,681 complete captures readable, 71 partial captures retained separately, and
98 executable lifecycle references resolved. Before restoration, the installed
reader reported 1,770 capture integrity failures on the same long-lived Home.

CI owns the full all-target test matrix.
