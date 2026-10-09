# Implementation choices — LOO-412, 2026-10-08

Jack Heart's custom-ref steers supersede host callbacks. The
[current design](work-on-another-machine-name.md) records the accepted boundary,
deletion cut and remaining acceptance. Earlier callback assumptions are retained
at `b040c7c8d:scratch/questions.md`.

- LOO-406's common writer is integrated from published `e68f2a423`. Peer
  import/export and ordering exist in source; foreground composition remains
  LOO-412's work, independent of the parent's unfinished Linear delivery.
- Follow Jack Heart's newer policy: Linear wins observed conflicts; otherwise host
  preference where appropriate, then last-write-wins with retained losing edits.
  Use user-keyed destination selection by default, explicit opt-in shared planning.
  Exact stable key setup and deterministic ties are implementation choices here.
- Planning remote selection must be explicit before ordinary commands can publish;
  no real planning data goes to the public code remote. Current transport tests
  use synthetic documents and disposable local remotes.
- Implementation choice: causality precedes a hybrid logical clock and stable
  change-ID tie-break. No initiating host has blanket preference over collaborators;
  positive Linear-origin observations win concurrent peer writes. Preserve losing
  mutations. Store APIs now provision/recover an explicit user key and bind an
  empty destination; public setup and active-plan selection remain unimplemented.
- `new_migration.py` identified the inherited LOO-406 draft as this Task's draft.
  Git's explicit stack boundary establishes that it belongs to the parent;
  LOO-412 therefore has one separate `planning_peers.sql` draft depending on it.
- October 8 implementation choice: project each object under a savepoint; retain
  expected constraint failures and their mutations while independent objects commit.
  Wave selection follows Project projection to break their reference cycle. Invalid
  documents, reused mutation IDs, foreign repository ownership and unexpected SQL
  failures still roll back the import. Legacy-provider association remains unfinished;
  conflicts preserve both IDs rather than guessing which execution history to use.

The source/destination primitives remain unpublished. Current verification and
remaining integration are in the existing design; previous adoption-only evidence
is retained at `5d336868f:scratch/questions.md`.

- October 8 selection choice: an explicit Wave selection includes its descendants;
  new descendants inherit membership, but binding/joining selects no existing work.
  A record belongs to one destination locally. Cross-plan moves or overlapping IDs
  fail without changing either selection; no implicit sharing or renumbering.
  Root-Wave creation and public setup still need active-plan routing. The endpoint
  is pinned at binding, not re-resolved from a mutable remote alias on publication.
