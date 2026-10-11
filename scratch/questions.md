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

October 11 operator review:
response structs/acceptance are now crate-private; wider Work edit/API ownership
remains unfinished. Relocation does not finish Jack Heart's deletion audit.

Review exposed stale-binary risk after moving CLI suites under `lib.rs`: deriving
`target/<profile>/lf` neither built nor refreshed it. The repair uses Cargo JSON
artifact output after an offline build in each fixture process; no guessed path.
Installation selectors target the moved library modules. The Flow-launch proof
passes using Cargo’s fresh artifact; no installation was performed.

Source repair: `acceptance::observe_task_state` now saves only revision/state/
completion evidence in new conflict receipts, matching acquisition. Historical
receipt bytes remain unchanged; 15 `task_completion_` checks pass (event 1828637).
Completed conversion findings: `b6df256d0f:scratch/questions.md`. Populated
Session-turn/live-control proof, typed journal, follow-ups, cron and final review
remain required by the design.
