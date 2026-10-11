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

## Remaining implementation — October 11

The PM facade, provider reconstruction, duplicate writer/journal vocabularies and
string/JSON content receipts are removed. Linear owns acquisition/projection;
WorkValue owns saved content and journal/receipt values. Creation retains captured
inputs and typed baselines, never reconstructing an attempt from current Work.
Historical intent state UUIDs cannot establish semantic state. Ordering/deletion
keep separate effect semantics. Full follow-up lifecycle removal, cron conversion
and conditional routing-inventory reduction remain before gate/republication.
Earlier store-owner and acquisition findings: `216668cd9b:scratch/questions.md`.

Populated migration/import proof retains Session turns, reservations, claims,
activity and uncertain controls. Stored preservation is not configured provider
control or installation acceptance. LOO-414's first-start authority remains open;
no missing-placement inference or LOO-427 landing authority is introduced.

## Receipt review — October 11

The field-attempt deletion check is restored inside its transaction, with a
select/edit/delete/attempt regression. Creation acquisition now resolves Linear
Project IDs to canonical Work IDs before matching typed baselines; its regression
retains a pre-creation membership receipt with distinct identities. Partial
assignee omission supplies no invented null observation. Released conversion retains
explicit-null membership separately from an absent baseline; older acknowledgment
rebases a later save without clearing it. The pursuit design owns check results.
