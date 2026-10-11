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

Persistence conversion is checkpointed at `fbc9c60e62`. The next source cut
removes production snapshot/detail envelopes and generic-provider acceptance APIs;
43 focused preservation/CLI checks pass. Concrete acceptance methods and
`PmItem`/`LinearProject` still escape the adapter. Confine those inputs and delete
the remaining PM hierarchy before the typed-edit/journal, follow-up, cron and
review obligations. The accepted HTML permits private Linear responses; moving
or renaming public models alone is insufficient. Broader independent/delayed
observations, Session turns and live-control preservation remain for gate.

Conversion review repairs: confirming refresh compares the retained claim;
membership/assignee receipt baselines preserve omitted versus explicit null.
Released-frontier Rust regressions pass (Session history events 1823411/1823459).
Wrong scalar types now reject conversion; original rows/schema survive rollback.
Independent SQL rechecks: `/tmp/loo412-membership-comparison-recheck.json` and
`/tmp/loo412-observation-type-conversion-recheck.json`; neither is public CLI proof.
Counterexamples remain in the corresponding `-proof.json` files.
The design's released-frontier section owns populated-fixture coverage and the
remaining Session-turn/live-control preservation obligations.
