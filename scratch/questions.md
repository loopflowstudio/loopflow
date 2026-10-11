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
remain before gate/republication; routing inventories are removed.

Populated migration/import proof retains Session turns, reservations, claims,
activity and uncertain controls. Stored preservation is not configured provider
control or installation acceptance. LOO-414's first-start authority remains open;
no missing-placement inference or LOO-427 landing authority is introduced.

## Receipt review — October 11

`52f7f3f99f` preserves transactional deletion eligibility, canonical creation
membership, partial assignee omission, explicit-null baselines and later saves
across older acknowledgment. Focused regressions pass; the design owns check results.

## Routing reduction review — October 11

Both routing inventories are now removed in source; preservation and transport
checks pass, including headless configuration refresh. Nextest additionally reported
leaks in `provider_columns_preserve_released_observations_and_local_execution` and
`observed_reopening_beats_delayed_completion`; retain both for gate alongside the
five earlier observations. Passing assertions do not prove handle cleanup.

## Stopped Flow — October 11

Flow `87ecd305-cafb-450f-8292-58101aadad4b` exited 1 during realign after Codex
reported its usage limit. Implement, compress and sync succeeded; source through
`47b5edb701` and local designs survive. Realign read sources and renamed its Session; no replay or delivery occurred.
