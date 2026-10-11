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

Admission and repository-scoped selector repairs are proved locally, not installed.
Exact IDs retain their repository; conflicting explicit Wave scope refuses.
Details and counterexamples: `1ac2bf694d:scratch/questions.md`.
Jack Heart requires removing `planning.rs`, not merely renaming its hierarchy.

## Git decoding repair — October 11

Unshipped compatibility decoding is removed; causal history and released SQLite
preservation remain required. Evidence: `1ac2bf694d:scratch/questions.md`.

## Next implementation boundary — October 11

Persistent-provider conversion and its consumers form the next unit in audit §§1–4;
narrower SQL extraction is preparatory. Acquisition rollback already compares Work,
provider input and Wave age, but populated released-0.13.11 conversion, independent
observations and delayed updates remain unproved. The design owns this boundary
and the subsequent journal, bindings, follow-up, cron, gate and review obligations.
