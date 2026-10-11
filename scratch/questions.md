# Implementation choices — LOO-412

[Design](work-on-another-machine-name.md). Earlier choices:
`ae96102580e442fe582dd17ce5e6fef9b8cee69d:scratch/questions.md`; full October 10
prose/sync evidence: `eab40bad8c:scratch/questions.md`.

- Personal keys need recovery on fresh Machines; automatic account pairing is absent.
  Repository configuration grants shared-ref permission, not Machine membership.
- Git-to-Linear switching retains records/receipts. Automatic export of unmapped
  imported Work needs design; uncertain effects stay unresolved.
- Causality precedes clocks and mutation-ID ties; preserve losers. No host preference.
- Imports preserve local execution. Missing placement proves no peer absence;
  distributed first-start authority belongs elsewhere.
- `planning_peers.sql` is LOO-412's sole draft. LOO-406/optional-PR is released in
  `0.13.11.001_release.sql`; main's separate `process_terminal.sql` is retained.
  Use disposable stores, never branch binaries against the installed Home.
- Prose replacement rejects live managed declarations/unclosed fences; closed fenced
  examples work. Ordinary saves trim boundaries/join prose with blank lines;
  migration retains bytes. Partial observations retain field revision/age. These
  choices have no UX approval.

## Repaired findings — f025e9b7ab

Admission now retains independent Project disputes when the issue payload is absent;
populated tests cover disputed/archived parents. Repository-scoped CLI status,
comments and edits pass the original duplicate-identifier fixture, including aliases.
Exact IDs route to their retained repository; conflicting explicit Wave scope refuses.
Evidence: `local_planning.rs`, `/tmp/loo412-selector-repro-result.json` and
`/tmp/loo412-admission-parent-evidence-proof.txt`. Source/disposable proofs, no install.
Jack Heart requires removing `planning.rs`, not merely renaming its hierarchy.

## Remaining review checks

Snapshot-reader deletion must retain rollback proof for canonical Project,
relationship/observation age and Wave acquisition age, including delayed updates.
Current assertions cover Task/PR only; check dedicated coverage before final gate.
This is a coverage concern, not observed data loss.

The stale `pm_show.json` documentation is removed. The Git-miss regression now
asserts no Linear refresh error as well as Unavailable; its focused rerun passed.

## Git decoding review — October 11

Compression adds a `PlanningSnapshot::from_bytes` decoder accepting old stringified
Teams/Initiatives/authors alongside typed values. The peer journal exists only in
this Task's unreleased `planning_peers.sql`; no published Git format or configured
retained-data requirement has been established. Jack Heart excludes older-version
support; AGENTS rejects compatibility for internal unshipped formats. Remove this
old-format branch and its compatibility-only fixture unless concrete retained data
requires conversion. Preserve released SQLite relationships, authors and uncertain
receipts through the finished draft; preserve causal/losing values in the selected
format. Those obligations do not by themselves justify a permanent dual decoder.
