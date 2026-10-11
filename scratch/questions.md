# Implementation choices — LOO-412

[Design](work-on-another-machine-name.md). Earlier choices/proofs:
`17314daeb5:scratch/questions.md`.

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

Jack Heart requires removal of the hierarchy, not renaming.

Connect selects repository Linear and retains its Team; Git-miss lookup respects
that transport without allocating Work. Reteam preserves unrelated edits and
creates no cleanup commit. Exact source proofs and prior counterevidence:
`4e78739a9f:scratch/questions.md`; live Linear/SSH and installation remain unproved.

## Next implementation boundary — October 11

The generic PM facade and nested Wave context are removed; ordinary Task
lookup/comments and foreground exchange now have domain/planning owners. Git
misses stay on Git. Linear refresh retains unmapped local Projects without lookup.
Populated migration/import fixtures now cover Session turns, capture reservations,
attachment claims/activity and an uncertain local cancellation receipt. This proves
stored preservation, not configured provider control.
The typed local writer/journal now shares WorkEdit. Linear receipt/creation baseline reduction,
historical follow-through effects and cron conversion remain before gate/republication. Earlier acquisition/fixture review findings:
`1930805f40:scratch/questions.md`; no installation or stopped-Flow replay occurred.

## Resolved store-owner review and remaining cut — October 11

The moved `ops/task.rs::resolve_saved_task` now uses the shared fallible
`planning_sync::planning_store`. This restores the previous store-opening semantics;
source readback confirms the repair. CLI admission can initialize a fresh registry,
so the earlier finding never established a public cold-start failure.

Both `planning.rs` modules are deleted. Linear acceptance owns projection,
Linear observations own invalidation/evidence, and field delivery owns accepted
revision eligibility. The journal stores typed mutations directly; receipt
translation and generic creation baselines still need deletion. Module removal
alone does not complete Jack Heart's audit.
